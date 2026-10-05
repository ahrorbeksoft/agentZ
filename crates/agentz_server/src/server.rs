//! The state the server owns, and how requests and background results change it.

mod custom_agents;
#[cfg(unix)]
mod hand_off;
mod prompt_requests;
mod session_requests;
mod space_requests;
mod terminal_requests;
mod tools;
mod workspace_requests;

use std::collections::{BTreeMap, VecDeque};
use std::future::Future;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};

use agent_client_protocol::schema::v1 as acp;
use agent_thread::{AgentThread, AgentThreadEvent, ThreadMessage, ThreadView};
use agentz_protocol::agents::{
    AgentId, AgentListing, AgentSettings, InstallState, RegistryAgentMetadata, RegistrySnapshot,
};
use agentz_protocol::diff::{DiffScope, RestoreAvailability, ThreadDiff};
use agentz_protocol::terminal::{TerminalFrame, TerminalKey};
use agentz_protocol::{
    AgentSettingsChange, ConnectionId, ErrorResponse, Event, MachineIcon, MachineInfo, PromptPart,
    Request, Response, ServerMessage, SessionSnapshot,
};
use anyhow::{Context as _, Result, anyhow};
use collections::{HashMap, HashSet};
use futures::channel::mpsc;
use futures::{FutureExt as _, StreamExt as _};
use gpui_shared_string::SharedString;
use projects::{ProjectStore, ThreadCreator, ThreadId};
use registry::{AgentRegistryStore, CommandFuture, RegistryMessage};
use serde::{Deserialize, Serialize};
use tokio::task::JoinSet;
use util::ResultExt as _;

use crate::agent_settings::AgentSettingsStore;
use crate::browser;
use crate::checkpoints::Checkpoints;
use crate::continuations;
use crate::machine_kind;
use crate::repositories::{self, RepositoryChecks};
use crate::spaces::SpaceStore;
use crate::{AgentControl, CustomAgent, ServerConfig};
use terminal_requests::Terminals;
use tools::{PendingToolCall, ToolResults};

const MAX_THREAD_TITLE_CHARS: usize = 48;
/// t3code sweeps every project each minute; lookups that aren't stale are skipped.
const REPOSITORY_SWEEP_INTERVAL: Duration = Duration::from_secs(60);

pub(crate) type ClientId = u64;

pub(crate) enum Input {
    Connected {
        client: ClientId,
        outgoing: mpsc::UnboundedSender<ServerMessage>,
    },
    Request {
        client: ClientId,
        id: u64,
        request: Request,
    },
    Disconnected(ClientId),
    Registry(RegistryMessage),
    Thread(ConnectionId, ThreadMessage),
    /// A waiting tool call's timeout has passed.
    ToolDeadline,
    /// A thread's agent asked for one of its terminals.
    AgentTerminal(ThreadId, agent_thread::TerminalRequest),
    /// An event from a terminal's run (`serial`).
    Terminal {
        key: TerminalKey,
        serial: u64,
        event: alacritty_terminal::event::Event,
    },
    /// The answer to a request that needed background work.
    Respond {
        client: ClientId,
        id: u64,
        result: Result<Response, ErrorResponse>,
    },
    /// Background work's result, applied to the server's state.
    Run(Box<dyn FnOnce(&mut Server) + Send>),
    Shutdown,
}

/// A message for a thread that was busy, sent once its turn ends: from an agent, or word that
/// a task it delegated has ended.
#[derive(Serialize, Deserialize)]
pub(crate) struct FollowUp {
    text: String,
    from: ThreadCreator,
    /// The subthread whose end this announces.
    task: Option<ThreadId>,
}

struct Client {
    outgoing: mpsc::UnboundedSender<ServerMessage>,
    subscribed_to_session: bool,
    /// What the client has been sent of each thread it subscribed to.
    threads: HashMap<ConnectionId, ThreadView>,
    /// And of each terminal it watches.
    terminals: HashMap<TerminalKey, TerminalFrame>,
}

/// An agent started only to log in or out, from its settings.
struct Account {
    agent_id: AgentId,
    owner: ClientId,
    thread: AgentThread,
}

pub(crate) struct Server {
    runtime: tokio::runtime::Handle,
    inputs: mpsc::UnboundedSender<Input>,
    machine: MachineInfo,
    data_dir: PathBuf,
    custom_agents: BTreeMap<AgentId, CustomAgent>,
    agent_control: Option<AgentControl>,
    /// Where agentZ's `xdg-open` and the like are, when agents' login pages go to the clients.
    browser_programs: Option<PathBuf>,
    terminal_shell: Option<String>,
    /// The socket clients connect to, for handing off.
    #[cfg(unix)]
    listener: Option<std::os::fd::RawFd>,
    /// A handoff to a newer server is under way.
    handing_off: bool,
    /// One waiting for threads to pause, to go on once [`Self::pausing_threads`] is empty.
    #[cfg(unix)]
    pending_hand_off: Option<hand_off::PendingHandOff>,
    /// Threads whose connections are pausing to hand their agents over.
    pausing_threads: HashSet<ThreadId>,
    /// Set once it's done, as the server stops.
    handed_off: Arc<AtomicBool>,
    /// The MCP bridges' credentials, each given to one thread's agent. Dropped with the agent.
    tool_sessions: HashMap<String, ThreadId>,
    follow_ups: HashMap<ThreadId, VecDeque<FollowUp>>,
    /// Threads handed off to a new workspace, with their continuation prompts, whose agents
    /// restart there once their turn ends.
    moving_threads: HashMap<ThreadId, Option<String>>,
    /// Tool calls waiting for a thread, answered as soon as it's ready or their time is up.
    pending_tool_calls: Vec<PendingToolCall>,
    /// Messages waiting for the threads they mention to load.
    pending_prompts: Vec<prompt_requests::PendingPrompt>,
    tool_results: ToolResults,
    /// Tool calls for other machines, relayed through app clients.
    relays: tools::Relays,
    projects: ProjectStore,
    repository_checks: RepositoryChecks,
    registry: AgentRegistryStore,
    agent_settings: AgentSettingsStore,
    threads: HashMap<ThreadId, AgentThread>,
    accounts: HashMap<u64, Account>,
    next_account_id: u64,
    terminals: Terminals,
    spaces: SpaceStore,
    /// The kind of machine this is, as detected and as chosen.
    machine_icon: MachineIcon,
    clients: HashMap<ClientId, Client>,
    // What session subscribers were last sent.
    projects_revision_sent: u64,
    spaces_revision_sent: u64,
    registry_sent: RegistrySnapshot,
    agent_settings_revision_sent: u64,
    machine_icon_sent: MachineIcon,
    registry_changed: bool,
    changed_connections: HashSet<ConnectionId>,
    /// When each draft no client has open is removed, unless something is typed in it (see
    /// [`Self::sweep_drafts`]).
    draft_due: HashMap<ThreadId, Instant>,
    /// When a sweep of them is due.
    draft_sweep_at: Option<Instant>,
    stopping: bool,
    /// Pass the stores' and threads' background results on to `inputs`.
    forwarders: JoinSet<()>,
}

/// How long a draft with nothing typed may go unwatched, once the user leaves it, before it's
/// removed: long enough for a view to be rebuilt.
const LEAVE_GRACE: Duration = Duration::from_secs(3);
/// The same, for one just made or found at start: the client opening it may be far away.
const OPEN_GRACE: Duration = Duration::from_secs(60);

