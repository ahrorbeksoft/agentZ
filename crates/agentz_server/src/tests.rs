//! The server driven over in-memory streams, with `agent_thread`'s mock agent.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use agent_client_protocol::schema::v1 as acp;
use agentz_protocol::agents::AgentId;
use agentz_protocol::diff::{DiffScope, DiffStatus, FileChange, RestoreAvailability, ThreadDiff};
use agentz_protocol::layout::{Direction, Node};
use agentz_protocol::spaces::{
    PaneAgentState, PaneContent, PaneLocation, PaneTerminal, SpaceRequest, SpacesSnapshot,
};
use agentz_protocol::terminal::{
    TerminalCommand, TerminalFrame, TerminalInput, TerminalKey, TerminalPoint,
    TerminalSelectionKind, TerminalSelectionUpdate,
};
use agentz_protocol::thread::{Entry, ThreadView};
use agentz_protocol::workspace::{WorkspaceChoice, WorkspaceRemoval};
use agentz_protocol::{
    ClientHello, ClientKind, ClientMessage, ConnectionId, ErrorResponse, Event, PROTOCOL_VERSION,
    PeerCheckout, PeerCheckouts, PeerMachine, Peers, Request, Response, ServerMessage,
    ServerWelcome, ToolCaller, ToolResult, read_message, write_message,
};
use futures::FutureExt as _;
use projects::{ProjectId, ProjectsSnapshot, ThreadCreator, ThreadId, WorkspaceKind};
use registry::AgentCommand;
use serde_json::{Value, json};
use tokio::io::{DuplexStream, ReadHalf, WriteHalf};

use crate::{CustomAgent, ServerConfig, ServerHandle};

const TIMEOUT: Duration = Duration::from_secs(10);

struct TestServer {
    handle: ServerHandle,
    data_dir: tempfile::TempDir,
    project_dir: tempfile::TempDir,
}

impl TestServer {
    /// `None` without python3 to run the mock agent.
    fn start() -> Option<Self> {
        Self::start_with(
            tempfile::tempdir().expect("temp dir"),
            tempfile::tempdir().expect("temp dir"),
        )
    }

    fn start_with(data_dir: tempfile::TempDir, project_dir: tempfile::TempDir) -> Option<Self> {
        let command = mock_agent()?;
        let custom_agents = BTreeMap::from_iter([(
            AgentId::new("mock"),
            CustomAgent {
                name: "Mock".into(),
                command,
            },
        )]);
        let handle = crate::start(
            tokio::runtime::Handle::current(),
            ServerConfig {
                data_dir: data_dir.path().to_path_buf(),
                version: "0.0.0-test".into(),
                http_client: Arc::new(http_client::BlockedHttpClient),
                shell_environment_ready: futures::future::ready(()).boxed().shared(),
                custom_agents,
                agent_control: None,
                terminal_shell: Some("/bin/sh".into()),
            },
        )
        .expect("server starts");
        Some(Self {
            handle,
            data_dir,
            project_dir,
        })
    }

    async fn connect(&self) -> TestClient {
        TestClient::connect(&self.handle).await
    }
}

struct TestClient {
    reader: ReadHalf<DuplexStream>,
    writer: WriteHalf<DuplexStream>,
    welcome: ServerWelcome,
    next_id: u64,
    projects: Option<ProjectsSnapshot>,
    threads: BTreeMap<ConnectionId, ThreadView>,
    terminals: BTreeMap<TerminalKey, TerminalFrame>,
    events: Vec<Event>,
}

impl TestClient {
    async fn connect(server: &ServerHandle) -> Self {
        Self::connect_with_version(server, PROTOCOL_VERSION).await
    }

    async fn connect_with_version(server: &ServerHandle, protocol_version: u32) -> Self {
        let (client, server_side) = tokio::io::duplex(1 << 20);
        server.serve(server_side);
        let (mut reader, mut writer) = tokio::io::split(client);
        let hello = ClientHello {
            protocol_version,
            client_version: "0.0.0-test".into(),
            client_kind: ClientKind::App,
        };
        write_message(&mut writer, &hello).await.expect("hello");
        let welcome: ServerWelcome = read_message(&mut reader)
            .await
            .expect("welcome")
            .expect("a welcome before the stream ends");
        Self {
            reader,
            writer,
            welcome,
            next_id: 1,
            projects: None,
            threads: BTreeMap::new(),
            terminals: BTreeMap::new(),
            events: Vec::new(),
        }
    }

    async fn request(&mut self, request: Request) -> Result<Response, ErrorResponse> {
        let id = self.next_id;
        self.next_id += 1;
        write_message(&mut self.writer, &ClientMessage::Request { id, request })
            .await
            .expect("request");
        tokio::time::timeout(TIMEOUT, async {
            loop {
                match self.next_message().await {
                    ServerMessage::Response {
                        id: response_id,
                        result,
                    } if response_id == id => return result,
                    ServerMessage::Event(event) => self.apply(event),
                    message => panic!("unexpected message: {message:?}"),
                }
            }
        })
        .await
        .expect("timed out waiting for a response")
    }

    async fn ok(&mut self, request: Request) -> Response {
        let description = format!("{request:?}");
        match self.request(request).await {
            Ok(response) => response,
            Err(error) => panic!("{description} failed: {}", error.message),
        }
    }

    async fn subscribe_thread(&mut self, connection: ConnectionId) {
        match self.ok(Request::SubscribeThread(connection)).await {
            Response::Thread(view) => {
                self.threads.insert(connection, view);
            }
            response => panic!("unexpected response: {response:?}"),
        }
    }

    async fn next_message(&mut self) -> ServerMessage {
        read_message(&mut self.reader)
            .await
            .expect("a message")
            .expect("the server closed the connection")
    }

    fn apply(&mut self, event: Event) {
        match &event {
            Event::Projects(projects) => self.projects = Some(projects.clone()),
            Event::Thread { connection, update } => {
                if let Some(view) = self.threads.get_mut(connection) {
                    view.apply(update.clone());
                }
            }
            Event::TerminalFrame { terminal, frame } => {
                if let Some(screen) = self.terminals.get_mut(terminal) {
                    screen.apply(frame.clone());
                }
            }
            Event::TerminalClosed(terminal) => {
                self.terminals.remove(terminal);
            }
            _ => {}
        }
        self.events.push(event);
    }

    /// Reads events until `done` holds.
    async fn wait_until(&mut self, done: impl Fn(&Self) -> bool) {
        let waited = tokio::time::timeout(TIMEOUT, async {
            while !done(self) {
                match self.next_message().await {
                    ServerMessage::Event(event) => self.apply(event),
                    message => panic!("unexpected message: {message:?}"),
                }
            }
        })
        .await;
        if waited.is_err() {
            panic!("timed out; threads {:#?}", self.threads);
        }
    }

    fn thread(&self, connection: ConnectionId) -> &ThreadView {
        &self.threads[&connection]
    }

    async fn create_thread(&mut self, server: &TestServer) -> ThreadId {
        let project_id = match self
            .ok(Request::AddProject {
                path: server.project_dir.path().to_path_buf(),
            })
            .await
        {
            Response::ProjectAdded(project_id) => project_id,
            response => panic!("unexpected response: {response:?}"),
        };
        match self
            .ok(Request::CreateThread {
                project_id,
                agent_id: AgentId::new("mock"),
                workspace: Default::default(),
            })
            .await
        {
            Response::ThreadCreated(thread_id) => thread_id,
            response => panic!("unexpected response: {response:?}"),
        }
    }
}

fn agent_text(view: &ThreadView) -> String {
    view.entries()
        .iter()
        .filter_map(|entry| match entry {
            Entry::AgentMessage(text) => Some(text.to_string()),
            _ => None,
        })
        .collect()
}

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

#[tokio::test(flavor = "multi_thread")]
async fn refuses_other_protocol_versions() {
    let Some(server) = TestServer::start() else {
        return;
    };
    let client = server.connect().await;
    assert_eq!(client.welcome.error, None);
    assert_eq!(client.welcome.pid, std::process::id());
    assert!(!client.welcome.machine.id.is_empty());

    let mut client = TestClient::connect_with_version(&server.handle, PROTOCOL_VERSION + 1).await;
    assert!(client.welcome.error.is_some());
    let closed: Option<ServerMessage> = read_message(&mut client.reader).await.expect("read");
    assert_eq!(closed, None);
}

#[tokio::test(flavor = "multi_thread")]
async fn prompts_a_thread_and_names_it() {
    let Some(server) = TestServer::start() else {
        return;
    };
    let mut client = server.connect().await;
    let Response::Session(session) = client.ok(Request::SubscribeSession).await else {
        panic!("expected a session snapshot");
    };
    assert!(session.projects.projects.is_empty());

    let thread_id = client.create_thread(&server).await;
    let connection = ConnectionId::Thread(thread_id);
    client.subscribe_thread(connection).await;
    client
        .ok(Request::Prompt {
            connection,
            text: "hello there".into(),
        })
        .await;
    client
        .wait_until(|client| {
            let thread = client.thread(connection);
            !thread.is_working() && agent_text(thread) == "Echo: hello there"
        })
        .await;
    // The first prompt names the thread, and the session subscription hears about it.
    client
        .wait_until(|client| {
            client.projects.as_ref().is_some_and(|projects| {
                projects
                    .threads
                    .iter()
                    .any(|thread| thread.id == thread_id && thread.title == "hello there")
            })
        })
        .await;
    // The agent's settings remember the options it offered.
    client
        .wait_until(|client| {
            client.events.iter().any(|event| match event {
                Event::AgentSettings(settings) => settings
                    .get(&AgentId::new("mock"))
                    .is_some_and(|settings| !settings.known_config_options.is_empty()),
                _ => false,
            })
        })
        .await;

    client.ok(Request::DeleteThread(thread_id)).await;
    client
        .wait_until(|client| {
            client
                .events
                .iter()
                .any(|event| *event == Event::ConnectionClosed(connection))
        })
        .await;
    let error = client
        .request(Request::Prompt {
            connection,
            text: "again".into(),
        })
        .await;
    assert!(error.is_err());
}

