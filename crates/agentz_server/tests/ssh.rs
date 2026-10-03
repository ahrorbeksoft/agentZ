//! Connecting over SSH, with a stand-in for `ssh` that runs the remote command here, under a
//! scratch home and data directory. Everything else is real: the probe, the upload, the
//! proxy, and the server it starts.

use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::time::Duration;

use agentz_client::Connection;
use agentz_client::ssh::{Connected, RemotePlatform, Ssh};
use agentz_protocol::agents::{AgentId, InstallState, RegistrySnapshot};
use agentz_protocol::{ClientKind, Event, Request, Response};

const VERSION: &str = "0.0.0-test";
const TIMEOUT: Duration = Duration::from_secs(30);

/// Skips ssh's options and runs the command with `sh -c`, as sshd hands it to the user's
/// shell. A few targets fail the way real ssh does.
fn fake_ssh(directory: &Path, home: &Path, data_dir: &Path) -> PathBuf {
    let script = format!(
        r#"#!/bin/sh
while [ $# -gt 0 ]; do
  case "$1" in
    -o|-S|-F) shift 2 ;;
    -*) shift ;;
    *) break ;;
  esac
done
target="$1"
shift
case "$target" in
  unreachable) echo "ssh: connect to host unreachable port 22: Connection refused" >&2; exit 255 ;;
  locked) echo "me@locked: Permission denied (publickey)." >&2; exit 255 ;;
esac
echo "Welcome to the test machine"
HOME='{home}' AGENTZ_DATA_DIR='{data_dir}' RUST_LOG=warn exec sh -c "$*"
"#,
        home = home.display(),
        data_dir = data_dir.display(),
    );
    let path = directory.join("ssh");
    std::fs::write(&path, script).expect("writes the fake ssh");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).expect("chmod");
    path
}

struct Session {
    connection: Connection,
    is_outdated: bool,
    /// Ends when the server closes the session.
    events_drained: tokio::task::JoinHandle<()>,
}

async fn connect(
    ssh: &Ssh,
    server_binary: impl FnOnce(RemotePlatform) -> anyhow::Result<PathBuf>,
) -> Session {
    let Connected {
        connection,
        mut events,
        is_outdated,
    } = tokio::time::timeout(
        TIMEOUT,
        agentz_client::ssh::connect(ssh, VERSION, server_binary, ClientKind::App),
    )
    .await
    .expect("in time")
    .expect("connects");
    // Answers are delivered while the events are taken.
    let events_drained = tokio::spawn(async move { while events.next().await.is_some() {} });
    Session {
        connection,
        is_outdated,
        events_drained,
    }
}

/// Returns once the server has closed the session.
async fn shut_down(session: Session) {
    session
        .connection
        .request(Request::Shutdown)
        .await
        .expect("shuts down");
    tokio::time::timeout(TIMEOUT, session.events_drained)
        .await
        .expect("in time")
        .expect("drains the events");
}

fn server_binary(platform: RemotePlatform) -> anyhow::Result<PathBuf> {
    anyhow::ensure!(
        Some(platform) == RemotePlatform::current(),
        "no agentz-server for {platform}"
    );
    Ok(PathBuf::from(env!("CARGO_BIN_EXE_agentz-server")))
}

#[tokio::test(flavor = "multi_thread")]
async fn connects_installs_and_reconnects_over_ssh() {
    let scratch = tempfile::tempdir().expect("temp dir");
    let home = scratch.path().join("home");
    std::fs::create_dir(&home).expect("home");
    // Short, for the server's socket path.
    let data_dir = tempfile::Builder::new()
        .prefix("azssh")
        .tempdir_in("/tmp")
        .expect("data dir");
    let project = scratch.path().join("project");
    std::fs::create_dir(&project).expect("project");
    let program = fake_ssh(scratch.path(), &home, data_dir.path());
    let ssh = Ssh::with_program(program.clone(), "devbox").expect("valid target");

    let connected = connect(&ssh, server_binary).await;
    assert!(!connected.is_outdated);
    let connection = connected.connection;
    let installed = home.join(".agentz/server").join(VERSION);
    assert!(installed.join("agentz-server").is_file());
    let hash = std::fs::read_to_string(installed.join("agentz-server.sha256")).expect("hash");
    assert_eq!(hash.trim().len(), 64);
    let server_pid = connection.welcome().pid;
    assert_ne!(server_pid, std::process::id());
    let added = connection
        .request(Request::AddProject {
            path: project.clone(),
        })
        .await
        .expect("adds the project");
    assert!(matches!(added, Response::ProjectAdded(_)));
    drop(connection);

    // The server outlives the session. Reconnecting finds it, and uploads nothing.
    let modified = std::fs::metadata(installed.join("agentz-server"))
        .and_then(|metadata| metadata.modified())
        .expect("mtime");
    let connected = connect(&ssh, server_binary).await;
    assert!(!connected.is_outdated);
    let connection = connected.connection;
    assert_eq!(connection.welcome().pid, server_pid);
    assert_eq!(
        std::fs::metadata(installed.join("agentz-server"))
            .and_then(|metadata| metadata.modified())
            .expect("mtime"),
        modified
    );
    let Response::Session(session) = connection
        .request(Request::SubscribeSession)
        .await
        .expect("session")
    else {
        panic!("expected the session");
    };
    assert_eq!(session.projects.projects.len(), 1);
    drop(connection);

    // A newer binary is installed, but the running server stays until it's stopped.
    let newer = scratch.path().join("agentz-server-newer");
    let mut bytes = std::fs::read(env!("CARGO_BIN_EXE_agentz-server")).expect("reads the server");
    bytes.push(0);
    std::fs::write(&newer, bytes).expect("writes the newer server");
    std::fs::set_permissions(&newer, std::fs::Permissions::from_mode(0o755)).expect("chmod");
    let newer_binary = |_: RemotePlatform| Ok(newer.clone());
    let connected = connect(&ssh, newer_binary).await;
    assert!(connected.is_outdated);
    assert_eq!(connected.connection.welcome().pid, server_pid);
    shut_down(connected).await;
    let connected = connect(&ssh, newer_binary).await;
    assert!(!connected.is_outdated);
    assert_ne!(connected.connection.welcome().pid, server_pid);
    shut_down(connected).await;

    let unreachable = Ssh::with_program(program.clone(), "unreachable").expect("valid target");
    let error = agentz_client::ssh::connect(&unreachable, VERSION, server_binary, ClientKind::App)
        .await
        .err()
        .expect("fails");
    assert!(error.message.contains("Connection refused"), "{error:?}");
    assert!(!error.needs_attention);

    let locked = Ssh::with_program(program, "locked").expect("valid target");
    let error = agentz_client::ssh::connect(&locked, VERSION, server_binary, ClientKind::App)
        .await
        .err()
        .expect("fails");
    assert!(error.needs_attention, "{error:?}");
    assert!(error.hint.is_some());
}

