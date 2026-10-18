//! Finding the editors installed on this machine and starting one on a path (t3code's
//! `resolveEditorCommand` and `launchEditor`, `packages/shared/src/editor.ts`).

use std::{
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use agentz_protocol::editors::{EDITORS, Editor, find};
use anyhow::{Context as _, Result, bail};
use util::ResultExt as _;

/// What an editor's command is looked for in.
pub struct Environment {
    /// The directories of `PATH`.
    path: Vec<PathBuf>,
    home: Option<PathBuf>,
    data_home: Option<PathBuf>,
}

impl Environment {
    pub fn current() -> Self {
        let home = std::env::var_os("HOME").map(PathBuf::from);
        let data_home = std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .or_else(|| Some(home.as_ref()?.join(".local/share")));
        Self {
            path: std::env::var_os("PATH")
                .map(|path| std::env::split_paths(&path).collect())
                .unwrap_or_default(),
            home,
            data_home,
        }
    }
}

/// The ids of the installed editors, in the order of [`EDITORS`].
pub fn installed(env: &Environment) -> Vec<String> {
    EDITORS
        .iter()
        .filter(|editor| resolve(editor, env).is_some())
        .map(|editor| editor.id.to_string())
        .collect()
}

/// Starts the editor on `path`, detached, so it outlives the server and its output goes
/// nowhere.
#[allow(
    clippy::disallowed_methods,
    reason = "only forks the editor, which a thread then waits for"
)]
pub fn launch(env: &Environment, editor_id: &str, path: &Path, line: Option<u32>) -> Result<()> {
    let editor = find(editor_id).with_context(|| format!("unknown editor “{editor_id}”"))?;
    let command = resolve(editor, env)
        .with_context(|| format!("{} isn't installed on this machine", editor.label))?;
    if !path.exists() {
        bail!("{} doesn't exist", path.display());
    }
    let mut process = Command::new(&command);
    process
        .args(editor.base_args)
        .args(editor.launch_args(&path.to_string_lossy(), line))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(unix)]
    std::os::unix::process::CommandExt::process_group(&mut process, 0);
    let mut child = process
        .spawn()
        .with_context(|| format!("couldn't start {}", command.display()))?;
    // A command that stays until the editor closes (JetBrains') must not stay a zombie after.
    std::thread::spawn(move || {
        child.wait().log_err();
    });
    Ok(())
}

/// The command that starts the editor: on `PATH`, or where its installer puts it.
fn resolve(editor: &Editor, env: &Environment) -> Option<PathBuf> {
    for command in editor.commands {
        for directory in &env.path {
            let candidate = directory.join(command);
            if is_executable(&candidate) {
                return Some(candidate);
            }
        }
    }
    installed_commands(editor, env)
        .into_iter()
        .find(|candidate| is_executable(candidate))
}

/// Where the editor's installer leaves its command when it isn't on `PATH`, which a program
/// started from the desktop often doesn't see.
fn installed_commands(editor: &Editor, env: &Environment) -> Vec<PathBuf> {
    let is_jetbrains = editor.launch == agentz_protocol::editors::LaunchStyle::LineColumn;
    let command = editor.commands[0];
    let mut candidates = Vec::new();
    if cfg!(target_os = "macos") {
        let roots = env
            .home
            .iter()
            .map(|home| home.join("Applications"))
            .chain([PathBuf::from("/Applications")]);
        for root in roots {
            let entries: Vec<String> = std::fs::read_dir(&root)
                .map(|entries| {
                    entries
                        .filter_map(|entry| entry.ok()?.file_name().into_string().ok())
                        .collect()
                })
                .unwrap_or_default();
            for entry in entries {
                if !editor
                    .mac_apps
                    .iter()
                    .any(|name| is_bundle_of(&entry, name))
                {
                    continue;
                }
                let contents = root.join(entry).join("Contents");
                if is_jetbrains || editor.id == "zed" {
                    let binary = if editor.id == "zed" { "cli" } else { command };
                    candidates.push(contents.join("MacOS").join(binary));
                } else {
                    candidates.push(contents.join("Resources/app/bin").join(command));
                    candidates.push(contents.join("Resources/app/bin/code"));
                }
            }
        }
        if let Some(home) = &env.home
            && is_jetbrains
        {
            candidates.push(
                home.join("Library/Application Support/JetBrains/Toolbox/scripts")
                    .join(command),
            );
        }
    } else {
        let mut directories: Vec<PathBuf> = env
            .home
            .iter()
            .map(|home| home.join(".local/bin"))
            .chain(
                ["/usr/local/bin", "/usr/bin", "/snap/bin"]
                    .into_iter()
                    .map(PathBuf::from),
            )
            .collect();
        if is_jetbrains && let Some(data_home) = &env.data_home {
            directories.push(data_home.join("JetBrains/Toolbox/scripts"));
        }
        for directory in directories {
            for command in editor.commands {
                candidates.push(directory.join(command));
            }
        }
    }
    candidates
}

