//! Agents' login pages, which the mock agent opens with `xdg-open`: agentZ's own (the real
//! `agentz-server open-url`). On a machine reached over SSH, the page reaches the clients instead
//! of a browser there, and the browser's redirect to the agent's callback then logs it in. On
//! every machine, a session opens no login page nobody asked for.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use agent_client_protocol::schema::v1 as acp;
use agentz_protocol::agents::AgentId;
use agentz_protocol::thread::{ConnectionStatus, ThreadView};
use agentz_protocol::{ClientKind, ConnectionId, Event, Request, Response};
use agentz_server::{AgentControl, AgentDescription, CustomAgent, ServerConfig, ServerHandle};
use futures::channel::mpsc;
use futures::{FutureExt as _, StreamExt as _};
use registry::AgentCommand;
use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};

const TIMEOUT: Duration = Duration::from_secs(20);
const SERVER: &str = env!("CARGO_BIN_EXE_agentz-server");

fn mock_agent(login_file: &Path) -> Option<AgentCommand> {
    let path = std::env::var_os("PATH")?;
    let Some(python) = std::env::split_paths(&path)
        .map(|dir| dir.join("python3"))
        .find(|candidate| candidate.is_file())
    else {
        eprintln!("skipping: python3 not found");
        return None;
    };
    let script = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../agent_thread/test_support/mock_agent.py");
    Some(AgentCommand {
        path: python,
        args: vec![script.to_string_lossy().into_owned()],
        env: [
            (
                "MOCK_LOGIN_FILE".to_string(),
                login_file.to_string_lossy().into_owned(),
            ),
            ("MOCK_BROWSER_OPEN".to_string(), "1".to_string()),
        ]
        .into_iter()
        .collect(),
        env_remove: Vec::new(),
    })
}

/// A server with the mock agent, and an app connected to it.
struct TestServer {
    server: ServerHandle,
    socket: PathBuf,
    client: agentz_client::Connection,
    events: mpsc::UnboundedReceiver<Event>,
}

impl TestServer {
    /// `hands_pages_to_clients` as on a machine reached over SSH.
    async fn start(
        data_dir: &Path,
        command: AgentCommand,
        accounts: Option<AgentDescription>,
        hands_pages_to_clients: bool,
    ) -> Self {
        let socket = data_dir.join("server.sock");
        let runtime = tokio::runtime::Handle::current();
        let server = agentz_server::start(
            runtime.clone(),
            ServerConfig {
                data_dir: data_dir.to_path_buf(),
                version: "0.0.0-test".into(),
                http_client: Arc::new(http_client::BlockedHttpClient),
                shell_environment_ready: futures::future::ready(()).boxed().shared(),
                custom_agents: BTreeMap::from_iter([(
                    AgentId::new("mock"),
                    CustomAgent {
                        name: "Mock".into(),
                        command,
                        info: None,
                        accounts,
                    },
                )]),
                agent_control: Some(AgentControl {
                    executable: PathBuf::from(SERVER),
                    socket: socket.clone(),
                }),
                hands_pages_to_clients,
                terminal_shell: Some("/bin/sh".into()),
                listener: None,
                handed_over: None,
            },
        )
        .expect("server starts");
        let listener = tokio::net::UnixListener::bind(&socket).expect("listen");
        let accepting = server.clone();
        tokio::spawn(async move {
            while let Ok((stream, _)) = listener.accept().await {
                accepting.serve(stream);
            }
        });

        let (client, mut client_events) =
            agentz_client::connect_local(&runtime, &socket, ClientKind::App, "0.0.0-test".into())
                .await
                .expect("connect");
        let (event_sender, events) = mpsc::unbounded();
        tokio::spawn(async move {
            while let Some(event) = client_events.next().await {
                event_sender.unbounded_send(event).ok();
            }
        });
        Self {
            server,
            socket,
            client,
            events,
        }
    }