/// Against a real machine, with the Linux servers from `tooling/build-remote-servers.sh`:
/// `AGENTZ_SSH_TEST_TARGET=devbox1 cargo test -p agentz_server --test ssh -- --ignored`.
/// Only `~/.agentz` changes there. With `AGENTZ_SSH_TEST_RESTART=1`, an older server running
/// there is stopped and replaced, which stops its agents. `AGENTZ_SSH_TEST_INSTALL_AGENT=<id>`
/// installs that registry agent there (downloading Node.js first for an npm agent, when the
/// machine has none), without starting it.
#[tokio::test(flavor = "multi_thread")]
#[ignore]
async fn connects_to_a_real_machine() {
    let Some(target) = std::env::var_os("AGENTZ_SSH_TEST_TARGET") else {
        panic!("set AGENTZ_SSH_TEST_TARGET to an SSH target");
    };
    let ssh = Ssh::new(&target.to_string_lossy()).expect("valid target");
    let servers = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/remote-servers");
    let (registry_sender, mut registry_updates) = tokio::sync::mpsc::unbounded_channel();
    let connect = || async {
        let Connected {
            connection,
            mut events,
            is_outdated,
        } = tokio::time::timeout(
            Duration::from_secs(300),
            agentz_client::ssh::connect(
                &ssh,
                env!("CARGO_PKG_VERSION"),
                |platform| {
                    let path = agentz_client::ssh::bundled_server_binary(&servers, platform);
                    anyhow::ensure!(path.is_file(), "no {}", path.display());
                    Ok(path)
                },
                ClientKind::App,
            ),
        )
        .await
        .expect("in time")
        .expect("connects");
        let registry_sender = registry_sender.clone();
        let events_drained = tokio::spawn(async move {
            while let Some(event) = events.next().await {
                if let Event::Registry(registry) = event {
                    registry_sender.send(registry).ok();
                }
            }
        });
        Session {
            connection,
            is_outdated,
            events_drained,
        }
    };
    let mut session = connect().await;
    eprintln!("outdated: {}", session.is_outdated);
    if session.is_outdated && std::env::var_os("AGENTZ_SSH_TEST_RESTART").is_some() {
        let old_pid = session.connection.welcome().pid;
        shut_down(session).await;
        session = connect().await;
        assert!(!session.is_outdated);
        assert_ne!(session.connection.welcome().pid, old_pid);
        eprintln!("replaced the server (pid {old_pid})");
    }
    let connection = session.connection;
    let welcome = connection.welcome();
    eprintln!(
        "connected to agentz-server {} on {} ({} {}), pid {}",
        welcome.server_version,
        welcome.machine.hostname,
        welcome.machine.os,
        welcome.machine.arch,
        welcome.pid
    );
    let Response::Session(_) = connection
        .request(Request::SubscribeSession)
        .await
        .expect("session")
    else {
        panic!("expected the session");
    };

    let Some(agent) = std::env::var_os("AGENTZ_SSH_TEST_INSTALL_AGENT") else {
        return;
    };
    let agent = AgentId::new(agent.to_string_lossy().into_owned());
    connection
        .request(Request::RefreshRegistry { if_stale: false })
        .await
        .expect("refreshes the registry");
    let install_state = |registry: &RegistrySnapshot| {
        registry
            .agents
            .iter()
            .find(|listing| listing.metadata.id == agent)
            .map(|listing| listing.install_state.clone())
    };
    let started = std::time::Instant::now();
    let mut requested = false;
    loop {
        let registry = tokio::time::timeout(Duration::from_secs(600), registry_updates.recv())
            .await
            .expect("in time")
            .expect("registry updates");
        if registry.is_fetching {
            continue;
        }
        match install_state(&registry) {
            None => panic!("{agent} isn't in the registry"),
            Some(InstallState::Installed { version, .. }) => {
                eprintln!("{agent} {version} is installed ({:.0?})", started.elapsed());
                break;
            }
            Some(InstallState::Failed(error)) => panic!("installing {agent} failed: {error}"),
            Some(InstallState::Installing) => {}
            // An update sent before the request may still be queued.
            Some(InstallState::NotInstalled) if requested => {}
            Some(InstallState::NotInstalled) => {
                connection
                    .request(Request::InstallAgent(agent.clone()))
                    .await
                    .expect("installs the agent");
                requested = true;
            }
        }
    }
}
