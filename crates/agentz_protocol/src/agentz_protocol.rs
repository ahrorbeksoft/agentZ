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

pub mod accounts;
pub mod agents;
pub mod attachments;
pub mod diff;
pub mod layout;
pub mod mcp_servers;
pub mod skills;
pub mod spaces;
pub mod terminal;
pub mod terminal_keys;
pub mod thread;
pub mod workspace;

use std::collections::BTreeMap;
use std::path::PathBuf;

use agent_client_protocol::schema::v1 as acp;
use anyhow::{Context as _, Result};
use projects::{
    ProjectIcon, ProjectId, ProjectScope, ProjectsSnapshot, ThreadId, ThreadOrder, ThreadSection,
    UnsentMention, WorkspaceKind,
};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncRead, AsyncReadExt as _, AsyncWrite, AsyncWriteExt as _};

use crate::accounts::{
    AccountChange, AccountChoice, AccountId, AgentAccount, AgentAccounts, SettingsSource,
};
use crate::agents::{
    AgentIcon, AgentId, AgentSession, AgentSessions, AgentSettings, CustomAgentChange, IconId,
    RegistrySnapshot,
};
use crate::attachments::{AttachmentData, AttachmentId};
use crate::diff::{DiffScope, ThreadDiff};
use crate::mcp_servers::McpServer;
use crate::skills::{Skill, SkillFile};
use crate::spaces::{PaneLocation, SpaceRequest, SpacesSnapshot};
use crate::terminal::{
    TerminalCommand, TerminalFrame, TerminalInput, TerminalKey, TerminalMatches,
};
use crate::thread::{ThreadUpdate, ThreadView};
use crate::workspace::{ProjectGit, RepositoryCheckouts, WorkspaceChoice, WorkspaceRemoval};

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
/// [`ServerWelcome::capabilities`]: the server can hand its terminals to the binary installed
/// now and exit ([`Request::HandOff`]), so updating it keeps them running.
pub const CAPABILITY_HAND_OFF: &str = "hand_off";
/// [`ServerWelcome::capabilities`]: the server lists agents' sessions and imports them as
/// threads ([`Request::ListAgentSessions`], [`Request::ImportAgentSessions`]).
pub const CAPABILITY_IMPORT_SESSIONS: &str = "import_sessions";

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
    /// When the binary the server runs was last modified, in milliseconds since the Unix epoch,
    /// read as it started. A client next to a rebuilt binary can tell the server is older.
    #[serde(default)]
    pub binary_modified: Option<u64>,
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
    LoginSession(u64),
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
    /// Projects, threads, the registry, agent settings, accounts, skills and MCP servers: a
    /// [`Response::Session`] snapshot, then [`Event::Projects`], [`Event::Registry`],
    /// [`Event::AgentSettings`], [`Event::Accounts`], [`Event::Skills`] and
    /// [`Event::McpServers`].
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
    ToggleWorkspacesExpanded,

    /// Answered with [`Response::ThreadCreated`], once its workspace is ready.
    CreateThread {
        project_id: ProjectId,
        agent_id: AgentId,
        #[serde(default)]
        workspace: WorkspaceChoice,
        #[serde(default)]
        account: AccountChoice,
    },
    /// A thread started in a workspace pane ([`ProjectId::WORKSPACES`]), from `folder`, any
    /// folder. [`WorkspaceChoice::Checkout`] works in `folder` itself; a new worktree or pasture
    /// is made from the repository `folder` is in, and an existing one is any folder.
    /// [`Response::ThreadCreated`], once that's made.
    CreateWorkspacesThread {
        folder: PathBuf,
        agent_id: AgentId,
        #[serde(default)]
        workspace: WorkspaceChoice,
        #[serde(default)]
        account: AccountChoice,
    },
    /// Makes a Workspaces thread one of the project its folder is in, adding the folder as a
    /// project when it's in none (the user agreed first).
    MoveToAgents(ThreadId),
    /// A thread that runs a terminal instead of an agent: [`Response::ThreadCreated`].
    CreateTerminalThread {
        project_id: ProjectId,
        command: TerminalCommand,
        #[serde(default)]
        workspace: WorkspaceChoice,
    },
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
    /// Where `query` appears, as plain text, in the screen and the history:
    /// [`Response::TerminalMatches`]. Lowercase queries ignore case, as alacritty's do.
    FindInTerminal {
        terminal: TerminalKey,
        query: String,
    },
    /// Starts the terminal's command again, after it exited or to replace it.
    RestartTerminal(TerminalKey),
    /// Ends the terminal's process and forgets it.
    CloseTerminal(TerminalKey),
    /// The project's branches and whether pastures work there: [`Response::ProjectGit`].
    ProjectGit(ProjectId),
    /// The repository a folder is in, and its checkouts: [`Response::RepositoryCheckouts`].
    /// Any repository, a project or not.
    RepositoryCheckouts(PathBuf),
    /// A worktree or pasture of the repository `folder` is in, on a new branch, with no thread
    /// in it yet: [`Response::WorkspaceCreated`] with its folder. A project's is recorded as one
    /// of its workspaces.
    CreateWorkspace {
        folder: PathBuf,
        kind: WorkspaceKind,
        /// What the branch starts from: what `folder` has checked out by default.
        #[serde(default)]
        base: Option<String>,
        /// `agentz/<short id>` by default.
        #[serde(default)]
        branch: Option<String>,
    },
    /// Deletes a project's worktree or pasture, or any linked worktree, from disk, keeping its
    /// branch: [`Response::WorkspaceRemoval`]. Refused while a running thread works there.
    RemoveWorkspace {
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
    /// [`projects::ProjectStore::pin_thread`]: at `order_key` among the pinned threads, or
    /// after the arranged ones.
    PinThread {
        thread_id: ThreadId,
        #[serde(default)]
        order_key: Option<String>,
    },
    UnpinThread(ThreadId),
    /// The order keys a drag writes on this machine ([`projects::order_key::plan_reorder`]),
    /// all or none.
    ReorderThreads {
        section: ThreadSection,
        keys: Vec<(ThreadId, String)>,
    },
    DeleteThread(ThreadId),
    /// What's typed in the thread's composer and not sent ([`projects::Thread::unsent_text`]),
    /// and the mentions in it.
    SetUnsentText {
        thread_id: ThreadId,
        text: Option<String>,
        mentions: Vec<UnsentMention>,
    },
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
        prompt: Vec<PromptPart>,
    },
    Cancel(ConnectionId),
    /// Adds a message to the thread's queue ([`thread::ThreadState::queued_messages`]), which
    /// the server keeps and sends one at a time whenever the agent is free, as Zed's queue does.
    QueueMessage {
        connection: ConnectionId,
        prompt: Vec<PromptPart>,
    },
    /// Takes a message out of the queue, unsent: to delete it, or to edit it.
    RemoveQueuedMessage {
        connection: ConnectionId,
        id: u64,
    },
    /// Zed's Steer: an agent that takes messages into its turn gets this one at once; for any
    /// other, it goes first and the turn ends once the agent's current step is done
    /// ([`thread::ThreadState::steering_queued`]). Steering the steering message again stops it.
    SteerQueuedMessage {
        connection: ConnectionId,
        id: u64,
    },
    /// Puts the message first and sends it as soon as possible, stopping the agent's turn.
    SendQueuedMessageNow {
        connection: ConnectionId,
        id: u64,
    },
    ClearQueue(ConnectionId),
    /// Keeps an image for the thread's messages: [`Response::Attachment`] with its id.
    AddAttachment {
        thread_id: ThreadId,
        mime_type: String,
        /// Its bytes, in base64.
        data: String,
    },
    /// An image the thread's messages link to, or a thumbnail of it that fits a hover
    /// preview: [`Response::AttachmentData`].
    Attachment {
        thread_id: ThreadId,
        id: AttachmentId,
        thumbnail: bool,
    },
    /// Keeps a file from the client's machine on the server's, for a thread whose agent can't
    /// reach the client's files (one on another machine): [`Response::UploadedFile`] with
    /// where it's kept.
    UploadFile {
        thread_id: ThreadId,
        name: String,
        /// Its bytes, in base64.
        data: String,
    },
    /// The files and folders of the folder a thread works in, for its composer's @-mentions:
    /// [`Response::Files`].
    ListFiles(ThreadId),
    /// The icon file the server found in a project's folder
    /// ([`projects::ProjectsSnapshot::favicons`]), for a client on another machine, which can't
    /// read it: [`Response::ProjectFavicon`].
    ProjectFavicon(ProjectId),
    RespondToPermission {
        connection: ConnectionId,
        tool_call_id: acp::ToolCallId,
        option_id: acp::PermissionOptionId,
    },
    /// Stops work the agent left running ([`thread::ThreadState::background_tasks`]).
    StopBackgroundTask {
        connection: ConnectionId,
        task_id: String,
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
        /// What the method takes from the user, such as an API key or a gateway, in the shape
        /// the agent reads from `authenticate`'s `_meta`.
        #[serde(default)]
        meta: Option<acp::Meta>,
    },
    /// Gives up on the `authenticate` in flight, restarting the agent: browser logins don't
    /// return until the user finishes.
    CancelAuthentication(ConnectionId),
    RespondToElicitation {
        connection: ConnectionId,
        elicitation: u64,
        action: acp::ElicitationAction,
    },
    /// Hides an opened URL elicitation the agent never said was done. It was answered when
    /// opened, so the agent hears nothing.
    DismissElicitation {
        connection: ConnectionId,
        elicitation: u64,
    },
    /// Runs one of the agent's terminal login methods in [`TerminalKey::Login`] on the server's
    /// machine, where the agent keeps its login. The agent restarts once the login exits
    /// successfully.
    TerminalLogin {
        connection: ConnectionId,
        method_id: acp::AuthMethodId,
    },
    /// Starts a thread with another agent, in the same workspace, that continues this one: its
    /// conversation goes with the new thread's first message ([`thread::handoff`]).
    /// [`Response::ThreadCreated`].
    ContinueThread {
        thread_id: ThreadId,
        agent_id: AgentId,
        #[serde(default)]
        account: AccountChoice,
    },
    /// Starts the continued thread without the conversation it would have brought.
    DropHandoff(ConnectionId),
    /// From the agent's own process, through agentZ's `xdg-open`: it tried to open a page in a
    /// browser. While it logs in, clients show it ([`thread::ThreadState::login_page`]); refused
    /// when the connection isn't logging in, or the page opens on the agent's machine. Dropped
    /// when its session opens with nobody asking it to log in, or it's a login session's.
    OpenLoginPage {
        connection: ConnectionId,
        url: String,
    },
    Reauthenticate(ConnectionId),
    Logout(ConnectionId),
    RetrySession(ConnectionId),
    Reload(ConnectionId),
    /// From a login session: checks again whether the agent is logged in.
    CheckLogin(ConnectionId),

    /// Starts an agent only to log in or out. Answered with [`Response::LoginSessionOpened`]. It
    /// closes with [`Request::CloseLoginSession`], or when the client disconnects.
    OpenLoginSession {
        agent_id: AgentId,
        /// The account it logs in or out, `None` being the External one.
        account: Option<AccountId>,
    },
    CloseLoginSession(u64),

    RefreshRegistry {
        /// Only if the last refresh was over an hour ago.
        if_stale: bool,
    },
    InstallAgent(AgentId),
    UninstallAgent(AgentId),
    /// Adds a custom agent, run from a command rather than the registry, or changes one. The
    /// server starts it once, only to initialize it, to check that it runs and to learn what
    /// it calls itself (ACP's `agentInfo`): [`Response::CustomAgentSaved`].
    SaveCustomAgent(CustomAgentChange),
    RemoveCustomAgent(AgentId),
    /// The registry's icons by id, answered with [`Response::AgentIcons`], leaving out those
    /// this machine doesn't have.
    AgentIcons(Vec<IconId>),
    UpdateAgentSettings {
        agent_id: AgentId,
        /// The account whose settings change, `None` being the External one: the agent's.
        #[serde(default)]
        account: Option<AccountId>,
        change: AgentSettingsChange,
    },
    /// A new agentZ account for the agent, logged out until it logs in:
    /// [`Response::AccountAdded`]. Its settings are copied from the account for new threads
    /// ([`accounts::AgentAccounts::default_settings_source`]).
    AddAccount(AgentId),
    /// Copy settings from: puts another account's Environment, defaults and the agent's own
    /// settings files in place of the account's (none with [`SettingsSource::Nothing`]), but
    /// never its login. Defaults it doesn't offer are dropped once a session lists what it
    /// does.
    CopyAccountSettings {
        agent_id: AgentId,
        account: AccountId,
        from: SettingsSource,
    },
    /// Deletes an agentZ account and its folder: its login, sessions and history. Its threads
    /// stay, but can't continue.
    RemoveAccount {
        agent_id: AgentId,
        account: AccountId,
    },
    UpdateAccount {
        agent_id: AgentId,
        /// `None` is the External account.
        account: Option<AccountId>,
        change: AccountChange,
    },
    /// The limit notice's Continue at <reset>: the thread, stopped by a limit its account's
    /// last read has used up, gets "Continue." when it resets ([`projects::Thread::continues_at`]).
    /// `on: false` cancels it.
    ContinueAtReset {
        thread_id: ThreadId,
        on: bool,
    },
    /// Refresh Usage: reads the account's identity and limits now. The read arrives with
    /// [`Event::Accounts`].
    RefreshUsage {
        agent_id: AgentId,
        /// `None` is the External account.
        account: Option<AccountId>,
    },
    /// Droid's "Switch to Droid Core" for the account ([`accounts::Overage`]): once its limits
    /// run out, Droid goes on with Droid Core models. Saved as Droid's own `/limits` saves it,
    /// and answered once the account's read that follows says so.
    SwitchToDroidCore {
        agent_id: AgentId,
        /// `None` is the External account.
        account: Option<AccountId>,
    },
    /// Uses one of the account's limit resets ([`accounts::LimitResets`]), which clears its
    /// limits now and can't be given back. Answered once the agent says it's used, with the
    /// account read again.
    UseLimitReset {
        agent_id: AgentId,
        /// `None` is the External account.
        account: Option<AccountId>,
    },
    /// The conversations the agent keeps on this machine for one account (`None` being the
    /// External one), to import as threads, as Zed's thread import lists them:
    /// [`Response::AgentSessions`]. The agent starts only for this.
    ListAgentSessions {
        agent_id: AgentId,
        account: Option<AccountId>,
    },
    /// Add from Folder…: a copy of a skill's folder, its `SKILL.md` at the top, becomes one of
    /// agentZ's skills, named as its `SKILL.md` says. Every account then loads it.
    AddSkill(Vec<SkillFile>),
    /// Create a Skill: one of agentZ's skills with a new `SKILL.md`, as Zed's form writes it.
    CreateSkill {
        name: String,
        description: String,
        body: String,
    },
    /// Deletes one of agentZ's skills, by its name, and its links from every account.
    DeleteSkill(String),
    /// Adds one of agentZ's MCP servers, or with `replacing`, changes the one of that name.
    /// Sessions opened from then on get it.
    SaveMcpServer {
        replacing: Option<String>,
        server: McpServer,
    },
    /// Deletes one of agentZ's MCP servers, by its name.
    DeleteMcpServer(String),
    /// Its switch: an MCP server that's off stays listed, and goes to no session.
    SetMcpServerEnabled {
        name: String,
        enabled: bool,
    },
    /// Its accounts menu: the accounts whose sessions don't get an MCP server.
    SetMcpServerKeptOff {
        name: String,
        kept_off: Vec<AgentAccount>,
    },
    /// Its accounts menu: the accounts a skill isn't linked into.
    SetSkillKeptOff {
        name: String,
        kept_off: Vec<AgentAccount>,
    },

    /// Adds a thread on the account for each of the agent's sessions from
    /// [`Request::ListAgentSessions`], in the project its folder belongs to. The agent loads
    /// the session when its thread opens. Sessions that have a thread already, or no project,
    /// are left out: [`Response::ThreadsImported`].
    ImportAgentSessions {
        agent_id: AgentId,
        account: Option<AccountId>,
        sessions: Vec<AgentSession>,
        /// Straight into Archived.
        #[serde(default)]
        archived: bool,
    },

    /// Ends the server, its agents and terminals.
    Shutdown,
    /// Starts the server binary installed now and hands it the terminals, which keep running
    /// with their screens, then ends this server and its agents. Agents start again in the new
    /// server and load their sessions. While turns run it answers
    /// [`Response::TurnsRunning`] instead, unless `stop_running_turns`.
    HandOff {
        #[serde(default)]
        stop_running_turns: bool,
    },

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

