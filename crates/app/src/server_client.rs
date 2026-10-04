//! The app's connection to one machine's `agentz-server`: this Mac's, started if it isn't
//! running, or another machine's over SSH. It keeps the app's copies of that server's session
//! (projects, registry, agent settings) and of its open threads and terminals up to date, and
//! reconnects when the connection drops. While it's down, the copies stay as they were.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use agentz_client::Connection;
use agentz_client::ssh::{RemotePlatform, Ssh, SshError, UploadProgress};
use agentz_protocol::agents::{AgentId, AgentSettings, RegistrySnapshot};
use agentz_protocol::layout::PaneId;
use agentz_protocol::spaces::{Pane, PaneAgentState, PaneContent, SpacesSnapshot};
use agentz_protocol::terminal::TerminalKey;
use agentz_protocol::{
    AgentSettingsChange, CAPABILITY_HAND_OFF, CAPABILITY_RELAY, ClientKind, ConnectionId,
    DirectoryListing, Event, MachineIcon, MachineKind, Peers, RelayToolCall, Request, Response,
};
use anyhow::{Context as _, Result, anyhow};
use collections::HashMap;
use futures::channel::{mpsc, oneshot};
use futures::future::BoxFuture;
use futures::{FutureExt as _, StreamExt as _};
use gpui::{App, AppContext as _, AsyncApp, Context, Entity, EventEmitter, Task, WeakEntity};
use ui::SharedString;