impl Server {
    pub(crate) fn new(
        runtime: tokio::runtime::Handle,
        config: ServerConfig,
        machine: MachineInfo,
        inputs: mpsc::UnboundedSender<Input>,
        handed_off: Arc<AtomicBool>,
    ) -> Self {
        let data_dir = config.data_dir;
        let mut projects = ProjectStore::load(Some(data_dir.join("state.json")));
        tools::interrupt_unfinished_tasks(&mut projects);
        let agent_settings = AgentSettingsStore::load(
            Some(data_dir.join("agents").join("settings.json")),
            Some(&data_dir.join("settings.json")),
        );
        let (mut registry, registry_inbox) = AgentRegistryStore::new(
            runtime.clone(),
            config.http_client,
            config.shell_environment_ready,
            registry_dir(&data_dir),
            data_dir.join("node"),
        );
        registry.refresh_if_stale();
        let spaces = SpaceStore::load(Some(data_dir.join("spaces.json")));
        let machine_icon = MachineIcon {
            detected: None,
            chosen: machine_kind::load_choice(&data_dir.join("machine.json")),
        };
        let browser_programs = config
            .agent_control
            .as_ref()
            .filter(|_| config.hands_pages_to_clients)
            .and_then(|control| {
                let directory = data_dir.join("browser");
                browser::install(&directory, &control.executable)
                    .log_err()
                    .map(|()| directory)
            });
        let mut server = Self {
            runtime,
            inputs,
            machine,
            data_dir,
            custom_agents: config.custom_agents,
            agent_control: config.agent_control,
            browser_programs,
            terminal_shell: config.terminal_shell,
            #[cfg(unix)]
            listener: config.listener,
            handing_off: false,
            #[cfg(unix)]
            pending_hand_off: None,
            pausing_threads: HashSet::default(),
            handed_off,
            tool_sessions: HashMap::default(),
            follow_ups: HashMap::default(),
            moving_threads: HashMap::default(),
            pending_tool_calls: Vec::new(),
            pending_prompts: Vec::new(),
            tool_results: ToolResults::default(),
            relays: tools::Relays::default(),
            projects_revision_sent: projects.revision(),
            registry_sent: RegistrySnapshot::default(),
            agent_settings_revision_sent: agent_settings.revision(),
            projects,
            repository_checks: RepositoryChecks::default(),
            registry,
            agent_settings,
            threads: HashMap::default(),
            accounts: HashMap::default(),
            next_account_id: 1,
            terminals: Terminals::default(),
            spaces_revision_sent: spaces.revision(),
            spaces,
            machine_icon_sent: machine_icon.clone(),
            machine_icon,
            clients: HashMap::default(),
            registry_changed: false,
            changed_connections: HashSet::default(),
            draft_due: HashMap::default(),
            draft_sweep_at: None,
            stopping: false,
            forwarders: JoinSet::new(),
        };
        server.forward(registry_inbox, Input::Registry);
        server.registry_sent = server.registry_snapshot();
        server.refresh_repositories();
        #[cfg(unix)]
        if let Some(handed_over) = config.handed_over {
            let crate::handoff::HandedOver {
                manifest,
                ptys,
                agent_pipes,
            } = handed_over;
            server.adopt_terminals(manifest.terminals, manifest.palette, ptys);
            server.adopt_agents(manifest.agents, agent_pipes);
            server.follow_ups = manifest
                .follow_ups
                .into_iter()
                .map(|(thread_id, follow_ups)| (thread_id, follow_ups.into()))
                .collect();
        }
        server.restore_spaces();
        let opened_by = Instant::now() + OPEN_GRACE;
        for thread in server.projects.threads() {
            if thread.is_draft {
                server.draft_due.insert(thread.id, opened_by);
            }
        }
        for thread_id in continuations::list(&server.data_dir) {
            if server
                .projects
                .thread(thread_id)
                .is_none_or(|thread| !thread.is_draft)
            {
                continuations::remove(&server.data_dir, thread_id).log_err();
            }
        }
        server.sweep_drafts();
        server.spawn_then(machine_kind::detect(), |server, detected| {
            server.machine_icon.detected = detected;
        });
        let inputs = server.inputs.clone();
        server.runtime.spawn(async move {
            loop {
                tokio::time::sleep(REPOSITORY_SWEEP_INTERVAL).await;
                let sweep = Input::Run(Box::new(|server: &mut Server| {
                    server.refresh_repositories()
                }));
                if inputs.unbounded_send(sweep).is_err() {
                    break;
                }
            }
        });
        server
    }

    /// Looks up the repository of each project whose last lookup is stale.
    fn refresh_repositories(&mut self) {
        let paths = self
            .projects
            .projects()
            .iter()
            .map(|project| project.path.clone());
        for path in self.repository_checks.take_due(paths, Instant::now()) {
            let resolved = repositories::resolve(path.clone());
            self.spawn_then(resolved, move |server, repository| {
                server
                    .repository_checks
                    .finish(path.clone(), repository.is_some(), Instant::now());
                let project = server
                    .projects
                    .projects()
                    .iter()
                    .find(|project| project.path == path)
                    .map(|project| project.id);
                if let Some(project) = project {
                    server.projects.set_project_repository(project, repository);
                }
            });
        }
    }

    pub(crate) async fn run(mut self, mut inbox: mpsc::UnboundedReceiver<Input>) {
        while let Some(input) = inbox.next().await {
            self.handle(input);
            // Take whatever else has arrived, so a burst of agent output is sent as one update.
            while let Ok(input) = inbox.try_recv() {
                self.handle(input);
            }
            self.send_changes();
            if self.stopping {
                break;
            }
        }
        log::info!("shutting down");
    }

