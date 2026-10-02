//! The state the server owns, and how requests and background results change it.

use std::collections::BTreeMap;
use std::path::PathBuf;

use agent_client_protocol::schema::v1 as acp;
use agent_thread::{AgentThread, AgentThreadEvent, ThreadMessage, ThreadView};
use agentz_protocol::agents::{
    AgentId, AgentListing, InstallState, RegistryAgentMetadata, RegistrySnapshot,
};
use agentz_protocol::{
    AgentSettingsChange, ConnectionId, ErrorResponse, Event, Request, Response, ServerMessage,
    SessionSnapshot,
};
use anyhow::{Context as _, Result, anyhow};
use collections::{HashMap, HashSet};
use futures::channel::mpsc;
use futures::{FutureExt as _, StreamExt as _};
use gpui_shared_string::SharedString;
use projects::{ProjectStore, ThreadId};
use registry::{AgentRegistryStore, CommandFuture, RegistryMessage};
use tokio::task::JoinSet;

use crate::agent_settings::AgentSettingsStore;
use crate::{CustomAgent, ServerConfig};

const MAX_THREAD_TITLE_CHARS: usize = 48;

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
    Shutdown,
}

struct Client {
    outgoing: mpsc::UnboundedSender<ServerMessage>,
    subscribed_to_session: bool,
    /// What the client has been sent of each thread it subscribed to.
    threads: HashMap<ConnectionId, ThreadView>,
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
    custom_agents: BTreeMap<AgentId, CustomAgent>,
    projects: ProjectStore,
    registry: AgentRegistryStore,
    agent_settings: AgentSettingsStore,
    threads: HashMap<ThreadId, AgentThread>,
    accounts: HashMap<u64, Account>,
    next_account_id: u64,
    clients: HashMap<ClientId, Client>,
    // What session subscribers were last sent.
    projects_revision_sent: u64,
    registry_sent: RegistrySnapshot,
    agent_settings_revision_sent: u64,
    registry_changed: bool,
    changed_connections: HashSet<ConnectionId>,
    stopping: bool,
    /// Pass the stores' and threads' background results on to `inputs`.
    forwarders: JoinSet<()>,
}