use crate::agent_icons::AgentIconStore;
use crate::machines::MachineId;
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
    /// The Workspaces view's spaces on this machine.
    spaces: SpacesSnapshot,
    /// Pane agents that finished working since this window last showed them (herdr's unseen
    /// idle), as `viewed.json` does for threads.
    unseen_panes: BTreeSet<PaneId>,
    /// What kind of machine the server says it's on, and the kind chosen for it.
    machine_icon: MachineIcon,
    /// Open threads and account connections, which get the server's updates.
    threads: HashMap<ConnectionId, WeakEntity<AgentThread>>,
    /// Terminals a view shows, which get the server's frames.
    terminals: HashMap<TerminalKey, WeakEntity<Terminal>>,
    /// Session events that arrived while the session snapshot was on its way.
    queued_session_events: Option<Vec<Event>>,
    /// How far installing the server on the machine is, while connecting.
    upload_progress: Option<UploadProgress>,
    /// Cuts the wait before the next attempt short.
    retry_now: Option<oneshot::Sender<()>>,
    /// What the server was last told of the other machines, this connection.
    peers_sent: Option<Peers>,
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
                spaces: SpacesSnapshot::default(),
                unseen_panes: BTreeSet::new(),
                machine_icon: MachineIcon::default(),
                threads: HashMap::default(),
                terminals: HashMap::default(),
                queued_session_events: None,
                upload_progress: None,
                retry_now: None,
                peers_sent: None,
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
            retry_now.send(()).ok();
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

    /// The server was told of this checkout, so its agents may work there.
    pub(crate) fn may_relay_to(&self, machine: &str, path: &std::path::Path) -> bool {
        self.peers_sent.as_ref().is_some_and(|peers| {
            peers
                .checkouts
                .iter()
                .flat_map(|checkouts| &checkouts.checkouts)
                .any(|checkout| checkout.machine == machine && checkout.path == path)
        })
    }

    pub fn agent_settings(&self, agent_id: &str) -> AgentSettings {
        self.agent_settings
            .get(&AgentId::new(agent_id.to_string()))
            .cloned()
            .unwrap_or_default()
    }

    /// Asks the server to make the change. The copy here follows when the server says so. The
    /// options and modes an agent offers are the server's to remember, so changes to them are
    /// ignored.
    pub fn update_agent_settings(
        &mut self,
        agent_id: &str,
        change: impl FnOnce(&mut AgentSettings),
        cx: &mut Context<Self>,
    ) {
        let previous = self.agent_settings(agent_id);
        let mut settings = previous.clone();
        change(&mut settings);
        for change in agent_settings_changes(&previous, &settings) {
            self.send(
                Request::UpdateAgentSettings {
                    agent_id: AgentId::new(agent_id.to_string()),
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

    pub fn machine_icon(&self) -> &MachineIcon {
        &self.machine_icon
    }

    fn set_machine_icon_state(&mut self, icon: MachineIcon, cx: &mut Context<Self>) {
        if icon != self.machine_icon {
            self.machine_icon = icon;
            cx.notify();
        }
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
        let was_working: BTreeSet<PaneId> = panes(&self.spaces)
            .filter(|pane| pane_agent_state(pane) == Some(PaneAgentState::Working))
            .map(|pane| pane.id)
            .collect();
        let mut live = BTreeSet::new();
        for pane in panes(&spaces) {
            live.insert(pane.id);
            match pane_agent_state(pane) {
                Some(PaneAgentState::Idle) if was_working.contains(&pane.id) => {
                    self.unseen_panes.insert(pane.id);
                }
                Some(PaneAgentState::Idle) => {}
                _ => {
                    self.unseen_panes.remove(&pane.id);
                }
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
        self.is_outdated = is_outdated;
        self.status = MachineStatus::Online;
        self.upload_progress = None;
        self.queued_session_events = Some(Vec::new());
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
        self.connection = None;
        self.status = status;
        self.upload_progress = None;
        self.queued_session_events = None;
        cx.notify();
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
        self.set_spaces(session.spaces, cx);
        self.set_machine_icon_state(session.machine_icon, cx);
        for event in self.queued_session_events.take().unwrap_or_default() {
            self.handle_event(event, cx);
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
                    | Event::Spaces(_)
                    | Event::MachineIcon(_)
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
            Event::Spaces(spaces) => self.set_spaces(spaces, cx),
            Event::MachineIcon(icon) => self.set_machine_icon_state(icon, cx),
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
        Transport::Ssh(_) => (REMOTE_MIN_RETRY_DELAY, REMOTE_MAX_RETRY_DELAY),
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
        let status = match connect(&runtime, &transport, upload_progress).await {
            Ok((connection, mut events, is_outdated)) => {
                let connected_at = Instant::now();
                let session = connection.request(Request::SubscribeSession);
                if this
                    .update(cx, |this, cx| this.connected(connection, is_outdated, cx))
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
                while let Some(event) = events.next().await {
                    if this
                        .update(cx, |this, cx| this.handle_event(event, cx))
                        .is_err()
                    {
                        return;
                    }
                }
                // Brief connections don't reset the backoff, so a server that keeps dropping
                // isn't hammered.
                if connected_at.elapsed() >= STABLE_CONNECTION_PERIOD
                    || transport == Transport::Local
                {
                    delay = min_delay;
                }
                MachineStatus::Reconnecting("the connection to agentz-server closed".into())
            }
            Err(error) => {
                if error.needs_attention {
                    MachineStatus::Attention {
                        error: error.message.into(),
                        hint: error.hint.map(SharedString::from),
                    }
                } else {
                    MachineStatus::Reconnecting(error.message.into())
                }
            }
        };
        let (retry_now, retry_requested) = oneshot::channel();
        if this
            .update(cx, |this, cx| {
                this.retry_now = Some(retry_now);
                this.disconnected(status.clone(), cx);
            })
            .is_err()
        {
            return;
        }
        let wait = match &status {
            MachineStatus::Attention { .. } => ATTENTION_RETRY_DELAY,
            _ => delay,
        };
        log::warn!("{status:?}; retrying in {wait:?}");
        futures::select_biased! {
            _ = retry_requested.fuse() => delay = min_delay,
            _ = cx.background_executor().timer(wait).fuse() => {
                delay = (delay * 2).min(max_delay);
            }
        }
        if this
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

/// The session, and whether the server is older than the one installed.
async fn connect(
    runtime: &tokio::runtime::Handle,
    transport: &Transport,
    upload_progress: mpsc::UnboundedSender<UploadProgress>,
) -> Result<(Connection, agentz_client::Events, bool), SshError> {
    match transport {
        Transport::Local => connect_local(runtime)
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
        Transport::Ssh(target) => {
            let target = target.clone();
            runtime
                .spawn(async move {
                    let ssh = Ssh::new(&target).map_err(|error| SshError {
                        message: format!("{error:#}"),
                        needs_attention: true,
                        hint: None,
                    })?;
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
                })
        }
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
                    }],
                }],
                git: None,
                current: None,
            }],
        }
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
}