    /// Starts a thread in a project at `project_dir`, as a new thread's draft does.
    async fn start_thread(&self, project_dir: &Path) -> (ConnectionId, ThreadView) {
        let Response::ProjectAdded(project_id) = self
            .client
            .request(Request::AddProject {
                path: project_dir.to_path_buf(),
            })
            .await
            .expect("add a project")
        else {
            panic!("expected a project");
        };
        let Response::ThreadCreated(thread_id) = self
            .client
            .request(Request::CreateThread {
                project_id,
                agent_id: AgentId::new("mock"),
                workspace: Default::default(),
                account: Default::default(),
            })
            .await
            .expect("create a thread")
        else {
            panic!("expected a thread");
        };
        let connection = ConnectionId::Thread(thread_id);
        (connection, self.subscribe(connection).await)
    }

    /// Opens the agent as its settings do, `None` being the External account.
    async fn open_login_session(&self) -> (ConnectionId, ThreadView) {
        let Response::LoginSessionOpened(login_session_id) = self
            .client
            .request(Request::OpenLoginSession {
                agent_id: AgentId::new("mock"),
                account: None,
            })
            .await
            .expect("open the agent")
        else {
            panic!("expected a login session");
        };
        let connection = ConnectionId::LoginSession(login_session_id);
        (connection, self.subscribe(connection).await)
    }

    async fn subscribe(&self, connection: ConnectionId) -> ThreadView {
        let Response::Thread(view) = self
            .client
            .request(Request::SubscribeThread(connection))
            .await
            .expect("subscribe")
        else {
            panic!("expected the connection");
        };
        view
    }

    async fn wait_until(
        &mut self,
        connection: ConnectionId,
        view: &mut ThreadView,
        done: impl Fn(&ThreadView) -> bool,
    ) {
        tokio::time::timeout(TIMEOUT, async {
            while !done(view) {
                match self.events.next().await {
                    Some(Event::Thread {
                        connection: updated,
                        update,
                    }) if updated == connection => view.apply(update),
                    Some(_) => {}
                    None => panic!("the server closed the connection"),
                }
            }
        })
        .await
        .expect("timed out waiting for the agent");
    }

    /// Waits until the server has found the External account logged in or out.
    async fn wait_for_external_login(&mut self, logged_in: bool) {
        let Response::Session(session) = self
            .client
            .request(Request::SubscribeSession)
            .await
            .expect("subscribe to the session")
        else {
            panic!("expected a session snapshot");
        };
        let mut found = session
            .accounts
            .get(&AgentId::new("mock"))
            .and_then(|accounts| accounts.logged_in(None));
        tokio::time::timeout(TIMEOUT, async {
            while found != Some(logged_in) {
                match self.events.next().await {
                    Some(Event::Accounts(accounts)) => {
                        found = accounts
                            .get(&AgentId::new("mock"))
                            .and_then(|accounts| accounts.logged_in(None));
                    }
                    Some(_) => {}
                    None => panic!("the server closed the connection"),
                }
            }
        })
        .await
        .expect("timed out waiting for the login check");
    }

    async fn stop(self) {
        self.server.shut_down();
        tokio::time::timeout(TIMEOUT, self.server.stopped())
            .await
            .expect("the server stops");
    }
}

/// A folder with an `xdg-open` that notes the page it's given in `opened` there: the real one,
/// next on the agent's `PATH` after agentZ's.
fn real_xdg_open() -> (tempfile::TempDir, PathBuf) {
    let real = tempfile::tempdir().expect("temp dir");
    let opened = real.path().join("opened");
    std::fs::write(
        real.path().join("xdg-open"),
        format!("#!/bin/sh\necho \"$1\" > '{}'\n", opened.display()),
    )
    .expect("write");
    std::fs::set_permissions(
        real.path().join("xdg-open"),
        std::os::unix::fs::PermissionsExt::from_mode(0o755),
    )
    .expect("make it executable");
    (real, opened)
}

