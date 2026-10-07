//! The server driven over in-memory streams, with `agent_thread`'s mock agent.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};

use agent_client_protocol::schema::v1 as acp;
use agentz_protocol::accounts::{
    AccountChange, AccountChoice, AccountId, AgentAccounts, AtLimit, OveragePreference,
    SettingsSource,
};
use agentz_protocol::agents::{AgentId, AgentSessions, CustomAgentChange};
use agentz_protocol::diff::{DiffScope, DiffStatus, FileChange, RestoreAvailability, ThreadDiff};
use agentz_protocol::layout::{Direction, Node};
use agentz_protocol::spaces::{
    LayoutNode, PaneAgentState, PaneContent, PaneLocation, PaneTerminal, SpaceRequest,
    SpacesSnapshot,
};
use agentz_protocol::terminal::{
    TerminalCommand, TerminalFrame, TerminalInput, TerminalKey, TerminalPoint,
    TerminalSelectionKind, TerminalSelectionUpdate,
};
use agentz_protocol::thread::{
    ConnectionStatus, Entry, LoginInput, ThreadView, api_key_meta, login_input,
};
use agentz_protocol::workspace::{WorkspaceChoice, WorkspaceRemoval};
use agentz_protocol::{
    AgentSettingsChange, ClientHello, ClientKind, ClientMessage, ConnectionId, ErrorResponse,
    Event, MachineKind, PROTOCOL_VERSION, PeerCheckout, PeerCheckouts, PeerMachine, Peers,
    PromptPart, Request, Response, ServerMessage, ServerWelcome, ToolCaller, ToolResult,
    read_message, write_message,
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
        Self::start_with_agent(data_dir, project_dir, mock_agent()?)
    }

    fn start_with_agent(
        data_dir: tempfile::TempDir,
        project_dir: tempfile::TempDir,
        command: AgentCommand,
    ) -> Option<Self> {
        Self::start_with_description(data_dir, project_dir, command, mock_accounts())
    }

    fn start_with_description(
        data_dir: tempfile::TempDir,
        project_dir: tempfile::TempDir,
        command: AgentCommand,
        description: crate::AgentDescription,
    ) -> Option<Self> {
        let custom_agents = BTreeMap::from_iter([(
            AgentId::new("mock"),
            CustomAgent {
                name: "Mock".into(),
                command,
                info: None,
                accounts: Some(description),
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
                hands_pages_to_clients: false,
                terminal_shell: Some("/bin/sh".into()),
                listener: None,
                handed_over: None,
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

    /// The agent's accounts, as the server last sent them.
    fn accounts(&self, agent_id: &str) -> AgentAccounts {
        self.events
            .iter()
            .rev()
            .find_map(|event| match event {
                Event::Accounts(accounts) => Some(
                    accounts
                        .get(&AgentId::new(agent_id))
                        .cloned()
                        .unwrap_or_default(),
                ),
                _ => None,
            })
            .unwrap_or_default()
    }

    /// Whether the server last found the account logged in, `None` being the External one.
    fn account_logged_in(&self, account: Option<AccountId>) -> Option<bool> {
        self.accounts("mock").logged_in(account)
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
                account: Default::default(),
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

/// The mock agent keeps an account's login in `MOCK_HOME`, and takes `MOCK_API_KEY` as one, as
/// its "mock-env-key" login does.
fn mock_accounts() -> crate::AgentDescription {
    crate::AgentDescription {
        home_variables: BTreeMap::from([("MOCK_HOME".into(), String::new())]),
        shared_folders: BTreeMap::new(),
        file_storage: BTreeMap::new(),
        home_files: BTreeMap::from([(".mock/settings.json".into(), r#"{"sync": false}"#.into())]),
        // Tests that copy them name a normal home of their own, outside the user's.
        settings_files: Vec::new(),
        login_settings: BTreeMap::new(),
        normal_home: String::new(),
        login_variables: vec!["MOCK_API_KEY".into()],
        login_check: crate::LoginCheck::Session,
        reader: None,
        key_login: Some(crate::KeyLogin {
            method: "mock-env-key".into(),
            variable: "MOCK_API_KEY".into(),
            reader: None,
        }),
        usage_page: None,
        extra_usage_page: None,
    }
}

/// The mock agent's `--usage`, which also says whether `MOCK_API_KEY` is a key that works.
fn mock_usage_reader(command: &AgentCommand) -> crate::Reader {
    let mut args = command.args.clone();
    args.push("--usage".into());
    crate::Reader::Command(crate::ReaderCommand {
        program: Some(command.path.to_string_lossy().into_owned()),
        args,
    })
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
        env_remove: Vec::new(),
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
            prompt: PromptPart::text("hello there"),
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
            prompt: PromptPart::text("again"),
        })
        .await;
    assert!(error.is_err());
}

/// "Continue with another agent": a new thread in the same workspace, pointing back at the old
/// one, whose first message brings the old conversation as embedded context.
#[tokio::test(flavor = "multi_thread")]
async fn continues_threads_with_another_agent() {
    let Some(server) = TestServer::start() else {
        return;
    };
    let mut client = server.connect().await;
    client.ok(Request::SubscribeSession).await;
    let thread_id = client.create_thread(&server).await;
    let old = ConnectionId::Thread(thread_id);
    client.subscribe_thread(old).await;
    client
        .ok(Request::Prompt {
            connection: old,
            prompt: PromptPart::text("build the page"),
        })
        .await;
    client
        .wait_until(|client| {
            let thread = client.thread(old);
            !thread.is_working() && agent_text(thread) == "Echo: build the page"
        })
        .await;

    let Response::ThreadCreated(new_id) = client
        .ok(Request::ContinueThread {
            thread_id,
            agent_id: AgentId::new("mock"),
            account: AccountChoice::Default,
        })
        .await
    else {
        panic!("expected a thread");
    };
    let new = ConnectionId::Thread(new_id);
    client.subscribe_thread(new).await;
    let handoff = client
        .thread(new)
        .pending_handoff()
        .cloned()
        .expect("the conversation waits for the first message");
    assert_eq!(handoff.messages, 2);
    assert!(
        handoff
            .text
            .contains("<message from=\"user\">\nbuild the page\n</message>")
    );
    assert!(
        handoff
            .text
            .contains("<message from=\"Mock\">\nEcho: build the page")
    );
    let saved = server
        .data_dir
        .path()
        .join(format!("handoffs/{}.json", new_id.0));
    assert!(saved.exists());
    let continued_from = |client: &TestClient| {
        client
            .projects
            .as_ref()
            .and_then(|projects| projects.threads.iter().find(|thread| thread.id == new_id))
            .and_then(|thread| thread.continued_from)
    };
    // A draft, which links to the old thread only once its first message goes.
    assert_eq!(continued_from(&client), None);

    client
        .ok(Request::Prompt {
            connection: new,
            prompt: PromptPart::text("next"),
        })
        .await;
    client
        .wait_until(|client| continued_from(client) == Some(thread_id))
        .await;
    client
        .wait_until(|client| {
            let thread = client.thread(new);
            !thread.is_working() && agent_text(thread) == "Echo: next [with agentz://handoff]"
        })
        .await;
    let thread = client.thread(new);
    assert_eq!(thread.pending_handoff(), None);
    // Only the user's words are their message.
    assert!(matches!(thread.entries().first(), Some(Entry::UserMessage(text)) if text == "next"));
    assert!(!saved.exists());

    // A continuation can run on another of the agent's accounts.
    let Response::AccountAdded(work) = client.ok(Request::AddAccount(AgentId::new("mock"))).await
    else {
        panic!("expected an account");
    };
    let Response::ThreadCreated(on_work) = client
        .ok(Request::ContinueThread {
            thread_id,
            agent_id: AgentId::new("mock"),
            account: AccountChoice::Account(work),
        })
        .await
    else {
        panic!("expected a thread");
    };
    client
        .wait_until(|client| {
            client.projects.as_ref().is_some_and(|projects| {
                projects
                    .threads
                    .iter()
                    .any(|thread| thread.id == on_work && thread.account == Some(work))
            })
        })
        .await;
}

/// An archived thread's agent stops once nothing needs it and no client has the thread open,
/// and opening the thread again starts it.
#[tokio::test(flavor = "multi_thread")]
async fn stops_the_agent_of_an_archived_thread_left_idle() {
    let Some(mut command) = mock_agent() else {
        return;
    };
    let dir = tempfile::tempdir().expect("temp dir");
    let closed = dir.path().join("closed");
    command.env.insert(
        "MOCK_CLOSED_FILE".into(),
        closed.to_string_lossy().into_owned(),
    );
    let Some(server) = TestServer::start_with_agent(
        tempfile::tempdir().expect("temp dir"),
        tempfile::tempdir().expect("temp dir"),
        command,
    ) else {
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
            prompt: PromptPart::text("hello"),
        })
        .await;
    client
        .wait_until(|client| {
            let thread = client.thread(connection);
            !thread.is_working() && agent_text(thread) == "Echo: hello"
        })
        .await;
    client.ok(Request::ArchiveThread(thread_id)).await;
    // It runs while it's open, past the moment a view takes to be rebuilt.
    tokio::time::sleep(Duration::from_secs(4)).await;
    assert!(!closed.exists());

    client.ok(Request::UnsubscribeThread(connection)).await;
    let deadline = Instant::now() + Duration::from_secs(10);
    while !closed.exists() && Instant::now() < deadline {
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    let closed_sessions = std::fs::read_to_string(&closed).expect("the agent closed its session");
    assert_eq!(closed_sessions.lines().count(), 1);

    client.subscribe_thread(connection).await;
    client
        .wait_until(|client| client.thread(connection).status() == &ConnectionStatus::Ready)
        .await;
}

/// An agent that goes on sending after its turn, as Claude Agent's background tasks do, isn't
/// idle: it stops once it has been quiet for as long as its thread allows.
#[tokio::test(flavor = "multi_thread")]
async fn keeps_an_agent_that_works_after_its_turn() {
    let Some(mut command) = mock_agent() else {
        return;
    };
    let dir = tempfile::tempdir().expect("temp dir");
    let closed = dir.path().join("closed");
    command.env.insert(
        "MOCK_CLOSED_FILE".into(),
        closed.to_string_lossy().into_owned(),
    );
    let Some(server) = TestServer::start_with_agent(
        tempfile::tempdir().expect("temp dir"),
        tempfile::tempdir().expect("temp dir"),
        command,
    ) else {
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
            prompt: PromptPart::text("background"),
        })
        .await;
    client
        .wait_until(|client| {
            let thread = client.thread(connection);
            !thread.is_working() && agent_text(thread).starts_with("Working in the background")
        })
        .await;
    // Archived, so 3 seconds without work would stop it.
    client.ok(Request::ArchiveThread(thread_id)).await;
    client.ok(Request::UnsubscribeThread(connection)).await;

    // It goes on for six seconds after its turn.
    tokio::time::sleep(Duration::from_secs(5)).await;
    assert!(!closed.exists(), "the agent stopped while it worked");
    let deadline = Instant::now() + Duration::from_secs(10);
    while !closed.exists() && Instant::now() < deadline {
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert!(closed.exists(), "the agent kept running once it went quiet");
}

/// A new thread is a draft until its first message, as in t3code: one left with nothing typed
/// is removed, and one with typed text stays until that's cleared.
#[tokio::test(flavor = "multi_thread")]
async fn removes_drafts_left_empty() {
    let Some(server) = TestServer::start() else {
        return;
    };
    let mut client = server.connect().await;
    client.ok(Request::SubscribeSession).await;
    let thread_id = client.create_thread(&server).await;
    let old = ConnectionId::Thread(thread_id);
    client.subscribe_thread(old).await;
    let thread = |client: &TestClient, thread_id: ThreadId| {
        client
            .projects
            .as_ref()
            .and_then(|projects| {
                projects
                    .threads
                    .iter()
                    .find(|thread| thread.id == thread_id)
            })
            .cloned()
    };
    assert!(thread(&client, thread_id).is_some_and(|thread| thread.is_draft));
    client
        .ok(Request::Prompt {
            connection: old,
            prompt: PromptPart::text("build the page"),
        })
        .await;
    client
        .wait_until(|client| {
            let view = client.thread(old);
            !view.is_working() && agent_text(view) == "Echo: build the page"
        })
        .await;
    assert!(thread(&client, thread_id).is_some_and(|thread| !thread.is_draft));

    let Response::ThreadCreated(continuation) = client
        .ok(Request::ContinueThread {
            thread_id,
            agent_id: AgentId::new("mock"),
            account: AccountChoice::Default,
        })
        .await
    else {
        panic!("expected a thread");
    };
    let project_id = thread(&client, thread_id)
        .expect("the thread exists")
        .project_id;
    let mut drafts = vec![continuation];
    for _ in 0..2 {
        match client
            .ok(Request::CreateThread {
                project_id,
                agent_id: AgentId::new("mock"),
                workspace: Default::default(),
                account: Default::default(),
            })
            .await
        {
            Response::ThreadCreated(thread_id) => drafts.push(thread_id),
            response => panic!("unexpected response: {response:?}"),
        }
    }
    let [continuation, left, typed] = drafts[..] else {
        panic!("expected three threads");
    };
    for thread_id in drafts.iter().copied() {
        client
            .subscribe_thread(ConnectionId::Thread(thread_id))
            .await;
    }
    client
        .ok(Request::SetUnsentText {
            thread_id: typed,
            text: Some("fix the header".into()),
            mentions: Vec::new(),
        })
        .await;
    let mention = projects::UnsentMention {
        range: 8..19,
        target: projects::Mentioned::Path("/tmp/footer.css".into()),
    };
    client
        .ok(Request::SetUnsentText {
            thread_id,
            text: Some("and the @footer.css".into()),
            mentions: vec![mention.clone()],
        })
        .await;
    for thread_id in drafts.iter().copied() {
        client
            .ok(Request::UnsubscribeThread(ConnectionId::Thread(thread_id)))
            .await;
    }
    client
        .wait_until(|client| {
            thread(client, continuation).is_none() && thread(client, left).is_none()
        })
        .await;
    let typed_thread = thread(&client, typed).expect("typed text keeps a draft");
    assert!(typed_thread.is_draft);
    assert_eq!(typed_thread.unsent_text.as_deref(), Some("fix the header"));
    let typed_in_thread = thread(&client, thread_id).expect("the thread");
    assert_eq!(
        typed_in_thread.unsent_text.as_deref(),
        Some("and the @footer.css")
    );
    assert_eq!(typed_in_thread.unsent_mentions, vec![mention]);
    let handoffs = server.data_dir.path().join("handoffs");
    assert!(!handoffs.join(format!("{}.json", continuation.0)).exists());

    // Cleared, the draft goes like any other left empty.
    client
        .ok(Request::SetUnsentText {
            thread_id: typed,
            text: None,
            mentions: Vec::new(),
        })
        .await;
    client
        .wait_until(|client| thread(client, typed).is_none())
        .await;
    assert!(thread(&client, thread_id).is_some());
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
            prompt: PromptPart::text("permission"),
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

/// The sidebar shows a thread whose agent asked for input as awaiting it, until it's answered.
#[tokio::test(flavor = "multi_thread")]
async fn threads_waiting_for_input_are_marked() {
    let Some(server) = TestServer::start() else {
        return;
    };
    let mut client = server.connect().await;
    let Response::Session(session) = client.ok(Request::SubscribeSession).await else {
        panic!("expected a session snapshot");
    };
    client.projects = Some(session.projects);
    let thread_id = client.create_thread(&server).await;
    let connection = ConnectionId::Thread(thread_id);
    client.subscribe_thread(connection).await;
    client
        .ok(Request::Prompt {
            connection,
            prompt: PromptPart::text("form"),
        })
        .await;
    let awaiting_input = move |client: &TestClient| {
        client
            .projects
            .as_ref()
            .is_some_and(|projects| projects.awaiting_input_threads.contains(&thread_id))
    };
    client
        .wait_until(move |client| {
            awaiting_input(client) && !client.thread(connection).elicitations().is_empty()
        })
        .await;
    let elicitation = client
        .thread(connection)
        .elicitations()
        .first()
        .expect("the agent asked for input")
        .clone();
    client
        .ok(Request::RespondToElicitation {
            connection,
            elicitation: elicitation.id,
            action: acp::ElicitationAction::Decline,
        })
        .await;
    client
        .wait_until(move |client| !awaiting_input(client))
        .await;
}

#[tokio::test(flavor = "multi_thread")]
async fn login_sessions_log_in_and_close_with_their_client() {
    let Some(server) = TestServer::start() else {
        return;
    };
    let mut client = server.connect().await;
    client.ok(Request::SubscribeSession).await;
    let Response::LoginSessionOpened(login_session_id) = client
        .ok(Request::OpenLoginSession {
            agent_id: AgentId::new("mock"),
            account: None,
        })
        .await
    else {
        panic!("expected a login session");
    };
    let connection = ConnectionId::LoginSession(login_session_id);
    client.subscribe_thread(connection).await;
    client
        .wait_until(|client| !client.thread(connection).auth_methods().is_empty())
        .await;
    client
        .ok(Request::Authenticate {
            connection,
            method_id: acp::AuthMethodId::new("mock-login"),
            meta: None,
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

/// An agent's sessions list with the project each would go to, import as threads in that
/// project (once), and load their conversation when opened.
#[tokio::test(flavor = "multi_thread")]
async fn lists_and_imports_an_agents_sessions() {
    let Some(mut command) = mock_agent() else {
        return;
    };
    let data_dir = tempfile::tempdir().expect("temp dir");
    let project_dir = tempfile::tempdir().expect("temp dir");
    // Agents report folders as they were given, which may not be canonical (`/var` on macOS
    // is `/private/var`).
    let project_path = project_dir.path().to_path_buf();
    let elsewhere = data_dir.path().to_path_buf();
    let sessions_file = data_dir.path().join("sessions.json");
    let sessions = json!([
        {"sessionId": "fix-login", "cwd": project_path, "title": "Fix the login",
         "updatedAt": "2026-05-01T10:00:00Z",
         "history": [{"sessionUpdate": "agent_message_chunk",
                      "content": {"type": "text", "text": "Fixed it earlier."}}]},
        {"sessionId": "untitled", "cwd": project_path},
        {"sessionId": "other-folder", "cwd": elsewhere, "title": "Somewhere else"},
    ]);
    std::fs::write(&sessions_file, sessions.to_string()).expect("writing sessions");
    command.env.insert(
        "MOCK_SESSIONS_FILE".into(),
        sessions_file.to_string_lossy().into_owned(),
    );
    let Some(server) = TestServer::start_with_agent(data_dir, project_dir, command) else {
        return;
    };
    let mut client = server.connect().await;
    client.ok(Request::SubscribeSession).await;
    let Response::ProjectAdded(project_id) = client
        .ok(Request::AddProject {
            path: project_path.clone(),
        })
        .await
    else {
        panic!("expected a project");
    };
    assert!(
        client
            .welcome
            .capabilities
            .iter()
            .any(|capability| capability == agentz_protocol::CAPABILITY_IMPORT_SESSIONS)
    );

    let list = async |client: &mut TestClient| match client
        .ok(Request::ListAgentSessions {
            agent_id: AgentId::new("mock"),
            account: None,
        })
        .await
    {
        Response::AgentSessions(AgentSessions::Listed(sessions)) => sessions,
        response => panic!("unexpected response: {response:?}"),
    };
    let sessions = list(&mut client).await;
    let summary: Vec<_> = sessions
        .iter()
        .map(|session| {
            (
                session.session_id.as_str(),
                session.title.as_deref(),
                session.project_id,
                session.thread_id,
            )
        })
        .collect();
    assert_eq!(
        summary,
        [
            ("fix-login", Some("Fix the login"), Some(project_id), None),
            ("untitled", None, Some(project_id), None),
            ("other-folder", Some("Somewhere else"), None, None),
        ]
    );
    let updated_at = sessions[0].updated_at.expect("a time");
    assert_eq!(
        updated_at
            .duration_since(std::time::UNIX_EPOCH)
            .expect("after the epoch")
            .as_secs(),
        1_777_629_600
    );

    let Response::ThreadsImported(imported) = client
        .ok(Request::ImportAgentSessions {
            agent_id: AgentId::new("mock"),
            account: None,
            sessions: sessions.clone(),
            archived: false,
        })
        .await
    else {
        panic!("expected imported threads");
    };
    assert_eq!(imported.len(), 2, "the session outside a project stays out");
    let projects = client.projects.clone().expect("projects");
    let titles: Vec<_> = imported
        .iter()
        .map(|id| {
            let thread = projects
                .threads
                .iter()
                .find(|thread| thread.id == *id)
                .expect("an imported thread");
            (thread.title.as_str(), thread.last_activity_at)
        })
        .collect();
    assert_eq!(titles[0], ("Fix the login", Some(updated_at)));
    assert_eq!(titles[1].0, "Imported thread");

    let listed_again = list(&mut client).await;
    assert_eq!(listed_again[0].thread_id, Some(imported[0]));
    let Response::ThreadsImported(again) = client
        .ok(Request::ImportAgentSessions {
            agent_id: AgentId::new("mock"),
            account: None,
            sessions,
            archived: false,
        })
        .await
    else {
        panic!("expected imported threads");
    };
    assert!(again.is_empty(), "sessions are imported once");

    let connection = ConnectionId::Thread(imported[0]);
    client.subscribe_thread(connection).await;
    client
        .wait_until(|client| agent_text(client.thread(connection)) == "Fixed it earlier.")
        .await;
}

/// Custom agents are added, changed and removed while the server runs. Saving starts the agent
/// once: its name fills a blank one, its version shows, and one that doesn't start says why.
#[tokio::test(flavor = "multi_thread")]
async fn custom_agents_are_saved_after_they_start() {
    let Some(server) = TestServer::start() else {
        return;
    };
    let Some(mut mock) = mock_agent() else {
        return;
    };
    let mut client = server.connect().await;
    let session = async |client: &mut TestClient| match client.ok(Request::SubscribeSession).await {
        Response::Session(session) => session,
        response => panic!("unexpected response: {response:?}"),
    };
    let save = |agent_id: Option<&AgentId>, name: &str, command: &AgentCommand| {
        Request::SaveCustomAgent(CustomAgentChange {
            agent_id: agent_id.cloned(),
            name: name.to_string(),
            command: command.clone(),
        })
    };

    mock.env.insert("TOKEN".into(), "secret".into());
    let Response::CustomAgentSaved(agent_id) = client.ok(save(None, " ", &mock)).await else {
        panic!("expected a saved agent");
    };
    assert_eq!(agent_id, AgentId::new("custom-mock-agent"));
    let snapshot = session(&mut client).await;
    let listing = snapshot.registry.agent(&agent_id).expect("listed");
    assert_eq!(listing.name().as_ref(), "Mock Agent");
    assert_eq!(listing.version().as_ref(), "1.2.3");
    let command = listing.custom_command.clone().expect("a custom agent");
    assert_eq!(command.path, mock.path);
    assert!(
        command.env.is_empty(),
        "the environment is the agent's settings'"
    );
    assert_eq!(
        snapshot.agent_settings[&agent_id].env.get("TOKEN"),
        Some(&"secret".to_string())
    );
    let stored = crate::load_custom_agents(server.data_dir.path()).expect("custom.json");
    assert_eq!(stored[&agent_id].name.as_ref(), "Mock Agent");

    // Two installed agents by one name would look the same in pickers.
    let taken = client
        .request(save(None, "mock", &mock))
        .await
        .expect_err("the name is taken");
    assert!(
        taken.message.contains("already called mock"),
        "{}",
        taken.message
    );

    let broken = AgentCommand {
        path: "/bin/sh".into(),
        args: vec![
            "-c".into(),
            "echo 'no such column: project_id' >&2; exit 3".into(),
        ],
        env: Default::default(),
        env_remove: Vec::new(),
    };
    let failed = client
        .request(save(None, "Broken", &broken))
        .await
        .expect_err("it doesn't start");
    assert!(
        failed.message.starts_with("It didn't start"),
        "{}",
        failed.message
    );
    assert!(
        failed.message.contains("no such column: project_id"),
        "{}",
        failed.message
    );

    // Its id stays, since threads keep it.
    let Response::CustomAgentSaved(renamed) =
        client.ok(save(Some(&agent_id), "Renamed", &mock)).await
    else {
        panic!("expected a saved agent");
    };
    assert_eq!(renamed, agent_id);
    let snapshot = session(&mut client).await;
    assert_eq!(
        snapshot
            .registry
            .agent(&agent_id)
            .map(|agent| agent.name().to_string()),
        Some("Renamed".to_string())
    );

    client
        .ok(Request::RemoveCustomAgent(agent_id.clone()))
        .await;
    let snapshot = session(&mut client).await;
    assert!(snapshot.registry.agent(&agent_id).is_none());
    assert!(snapshot.registry.agent(&AgentId::new("mock")).is_some());
    let stored = crate::load_custom_agents(server.data_dir.path()).expect("custom.json");
    assert!(!stored.contains_key(&agent_id));
}

/// New threads start on Use for New Threads' account, else the External one while the normal
/// home is logged in, else the first agentZ account, and keep it.
#[tokio::test(flavor = "multi_thread")]
async fn accounts_decide_where_new_threads_run() {
    let Some(mut command) = mock_agent() else {
        return;
    };
    let data_dir = tempfile::tempdir().expect("temp dir");
    // Never written, so the normal home stays logged out.
    let login_file = data_dir.path().join("logged-in");
    command.env.insert(
        "MOCK_LOGIN_FILE".into(),
        login_file.to_string_lossy().into_owned(),
    );
    let Some(server) =
        TestServer::start_with_agent(data_dir, tempfile::tempdir().expect("temp dir"), command)
    else {
        return;
    };
    let mock = AgentId::new("mock");
    let mut client = server.connect().await;
    let Response::Session(session) = client.ok(Request::SubscribeSession).await else {
        panic!("expected a session snapshot");
    };
    assert!(session.accounts.is_empty());
    let project_id = client.add_project(server.project_dir.path()).await;
    let create = |account| Request::CreateThread {
        project_id,
        agent_id: AgentId::new("mock"),
        workspace: Default::default(),
        account,
    };
    let account_of = |client: &TestClient, thread_id: ThreadId| {
        client
            .projects
            .as_ref()
            .and_then(|projects| {
                projects
                    .threads
                    .iter()
                    .find(|thread| thread.id == thread_id)
            })
            .expect("the thread")
            .account
    };

    let mut added = Vec::new();
    for _ in 0..2 {
        match client.ok(Request::AddAccount(mock.clone())).await {
            Response::AccountAdded(id) => added.push(id),
            response => panic!("unexpected response: {response:?}"),
        }
    }
    let [work, side] = added[..] else {
        panic!("expected two accounts");
    };
    client
        .ok(Request::UpdateAccount {
            agent_id: mock.clone(),
            account: Some(work),
            change: AccountChange::Rename(Some("Work".into())),
        })
        .await;

    // Before the normal home is checked, the External account is listed, as the agent's login
    // was before it had accounts.
    let Response::ThreadCreated(external) = client.ok(create(AccountChoice::Default)).await else {
        panic!("expected a thread");
    };
    assert_eq!(account_of(&client, external), None);
    client
        .subscribe_thread(ConnectionId::Thread(external))
        .await;
    client
        .wait_until(|client| {
            client.events.iter().any(|event| match event {
                Event::Accounts(accounts) => accounts
                    .get(&AgentId::new("mock"))
                    .is_some_and(|accounts| accounts.external_logged_in == Some(false)),
                _ => false,
            })
        })
        .await;

    let Response::ThreadCreated(on_work) = client.ok(create(AccountChoice::Default)).await else {
        panic!("expected a thread");
    };
    assert_eq!(account_of(&client, on_work), Some(work));
    client
        .ok(Request::UpdateAccount {
            agent_id: mock.clone(),
            account: Some(side),
            change: AccountChange::MakeDefault,
        })
        .await;
    let Response::ThreadCreated(on_side) = client.ok(create(AccountChoice::Default)).await else {
        panic!("expected a thread");
    };
    assert_eq!(account_of(&client, on_side), Some(side));
    let Response::ThreadCreated(chosen) = client.ok(create(AccountChoice::External)).await else {
        panic!("expected a thread");
    };
    assert_eq!(account_of(&client, chosen), None);
    assert!(
        client
            .request(create(AccountChoice::Account(AccountId(99))))
            .await
            .is_err()
    );

    // A new account's folder starts with its agent's files. Removing the account deletes the
    // folder; its threads keep it.
    let home = server
        .data_dir
        .path()
        .join("accounts")
        .join("mock")
        .join(side.to_string());
    assert_eq!(
        std::fs::read_to_string(home.join(".mock/settings.json")).expect("the starting file"),
        r#"{"sync": false}"#
    );
    client
        .ok(Request::RemoveAccount {
            agent_id: mock.clone(),
            account: side,
        })
        .await;
    assert!(!home.exists());
    assert_eq!(account_of(&client, on_side), Some(side));
    let Response::ThreadCreated(after) = client.ok(create(AccountChoice::Default)).await else {
        panic!("expected a thread");
    };
    assert_eq!(account_of(&client, after), Some(work));

    let saved = std::fs::read_to_string(server.data_dir.path().join("agents/accounts.json"))
        .expect("accounts.json");
    let saved: serde_json::Value = serde_json::from_str(&saved).expect("json");
    assert_eq!(saved["mock"]["accounts"][0]["label"], "Work");
    assert_eq!(saved["mock"]["accounts"].as_array().map(Vec::len), Some(1));
}

/// Each account has its own defaults for new threads, and a choice made in a thread becomes
/// its account's default. The External account's are the agent's settings from before.
#[tokio::test(flavor = "multi_thread")]
async fn each_account_keeps_its_own_defaults() {
    let Some(server) = TestServer::start() else {
        return;
    };
    let mock = AgentId::new("mock");
    let mut client = server.connect().await;
    client.ok(Request::SubscribeSession).await;
    let project_id = client.add_project(server.project_dir.path()).await;
    let Response::AccountAdded(work) = client.ok(Request::AddAccount(mock.clone())).await else {
        panic!("expected an account");
    };
    client
        .ok(Request::UpdateAgentSettings {
            agent_id: mock.clone(),
            account: Some(work),
            change: AgentSettingsChange::SetDefaultConfigOption {
                config_id: "model".into(),
                value: Some(acp::SessionConfigOptionValue::value_id("opus")),
            },
        })
        .await;
    assert!(
        client
            .request(Request::UpdateAgentSettings {
                agent_id: mock.clone(),
                account: Some(AccountId(99)),
                change: AgentSettingsChange::SetDefaultMode(None),
            })
            .await
            .is_err()
    );
    let create = async |client: &mut TestClient, account| match client
        .ok(Request::CreateThread {
            project_id,
            agent_id: AgentId::new("mock"),
            workspace: Default::default(),
            account,
        })
        .await
    {
        Response::ThreadCreated(thread_id) => thread_id,
        response => panic!("unexpected response: {response:?}"),
    };

    let on_work = create(&mut client, AccountChoice::Account(work)).await;
    let external = create(&mut client, AccountChoice::External).await;
    client.wait_until_ready(on_work).await;
    client.wait_until_ready(external).await;
    let model = |client: &TestClient, thread_id| {
        config_value(client.thread(ConnectionId::Thread(thread_id)), "model")
    };
    assert_eq!(model(&client, on_work).as_deref(), Some("opus"));
    assert_eq!(model(&client, external).as_deref(), Some("sonnet"));

    client
        .ok(Request::SetConfigOption {
            connection: ConnectionId::Thread(on_work),
            config_id: acp::SessionConfigId::new("effort"),
            value: acp::SessionConfigOptionValue::value_id("high"),
        })
        .await;
    client
        .wait_until(|client| {
            client.events.iter().any(|event| match event {
                Event::Accounts(accounts) => accounts
                    .get(&AgentId::new("mock"))
                    .and_then(|accounts| accounts.account(work))
                    .is_some_and(|account| {
                        account
                            .settings
                            .default_config_options
                            .contains_key("effort")
                            && !account.settings.known_config_options.is_empty()
                    }),
                _ => false,
            })
        })
        .await;
    let Response::Session(session) = client.ok(Request::SubscribeSession).await else {
        panic!("expected a session snapshot");
    };
    let external_settings = session
        .agent_settings
        .get(&mock)
        .cloned()
        .unwrap_or_default();
    assert!(
        !external_settings
            .default_config_options
            .contains_key("effort")
    );
    assert!(
        !external_settings
            .default_config_options
            .contains_key("model")
    );
}

/// A new account starts with the default account's Environment, defaults and settings files,
/// but not its login, and drops the defaults its first session doesn't offer. Copy settings
/// from puts another account's in their place, or nothing.
#[tokio::test(flavor = "multi_thread")]
async fn new_accounts_copy_another_accounts_settings() {
    let Some(command) = mock_agent() else {
        return;
    };
    let normal_home = tempfile::tempdir().expect("temp dir");
    std::fs::create_dir_all(normal_home.path().join(".mock")).expect("create");
    // The mock reads it only in an account's home, where it then offers two models.
    std::fs::write(
        normal_home.path().join(".mock/settings.json"),
        r#"{"models": ["sonnet", "haiku"], "sync": true}"#,
    )
    .expect("write");
    std::fs::write(normal_home.path().join("login"), "").expect("write");
    let description = crate::AgentDescription {
        settings_files: vec![".mock/settings.json".into()],
        normal_home: normal_home.path().to_string_lossy().into_owned(),
        ..mock_accounts()
    };
    let Some(server) = TestServer::start_with_description(
        tempfile::tempdir().expect("temp dir"),
        tempfile::tempdir().expect("temp dir"),
        command,
        description,
    ) else {
        return;
    };
    let mock = AgentId::new("mock");
    let mut client = server.connect().await;
    client.ok(Request::SubscribeSession).await;
    let project_id = client.add_project(server.project_dir.path()).await;
    let proxy = BTreeMap::from([("HTTPS_PROXY".to_string(), "http://proxy:3128".to_string())]);
    let mut external_env = proxy.clone();
    external_env.insert("MOCK_API_KEY".into(), "external-key".into());
    for change in [
        AgentSettingsChange::SetEnv(external_env),
        AgentSettingsChange::SetDefaultConfigOption {
            config_id: "model".into(),
            value: Some(acp::SessionConfigOptionValue::value_id("opus")),
        },
        AgentSettingsChange::SetDefaultConfigOption {
            config_id: "effort".into(),
            value: Some(acp::SessionConfigOptionValue::value_id("high")),
        },
    ] {
        client
            .ok(Request::UpdateAgentSettings {
                agent_id: mock.clone(),
                account: None,
                change,
            })
            .await;
    }
    let add = async |client: &mut TestClient| match client
        .ok(Request::AddAccount(AgentId::new("mock")))
        .await
    {
        Response::AccountAdded(id) => id,
        response => panic!("unexpected response: {response:?}"),
    };
    let settings_of = |client: &TestClient, id| {
        client
            .accounts("mock")
            .account(id)
            .map(|account| (account.settings_from, account.settings.clone()))
    };
    let home = |id: AccountId| {
        server
            .data_dir
            .path()
            .join("accounts/mock")
            .join(id.to_string())
    };
    let settings_file = |id: AccountId| -> Value {
        let text = std::fs::read_to_string(home(id).join(".mock/settings.json")).expect("read");
        serde_json::from_str(&text).expect("json")
    };

    let work = add(&mut client).await;
    client
        .wait_until(|client| settings_of(client, work).is_some())
        .await;
    let (from, settings) = settings_of(&client, work).expect("account");
    assert_eq!(from, SettingsSource::External);
    assert_eq!(settings.env, proxy);
    assert_eq!(settings.default_config_options.len(), 2);
    // The file the home starts with keeps sessions to itself.
    assert_eq!(
        settings_file(work),
        json!({"models": ["sonnet", "haiku"], "sync": false})
    );
    assert!(!home(work).join("login").exists());

    // Its first session offers no Opus, so that default goes.
    let on_work = match client
        .ok(Request::CreateThread {
            project_id,
            agent_id: mock.clone(),
            workspace: Default::default(),
            account: AccountChoice::Account(work),
        })
        .await
    {
        Response::ThreadCreated(thread_id) => thread_id,
        response => panic!("unexpected response: {response:?}"),
    };
    client.wait_until_ready(on_work).await;
    client
        .wait_until(|client| {
            settings_of(client, work).is_some_and(|(_, settings)| !settings.copied_defaults)
                && config_value(client.thread(ConnectionId::Thread(on_work)), "effort").as_deref()
                    == Some("high")
        })
        .await;
    let (_, settings) = settings_of(&client, work).expect("account");
    assert_eq!(
        settings.default_config_options.keys().collect::<Vec<_>>(),
        ["effort"]
    );
    assert_eq!(
        config_value(client.thread(ConnectionId::Thread(on_work)), "model").as_deref(),
        Some("sonnet")
    );

    // The next one copies the account for new threads.
    client
        .ok(Request::UpdateAccount {
            agent_id: mock.clone(),
            account: Some(work),
            change: AccountChange::MakeDefault,
        })
        .await;
    let side = add(&mut client).await;
    client
        .wait_until(|client| settings_of(client, side).is_some())
        .await;
    let (from, settings) = settings_of(&client, side).expect("account");
    assert_eq!(from, SettingsSource::Account(work));
    assert_eq!(settings.env, proxy);
    assert_eq!(settings_file(side), settings_file(work));

    // Nothing empties them, but keeps the file a new home starts with.
    client
        .ok(Request::CopyAccountSettings {
            agent_id: mock.clone(),
            account: side,
            from: SettingsSource::Nothing,
        })
        .await;
    client
        .wait_until(|client| {
            settings_of(client, side).is_some_and(|(from, _)| from == SettingsSource::Nothing)
        })
        .await;
    let (_, settings) = settings_of(&client, side).expect("account");
    assert!(settings.env.is_empty() && settings.default_config_options.is_empty());
    assert_eq!(settings_file(side), json!({"sync": false}));
    for from in [
        SettingsSource::Account(side),
        SettingsSource::Account(AccountId(99)),
    ] {
        assert!(
            client
                .request(Request::CopyAccountSettings {
                    agent_id: mock.clone(),
                    account: side,
                    from,
                })
                .await
                .is_err()
        );
    }
}

/// Each agentZ account runs the agent in a home of its own and logs in there, without the
/// login in the server's environment unless its Environment sets one. Removing an account
/// stops its agents before its folder goes, and its threads can't start again.
#[tokio::test(flavor = "multi_thread")]
async fn accounts_run_in_homes_of_their_own() {
    let Some(mut command) = mock_agent() else {
        return;
    };
    let data_dir = tempfile::tempdir().expect("temp dir");
    let external_login = data_dir.path().join("external-login");
    command.env.insert(
        "MOCK_LOGIN_FILE".into(),
        external_login.to_string_lossy().into_owned(),
    );
    // Stands for a key in the server's environment, which logs in the normal home.
    command
        .env
        .insert("MOCK_API_KEY".into(), "from-the-server".into());
    let Some(server) =
        TestServer::start_with_agent(data_dir, tempfile::tempdir().expect("temp dir"), command)
    else {
        return;
    };
    let mock = AgentId::new("mock");
    let mut client = server.connect().await;
    client.ok(Request::SubscribeSession).await;
    let project_id = client.add_project(server.project_dir.path()).await;
    let add = async |client: &mut TestClient| match client
        .ok(Request::AddAccount(AgentId::new("mock")))
        .await
    {
        Response::AccountAdded(id) => id,
        response => panic!("unexpected response: {response:?}"),
    };
    let work = add(&mut client).await;
    let side = add(&mut client).await;
    let keyed = add(&mut client).await;
    let create = async |client: &mut TestClient, account| match client
        .ok(Request::CreateThread {
            project_id,
            agent_id: AgentId::new("mock"),
            workspace: Default::default(),
            account,
        })
        .await
    {
        Response::ThreadCreated(thread_id) => ConnectionId::Thread(thread_id),
        response => panic!("unexpected response: {response:?}"),
    };
    let open = async |client: &mut TestClient, account| {
        let connection = create(client, account).await;
        client.subscribe_thread(connection).await;
        client
            .wait_until(|client| {
                let view = client.thread(connection);
                *view.status() == ConnectionStatus::AuthRequired
                    || !view.config_options().is_empty()
            })
            .await;
        connection
    };
    let logged_in = |client: &TestClient, connection| {
        *client.thread(connection).status() != ConnectionStatus::AuthRequired
    };
    let home = |account: AccountId| {
        server
            .data_dir
            .path()
            .join("accounts")
            .join("mock")
            .join(account.to_string())
    };

    let external = open(&mut client, AccountChoice::External).await;
    assert!(logged_in(&client, external));
    let on_work = open(&mut client, AccountChoice::Account(work)).await;
    assert!(!logged_in(&client, on_work));
    client
        .wait_until(|client| client.account_logged_in(Some(work)) == Some(false))
        .await;
    client
        .ok(Request::Authenticate {
            connection: on_work,
            method_id: acp::AuthMethodId::new("mock-login"),
            meta: None,
        })
        .await;
    client
        .wait_until(|client| !client.thread(on_work).config_options().is_empty())
        .await;
    assert!(home(work).join("login").exists());
    assert!(!external_login.exists());
    // Each account's threads tell whether it's logged in.
    client
        .wait_until(|client| {
            client.account_logged_in(Some(work)) == Some(true)
                && client.account_logged_in(None) == Some(true)
        })
        .await;

    // The other account shares nothing. The agent's settings log it in, here with a terminal
    // login, which runs without the server's key too.
    let on_side = open(&mut client, AccountChoice::Account(side)).await;
    assert!(!logged_in(&client, on_side));
    let Response::LoginSessionOpened(login_session_id) = client
        .ok(Request::OpenLoginSession {
            agent_id: mock.clone(),
            account: Some(side),
        })
        .await
    else {
        panic!("expected a login session");
    };
    let login = ConnectionId::LoginSession(login_session_id);
    client.subscribe_thread(login).await;
    let method_id = acp::AuthMethodId::new("mock-terminal-login");
    client
        .wait_until(|client| {
            let thread = client.thread(login);
            thread.logged_in() == Some(false)
                && thread
                    .auth_methods()
                    .iter()
                    .any(|method| *method.id() == method_id)
        })
        .await;
    client
        .ok(Request::TerminalLogin {
            connection: login,
            method_id,
        })
        .await;
    let key = TerminalKey::Login(login);
    client.subscribe_terminal(key.clone()).await;
    client
        .wait_for_screen(&key, "Press Enter to log in to the mock agent.")
        .await;
    client.type_into(&key, "\r").await;
    client
        .wait_until(|client| {
            client.thread(login).logged_in() == Some(true)
                && client.account_logged_in(Some(side)) == Some(true)
        })
        .await;
    assert!(home(side).join("login").exists());

    // A key an account's Environment sets on purpose stays.
    client
        .ok(Request::UpdateAgentSettings {
            agent_id: mock.clone(),
            account: Some(keyed),
            change: AgentSettingsChange::SetEnv(BTreeMap::from([(
                "MOCK_API_KEY".to_string(),
                "on-purpose".to_string(),
            )])),
        })
        .await;
    let on_keyed = open(&mut client, AccountChoice::Account(keyed)).await;
    assert!(logged_in(&client, on_keyed));
    assert!(!home(keyed).join("login").exists());

    client
        .ok(Request::RemoveAccount {
            agent_id: mock.clone(),
            account: side,
        })
        .await;
    assert!(!home(side).exists());
    client
        .wait_until(|client| {
            matches!(
                client.thread(on_side).status(),
                ConnectionStatus::Failed(error) if error.contains("account was removed")
            ) && client
                .events
                .iter()
                .any(|event| matches!(event, Event::ConnectionClosed(closed) if *closed == login))
        })
        .await;
    assert!(home(work).join("login").exists());
    assert!(
        client
            .request(Request::OpenLoginSession {
                agent_id: mock.clone(),
                account: Some(side),
            })
            .await
            .is_err()
    );
}

/// An account can log in with a key its agent reads from its environment, as with Droid's
/// "Factory API Key": agentZ asks for the key, keeps it in the account's folder for the user
/// only, and starts the account's agents with it. A key its reader refuses isn't tried, as
/// Droid would take any key, and Log Out forgets it.
#[tokio::test(flavor = "multi_thread")]
async fn api_key_accounts_keep_their_key() {
    let Some(mut command) = mock_agent() else {
        return;
    };
    let data_dir = tempfile::tempdir().expect("temp dir");
    command.env.insert(
        "MOCK_LOGIN_FILE".into(),
        data_dir
            .path()
            .join("external-login")
            .to_string_lossy()
            .into_owned(),
    );
    let description = crate::AgentDescription {
        key_login: Some(crate::KeyLogin {
            method: "mock-env-key".into(),
            variable: "MOCK_API_KEY".into(),
            reader: Some(mock_usage_reader(&command)),
        }),
        ..mock_accounts()
    };
    let Some(server) = TestServer::start_with_description(
        data_dir,
        tempfile::tempdir().expect("temp dir"),
        command,
        description,
    ) else {
        return;
    };
    let mock = AgentId::new("mock");
    let mut client = server.connect().await;
    client.ok(Request::SubscribeSession).await;
    let Response::AccountAdded(account) = client.ok(Request::AddAccount(mock.clone())).await else {
        panic!("expected an account");
    };
    let key_file = server
        .data_dir
        .path()
        .join("accounts/mock")
        .join(account.to_string())
        .join("agentz-api-key");
    let Response::LoginSessionOpened(login_session_id) = client
        .ok(Request::OpenLoginSession {
            agent_id: mock.clone(),
            account: Some(account),
        })
        .await
    else {
        panic!("expected a login session");
    };
    let login = ConnectionId::LoginSession(login_session_id);
    client.subscribe_thread(login).await;
    let key_method = acp::AuthMethodId::new("mock-env-key");
    client
        .wait_until(|client| {
            let thread = client.thread(login);
            thread.logged_in() == Some(false)
                && thread.auth_methods().iter().any(|method| {
                    *method.id() == key_method && login_input(method) == LoginInput::ApiKey
                })
        })
        .await;
    let log_in = |key: Option<&str>| Request::Authenticate {
        connection: login,
        method_id: key_method.clone(),
        meta: key.map(api_key_meta),
    };
    assert!(client.request(log_in(None)).await.is_err());

    let refused = client.request(log_in(Some("refused"))).await;
    assert!(
        refused
            .as_ref()
            .is_err_and(|error| error.message.contains("refused")),
        "{refused:?}"
    );
    assert!(!key_file.exists());

    client.ok(log_in(Some(" mk-good "))).await;
    client
        .wait_until(|client| {
            client.account_logged_in(Some(account)) == Some(true)
                && client.accounts("mock").logs_in_with_key(Some(account))
        })
        .await;
    assert_eq!(
        std::fs::read_to_string(&key_file).expect("the key"),
        "mk-good"
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        let mode = std::fs::metadata(&key_file)
            .expect("metadata")
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600);
    }

    // The account's threads start with it.
    let project_id = client.add_project(server.project_dir.path()).await;
    let Response::ThreadCreated(thread_id) = client
        .ok(Request::CreateThread {
            project_id,
            agent_id: mock.clone(),
            workspace: Default::default(),
            account: AccountChoice::Account(account),
        })
        .await
    else {
        panic!("expected a thread");
    };
    let thread = ConnectionId::Thread(thread_id);
    client.subscribe_thread(thread).await;
    client
        .wait_until(|client| *client.thread(thread).status() == ConnectionStatus::Ready)
        .await;

    client.ok(Request::Logout(login)).await;
    client
        .wait_until(|client| {
            client.account_logged_in(Some(account)) == Some(false)
                && !client.accounts("mock").logs_in_with_key(Some(account))
        })
        .await;
    assert!(!key_file.exists());
}

/// Gives the mock agent an agentZ account before the server starts, so it checks the External
/// one as it starts.
fn add_an_account_before_start(data_dir: &Path) -> AccountId {
    let agents = data_dir.join("agents");
    std::fs::create_dir_all(&agents).expect("create agents");
    std::fs::write(
        agents.join("accounts.json"),
        json!({"mock": {"accounts": [{"id": 1}], "last_id": 1}}).to_string(),
    )
    .expect("write accounts.json");
    AccountId(1)
}

/// Waits until the server has found the account logged in or out, `None` being the External
/// one. It may have before the client subscribed.
async fn wait_for_login_check(
    client: &mut TestClient,
    account: Option<AccountId>,
    logged_in: bool,
) {
    let Response::Session(session) = client.ok(Request::SubscribeSession).await else {
        panic!("expected a session snapshot");
    };
    let found = session
        .accounts
        .get(&AgentId::new("mock"))
        .and_then(|accounts| accounts.logged_in(account));
    if found != Some(logged_in) {
        client
            .wait_until(|client| client.account_logged_in(account) == Some(logged_in))
            .await;
    }
}

/// As the server starts, it opens an empty session in the normal home of each agent with agentZ
/// accounts, since whether it's logged in decides whether the External account is listed.
#[tokio::test(flavor = "multi_thread")]
async fn the_server_checks_the_external_login_as_it_starts() {
    let Some(mut command) = mock_agent() else {
        return;
    };
    let data_dir = tempfile::tempdir().expect("temp dir");
    // Never written, so the normal home is logged out.
    let external_login = data_dir.path().join("external-login");
    command.env.insert(
        "MOCK_LOGIN_FILE".into(),
        external_login.to_string_lossy().into_owned(),
    );
    let work = add_an_account_before_start(data_dir.path());
    let Some(server) =
        TestServer::start_with_agent(data_dir, tempfile::tempdir().expect("temp dir"), command)
    else {
        return;
    };
    let mut client = server.connect().await;
    wait_for_login_check(&mut client, None, false).await;
    let Response::Session(session) = client.ok(Request::SubscribeSession).await else {
        panic!("expected a session snapshot");
    };
    let accounts = &session.accounts[&AgentId::new("mock")];
    assert!(!accounts.lists_external());
    assert_eq!(accounts.new_thread_account(), Some(work));
    // Only the normal home is checked as the server starts.
    assert_eq!(accounts.account(work).expect("account").logged_in, None);
}

/// Where the agent's sessions open while it's logged out, as Claude's do, its status command
/// checks the login instead: as the server starts, as the agent's settings open, and after a
/// login or logout.
#[tokio::test(flavor = "multi_thread")]
async fn status_commands_check_logins_where_sessions_open_logged_out() {
    let Some(mut command) = mock_agent() else {
        return;
    };
    let data_dir = tempfile::tempdir().expect("temp dir");
    let external_login = data_dir.path().join("external-login");
    std::fs::write(&external_login, "").expect("log in the normal home");
    command.env.insert(
        "MOCK_LOGIN_FILE".into(),
        external_login.to_string_lossy().into_owned(),
    );
    command
        .env
        .insert("MOCK_OPENS_LOGGED_OUT".into(), "1".into());
    // As Claude's adapter takes `--cli auth status --json` after its script.
    let description = crate::AgentDescription {
        login_check: crate::LoginCheck::Command(crate::StatusCommand {
            program: None,
            args: vec!["--status".into()],
            after_agent_args: true,
            logged_in: crate::LoggedIn::Pointer("/logged_in".into()),
        }),
        ..mock_accounts()
    };
    let work = add_an_account_before_start(data_dir.path());
    let Some(server) = TestServer::start_with_description(
        data_dir,
        tempfile::tempdir().expect("temp dir"),
        command,
        description,
    ) else {
        return;
    };
    let mut client = server.connect().await;
    // No session was opened for it, so the status command said so.
    wait_for_login_check(&mut client, None, true).await;

    let Response::LoginSessionOpened(login_session_id) = client
        .ok(Request::OpenLoginSession {
            agent_id: AgentId::new("mock"),
            account: Some(work),
        })
        .await
    else {
        panic!("expected a login session");
    };
    let login = ConnectionId::LoginSession(login_session_id);
    client.subscribe_thread(login).await;
    // Its session opened, but the account isn't logged in.
    client
        .wait_until(|client| {
            client.thread(login).status() == &ConnectionStatus::Ready
                && client.account_logged_in(Some(work)) == Some(false)
        })
        .await;

    client
        .ok(Request::Authenticate {
            connection: login,
            method_id: acp::AuthMethodId::new("mock-login"),
            meta: None,
        })
        .await;
    client
        .wait_until(|client| client.account_logged_in(Some(work)) == Some(true))
        .await;
    client.ok(Request::Logout(login)).await;
    client
        .wait_until(|client| client.account_logged_in(Some(work)) == Some(false))
        .await;
    assert_eq!(client.account_logged_in(None), Some(true));
}

/// Each account's identity and limits are read with its agent's reader: as an app opens, after
/// each turn on the account, and on demand. A logged-out account keeps what was read before.
#[tokio::test(flavor = "multi_thread")]
async fn accounts_read_their_identity_and_limits() {
    let Some(mut command) = mock_agent() else {
        return;
    };
    let data_dir = tempfile::tempdir().expect("temp dir");
    let external_login = data_dir.path().join("external-login");
    std::fs::write(&external_login, "").expect("log in the normal home");
    command.env.insert(
        "MOCK_LOGIN_FILE".into(),
        external_login.to_string_lossy().into_owned(),
    );
    let description = crate::AgentDescription {
        reader: Some(mock_usage_reader(&command)),
        ..mock_accounts()
    };
    let work = add_an_account_before_start(data_dir.path());
    let home = data_dir.path().join("accounts/mock").join(work.to_string());
    std::fs::create_dir_all(&home).expect("create the account's home");
    std::fs::write(home.join("login"), "").expect("log in the account");
    let Some(server) = TestServer::start_with_description(
        data_dir,
        tempfile::tempdir().expect("temp dir"),
        command,
        description,
    ) else {
        return;
    };
    let mock = AgentId::new("mock");
    let mut client = server.connect().await;
    let used = |client: &TestClient, account| {
        client.accounts("mock").status(account).and_then(|read| {
            read.status
                .windows
                .first()
                .map(|window| window.used_percent)
        })
    };

    // An app opened, so every account is read.
    client.ok(Request::SubscribeSession).await;
    client
        .wait_until(|client| used(client, Some(work)) == Some(0.0) && used(client, None).is_some())
        .await;
    let read = client
        .accounts("mock")
        .status(Some(work))
        .cloned()
        .expect("a read");
    assert_eq!(read.status.email.as_deref(), Some("mock@example.com"));
    assert_eq!(read.status.plan.as_deref(), Some("Pro"));
    assert!(read.status.windows[0].resets_at.is_some());
    assert_eq!(client.account_logged_in(Some(work)), Some(true));

    let project_id = client.add_project(server.project_dir.path()).await;
    let Response::ThreadCreated(thread_id) = client
        .ok(Request::CreateThread {
            project_id,
            agent_id: mock.clone(),
            workspace: Default::default(),
            account: AccountChoice::Account(work),
        })
        .await
    else {
        panic!("expected a thread");
    };
    let connection = ConnectionId::Thread(thread_id);
    client.subscribe_thread(connection).await;
    client
        .ok(Request::Prompt {
            connection,
            prompt: PromptPart::text("hello"),
        })
        .await;
    client
        .wait_until(|client| used(client, Some(work)) == Some(10.0))
        .await;

    std::fs::write(home.join("usage"), "55").expect("use more elsewhere");
    client
        .ok(Request::RefreshUsage {
            agent_id: mock.clone(),
            account: Some(work),
        })
        .await;
    client
        .wait_until(|client| used(client, Some(work)) == Some(55.0))
        .await;

    std::fs::remove_file(home.join("login")).expect("log out elsewhere");
    client
        .ok(Request::RefreshUsage {
            agent_id: mock.clone(),
            account: Some(work),
        })
        .await;
    client
        .wait_until(|client| client.account_logged_in(Some(work)) == Some(false))
        .await;
    assert_eq!(used(&client, Some(work)), Some(55.0));
    assert!(
        client
            .request(Request::RefreshUsage {
                agent_id: mock,
                account: Some(AccountId(9)),
            })
            .await
            .is_err()
    );
}

/// Switch to Droid Core is saved through the account's reader, as Droid's `/limits` saves it,
/// and answered once the read after it says it's chosen.
#[tokio::test(flavor = "multi_thread")]
async fn droid_core_is_chosen_through_the_reader() {
    let Some(command) = mock_agent() else {
        return;
    };
    let data_dir = tempfile::tempdir().expect("temp dir");
    let description = crate::AgentDescription {
        reader: Some(mock_usage_reader(&command)),
        ..mock_accounts()
    };
    let work = add_an_account_before_start(data_dir.path());
    let home = data_dir.path().join("accounts/mock").join(work.to_string());
    std::fs::create_dir_all(&home).expect("create the account's home");
    // Droid-like, with nothing chosen.
    std::fs::write(home.join("overage"), "").expect("write the choice");
    let Some(server) = TestServer::start_with_description(
        data_dir,
        tempfile::tempdir().expect("temp dir"),
        command,
        description,
    ) else {
        return;
    };
    let mock = AgentId::new("mock");
    let mut client = server.connect().await;
    let overage = |client: &TestClient| {
        client
            .accounts("mock")
            .status(Some(work))
            .and_then(|read| read.status.overage)
    };
    client.ok(Request::SubscribeSession).await;
    client.wait_until(|client| overage(client).is_some()).await;
    let read = client
        .accounts("mock")
        .status(Some(work))
        .cloned()
        .expect("a read");
    assert_eq!(read.status.pool.as_deref(), Some("Standard"));
    assert_eq!(read.status.other_pools[0].label, "Droid Core");
    assert_eq!(read.status.credits.as_deref(), Some("$18.20"));
    assert_eq!(
        overage(&client).and_then(|overage| overage.preference),
        None
    );

    client
        .ok(Request::SwitchToDroidCore {
            agent_id: mock.clone(),
            account: Some(work),
        })
        .await;
    assert_eq!(
        overage(&client).and_then(|overage| overage.preference),
        Some(OveragePreference::DroidCore)
    );
    assert_eq!(
        std::fs::read_to_string(home.join("overage")).expect("the choice"),
        "DroidCore"
    );

    // An account without the choice, or none at all.
    std::fs::remove_file(home.join("overage")).expect("no choice");
    client
        .ok(Request::RefreshUsage {
            agent_id: mock.clone(),
            account: Some(work),
        })
        .await;
    client.wait_until(|client| overage(client).is_none()).await;
    for account in [Some(work), Some(AccountId(9))] {
        assert!(
            client
                .request(Request::SwitchToDroidCore {
                    agent_id: mock.clone(),
                    account,
                })
                .await
                .is_err()
        );
    }
}

/// Use Reset spends one of the account's limit resets through its reader, as Codex's does,
/// and is answered with the account read again. Without any left, or with nothing used, it
/// fails and spends nothing.
#[tokio::test(flavor = "multi_thread")]
async fn limit_resets_are_used_through_the_reader() {
    let Some(command) = mock_agent() else {
        return;
    };
    let data_dir = tempfile::tempdir().expect("temp dir");
    let description = crate::AgentDescription {
        reader: Some(mock_usage_reader(&command)),
        ..mock_accounts()
    };
    let work = add_an_account_before_start(data_dir.path());
    let home = data_dir.path().join("accounts/mock").join(work.to_string());
    std::fs::create_dir_all(&home).expect("create the account's home");
    std::fs::write(home.join("usage"), "100").expect("use the account up");
    std::fs::write(home.join("limit_resets"), "2").expect("grant resets");
    let Some(server) = TestServer::start_with_description(
        data_dir,
        tempfile::tempdir().expect("temp dir"),
        command,
        description,
    ) else {
        return;
    };
    let mock = AgentId::new("mock");
    let mut client = server.connect().await;
    let resets = |client: &TestClient| {
        client
            .accounts("mock")
            .status(Some(work))
            .and_then(|read| read.status.limit_resets)
    };
    let used = |client: &TestClient, account| {
        client.accounts("mock").status(account).and_then(|read| {
            read.status
                .windows
                .first()
                .map(|window| window.used_percent)
        })
    };
    client.ok(Request::SubscribeSession).await;
    client.wait_until(|client| resets(client).is_some()).await;
    let granted = resets(&client).expect("resets");
    assert_eq!(granted.available, 2);
    assert!(granted.next_expires_at.is_some());
    assert_eq!(used(&client, Some(work)), Some(100.0));

    client
        .ok(Request::UseLimitReset {
            agent_id: mock.clone(),
            account: Some(work),
        })
        .await;
    assert_eq!(used(&client, Some(work)), Some(0.0));
    assert_eq!(resets(&client).map(|resets| resets.available), Some(1));

    // Short of a limit, Codex has nothing to reset, and the reset stays.
    std::fs::write(home.join("usage"), "30").expect("use some of the account");
    let error = client
        .request(Request::UseLimitReset {
            agent_id: mock.clone(),
            account: Some(work),
        })
        .await
        .expect_err("nothing to reset");
    assert!(
        error.message.contains("nothing to reset right now"),
        "{}",
        error.message
    );
    assert_eq!(
        std::fs::read_to_string(home.join("limit_resets")).expect("resets"),
        "1"
    );

    // The External account has none, and an account that isn't there can't have any.
    for account in [None, Some(AccountId(9))] {
        assert!(
            client
                .request(Request::UseLimitReset {
                    agent_id: mock.clone(),
                    account,
                })
                .await
                .is_err()
        );
    }
}

/// A thread stopped by its account's limit gets "Continue." from the server when the limit
/// resets: every thread on an account set to Continue at reset, or one the limit notice asked
/// for. On an account set to Stop, it waits for the user.
#[tokio::test(flavor = "multi_thread")]
async fn threads_stopped_by_a_limit_continue_at_the_reset() {
    let Some(command) = mock_agent() else {
        return;
    };
    let data_dir = tempfile::tempdir().expect("temp dir");
    let description = crate::AgentDescription {
        reader: Some(mock_usage_reader(&command)),
        ..mock_accounts()
    };
    let work = add_an_account_before_start(data_dir.path());
    let home = data_dir.path().join("accounts/mock").join(work.to_string());
    std::fs::create_dir_all(&home).expect("create the account's home");
    std::fs::write(home.join("usage"), "100").expect("use the account up");
    let Some(server) = TestServer::start_with_description(
        data_dir,
        tempfile::tempdir().expect("temp dir"),
        command,
        description,
    ) else {
        return;
    };
    let mock = AgentId::new("mock");
    let mut client = server.connect().await;
    client.ok(Request::SubscribeSession).await;
    let project_id = client.add_project(server.project_dir.path()).await;
    let Response::ThreadCreated(thread_id) = client
        .ok(Request::CreateThread {
            project_id,
            agent_id: mock.clone(),
            workspace: Default::default(),
            account: AccountChoice::Account(work),
        })
        .await
    else {
        panic!("expected a thread");
    };
    let connection = ConnectionId::Thread(thread_id);
    client.subscribe_thread(connection).await;
    let continues_at = |client: &TestClient| {
        client
            .project_thread(thread_id)
            .and_then(|thread| thread.continues_at)
    };
    let used_up = |client: &TestClient| {
        client
            .accounts("mock")
            .status(Some(work))
            .is_some_and(|read| read.status.windows[0].used_percent == 100.)
    };

    client
        .ok(Request::Prompt {
            connection,
            prompt: PromptPart::text("hello"),
        })
        .await;
    client
        .wait_until(|client| {
            let thread = client.thread(connection);
            thread.turn_error().is_some() && !thread.is_working() && used_up(client)
        })
        .await;
    assert_eq!(continues_at(&client), None);

    // The notice's button, then its cancel.
    client
        .ok(Request::ContinueAtReset {
            thread_id,
            on: true,
        })
        .await;
    client
        .wait_until(|client| continues_at(client).is_some())
        .await;
    client
        .ok(Request::ContinueAtReset {
            thread_id,
            on: false,
        })
        .await;
    client
        .wait_until(|client| continues_at(client).is_none())
        .await;

    client
        .ok(Request::UpdateAccount {
            agent_id: mock,
            account: Some(work),
            change: AccountChange::SetAtLimit(AtLimit::ContinueAtReset),
        })
        .await;
    let resets_at = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .expect("time")
        .as_secs()
        + 3;
    std::fs::write(home.join("resets_at"), resets_at.to_string()).expect("set the reset");
    client
        .ok(Request::Prompt {
            connection,
            prompt: PromptPart::text("again"),
        })
        .await;
    client
        .wait_until(|client| continues_at(client).is_some())
        .await;
    assert_eq!(
        continues_at(&client),
        Some(SystemTime::UNIX_EPOCH + Duration::from_secs(resets_at))
    );
    client
        .wait_until(|client| {
            let thread = client.thread(connection);
            agent_text(thread).contains("Echo: Continue.") && !thread.is_working()
        })
        .await;
    assert_eq!(
        client.user_messages(thread_id),
        ["hello", "again", "Continue."]
    );
    assert_eq!(continues_at(&client), None);
}

/// Codex's device-code login asks the client to open a URL (an elicitation) while
/// `authenticate` waits. The request reaches the app through the server, and so does the
/// answer.
#[tokio::test(flavor = "multi_thread")]
async fn url_logins_relay_the_agents_elicitation() {
    let Some(mut command) = mock_agent() else {
        return;
    };
    let data_dir = tempfile::tempdir().expect("temp dir");
    let login_file = data_dir.path().join("logged-in");
    command.env.insert(
        "MOCK_LOGIN_FILE".into(),
        login_file.to_string_lossy().into_owned(),
    );
    let Some(server) =
        TestServer::start_with_agent(data_dir, tempfile::tempdir().expect("temp dir"), command)
    else {
        return;
    };
    let mut client = server.connect().await;
    client.ok(Request::SubscribeSession).await;
    let Response::LoginSessionOpened(login_session_id) = client
        .ok(Request::OpenLoginSession {
            agent_id: AgentId::new("mock"),
            account: None,
        })
        .await
    else {
        panic!("expected a login session");
    };
    let connection = ConnectionId::LoginSession(login_session_id);
    client.subscribe_thread(connection).await;
    // The agent's words when its session asked for a login.
    client
        .wait_until(|client| client.thread(connection).auth_description().is_some())
        .await;
    assert_eq!(
        client
            .thread(connection)
            .auth_description()
            .map(|description| description.as_ref()),
        Some("Your code: MOCK-1234\n\nClick Log In.")
    );
    assert_eq!(client.thread(connection).logged_in(), Some(false));

    client
        .ok(Request::Authenticate {
            connection,
            method_id: acp::AuthMethodId::new("mock-browser-login"),
            meta: None,
        })
        .await;
    client
        .wait_until(|client| !client.thread(connection).elicitations().is_empty())
        .await;
    let thread = client.thread(connection);
    assert!(thread.is_authenticating());
    let elicitation = thread.elicitations()[0].clone();
    assert_eq!(
        elicitation.url(),
        Some("https://example.com/device?code=MOCK-1234")
    );

    client
        .ok(Request::RespondToElicitation {
            connection,
            elicitation: elicitation.id,
            action: acp::ElicitationAction::Accept(acp::ElicitationAcceptAction::new()),
        })
        .await;
    client
        .wait_until(|client| {
            let thread = client.thread(connection);
            thread.logged_in() == Some(true)
                && !thread.is_authenticating()
                && thread.elicitations().is_empty()
        })
        .await;
    assert!(login_file.exists());
    let status = client.thread(connection).auth_status().cloned();
    assert_eq!(
        status
            .and_then(|status| status.account)
            .and_then(|account| account.email),
        Some("mock@example.com".to_string())
    );
}

/// Claude Agent and others log in by running a command in a terminal. It runs on the server's
/// machine, where the agent keeps its login, and the agent restarts logged in once it exits.
#[tokio::test(flavor = "multi_thread")]
async fn terminal_logins_run_on_the_server_and_restart_the_agent() {
    let Some(mut command) = mock_agent() else {
        return;
    };
    let data_dir = tempfile::tempdir().expect("temp dir");
    let login_file = data_dir.path().join("logged-in");
    command.env.insert(
        "MOCK_LOGIN_FILE".into(),
        login_file.to_string_lossy().into_owned(),
    );
    let Some(server) =
        TestServer::start_with_agent(data_dir, tempfile::tempdir().expect("temp dir"), command)
    else {
        return;
    };
    let mut client = server.connect().await;
    client.ok(Request::SubscribeSession).await;
    let Response::LoginSessionOpened(login_session_id) = client
        .ok(Request::OpenLoginSession {
            agent_id: AgentId::new("mock"),
            account: None,
        })
        .await
    else {
        panic!("expected a login session");
    };
    let connection = ConnectionId::LoginSession(login_session_id);
    client.subscribe_thread(connection).await;
    let method_id = acp::AuthMethodId::new("mock-terminal-login");
    // The agent reports its account apart from answering `initialize`, so it can say it's
    // logged out before its login methods are known.
    client
        .wait_until(|client| {
            let thread = client.thread(connection);
            thread.logged_in() == Some(false)
                && thread
                    .auth_methods()
                    .iter()
                    .any(|method| *method.id() == method_id)
        })
        .await;

    client
        .ok(Request::TerminalLogin {
            connection,
            method_id,
        })
        .await;
    let key = TerminalKey::Login(connection);
    client.subscribe_terminal(key.clone()).await;
    client.wait_for_screen(&key, "Press Enter to log in").await;
    client.type_into(&key, "\r").await;

    client
        .wait_until(|client| {
            !client.terminals.contains_key(&key)
                && client.thread(connection).logged_in() == Some(true)
        })
        .await;
    assert!(login_file.exists());
    assert_eq!(
        client
            .thread(connection)
            .login_notice()
            .map(|notice| notice.as_ref()),
        Some("Logged in.")
    );
    client
        .wait_until(|client| {
            client.events.iter().any(|event| match event {
                Event::AgentSettings(settings) => {
                    settings.get(&AgentId::new("mock")).is_some_and(|settings| {
                        settings.login_method.as_deref() == Some("Log in in a terminal")
                    })
                }
                _ => false,
            })
        })
        .await;

    // A method that doesn't run in a terminal is refused.
    assert!(
        client
            .request(Request::TerminalLogin {
                connection,
                method_id: acp::AuthMethodId::new("mock-login"),
            })
            .await
            .is_err()
    );
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

#[tokio::test(flavor = "multi_thread")]
async fn projects_show_their_branch() {
    let Some(server) = TestServer::start() else {
        return;
    };
    let folder = server.project_dir.path();
    crate::git::git(
        folder,
        &["init", "--quiet", "--initial-branch", "first"],
        &[],
    )
    .await
    .expect("git");
    let mut client = server.connect().await;
    client.ok(Request::SubscribeSession).await;
    let project_id = client.add_project(folder).await;
    let branch = |client: &TestClient| {
        let projects = client.projects.as_ref()?;
        let project = projects
            .projects
            .iter()
            .find(|project| project.id == project_id)?;
        projects
            .git_heads
            .iter()
            .find(|(folder, _)| *folder == project.path)
            .map(|(_, head)| head.branch.clone())
    };
    client
        .wait_until(|client| branch(client).as_deref() == Some("first"))
        .await;
    // A switch made outside agentZ shows at the next refresh.
    crate::git::git(folder, &["checkout", "--quiet", "-b", "second"], &[])
        .await
        .expect("git");
    client
        .wait_until(|client| branch(client).as_deref() == Some("second"))
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
                account: Default::default(),
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
    let pinned = client
        .tool(
            orchestrator,
            "agentz_thread_organize",
            json!({"threadId": worker.0, "action": "pin"}),
        )
        .await;
    assert_eq!(pinned["pinned"], json!(true));
    let organized = client
        .tool(
            orchestrator,
            "agentz_thread_organize",
            json!({"threadId": worker.0, "action": "archive"}),
        )
        .await;
    assert_eq!(
        (&organized["pinned"], &organized["archived"]),
        (&json!(false), &json!(true)),
        "archiving unpins"
    );
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

/// An agent with several accounts is listed with each one's models, and a launched thread or
/// delegated task runs on the account asked for, its model checked against that account's.
#[tokio::test(flavor = "multi_thread")]
async fn agents_launch_threads_on_an_account() {
    let Some(server) = TestServer::start() else {
        return;
    };
    let mock = AgentId::new("mock");
    let mut client = server.connect().await;
    let Response::Session(session) = client.ok(Request::SubscribeSession).await else {
        panic!("expected a session snapshot");
    };
    client.projects = Some(session.projects);
    let project_id = client.add_project(server.project_dir.path()).await;
    let orchestrator = client.create_thread_in(project_id).await;
    client.wait_until_ready(orchestrator).await;
    client
        .wait_until(|client| {
            client.events.iter().any(|event| match event {
                Event::AgentSettings(settings) => settings
                    .get(&mock)
                    .is_some_and(|settings| !settings.known_config_options.is_empty()),
                _ => false,
            })
        })
        .await;

    // With only the agent's own login, there's no account to choose.
    let capabilities = client
        .tool(orchestrator, "orchestrator_capabilities", json!({}))
        .await;
    assert_eq!(capabilities["agents"][0]["accounts"], Value::Null);

    // The mock offers fewer models in a home whose settings name them, as a plan can.
    let Response::AccountAdded(side) = client.ok(Request::AddAccount(mock.clone())).await else {
        panic!("expected an account");
    };
    client
        .ok(Request::UpdateAccount {
            agent_id: mock.clone(),
            account: Some(side),
            change: AccountChange::Rename(Some("Side".into())),
        })
        .await;
    let home = server
        .data_dir
        .path()
        .join("accounts/mock")
        .join(side.to_string());
    std::fs::write(home.join(".mock/settings.json"), r#"{"models": ["haiku"]}"#).expect("write");
    let Response::ThreadCreated(first) = client
        .ok(Request::CreateThread {
            project_id,
            agent_id: mock.clone(),
            workspace: Default::default(),
            account: AccountChoice::Account(side),
        })
        .await
    else {
        panic!("expected a thread");
    };
    client.wait_until_ready(first).await;
    client
        .wait_until(|client| {
            client.events.iter().any(|event| match event {
                Event::Accounts(accounts) => accounts
                    .get(&mock)
                    .and_then(|accounts| accounts.account(side))
                    .is_some_and(|account| !account.settings.known_config_options.is_empty()),
                _ => false,
            })
        })
        .await;

    let capabilities = client
        .tool(orchestrator, "orchestrator_capabilities", json!({}))
        .await;
    let accounts = capabilities["agents"][0]["accounts"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let listed: Vec<(Value, Value, Value, Vec<Value>)> = accounts
        .iter()
        .map(|account| {
            let models = account["models"]
                .as_array()
                .map(|models| models.iter().map(|model| model["id"].clone()).collect())
                .unwrap_or_default();
            (
                account["account"].clone(),
                account["name"].clone(),
                account["isDefault"].clone(),
                models,
            )
        })
        .collect();
    assert_eq!(
        listed,
        vec![
            (
                json!("external"),
                Value::Null,
                json!(true),
                vec![json!("opus"), json!("sonnet"), json!("haiku")]
            ),
            (
                json!(side.to_string()),
                json!("Side"),
                json!(false),
                vec![json!("haiku")]
            ),
        ]
    );

    // The model is checked against the account's own.
    let code = client
        .tool_failure(
            ToolCaller::Thread(orchestrator),
            "agentz_thread_launch",
            json!({"account": "side", "model": "opus"}),
        )
        .await;
    assert_eq!(code, "model_unavailable");
    let code = client
        .tool_failure(
            ToolCaller::Thread(orchestrator),
            "agentz_thread_launch",
            json!({"account": "nobody"}),
        )
        .await;
    assert_eq!(code, "account_unavailable");

    let launched = client
        .tool(
            orchestrator,
            "agentz_thread_launch",
            json!({"account": side.to_string(), "model": "Haiku"}),
        )
        .await;
    let on_side = ThreadId(launched["threadId"].as_u64().expect("a thread id"));
    assert_eq!(launched["model"], json!("haiku"));
    let launched = client
        .tool(orchestrator, "agentz_thread_launch", json!({}))
        .await;
    let on_default = ThreadId(launched["threadId"].as_u64().expect("a thread id"));

    // A task on another account doesn't take its parent's model when the account lacks it.
    let delegated = client
        .tool(
            orchestrator,
            "delegate_task",
            json!({"task": "hello", "account": side.0}),
        )
        .await;
    let task = task_id(&delegated);
    client.wait_until_ready(task).await;
    assert_eq!(
        config_value(client.thread(ConnectionId::Thread(task)), "model").as_deref(),
        Some("haiku")
    );
    client
        .wait_until(|client| {
            [on_side, on_default, task].map(|thread_id| {
                client
                    .project_thread(thread_id)
                    .map(|thread| thread.account)
            }) == [Some(Some(side)), Some(None), Some(Some(side))]
        })
        .await;
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
            prompt: PromptPart::text("more"),
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
            prompt: PromptPart::text(text),
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

    // One earlier turn, and the turns there are to pick from.
    let first = client.thread_diff(thread, DiffScope::Turn(1)).await;
    assert_eq!(
        diff_files(&first),
        vec![("src/a.txt", FileChange::Added, 1, 0)]
    );
    let numbers: Vec<u32> = first
        .finished_turns
        .iter()
        .map(|turn| turn.number)
        .collect();
    assert_eq!(numbers, [1, 2]);

    // t3code's working tree: everything uncommitted, untracked files too, as it is now.
    std::fs::write(repository.join("notes.txt"), "todo\n").expect("a file");
    let working_tree = client.thread_diff(thread, DiffScope::WorkingTree).await;
    assert_eq!(
        diff_files(&working_tree),
        vec![
            ("README.md", FileChange::Modified, 1, 1),
            ("notes.txt", FileChange::Added, 1, 0),
            ("src/a.txt", FileChange::Added, 1, 0),
        ]
    );
    assert!(matches!(
        working_tree.restore,
        RestoreAvailability::Unavailable(_)
    ));
    std::fs::remove_file(repository.join("notes.txt")).expect("removed");

    // t3code's branch changes: on main there's no base to compare with.
    let branch = client.thread_diff(thread, DiffScope::Branch).await;
    assert_eq!((branch.base_ref.clone(), branch.files.len()), (None, 0));

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
            account: Default::default(),
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

    // Any linked worktree goes too, one the project never made (herdr's Delete worktree
    // checkout), keeping its branch.
    let outside = tempfile::tempdir().expect("tempdir");
    let linked = outside.path().join("linked");
    git(
        &repository,
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            "linked",
            &linked.to_string_lossy(),
        ],
    )
    .await;
    let Response::WorkspaceRemoval(removal) = client
        .ok(Request::RemoveWorkspace {
            path: linked.clone(),
            force: false,
        })
        .await
    else {
        panic!("expected a removal");
    };
    assert_eq!(removal, WorkspaceRemoval::Removed);
    assert!(!linked.exists());
    assert!(
        git(&repository, &["branch", "--list", "linked"])
            .await
            .contains("linked")
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

    // A worktree made with no thread, for a terminal, is one of the project's for later threads.
    let Response::WorkspaceCreated(empty) = client
        .ok(Request::CreateWorkspace {
            folder: repository.to_path_buf(),
            kind: WorkspaceKind::Worktree,
            base: None,
            branch: Some("shell".into()),
        })
        .await
    else {
        panic!("expected the workspace's folder");
    };
    assert!(empty.ends_with("shell"), "{}", empty.display());
    assert!(empty.join("README.md").exists());
    assert!(matches!(
        client
            .ok(Request::CreateTerminalThread {
                project_id,
                command: TerminalCommand::default(),
                workspace: WorkspaceChoice::Existing(empty.clone()),
            })
            .await,
        Response::ThreadCreated(_)
    ));
    assert!(
        client
            .request(Request::CreateWorkspace {
                folder: repository.to_path_buf(),
                kind: WorkspaceKind::Worktree,
                base: None,
                branch: Some("shell".into()),
            })
            .await
            .is_err()
    );
    // Asked from inside a worktree, the checkouts are the repository's, with that branch.
    let Response::RepositoryCheckouts(checkouts) =
        client.ok(Request::RepositoryCheckouts(empty.clone())).await
    else {
        panic!("expected the repository's checkouts");
    };
    assert_eq!(checkouts.git.branch.as_deref(), Some("shell"));
    assert_eq!(checkouts.checkouts[0].kind, None);
    assert!(
        checkouts.checkouts.iter().any(
            |checkout| checkout.path == empty && checkout.kind == Some(WorkspaceKind::Worktree)
        )
    );

    // A repository that's no project gets worktrees too, starting from its own branch.
    let other = tempfile::tempdir().expect("a folder");
    let other = std::fs::canonicalize(other.path()).expect("a resolved path");
    git(&other, &["init", "-q", "-b", "trunk"]).await;
    git(&other, &["config", "user.name", "Test"]).await;
    git(&other, &["config", "user.email", "test@example.com"]).await;
    std::fs::write(other.join("README.md"), "other\n").expect("a file");
    git(&other, &["add", "."]).await;
    git(&other, &["commit", "-q", "-m", "first"]).await;
    let Response::WorkspaceCreated(loose) = client
        .ok(Request::CreateWorkspace {
            folder: other.clone(),
            kind: WorkspaceKind::Worktree,
            base: None,
            branch: Some("loose".into()),
        })
        .await
    else {
        panic!("expected the workspace's folder");
    };
    assert!(loose.join("README.md").exists());
    let Response::RepositoryCheckouts(checkouts) =
        client.ok(Request::RepositoryCheckouts(other.clone())).await
    else {
        panic!("expected the repository's checkouts");
    };
    let branches: Vec<_> = checkouts
        .checkouts
        .iter()
        .map(|checkout| checkout.branch.as_deref())
        .collect();
    assert_eq!(branches, vec![Some("trunk"), Some("loose")]);

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
                account: Default::default(),
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

#[tokio::test(flavor = "multi_thread")]
async fn threads_started_in_panes_work_in_any_folder() {
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
    let repository = std::fs::canonicalize(server.project_dir.path()).expect("a resolved path");
    git(&repository, &["init", "-q", "-b", "main"]).await;
    git(&repository, &["config", "user.name", "Test"]).await;
    git(&repository, &["config", "user.email", "test@example.com"]).await;
    std::fs::create_dir_all(repository.join("src")).expect("a folder");
    std::fs::write(repository.join("src/main.rs"), "fn main() {}\n").expect("a file");
    git(&repository, &["add", "."]).await;
    git(&repository, &["commit", "-q", "-m", "first"]).await;

    let mut client = server.connect().await;
    let Response::Session(session) = client.ok(Request::SubscribeSession).await else {
        panic!("expected a session snapshot");
    };
    client.projects = Some(session.projects);
    let project_id = client.add_project(&repository).await;
    let create = |folder: PathBuf, workspace| Request::CreateWorkspacesThread {
        folder,
        agent_id: AgentId::new("mock"),
        workspace,
        account: Default::default(),
    };
    let new_worktree = || WorkspaceChoice::New {
        kind: WorkspaceKind::Worktree,
        base: None,
        branch: None,
    };

    // It works in the folder it was started in, inside a project but not one of its threads.
    let src = repository.join("src");
    let Response::ThreadCreated(in_src) = client
        .ok(create(src.clone(), WorkspaceChoice::Checkout))
        .await
    else {
        panic!("expected a thread");
    };
    let thread = client.project_thread(in_src).expect("the thread");
    assert_eq!(thread.project_id, ProjectId::WORKSPACES);
    assert_eq!(thread.workspace.as_ref(), Some(&src));
    client.wait_until_ready(in_src).await;
    client.prompt_and_wait(in_src, "write a.txt hi").await;
    assert!(src.join("a.txt").exists());
    let capabilities = client
        .tool(in_src, "orchestrator_capabilities", json!({}))
        .await;
    assert_eq!(capabilities["projectId"], Value::Null);
    assert_eq!(capabilities["projectPath"], json!(src));

    // Or in a new worktree of the folder's repository, which is the project's.
    let Response::ThreadCreated(in_worktree) = client.ok(create(src.clone(), new_worktree())).await
    else {
        panic!("expected a thread");
    };
    let thread = client.project_thread(in_worktree).expect("the thread");
    let worktree = thread.workspace.clone().expect("a worktree");
    assert_eq!(thread.started_in.as_ref(), Some(&src));
    assert!(worktree.join("src/main.rs").exists());
    let project = |client: &TestClient| {
        client
            .projects
            .as_ref()
            .and_then(|projects| projects.projects.iter().find(|p| p.id == project_id))
            .cloned()
            .expect("the project")
    };
    assert!(
        project(&client)
            .workspaces
            .iter()
            .any(|workspace| workspace.path == worktree)
    );

    // Moved to the Agents list, it's a thread of the project its folder is in, still there.
    client.ok(Request::MoveToAgents(in_src)).await;
    let thread = client.project_thread(in_src).expect("the thread");
    assert_eq!(thread.project_id, project_id);
    assert_eq!(thread.workspace.as_ref(), Some(&src));
    assert!(client.request(Request::MoveToAgents(in_src)).await.is_err());

    // Outside every project, moving it adds its folder as a project.
    let docs = tempfile::tempdir().expect("a folder");
    let docs = std::fs::canonicalize(docs.path()).expect("a resolved path");
    let Response::ThreadCreated(in_docs) = client
        .ok(create(docs.clone(), WorkspaceChoice::Checkout))
        .await
    else {
        panic!("expected a thread");
    };
    assert!(
        client
            .request(create(docs.clone(), new_worktree()))
            .await
            .is_err(),
        "a folder outside git has no worktrees"
    );
    client.wait_until_ready(in_docs).await;
    client.prompt_and_wait(in_docs, "hello").await;
    client.ok(Request::MoveToAgents(in_docs)).await;
    let projects = client.projects.clone().expect("projects");
    let docs_project = projects
        .projects
        .iter()
        .find(|project| project.path == docs)
        .expect("the folder is a project");
    let thread = client.project_thread(in_docs).expect("the thread");
    assert_eq!(thread.project_id, docs_project.id);
    assert_eq!(thread.workspace, None);

    assert!(!projects.workspaces_expanded);
    client.ok(Request::ToggleWorkspacesExpanded).await;
    assert!(
        client
            .projects
            .as_ref()
            .is_some_and(|p| p.workspaces_expanded)
    );
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
    // A repository of its own, so its branch shows the shell follows the folder.
    let inner = server.project_dir.path().join("inner");
    std::fs::create_dir(&inner).expect("folder");
    crate::git::git(
        &inner,
        &["init", "--quiet", "--initial-branch", "elsewhere"],
        &[],
    )
    .await
    .expect("git");
    client.type_into(&key, "cd inner\n").await;
    client
        .wait_until(move |client| title(client).as_deref() == Some("inner"))
        .await;
    let folder = move |client: &TestClient| {
        snapshot(client)
            .terminal_folders
            .iter()
            .find(|(id, _)| *id == thread_id)
            .map(|(_, folder)| folder.clone())
    };
    client
        .wait_until(move |client| {
            folder(client).is_some_and(|folder| {
                folder.path.ends_with("inner")
                    && folder.is_repository
                    && folder.branch.as_deref() == Some("elsewhere")
            })
        })
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

    // Finding sees the command and its output. A capital letter makes case count.
    let find = |query: &str| Request::FindInTerminal {
        terminal: key.clone(),
        query: query.to_string(),
    };
    let found = match client.ok(find("Selected-")).await {
        Response::TerminalMatches(found) => found,
        response => panic!("unexpected response: {response:?}"),
    };
    assert_eq!(found.total, 0);
    let found = match client.ok(find("selected-")).await {
        Response::TerminalMatches(found) => found,
        response => panic!("unexpected response: {response:?}"),
    };
    assert_eq!(found.total, 2);
    let output = found.matches[1];
    assert_eq!(
        (output.start, output.end),
        (
            TerminalPoint {
                line: row as i32,
                column: 0
            },
            TerminalPoint {
                line: row as i32,
                column: 8
            }
        )
    );
    // Showing a match selects it.
    client
        .ok(Request::TerminalInput {
            terminal: key.clone(),
            input: TerminalInput::ShowMatch(output),
        })
        .await;
    assert_eq!(
        client.ok(Request::TerminalSelectionText(key.clone())).await,
        Response::Message("selected-".into())
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

/// An agent CLI in a terminal thread gets checkpoints as an ACP agent's turns do, with its
/// turns read from the screen.
#[cfg(any(target_os = "macos", target_os = "linux"))]
#[tokio::test(flavor = "multi_thread")]
async fn terminal_agents_turns_are_checkpointed() {
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
    // Codex shows its state in the terminal's title; "write <file>" edits as a turn would.
    let bin = tempfile::tempdir().expect("temp dir");
    let codex = bin.path().join("codex");
    std::fs::write(
        &codex,
        "#!/bin/sh\necho codex ready\nwhile read line; do case \"$line\" in \
         write\\ *) echo hello > \"${line#write }\" ;; \
         *) printf '\\033]0;%s\\007' \"$line\" ;; esac; done\n",
    )
    .expect("script");
    std::fs::set_permissions(&codex, std::os::unix::fs::PermissionsExt::from_mode(0o755))
        .expect("permissions");

    let mut client = server.connect().await;
    client.ok(Request::SubscribeSession).await;
    let project_id = client.add_project(repository).await;
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
    let is_working = move |client: &TestClient| {
        client
            .projects
            .as_ref()
            .is_some_and(|projects| projects.working_threads.contains(&thread_id))
    };

    // The baseline is taken once the agent is seen.
    client
        .wait_until(move |client| {
            client.projects.as_ref().is_some_and(|projects| {
                projects
                    .terminal_agents
                    .iter()
                    .any(|(id, _)| *id == thread_id)
            })
        })
        .await;
    client.type_into(&key, "⠋ project\n").await;
    client.wait_until(is_working).await;
    client.type_into(&key, "write a.txt\n").await;
    client.type_into(&key, "project\n").await;
    // Done only once the turn's checkpoint is there.
    client.wait_until(move |client| !is_working(client)).await;
    let diff = client.thread_diff(thread_id, DiffScope::All).await;
    assert_eq!((diff.status.clone(), diff.turns), (DiffStatus::Ready, 1));
    assert_eq!(diff_files(&diff), vec![("a.txt", FileChange::Added, 1, 0)]);

    client.type_into(&key, "⠙ project\n").await;
    client.wait_until(is_working).await;
    client.type_into(&key, "write README.md\n").await;
    client.type_into(&key, "project\n").await;
    client.wait_until(move |client| !is_working(client)).await;
    let latest = client.thread_diff(thread_id, DiffScope::LatestTurn).await;
    assert_eq!(latest.turns, 2);
    assert_eq!(
        diff_files(&latest),
        vec![("README.md", FileChange::Modified, 1, 1)]
    );
}

/// Zed's mentions: a file goes as its contents and another thread as its conversation, and the
/// user's message shows them as links.
#[tokio::test(flavor = "multi_thread")]
async fn mentions_go_to_the_agent() {
    let Some(server) = TestServer::start() else {
        return;
    };
    let notes = server.project_dir.path().join("notes.md");
    std::fs::write(&notes, "remember the milk").expect("a file");
    let mut client = server.connect().await;
    client.ok(Request::SubscribeSession).await;
    let project_id = client.add_project(server.project_dir.path()).await;
    let earlier = client.create_thread_in(project_id).await;
    client.wait_until_ready(earlier).await;
    client.prompt_and_wait(earlier, "hello there").await;

    let thread = client.create_thread_in(project_id).await;
    client.wait_until_ready(thread).await;
    let connection = ConnectionId::Thread(thread);
    client
        .ok(Request::Prompt {
            connection,
            prompt: vec![
                PromptPart::Text("look at ".into()),
                PromptPart::Path(notes.clone()),
                PromptPart::Text(" and ".into()),
                PromptPart::Thread(earlier),
            ],
        })
        .await;
    let file_uri = format!("file://{}", notes.display());
    let thread_uri = format!("agentz://thread/{}", earlier.0);
    let expected = format!("[with {file_uri}, {thread_uri}]");
    client
        .wait_until(|client| {
            let view = client.thread(connection);
            !view.is_working() && agent_text(view).contains(&expected)
        })
        .await;
    assert_eq!(
        client.user_messages(thread),
        [format!(
            "look at [@notes.md]({file_uri}) and [@hello there]({thread_uri})"
        )]
    );

    // The files @ can mention are the folder's.
    let Response::Files(listing) = client.ok(Request::ListFiles(thread)).await else {
        panic!("expected the thread's files");
    };
    assert!(listing.entries.iter().any(|entry| entry.path == "notes.md"));
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
            prompt: PromptPart::text("terminal printf 'built %s\\n' \"$PAGER-$GIT_PAGER\"; exit 4"),
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

/// A saved layout opens as a tab of its splits in the workspace's folder, running its
/// commands; what runs in a pane is read back as a command line to save.
#[cfg(any(target_os = "macos", target_os = "linux"))]
#[tokio::test(flavor = "multi_thread")]
async fn a_layout_opens_as_a_tab_running_its_commands() {
    let Some(server) = TestServer::start() else {
        return;
    };
    let folder = std::fs::canonicalize(server.project_dir.path()).expect("canonical path");
    let mut client = server.connect().await;
    client.ok(Request::SubscribeSession).await;
    let first = client
        .space_pane(SpaceRequest::CreateSpace {
            folder: folder.clone(),
            project_id: None,
            content: shell_in(&folder),
        })
        .await;
    let pane = |command: Option<&str>| {
        Box::new(LayoutNode::Pane {
            command: command.map(str::to_string),
        })
    };
    let layout = LayoutNode::Split {
        direction: Direction::Horizontal,
        ratio: 0.7,
        first: pane(Some("echo from-$PWD; sleep 30")),
        second: Box::new(LayoutNode::Split {
            direction: Direction::Vertical,
            ratio: 0.5,
            first: pane(None),
            second: pane(Some(" ")),
        }),
    };
    let location = client
        .space_pane(SpaceRequest::CreateTabFromLayout {
            space: first.space,
            layout,
        })
        .await;
    assert_eq!(location.space, first.space);
    client
        .wait_until(|client| client.space_snapshot().tab(location.tab).is_some())
        .await;
    let spaces = client.space_snapshot();
    let (_, tab) = spaces.tab(location.tab).expect("the tab");
    assert_eq!(tab.root.first_pane(), location.pane);
    assert!(matches!(
        tab.root,
        Node::Split { direction: Direction::Horizontal, ratio, .. } if (ratio - 0.7).abs() < 1e-6
    ));
    let commands: Vec<Option<String>> = tab
        .panes
        .iter()
        .map(|pane| match &pane.content {
            PaneContent::Terminal(terminal) => {
                assert_eq!(terminal.folder, folder);
                terminal.command.clone()
            }
            content => panic!("unexpected content: {content:?}"),
        })
        .collect();
    assert_eq!(
        commands,
        [Some("echo from-$PWD; sleep 30".to_string()), None, None]
    );
    let key = TerminalKey::Pane(location.pane);
    client.subscribe_terminal(key.clone()).await;
    client
        .wait_for_screen(&key, &format!("from-{}", folder.display()))
        .await;

    let shell = tab.panes[1].id;
    let shell_key = TerminalKey::Pane(shell);
    client.subscribe_terminal(shell_key.clone()).await;
    client.type_into(&shell_key, "cat - 'a b'\n").await;
    client
        .wait_until(|client| {
            client
                .space_snapshot()
                .pane(shell)
                .is_some_and(|(_, _, pane)| pane.command_line.as_deref() == Some("cat - 'a b'"))
        })
        .await;
}

/// A workspace is named after the folder most of its tabs are in, and describes that folder.
#[cfg(any(target_os = "macos", target_os = "linux"))]
#[tokio::test(flavor = "multi_thread")]
async fn a_space_follows_the_folder_most_tabs_are_in() {
    let Some(server) = TestServer::start() else {
        return;
    };
    let folder = std::fs::canonicalize(server.project_dir.path()).expect("canonical path");
    let inner = folder.join("inner");
    std::fs::create_dir(&inner).expect("an inner folder");
    let mut client = server.connect().await;
    client.ok(Request::SubscribeSession).await;
    let first = client
        .space_pane(SpaceRequest::CreateSpace {
            folder: folder.clone(),
            project_id: None,
            content: shell_in(&folder),
        })
        .await;
    let current = move |client: &TestClient| {
        client
            .space_snapshot()
            .space(first.space)
            .and_then(|space| space.current.clone())
    };
    client
        .wait_until(|client| current(client).is_some_and(|current| current.path == folder))
        .await;

    // The only tab moved, so the workspace did.
    let key = TerminalKey::Pane(first.pane);
    client.subscribe_terminal(key.clone()).await;
    client.type_into(&key, "cd inner\n").await;
    client
        .wait_until(|client| current(client).is_some_and(|current| current.path == inner))
        .await;
    assert_eq!(
        client
            .space_snapshot()
            .space(first.space)
            .expect("the space")
            .label(),
        "inner"
    );

    // Two more tabs in the original folder outvote it.
    for _ in 0..2 {
        client
            .space_pane(SpaceRequest::CreateTab {
                space: first.space,
                content: shell_in(&folder),
            })
            .await;
    }
    client
        .wait_until(|client| current(client).is_some_and(|current| current.path == folder))
        .await;
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

#[tokio::test(flavor = "multi_thread")]
async fn the_machine_icon_is_chosen_and_kept() {
    let Some(server) = TestServer::start() else {
        return;
    };
    let mut client = server.connect().await;
    assert!(
        client
            .welcome
            .capabilities
            .iter()
            .any(|capability| capability == agentz_protocol::CAPABILITY_MACHINE_ICON)
    );
    let Response::Session(session) = client.ok(Request::SubscribeSession).await else {
        panic!("expected the session");
    };
    assert_eq!(session.machine_icon.chosen, None);

    client
        .ok(Request::SetMachineIcon(Some(MachineKind::MacStudio)))
        .await;
    client
        .wait_until(|client| {
            client.events.iter().any(|event| {
                matches!(event, Event::MachineIcon(icon) if icon.kind() == MachineKind::MacStudio)
            })
        })
        .await;

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
        panic!("expected the session");
    };
    assert_eq!(session.machine_icon.chosen, Some(MachineKind::MacStudio));
}

#[cfg(unix)]
#[tokio::test(flavor = "multi_thread")]
async fn deleting_a_terminal_thread_ends_everything_it_started() {
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
    let key = TerminalKey::Thread(thread_id);
    client.subscribe_terminal(key.clone()).await;
    // Ignores the hangup the closing terminal sends.
    client
        .type_into(&key, "nohup sleep 300 >/dev/null 2>&1 & echo started=$!\n")
        .await;
    client.wait_for_screen(&key, "started=").await;
    let screen = client.screen(&key);
    let pid: libc::pid_t = screen
        .split("started=")
        .filter_map(|rest| {
            rest.chars()
                .take_while(char::is_ascii_digit)
                .collect::<String>()
                .parse()
                .ok()
        })
        .last()
        .expect("the pid");
    // SAFETY: signal 0 only checks the process exists.
    let is_alive = move || unsafe { libc::kill(pid, 0) == 0 };
    assert!(is_alive());

    client.ok(Request::DeleteThread(thread_id)).await;
    let ended = tokio::time::timeout(TIMEOUT, async {
        while is_alive() {
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await;
    assert!(ended.is_ok(), "process {pid} outlived its terminal");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_drawer_holds_several_terminals() {
    let Some(server) = TestServer::start() else {
        return;
    };
    let mut client = server.connect().await;
    client.ok(Request::SubscribeSession).await;
    let thread_id = client.create_thread(&server).await;
    let drawer_terminals = |response: Response| match response {
        Response::DrawerTerminals(numbers) => numbers,
        response => panic!("unexpected response: {response:?}"),
    };
    assert_eq!(
        drawer_terminals(client.ok(Request::DrawerTerminals(thread_id)).await),
        Vec::<u32>::new()
    );

    // Terminal 1 is the drawer's own; split and new terminals are numbered after it.
    for number in [1, 2, 3] {
        let key = TerminalKey::drawer(thread_id, number);
        client.subscribe_terminal(key.clone()).await;
        client
            .type_into(&key, &format!("echo terminal-{number}\n"))
            .await;
        client
            .wait_for_screen(&key, &format!("terminal-{number}"))
            .await;
    }
    assert_eq!(
        drawer_terminals(client.ok(Request::DrawerTerminals(thread_id)).await),
        vec![1, 2, 3]
    );

    // What runs in front of a drawer terminal's shell is reported, for the hidden drawer's
    // indicator.
    let running = move |client: &TestClient| {
        client
            .projects
            .as_ref()
            .map(|projects| projects.drawer_commands.clone())
            .unwrap_or_default()
    };
    let third = TerminalKey::drawer(thread_id, 3);
    client.type_into(&third, "sleep 30\n").await;
    client
        .wait_until(move |client| running(client) == vec![(thread_id, 3, "sleep".to_string())])
        .await;
    client.type_into(&third, "\x03").await;
    client
        .wait_until(move |client| running(client).is_empty())
        .await;

    client
        .ok(Request::CloseTerminal(TerminalKey::drawer(thread_id, 2)))
        .await;
    assert_eq!(
        drawer_terminals(client.ok(Request::DrawerTerminals(thread_id)).await),
        vec![1, 3]
    );
}

impl TestClient {
    /// Starts a "permission" turn, which waits for its answer.
    async fn start_waiting_turn(&mut self, thread_id: ThreadId) {
        let connection = ConnectionId::Thread(thread_id);
        self.ok(Request::Prompt {
            connection,
            prompt: PromptPart::text("permission"),
        })
        .await;
        self.wait_until(|client| {
            !client
                .thread(connection)
                .state
                .permission_requests
                .is_empty()
        })
        .await;
    }

    async fn queue(&mut self, thread_id: ThreadId, text: &str) {
        self.ok(Request::QueueMessage {
            connection: ConnectionId::Thread(thread_id),
            prompt: PromptPart::text(text),
        })
        .await;
    }

    fn queued_texts(&self, thread_id: ThreadId) -> Vec<String> {
        self.thread(ConnectionId::Thread(thread_id))
            .state
            .queued_messages
            .iter()
            .map(|message| {
                message
                    .prompt
                    .iter()
                    .map(|part| match part {
                        PromptPart::Text(text) => text.as_str(),
                        _ => "",
                    })
                    .collect()
            })
            .collect()
    }

    async fn answer_permission(&mut self, thread_id: ThreadId) {
        let connection = ConnectionId::Thread(thread_id);
        let request = self
            .thread(connection)
            .state
            .permission_requests
            .first()
            .expect("a permission request")
            .clone();
        self.ok(Request::RespondToPermission {
            connection,
            tool_call_id: request.tool_call_id,
            option_id: acp::PermissionOptionId::new("allow"),
        })
        .await;
    }
}

/// Zed's queue, kept by the server: what's queued while the agent works waits there for the
/// app to come back, and for the server to come back, and goes once the turn ends.
#[tokio::test(flavor = "multi_thread")]
async fn queued_messages_outlive_the_app_and_the_server() {
    let Some(server) = TestServer::start() else {
        return;
    };
    let mut client = server.connect().await;
    let project_id = client.add_project(server.project_dir.path()).await;
    let thread_id = client.create_thread_in(project_id).await;
    client.wait_until_ready(thread_id).await;
    client.start_waiting_turn(thread_id).await;
    client.queue(thread_id, "first").await;
    client.queue(thread_id, "second").await;
    client
        .wait_until(|client| client.queued_texts(thread_id) == ["first", "second"])
        .await;
    drop(client);

    // The app comes back to the same queue.
    let mut client = server.connect().await;
    client
        .subscribe_thread(ConnectionId::Thread(thread_id))
        .await;
    assert_eq!(client.queued_texts(thread_id), ["first", "second"]);
    let first = client
        .thread(ConnectionId::Thread(thread_id))
        .state
        .queued_messages[0]
        .id;
    client
        .ok(Request::RemoveQueuedMessage {
            connection: ConnectionId::Thread(thread_id),
            id: first,
        })
        .await;
    client
        .wait_until(|client| client.queued_texts(thread_id) == ["second"])
        .await;
    client.ok(Request::Shutdown).await;
    tokio::time::timeout(TIMEOUT, server.handle.stopped())
        .await
        .expect("the server stops");
    drop(client);

    // So does a new server, which sends it once the thread's agent is back.
    let Some(server) = TestServer::start_with(server.data_dir, server.project_dir) else {
        return;
    };
    let mut client = server.connect().await;
    client
        .subscribe_thread(ConnectionId::Thread(thread_id))
        .await;
    client
        .wait_until(|client| {
            client.queued_texts(thread_id).is_empty()
                && agent_text(client.thread(ConnectionId::Thread(thread_id)))
                    .contains("Echo: second")
        })
        .await;
}

/// A thread's conversation outlives the server in its transcript, though the agent replays
/// only its end, as Factory Droid replays only its last 100 messages, and goes with the
/// thread.
#[tokio::test(flavor = "multi_thread")]
async fn conversations_outlive_the_server() {
    let Some(mut command) = mock_agent() else {
        return;
    };
    let history_dir = tempfile::tempdir().expect("temp dir");
    let history = history_dir.path().join("history.json");
    command.args.push(history.to_string_lossy().into_owned());
    let Some(server) = TestServer::start_with_agent(
        tempfile::tempdir().expect("temp dir"),
        tempfile::tempdir().expect("temp dir"),
        command.clone(),
    ) else {
        return;
    };
    let mut client = server.connect().await;
    let project_id = client.add_project(server.project_dir.path()).await;
    let thread_id = client.create_thread_in(project_id).await;
    let connection = ConnectionId::Thread(thread_id);
    client.wait_until_ready(thread_id).await;
    client
        .ok(Request::Prompt {
            connection,
            prompt: PromptPart::text("hello there"),
        })
        .await;
    client
        .wait_until(|client| {
            let thread = client.thread(connection);
            !thread.is_working() && agent_text(thread) == "Echo: hello there"
        })
        .await;
    let entries = client.thread(connection).entries.clone();
    client.ok(Request::Shutdown).await;
    tokio::time::timeout(TIMEOUT, server.handle.stopped())
        .await
        .expect("the server stops");
    drop(client);
    let transcript = server
        .data_dir
        .path()
        .join("transcripts")
        .join(format!("{}.json", thread_id.0));
    assert!(transcript.exists());
    let updates: Vec<Value> =
        serde_json::from_slice(&std::fs::read(&history).expect("the agent's history"))
            .expect("updates");
    let end = &updates[updates.len() - 1..];
    std::fs::write(&history, serde_json::to_vec(end).expect("encodes")).expect("written");

    let Some(server) = TestServer::start_with_agent(server.data_dir, server.project_dir, command)
    else {
        return;
    };
    let mut client = server.connect().await;
    client.subscribe_thread(connection).await;
    assert_eq!(client.thread(connection).entries, entries);
    client
        .wait_until(|client| client.thread(connection).status() == &ConnectionStatus::Ready)
        .await;
    assert_eq!(
        client.thread(connection).session_restore(),
        Some(agentz_protocol::thread::SessionRestore::Loaded)
    );
    assert_eq!(client.thread(connection).entries, entries);

    client.ok(Request::DeleteThread(thread_id)).await;
    assert!(!transcript.exists());
}

/// Steering, for an agent that can't take a message into its turn: the message goes first and
/// the turn ends at the next step, here at once, as its only step waits for an answer.
#[tokio::test(flavor = "multi_thread")]
async fn steering_a_queued_message_ends_the_turn_for_it() {
    let Some(server) = TestServer::start() else {
        return;
    };
    let mut client = server.connect().await;
    let project_id = client.add_project(server.project_dir.path()).await;
    let thread_id = client.create_thread_in(project_id).await;
    let connection = ConnectionId::Thread(thread_id);
    client.wait_until_ready(thread_id).await;
    client.start_waiting_turn(thread_id).await;
    client.queue(thread_id, "later").await;
    client.queue(thread_id, "now").await;
    client
        .wait_until(|client| client.queued_texts(thread_id).len() == 2)
        .await;
    let now = client.thread(connection).state.queued_messages[1].id;
    client
        .ok(Request::SteerQueuedMessage {
            connection,
            id: now,
        })
        .await;
    client
        .wait_until(|client| {
            client.user_messages(thread_id) == ["permission", "now", "later"]
                && !client.thread(connection).is_working()
        })
        .await;
    assert!(client.queued_texts(thread_id).is_empty());
    assert!(!client.thread(connection).state.steering_queued);
}

/// An agent that takes messages into its turn gets a steered message at once.
#[tokio::test(flavor = "multi_thread")]
async fn steering_sends_a_queued_message_into_the_turn() {
    let Some(mut command) = mock_agent() else {
        return;
    };
    command.env.insert("MOCK_STEERING".into(), "1".into());
    let Some(server) = TestServer::start_with_agent(
        tempfile::tempdir().expect("temp dir"),
        tempfile::tempdir().expect("temp dir"),
        command,
    ) else {
        return;
    };
    let mut client = server.connect().await;
    let project_id = client.add_project(server.project_dir.path()).await;
    let thread_id = client.create_thread_in(project_id).await;
    let connection = ConnectionId::Thread(thread_id);
    client.wait_until_ready(thread_id).await;
    client.start_waiting_turn(thread_id).await;
    client.queue(thread_id, "also this").await;
    client
        .wait_until(|client| client.queued_texts(thread_id).len() == 1)
        .await;
    let id = client.thread(connection).state.queued_messages[0].id;
    client
        .ok(Request::SteerQueuedMessage { connection, id })
        .await;
    client
        .wait_until(|client| client.queued_texts(thread_id).is_empty())
        .await;
    client.answer_permission(thread_id).await;
    client
        .wait_until(|client| {
            !client.thread(connection).is_working()
                && agent_text(client.thread(connection)).ends_with("(steered: also this)")
        })
        .await;
}

/// Images are kept by the thread's server and named by their hash: the user's go to the agent
/// from there, the agent's are kept as it shows them, and clients fetch both, or thumbnails.
#[tokio::test(flavor = "multi_thread")]
async fn images_are_kept_for_the_thread() {
    use agentz_protocol::attachments::AttachmentId;
    use base64::Engine as _;

    let Some(mut command) = mock_agent() else {
        return;
    };
    command.env.insert("MOCK_IMAGES".into(), "1".into());
    let Some(server) = TestServer::start_with_agent(
        tempfile::tempdir().expect("temp dir"),
        tempfile::tempdir().expect("temp dir"),
        command,
    ) else {
        return;
    };
    let mut client = server.connect().await;
    client.ok(Request::SubscribeSession).await;
    let project_id = client.add_project(server.project_dir.path()).await;
    let thread_id = client.create_thread_in(project_id).await;
    let connection = ConnectionId::Thread(thread_id);
    client.wait_until_ready(thread_id).await;

    let mut png = Vec::new();
    image::RgbaImage::from_pixel(1200, 600, image::Rgba([10, 120, 200, 255]))
        .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
        .expect("a PNG");
    let base64 = &base64::engine::general_purpose::STANDARD;
    let Response::Attachment(id) = client
        .ok(Request::AddAttachment {
            thread_id,
            mime_type: "image/png".into(),
            data: base64.encode(&png),
        })
        .await
    else {
        panic!("expected the image's id");
    };
    client
        .ok(Request::Prompt {
            connection,
            prompt: vec![
                PromptPart::Text("see ".into()),
                PromptPart::Image(id.clone()),
            ],
        })
        .await;
    client
        .wait_until(|client| {
            !client.thread(connection).is_working()
                && agent_text(client.thread(connection)).contains("[with image/png]")
        })
        .await;
    assert_eq!(
        client.user_messages(thread_id),
        [format!("see [@Image]({})", id.uri())]
    );
    let Response::AttachmentData(thumbnail) = client
        .ok(Request::Attachment {
            thread_id,
            id: id.clone(),
            thumbnail: true,
        })
        .await
    else {
        panic!("expected a thumbnail");
    };
    let thumbnail =
        image::load_from_memory(&base64.decode(thumbnail.data).expect("base64")).expect("an image");
    assert_eq!((thumbnail.width(), thumbnail.height()), (640, 320));

    // The agent's images, in its reply and its tool call.
    client.prompt_and_wait(thread_id, "image").await;
    let view = client.thread(connection);
    let tool_images = view
        .entries()
        .iter()
        .find_map(|entry| match entry {
            Entry::ToolCall(tool_call) if tool_call.title == "Take a screenshot" => {
                Some(tool_call.images.clone())
            }
            _ => None,
        })
        .expect("the tool call");
    let [shown] = tool_images.as_slice() else {
        panic!("expected one image, got {tool_images:?}");
    };
    assert!(agent_text(view).contains(&shown.markdown_link()));
    let Response::AttachmentData(original) = client
        .ok(Request::Attachment {
            thread_id,
            id: shown.clone(),
            thumbnail: false,
        })
        .await
    else {
        panic!("expected the image");
    };
    assert_eq!(original.mime_type, "image/png");

    // Only well-formed ids name an image, and deleting the thread forgets its images.
    let attachments = server
        .data_dir
        .path()
        .join("attachments")
        .join(thread_id.0.to_string());
    assert!(attachments.is_dir());
    assert!(serde_json::from_value::<AttachmentId>(json!("../state.json")).is_err());
    client.ok(Request::DeleteThread(thread_id)).await;
    let deadline = Instant::now() + TIMEOUT;
    while attachments.exists() {
        assert!(Instant::now() < deadline, "the images are still kept");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}