/// The point of the server: a turn keeps going without a client, and the next client sees
/// where it got to.
#[tokio::test(flavor = "multi_thread")]
async fn threads_outlive_their_clients() {
    let Some(server) = TestServer::start() else {
        return;
    };
    let mut client = server.connect().await;
    let thread_id = client.create_thread(&server).await;
    let connection = ConnectionId::Thread(thread_id);
    client.subscribe_thread(connection).await;
    client
        .ok(Request::Prompt {
            connection,
            text: "permission".into(),
        })
        .await;
    client
        .wait_until(|client| {
            !client
                .thread(connection)
                .state
                .permission_requests
                .is_empty()
        })
        .await;
    drop(client);

    let mut client = server.connect().await;
    let Response::Session(session) = client.ok(Request::SubscribeSession).await else {
        panic!("expected a session snapshot");
    };
    assert_eq!(session.projects.blocked_threads, vec![thread_id]);
    assert_eq!(session.projects.working_threads, vec![thread_id]);
    client.projects = Some(session.projects);
    client.subscribe_thread(connection).await;
    let thread = client.thread(connection);
    assert!(thread.is_working());
    let request = thread
        .state
        .permission_requests
        .first()
        .expect("the request is still waiting")
        .clone();
    client
        .ok(Request::RespondToPermission {
            connection,
            tool_call_id: request.tool_call_id,
            option_id: acp::PermissionOptionId::new("allow"),
        })
        .await;
    client
        .wait_until(|client| {
            let thread = client.thread(connection);
            !thread.is_working() && agent_text(thread).ends_with("(chose allow)")
        })
        .await;
    client
        .wait_until(|client| {
            client.projects.as_ref().is_some_and(|projects| {
                projects.blocked_threads.is_empty()
                    && projects.working_threads.is_empty()
                    && projects
                        .threads
                        .iter()
                        .any(|thread| thread.id == thread_id && thread.completed_at.is_some())
            })
        })
        .await;
}