/// What a browser does once the user logs in: it follows the redirect to the callback.
async fn follow_redirect(page: &str) -> String {
    let encoded = page
        .split_once("redirect_uri=")
        .map(|(_, rest)| rest.split('&').next().unwrap_or(rest))
        .expect("the page names its callback");
    let callback = encoded.replace("%3A", ":").replace("%2F", "/");
    let address = callback
        .strip_prefix("http://")
        .and_then(|rest| rest.split('/').next())
        .expect("an http callback");
    let mut stream = tokio::net::TcpStream::connect(address)
        .await
        .expect("the agent waits on its callback");
    stream
        .write_all(b"GET /callback?code=from-the-browser HTTP/1.0\r\nHost: localhost\r\n\r\n")
        .await
        .expect("request");
    let mut response = String::new();
    stream
        .read_to_string(&mut response)
        .await
        .expect("response");
    response
}

#[tokio::test(flavor = "multi_thread")]
async fn remote_agents_hand_their_login_pages_to_the_clients() {
    let data_dir = tempfile::tempdir().expect("temp dir");
    let project_dir = tempfile::tempdir().expect("temp dir");
    let Some(command) = mock_agent(&data_dir.path().join("logged-in")) else {
        return;
    };
    let mut server = TestServer::start(data_dir.path(), command, None, true).await;
    let (connection, mut view) = server.open_login_session().await;
    server
        .wait_until(connection, &mut view, |view| {
            view.logged_in() == Some(false)
        })
        .await;

    // Outside a login, a thread's `xdg-open` is the real one, the next on the agent's `PATH`.
    let (thread, mut thread_view) = server.start_thread(project_dir.path()).await;
    server
        .wait_until(thread, &mut thread_view, |view| {
            view.status() != &ConnectionStatus::Connecting
        })
        .await;
    let (real, opened) = real_xdg_open();
    let browser_programs = data_dir.path().join("browser");
    let path = std::env::join_paths([browser_programs.as_path(), real.path()]).expect("PATH");
    let status = tokio::process::Command::new(browser_programs.join("xdg-open"))
        .arg("https://example.com/docs")
        .env("PATH", &path)
        .env("AGENTZ_SOCKET", &server.socket)
        .env(
            "AGENTZ_CONNECTION",
            agentz_server::browser::connection_to_string(thread),
        )
        .status()
        .await
        .expect("run xdg-open");
    assert!(status.success());
    assert_eq!(
        std::fs::read_to_string(&opened).expect("the real xdg-open ran"),
        "https://example.com/docs\n"
    );

    // While it logs in, the page goes to the clients, and nothing opens on its machine.
    std::fs::remove_file(&opened).expect("remove");
    server
        .client
        .request(Request::Authenticate {
            connection,
            method_id: acp::AuthMethodId::new("mock-browser-open-login"),
            meta: None,
        })
        .await
        .expect("authenticate");
    server
        .wait_until(connection, &mut view, |view| view.login_page().is_some())
        .await;
    let page = view.login_page().expect("the page").to_string();
    assert!(page.starts_with("https://example.com/login?redirect_uri=http%3A%2F%2F127.0.0.1%3A"));
    assert!(!opened.exists());

    let response = follow_redirect(&page).await;
    assert!(response.starts_with("HTTP/1.0 200"), "{response}");
    server
        .wait_until(connection, &mut view, |view| {
            !view.is_authenticating() && view.logged_in() == Some(true)
        })
        .await;
    assert_eq!(view.login_page(), None);

    // agentZ made this login, until it's logged out.
    let login_method = || {
        let settings: serde_json::Value = serde_json::from_slice(
            &std::fs::read(data_dir.path().join("agents/settings.json")).expect("settings"),
        )
        .expect("JSON");
        settings["mock"]["login_method"].clone()
    };
    assert_eq!(
        login_method(),
        serde_json::json!("Log in with your browser")
    );

    // A login command in the login terminal (`claude /login`) opens its page the same way.
    server
        .client
        .request(Request::Logout(connection))
        .await
        .expect("log out");
    server
        .wait_until(connection, &mut view, |view| {
            view.logged_in() == Some(false)
        })
        .await;
    assert_eq!(login_method(), serde_json::Value::Null);
    server
        .client
        .request(Request::TerminalLogin {
            connection,
            method_id: acp::AuthMethodId::new("mock-terminal-login"),
        })
        .await
        .expect("start the terminal login");
    server
        .wait_until(connection, &mut view, |view| view.login_page().is_some())
        .await;
    assert_eq!(
        view.login_page().map(|page| page.as_ref()),
        Some("https://example.com/terminal-login")
    );
    assert!(!opened.exists());

    server.stop().await;
}