    fn forward<T: Send + 'static>(
        &mut self,
        mut inbox: mpsc::UnboundedReceiver<T>,
        input: impl Fn(T) -> Input + Send + 'static,
    ) {
        while self.forwarders.try_join_next().is_some() {}
        let inputs = self.inputs.clone();
        self.forwarders.spawn_on(
            async move {
                while let Some(message) = inbox.next().await {
                    if inputs.unbounded_send(input(message)).is_err() {
                        break;
                    }
                }
            },
            &self.runtime,
        );
    }

    fn handle(&mut self, input: Input) {
        match input {
            Input::Connected { client, outgoing } => {
                self.clients.insert(
                    client,
                    Client {
                        outgoing,
                        subscribed_to_session: false,
                        threads: HashMap::default(),
                        terminals: HashMap::default(),
                    },
                );
            }
            Input::Request {
                client,
                id,
                request:
                    Request::CallTool {
                        caller,
                        name,
                        arguments,
                    },
            } => self.call_tool(client, id, caller, name, arguments),
            Input::Request {
                client,
                id,
                request: Request::ThreadDiff { thread_id, scope },
            } => self.thread_diff(client, id, thread_id, scope),
            #[cfg(unix)]
            Input::Request {
                client,
                id,
                request: Request::HandOff { stop_running_turns },
            } => self.hand_off(client, id, stop_running_turns),
            Input::Request {
                client,
                id,
                request: Request::RestoreCheckpoint { thread_id, scope },
            } => self.restore_checkpoint(client, id, thread_id, scope),
            Input::Request {
                client,
                id,
                request: Request::ListAgentSessions(agent_id),
            } => self.list_agent_sessions(client, id, agent_id),
            Input::Request {
                client,
                id,
                request: Request::ListFiles(thread_id),
            } => self.list_files(client, id, thread_id),
            Input::Request {
                client,
                id,
                request: Request::SaveCustomAgent(change),
            } => self.save_custom_agent(client, id, change),
            Input::Respond { client, id, result } => {
                self.send(client, ServerMessage::Response { id, result })
            }
            Input::Run(then) => then(self),
            Input::Request {
                client,
                id,
                request:
                    request @ (Request::CreateThread { .. }
                    | Request::CreateTerminalThread { .. }
                    | Request::CreateWorkspacesThread { .. }
                    | Request::ProjectGit(_)
                    | Request::RepositoryCheckouts(_)
                    | Request::CreateWorkspace { .. }
                    | Request::RemoveWorkspace { .. }
                    | Request::SyncWorkspace { .. }
                    | Request::BringBackWorkspace { .. }),
            } => self.workspace_request(client, id, request),
            Input::Request {
                client,
                id,
                request: Request::BrowseDirectories { partial_path },
            } => self.spawn_then(
                async move {
                    tokio::task::spawn_blocking(move || crate::directories::browse(&partial_path))
                        .await
                        .map_err(anyhow::Error::from)
                        .and_then(|listing| listing)
                },
                move |server, listing| {
                    server.respond(client, id, listing.map(Response::Directories))
                },
            ),
            Input::Request {
                client,
                id,
                request,
            } => {
                let result = self
                    .handle_request(client, request)
                    .map_err(|error| ErrorResponse {
                        message: format!("{error:#}"),
                    });
                // What the request changed goes first, so a client that hears back from, say,
                // `CreateThread` already has the thread.
                self.send_changes();
                self.send(client, ServerMessage::Response { id, result });
            }
            Input::Disconnected(client) => {
                self.clients.remove(&client);
                self.relays.client_gone(client);
                // Nobody is left to see an account panel's agent.
                self.accounts.retain(|_, account| account.owner != client);
                self.sweep_drafts();
            }
            Input::Registry(message) => {
                self.registry.handle(message);
                self.registry_changed = true;
            }
            Input::Shutdown => self.stopping = true,
            // The waiting calls are checked once this batch is handled.
            Input::ToolDeadline => {}
            Input::Terminal { key, serial, event } => self.terminal_event(key, serial, event),
            Input::AgentTerminal(thread_id, request) => {
                self.agent_terminal_request(thread_id, request)
            }
            Input::Thread(connection, message) => {
                let thread = match connection {
                    ConnectionId::Thread(id) => self.threads.get_mut(&id),
                    ConnectionId::Account(id) => self
                        .accounts
                        .get_mut(&id)
                        .map(|account| &mut account.thread),
                };
                // Otherwise the thread was deleted or the account closed.
                if let Some(thread) = thread {
                    thread.handle(message);
                    self.thread_changed(connection);
                }
            }
        }
    }

    fn handle_request(&mut self, client: ClientId, request: Request) -> Result<Response> {
        match request {
            Request::SubscribeSession => {
                let client = self.client(client)?;
                client.subscribed_to_session = true;
                Ok(Response::Session(SessionSnapshot {
                    projects: self.projects.snapshot(),
                    registry: self.registry_snapshot(),
                    agent_settings: self.agent_settings.all().clone(),
                    spaces: self.spaces.snapshot(),
                    machine_icon: self.machine_icon.clone(),
                }))
            }
            Request::SubscribeThread(connection) => {
                self.client(client)?;
                let view = self.update_thread(connection, |thread| ThreadView::clone(thread))?;
                self.client(client)?
                    .threads
                    .insert(connection, view.clone());
                self.sweep_drafts();
                Ok(Response::Thread(view))
            }
            Request::UnsubscribeThread(connection) => {
                self.client(client)?.threads.remove(&connection);
                self.sweep_drafts();
                Ok(Response::Ok)
            }

            Request::AddProject { path } => {
                // Typed on another machine, so it may not exist here.
                // Collecting the components drops a trailing `/` left by completion.
                let path: PathBuf = crate::directories::expand_home(&path)
                    .components()
                    .collect();
                if !path.is_dir() {
                    return Err(anyhow!("{} isn't a folder here", path.display()));
                }
                let project = self.projects.add_project(path);
                self.refresh_repositories();
                Ok(Response::ProjectAdded(project))
            }
            Request::SetProjectName { project_id, name } => {
                self.projects.set_project_name(project_id, &name);
                Ok(Response::Ok)
            }
            Request::SetProjectIcon { project_id, icon } => {
                self.projects.set_project_icon(project_id, icon);
                Ok(Response::Ok)
            }
            Request::RemoveProject(project_id) => {
                let threads = self
                    .projects
                    .threads()
                    .iter()
                    .filter(|thread| thread.project_id == project_id)
                    .map(|thread| thread.id)
                    .collect();
                self.delete_checkpoints(threads);
                self.projects.remove_project(project_id);
                Ok(Response::Ok)
            }
            Request::SetScope(scope) => {
                self.projects.set_scope(scope);
                Ok(Response::Ok)
            }
            Request::SetThreadOrder(order) => {
                self.projects.set_thread_order(order);
                Ok(Response::Ok)
            }
            Request::ToggleArchivedExpanded => {
                self.projects.toggle_archived_expanded();
                Ok(Response::Ok)
            }
            Request::ToggleWorkspacesExpanded => {
                self.projects.toggle_workspaces_expanded();
                Ok(Response::Ok)
            }
            Request::MoveToAgents(thread_id) => {
                self.move_to_agents(thread_id)?;
                Ok(Response::Ok)
            }

            Request::RenameThread { thread_id, title } => {
                self.existing_thread(thread_id)?;
                self.projects.set_custom_title(thread_id, title);
                Ok(Response::Ok)
            }
            Request::ArchiveThread(thread_id) => {
                self.existing_thread(thread_id)?;
                self.projects.archive_thread(thread_id);
                Ok(Response::Ok)
            }
            Request::UnarchiveThread(thread_id) => {
                self.existing_thread(thread_id)?;
                self.projects.unarchive_thread(thread_id);
                Ok(Response::Ok)
            }
            Request::DeleteThread(thread_id) => {
                self.existing_thread(thread_id)?;
                self.delete_thread(thread_id);
                Ok(Response::Ok)
            }
            Request::SetUnsentText { thread_id, text } => {
                self.existing_thread(thread_id)?;
                self.projects.set_unsent_text(thread_id, text);
                // A draft emptied elsewhere goes, like one left empty.
                self.sweep_drafts();
                Ok(Response::Ok)
            }
            Request::ContinueThread {
                thread_id,
                agent_id,
            } => Ok(Response::ThreadCreated(
                self.continue_thread(thread_id, agent_id)?,
            )),
            Request::DropHandoff(connection) => {
                self.update_thread(connection, AgentThread::drop_handoff)?;
                Ok(Response::Ok)
            }

            Request::Prompt { connection, prompt } => {
                if let ConnectionId::Thread(thread_id) = connection
                    && self
                        .projects
                        .thread(thread_id)
                        .is_some_and(|thread| thread.task.is_some())
                {
                    return Err(anyhow!(
                        "a subthread only takes its task; message its parent instead"
                    ));
                }
                if prompt
                    .iter()
                    .all(|part| matches!(part, PromptPart::Text(_)))
                {
                    let text = prompt
                        .into_iter()
                        .map(|part| match part {
                            PromptPart::Text(text) => text,
                            _ => String::new(),
                        })
                        .collect();
                    self.update_thread(connection, |thread| thread.send(text))?;
                } else {
                    // Starts the thread's agent first, as plain text does.
                    self.update_thread(connection, |_| ())?;
                    self.queue_prompt(connection, prompt)?;
                }
                Ok(Response::Ok)
            }
            Request::Cancel(connection) => {
                self.update_thread(connection, |thread| thread.cancel())?;
                Ok(Response::Ok)
            }
            Request::RespondToPermission {
                connection,
                tool_call_id,
                option_id,
            } => {
                self.update_thread(connection, |thread| {
                    thread.respond_to_permission(&tool_call_id, option_id)
                })?;
                Ok(Response::Ok)
            }
            Request::SetConfigOption {
                connection,
                config_id,
                value,
            } => {
                self.update_thread(connection, |thread| {
                    thread.set_config_option(config_id, value)
                })?;
                Ok(Response::Ok)
            }
            Request::SetMode {
                connection,
                mode_id,
            } => {
                self.update_thread(connection, |thread| thread.set_mode(mode_id))?;
                Ok(Response::Ok)
            }
            Request::ClearPlan(connection) => {
                self.update_thread(connection, |thread| thread.clear_plan())?;
                Ok(Response::Ok)
            }
            Request::Authenticate {
                connection,
                method_id,
                meta,
            } => {
                self.update_thread(connection, |thread| thread.authenticate(method_id, meta))?;
                Ok(Response::Ok)
            }
            Request::CancelAuthentication(connection) => {
                self.update_thread(connection, |thread| thread.cancel_authentication())?;
                Ok(Response::Ok)
            }
            Request::RespondToElicitation {
                connection,
                elicitation,
                action,
            } => {
                self.update_thread(connection, |thread| {
                    thread.respond_to_elicitation(elicitation, action)
                })?;
                Ok(Response::Ok)
            }
            Request::DismissElicitation {
                connection,
                elicitation,
            } => {
                self.update_thread(connection, |thread| thread.dismiss_elicitation(elicitation))?;
                Ok(Response::Ok)
            }
            Request::TerminalLogin {
                connection,
                method_id,
            } => {
                self.client(client)?;
                self.start_terminal_login(connection, method_id)?;
                Ok(Response::Ok)
            }
            Request::OpenLoginPage { connection, url } => {
                let in_terminal_login = self.terminal_login_runs(connection);
                // Only a connection that's running: this mustn't start a thread's agent.
                let runs = match connection {
                    ConnectionId::Thread(thread_id) => self.threads.contains_key(&thread_id),
                    ConnectionId::Account(account_id) => self.accounts.contains_key(&account_id),
                };
                if !runs {
                    return Err(anyhow!("the agent isn't running"));
                }
                self.update_thread(connection, |thread| {
                    thread.open_login_page(url.into(), in_terminal_login)
                })?
                .map_err(|error| anyhow!(error))?;
                Ok(Response::Ok)
            }
            Request::Reauthenticate(connection) => {
                self.update_thread(connection, |thread| thread.reauthenticate())?;
                Ok(Response::Ok)
            }
            Request::Logout(connection) => {
                self.update_thread(connection, |thread| thread.logout())?;
                Ok(Response::Ok)
            }
            Request::RetrySession(connection) => {
                self.update_thread(connection, |thread| thread.retry_session())?;
                Ok(Response::Ok)
            }
            Request::Reload(connection) => {
                self.update_thread(connection, |thread| thread.reload())?;
                Ok(Response::Ok)
            }
            Request::CheckLogin(connection) => {
                self.update_thread(connection, |thread| thread.check_login())?;
                Ok(Response::Ok)
            }

            Request::OpenAccount(agent_id) => {
                self.client(client)?;
                let account_id = self.next_account_id;
                self.next_account_id += 1;
                let command = self.agent_command(&agent_id, false);
                let command =
                    self.with_browser_programs(command, ConnectionId::Account(account_id));
                let (thread, inbox) = AgentThread::start_for_account(
                    self.runtime.clone(),
                    self.agent_name(&agent_id),
                    command,
                );
                self.accounts.insert(
                    account_id,
                    Account {
                        agent_id,
                        owner: client,
                        thread,
                    },
                );
                let connection = ConnectionId::Account(account_id);
                self.forward(inbox, move |message| Input::Thread(connection, message));
                self.thread_changed(connection);
                Ok(Response::AccountOpened(account_id))
            }
            Request::CloseAccount(account_id) => {
                self.accounts
                    .remove(&account_id)
                    .context("no such account")?;
                Ok(Response::Ok)
            }

            Request::RefreshRegistry { if_stale } => {
                if if_stale {
                    self.registry.refresh_if_stale();
                } else {
                    self.registry.refresh();
                }
                self.registry_changed = true;
                Ok(Response::Ok)
            }
            Request::InstallAgent(agent_id) => {
                self.registry.install(&agent_id);
                self.registry_changed = true;
                Ok(Response::Ok)
            }
            Request::UninstallAgent(agent_id) => {
                self.registry.uninstall(&agent_id);
                self.registry_changed = true;
                Ok(Response::Ok)
            }
            Request::SaveCustomAgent(_) => Err(anyhow!("saving an agent is handled separately")),
            Request::RemoveCustomAgent(agent_id) => self.remove_custom_agent(&agent_id),
            Request::AgentIcons(ids) => Ok(Response::AgentIcons(
                ids.iter()
                    .filter_map(|id| self.registry.icon(id).cloned())
                    .collect(),
            )),
            Request::UpdateAgentSettings { agent_id, change } => {
                match change {
                    AgentSettingsChange::SetEnv(env) => self
                        .agent_settings
                        .update(&agent_id, |settings| settings.env = env),
                    AgentSettingsChange::SetLoginMethod(method) => self
                        .agent_settings
                        .update(&agent_id, |settings| settings.login_method = method),
                    AgentSettingsChange::SetDefaultConfigOption { config_id, value } => self
                        .agent_settings
                        .update(&agent_id, |settings| match value {
                            Some(value) => {
                                settings.default_config_options.insert(config_id, value);
                            }
                            None => {
                                settings.default_config_options.remove(&config_id);
                            }
                        }),
                    AgentSettingsChange::SetDefaultMode(mode) => self
                        .agent_settings
                        .update(&agent_id, |settings| settings.default_mode = mode),
                    AgentSettingsChange::Unknown(change) => {
                        return Err(anyhow!("unsupported agent settings change: {change}"));
                    }
                }
                Ok(Response::Ok)
            }
            Request::ImportAgentSessions {
                agent_id,
                sessions,
                archived,
            } => self.import_agent_sessions(agent_id, sessions, archived),
            Request::ListAgentSessions(_) => Err(anyhow!("listing sessions is handled separately")),
            Request::ListFiles(_) => Err(anyhow!("listing files is handled separately")),

            Request::Shutdown => {
                self.stopping = true;
                Ok(Response::Ok)
            }
            Request::HandOff { .. } => Err(anyhow!("this server can't hand off")),
            Request::ListTools => Ok(Response::Tools(tools::definitions())),
            Request::SetMachineIcon(icon) => {
                machine_kind::save_choice(&self.data_dir.join("machine.json"), icon.clone())?;
                self.machine_icon.chosen = icon;
                Ok(Response::Ok)
            }
            Request::SetPeers(peers) => {
                self.client(client)?;
                self.relays.set_peers(client, peers);
                Ok(Response::Ok)
            }
            Request::RelayToolResult { relay_id, result } => {
                self.relays.finish(relay_id, result);
                Ok(Response::Ok)
            }
            // Handled by `call_tool`, `thread_diff` and `workspace_request`, since they may answer
            // later.
            Request::CallTool { .. } => Err(anyhow!("tool calls are handled separately")),
            Request::ThreadDiff { .. } | Request::RestoreCheckpoint { .. } => {
                Err(anyhow!("diffs are handled separately"))
            }
            request @ (Request::DrawerTerminals(_)
            | Request::SubscribeTerminal(_)
            | Request::UnsubscribeTerminal(_)
            | Request::TerminalInput { .. }
            | Request::TerminalSelectionText(_)
            | Request::FindInTerminal { .. }
            | Request::RestartTerminal(_)
            | Request::CloseTerminal(_)) => self.terminal_request(client, request),
            Request::CreateThread { .. }
            | Request::CreateTerminalThread { .. }
            | Request::CreateWorkspacesThread { .. }
            | Request::ProjectGit(_)
            | Request::RepositoryCheckouts(_)
            | Request::CreateWorkspace { .. }
            | Request::RemoveWorkspace { .. }
            | Request::SyncWorkspace { .. }
            | Request::BringBackWorkspace { .. } => {
                Err(anyhow!("workspace requests are handled separately"))
            }
            Request::BrowseDirectories { .. } => Err(anyhow!("browsing is handled separately")),
            Request::Spaces(request) => self.space_request(request),
            Request::Unknown(request) => Err(anyhow!("unsupported request: {request}")),
        }
    }

    fn client(&mut self, client: ClientId) -> Result<&mut Client> {
        self.clients
            .get_mut(&client)
            .context("the client disconnected")
    }

    /// The thread's checkpoints, in the folder it works in: a terminal thread's are where its
    /// shell is, as the agent CLIs it runs work there.
    fn checkpoints(&self, thread_id: ThreadId) -> Option<Checkpoints> {
        Some(Checkpoints::new(
            self.terminal_folder(thread_id)
                .or_else(|| self.projects.thread_folder(thread_id))?,
            &self.machine.id,
            thread_id,
        ))
    }

    /// Runs `work` off the server's task, then `then` with its result on it.
    fn spawn_then<T: Send + 'static>(
        &self,
        work: impl Future<Output = T> + Send + 'static,
        then: impl FnOnce(&mut Server, T) + Send + 'static,
    ) {
        let inputs = self.inputs.clone();
        self.runtime.spawn(async move {
            let result = work.await;
            inputs
                .unbounded_send(Input::Run(Box::new(move |server| then(server, result))))
                .ok();
        });
    }

    /// Answers a request, after sending what it changed.
    fn respond(&mut self, client: ClientId, id: u64, result: Result<Response>) {
        let result = result.map_err(|error| ErrorResponse {
            message: format!("{error:#}"),
        });
        self.send_changes();
        self.send(client, ServerMessage::Response { id, result });
    }

    /// A checkpoint is the whole folder, so restoring is only for a thread alone in its own
    /// worktree or pasture, with its own subthreads (t3code's isolation rule).
    fn restore_availability(&self, thread_id: ThreadId) -> RestoreAvailability {
        let Some(folder) = self.projects.thread_folder(thread_id) else {
            return RestoreAvailability::Unavailable("There's no such thread.".into());
        };
        let Some(workspace) = self.projects.thread_workspace(thread_id) else {
            return RestoreAvailability::Unavailable(
                "Restoring files needs a thread in its own worktree or pasture. The project's \
                 folder may hold changes from other threads and from you."
                    .into(),
            );
        };
        let is_shared = self
            .projects
            .threads_in_folder(&folder)
            .into_iter()
            .any(|other| other != thread_id && self.projects.root_thread(other) != thread_id);
        if is_shared {
            return RestoreAvailability::Unavailable(format!(
                "Another thread works in this {} too, so its files may hold that thread's \
                 changes.",
                workspace.kind.label().to_lowercase()
            ));
        }
        RestoreAvailability::Available
    }

    fn restore_checkpoint(
        &mut self,
        client: ClientId,
        id: u64,
        thread_id: ThreadId,
        scope: DiffScope,
    ) {
        let prepared = (|| {
            if let RestoreAvailability::Unavailable(reason) = self.restore_availability(thread_id) {
                anyhow::bail!(reason);
            }
            let checkpoints = self.checkpoints(thread_id).context("no such thread")?;
            let folder = self
                .projects
                .thread_folder(thread_id)
                .context("no such thread")?;
            anyhow::ensure!(
                !self
                    .projects
                    .threads_in_folder(&folder)
                    .into_iter()
                    .any(|thread| self.projects.is_thread_working(thread)),
                "a turn is running there; wait for it to end, or stop it"
            );
            Ok(checkpoints)
        })();
        let checkpoints = match prepared {
            Ok(checkpoints) => checkpoints,
            Err(error) => return self.respond(client, id, Err(error)),
        };
        self.spawn_then(
            async move {
                checkpoints.restore(scope).await?;
                thread_diff(&checkpoints, scope, RestoreAvailability::Available).await
            },
            move |server, diff| server.respond(client, id, diff.map(Response::ThreadDiff)),
        );
    }

    fn delete_checkpoints(&self, threads: Vec<ThreadId>) {
        let checkpoints: Vec<Checkpoints> = threads
            .into_iter()
            .filter_map(|thread_id| self.checkpoints(thread_id))
            .collect();
        self.runtime.spawn(async move {
            for checkpoints in checkpoints {
                checkpoints.delete().await.log_err();
            }
        });
    }

    fn thread_diff(&mut self, client: ClientId, id: u64, thread_id: ThreadId, scope: DiffScope) {
        let Some(checkpoints) = self.checkpoints(thread_id) else {
            let error = ErrorResponse {
                message: "no such thread".into(),
            };
            self.send(
                client,
                ServerMessage::Response {
                    id,
                    result: Err(error),
                },
            );
            return;
        };
        let restore = self.restore_availability(thread_id);
        let inputs = self.inputs.clone();
        self.runtime.spawn(async move {
            let result = thread_diff(&checkpoints, scope, restore)
                .await
                .map(Response::ThreadDiff)
                .map_err(|error| ErrorResponse {
                    message: format!("{error:#}"),
                });
            inputs
                .unbounded_send(Input::Respond { client, id, result })
                .ok();
        });
    }

    fn existing_thread(&self, thread_id: ThreadId) -> Result<()> {
        self.projects
            .thread(thread_id)
            .map(|_| ())
            .context("no such thread")
    }

    fn move_to_agents(&mut self, thread_id: ThreadId) -> Result<()> {
        let thread = self.projects.thread(thread_id).context("no such thread")?;
        anyhow::ensure!(
            thread.in_workspaces(),
            "the thread is already in the Agents list"
        );
        let folder = self
            .projects
            .thread_folder(thread_id)
            .context("the thread has no folder")?;
        let project_id = match self.projects.thread_project(thread_id) {
            Some(project_id) => project_id,
            None => {
                anyhow::ensure!(folder.is_dir(), "{} was removed", folder.display());
                let project_id = self.projects.add_project(folder);
                self.refresh_repositories();
                project_id
            }
        };
        self.projects.move_thread_to_project(thread_id, project_id);
        Ok(())
    }

    /// Changes a connection's agent thread, starting a thread's agent if it isn't running.
    fn update_thread<R>(
        &mut self,
        connection: ConnectionId,
        change: impl FnOnce(&mut AgentThread) -> R,
    ) -> Result<R> {
        let thread = match connection {
            ConnectionId::Thread(thread_id) => {
                // What's sent to it now would be lost with this server.
                if self
                    .threads
                    .get(&thread_id)
                    .is_some_and(AgentThread::is_paused)
                {
                    return Err(anyhow!(
                        "agentZ is updating its server; try again in a moment"
                    ));
                }
                if !self.threads.contains_key(&thread_id) {
                    let thread = self.start_thread(thread_id)?;
                    self.threads.insert(thread_id, thread);
                }
                self.threads.get_mut(&thread_id).context("no such thread")?
            }
            ConnectionId::Account(account_id) => {
                &mut self
                    .accounts
                    .get_mut(&account_id)
                    .context("no such account")?
                    .thread
            }
        };
        let result = change(thread);
        self.thread_changed(connection);
        Ok(result)
    }

    fn start_thread(&mut self, thread_id: ThreadId) -> Result<AgentThread> {
        let thread = self.projects.thread(thread_id).context("no such thread")?;
        let cwd = self
            .projects
            .thread_folder(thread_id)
            .context("no such project")?;
        // Older builds saved a session for every new thread, even before its first prompt. An
        // untitled thread never got one, so the agent has nothing to load.
        let never_prompted = !thread.has_custom_title && thread.title == projects::NEW_THREAD_TITLE;
        let previous_session = thread
            .session_id
            .clone()
            .filter(|_| !never_prompted)
            .map(acp::SessionId::new);
        if thread.terminal.is_some() {
            return Ok(AgentThread::failed(
                "Terminal".into(),
                "This thread runs a terminal, not an agent.",
            ));
        }
        let Some(agent_id) = thread.agent_id.clone().map(AgentId::new) else {
            return Ok(AgentThread::failed(
                "Agent".into(),
                "This thread has no agent.",
            ));
        };
        let mut command = self.agent_command(&agent_id, true);
        let mut mcp_servers = Vec::new();
        if let Some(control) = self.agent_control.clone() {
            let token = uuid::Uuid::new_v4().to_string();
            self.tool_sessions.insert(token.clone(), thread_id);
            mcp_servers.push(acp::McpServer::Stdio(
                acp::McpServerStdio::new("agentz", &control.executable)
                    .args(vec!["mcp-bridge".into()])
                    .env(vec![
                        acp::EnvVariable::new(
                            "AGENTZ_SOCKET",
                            control.socket.to_string_lossy().into_owned(),
                        ),
                        acp::EnvVariable::new("AGENTZ_MCP_TOKEN", token),
                    ]),
            ));
            // For agents without MCP, which can still run the CLI from their shell tool, as
            // herdr's `HERDR_*` variables allow.
            command = async move {
                let mut command = command.await?;
                command.env.extend([
                    (
                        "AGENTZ_BIN_PATH".to_string(),
                        control.executable.to_string_lossy().into_owned(),
                    ),
                    (
                        "AGENTZ_SOCKET".to_string(),
                        control.socket.to_string_lossy().into_owned(),
                    ),
                    ("AGENTZ_THREAD_ID".to_string(), thread_id.0.to_string()),
                ]);
                Ok(command)
            }
            .boxed();
        }
        let command = self.with_browser_programs(command, ConnectionId::Thread(thread_id));
        let (mut agent_thread, inbox) = AgentThread::start(
            self.runtime.clone(),
            self.agent_name(&agent_id),
            command,
            cwd.clone(),
            previous_session,
            Some(self.agent_terminal_host(thread_id)),
        );
        if let Some(handoff) = continuations::load(&self.data_dir, thread_id)
            .log_err()
            .flatten()
        {
            agent_thread.set_handoff(Some(handoff));
        }
        agent_thread.set_mcp_servers(mcp_servers);
        agent_thread.set_turn_hook(self.turn_hook(cwd, thread_id));
        agent_thread.set_defaults(self.agent_settings.get(&agent_id).session_defaults());
        let connection = ConnectionId::Thread(thread_id);
        self.forward(inbox, move |message| Input::Thread(connection, message));
        Ok(agent_thread)
    }

    /// Runs the `terminal/*` requests of a thread's agent.
    fn agent_terminal_host(&self, thread_id: ThreadId) -> agent_thread::TerminalHost {
        let inputs = self.inputs.clone();
        Arc::new(move |request| {
            inputs
                .unbounded_send(Input::AgentTerminal(thread_id, request))
                .ok();
        })
    }

    /// Takes a checkpoint as each of a thread's turns starts and ends.
    fn turn_hook(&self, cwd: PathBuf, thread_id: ThreadId) -> agent_thread::TurnHook {
        let checkpoints = Checkpoints::new(cwd, &self.machine.id, thread_id);
        Arc::new(move |point| {
            let checkpoints = checkpoints.clone();
            async move { checkpoints.on_turn(point).await }.boxed()
        })
    }

    /// The registry's agents, then the custom ones, which count as installed.
    fn registry_snapshot(&self) -> RegistrySnapshot {
        let mut snapshot = self.registry.snapshot();
        snapshot
            .agents
            .extend(self.custom_agents.iter().map(|(id, agent)| {
                let version: SharedString = custom_agents::custom_agent_version(agent).into();
                AgentListing {
                    metadata: RegistryAgentMetadata {
                        id: id.clone(),
                        name: agent.name.clone(),
                        description: "A custom agent".into(),
                        version: version.clone(),
                        repository: None,
                        website: None,
                        license_url: None,
                        icon: self.custom_agent_icon(agent),
                    },
                    supports_current_platform: true,
                    install_state: InstallState::Installed {
                        version,
                        update_available: false,
                    },
                    custom_command: Some(agent.command.clone()),
                }
            }));
        snapshot
    }

    fn agent_name(&self, agent_id: &AgentId) -> SharedString {
        if let Some(agent) = self.custom_agents.get(agent_id) {
            return agent.name.clone();
        }
        self.registry
            .agent(agent_id)
            .map(|agent| agent.name().clone())
            .unwrap_or_else(|| agent_id.0.clone())
    }

    /// Removes the thread, its subthreads, and what they kept: their checkpoints and any
    /// conversation waiting to go with a first message. Their agents stop with the next changes.
    fn delete_thread(&mut self, thread_id: ThreadId) {
        let threads = self.projects.thread_and_subthreads(thread_id);
        for thread_id in &threads {
            self.draft_due.remove(thread_id);
            continuations::remove(&self.data_dir, *thread_id).log_err();
        }
        self.delete_checkpoints(threads);
        self.projects.delete_thread(thread_id);
    }

    /// A thread the user starts is a draft until its first message, and is removed once they
    /// leave it with nothing typed, as t3code drops an empty draft: when no client has had it
    /// open for a moment. Typed text keeps it, as a draft in the sidebar.
    pub(super) fn sweep_drafts(&mut self) {
        let now = Instant::now();
        let watched: HashSet<ThreadId> = self
            .clients
            .values()
            .flat_map(|client| client.threads.keys())
            .filter_map(|connection| match connection {
                ConnectionId::Thread(thread_id) => Some(*thread_id),
                ConnectionId::Account(_) => None,
            })
            .collect();
        let left_empty: Vec<ThreadId> = self
            .projects
            .threads()
            .iter()
            .filter(|thread| {
                thread.is_draft && thread.unsent_text.is_none() && !watched.contains(&thread.id)
            })
            .map(|thread| thread.id)
            .collect();
        self.draft_due
            .retain(|thread_id, _| left_empty.contains(thread_id));
        let mut expired = Vec::new();
        let mut next_due: Option<Instant> = None;
        for thread_id in left_empty {
            let due = *self.draft_due.entry(thread_id).or_insert(now + LEAVE_GRACE);
            if due <= now {
                expired.push(thread_id);
            } else {
                next_due = Some(next_due.map_or(due, |next| next.min(due)));
            }
        }
        for thread_id in expired {
            log::info!("removing draft thread {} that was left empty", thread_id.0);
            self.delete_thread(thread_id);
        }
        if let Some(due) = next_due
            && self.draft_sweep_at.is_none_or(|scheduled| scheduled > due)
        {
            self.draft_sweep_at = Some(due);
            self.spawn_then(
                tokio::time::sleep(due.saturating_duration_since(now)),
                |server, ()| {
                    server.draft_sweep_at = None;
                    server.sweep_drafts();
                },
            );
        }
    }

    /// The command that starts the agent, with the environment from its settings. Threads
    /// opened right after launch wait for the registry to load.
    fn agent_command(&mut self, agent_id: &AgentId, when_loaded: bool) -> CommandFuture {
        let command = match self.custom_agents.get(agent_id) {
            Some(agent) => futures::future::ready(Ok(agent.command.clone())).boxed(),
            None if when_loaded => self.registry.command_when_loaded(agent_id),
            None => self.registry.command(agent_id),
        };
        let env = self.agent_settings.get(agent_id).env;
        async move {
            let mut command = command.await?;
            command.env.extend(env);
            Ok(command)
        }
        .boxed()
    }

    /// Puts agentZ's `xdg-open` and the like first for the agent of `connection`, when agents'
    /// login pages go to the clients.
    fn with_browser_programs(
        &self,
        command: CommandFuture,
        connection: ConnectionId,
    ) -> CommandFuture {
        let (Some(directory), Some(control)) = (&self.browser_programs, &self.agent_control) else {
            return command;
        };
        let directory = directory.clone();
        let socket = control.socket.clone();
        async move {
            let mut command = command.await?;
            // Read once the command is ready, which waits for the login shell's environment.
            let path = command
                .env
                .get("PATH")
                .cloned()
                .or_else(|| std::env::var("PATH").ok());
            command
                .env
                .extend(browser::agent_env(&directory, connection, path.as_deref()));
            command.env.insert(
                "AGENTZ_SOCKET".to_string(),
                socket.to_string_lossy().into_owned(),
            );
            Ok(command)
        }
        .boxed()
    }

    /// Applies what the thread reports to the projects and agent settings.
    fn thread_changed(&mut self, connection: ConnectionId) {
        self.changed_connections.insert(connection);
        let (thread, agent_id) = match connection {
            ConnectionId::Thread(thread_id) => {
                let agent_id = self
                    .projects
                    .thread(thread_id)
                    .and_then(|thread| thread.agent_id.clone())
                    .map(AgentId::new);
                (self.threads.get_mut(&thread_id), agent_id)
            }
            ConnectionId::Account(account_id) => match self.accounts.get_mut(&account_id) {
                Some(account) => (Some(&mut account.thread), Some(account.agent_id.clone())),
                None => (None, None),
            },
        };
        let Some(thread) = thread else {
            return;
        };
        let events = thread.take_events();
        let mut paused = false;
        let model = thread.model_name();
        let config_options = thread.config_options().to_vec();
        let modes = thread.modes().cloned();

        for event in events {
            match (connection, event) {
                (ConnectionId::Thread(thread_id), AgentThreadEvent::WorkingChanged(working)) => {
                    self.projects.set_thread_working(thread_id, working)
                }
                (ConnectionId::Thread(thread_id), AgentThreadEvent::SessionStarted(session)) => {
                    self.projects
                        .set_thread_session(thread_id, session.0.to_string())
                }
                (ConnectionId::Thread(thread_id), AgentThreadEvent::TitleChanged(title)) => self
                    .projects
                    .rename_thread(thread_id, thread_title_from_prompt(&title)),
                // Its first message, sent or queued, makes a draft a thread.
                (ConnectionId::Thread(thread_id), AgentThreadEvent::FirstPrompt(title)) => {
                    self.projects
                        .rename_thread(thread_id, thread_title_from_prompt(&title));
                    self.projects.set_draft(thread_id, false);
                    self.draft_due.remove(&thread_id);
                }
                // As in Zed, the user's last choice becomes the agent's default.
                (
                    ConnectionId::Thread(_),
                    AgentThreadEvent::ConfigOptionChanged(config_id, value),
                ) => {
                    if let Some(agent_id) = &agent_id {
                        self.agent_settings.update(agent_id, |settings| {
                            settings
                                .default_config_options
                                .insert(config_id.0.to_string(), value);
                        });
                    }
                }
                (ConnectionId::Thread(_), AgentThreadEvent::ModeChanged(mode)) => {
                    if let Some(agent_id) = &agent_id {
                        self.agent_settings
                            .update(agent_id, |settings| settings.default_mode = Some(mode));
                    }
                }
                // What agentZ logged in, until the agent is logged out or in again elsewhere.
                (_, AgentThreadEvent::LoggedIn(method)) => {
                    if let Some(agent_id) = &agent_id {
                        self.agent_settings
                            .update(agent_id, |settings| settings.logged_in(method.to_string()));
                    }
                }
                (_, AgentThreadEvent::LoggedOut) => {
                    if let Some(agent_id) = &agent_id {
                        self.agent_settings
                            .update(agent_id, AgentSettings::logged_out);
                    }
                }
                (_, AgentThreadEvent::AccountReported(status)) => {
                    if let Some(agent_id) = &agent_id {
                        self.agent_settings
                            .update(agent_id, |settings| settings.account_reported(&status));
                    }
                }
                (ConnectionId::Thread(_), AgentThreadEvent::Paused) => paused = true,
                // Sent, the thread is the continuation of the other.
                (ConnectionId::Thread(thread_id), AgentThreadEvent::HandoffSent(from)) => {
                    self.projects.set_continued_from(thread_id, from);
                    continuations::remove(&self.data_dir, thread_id).log_err();
                }
                // Dropped, it's a new thread like any other.
                (ConnectionId::Thread(thread_id), AgentThreadEvent::HandoffDropped) => {
                    continuations::remove(&self.data_dir, thread_id).log_err();
                }
                (ConnectionId::Account(_), _) => {}
            }
        }
        #[cfg(unix)]
        if paused && let ConnectionId::Thread(thread_id) = connection {
            self.thread_paused(thread_id);
        }
        #[cfg(not(unix))]
        let _ = paused;

        // Remembered so clients can name the model of threads that aren't open.
        if let (ConnectionId::Thread(thread_id), Some(model)) = (connection, model) {
            self.projects.set_thread_model(thread_id, model);
        }
        // And so the agent's settings can list its options without starting it.
        if let Some(agent_id) = &agent_id
            && (!config_options.is_empty() || modes.is_some())
        {
            self.agent_settings.update(agent_id, |settings| {
                settings.known_config_options = config_options;
                settings.known_modes = modes;
            });
        }
    }

    /// Sends subscribers what changed since the last call.
    fn send_changes(&mut self) {
        // Stop the agents of threads that were deleted, or removed along with their project.
        // Archived threads keep running.
        let projects = &self.projects;
        self.threads
            .retain(|thread_id, _| projects.thread(*thread_id).is_some());
        let threads = &self.threads;
        self.tool_sessions
            .retain(|_, thread_id| threads.contains_key(thread_id));
        self.close_orphaned_panes();
        self.close_orphaned_terminals();
        self.move_threads();
        self.finish_tasks();
        let answers = self.answer_waiting_tool_calls();
        self.send_waiting_prompts();
        self.announce_finished_tasks();
        self.send_follow_ups();
        for (thread_id, thread) in &self.threads {
            self.projects
                .set_thread_blocked(*thread_id, !thread.state.permission_requests.is_empty());
            self.projects
                .set_thread_awaiting_input(*thread_id, thread.is_awaiting_input());
        }
        let projects = &self.projects;
        let accounts = &self.accounts;
        for client in self.clients.values_mut() {
            client.threads.retain(|connection, _| {
                let is_live = match connection {
                    ConnectionId::Thread(thread_id) => projects.thread(*thread_id).is_some(),
                    ConnectionId::Account(account_id) => accounts.contains_key(account_id),
                };
                if !is_live {
                    send_to(
                        &client.outgoing,
                        ServerMessage::Event(Event::ConnectionClosed(*connection)),
                    );
                }
                is_live
            });
        }

        if self.projects.revision() != self.projects_revision_sent {
            self.projects_revision_sent = self.projects.revision();
            self.broadcast(Event::Projects(self.projects.snapshot()));
        }
        if self.spaces.revision() != self.spaces_revision_sent {
            self.spaces_revision_sent = self.spaces.revision();
            self.broadcast(Event::Spaces(self.spaces.snapshot()));
        }
        if std::mem::take(&mut self.registry_changed) {
            let registry = self.registry_snapshot();
            if registry != self.registry_sent {
                self.registry_sent = registry.clone();
                self.broadcast(Event::Registry(registry));
            }
        }
        if self.agent_settings.revision() != self.agent_settings_revision_sent {
            self.agent_settings_revision_sent = self.agent_settings.revision();
            self.broadcast(Event::AgentSettings(self.agent_settings.all().clone()));
        }
        if self.machine_icon != self.machine_icon_sent {
            self.machine_icon_sent = self.machine_icon.clone();
            self.broadcast(Event::MachineIcon(self.machine_icon.clone()));
        }

        for connection in std::mem::take(&mut self.changed_connections) {
            // Entries before the first that changed are as every subscriber last got them.
            let changed_from = match connection {
                ConnectionId::Thread(thread_id) => self
                    .threads
                    .get_mut(&thread_id)
                    .and_then(AgentThread::take_entries_changed_from),
                ConnectionId::Account(account_id) => self
                    .accounts
                    .get_mut(&account_id)
                    .and_then(|account| account.thread.take_entries_changed_from()),
            }
            .unwrap_or(usize::MAX);
            let view = match connection {
                ConnectionId::Thread(thread_id) => self.threads.get(&thread_id),
                ConnectionId::Account(account_id) => self
                    .accounts
                    .get(&account_id)
                    .map(|account| &account.thread),
            };
            let Some(view) = view else {
                continue;
            };
            for client in self.clients.values_mut() {
                let Some(sent) = client.threads.get_mut(&connection) else {
                    continue;
                };
                if let Some(update) = view.changes_since_from(sent, changed_from) {
                    sent.apply(update.clone());
                    send_to(
                        &client.outgoing,
                        ServerMessage::Event(Event::Thread { connection, update }),
                    );
                }
            }
        }

        self.send_terminal_frames();

        // After the changes, as for any other response.
        for (client, id, result) in answers {
            self.send(
                client,
                ServerMessage::Response {
                    id,
                    result: Ok(Response::ToolResult(result)),
                },
            );
        }
    }

    fn broadcast(&self, event: Event) {
        for client in self.clients.values() {
            if client.subscribed_to_session {
                send_to(&client.outgoing, ServerMessage::Event(event.clone()));
            }
        }
    }

    fn send(&self, client: ClientId, message: ServerMessage) {
        if let Some(client) = self.clients.get(&client) {
            send_to(&client.outgoing, message);
        }
    }
}

