//! The editors a thread's folder opens in (t3code's `EDITORS`, `packages/contracts/src/editor.ts`):
//! how each is started, and how an editor on this machine opens a folder on another over SSH.
//! A server finds which are installed on its machine ([`crate::Request::ListEditors`]) and starts
//! one ([`crate::Request::OpenInEditor`]).

/// How an editor's command takes a line to open a file at.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LaunchStyle {
    /// The path alone, as Zed's `zed path` (Zed takes `path:line` in it).
    Path,
    /// VS Code's: `--goto path:line`.
    Goto,
    /// JetBrains': `--line n path`.
    LineColumn,
}

/// The deep link an editor opens a folder over SSH from, which the OS hands to it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RemoteLink {
    /// `<scheme>://vscode-remote/ssh-remote+<host><path>`, for VS Code and its forks that ship
    /// Remote - SSH.
    VsCode(&'static str),
    /// `zed://ssh/<host><path>`.
    Zed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Editor {
    /// What requests and settings call it.
    pub id: &'static str,
    pub label: &'static str,
    /// The commands that start it, in the order to try them.
    pub commands: &'static [&'static str],
    /// What comes before the path in every launch.
    pub base_args: &'static [&'static str],
    pub launch: LaunchStyle,
    pub remote: Option<RemoteLink>,
    /// The names of its application on macOS, where its command may not be on `PATH`.
    pub mac_apps: &'static [&'static str],
}

const fn editor(
    id: &'static str,
    label: &'static str,
    commands: &'static [&'static str],
    launch: LaunchStyle,
) -> Editor {
    Editor {
        id,
        label,
        commands,
        base_args: &[],
        launch,
        remote: None,
        mac_apps: &[],
    }
}

/// The editors agentZ knows, in the order a menu lists those that are installed.
pub const EDITORS: &[Editor] = &[
    Editor {
        // File and workspace opens must target the IDE even when the Agents Window is active.
        base_args: &["--classic"],
        remote: Some(RemoteLink::VsCode("cursor")),
        mac_apps: &["Cursor"],
        ..editor("cursor", "Cursor", &["cursor"], LaunchStyle::Goto)
    },
    Editor {
        remote: Some(RemoteLink::VsCode("vscode")),
        mac_apps: &["Visual Studio Code"],
        ..editor("vscode", "VS Code", &["code"], LaunchStyle::Goto)
    },
    Editor {
        remote: Some(RemoteLink::VsCode("vscode-insiders")),
        mac_apps: &["Visual Studio Code - Insiders"],
        ..editor(
            "vscode-insiders",
            "VS Code Insiders",
            &["code-insiders"],
            LaunchStyle::Goto,
        )
    },
    Editor {
        remote: Some(RemoteLink::VsCode("vscodium")),
        mac_apps: &["VSCodium"],
        ..editor("vscodium", "VSCodium", &["codium"], LaunchStyle::Goto)
    },
    Editor {
        remote: Some(RemoteLink::Zed),
        mac_apps: &["Zed", "Zed Preview"],
        ..editor("zed", "Zed", &["zed", "zeditor"], LaunchStyle::Path)
    },
    Editor {
        mac_apps: &[
            "IntelliJ IDEA",
            "IntelliJ IDEA CE",
            "IntelliJ IDEA Ultimate",
        ],
        ..editor("idea", "IntelliJ IDEA", &["idea"], LaunchStyle::LineColumn)
    },
    Editor {
        mac_apps: &["WebStorm"],
        ..editor(
            "webstorm",
            "WebStorm",
            &["webstorm"],
            LaunchStyle::LineColumn,
        )
    },
    Editor {
        mac_apps: &["PyCharm", "PyCharm CE"],
        ..editor("pycharm", "PyCharm", &["pycharm"], LaunchStyle::LineColumn)
    },
    Editor {
        mac_apps: &["GoLand"],
        ..editor("goland", "GoLand", &["goland"], LaunchStyle::LineColumn)
    },
    Editor {
        mac_apps: &["RustRover"],
        ..editor(
            "rustrover",
            "RustRover",
            &["rustrover"],
            LaunchStyle::LineColumn,
        )
    },
    Editor {
        mac_apps: &["CLion"],
        ..editor("clion", "CLion", &["clion"], LaunchStyle::LineColumn)
    },
    Editor {
        mac_apps: &["PhpStorm"],
        ..editor(
            "phpstorm",
            "PhpStorm",
            &["phpstorm"],
            LaunchStyle::LineColumn,
        )
    },
    Editor {
        mac_apps: &["RubyMine"],
        ..editor(
            "rubymine",
            "RubyMine",
            &["rubymine"],
            LaunchStyle::LineColumn,
        )
    },
    Editor {
        mac_apps: &["Rider", "JetBrains Rider"],
        ..editor("rider", "Rider", &["rider"], LaunchStyle::LineColumn)
    },
];

