//! The app's connection to one machine's `agentz-server`: this Mac's, started if it isn't
//! running, or another machine's over SSH. It keeps the app's copies of that server's session
//! (projects, registry, agent settings) and of its open threads and terminals up to date, and
//! reconnects when the connection drops. While it's down, the copies stay as they were.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use agentz_client::Connection;
use agentz_client::ssh::{RemotePlatform, Ssh, SshError};
use agentz_protocol::agents::{AgentId, AgentSettings};
use agentz_protocol::terminal::TerminalKey;
use agentz_protocol::{
    AgentSettingsChange, CAPABILITY_RELAY, ClientKind, ConnectionId, DirectoryListing, Event,
    Peers, RelayToolCall, Request, Response,
};
use anyhow::{Context as _, Result, anyhow};
use collections::HashMap;
use futures::FutureExt as _;
use futures::channel::oneshot;
use futures::future::BoxFuture;
use gpui::{App, AppContext as _, AsyncApp, Context, Entity, EventEmitter, Task, WeakEntity};
use ui::SharedString;

use crate::machines::MachineId;
use crate::project_store::ProjectStore;
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
    /// The user stopped the server. Connecting would start it again, so that waits until
    /// they ask.
    Stopped,
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
    /// The server was asked to stop; when the connection closes, don't reconnect.
    stopping: bool,
    /// The copies of the server's session.
    projects: Entity<ProjectStore>,
    registry: Entity<AgentRegistryStore>,
    agent_settings: BTreeMap<AgentId, AgentSettings>,
    /// Open threads and account connections, which get the server's updates.
    threads: HashMap<ConnectionId, WeakEntity<AgentThread>>,
    /// Terminals a view shows, which get the server's frames.
    terminals: HashMap<TerminalKey, WeakEntity<Terminal>>,
    /// Session events that arrived while the session snapshot was on its way.
    queued_session_events: Option<Vec<Event>>,
    /// Cuts the wait before the next attempt short.
    retry_now: Option<oneshot::Sender<()>>,
    /// What the server was last told of the other machines, this connection.
    peers_sent: Option<Peers>,
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
                stopping: false,
                projects,
                registry,
                agent_settings: BTreeMap::new(),
                threads: HashMap::default(),
                terminals: HashMap::default(),
                queued_session_events: None,
                retry_now: None,
                peers_sent: None,
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

    pub fn is_outdated(&self) -> bool {
        self.is_outdated && self.is_online()
    }

    /// Stops the machine's server. The connection drops, and reconnecting starts the server
    /// installed now.
    pub fn restart_server(&self, cx: &App) {
        self.send(Request::Shutdown, cx);
    }

    /// Stops the machine's server, and its agents and terminals, until [`Self::retry`].
    pub fn stop_server(&mut self, cx: &mut Context<Self>) {
        if self.connection.is_none() {
            return;
        }
        self.stopping = true;
        self.send(Request::Shutdown, cx);
    }

    /// Tries again now instead of waiting out the backoff, as after fixing what needed
    /// attention. A stopped server starts again.
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
        self.registry.update(cx, |registry, cx| {
            registry.set_snapshot(session.registry, cx)
        });
        self.set_agent_settings(session.agent_settings, cx);
        for event in self.queued_session_events.take().unwrap_or_default() {
            self.handle_event(event, cx);
        }
    }

    fn handle_event(&mut self, event: Event, cx: &mut Context<Self>) {
        if let Some(queued) = &mut self.queued_session_events
            && matches!(
                event,
                Event::Projects(_) | Event::Registry(_) | Event::AgentSettings(_)
            )
        {
            queued.push(event);
            return;
        }
        match event {
            Event::Projects(projects) => self
                .projects
                .update(cx, |store, cx| store.set_snapshot(projects, cx)),
            Event::Registry(registry) => self
                .registry
                .update(cx, |store, cx| store.set_snapshot(registry, cx)),
            Event::AgentSettings(agent_settings) => self.set_agent_settings(agent_settings, cx),
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
        let status = match connect(&runtime, &transport).await {
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
        let Ok(status) = this.update(cx, |this, cx| {
            let status = if std::mem::take(&mut this.stopping) {
                MachineStatus::Stopped
            } else {
                status
            };
            this.retry_now = Some(retry_now);
            this.disconnected(status.clone(), cx);
            status
        }) else {
            return;
        };
        if status == MachineStatus::Stopped {
            log::info!("agentz-server stopped; waiting to be started");
            if retry_requested.await.is_err() {
                return;
            }
            delay = min_delay;
        } else {
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
) -> Result<(Connection, agentz_client::Events, bool), SshError> {
    match transport {
        Transport::Local => connect_local(runtime)
            .await
            .map(|(connection, events)| (connection, events, false))
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
