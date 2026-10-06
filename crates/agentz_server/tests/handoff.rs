//! `Request::HandOff` against the real binary: the server that takes over runs the same
//! terminal processes, with their screens, and the same agents, in the middle of their turns.

#![allow(
    clippy::disallowed_methods,
    reason = "starts the server as the app does, before anything else runs"
)]

use std::path::Path;
use std::time::{Duration, Instant};

use agent_client_protocol::schema::v1 as acp;
use agentz_client::Connection;
use agentz_protocol::agents::AgentId;
use agentz_protocol::spaces::{PaneContent, PaneTerminal, SpaceRequest};
use agentz_protocol::terminal::{TerminalInput, TerminalKey};
use agentz_protocol::thread::{Entry, ThreadView};
use agentz_protocol::{CAPABILITY_HAND_OFF, ClientKind, ConnectionId, Event, Request, Response};
use futures::StreamExt as _;

const TIMEOUT: Duration = Duration::from_secs(30);

async fn connect(socket: &Path) -> anyhow::Result<Connection> {
    let (connection, mut events) = agentz_client::connect_local(
        &tokio::runtime::Handle::current(),
        socket,
        ClientKind::Cli,
        "0.0.0-test".into(),
    )
    .await?;
    // Answers are delivered while the events are taken.
    tokio::spawn(async move { while events.next().await.is_some() {} });
    Ok(connection)
}

async fn screen(connection: &Connection, key: &TerminalKey) -> String {
    match connection
        .request(Request::SubscribeTerminal(key.clone()))
        .await
        .expect("subscribes")
    {
        Response::TerminalFrame(frame) => frame.text(),
        response => panic!("unexpected response: {response:?}"),
    }
}

