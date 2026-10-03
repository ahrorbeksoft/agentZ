//! The server driven over in-memory streams, with `agent_thread`'s mock agent.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use agent_client_protocol::schema::v1 as acp;
use agentz_protocol::agents::AgentId;
use agentz_protocol::thread::{Entry, ThreadView};
use agentz_protocol::{
    ClientHello, ClientKind, ClientMessage, ConnectionId, ErrorResponse, Event, PROTOCOL_VERSION,
    Request, Response, ServerMessage, ServerWelcome, ToolCaller, ToolResult, read_message,
    write_message,
};
use futures::FutureExt as _;
use projects::{ProjectId, ProjectsSnapshot, ThreadCreator, ThreadId};
use registry::AgentCommand;
use serde_json::{Value, json};
use tokio::io::{DuplexStream, ReadHalf, WriteHalf};

use crate::{CustomAgent, ServerConfig, ServerHandle};

const TIMEOUT: Duration = Duration::from_secs(10);

struct TestServer {
    handle: ServerHandle,
    _data_dir: tempfile::TempDir,
    project_dir: tempfile::TempDir,
}

impl TestServer {
    /// `None` without python3 to run the mock agent.
    fn start() -> Option<Self> {
        let command = mock_agent()?;
        let data_dir = tempfile::tempdir().expect("temp dir");
        let project_dir = tempfile::tempdir().expect("temp dir");
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
            },
        )
        .expect("server starts");
        Some(Self {
            handle,
            _data_dir: data_dir,
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
