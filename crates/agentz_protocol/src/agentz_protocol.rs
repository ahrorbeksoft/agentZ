//! What `agentz-server` and its clients (the app, and later the MCP bridge and CLI) say to each
//! other, and how it's framed.
//!
//! - **Framing:** each message is JSON behind a little-endian `u32` length, as in herdr's wire
//!   protocol. It runs over any byte stream: a unix socket, or SSH's stdio through `proxy`.
//! - **Handshake:** the client sends a [`ClientHello`], the server answers with a
//!   [`ServerWelcome`].
//! - **Messages:** [`Request`]s carry ids and get one [`Response`] each. Subscriptions start
//!   with a snapshot in the response, then [`Event`]s follow.
//! - **Compatibility:** remote servers outlive client releases. New fields need serde defaults,
//!   and the enums fall back to `Unknown`, which the other side ignores or refuses. Clients turn
//!   off features the server's [`ServerWelcome::capabilities`] don't list (herdr's and t3code's
//!   rule).

pub mod agents;
pub mod diff;
pub mod layout;
pub mod spaces;
pub mod terminal;
pub mod terminal_keys;
pub mod thread;
pub mod workspace;

use std::collections::BTreeMap;
use std::path::PathBuf;

use agent_client_protocol::schema::v1 as acp;
use anyhow::{Context as _, Result};
use projects::{ProjectIcon, ProjectId, ProjectScope, ProjectsSnapshot, ThreadId, ThreadOrder};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncRead, AsyncReadExt as _, AsyncWrite, AsyncWriteExt as _};

use crate::agents::{AgentId, AgentSettings, RegistrySnapshot};
use crate::diff::{DiffScope, ThreadDiff};
use crate::spaces::{PaneLocation, SpaceRequest, SpacesSnapshot};
use crate::terminal::{
    TerminalCommand, TerminalFrame, TerminalInput, TerminalKey, TerminalProgram,
};
use crate::thread::{ThreadUpdate, ThreadView};
use crate::workspace::{ProjectGit, WorkspaceChoice, WorkspaceRemoval};

/// Bumped when a change can't be made compatibly.
pub const PROTOCOL_VERSION: u32 = 1;

/// [`ServerWelcome::capabilities`]: the server answers [`Request::ThreadDiff`].
pub const CAPABILITY_THREAD_DIFF: &str = "thread_diff";
/// [`ServerWelcome::capabilities`]: threads can work in worktrees and pastures
/// ([`Request::ProjectGit`], [`Request::RemoveWorkspace`] and the rest).
pub const CAPABILITY_WORKSPACES: &str = "workspaces";
/// [`ServerWelcome::capabilities`]: the server runs terminals ([`Request::SubscribeTerminal`]
/// and the rest), and terminal threads.
pub const CAPABILITY_TERMINALS: &str = "terminals";
/// [`ServerWelcome::capabilities`]: the server lists folders ([`Request::BrowseDirectories`]),
/// so projects can be added on its machine.
pub const CAPABILITY_BROWSE_DIRECTORIES: &str = "browse_directories";
/// [`ServerWelcome::capabilities`]: agents can reach the app's other machines through it
/// ([`Request::SetPeers`], [`Event::RelayToolCall`]).
pub const CAPABILITY_RELAY: &str = "relay";
/// [`ServerWelcome::capabilities`]: the server keeps the Workspaces view's spaces
/// ([`Request::Spaces`], [`SessionSnapshot::spaces`]) and runs their pane terminals.
pub const CAPABILITY_SPACES: &str = "spaces";
/// [`ServerWelcome::capabilities`]: the server detects its machine's kind and keeps the icon
/// chosen for it ([`SessionSnapshot::machine_icon`], [`Request::SetMachineIcon`]).
pub const CAPABILITY_MACHINE_ICON: &str = "machine_icon";
/// [`ServerWelcome::capabilities`]: a thread's drawer holds several terminals
/// ([`terminal::TerminalKey::DrawerTerminal`], [`Request::DrawerTerminals`]).
pub const CAPABILITY_DRAWER_TERMINALS: &str = "drawer_terminals";