/// The screen once a line starts with `prefix`, and the rest of that line.
async fn wait_for_line(connection: &Connection, key: &TerminalKey, prefix: &str) -> String {
    let deadline = Instant::now() + TIMEOUT;
    loop {
        let text = screen(connection, key).await;
        if let Some(rest) = text
            .lines()
            .find_map(|line| line.trim().strip_prefix(prefix))
        {
            return rest.trim().to_string();
        }
        assert!(
            Instant::now() < deadline,
            "no {prefix:?} on the screen:\n{text}"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

fn type_line(connection: &Connection, key: &TerminalKey, line: &str) {
    drop(connection.request(Request::TerminalInput {
        terminal: key.clone(),
        input: TerminalInput::Bytes(format!("{line}\r").into_bytes()),
    }));
}

#[tokio::test(flavor = "multi_thread")]
async fn hands_terminals_to_a_new_server() {
    // `server.sock`'s path must fit in `SUN_LEN`.
    let data_dir = tempfile::Builder::new()
        .prefix("az")
        .tempdir_in("/tmp")
        .expect("temp dir");
    let socket = data_dir.path().join("server.sock");
    let status = std::process::Command::new(env!("CARGO_BIN_EXE_agentz-server"))
        .arg("start")
        .env("AGENTZ_DATA_DIR", data_dir.path())
        .status()
        .expect("start");
    assert!(status.success());

    let connection = connect(&socket).await.expect("connects");
    let old_pid = connection.welcome().pid;
    assert!(
        connection
            .welcome()
            .capabilities
            .iter()
            .any(|capability| capability == CAPABILITY_HAND_OFF)
    );
    let folder = data_dir.path().to_path_buf();
    let location = match connection
        .request(Request::Spaces(SpaceRequest::CreateSpace {
            folder: folder.clone(),
            project_id: None,
            content: PaneContent::Terminal(PaneTerminal {
                folder,
                command: None,
            }),
        }))
        .await
        .expect("creates a space")
    {
        Response::SpacePane(location) => location,
        response => panic!("unexpected response: {response:?}"),
    };
    let key = TerminalKey::Pane(location.pane);
    type_line(&connection, &key, "echo before-$$");
    let shell = wait_for_line(&connection, &key, "before-").await;

    let response = connection
        .request(Request::HandOff {
            stop_running_turns: false,
        })
        .await
        .expect("hands off");
    assert_eq!(response, Response::Ok);

    let deadline = Instant::now() + TIMEOUT;
    let connection = loop {
        if let Ok(connection) = connect(&socket).await
            && connection.welcome().pid != old_pid
        {
            break connection;
        }
        assert!(Instant::now() < deadline, "no new server");
        tokio::time::sleep(Duration::from_millis(50)).await;
    };
    assert_eq!(wait_for_line(&connection, &key, "before-").await, shell);
    type_line(&connection, &key, "echo after-$$");
    assert_eq!(wait_for_line(&connection, &key, "after-").await, shell);

    drop(connection.request(Request::Shutdown));
    let deadline = Instant::now() + TIMEOUT;
    while socket.exists() {
        assert!(Instant::now() < deadline, "the new server didn't stop");
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

/// Connects as the app does, with the events (and so the answers) passed on.
async fn connect_app(
    socket: &Path,
) -> anyhow::Result<(Connection, futures::channel::mpsc::UnboundedReceiver<Event>)> {
    let (connection, mut client_events) = agentz_client::connect_local(
        &tokio::runtime::Handle::current(),
        socket,
        ClientKind::App,
        "0.0.0-test".into(),
    )
    .await?;
    let (sender, events) = futures::channel::mpsc::unbounded();
    tokio::spawn(async move {
        while let Some(event) = client_events.next().await {
            sender.unbounded_send(event).ok();
        }
    });
    Ok((connection, events))
}

/// Applies the thread's updates until `done`.
async fn wait_for_thread(
    events: &mut futures::channel::mpsc::UnboundedReceiver<Event>,
    thread: ConnectionId,
    view: &mut ThreadView,
    done: impl Fn(&ThreadView) -> bool,
) {
    tokio::time::timeout(TIMEOUT, async {
        while !done(view) {
            match events.next().await {
                Some(Event::Thread { connection, update }) if connection == thread => {
                    view.apply(update)
                }
                Some(_) => {}
                None => panic!("the server closed the connection"),
            }
        }
    })
    .await
    .unwrap_or_else(|_| panic!("timed out; the thread shows {:?}", view.entries()));
}

fn agent_text(view: &ThreadView) -> String {
    view.entries()
        .iter()
        .filter_map(|entry| match entry {
            Entry::AgentMessage(text) => Some(text.as_str()),
            _ => None,
        })
        .collect()
}

/// An agent in the middle of a turn goes on with it in the new server: the same process, on
/// the same session, without starting again.
#[tokio::test(flavor = "multi_thread")]
async fn hands_agents_to_a_new_server() {
    let path = std::env::var_os("PATH").unwrap_or_default();
    let Some(python) = std::env::split_paths(&path)
        .map(|dir| dir.join("python3"))
        .find(|candidate| candidate.is_file())
    else {
        eprintln!("skipping: python3 not found");
        return;
    };
    let script = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../agent_thread/test_support/mock_agent.py");
    let data_dir = tempfile::Builder::new()
        .prefix("az")
        .tempdir_in("/tmp")
        .expect("temp dir");
    let project_dir = tempfile::tempdir().expect("temp dir");
    std::fs::create_dir_all(data_dir.path().join("agents")).expect("agents dir");
    std::fs::write(
        data_dir.path().join("agents/custom.json"),
        serde_json::json!({"mock": {"name": "Mock", "command": {
            "path": python, "args": [script], "env": {}}}})
        .to_string(),
    )
    .expect("custom agents");
    let socket = data_dir.path().join("server.sock");
    let status = std::process::Command::new(env!("CARGO_BIN_EXE_agentz-server"))
        .arg("start")
        .env("AGENTZ_DATA_DIR", data_dir.path())
        .status()
        .expect("start");
    assert!(status.success());

    let (connection, mut events) = connect_app(&socket).await.expect("connects");
    let old_pid = connection.welcome().pid;
    let Response::ProjectAdded(project_id) = connection
        .request(Request::AddProject {
            path: project_dir.path().to_path_buf(),
        })
        .await
        .expect("adds a project")
    else {
        panic!("expected a project");
    };
    let Response::ThreadCreated(thread_id) = connection
        .request(Request::CreateThread {
            project_id,
            agent_id: AgentId::new("mock"),
            workspace: Default::default(),
            account: Default::default(),
        })
        .await
        .expect("creates a thread")
    else {
        panic!("expected a thread");
    };
    let thread = ConnectionId::Thread(thread_id);
    let Response::Thread(mut view) = connection
        .request(Request::SubscribeThread(thread))
        .await
        .expect("subscribes")
    else {
        panic!("expected the thread");
    };
    let response = connection
        .request(Request::Prompt {
            connection: thread,
            prompt: agentz_protocol::PromptPart::text("slow"),
        })
        .await
        .expect("prompts");
    assert_eq!(response, Response::Ok);
    wait_for_thread(&mut events, thread, &mut view, |view| {
        agent_text(view).starts_with("One")
    })
    .await;
    assert!(view.is_working());

    // The turn can be handed over, so the server doesn't ask to stop it.
    let response = connection
        .request(Request::HandOff {
            stop_running_turns: false,
        })
        .await
        .expect("hands off");
    assert_eq!(response, Response::Ok);

    let deadline = Instant::now() + TIMEOUT;
    let (connection, mut events) = loop {
        if let Ok((connection, events)) = connect_app(&socket).await
            && connection.welcome().pid != old_pid
        {
            break (connection, events);
        }
        assert!(Instant::now() < deadline, "no new server");
        tokio::time::sleep(Duration::from_millis(50)).await;
    };
    let Response::Thread(mut view) = connection
        .request(Request::SubscribeThread(thread))
        .await
        .expect("subscribes")
    else {
        panic!("expected the thread");
    };
    wait_for_thread(&mut events, thread, &mut view, |view| !view.is_working()).await;
    assert_eq!(agent_text(&view), "One two three four five");
    assert_eq!(view.last_stop_reason(), Some(&acp::StopReason::EndTurn));

    // The same agent answers on: a new one couldn't load the session (the mock keeps no
    // history), and the earlier messages would be gone.
    let response = connection
        .request(Request::Prompt {
            connection: thread,
            prompt: agentz_protocol::PromptPart::text("hello"),
        })
        .await
        .expect("prompts");
    assert_eq!(response, Response::Ok);
    wait_for_thread(&mut events, thread, &mut view, |view| {
        !view.is_working() && agent_text(view).ends_with("Echo: hello")
    })
    .await;
    assert!(agent_text(&view).starts_with("One two three four five"));

    drop(connection.request(Request::Shutdown));
    let deadline = Instant::now() + TIMEOUT;
    while socket.exists() {
        assert!(Instant::now() < deadline, "the new server didn't stop");
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}