#[tokio::test(flavor = "multi_thread")]
async fn accounts_log_in_and_close_with_their_client() {
    let Some(server) = TestServer::start() else {
        return;
    };
    let mut client = server.connect().await;
    client.ok(Request::SubscribeSession).await;
    let Response::AccountOpened(account_id) =
        client.ok(Request::OpenAccount(AgentId::new("mock"))).await
    else {
        panic!("expected an account");
    };
    let connection = ConnectionId::Account(account_id);
    client.subscribe_thread(connection).await;
    client
        .wait_until(|client| !client.thread(connection).auth_methods().is_empty())
        .await;
    client
        .ok(Request::Authenticate {
            connection,
            method_id: acp::AuthMethodId::new("mock-login"),
        })
        .await;
    client
        .wait_until(|client| {
            client.events.iter().any(|event| match event {
                Event::AgentSettings(settings) => settings
                    .get(&AgentId::new("mock"))
                    .is_some_and(|settings| settings.login_method.as_deref() == Some("Log In")),
                _ => false,
            })
        })
        .await;
    drop(client);

    let mut client = server.connect().await;
    // Closed when its client went away; give the server a moment to hear about it.
    let mut closed = false;
    for _ in 0..50 {
        if client
            .request(Request::SubscribeThread(connection))
            .await
            .is_err()
        {
            closed = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert!(closed);
}

#[tokio::test(flavor = "multi_thread")]
async fn shuts_down_on_request() {
    let Some(server) = TestServer::start() else {
        return;
    };
    let mut client = server.connect().await;
    let unknown = client
        .request(Request::Unknown(serde_json::json!({"FromTheFuture": 1})))
        .await;
    assert!(unknown.is_err());
    client.ok(Request::Shutdown).await;
    tokio::time::timeout(TIMEOUT, server.handle.stopped())
        .await
        .expect("the server stops");
}

#[tokio::test(flavor = "multi_thread")]
async fn projects_learn_their_repository() {
    let Some(server) = TestServer::start() else {
        return;
    };
    let folder = server.project_dir.path();
    for args in [
        &["init", "--quiet"][..],
        &[
            "remote",
            "add",
            "origin",
            "https://github.com/Owner/Repo.git",
        ],
    ] {
        crate::git::git(folder, args, &[]).await.expect("git");
    }
    let mut client = server.connect().await;
    client.ok(Request::SubscribeSession).await;
    let project_id = client.add_project(folder).await;
    client
        .wait_until(|client| {
            client.projects.as_ref().is_some_and(|projects| {
                projects.projects.iter().any(|project| {
                    project.id == project_id
                        && project
                            .repository
                            .as_ref()
                            .map(|r| r.canonical_key.as_str())
                            == Some("github.com/owner/repo")
                })
            })
        })
        .await;
}

impl TestClient {
    async fn add_project(&mut self, path: &std::path::Path) -> ProjectId {
        match self
            .ok(Request::AddProject {
                path: path.to_path_buf(),
            })
            .await
        {
            Response::ProjectAdded(project_id) => project_id,
            response => panic!("unexpected response: {response:?}"),
        }
    }

    async fn create_thread_in(&mut self, project_id: ProjectId) -> ThreadId {
        match self
            .ok(Request::CreateThread {
                project_id,
                agent_id: AgentId::new("mock"),
                workspace: Default::default(),
            })
            .await
        {
            Response::ThreadCreated(thread_id) => thread_id,
            response => panic!("unexpected response: {response:?}"),
        }
    }

    async fn call_tool(&mut self, caller: ToolCaller, name: &str, arguments: Value) -> ToolResult {
        match self
            .ok(Request::CallTool {
                caller,
                name: name.into(),
                arguments,
            })
            .await
        {
            Response::ToolResult(result) => result,
            response => panic!("unexpected response: {response:?}"),
        }
    }

    /// Calls a tool as the thread, expecting it to succeed.
    async fn tool(&mut self, thread_id: ThreadId, name: &str, arguments: Value) -> Value {
        let result = self
            .call_tool(ToolCaller::Thread(thread_id), name, arguments)
            .await;
        assert!(!result.is_error, "{name} failed: {}", result.value);
        result.value
    }

    /// Calls a tool expecting it to fail, and returns the failure's code.
    async fn tool_failure(&mut self, caller: ToolCaller, name: &str, arguments: Value) -> String {
        let result = self.call_tool(caller, name, arguments).await;
        assert!(result.is_error, "{name} succeeded: {}", result.value);
        result.value["code"]
            .as_str()
            .unwrap_or_default()
            .to_string()
    }

    fn project_thread(&self, thread_id: ThreadId) -> Option<&projects::Thread> {
        self.projects
            .as_ref()?
            .threads
            .iter()
            .find(|thread| thread.id == thread_id)
    }
}

fn thread_ids(list: &Value) -> Vec<u64> {
    list["threads"]
        .as_array()
        .map(|threads| {
            threads
                .iter()
                .filter_map(|thread| thread["threadId"].as_u64())
                .collect()
        })
        .unwrap_or_default()
}

#[tokio::test(flavor = "multi_thread")]
async fn agents_manage_the_threads_of_their_project() {
    let Some(server) = TestServer::start() else {
        return;
    };
    let other_project_dir = tempfile::tempdir().expect("temp dir");
    let mut client = server.connect().await;
    let Response::Session(session) = client.ok(Request::SubscribeSession).await else {
        panic!("expected a session snapshot");
    };
    client.projects = Some(session.projects);
    let project_id = client.add_project(server.project_dir.path()).await;
    let orchestrator = client.create_thread_in(project_id).await;
    let other_project = client.add_project(other_project_dir.path()).await;
    let elsewhere = client.create_thread_in(other_project).await;
    let connection = ConnectionId::Thread(orchestrator);
    client.subscribe_thread(connection).await;
    client
        .wait_until(|client| !client.thread(connection).config_options().is_empty())
        .await;

    let capabilities = client
        .tool(orchestrator, "orchestrator_capabilities", json!({}))
        .await;
    assert_eq!(capabilities["currentThreadId"], json!(orchestrator.0));
    assert_eq!(capabilities["agentId"], json!("mock"));
    assert_eq!(capabilities["agents"][0]["agentId"], json!("mock"));

    // A launched thread runs its prompt with the chosen model, and is marked as the agent's.
    let launched = client
        .tool(
            orchestrator,
            "agentz_thread_launch",
            json!({"prompt": "hello", "title": "Worker", "model": "Haiku"}),
        )
        .await;
    let worker = ThreadId(launched["threadId"].as_u64().expect("a thread id"));
    assert_eq!(launched["createdBy"], json!("agent"));
    assert_eq!(launched["createdByThreadId"], json!(orchestrator.0));
    assert_eq!(launched["model"], json!("haiku"));
    let waited = client
        .tool(
            orchestrator,
            "agentz_thread_wait",
            json!({"threadId": worker.0}),
        )
        .await;
    assert_eq!(waited["timedOut"], json!(false));
    assert_eq!(waited["status"], json!("idle"));
    assert_eq!(waited["lastAgentMessage"], json!("Echo: hello"));
    client
        .wait_until(|client| {
            client.project_thread(worker).is_some_and(|thread| {
                thread.created_by == Some(ThreadCreator::Thread(orchestrator))
                    && thread.title == "Worker"
                    && thread.has_custom_title
            })
        })
        .await;
    let worker_connection = ConnectionId::Thread(worker);
    client.subscribe_thread(worker_connection).await;
    let worker_view = client.thread(worker_connection);
    assert_eq!(
        worker_view.state.prompts_from_agents,
        vec![(0, ThreadCreator::Thread(orchestrator))]
    );
    assert_eq!(worker_view.model_name().as_deref(), Some("Haiku"));

    let read = client
        .tool(
            orchestrator,
            "agentz_thread_read",
            json!({"threadId": worker.0}),
        )
        .await;
    assert_eq!(read["thread"]["title"], json!("Worker"));
    assert_eq!(read["items"][0]["type"], json!("user_message"));
    assert_eq!(read["items"][0]["createdBy"], json!("agent"));
    assert_eq!(read["items"][1]["text"], json!("Echo: hello"));
    assert_eq!(read["hasMore"], json!(false));
    let rest = client
        .tool(
            orchestrator,
            "agentz_thread_read",
            json!({"threadId": worker.0, "afterPosition": 0, "view": "activity"}),
        )
        .await;
    assert_eq!(rest["items"][0]["position"], json!(1));
    assert_eq!(rest["items"][1]["type"], json!("tool_call"));

    let list = client
        .tool(orchestrator, "agentz_thread_list", json!({}))
        .await;
    assert_eq!(thread_ids(&list), vec![worker.0, orchestrator.0]);
    assert_eq!(list["total"], json!(2));

    // Retrying with the same request id doesn't launch another thread.
    let first = client
        .tool(
            orchestrator,
            "agentz_thread_launch",
            json!({"clientRequestId": "retry-1"}),
        )
        .await;
    let retried = client
        .tool(
            orchestrator,
            "agentz_thread_launch",
            json!({"clientRequestId": "retry-1"}),
        )
        .await;
    assert_eq!(first["threadId"], retried["threadId"]);
    let list = client
        .tool(orchestrator, "agentz_thread_list", json!({"limit": 2}))
        .await;
    assert_eq!(list["total"], json!(3));
    assert_eq!(list["nextCursor"], json!(2));

    client
        .tool(
            orchestrator,
            "agentz_thread_update",
            json!({"threadId": worker.0, "title": "Renamed"}),
        )
        .await;
    client
        .tool(
            orchestrator,
            "agentz_thread_organize",
            json!({"threadId": worker.0, "action": "archive"}),
        )
        .await;
    let archived = client
        .tool(
            orchestrator,
            "agentz_thread_list",
            json!({"archived": true}),
        )
        .await;
    assert_eq!(thread_ids(&archived), vec![worker.0]);
    assert_eq!(archived["threads"][0]["title"], json!("Renamed"));

    // The CLI run in a project's folder manages that project, as nobody's thread.
    let subdirectory = server.project_dir.path().join("src");
    std::fs::create_dir_all(&subdirectory).expect("create a folder");
    let result = client
        .call_tool(
            ToolCaller::Directory(subdirectory),
            "agentz_thread_list",
            json!({}),
        )
        .await;
    assert!(!result.is_error, "{}", result.value);
    assert_eq!(result.value["currentThreadId"], Value::Null);
    assert_eq!(result.value["total"], json!(2));

    // The policy.
    let code = client
        .tool_failure(
            ToolCaller::Thread(orchestrator),
            "agentz_thread_read",
            json!({"threadId": elsewhere.0}),
        )
        .await;
    assert_eq!(code, "thread_not_found");
    let code = client
        .tool_failure(
            ToolCaller::Thread(orchestrator),
            "agentz_thread_send",
            json!({"threadId": orchestrator.0, "message": "hi me"}),
        )
        .await;
    assert_eq!(code, "thread_not_sendable");
    let code = client
        .tool_failure(
            ToolCaller::Session("forged".into()),
            "agentz_thread_list",
            json!({}),
        )
        .await;
    assert_eq!(code, "capability_denied");
    let code = client
        .tool_failure(
            ToolCaller::Directory(other_project_dir.path().parent().expect("a parent").into()),
            "agentz_thread_list",
            json!({}),
        )
        .await;
    assert_eq!(code, "capability_denied");
    let code = client
        .tool_failure(
            ToolCaller::Thread(orchestrator),
            "agentz_thread_launch",
            json!({"model": "gpt-9"}),
        )
        .await;
    assert_eq!(code, "model_unavailable");
    let code = client
        .tool_failure(
            ToolCaller::Thread(orchestrator),
            "agentz_thread_delete",
            json!({}),
        )
        .await;
    assert_eq!(code, "invalid_request");
}

#[tokio::test(flavor = "multi_thread")]
async fn agents_queue_restart_and_interrupt_turns() {
    let Some(server) = TestServer::start() else {
        return;
    };
    let mut client = server.connect().await;
    let project_id = client.add_project(server.project_dir.path()).await;
    let orchestrator = client.create_thread_in(project_id).await;
    let worker = client.create_thread_in(project_id).await;

    let sent = client
        .tool(
            orchestrator,
            "agentz_thread_send",
            json!({"threadId": worker.0, "message": "slow"}),
        )
        .await;
    assert_eq!(sent["delivery"], json!("started"));
    let sent = client
        .tool(
            orchestrator,
            "agentz_thread_send",
            json!({"threadId": worker.0, "message": "second"}),
        )
        .await;
    assert_eq!(sent["delivery"], json!("queued"));
    let timed_out = client
        .tool(
            orchestrator,
            "agentz_thread_wait",
            json!({"threadId": worker.0, "timeoutMs": 50}),
        )
        .await;
    assert_eq!(timed_out["timedOut"], json!(true));
    let waited = client
        .tool(
            orchestrator,
            "agentz_thread_wait",
            json!({"threadId": worker.0}),
        )
        .await;
    assert_eq!(waited["lastAgentMessage"], json!("Echo: second"));

    let connection = ConnectionId::Thread(worker);
    client.subscribe_thread(connection).await;
    let entries = client.thread(connection).entries().to_vec();
    assert_eq!(entries[0], Entry::UserMessage("slow".into()));
    assert_eq!(
        entries[1],
        Entry::AgentMessage("One two three four five".into())
    );
    assert_eq!(entries[2], Entry::UserMessage("second".into()));
    assert_eq!(client.thread(connection).state.prompts_from_agents.len(), 2);

    client
        .tool(
            orchestrator,
            "agentz_thread_send",
            json!({"threadId": worker.0, "message": "slow"}),
        )
        .await;
    let sent = client
        .tool(
            orchestrator,
            "agentz_thread_send",
            json!({"threadId": worker.0, "message": "instead", "mode": "restart"}),
        )
        .await;
    assert_eq!(sent["delivery"], json!("restarted"));
    let waited = client
        .tool(
            orchestrator,
            "agentz_thread_wait",
            json!({"threadId": worker.0}),
        )
        .await;
    assert_eq!(waited["lastAgentMessage"], json!("Echo: instead"));

    client
        .tool(
            orchestrator,
            "agentz_thread_send",
            json!({"threadId": worker.0, "message": "slow"}),
        )
        .await;
    client
        .tool(
            orchestrator,
            "agentz_thread_send",
            json!({"threadId": worker.0, "message": "dropped"}),
        )
        .await;
    let interrupted = client
        .tool(
            orchestrator,
            "agentz_thread_interrupt",
            json!({"threadId": worker.0}),
        )
        .await;
    assert_eq!(interrupted["status"], json!("interrupt_requested"));
    assert_eq!(interrupted["droppedQueuedMessages"], json!(1));
    let waited = client
        .tool(
            orchestrator,
            "agentz_thread_wait",
            json!({"threadId": worker.0}),
        )
        .await;
    assert_eq!(waited["status"], json!("idle"));
    let interrupted = client
        .tool(
            orchestrator,
            "agentz_thread_interrupt",
            json!({"threadId": worker.0}),
        )
        .await;
    assert_eq!(interrupted["status"], json!("no_active_run"));
}

impl TestClient {
    /// Waits for the thread's agent to report its settings, so it's ready for prompts.
    async fn wait_until_ready(&mut self, thread_id: ThreadId) {
        let connection = ConnectionId::Thread(thread_id);
        if !self.threads.contains_key(&connection) {
            self.subscribe_thread(connection).await;
        }
        self.wait_until(|client| !client.thread(connection).config_options().is_empty())
            .await;
    }

    fn task(&self, thread_id: ThreadId) -> Option<&projects::Task> {
        self.project_thread(thread_id)?.task.as_ref()
    }

    fn user_messages(&self, thread_id: ThreadId) -> Vec<String> {
        self.thread(ConnectionId::Thread(thread_id))
            .entries()
            .iter()
            .filter_map(|entry| match entry {
                Entry::UserMessage(text) => Some(text.to_string()),
                _ => None,
            })
            .collect()
    }
}

fn task_id(result: &Value) -> ThreadId {
    ThreadId(result["taskId"].as_u64().expect("a task id"))
}

fn config_value(view: &ThreadView, config_id: &str) -> Option<String> {
    view.config_options().iter().find_map(|option| {
        let acp::SessionConfigKind::Select(select) = &option.kind else {
            return None;
        };
        (option.id.0.as_ref() == config_id).then(|| select.current_value.0.to_string())
    })
}

#[tokio::test(flavor = "multi_thread")]
async fn agents_work_on_other_machines_through_the_app() {
    let (Some(here), Some(there)) = (TestServer::start(), TestServer::start()) else {
        return;
    };
    let mut caller = here.connect().await;
    let Response::Session(session) = caller.ok(Request::SubscribeSession).await else {
        panic!("expected a session snapshot");
    };
    caller.projects = Some(session.projects);
    let project_id = caller.add_project(here.project_dir.path()).await;
    let orchestrator = caller.create_thread_in(project_id).await;
    let connection = ConnectionId::Thread(orchestrator);
    caller.subscribe_thread(connection).await;
    caller
        .wait_until(|client| !client.thread(connection).config_options().is_empty())
        .await;

    // Without the app, other machines are out of reach.
    let code = caller
        .tool_failure(
            ToolCaller::Thread(orchestrator),
            "agentz_thread_list",
            json!({"machine": "devbox"}),
        )
        .await;
    assert_eq!(code, "machine_unavailable");

    // The app: it tells this server about the other machine, and runs relayed calls there.
    let mut app_there = there.connect().await;
    app_there.add_project(there.project_dir.path()).await;
    let mut app_here = here.connect().await;
    app_here
        .ok(Request::SetPeers(Peers {
            this_machine: "mac".into(),
            machines: vec![
                PeerMachine {
                    name: "devbox".into(),
                    online: true,
                },
                PeerMachine {
                    name: "laptop".into(),
                    online: false,
                },
            ],
            checkouts: vec![PeerCheckouts {
                project_id,
                checkouts: vec![PeerCheckout {
                    machine: "devbox".into(),
                    path: there.project_dir.path().to_path_buf(),
                }],
            }],
        }))
        .await;
    let relay = tokio::spawn(async move {
        loop {
            let ServerMessage::Event(Event::RelayToolCall(call)) = app_here.next_message().await
            else {
                continue;
            };
            assert_eq!(call.machine, "devbox");
            let result = app_there
                .call_tool(ToolCaller::Directory(call.path), &call.name, call.arguments)
                .await;
            app_here
                .ok(Request::RelayToolResult {
                    relay_id: call.relay_id,
                    result,
                })
                .await;
        }
    });

    let capabilities = caller
        .tool(orchestrator, "orchestrator_capabilities", json!({}))
        .await;
    assert_eq!(capabilities["machine"]["name"], json!("mac"));
    assert_eq!(capabilities["otherMachines"][0]["name"], json!("devbox"));
    assert_eq!(
        capabilities["otherMachines"][0]["hasThisProject"],
        json!(true)
    );
    assert_eq!(
        capabilities["otherMachines"][1]["status"],
        json!("disconnected")
    );
    let remote_capabilities = caller
        .tool(
            orchestrator,
            "orchestrator_capabilities",
            json!({"machine": "DevBox"}),
        )
        .await;
    assert_eq!(remote_capabilities["machine"]["status"], json!("connected"));
    assert_eq!(remote_capabilities["agents"][0]["agentId"], json!("mock"));

    // A thread launched there takes this thread's agent, and is followed by machine.
    let launched = caller
        .tool(
            orchestrator,
            "agentz_thread_launch",
            json!({"prompt": "hello", "machine": "devbox"}),
        )
        .await;
    assert_eq!(launched["machine"], json!("devbox"));
    assert_eq!(launched["agentId"], json!("mock"));
    let remote_thread = launched["threadId"].clone();
    let waited = caller
        .tool(
            orchestrator,
            "agentz_thread_wait",
            json!({"threadId": remote_thread, "machine": "devbox"}),
        )
        .await;
    assert_eq!(waited["lastAgentMessage"], json!("Echo: hello"));

    // The project's list covers both machines.
    let list = caller
        .tool(orchestrator, "agentz_thread_list", json!({}))
        .await;
    let listed: Vec<(Value, Value)> = list["threads"]
        .as_array()
        .expect("threads")
        .iter()
        .map(|thread| (thread["machine"].clone(), thread["threadId"].clone()))
        .collect();
    assert!(listed.contains(&(json!("devbox"), remote_thread.clone())));
    assert!(listed.contains(&(json!("mac"), json!(orchestrator.0))));
    assert_eq!(list["total"], json!(2));

    // A task for another machine runs as an ordinary thread there.
    let delegated = caller
        .tool(
            orchestrator,
            "delegate_task",
            json!({"task": "hi", "mode": "wait", "machine": "devbox"}),
        )
        .await;
    assert_eq!(delegated["lastAgentMessage"], json!("Echo: hi"));
    assert_eq!(delegated["machine"], json!("devbox"));
    assert!(delegated["note"].is_string());

    for (machine, expected) in [
        ("nowhere", "invalid_request"),
        ("laptop", "machine_unavailable"),
    ] {
        let code = caller
            .tool_failure(
                ToolCaller::Thread(orchestrator),
                "agentz_thread_list",
                json!({"machine": machine}),
            )
            .await;
        assert_eq!(code, expected);
    }
    relay.abort();
}

#[tokio::test(flavor = "multi_thread")]
async fn agents_delegate_tasks_to_subthreads() {
    let Some(server) = TestServer::start() else {
        return;
    };
    let mut client = server.connect().await;
    let Response::Session(session) = client.ok(Request::SubscribeSession).await else {
        panic!("expected a session snapshot");
    };
    client.projects = Some(session.projects);
    let project_id = client.add_project(server.project_dir.path()).await;
    let parent = client.create_thread_in(project_id).await;
    client.wait_until_ready(parent).await;
    client
        .ok(Request::SetConfigOption {
            connection: ConnectionId::Thread(parent),
            config_id: acp::SessionConfigId::new("mode"),
            value: acp::SessionConfigOptionValue::value_id("plan"),
        })
        .await;
    client
        .wait_until(|client| {
            config_value(client.thread(ConnectionId::Thread(parent)), "mode").as_deref()
                == Some("plan")
        })
        .await;

    // Waiting returns the result, and the parent isn't told again.
    let waited = client
        .tool(
            parent,
            "delegate_task",
            json!({"task": "hello", "mode": "wait", "title": "Helper", "role": "research"}),
        )
        .await;
    let helper = task_id(&waited);
    assert_eq!(waited["status"], json!("completed"));
    assert_eq!(waited["workState"], json!("result_available"));
    assert_eq!(waited["summary"], json!("Echo: hello"));
    assert_eq!(waited["title"], json!("Helper"));
    assert_eq!(waited["waitTimedOut"], json!(false));
    client
        .wait_until(|client| client.task(helper).is_some_and(|task| task.delivered))
        .await;
    let helper_thread = client.project_thread(helper).expect("the subthread");
    assert_eq!(
        helper_thread.created_by,
        Some(ThreadCreator::Thread(parent))
    );
    assert_eq!(
        helper_thread.task.as_ref().map(|task| task.prompt.as_str()),
        Some("hello")
    );
    // The child keeps the parent's mode.
    client.subscribe_thread(ConnectionId::Thread(helper)).await;
    assert_eq!(
        config_value(client.thread(ConnectionId::Thread(helper)), "mode").as_deref(),
        Some("plan")
    );
    // Subthreads aren't in the thread list, and only take their task.
    let list = client.tool(parent, "agentz_thread_list", json!({})).await;
    assert_eq!(thread_ids(&list), vec![parent.0]);
    let refused = client
        .request(Request::Prompt {
            connection: ConnectionId::Thread(helper),
            text: "more".into(),
        })
        .await;
    assert!(refused.is_err());
    let code = client
        .tool_failure(
            ToolCaller::Thread(parent),
            "agentz_thread_send",
            json!({"threadId": helper.0, "message": "more"}),
        )
        .await;
    assert_eq!(code, "thread_not_sendable");

    // An async task's end is announced to the parent, as sent by the task.
    let started = client
        .tool(parent, "delegate_task", json!({"task": "slow"}))
        .await;
    let slow = task_id(&started);
    assert!(
        matches!(started["status"].as_str(), Some("queued" | "running")),
        "{started}"
    );
    let retried = client
        .tool(
            parent,
            "delegate_task",
            json!({"task": "slow", "clientRequestId": "once"}),
        )
        .await;
    let again = client
        .tool(
            parent,
            "delegate_task",
            json!({"task": "slow", "clientRequestId": "once"}),
        )
        .await;
    assert_eq!(retried["taskId"], again["taskId"]);
    let once = task_id(&retried);
    client
        .wait_until(|client| {
            client
                .user_messages(parent)
                .iter()
                .any(|message| message.contains(&slow.0.to_string()))
        })
        .await;
    let announced = client
        .user_messages(parent)
        .into_iter()
        .find(|message| message.contains(&slow.0.to_string()))
        .expect("an announcement");
    assert!(announced.starts_with("Delegated task"), "{announced}");
    let status = client
        .tool(parent, "task_status", json!({"taskId": slow.0}))
        .await;
    assert_eq!(status["status"], json!("completed"));
    assert_eq!(status["summary"], json!("One two three four five"));
    client
        .wait_until(|client| client.task(once).is_some_and(|task| task.delivered))
        .await;

    // Cancelling ends the task without telling the parent.
    let cancelled = task_id(
        &client
            .tool(parent, "delegate_task", json!({"task": "slow"}))
            .await,
    );
    let cancel = client
        .tool(
            parent,
            "task_cancel",
            json!({"taskId": cancelled.0, "reason": "Not needed"}),
        )
        .await;
    assert_eq!(cancel["status"], json!("cancel_requested"));
    let status = client
        .tool(parent, "task_status", json!({"taskId": cancelled.0}))
        .await;
    assert_eq!(status["status"], json!("cancelled"));
    assert_eq!(status["summary"], json!("Not needed"));
    let cancel = client
        .tool(parent, "task_cancel", json!({"taskId": cancelled.0}))
        .await;
    assert_eq!(cancel["status"], json!("cancelled"));

    // A task that delegates ends only once its own task has ended and it has heard.
    let outer = task_id(
        &client
            .tool(parent, "delegate_task", json!({"task": "slow"}))
            .await,
    );
    client.wait_until_ready(outer).await;
    let inner = task_id(
        &client
            .tool(outer, "delegate_task", json!({"task": "hello"}))
            .await,
    );
    client
        .wait_until(|client| {
            client
                .task(outer)
                .is_some_and(|task| task.outcome.is_some())
        })
        .await;
    let status = client
        .tool(parent, "task_status", json!({"taskId": outer.0}))
        .await;
    assert_eq!(status["status"], json!("completed"));
    assert_eq!(
        status["summary"],
        json!(format!(
            "Echo: Delegated task {} reached a terminal state. Use task_status with taskId {} to read the result.",
            inner.0, inner.0
        ))
    );
    assert_eq!(client.task(inner).map(|task| task.delivered), Some(true));

    // The policy.
    let code = client
        .tool_failure(
            ToolCaller::Thread(parent),
            "task_status",
            json!({"taskId": inner.0}),
        )
        .await;
    assert_eq!(code, "task_not_found");
    let code = client
        .tool_failure(
            ToolCaller::Directory(server.project_dir.path().into()),
            "delegate_task",
            json!({"task": "hello"}),
        )
        .await;
    assert_eq!(code, "capability_denied");
    let code = client
        .tool_failure(
            ToolCaller::Thread(parent),
            "delegate_task",
            json!({"task": "hello", "role": "boss"}),
        )
        .await;
    assert_eq!(code, "invalid_request");
    let code = client
        .tool_failure(
            ToolCaller::Thread(helper),
            "delegate_task",
            json!({"task": "hello"}),
        )
        .await;
    assert_eq!(code, "parent_not_active");

    // Deleting the parent deletes its subthreads.
    client.ok(Request::DeleteThread(parent)).await;
    client
        .wait_until(|client| client.project_thread(helper).is_none())
        .await;
}

#[tokio::test(flavor = "multi_thread")]
async fn subthread_permission_requests_block_the_parent() {
    let Some(server) = TestServer::start() else {
        return;
    };
    let mut client = server.connect().await;
    let Response::Session(session) = client.ok(Request::SubscribeSession).await else {
        panic!("expected a session snapshot");
    };
    client.projects = Some(session.projects);
    let project_id = client.add_project(server.project_dir.path()).await;
    let parent = client.create_thread_in(project_id).await;
    let child = task_id(
        &client
            .tool(parent, "delegate_task", json!({"task": "permission"}))
            .await,
    );
    let connection = ConnectionId::Thread(child);
    client.subscribe_thread(connection).await;
    client
        .wait_until(|client| {
            !client
                .thread(connection)
                .state
                .permission_requests
                .is_empty()
                && client.projects.as_ref().is_some_and(|projects| {
                    ProjectStoreSnapshot(projects).is_thread_or_subthread_blocked(parent)
                })
        })
        .await;
    let status = client
        .tool(parent, "task_status", json!({"taskId": child.0}))
        .await;
    assert_eq!(status["status"], json!("waiting"));
    let tool_call_id = client.thread(connection).state.permission_requests[0]
        .tool_call_id
        .clone();
    client
        .ok(Request::RespondToPermission {
            connection,
            tool_call_id,
            option_id: acp::PermissionOptionId::new("allow"),
        })
        .await;
    client
        .wait_until(|client| {
            client
                .task(child)
                .is_some_and(|task| task.outcome.is_some())
        })
        .await;
    let status = client
        .tool(parent, "task_status", json!({"taskId": child.0}))
        .await;
    assert_eq!(status["status"], json!("completed"));
}

/// The app's view of a snapshot, for the questions it asks of one.
struct ProjectStoreSnapshot<'a>(&'a ProjectsSnapshot);

impl ProjectStoreSnapshot<'_> {
    fn is_thread_or_subthread_blocked(&self, thread_id: ThreadId) -> bool {
        projects::ProjectStore::from_snapshot(self.0.clone())
            .is_thread_or_subthread_blocked(thread_id)
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn tasks_running_when_the_server_stops_end_as_interrupted() {
    let Some(server) = TestServer::start() else {
        return;
    };
    let mut client = server.connect().await;
    let project_id = client.add_project(server.project_dir.path()).await;
    let parent = client.create_thread_in(project_id).await;
    let child = task_id(
        &client
            .tool(parent, "delegate_task", json!({"task": "slow"}))
            .await,
    );
    client.ok(Request::Shutdown).await;
    tokio::time::timeout(TIMEOUT, server.handle.stopped())
        .await
        .expect("the server stops");
    drop(client);

    let Some(server) = TestServer::start_with(server.data_dir, server.project_dir) else {
        return;
    };
    let mut client = server.connect().await;
    let Response::Session(session) = client.ok(Request::SubscribeSession).await else {
        panic!("expected a session snapshot");
    };
    client.projects = Some(session.projects);
    let outcome = client
        .task(child)
        .and_then(|task| task.outcome.clone())
        .expect("the task ended");
    assert_eq!(outcome.end, projects::TaskEnd::Interrupted);
    // The parent is told once the server is back.
    client.subscribe_thread(ConnectionId::Thread(parent)).await;
    client
        .wait_until(|client| {
            client
                .user_messages(parent)
                .iter()
                .any(|message| message.starts_with("Delegated task"))
        })
        .await;
    let status = client
        .tool(parent, "task_status", json!({"taskId": child.0}))
        .await;
    assert_eq!(status["status"], json!("interrupted"));
}

impl TestClient {
    /// Sends a prompt and waits for the turn, checkpoints included, to end.
    async fn prompt_and_wait(&mut self, thread_id: ThreadId, text: &str) {
        let connection = ConnectionId::Thread(thread_id);
        let before = self.user_messages(thread_id).len();
        self.ok(Request::Prompt {
            connection,
            text: text.into(),
        })
        .await;
        self.wait_until(|client| {
            client.user_messages(thread_id).len() > before
                && !client.thread(connection).is_working()
        })
        .await;
    }

    async fn thread_diff(&mut self, thread_id: ThreadId, scope: DiffScope) -> ThreadDiff {
        match self.ok(Request::ThreadDiff { thread_id, scope }).await {
            Response::ThreadDiff(diff) => diff,
            response => panic!("unexpected response: {response:?}"),
        }
    }
}

async fn git(dir: &std::path::Path, args: &[&str]) -> String {
    let output = tokio::process::Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .await
        .expect("git runs");
    assert!(output.status.success(), "git {args:?} failed");
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn diff_files(diff: &ThreadDiff) -> Vec<(&str, FileChange, u32, u32)> {
    diff.files
        .iter()
        .map(|file| {
            (
                file.path.as_str(),
                file.change,
                file.additions,
                file.deletions,
            )
        })
        .collect()
}

#[tokio::test(flavor = "multi_thread")]
async fn turns_are_checkpointed_for_diffs() {
    if tokio::process::Command::new("git")
        .arg("--version")
        .output()
        .await
        .is_err()
    {
        return;
    }
    let Some(server) = TestServer::start() else {
        return;
    };
    let repository = server.project_dir.path();
    git(repository, &["init", "-q", "-b", "main"]).await;
    git(repository, &["config", "user.name", "Test"]).await;
    git(repository, &["config", "user.email", "test@example.com"]).await;
    std::fs::write(repository.join("README.md"), "one\n").expect("a file");
    git(repository, &["add", "."]).await;
    git(repository, &["commit", "-q", "-m", "first"]).await;

    let mut client = server.connect().await;
    assert!(
        client
            .welcome
            .capabilities
            .iter()
            .any(|capability| capability == agentz_protocol::CAPABILITY_THREAD_DIFF)
    );
    let Response::Session(session) = client.ok(Request::SubscribeSession).await else {
        panic!("expected a session snapshot");
    };
    client.projects = Some(session.projects);
    let project_id = client.add_project(repository).await;
    let thread = client.create_thread_in(project_id).await;
    client.wait_until_ready(thread).await;
    let diff = client.thread_diff(thread, DiffScope::All).await;
    assert_eq!((diff.status, diff.turns), (DiffStatus::NoTurns, 0));

    client
        .prompt_and_wait(thread, "write src/a.txt hello")
        .await;
    let diff = client.thread_diff(thread, DiffScope::All).await;
    assert_eq!((diff.status.clone(), diff.turns), (DiffStatus::Ready, 1));
    assert_eq!(
        diff_files(&diff),
        vec![("src/a.txt", FileChange::Added, 1, 0)]
    );

    client.prompt_and_wait(thread, "write README.md two").await;
    let latest = client.thread_diff(thread, DiffScope::LatestTurn).await;
    assert_eq!(latest.turns, 2);
    assert_eq!(
        diff_files(&latest),
        vec![("README.md", FileChange::Modified, 1, 1)]
    );
    let all = client.thread_diff(thread, DiffScope::All).await;
    assert_eq!(
        diff_files(&all),
        vec![
            ("README.md", FileChange::Modified, 1, 1),
            ("src/a.txt", FileChange::Added, 1, 0),
        ]
    );
    // The user's branch has no new commits.
    assert_eq!(
        git(repository, &["rev-list", "--count", "HEAD"])
            .await
            .trim(),
        "1"
    );

    // Agents read diffs too.
    let files = client
        .tool(
            thread,
            "agentz_thread_diff",
            json!({"scope": "latest_turn", "format": "files"}),
        )
        .await;
    assert_eq!(files["status"], json!("ready"));
    assert_eq!(files["files"][0]["path"], json!("README.md"));
    assert_eq!(files["files"][0]["change"], json!("modified"));
    assert!(files.get("patch").is_none());
    let patch = client
        .tool(thread, "agentz_thread_diff", json!({"threadId": thread.0}))
        .await;
    let text = patch["patch"].as_str().unwrap_or_default();
    assert!(text.contains("+two") && text.contains("+hello"), "{text}");

    // A folder outside git has no checkpoints.
    let plain = tempfile::tempdir().expect("temp dir");
    let plain_project = client.add_project(plain.path()).await;
    let plain_thread = client.create_thread_in(plain_project).await;
    client.wait_until_ready(plain_thread).await;
    client.prompt_and_wait(plain_thread, "write a.txt hi").await;
    let diff = client.thread_diff(plain_thread, DiffScope::All).await;
    assert_eq!(diff.status, DiffStatus::NotRepository);

    // Deleting the thread removes its checkpoints.
    assert!(
        !git(repository, &["for-each-ref", "refs/agentz/"])
            .await
            .is_empty()
    );
    client.ok(Request::DeleteThread(thread)).await;
    tokio::time::timeout(TIMEOUT, async {
        while !git(repository, &["for-each-ref", "refs/agentz/"])
            .await
            .is_empty()
        {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("the checkpoints are removed");
}

async fn wait_for_file(path: &std::path::Path) {
    tokio::time::timeout(TIMEOUT, async {
        while !path.exists() {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap_or_else(|_| panic!("{} never appeared", path.display()));
}

#[tokio::test(flavor = "multi_thread")]
async fn threads_work_in_worktrees_and_pastures() {
    if tokio::process::Command::new("git")
        .arg("--version")
        .output()
        .await
        .is_err()
    {
        return;
    }
    let Some(server) = TestServer::start() else {
        return;
    };
    let repository = server.project_dir.path();
    git(repository, &["init", "-q", "-b", "main"]).await;
    git(repository, &["config", "user.name", "Test"]).await;
    git(repository, &["config", "user.email", "test@example.com"]).await;
    std::fs::write(repository.join("README.md"), "one\n").expect("a file");
    git(repository, &["add", "."]).await;
    git(repository, &["commit", "-q", "-m", "first"]).await;

    let mut client = server.connect().await;
    assert!(
        client
            .welcome
            .capabilities
            .iter()
            .any(|capability| capability == agentz_protocol::CAPABILITY_WORKSPACES)
    );
    let Response::Session(session) = client.ok(Request::SubscribeSession).await else {
        panic!("expected a session snapshot");
    };
    client.projects = Some(session.projects);
    let project_id = client.add_project(repository).await;
    let Response::ProjectGit(project_git) = client.ok(Request::ProjectGit(project_id)).await else {
        panic!("expected the project's git");
    };
    assert!(project_git.is_repository);
    assert_eq!(project_git.branch.as_deref(), Some("main"));
    assert_eq!(project_git.branches, vec!["main".to_string()]);

    // A new thread in a new worktree works there, and its turns are checkpointed there.
    let Response::ThreadCreated(thread) = client
        .ok(Request::CreateThread {
            project_id,
            agent_id: AgentId::new("mock"),
            workspace: WorkspaceChoice::New {
                kind: WorkspaceKind::Worktree,
                base: None,
                branch: Some("feature".into()),
            },
        })
        .await
    else {
        panic!("expected a thread");
    };
    let worktree = client
        .project_thread(thread)
        .and_then(|thread| thread.workspace.clone())
        .expect("the thread works in a workspace");
    assert!(worktree.ends_with("feature"), "{}", worktree.display());
    client.wait_until_ready(thread).await;
    client.prompt_and_wait(thread, "write a.txt hi").await;
    assert!(worktree.join("a.txt").exists());
    assert!(!repository.join("a.txt").exists());
    let diff = client.thread_diff(thread, DiffScope::All).await;
    assert_eq!(diff_files(&diff), vec![("a.txt", FileChange::Added, 1, 0)]);

    // Alone in its worktree, the thread can put its files back.
    assert_eq!(diff.restore, RestoreAvailability::Available);
    client.prompt_and_wait(thread, "write c.txt bye").await;
    let Response::ThreadDiff(restored) = client
        .ok(Request::RestoreCheckpoint {
            thread_id: thread,
            scope: DiffScope::LatestTurn,
        })
        .await
    else {
        panic!("expected the diff after restoring");
    };
    assert!(!worktree.join("c.txt").exists());
    assert!(worktree.join("a.txt").exists());
    assert_eq!(restored.turns, 1);
    assert_eq!(
        diff_files(&restored),
        vec![("a.txt", FileChange::Added, 1, 0)]
    );

    let status = client
        .tool(thread, "agentz_workspace_status", json!({}))
        .await;
    assert_eq!(status["kind"], json!("worktree"));
    assert_eq!(status["branch"], json!("feature"));
    assert_eq!(status["baseRef"], json!("main"));
    let list = client
        .tool(thread, "agentz_workspace_list", json!({}))
        .await;
    assert_eq!(list["workspaces"][0]["threadIds"], json!([thread.0]));
    let feature = list["branches"]
        .as_array()
        .and_then(|branches| branches.iter().find(|branch| branch["name"] == "feature"))
        .expect("the worktree's branch is listed");
    assert_eq!(feature["checkouts"][0]["kind"], json!("worktree"));

    // A delegated task works where its parent does unless told otherwise; this one gets a
    // worktree of its own, made before the call is answered.
    let delegated = client
        .tool(
            thread,
            "delegate_task",
            json!({"task": "write b.txt hi", "workspaceStrategy": {"type": "worktree", "branch": "task"}}),
        )
        .await;
    let task = ThreadId(delegated["taskId"].as_u64().expect("a task id"));
    let task_folder = client
        .project_thread(task)
        .and_then(|thread| thread.workspace.clone())
        .expect("the task works in a workspace");
    assert!(task_folder.ends_with("task"), "{}", task_folder.display());
    wait_for_file(&task_folder.join("b.txt")).await;
    // A launched thread can join an existing worktree.
    let launched = client
        .tool(
            thread,
            "agentz_thread_launch",
            json!({"workspaceStrategy": {"type": "existing", "path": worktree}}),
        )
        .await;
    let launched = ThreadId(launched["threadId"].as_u64().expect("a thread id"));
    assert_eq!(
        client
            .project_thread(launched)
            .and_then(|thread| thread.workspace.clone()),
        Some(worktree.clone())
    );
    // Sharing it, neither may restore.
    let shared = client.thread_diff(thread, DiffScope::All).await;
    assert!(matches!(
        shared.restore,
        RestoreAvailability::Unavailable(_)
    ));
    assert!(
        client
            .request(Request::RestoreCheckpoint {
                thread_id: thread,
                scope: DiffScope::All,
            })
            .await
            .is_err()
    );

    // Removing asks first when work would be lost.
    let Response::WorkspaceRemoval(removal) = client
        .ok(Request::RemoveWorkspace {
            project_id,
            path: worktree.clone(),
            force: false,
        })
        .await
    else {
        panic!("expected a removal");
    };
    assert!(matches!(removal, WorkspaceRemoval::NeedsConfirmation(_)));
    let Response::WorkspaceRemoval(removal) = client
        .ok(Request::RemoveWorkspace {
            project_id,
            path: worktree.clone(),
            force: true,
        })
        .await
    else {
        panic!("expected a removal");
    };
    assert_eq!(removal, WorkspaceRemoval::Removed);
    assert!(!worktree.exists());
    assert!(
        client.projects.as_ref().expect("projects").projects[0]
            .workspaces
            .iter()
            .all(|workspace| workspace.path != worktree)
    );

    // A thread in the checkout hands itself off, and its continuation runs in the new folder.
    let root_thread = client.create_thread_in(project_id).await;
    client.wait_until_ready(root_thread).await;
    let handoff = client
        .tool(
            root_thread,
            "agentz_workspace_handoff",
            json!({"type": "worktree", "branch": "moved", "continuationPrompt": "write c.txt hi"}),
        )
        .await;
    let moved = PathBuf::from(handoff["workspacePath"].as_str().expect("a path"));
    wait_for_file(&moved.join("c.txt")).await;
    assert!(!repository.join("c.txt").exists());
    let again = client
        .call_tool(
            ToolCaller::Thread(root_thread),
            "agentz_workspace_handoff",
            json!({"type": "worktree"}),
        )
        .await;
    assert_eq!(again.value["code"], json!("already_in_workspace"));

    if cfg!(target_os = "macos") {
        let Response::ThreadCreated(pasture_thread) = client
            .ok(Request::CreateThread {
                project_id,
                agent_id: AgentId::new("mock"),
                workspace: WorkspaceChoice::New {
                    kind: WorkspaceKind::Pasture,
                    base: None,
                    branch: Some("grazing".into()),
                },
            })
            .await
        else {
            panic!("expected a thread");
        };
        let pasture = client
            .project_thread(pasture_thread)
            .and_then(|thread| thread.workspace.clone())
            .expect("the thread works in a pasture");
        assert!(pasture.join("README.md").exists());
        let brought = client
            .tool(pasture_thread, "agentz_workspace_bring_back", json!({}))
            .await;
        assert!(
            brought["message"]
                .as_str()
                .is_some_and(|message| message.contains("grazing")),
            "{brought}"
        );
        assert!(
            git(repository, &["branch", "--list", "grazing"])
                .await
                .contains("grazing")
        );
    }
}

impl TestClient {
    async fn subscribe_terminal(&mut self, key: TerminalKey) {
        match self.ok(Request::SubscribeTerminal(key.clone())).await {
            Response::TerminalFrame(frame) => {
                assert!(frame.full);
                self.terminals.insert(key, frame);
            }
            response => panic!("unexpected response: {response:?}"),
        }
    }

    async fn type_into(&mut self, key: &TerminalKey, text: &str) {
        self.ok(Request::TerminalInput {
            terminal: key.clone(),
            input: TerminalInput::Bytes(text.as_bytes().to_vec()),
        })
        .await;
    }

    fn screen(&self, key: &TerminalKey) -> String {
        self.terminals
            .get(key)
            .map(TerminalFrame::text)
            .unwrap_or_default()
    }

    /// Reads frames until the terminal's screen contains `text`.
    async fn wait_for_screen(&mut self, key: &TerminalKey, text: &str) {
        let waited = tokio::time::timeout(TIMEOUT, async {
            while !self.screen(key).contains(text) {
                match self.next_message().await {
                    ServerMessage::Event(event) => self.apply(event),
                    message => panic!("unexpected message: {message:?}"),
                }
            }
        })
        .await;
        if waited.is_err() {
            panic!(
                "timed out waiting for {text:?}; the screen is:\n{}",
                self.screen(key)
            );
        }
    }
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
#[tokio::test(flavor = "multi_thread")]
async fn terminal_threads_show_where_they_are_and_what_runs() {
    let Some(server) = TestServer::start() else {
        return;
    };
    let mut client = server.connect().await;
    client.ok(Request::SubscribeSession).await;
    let project_id = client.add_project(server.project_dir.path()).await;
    let thread_id = match client
        .ok(Request::CreateTerminalThread {
            project_id,
            command: TerminalCommand { command: None },
            workspace: Default::default(),
        })
        .await
    {
        Response::ThreadCreated(thread_id) => thread_id,
        response => panic!("unexpected response: {response:?}"),
    };
    let snapshot = |client: &TestClient| client.projects.clone().expect("projects");
    let last_activity = move |client: &TestClient| {
        snapshot(client)
            .threads
            .iter()
            .find(|thread| thread.id == thread_id)
            .and_then(|thread| thread.last_activity_at)
    };
    let command = move |client: &TestClient| {
        snapshot(client)
            .terminal_commands
            .iter()
            .find(|(id, _)| *id == thread_id)
            .map(|(_, command)| command.clone())
    };
    let created_activity = last_activity(&client);
    assert!(created_activity.is_some());

    // The shell's output counts as the thread's activity.
    let key = TerminalKey::Thread(thread_id);
    client.subscribe_terminal(key.clone()).await;
    client
        .wait_until(move |client| last_activity(client) > created_activity)
        .await;
    assert_eq!(command(&client), None);

    // A shell is named after the folder it's in, and follows `cd`.
    let title = move |client: &TestClient| {
        snapshot(client)
            .threads
            .iter()
            .find(|thread| thread.id == thread_id)
            .map(|thread| thread.title.clone())
    };
    let project_name = server
        .project_dir
        .path()
        .canonicalize()
        .expect("project folder")
        .file_name()
        .expect("a name")
        .to_string_lossy()
        .into_owned();
    client
        .wait_until(move |client| title(client).as_deref() == Some(project_name.as_str()))
        .await;
    std::fs::create_dir(server.project_dir.path().join("inner")).expect("folder");
    client.type_into(&key, "cd inner\n").await;
    client
        .wait_until(move |client| title(client).as_deref() == Some("inner"))
        .await;

    // A program in front of the shell is running there, until it ends.
    client.type_into(&key, "sleep 30\n").await;
    client
        .wait_until(move |client| command(client).as_deref() == Some("sleep"))
        .await;
    client.type_into(&key, "\x03").await;
    client
        .wait_until(move |client| command(client).is_none())
        .await;
}

#[tokio::test(flavor = "multi_thread")]
async fn terminal_threads_stream_their_screens_to_watchers() {
    let Some(server) = TestServer::start() else {
        return;
    };
    let mut client = server.connect().await;
    assert!(
        client
            .welcome
            .capabilities
            .iter()
            .any(|capability| capability == agentz_protocol::CAPABILITY_TERMINALS)
    );
    client.ok(Request::SubscribeSession).await;
    let project_id = client.add_project(server.project_dir.path()).await;
    let thread_id = match client
        .ok(Request::CreateTerminalThread {
            project_id,
            command: TerminalCommand {
                command: Some("printf 'ready in %s\\n' \"$PWD\"; PS1='$ ' exec /bin/sh".into()),
            },
            workspace: Default::default(),
        })
        .await
    {
        Response::ThreadCreated(thread_id) => thread_id,
        response => panic!("unexpected response: {response:?}"),
    };
    let thread = client
        .projects
        .as_ref()
        .and_then(|projects| {
            projects
                .threads
                .iter()
                .find(|thread| thread.id == thread_id)
        })
        .expect("the thread")
        .clone();
    assert_eq!(thread.agent_id, None);
    assert!(thread.terminal.is_some());

    // The terminal started with the thread, so its first output is already on the screen.
    let key = TerminalKey::Thread(thread_id);
    client.subscribe_terminal(key.clone()).await;
    let folder = std::fs::canonicalize(server.project_dir.path()).expect("canonical path");
    client
        .wait_for_screen(&key, &format!("ready in {}", folder.display()))
        .await;
    client.type_into(&key, "echo sum=$((40 + 2))\n").await;
    client.wait_for_screen(&key, "sum=42").await;

    client
        .ok(Request::TerminalInput {
            terminal: key.clone(),
            input: TerminalInput::Resize {
                columns: 50,
                screen_lines: 12,
                cell_width: 8,
                cell_height: 16,
            },
        })
        .await;
    client.type_into(&key, "stty size\n").await;
    client.wait_for_screen(&key, "12 50").await;
    assert_eq!(client.terminals[&key].columns, 50);

    // A second client starts from a full frame of the same screen.
    let mut watcher = server.connect().await;
    watcher.subscribe_terminal(key.clone()).await;
    assert!(watcher.screen(&key).contains("sum=42"));

    client.type_into(&key, "echo selected-word\n").await;
    client.wait_for_screen(&key, "\nselected-word").await;
    let row = client.terminals[&key]
        .lines
        .iter()
        .find(|(_, line)| line.text() == "selected-word")
        .map(|(row, _)| *row)
        .expect("the output row");
    client
        .ok(Request::TerminalInput {
            terminal: key.clone(),
            input: TerminalInput::Select(Some(TerminalSelectionUpdate {
                point: TerminalPoint {
                    line: row as i32,
                    column: 3,
                },
                right_half: false,
                start: Some(TerminalSelectionKind::Semantic),
            })),
        })
        .await;
    assert_eq!(
        client.ok(Request::TerminalSelectionText(key.clone())).await,
        Response::Message("selected-word".into())
    );

    // Clearing keeps only the prompt's line.
    client
        .ok(Request::TerminalInput {
            terminal: key.clone(),
            input: TerminalInput::Clear,
        })
        .await;
    client
        .wait_until(|client| !client.screen(&key).contains("selected-word"))
        .await;
    assert_eq!(client.screen(&key).trim(), "$");

    // The drawer under a thread is a shell in the thread's folder.
    let drawer = TerminalKey::Drawer(thread_id);
    client.subscribe_terminal(drawer.clone()).await;
    client.type_into(&drawer, "echo in-$PWD\n").await;
    client
        .wait_for_screen(&drawer, &format!("in-{}", folder.display()))
        .await;
    client.ok(Request::CloseTerminal(drawer.clone())).await;
    client
        .wait_until(|client| !client.terminals.contains_key(&drawer))
        .await;

    // Exiting leaves the screen, marked; restarting runs the command again.
    client.type_into(&key, "exit 7\n").await;
    client
        .wait_until(|client| {
            client.terminals[&key]
                .exited
                .as_ref()
                .is_some_and(|exit| exit.code == Some(7))
        })
        .await;
    client.ok(Request::RestartTerminal(key.clone())).await;
    client
        .wait_until(|client| client.terminals[&key].exited.is_none())
        .await;
    client.wait_for_screen(&key, "ready in").await;

    // Deleting the thread closes its terminal for everyone watching.
    client.ok(Request::DeleteThread(thread_id)).await;
    client
        .wait_until(|client| !client.terminals.contains_key(&key))
        .await;
    watcher
        .wait_until(|watcher| !watcher.terminals.contains_key(&key))
        .await;
    assert!(
        client
            .request(Request::SubscribeTerminal(key.clone()))
            .await
            .is_err()
    );
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
#[tokio::test(flavor = "multi_thread")]
async fn terminal_threads_show_their_agents_state() {
    let Some(server) = TestServer::start() else {
        return;
    };
    // Named like an agent CLI, so the thread's foreground process reads as Codex. It sets
    // the terminal's title to each line it reads, as Codex shows its state in the title.
    let bin = tempfile::tempdir().expect("temp dir");
    let codex = bin.path().join("codex");
    std::fs::write(
        &codex,
        "#!/bin/sh\necho codex ready\nwhile read line; do printf '\\033]0;%s\\007' \"$line\"; done\n",
    )
    .expect("script");
    std::fs::set_permissions(&codex, std::os::unix::fs::PermissionsExt::from_mode(0o755))
        .expect("permissions");

    let mut client = server.connect().await;
    client.ok(Request::SubscribeSession).await;
    let project_id = client.add_project(server.project_dir.path()).await;
    let thread_id = match client
        .ok(Request::CreateTerminalThread {
            project_id,
            command: TerminalCommand {
                command: Some(codex.display().to_string()),
            },
            workspace: Default::default(),
        })
        .await
    {
        Response::ThreadCreated(thread_id) => thread_id,
        response => panic!("unexpected response: {response:?}"),
    };
    let key = TerminalKey::Thread(thread_id);
    client.subscribe_terminal(key.clone()).await;
    client.wait_for_screen(&key, "codex ready").await;

    let snapshot = |client: &TestClient| client.projects.clone().expect("projects");
    let is_working =
        move |client: &TestClient| snapshot(client).working_threads.contains(&thread_id);
    let is_blocked =
        move |client: &TestClient| snapshot(client).blocked_threads.contains(&thread_id);
    let completed_at = move |client: &TestClient| {
        snapshot(client)
            .threads
            .iter()
            .find(|thread| thread.id == thread_id)
            .and_then(|thread| thread.completed_at)
    };

    // After the agent's startup grace, its title spinner says it's working.
    let terminal_agent = move |client: &TestClient| {
        snapshot(client)
            .terminal_agents
            .iter()
            .find(|(id, _)| *id == thread_id)
            .map(|(_, agent)| agent.clone())
    };
    client.type_into(&key, "⠋ project\n").await;
    client.wait_until(is_working).await;
    assert!(!is_blocked(&client));
    // Running an agent CLI makes it a thread rather than a shell.
    assert_eq!(terminal_agent(&client).as_deref(), Some("Codex"));

    client.type_into(&key, "Action Required\n").await;
    client.wait_until(is_blocked).await;
    assert!(is_working(&client));

    client.type_into(&key, "project\n").await;
    client.wait_until(move |client| !is_working(client)).await;
    assert!(!is_blocked(&client));
    let first_completion = completed_at(&client).expect("a completion");

    // Exiting while working completes the thread too.
    client.type_into(&key, "⠙ project\n").await;
    client.wait_until(is_working).await;
    client.type_into(&key, "\x04").await;
    client.wait_until(move |client| !is_working(client)).await;
    assert!(completed_at(&client).expect("a completion") > first_completion);
    // With the agent gone, it's a shell again.
    client
        .wait_until(move |client| terminal_agent(client).is_none())
        .await;
}

#[tokio::test(flavor = "multi_thread")]
async fn agents_run_commands_in_server_terminals() {
    let Some(server) = TestServer::start() else {
        return;
    };
    let mut client = server.connect().await;
    client.ok(Request::SubscribeSession).await;
    let thread_id = client.create_thread(&server).await;
    let connection = ConnectionId::Thread(thread_id);
    client.subscribe_thread(connection).await;
    client
        .ok(Request::Prompt {
            connection,
            text: "terminal printf 'built %s\\n' \"$PAGER-$GIT_PAGER\"; exit 4".into(),
        })
        .await;
    client
        .wait_until(|client| agent_text(client.thread(connection)).starts_with("Terminal"))
        .await;
    assert_eq!(
        agent_text(client.thread(connection)),
        "Terminal 4: built -cat"
    );

    // The tool call names its terminal, which the user can still open after the agent
    // released it.
    let terminal_id = client
        .thread(connection)
        .entries()
        .iter()
        .find_map(|entry| match entry {
            Entry::ToolCall(tool_call) => tool_call.terminals.first().cloned(),
            _ => None,
        })
        .expect("a tool call with a terminal");
    let key = TerminalKey::Agent {
        thread_id,
        terminal_id,
    };
    client.subscribe_terminal(key.clone()).await;
    assert!(client.screen(&key).contains("built -cat"));
    assert_eq!(
        client.terminals[&key]
            .exited
            .as_ref()
            .and_then(|exit| exit.code),
        Some(4)
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn agents_drive_terminals_through_tools() {
    let Some(server) = TestServer::start() else {
        return;
    };
    let mut client = server.connect().await;
    client.ok(Request::SubscribeSession).await;
    let thread_id = client.create_thread(&server).await;

    // A terminal thread the agent starts, which the user sees in the project.
    let started = client
        .tool(thread_id, "agentz_terminal_start", json!({}))
        .await;
    assert_eq!(started["kind"], "terminal_thread");
    assert_eq!(started["status"], "running");
    let terminal = ThreadId(started["threadId"].as_u64().expect("a thread id"));
    client
        .wait_until(|client| {
            client
                .project_thread(terminal)
                .is_some_and(|thread| thread.terminal.is_some())
        })
        .await;

    client
        .tool(
            thread_id,
            "agentz_terminal_send",
            json!({"threadId": terminal.0, "text": "echo sum-$((40+2))", "submit": true}),
        )
        .await;
    let waited = client
        .tool(
            thread_id,
            "agentz_terminal_wait",
            json!({"threadId": terminal.0, "match": "sum-42", "timeoutMs": 10_000}),
        )
        .await;
    assert_eq!(waited["matched"], true, "{waited}");
    let read = client
        .tool(
            thread_id,
            "agentz_terminal_read",
            json!({"threadId": terminal.0, "source": "visible"}),
        )
        .await;
    assert!(
        read["text"]
            .as_str()
            .is_some_and(|text| text.contains("sum-42"))
    );

    assert_eq!(
        client
            .tool_failure(
                ToolCaller::Thread(thread_id),
                "agentz_terminal_send",
                json!({"threadId": terminal.0, "keys": ["hyper-x"]}),
            )
            .await,
        "invalid_request"
    );

    // Keys end the shell; a wait without a match returns once it has exited.
    client
        .tool(
            thread_id,
            "agentz_terminal_send",
            json!({"threadId": terminal.0, "text": "exit 3", "keys": ["enter"]}),
        )
        .await;
    let waited = client
        .tool(
            thread_id,
            "agentz_terminal_wait",
            json!({"threadId": terminal.0, "timeoutMs": 10_000}),
        )
        .await;
    assert_eq!(waited["status"], "exited", "{waited}");
    assert_eq!(waited["exitCode"], 3);
    assert_eq!(
        client
            .tool_failure(
                ToolCaller::Thread(thread_id),
                "agentz_terminal_send",
                json!({"threadId": terminal.0, "keys": ["enter"]}),
            )
            .await,
        "terminal_exited"
    );

    // Without a thread id, an agent thread's own drawer.
    client
        .tool(
            thread_id,
            "agentz_terminal_send",
            json!({"text": "echo drawer-ok", "submit": true}),
        )
        .await;
    let waited = client
        .tool(
            thread_id,
            "agentz_terminal_wait",
            json!({"match": "drawer-ok\n", "timeoutMs": 10_000}),
        )
        .await;
    assert_eq!(waited["kind"], "drawer");
    assert_eq!(waited["matched"], true, "{waited}");

    let listed = client
        .tool(thread_id, "agentz_terminal_list", json!({}))
        .await;
    let kinds: Vec<&str> = listed["terminals"]
        .as_array()
        .expect("terminals")
        .iter()
        .filter_map(|terminal| terminal["kind"].as_str())
        .collect();
    assert_eq!(kinds, ["drawer", "terminal_thread"]);
}

impl TestClient {
    async fn space_pane(&mut self, request: SpaceRequest) -> PaneLocation {
        match self.ok(Request::Spaces(request)).await {
            Response::SpacePane(location) => location,
            response => panic!("unexpected response: {response:?}"),
        }
    }

    fn space_snapshot(&self) -> SpacesSnapshot {
        self.events
            .iter()
            .rev()
            .find_map(|event| match event {
                Event::Spaces(spaces) => Some(spaces.clone()),
                _ => None,
            })
            .unwrap_or_default()
    }
}

fn shell_in(folder: &std::path::Path) -> PaneContent {
    PaneContent::Terminal(PaneTerminal {
        folder: folder.to_path_buf(),
        command: None,
    })
}

#[tokio::test(flavor = "multi_thread")]
async fn spaces_are_saved_restored_and_streamed() {
    let Some(server) = TestServer::start() else {
        return;
    };
    let folder = std::fs::canonicalize(server.project_dir.path()).expect("canonical path");
    let mut client = server.connect().await;
    assert!(
        client
            .welcome
            .capabilities
            .iter()
            .any(|capability| capability == agentz_protocol::CAPABILITY_SPACES)
    );
    client.ok(Request::SubscribeSession).await;
    let project_id = client.add_project(&folder).await;
    let thread_id = client.create_thread_in(project_id).await;

    // A space opens with one terminal pane in its folder.
    let first = client
        .space_pane(SpaceRequest::CreateSpace {
            folder: folder.clone(),
            project_id: Some(project_id),
            content: shell_in(std::path::Path::new("")),
        })
        .await;
    let key = TerminalKey::Pane(first.pane);
    client.subscribe_terminal(key.clone()).await;
    client.type_into(&key, "echo in-$PWD\n").await;
    client
        .wait_for_screen(&key, &format!("in-{}", folder.display()))
        .await;

    // Split right with a shell, and down with the thread.
    let second = client
        .space_pane(SpaceRequest::SplitPane {
            pane: first.pane,
            direction: Direction::Horizontal,
            content: shell_in(&folder),
        })
        .await;
    let third = client
        .space_pane(SpaceRequest::SplitPane {
            pane: second.pane,
            direction: Direction::Vertical,
            content: PaneContent::Thread(thread_id),
        })
        .await;
    assert_eq!((second.tab, third.tab), (first.tab, first.tab));
    client
        .ok(Request::Spaces(SpaceRequest::SetSplitRatio {
            tab: first.tab,
            path: vec![],
            ratio: 0.25,
        }))
        .await;
    client
        .ok(Request::Spaces(SpaceRequest::RenameSpace {
            space: first.space,
            name: Some("Work".into()),
        }))
        .await;
    let tab = client
        .space_pane(SpaceRequest::CreateTab {
            space: first.space,
            content: shell_in(&folder),
        })
        .await;
    client
        .wait_until(|client| {
            client
                .space_snapshot()
                .space(first.space)
                .is_some_and(|space| space.tabs.len() == 2 && space.name.is_some())
        })
        .await;
    let spaces = client.space_snapshot();
    let (space, tab_one) = spaces.tab(first.tab).expect("the tab");
    assert_eq!(space.label(), "Work");
    assert_eq!(tab_one.panes.len(), 3);
    assert!(matches!(
        tab_one.root,
        Node::Split { direction: Direction::Horizontal, ratio, .. } if (ratio - 0.25).abs() < 1e-6
    ));

    // A pane whose shell exits closes, as in herdr.
    let second_key = TerminalKey::Pane(second.pane);
    client.subscribe_terminal(second_key.clone()).await;
    client.type_into(&second_key, "exit\n").await;
    client
        .wait_until(|client| {
            client
                .space_snapshot()
                .tab(first.tab)
                .is_some_and(|(_, tab)| tab.pane(second.pane).is_none())
        })
        .await;
    assert!(!client.terminals.contains_key(&second_key));

    // Closing the tab's last pane closes the tab.
    client
        .ok(Request::Spaces(SpaceRequest::ClosePane(tab.pane)))
        .await;
    client
        .wait_until(|client| client.space_snapshot().tab(tab.tab).is_none())
        .await;

    client.ok(Request::Shutdown).await;
    tokio::time::timeout(TIMEOUT, server.handle.stopped())
        .await
        .expect("the server stops");
    drop(client);

    // After a restart, the layout is back, terminal panes as new shells in their folders,
    // and thread panes on their threads.
    let Some(server) = TestServer::start_with(server.data_dir, server.project_dir) else {
        return;
    };
    let mut client = server.connect().await;
    let Response::Session(session) = client.ok(Request::SubscribeSession).await else {
        panic!("expected a session snapshot");
    };
    let (space, tab) = session.spaces.tab(first.tab).expect("the tab is back");
    assert_eq!(space.name.as_deref(), Some("Work"));
    assert_eq!(space.tabs.len(), 1);
    assert_eq!(tab.panes.len(), 2);
    assert_eq!(
        tab.pane(third.pane).map(|pane| &pane.content),
        Some(&PaneContent::Thread(thread_id))
    );
    client.subscribe_terminal(key.clone()).await;
    client.type_into(&key, "echo back-in-$PWD\n").await;
    client
        .wait_for_screen(&key, &format!("back-in-{}", folder.display()))
        .await;

    // Deleting the thread closes its pane; closing the space ends its terminals.
    client.ok(Request::DeleteThread(thread_id)).await;
    client
        .wait_until(|client| {
            client
                .space_snapshot()
                .tab(first.tab)
                .is_some_and(|(_, tab)| tab.panes.len() == 1)
        })
        .await;
    client
        .ok(Request::Spaces(SpaceRequest::CloseSpace(first.space)))
        .await;
    client
        .wait_until(|client| !client.terminals.contains_key(&key))
        .await;
    assert!(client.space_snapshot().spaces.is_empty());
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
#[tokio::test(flavor = "multi_thread")]
async fn agents_started_from_a_panes_shell_are_found() {
    let Some(server) = TestServer::start() else {
        return;
    };
    let bin = tempfile::tempdir().expect("temp dir");
    let codex = bin.path().join("codex");
    std::fs::write(
        &codex,
        "#!/bin/sh\necho codex ready\nwhile read line; do :; done\n",
    )
    .expect("script");
    std::fs::set_permissions(&codex, std::os::unix::fs::PermissionsExt::from_mode(0o755))
        .expect("permissions");

    // A shell pane, as the user opens one, with the agent typed at its prompt. On macOS the
    // pane's own process is `login`, which can't be inspected.
    let mut client = server.connect().await;
    client.ok(Request::SubscribeSession).await;
    let location = client
        .space_pane(SpaceRequest::CreateSpace {
            folder: server.project_dir.path().to_path_buf(),
            project_id: None,
            content: PaneContent::Terminal(PaneTerminal {
                folder: server.project_dir.path().to_path_buf(),
                command: None,
            }),
        })
        .await;
    let key = TerminalKey::Pane(location.pane);
    client.subscribe_terminal(key.clone()).await;
    let agent = move |client: &TestClient| {
        client
            .space_snapshot()
            .pane(location.pane)
            .and_then(|(_, _, pane)| pane.agent.clone())
    };
    client
        .type_into(&key, &format!("{}\n", codex.display()))
        .await;
    client.wait_for_screen(&key, "codex ready").await;
    client.wait_until(|client| agent(client).is_some()).await;
    assert_eq!(agent(&client).expect("an agent").name, "Codex");

    // Back at the shell, the agent is gone.
    client.type_into(&key, "\x04").await;
    client.wait_until(|client| agent(client).is_none()).await;
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
#[tokio::test(flavor = "multi_thread")]
async fn terminal_panes_show_their_agents() {
    let Some(server) = TestServer::start() else {
        return;
    };
    let bin = tempfile::tempdir().expect("temp dir");
    let codex = bin.path().join("codex");
    std::fs::write(
        &codex,
        "#!/bin/sh\necho codex ready\nwhile read line; do printf '\\033]0;%s\\007' \"$line\"; done\n",
    )
    .expect("script");
    std::fs::set_permissions(&codex, std::os::unix::fs::PermissionsExt::from_mode(0o755))
        .expect("permissions");

    let mut client = server.connect().await;
    client.ok(Request::SubscribeSession).await;
    let location = client
        .space_pane(SpaceRequest::CreateSpace {
            folder: server.project_dir.path().to_path_buf(),
            project_id: None,
            content: PaneContent::Terminal(PaneTerminal {
                folder: server.project_dir.path().to_path_buf(),
                command: Some(codex.display().to_string()),
            }),
        })
        .await;
    let key = TerminalKey::Pane(location.pane);
    client.subscribe_terminal(key.clone()).await;
    client.wait_for_screen(&key, "codex ready").await;
    let agent = move |client: &TestClient| {
        client
            .space_snapshot()
            .pane(location.pane)
            .and_then(|(_, _, pane)| pane.agent.clone())
    };
    client.type_into(&key, "⠋ project\n").await;
    client
        .wait_until(|client| {
            agent(client).is_some_and(|agent| agent.state == PaneAgentState::Working)
        })
        .await;
    assert_eq!(agent(&client).expect("an agent").name, "Codex");
    client.type_into(&key, "project\n").await;
    client
        .wait_until(|client| agent(client).is_some_and(|agent| agent.state == PaneAgentState::Idle))
        .await;
}
