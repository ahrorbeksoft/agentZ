//! The app's connection to one machine's `agentz-server`: this Mac's, started if it isn't
//! running, or another machine's over SSH. It keeps the app's copies of that server's session
//! (projects, registry, agent settings) and of its open threads and terminals up to date, and
//! reconnects when the connection drops. While it's down, the copies stay as they were.

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use agentz_client::Connection;
use agentz_client::ssh::{RemotePlatform, Ssh, SshError, UploadProgress};
use agentz_protocol::accounts::{AccountId, AgentAccount, AgentAccounts};
use agentz_protocol::agents::{AgentId, AgentSettings, RegistrySnapshot};
use agentz_protocol::artifacts::{
    Artifact, ArtifactId, ArtifactRequest, Artifacts, DesignSettings, PageTheme,
};
use agentz_protocol::cloud::{CloudSnapshot, CloudState};
use agentz_protocol::layout::PaneId;
use agentz_protocol::mcp_servers::McpServer;
use agentz_protocol::skills::Skill;
use agentz_protocol::spaces::{Pane, PaneAgentState, PaneContent, SpacesSnapshot};
use agentz_protocol::storage::{CleanupRules, Storage, StorageCache};
use agentz_protocol::terminal::TerminalKey;
use agentz_protocol::title_generation::{TitleGeneration, TitleGenerationState};
use agentz_protocol::{
    AgentSettingsChange, CAPABILITY_HAND_OFF, CAPABILITY_RELAY, ClientKind, ConnectionId,
    DirectoryListing, Event, MachineIcon, MachineKind, Peers, RelayToolCall, Request, Response,
};
use anyhow::{Context as _, Result, anyhow};
use collections::HashMap;
use futures::channel::{mpsc, oneshot};
use futures::future::{BoxFuture, Fuse, LocalBoxFuture};
use futures::{FutureExt as _, StreamExt as _};
use gpui::{App, AppContext as _, AsyncApp, Context, Entity, EventEmitter, Task, WeakEntity};
use projects::ThreadId;
use ui::SharedString;
use util::ResultExt as _;

use crate::agent_icons::AgentIconStore;
use crate::app_settings::CloudLink;
use crate::machines::{MachineId, Machines};
use crate::project_store::{ProjectStore, ThreadStatus};
use crate::registry_store::AgentRegistryStore;
use crate::terminal_entity::Terminal;
use crate::thread_entity::AgentThread;

const MIN_RETRY_DELAY: Duration = Duration::from_millis(250);
const MAX_RETRY_DELAY: Duration = Duration::from_secs(5);
/// herdr's supervisor: remote machines back off from half a second to two minutes, retry every
/// 30 seconds while they need attention, and only reset the backoff after a minute connected.
const REMOTE_MIN_RETRY_DELAY: Duration = Duration::from_millis(500);
const REMOTE_MAX_RETRY_DELAY: Duration = Duration::from_secs(120);
const ATTENTION_RETRY_DELAY: Duration = Duration::from_secs(30);
const STABLE_CONNECTION_PERIOD: Duration = Duration::from_secs(60);
/// herdr's wake recovery: for a while after the Mac wakes, a machine that fails to connect is
/// tried again every couple of seconds, as Wi-Fi comes back.
const WAKE_RECOVERY_PERIOD: Duration = Duration::from_secs(30);
const WAKE_RETRY_DELAY: Duration = Duration::from_secs(2);
/// How long a connection has to answer a ping after the Mac wakes (herdr's
/// `HEARTBEAT_TIMEOUT`).
const PING_TIMEOUT: Duration = Duration::from_secs(10);
/// How often a cloud machine that isn't running is looked at again, in this Mac's server's
/// latest listing (no request is sent for it). A new listing retries it at once.
const CLOUD_RECHECK_DELAY: Duration = Duration::from_secs(30);
const SERVER_BINARY_ENV_VAR: &str = "AGENTZ_SERVER_BIN";
/// A directory of `agentz-server-<rust target>` binaries for other machines.
const REMOTE_SERVERS_ENV_VAR: &str = "AGENTZ_REMOTE_SERVERS";
const VERSION: &str = env!("CARGO_PKG_VERSION");