/// Fails only once the client has disconnected, which the server hears about separately.
fn send_to(outgoing: &mpsc::UnboundedSender<ServerMessage>, message: ServerMessage) {
    outgoing.unbounded_send(message).ok();
}

/// The thread's changes in the scope, with the patch parsed into files.
async fn thread_diff(
    checkpoints: &Checkpoints,
    scope: DiffScope,
    restore: RestoreAvailability,
) -> Result<ThreadDiff> {
    let diff = checkpoints.diff(scope).await?;
    Ok(ThreadDiff {
        status: diff.status,
        turns: diff.turns,
        files: agentz_protocol::diff::parse_patch(&diff.patch),
        truncated: diff.truncated,
        // Only turns can be reverted.
        restore: if scope.is_turns() {
            restore
        } else {
            RestoreAvailability::Unavailable("Only turns' changes can be reverted.".into())
        },
        finished_turns: diff.finished_turns,
        base_ref: diff.base_ref,
    })
}

fn registry_dir(data_dir: &std::path::Path) -> PathBuf {
    data_dir.join("agents").join("registry")
}

/// The first line of the first prompt, shortened to fit the sidebar.
fn thread_title_from_prompt(prompt: &str) -> String {
    let first_line = prompt.lines().next().unwrap_or_default().trim();
    if first_line.chars().count() <= MAX_THREAD_TITLE_CHARS {
        return first_line.to_string();
    }
    let shortened: String = first_line
        .chars()
        .take(MAX_THREAD_TITLE_CHARS - 1)
        .collect();
    format!("{}…", shortened.trim_end())
}

#[cfg(test)]
mod tests {
    use super::{MAX_THREAD_TITLE_CHARS, thread_title_from_prompt};

    #[test]
    fn thread_titles() {
        assert_eq!(
            thread_title_from_prompt("Fix the login bug\nmore detail"),
            "Fix the login bug"
        );
        let long = "Build a checkout page with a cart summary, a pay button and order history";
        let title = thread_title_from_prompt(long);
        assert_eq!(title.chars().count(), MAX_THREAD_TITLE_CHARS);
        assert!(title.ends_with('…'));
    }
}
