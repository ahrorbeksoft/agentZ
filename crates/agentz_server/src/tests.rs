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
    Request, Response, ServerMessage, ServerWelcome, read_message, write_message,
};
use futures::FutureExt as _;
use projects::{ProjectsSnapshot, ThreadId};
use registry::AgentCommand;
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
