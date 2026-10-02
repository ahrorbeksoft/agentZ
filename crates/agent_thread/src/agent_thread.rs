//! One conversation with an ACP agent: starts the agent process, runs an ACP session in the
//! project folder, and keeps the conversation (messages, tool calls, plan, permission
//! requests) as it streams in.
//!
//! The connection setup follows Zed's `agent_servers::acp`: the SDK's handlers must be `Send`,
//! so they forward everything onto a channel that is processed on the foreground thread.

use std::collections::VecDeque;
use std::path::PathBuf;
use std::process::Stdio;
use std::time::Instant;

use agent_client_protocol::schema::ProtocolVersion;
use agent_client_protocol::schema::v1 as acp;
use agent_client_protocol::{Agent, Client, ConnectionTo, Lines, Responder};
use anyhow::{Context as _, Result, anyhow};
use futures::channel::{mpsc, oneshot};
use futures::{AsyncBufReadExt as _, AsyncWriteExt as _, FutureExt as _, StreamExt as _};
use gpui::{AppContext as _, Context, EventEmitter, SharedString, Task};
use registry::AgentCommand;

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
    /// The ACP session the thread is talking to; store it to restore the thread later.
    SessionStarted(acp::SessionId),
    /// The first prompt of a new conversation was sent; useful as a title.
    FirstPrompt(String),
    /// The agent named the session.
    TitleChanged(String),
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
    _tasks: Vec<Task<()>>,
}

impl EventEmitter<AgentThreadEvent> for AgentThread {}

impl AgentThread {
    /// Starts the agent and an ACP session in `cwd`. With `previous_session`, the earlier
    /// conversation is loaded (or at least resumed) when the agent supports it. Prompts sent
    /// before the session is ready are queued.
    pub fn start(
        agent_name: SharedString,
        command: Task<Result<AgentCommand>>,
        cwd: PathBuf,
        previous_session: Option<acp::SessionId>,
        cx: &mut Context<Self>,
    ) -> Self {
        let connect = cx.spawn({
            let cwd = cwd.clone();
            async move |this, cx| {
                let result = async {
                    let command = command.await?;
                    let connected = connect(command.clone(), cwd, this.clone(), cx).await?;
                    anyhow::Ok((command, connected))
                }
                .await;
                this.update(cx, |this, cx| match result {
                    Ok((command, connected)) => {
                        this.command = Some(command);
                        this.connection = Some(connected.connection);
                        this.capabilities = connected.capabilities;
                        this.auth_methods = connected.auth_methods;
                        this.open_session(cx);
                    }
                    Err(error) => {
                        log::error!("failed to start agent: {error:#}");
                        this.fail(format!("{error:#}"), cx);
                    }
                })
                .ok();
            }
        });

        let mut this = Self::new(agent_name, ConnectionStatus::Connecting, cwd);
        this.previous_session = previous_session;
        this._tasks.push(connect);
        this
    }

    /// A thread that cannot start, e.g. because its agent is not installed.
    pub fn failed(agent_name: SharedString, error: impl Into<SharedString>) -> Self {
        Self::new(
            agent_name,
            ConnectionStatus::Failed(error.into()),
            PathBuf::new(),
        )
    }

    fn new(agent_name: SharedString, status: ConnectionStatus, cwd: PathBuf) -> Self {
        Self {
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
            _tasks: Vec::new(),
        }
    }