/// Where pages open on the agent's own machine, as on a Mac, a session that logs in on its own
/// (Antigravity's, with a login method in its settings and none stored) opens no page from the
/// server's own login check, a new thread or the agent's settings. The account is found logged
/// out instead, and its page opens once the user logs in.
#[tokio::test(flavor = "multi_thread")]
async fn sessions_open_no_page_nobody_asked_for() {
    let data_dir = tempfile::tempdir().expect("temp dir");
    let Some(mut command) = mock_agent(&data_dir.path().join("logged-in")) else {
        return;
    };
    let (real, opened) = real_xdg_open();
    let path = std::env::join_paths(std::iter::once(real.path().to_path_buf()).chain(
        std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()),
    ))
    .expect("PATH");
    command.env.extend([
        ("MOCK_SESSION_LOGS_IN".to_string(), "1".to_string()),
        ("PATH".to_string(), path.to_string_lossy().into_owned()),
    ]);
    // With an agentZ account, the server checks the External one's login as it starts.
    let agents = data_dir.path().join("agents");
    std::fs::create_dir_all(&agents).expect("create agents");
    std::fs::write(
        agents.join("accounts.json"),
        serde_json::json!({"mock": {"accounts": [{"id": 1}], "last_id": 1}}).to_string(),
    )
    .expect("write accounts.json");
    let accounts = AgentDescription {
        home_variables: BTreeMap::from([("MOCK_HOME".into(), String::new())]),
        ..AgentDescription::default()
    };
    let mut server = TestServer::start(data_dir.path(), command, Some(accounts), false).await;
    server.wait_for_external_login(false).await;
    assert!(!opened.exists());

    // A new thread waits, logged out, for the user to log in.
    let project_dir = tempfile::tempdir().expect("temp dir");
    let (thread, mut thread_view) = server.start_thread(project_dir.path()).await;
    server
        .wait_until(thread, &mut thread_view, |view| {
            view.status() == &ConnectionStatus::AuthRequired && view.logged_in() == Some(false)
        })
        .await;
    assert!(!opened.exists());

    // The agent starts again without a session, to log in from its settings.
    let (connection, mut view) = server.open_login_session().await;
    server
        .wait_until(connection, &mut view, |view| {
            view.status() == &ConnectionStatus::AuthRequired && view.logged_in() == Some(false)
        })
        .await;
    assert!(!opened.exists());

    server
        .client
        .request(Request::Authenticate {
            connection,
            method_id: acp::AuthMethodId::new("mock-browser-open-login"),
            meta: None,
        })
        .await
        .expect("authenticate");
    let page = tokio::time::timeout(TIMEOUT, async {
        loop {
            if let Ok(page) = std::fs::read_to_string(&opened) {
                break page;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("the real xdg-open opens the page");
    assert_eq!(view.login_page(), None);
    let response = follow_redirect(page.trim()).await;
    assert!(response.starts_with("HTTP/1.0 200"), "{response}");
    server
        .wait_until(connection, &mut view, |view| {
            !view.is_authenticating() && view.logged_in() == Some(true)
        })
        .await;
    server.wait_for_external_login(true).await;

    server.stop().await;
}