impl Server {
    pub(crate) fn new(
        runtime: tokio::runtime::Handle,
        config: ServerConfig,
        inputs: mpsc::UnboundedSender<Input>,
    ) -> Self {
        let data_dir = config.data_dir;
        let projects = ProjectStore::load(Some(data_dir.join("state.json")));
        let agent_settings = AgentSettingsStore::load(
            Some(data_dir.join("agents").join("settings.json")),
            Some(&data_dir.join("settings.json")),
        );
        let (mut registry, registry_inbox) = AgentRegistryStore::new(
            runtime.clone(),
            config.http_client,
            config.shell_environment_ready,
            registry_dir(&data_dir),
        );
        registry.refresh_if_stale();
        let mut server = Self {
            runtime,
            inputs,
            custom_agents: config.custom_agents,
            projects_revision_sent: projects.revision(),
            registry_sent: RegistrySnapshot::default(),
            agent_settings_revision_sent: agent_settings.revision(),
            projects,
            registry,
            agent_settings,
            threads: HashMap::default(),
            accounts: HashMap::default(),
            next_account_id: 1,
            clients: HashMap::default(),
            registry_changed: false,
            changed_connections: HashSet::default(),
            stopping: false,
            forwarders: JoinSet::new(),
        };
        server.forward(registry_inbox, Input::Registry);
        server.registry_sent = server.registry_snapshot();
        server
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
                    },
                );
            }
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
                // Nobody is left to see an account panel's agent.
                self.accounts.retain(|_, account| account.owner != client);
            }
            Input::Registry(message) => {
                self.registry.handle(message);
                self.registry_changed = true;
            }
            Input::Shutdown => self.stopping = true,
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
                }))
            }
            Request::SubscribeThread(connection) => {
                self.client(client)?;
                let view = self.update_thread(connection, |thread| ThreadView::clone(thread))?;
                self.client(client)?
                    .threads
                    .insert(connection, view.clone());
                Ok(Response::Thread(view))
            }
            Request::UnsubscribeThread(connection) => {
                self.client(client)?.threads.remove(&connection);
                Ok(Response::Ok)
            }

            Request::AddProject { path } => {
                Ok(Response::ProjectAdded(self.projects.add_project(path)))
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

            Request::CreateThread {
                project_id,
                agent_id,
            } => {
                let thread_id = self
                    .projects
                    .add_thread(
                        project_id,
                        projects::NEW_THREAD_TITLE,
                        Some(agent_id.0.to_string()),
                    )
                    .context("no such project")?;
                // Started now, so the agent is ready by the time the user has typed a prompt.
                self.update_thread(ConnectionId::Thread(thread_id), |_| {})?;
                Ok(Response::ThreadCreated(thread_id))
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
                self.projects.delete_thread(thread_id);
                Ok(Response::Ok)
            }

            Request::Prompt { connection, text } => {
                self.update_thread(connection, |thread| thread.send(text))?;
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
            } => {
                self.update_thread(connection, |thread| thread.authenticate(method_id))?;
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
                let command = self.agent_command(&agent_id, false);
                let (thread, inbox) = AgentThread::start_for_account(
                    self.runtime.clone(),
                    self.agent_name(&agent_id),
                    command,
                );
                let account_id = self.next_account_id;
                self.next_account_id += 1;
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

            Request::Shutdown => {
                self.stopping = true;
                Ok(Response::Ok)
            }
            Request::Unknown(request) => Err(anyhow!("unsupported request: {request}")),
        }
    }

    fn client(&mut self, client: ClientId) -> Result<&mut Client> {
        self.clients
            .get_mut(&client)
            .context("the client disconnected")
    }

    fn existing_thread(&self, thread_id: ThreadId) -> Result<()> {
        self.projects
            .thread(thread_id)
            .map(|_| ())
            .context("no such thread")
    }

    /// Changes a connection's agent thread, starting a thread's agent if it isn't running.
    fn update_thread<R>(
        &mut self,
        connection: ConnectionId,
        change: impl FnOnce(&mut AgentThread) -> R,
    ) -> Result<R> {
        let thread = match connection {
            ConnectionId::Thread(thread_id) => {
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
            .project(thread.project_id)
            .context("no such project")?
            .path
            .clone();
        // Older builds saved a session for every new thread, even before its first prompt. An
        // untitled thread never got one, so the agent has nothing to load.
        let never_prompted = !thread.has_custom_title && thread.title == projects::NEW_THREAD_TITLE;
        let previous_session = thread
            .session_id
            .clone()
            .filter(|_| !never_prompted)
            .map(acp::SessionId::new);
        let Some(agent_id) = thread.agent_id.clone().map(AgentId::new) else {
            return Ok(AgentThread::failed(
                "Agent".into(),
                "This thread has no agent.",
            ));
        };
        let command = self.agent_command(&agent_id, true);
        let (mut agent_thread, inbox) = AgentThread::start(
            self.runtime.clone(),
            self.agent_name(&agent_id),
            command,
            cwd,
            previous_session,
        );
        agent_thread.set_defaults(self.agent_settings.get(&agent_id).session_defaults());
        let connection = ConnectionId::Thread(thread_id);
        self.forward(inbox, move |message| Input::Thread(connection, message));
        Ok(agent_thread)
    }

    /// The registry's agents, then the custom ones, which count as installed.
    fn registry_snapshot(&self) -> RegistrySnapshot {
        let mut snapshot = self.registry.snapshot();
        snapshot
            .agents
            .extend(self.custom_agents.iter().map(|(id, agent)| AgentListing {
                metadata: RegistryAgentMetadata {
                    id: id.clone(),
                    name: agent.name.clone(),
                    description: "A custom agent".into(),
                    version: "custom".into(),
                    repository: None,
                    website: None,
                    license_url: None,
                    icon_path: None,
                },
                supports_current_platform: true,
                install_state: InstallState::Installed {
                    version: "custom".into(),
                    update_available: false,
                },
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
                (
                    ConnectionId::Thread(thread_id),
                    AgentThreadEvent::TitleChanged(title) | AgentThreadEvent::FirstPrompt(title),
                ) => self
                    .projects
                    .rename_thread(thread_id, thread_title_from_prompt(&title)),
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
                (_, AgentThreadEvent::LoggedIn(method)) => {
                    if let Some(agent_id) = &agent_id {
                        self.agent_settings.update(agent_id, |settings| {
                            settings.login_method = Some(method.to_string())
                        });
                    }
                }
                (_, AgentThreadEvent::LoggedOut) => {
                    if let Some(agent_id) = &agent_id {
                        self.agent_settings
                            .update(agent_id, |settings| settings.login_method = None);
                    }
                }
                (ConnectionId::Account(_), _) => {}
            }
        }

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

        for connection in std::mem::take(&mut self.changed_connections) {
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
                if let Some(update) = view.changes_since(sent) {
                    sent.apply(update.clone());
                    send_to(
                        &client.outgoing,
                        ServerMessage::Event(Event::Thread { connection, update }),
                    );
                }
            }
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