/// How the server is reached.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Transport {
    Local,
    /// An SSH target.
    Ssh(String),
    /// A cloud provider's machine (design/cloud): dialed only while its provider says it runs,
    /// with the SSH options its CLI gives through this Mac's server, or with `target` when it
    /// was saved over SSH before it was linked.
    Cloud {
        link: CloudLink,
        target: Option<String>,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub enum MachineStatus {
    Connecting,
    Online,
    /// Retrying after the error.
    Reconnecting(SharedString),
    /// Retrying won't help until the user does something (herdr's Attention).
    Attention {
        error: SharedString,
        hint: Option<SharedString>,
    },
    /// A cloud machine its provider says isn't running. It's dialed once it is.
    NotRunning(CloudState),
}

/// Why the wait before reconnecting was cut short.
#[derive(Debug, PartialEq)]
enum Retry {
    /// The user fixed what needed attention: the backoff starts over.
    Asked,
    /// The Mac woke: the network may need a moment, and the backoff stays.
    Woke,
}

/// What came of asking a server to update.
#[derive(Debug, PartialEq)]
pub enum ServerUpdate {
    /// The server installed now is taking over; the connection drops and comes back to it.
    Started,
    /// These threads' turns are running, and would stop.
    TurnsRunning(Vec<String>),
}

pub enum ServerClientEvent {
    /// An agent on this machine wants a tool run on another.
    RelayToolCall(RelayToolCall),
    /// A terminal pane's agent finished working (`Completed`) or got blocked on a prompt
    /// (`PendingApproval`).
    PaneNeedsAttention(PaneId, ThreadStatus),
    /// Another machine's pages server needs this machine's artifacts answered.
    RelayArtifact {
        relay_id: u64,
        machine: String,
        request: ArtifactRequest,
    },
    /// A page asks agentZ to show its thread: on this machine, or on the one named.
    ShowThread {
        machine: Option<String>,
        thread_id: ThreadId,
    },
    /// An agent published an artifact or a new version of one while the app was connected.
    ArtifactPublished(Artifact),
    /// This Mac's server found the cloud providers' machines changed (design/cloud).
    CloudChanged,
}

pub struct ServerClient {
    machine: MachineId,
    label: SharedString,
    transport: Transport,
    connection: Option<Connection>,
    status: MachineStatus,
    /// The machine's server is older than the one installed there now, and keeps running
    /// until the user restarts it.
    is_outdated: bool,
    /// The copies of the server's session.
    projects: Entity<ProjectStore>,
    registry: Entity<AgentRegistryStore>,
    agent_settings: BTreeMap<AgentId, AgentSettings>,
    /// Each agent's accounts, for those that have any of agentZ's or a read of the External one.
    accounts: BTreeMap<AgentId, AgentAccounts>,
    /// agentZ's skills on this machine.
    skills: Vec<Skill>,
    /// agentZ's MCP servers on this machine.
    mcp_servers: Vec<McpServer>,
    /// The Workspaces view's spaces on this machine.
    spaces: SpacesSnapshot,
    /// Pane agents that finished working since this window last showed them (herdr's unseen
    /// idle), as `viewed.json` does for threads.
    unseen_panes: BTreeSet<PaneId>,
    /// What kind of machine the server says it's on, and the kind chosen for it.
    machine_icon: MachineIcon,
    /// How the machine titles threads whose agent doesn't, and the CLIs it found for it.
    title_generation: TitleGenerationState,
    /// Threads whose title was asked to be regenerated, till the server answers.
    regenerating_titles: HashSet<ThreadId>,
    /// What agentZ keeps on the machine, as its server last measured it.
    storage: Storage,
    /// The machine's published artifacts, and where its pages server is.
    artifacts: Artifacts,
    /// The cloud providers its server found, and their machines: this Mac's only.
    cloud: CloudSnapshot,
    /// The ids of the editors installed on the machine
    /// ([`agentz_protocol::editors::EDITORS`]), asked for of this Mac's server only.
    editors: Vec<String>,
    /// Open threads and login sessions, which get the server's updates.
    threads: HashMap<ConnectionId, WeakEntity<AgentThread>>,
    /// Terminals a view shows, which get the server's frames.
    terminals: HashMap<TerminalKey, WeakEntity<Terminal>>,
    /// Session events that arrived while the session snapshot was on its way.
    queued_session_events: Option<Vec<Event>>,
    /// How far installing the server on the machine is, while connecting.
    upload_progress: Option<UploadProgress>,
    /// Cuts the wait before the next attempt short.
    retry_now: Option<oneshot::Sender<Retry>>,
    /// What last reached a machine over SSH, which forwards its ports.
    ssh: Option<Ssh>,
    /// Asks the connection to show it still works.
    check_connection: Option<mpsc::UnboundedSender<()>>,
    /// When the Mac last woke from sleep.
    woke_at: Option<Instant>,
    /// What the server was last told of the other machines, this connection.
    peers_sent: Option<Peers>,
    /// The design defaults the server was last given, this connection.
    design_settings_sent: Option<DesignSettings>,
    /// The days of inactivity the server was last told to archive threads after, this
    /// connection.
    auto_archive_sent: Option<Option<u32>>,
    #[cfg(test)]
    sent_for_test: std::cell::RefCell<Vec<Request>>,
    /// Answers requests as the server would, for a test client.
    #[cfg(test)]
    answer_for_test: Option<Box<dyn Fn(&Request) -> Option<Response>>>,
    _maintain_connection: Task<()>,
}

impl EventEmitter<ServerClientEvent> for ServerClient {}

impl ServerClient {
    pub fn new(
        machine: MachineId,
        label: SharedString,
        transport: Transport,
        cx: &mut App,
    ) -> Entity<Self> {
        cx.new(|cx| {
            let this = cx.weak_entity();
            let projects = cx.new(|_| ProjectStore::new(machine, this.clone()));
            let registry = cx.new(|_| AgentRegistryStore::new(this));
            let connect_transport = transport.clone();
            Self {
                machine,
                label,
                transport,
                connection: None,
                status: MachineStatus::Connecting,
                is_outdated: false,
                projects,
                registry,
                agent_settings: BTreeMap::new(),
                accounts: BTreeMap::new(),
                skills: Vec::new(),
                mcp_servers: Vec::new(),
                spaces: SpacesSnapshot::default(),
                unseen_panes: BTreeSet::new(),
                machine_icon: MachineIcon::default(),
                title_generation: TitleGenerationState::default(),
                regenerating_titles: HashSet::default(),
                storage: Storage::default(),
                artifacts: Artifacts::default(),
                cloud: CloudSnapshot::default(),
                editors: Vec::new(),
                threads: HashMap::default(),
                terminals: HashMap::default(),
                queued_session_events: None,
                upload_progress: None,
                retry_now: None,
                ssh: None,
                check_connection: None,
                woke_at: None,
                peers_sent: None,
                design_settings_sent: None,
                auto_archive_sent: None,
                #[cfg(test)]
                sent_for_test: Default::default(),
                #[cfg(test)]
                answer_for_test: None,
                _maintain_connection: cx.spawn(async move |this, cx| {
                    maintain_connection(this, connect_transport, cx).await
                }),
            }
        })
    }

    pub fn machine(&self) -> MachineId {
        self.machine
    }

    pub fn label(&self) -> &SharedString {
        &self.label
    }

    pub(crate) fn set_label(&mut self, label: SharedString, cx: &mut Context<Self>) {
        if self.label != label {
            self.label = label;
            cx.notify();
        }
    }

    /// `ssh` to the machine, for forwarding its ports: a cloud machine's is what last reached
    /// it.
    pub fn ssh(&self) -> Option<Ssh> {
        match &self.transport {
            Transport::Local => None,
            Transport::Ssh(target) => Ssh::new(target).log_err(),
            Transport::Cloud { .. } => self.ssh.clone(),
        }
    }

    pub fn transport(&self) -> &Transport {
        &self.transport
    }

    pub fn projects(&self) -> &Entity<ProjectStore> {
        &self.projects
    }

    pub fn registry(&self) -> &Entity<AgentRegistryStore> {
        &self.registry
    }

    pub fn status(&self) -> &MachineStatus {
        &self.status
    }

    pub fn is_online(&self) -> bool {
        self.status == MachineStatus::Online
    }

    /// The editors installed on this Mac, as last asked ([`Self::refresh_editors`]).
    pub fn editors(&self) -> &[String] {
        &self.editors
    }

    /// Asks this Mac's server which editors are installed, which Open in Editor lists. An
    /// editor installed later shows up the next time it's asked.
    pub fn refresh_editors(&mut self, cx: &mut Context<Self>) {
        if self.transport != Transport::Local {
            return;
        }
        let response = self.request(Request::ListEditors);
        cx.spawn(async move |this, cx| {
            let editors = match response.await {
                Ok(Response::Editors(editors)) => editors,
                Ok(response) => {
                    log::error!("expected the editors, got {response:?}");
                    return;
                }
                Err(error) => {
                    log::error!("couldn't list the editors: {error:#}");
                    return;
                }
            };
            this.update(cx, |this, cx| {
                if this.editors != editors {
                    this.editors = editors;
                    cx.notify();
                }
            })
            .log_err();
        })
        .detach();
    }

    pub fn connection(&self) -> Option<&Connection> {
        self.connection.as_ref()
    }

    /// How far installing the server on the machine is, while connecting to it.
    pub fn upload_progress(&self) -> Option<UploadProgress> {
        self.upload_progress
            .filter(|_| self.status == MachineStatus::Connecting)
    }

    pub fn is_outdated(&self) -> bool {
        self.is_outdated && self.is_online()
    }

    /// What restarting the machine's server would stop: turns in progress, programs running in
    /// front of a terminal's shell, and agent CLIs in workspace panes. Idle shells come back.
    pub fn running(&self, cx: &App) -> Vec<String> {
        let store = self.projects.read(cx);
        let mut running = Vec::new();
        for thread in store.threads() {
            if store.is_thread_working(thread.id) {
                running.push(format!("the turn in {}", thread.title));
            }
            running.extend(store.terminal_command(thread.id).map(str::to_string));
            running.extend(
                store
                    .drawer_commands(thread.id)
                    .map(|(_, command)| command.to_string()),
            );
        }
        running.extend(
            panes(&self.spaces)
                .filter_map(|pane| pane.agent.as_ref().map(|agent| agent.name.clone())),
        );
        running
    }

    /// Stops the machine's server. The connection drops, and reconnecting starts the server
    /// installed now.
    pub fn restart_server(&self, cx: &App) {
        self.send(Request::Shutdown, cx);
    }

    /// Whether the server can update without ending its terminals ([`Self::update_server`]).
    pub fn can_update_server(&self) -> bool {
        self.has_capability(CAPABILITY_HAND_OFF)
    }

    /// Has the server hand its terminals to the binary installed now and exit, as herdr's
    /// server handoff does. Agents start again in the new server and load their sessions. A
    /// turn that's running stops only with `stop_running_turns`.
    pub fn update_server(&self, stop_running_turns: bool, cx: &App) -> Task<Result<ServerUpdate>> {
        let response = self.request(Request::HandOff { stop_running_turns });
        cx.background_spawn(async move {
            match response.await? {
                Response::Ok => Ok(ServerUpdate::Started),
                Response::TurnsRunning(threads) => Ok(ServerUpdate::TurnsRunning(threads)),
                response => Err(anyhow!("unexpected response: {response:?}")),
            }
        })
    }

    /// Tries again now instead of waiting out the backoff, as after fixing what needed
    /// attention.
    pub fn retry(&mut self) {
        if let Some(retry_now) = self.retry_now.take() {
            retry_now.send(Retry::Asked).ok();
        }
    }

    /// herdr's wake recovery. An SSH connection may have died while the Mac slept, which ssh's
    /// keepalives notice only a minute later, so a connection is asked to answer now, and a
    /// machine waiting to reconnect is tried now. A socket on this Mac doesn't go stale.
    pub fn system_woke(&mut self) {
        if self.transport == Transport::Local {
            return;
        }
        self.woke_at = Some(Instant::now());
        if let Some(check_connection) = &self.check_connection {
            check_connection.unbounded_send(()).ok();
        }
        if let Some(retry_now) = self.retry_now.take() {
            retry_now.send(Retry::Woke).ok();
        }
    }

    /// Whether the connected server has the feature. An older server lacks newer ones.
    pub fn has_capability(&self, capability: &str) -> bool {
        self.connection.as_ref().is_some_and(|connection| {
            connection
                .welcome()
                .capabilities
                .iter()
                .any(|known| known == capability)
        })
    }

    /// Sends a request now; the future waits for the answer. It fails at once while
    /// disconnected.
    pub fn request(&self, request: Request) -> BoxFuture<'static, Result<Response>> {
        #[cfg(test)]
        if let Some(response) = self
            .answer_for_test
            .as_ref()
            .and_then(|answer| answer(&request))
        {
            return futures::future::ready(Ok(response)).boxed();
        }
        match &self.connection {
            Some(connection) => connection.request(request).boxed(),
            None => {
                let error = anyhow!("not connected to {}", self.label);
                futures::future::ready(Err(error)).boxed()
            }
        }
    }

    /// Sends a request whose answer only matters if it's an error.
    pub fn send(&self, request: Request, cx: &App) {
        #[cfg(test)]
        self.sent_for_test.borrow_mut().push(request.clone());
        let description = request_name(&request);
        let response = self.request(request);
        cx.background_spawn(async move {
            if let Err(error) = response.await {
                log::error!("{description} failed: {error:#}");
            }
        })
        .detach();
    }

    /// The machine's folders that complete `partial_path`, for adding a project there.
    pub fn browse_directories(
        &self,
        partial_path: String,
        cx: &App,
    ) -> Task<Result<DirectoryListing>> {
        let response = self.request(Request::BrowseDirectories { partial_path });
        cx.background_spawn(async move {
            match response.await? {
                Response::Directories(listing) => Ok(listing),
                response => Err(anyhow!("unexpected response: {response:?}")),
            }
        })
    }

    /// Tells the server about the other machines, when that's changed.
    pub(crate) fn set_peers(&mut self, peers: Peers, cx: &App) {
        if !self.has_capability(CAPABILITY_RELAY) || self.peers_sent.as_ref() == Some(&peers) {
            return;
        }
        self.send(Request::SetPeers(peers.clone()), cx);
        self.peers_sent = Some(peers);
    }

    /// Gives the server the design defaults for its agents' design tool, when they've changed.
    pub(crate) fn set_design_settings(&mut self, settings: DesignSettings, cx: &App) {
        if !self.is_online() || self.design_settings_sent.as_ref() == Some(&settings) {
            return;
        }
        self.send(Request::SetDesignDefaults(settings.clone()), cx);
        self.design_settings_sent = Some(settings);
    }

    /// Tells the server how long a thread may go unused before it archives it, when that's
    /// changed.
    pub(crate) fn set_auto_archive(&mut self, days: Option<u32>, cx: &App) {
        if !self.is_online() || self.auto_archive_sent == Some(days) {
            return;
        }
        self.send(Request::SetAutoArchive(days), cx);
        self.auto_archive_sent = Some(days);
    }

    /// The server was told of this checkout, so its agents may work there. Without one, the
    /// call reaches only what needs no project there, which that machine's server enforces.
    pub(crate) fn may_relay_to(&self, machine: &str, path: Option<&std::path::Path>) -> bool {
        self.peers_sent.as_ref().is_some_and(|peers| match path {
            Some(path) => peers
                .checkouts
                .iter()
                .flat_map(|checkouts| &checkouts.checkouts)
                .any(|checkout| checkout.machine == machine && checkout.path == path),
            None => peers.machines.iter().any(|known| known.name == machine),
        })
    }

    pub fn agent_settings(&self, agent_id: &str) -> AgentSettings {
        self.agent_settings
            .get(&AgentId::new(agent_id.to_string()))
            .cloned()
            .unwrap_or_default()
    }

    /// The account's settings, `None` being the External account, whose are the agent's.
    pub fn account_settings(
        &self,
        agent_id: &AgentId,
        account: Option<AccountId>,
    ) -> AgentSettings {
        match account {
            None => self.agent_settings(&agent_id.0),
            Some(id) => self
                .accounts
                .get(agent_id)
                .and_then(|accounts| accounts.account(id))
                .map(|account| account.settings.clone())
                .unwrap_or_default(),
        }
    }

    /// Asks the server to change the account's settings (`None` being the External account's).
    /// The copy here follows when the server says so. The options and modes an agent offers
    /// are the server's to remember, so changes to them are ignored.
    pub fn update_account_settings(
        &mut self,
        agent_id: &AgentId,
        account: Option<AccountId>,
        change: impl FnOnce(&mut AgentSettings),
        cx: &mut Context<Self>,
    ) {
        let previous = self.account_settings(agent_id, account);
        let mut settings = previous.clone();
        change(&mut settings);
        for change in agent_settings_changes(&previous, &settings) {
            self.send(
                Request::UpdateAgentSettings {
                    agent_id: agent_id.clone(),
                    account,
                    change,
                },
                cx,
            );
        }
    }

    fn set_agent_settings(
        &mut self,
        agents: BTreeMap<AgentId, AgentSettings>,
        cx: &mut Context<Self>,
    ) {
        if agents != self.agent_settings {
            self.agent_settings = agents;
            cx.notify();
        }
    }

    /// The agent's accounts. An agent with none of agentZ's has only the External one.
    pub fn accounts(&self, agent_id: &AgentId) -> AgentAccounts {
        self.accounts.get(agent_id).cloned().unwrap_or_default()
    }

    /// [`AgentAccounts::thread_color`], as `#rrggbb`.
    pub fn thread_color(&self, agent_id: &AgentId, account: Option<AccountId>) -> Option<&str> {
        self.accounts.get(agent_id)?.thread_color(account)
    }

    fn set_accounts(&mut self, accounts: BTreeMap<AgentId, AgentAccounts>, cx: &mut Context<Self>) {
        if accounts != self.accounts {
            self.accounts = accounts;
            cx.notify();
        }
    }

    pub fn skills(&self) -> &[Skill] {
        &self.skills
    }

    fn set_skills(&mut self, skills: Vec<Skill>, cx: &mut Context<Self>) {
        if skills != self.skills {
            self.skills = skills;
            cx.notify();
        }
    }

    #[cfg(test)]
    pub fn set_skills_for_test(&mut self, skills: Vec<Skill>, cx: &mut Context<Self>) {
        self.set_skills(skills, cx);
    }

    pub fn mcp_servers(&self) -> &[McpServer] {
        &self.mcp_servers
    }

    fn set_mcp_servers(&mut self, mcp_servers: Vec<McpServer>, cx: &mut Context<Self>) {
        if mcp_servers != self.mcp_servers {
            self.mcp_servers = mcp_servers;
            cx.notify();
        }
    }

    /// An accounts menu's change, shown before the server confirms it, so the open menu checks
    /// what was clicked.
    pub fn show_skill_kept_off(
        &mut self,
        name: &str,
        kept_off: Vec<AgentAccount>,
        cx: &mut Context<Self>,
    ) {
        if let Some(skill) = self.skills.iter_mut().find(|skill| skill.name == name) {
            skill.kept_off = kept_off;
            cx.notify();
        }
    }

    pub fn show_mcp_server_kept_off(
        &mut self,
        name: &str,
        kept_off: Vec<AgentAccount>,
        cx: &mut Context<Self>,
    ) {
        if let Some(server) = self
            .mcp_servers
            .iter_mut()
            .find(|server| server.name == name)
        {
            server.kept_off = kept_off;
            cx.notify();
        }
    }

    #[cfg(test)]
    pub fn set_mcp_servers_for_test(
        &mut self,
        mcp_servers: Vec<McpServer>,
        cx: &mut Context<Self>,
    ) {
        self.set_mcp_servers(mcp_servers, cx);
    }

    #[cfg(test)]
    pub fn set_agent_settings_for_test(
        &mut self,
        agents: BTreeMap<AgentId, AgentSettings>,
        cx: &mut Context<Self>,
    ) {
        self.set_agent_settings(agents, cx);
    }

    #[cfg(test)]
    pub fn set_accounts_for_test(
        &mut self,
        accounts: BTreeMap<AgentId, AgentAccounts>,
        cx: &mut Context<Self>,
    ) {
        self.set_accounts(accounts, cx);
    }

    pub fn machine_icon(&self) -> &MachineIcon {
        &self.machine_icon
    }

    fn set_machine_icon_state(&mut self, icon: MachineIcon, cx: &mut Context<Self>) {
        if icon != self.machine_icon {
            self.machine_icon = icon;
            cx.notify();
        }
    }

    pub fn title_generation(&self) -> &TitleGenerationState {
        &self.title_generation
    }

    /// Titles the thread again from its conversation (`Request::RegenerateTitle`), as
    /// t3code's Regenerate title does; [`Self::is_regenerating_title`] till it's done.
    pub fn regenerate_title(&mut self, thread_id: ThreadId, cx: &mut Context<Self>) {
        if !self.regenerating_titles.insert(thread_id) {
            return;
        }
        cx.notify();
        let response = self.request(Request::RegenerateTitle(thread_id));
        cx.spawn(async move |this, cx| {
            response.await.log_err();
            this.update(cx, |this, cx| {
                this.regenerating_titles.remove(&thread_id);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub fn is_regenerating_title(&self, thread_id: ThreadId) -> bool {
        self.regenerating_titles.contains(&thread_id)
    }

    /// Whether the machine titles threads, so it can title one again.
    pub fn can_regenerate_titles(&self) -> bool {
        self.title_generation.settings.enabled
    }

    /// Changes how the machine titles threads. Shown at once; the server's answer, with the
    /// providers it found again, follows.
    pub fn choose_title_generation(&mut self, settings: TitleGeneration, cx: &mut Context<Self>) {
        self.send(Request::SetTitleGeneration(settings.clone()), cx);
        self.title_generation.settings = settings;
        cx.notify();
    }

    fn set_title_generation(&mut self, state: TitleGenerationState, cx: &mut Context<Self>) {
        if state != self.title_generation {
            self.title_generation = state;
            cx.notify();
        }
    }

    #[cfg(test)]
    pub fn set_title_generation_for_test(
        &mut self,
        state: TitleGenerationState,
        cx: &mut Context<Self>,
    ) {
        self.set_title_generation(state, cx);
    }

    pub fn storage(&self) -> &Storage {
        &self.storage
    }

    pub fn artifacts(&self) -> &Artifacts {
        &self.artifacts
    }

    /// The cloud providers this Mac's server found, and their machines (design/cloud).
    pub fn cloud(&self) -> &CloudSnapshot {
        &self.cloud
    }

    fn set_cloud(&mut self, cloud: CloudSnapshot, cx: &mut Context<Self>) {
        if cloud != self.cloud {
            self.cloud = cloud;
            cx.emit(ServerClientEvent::CloudChanged);
            cx.notify();
        }
    }

    fn set_artifacts(&mut self, artifacts: Artifacts, cx: &mut Context<Self>) {
        if artifacts != self.artifacts {
            self.artifacts = artifacts;
            cx.notify();
        }
    }

    #[cfg(test)]
    pub fn set_artifacts_for_test(&mut self, artifacts: Artifacts, cx: &mut Context<Self>) {
        self.set_artifacts(artifacts, cx);
    }

    /// Deletes an artifact and its versions everywhere it shows.
    pub fn delete_artifact(&self, id: ArtifactId, cx: &App) -> Task<Result<()>> {
        let response = self.request(Request::DeleteArtifact(id));
        cx.background_spawn(async move { response.await.map(|_| ()) })
    }

    /// The app's look for this machine's pages server, sent as it changes.
    pub fn set_page_theme(&self, theme: PageTheme, cx: &App) {
        self.send(Request::SetPageTheme(theme), cx);
    }

    fn set_storage(&mut self, storage: Storage, cx: &mut Context<Self>) {
        if storage != self.storage {
            self.storage = storage;
            cx.notify();
        }
    }

    #[cfg(test)]
    pub fn set_storage_for_test(&mut self, storage: Storage, cx: &mut Context<Self>) {
        self.set_storage(storage, cx);
    }

    /// Settings › Storage's Delete or Clear for what isn't a thread or a checkout.
    pub fn clear_storage(&self, cache: StorageCache, cx: &App) -> Task<Result<()>> {
        let response = self.request(Request::ClearStorage(cache));
        cx.background_spawn(async move { response.await.map(|_| ()) })
    }

    /// Settings › Storage's cleanup rules, shown at once as the server will have them.
    pub fn set_cleanup_rules(&mut self, rules: CleanupRules, cx: &mut Context<Self>) {
        self.storage.cleanup.rules = rules.clone();
        cx.notify();
        self.send(Request::SetCleanupRules(rules), cx);
    }

    /// Runs the cleanup rules now. Resolves once the run ends.
    pub fn clean_up_now(&self, cx: &App) -> Task<Result<()>> {
        let response = self.request(Request::CleanUpNow);
        cx.background_spawn(async move { response.await.map(|_| ()) })
    }

    /// Chooses the machine's icon, for every client of its server. Choosing what was
    /// detected clears the choice, as t3code's picker does, so detection keeps deciding.
    pub fn choose_machine_icon(&self, kind: MachineKind, cx: &App) {
        let detected = self
            .machine_icon
            .detected
            .clone()
            .unwrap_or(MachineKind::Server);
        let choice = (kind != detected).then_some(kind);
        self.send(Request::SetMachineIcon(choice), cx);
    }

    pub fn spaces(&self) -> &SpacesSnapshot {
        &self.spaces
    }

    fn set_spaces(&mut self, spaces: SpacesSnapshot, cx: &mut Context<Self>) {
        if spaces == self.spaces {
            return;
        }
        let previous: BTreeMap<PaneId, Option<PaneAgentState>> = panes(&self.spaces)
            .map(|pane| (pane.id, pane_agent_state(pane)))
            .collect();
        let mut live = BTreeSet::new();
        for pane in panes(&spaces) {
            live.insert(pane.id);
            // Only a pane seen before can change, so the first spaces after connecting are
            // no news.
            let Some(&was) = previous.get(&pane.id) else {
                continue;
            };
            let state = pane_agent_state(pane);
            match state {
                Some(PaneAgentState::Idle) if was == Some(PaneAgentState::Working) => {
                    self.unseen_panes.insert(pane.id);
                    cx.emit(ServerClientEvent::PaneNeedsAttention(
                        pane.id,
                        ThreadStatus::Completed,
                    ));
                }
                Some(PaneAgentState::Idle) => {}
                _ => {
                    self.unseen_panes.remove(&pane.id);
                }
            }
            // herdr's request sound: every time it becomes blocked.
            if state == Some(PaneAgentState::Blocked) && was != state {
                cx.emit(ServerClientEvent::PaneNeedsAttention(
                    pane.id,
                    ThreadStatus::PendingApproval,
                ));
            }
        }
        self.unseen_panes.retain(|pane| live.contains(pane));
        self.spaces = spaces;
        cx.notify();
    }

    /// A terminal pane's agent's state, as a thread's: blocked, working, or finished and not yet
    /// shown.
    pub fn pane_agent_status(&self, pane: &Pane) -> Option<ThreadStatus> {
        match pane_agent_state(pane)? {
            PaneAgentState::Blocked => Some(ThreadStatus::PendingApproval),
            PaneAgentState::Working => Some(ThreadStatus::Working),
            PaneAgentState::Idle => self
                .unseen_panes
                .contains(&pane.id)
                .then_some(ThreadStatus::Completed),
            PaneAgentState::Unknown => None,
        }
    }

    /// A view showed the pane, so its agent's finish has been seen.
    pub fn mark_pane_seen(&mut self, pane: PaneId, cx: &mut Context<Self>) {
        if self.unseen_panes.remove(&pane) {
            cx.notify();
        }
    }

    /// The thread's copy, if a view still holds it.
    pub(crate) fn thread(&self, connection: ConnectionId) -> Option<Entity<AgentThread>> {
        self.threads.get(&connection)?.upgrade()
    }

    pub(crate) fn register_thread(
        &mut self,
        connection: ConnectionId,
        thread: WeakEntity<AgentThread>,
    ) {
        self.threads.retain(|_, thread| thread.upgrade().is_some());
        self.threads.insert(connection, thread);
    }

    /// The terminal's copy, if a view still holds it.
    pub(crate) fn terminal(&self, key: &TerminalKey) -> Option<Entity<Terminal>> {
        self.terminals.get(key)?.upgrade()
    }

    pub(crate) fn register_terminal(&mut self, key: TerminalKey, terminal: WeakEntity<Terminal>) {
        self.terminals
            .retain(|_, terminal| terminal.upgrade().is_some());
        self.terminals.insert(key, terminal);
    }

    /// A client that never connects, holding the given spaces, for the views' tests.
    #[cfg(test)]
    pub fn new_for_test(
        machine: MachineId,
        label: SharedString,
        spaces: SpacesSnapshot,
        cx: &mut App,
    ) -> Entity<Self> {
        let client = Self::new(machine, label, Transport::Local, cx);
        client.update(cx, |client, _| {
            client._maintain_connection = Task::ready(());
            client.spaces = spaces;
        });
        client
    }

    /// New spaces, as the server would send them.
    #[cfg(test)]
    pub fn set_spaces_for_test(&mut self, spaces: SpacesSnapshot, cx: &mut Context<Self>) {
        self.set_spaces(spaces, cx);
    }

    /// What this Mac's server would say of the cloud providers.
    #[cfg(test)]
    pub fn set_cloud_for_test(&mut self, cloud: CloudSnapshot, cx: &mut Context<Self>) {
        self.set_cloud(cloud, cx);
    }

    /// How the machine is reached, for a test that needs it to be over SSH.
    #[cfg(test)]
    pub fn set_transport_for_test(&mut self, transport: Transport) {
        self.transport = transport;
    }

    /// The editors this Mac's server would say are installed.
    #[cfg(test)]
    pub fn set_editors_for_test(&mut self, editors: &[&str], cx: &mut Context<Self>) {
        self.editors = editors.iter().map(|editor| editor.to_string()).collect();
        cx.notify();
    }

    /// What `send` was given, oldest first.
    #[cfg(test)]
    pub fn sent_for_test(&self) -> Vec<Request> {
        self.sent_for_test.borrow().clone()
    }

    /// Answers the requests `answer` knows, as the server would; the rest fail as unconnected.
    #[cfg(test)]
    pub fn answer_for_test(&mut self, answer: impl Fn(&Request) -> Option<Response> + 'static) {
        self.answer_for_test = Some(Box::new(answer));
    }

    /// Reads as connected, though a test client has no connection.
    #[cfg(test)]
    pub fn set_online_for_test(&mut self, cx: &mut Context<Self>) {
        self.status = MachineStatus::Online;
        cx.notify();
    }

    fn connected(&mut self, connection: Connection, is_outdated: bool, cx: &mut Context<Self>) {
        log::info!(
            "connected to agentz-server {} on {} (pid {})",
            connection.welcome().server_version,
            self.label,
            connection.welcome().pid
        );
        self.connection = Some(connection);
        self.peers_sent = None;
        self.design_settings_sent = None;
        self.auto_archive_sent = None;
        self.is_outdated = is_outdated;
        self.status = MachineStatus::Online;
        self.upload_progress = None;
        self.queued_session_events = Some(Vec::new());
        self.refresh_editors(cx);
        let threads: Vec<_> = self
            .threads
            .values()
            .filter_map(|thread| thread.upgrade())
            .collect();
        let terminals: Vec<_> = self
            .terminals
            .values()
            .filter_map(|terminal| terminal.upgrade())
            .collect();
        // Deferred: the threads read this client, which is being updated.
        cx.defer(move |cx| {
            for thread in threads {
                thread.update(cx, |thread, cx| thread.reconnected(cx));
            }
            for terminal in terminals {
                terminal.update(cx, |terminal, cx| terminal.reconnected(cx));
            }
        });
        cx.notify();
    }

    fn disconnected(&mut self, status: MachineStatus, cx: &mut Context<Self>) {
        // A cloud machine that isn't running is looked at again and again, unchanged.
        let changed =
            self.connection.is_some() || self.status != status || self.upload_progress.is_some();
        self.connection = None;
        self.check_connection = None;
        self.status = status;
        self.upload_progress = None;
        self.queued_session_events = None;
        if changed {
            cx.notify();
        }
    }

    fn apply_session(&mut self, response: Result<Response>, cx: &mut Context<Self>) {
        let session = match response {
            Ok(Response::Session(session)) => session,
            Ok(response) => {
                log::error!("expected a session snapshot, got {response:?}");
                return;
            }
            Err(error) => {
                log::error!("failed to subscribe to the session: {error:#}");
                return;
            }
        };
        self.projects
            .update(cx, |store, cx| store.set_snapshot(session.projects, cx));
        self.set_registry(session.registry, cx);
        self.set_agent_settings(session.agent_settings, cx);
        self.set_accounts(session.accounts, cx);
        self.set_skills(session.skills, cx);
        self.set_mcp_servers(session.mcp_servers, cx);
        self.set_spaces(session.spaces, cx);
        self.set_machine_icon_state(session.machine_icon, cx);
        self.set_title_generation(session.title_generation, cx);
        self.set_storage(session.storage, cx);
        self.set_artifacts(session.artifacts, cx);
        self.set_cloud(session.cloud, cx);
        for event in self.queued_session_events.take().unwrap_or_default() {
            self.handle_event(event, cx);
        }
        // The providers' CLIs are installed here, so only this Mac's server runs them, and it
        // watches them once an app asks.
        if self.machine == MachineId::Local {
            self.send(Request::CheckCloud, cx);
        }
    }

    fn set_registry(&mut self, registry: RegistrySnapshot, cx: &mut Context<Self>) {
        AgentIconStore::global(cx).update(cx, |icons, cx| {
            icons.learn(&registry, |request| self.request(request), cx)
        });
        self.registry
            .update(cx, |store, cx| store.set_snapshot(registry, cx));
    }

    fn handle_event(&mut self, event: Event, cx: &mut Context<Self>) {
        if let Some(queued) = &mut self.queued_session_events
            && matches!(
                event,
                Event::Projects(_)
                    | Event::Registry(_)
                    | Event::AgentSettings(_)
                    | Event::Accounts(_)
                    | Event::Skills(_)
                    | Event::McpServers(_)
                    | Event::Spaces(_)
                    | Event::MachineIcon(_)
                    | Event::TitleGeneration(_)
                    | Event::Storage(_)
                    | Event::Artifacts(_)
                    | Event::Cloud(_)
            )
        {
            queued.push(event);
            return;
        }
        match event {
            Event::Projects(projects) => self
                .projects
                .update(cx, |store, cx| store.set_snapshot(projects, cx)),
            Event::Registry(registry) => self.set_registry(registry, cx),
            Event::AgentSettings(agent_settings) => self.set_agent_settings(agent_settings, cx),
            Event::Accounts(accounts) => self.set_accounts(accounts, cx),
            Event::Skills(skills) => self.set_skills(skills, cx),
            Event::McpServers(mcp_servers) => self.set_mcp_servers(mcp_servers, cx),
            Event::Spaces(spaces) => self.set_spaces(spaces, cx),
            Event::MachineIcon(icon) => self.set_machine_icon_state(icon, cx),
            Event::TitleGeneration(state) => self.set_title_generation(state, cx),
            Event::Storage(storage) => self.set_storage(storage, cx),
            Event::Cloud(cloud) => self.set_cloud(cloud, cx),
            Event::Artifacts(artifacts) => {
                // Only a change is a publish: the session's list is what was there before.
                let published: Vec<Artifact> = artifacts
                    .artifacts
                    .iter()
                    .filter(|artifact| {
                        let before = self
                            .artifacts
                            .artifacts
                            .iter()
                            .find(|known| known.id == artifact.id)
                            .map_or(0, Artifact::latest);
                        artifact.latest() > before
                    })
                    .cloned()
                    .collect();
                self.set_artifacts(artifacts, cx);
                for artifact in published {
                    cx.emit(ServerClientEvent::ArtifactPublished(artifact));
                }
            }
            Event::RelayArtifact {
                relay_id,
                machine,
                request,
            } => cx.emit(ServerClientEvent::RelayArtifact {
                relay_id,
                machine,
                request,
            }),
            Event::ShowThread { machine, thread_id } => {
                cx.emit(ServerClientEvent::ShowThread { machine, thread_id })
            }
            Event::Thread { connection, update } => {
                if let Some(thread) = self.threads.get(&connection).and_then(|t| t.upgrade()) {
                    thread.update(cx, |thread, cx| thread.apply_update(update, cx));
                }
            }
            Event::ConnectionClosed(connection) => {
                if let Some(thread) = self.threads.remove(&connection).and_then(|t| t.upgrade()) {
                    thread.update(cx, |thread, cx| thread.closed(cx));
                }
            }
            Event::TerminalFrame { terminal, frame } => {
                if let Some(terminal) = self.terminal(&terminal) {
                    terminal.update(cx, |terminal, cx| terminal.apply_frame(frame, cx));
                }
            }
            Event::TerminalClosed(key) => {
                if let Some(terminal) = self.terminal(&key) {
                    terminal.update(cx, |terminal, cx| terminal.closed(cx));
                }
            }
            Event::RelayToolCall(call) => cx.emit(ServerClientEvent::RelayToolCall(call)),
            Event::Unknown(event) => log::warn!("unknown event from the server: {event}"),
        }
    }
}

async fn maintain_connection(
    this: WeakEntity<ServerClient>,
    transport: Transport,
    cx: &mut AsyncApp,
) {
    let runtime = reqwest_client::runtime().handle().clone();
    let (min_delay, max_delay) = match transport {
        Transport::Local => (MIN_RETRY_DELAY, MAX_RETRY_DELAY),
        Transport::Ssh(_) | Transport::Cloud { .. } => {
            (REMOTE_MIN_RETRY_DELAY, REMOTE_MAX_RETRY_DELAY)
        }
    };
    let mut delay = min_delay;
    loop {
        // Shown while installing the server on the machine takes a while.
        let (upload_progress, mut upload_updates) = mpsc::unbounded();
        let shows_progress = this.clone();
        cx.spawn(async move |cx| {
            while let Some(update) = upload_updates.next().await {
                let shown = shows_progress.update(cx, |this, cx| {
                    this.upload_progress = Some(update);
                    cx.notify();
                });
                if shown.is_err() {
                    break;
                }
            }
        })
        .detach();
        let connected = match prepare_connection(&this, &transport, cx).await {
            Ok(ssh) => {
                let shown = this.update(cx, |this, cx| {
                    if this.status != MachineStatus::Connecting {
                        this.status = MachineStatus::Connecting;
                        cx.notify();
                    }
                });
                if shown.is_err() {
                    return;
                }
                connect(&runtime, ssh, upload_progress)
                    .await
                    .map_err(|error| {
                        if error.needs_attention {
                            MachineStatus::Attention {
                                error: error.message.into(),
                                hint: error.hint.map(SharedString::from),
                            }
                        } else {
                            MachineStatus::Reconnecting(error.message.into())
                        }
                    })
            }
            Err(status) => Err(status),
        };
        let status = match connected {
            Ok((connection, mut events, is_outdated)) => {
                let connected_at = Instant::now();
                let session = connection.request(Request::SubscribeSession);
                let (check_connection, mut check_requests) = mpsc::unbounded();
                if this
                    .update(cx, |this, cx| {
                        this.check_connection = Some(check_connection);
                        this.connected(connection.clone(), is_outdated, cx)
                    })
                    .is_err()
                {
                    return;
                }
                let session_applied = this.clone();
                cx.spawn(async move |cx| {
                    let response = session.await;
                    session_applied
                        .update(cx, |this, cx| this.apply_session(response, cx))
                        .ok();
                })
                .detach();
                let mut ping: Fuse<LocalBoxFuture<'static, bool>> = Fuse::terminated();
                let status = loop {
                    // The ping's answer arrives through the events, so they're taken meanwhile.
                    futures::select_biased! {
                        event = events.next().fuse() => {
                            let Some(event) = event else {
                                break "the connection to agentz-server closed";
                            };
                            if this
                                .update(cx, |this, cx| this.handle_event(event, cx))
                                .is_err()
                            {
                                return;
                            }
                        }
                        _ = check_requests.next() => {
                            let answer = connection.request(Request::Ping);
                            let timeout = cx.background_executor().timer(PING_TIMEOUT);
                            ping = async move {
                                futures::select_biased! {
                                    answer = answer.fuse() => answer.is_ok(),
                                    _ = timeout.fuse() => false,
                                }
                            }
                            .boxed_local()
                            .fuse();
                        }
                        answered = ping => {
                            if !answered {
                                connection.close();
                                break "agentz-server stopped answering";
                            }
                        }
                    }
                };
                // Brief connections don't reset the backoff, so a server that keeps dropping
                // isn't hammered.
                if connected_at.elapsed() >= STABLE_CONNECTION_PERIOD
                    || transport == Transport::Local
                {
                    delay = min_delay;
                }
                MachineStatus::Reconnecting(status.into())
            }
            Err(status) => status,
        };
        let (retry_now, retry_requested) = oneshot::channel();
        let Ok(woke_recently) = this.update(cx, |this, cx| {
            this.retry_now = Some(retry_now);
            this.disconnected(status.clone(), cx);
            this.woke_at
                .is_some_and(|woke_at| woke_at.elapsed() < WAKE_RECOVERY_PERIOD)
        }) else {
            return;
        };
        let wait = match &status {
            MachineStatus::Attention { .. } => ATTENTION_RETRY_DELAY,
            MachineStatus::NotRunning(_) => CLOUD_RECHECK_DELAY,
            _ if woke_recently => delay.min(WAKE_RETRY_DELAY),
            _ => delay,
        };
        if !matches!(status, MachineStatus::NotRunning(_)) {
            log::warn!("{status:?}; retrying in {wait:?}");
        }
        futures::select_biased! {
            retry = retry_requested.fuse() => {
                if retry != Ok(Retry::Woke) {
                    delay = min_delay;
                }
            }
            _ = cx.background_executor().timer(wait).fuse() => {
                delay = (delay * 2).min(max_delay);
            }
        }
        // A machine that isn't running stays so until it's dialed.
        if !matches!(status, MachineStatus::NotRunning(_))
            && this
                .update(cx, |this, cx| {
                    this.status = MachineStatus::Connecting;
                    cx.notify();
                })
                .is_err()
        {
            return;
        }
    }
}

/// What reaches the machine: nothing for this Mac, `ssh` for another. A cloud machine is
/// dialed only while its provider says it runs.
async fn prepare_connection(
    this: &WeakEntity<ServerClient>,
    transport: &Transport,
    cx: &mut AsyncApp,
) -> Result<Option<Ssh>, MachineStatus> {
    let ssh = match transport {
        Transport::Local => return Ok(None),
        Transport::Ssh(target) => Ssh::new(target).map_err(|error| MachineStatus::Attention {
            error: format!("{error:#}").into(),
            hint: None,
        })?,
        Transport::Cloud { link, target } => cloud_ssh(link, target.as_deref(), cx).await?,
    };
    this.update(cx, |this, _| this.ssh = Some(ssh.clone())).ok();
    Ok(Some(ssh))
}

/// `ssh` to a provider's machine once this Mac's server lists it running: with its saved
/// target, or with the options its CLI gives (Instabox renews its certificate for each).
async fn cloud_ssh(
    link: &CloudLink,
    target: Option<&str>,
    cx: &mut AsyncApp,
) -> Result<Ssh, MachineStatus> {
    let description = link.provider.description();
    let (machine, listed, options) = cx.update(|cx| {
        let local = Machines::local(cx);
        let local = local.read(cx);
        let state = local.cloud().provider(link.provider);
        let machine = state.and_then(|state| {
            state
                .machines
                .iter()
                .find(|machine| machine.id == link.machine)
                .cloned()
        });
        let listed = state.is_some_and(|state| state.listed_at.is_some());
        let options = machine
            .as_ref()
            .filter(|machine| machine.state.is_running() && target.is_none())
            .map(|_| {
                local.request(Request::CloudSshOptions {
                    provider: link.provider,
                    machine: link.machine.clone(),
                })
            });
        (machine, listed, options)
    });
    let machine = match machine {
        Some(machine) => machine,
        None if listed => {
            return Err(MachineStatus::Attention {
                error: format!("It isn't in your {} account anymore.", description.name).into(),
                hint: None,
            });
        }
        None => {
            return Err(MachineStatus::Reconnecting(
                format!("waiting for {} to list its machines", description.name).into(),
            ));
        }
    };
    if !machine.state.is_running() {
        return Err(MachineStatus::NotRunning(machine.state));
    }
    if let Some(target) = target {
        return Ssh::new(target).map_err(|error| MachineStatus::Attention {
            error: format!("{error:#}").into(),
            hint: None,
        });
    }
    let options = match options {
        Some(options) => match options.await {
            Ok(Response::SshOptions(options)) => options,
            Ok(response) => {
                return Err(MachineStatus::Reconnecting(
                    format!("expected SSH options, got {response:?}").into(),
                ));
            }
            Err(error) => {
                return Err(MachineStatus::Reconnecting(
                    format!(
                        "{} couldn't give its SSH options: {error:#}",
                        description.name
                    )
                    .into(),
                ));
            }
        },
        None => return Err(MachineStatus::NotRunning(machine.state)),
    };
    let name = format!("{}-{}", description.program, machine.name);
    let terminal_command = format!("{} ssh {}", description.program, machine.name);
    Ssh::with_options(&name, &options, &terminal_command).map_err(|error| {
        MachineStatus::Attention {
            error: format!("{error:#}").into(),
            hint: None,
        }
    })
}

/// The session, and whether the server is older than the one installed.
async fn connect(
    runtime: &tokio::runtime::Handle,
    ssh: Option<Ssh>,
    upload_progress: mpsc::UnboundedSender<UploadProgress>,
) -> Result<(Connection, agentz_client::Events, bool), SshError> {
    match ssh {
        None => connect_local(runtime)
            .await
            .map(|(connection, events)| {
                let is_outdated = local_server_is_outdated(&connection);
                (connection, events, is_outdated)
            })
            .map_err(|error| SshError {
                message: format!("{error:#}"),
                needs_attention: false,
                hint: None,
            }),
        Some(ssh) => runtime
            .spawn(async move {
                agentz_client::ssh::connect(
                    &ssh,
                    VERSION,
                    remote_server_binary,
                    ClientKind::App,
                    move |update| {
                        upload_progress.unbounded_send(update).ok();
                    },
                )
                .await
            })
            .await
            .map_err(|error| SshError {
                message: format!("the connection task ended: {error}"),
                needs_attention: false,
                hint: None,
            })?
            .map(|connected| {
                (
                    connected.connection,
                    connected.events,
                    connected.is_outdated,
                )
            }),
    }
}

/// Whether the binary next to the app changed since the server started from it, as after
/// `cargo build` or installing a new version.
fn local_server_is_outdated(connection: &Connection) -> bool {
    let Some(started_from) = connection.welcome().binary_modified else {
        return false;
    };
    let installed = server_binary()
        .ok()
        .and_then(|path| std::fs::metadata(path).ok())
        .and_then(|metadata| metadata.modified().ok())
        .and_then(|modified| modified.duration_since(std::time::UNIX_EPOCH).ok())
        .and_then(|modified| u64::try_from(modified.as_millis()).ok());
    installed.is_some_and(|installed| installed != started_from)
}

/// Connects to this Mac's server, starting it first if nothing is listening.
async fn connect_local(
    runtime: &tokio::runtime::Handle,
) -> Result<(Connection, agentz_client::Events)> {
    let socket = paths::server_socket();
    let version = VERSION.to_string();
    match agentz_client::connect_local(runtime, &socket, ClientKind::App, version.clone()).await {
        Ok(connected) => return Ok(connected),
        Err(error) => log::info!("starting agentz-server ({error:#})"),
    }
    agentz_client::start_local_server(runtime, &server_binary()?).await?;
    agentz_client::connect_local(runtime, &socket, ClientKind::App, version).await
}

/// `agentz-server` next to the app's executable, as `cargo build` and the app bundle place it.
pub(crate) fn server_binary() -> Result<PathBuf> {
    if let Some(path) = std::env::var_os(SERVER_BINARY_ENV_VAR) {
        return Ok(PathBuf::from(path));
    }
    let path = executable_directory()?.join("agentz-server");
    anyhow::ensure!(
        path.exists(),
        "{} is missing; build it with `cargo build`",
        path.display()
    );
    Ok(path)
}

fn executable_directory() -> Result<PathBuf> {
    let executable = std::env::current_exe().context("finding the app's executable")?;
    Ok(executable
        .parent()
        .context("finding the app's directory")?
        .to_path_buf())
}

/// The server to install on a machine of this platform: this Mac's own when it matches, or
/// `agentz-server-<rust target>` from `AGENTZ_REMOTE_SERVERS`, next to the app, in the bundle's
/// resources, or where `tooling/build-remote-servers.sh` puts it.
fn remote_server_binary(platform: RemotePlatform) -> Result<PathBuf> {
    if Some(platform) == RemotePlatform::current() {
        return server_binary();
    }
    let executable_directory = executable_directory()?;
    let mut directories = Vec::new();
    if let Some(directory) = std::env::var_os(REMOTE_SERVERS_ENV_VAR) {
        directories.push(PathBuf::from(directory));
    }
    directories.push(executable_directory.clone());
    directories.push(executable_directory.join("../Resources"));
    directories.push(executable_directory.join("../remote-servers"));
    directories
        .iter()
        .map(|directory| agentz_client::ssh::bundled_server_binary(directory, platform))
        .find(|path| path.is_file())
        .ok_or_else(|| {
            anyhow!(
                "no agentz-server for {platform} was built with this app; build it with \
                 `tooling/build-remote-servers.sh {}`",
                platform.rust_target()
            )
        })
}

fn agent_settings_changes(
    previous: &AgentSettings,
    settings: &AgentSettings,
) -> Vec<AgentSettingsChange> {
    let mut changes = Vec::new();
    if settings.env != previous.env {
        changes.push(AgentSettingsChange::SetEnv(settings.env.clone()));
    }
    if settings.login_method != previous.login_method {
        changes.push(AgentSettingsChange::SetLoginMethod(
            settings.login_method.clone(),
        ));
    }
    if settings.default_mode != previous.default_mode {
        changes.push(AgentSettingsChange::SetDefaultMode(
            settings.default_mode.clone(),
        ));
    }
    let config_ids: BTreeSet<&String> = previous
        .default_config_options
        .keys()
        .chain(settings.default_config_options.keys())
        .collect();
    for config_id in config_ids {
        let value = settings.default_config_options.get(config_id);
        if value != previous.default_config_options.get(config_id) {
            changes.push(AgentSettingsChange::SetDefaultConfigOption {
                config_id: config_id.clone(),
                value: value.cloned(),
            });
        }
    }
    changes
}

/// The request's variant, for logs.
fn request_name(request: &Request) -> String {
    let debug = format!("{request:?}");
    debug
        .split(|character: char| !character.is_alphanumeric())
        .next()
        .unwrap_or_default()
        .to_string()
}

fn panes(spaces: &SpacesSnapshot) -> impl Iterator<Item = &Pane> {
    spaces
        .spaces
        .iter()
        .flat_map(|space| &space.tabs)
        .flat_map(|tab| &tab.panes)
}

/// The state of the agent CLI a terminal pane runs, if any.
fn pane_agent_state(pane: &Pane) -> Option<PaneAgentState> {
    match &pane.content {
        PaneContent::Terminal(_) => Some(pane.agent.as_ref()?.state),
        PaneContent::Thread(_) | PaneContent::Unknown(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use agentz_protocol::layout::Node;
    use agentz_protocol::spaces::{PaneAgent, PaneTerminal, Space, SpaceId, Tab, TabId};
    use gpui::TestAppContext;

    use super::*;

    fn spaces(state: Option<PaneAgentState>) -> SpacesSnapshot {
        SpacesSnapshot {
            spaces: vec![Space {
                id: SpaceId(1),
                name: None,
                folder: PathBuf::from("/tmp/demo"),
                project_id: None,
                tabs: vec![Tab {
                    id: TabId(2),
                    name: None,
                    root: Node::Pane(PaneId(3)),
                    panes: vec![Pane {
                        id: PaneId(3),
                        content: PaneContent::Terminal(PaneTerminal {
                            folder: PathBuf::from("/tmp/demo"),
                            command: None,
                        }),
                        agent: state.map(|state| PaneAgent {
                            registry_agent: None,
                            name: "Codex".to_string(),
                            state,
                        }),
                        folder: None,
                        program: None,
                        command_line: None,
                    }],
                }],
                git: None,
                current: None,
            }],
        }
    }

    /// herdr's wake recovery: a machine over SSH that waits to reconnect is tried now, keeping
    /// its backoff, and a connected one is asked to answer. This Mac's own socket is left alone.
    #[gpui::test]
    fn waking_retries_and_checks_machines_over_ssh(cx: &mut TestAppContext) {
        let client = cx.update(|cx| {
            crate::init_for_test(cx);
            ServerClient::new_for_test(MachineId::Local, "devbox".into(), spaces(None), cx)
        });
        let (retry_now, mut retry_requested) = oneshot::channel();
        let (check_connection, mut check_requests) = mpsc::unbounded();
        client.update(cx, |client, _| {
            client.retry_now = Some(retry_now);
            client.check_connection = Some(check_connection);
            client.system_woke();
            assert!(client.retry_now.is_some());
            assert!(client.woke_at.is_none());

            client.set_transport_for_test(Transport::Ssh("devbox".into()));
            client.system_woke();
            assert!(client.woke_at.is_some());
        });
        assert_eq!(retry_requested.try_recv(), Ok(Some(Retry::Woke)));
        assert_eq!(check_requests.try_recv(), Ok(()));
    }

    #[gpui::test]
    fn a_pane_agent_that_finishes_is_unseen_until_shown(cx: &mut TestAppContext) {
        let client = cx.update(|cx| {
            crate::init_for_test(cx);
            ServerClient::new_for_test(MachineId::Local, "This Mac".into(), spaces(None), cx)
        });
        let status = |cx: &mut TestAppContext| {
            client.read_with(cx, |client, _| {
                let (_, _, pane) = client.spaces().pane(PaneId(3)).expect("the pane");
                client.pane_agent_status(pane)
            })
        };
        let set = |state, cx: &mut TestAppContext| {
            client.update(cx, |client, cx| client.set_spaces(spaces(Some(state)), cx))
        };

        // Idle from the start isn't a finish.
        set(PaneAgentState::Idle, cx);
        assert_eq!(status(cx), None);
        set(PaneAgentState::Working, cx);
        assert_eq!(status(cx), Some(ThreadStatus::Working));
        set(PaneAgentState::Blocked, cx);
        assert_eq!(status(cx), Some(ThreadStatus::PendingApproval));
        set(PaneAgentState::Working, cx);
        set(PaneAgentState::Idle, cx);
        assert_eq!(status(cx), Some(ThreadStatus::Completed));

        client.update(cx, |client, cx| client.mark_pane_seen(PaneId(3), cx));
        assert_eq!(status(cx), None);
    }

    #[gpui::test]
    fn a_pane_agent_asks_for_attention_as_it_finishes_or_gets_blocked(cx: &mut TestAppContext) {
        let client = cx.update(|cx| {
            crate::init_for_test(cx);
            ServerClient::new_for_test(MachineId::Local, "This Mac".into(), spaces(None), cx)
        });
        let attention = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
        cx.update(|cx| {
            let attention = attention.clone();
            cx.subscribe(&client, move |_, event: &ServerClientEvent, _| {
                if let ServerClientEvent::PaneNeedsAttention(pane, status) = event {
                    attention.borrow_mut().push((*pane, *status));
                }
            })
            .detach();
        });
        let set = |state, cx: &mut TestAppContext| {
            client.update(cx, |client, cx| client.set_spaces(spaces(Some(state)), cx))
        };

        set(PaneAgentState::Idle, cx);
        set(PaneAgentState::Working, cx);
        set(PaneAgentState::Blocked, cx);
        set(PaneAgentState::Working, cx);
        set(PaneAgentState::Idle, cx);
        // An agent at its prompt from the start finished nothing.
        set(PaneAgentState::Unknown, cx);
        set(PaneAgentState::Idle, cx);
        assert_eq!(
            *attention.borrow(),
            vec![
                (PaneId(3), ThreadStatus::PendingApproval),
                (PaneId(3), ThreadStatus::Completed),
            ]
        );

        // A pane the client hasn't seen before is no news, blocked or not: so are the spaces
        // that arrive on connecting.
        attention.borrow_mut().clear();
        client.update(cx, |client, cx| {
            client.set_spaces(SpacesSnapshot::default(), cx);
            client.set_spaces(spaces(Some(PaneAgentState::Blocked)), cx);
        });
        assert!(attention.borrow().is_empty());
    }

    #[gpui::test]
    fn only_a_publish_while_connected_is_news(cx: &mut TestAppContext) {
        use agentz_protocol::artifacts::{ArtifactKind, ArtifactVersion};

        let artifact = |id: &str, versions: usize| Artifact {
            id: ArtifactId(id.to_string()),
            title: id.to_string(),
            kind: ArtifactKind::Document,
            versions: vec![
                ArtifactVersion {
                    published_at: 1,
                    bytes: 10,
                    files: Vec::new(),
                };
                versions
            ],
            thread_id: Some(ThreadId(1)),
            project_id: None,
            agent_id: None,
            preview: String::new(),
            design: None,
            frame_key: "key".into(),
        };
        let artifacts = |list: Vec<Artifact>| Artifacts {
            artifacts: list,
            pages: None,
        };
        let client = cx.update(|cx| {
            crate::init_for_test(cx);
            ServerClient::new_for_test(
                MachineId::Local,
                "This Mac".into(),
                SpacesSnapshot::default(),
                cx,
            )
        });
        let published = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
        cx.update(|cx| {
            let published = published.clone();
            cx.subscribe(&client, move |_, event: &ServerClientEvent, _| {
                if let ServerClientEvent::ArtifactPublished(artifact) = event {
                    published
                        .borrow_mut()
                        .push((artifact.id.0.clone(), artifact.latest()));
                }
            })
            .detach();
        });

        // What the session brings was published before.
        client.update(cx, |client, cx| {
            client.set_artifacts_for_test(artifacts(vec![artifact("plan", 1)]), cx)
        });
        let change = |list: Vec<Artifact>, cx: &mut TestAppContext| {
            client.update(cx, |client, cx| {
                client.handle_event(Event::Artifacts(artifacts(list)), cx)
            })
        };
        change(vec![artifact("plan", 1), artifact("report", 1)], cx);
        change(vec![artifact("plan", 2), artifact("report", 1)], cx);
        // A deletion is no publish.
        change(vec![artifact("plan", 2)], cx);
        assert_eq!(
            *published.borrow(),
            vec![("report".to_string(), 1), ("plan".to_string(), 2)]
        );
    }
}