/// Larger frames are refused, so a bad length can't make the reader allocate without bound.
/// Long threads with big tool outputs are the largest messages.
pub const MAX_FRAME_SIZE: usize = 64 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ClientHello {
    pub protocol_version: u32,
    pub client_version: String,
    pub client_kind: ClientKind,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ClientKind {
    App,
    /// `agentz-server` itself, e.g. for `stop` or `call`.
    Cli,
    /// `agentz-server mcp-bridge`, started by an agent.
    Mcp,
    /// From a newer version.
    #[serde(untagged)]
    Unknown(serde_json::Value),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ServerWelcome {
    pub protocol_version: u32,
    pub server_version: String,
    pub machine: MachineInfo,
    /// The process id, so a client can tell whether it's talking to the server it started.
    #[serde(default)]
    pub pid: u32,
    #[serde(default)]
    pub capabilities: Vec<String>,
    /// The SHA-256 recorded next to the binary when it was installed over SSH, read as the
    /// server started. A client that has since installed another binary can tell the running
    /// server is older.
    #[serde(default)]
    pub build: Option<String>,
    /// Set when the server refuses the client, e.g. for an unsupported protocol version.
    #[serde(default)]
    pub error: Option<String>,
}

/// The machine a server runs on.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MachineInfo {
    /// Stable for the machine, whatever route reaches it (t3code's machine identity).
    pub id: String,
    pub hostname: String,
    pub os: String,
    pub arch: String,
}

/// What a machine is, drawn as its icon (t3code's `EnvironmentMachineKind`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum MachineKind {
    Server,
    Cloud,
    Linux,
    Desktop,
    Laptop,
    MacMini,
    MacStudio,
    /// From a newer version.
    #[serde(untagged)]
    Unknown(serde_json::Value),
}

impl MachineKind {
    /// Every kind, in t3code's order.
    pub const ALL: [MachineKind; 7] = [
        MachineKind::Server,
        MachineKind::Cloud,
        MachineKind::Linux,
        MachineKind::Desktop,
        MachineKind::Laptop,
        MachineKind::MacMini,
        MachineKind::MacStudio,
    ];

    /// t3code's names for them.
    pub fn label(&self) -> &'static str {
        match self {
            MachineKind::Server | MachineKind::Unknown(_) => "Server",
            MachineKind::Cloud => "Cloud VM",
            MachineKind::Linux => "Linux/WSL",
            MachineKind::Desktop => "Desktop",
            MachineKind::Laptop => "Laptop",
            MachineKind::MacMini => "Mini PC",
            MachineKind::MacStudio => "Workstation",
        }
    }
}

/// A machine's icon: the kind its server detected, and the one chosen for it, if any.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MachineIcon {
    #[serde(default)]
    pub detected: Option<MachineKind>,
    #[serde(default)]
    pub chosen: Option<MachineKind>,
}

impl MachineIcon {
    /// The chosen kind, else the detected one, else a server (t3code's
    /// `resolveEnvironmentMachineKind`). A kind from a newer version counts as none.
    pub fn kind(&self) -> MachineKind {
        let known = |kind: &Option<MachineKind>| {
            kind.clone()
                .filter(|kind| !matches!(kind, MachineKind::Unknown(_)))
        };
        known(&self.chosen)
            .or_else(|| known(&self.detected))
            .unwrap_or(MachineKind::Server)
    }
}