/// A part of a message to an agent, in the order typed: its text, and what the user mentioned
/// in it (Zed's mentions).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PromptPart {
    Text(String),
    /// A file or folder on the thread's machine.
    Path(PathBuf),
    /// Another thread on the thread's machine, which goes along as its conversation.
    Thread(ThreadId),
    /// A pasted image, kept for the thread ([`Request::AddAttachment`]).
    Image(AttachmentId),
}

impl PromptPart {
    /// A message of only text.
    pub fn text(text: impl Into<String>) -> Vec<PromptPart> {
        vec![PromptPart::Text(text.into())]
    }
}

/// What @ can mention in a thread's folder.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct FileListing {
    pub root: PathBuf,
    /// Relative to `root`, with `/` between names, as git lists them. Gitignored files are
    /// left out, and the list stops at [`FileListing::LIMIT`].
    pub entries: Vec<FileEntry>,
}

impl FileListing {
    pub const LIMIT: usize = 50_000;
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FileEntry {
    pub path: String,
    pub is_dir: bool,
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
    LoginSessionOpened(u64),
    Tools(serde_json::Value),
    ToolResult(ToolResult),
    ThreadDiff(ThreadDiff),
    ProjectGit(ProjectGit),
    WorkspaceRemoval(WorkspaceRemoval),
    WorkspaceCreated(PathBuf),
    RepositoryCheckouts(RepositoryCheckouts),
    DrawerTerminals(Vec<u32>),
    TerminalFrame(TerminalFrame),
    TerminalMatches(TerminalMatches),
    Directories(DirectoryListing),
    Files(FileListing),
    SpacePane(PaneLocation),
    AgentIcons(Vec<AgentIcon>),
    AgentSessions(AgentSessions),
    ThreadsImported(Vec<ThreadId>),
    CustomAgentSaved(AgentId),
    AccountAdded(AccountId),
    Attachment(AttachmentId),
    AttachmentData(AttachmentData),
    /// A project's icon file, in base64.
    ProjectFavicon(String),
    /// Where the server keeps a file sent with [`Request::UploadFile`].
    UploadedFile(PathBuf),
    /// The titles of the threads whose turns are running.
    TurnsRunning(Vec<String>),
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
    pub accounts: BTreeMap<AgentId, AgentAccounts>,
    /// agentZ's skills on the machine, by name.
    #[serde(default)]
    pub skills: Vec<Skill>,
    /// agentZ's MCP servers on the machine, in the order they were added.
    #[serde(default)]
    pub mcp_servers: Vec<McpServer>,
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
    Accounts(BTreeMap<AgentId, AgentAccounts>),
    Skills(Vec<Skill>),
    McpServers(Vec<McpServer>),
    Spaces(SpacesSnapshot),
    MachineIcon(MachineIcon),
    Thread {
        connection: ConnectionId,
        update: ThreadUpdate,
    },
    /// The connection is gone (its thread was deleted, or its login session closed).
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
                prompt: PromptPart::text("hello"),
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
            connection: ConnectionId::LoginSession(2),
            update: ThreadUpdate {
                state: None,
                entry_count: 1,
                entries: vec![(0, Entry::AgentMessage("hi".into()))],
                appended: Vec::new(),
            },
        });
        let json = serde_json::to_string(&message).expect("encodes");
        let decoded: ServerMessage = serde_json::from_str(&json).expect("decodes");
        assert_eq!(decoded, message);
    }
}
