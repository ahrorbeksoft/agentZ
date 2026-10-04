//! Login pages for agents on a machine agentZ reaches over SSH. Agents log in by opening a page
//! in a browser, which on that machine is no browser the user sees (or none at all), and the page
//! sends its answer to `localhost` there. So agents get agentZ's own `xdg-open` (and `BROWSER`,
//! and `open` on a Mac), which hand the page to the server while the agent logs in. The clients
//! show it, and open it where the user is, forwarding its `localhost` port back over SSH. Outside
//! a login, the real program runs, as it would have.

use std::path::{Path, PathBuf};

use agentz_protocol::ConnectionId;
use anyhow::{Context as _, Result};

/// The variable naming the connection whose agent runs the program.
pub const CONNECTION_ENV_VAR: &str = "AGENTZ_CONNECTION";

/// The programs agents open pages with: `xdg-open` (Node's `open`, Rust's `open`, Go's
/// `pkg/browser`), the Debian alternatives, and on a Mac, `open`.
const PROGRAMS: &[&str] = &[
    "xdg-open",
    "x-www-browser",
    "www-browser",
    "sensible-browser",
    #[cfg(target_os = "macos")]
    "open",
];

/// Whether agents' pages go to the clients: this server runs on a machine reached over SSH.
/// Linux machines always are, since agentZ runs on Macs; a Mac is when SSH started the server.
pub fn hands_pages_to_clients() -> bool {
    cfg!(target_os = "linux") || std::env::var_os("SSH_CONNECTION").is_some()
}

/// Writes the programs into `directory`, each running `executable`'s `open-url`. Written at
/// every start, since an update may move the executable.
pub fn install(directory: &Path, executable: &Path) -> Result<()> {
    std::fs::create_dir_all(directory)
        .with_context(|| format!("creating {}", directory.display()))?;
    for program in PROGRAMS {
        let path = directory.join(program);
        let script = format!(
            "#!/bin/sh\n\
             # agentZ: hands a login page to the Mac you work from. See agentz-server open-url.\n\
             exec {} open-url --program {program} --skip {} -- \"$@\"\n",
            shell_quote(&executable.to_string_lossy()),
            shell_quote(&directory.to_string_lossy()),
        );
        write_executable(&path, &script)?;
    }
    Ok(())
}

fn write_executable(path: &Path, contents: &str) -> Result<()> {
    use std::os::unix::fs::PermissionsExt as _;
    let temporary = path.with_extension("tmp");
    std::fs::write(&temporary, contents)
        .with_context(|| format!("writing {}", temporary.display()))?;
    std::fs::set_permissions(&temporary, std::fs::Permissions::from_mode(0o755))
        .with_context(|| format!("making {} executable", temporary.display()))?;
    std::fs::rename(&temporary, path).with_context(|| format!("writing {}", path.display()))
}

/// The environment that puts the programs first for an agent of `connection`, or its login
/// terminal. `path` is the `PATH` it would have had.
pub fn agent_env(
    directory: &Path,
    connection: ConnectionId,
    path: Option<&str>,
) -> Vec<(String, String)> {
    let directory = directory.to_string_lossy().into_owned();
    let path = match path.filter(|path| !path.is_empty()) {
        Some(path) => format!("{directory}:{path}"),
        None => directory.clone(),
    };
    vec![
        ("PATH".to_string(), path),
        // Python's `webbrowser` and Rust's `webbrowser` try it first.
        ("BROWSER".to_string(), format!("{directory}/xdg-open")),
        (
            CONNECTION_ENV_VAR.to_string(),
            connection_to_string(connection),
        ),
    ]
}

pub fn connection_to_string(connection: ConnectionId) -> String {
    match connection {
        ConnectionId::Thread(thread_id) => format!("thread:{}", thread_id.0),
        ConnectionId::Account(account_id) => format!("account:{account_id}"),
    }
}

pub fn connection_from_string(text: &str) -> Option<ConnectionId> {
    let (kind, id) = text.split_once(':')?;
    let id = id.parse().ok()?;
    match kind {
        "thread" => Some(ConnectionId::Thread(projects::ThreadId(id))),
        "account" => Some(ConnectionId::Account(id)),
        _ => None,
    }
}

/// The page to hand over, when the program was asked to open just one web page, as logins do.
pub fn page_to_hand_over(arguments: &[String]) -> Option<&str> {
    match arguments {
        [url] if url.starts_with("https://") || url.starts_with("http://") => Some(url),
        _ => None,
    }
}

/// The program that `program` stands in for: the next one on `path` (a `PATH`) outside
/// `skip`.
pub fn real_program(program: &str, skip: &Path, path: &std::ffi::OsStr) -> Option<PathBuf> {
    let skip = skip.canonicalize().unwrap_or_else(|_| skip.to_path_buf());
    std::env::split_paths(path)
        .filter(|directory| {
            directory
                .canonicalize()
                .map_or(true, |directory| directory != skip)
        })
        .map(|directory| directory.join(program))
        .find(|candidate| candidate.is_file())
}

/// Quotes a word for a POSIX shell.
fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn connections_round_trip() {
        for connection in [
            ConnectionId::Thread(projects::ThreadId(12)),
            ConnectionId::Account(3),
        ] {
            assert_eq!(
                connection_from_string(&connection_to_string(connection)),
                Some(connection)
            );
        }
        assert_eq!(connection_from_string("thread:x"), None);
        assert_eq!(connection_from_string("pane:1"), None);
    }

    #[test]
    fn hands_over_single_web_pages() {
        let page = |arguments: &[&str]| {
            let arguments: Vec<String> = arguments.iter().map(|text| text.to_string()).collect();
            page_to_hand_over(&arguments).map(str::to_string)
        };
        assert_eq!(
            page(&["https://app.devin.ai/auth?state=1"]).as_deref(),
            Some("https://app.devin.ai/auth?state=1")
        );
        assert_eq!(page(&["/tmp/report.html"]), None);
        assert_eq!(page(&["-a", "Safari", "https://example.com"]), None);
        assert_eq!(page(&[]), None);
    }

    #[test]
    fn finds_the_program_it_stands_in_for() {
        let shims = tempfile::tempdir().expect("temp dir");
        let real = tempfile::tempdir().expect("temp dir");
        install(shims.path(), Path::new("/bin/agentz-server")).expect("installs");
        std::fs::write(real.path().join("xdg-open"), "").expect("writes");
        let script =
            std::fs::read_to_string(shims.path().join("xdg-open")).expect("the shim is written");
        assert!(script.contains("open-url --program xdg-open"));
        let path = std::env::join_paths([shims.path(), real.path()]).expect("joins");
        assert_eq!(
            real_program("xdg-open", shims.path(), &path),
            Some(real.path().join("xdg-open"))
        );
        assert_eq!(real_program("www-browser", shims.path(), &path), None);
    }
}
