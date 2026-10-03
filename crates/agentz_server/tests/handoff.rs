//! `Request::HandOff` against the real binary: the server that takes over runs the same
//! terminal processes, with their screens.

#![allow(
    clippy::disallowed_methods,
    reason = "starts the server as the app does, before anything else runs"
)]

use std::path::Path;
use std::time::{Duration, Instant};

use agentz_client::Connection;
use agentz_protocol::spaces::{PaneContent, PaneTerminal, SpaceRequest};
use agentz_protocol::terminal::{TerminalInput, TerminalKey};
use agentz_protocol::{CAPABILITY_HAND_OFF, ClientKind, Request, Response};

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