    fn open_session(&mut self, cx: &mut Context<Self>) {
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
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = opening.await;
            this.update(cx, |this, cx| match result {
                Ok(setup) => {
                    this.config_options = setup.config_options;
                    this.modes = setup.modes;
                    this.session_restore = Some(setup.restore);
                    // Later retries should restore this session rather than start another.
                    this.previous_session = Some(setup.session_id.clone());
                    cx.emit(AgentThreadEvent::SessionStarted(setup.session_id.clone()));
                    this.session = Some(Session {
                        connection,
                        session_id: setup.session_id,
                    });
                    this.status = ConnectionStatus::Ready;
                    for prompt in std::mem::take(&mut this.queued_prompts) {
                        this.send_to_agent(prompt, cx);
                    }
                    cx.notify();
                }
                Err(error) if is_auth_required(&error) => {
                    this.status = ConnectionStatus::AuthRequired;
                    this.set_working(false, cx);
                    cx.notify();
                }
                Err(error) => {
                    this.fail(format!("starting a session: {}", error_message(&error)), cx)
                }
            })
            .ok();
        })
        .detach();
    }

    pub fn auth_methods(&self) -> &[acp::AuthMethod] {
        &self.auth_methods
    }

    pub fn auth_error(&self) -> Option<&SharedString> {
        self.auth_error.as_ref()
    }

    /// Logs in with one of the agent's own methods, then opens the session.
    pub fn authenticate(&mut self, method_id: acp::AuthMethodId, cx: &mut Context<Self>) {
        let Some(connection) = self.connection.clone() else {
            return;
        };
        let request = connection
            .send_request(acp::AuthenticateRequest::new(method_id))
            .block_task();
        self.status = ConnectionStatus::Connecting;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = request.await;
            this.update(cx, |this, cx| match result {
                Ok(_) => this.open_session(cx),
                Err(error) => {
                    this.status = ConnectionStatus::AuthRequired;
                    this.auth_error = Some(error_message(&error).into());
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    /// Tries to open the session again, e.g. after logging in through a terminal.
    pub fn retry_session(&mut self, cx: &mut Context<Self>) {
        if self.status == ConnectionStatus::AuthRequired {
            self.open_session(cx);
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

    pub fn clear_plan(&mut self, cx: &mut Context<Self>) {
        self.plan.clear();
        cx.notify();
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

    /// Changes one of the agent's session settings. The new value shows immediately and is
    /// reverted if the agent rejects it.
    pub fn set_config_option(
        &mut self,
        config_id: acp::SessionConfigId,
        value: acp::SessionConfigOptionValue,
        cx: &mut Context<Self>,
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
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = response.await;
            this.update(cx, |this, cx| {
                match result {
                    Ok(response) => this.config_options = response.config_options,
                    Err(error) => {
                        log::error!("failed to change an agent setting: {error:?}");
                        this.config_options = previous;
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub fn set_mode(&mut self, mode_id: acp::SessionModeId, cx: &mut Context<Self>) {
        let Some(session) = &self.session else {
            return;
        };
        let Some(modes) = &mut self.modes else {
            return;
        };
        let previous_mode = std::mem::replace(&mut modes.current_mode_id, mode_id.clone());
        let request = acp::SetSessionModeRequest::new(session.session_id.clone(), mode_id);
        let response = session.connection.send_request(request).block_task();
        cx.notify();
        cx.spawn(async move |this, cx| {
            if let Err(error) = response.await {
                log::error!("failed to change the agent's mode: {error:?}");
                this.update(cx, |this, cx| {
                    if let Some(modes) = &mut this.modes {
                        modes.current_mode_id = previous_mode;
                    }
                    cx.notify();
                })
                .ok();
            }
        })
        .detach();
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

    pub fn send(&mut self, text: String, cx: &mut Context<Self>) {
        let text = text.trim().to_string();
        if text.is_empty() || self.is_working() {
            return;
        }
        if !self
            .entries
            .iter()
            .any(|entry| matches!(entry, Entry::UserMessage(_)))
        {
            cx.emit(AgentThreadEvent::FirstPrompt(text.clone()));
        }
        self.entries.push(Entry::UserMessage(text.clone()));
        self.turn_error = None;
        match self.status {
            ConnectionStatus::Ready => self.send_to_agent(text, cx),
            ConnectionStatus::Connecting => {
                self.queued_prompts.push(text);
                self.set_working(true, cx);
            }
            // Sent once the user has logged in and the session opens.
            ConnectionStatus::AuthRequired => self.queued_prompts.push(text),
            ConnectionStatus::Failed(_) => {}
        }
        cx.notify();
    }

    fn send_to_agent(&mut self, text: String, cx: &mut Context<Self>) {
        let Some(session) = &self.session else {
            return;
        };
        let request = acp::PromptRequest::new(
            session.session_id.clone(),
            vec![acp::ContentBlock::Text(acp::TextContent::new(text))],
        );
        let response = session.connection.send_request(request).block_task();
        self.set_working(true, cx);
        cx.spawn(async move |this, cx| {
            let result = response.await;
            this.update(cx, |this, cx| {
                match result {
                    Ok(response) => this.last_stop_reason = Some(response.stop_reason),
                    Err(error) => {
                        log::error!("agent prompt failed: {error:?}");
                        this.turn_error = Some(error_message(&error).into());
                    }
                }
                // A finished turn can't still be waiting on a permission answer.
                for request in this.permission_requests.drain(..) {
                    request
                        .responder
                        .respond(acp::RequestPermissionResponse::new(
                            acp::RequestPermissionOutcome::Cancelled,
                        ))
                        .ok();
                }
                this.set_working(false, cx);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// Asks the agent to stop the current turn.
    pub fn cancel(&mut self, cx: &mut Context<Self>) {
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
            self.set_working(false, cx);
        }
        for request in self.permission_requests.drain(..) {
            request
                .responder
                .respond(acp::RequestPermissionResponse::new(
                    acp::RequestPermissionOutcome::Cancelled,
                ))
                .ok();
        }
        cx.notify();
    }

    pub fn respond_to_permission(
        &mut self,
        tool_call_id: &acp::ToolCallId,
        option_id: acp::PermissionOptionId,
        cx: &mut Context<Self>,
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
        cx.notify();
    }

    fn set_working(&mut self, working: bool, cx: &mut Context<Self>) {
        if working == self.is_working() {
            return;
        }
        self.turn_started_at = working.then(Instant::now);
        cx.emit(AgentThreadEvent::WorkingChanged(working));
    }

    fn fail(&mut self, error: String, cx: &mut Context<Self>) {
        let mut message = error;
        if !self.stderr_lines.is_empty() {
            message.push_str("\n\n");
            message.push_str(&Vec::from(self.stderr_lines.clone()).join("\n"));
        }
        self.status = ConnectionStatus::Failed(message.into());
        self.session = None;
        self.queued_prompts.clear();
        self.set_working(false, cx);
        cx.notify();
    }

    fn record_stderr(&mut self, line: String) {
        if self.stderr_lines.len() == STDERR_LINES_KEPT {
            self.stderr_lines.pop_front();
        }
        self.stderr_lines.push_back(line);
    }

    fn handle_incoming(&mut self, incoming: Incoming, cx: &mut Context<Self>) {
        match incoming {
            Incoming::Notification(notification) => {
                self.apply_update(notification.update);
                if let Some(title) = self.pending_title.take() {
                    cx.emit(AgentThreadEvent::TitleChanged(title));
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
        cx.notify();
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

/// Spawns the agent, wires up the ACP connection, and creates a session.
async fn connect(
    command: AgentCommand,
    cwd: PathBuf,
    this: gpui::WeakEntity<AgentThread>,
    cx: &mut gpui::AsyncApp,
) -> Result<Connected> {
    let mut child = smol::process::Command::new(&command.path)
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

    let incoming_lines = futures::io::BufReader::new(stdout).lines().boxed();
    let outgoing_lines = Box::pin(futures::sink::unfold(
        Box::pin(stdin),
        async move |mut writer, line: String| {
            let mut bytes = line.into_bytes();
            bytes.push(b'\n');
            writer.write_all(&bytes).await?;
            writer.flush().await?;
            Ok::<_, std::io::Error>(writer)
        },
    ));

    let (incoming_sender, mut incoming_receiver) = mpsc::unbounded::<Incoming>();
    let (connection_sender, connection_receiver) = oneshot::channel();
    let connection_future = {
        let notification_sender = incoming_sender.clone();
        let permission_sender = incoming_sender;
        Client
            .builder()
            .name("agentZ")
            .on_receive_notification(
                async move |notification: acp::SessionNotification, _connection| {
                    notification_sender
                        .unbounded_send(Incoming::Notification(notification))
                        .ok();
                    Ok(())
                },
                agent_client_protocol::on_receive_notification!(),
            )
            .on_receive_request(
                async move |request: acp::RequestPermissionRequest,
                            responder: Responder<acp::RequestPermissionResponse>,
                            _connection| {
                    if let Err(error) =
                        permission_sender.unbounded_send(Incoming::Permission(request, responder))
                    {
                        let Incoming::Permission(_, responder) = error.into_inner() else {
                            return Ok(());
                        };
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

    let io_task = cx.background_spawn(async move {
        if let Err(error) = connection_future.await {
            log::error!("ACP connection error: {error:?}");
        }
    });
    let stderr_task = cx.spawn({
        let this = this.clone();
        async move |cx| {
            let mut lines = futures::io::BufReader::new(stderr).lines();
            while let Some(Ok(line)) = lines.next().await {
                log::warn!("agent stderr: {line}");
                if this.update(cx, |this, _| this.record_stderr(line)).is_err() {
                    break;
                }
            }
        }
    });
    let incoming_task = cx.spawn({
        let this = this.clone();
        async move |cx| {
            while let Some(incoming) = incoming_receiver.next().await {
                if this
                    .update(cx, |this, cx| this.handle_incoming(incoming, cx))
                    .is_err()
                {
                    break;
                }
            }
        }
    });
    let exit_task = cx.spawn({
        let this = this.clone();
        async move |cx| {
            let status = child.status().await;
            this.update(cx, |this, cx| {
                let message = match status {
                    Ok(status) => format!("The agent exited ({status})."),
                    Err(error) => format!("The agent stopped: {error}"),
                };
                this.fail(message, cx);
            })
            .ok();
        }
    });
    this.update(cx, |this, _| {
        this._tasks
            .extend([io_task, stderr_task, incoming_task, exit_task]);
    })?;

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
    })
}

struct Connected {
    connection: ConnectionTo<Agent>,
    capabilities: acp::AgentCapabilities,
    auth_methods: Vec<acp::AuthMethod>,
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

    /// Runs the real process and protocol plumbing against `test_support/mock_agent.py`.
    #[gpui::test]
    fn talks_to_a_real_agent_process(cx: &mut gpui::TestAppContext) {
        let Some(python) = which_python() else {
            eprintln!("skipping: python3 not found");
            return;
        };
        cx.executor().allow_parking();
        let script = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("test_support/mock_agent.py");
        let command = AgentCommand {
            path: python,
            args: vec![script.to_string_lossy().into_owned()],
            env: Default::default(),
        };
        let cwd = std::env::temp_dir();
        let thread =
            cx.new(|cx| AgentThread::start("Mock".into(), Task::ready(Ok(command)), cwd, None, cx));

        let wait_until = |cx: &mut gpui::TestAppContext, done: &dyn Fn(&AgentThread) -> bool| {
            for _ in 0..500 {
                cx.run_until_parked();
                if thread.read_with(cx, |thread, _| done(thread)) {
                    return;
                }
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            let status = thread.read_with(cx, |thread, _| thread.status().clone());
            panic!("timed out; status {status:?}");
        };

        wait_until(cx, &|thread| thread.status() == &ConnectionStatus::Ready);
        thread.update(cx, |thread, cx| thread.send("hello".into(), cx));
        wait_until(cx, &|thread| {
            !thread.is_working() && thread.entries().len() >= 3
        });
        thread.read_with(cx, |thread, _| {
            assert_eq!(thread.entries()[0], Entry::UserMessage("hello".into()));
            assert_eq!(thread.entries()[1], Entry::AgentMessage("Echo: hello".into()));
            assert!(matches!(&thread.entries()[2], Entry::ToolCall(call) if call.title == "Read README.md"));
            assert_eq!(thread.last_stop_reason(), Some(&acp::StopReason::EndTurn));
        });

        thread.read_with(cx, |thread, _| {
            let ids: Vec<_> = thread
                .config_options()
                .iter()
                .map(|option| option.id.0.to_string())
                .collect();
            assert_eq!(ids, vec!["mode", "model", "effort", "fast"]);
        });
        thread.update(cx, |thread, cx| {
            thread.set_config_option(
                acp::SessionConfigId::new("model"),
                acp::SessionConfigOptionValue::value_id("opus"),
                cx,
            )
        });
        let model_is = |thread: &AgentThread, expected: &str| {
            thread.config_options().iter().any(|option| {
                option.id.0.as_ref() == "model"
                    && matches!(&option.kind, acp::SessionConfigKind::Select(select)
                        if select.current_value.0.as_ref() == expected)
            })
        };
        thread.read_with(cx, |thread, _| assert!(model_is(thread, "opus")));
        wait_until(cx, &|thread| model_is(thread, "opus"));

        thread.update(cx, |thread, cx| thread.send("permission".into(), cx));
        let tool_call_id = acp::ToolCallId::new("call-2");
        wait_until(cx, &|thread| {
            thread.permission_request(&tool_call_id).is_some()
        });
        thread.update(cx, |thread, cx| {
            let request = thread.permission_request(&tool_call_id).expect("request");
            assert_eq!(request.options.len(), 2);
            let allow = request.options[0].id.clone();
            thread.respond_to_permission(&tool_call_id, allow, cx);
        });
        wait_until(cx, &|thread| !thread.is_working());
        thread.read_with(cx, |thread, _| {
            let last_message = thread.entries().iter().rev().find_map(|entry| match entry {
                Entry::AgentMessage(text) => Some(text.clone()),
                _ => None,
            });
            assert_eq!(
                last_message.as_deref(),
                Some("Echo: permission (chose allow)")
            );
        });
    }

    /// A second thread given the first one's session id gets the conversation replayed.
    #[gpui::test]
    fn reloads_previous_session(cx: &mut gpui::TestAppContext) {
        let Some(python) = which_python() else {
            eprintln!("skipping: python3 not found");
            return;
        };
        cx.executor().allow_parking();
        let history_dir = tempfile::tempdir().expect("temp dir");
        let script = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("test_support/mock_agent.py");
        let command = AgentCommand {
            path: python,
            args: vec![
                script.to_string_lossy().into_owned(),
                history_dir
                    .path()
                    .join("history.json")
                    .to_string_lossy()
                    .into_owned(),
            ],
            env: Default::default(),
        };
        let wait_until = |cx: &mut gpui::TestAppContext,
                          thread: &gpui::Entity<AgentThread>,
                          done: &dyn Fn(&AgentThread) -> bool| {
            for _ in 0..500 {
                cx.run_until_parked();
                if thread.read_with(cx, |thread, _| done(thread)) {
                    return;
                }
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            panic!("timed out");
        };

        let first = cx.new(|cx| {
            AgentThread::start(
                "Mock".into(),
                Task::ready(Ok(command.clone())),
                std::env::temp_dir(),
                None,
                cx,
            )
        });
        wait_until(cx, &first, &|thread| {
            thread.status() == &ConnectionStatus::Ready
        });
        first.update(cx, |thread, cx| thread.send("hello".into(), cx));
        wait_until(cx, &first, &|thread| {
            !thread.is_working() && thread.entries().len() >= 3
        });
        assert_eq!(
            first.read_with(cx, |thread, _| thread.session_restore()),
            Some(SessionRestore::New)
        );

        let second = cx.new(|cx| {
            AgentThread::start(
                "Mock".into(),
                Task::ready(Ok(command)),
                std::env::temp_dir(),
                Some(acp::SessionId::new("session-1")),
                cx,
            )
        });
        wait_until(cx, &second, &|thread| {
            thread.status() == &ConnectionStatus::Ready
        });
        second.read_with(cx, |thread, _| {
            assert_eq!(thread.session_restore(), Some(SessionRestore::Loaded));
            assert_eq!(thread.entries()[0], Entry::UserMessage("hello".into()));
            assert_eq!(
                thread.entries()[1],
                Entry::AgentMessage("Echo: hello".into())
            );
        });
    }

    fn which_python() -> Option<PathBuf> {
        let path = std::env::var_os("PATH")?;
        std::env::split_paths(&path)
            .map(|dir| dir.join("python3"))
            .find(|candidate| candidate.is_file())
    }

    #[gpui::test]
    fn streams_updates_into_entries(cx: &mut gpui::TestAppContext) {
        let thread = cx.new(|_| AgentThread::failed("Test".into(), "not started"));
        thread.update(cx, |thread, cx| {
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
            cx.notify();
        });
        thread.read_with(cx, |thread, _| {
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
        });
    }
}
