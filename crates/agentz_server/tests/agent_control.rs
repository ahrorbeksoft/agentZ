//! Agent control end to end: the mock agent starts the real `agentz-server mcp-bridge` it's
//! given with its session and calls tools through it, and the CLI calls them as a thread.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use agentz_client::Connection;
use agentz_protocol::agents::AgentId;
use agentz_protocol::thread::{Entry, ThreadView};
use agentz_protocol::{ClientKind, ConnectionId, Event, Request, Response};
use agentz_server::{AgentControl, CustomAgent, ServerConfig};
use futures::channel::mpsc;
use futures::{FutureExt as _, StreamExt as _};
use registry::AgentCommand;
use serde_json::{Value, json};

const TIMEOUT: Duration = Duration::from_secs(20);
const BRIDGE: &str = env!("CARGO_BIN_EXE_agentz-server");

fn mock_agent() -> Option<AgentCommand> {
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
        env: Default::default(),
    })
}

/// Sends the prompt and waits for the agent's answer, which the mock agent prefixes with
/// `MCP: `, and decodes the tool result in it.
async fn mcp_call(
    connection: &Connection,
    events: &mut mpsc::UnboundedReceiver<Event>,
    thread: ConnectionId,
    view: &mut ThreadView,
    text: &str,
) -> Value {
    let response = connection
        .request(Request::Prompt {
            connection: thread,
            text: text.into(),
        })
        .await
        .expect("prompt");
    assert_eq!(response, Response::Ok);
    let answered = tokio::time::timeout(TIMEOUT, async {
        loop {
            let entries = view.entries();
            if !view.is_working()
                && let [.., Entry::UserMessage(prompt), Entry::AgentMessage(answer)] = entries
                && prompt == text
                && let Some(result) = answer.strip_prefix("MCP: ")
            {
                return result.to_string();
            }
            match events.next().await {
                Some(Event::Thread {
                    connection: updated,
                    update,
                }) if updated == thread => view.apply(update),
                Some(_) => {}
                None => panic!("the server closed the connection"),
            }
        }
    })
    .await
    .expect("timed out waiting for the agent's answer");
    serde_json::from_str(&answered).unwrap_or_else(|_| panic!("not a tool result: {answered}"))
}

#[tokio::test(flavor = "multi_thread")]
async fn agents_call_tools_through_the_mcp_bridge_and_the_cli() {
    let Some(command) = mock_agent() else {
        return;
    };
    let data_dir = tempfile::tempdir().expect("temp dir");
    let project_dir = tempfile::tempdir().expect("temp dir");
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
                },
            )]),
            agent_control: Some(AgentControl {
                executable: PathBuf::from(BRIDGE),
                socket: socket.clone(),
            }),
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

    let (connection, mut client_events) =
        agentz_client::connect_local(&runtime, &socket, ClientKind::App, "0.0.0-test".into())
            .await
            .expect("connect");
    // Taking the events is what delivers the answers.
    let (event_sender, mut events) = mpsc::unbounded();
    tokio::spawn(async move {
        while let Some(event) = client_events.next().await {
            event_sender.unbounded_send(event).ok();
        }
    });
    let Response::ProjectAdded(project_id) = connection
        .request(Request::AddProject {
            path: project_dir.path().to_path_buf(),
        })
        .await
        .expect("add a project")
    else {
        panic!("expected a project");
    };
    let Response::ThreadCreated(thread_id) = connection
        .request(Request::CreateThread {
            project_id,
            agent_id: AgentId::new("mock"),
            workspace: Default::default(),
        })
        .await
        .expect("create a thread")
    else {
        panic!("expected a thread");
    };
    let thread = ConnectionId::Thread(thread_id);
    let Response::Thread(mut view) = connection
        .request(Request::SubscribeThread(thread))
        .await
        .expect("subscribe")
    else {
        panic!("expected the thread");
    };

    // The agent's first tool, with the thread it was started for as the caller.
    let capabilities = mcp_call(&connection, &mut events, thread, &mut view, "mcp").await;
    assert_eq!(capabilities["currentThreadId"], json!(thread_id.0));

    let launched = mcp_call(
        &connection,
        &mut events,
        thread,
        &mut view,
        r#"mcp agentz_thread_launch {"prompt": "from the bridge"}"#,
    )
    .await;
    assert_eq!(launched["createdByThreadId"], json!(thread_id.0));
    let worker = launched["threadId"].as_u64().expect("a thread id");

    let delegated = mcp_call(
        &connection,
        &mut events,
        thread,
        &mut view,
        r#"mcp delegate_task {"task": "delegated", "mode": "wait"}"#,
    )
    .await;
    assert_eq!(delegated["status"], json!("completed"));
    assert_eq!(delegated["summary"], json!("Echo: delegated"));

    // A thread's agent can run the CLI from its shell with the environment it was given.
    let output = tokio::process::Command::new(BRIDGE)
        .args([
            "call",
            "agentz_thread_wait",
            &json!({"threadId": worker}).to_string(),
        ])
        .env("AGENTZ_SOCKET", &socket)
        .env("AGENTZ_THREAD_ID", thread_id.0.to_string())
        .output()
        .await
        .expect("run the CLI");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let waited: Value = serde_json::from_slice(&output.stdout).expect("JSON");
    assert_eq!(waited["lastAgentMessage"], json!("Echo: from the bridge"));

    // Failures come back as tool errors, with a non-zero exit from the CLI.
    let output = tokio::process::Command::new(BRIDGE)
        .args(["call", "agentz_thread_read", r#"{"threadId": 999}"#])
        .env("AGENTZ_SOCKET", &socket)
        .env("AGENTZ_THREAD_ID", thread_id.0.to_string())
        .output()
        .await
        .expect("run the CLI");
    assert!(!output.status.success());
    let failure: Value = serde_json::from_slice(&output.stdout).expect("JSON");
    assert_eq!(failure["code"], json!("thread_not_found"));

    server.shut_down();
    tokio::time::timeout(TIMEOUT, server.stopped())
        .await
        .expect("the server stops");
}
