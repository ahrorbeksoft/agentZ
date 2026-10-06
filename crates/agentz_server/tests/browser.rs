//! Login pages on a machine reached over SSH: the mock agent opens its page with `xdg-open`,
//! which is agentZ's own (the real `agentz-server open-url`), and the page reaches the clients
//! instead of a browser there. The browser's redirect to the agent's callback then logs it in.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use agent_client_protocol::schema::v1 as acp;
use agentz_protocol::agents::AgentId;
use agentz_protocol::thread::ThreadView;
use agentz_protocol::{ClientKind, ConnectionId, Event, Request, Response};
use agentz_server::{AgentControl, CustomAgent, ServerConfig};
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

async fn wait_until(
    events: &mut mpsc::UnboundedReceiver<Event>,
    connection: ConnectionId,
    view: &mut ThreadView,
    done: impl Fn(&ThreadView) -> bool,
) {
    tokio::time::timeout(TIMEOUT, async {
        while !done(view) {
            match events.next().await {
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
    let Some(command) = mock_agent(&data_dir.path().join("logged-in")) else {
        return;
    };
    let socket = data_dir.path().join("server.sock");
    let runtime = tokio::runtime::Handle::current();
    let server = agentz_server::start(
        runtime.clone(),
        ServerConfig {
            data_dir: data_dir.path().to_path_buf(),
            version: "0.0.0-test".into(),
            http_client: Arc::new(http_client::BlockedHttpClient),
            shell_environment_ready: futures::future::ready(()).boxed().shared(),
            custom_agents: BTreeMap::from_iter([(
                AgentId::new("mock"),
                CustomAgent {
                    name: "Mock".into(),
                    command,
                    info: None,
                    accounts: None,
                },
            )]),
            agent_control: Some(AgentControl {
                executable: PathBuf::from(SERVER),
                socket: socket.clone(),
            }),
            hands_pages_to_clients: true,
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
    let (event_sender, mut events) = mpsc::unbounded();
    tokio::spawn(async move {
        while let Some(event) = client_events.next().await {
            event_sender.unbounded_send(event).ok();
        }
    });
    let Response::LoginSessionOpened(login_session_id) = client
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
    let Response::Thread(mut view) = client
        .request(Request::SubscribeThread(connection))
        .await
        .expect("subscribe")
    else {
        panic!("expected the connection");
    };
    wait_until(&mut events, connection, &mut view, |view| {
        view.logged_in() == Some(false)
    })
    .await;

    // Outside a login, `xdg-open` is the real one, the next on the agent's `PATH`.
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
    let browser_programs = data_dir.path().join("browser");
    let path = std::env::join_paths([browser_programs.as_path(), real.path()]).expect("PATH");
    let status = tokio::process::Command::new(browser_programs.join("xdg-open"))
        .arg("https://example.com/docs")
        .env("PATH", &path)
        .env("AGENTZ_SOCKET", &socket)
        .env("AGENTZ_CONNECTION", format!("login:{login_session_id}"))
        .status()
        .await
        .expect("run xdg-open");
    assert!(status.success());
    assert_eq!(
        std::fs::read_to_string(&opened).expect("the real xdg-open ran"),
        "https://example.com/docs\n"
    );
    assert_eq!(view.login_page(), None);

    // While it logs in, the page goes to the clients, and nothing opens on its machine.
    std::fs::remove_file(&opened).expect("remove");
    client
        .request(Request::Authenticate {
            connection,
            method_id: acp::AuthMethodId::new("mock-browser-open-login"),
            meta: None,
        })
        .await
        .expect("authenticate");
    wait_until(&mut events, connection, &mut view, |view| {
        view.login_page().is_some()
    })
    .await;
    let page = view.login_page().expect("the page").to_string();
    assert!(page.starts_with("https://example.com/login?redirect_uri=http%3A%2F%2F127.0.0.1%3A"));
    assert!(!opened.exists());

    let response = follow_redirect(&page).await;
    assert!(response.starts_with("HTTP/1.0 200"), "{response}");
    wait_until(&mut events, connection, &mut view, |view| {
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
    client
        .request(Request::Logout(connection))
        .await
        .expect("log out");
    wait_until(&mut events, connection, &mut view, |view| {
        view.logged_in() == Some(false)
    })
    .await;
    assert_eq!(login_method(), serde_json::Value::Null);
    client
        .request(Request::TerminalLogin {
            connection,
            method_id: acp::AuthMethodId::new("mock-terminal-login"),
        })
        .await
        .expect("start the terminal login");
    wait_until(&mut events, connection, &mut view, |view| {
        view.login_page().is_some()
    })
    .await;
    assert_eq!(
        view.login_page().map(|page| page.as_ref()),
        Some("https://example.com/terminal-login")
    );
    assert!(!opened.exists());

    server.shut_down();
    tokio::time::timeout(TIMEOUT, server.stopped())
        .await
        .expect("the server stops");
}