/// Whether the application folder `entry` is `<name>.app`, or `<name> <version>.app` as
/// JetBrains' Toolbox names them. A plain prefix would let VS Code take Insiders' bundle.
fn is_bundle_of(entry: &str, name: &str) -> bool {
    let Some(stem) = entry.strip_suffix(".app") else {
        return false;
    };
    if stem == name {
        return true;
    }
    stem.strip_prefix(name)
        .and_then(|rest| rest.strip_prefix(' '))
        .is_some_and(|version| {
            !version.is_empty() && version.chars().all(|c| c.is_ascii_digit() || c == '.')
        })
}

fn is_executable(path: &Path) -> bool {
    let Ok(metadata) = std::fs::metadata(path) else {
        return false;
    };
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        metadata.is_file() && metadata.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        metadata.is_file()
    }
}

#[cfg(test)]
impl Environment {
    /// Only these directories, as `PATH`.
    fn only(path: Vec<PathBuf>) -> Self {
        Self {
            path,
            home: None,
            data_home: None,
        }
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt as _;

    fn script(directory: &Path, name: &str, body: &str) {
        let path = directory.join(name);
        std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).expect("a script");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))
            .expect("made executable");
    }

    #[test]
    fn the_installed_editors_are_those_with_a_command_on_the_path() {
        let bin = tempfile::tempdir().expect("a directory");
        script(bin.path(), "zeditor", "");
        script(bin.path(), "code", "");
        // Not executable, so it doesn't count.
        std::fs::write(bin.path().join("cursor"), "").expect("a file");
        let env = Environment::only(vec![bin.path().to_path_buf()]);
        assert_eq!(installed(&env), ["vscode", "zed"]);
        assert_eq!(
            installed(&Environment::only(Vec::new())),
            Vec::<String>::new()
        );
    }

    #[test]
    fn launching_gives_the_editor_the_path_and_line() {
        let bin = tempfile::tempdir().expect("a directory");
        let out = bin.path().join("args");
        script(
            bin.path(),
            "code",
            &format!("echo \"$@\" > '{}'", out.display()),
        );
        let env = Environment::only(vec![bin.path().to_path_buf()]);
        let folder = tempfile::tempdir().expect("a folder");
        launch(&env, "vscode", folder.path(), Some(7)).expect("it starts");
        let mut args = String::new();
        for _ in 0..100 {
            args = std::fs::read_to_string(&out).unwrap_or_default();
            if !args.is_empty() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        assert_eq!(args.trim(), format!("--goto {}:7", folder.path().display()));

        let error = launch(&env, "zed", folder.path(), None).expect_err("zed isn't installed");
        assert_eq!(error.to_string(), "Zed isn't installed on this machine");
        let error = launch(&env, "vscode", &folder.path().join("gone"), None)
            .expect_err("the folder is gone");
        assert!(error.to_string().ends_with("doesn't exist"));
        let error = launch(&env, "notepad", folder.path(), None).expect_err("an unknown editor");
        assert!(error.to_string().starts_with("unknown editor"));
    }

    #[test]
    fn a_bundle_is_the_editors_own_not_a_longer_name() {
        assert!(is_bundle_of("Visual Studio Code.app", "Visual Studio Code"));
        assert!(!is_bundle_of(
            "Visual Studio Code - Insiders.app",
            "Visual Studio Code"
        ));
        assert!(is_bundle_of("IntelliJ IDEA 2026.1.4.app", "IntelliJ IDEA"));
        assert!(!is_bundle_of("IntelliJ IDEA CE.app", "IntelliJ IDEA"));
        assert!(!is_bundle_of("Zed", "Zed"));
    }
}
