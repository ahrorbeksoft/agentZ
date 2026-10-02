//! One conversation with an ACP agent: starts the agent process, runs an ACP session in the
//! project folder, and keeps the conversation (messages, tool calls, plan, permission
//! requests) as it streams in.
//!
//! The connection setup follows Zed's `agent_servers::acp`. Plain Rust on tokio, so the server
//! can own it: the agent's process, the SDK's handlers and requests in flight report back as
//! [`ThreadMessage`]s, which the thread's owner passes to [`AgentThread::handle`]. What the
//! owner should hear about queues up as [`AgentThreadEvent`]s.

use std::collections::VecDeque;
use std::future::Future;
use std::path::PathBuf;
use std::process::Stdio;
use std::time::Instant;

use agent_client_protocol::schema::ProtocolVersion;
use agent_client_protocol::schema::v1 as acp;
use agent_client_protocol::{Agent, Client, ConnectionTo, Lines, Responder};
use anyhow::{Context as _, Result, anyhow};
use futures::channel::{mpsc, oneshot};
use futures::{FutureExt as _, StreamExt as _};
use gpui_shared_string::SharedString;
use registry::AgentCommand;
pub use registry::CommandFuture;
use tokio::io::{AsyncBufReadExt as _, AsyncWriteExt as _};
use tokio::task::JoinSet;

const STDERR_LINES_KEPT: usize = 20;

#[derive(Clone, Debug, PartialEq)]
pub enum ConnectionStatus {
    Connecting,
    /// The agent is running but needs the user to log in before a session can start.
    AuthRequired,
    Ready,
    Failed(SharedString),
}