pub fn find(id: &str) -> Option<&'static Editor> {
    EDITORS.iter().find(|editor| editor.id == id)
}

impl Editor {
    /// Whether it opens a folder on another machine over SSH from a link
    /// ([`Self::remote_url`]).
    pub fn opens_over_ssh(&self) -> bool {
        self.remote.is_some()
    }

    /// What follows [`Self::base_args`] on its command line to open `path`, at `line` if the
    /// editor takes one.
    pub fn launch_args(&self, path: &str, line: Option<u32>) -> Vec<String> {
        match (self.launch, line) {
            (_, None) => vec![path.to_string()],
            (LaunchStyle::Path, Some(line)) => vec![format!("{path}:{line}")],
            (LaunchStyle::Goto, Some(line)) => vec!["--goto".into(), format!("{path}:{line}")],
            (LaunchStyle::LineColumn, Some(line)) => {
                vec!["--line".into(), line.to_string(), path.to_string()]
            }
        }
    }

    /// The link that opens `path` on the machine reached by the SSH `target` (a host, an alias
    /// from `~/.ssh/config`, `user@host`, or `ssh://user@host`), at `line` where the link takes
    /// one, as t3code's `buildRemoteOpenUrl` does. `None` for an editor with no such link.
    pub fn remote_url(&self, target: &str, path: &str, line: Option<u32>) -> Option<String> {
        let link = self.remote?;
        let host = encode_component(target.strip_prefix("ssh://").unwrap_or(target));
        let path = if path.starts_with('/') {
            path.to_string()
        } else {
            format!("/{path}")
        };
        let path = path
            .split('/')
            .map(encode_component)
            .collect::<Vec<_>>()
            .join("/");
        Some(match link {
            RemoteLink::VsCode(scheme) => {
                let line = line.map(|line| format!(":{line}")).unwrap_or_default();
                format!("{scheme}://vscode-remote/ssh-remote+{host}{path}{line}")
            }
            RemoteLink::Zed => format!("zed://ssh/{host}{path}"),
        })
    }
}

/// JavaScript's `encodeURIComponent`: everything but letters, digits and `-_.!~*'()`.
fn encode_component(text: &str) -> String {
    let mut encoded = String::with_capacity(text.len());
    for byte in text.bytes() {
        if byte.is_ascii_alphanumeric() || b"-_.!~*'()".contains(&byte) {
            encoded.push(byte as char);
        } else {
            encoded.push_str(&format!("%{byte:02X}"));
        }
    }
    encoded
}

#[cfg(test)]
mod tests {
    use super::*;

    fn editor(id: &str) -> &'static Editor {
        find(id).expect("a known editor")
    }

    #[test]
    fn ids_are_unique_and_every_editor_has_a_command() {
        for (index, editor) in EDITORS.iter().enumerate() {
            assert!(!editor.commands.is_empty(), "{}", editor.id);
            assert!(
                EDITORS[..index].iter().all(|other| other.id != editor.id),
                "{} is listed twice",
                editor.id
            );
        }
    }

    #[test]
    fn a_line_goes_the_way_the_editor_takes_it() {
        assert_eq!(editor("zed").launch_args("/a/b", None), ["/a/b"]);
        assert_eq!(editor("zed").launch_args("/a/b", Some(3)), ["/a/b:3"]);
        assert_eq!(editor("vscode").launch_args("/a/b", None), ["/a/b"]);
        assert_eq!(
            editor("vscode").launch_args("/a/b", Some(3)),
            ["--goto", "/a/b:3"]
        );
        assert_eq!(
            editor("webstorm").launch_args("/a/b", Some(3)),
            ["--line", "3", "/a/b"]
        );
    }

    #[test]
    fn remote_folders_open_from_the_editors_own_links() {
        assert_eq!(
            editor("vscode").remote_url("devbox1", "/home/ubuntu/api", None),
            Some("vscode://vscode-remote/ssh-remote+devbox1/home/ubuntu/api".into())
        );
        assert_eq!(
            editor("cursor").remote_url("ssh://ubuntu@devbox1", "/srv/my app", Some(12)),
            Some("cursor://vscode-remote/ssh-remote+ubuntu%40devbox1/srv/my%20app:12".into())
        );
        assert_eq!(
            editor("zed").remote_url("devbox1", "/home/ubuntu/api", Some(12)),
            Some("zed://ssh/devbox1/home/ubuntu/api".into())
        );
        assert_eq!(editor("webstorm").remote_url("devbox1", "/a", None), None);
        let remote: Vec<&str> = EDITORS
            .iter()
            .filter(|editor| editor.opens_over_ssh())
            .map(|editor| editor.id)
            .collect();
        assert_eq!(
            remote,
            ["cursor", "vscode", "vscode-insiders", "vscodium", "zed"]
        );
    }
}