/// A live agent connection on the server: a thread's, or one opened from an agent's settings
/// to log in or out.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ConnectionId {
    Thread(ThreadId),
    Account(u64),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ClientMessage {
    Request {
        id: u64,
        request: Request,
    },
    /// From a newer version.
    #[serde(untagged)]
    Unknown(serde_json::Value),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ServerMessage {
    Response {
        id: u64,
        result: std::result::Result<Response, ErrorResponse>,
    },
    Event(Event),
    /// From a newer version.
    #[serde(untagged)]
    Unknown(serde_json::Value),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ErrorResponse {
    pub message: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Request {
    /// Projects, threads, the registry and agent settings: a [`Response::Session`] snapshot,
    /// then [`Event::Projects`], [`Event::Registry`] and [`Event::AgentSettings`].
    SubscribeSession,
    /// One connection's conversation: a [`Response::Thread`] snapshot, then
    /// [`Event::Thread`]s. Subscribing to a thread starts its agent if it isn't running.
    SubscribeThread(ConnectionId),
    UnsubscribeThread(ConnectionId),

    /// The folders matching a path being typed, as t3code's `filesystem.browse`: the folder's
    /// own entries after a trailing `/`, otherwise its siblings starting with the last segment.
    /// `~` is the server's home.
    BrowseDirectories {
        partial_path: String,
    },
    AddProject {
        path: PathBuf,
    },
    SetProjectName {
        project_id: ProjectId,
        name: String,
    },
    SetProjectIcon {
        project_id: ProjectId,
        icon: Option<ProjectIcon>,
    },
    RemoveProject(ProjectId),
    SetScope(ProjectScope),
    SetThreadOrder(ThreadOrder),
    ToggleArchivedExpanded,

    /// Answered with [`Response::ThreadCreated`], once its workspace is ready.
    CreateThread {
        project_id: ProjectId,
        agent_id: AgentId,
        #[serde(default)]
        workspace: WorkspaceChoice,
    },
    /// A thread that runs a terminal instead of an agent: [`Response::ThreadCreated`].
    CreateTerminalThread {
        project_id: ProjectId,
        command: TerminalCommand,
        #[serde(default)]
        workspace: WorkspaceChoice,
    },
    /// Agent CLIs on the server's `PATH`: [`Response::TerminalPrograms`].
    TerminalPrograms,
    /// The numbers of the thread's drawer terminals running now:
    /// [`Response::DrawerTerminals`].
    DrawerTerminals(ThreadId),
    /// A terminal's screen: a full [`Response::TerminalFrame`], then [`Event::TerminalFrame`]s
    /// while it changes. A thread's terminal or drawer starts if it isn't running.
    SubscribeTerminal(TerminalKey),
    UnsubscribeTerminal(TerminalKey),
    TerminalInput {
        terminal: TerminalKey,
        input: TerminalInput,
    },
    /// The selected text: [`Response::Message`], empty without a selection.
    TerminalSelectionText(TerminalKey),
    /// Starts the terminal's command again, after it exited or to replace it.
    RestartTerminal(TerminalKey),
    /// Ends the terminal's process and forgets it.
    CloseTerminal(TerminalKey),
    /// The project's branches and whether pastures work there: [`Response::ProjectGit`].
    ProjectGit(ProjectId),
    /// Deletes a worktree or pasture from disk, keeping its branch:
    /// [`Response::WorkspaceRemoval`]. Refused while a running thread works there.
    RemoveWorkspace {
        project_id: ProjectId,
        path: PathBuf,
        /// Remove even with uncommitted changes or commits the project doesn't have.
        force: bool,
    },
    /// cow's `sync`: brings a pasture up to date with a branch of the project's checkout
    /// (the pasture's base by default), by rebase or merge. [`Response::Message`].
    SyncWorkspace {
        project_id: ProjectId,
        path: PathBuf,
        #[serde(default)]
        branch: Option<String>,
        #[serde(default)]
        merge: bool,
    },
    /// cow's `extract --branch`: makes the pasture's `HEAD` a branch of the project's checkout
    /// (the pasture's branch name by default). [`Response::Message`].
    BringBackWorkspace {
        project_id: ProjectId,
        path: PathBuf,
        #[serde(default)]
        branch: Option<String>,
    },
    /// The user's own title, which shows instead of the automatic one. An empty title shows
    /// the automatic one again.
    RenameThread {
        thread_id: ThreadId,
        title: String,
    },
    ArchiveThread(ThreadId),
    UnarchiveThread(ThreadId),
    DeleteThread(ThreadId),
    /// The thread's changes from its checkpoints: [`Response::ThreadDiff`].
    ThreadDiff {
        thread_id: ThreadId,
        scope: DiffScope,
    },
    /// Puts the thread's files back as they were before the scope's changes: before its latest
    /// turn, or before its first. Later checkpoints are dropped. Answers with the new
    /// [`Response::ThreadDiff`].
    RestoreCheckpoint {
        thread_id: ThreadId,
        scope: DiffScope,
    },

    Prompt {
        connection: ConnectionId,
        text: String,
    },
    Cancel(ConnectionId),
    RespondToPermission {
        connection: ConnectionId,
        tool_call_id: acp::ToolCallId,
        option_id: acp::PermissionOptionId,
    },
    SetConfigOption {
        connection: ConnectionId,
        config_id: acp::SessionConfigId,
        value: acp::SessionConfigOptionValue,
    },
    SetMode {
        connection: ConnectionId,
        mode_id: acp::SessionModeId,
    },
    ClearPlan(ConnectionId),
    Authenticate {
        connection: ConnectionId,
        method_id: acp::AuthMethodId,
    },
    Reauthenticate(ConnectionId),
    Logout(ConnectionId),
    RetrySession(ConnectionId),
    Reload(ConnectionId),
    /// From an account connection: checks again whether the agent is logged in.
    CheckLogin(ConnectionId),

    /// Starts an agent only to log in or out. Answered with [`Response::AccountOpened`]. It
    /// closes with [`Request::CloseAccount`], or when the client disconnects.
    OpenAccount(AgentId),
    CloseAccount(u64),

    RefreshRegistry {
        /// Only if the last refresh was over an hour ago.
        if_stale: bool,
    },
    InstallAgent(AgentId),
    UninstallAgent(AgentId),
    UpdateAgentSettings {
        agent_id: AgentId,
        change: AgentSettingsChange,
    },

    /// Ends the server, its agents and terminals.
    Shutdown,

    /// The agent-control tools, as MCP tool definitions: [`Response::Tools`].
    ListTools,
    /// Runs an agent-control tool for the caller: [`Response::ToolResult`]. Tools that wait
    /// answer when they're done.
    CallTool {
        caller: ToolCaller,
        name: String,
        arguments: serde_json::Value,
    },

    /// The app's other machines, and which of their projects it combines with this server's,
    /// so agents here can work there: their tool calls naming a machine are relayed through
    /// this client ([`Event::RelayToolCall`]). Replaces what the client sent before.
    SetPeers(Peers),
    /// The machine's icon, or `None` for the detected one. Reaches session subscribers as
    /// [`Event::MachineIcon`].
    SetMachineIcon(Option<MachineKind>),
    /// The answer to an [`Event::RelayToolCall`].
    RelayToolResult {
        relay_id: u64,
        result: ToolResult,
    },

    /// The Workspaces view's spaces, tabs and panes. Changes reach session subscribers as
    /// [`Event::Spaces`].
    Spaces(SpaceRequest),

    /// From a newer version.
    #[serde(untagged)]
    Unknown(serde_json::Value),
}

/// The machines an app reaches besides this server's, as it names them.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Peers {
    /// What the app calls this server's machine.
    pub this_machine: String,
    pub machines: Vec<PeerMachine>,
    /// For each project here that the app combines with projects on other machines, those.
    pub checkouts: Vec<PeerCheckouts>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PeerMachine {
    pub name: String,
    pub online: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PeerCheckouts {
    pub project_id: ProjectId,
    pub checkouts: Vec<PeerCheckout>,
}

/// A project on another machine, by its folder there.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PeerCheckout {
    pub machine: String,
    pub path: PathBuf,
}

/// A tool call for another machine: the client runs it there for the project at `path`
/// ([`ToolCaller::Directory`]) and answers with [`Request::RelayToolResult`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RelayToolCall {
    pub relay_id: u64,
    pub machine: String,
    pub path: PathBuf,
    pub name: String,
    pub arguments: serde_json::Value,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct DirectoryListing {
    /// The folder listed, with `~` expanded.
    pub parent: PathBuf,
    /// Sorted by name.
    pub entries: Vec<DirectoryEntry>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DirectoryEntry {
    pub name: String,
    pub path: PathBuf,
}

/// Who is calling a tool, which decides the project it may manage.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ToolCaller {
    /// The credential the MCP bridge got with its agent's session.
    Session(String),
    /// A thread, as the CLI knows it from `AGENTZ_THREAD_ID`.
    Thread(ThreadId),
    /// A directory inside a project, as the CLI run elsewhere knows it.
    Directory(PathBuf),
}

/// A tool's answer: its result, or for a failure `{"code", "message"}` with t3code's failure
/// codes. The MCP bridge wraps it as a `tools/call` result.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ToolResult {
    pub value: serde_json::Value,
    pub is_error: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum AgentSettingsChange {
    SetEnv(BTreeMap<String, String>),
    SetLoginMethod(Option<String>),
    SetDefaultConfigOption {
        config_id: String,
        value: Option<acp::SessionConfigOptionValue>,
    },
    SetDefaultMode(Option<acp::SessionModeId>),
    /// From a newer version.
    #[serde(untagged)]
    Unknown(serde_json::Value),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Response {
    Ok,
    Session(SessionSnapshot),
    Thread(ThreadView),
    ThreadCreated(ThreadId),
    ProjectAdded(ProjectId),
    AccountOpened(u64),
    Tools(serde_json::Value),
    ToolResult(ToolResult),
    ThreadDiff(ThreadDiff),
    ProjectGit(ProjectGit),
    WorkspaceRemoval(WorkspaceRemoval),
    TerminalPrograms(Vec<TerminalProgram>),
    DrawerTerminals(Vec<u32>),
    TerminalFrame(TerminalFrame),
    Directories(DirectoryListing),
    SpacePane(PaneLocation),
    /// What a finished action did, to show the user.
    Message(String),
    /// From a newer version.
    #[serde(untagged)]
    Unknown(serde_json::Value),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SessionSnapshot {
    pub projects: ProjectsSnapshot,
    pub registry: RegistrySnapshot,
    pub agent_settings: BTreeMap<AgentId, AgentSettings>,
    #[serde(default)]
    pub spaces: SpacesSnapshot,
    #[serde(default)]
    pub machine_icon: MachineIcon,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Event {
    Projects(ProjectsSnapshot),
    Registry(RegistrySnapshot),
    AgentSettings(BTreeMap<AgentId, AgentSettings>),
    Spaces(SpacesSnapshot),
    MachineIcon(MachineIcon),
    Thread {
        connection: ConnectionId,
        update: ThreadUpdate,
    },
    /// The connection is gone (its thread was deleted, or its account closed).
    ConnectionClosed(ConnectionId),
    /// What changed on a subscribed terminal's screen.
    TerminalFrame {
        terminal: TerminalKey,
        frame: TerminalFrame,
    },
    /// The terminal is gone (closed, or its thread deleted).
    TerminalClosed(TerminalKey),
    /// A tool call for one of the client's other machines, from [`Request::SetPeers`].
    RelayToolCall(RelayToolCall),
    /// From a newer version.
    #[serde(untagged)]
    Unknown(serde_json::Value),
}

/// Writes one length-prefixed JSON message.
pub async fn write_message(
    writer: &mut (impl AsyncWrite + Unpin),
    message: &impl Serialize,
) -> Result<()> {
    let body = serde_json::to_vec(message).context("encoding a message")?;
    anyhow::ensure!(
        body.len() <= MAX_FRAME_SIZE,
        "message of {} bytes is over the limit",
        body.len()
    );
    let length = u32::try_from(body.len()).context("message length")?;
    writer.write_all(&length.to_le_bytes()).await?;
    writer.write_all(&body).await?;
    writer.flush().await?;
    Ok(())
}

/// Reads one length-prefixed JSON message, or `None` if the stream ended cleanly before it.
pub async fn read_message<T: DeserializeOwned>(
    reader: &mut (impl AsyncRead + Unpin),
) -> Result<Option<T>> {
    let mut length = [0; 4];
    match reader.read_exact(&mut length).await {
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(error) => return Err(error.into()),
    }
    let length = u32::from_le_bytes(length) as usize;
    anyhow::ensure!(
        length <= MAX_FRAME_SIZE,
        "message of {length} bytes is over the limit"
    );
    let mut body = vec![0; length];
    reader
        .read_exact(&mut body)
        .await
        .context("reading a message")?;
    let message = serde_json::from_slice(&body).context("decoding a message")?;
    Ok(Some(message))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::thread::Entry;

    #[tokio::test]
    async fn frames_round_trip() {
        let (mut client, mut server) = tokio::io::duplex(1024);
        let hello = ClientHello {
            protocol_version: PROTOCOL_VERSION,
            client_version: "0.1.0".into(),
            client_kind: ClientKind::App,
        };
        let request = ClientMessage::Request {
            id: 7,
            request: Request::Prompt {
                connection: ConnectionId::Thread(ThreadId(3)),
                text: "hello".into(),
            },
        };
        write_message(&mut client, &hello).await.expect("write");
        write_message(&mut client, &request).await.expect("write");
        drop(client);

        let received: Option<ClientHello> = read_message(&mut server).await.expect("read");
        assert_eq!(received, Some(hello));
        let received: Option<ClientMessage> = read_message(&mut server).await.expect("read");
        assert_eq!(received, Some(request));
        let received: Option<ClientMessage> = read_message(&mut server).await.expect("read");
        assert_eq!(received, None);
    }

    #[tokio::test]
    async fn oversized_frames_are_refused() {
        let (mut client, mut server) = tokio::io::duplex(64);
        let length = u32::try_from(MAX_FRAME_SIZE + 1).expect("fits");
        client
            .write_all(&length.to_le_bytes())
            .await
            .expect("write");
        let received = read_message::<ClientMessage>(&mut server).await;
        assert!(received.is_err());
    }

    #[test]
    fn unknown_variants_fall_back() {
        let message: ClientMessage =
            serde_json::from_str(r#"{"Request":{"id":1,"request":{"FromTheFuture":{"x":1}}}}"#)
                .expect("decodes");
        assert!(matches!(
            message,
            ClientMessage::Request {
                id: 1,
                request: Request::Unknown(_)
            }
        ));
        let message: ServerMessage =
            serde_json::from_str(r#"{"Event":{"Novel":[1]}}"#).expect("decodes");
        assert!(matches!(message, ServerMessage::Event(Event::Unknown(_))));
        let kind: ClientKind = serde_json::from_str(r#""Telepathy""#).expect("decodes");
        assert!(matches!(kind, ClientKind::Unknown(_)));
    }

    #[test]
    fn thread_events_round_trip() {
        let message = ServerMessage::Event(Event::Thread {
            connection: ConnectionId::Account(2),
            update: ThreadUpdate {
                state: None,
                entry_count: 1,
                entries: vec![(0, Entry::AgentMessage("hi".into()))],
            },
        });
        let json = serde_json::to_string(&message).expect("encodes");
        let decoded: ServerMessage = serde_json::from_str(&json).expect("decodes");
        assert_eq!(decoded, message);
    }
}