/// Context-window usage reported by the agent.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ContextUsage {
    pub used: u64,
    pub size: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Entry {
    UserMessage(String),
    AgentMessage(String),
    AgentThought(String),
    ToolCall(ToolCall),
    /// Where the plan first appeared; the plan itself is kept up to date in [`AgentThread::plan`].
    Plan,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ToolCall {
    pub id: acp::ToolCallId,
    pub title: String,
    pub kind: acp::ToolKind,
    pub status: acp::ToolCallStatus,
    pub text: Vec<String>,
    pub diffs: Vec<FileDiff>,
    pub locations: Vec<PathBuf>,
    /// The tool's input as markdown (JSON in a code block), for Zed's "Raw Input" view.
    pub raw_input: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FileDiff {
    pub path: PathBuf,
    pub old_text: Option<String>,
    pub new_text: String,
}

impl FileDiff {
    /// The lines that differ between the old and new text, as one removed and one added block.
    /// The shared prefix and suffix are trimmed, which is enough for a summary card.
    pub fn changed_lines(&self) -> (Vec<&str>, Vec<&str>) {
        let new_lines: Vec<&str> = self.new_text.lines().collect();
        let Some(old_text) = &self.old_text else {
            return (Vec::new(), new_lines);
        };
        let old_lines: Vec<&str> = old_text.lines().collect();
        let common_prefix = old_lines
            .iter()
            .zip(&new_lines)
            .take_while(|(old, new)| old == new)
            .count();
        let common_suffix = old_lines[common_prefix..]
            .iter()
            .rev()
            .zip(new_lines[common_prefix..].iter().rev())
            .take_while(|(old, new)| old == new)
            .count();
        (
            old_lines[common_prefix..old_lines.len() - common_suffix].to_vec(),
            new_lines[common_prefix..new_lines.len() - common_suffix].to_vec(),
        )
    }

    /// The changed region with up to `context` unchanged lines on each side, as a diff editor
    /// would show it.
    pub fn hunk(&self, context: usize) -> Vec<(DiffLineKind, &str)> {
        let new_lines: Vec<&str> = self.new_text.lines().collect();
        let old_lines: Vec<&str> = self
            .old_text
            .as_deref()
            .map(|text| text.lines().collect())
            .unwrap_or_default();
        let common_prefix = old_lines
            .iter()
            .zip(&new_lines)
            .take_while(|(old, new)| old == new)
            .count();
        let common_suffix = old_lines[common_prefix..]
            .iter()
            .rev()
            .zip(new_lines[common_prefix..].iter().rev())
            .take_while(|(old, new)| old == new)
            .count();

        let mut lines = Vec::new();
        for line in &new_lines[common_prefix.saturating_sub(context)..common_prefix] {
            lines.push((DiffLineKind::Context, *line));
        }
        for line in &old_lines[common_prefix..old_lines.len() - common_suffix] {
            lines.push((DiffLineKind::Removed, *line));
        }
        for line in &new_lines[common_prefix..new_lines.len() - common_suffix] {
            lines.push((DiffLineKind::Added, *line));
        }
        let suffix_start = new_lines.len() - common_suffix;
        for line in &new_lines[suffix_start..(suffix_start + context).min(new_lines.len())] {
            lines.push((DiffLineKind::Context, *line));
        }
        lines
    }

    /// Line counts added and removed, for the "+84 −12" summary.
    pub fn line_counts(&self) -> (usize, usize) {
        let (removed, added) = self.changed_lines();
        (added.len(), removed.len())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiffLineKind {
    Context,
    Removed,
    Added,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PlanItem {
    pub content: String,
    pub status: acp::PlanEntryStatus,
}

#[derive(Clone, Debug)]
pub struct PermissionOption {
    pub id: acp::PermissionOptionId,
    pub name: String,
    pub kind: acp::PermissionOptionKind,
}

pub struct PermissionRequest {
    pub tool_call_id: acp::ToolCallId,
    pub title: String,
    pub options: Vec<PermissionOption>,
    responder: Responder<acp::RequestPermissionResponse>,
}

/// How the thread's ACP session was set up when the agent started.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SessionRestore {
    /// A brand-new conversation.
    New,
    /// The previous session was loaded and the agent replayed its history.
    Loaded,
    /// The previous session continues, but the agent can't show its earlier messages.
    ResumedWithoutHistory,
    /// The previous session couldn't be restored, so a new one was started.
    Unavailable,
}

pub enum AgentThreadEvent {
    /// The agent started or finished working on a prompt.
    WorkingChanged(bool),
    /// The ACP session to restore the thread from later: sent once the session has a prompt.
    SessionStarted(acp::SessionId),
    /// The first prompt of a new conversation was sent; useful as a title.
    FirstPrompt(String),
    /// The agent named the session.
    TitleChanged(String),
    /// The user changed one of the session's settings; Zed keeps it as the agent's default.
    ConfigOptionChanged(acp::SessionConfigId, acp::SessionConfigOptionValue),
    /// The user changed the session's mode; Zed keeps it as the agent's default.
    ModeChanged(acp::SessionModeId),
    /// Logging in with the named method succeeded.
    LoggedIn(SharedString),
    LoggedOut,
}

/// Settings applied to new sessions (not to loaded ones), as Zed's per-agent defaults are.
#[derive(Clone, Debug, Default)]
pub struct SessionDefaults {
    pub mode: Option<acp::SessionModeId>,
    pub config_options: Vec<(acp::SessionConfigId, acp::SessionConfigOptionValue)>,
}

/// The result of background work, for [`AgentThread::handle`].
pub struct ThreadMessage {
    /// Which connection the message belongs to. Messages from before a reload are dropped.
    generation: u64,
    kind: MessageKind,
}

pub type ThreadInbox = mpsc::UnboundedReceiver<ThreadMessage>;

enum MessageKind {
    Connected(Result<(AgentCommand, Connected)>),
    Incoming(Incoming),
    Stderr(String),
    Exited(String),
    SessionOpened {
        connection: ConnectionTo<Agent>,
        result: std::result::Result<SessionSetup, agent_client_protocol::Error>,
    },
    Authenticated {
        method_name: Option<SharedString>,
        result: std::result::Result<(), agent_client_protocol::Error>,
    },
    LoggedOut(std::result::Result<(), agent_client_protocol::Error>),
    ConfigOptionSet {
        previous: Vec<acp::SessionConfigOption>,
        result:
            std::result::Result<acp::SetSessionConfigOptionResponse, agent_client_protocol::Error>,
    },
    ModeSet {
        previous_mode: acp::SessionModeId,
        result: std::result::Result<(), agent_client_protocol::Error>,
    },
    PromptFinished(std::result::Result<acp::PromptResponse, agent_client_protocol::Error>),
}

#[derive(Clone)]
struct MessageSender {
    sender: mpsc::UnboundedSender<ThreadMessage>,
    generation: u64,
}

impl MessageSender {
    /// Hands the message back if the thread is gone.
    fn send(&self, kind: MessageKind) -> std::result::Result<(), Box<MessageKind>> {
        self.sender
            .unbounded_send(ThreadMessage {
                generation: self.generation,
                kind,
            })
            .map_err(|error| Box::new(error.into_inner().kind))
    }
}

enum Incoming {
    Notification(acp::SessionNotification),
    Permission(
        acp::RequestPermissionRequest,
        Responder<acp::RequestPermissionResponse>,
    ),
}

struct Session {
    connection: ConnectionTo<Agent>,
    session_id: acp::SessionId,
}

pub struct AgentThread {
    agent_name: SharedString,
    status: ConnectionStatus,
    entries: Vec<Entry>,
    plan: Vec<PlanItem>,
    /// Settings the agent exposes for this session (model, effort, mode, …).
    config_options: Vec<acp::SessionConfigOption>,
    /// Session modes from agents that predate config options.
    modes: Option<acp::SessionModeState>,
    session_restore: Option<SessionRestore>,
    permission_requests: Vec<PermissionRequest>,
    /// Set once the agent is initialized; kept so a session can be (re)opened after logging in.
    connection: Option<ConnectionTo<Agent>>,
    capabilities: acp::AgentCapabilities,
    auth_methods: Vec<acp::AuthMethod>,
    auth_error: Option<SharedString>,
    command: Option<AgentCommand>,
    cwd: PathBuf,
    previous_session: Option<acp::SessionId>,
    session: Option<Session>,
    usage: Option<ContextUsage>,
    cost: Option<acp::Cost>,
    available_commands: Vec<acp::AvailableCommand>,
    pending_title: Option<String>,
    queued_prompts: Vec<String>,
    turn_started_at: Option<Instant>,
    last_stop_reason: Option<acp::StopReason>,
    turn_error: Option<SharedString>,
    stderr_lines: VecDeque<String>,
    /// False for a connection made only to log in or out (from settings), which never opens a
    /// session.
    opens_session: bool,
    /// The outcome of the last log in or out on such a connection.
    account_notice: Option<SharedString>,
    /// What the agent says about itself when it starts.
    agent_info: Option<acp::Implementation>,
    /// Whether the agent let a session open (logged in) or asked for a login. ACP has no way to
    /// ask directly, so this is the closest status there is. `None` until known.
    logged_in: Option<bool>,
    defaults: SessionDefaults,
    events: Vec<AgentThreadEvent>,
    /// `None` for a thread that never starts.
    runtime: Option<tokio::runtime::Handle>,
    messages: mpsc::UnboundedSender<ThreadMessage>,
    generation: u64,
    /// The agent process, its connection and requests in flight. Dropping the thread (or
    /// reloading it) aborts them, which also stops the agent.
    tasks: JoinSet<()>,
}

impl AgentThread {
    /// Starts the agent and an ACP session in `cwd`. With `previous_session`, the earlier
    /// conversation is loaded (or at least resumed) when the agent supports it. Prompts sent
    /// before the session is ready are queued.
    ///
    /// Pass what arrives on the returned inbox to [`Self::handle`], and drain
    /// [`Self::take_events`] after each call.
    pub fn start(
        runtime: tokio::runtime::Handle,
        agent_name: SharedString,
        command: CommandFuture,
        cwd: PathBuf,
        previous_session: Option<acp::SessionId>,
    ) -> (Self, ThreadInbox) {
        let (mut this, inbox) =
            Self::new(Some(runtime), agent_name, ConnectionStatus::Connecting, cwd);
        this.previous_session = previous_session;
        this.connect_agent(command);
        (this, inbox)
    }

    /// Starts the agent only to log in or out of it, as from its settings. No session is opened,
    /// so the agent starts in a scratch directory.
    pub fn start_for_account(
        runtime: tokio::runtime::Handle,
        agent_name: SharedString,
        command: CommandFuture,
    ) -> (Self, ThreadInbox) {
        let (mut this, inbox) = Self::new(
            Some(runtime),
            agent_name,
            ConnectionStatus::Connecting,
            std::env::temp_dir(),
        );
        this.opens_session = false;
        this.connect_agent(command);
        (this, inbox)
    }

    /// A thread that cannot start, e.g. because its agent is not installed.
    pub fn failed(agent_name: SharedString, error: impl Into<SharedString>) -> Self {
        let (this, _) = Self::new(
            None,
            agent_name,
            ConnectionStatus::Failed(error.into()),
            PathBuf::new(),
        );
        this
    }

    fn new(
        runtime: Option<tokio::runtime::Handle>,
        agent_name: SharedString,
        status: ConnectionStatus,
        cwd: PathBuf,
    ) -> (Self, ThreadInbox) {
        let (messages, inbox) = mpsc::unbounded();
        let this = Self {
            agent_name,
            status,
            entries: Vec::new(),
            plan: Vec::new(),
            config_options: Vec::new(),
            modes: None,
            session_restore: None,
            permission_requests: Vec::new(),
            connection: None,
            capabilities: acp::AgentCapabilities::default(),
            auth_methods: Vec::new(),
            auth_error: None,
            command: None,
            cwd,
            previous_session: None,
            session: None,
            usage: None,
            cost: None,
            available_commands: Vec::new(),
            pending_title: None,
            queued_prompts: Vec::new(),
            turn_started_at: None,
            last_stop_reason: None,
            turn_error: None,
            stderr_lines: VecDeque::new(),
            opens_session: true,
            account_notice: None,
            agent_info: None,
            logged_in: None,
            defaults: SessionDefaults::default(),
            events: Vec::new(),
            runtime,
            messages,
            generation: 0,
            tasks: JoinSet::new(),
        };
        (this, inbox)
    }

    /// The events since the last call, oldest first.
    pub fn take_events(&mut self) -> Vec<AgentThreadEvent> {
        std::mem::take(&mut self.events)
    }

    fn emit(&mut self, event: AgentThreadEvent) {
        self.events.push(event);
    }

    fn sender(&self) -> MessageSender {
        MessageSender {
            sender: self.messages.clone(),
            generation: self.generation,
        }
    }

    /// Runs `work` in the background and passes its result back through the inbox.
    fn spawn(&mut self, work: impl Future<Output = MessageKind> + Send + 'static) {
        let sender = self.sender();
        self.spawn_task(async move {
            sender.send(work.await).ok();
        });
    }

    fn spawn_task(&mut self, task: impl Future<Output = ()> + Send + 'static) {
        let Some(runtime) = &self.runtime else {
            log::error!("{} can't start background work", self.agent_name);
            return;
        };
        while self.tasks.try_join_next().is_some() {}
        self.tasks.spawn_on(task, runtime);
    }

    fn connect_agent(&mut self, command: CommandFuture) {
        let cwd = self.cwd.clone();
        let sender = self.sender();
        self.spawn_task(async move {
            // The agent's own tasks, which stop the agent when dropped.
            let mut agent_tasks = JoinSet::new();
            let result = async {
                let command = command.await?;
                let connected =
                    connect(command.clone(), cwd, sender.clone(), &mut agent_tasks).await?;
                anyhow::Ok((command, connected))
            }
            .await;
            let connected = result.is_ok();
            sender.send(MessageKind::Connected(result)).ok();
            if connected {
                while agent_tasks.join_next().await.is_some() {}
            }
        });
    }

    /// Applies the result of background work.
    pub fn handle(&mut self, message: ThreadMessage) {
        if message.generation != self.generation {
            if let MessageKind::Incoming(Incoming::Permission(_, responder)) = message.kind {
                cancel_permission(responder);
            }
            return;
        }
        match message.kind {
            MessageKind::Connected(Ok((command, connected))) => {
                self.command = Some(command);
                self.connection = Some(connected.connection);
                self.capabilities = connected.capabilities;
                self.auth_methods = connected.auth_methods;
                self.agent_info = connected.agent_info;
                // An account connection opens an empty session too: it is how the login
                // status (and the agent's settings) can be learned over ACP.
                self.open_session();
            }
            MessageKind::Connected(Err(error)) => {
                log::error!("failed to start agent: {error:#}");
                self.fail(format!("{error:#}"));
            }
            MessageKind::Incoming(incoming) => self.handle_incoming(incoming),
            MessageKind::Stderr(line) => self.record_stderr(line),
            MessageKind::Exited(message) => self.fail(message),
            MessageKind::SessionOpened { connection, result } => {
                self.session_opened(connection, result)
            }
            MessageKind::Authenticated {
                method_name,
                result,
            } => match result {
                Ok(()) => {
                    if let Some(method_name) = method_name {
                        self.emit(AgentThreadEvent::LoggedIn(method_name));
                    }
                    if !self.opens_session {
                        self.account_notice = Some("Logged in.".into());
                        self.session = None;
                    }
                    self.open_session();
                }
                Err(error) => {
                    self.status = ConnectionStatus::AuthRequired;
                    self.auth_error = Some(error_message(&error).into());
                }
            },
            MessageKind::LoggedOut(result) => match result {
                Ok(()) => {
                    self.auth_error = None;
                    self.logged_in = Some(false);
                    self.emit(AgentThreadEvent::LoggedOut);
                    if self.opens_session {
                        self.status = ConnectionStatus::AuthRequired;
                    } else {
                        self.account_notice = Some("Logged out.".into());
                        self.session = None;
                    }
                }
                Err(error) => {
                    self.auth_error =
                        Some(format!("Couldn't log out: {}", error_message(&error)).into())
                }
            },
            MessageKind::ConfigOptionSet { previous, result } => match result {
                Ok(response) => self.config_options = response.config_options,
                Err(error) => {
                    log::error!("failed to change an agent setting: {error:?}");
                    self.config_options = previous;
                }
            },
            MessageKind::ModeSet {
                previous_mode,
                result,
            } => {
                if let Err(error) = result {
                    log::error!("failed to change the agent's mode: {error:?}");
                    if let Some(modes) = &mut self.modes {
                        modes.current_mode_id = previous_mode;
                    }
                }
            }
            MessageKind::PromptFinished(result) => {
                match result {
                    Ok(response) => self.last_stop_reason = Some(response.stop_reason),
                    Err(error) => {
                        log::error!("agent prompt failed: {error:?}");
                        self.turn_error = Some(error_message(&error).into());
                    }
                }
                // A finished turn can't still be waiting on a permission answer.
                self.cancel_permission_requests();
                self.set_working(false);
            }
        }
    }

    /// Zed's "Reload Agent": restarts the agent and reopens the session, whose history the
    /// agent replays when it can load sessions.
    pub fn reload(&mut self) {
        let Some(command) = self.command.clone() else {
            return;
        };
        // Aborting the tasks stops the agent process along with its connection.
        self.tasks.abort_all();
        self.generation += 1;
        self.connection = None;
        self.session = None;
        self.entries.clear();
        self.plan.clear();
        self.cancel_permission_requests();
        self.queued_prompts.clear();
        self.auth_error = None;
        self.turn_error = None;
        self.status = ConnectionStatus::Connecting;
        self.set_working(false);
        self.connect_agent(futures::future::ready(Ok(command)).boxed());
    }

    /// Whether the agent advertises ACP's logout method.
    pub fn supports_logout(&self) -> bool {
        self.capabilities.auth.logout.is_some()
    }

    /// Zed's "Reauthenticate": shows the agent's login methods again.
    pub fn reauthenticate(&mut self) {
        if self.auth_methods.is_empty() || self.connection.is_none() {
            return;
        }
        self.status = ConnectionStatus::AuthRequired;
        self.auth_error = None;
        self.account_notice = None;
    }

    /// Logs out of the agent. A thread then asks to log in again, as in Zed.
    pub fn logout(&mut self) {
        let Some(connection) = self.connection.clone() else {
            return;
        };
        if !self.supports_logout() {
            return;
        }
        let request = connection
            .send_request(acp::LogoutRequest::new())
            .block_task();
        self.account_notice = None;
        self.spawn(async move { MessageKind::LoggedOut(request.await.map(|_| ())) });
    }

    /// For a connection made from settings: opens a fresh empty session (sending no prompt) to
    /// learn again whether the agent is logged in, e.g. after logging in through a terminal.
    pub fn check_login(&mut self) {
        if self.opens_session || self.connection.is_none() {
            return;
        }
        self.session = None;
        self.account_notice = None;
        self.open_session();
    }

    /// Whether the agent is logged in, as far as ACP can tell; see the field.
    pub fn logged_in(&self) -> Option<bool> {
        self.logged_in
    }

    /// Settings for new sessions; see [`SessionDefaults`].
    pub fn set_defaults(&mut self, defaults: SessionDefaults) {
        self.defaults = defaults;
    }

    pub fn agent_info(&self) -> Option<&acp::Implementation> {
        self.agent_info.as_ref()
    }

    /// Applies the defaults the session doesn't already match. Values the agent no longer
    /// offers are skipped.
    fn apply_defaults(&mut self) {
        let defaults = self.defaults.clone();
        if let Some(mode) = defaults.mode
            && let Some(modes) = &self.modes
            && modes.current_mode_id != mode
            && modes
                .available_modes
                .iter()
                .any(|available| available.id == mode)
        {
            self.send_mode(mode);
        }
        for (config_id, value) in defaults.config_options {
            let Some(option) = self
                .config_options
                .iter()
                .find(|option| option.id == config_id)
            else {
                continue;
            };
            let applies = match (&option.kind, &value) {
                (
                    acp::SessionConfigKind::Select(select),
                    acp::SessionConfigOptionValue::ValueId { value },
                ) => select.current_value != *value && select_offers(select, value),
                (
                    acp::SessionConfigKind::Boolean(boolean),
                    acp::SessionConfigOptionValue::Boolean { value },
                ) => boolean.current_value != *value,
                _ => false,
            };
            if applies {
                self.send_config_option(config_id, value);
            }
        }
    }

    /// What happened on the last log in or out of an account connection.
    pub fn account_notice(&self) -> Option<&SharedString> {
        self.account_notice.as_ref()
    }

    fn open_session(&mut self) {
        let Some(connection) = self.connection.clone() else {
            return;
        };
        self.status = ConnectionStatus::Connecting;
        self.auth_error = None;
        let opening = open_session(
            connection.clone(),
            self.capabilities.clone(),
            self.cwd.clone(),
            self.previous_session.clone(),
        );
        self.spawn(async move {
            MessageKind::SessionOpened {
                connection,
                result: opening.await,
            }
        });
    }

    fn session_opened(
        &mut self,
        connection: ConnectionTo<Agent>,
        result: std::result::Result<SessionSetup, agent_client_protocol::Error>,
    ) {
        match result {
            Ok(setup) => {
                self.config_options = setup.config_options;
                self.modes = setup.modes;
                self.session_restore = Some(setup.restore);
                self.session = Some(Session {
                    connection,
                    session_id: setup.session_id,
                });
                if matches!(
                    setup.restore,
                    SessionRestore::Loaded | SessionRestore::ResumedWithoutHistory
                ) {
                    self.remember_session();
                }
                self.status = ConnectionStatus::Ready;
                self.logged_in = Some(true);
                if setup.restore == SessionRestore::New && self.opens_session {
                    self.apply_defaults();
                }
                for prompt in std::mem::take(&mut self.queued_prompts) {
                    self.send_to_agent(prompt);
                }
            }
            Err(error) if is_auth_required(&error) => {
                self.status = ConnectionStatus::AuthRequired;
                self.logged_in = Some(false);
                self.set_working(false);
            }
            Err(error) => self.fail(format!("starting a session: {}", error_message(&error))),
        }
    }

    pub fn auth_methods(&self) -> &[acp::AuthMethod] {
        &self.auth_methods
    }

    pub fn auth_error(&self) -> Option<&SharedString> {
        self.auth_error.as_ref()
    }

    /// Logs in with one of the agent's own methods, then opens the session.
    pub fn authenticate(&mut self, method_id: acp::AuthMethodId) {
        let Some(connection) = self.connection.clone() else {
            return;
        };
        let method_name = self
            .auth_methods
            .iter()
            .find(|method| *method.id() == method_id)
            .map(|method| SharedString::from(method.name().to_string()));
        let request = connection
            .send_request(acp::AuthenticateRequest::new(method_id))
            .block_task();
        self.account_notice = None;
        self.status = ConnectionStatus::Connecting;
        self.spawn(async move {
            MessageKind::Authenticated {
                method_name,
                result: request.await.map(|_| ()),
            }
        });
    }

    /// Tries to open the session again, e.g. after logging in through a terminal.
    pub fn retry_session(&mut self) {
        if self.status == ConnectionStatus::AuthRequired {
            self.open_session();
        }
    }

    /// The command to run in a terminal for one of the agent's terminal login methods.
    pub fn terminal_auth_command(&self, method_id: &acp::AuthMethodId) -> Option<AgentCommand> {
        let command = self.command.as_ref()?;
        let method = self.auth_methods.iter().find_map(|method| match method {
            acp::AuthMethod::Terminal(terminal) if &terminal.id == method_id => Some(terminal),
            _ => None,
        })?;
        let mut auth_command = command.clone();
        auth_command.args.extend(method.args.iter().cloned());
        auth_command.env.extend(
            method
                .env
                .iter()
                .map(|(key, value)| (key.clone(), value.clone())),
        );
        Some(auth_command)
    }

    pub fn cwd(&self) -> &PathBuf {
        &self.cwd
    }

    pub fn context_usage(&self) -> Option<ContextUsage> {
        self.usage
    }

    pub fn cost(&self) -> Option<&acp::Cost> {
        self.cost.as_ref()
    }

    pub fn available_commands(&self) -> &[acp::AvailableCommand] {
        &self.available_commands
    }

    pub fn supports_images(&self) -> bool {
        self.capabilities.prompt_capabilities.image
    }

    pub fn clear_plan(&mut self) {
        self.plan.clear();
    }

    pub fn agent_name(&self) -> &SharedString {
        &self.agent_name
    }

    pub fn status(&self) -> &ConnectionStatus {
        &self.status
    }

    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    pub fn session_restore(&self) -> Option<SessionRestore> {
        self.session_restore
    }

    pub fn config_options(&self) -> &[acp::SessionConfigOption] {
        &self.config_options
    }

    pub fn modes(&self) -> Option<&acp::SessionModeState> {
        self.modes.as_ref()
    }

    /// The display name of the model the agent's model selector currently has chosen.
    pub fn model_name(&self) -> Option<String> {
        self.config_options.iter().find_map(|option| {
            if option.category != Some(acp::SessionConfigOptionCategory::Model) {
                return None;
            }
            let acp::SessionConfigKind::Select(select) = &option.kind else {
                return None;
            };
            let current = &select.current_value;
            let name = match &select.options {
                acp::SessionConfigSelectOptions::Ungrouped(options) => options
                    .iter()
                    .find(|choice| choice.value == *current)
                    .map(|choice| choice.name.clone()),
                acp::SessionConfigSelectOptions::Grouped(groups) => groups
                    .iter()
                    .flat_map(|group| &group.options)
                    .find(|choice| choice.value == *current)
                    .map(|choice| choice.name.clone()),
                _ => None,
            };
            Some(name.unwrap_or_else(|| current.0.to_string()))
        })
    }

    /// Changes one of the agent's session settings at the user's request, which also makes it
    /// the agent's default. The new value shows immediately and is reverted if the agent
    /// rejects it.
    pub fn set_config_option(
        &mut self,
        config_id: acp::SessionConfigId,
        value: acp::SessionConfigOptionValue,
    ) {
        if self.session.is_none() {
            return;
        }
        self.emit(AgentThreadEvent::ConfigOptionChanged(
            config_id.clone(),
            value.clone(),
        ));
        self.send_config_option(config_id, value);
    }

    fn send_config_option(
        &mut self,
        config_id: acp::SessionConfigId,
        value: acp::SessionConfigOptionValue,
    ) {
        let Some(session) = &self.session else {
            return;
        };
        let previous = self.config_options.clone();
        if let Some(option) = self
            .config_options
            .iter_mut()
            .find(|option| option.id == config_id)
        {
            match (&mut option.kind, &value) {
                (
                    acp::SessionConfigKind::Select(select),
                    acp::SessionConfigOptionValue::ValueId { value },
                ) => {
                    select.current_value = value.clone();
                }
                (
                    acp::SessionConfigKind::Boolean(boolean),
                    acp::SessionConfigOptionValue::Boolean { value },
                ) => {
                    boolean.current_value = *value;
                }
                _ => {}
            }
        }
        let request =
            acp::SetSessionConfigOptionRequest::new(session.session_id.clone(), config_id, value);
        let response = session.connection.send_request(request).block_task();
        self.spawn(async move {
            MessageKind::ConfigOptionSet {
                previous,
                result: response.await,
            }
        });
    }

    /// Changes the mode at the user's request, which also makes it the agent's default.
    pub fn set_mode(&mut self, mode_id: acp::SessionModeId) {
        if self.session.is_none() || self.modes.is_none() {
            return;
        }
        self.emit(AgentThreadEvent::ModeChanged(mode_id.clone()));
        self.send_mode(mode_id);
    }

    fn send_mode(&mut self, mode_id: acp::SessionModeId) {
        let Some(session) = &self.session else {
            return;
        };
        let Some(modes) = &mut self.modes else {
            return;
        };
        let previous_mode = std::mem::replace(&mut modes.current_mode_id, mode_id.clone());
        let request = acp::SetSessionModeRequest::new(session.session_id.clone(), mode_id);
        let response = session.connection.send_request(request).block_task();
        self.spawn(async move {
            MessageKind::ModeSet {
                previous_mode,
                result: response.await.map(|_| ()),
            }
        });
    }

    pub fn plan(&self) -> &[PlanItem] {
        &self.plan
    }

    pub fn permission_request(&self, tool_call_id: &acp::ToolCallId) -> Option<&PermissionRequest> {
        self.permission_requests
            .iter()
            .find(|request| &request.tool_call_id == tool_call_id)
    }

    /// Permission requests whose tool call isn't shown as an entry.
    pub fn orphan_permission_requests(&self) -> impl Iterator<Item = &PermissionRequest> {
        self.permission_requests.iter().filter(|request| {
            !self.entries.iter().any(|entry| {
                matches!(entry, Entry::ToolCall(tool_call) if tool_call.id == request.tool_call_id)
            })
        })
    }

    pub fn is_working(&self) -> bool {
        self.turn_started_at.is_some()
    }

    pub fn turn_started_at(&self) -> Option<Instant> {
        self.turn_started_at
    }

    pub fn turn_error(&self) -> Option<&SharedString> {
        self.turn_error.as_ref()
    }

    pub fn last_stop_reason(&self) -> Option<&acp::StopReason> {
        self.last_stop_reason.as_ref()
    }

    pub fn send(&mut self, text: String) {
        let text = text.trim().to_string();
        if text.is_empty() || self.is_working() {
            return;
        }
        if !self
            .entries
            .iter()
            .any(|entry| matches!(entry, Entry::UserMessage(_)))
        {
            self.emit(AgentThreadEvent::FirstPrompt(text.clone()));
        }
        self.entries.push(Entry::UserMessage(text.clone()));
        self.turn_error = None;
        match self.status {
            ConnectionStatus::Ready => self.send_to_agent(text),
            ConnectionStatus::Connecting => {
                self.queued_prompts.push(text);
                self.set_working(true);
            }
            // Sent once the user has logged in and the session opens.
            ConnectionStatus::AuthRequired => self.queued_prompts.push(text),
            ConnectionStatus::Failed(_) => {}
        }
    }

    /// Keeps the open session as the one to restore. A new session is only worth restoring once it
    /// has a prompt: agents don't save empty sessions, so loading one later would fail.
    fn remember_session(&mut self) {
        let Some(session) = &self.session else {
            return;
        };
        if self.previous_session.as_ref() == Some(&session.session_id) {
            return;
        }
        let session_id = session.session_id.clone();
        // Later retries should restore this session rather than start another.
        self.previous_session = Some(session_id.clone());
        self.emit(AgentThreadEvent::SessionStarted(session_id));
    }

    fn send_to_agent(&mut self, text: String) {
        self.remember_session();
        let Some(session) = &self.session else {
            return;
        };
        let request = acp::PromptRequest::new(
            session.session_id.clone(),
            vec![acp::ContentBlock::Text(acp::TextContent::new(text))],
        );
        let response = session.connection.send_request(request).block_task();
        self.set_working(true);
        self.spawn(async move { MessageKind::PromptFinished(response.await) });
    }

    /// Asks the agent to stop the current turn.
    pub fn cancel(&mut self) {
        if !self.is_working() {
            return;
        }
        if let Some(session) = &self.session {
            if let Err(error) = session
                .connection
                .send_notification(acp::CancelNotification::new(session.session_id.clone()))
            {
                log::error!("failed to cancel the agent's turn: {error:?}");
            }
        } else {
            self.queued_prompts.clear();
            self.set_working(false);
        }
        self.cancel_permission_requests();
    }

    pub fn respond_to_permission(
        &mut self,
        tool_call_id: &acp::ToolCallId,
        option_id: acp::PermissionOptionId,
    ) {
        let Some(index) = self
            .permission_requests
            .iter()
            .position(|request| &request.tool_call_id == tool_call_id)
        else {
            return;
        };
        let request = self.permission_requests.remove(index);
        if let Err(error) = request
            .responder
            .respond(acp::RequestPermissionResponse::new(
                acp::RequestPermissionOutcome::Selected(acp::SelectedPermissionOutcome::new(
                    option_id,
                )),
            ))
        {
            log::error!("failed to answer the agent's permission request: {error:?}");
        }
    }

    fn cancel_permission_requests(&mut self) {
        for request in self.permission_requests.drain(..) {
            cancel_permission(request.responder);
        }
    }

    fn set_working(&mut self, working: bool) {
        if working == self.is_working() {
            return;
        }
        self.turn_started_at = working.then(Instant::now);
        self.emit(AgentThreadEvent::WorkingChanged(working));
    }

    fn fail(&mut self, error: String) {
        let mut message = error;
        if !self.stderr_lines.is_empty() {
            message.push_str("\n\n");
            message.push_str(&Vec::from(self.stderr_lines.clone()).join("\n"));
        }
        self.status = ConnectionStatus::Failed(message.into());
        self.session = None;
        self.queued_prompts.clear();
        self.set_working(false);
    }

    fn record_stderr(&mut self, line: String) {
        if self.stderr_lines.len() == STDERR_LINES_KEPT {
            self.stderr_lines.pop_front();
        }
        self.stderr_lines.push_back(line);
    }

    fn handle_incoming(&mut self, incoming: Incoming) {
        match incoming {
            Incoming::Notification(notification) => {
                self.apply_update(notification.update);
                if let Some(title) = self.pending_title.take() {
                    self.emit(AgentThreadEvent::TitleChanged(title));
                }
            }
            Incoming::Permission(request, responder) => {
                let tool_call_id = request.tool_call.tool_call_id.clone();
                // Permission requests can describe a tool call we haven't been told about yet.
                self.apply_tool_call_update(request.tool_call.clone());
                let title = request.tool_call.fields.title.clone().unwrap_or_default();
                self.permission_requests.push(PermissionRequest {
                    tool_call_id,
                    title,
                    options: request
                        .options
                        .into_iter()
                        .map(|option| PermissionOption {
                            id: option.option_id,
                            name: option.name,
                            kind: option.kind,
                        })
                        .collect(),
                    responder,
                });
            }
        }
    }

    fn apply_update(&mut self, update: acp::SessionUpdate) {
        match update {
            acp::SessionUpdate::UserMessageChunk(chunk) => self.append_text(
                chunk.content,
                |text| Entry::UserMessage(text),
                |entry| match entry {
                    Entry::UserMessage(text) => Some(text),
                    _ => None,
                },
            ),
            acp::SessionUpdate::AgentMessageChunk(chunk) => self.append_text(
                chunk.content,
                |text| Entry::AgentMessage(text),
                |entry| match entry {
                    Entry::AgentMessage(text) => Some(text),
                    _ => None,
                },
            ),
            acp::SessionUpdate::AgentThoughtChunk(chunk) => self.append_text(
                chunk.content,
                |text| Entry::AgentThought(text),
                |entry| match entry {
                    Entry::AgentThought(text) => Some(text),
                    _ => None,
                },
            ),
            acp::SessionUpdate::ToolCall(tool_call) => self.upsert_tool_call(tool_call),
            acp::SessionUpdate::ToolCallUpdate(update) => self.apply_tool_call_update(update),
            acp::SessionUpdate::Plan(plan) => {
                self.plan = plan
                    .entries
                    .into_iter()
                    .map(|entry| PlanItem {
                        content: entry.content,
                        status: entry.status,
                    })
                    .collect();
                if !self.entries.contains(&Entry::Plan) {
                    self.entries.push(Entry::Plan);
                }
            }
            acp::SessionUpdate::ConfigOptionUpdate(update) => {
                self.config_options = update.config_options;
            }
            acp::SessionUpdate::CurrentModeUpdate(update) => {
                if let Some(modes) = &mut self.modes {
                    modes.current_mode_id = update.current_mode_id;
                }
            }
            acp::SessionUpdate::UsageUpdate(update) => {
                self.usage = Some(ContextUsage {
                    used: update.used,
                    size: update.size,
                });
                if update.cost.is_some() {
                    self.cost = update.cost;
                }
            }
            acp::SessionUpdate::AvailableCommandsUpdate(update) => {
                self.available_commands = update.available_commands;
            }
            acp::SessionUpdate::SessionInfoUpdate(update) => {
                if let agent_client_protocol::schema::MaybeUndefined::Value(title) = update.title {
                    self.pending_title = Some(title);
                }
            }
            _ => {}
        }
    }

    fn append_text(
        &mut self,
        content: acp::ContentBlock,
        new_entry: impl FnOnce(String) -> Entry,
        existing_text: impl FnOnce(&mut Entry) -> Option<&mut String>,
    ) {
        let acp::ContentBlock::Text(text) = content else {
            return;
        };
        if let Some(existing) = self.entries.last_mut().and_then(existing_text) {
            existing.push_str(&text.text);
        } else {
            self.entries.push(new_entry(text.text));
        }
    }

    fn upsert_tool_call(&mut self, tool_call: acp::ToolCall) {
        let mut entry = ToolCall {
            id: tool_call.tool_call_id,
            title: tool_call.title,
            kind: tool_call.kind,
            status: tool_call.status,
            text: Vec::new(),
            diffs: Vec::new(),
            locations: tool_call
                .locations
                .into_iter()
                .map(|location| location.path)
                .collect(),
            raw_input: tool_call.raw_input.as_ref().and_then(raw_input_text),
        };
        set_tool_call_content(&mut entry, tool_call.content);
        if let Some(existing) = self.tool_call_mut(&entry.id) {
            *existing = entry;
        } else {
            self.entries.push(Entry::ToolCall(entry));
        }
    }

    fn apply_tool_call_update(&mut self, update: acp::ToolCallUpdate) {
        let fields = update.fields;
        let Some(existing) = self.tool_call_mut(&update.tool_call_id) else {
            let mut entry = ToolCall {
                id: update.tool_call_id,
                title: fields.title.unwrap_or_default(),
                kind: fields.kind.unwrap_or_default(),
                status: fields.status.unwrap_or_default(),
                text: Vec::new(),
                diffs: Vec::new(),
                locations: fields
                    .locations
                    .unwrap_or_default()
                    .into_iter()
                    .map(|location| location.path)
                    .collect(),
                raw_input: fields.raw_input.as_ref().and_then(raw_input_text),
            };
            set_tool_call_content(&mut entry, fields.content.unwrap_or_default());
            self.entries.push(Entry::ToolCall(entry));
            return;
        };
        if let Some(title) = fields.title {
            existing.title = title;
        }
        if let Some(kind) = fields.kind {
            existing.kind = kind;
        }
        if let Some(status) = fields.status {
            existing.status = status;
        }
        if let Some(locations) = fields.locations {
            existing.locations = locations
                .into_iter()
                .map(|location| location.path)
                .collect();
        }
        if let Some(content) = fields.content {
            set_tool_call_content(existing, content);
        }
        if let Some(raw_input) = fields.raw_input.as_ref() {
            existing.raw_input = raw_input_text(raw_input);
        }
    }

    fn tool_call_mut(&mut self, id: &acp::ToolCallId) -> Option<&mut ToolCall> {
        self.entries.iter_mut().rev().find_map(|entry| match entry {
            Entry::ToolCall(tool_call) if &tool_call.id == id => Some(tool_call),
            _ => None,
        })
    }
}

/// Formats a tool's raw input the way Zed does: plain values as text, anything else as a
/// pretty-printed JSON code block.
fn raw_input_text(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::Null => None,
        serde_json::Value::Bool(value) => Some(value.to_string()),
        serde_json::Value::Number(value) => Some(value.to_string()),
        serde_json::Value::String(value) => Some(value.clone()),
        value => {
            let pretty = serde_json::to_string_pretty(value).unwrap_or_else(|_| value.to_string());
            Some(format!("```json\n{pretty}\n```"))
        }
    }
}

fn set_tool_call_content(tool_call: &mut ToolCall, content: Vec<acp::ToolCallContent>) {
    tool_call.text.clear();
    tool_call.diffs.clear();
    for item in content {
        match item {
            acp::ToolCallContent::Content(content) => {
                if let acp::ContentBlock::Text(text) = content.content {
                    tool_call.text.push(text.text);
                }
            }
            acp::ToolCallContent::Diff(diff) => tool_call.diffs.push(FileDiff {
                path: diff.path,
                old_text: diff.old_text,
                new_text: diff.new_text,
            }),
            _ => {}
        }
    }
}

fn error_message(error: &agent_client_protocol::Error) -> String {
    match &error.data {
        Some(data) => format!("{} ({data})", error.message),
        None => error.message.clone(),
    }
}

fn cancel_permission(responder: Responder<acp::RequestPermissionResponse>) {
    responder
        .respond(acp::RequestPermissionResponse::new(
            acp::RequestPermissionOutcome::Cancelled,
        ))
        .ok();
}

/// Spawns the agent and wires up and initializes the ACP connection. The agent's process and
/// transport run in `agent_tasks`, and report through `sender`.
async fn connect(
    command: AgentCommand,
    cwd: PathBuf,
    sender: MessageSender,
    agent_tasks: &mut JoinSet<()>,
) -> Result<Connected> {
    let mut child = tokio::process::Command::new(&command.path)
        .args(&command.args)
        .envs(&command.env)
        .current_dir(&cwd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .with_context(|| format!("starting {}", command.path.display()))?;
    let stdin = child.stdin.take().context("agent has no stdin")?;
    let stdout = child.stdout.take().context("agent has no stdout")?;
    let stderr = child.stderr.take().context("agent has no stderr")?;

    let incoming_lines = lines(stdout).boxed();
    let outgoing_lines = Box::pin(futures::sink::unfold(
        stdin,
        async move |mut writer, line: String| {
            let mut bytes = line.into_bytes();
            bytes.push(b'\n');
            writer.write_all(&bytes).await?;
            writer.flush().await?;
            Ok::<_, std::io::Error>(writer)
        },
    ));

    let (connection_sender, connection_receiver) = oneshot::channel();
    let connection_future = {
        let notification_sender = sender.clone();
        let permission_sender = sender.clone();
        Client
            .builder()
            .name("agentZ")
            .on_receive_notification(
                async move |notification: acp::SessionNotification, _connection| {
                    notification_sender
                        .send(MessageKind::Incoming(Incoming::Notification(notification)))
                        .ok();
                    Ok(())
                },
                agent_client_protocol::on_receive_notification!(),
            )
            .on_receive_request(
                async move |request: acp::RequestPermissionRequest,
                            responder: Responder<acp::RequestPermissionResponse>,
                            _connection| {
                    if let Err(message) = permission_sender.send(MessageKind::Incoming(
                        Incoming::Permission(request, responder),
                    )) && let MessageKind::Incoming(Incoming::Permission(_, responder)) =
                        *message
                    {
                        responder.respond(acp::RequestPermissionResponse::new(
                            acp::RequestPermissionOutcome::Cancelled,
                        ))?;
                    }
                    Ok(())
                },
                agent_client_protocol::on_receive_request!(),
            )
            .connect_with(
                Lines::new(outgoing_lines, incoming_lines),
                move |connection: ConnectionTo<Agent>| async move {
                    connection_sender.send(connection).ok();
                    // Keep the connection open until the transport closes.
                    futures::future::pending::<Result<(), agent_client_protocol::Error>>().await
                },
            )
    };

    agent_tasks.spawn(async move {
        if let Err(error) = connection_future.await {
            log::error!("ACP connection error: {error:?}");
        }
    });
    agent_tasks.spawn({
        let sender = sender.clone();
        async move {
            let mut stderr_lines = lines(stderr).boxed();
            while let Some(Ok(line)) = stderr_lines.next().await {
                log::warn!("agent stderr: {line}");
                if sender.send(MessageKind::Stderr(line)).is_err() {
                    break;
                }
            }
        }
    });
    agent_tasks.spawn(async move {
        let message = match child.wait().await {
            Ok(status) => format!("The agent exited ({status})."),
            Err(error) => format!("The agent stopped: {error}"),
        };
        sender.send(MessageKind::Exited(message)).ok();
    });

    let connection = connection_receiver
        .await
        .map_err(|_| anyhow!("the agent closed the connection before it was ready"))?;

    let version = env!("CARGO_PKG_VERSION");
    let initialize = connection
        .send_request(
            acp::InitializeRequest::new(ProtocolVersion::V1)
                .client_capabilities(acp::ClientCapabilities::new())
                .client_info(acp::Implementation::new("agentZ", version)),
        )
        .block_task()
        .map(|result| result.map_err(|error| anyhow!(error_message(&error))));
    let initialize_response = initialize.await.context("initializing the agent")?;
    anyhow::ensure!(
        initialize_response.protocol_version >= ProtocolVersion::V1,
        "the agent speaks an unsupported ACP version"
    );

    Ok(Connected {
        connection,
        capabilities: initialize_response.agent_capabilities,
        auth_methods: initialize_response.auth_methods,
        agent_info: initialize_response.agent_info,
    })
}

/// The lines of an agent's output stream.
fn lines(
    reader: impl tokio::io::AsyncRead + Send + Unpin + 'static,
) -> impl futures::Stream<Item = std::io::Result<String>> + Send + 'static {
    futures::stream::unfold(
        tokio::io::BufReader::new(reader).lines(),
        async |mut lines| match lines.next_line().await {
            Ok(Some(line)) => Some((Ok(line), lines)),
            Ok(None) => None,
            Err(error) => Some((Err(error), lines)),
        },
    )
}

fn select_offers(select: &acp::SessionConfigSelect, value: &acp::SessionConfigValueId) -> bool {
    match &select.options {
        acp::SessionConfigSelectOptions::Ungrouped(options) => {
            options.iter().any(|option| option.value == *value)
        }
        acp::SessionConfigSelectOptions::Grouped(groups) => groups
            .iter()
            .flat_map(|group| &group.options)
            .any(|option| option.value == *value),
        _ => false,
    }
}

struct Connected {
    connection: ConnectionTo<Agent>,
    capabilities: acp::AgentCapabilities,
    auth_methods: Vec<acp::AuthMethod>,
    agent_info: Option<acp::Implementation>,
}

struct SessionSetup {
    session_id: acp::SessionId,
    config_options: Vec<acp::SessionConfigOption>,
    modes: Option<acp::SessionModeState>,
    restore: SessionRestore,
}

/// Opens the thread's session: loads or resumes `previous_session` when the agent supports it
/// (in that order, like Zed), otherwise starts a new one.
async fn open_session(
    connection: ConnectionTo<Agent>,
    capabilities: acp::AgentCapabilities,
    cwd: PathBuf,
    previous_session: Option<acp::SessionId>,
) -> std::result::Result<SessionSetup, agent_client_protocol::Error> {
    let had_previous_session = previous_session.is_some();
    if let Some(session_id) = previous_session {
        if capabilities.load_session {
            match connection
                .send_request(acp::LoadSessionRequest::new(
                    session_id.clone(),
                    cwd.clone(),
                ))
                .block_task()
                .await
            {
                Ok(response) => {
                    return Ok(SessionSetup {
                        session_id,
                        config_options: response.config_options.unwrap_or_default(),
                        modes: response.modes,
                        restore: SessionRestore::Loaded,
                    });
                }
                Err(error) if is_auth_required(&error) => return Err(error),
                Err(error) => {
                    log::warn!(
                        "couldn't load session {session_id}: {}",
                        error_message(&error)
                    )
                }
            }
        } else if capabilities.session_capabilities.resume.is_some() {
            match connection
                .send_request(acp::ResumeSessionRequest::new(
                    session_id.clone(),
                    cwd.clone(),
                ))
                .block_task()
                .await
            {
                Ok(response) => {
                    return Ok(SessionSetup {
                        session_id,
                        config_options: response.config_options.unwrap_or_default(),
                        modes: response.modes,
                        restore: SessionRestore::ResumedWithoutHistory,
                    });
                }
                Err(error) if is_auth_required(&error) => return Err(error),
                Err(error) => {
                    log::warn!(
                        "couldn't resume session {session_id}: {}",
                        error_message(&error)
                    )
                }
            }
        }
    }

    let new_session = connection
        .send_request(acp::NewSessionRequest::new(cwd))
        .block_task()
        .await?;
    Ok(SessionSetup {
        session_id: new_session.session_id,
        config_options: new_session.config_options.unwrap_or_default(),
        modes: new_session.modes,
        restore: if had_previous_session {
            SessionRestore::Unavailable
        } else {
            SessionRestore::New
        },
    })
}

fn is_auth_required(error: &agent_client_protocol::Error) -> bool {
    error.code == acp::ErrorCode::AuthRequired
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diff_line_counts() {
        let diff = FileDiff {
            path: PathBuf::from("a.rs"),
            old_text: Some("a\nb\nc\nd\n".into()),
            new_text: "a\nB\nB2\nc\nd\n".into(),
        };
        assert_eq!(diff.line_counts(), (2, 1));
        assert_eq!(diff.changed_lines(), (vec!["b"], vec!["B", "B2"]));
        assert_eq!(
            diff.hunk(1),
            vec![
                (DiffLineKind::Context, "a"),
                (DiffLineKind::Removed, "b"),
                (DiffLineKind::Added, "B"),
                (DiffLineKind::Added, "B2"),
                (DiffLineKind::Context, "c"),
            ]
        );
        let created = FileDiff {
            path: PathBuf::from("b.rs"),
            old_text: None,
            new_text: "x\ny\n".into(),
        };
        assert_eq!(created.line_counts(), (2, 0));
    }

    /// A thread under test, with its inbox pumped by [`Self::wait_until`].
    struct TestThread {
        thread: AgentThread,
        inbox: ThreadInbox,
        events: Vec<AgentThreadEvent>,
    }

    impl TestThread {
        fn new((thread, inbox): (AgentThread, ThreadInbox)) -> Self {
            Self {
                thread,
                inbox,
                events: Vec::new(),
            }
        }

        fn update(&mut self, change: impl FnOnce(&mut AgentThread)) {
            change(&mut self.thread);
            self.events.extend(self.thread.take_events());
        }

        async fn wait_until(&mut self, done: impl Fn(&AgentThread) -> bool) {
            let waited = tokio::time::timeout(std::time::Duration::from_secs(5), async {
                while !done(&self.thread) {
                    let message = self.inbox.next().await.expect("the inbox closed");
                    self.thread.handle(message);
                    self.events.extend(self.thread.take_events());
                }
            })
            .await;
            if waited.is_err() {
                panic!("timed out; status {:?}", self.thread.status());
            }
        }
    }

    fn mock_agent(args: &[String]) -> Option<AgentCommand> {
        let Some(python) = which_python() else {
            eprintln!("skipping: python3 not found");
            return None;
        };
        let script = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("test_support/mock_agent.py");
        let mut command_args = vec![script.to_string_lossy().into_owned()];
        command_args.extend(args.iter().cloned());
        Some(AgentCommand {
            path: python,
            args: command_args,
            env: Default::default(),
        })
    }

    fn ready(command: AgentCommand) -> CommandFuture {
        futures::future::ready(Ok(command)).boxed()
    }

    fn start(command: AgentCommand, previous_session: Option<acp::SessionId>) -> TestThread {
        TestThread::new(AgentThread::start(
            tokio::runtime::Handle::current(),
            "Mock".into(),
            ready(command),
            std::env::temp_dir(),
            previous_session,
        ))
    }

    /// Logging in and out from settings, against `test_support/mock_agent.py`.
    #[tokio::test(flavor = "multi_thread")]
    async fn logs_in_and_out_of_an_account() {
        let Some(command) = mock_agent(&[]) else {
            return;
        };
        let mut account = TestThread::new(AgentThread::start_for_account(
            tokio::runtime::Handle::current(),
            "Mock".into(),
            ready(command),
        ));

        account
            .wait_until(|account| account.status() == &ConnectionStatus::Ready)
            .await;
        assert!(account.thread.supports_logout());
        assert_eq!(account.thread.auth_methods().len(), 1);
        assert_eq!(
            account.thread.logged_in(),
            Some(true),
            "the mock opens sessions freely"
        );
        assert_eq!(account.thread.config_options().len(), 4);

        account.update(|account| account.authenticate(acp::AuthMethodId::new("mock-login")));
        account
            .wait_until(|account| {
                account.account_notice().is_some() && account.status() == &ConnectionStatus::Ready
            })
            .await;
        assert_eq!(
            account.thread.account_notice().map(|n| n.as_ref()),
            Some("Logged in.")
        );
        assert_eq!(account.thread.logged_in(), Some(true));
        assert!(
            account
                .events
                .iter()
                .any(|event| matches!(event, AgentThreadEvent::LoggedIn(_)))
        );

        account.update(|account| account.logout());
        account
            .wait_until(|account| {
                account.account_notice().map(|n| n.as_ref()) == Some("Logged out.")
            })
            .await;
        assert_eq!(account.thread.logged_in(), Some(false));

        account.update(|account| account.check_login());
        account
            .wait_until(|account| account.logged_in() == Some(true))
            .await;
    }

    /// New sessions start with the agent's saved defaults, against `test_support/mock_agent.py`.
    #[tokio::test(flavor = "multi_thread")]
    async fn applies_defaults_to_new_sessions() {
        let Some(command) = mock_agent(&[]) else {
            return;
        };
        let mut thread = start(command, None);
        thread.update(|thread| {
            thread.set_defaults(SessionDefaults {
                mode: None,
                config_options: vec![
                    (
                        acp::SessionConfigId::new("model"),
                        acp::SessionConfigOptionValue::value_id("opus"),
                    ),
                    (
                        acp::SessionConfigId::new("fast"),
                        acp::SessionConfigOptionValue::boolean(true),
                    ),
                    // No longer offered by the agent, so it's skipped.
                    (
                        acp::SessionConfigId::new("effort"),
                        acp::SessionConfigOptionValue::value_id("extreme"),
                    ),
                ],
            })
        });
        let current = |thread: &AgentThread, id: &str| {
            thread
                .config_options()
                .iter()
                .find(|option| option.id.0.as_ref() == id)
                .map(|option| match &option.kind {
                    acp::SessionConfigKind::Select(select) => select.current_value.0.to_string(),
                    acp::SessionConfigKind::Boolean(boolean) => boolean.current_value.to_string(),
                    _ => String::new(),
                })
        };
        thread
            .wait_until(|thread| {
                thread.status() == &ConnectionStatus::Ready
                    && current(thread, "model").as_deref() == Some("opus")
                    && current(thread, "fast").as_deref() == Some("true")
            })
            .await;
        assert_eq!(current(&thread.thread, "effort").as_deref(), Some("medium"));
    }

    /// Zed's Log Out and Reload Agent on a thread, against `test_support/mock_agent.py`.
    #[tokio::test(flavor = "multi_thread")]
    async fn logs_out_and_reloads_a_thread() {
        let Some(command) = mock_agent(&[]) else {
            return;
        };
        let mut thread = start(command, None);

        thread
            .wait_until(|thread| thread.status() == &ConnectionStatus::Ready)
            .await;
        thread.update(|thread| thread.logout());
        thread
            .wait_until(|thread| thread.status() == &ConnectionStatus::AuthRequired)
            .await;
        thread.update(|thread| thread.authenticate(acp::AuthMethodId::new("mock-login")));
        thread
            .wait_until(|thread| thread.status() == &ConnectionStatus::Ready)
            .await;

        thread.update(|thread| thread.reload());
        assert_eq!(thread.thread.status(), &ConnectionStatus::Connecting);
        thread
            .wait_until(|thread| thread.status() == &ConnectionStatus::Ready)
            .await;
    }

    /// Runs the real process and protocol plumbing against `test_support/mock_agent.py`.
    #[tokio::test(flavor = "multi_thread")]
    async fn talks_to_a_real_agent_process() {
        let Some(command) = mock_agent(&[]) else {
            return;
        };
        let mut thread = start(command, None);

        thread
            .wait_until(|thread| thread.status() == &ConnectionStatus::Ready)
            .await;
        thread.update(|thread| thread.send("hello".into()));
        thread
            .wait_until(|thread| !thread.is_working() && thread.entries().len() >= 3)
            .await;
        let entries = thread.thread.entries();
        assert_eq!(entries[0], Entry::UserMessage("hello".into()));
        assert_eq!(entries[1], Entry::AgentMessage("Echo: hello".into()));
        assert!(matches!(&entries[2], Entry::ToolCall(call) if call.title == "Read README.md"));
        assert_eq!(
            thread.thread.last_stop_reason(),
            Some(&acp::StopReason::EndTurn)
        );
        assert!(thread.events.iter().any(
            |event| matches!(event, AgentThreadEvent::FirstPrompt(prompt) if prompt == "hello")
        ));

        let ids: Vec<_> = thread
            .thread
            .config_options()
            .iter()
            .map(|option| option.id.0.to_string())
            .collect();
        assert_eq!(ids, vec!["mode", "model", "effort", "fast"]);
        thread.update(|thread| {
            thread.set_config_option(
                acp::SessionConfigId::new("model"),
                acp::SessionConfigOptionValue::value_id("opus"),
            )
        });
        let model_is = |thread: &AgentThread, expected: &str| {
            thread.config_options().iter().any(|option| {
                option.id.0.as_ref() == "model"
                    && matches!(&option.kind, acp::SessionConfigKind::Select(select)
                        if select.current_value.0.as_ref() == expected)
            })
        };
        assert!(model_is(&thread.thread, "opus"));
        thread.wait_until(|thread| model_is(thread, "opus")).await;

        thread.update(|thread| thread.send("permission".into()));
        let tool_call_id = acp::ToolCallId::new("call-2");
        thread
            .wait_until(|thread| thread.permission_request(&tool_call_id).is_some())
            .await;
        thread.update(|thread| {
            let request = thread.permission_request(&tool_call_id).expect("request");
            assert_eq!(request.options.len(), 2);
            let allow = request.options[0].id.clone();
            thread.respond_to_permission(&tool_call_id, allow);
        });
        thread.wait_until(|thread| !thread.is_working()).await;
        let last_message = thread
            .thread
            .entries()
            .iter()
            .rev()
            .find_map(|entry| match entry {
                Entry::AgentMessage(text) => Some(text.clone()),
                _ => None,
            });
        assert_eq!(
            last_message.as_deref(),
            Some("Echo: permission (chose allow)")
        );
    }

    /// A second thread given the first one's session id gets the conversation replayed.
    #[tokio::test(flavor = "multi_thread")]
    async fn reloads_previous_session() {
        let history_dir = tempfile::tempdir().expect("temp dir");
        let history_file = history_dir
            .path()
            .join("history.json")
            .to_string_lossy()
            .into_owned();
        let Some(command) = mock_agent(&[history_file]) else {
            return;
        };
        let saved_sessions = |thread: &TestThread| -> Vec<acp::SessionId> {
            thread
                .events
                .iter()
                .filter_map(|event| match event {
                    AgentThreadEvent::SessionStarted(session_id) => Some(session_id.clone()),
                    _ => None,
                })
                .collect()
        };

        let mut first = start(command.clone(), None);
        first
            .wait_until(|thread| thread.status() == &ConnectionStatus::Ready)
            .await;
        // Agents don't keep a session without a prompt, so it isn't worth restoring yet.
        assert!(saved_sessions(&first).is_empty());
        first.update(|thread| thread.send("hello".into()));
        first
            .wait_until(|thread| !thread.is_working() && thread.entries().len() >= 3)
            .await;
        assert_eq!(saved_sessions(&first), [acp::SessionId::new("session-1")]);
        assert_eq!(first.thread.session_restore(), Some(SessionRestore::New));

        let mut second = start(command, Some(acp::SessionId::new("session-1")));
        second
            .wait_until(|thread| thread.status() == &ConnectionStatus::Ready)
            .await;
        assert_eq!(
            second.thread.session_restore(),
            Some(SessionRestore::Loaded)
        );
        assert_eq!(
            second.thread.entries()[0],
            Entry::UserMessage("hello".into())
        );
        assert_eq!(
            second.thread.entries()[1],
            Entry::AgentMessage("Echo: hello".into())
        );
    }

    fn which_python() -> Option<PathBuf> {
        let path = std::env::var_os("PATH")?;
        std::env::split_paths(&path)
            .map(|dir| dir.join("python3"))
            .find(|candidate| candidate.is_file())
    }

    #[test]
    fn streams_updates_into_entries() {
        let mut thread = AgentThread::failed("Test".into(), "not started");
        let chunk = |text: &str| {
            acp::ContentChunk::new(acp::ContentBlock::Text(acp::TextContent::new(text)))
        };
        thread.apply_update(acp::SessionUpdate::AgentMessageChunk(chunk("Hel")));
        thread.apply_update(acp::SessionUpdate::AgentMessageChunk(chunk("lo")));
        thread.apply_update(acp::SessionUpdate::ToolCall(
            acp::ToolCall::new("call-1", "Read src/main.rs").kind(acp::ToolKind::Read),
        ));
        thread.apply_update(acp::SessionUpdate::ToolCallUpdate(
            acp::ToolCallUpdate::new(
                "call-1",
                acp::ToolCallUpdateFields::new().status(acp::ToolCallStatus::Completed),
            ),
        ));
        thread.apply_update(acp::SessionUpdate::AgentMessageChunk(chunk("Done")));

        assert_eq!(thread.entries().len(), 3);
        assert_eq!(thread.entries()[0], Entry::AgentMessage("Hello".into()));
        match &thread.entries()[1] {
            Entry::ToolCall(tool_call) => {
                assert_eq!(tool_call.title, "Read src/main.rs");
                assert_eq!(tool_call.status, acp::ToolCallStatus::Completed);
            }
            other => panic!("expected a tool call, got {other:?}"),
        }
        assert_eq!(thread.entries()[2], Entry::AgentMessage("Done".into()));
    }
}
