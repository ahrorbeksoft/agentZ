//! One conversation with an ACP agent: starts the agent process, runs an ACP session in the
//! project folder, and keeps the conversation (messages, tool calls, plan, permission
//! requests) as it streams in.
//!
//! The connection setup follows Zed's `agent_servers::acp`. Plain Rust on tokio, so the server
//! can own it: the agent's process, the SDK's handlers and requests in flight report back as
//! [`ThreadMessage`]s, which the thread's owner passes to [`AgentThread::handle`]. What the
//! owner should hear about queues up as [`AgentThreadEvent`]s.

mod attachments;
mod wire;

pub use attachments::Attachments;

use std::collections::{HashMap, VecDeque};
use std::future::Future;
use std::os::fd::OwnedFd;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

use agent_client_protocol::schema::ProtocolVersion;
use agent_client_protocol::schema::v1 as acp;
use agent_client_protocol::{Agent, Client, ConnectionTo, Responder};
use agentz_protocol::agents::select_offers;
use agentz_protocol::attachments::AttachmentId;
use agentz_protocol::thread::login_code;
pub use agentz_protocol::thread::{
    AuthStatus, BackgroundTask, ConnectionStatus, ContextUsage, DiffLineKind, Elicitation, Entry,
    FailedMessage, FileDiff, LoginIdentity, LostHistory, PendingHandoff, PermissionOption,
    PermissionRequest, PlanItem, QueuedMessage, SessionDefaults, SessionRestore, ThreadState,
    ThreadView, ToolCall, TurnTime,
};
use anyhow::{Context as _, Result, anyhow};
use futures::channel::{mpsc, oneshot};
use futures::future::BoxFuture;
use futures::{FutureExt as _, SinkExt as _, StreamExt as _};
use gpui_shared_string::SharedString;
use registry::AgentCommand;
pub use registry::CommandFuture;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::task::JoinSet;

use crate::wire::{ANSWER_MARKER_SESSION, PAUSE_MARKER_SESSION, Parked, Wire, marker_line};

const STDERR_LINES_KEPT: usize = 20;

/// How long an agent may take to close its session before it's stopped anyway.
const CLOSE_SESSION_TIMEOUT: Duration = Duration::from_secs(3);

/// How Claude Agent and Codex report their login, unasked: an ACP extension notification.
const AUTH_STATUS_NOTIFICATION: &str = "_auth/status_update";

/// How Claude Agent and Codex take a message into a running turn: an ACP extension request,
/// until ACP has its own.
const STEERING_REQUEST: &str = "_session/steering";

/// How Claude Agent stops a background task: JetBrains AIR's async tasks extension.
const STOP_BACKGROUND_TASK_REQUEST: &str = "_session/async_task/stop";

/// How long an agent working without a prompt may send nothing, with no tool call running,
/// before it counts as done. Only for agents that don't end that work with a cost, as Claude
/// Agent does.
const OWN_WORK_QUIET: Duration = Duration::from_secs(30);

/// How long after a background task ends by itself the agent may take to go on with its
/// result, as Claude Agent does, before the thread counts as done without it.
const OWN_WORK_GRACE: Duration = Duration::from_secs(15);

pub enum AgentThreadEvent {
    /// The agent started or finished working on a prompt.
    WorkingChanged(bool),
    /// The ACP session to restore the thread from later: sent once the session has a prompt.
    SessionStarted(acp::SessionId),
    /// The login the agent reported while that session is in use, to start the thread with
    /// later ([`AgentThread::set_session_login`]).
    SessionLogin(LoginIdentity),
    /// The first prompt of a new conversation was sent; useful as a title.
    FirstPrompt(String),
    /// The agent named the session.
    TitleChanged(String),
    /// The user changed one of the session's settings; Zed keeps it as the agent's default.
    ConfigOptionChanged(acp::SessionConfigId, acp::SessionConfigOptionValue),
    /// The user changed the session's mode; Zed keeps it as the agent's default.
    ModeChanged(acp::SessionModeId),
    /// The conversation this thread continues, from the given thread, went to the agent with
    /// its first message.
    HandoffSent(projects::ThreadId),
    /// The user started the thread without the conversation it would have brought.
    HandoffDropped,
    /// Logging in with the named method succeeded.
    LoggedIn(SharedString),
    /// The agent is logged out: agentZ logged it out, or it asked for a login.
    LoggedOut,
    /// The account the agent says it's logged in to (`_auth/status_update`).
    AccountReported(AuthStatus),
    /// The connection paused after [`AgentThread::pause`], with everything the agent sent
    /// before handled: the thread can be handed off.
    Paused,
    /// The agent started one of its own subagents, in a session of its own.
    SubagentStarted(Subagent),
    /// What the agent sent in a subagent's session, for its subthread.
    SubagentUpdate {
        session: String,
        update: acp::SessionUpdate,
    },
    /// A subagent ended, or stopped with the agent or its session.
    SubagentEnded {
        session: String,
        end: projects::TaskEnd,
    },
}

/// One of the agent's own subagents, which it runs in a session under the thread's: Claude
/// Agent's native subagent sessions (JetBrains AIR's `nativeSubagentSessions`). The agent
/// sends its steps to that session, and hears of its end itself.
#[derive(Clone, Debug, PartialEq)]
pub struct Subagent {
    /// The subagent's session.
    pub session: String,
    /// The subagent that started it, by its session; `None` for the thread's own.
    pub parent_session: Option<String>,
    pub name: String,
    /// What it was asked: its prompt, or else its description.
    pub task: String,
}

/// Work the owner does around every turn, such as taking checkpoints. The turn waits for it:
/// the prompt goes to the agent once [`TurnPoint::Starting`] is done, and the thread stops
/// working once [`TurnPoint::Ended`] is.
pub type TurnHook = Arc<dyn Fn(TurnPoint) -> BoxFuture<'static, ()> + Send + Sync>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TurnPoint {
    Starting,
    Ended,
}

/// A `terminal/*` request from the agent, with its answer to give (ACP's client terminals).
pub enum TerminalRequest {
    Create(
        acp::CreateTerminalRequest,
        Responder<acp::CreateTerminalResponse>,
    ),
    Output(
        acp::TerminalOutputRequest,
        Responder<acp::TerminalOutputResponse>,
    ),
    WaitForExit(
        acp::WaitForTerminalExitRequest,
        Responder<acp::WaitForTerminalExitResponse>,
    ),
    Kill(
        acp::KillTerminalRequest,
        Responder<acp::KillTerminalResponse>,
    ),
    Release(
        acp::ReleaseTerminalRequest,
        Responder<acp::ReleaseTerminalResponse>,
    ),
}

/// Runs the terminals the agent asks for. With one, the client advertises ACP's `terminal`
/// capability. It must answer without blocking: waiting for an exit answers later.
pub type TerminalHost = Arc<dyn Fn(TerminalRequest) + Send + Sync>;

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
    /// The agent answered a message sent into its turn ([`AgentThread::steer_message`]).
    Steered {
        parts: Vec<MessagePart>,
        result: std::result::Result<Value, agent_client_protocol::Error>,
    },
    /// The pause's marker made it through the SDK.
    PauseMarker,
    /// The answer to an adopted agent's prompt arrived, after what the agent sent before it.
    AnswerMarker,
    /// A pause that was called off finished; the connection resumes.
    PauseCalledOff(Parked),
    /// An adopted agent's new connection is ready.
    Reconnected(ConnectionTo<Agent>),
    /// The agent says the URL elicitation is done (`elicitation/complete`).
    ElicitationCompleted(acp::ElicitationId),
    /// The agent called off an elicitation it asked for.
    ElicitationCancelled(u64),
    /// The agent reported its login (`_auth/status_update`).
    AuthStatus(AuthStatus),
    /// The agent reported on work it left running ([`ThreadState::background_tasks`]).
    BackgroundTask(BackgroundTaskUpdate),
    /// The agent answered [`AgentThread::stop_background_task`].
    BackgroundTaskStopped {
        task_id: SharedString,
        result: std::result::Result<Value, agent_client_protocol::Error>,
    },
    /// [`OWN_WORK_QUIET`] passed since the agent, working without a prompt, had sent this many
    /// updates.
    OwnWorkQuiet(u64),
    /// [`OWN_WORK_GRACE`] passed since the background task with this count ended.
    OwnWorkOverdue(u64),
    /// The owner's work after the agent's work of its own is done.
    OwnWorkEnded,
    /// The agent reported on one of its own subagents, in the session given.
    Subagent {
        session: acp::SessionId,
        update: SubagentNotice,
    },
}

/// A `session/update` by which Claude Agent reports its subagents' sessions (JetBrains AIR's
/// native subagent sessions). ACP's own updates don't include them.
#[derive(Debug, Deserialize)]
#[serde(
    tag = "sessionUpdate",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
enum SubagentNotice {
    SubagentSpawned {
        subagent_session_id: String,
        name: Option<String>,
        task: Option<String>,
        prompt: Option<String>,
    },
    SubagentStateUpdate {
        subagent_session_id: String,
        /// "completed", "failed", "cancelled" or "disconnected".
        state: String,
    },
}

/// A `session/update` of JetBrains AIR's async tasks extension, by which Claude Agent reports
/// the work it left running. ACP's own updates don't include them.
#[derive(Debug, Deserialize)]
#[serde(
    tag = "sessionUpdate",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
enum BackgroundTaskUpdate {
    AsyncTaskSpawned {
        async_task_id: String,
        name: Option<String>,
        description: Option<String>,
        task_type: Option<String>,
        #[serde(default)]
        can_stop: bool,
        output_file_path: Option<PathBuf>,
        tool_call_id: Option<String>,
    },
    AsyncTaskProgress {
        async_task_id: String,
        description: Option<String>,
        summary: Option<String>,
        output_file_path: Option<PathBuf>,
        tool_call_id: Option<String>,
    },
    AsyncTaskStateUpdate {
        async_task_id: String,
        /// "running", "paused", "completed", "failed" or "stopped".
        state: String,
        output_file_path: Option<PathBuf>,
        tool_call_id: Option<String>,
    },
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
    Elicitation(
        acp::CreateElicitationRequest,
        Responder<acp::CreateElicitationResponse>,
    ),
}

/// The answer to an elicitation in [`ThreadState::elicitations`].
struct ElicitationResponder {
    id: u64,
    responder: Responder<acp::CreateElicitationResponse>,
    /// Dropped once answered, which ends the task watching for the agent to call it off.
    _answered: oneshot::Sender<()>,
}

struct Session {
    connection: ConnectionTo<Agent>,
    session_id: acp::SessionId,
}

struct SentMessage {
    parts: Vec<MessagePart>,
    /// The conversation that went with it, which goes again with it.
    handoff: Option<PendingHandoff>,
}

pub struct AgentThread {
    /// What clients see.
    view: ThreadView,
    /// Answers to the permission requests in `view`, by tool call.
    permission_responders: Vec<(acp::ToolCallId, Responder<acp::RequestPermissionResponse>)>,
    elicitation_responders: Vec<ElicitationResponder>,
    next_elicitation_id: u64,
    /// Set once the agent is initialized; kept so a session can be (re)opened after logging in.
    connection: Option<ConnectionTo<Agent>>,
    previous_session: Option<acp::SessionId>,
    /// The login the agent reported while `previous_session` was in use.
    session_login: Option<LoginIdentity>,
    session: Option<Session>,
    pending_title: Option<String>,
    queued_prompts: Vec<Vec<MessagePart>>,
    /// Messages sent into a turn that the agent didn't take, sent one a turn as turns end.
    after_turn: VecDeque<Vec<MessagePart>>,
    /// The first entry that changed since the server last sent this thread's changes, so it
    /// doesn't compare a long conversation's every entry on each update.
    entries_changed_from: Option<usize>,
    /// The conversation this thread continues, taken from [`ThreadState::handoff`] by the first
    /// message, to go with it once that's sent.
    handoff_to_send: Option<PendingHandoff>,
    /// The user's last message, as it went to the agent, to send again if it doesn't get
    /// through ([`ThreadState::failed_message`]).
    last_message: Option<SentMessage>,
    /// Given to the agent with every session it opens.
    mcp_servers: Vec<acp::McpServer>,
    /// Where images the agent shows or replays are kept, for messages to link to.
    attachments: Option<Attachments>,
    terminal_host: Option<TerminalHost>,
    turn_hook: Option<TurnHook>,
    /// Set by [`Self::cancel`] for the turn in flight, in case its prompt hasn't gone out yet.
    turn_cancelled: Option<Arc<AtomicBool>>,
    stderr_lines: VecDeque<String>,
    /// False for a connection made only to log in or out (from settings), which never opens a
    /// session.
    opens_session: bool,
    /// The login method that reads a key from the agent's environment, which clients are asked
    /// for ([`Self::set_key_method`]).
    key_method: Option<acp::AuthMethodId>,
    /// The login to start once the restarted agent is up ([`Self::restart_with`]).
    authenticate_once_connected: Option<acp::AuthMethodId>,
    /// The restarted agent waits for the user to log in instead of opening a session
    /// ([`Self::started_login_unasked`]).
    waits_for_login: bool,
    defaults: SessionDefaults,
    events: Vec<AgentThreadEvent>,
    /// `None` for a thread that never starts.
    runtime: Option<tokio::runtime::Handle>,
    messages: mpsc::UnboundedSender<ThreadMessage>,
    generation: u64,
    /// The agent process, its connection and requests in flight. Dropping the thread (or
    /// reloading it) aborts them, which also stops the agent.
    tasks: JoinSet<()>,
    /// The agent's pipes under the SDK.
    wire: Option<Wire>,
    /// The agent's process, which [`Self::release`] leaves running.
    process: Option<AgentProcess>,
    /// Stops an adopted agent, which isn't this process's child, with the thread.
    adopted_process: Option<ProcessGuard>,
    /// A pause in progress (see [`Self::pause`]).
    pausing: Option<Arc<Mutex<PauseSlot>>>,
    /// An adopted agent's session, open once its new connection is.
    pending_session: Option<acp::SessionId>,
    /// The thread has its whole conversation: from its [`Transcript`], or since a session
    /// opened. What the agent replays as the session loads again is dropped then, as t3code
    /// drops it.
    has_conversation: bool,
    /// The session is loading, and what the agent replays is dropped.
    dropping_replay: bool,
    /// Counts changes to what [`Self::transcript`] gives, so the server saves it only when it
    /// changed.
    conversation_revision: u64,
    /// The agent works without a prompt, as Claude Agent does once a background task it
    /// started ends. ACP has no turn for that, so the thread works until the agent's result
    /// (an update with its cost) or until it has been quiet for [`OWN_WORK_QUIET`].
    working_on_its_own: bool,
    /// The updates the agent sent while working on its own, to tell when it went quiet.
    own_work_updates: u64,
    /// The first entry of the agent's work of its own.
    own_work_from: usize,
    /// The agent's work of its own ended, and the owner's work after a turn is being done.
    own_work_ending: bool,
    /// A background task ended by itself while the agent was idle, and the agent is expected
    /// to go on with its result for up to [`OWN_WORK_GRACE`].
    own_work_due: bool,
    /// Counts background tasks that ended by themselves, so only the latest one's grace ends
    /// [`Self::own_work_due`].
    tasks_ended: u64,
    /// Stops of background tasks the agent hasn't answered yet. What it says meanwhile is its
    /// note of the stop, not work of its own.
    stops_in_flight: usize,
    /// The agent's next message starts an entry of its own rather than going on with the last.
    new_message_due: bool,
    /// The sessions of the agent's own subagents ([`Subagent`]), and whether each still runs.
    /// Their updates go to their subthreads rather than into this conversation. Those of one
    /// that's over, or that a loading session replays (its subthread has its conversation
    /// already), are dropped.
    subagent_sessions: HashMap<String, bool>,
    /// The thread is a subagent's ([`Self::subagent`]): it takes no messages, and only the
    /// parent's agent stops it.
    runs_in_parent: bool,
}

/// The agent's process id, and whether stopping the thread stops it.
#[derive(Clone)]
struct AgentProcess {
    pid: u32,
    armed: Arc<AtomicBool>,
}

/// Kills the agent when dropped, unless disarmed: once it exited (its id may be reused) or
/// was handed to another server.
struct ProcessGuard(AgentProcess);

impl Drop for ProcessGuard {
    fn drop(&mut self) {
        if self.0.armed.swap(false, Ordering::SeqCst) {
            kill(self.0.pid);
        }
    }
}

/// Kills the agent with what it started (Zed's `util::process::Child::kill`): Factory Droid's
/// `acp-daemon` runs each session in a worker, and agents start tools and MCP servers.
fn kill(pid: u32) {
    let Ok(pid) = libc::pid_t::try_from(pid) else {
        return;
    };
    // SAFETY: only sends a signal to the agent's process group, which it leads.
    if unsafe { libc::killpg(pid, libc::SIGKILL) } == 0 {
        return;
    }
    // An agent started by an older server, handed off from it, shares that server's group.
    // SAFETY: only sends a signal to the agent's process.
    if unsafe { libc::kill(pid, libc::SIGKILL) } != 0 {
        log::warn!(
            "failed to stop the agent (pid {pid}): {}",
            std::io::Error::last_os_error()
        );
    }
}

#[derive(Default)]
struct PauseSlot {
    parked: Option<Parked>,
    called_off: bool,
}

/// What a thread knew when its agent was handed to another server, to go on from there with
/// [`AgentThread::adopt`].
#[derive(Serialize, Deserialize)]
pub struct AgentSnapshot {
    view: ThreadView,
    session_id: acp::SessionId,
    previous_session: Option<acp::SessionId>,
    pending_title: Option<String>,
    mcp_servers: Vec<acp::McpServer>,
    defaults: SessionDefaults,
    stderr_lines: Vec<String>,
    pid: u32,
    /// The prompt the agent works on, by its JSON-RPC id.
    prompt_id: Option<Value>,
    /// The agent works without a prompt ([`AgentThread::working_on_its_own`]).
    #[serde(default)]
    working_on_its_own: bool,
    /// [`AgentThread::subagent_sessions`].
    #[serde(default)]
    subagent_sessions: HashMap<String, bool>,
    /// Requests the agent waits on an answer to, as it sent them.
    unanswered: Vec<String>,
    stdout_rest: Vec<u8>,
    stderr_rest: Vec<u8>,
}

/// A thread's conversation as its server keeps it, as t3code keeps each thread's messages, so
/// the thread opens with all of it: an agent's `session/load` may replay only the end of a long
/// conversation (Factory Droid replays its last 100 messages), and some can't replay at all.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Transcript {
    pub entries: Vec<Entry>,
    pub plan: Vec<PlanItem>,
    pub finished_turns: Vec<TurnTime>,
    pub prompts_from_agents: Vec<(usize, projects::ThreadCreator)>,
    pub sent_times: Vec<(usize, SystemTime)>,
}

/// A [`Transcript`] borrowed from its thread, which saves in the same form. The server saves
/// it every few seconds while the thread works, and copying a long conversation each time
/// left it holding many times the conversation's size in memory.
#[derive(Clone, Copy, Debug, Serialize)]
pub struct TranscriptRef<'a> {
    pub entries: &'a [Entry],
    pub plan: &'a [PlanItem],
    pub finished_turns: &'a [TurnTime],
    pub prompts_from_agents: &'a [(usize, projects::ThreadCreator)],
    pub sent_times: &'a [(usize, SystemTime)],
}

impl From<TranscriptRef<'_>> for Transcript {
    fn from(transcript: TranscriptRef<'_>) -> Self {
        Self {
            entries: transcript.entries.to_vec(),
            plan: transcript.plan.to_vec(),
            finished_turns: transcript.finished_turns.to_vec(),
            prompts_from_agents: transcript.prompts_from_agents.to_vec(),
            sent_times: transcript.sent_times.to_vec(),
        }
    }
}

/// A thread's agent, handed to another server: the snapshot and the agent's pipes.
pub struct HandedOffAgent {
    pub snapshot: AgentSnapshot,
    pub stdin: OwnedFd,
    pub stdout: OwnedFd,
    pub stderr: OwnedFd,
}

impl std::ops::Deref for AgentThread {
    type Target = ThreadView;

    fn deref(&self) -> &Self::Target {
        &self.view
    }
}

impl Drop for AgentThread {
    /// A thread that goes away closes its session before its agent stops. Otherwise the agent
    /// stops with the thread's fields.
    fn drop(&mut self) {
        if !self.can_close_session() {
            return;
        }
        let Some(runtime) = self.runtime.clone() else {
            return;
        };
        runtime.spawn(self.stop_agent());
    }
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
        terminal_host: Option<TerminalHost>,
    ) -> (Self, ThreadInbox) {
        let (mut this, inbox) =
            Self::new(Some(runtime), agent_name, ConnectionStatus::Connecting, cwd);
        this.previous_session = previous_session;
        this.terminal_host = terminal_host;
        this.connect_agent(command);
        (this, inbox)
    }

    /// Starts the agent only to log in or out of it, as from its settings. No session is opened,
    /// so the agent starts in a scratch directory.
    pub fn start_for_login_session(
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

    /// The subthread of one of the agent's own subagents ([`Subagent`]). It has no agent of its
    /// own: the parent's runs the subagent, and the owner passes on what that sends in the
    /// subagent's session ([`Self::apply_subagent_update`]).
    pub fn subagent(agent_name: SharedString, cwd: PathBuf) -> Self {
        let (mut this, _) = Self::new(None, agent_name, ConnectionStatus::Ready, cwd);
        this.has_conversation = true;
        this.runs_in_parent = true;
        this
    }

    /// The subagent works, from the agent that gave it `prompt`, if that's still to show.
    pub fn start_subagent(&mut self, prompt: Option<(String, projects::ThreadCreator)>) {
        if let Some((prompt, from)) = prompt {
            let index = self.view.entries.len();
            self.view.state.sent_times.push((index, SystemTime::now()));
            self.view.state.prompts_from_agents.push((index, from));
            self.push_entry(Entry::UserMessage(prompt));
        }
        self.set_working(true);
    }

    /// Whether it's a subagent's thread ([`Self::subagent`]).
    pub fn runs_in_parent(&self) -> bool {
        self.runs_in_parent
    }

    /// What the parent's agent sent in the subagent's session.
    pub fn apply_subagent_update(&mut self, update: acp::SessionUpdate) {
        self.apply_update(update);
        // Named by the parent's agent as it started it.
        self.pending_title = None;
    }

    /// The subagent ended, so its subthread is done.
    pub fn end_subagent(&mut self) {
        self.set_working(false);
    }

    /// Shows a subagent the agent started as a card in the conversation, which opens its
    /// subthread.
    pub fn add_subagent_card(&mut self, subagent: &Subagent, subthread: projects::ThreadId) {
        self.push_entry(Entry::ToolCall(ToolCall {
            id: subagent_card_id(&subagent.session),
            title: subagent.name.clone(),
            kind: acp::ToolKind::Other,
            status: acp::ToolCallStatus::InProgress,
            text: Vec::new(),
            diffs: Vec::new(),
            locations: Vec::new(),
            raw_input: None,
            terminals: Vec::new(),
            images: Vec::new(),
            started_at: Some(SystemTime::now()),
            duration: None,
            subthread: Some(subthread),
        }));
    }

    /// The card of the subagent working in `subthread` shows its end, and its report.
    pub fn end_subagent_card(
        &mut self,
        subthread: projects::ThreadId,
        end: projects::TaskEnd,
        report: Option<String>,
    ) {
        let Some(index) = self.view.entries.iter().rposition(|entry| {
            matches!(
                entry,
                Entry::ToolCall(tool_call)
                    if tool_call.subthread == Some(subthread) && tool_call.is_running()
            )
        }) else {
            return;
        };
        self.entry_changed(index);
        let Entry::ToolCall(tool_call) = &mut self.view.entries[index] else {
            return;
        };
        tool_call.status = match end {
            projects::TaskEnd::Completed => acp::ToolCallStatus::Completed,
            _ => acp::ToolCallStatus::Failed,
        };
        tool_call.text = report.into_iter().collect();
        note_end(tool_call);
    }

    fn new(
        runtime: Option<tokio::runtime::Handle>,
        agent_name: SharedString,
        status: ConnectionStatus,
        cwd: PathBuf,
    ) -> (Self, ThreadInbox) {
        let (messages, inbox) = mpsc::unbounded();
        let this = Self {
            view: ThreadView {
                state: ThreadState {
                    agent_name,
                    status,
                    cwd,
                    ..ThreadState::default()
                },
                entries: Vec::new(),
            },
            permission_responders: Vec::new(),
            elicitation_responders: Vec::new(),
            next_elicitation_id: 0,
            connection: None,
            previous_session: None,
            session_login: None,
            session: None,
            pending_title: None,
            queued_prompts: Vec::new(),
            after_turn: VecDeque::new(),
            entries_changed_from: Some(0),
            handoff_to_send: None,
            last_message: None,
            mcp_servers: Vec::new(),
            attachments: None,
            terminal_host: None,
            turn_hook: None,
            turn_cancelled: None,
            stderr_lines: VecDeque::new(),
            opens_session: true,
            key_method: None,
            authenticate_once_connected: None,
            waits_for_login: false,
            defaults: SessionDefaults::default(),
            events: Vec::new(),
            runtime,
            messages,
            generation: 0,
            tasks: JoinSet::new(),
            wire: None,
            process: None,
            adopted_process: None,
            pausing: None,
            pending_session: None,
            has_conversation: false,
            dropping_replay: false,
            conversation_revision: 0,
            working_on_its_own: false,
            own_work_updates: 0,
            own_work_from: 0,
            own_work_ending: false,
            own_work_due: false,
            tasks_ended: 0,
            stops_in_flight: 0,
            new_message_due: false,
            subagent_sessions: HashMap::new(),
            runs_in_parent: false,
        };
        (this, inbox)
    }

    /// Starts with the conversation the server kept ([`Self::transcript`]). Set it right after
    /// starting: the session opens once the agent has connected.
    pub fn restore_transcript(&mut self, transcript: Transcript) {
        if transcript.entries.is_empty() {
            return;
        }
        self.view.entries = transcript.entries;
        self.view.state.plan = transcript.plan;
        self.view.state.finished_turns = transcript.finished_turns;
        self.view.state.prompts_from_agents = transcript.prompts_from_agents;
        self.view.state.sent_times = transcript.sent_times;
        self.entry_changed(0);
        self.has_conversation = true;
    }

    /// The login the agent reported while the previous session was in use
    /// ([`AgentThreadEvent::SessionLogin`]), to tell whether a session that doesn't load was
    /// lost to an account change. Set it right after starting.
    pub fn set_session_login(&mut self, login: Option<LoginIdentity>) {
        self.session_login = login;
    }

    /// Changes whenever [`Self::transcript`] does, and is `None` while that is.
    pub fn conversation_revision(&self) -> Option<u64> {
        self.has_conversation.then_some(self.conversation_revision)
    }

    /// The conversation for the server to keep, once the thread has all of it: not while its
    /// session is still replaying it.
    pub fn transcript(&self) -> Option<TranscriptRef<'_>> {
        self.has_conversation.then(|| TranscriptRef {
            entries: &self.view.entries,
            plan: &self.view.state.plan,
            finished_turns: &self.view.state.finished_turns,
            prompts_from_agents: &self.view.state.prompts_from_agents,
            sent_times: &self.view.state.sent_times,
        })
    }

    /// MCP servers for the agent to start with its sessions. Set it right after starting: the
    /// session opens once the agent has connected.
    pub fn set_mcp_servers(&mut self, mcp_servers: Vec<acp::McpServer>) {
        self.mcp_servers = mcp_servers;
    }

    /// Work to do around each turn. Set it right after starting.
    pub fn set_turn_hook(&mut self, hook: TurnHook) {
        self.turn_hook = Some(hook);
    }

    /// Where to keep the images the agent shows, and the ones its history replays. Without it
    /// they show as a plain `@Image`. Set it right after starting.
    pub fn set_attachments(&mut self, attachments: Attachments) {
        self.attachments = Some(attachments);
    }

    /// The thread's queue, which its owner keeps, for clients to see.
    pub fn set_queued_messages(&mut self, messages: Vec<QueuedMessage>, steering: bool) {
        self.view.state.queued_messages = messages;
        self.view.state.steering_queued = steering;
    }

    /// Keeps an image the agent sent, for a message to link to.
    fn keep_image(&self, mime_type: &str, data: &str) -> Option<AttachmentId> {
        let attachments = self.attachments.as_ref()?;
        match attachments.add_base64(mime_type, data) {
            Ok(id) => Some(id),
            Err(error) => {
                log::error!("failed to keep an image from the agent: {error:#}");
                None
            }
        }
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
            log::error!("{} can't start background work", self.view.state.agent_name);
            return;
        };
        while self.tasks.try_join_next().is_some() {}
        self.tasks.spawn_on(task, runtime);
    }

    fn connect_agent(&mut self, command: CommandFuture) {
        let cwd = self.view.state.cwd.clone();
        let sender = self.sender();
        let terminal_host = self.terminal_host.clone();
        self.spawn_task(async move {
            // The agent's own tasks, which stop the agent when dropped.
            let mut agent_tasks = JoinSet::new();
            let result = async {
                let command = command.await?;
                let connected = connect(
                    command.clone(),
                    cwd,
                    sender.clone(),
                    terminal_host,
                    &mut agent_tasks,
                )
                .await?;
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
            match message.kind {
                MessageKind::Incoming(Incoming::Permission(_, responder)) => {
                    cancel_permission(responder)
                }
                MessageKind::Incoming(Incoming::Elicitation(_, responder)) => {
                    cancel_elicitation(responder)
                }
                _ => {}
            }
            return;
        }
        match message.kind {
            MessageKind::Connected(Ok((command, connected))) => {
                self.view.state.command = Some(command);
                self.wire = Some(connected.wire);
                self.process = Some(connected.process);
                self.connection = Some(connected.connection);
                self.view.state.capabilities = connected.capabilities;
                self.view.state.supports_steering = connected.supports_steering;
                self.view.state.auth_methods = connected.auth_methods;
                self.mark_key_method();
                self.view.state.agent_info = connected.agent_info;
                if let Some(method_id) = self.authenticate_once_connected.take() {
                    self.view.state.status = ConnectionStatus::AuthRequired;
                    self.authenticate(method_id, None);
                    return;
                }
                if std::mem::take(&mut self.waits_for_login) {
                    self.view.state.status = ConnectionStatus::AuthRequired;
                    self.fail_queued_prompts(FailedMessage::NeedsLogin);
                    return;
                }
                // A login session opens an empty session too: it is how the login
                // status (and the agent's settings) can be learned over ACP.
                self.open_session();
            }
            MessageKind::Connected(Err(error)) => {
                log::error!("failed to start agent: {error:#}");
                self.fail(format!("{error:#}"));
            }
            MessageKind::Incoming(incoming) => self.handle_incoming(incoming),
            MessageKind::Stderr(line) => self.record_stderr(line),
            MessageKind::Exited(message) => {
                // Its id may be reused now.
                if let Some(guard) = &self.adopted_process {
                    guard.0.armed.store(false, Ordering::SeqCst);
                }
                self.fail(message)
            }
            MessageKind::PauseMarker => {
                let parked = self
                    .pausing
                    .as_ref()
                    .and_then(|slot| lock_slot(slot).parked.take());
                if let (Some(parked), Some(wire)) = (parked, &mut self.wire) {
                    wire.park(parked);
                    self.emit(AgentThreadEvent::Paused);
                }
            }
            MessageKind::PauseCalledOff(parked) => {
                if let (Some(wire), Some(runtime)) = (&mut self.wire, &self.runtime) {
                    wire.park(parked);
                    wire.resume(runtime);
                }
            }
            MessageKind::Reconnected(connection) => self.reconnected(connection),
            MessageKind::AnswerMarker => {
                let Some(answer) = self.wire.as_ref().and_then(Wire::taken_answer) else {
                    return;
                };
                let response = prompt_answer(answer);
                let hook = self.turn_hook.clone();
                self.spawn(async move {
                    if let Some(hook) = hook {
                        hook(TurnPoint::Ended).await;
                    }
                    MessageKind::PromptFinished(response)
                });
            }
            MessageKind::SessionOpened { connection, result } => {
                self.session_opened(connection, result)
            }
            MessageKind::Authenticated {
                method_name,
                result,
            } => {
                self.view.state.authenticating = None;
                self.view.state.auth_links.clear();
                self.view.state.auth_code = None;
                self.view.state.login_page = None;
                // Whatever the login asked for is moot once it's over.
                self.cancel_elicitations(|elicitation| {
                    matches!(
                        elicitation.request.scope(),
                        acp::ElicitationScope::Request(_)
                    )
                });
                match result {
                    Ok(()) => {
                        if let Some(method_name) = method_name {
                            self.emit(AgentThreadEvent::LoggedIn(method_name));
                        }
                        self.view.state.auth_description = None;
                        if !self.opens_session {
                            self.view.state.login_notice = Some("Logged in.".into());
                            self.session = None;
                        }
                        if self.session.is_some() {
                            // The session was open when a prompt asked for the login.
                            self.view.state.status = ConnectionStatus::Ready;
                            self.view.state.logged_in = Some(true);
                        } else {
                            self.open_session();
                        }
                    }
                    Err(error) => {
                        self.view.state.auth_error = Some(error_message(&error).into());
                    }
                }
            }
            MessageKind::ElicitationCompleted(elicitation_id) => {
                self.view.state.elicitations.retain(|elicitation| {
                    !(elicitation.opened
                        && matches!(&elicitation.request.mode,
                            acp::ElicitationMode::Url(url) if url.elicitation_id == elicitation_id))
                });
            }
            MessageKind::ElicitationCancelled(id) => {
                self.elicitation_responders
                    .retain(|responder| responder.id != id);
                self.view
                    .state
                    .elicitations
                    .retain(|elicitation| elicitation.id != id);
            }
            MessageKind::AuthStatus(status) => {
                self.view.state.logged_in = Some(status.is_logged_in());
                self.emit(AgentThreadEvent::AccountReported(status.clone()));
                self.view.state.auth_status = Some(status);
                self.note_session_login();
                // The agent may report its login only after the session opened.
                if self.view.state.lost_history.is_some() {
                    self.view.state.lost_history = Some(self.why_history_lost());
                }
            }
            MessageKind::LoggedOut(result) => match result {
                Ok(()) => {
                    self.view.state.auth_error = None;
                    self.view.state.logged_in = Some(false);
                    self.emit(AgentThreadEvent::LoggedOut);
                    if self.opens_session {
                        self.view.state.status = ConnectionStatus::AuthRequired;
                    } else {
                        self.view.state.login_notice = Some("Logged out.".into());
                        self.drop_session();
                    }
                }
                Err(error) => {
                    self.view.state.auth_error =
                        Some(format!("Couldn't log out: {}", error_message(&error)).into())
                }
            },
            MessageKind::ConfigOptionSet { previous, result } => match result {
                Ok(response) => self.view.state.config_options = response.config_options,
                Err(error) => {
                    log::error!("failed to change an agent setting: {error:?}");
                    self.view.state.config_options = previous;
                }
            },
            MessageKind::ModeSet {
                previous_mode,
                result,
            } => {
                if let Err(error) = result {
                    log::error!("failed to change the agent's mode: {error:?}");
                    if let Some(modes) = &mut self.view.state.modes {
                        modes.current_mode_id = previous_mode;
                    }
                }
            }
            MessageKind::PromptFinished(result) => {
                self.turn_cancelled = None;
                match result {
                    Ok(response) => {
                        self.view.state.last_stop_reason = Some(response.stop_reason);
                        self.last_message = None;
                    }
                    // Some agents (Claude Agent, OpenCode) open sessions logged out and ask at
                    // the first prompt.
                    Err(error) if is_auth_required(&error) => {
                        self.view.state.status = ConnectionStatus::AuthRequired;
                        self.found_logged_out();
                        self.view.state.auth_description = auth_description(&error);
                        self.message_failed(FailedMessage::NeedsLogin);
                    }
                    Err(error) => {
                        log::error!("agent prompt failed: {error:?}");
                        self.view.state.turn_error = Some(error_message(&error).into());
                        self.message_failed(FailedMessage::TurnFailed);
                    }
                }
                // A finished turn can't still be waiting on a permission answer.
                self.cancel_permission_requests();
                self.cancel_elicitations(|elicitation| {
                    !elicitation.opened
                        && matches!(
                            elicitation.request.scope(),
                            acp::ElicitationScope::Session(_)
                        )
                });
                self.set_working(false);
                if let Some(parts) = self.after_turn.pop_front() {
                    self.send_after_turn(parts);
                }
            }
            MessageKind::Steered { parts, result } => self.steered(parts, result),
            MessageKind::BackgroundTask(update) => self.apply_background_task_update(update),
            MessageKind::BackgroundTaskStopped { task_id, result } => {
                self.background_task_stopped(task_id, result)
            }
            MessageKind::Subagent { session, update } => self.subagent_notice(session, update),
            // Unless a turn took over meanwhile.
            MessageKind::OwnWorkEnded if self.own_work_ending => self.own_work_ended(),
            MessageKind::OwnWorkEnded => {}
            MessageKind::OwnWorkQuiet(updates) => {
                if !self.working_on_its_own {
                    return;
                }
                let since = self.own_work_from.min(self.view.entries.len());
                let is_waiting = self.view.entries[since..].iter().any(|entry| {
                    matches!(entry, Entry::ToolCall(tool_call) if matches!(
                        tool_call.status,
                        acp::ToolCallStatus::Pending | acp::ToolCallStatus::InProgress
                    ))
                }) || !self.view.state.permission_requests.is_empty()
                    || !self.view.state.elicitations.is_empty();
                if updates == self.own_work_updates && !is_waiting {
                    self.end_own_work();
                } else {
                    self.check_own_work_later();
                }
            }
            MessageKind::OwnWorkOverdue(ended) => {
                if ended == self.tasks_ended {
                    self.own_work_due = false;
                }
            }
        }
    }

    /// Zed's "Reload Agent": restarts the agent and reopens the session. The thread keeps its
    /// conversation, unless it was still replaying in, when it comes again.
    pub fn reload(&mut self) {
        let Some(command) = self.view.state.command.clone() else {
            return;
        };
        self.restart_with(command, None);
    }

    /// Restarts the agent with `command` in place of the one it runs, as [`Self::reload`] does,
    /// then logs in with `log_in_with` if given: for a login the agent reads from its
    /// environment, which only a new process takes.
    pub fn restart_with(&mut self, command: AgentCommand, log_in_with: Option<acp::AuthMethodId>) {
        // The new agent waits for the old one to close the session it will load.
        let stopping = self.stop_agent();
        self.generation += 1;
        // What a loading session replayed comes again. An agent that asked for a login
        // replayed nothing: its entries are messages that didn't go, which Retry sends.
        if !self.has_conversation && self.view.state.status != ConnectionStatus::AuthRequired {
            self.view.entries.clear();
            self.view.state.finished_turns.clear();
            self.entry_changed(0);
            self.view.state.prompts_from_agents.clear();
            self.view.state.sent_times.clear();
            self.view.state.plan.clear();
        }
        self.cancel_permission_requests();
        self.cancel_elicitations(|_| true);
        self.queued_prompts.clear();
        self.after_turn.clear();
        // They stop with the agent.
        self.view.state.background_tasks.clear();
        self.stops_in_flight = 0;
        self.view.state.auth_error = None;
        self.view.state.auth_description = None;
        self.view.state.authenticating = None;
        self.view.state.auth_links.clear();
        self.view.state.auth_code = None;
        self.view.state.login_page = None;
        // A failed message can still go once the agent is back (after a login, often), and its
        // error says why it failed.
        if self.view.state.failed_message.is_none() {
            self.view.state.turn_error = None;
        }
        self.view.state.status = ConnectionStatus::Connecting;
        self.authenticate_once_connected = log_in_with;
        self.waits_for_login = false;
        self.set_working(false);
        self.connect_agent(
            async move {
                stopping.await;
                Ok(command)
            }
            .boxed(),
        );
    }

    /// Marks `method_id`, a login the agent reads from a variable in its environment (Droid's
    /// "Factory API Key"), as taking an API key, so clients ask for one
    /// ([`agentz_protocol::thread::LoginInput::ApiKey`]).
    /// Whoever starts the agent passes the key in that variable.
    pub fn set_key_method(&mut self, method_id: acp::AuthMethodId) {
        self.key_method = Some(method_id);
        self.mark_key_method();
    }

    fn mark_key_method(&mut self) {
        let Some(method_id) = &self.key_method else {
            return;
        };
        for method in &mut self.view.state.auth_methods {
            if let acp::AuthMethod::Agent(method) = method
                && method.id == *method_id
            {
                method
                    .meta
                    .get_or_insert_default()
                    .entry("api-key")
                    .or_insert_with(|| serde_json::json!({}));
            }
        }
    }

    /// Takes the agent out of the thread. The future it returns closes the agent's session
    /// first when it can (see [`Self::close_session`]), then stops the agent.
    fn stop_agent(&mut self) -> BoxFuture<'static, ()> {
        let closing = self.close_session();
        // Dropping these stops the agent process along with its connection.
        let running = (
            std::mem::take(&mut self.tasks),
            self.wire.take(),
            self.adopted_process.take(),
        );
        self.process = None;
        self.pausing = None;
        self.connection = None;
        self.session = None;
        self.end_subagents();
        async move {
            if let Some(closing) = closing {
                closing.await;
            }
            drop(running);
        }
        .boxed()
    }

    /// Forgets the open session, closing it while the agent keeps running.
    fn drop_session(&mut self) {
        if let Some(closing) = self.close_session() {
            self.spawn_task(closing);
        }
        self.session = None;
        // They belong to the session.
        self.view.state.background_tasks.clear();
        self.end_subagents();
    }

    /// Whether the open session can be closed: the agent supports it and still runs here (it
    /// didn't exit or go to another server), and the connection isn't paused, which would hold
    /// the request back.
    fn can_close_session(&self) -> bool {
        let Some(process) = self
            .process
            .as_ref()
            .or(self.adopted_process.as_ref().map(|guard| &guard.0))
        else {
            return false;
        };
        self.session.is_some()
            && self
                .view
                .state
                .capabilities
                .session_capabilities
                .close
                .is_some()
            && process.armed.load(Ordering::SeqCst)
            && !self.is_paused()
    }

    /// Asks the agent to close the open session, so it stops its work on it and frees it, as
    /// Zed does when a thread goes away. Resolves once the agent answers, or gives up.
    fn close_session(&self) -> Option<BoxFuture<'static, ()>> {
        if !self.can_close_session() {
            return None;
        }
        let session = self.session.as_ref()?;
        let reply = session
            .connection
            .send_request(acp::CloseSessionRequest::new(session.session_id.clone()))
            .block_task();
        Some(
            async move {
                match tokio::time::timeout(CLOSE_SESSION_TIMEOUT, reply).await {
                    Ok(Ok(_)) => {}
                    Ok(Err(error)) => log::warn!(
                        "the agent couldn't close its session: {}",
                        error_message(&error)
                    ),
                    Err(_) => log::warn!("the agent didn't close its session in time"),
                }
            }
            .boxed(),
        )
    }

    /// Gives up on the login in flight. Agents' browser logins only return once the user
    /// finishes (or the login expires), and some keep a callback server on a fixed port, so the
    /// agent restarts, as t3code does. A message that failed for want of the login can still be
    /// retried after one.
    pub fn cancel_authentication(&mut self) {
        if self.view.state.authenticating.is_none() {
            return;
        }
        self.restart_keeping_prompts();
    }

    /// [`Self::reload`], keeping the prompts waiting for the session, and the entries that
    /// show them while there's no session to replay them.
    fn restart_keeping_prompts(&mut self) {
        let queued_prompts = std::mem::take(&mut self.queued_prompts);
        // Without a session, the entries are only those prompts: nothing will replay them.
        let conversation = self.session.is_none().then(|| {
            (
                std::mem::take(&mut self.view.entries),
                std::mem::take(&mut self.view.state.prompts_from_agents),
                std::mem::take(&mut self.view.state.sent_times),
            )
        });
        self.reload();
        self.queued_prompts = queued_prompts;
        if let Some((entries, prompts_from_agents, sent_times)) = conversation {
            self.view.entries = entries;
            self.entry_changed(0);
            self.view.state.prompts_from_agents = prompts_from_agents;
            self.view.state.sent_times = sent_times;
        }
    }

    /// Pauses the connection between messages, to hand the agent to another server with
    /// [`Self::hand_off`]. [`AgentThreadEvent::Paused`] follows once everything the agent sent
    /// before the pause has been handled, so the thread's state is complete. Returns whether
    /// pausing started.
    pub fn pause(&mut self) -> bool {
        if self.pausing.is_some() {
            return false;
        }
        let Some(wire) = &mut self.wire else {
            return false;
        };
        if wire.is_paused() {
            return false;
        }
        let paused = wire.pause();
        let slot = Arc::new(Mutex::new(PauseSlot::default()));
        self.pausing = Some(slot.clone());
        let sender = self.sender();
        self.spawn_task(async move {
            let Some(parked) = paused.await else {
                return;
            };
            let mut pause = lock_slot(&slot);
            if pause.called_off {
                drop(pause);
                sender.send(MessageKind::PauseCalledOff(parked)).ok();
                return;
            }
            let incoming = parked.incoming();
            pause.parked = Some(parked);
            drop(pause);
            // Behind every line the agent sent before the pause, the SDK passes this on last.
            incoming
                .unbounded_send(Ok(marker_line(PAUSE_MARKER_SESSION)))
                .ok();
        });
        true
    }

    /// Whether the connection is paused or pausing: what's sent to the agent waits.
    pub fn is_paused(&self) -> bool {
        self.pausing.is_some() || self.wire.as_ref().is_some_and(Wire::is_paused)
    }

    /// Whether the thread could be handed off once paused: its session is open, and it isn't
    /// starting or ending a turn, or waiting on the agent for anything but the turn.
    pub fn can_hand_off(&self) -> bool {
        self.view.state.status == ConnectionStatus::Ready
            && self.session.is_some()
            && (self.process.is_some() || self.adopted_process.is_some())
            && self.queued_prompts.is_empty()
            && self.after_turn.is_empty()
            && self.wire.as_ref().is_some_and(|wire| {
                wire.prompt_in_flight()
                    .is_ok_and(|prompt| prompt.is_some() == self.is_working())
            })
    }

    /// What another server needs to take over the paused agent with [`Self::adopt`]. Fails
    /// when the thread can't be handed off as it is; then [`Self::resume`] it. Until
    /// [`Self::release`], this thread still runs the agent.
    pub fn hand_off(&mut self) -> Result<HandedOffAgent> {
        anyhow::ensure!(self.can_hand_off(), "the agent is busy with something else");
        let session_id = self
            .session
            .as_ref()
            .map(|session| session.session_id.clone())
            .context("the session isn't open")?;
        let pid = self.process.as_ref().map(|process| process.pid);
        let pid = pid.or_else(|| self.adopted_process.as_ref().map(|guard| guard.0.pid));
        let pid = pid.context("the agent's process is unknown")?;
        let wire = self
            .wire
            .as_mut()
            .context("the agent isn't connected")?
            .hand_off()?;
        anyhow::ensure!(
            wire.prompt_id.is_some() == (self.is_working() && !self.working_on_its_own),
            "the turn is starting or ending"
        );
        let mut view = self.view.clone();
        // The new connection is asked again, which shows them again.
        view.state.permission_requests.clear();
        view.state.elicitations.clear();
        Ok(HandedOffAgent {
            snapshot: AgentSnapshot {
                view,
                session_id,
                previous_session: self.previous_session.clone(),
                pending_title: self.pending_title.clone(),
                mcp_servers: self.mcp_servers.clone(),
                defaults: self.defaults.clone(),
                stderr_lines: self.stderr_lines.iter().cloned().collect(),
                pid,
                prompt_id: wire.prompt_id,
                working_on_its_own: self.working_on_its_own,
                subagent_sessions: self.subagent_sessions.clone(),
                unanswered: wire.unanswered,
                stdout_rest: wire.stdout_rest,
                stderr_rest: wire.stderr_rest,
            },
            stdin: wire.stdin,
            stdout: wire.stdout,
            stderr: wire.stderr,
        })
    }

    /// Goes on after a pause, as when handing off failed.
    pub fn resume(&mut self) {
        if let Some(slot) = self.pausing.take() {
            let mut pause = lock_slot(&slot);
            pause.called_off = true;
            if let (Some(parked), Some(wire)) = (pause.parked.take(), &mut self.wire) {
                wire.park(parked);
            }
        }
        if let (Some(wire), Some(runtime)) = (&mut self.wire, &self.runtime) {
            wire.resume(runtime);
        }
    }

    /// Leaves the agent running when the thread is dropped: another server took it over.
    pub fn release(&mut self) {
        if let Some(process) = &self.process {
            process.armed.store(false, Ordering::SeqCst);
        }
        if let Some(guard) = &self.adopted_process {
            guard.0.armed.store(false, Ordering::SeqCst);
        }
    }

    /// Takes over an agent another server handed off (see [`Self::hand_off`]): a new
    /// connection on its pipes goes on with the open session and the turn in progress, without
    /// starting the agent again.
    pub fn adopt(
        runtime: tokio::runtime::Handle,
        snapshot: AgentSnapshot,
        stdin: OwnedFd,
        stdout: OwnedFd,
        stderr: OwnedFd,
        terminal_host: Option<TerminalHost>,
    ) -> Result<(Self, ThreadInbox)> {
        let process = AgentProcess {
            pid: snapshot.pid,
            armed: Arc::new(AtomicBool::new(true)),
        };
        let (mut this, inbox) = Self::new(
            Some(runtime.clone()),
            snapshot.view.state.agent_name.clone(),
            ConnectionStatus::Connecting,
            snapshot.view.state.cwd.clone(),
        );
        // Stops the agent if adopting fails below.
        this.adopted_process = Some(ProcessGuard(process));
        let is_working = snapshot.view.is_working();
        this.view = snapshot.view;
        // The message to retry stayed with the server it came from.
        this.view.state.failed_message = None;
        this.has_conversation = true;
        // Ready once the new connection is.
        this.view.state.status = ConnectionStatus::Connecting;
        this.previous_session = snapshot.previous_session;
        this.pending_title = snapshot.pending_title;
        this.mcp_servers = snapshot.mcp_servers;
        this.defaults = snapshot.defaults;
        this.stderr_lines = snapshot.stderr_lines.into();
        this.subagent_sessions = snapshot.subagent_sessions;
        this.terminal_host = terminal_host.clone();
        if is_working {
            this.emit(AgentThreadEvent::WorkingChanged(true));
        }
        if snapshot.working_on_its_own {
            this.working_on_its_own = true;
            this.own_work_from = this.view.entries.len();
            this.check_own_work_later();
        }

        let sender = this.sender();
        let (incoming, incoming_lines) = mpsc::unbounded();
        let (outgoing_lines, outgoing) = mpsc::unbounded();
        let on_stderr = stderr_reporter(sender.clone());
        let on_exit: Arc<dyn Fn() + Send + Sync> = {
            let sender = sender.clone();
            Arc::new(move || {
                sender
                    .send(MessageKind::Exited("The agent exited.".to_string()))
                    .ok();
            })
        };
        let mut wire = Wire::new(
            &runtime,
            stdin,
            stdout,
            stderr,
            (snapshot.stdout_rest, snapshot.stderr_rest),
            incoming,
            outgoing,
            on_stderr,
            Some(on_exit),
        )?;
        if let Some(id) = &snapshot.prompt_id {
            wire.take_answer(id);
        }
        wire.receive_again(snapshot.unanswered);
        wire.resume(&runtime);
        this.wire = Some(wire);

        let session_id = snapshot.session_id;
        let transport = transport(outgoing_lines, incoming_lines);
        let (connection_future, connection) = client_connection(transport, sender, terminal_host);
        let sender = this.sender();
        this.spawn_task(async move {
            let mut connection_tasks = JoinSet::new();
            connection_tasks.spawn(connection_future);
            match connection.await {
                Ok(connection) => {
                    sender.send(MessageKind::Reconnected(connection)).ok();
                }
                Err(_) => {
                    sender
                        .send(MessageKind::Exited(
                            "The agent's connection closed.".to_string(),
                        ))
                        .ok();
                }
            }
            while connection_tasks.join_next().await.is_some() {}
        });
        this.session = None;
        this.pending_session = Some(session_id);
        Ok((this, inbox))
    }

    fn reconnected(&mut self, connection: ConnectionTo<Agent>) {
        let Some(session_id) = self.pending_session.take() else {
            return;
        };
        self.connection = Some(connection.clone());
        self.session = Some(Session {
            connection,
            session_id,
        });
        self.view.state.status = ConnectionStatus::Ready;
        for prompt in std::mem::take(&mut self.queued_prompts) {
            self.send_to_agent(prompt);
        }
    }

    /// Zed's "Reauthenticate": shows the agent's login methods again.
    pub fn reauthenticate(&mut self) {
        if self.view.state.auth_methods.is_empty() || self.connection.is_none() {
            return;
        }
        self.view.state.status = ConnectionStatus::AuthRequired;
        self.view.state.auth_error = None;
        self.view.state.login_notice = None;
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
        self.view.state.login_notice = None;
        self.spawn(async move { MessageKind::LoggedOut(request.await.map(|_| ())) });
    }

    /// For a connection made from settings: opens a fresh empty session (sending no prompt) to
    /// learn again whether the agent is logged in, e.g. after logging in through a terminal.
    pub fn check_login(&mut self) {
        if self.opens_session || self.connection.is_none() {
            return;
        }
        self.drop_session();
        self.view.state.login_notice = None;
        self.open_session();
    }

    /// Settings for new sessions; see [`SessionDefaults`].
    pub fn set_defaults(&mut self, defaults: SessionDefaults) {
        self.defaults = defaults;
    }

    /// Applies the defaults the session doesn't already match. Values the agent no longer
    /// offers are skipped.
    fn apply_defaults(&mut self) {
        let defaults = self.defaults.clone();
        if let Some(mode) = defaults.mode
            && let Some(modes) = &self.view.state.modes
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
                .view
                .state
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

    fn open_session(&mut self) {
        let Some(connection) = self.connection.clone() else {
            return;
        };
        self.view.state.status = ConnectionStatus::Connecting;
        self.view.state.auth_error = None;
        self.end_subagents();
        // The new session's are new.
        self.subagent_sessions.clear();
        self.dropping_replay = self.has_conversation && self.previous_session.is_some();
        let opening = open_session(
            connection.clone(),
            self.view.state.capabilities.clone(),
            self.view.state.cwd.clone(),
            self.previous_session.clone(),
            self.mcp_servers.clone(),
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
        self.dropping_replay = false;
        match result {
            Ok(setup) => {
                self.has_conversation = true;
                self.view.state.config_options = setup.config_options;
                self.view.state.modes = setup.modes;
                self.view.state.session_restore = Some(setup.restore);
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
                self.view.state.status = ConnectionStatus::Ready;
                self.view.state.logged_in = Some(true);
                self.view.state.auth_description = None;
                if setup.restore == SessionRestore::New && self.opens_session {
                    self.apply_defaults();
                }
                // Going on in the new session would have the agent answer without the
                // conversation, unaware of it, so the user decides how it goes on.
                self.view.state.lost_history =
                    (setup.restore == SessionRestore::Unavailable).then(|| self.why_history_lost());
                if self.view.state.lost_history.is_some() {
                    // One that failed before (for want of a login, often) would go there too.
                    if self.view.state.failed_message.is_some() {
                        self.view.state.failed_message = Some(FailedMessage::LostHistory);
                    }
                    self.fail_queued_prompts(FailedMessage::LostHistory);
                    self.set_working(false);
                    return;
                }
                for prompt in std::mem::take(&mut self.queued_prompts) {
                    self.send_to_agent(prompt);
                }
            }
            Err(error) if is_auth_required(&error) => {
                self.view.state.status = ConnectionStatus::AuthRequired;
                self.found_logged_out();
                self.view.state.auth_description = auth_description(&error);
                self.fail_queued_prompts(FailedMessage::NeedsLogin);
                self.set_working(false);
            }
            Err(error) => self.fail(format!("starting a session: {}", error_message(&error))),
        }
    }

    /// The agent asked for a login, so whatever logged it in, agentZ or something else, no
    /// longer does.
    fn found_logged_out(&mut self) {
        if self.view.state.logged_in != Some(false) {
            self.emit(AgentThreadEvent::LoggedOut);
        }
        self.view.state.logged_in = Some(false);
    }

    /// The user's last message didn't get through: it waits for their Retry
    /// ([`Self::retry_message`]).
    fn message_failed(&mut self, reason: FailedMessage) {
        if self.last_message.is_some() {
            self.view.state.failed_message = Some(reason);
        }
    }

    /// The message waiting for the session can't go to it: the agent asked for a login before
    /// its session opened (and the message fails as one the agent asks for a login at does), or
    /// the session has lost the conversation. The user retries it, rather than it going by
    /// itself after a login.
    fn fail_queued_prompts(&mut self, reason: FailedMessage) {
        // Only one waits: the thread works while it does, and takes no other.
        let Some(parts) = self.queued_prompts.pop() else {
            return;
        };
        self.queued_prompts.clear();
        // The conversation it continues is still to go, in `handoff_to_send`.
        self.last_message = Some(SentMessage {
            parts,
            handoff: None,
        });
        self.message_failed(reason);
    }

    /// Sends the message that didn't get through ([`ThreadState::failed_message`]) again, as it
    /// went, once the agent is ready for it. The thread shows it once. One that failed for the
    /// lost conversation goes to the new session anyway, and so do the messages after it.
    pub fn retry_message(&mut self) {
        if self.view.state.failed_message.is_none()
            || self.view.state.status != ConnectionStatus::Ready
            || self.is_working()
        {
            return;
        }
        let Some(message) = self.last_message.take() else {
            return;
        };
        if self.view.state.failed_message == Some(FailedMessage::LostHistory) {
            self.view.state.lost_history = None;
        }
        self.view.state.failed_message = None;
        self.view.state.turn_error = None;
        if message.handoff.is_some() {
            self.handoff_to_send = message.handoff;
        }
        // The new reply isn't more of what the failed turn said.
        self.new_message_due = true;
        self.send_to_agent(message.parts);
    }

    /// Logs in with one of the agent's own methods, then opens the session. `meta` carries
    /// what the method takes from the user (an API key, a gateway), as the agent reads it.
    pub fn authenticate(&mut self, method_id: acp::AuthMethodId, meta: Option<acp::Meta>) {
        let Some(connection) = self.connection.clone() else {
            return;
        };
        if self.view.state.authenticating.is_some() {
            return;
        }
        let method_name = self
            .view
            .state
            .auth_methods
            .iter()
            .find(|method| *method.id() == method_id)
            .map(|method| SharedString::from(method.name().to_string()));
        let request = connection
            .send_request(acp::AuthenticateRequest::new(method_id.clone()).meta(meta))
            .block_task();
        self.view.state.login_notice = None;
        self.view.state.auth_error = None;
        self.view.state.auth_links.clear();
        self.view.state.auth_code = None;
        self.view.state.login_page = None;
        self.view.state.authenticating = Some(method_id);
        self.spawn(async move {
            MessageKind::Authenticated {
                method_name,
                result: request.await.map(|_| ()),
            }
        });
    }

    /// The conversation this thread continues, to go with its first message.
    pub fn set_handoff(&mut self, handoff: Option<PendingHandoff>) {
        self.view.state.handoff = handoff;
    }

    /// Starts without the conversation the thread would have brought.
    pub fn drop_handoff(&mut self) {
        if self.view.state.handoff.take().is_some() || self.handoff_to_send.take().is_some() {
            self.emit(AgentThreadEvent::HandoffDropped);
        }
    }

    /// Whether the user has sent a message, or queued one.
    pub fn has_user_message(&self) -> bool {
        self.view
            .entries
            .iter()
            .any(|entry| matches!(entry, Entry::UserMessage(_)))
    }

    /// The agent tried to open a page in a browser, and agentZ's `xdg-open` (or `open`) asked
    /// here first. While it's logging in, through `authenticate` or the connection's login
    /// terminal, the page goes to the clients if `for_clients` (see
    /// [`ThreadState::login_page`]). Otherwise, while its session opens, the page is a login
    /// nobody asked for ([`Self::started_login_unasked`]), and a connection made only to log in
    /// or out opens no page at all. An `Err` leaves the page to the real program.
    pub fn open_login_page(
        &mut self,
        url: SharedString,
        in_terminal_login: bool,
        for_clients: bool,
    ) -> std::result::Result<(), String> {
        let logging_in = self.view.state.authenticating.is_some()
            || self.authenticate_once_connected.is_some()
            || in_terminal_login;
        if !logging_in {
            if self.view.state.status == ConnectionStatus::Connecting {
                self.started_login_unasked();
                return Ok(());
            }
            if !self.opens_session {
                return Ok(());
            }
            return Err(format!("{} isn't logging in", self.view.state.agent_name));
        }
        if !for_clients {
            return Err("login pages open on the agent's machine".into());
        }
        self.view.state.login_page = Some(url);
        Ok(())
    }

    /// The agent opened a page while its session opens, with nobody asking it to log in: a login
    /// it started on its own, which the session waits on (Antigravity's, with a login method in
    /// its settings and none stored, waits minutes on Google's). So it's logged out, and it
    /// starts again without a session, for the user to log in. Messages sent meanwhile go once
    /// the session opens.
    fn started_login_unasked(&mut self) {
        self.found_logged_out();
        // Started again already, or not up yet: once it is, its session opens, and the page
        // comes again.
        if self.waits_for_login || self.view.state.command.is_none() {
            return;
        }
        self.restart_keeping_prompts();
        self.waits_for_login = true;
    }

    /// A terminal login starts over, so the page its last run asked for is moot.
    pub fn terminal_login_started(&mut self) {
        self.view.state.login_page = None;
    }

    /// A terminal login method exited successfully. The agent is restarted to pick up its new
    /// login, as Zed and t3code do, and the session opens again.
    pub fn terminal_login_finished(&mut self, method_id: &acp::AuthMethodId) {
        let method_name = self
            .view
            .state
            .auth_methods
            .iter()
            .find(|method| method.id() == method_id)
            .map(|method| SharedString::from(method.name().to_string()));
        if let Some(method_name) = method_name {
            self.emit(AgentThreadEvent::LoggedIn(method_name));
        }
        if !self.opens_session {
            self.view.state.login_notice = Some("Logged in.".into());
        }
        self.reload();
    }

    /// Tries to open the session again, e.g. after logging in through a terminal.
    pub fn retry_session(&mut self) {
        if self.view.state.status == ConnectionStatus::AuthRequired {
            self.open_session();
        }
    }

    pub fn clear_plan(&mut self) {
        self.view.state.plan.clear();
        self.conversation_revision += 1;
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
        let previous = self.view.state.config_options.clone();
        if let Some(option) = self
            .view
            .state
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
        if self.session.is_none() || self.view.state.modes.is_none() {
            return;
        }
        self.emit(AgentThreadEvent::ModeChanged(mode_id.clone()));
        self.send_mode(mode_id);
    }

    fn send_mode(&mut self, mode_id: acp::SessionModeId) {
        let Some(session) = &self.session else {
            return;
        };
        let Some(modes) = &mut self.view.state.modes else {
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

    pub fn send(&mut self, text: String) {
        self.send_message(vec![MessagePart::Text(text)]);
    }

    /// Sends a message of text and what's mentioned in it.
    pub fn send_message(&mut self, parts: Vec<MessagePart>) {
        let parts = trim_message(parts);
        if parts.is_empty() || self.is_working() || self.runs_in_parent {
            return;
        }
        if !self
            .view
            .entries
            .iter()
            .any(|entry| matches!(entry, Entry::UserMessage(_)))
        {
            self.emit(AgentThreadEvent::FirstPrompt(message_title(&parts)));
        }
        self.view
            .state
            .sent_times
            .push((self.view.entries.len(), SystemTime::now()));
        self.push_entry(Entry::UserMessage(message_markdown(&parts)));
        if let Some(handoff) = self.view.state.handoff.take() {
            self.handoff_to_send = Some(handoff);
        }
        self.view.state.turn_error = None;
        // A new message takes the place of one that didn't get through.
        self.view.state.failed_message = None;
        self.last_message = None;
        match self.view.state.status {
            ConnectionStatus::Ready if self.view.state.lost_history.is_some() => {
                self.last_message = Some(SentMessage {
                    parts,
                    handoff: None,
                });
                self.message_failed(FailedMessage::LostHistory);
            }
            ConnectionStatus::Ready => self.send_to_agent(parts),
            ConnectionStatus::Connecting => {
                self.queued_prompts.push(parts);
                self.set_working(true);
            }
            // As if the agent had asked for the login at it: the user retries it once logged in.
            ConnectionStatus::AuthRequired => {
                self.last_message = Some(SentMessage {
                    parts,
                    handoff: None,
                });
                self.message_failed(FailedMessage::NeedsLogin);
            }
            ConnectionStatus::Failed(_) => {}
        }
    }

    pub fn steer(&mut self, text: String) {
        self.steer_message(vec![MessagePart::Text(text)]);
    }

    /// Sends a message into the turn the agent is working on, when the agent takes one
    /// ([`ThreadState::supports_steering`]). It shows in the thread once the agent takes it.
    /// One it doesn't take goes once the turn ends, and one sent while the agent isn't working
    /// goes at once, as [`Self::send_message`] sends it.
    pub fn steer_message(&mut self, parts: Vec<MessagePart>) {
        let parts = trim_message(parts);
        if parts.is_empty() || self.runs_in_parent {
            return;
        }
        if !self.is_working() {
            self.send_message(parts);
            return;
        }
        let Some(session) = self
            .session
            .as_ref()
            .filter(|_| self.view.state.supports_steering)
        else {
            self.after_turn.push_back(parts);
            return;
        };
        let request = agent_client_protocol::UntypedMessage::new(
            STEERING_REQUEST,
            serde_json::json!({
                "sessionId": session.session_id,
                "prompt": self.content_blocks(parts.clone()),
                // Otherwise Claude Agent starts a turn of its own when the turn has just
                // ended, one that no `session/prompt` waits for.
                "_meta": {"steering": {"idleBehavior": "promptRequired"}},
            }),
        );
        let request = match request {
            Ok(request) => request,
            Err(error) => {
                log::error!("failed to write the message into the turn: {error:?}");
                self.after_turn.push_back(parts);
                return;
            }
        };
        let response = session.connection.send_request(request).block_task();
        self.spawn(async move {
            MessageKind::Steered {
                parts,
                result: response.await,
            }
        });
    }

    /// The agent's answer to [`Self::steer_message`]: `injected` when the message joined the
    /// turn, `promptRequired` when the turn had ended (Claude Agent), `startedNewTurn` when it
    /// started a turn with it instead (Codex), or `failed`.
    fn steered(
        &mut self,
        parts: Vec<MessagePart>,
        result: std::result::Result<Value, agent_client_protocol::Error>,
    ) {
        let outcome = match &result {
            Ok(response) => response.get("outcome").and_then(Value::as_str),
            Err(_) => None,
        };
        match outcome {
            Some("injected" | "startedNewTurn") => {
                self.view
                    .state
                    .sent_times
                    .push((self.view.entries.len(), SystemTime::now()));
                self.push_entry(Entry::UserMessage(message_markdown(&parts)));
            }
            Some("promptRequired") => self.send_after_turn(parts),
            _ => {
                match &result {
                    Ok(response) => {
                        log::warn!("the agent didn't take a message into its turn: {response}")
                    }
                    Err(error) => log::warn!(
                        "the agent didn't take a message into its turn: {}",
                        error_message(error)
                    ),
                }
                self.send_after_turn(parts);
            }
        }
    }

    /// Sends a message once no turn is running.
    pub fn send_after_turn(&mut self, parts: Vec<MessagePart>) {
        if self.is_working() {
            self.after_turn.push_back(parts);
        } else {
            self.send_message(parts);
        }
    }

    /// Sends a message an agent wrote, marked as coming from it.
    pub fn send_from(&mut self, text: String, from: projects::ThreadCreator) {
        let index = self.view.entries.len();
        self.send(text);
        if self.view.entries.len() > index {
            self.view.state.prompts_from_agents.push((index, from));
        }
    }

    /// Keeps the open session as the one to restore. A new session is only worth restoring once it
    /// has a prompt: agents don't save empty sessions, so loading one later would fail.
    fn remember_session(&mut self) {
        let Some(session) = &self.session else {
            return;
        };
        if self.previous_session.as_ref() != Some(&session.session_id) {
            let session_id = session.session_id.clone();
            // Later retries should restore this session rather than start another.
            self.previous_session = Some(session_id.clone());
            self.session_login = None;
            self.emit(AgentThreadEvent::SessionStarted(session_id));
        }
        self.note_session_login();
    }

    /// Keeps the login the agent reports while the session to restore is open: an agent that
    /// goes on with a session on another account (Claude Agent, Codex) uses it there from now.
    fn note_session_login(&mut self) {
        let in_use = self
            .session
            .as_ref()
            .is_some_and(|session| self.previous_session.as_ref() == Some(&session.session_id));
        let Some(status) = self
            .view
            .state
            .auth_status
            .as_ref()
            .filter(|status| in_use && status.is_logged_in())
        else {
            return;
        };
        let login = status.identity();
        let is_news = self
            .session_login
            .as_ref()
            .is_none_or(|known| known.differs_from(&login) || known.is_completed_by(&login));
        if is_news {
            self.session_login = Some(login.clone());
            self.emit(AgentThreadEvent::SessionLogin(login));
        }
    }

    /// Why the previous session didn't load: as far as agentZ can tell, because the agent
    /// reports another login than the one that session was used with.
    fn why_history_lost(&self) -> LostHistory {
        let account_changed = self
            .session_login
            .as_ref()
            .zip(self.view.state.auth_status.as_ref())
            .is_some_and(|(known, status)| {
                status.is_logged_in() && known.differs_from(&status.identity())
            });
        if account_changed {
            LostHistory::AccountChanged
        } else {
            LostHistory::Unexplained
        }
    }

    /// The message as the agent takes it.
    fn content_blocks(&self, parts: Vec<MessagePart>) -> Vec<acp::ContentBlock> {
        let capabilities = &self.view.state.capabilities.prompt_capabilities;
        parts
            .into_iter()
            .filter_map(|part| {
                part.into_content_block(capabilities.embedded_context, capabilities.image)
            })
            .collect()
    }

    fn send_to_agent(&mut self, parts: Vec<MessagePart>) {
        self.remember_session();
        if self.session.is_none() {
            return;
        }
        let mut prompt = self.content_blocks(parts.clone());
        let handoff = self.handoff_to_send.take();
        if let Some(handoff) = &handoff {
            // Embedded, the agent tells it from the message: replays show only the message.
            let block = if self
                .view
                .state
                .capabilities
                .prompt_capabilities
                .embedded_context
            {
                acp::ContentBlock::Resource(acp::EmbeddedResource::new(
                    acp::EmbeddedResourceResource::TextResourceContents(
                        acp::TextResourceContents::new(handoff.text.clone(), "agentz://handoff")
                            .mime_type("text/markdown".to_string()),
                    ),
                ))
            } else {
                acp::ContentBlock::Text(acp::TextContent::new(handoff.text.clone()))
            };
            prompt.insert(0, block);
            self.emit(AgentThreadEvent::HandoffSent(handoff.from));
        }
        self.last_message = Some(SentMessage { parts, handoff });
        let Some(session) = &self.session else {
            return;
        };
        let request = acp::PromptRequest::new(session.session_id.clone(), prompt);
        let connection = session.connection.clone();
        // The prompt's turn ends the work, if the agent was working on its own.
        self.working_on_its_own = false;
        self.own_work_ending = false;
        self.set_working(true);
        let Some(hook) = self.turn_hook.clone() else {
            let response = connection.send_request(request).block_task();
            self.spawn(async move { MessageKind::PromptFinished(response.await) });
            return;
        };
        let cancelled = Arc::new(AtomicBool::new(false));
        self.turn_cancelled = Some(cancelled.clone());
        self.spawn(async move {
            hook(TurnPoint::Starting).await;
            let response = if cancelled.load(Ordering::SeqCst) {
                Ok(acp::PromptResponse::new(acp::StopReason::Cancelled))
            } else {
                connection.send_request(request).block_task().await
            };
            hook(TurnPoint::Ended).await;
            MessageKind::PromptFinished(response)
        });
    }

    /// Asks the agent to stop the current turn.
    pub fn cancel(&mut self) {
        if !self.is_working() || self.runs_in_parent {
            return;
        }
        if let Some(cancelled) = &self.turn_cancelled {
            cancelled.store(true, Ordering::SeqCst);
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
        // No prompt's answer will end it.
        if self.working_on_its_own {
            self.end_own_work();
        }
    }

    /// Asks the agent to stop work it left running ([`ThreadState::background_tasks`]).
    pub fn stop_background_task(&mut self, task_id: &str) {
        let Some(session) = &self.session else {
            return;
        };
        let Some(task) = self
            .view
            .state
            .background_tasks
            .iter_mut()
            .find(|task| task.id.as_ref() == task_id && task.can_stop && !task.stopping)
        else {
            return;
        };
        task.stopping = true;
        let request = agent_client_protocol::UntypedMessage::new(
            STOP_BACKGROUND_TASK_REQUEST,
            serde_json::json!({
                "sessionId": session.session_id,
                "asyncTaskId": task_id,
            }),
        );
        let request = match request {
            Ok(request) => request,
            Err(error) => {
                log::error!("failed to ask the agent to stop a background task: {error:?}");
                task.stopping = false;
                return;
            }
        };
        let response = session.connection.send_request(request).block_task();
        let task_id = task.id.clone();
        self.stops_in_flight += 1;
        // Claude Agent notes the stop in the conversation.
        self.new_message_due = true;
        self.spawn(async move {
            MessageKind::BackgroundTaskStopped {
                task_id,
                result: response.await,
            }
        });
    }

    fn background_task_stopped(
        &mut self,
        task_id: SharedString,
        result: std::result::Result<Value, agent_client_protocol::Error>,
    ) {
        self.stops_in_flight = self.stops_in_flight.saturating_sub(1);
        let stopped = match &result {
            Ok(response) => response
                .get("stopped")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            Err(error) => {
                log::warn!(
                    "the agent didn't stop a background task: {}",
                    error_message(error)
                );
                false
            }
        };
        let tasks = &mut self.view.state.background_tasks;
        if stopped {
            tasks.retain(|task| task.id != task_id);
        } else if let Some(task) = tasks.iter_mut().find(|task| task.id == task_id) {
            task.stopping = false;
        }
    }

    fn apply_background_task_update(&mut self, update: BackgroundTaskUpdate) {
        // What a loading session replays is over. An adopted agent's session goes on.
        if self.session.is_none() && self.pending_session.is_none() {
            return;
        }
        let tasks = &mut self.view.state.background_tasks;
        match update {
            BackgroundTaskUpdate::AsyncTaskSpawned {
                async_task_id,
                name,
                description,
                task_type,
                can_stop,
                output_file_path,
                tool_call_id,
            } => {
                let task = BackgroundTask {
                    id: async_task_id.into(),
                    name: name
                        .or(description)
                        .unwrap_or_else(|| "Background task".to_string())
                        .into(),
                    kind: task_type.unwrap_or_else(|| "task".to_string()).into(),
                    tool_call_id: tool_call_id.map(acp::ToolCallId::new),
                    output_file: output_file_path,
                    progress: None,
                    paused: false,
                    can_stop,
                    stopping: false,
                    started_at: SystemTime::now(),
                };
                match tasks.iter_mut().find(|existing| existing.id == task.id) {
                    Some(existing) => *existing = task,
                    None => tasks.push(task),
                }
            }
            BackgroundTaskUpdate::AsyncTaskProgress {
                async_task_id,
                description,
                summary,
                output_file_path,
                tool_call_id,
            } => {
                let Some(task) = tasks
                    .iter_mut()
                    .find(|task| task.id.as_ref() == async_task_id)
                else {
                    return;
                };
                if let Some(description) = description {
                    task.name = description.into();
                }
                if let Some(summary) = summary {
                    task.progress = Some(summary.into());
                }
                if let Some(path) = output_file_path {
                    task.output_file = Some(path);
                }
                if let Some(id) = tool_call_id {
                    task.tool_call_id = Some(acp::ToolCallId::new(id));
                }
            }
            BackgroundTaskUpdate::AsyncTaskStateUpdate {
                async_task_id,
                state,
                output_file_path,
                tool_call_id,
            } => {
                let Some(index) = tasks
                    .iter()
                    .position(|task| task.id.as_ref() == async_task_id)
                else {
                    return;
                };
                match state.as_str() {
                    "running" | "paused" => {
                        let task = &mut tasks[index];
                        task.paused = state == "paused";
                        if let Some(path) = output_file_path {
                            task.output_file = Some(path);
                        }
                        if let Some(id) = tool_call_id {
                            task.tool_call_id = Some(acp::ToolCallId::new(id));
                        }
                    }
                    // Ended: the agent says how in the conversation, as it picks it up.
                    _ => {
                        let task = tasks.remove(index);
                        // A stop asked for here gets only the agent's note of it.
                        if !task.stopping && !self.is_working() {
                            self.expect_own_work();
                        }
                    }
                }
            }
        }
    }

    /// The agent's turn is over, but not its work: something it left running in the
    /// background, or what it goes on with once that ends.
    pub fn is_waiting(&self) -> bool {
        !self.view.state.background_tasks.is_empty() || self.own_work_due
    }

    fn expect_own_work(&mut self) {
        self.own_work_due = true;
        self.tasks_ended += 1;
        let ended = self.tasks_ended;
        self.spawn(async move {
            tokio::time::sleep(OWN_WORK_GRACE).await;
            MessageKind::OwnWorkOverdue(ended)
        });
    }

    /// Whether the update shows the agent working although no prompt is: Claude Agent going
    /// on once a background task it started ended. What follows a turn's end, or a session's
    /// loading, or a stop of a background task (the agent's note of it), doesn't count.
    fn starts_own_work(&self, update: &acp::SessionUpdate) -> bool {
        !self.is_working()
            && self.session.is_some()
            && self.view.state.status == ConnectionStatus::Ready
            && self.stops_in_flight == 0
            && matches!(
                update,
                acp::SessionUpdate::AgentMessageChunk(_)
                    | acp::SessionUpdate::AgentThoughtChunk(_)
                    | acp::SessionUpdate::ToolCall(_)
            )
    }

    fn start_own_work(&mut self) {
        self.working_on_its_own = true;
        self.own_work_from = self.view.entries.len();
        // Its reply is its own, not more of the last turn's.
        self.new_message_due = true;
        self.set_working(true);
        self.check_own_work_later();
    }

    fn check_own_work_later(&mut self) {
        let updates = self.own_work_updates;
        self.spawn(async move {
            tokio::time::sleep(OWN_WORK_QUIET).await;
            MessageKind::OwnWorkQuiet(updates)
        });
    }

    /// The agent's work of its own ended. It's a turn, so the owner's work after a turn (a
    /// checkpoint) is done for it too, and then what waited for it goes.
    fn end_own_work(&mut self) {
        self.working_on_its_own = false;
        let Some(hook) = self.turn_hook.clone() else {
            self.own_work_ended();
            return;
        };
        self.own_work_ending = true;
        self.spawn(async move {
            hook(TurnPoint::Ended).await;
            MessageKind::OwnWorkEnded
        });
    }

    fn own_work_ended(&mut self) {
        self.own_work_ending = false;
        self.set_working(false);
        if let Some(parts) = self.after_turn.pop_front() {
            self.send_after_turn(parts);
        }
    }

    pub fn respond_to_permission(
        &mut self,
        tool_call_id: &acp::ToolCallId,
        option_id: acp::PermissionOptionId,
    ) {
        self.view
            .state
            .permission_requests
            .retain(|request| &request.tool_call_id != tool_call_id);
        let Some(index) = self
            .permission_responders
            .iter()
            .position(|(id, _)| id == tool_call_id)
        else {
            return;
        };
        let (_, responder) = self.permission_responders.remove(index);
        if let Err(error) = responder.respond(acp::RequestPermissionResponse::new(
            acp::RequestPermissionOutcome::Selected(acp::SelectedPermissionOutcome::new(option_id)),
        )) {
            log::error!("failed to answer the agent's permission request: {error:?}");
        }
    }

    fn cancel_permission_requests(&mut self) {
        self.view.state.permission_requests.clear();
        for (_, responder) in self.permission_responders.drain(..) {
            cancel_permission(responder);
        }
    }

    /// Answers an elicitation. Accepting a URL keeps it shown, opened, until the agent says
    /// it's done (`elicitation/complete`), as in Zed.
    pub fn respond_to_elicitation(&mut self, id: u64, action: acp::ElicitationAction) {
        let Some(index) = self
            .elicitation_responders
            .iter()
            .position(|responder| responder.id == id)
        else {
            return;
        };
        let ElicitationResponder { responder, .. } = self.elicitation_responders.remove(index);
        let accepted = matches!(action, acp::ElicitationAction::Accept(_));
        self.view.state.elicitations.retain_mut(|elicitation| {
            if elicitation.id != id {
                return true;
            }
            elicitation.opened = accepted && elicitation.url().is_some();
            elicitation.opened
        });
        if let Err(error) = responder.respond(acp::CreateElicitationResponse::new(action)) {
            log::error!("failed to answer the agent's elicitation: {error:?}");
        }
    }

    /// Hides an opened URL elicitation, which was answered when it was opened.
    pub fn dismiss_elicitation(&mut self, id: u64) {
        self.view
            .state
            .elicitations
            .retain(|elicitation| !(elicitation.id == id && elicitation.opened));
    }

    /// Cancels the elicitations that match, and hides them.
    fn cancel_elicitations(&mut self, matches: impl Fn(&Elicitation) -> bool) {
        let mut cancelled = Vec::new();
        self.view.state.elicitations.retain(|elicitation| {
            let keep = !matches(elicitation);
            if !keep {
                cancelled.push(elicitation.id);
            }
            keep
        });
        let (cancel, keep): (Vec<_>, Vec<_>) = std::mem::take(&mut self.elicitation_responders)
            .into_iter()
            .partition(|responder| cancelled.contains(&responder.id));
        self.elicitation_responders = keep;
        for ElicitationResponder { responder, .. } in cancel {
            cancel_elicitation(responder);
        }
    }

    fn set_working(&mut self, working: bool) {
        // Work that starts is what was due; an agent that stops won't go on.
        self.own_work_due = false;
        if !working {
            self.working_on_its_own = false;
            self.own_work_ending = false;
        }
        if working == self.is_working() {
            return;
        }
        if !working
            && let Some(duration) = self
                .view
                .state
                .turn_started_at
                .and_then(|started| started.elapsed().ok())
        {
            self.view.state.finished_turns.push(TurnTime {
                entries_end: self.view.entries.len(),
                duration,
            });
            self.conversation_revision += 1;
        }
        self.view.state.turn_started_at = working.then(SystemTime::now);
        self.emit(AgentThreadEvent::WorkingChanged(working));
    }

    fn fail(&mut self, error: String) {
        let mut message = error;
        if !self.stderr_lines.is_empty() {
            message.push_str("\n\n");
            message.push_str(&Vec::from(self.stderr_lines.clone()).join("\n"));
        }
        self.view.state.status = ConnectionStatus::Failed(message.into());
        self.session = None;
        self.end_subagents();
        self.queued_prompts.clear();
        self.after_turn.clear();
        self.view.state.background_tasks.clear();
        self.stops_in_flight = 0;
        self.view.state.authenticating = None;
        self.view.state.auth_links.clear();
        self.view.state.auth_code = None;
        self.view.state.login_page = None;
        self.cancel_elicitations(|_| true);
        self.set_working(false);
    }

    fn record_stderr(&mut self, line: String) {
        if self.view.state.authenticating.is_some() {
            // Device logins often print the code on the line after the one that mentions it.
            let previous = self.stderr_lines.back().map(String::as_str).unwrap_or("");
            if let Some(code) = login_code(&line).or_else(|| {
                login_code(&format!("{previous}\n{line}")).filter(|code| line.contains(code))
            }) {
                self.view.state.auth_code = Some(code.into());
            }
            for link in login_links(&line) {
                if !self
                    .view
                    .state
                    .auth_links
                    .iter()
                    .any(|known| *known == link)
                {
                    self.view.state.auth_links.push(link.into());
                }
            }
        }
        if self.stderr_lines.len() == STDERR_LINES_KEPT {
            self.stderr_lines.pop_front();
        }
        self.stderr_lines.push_back(line);
    }

    fn handle_incoming(&mut self, incoming: Incoming) {
        match incoming {
            Incoming::Notification(notification) => {
                if let Some(&running) = self.subagent_sessions.get(&*notification.session_id.0) {
                    // The subthread starts with the prompt, so what the agent sends as the
                    // user's there is only its tools' results.
                    if running
                        && !matches!(notification.update, acp::SessionUpdate::UserMessageChunk(_))
                    {
                        self.emit(AgentThreadEvent::SubagentUpdate {
                            session: notification.session_id.0.to_string(),
                            update: notification.update,
                        });
                    }
                    return;
                }
                if self.dropping_replay && is_conversation_update(&notification.update) {
                    return;
                }
                if self.starts_own_work(&notification.update) {
                    self.start_own_work();
                }
                // Claude Agent reports its cost with each result, its own work's too.
                let ends_own_work = self.working_on_its_own
                    && matches!(
                        &notification.update,
                        acp::SessionUpdate::UsageUpdate(update) if update.cost.is_some()
                    );
                if self.working_on_its_own && is_conversation_update(&notification.update) {
                    self.own_work_updates += 1;
                }
                self.apply_update(notification.update);
                if let Some(title) = self.pending_title.take() {
                    self.emit(AgentThreadEvent::TitleChanged(title));
                }
                if ends_own_work {
                    self.end_own_work();
                }
            }
            Incoming::Permission(request, responder) => {
                let tool_call_id = request.tool_call.tool_call_id.clone();
                // Permission requests can describe a tool call we haven't been told about yet.
                // A subagent's goes to its subthread, and the request is asked here, at its card.
                let is_subagents = self
                    .subagent_sessions
                    .get(&*request.session_id.0)
                    .is_some_and(|running| *running);
                let subagent_card = if is_subagents {
                    self.emit(AgentThreadEvent::SubagentUpdate {
                        session: request.session_id.0.to_string(),
                        update: acp::SessionUpdate::ToolCallUpdate(request.tool_call.clone()),
                    });
                    Some(subagent_card_id(&request.session_id.0))
                } else {
                    self.apply_tool_call_update(request.tool_call.clone());
                    None
                };
                let title = request.tool_call.fields.title.clone().unwrap_or_default();
                self.permission_responders
                    .push((tool_call_id.clone(), responder));
                self.view.state.permission_requests.push(PermissionRequest {
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
                    subagent_card,
                });
            }
            Incoming::Elicitation(request, responder) => {
                if let Err(message) = validate_elicitation(&request) {
                    if let Err(error) =
                        responder.respond_with_error(acp::Error::invalid_params().data(message))
                    {
                        log::error!("failed to refuse the agent's elicitation: {error:?}");
                    }
                    return;
                }
                let id = self.next_elicitation_id;
                self.next_elicitation_id += 1;
                let (answered, answered_receiver) = oneshot::channel::<()>();
                let cancellation = responder.cancellation();
                let sender = self.sender();
                self.spawn_task(async move {
                    let cancelled = std::pin::pin!(cancellation.cancelled());
                    if let futures::future::Either::Left(_) =
                        futures::future::select(cancelled, answered_receiver).await
                    {
                        sender.send(MessageKind::ElicitationCancelled(id)).ok();
                    }
                });
                self.elicitation_responders.push(ElicitationResponder {
                    id,
                    responder,
                    _answered: answered,
                });
                self.view.state.elicitations.push(Elicitation {
                    id,
                    request,
                    opened: false,
                });
            }
        }
    }

    fn subagent_notice(&mut self, session: acp::SessionId, notice: SubagentNotice) {
        match notice {
            SubagentNotice::SubagentSpawned {
                subagent_session_id,
                name,
                task,
                prompt,
            } => {
                // Replayed as the session loads: the subthread is there already.
                if self.view.state.status != ConnectionStatus::Ready {
                    self.subagent_sessions.insert(subagent_session_id, false);
                    return;
                }
                if self.subagent_sessions.contains_key(&subagent_session_id) {
                    return;
                }
                let parent_session = self
                    .subagent_sessions
                    .contains_key(&*session.0)
                    .then(|| session.0.to_string());
                self.subagent_sessions
                    .insert(subagent_session_id.clone(), true);
                let task = prompt.or(task).unwrap_or_default();
                self.emit(AgentThreadEvent::SubagentStarted(Subagent {
                    name: name
                        .filter(|name| !name.trim().is_empty())
                        .unwrap_or_else(|| "Subagent".to_string()),
                    session: subagent_session_id,
                    parent_session,
                    task,
                }));
            }
            SubagentNotice::SubagentStateUpdate {
                subagent_session_id,
                state,
            } => {
                let Some(running) = self.subagent_sessions.get_mut(&subagent_session_id) else {
                    return;
                };
                if !std::mem::replace(running, false) {
                    return;
                }
                let end = match state.as_str() {
                    "completed" => projects::TaskEnd::Completed,
                    "failed" => projects::TaskEnd::Failed,
                    "cancelled" => projects::TaskEnd::Cancelled,
                    _ => projects::TaskEnd::Interrupted,
                };
                self.emit(AgentThreadEvent::SubagentEnded {
                    session: subagent_session_id,
                    end,
                });
            }
        }
    }

    /// The subagents still running stop with the agent, or with the session they run under.
    fn end_subagents(&mut self) {
        let mut ended = Vec::new();
        for (session, running) in &mut self.subagent_sessions {
            if std::mem::replace(running, false) {
                ended.push(session.clone());
            }
        }
        for session in ended {
            self.emit(AgentThreadEvent::SubagentEnded {
                session,
                end: projects::TaskEnd::Interrupted,
            });
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
                self.view.state.plan = plan
                    .entries
                    .into_iter()
                    .map(|entry| PlanItem {
                        content: entry.content,
                        status: entry.status,
                    })
                    .collect();
                self.conversation_revision += 1;
                if !self.view.entries.contains(&Entry::Plan) {
                    self.push_entry(Entry::Plan);
                }
            }
            acp::SessionUpdate::ConfigOptionUpdate(update) => {
                self.view.state.config_options = update.config_options;
            }
            acp::SessionUpdate::CurrentModeUpdate(update) => {
                if let Some(modes) = &mut self.view.state.modes {
                    modes.current_mode_id = update.current_mode_id;
                }
            }
            acp::SessionUpdate::UsageUpdate(update) => {
                self.view.state.usage = Some(ContextUsage {
                    used: update.used,
                    size: update.size,
                });
                if update.cost.is_some() {
                    self.view.state.cost = update.cost;
                }
            }
            acp::SessionUpdate::AvailableCommandsUpdate(update) => {
                self.view.state.available_commands = update.available_commands;
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
        let Some(text) = self.content_markdown(content) else {
            return;
        };
        let continues = !std::mem::take(&mut self.new_message_due);
        if let Some(existing) = self
            .view
            .entries
            .last_mut()
            .filter(|_| continues)
            .and_then(existing_text)
        {
            existing.push_str(&text);
            self.entry_changed(self.view.entries.len() - 1);
        } else {
            self.push_entry(new_entry(text));
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
            terminals: Vec::new(),
            images: Vec::new(),
            started_at: None,
            duration: None,
            subthread: None,
        };
        self.tool_content(tool_call.content).apply_to(&mut entry);
        let live = self.view.state.status == ConnectionStatus::Ready;
        if let Some(existing) = self.tool_call_mut(&entry.id) {
            entry.started_at = existing.started_at;
            entry.duration = existing.duration;
            entry.subthread = existing.subthread;
            note_end(&mut entry);
            *existing = entry;
        } else {
            entry.started_at = live.then(SystemTime::now);
            note_end(&mut entry);
            self.push_entry(Entry::ToolCall(entry));
        }
    }

    fn apply_tool_call_update(&mut self, update: acp::ToolCallUpdate) {
        let fields = update.fields;
        // Read before the tool call is borrowed to change, as it keeps the images.
        let content = fields.content.map(|content| self.tool_content(content));
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
                terminals: Vec::new(),
                images: Vec::new(),
                started_at: None,
                duration: None,
                subthread: None,
            };
            if let Some(content) = content {
                content.apply_to(&mut entry);
            }
            entry.started_at =
                (self.view.state.status == ConnectionStatus::Ready).then(SystemTime::now);
            note_end(&mut entry);
            self.push_entry(Entry::ToolCall(entry));
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
        if let Some(content) = content {
            content.apply_to(existing);
        }
        if let Some(raw_input) = fields.raw_input.as_ref() {
            existing.raw_input = raw_input_text(raw_input);
        }
        note_end(existing);
    }

    /// What a tool call shows of its content: text, diffs, terminals, and images, which are
    /// kept for the thread.
    fn tool_content(&self, content: Vec<acp::ToolCallContent>) -> ToolContent {
        let mut tool_content = ToolContent::default();
        for item in content {
            match item {
                acp::ToolCallContent::Content(content) => match content.content {
                    acp::ContentBlock::Text(text) => tool_content.text.push(text.text),
                    acp::ContentBlock::Image(image) => {
                        if let Some(id) = self.keep_image(&image.mime_type, &image.data) {
                            tool_content.images.push(id);
                        }
                    }
                    _ => {}
                },
                acp::ToolCallContent::Diff(diff) => tool_content.diffs.push(FileDiff {
                    path: diff.path,
                    old_text: diff.old_text,
                    new_text: diff.new_text,
                }),
                acp::ToolCallContent::Terminal(terminal) => tool_content
                    .terminals
                    .push(terminal.terminal_id.0.to_string()),
                _ => {}
            }
        }
        tool_content
    }

    /// A replayed or streamed chunk of a message as its entry shows it, mentions as links. The
    /// handoff a continued thread began with stays hidden.
    fn content_markdown(&self, content: acp::ContentBlock) -> Option<String> {
        match content {
            acp::ContentBlock::Text(text) => Some(text.text),
            acp::ContentBlock::ResourceLink(link) => Some(mention_link(&link.name, &link.uri)),
            acp::ContentBlock::Resource(resource) => {
                let uri = match resource.resource {
                    acp::EmbeddedResourceResource::TextResourceContents(contents) => contents.uri,
                    acp::EmbeddedResourceResource::BlobResourceContents(contents) => contents.uri,
                    _ => return None,
                };
                if uri == "agentz://handoff" {
                    return None;
                }
                let name = url::Url::parse(&uri)
                    .ok()
                    .and_then(|url| url.to_file_path().ok())
                    .map(|path| path_name(&path))
                    .unwrap_or_else(|| uri.rsplit('/').next().unwrap_or(&uri).to_string());
                Some(mention_link(&name, &uri))
            }
            acp::ContentBlock::Image(image) => Some(
                self.keep_image(&image.mime_type, &image.data)
                    .map_or_else(|| "`@Image`".to_string(), |id| id.markdown_link()),
            ),
            _ => None,
        }
    }

    /// The tool call, noted as changed since the caller changes it.
    fn tool_call_mut(&mut self, id: &acp::ToolCallId) -> Option<&mut ToolCall> {
        let index = self.view.entries.iter().rposition(
            |entry| matches!(entry, Entry::ToolCall(tool_call) if &tool_call.id == id),
        )?;
        self.entry_changed(index);
        match &mut self.view.entries[index] {
            Entry::ToolCall(tool_call) => Some(tool_call),
            _ => None,
        }
    }

    fn push_entry(&mut self, entry: Entry) {
        self.entry_changed(self.view.entries.len());
        self.view.entries.push(entry);
    }

    fn entry_changed(&mut self, index: usize) {
        self.conversation_revision += 1;
        self.entries_changed_from = Some(
            self.entries_changed_from
                .map_or(index, |changed_from| changed_from.min(index)),
        );
    }

    /// The first entry that changed since this was last called, if any did.
    pub fn take_entries_changed_from(&mut self) -> Option<usize> {
        self.entries_changed_from.take()
    }
}

/// Notes how long the tool call ran, once it ended, if it started here.
fn note_end(tool_call: &mut ToolCall) {
    if tool_call.duration.is_none() && !tool_call.is_running() {
        tool_call.duration = tool_call
            .started_at
            .and_then(|started| started.elapsed().ok());
    }
}

/// Whether the update is part of the conversation, which a loading session replays, rather
/// than the session's settings or usage.
fn is_conversation_update(update: &acp::SessionUpdate) -> bool {
    matches!(
        update,
        acp::SessionUpdate::UserMessageChunk(_)
            | acp::SessionUpdate::AgentMessageChunk(_)
            | acp::SessionUpdate::AgentThoughtChunk(_)
            | acp::SessionUpdate::ToolCall(_)
            | acp::SessionUpdate::ToolCallUpdate(_)
            | acp::SessionUpdate::Plan(_)
    )
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

/// A part of a message to the agent: its text, or what the server made of something the user
/// mentioned in it.
#[derive(Clone, Debug, PartialEq)]
pub enum MessagePart {
    Text(String),
    /// A file, with its contents when they're text.
    File {
        path: PathBuf,
        contents: Option<String>,
    },
    Folder(PathBuf),
    /// Another thread's conversation.
    Thread {
        uri: String,
        title: String,
        text: String,
    },
    /// An image kept for the thread, with its bytes in base64.
    Image {
        id: AttachmentId,
        data: String,
    },
}

impl MessagePart {
    /// As Zed sends mentions: a file's contents embedded when the agent takes them, else a link
    /// to it; an image only to an agent that takes images.
    fn into_content_block(self, embedded_context: bool, images: bool) -> Option<acp::ContentBlock> {
        Some(match self {
            MessagePart::Text(text) => acp::ContentBlock::Text(acp::TextContent::new(text)),
            MessagePart::File {
                path,
                contents: Some(contents),
            } if embedded_context => acp::ContentBlock::Resource(acp::EmbeddedResource::new(
                acp::EmbeddedResourceResource::TextResourceContents(
                    acp::TextResourceContents::new(contents, file_uri(&path)),
                ),
            )),
            MessagePart::File { path, .. } | MessagePart::Folder(path) => {
                acp::ContentBlock::ResourceLink(acp::ResourceLink::new(
                    path_name(&path),
                    file_uri(&path),
                ))
            }
            MessagePart::Thread { uri, text, .. } if embedded_context => {
                acp::ContentBlock::Resource(acp::EmbeddedResource::new(
                    acp::EmbeddedResourceResource::TextResourceContents(
                        acp::TextResourceContents::new(text, uri)
                            .mime_type("text/markdown".to_string()),
                    ),
                ))
            }
            MessagePart::Thread { text, .. } => {
                acp::ContentBlock::Text(acp::TextContent::new(text))
            }
            MessagePart::Image { id, data } if images => {
                acp::ContentBlock::Image(acp::ImageContent::new(data, id.mime_type()))
            }
            MessagePart::Image { .. } => return None,
        })
    }
}

/// A message without whitespace around it, and without empty text.
fn trim_message(parts: Vec<MessagePart>) -> Vec<MessagePart> {
    let mut parts: Vec<MessagePart> = parts
        .into_iter()
        .filter(|part| !matches!(part, MessagePart::Text(text) if text.is_empty()))
        .collect();
    if let Some(MessagePart::Text(text)) = parts.first_mut() {
        *text = text.trim_start().to_string();
    }
    if let Some(MessagePart::Text(text)) = parts.last_mut() {
        *text = text.trim_end().to_string();
    }
    parts.retain(|part| !matches!(part, MessagePart::Text(text) if text.is_empty()));
    parts
}

/// A message as the user's entry shows it: each mention as Zed writes one, `[@name](uri)`.
fn message_markdown(parts: &[MessagePart]) -> String {
    parts
        .iter()
        .map(|part| match part {
            MessagePart::Text(text) => text.clone(),
            MessagePart::File { path, .. } | MessagePart::Folder(path) => {
                format!("[@{}]({})", path_name(path), file_uri(path))
            }
            MessagePart::Thread { uri, title, .. } => format!("[@{title}]({uri})"),
            MessagePart::Image { id, .. } => id.markdown_link(),
        })
        .collect()
}

/// A message as plain text, each mention as `@name`, for naming the thread.
fn message_title(parts: &[MessagePart]) -> String {
    parts
        .iter()
        .map(|part| match part {
            MessagePart::Text(text) => text.clone(),
            MessagePart::File { path, .. } | MessagePart::Folder(path) => {
                format!("@{}", path_name(path))
            }
            MessagePart::Thread { title, .. } => format!("@{title}"),
            MessagePart::Image { .. } => "@Image".to_string(),
        })
        .collect()
}

/// As Zed writes one (`MentionUri::to_uri`): percent-encoded, so a path with spaces stays one
/// link in markdown.
fn file_uri(path: &Path) -> String {
    url::Url::from_file_path(path)
        .map_or_else(|()| format!("file://{}", path.display()), String::from)
}

/// A mention from an agent's history as a link. Its URI is parsed again, which percent-encodes
/// one sent with spaces in it (as before `file_uri` encoded them): markdown ends a link at the
/// first space.
fn mention_link(name: &str, uri: &str) -> String {
    let uri = url::Url::parse(uri).map_or_else(|_| uri.to_string(), String::from);
    format!("[@{name}]({uri})")
}

fn path_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}

/// A tool call's content, read by [`AgentThread::tool_content`].
#[derive(Default)]
struct ToolContent {
    text: Vec<String>,
    diffs: Vec<FileDiff>,
    terminals: Vec<String>,
    images: Vec<AttachmentId>,
}

impl ToolContent {
    /// Replaces the tool call's content, as ACP's updates do.
    fn apply_to(self, tool_call: &mut ToolCall) {
        tool_call.text = self.text;
        tool_call.diffs = self.diffs;
        tool_call.terminals = self.terminals;
        tool_call.images = self.images;
    }
}

fn error_message(error: &agent_client_protocol::Error) -> String {
    match &error.data {
        Some(data) => format!("{} ({data})", error.message),
        None => error.message.clone(),
    }
}

/// Answers a terminal request from an agent that wasn't offered terminals.
fn refuse_terminal_request(request: TerminalRequest) {
    let message = "agentZ doesn't run terminals for this agent";
    let result = match request {
        TerminalRequest::Create(_, responder) => responder.respond_with_internal_error(message),
        TerminalRequest::Output(_, responder) => responder.respond_with_internal_error(message),
        TerminalRequest::WaitForExit(_, responder) => {
            responder.respond_with_internal_error(message)
        }
        TerminalRequest::Kill(_, responder) => responder.respond_with_internal_error(message),
        TerminalRequest::Release(_, responder) => responder.respond_with_internal_error(message),
    };
    if let Err(error) = result {
        log::error!("failed to refuse a terminal request: {error:?}");
    }
}

fn cancel_permission(responder: Responder<acp::RequestPermissionResponse>) {
    responder
        .respond(acp::RequestPermissionResponse::new(
            acp::RequestPermissionOutcome::Cancelled,
        ))
        .ok();
}

fn cancel_elicitation(responder: Responder<acp::CreateElicitationResponse>) {
    responder
        .respond(acp::CreateElicitationResponse::new(
            acp::ElicitationAction::Cancel,
        ))
        .ok();
}

/// Zed's checks: a URL must be web address with a host, and the mode one ACP defines.
fn validate_elicitation(
    request: &acp::CreateElicitationRequest,
) -> std::result::Result<(), String> {
    match &request.mode {
        acp::ElicitationMode::Url(mode) => {
            let url = url::Url::parse(&mode.url)
                .map_err(|error| format!("invalid elicitation URL: {error}"))?;
            if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
                return Err("elicitation URL must use HTTP or HTTPS and include a host".into());
            }
            Ok(())
        }
        acp::ElicitationMode::Form(_) => Ok(()),
        _ => Err("unsupported elicitation mode".into()),
    }
}

/// The agent's message when it asks for a login, unless it's only ACP's stock wording, as
/// Zed decides.
fn auth_description(error: &agent_client_protocol::Error) -> Option<SharedString> {
    let message = error.message.trim();
    (!message.is_empty() && message != acp::ErrorCode::AuthRequired.to_string())
        .then(|| SharedString::from(message.to_string()))
}

/// The status in an `_auth/status_update`: `{"authStatus": {"kind": …, "label": …,
/// "account": {"email": …, "plan": …}, "detail": …}}`.
fn auth_status(params: &Value) -> Option<AuthStatus> {
    let status = params.get("authStatus")?;
    let status: AuthStatus = serde_json::from_value(status.clone()).ok()?;
    (!status.kind.is_empty()).then_some(status)
}

/// The notification's background task update, if it's one ([`BackgroundTaskUpdate`]).
fn background_task_update(
    notification: &agent_client_protocol::UntypedMessage,
) -> Option<BackgroundTaskUpdate> {
    if notification.method != "session/update" {
        return None;
    }
    let update = notification.params.get("update")?;
    let kind = update.get("sessionUpdate")?.as_str()?;
    if !kind.starts_with("async_task_") {
        return None;
    }
    serde_json::from_value(update.clone())
        .inspect_err(|error| {
            log::warn!("the agent sent a background task update that couldn't be read: {error}")
        })
        .ok()
}

/// The notification's subagent update, with the session it came in, if it's one
/// ([`SubagentNotice`]).
fn subagent_notice(
    notification: &agent_client_protocol::UntypedMessage,
) -> Option<(acp::SessionId, SubagentNotice)> {
    if notification.method != "session/update" {
        return None;
    }
    let update = notification.params.get("update")?;
    let kind = update.get("sessionUpdate")?.as_str()?;
    if !kind.starts_with("subagent_") {
        return None;
    }
    let session = notification.params.get("sessionId")?.as_str()?;
    serde_json::from_value(update.clone())
        .inspect_err(|error| {
            log::warn!("the agent sent a subagent update that couldn't be read: {error}")
        })
        .ok()
        .map(|update| (acp::SessionId::new(session.to_string()), update))
}

/// The id of the card in the thread for the agent's own subagent running in `session`.
fn subagent_card_id(session: &str) -> acp::ToolCallId {
    acp::ToolCallId::new(format!("subagent:{session}"))
}

/// Web links in a line the agent printed, except local callback addresses, which only the
/// agent itself can use.
fn login_links(line: &str) -> Vec<String> {
    let mut links = Vec::new();
    let mut rest = line;
    while let Some(start) = rest.find("http") {
        let candidate = &rest[start..];
        let end = candidate
            .find(|character: char| {
                character.is_whitespace()
                    || character.is_control()
                    || matches!(character, '"' | '\'' | '<' | '>' | '`')
            })
            .unwrap_or(candidate.len());
        let link = candidate[..end].trim_end_matches(['.', ',', ';', ':', ')', ']', '}']);
        rest = &candidate[end.max(4)..];
        let Ok(url) = url::Url::parse(link) else {
            continue;
        };
        let is_local = match url.host() {
            Some(url::Host::Domain(domain)) => domain == "localhost",
            Some(url::Host::Ipv4(address)) => address.is_loopback(),
            Some(url::Host::Ipv6(address)) => address.is_loopback(),
            None => true,
        };
        if matches!(url.scheme(), "http" | "https") && !is_local {
            links.push(link.to_string());
        }
    }
    links
}

/// Spawns the agent and wires up and initializes the ACP connection. The agent's process and
/// transport run in `agent_tasks`, and report through `sender`.
async fn connect(
    command: AgentCommand,
    cwd: PathBuf,
    sender: MessageSender,
    terminal_host: Option<TerminalHost>,
    agent_tasks: &mut JoinSet<()>,
) -> Result<Connected> {
    // Stopped by `ProcessGuard` rather than on drop, so a handed-off agent can outlive this
    // process. In a group of its own, so what it starts stops with it.
    let mut process_command = tokio::process::Command::new(&command.path);
    for variable in &command.env_remove {
        process_command.env_remove(variable);
    }
    let mut child = process_command
        .args(&command.args)
        .envs(&command.env)
        .current_dir(&cwd)
        .process_group(0)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| format!("starting {}", command.path.display()))?;
    let process = AgentProcess {
        pid: child.id().context("the agent exited at once")?,
        armed: Arc::new(AtomicBool::new(true)),
    };
    let guard = ProcessGuard(process.clone());
    let stdin = child.stdin.take().context("agent has no stdin")?;
    let stdout = child.stdout.take().context("agent has no stdout")?;
    let stderr = child.stderr.take().context("agent has no stderr")?;
    agent_tasks.spawn({
        let sender = sender.clone();
        async move {
            let message = match child.wait().await {
                Ok(status) => format!("The agent exited ({status})."),
                Err(error) => format!("The agent stopped: {error}"),
            };
            // Its id may be reused now.
            guard.0.armed.store(false, Ordering::SeqCst);
            sender.send(MessageKind::Exited(message)).ok();
        }
    });

    let (incoming, incoming_lines) = mpsc::unbounded();
    let (outgoing_lines, outgoing) = mpsc::unbounded();
    let runtime = tokio::runtime::Handle::current();
    let mut wire = Wire::new(
        &runtime,
        stdin.into_owned_fd()?,
        stdout.into_owned_fd()?,
        stderr.into_owned_fd()?,
        (Vec::new(), Vec::new()),
        incoming,
        outgoing,
        stderr_reporter(sender.clone()),
        None,
    )?;
    wire.resume(&runtime);

    let supports_terminals = terminal_host.is_some();
    let transport = transport(outgoing_lines, incoming_lines);
    let (connection_future, connection) = client_connection(transport, sender, terminal_host);
    agent_tasks.spawn(connection_future);
    let connection = connection
        .await
        .map_err(|_| anyhow!("the agent closed the connection before it was ready"))?;

    let version = env!("CARGO_PKG_VERSION");
    let initialize = connection
        .send_request(
            acp::InitializeRequest::new(ProtocolVersion::V1)
                .client_capabilities(client_capabilities(supports_terminals))
                .client_info(acp::Implementation::new("agentZ", version)),
        )
        .block_task()
        .map(|result| result.map_err(|error| anyhow!(error_message(&error))));
    let initialize_response = initialize.await.context("initializing the agent")?;
    anyhow::ensure!(
        initialize_response.protocol_version >= ProtocolVersion::V1,
        "the agent speaks an unsupported ACP version"
    );
    // Beside `agentCapabilities` rather than in them, as Claude Agent and Codex send it.
    let supports_steering = initialize_response
        .meta
        .as_ref()
        .and_then(|meta| meta.get("steering"))
        .and_then(|steering| steering.get("supported"))
        .and_then(Value::as_bool)
        .unwrap_or(false);

    Ok(Connected {
        connection,
        capabilities: initialize_response.agent_capabilities,
        supports_steering,
        auth_methods: initialize_response.auth_methods,
        agent_info: initialize_response.agent_info,
        wire,
        process,
    })
}

/// What agentZ supports, as Zed's `client_capabilities_for_agent` says it.
fn client_capabilities(supports_terminals: bool) -> acp::ClientCapabilities {
    acp::ClientCapabilities::new()
        .terminal(supports_terminals)
        // The server runs terminal logins itself (`TerminalKey::Login`). Agents offer them
        // only to clients that say so: Claude Agent offers no login at all otherwise. The
        // `_meta` flag below is the older form, as Zed sends it. `gateway` asks for the
        // methods that log in through an LLM gateway (Claude Agent, Codex), which take a
        // base URL and headers in `authenticate`'s `_meta`.
        .auth(
            acp::AuthCapabilities::new()
                .terminal(true)
                .meta(acp::Meta::from_iter([("gateway".to_string(), true.into())])),
        )
        .session(
            acp::ClientSessionCapabilities::new().config_options(
                acp::SessionConfigOptionsCapabilities::new()
                    .boolean(acp::BooleanConfigOptionCapabilities::new()),
            ),
        )
        .elicitation(
            acp::ElicitationCapabilities::new()
                .form(acp::ElicitationFormCapabilities::new())
                .url(acp::ElicitationUrlCapabilities::new()),
        )
        // JetBrains AIR's async tasks, by which Claude Agent reports what it left running and
        // stops it, and its native subagent sessions, in which it runs each subagent, are only
        // for clients that call themselves AIR. That also changes a little of how it reports
        // tool calls: a file read or search names the files without their text.
        .meta(acp::Meta::from_iter([
            ("terminal-auth".to_string(), true.into()),
            (
                "jetbrains".to_string(),
                serde_json::json!({"air": {
                    "version": 1,
                    "capabilities": ["asyncTasks", "nativeSubagentSessions"],
                }}),
            ),
        ]))
}

type Transport = agent_client_protocol::Lines<
    futures::sink::SinkMapErr<mpsc::UnboundedSender<String>, fn(mpsc::SendError) -> std::io::Error>,
    mpsc::UnboundedReceiver<std::io::Result<String>>,
>;

/// Lines over the channels the wire passes them through.
fn transport(
    outgoing: mpsc::UnboundedSender<String>,
    incoming: mpsc::UnboundedReceiver<std::io::Result<String>>,
) -> Transport {
    let to_io_error: fn(mpsc::SendError) -> std::io::Error = std::io::Error::other;
    agent_client_protocol::Lines::new(outgoing.sink_map_err(to_io_error), incoming)
}

/// The client side of an ACP connection on `transport`. Run the future for as long as the
/// connection lasts; the connection itself arrives once it's set up.
fn client_connection(
    transport: Transport,
    sender: MessageSender,
    terminal_host: Option<TerminalHost>,
) -> (
    BoxFuture<'static, ()>,
    oneshot::Receiver<ConnectionTo<Agent>>,
) {
    let (connection_sender, connection_receiver) = oneshot::channel();
    let host = move |request: TerminalRequest| match &terminal_host {
        Some(host) => host(request),
        None => refuse_terminal_request(request),
    };
    let host = Arc::new(host);
    let notification_sender = sender.clone();
    let elicitation_sender = sender.clone();
    let completion_sender = sender.clone();
    let extension_sender = sender.clone();
    let background_task_sender = sender.clone();
    let permission_sender = sender;
    let (create_host, output_host, wait_host, kill_host, release_host) = (
        host.clone(),
        host.clone(),
        host.clone(),
        host.clone(),
        host.clone(),
    );
    let connection_future = Client
        .builder()
        .name("agentZ")
        // Before the typed updates, which can't read these and would drop them.
        .on_receive_notification(
            async move |notification: agent_client_protocol::UntypedMessage, connection| {
                if let Some(update) = background_task_update(&notification) {
                    background_task_sender
                        .send(MessageKind::BackgroundTask(update))
                        .ok();
                    return Ok(agent_client_protocol::Handled::Yes);
                }
                if let Some((session, update)) = subagent_notice(&notification) {
                    background_task_sender
                        .send(MessageKind::Subagent { session, update })
                        .ok();
                    return Ok(agent_client_protocol::Handled::Yes);
                }
                Ok(agent_client_protocol::Handled::No {
                    message: (notification, connection),
                    retry: false,
                })
            },
            agent_client_protocol::on_receive_notification!(),
        )
        .on_receive_notification(
            async move |notification: acp::SessionNotification, _connection| {
                let message = match &*notification.session_id.0 {
                    PAUSE_MARKER_SESSION => MessageKind::PauseMarker,
                    ANSWER_MARKER_SESSION => MessageKind::AnswerMarker,
                    _ => MessageKind::Incoming(Incoming::Notification(notification)),
                };
                notification_sender.send(message).ok();
                Ok(())
            },
            agent_client_protocol::on_receive_notification!(),
        )
        .on_receive_notification(
            async move |notification: acp::CompleteElicitationNotification, _connection| {
                completion_sender
                    .send(MessageKind::ElicitationCompleted(
                        notification.elicitation_id,
                    ))
                    .ok();
                Ok(())
            },
            agent_client_protocol::on_receive_notification!(),
        )
        // Extension notifications, after the typed ones so it sees only what they don't take.
        .on_receive_notification(
            async move |notification: agent_client_protocol::UntypedMessage, _connection| {
                if notification.method == AUTH_STATUS_NOTIFICATION {
                    match auth_status(&notification.params) {
                        Some(status) => {
                            extension_sender.send(MessageKind::AuthStatus(status)).ok();
                        }
                        None => log::warn!(
                            "the agent sent an auth status that couldn't be read: {}",
                            notification.params
                        ),
                    }
                }
                Ok(())
            },
            agent_client_protocol::on_receive_notification!(),
        )
        .on_receive_request(
            async move |request: acp::CreateElicitationRequest,
                        responder: Responder<acp::CreateElicitationResponse>,
                        _connection| {
                if let Err(message) = elicitation_sender.send(MessageKind::Incoming(
                    Incoming::Elicitation(request, responder),
                )) && let MessageKind::Incoming(Incoming::Elicitation(_, responder)) = *message
                {
                    cancel_elicitation(responder);
                }
                Ok(())
            },
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            async move |request: acp::RequestPermissionRequest,
                        responder: Responder<acp::RequestPermissionResponse>,
                        _connection| {
                if let Err(message) = permission_sender.send(MessageKind::Incoming(
                    Incoming::Permission(request, responder),
                )) && let MessageKind::Incoming(Incoming::Permission(_, responder)) = *message
                {
                    responder.respond(acp::RequestPermissionResponse::new(
                        acp::RequestPermissionOutcome::Cancelled,
                    ))?;
                }
                Ok(())
            },
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            async move |request: acp::CreateTerminalRequest, responder, _connection| {
                create_host(TerminalRequest::Create(request, responder));
                Ok(())
            },
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            async move |request: acp::TerminalOutputRequest, responder, _connection| {
                output_host(TerminalRequest::Output(request, responder));
                Ok(())
            },
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            async move |request: acp::WaitForTerminalExitRequest, responder, _connection| {
                wait_host(TerminalRequest::WaitForExit(request, responder));
                Ok(())
            },
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            async move |request: acp::KillTerminalRequest, responder, _connection| {
                kill_host(TerminalRequest::Kill(request, responder));
                Ok(())
            },
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            async move |request: acp::ReleaseTerminalRequest, responder, _connection| {
                release_host(TerminalRequest::Release(request, responder));
                Ok(())
            },
            agent_client_protocol::on_receive_request!(),
        )
        .connect_with(
            transport,
            move |connection: ConnectionTo<Agent>| async move {
                connection_sender.send(connection).ok();
                // Keep the connection open until the transport closes.
                futures::future::pending::<Result<(), agent_client_protocol::Error>>().await
            },
        );
    let connection_future = async move {
        if let Err(error) = connection_future.await {
            log::error!("ACP connection error: {error:?}");
        }
    }
    .boxed();
    (connection_future, connection_receiver)
}

/// Keeps the agent's last stderr lines for errors, and logs them.
fn stderr_reporter(sender: MessageSender) -> Arc<dyn Fn(String) + Send + Sync> {
    Arc::new(move |line: String| {
        let line = without_terminal_escapes(&line);
        log::warn!("agent stderr: {line}");
        sender.send(MessageKind::Stderr(line)).ok();
    })
}

/// Agents color their stderr for a terminal even through a pipe (OpenCode's errors start with
/// a red bold "Error:"), and the codes would show as text in an error.
fn without_terminal_escapes(line: &str) -> String {
    let mut plain = String::with_capacity(line.len());
    let mut characters = line.chars().peekable();
    while let Some(character) = characters.next() {
        if character != '\x1b' {
            plain.push(character);
            continue;
        }
        match characters.next() {
            // A control sequence, such as a color: parameters up to a final byte.
            Some('[') => {
                for character in characters.by_ref() {
                    if ('\x40'..='\x7e').contains(&character) {
                        break;
                    }
                }
            }
            // An operating system command, such as a hyperlink: up to BEL or ESC \.
            Some(']') => {
                while let Some(character) = characters.next() {
                    if character == '\x07' {
                        break;
                    }
                    if character == '\x1b' && characters.peek() == Some(&'\\') {
                        characters.next();
                        break;
                    }
                }
            }
            // Character set selections and the like: intermediates, then a final byte.
            Some(character) if ('\x20'..='\x2f').contains(&character) => {
                for character in characters.by_ref() {
                    if !('\x20'..='\x2f').contains(&character) {
                        break;
                    }
                }
            }
            _ => {}
        }
    }
    plain
}

/// The answer to a prompt, as the SDK would have parsed it.
fn prompt_answer(
    answer: Value,
) -> std::result::Result<acp::PromptResponse, agent_client_protocol::Error> {
    if let Some(error) = answer.get("error") {
        return Err(
            serde_json::from_value(error.clone()).unwrap_or_else(|error| {
                agent_client_protocol::Error::internal_error().data(error.to_string())
            }),
        );
    }
    serde_json::from_value(answer.get("result").cloned().unwrap_or(Value::Null))
        .map_err(|error| agent_client_protocol::Error::internal_error().data(error.to_string()))
}

fn lock_slot(slot: &Mutex<PauseSlot>) -> std::sync::MutexGuard<'_, PauseSlot> {
    slot.lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

struct Connected {
    connection: ConnectionTo<Agent>,
    capabilities: acp::AgentCapabilities,
    supports_steering: bool,
    auth_methods: Vec<acp::AuthMethod>,
    agent_info: Option<acp::Implementation>,
    wire: Wire,
    process: AgentProcess,
}

struct SessionSetup {
    session_id: acp::SessionId,
    config_options: Vec<acp::SessionConfigOption>,
    modes: Option<acp::SessionModeState>,
    restore: SessionRestore,
}

/// Opens the thread's session: loads or resumes `previous_session` when the agent supports it
/// (in that order, like Zed), otherwise starts a new one.
///
/// If the agent can't open a session with the given MCP servers, it is opened without them.
/// Factory Droid 0.233.0's `acp-daemon` fails every session that has any: it passes them on to
/// its worker as `--mcp-servers`, an option the worker doesn't have.
async fn open_session(
    connection: ConnectionTo<Agent>,
    capabilities: acp::AgentCapabilities,
    cwd: PathBuf,
    previous_session: Option<acp::SessionId>,
    mcp_servers: Vec<acp::McpServer>,
) -> std::result::Result<SessionSetup, agent_client_protocol::Error> {
    let mcp_servers = servers_the_agent_takes(&capabilities.mcp_capabilities, mcp_servers);
    if mcp_servers.is_empty() {
        return open_session_with(connection, capabilities, cwd, previous_session, mcp_servers)
            .await;
    }
    match open_session_with(
        connection.clone(),
        capabilities.clone(),
        cwd.clone(),
        previous_session.clone(),
        mcp_servers,
    )
    .await
    {
        Err(error) if !is_auth_required(&error) => {
            log::warn!(
                "couldn't open a session with MCP servers, opening it without them: {}",
                error_message(&error)
            );
            open_session_with(connection, capabilities, cwd, previous_session, Vec::new()).await
        }
        result => result,
    }
}

/// Every agent takes stdio servers; HTTP and SSE ones only go to agents that announce them, as
/// ACP asks.
fn servers_the_agent_takes(
    capabilities: &acp::McpCapabilities,
    mcp_servers: Vec<acp::McpServer>,
) -> Vec<acp::McpServer> {
    mcp_servers
        .into_iter()
        .filter(|server| match server {
            acp::McpServer::Http(_) => capabilities.http,
            acp::McpServer::Sse(_) => capabilities.sse,
            _ => true,
        })
        .collect()
}

async fn open_session_with(
    connection: ConnectionTo<Agent>,
    capabilities: acp::AgentCapabilities,
    cwd: PathBuf,
    previous_session: Option<acp::SessionId>,
    mcp_servers: Vec<acp::McpServer>,
) -> std::result::Result<SessionSetup, agent_client_protocol::Error> {
    let had_previous_session = previous_session.is_some();
    if let Some(session_id) = previous_session {
        if capabilities.load_session {
            match connection
                .send_request(
                    acp::LoadSessionRequest::new(session_id.clone(), cwd.clone())
                        .mcp_servers(mcp_servers.clone()),
                )
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
                .send_request(
                    acp::ResumeSessionRequest::new(session_id.clone(), cwd.clone())
                        .mcp_servers(mcp_servers.clone()),
                )
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
        .send_request(acp::NewSessionRequest::new(cwd).mcp_servers(mcp_servers))
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

/// Pages of `session/list` read at most, in case an agent keeps handing out new cursors.
const MAX_SESSION_LIST_PAGES: usize = 100;

/// How long listing an agent's sessions may take, starting the agent included.
const LIST_SESSIONS_TIMEOUT: Duration = Duration::from_secs(60);
const AGENT_INFO_TIMEOUT: Duration = Duration::from_secs(30);
const STDERR_DRAIN: Duration = Duration::from_millis(200);

/// What an agent says about the sessions it keeps, from ACP's `session/list`.
#[derive(Debug)]
pub enum SessionListing {
    Listed(Vec<acp::SessionInfo>),
    /// The agent doesn't advertise `sessionCapabilities.list`.
    Unsupported,
    /// The agent wants a login first.
    LoggedOut,
}

/// Starts the agent only to initialize it, for what it says it is (ACP's `agentInfo`), and
/// stops it. An agent that doesn't start fails with the last lines it printed.
pub async fn agent_info(command: AgentCommand) -> Result<Option<acp::Implementation>> {
    let (messages, mut inbox) = mpsc::unbounded();
    let sender = MessageSender {
        sender: messages,
        generation: 0,
    };
    // Stops the agent when dropped.
    let mut agent_tasks = JoinSet::new();
    let connecting = connect(
        command,
        std::env::temp_dir(),
        sender,
        None,
        &mut agent_tasks,
    );
    let deadline = tokio::time::sleep(AGENT_INFO_TIMEOUT);
    tokio::pin!(connecting, deadline);
    let mut stderr = VecDeque::new();
    let mut failure = None;
    loop {
        tokio::select! {
            connected = &mut connecting, if failure.is_none() => match connected {
                Ok(connected) => return Ok(connected.agent_info),
                Err(error) => {
                    failure = Some(error);
                    deadline.as_mut().reset(tokio::time::Instant::now() + STDERR_DRAIN);
                }
            },
            Some(message) = inbox.next() => match message.kind {
                MessageKind::Stderr(line) => {
                    if stderr.len() == STDERR_LINES_KEPT {
                        stderr.pop_front();
                    }
                    stderr.push_back(line);
                }
                // An agent that exits before it reads `initialize` leaves the request
                // unanswered. What it printed last may still be on its way.
                MessageKind::Exited(exited) if failure.is_none() => {
                    failure = Some(anyhow!(exited));
                    deadline.as_mut().reset(tokio::time::Instant::now() + STDERR_DRAIN);
                }
                _ => {}
            },
            () = &mut deadline => break,
        }
    }
    let error = failure.unwrap_or_else(|| anyhow!("the agent didn't answer `initialize` in time"));
    let printed = Vec::from(stderr).join("\n");
    if printed.trim().is_empty() {
        return Err(error);
    }
    Err(anyhow!("{error:#}\n\n{}", printed.trim()))
}

/// The agent's sessions, every page of them, as Zed's thread import collects them. The agent
/// starts only for this, in a scratch directory, and stops afterwards.
pub async fn list_sessions(command: CommandFuture) -> Result<SessionListing> {
    tokio::time::timeout(LIST_SESSIONS_TIMEOUT, async {
        let command = command.await?;
        // Nothing listens: the agent has no session to report on, and a request it sends is
        // turned down.
        let (messages, _) = mpsc::unbounded();
        let sender = MessageSender {
            sender: messages,
            generation: 0,
        };
        // Stops the agent when dropped.
        let mut agent_tasks = JoinSet::new();
        let connected = connect(
            command,
            std::env::temp_dir(),
            sender,
            None,
            &mut agent_tasks,
        )
        .await?;
        if connected.capabilities.session_capabilities.list.is_none() {
            return Ok(SessionListing::Unsupported);
        }
        let mut sessions = Vec::new();
        let mut cursor: Option<String> = None;
        for _ in 0..MAX_SESSION_LIST_PAGES {
            let response = connected
                .connection
                .send_request(acp::ListSessionsRequest::new().cursor(cursor.clone()))
                .block_task()
                .await;
            let response = match response {
                Ok(response) => response,
                Err(error) if is_auth_required(&error) => return Ok(SessionListing::LoggedOut),
                Err(error) => return Err(anyhow!(error_message(&error))),
            };
            sessions.extend(response.sessions);
            match response.next_cursor {
                Some(next) if Some(&next) != cursor.as_ref() => cursor = Some(next),
                _ => break,
            }
        }
        Ok(SessionListing::Listed(sessions))
    })
    .await
    .map_err(|_| anyhow!("the agent didn't list its sessions in time"))?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mentions_of_paths_with_spaces_stay_one_link() {
        let path = PathBuf::from("/Users/me/Desktop/Screen Recording 1.mov");
        assert_eq!(
            message_markdown(&[
                MessagePart::Text("look at ".into()),
                MessagePart::File {
                    path,
                    contents: None,
                },
            ]),
            "look at [@Screen Recording 1.mov](file:///Users/me/Desktop/Screen%20Recording%201.mov)"
        );
        assert_eq!(
            mention_link("a b.md", "file:///tmp/a b.md"),
            "[@a b.md](file:///tmp/a%20b.md)"
        );
        assert_eq!(
            mention_link("hello there", "agentz://thread/7"),
            "[@hello there](agentz://thread/7)"
        );
    }

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

    #[test]
    fn stderr_loses_terminal_escapes() {
        assert_eq!(
            without_terminal_escapes("\x1b[91m\x1b[1mError: \x1b[0mUnexpected error"),
            "Error: Unexpected error"
        );
        assert_eq!(
            without_terminal_escapes(
                "Open \x1b]8;;https://example.com/login\x1b\\the page\x1b]8;;\x07 \x1b(Bnow"
            ),
            "Open the page now"
        );
        assert_eq!(without_terminal_escapes("plain ✓"), "plain ✓");
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
            env_remove: Vec::new(),
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
            None,
        ))
    }

    /// Logging in and out from settings, against `test_support/mock_agent.py`.
    #[tokio::test(flavor = "multi_thread")]
    async fn logs_in_and_out_of_a_login_session() {
        let Some(command) = mock_agent(&[]) else {
            return;
        };
        let mut login = TestThread::new(AgentThread::start_for_login_session(
            tokio::runtime::Handle::current(),
            "Mock".into(),
            ready(command),
        ));

        login
            .wait_until(|login| login.status() == &ConnectionStatus::Ready)
            .await;
        assert!(login.thread.supports_logout());
        // The terminal, browser and gateway logins are offered because the client says it
        // takes them.
        let methods: Vec<&str> = login
            .thread
            .auth_methods()
            .iter()
            .map(|method| &*method.id().0)
            .collect();
        assert_eq!(
            methods,
            [
                "mock-login",
                "mock-terminal-login",
                "mock-browser-login",
                "mock-api-key",
                "mock-gateway"
            ]
        );
        assert_eq!(
            login.thread.logged_in(),
            Some(true),
            "the mock opens sessions freely"
        );
        assert_eq!(login.thread.config_options().len(), 4);
        login
            .wait_until(|login| login.auth_status().is_some())
            .await;
        let status = login.thread.auth_status().expect("a status");
        assert!(status.is_logged_in());
        assert_eq!(
            status
                .account
                .as_ref()
                .and_then(|account| account.email.as_deref()),
            Some("mock@example.com")
        );

        login.update(|login| login.authenticate(acp::AuthMethodId::new("mock-login"), None));
        login
            .wait_until(|login| {
                login.login_notice().is_some() && login.status() == &ConnectionStatus::Ready
            })
            .await;
        assert_eq!(
            login.thread.login_notice().map(|n| n.as_ref()),
            Some("Logged in.")
        );
        assert_eq!(login.thread.logged_in(), Some(true));
        assert!(
            login
                .events
                .iter()
                .any(|event| matches!(event, AgentThreadEvent::LoggedIn(_)))
        );

        login.update(|login| login.logout());
        login
            .wait_until(|login| {
                login.login_notice().map(|n| n.as_ref()) == Some("Logged out.")
                    && login
                        .auth_status()
                        .is_some_and(|status| !status.is_logged_in())
            })
            .await;
        assert_eq!(login.thread.logged_in(), Some(false));

        login.update(|login| login.check_login());
        login
            .wait_until(|login| login.status() == &ConnectionStatus::AuthRequired)
            .await;
        assert_eq!(login.thread.logged_in(), Some(false));
        login.update(|login| login.authenticate(acp::AuthMethodId::new("mock-login"), None));
        login
            .wait_until(|login| login.logged_in() == Some(true))
            .await;
    }

    /// An agent that fails sessions given MCP servers, as Factory Droid 0.233.0 does, still
    /// gets a session, without them.
    #[tokio::test(flavor = "multi_thread")]
    async fn opens_a_session_without_mcp_servers_the_agent_rejects() {
        let Some(mut command) = mock_agent(&[]) else {
            return;
        };
        command.env.insert("MOCK_REJECT_MCP".into(), "1".into());
        let mut thread = start(command, None);
        thread.update(|thread| {
            thread.set_mcp_servers(vec![acp::McpServer::Stdio(acp::McpServerStdio::new(
                "agentz", "/bin/cat",
            ))])
        });
        thread
            .wait_until(|thread| thread.status() == &ConnectionStatus::Ready)
            .await;
        assert!(thread.thread.session.is_some());
    }

    fn user_messages(thread: &AgentThread) -> Vec<String> {
        thread
            .entries()
            .iter()
            .filter_map(|entry| match entry {
                Entry::UserMessage(text) => Some(text.to_string()),
                _ => None,
            })
            .collect()
    }

    fn last_agent_message(thread: &AgentThread) -> Option<String> {
        thread.entries().iter().rev().find_map(|entry| match entry {
            Entry::AgentMessage(text) => Some(text.to_string()),
            _ => None,
        })
    }

    /// Stopping an agent stops what it started with it, as Factory Droid's session workers.
    #[tokio::test(flavor = "multi_thread")]
    async fn stops_what_the_agent_started_with_it() {
        let Some(mut command) = mock_agent(&[]) else {
            return;
        };
        let dir = tempfile::tempdir().expect("temp dir");
        let pid_file = dir.path().join("worker");
        command.env.insert(
            "MOCK_CHILD_PID_FILE".into(),
            pid_file.to_string_lossy().into_owned(),
        );
        let mut thread = start(command, None);
        thread
            .wait_until(|thread| thread.status() == &ConnectionStatus::Ready)
            .await;
        let pid: libc::pid_t = std::fs::read_to_string(&pid_file)
            .expect("the worker's pid")
            .parse()
            .expect("a pid");
        // SAFETY: signal 0 only checks that the process exists.
        let is_running = || unsafe { libc::kill(pid, 0) } == 0;
        assert!(is_running());

        drop(thread);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while is_running() && std::time::Instant::now() < deadline {
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
        assert!(!is_running(), "the worker outlived its agent");
    }

    /// A message steered into a turn joins it, as Claude Agent and Codex take one, rather than
    /// waiting for it to end.
    #[tokio::test(flavor = "multi_thread")]
    async fn steers_a_message_into_the_running_turn() {
        let Some(mut command) = mock_agent(&[]) else {
            return;
        };
        command.env.insert("MOCK_STEERING".into(), "1".into());
        let mut thread = start(command, None);
        thread
            .wait_until(|thread| thread.status() == &ConnectionStatus::Ready)
            .await;
        assert!(thread.thread.state.supports_steering);

        thread.update(|thread| thread.send("permission".into()));
        let tool_call_id = acp::ToolCallId::new("call-2");
        thread
            .wait_until(|thread| thread.permission_request(&tool_call_id).is_some())
            .await;
        thread.update(|thread| thread.steer("use tabs".into()));
        thread
            .wait_until(|thread| user_messages(thread) == ["permission", "use tabs"])
            .await;
        thread.update(|thread| {
            let allow = acp::PermissionOptionId::new("allow");
            thread.respond_to_permission(&tool_call_id, allow);
        });
        thread.wait_until(|thread| !thread.is_working()).await;
        assert_eq!(
            agent_text(&thread.thread),
            "Echo: permission (chose allow) (steered: use tabs)"
        );
        assert_eq!(user_messages(&thread.thread), ["permission", "use tabs"]);
    }

    /// A message the agent doesn't take into its turn goes as the next prompt once the turn
    /// ends.
    #[tokio::test(flavor = "multi_thread")]
    async fn sends_a_refused_steering_message_after_the_turn() {
        let Some(mut command) = mock_agent(&[]) else {
            return;
        };
        command.env.insert("MOCK_STEERING".into(), "1".into());
        let mut thread = start(command, None);
        thread
            .wait_until(|thread| thread.status() == &ConnectionStatus::Ready)
            .await;
        thread.update(|thread| thread.send("permission".into()));
        let tool_call_id = acp::ToolCallId::new("call-2");
        thread
            .wait_until(|thread| thread.permission_request(&tool_call_id).is_some())
            .await;
        thread.update(|thread| thread.steer("refuse".into()));
        thread
            .wait_until(|thread| !thread.after_turn.is_empty())
            .await;
        // It shows once it's sent.
        assert_eq!(user_messages(&thread.thread), ["permission"]);
        thread.update(|thread| {
            let allow = acp::PermissionOptionId::new("allow");
            thread.respond_to_permission(&tool_call_id, allow);
        });
        thread
            .wait_until(|thread| {
                !thread.is_working()
                    && last_agent_message(thread).as_deref() == Some("Echo: refuse")
            })
            .await;
        assert_eq!(user_messages(&thread.thread), ["permission", "refuse"]);
    }

    fn agent_messages(thread: &AgentThread) -> Vec<String> {
        thread
            .entries()
            .iter()
            .filter_map(|entry| match entry {
                Entry::AgentMessage(text) => Some(text.to_string()),
                _ => None,
            })
            .collect()
    }

    /// A command the agent sends to the background shows until it ends, as Claude Agent reports
    /// it (JetBrains AIR's async tasks). The agent then goes on with no prompt: the thread works
    /// until its result, with a message of its own, and what was sent meanwhile goes after.
    #[tokio::test(flavor = "multi_thread")]
    async fn shows_background_tasks_and_the_work_after_them() {
        let Some(command) = mock_agent(&[]) else {
            return;
        };
        let mut thread = start(command, None);
        thread
            .wait_until(|thread| thread.status() == &ConnectionStatus::Ready)
            .await;
        thread.update(|thread| thread.send("background-task 1".into()));
        thread
            .wait_until(|thread| !thread.is_working() && !thread.state.background_tasks.is_empty())
            .await;
        let task = thread.thread.state.background_tasks[0].clone();
        assert_eq!(task.name.as_ref(), "Sleep for 1 seconds");
        assert_eq!(task.kind.as_ref(), "shell");
        assert_eq!(task.tool_call_id, Some(acp::ToolCallId::new("bash-1")));
        assert!(task.can_stop);
        assert!(thread.thread.is_waiting());

        // Once it ends, the agent's work with its result is due, and then it's on.
        thread
            .wait_until(|thread| thread.state.background_tasks.is_empty())
            .await;
        assert!(thread.thread.is_waiting() || thread.thread.is_working());
        thread
            .wait_until(|thread| thread.is_working() && thread.state.background_tasks.is_empty())
            .await;
        assert!(!thread.thread.is_waiting());
        thread.update(|thread| thread.steer("next".into()));
        thread
            .wait_until(|thread| {
                !thread.is_working()
                    && last_agent_message(thread).is_some_and(|text| text.starts_with("Echo: next"))
            })
            .await;
        assert_eq!(
            agent_messages(&thread.thread)[..2],
            ["Started in the background.", "The command finished."]
        );
        assert_eq!(user_messages(&thread.thread), ["background-task 1", "next"]);
        // Each piece of work counts as a turn.
        assert_eq!(thread.thread.state.finished_turns.len(), 3);
    }

    /// Claude Agent runs each subagent in a session of its own once the client says it takes
    /// them: its steps go to the subagent's subthread, not into the thread, which hears when
    /// each starts and ends.
    #[tokio::test(flavor = "multi_thread")]
    async fn runs_subagents_in_sessions_of_their_own() {
        let Some(command) = mock_agent(&[]) else {
            return;
        };
        let mut thread = start(command, None);
        thread
            .wait_until(|thread| thread.status() == &ConnectionStatus::Ready)
            .await;
        thread.update(|thread| thread.send("subagents 0".into()));
        thread
            .wait_until(|thread| {
                !thread.is_working()
                    && last_agent_message(thread).as_deref() == Some("Both subagents are done.")
            })
            .await;
        // The thread has only its own conversation.
        assert_eq!(user_messages(&thread.thread), ["subagents 0"]);
        assert_eq!(agent_messages(&thread.thread), ["Both subagents are done."]);
        assert!(
            !thread
                .thread
                .entries()
                .iter()
                .any(|entry| matches!(entry, Entry::ToolCall(_)))
        );

        let started: Vec<Subagent> = thread
            .events
            .iter()
            .filter_map(|event| match event {
                AgentThreadEvent::SubagentStarted(subagent) => Some(subagent.clone()),
                _ => None,
            })
            .collect();
        assert_eq!(started.len(), 2);
        assert_eq!(started[0].name, "Find where the login view is drawn");
        assert_eq!(
            started[0].task,
            "Find the code that draws the login view in a thread."
        );
        assert_eq!(started[0].parent_session, None);
        let ended: Vec<(String, projects::TaskEnd)> = thread
            .events
            .iter()
            .filter_map(|event| match event {
                AgentThreadEvent::SubagentEnded { session, end } => Some((session.clone(), *end)),
                _ => None,
            })
            .collect();
        assert_eq!(
            ended,
            started
                .iter()
                .map(|subagent| (subagent.session.clone(), projects::TaskEnd::Completed))
                .collect::<Vec<_>>()
        );

        // What its session got makes up a subagent's subthread.
        let mut subthread = AgentThread::subagent("Mock".into(), std::env::temp_dir());
        subthread.start_subagent(Some((
            started[0].task.clone(),
            projects::ThreadCreator::Thread(projects::ThreadId(1)),
        )));
        assert!(subthread.is_working());
        for event in &thread.events {
            if let AgentThreadEvent::SubagentUpdate { session, update } = event
                && *session == started[0].session
            {
                subthread.apply_subagent_update(update.clone());
            }
        }
        subthread.end_subagent();
        assert!(!subthread.is_working());
        assert_eq!(
            user_messages(&subthread),
            ["Find the code that draws the login view in a thread."]
        );
        let steps: Vec<(&str, bool)> = subthread
            .entries()
            .iter()
            .filter_map(|entry| match entry {
                Entry::ToolCall(tool_call) => {
                    Some((tool_call.title.as_str(), tool_call.duration.is_some()))
                }
                _ => None,
            })
            .collect();
        assert_eq!(
            steps,
            [
                ("Search for \"render_centered\"", true),
                ("Read crates/app/src/agent_login.rs", true),
                ("Read crates/app/src/agent_view.rs", true),
            ]
        );
        assert_eq!(
            last_agent_message(&subthread).as_deref(),
            Some(
                "The thread's login is drawn by `AgentLogin::render_centered` in \
                 `crates/app/src/agent_login.rs`."
            )
        );
        // A subagent's thread takes no messages: its agent is the parent's.
        subthread.send("more".into());
        assert_eq!(user_messages(&subthread).len(), 1);

        // Its card in the parent's conversation runs until it ends, with its report.
        let subthread_id = projects::ThreadId(2);
        thread.update(|thread| thread.add_subagent_card(&started[0], subthread_id));
        let card = |thread: &AgentThread| {
            thread.entries().iter().find_map(|entry| match entry {
                Entry::ToolCall(tool_call) if tool_call.subthread == Some(subthread_id) => {
                    Some(tool_call.clone())
                }
                _ => None,
            })
        };
        let running = card(&thread.thread).expect("a card");
        assert_eq!(running.title, "Find where the login view is drawn");
        assert!(running.is_running());
        thread.update(|thread| {
            thread.end_subagent_card(
                subthread_id,
                projects::TaskEnd::Completed,
                Some("The report.".into()),
            )
        });
        let done = card(&thread.thread).expect("a card");
        assert_eq!(done.status, acp::ToolCallStatus::Completed);
        assert_eq!(done.text, ["The report."]);
        assert!(done.duration.is_some());
    }

    /// A subagent's permission request is asked in the thread, while its tool call goes to
    /// the subagent's subthread.
    #[tokio::test(flavor = "multi_thread")]
    async fn asks_a_subagents_permission_in_the_thread() {
        let Some(command) = mock_agent(&[]) else {
            return;
        };
        let mut thread = start(command, None);
        thread
            .wait_until(|thread| thread.status() == &ConnectionStatus::Ready)
            .await;
        thread.update(|thread| thread.send("subagents permission".into()));
        thread
            .wait_until(|thread| !thread.state.permission_requests.is_empty())
            .await;
        let request = thread.thread.state.permission_requests[0].clone();
        assert_eq!(request.title, "Edit .env");
        assert_eq!(
            thread.thread.orphan_permission_requests().count(),
            1,
            "the tool call is the subagent's"
        );
        let session = thread.events.iter().find_map(|event| match event {
            AgentThreadEvent::SubagentUpdate {
                session,
                update: acp::SessionUpdate::ToolCallUpdate(update),
            } if update.tool_call_id == request.tool_call_id => Some(session.clone()),
            _ => None,
        });
        let session = session.expect("the tool call goes to the subagent's subthread");
        assert_eq!(request.subagent_card, Some(subagent_card_id(&session)));
        thread.update(|thread| {
            thread
                .respond_to_permission(&request.tool_call_id, acp::PermissionOptionId::new("allow"))
        });
        thread
            .wait_until(|thread| {
                !thread.is_working()
                    && last_agent_message(thread).as_deref() == Some("Both subagents are done.")
            })
            .await;
        assert!(
            !thread
                .thread
                .entries()
                .iter()
                .any(|entry| matches!(entry, Entry::ToolCall(_)))
        );
    }

    /// Stopping a background task stops it on the agent's side. The agent's note of the stop
    /// is a message of its own, and not work: the thread stays idle.
    #[tokio::test(flavor = "multi_thread")]
    async fn stops_a_background_task() {
        let Some(command) = mock_agent(&[]) else {
            return;
        };
        let mut thread = start(command, None);
        thread
            .wait_until(|thread| thread.status() == &ConnectionStatus::Ready)
            .await;
        thread.update(|thread| thread.send("background-task 30".into()));
        thread
            .wait_until(|thread| !thread.is_working() && !thread.state.background_tasks.is_empty())
            .await;
        thread.update(|thread| thread.stop_background_task("task-1"));
        assert!(thread.thread.state.background_tasks[0].stopping);
        thread
            .wait_until(|thread| {
                thread.state.background_tasks.is_empty() && thread.stops_in_flight == 0
            })
            .await;
        assert!(!thread.thread.is_working());
        // No work is due after a stop, so it's done.
        assert!(!thread.thread.is_waiting());
        assert_eq!(
            agent_messages(&thread.thread),
            [
                "Started in the background.",
                "**Task stopped by user:** Sleep for 30 seconds."
            ]
        );
    }

    /// A browser login that asks the client to open a URL, as Codex's device code login does,
    /// after the agent's own message asked for a login, as Factory Droid's does.
    #[tokio::test(flavor = "multi_thread")]
    async fn logs_in_through_a_url_the_agent_asks_to_open() {
        let Some(mut command) = mock_agent(&[]) else {
            return;
        };
        let login_dir = tempfile::tempdir().expect("temp dir");
        let login_file = login_dir.path().join("logged-in");
        command.env.insert(
            "MOCK_LOGIN_FILE".into(),
            login_file.to_string_lossy().into_owned(),
        );
        let mut thread = start(command, None);
        thread
            .wait_until(|thread| thread.status() == &ConnectionStatus::AuthRequired)
            .await;
        assert_eq!(
            thread.thread.auth_description().map(|text| text.as_ref()),
            Some("Your code: MOCK-1234\n\nClick Log In.")
        );

        thread.update(|thread| {
            thread.authenticate(acp::AuthMethodId::new("mock-browser-login"), None)
        });
        assert!(thread.thread.is_authenticating());
        thread
            .wait_until(|thread| !thread.elicitations().is_empty())
            .await;
        let elicitation = thread.thread.elicitations()[0].clone();
        assert_eq!(
            elicitation.url(),
            Some("https://example.com/device?code=MOCK-1234")
        );
        assert!(matches!(
            elicitation.request.scope(),
            acp::ElicitationScope::Request(_)
        ));

        thread.update(|thread| {
            thread.respond_to_elicitation(
                elicitation.id,
                acp::ElicitationAction::Accept(acp::ElicitationAcceptAction::new()),
            )
        });
        thread
            .wait_until(|thread| thread.status() == &ConnectionStatus::Ready)
            .await;
        assert!(!thread.thread.is_authenticating());
        assert!(thread.thread.elicitations().is_empty());
        assert_eq!(thread.thread.logged_in(), Some(true));
        assert_eq!(thread.thread.auth_description(), None);
        assert!(thread.events.iter().any(
            |event| matches!(event, AgentThreadEvent::LoggedIn(name) if name == "Log in with a browser")
        ));
    }

    /// Cancelling a login that waits on the user restarts the agent, which asks again.
    #[tokio::test(flavor = "multi_thread")]
    async fn cancels_a_login_in_progress() {
        let Some(mut command) = mock_agent(&[]) else {
            return;
        };
        let login_dir = tempfile::tempdir().expect("temp dir");
        command.env.insert(
            "MOCK_LOGIN_FILE".into(),
            login_dir
                .path()
                .join("logged-in")
                .to_string_lossy()
                .into_owned(),
        );
        let mut thread = start(command, None);
        thread
            .wait_until(|thread| thread.status() == &ConnectionStatus::AuthRequired)
            .await;
        thread.update(|thread| thread.send("hello".into()));
        thread.update(|thread| {
            thread.authenticate(acp::AuthMethodId::new("mock-browser-login"), None)
        });
        thread
            .wait_until(|thread| !thread.elicitations().is_empty())
            .await;

        thread.update(AgentThread::cancel_authentication);
        assert!(!thread.thread.is_authenticating());
        assert!(thread.thread.elicitations().is_empty());
        assert_eq!(thread.thread.status(), &ConnectionStatus::Connecting);
        thread
            .wait_until(|thread| thread.status() == &ConnectionStatus::AuthRequired)
            .await;
        // The message that failed for want of the login is kept, with when it was sent.
        assert_eq!(
            thread.thread.entries(),
            [Entry::UserMessage("hello".into())]
        );
        assert!(thread.thread.sent_time(0).is_some());
        assert_eq!(
            thread.thread.failed_message(),
            Some(FailedMessage::NeedsLogin)
        );
        thread.update(|thread| thread.authenticate(acp::AuthMethodId::new("mock-login"), None));
        thread
            .wait_until(|thread| thread.status() == &ConnectionStatus::Ready)
            .await;
        thread.update(AgentThread::retry_message);
        thread
            .wait_until(|thread| !thread.is_working() && agent_text(thread) == "Echo: hello")
            .await;
    }

    /// A message the agent asks for a login at, as Claude Agent asks in a session it opened
    /// logged out, fails. Once logged in, Retry sends it, and the thread shows it once.
    #[tokio::test(flavor = "multi_thread")]
    async fn a_message_that_needed_a_login_is_retried_after_one() {
        let Some(mut command) = mock_agent(&[]) else {
            return;
        };
        let login_dir = tempfile::tempdir().expect("temp dir");
        command.env.insert(
            "MOCK_LOGIN_FILE".into(),
            login_dir
                .path()
                .join("logged-in")
                .to_string_lossy()
                .into_owned(),
        );
        command
            .env
            .insert("MOCK_OPENS_LOGGED_OUT".into(), "1".into());
        let mut thread = start(command, None);
        thread
            .wait_until(|thread| thread.status() == &ConnectionStatus::Ready)
            .await;
        thread.update(|thread| thread.send("hello".into()));
        thread
            .wait_until(|thread| {
                !thread.is_working() && thread.failed_message() == Some(FailedMessage::NeedsLogin)
            })
            .await;
        assert_eq!(thread.thread.status(), &ConnectionStatus::AuthRequired);
        // Not until the agent is ready for it.
        thread.update(AgentThread::retry_message);
        assert!(!thread.thread.is_working());

        thread.update(|thread| thread.authenticate(acp::AuthMethodId::new("mock-login"), None));
        thread
            .wait_until(|thread| thread.status() == &ConnectionStatus::Ready)
            .await;
        // Logging in doesn't send it by itself.
        assert_eq!(
            thread.thread.failed_message(),
            Some(FailedMessage::NeedsLogin)
        );
        assert_eq!(agent_text(&thread.thread), "");

        thread.update(AgentThread::retry_message);
        assert_eq!(thread.thread.failed_message(), None);
        thread
            .wait_until(|thread| !thread.is_working() && agent_text(thread) == "Echo: hello")
            .await;
        assert_eq!(user_messages(&thread.thread), ["hello"]);
    }

    /// A message sent while the session opens fails when the agent asks for a login to open
    /// it, as one it asks at does, rather than going by itself once the user has logged in.
    #[tokio::test(flavor = "multi_thread")]
    async fn a_message_waiting_for_a_session_fails_when_it_needs_a_login() {
        let Some(mut command) = mock_agent(&[]) else {
            return;
        };
        let login_dir = tempfile::tempdir().expect("temp dir");
        command.env.insert(
            "MOCK_LOGIN_FILE".into(),
            login_dir
                .path()
                .join("logged-in")
                .to_string_lossy()
                .into_owned(),
        );
        let mut thread = start(command, None);
        thread.update(|thread| thread.send("hello".into()));
        assert!(thread.thread.is_working());
        thread
            .wait_until(|thread| thread.status() == &ConnectionStatus::AuthRequired)
            .await;
        assert!(!thread.thread.is_working());
        assert_eq!(
            thread.thread.failed_message(),
            Some(FailedMessage::NeedsLogin)
        );

        thread.update(|thread| thread.authenticate(acp::AuthMethodId::new("mock-login"), None));
        thread
            .wait_until(|thread| thread.status() == &ConnectionStatus::Ready)
            .await;
        assert!(!thread.thread.is_working());
        thread.update(AgentThread::retry_message);
        thread
            .wait_until(|thread| !thread.is_working() && agent_text(thread) == "Echo: hello")
            .await;
        assert_eq!(user_messages(&thread.thread), ["hello"]);
    }

    /// A turn that fails, as on a lost connection, keeps its message and error, even across a
    /// reload, and Retry sends the message again.
    #[tokio::test(flavor = "multi_thread")]
    async fn a_failed_turn_is_retried() {
        let history_dir = tempfile::tempdir().expect("temp dir");
        let history_file = history_dir
            .path()
            .join("history.json")
            .to_string_lossy()
            .into_owned();
        // The reloaded agent loads the session.
        let Some(command) = mock_agent(&[history_file]) else {
            return;
        };
        let mut thread = start(command, None);
        thread
            .wait_until(|thread| thread.status() == &ConnectionStatus::Ready)
            .await;
        thread.update(|thread| thread.send("network-error".into()));
        thread
            .wait_until(|thread| {
                !thread.is_working() && thread.failed_message() == Some(FailedMessage::TurnFailed)
            })
            .await;
        assert_eq!(
            thread.thread.turn_error().map(|error| error.as_ref()),
            Some("API Error: Connection error.")
        );

        thread.update(AgentThread::reload);
        thread
            .wait_until(|thread| thread.status() == &ConnectionStatus::Ready)
            .await;
        assert_eq!(
            thread.thread.failed_message(),
            Some(FailedMessage::TurnFailed)
        );
        assert!(thread.thread.turn_error().is_some());

        // The restarted mock fails it once more, then takes it.
        thread.update(AgentThread::retry_message);
        assert_eq!(thread.thread.turn_error(), None);
        assert_eq!(thread.thread.failed_message(), None);
        thread
            .wait_until(|thread| {
                !thread.is_working() && thread.failed_message() == Some(FailedMessage::TurnFailed)
            })
            .await;
        thread.update(AgentThread::retry_message);
        thread
            .wait_until(|thread| {
                !thread.is_working()
                    && last_agent_message(thread).is_some_and(|text| text.contains("Echo"))
            })
            .await;
        assert_eq!(thread.thread.failed_message(), None);
        assert_eq!(thread.thread.turn_error(), None);
        assert_eq!(user_messages(&thread.thread), ["network-error"]);
        // Each try's reply is its own.
        assert_eq!(
            agent_messages(&thread.thread),
            ["Let me", "Let me", "Echo: network-error"]
        );
    }

    /// Logins that take something from the user pass it in `authenticate`'s `_meta`.
    #[tokio::test(flavor = "multi_thread")]
    async fn api_key_and_gateway_logins_pass_what_the_user_entered() {
        let Some(mut command) = mock_agent(&[]) else {
            return;
        };
        let login_dir = tempfile::tempdir().expect("temp dir");
        command.env.insert(
            "MOCK_LOGIN_FILE".into(),
            login_dir
                .path()
                .join("logged-in")
                .to_string_lossy()
                .into_owned(),
        );
        let mut thread = start(command, None);
        thread
            .wait_until(|thread| thread.status() == &ConnectionStatus::AuthRequired)
            .await;

        thread.update(|thread| thread.authenticate(acp::AuthMethodId::new("mock-api-key"), None));
        thread
            .wait_until(|thread| thread.auth_error().is_some())
            .await;
        assert!(
            thread
                .thread
                .auth_error()
                .is_some_and(|error| error.contains("No API key given"))
        );
        assert_eq!(thread.thread.status(), &ConnectionStatus::AuthRequired);

        let meta = |key: &str, value: Value| acp::Meta::from_iter([(key.to_string(), value)]);
        thread.update(|thread| {
            thread.authenticate(
                acp::AuthMethodId::new("mock-gateway"),
                Some(meta("gateway", serde_json::json!({}))),
            )
        });
        thread
            .wait_until(|thread| {
                thread
                    .auth_error()
                    .is_some_and(|error| error.contains("No gateway given"))
            })
            .await;
        thread.update(|thread| {
            thread.authenticate(
                acp::AuthMethodId::new("mock-api-key"),
                Some(meta("api-key", serde_json::json!({"apiKey": "sk-mock"}))),
            )
        });
        thread
            .wait_until(|thread| thread.status() == &ConnectionStatus::Ready)
            .await;
    }

    /// A form the agent asks to fill in during a turn.
    #[tokio::test(flavor = "multi_thread")]
    async fn answers_a_form_elicitation() {
        let Some(command) = mock_agent(&[]) else {
            return;
        };
        let mut thread = start(command, None);
        thread
            .wait_until(|thread| thread.status() == &ConnectionStatus::Ready)
            .await;
        thread.update(|thread| thread.send("form".into()));
        thread
            .wait_until(|thread| !thread.elicitations().is_empty())
            .await;
        let elicitation = thread.thread.elicitations()[0].clone();
        let acp::ElicitationMode::Form(form) = &elicitation.request.mode else {
            panic!("expected a form, got {:?}", elicitation.request.mode);
        };
        assert_eq!(form.requested_schema.properties.len(), 4);
        let content: std::collections::BTreeMap<_, _> = [
            (
                "name".to_string(),
                acp::ElicitationContentValue::String("Ada".into()),
            ),
            (
                "times".to_string(),
                acp::ElicitationContentValue::Integer(2),
            ),
        ]
        .into_iter()
        .collect();
        thread.update(|thread| {
            thread.respond_to_elicitation(
                elicitation.id,
                acp::ElicitationAction::Accept(
                    acp::ElicitationAcceptAction::new().content(content),
                ),
            )
        });
        thread.wait_until(|thread| !thread.is_working()).await;
        assert!(thread.thread.elicitations().is_empty());
        assert_eq!(
            agent_text(&thread.thread),
            r#"Form: accept {"name": "Ada", "times": 2}"#
        );
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
        thread.update(|thread| thread.authenticate(acp::AuthMethodId::new("mock-login"), None));
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
        // The message's time and the turn's length show in the thread.
        assert!(thread.thread.sent_time(0).is_some());
        assert_eq!(thread.thread.sent_time(1), None);
        assert!(thread.thread.turn_time(1).is_some());
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

    /// A thread closes its session before its agent stops: when reloading, before the new agent
    /// opens one, and when the thread goes away.
    #[tokio::test(flavor = "multi_thread")]
    async fn closes_its_session_before_the_agent_stops() {
        let Some(mut command) = mock_agent(&[]) else {
            return;
        };
        let closed_dir = tempfile::tempdir().expect("temp dir");
        let closed_file = closed_dir.path().join("closed");
        command.env.insert(
            "MOCK_CLOSED_FILE".into(),
            closed_file.to_string_lossy().into_owned(),
        );
        let closed = || std::fs::read_to_string(&closed_file).unwrap_or_default();
        let mut thread = start(command, None);
        thread
            .wait_until(|thread| thread.status() == &ConnectionStatus::Ready)
            .await;
        assert_eq!(closed(), "");

        thread.update(AgentThread::reload);
        thread
            .wait_until(|thread| thread.status() == &ConnectionStatus::Ready)
            .await;
        assert_eq!(closed(), "session-1\n");

        drop(thread);
        let waited = tokio::time::timeout(Duration::from_secs(5), async {
            while closed() != "session-1\nsession-1\n" {
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        })
        .await;
        assert!(waited.is_ok(), "closed: {:?}", closed());
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

    /// A session that doesn't load leaves the new one without the conversation: messages fail
    /// rather than go to an agent that doesn't know it, until the user sends one anyway. When
    /// the agent is logged in to another account than the session ran on, the thread says so.
    #[tokio::test(flavor = "multi_thread")]
    async fn holds_messages_when_the_conversation_is_lost() {
        let history_dir = tempfile::tempdir().expect("temp dir");
        let history_file = history_dir
            .path()
            .join("history.json")
            .to_string_lossy()
            .into_owned();
        let Some(mut command) = mock_agent(&[history_file]) else {
            return;
        };
        let home = tempfile::tempdir().expect("temp dir");
        command.env.insert(
            "MOCK_HOME".into(),
            home.path().to_string_lossy().into_owned(),
        );
        let email = home.path().join("email");
        std::fs::write(&email, "a@example.com").expect("written");
        let session_logins = |thread: &TestThread| -> Vec<LoginIdentity> {
            thread
                .events
                .iter()
                .filter_map(|event| match event {
                    AgentThreadEvent::SessionLogin(login) => Some(login.clone()),
                    _ => None,
                })
                .collect()
        };
        let login = |email: &str| LoginIdentity {
            kind: "account".into(),
            key: Some(email.into()),
        };

        let mut first = start(command.clone(), None);
        first
            .wait_until(|thread| thread.status() == &ConnectionStatus::Ready)
            .await;
        first.update(|thread| thread.send("hello".into()));
        first
            .wait_until(|thread| !thread.is_working() && thread.entries().len() >= 3)
            .await;
        assert_eq!(session_logins(&first), [login("a@example.com")]);

        let session = Some(acp::SessionId::new("session-1"));
        let mut same = start(command.clone(), session.clone());
        same.update(|thread| thread.set_session_login(Some(login("a@example.com"))));
        same.wait_until(|thread| thread.status() == &ConnectionStatus::Ready)
            .await;
        assert_eq!(same.thread.session_restore(), Some(SessionRestore::Loaded));
        assert_eq!(same.thread.lost_history(), None);
        drop(same);

        std::fs::write(&email, "b@example.com").expect("written");
        let mut other = start(command, session);
        other.update(|thread| thread.set_session_login(Some(login("a@example.com"))));
        // Sent while the session opens, it waits for it.
        other.update(|thread| thread.send("go on".into()));
        other
            .wait_until(|thread| {
                thread.failed_message() == Some(FailedMessage::LostHistory)
                    && thread.lost_history() == Some(LostHistory::AccountChanged)
            })
            .await;
        assert_eq!(
            other.thread.session_restore(),
            Some(SessionRestore::Unavailable)
        );
        assert!(!other.thread.is_working());
        other.update(|thread| thread.send("are you there?".into()));
        assert_eq!(
            other.thread.failed_message(),
            Some(FailedMessage::LostHistory)
        );
        assert!(!other.thread.is_working());
        assert!(session_logins(&other).is_empty());

        other.update(AgentThread::retry_message);
        other
            .wait_until(|thread| {
                !thread.is_working()
                    && thread
                        .entries()
                        .contains(&Entry::AgentMessage("Echo: are you there?".into()))
            })
            .await;
        assert_eq!(other.thread.failed_message(), None);
        assert_eq!(other.thread.lost_history(), None);
        assert_eq!(session_logins(&other), [login("b@example.com")]);
        other.update(|thread| thread.send("next".into()));
        other
            .wait_until(|thread| {
                !thread.is_working()
                    && thread
                        .entries()
                        .contains(&Entry::AgentMessage("Echo: next".into()))
            })
            .await;
        let replies: Vec<&Entry> = other
            .thread
            .entries()
            .iter()
            .filter(|entry| matches!(entry, Entry::AgentMessage(_)))
            .collect();
        assert_eq!(
            replies,
            [
                &Entry::AgentMessage("Echo: are you there?".into()),
                &Entry::AgentMessage("Echo: next".into())
            ]
        );
    }

    /// A thread started with its transcript shows it, and drops what the agent replays of it as
    /// the session loads, as when the agent reloads. One that has none yet keeps the replay.
    #[tokio::test(flavor = "multi_thread")]
    async fn keeps_its_transcript_over_the_replay() {
        let history_dir = tempfile::tempdir().expect("temp dir");
        let history_file = history_dir
            .path()
            .join("history.json")
            .to_string_lossy()
            .into_owned();
        let Some(command) = mock_agent(&[history_file]) else {
            return;
        };
        let mut first = start(command.clone(), None);
        assert_eq!(first.thread.conversation_revision(), None);
        first
            .wait_until(|thread| thread.status() == &ConnectionStatus::Ready)
            .await;
        first.update(|thread| thread.send("hello".into()));
        first
            .wait_until(|thread| !thread.is_working() && thread.entries().len() >= 3)
            .await;
        let transcript =
            Transcript::from(first.thread.transcript().expect("the whole conversation"));
        assert_eq!(transcript.entries, first.thread.entries());
        assert_eq!(transcript.finished_turns.len(), 1);
        assert!(transcript.sent_times.iter().any(|(index, _)| *index == 0));

        // Only the end of a long conversation comes back from some agents: the thread keeps
        // the start the agent no longer replays.
        let mut kept = transcript.clone();
        kept.entries
            .insert(0, Entry::AgentMessage("From before the replay".into()));
        let session = Some(acp::SessionId::new("session-1"));
        let mut second = start(command.clone(), session.clone());
        second.update(|thread| thread.restore_transcript(kept.clone()));
        assert_eq!(second.thread.entries(), kept.entries);
        assert!(second.thread.conversation_revision().is_some());
        second
            .wait_until(|thread| thread.status() == &ConnectionStatus::Ready)
            .await;
        assert_eq!(
            second.thread.session_restore(),
            Some(SessionRestore::Loaded)
        );
        assert_eq!(second.thread.entries(), kept.entries);
        assert_eq!(
            second.thread.transcript().map(Transcript::from),
            Some(kept.clone())
        );

        second.update(AgentThread::reload);
        second
            .wait_until(|thread| thread.status() == &ConnectionStatus::Ready)
            .await;
        assert_eq!(second.thread.entries(), kept.entries);

        let mut replayed = start(command, session);
        replayed
            .wait_until(|thread| thread.status() == &ConnectionStatus::Ready)
            .await;
        // A replayed tool call has no times: it didn't run here.
        let untimed = |entries: &[Entry]| -> Vec<Entry> {
            entries
                .iter()
                .cloned()
                .map(|mut entry| {
                    if let Entry::ToolCall(tool_call) = &mut entry {
                        tool_call.started_at = None;
                        tool_call.duration = None;
                    }
                    entry
                })
                .collect()
        };
        assert_eq!(replayed.thread.entries(), untimed(&transcript.entries));
    }

    /// Every page of an agent's sessions is listed, and a thread opened with a listed session
    /// loads that session's conversation. Agents that can't list, or want a login, say so.
    #[tokio::test(flavor = "multi_thread")]
    async fn lists_an_agents_sessions() {
        let Some(mut command) = mock_agent(&[]) else {
            return;
        };
        let unsupported = list_sessions(ready(command.clone()))
            .await
            .expect("listing");
        assert!(matches!(unsupported, SessionListing::Unsupported));

        let sessions_dir = tempfile::tempdir().expect("temp dir");
        let sessions_file = sessions_dir.path().join("sessions.json");
        let sessions: Vec<Value> = (1..=5)
            .map(|number| {
                serde_json::json!({
                    "sessionId": format!("listed-{number}"),
                    "cwd": format!("/projects/{}", number % 2),
                    "title": format!("Session {number}"),
                    "history": [{"sessionUpdate": "agent_message_chunk",
                                 "content": {"type": "text", "text": format!("Earlier {number}")}}],
                })
            })
            .collect();
        std::fs::write(&sessions_file, serde_json::to_vec(&sessions).expect("json"))
            .expect("writing sessions");
        command.env.insert(
            "MOCK_SESSIONS_FILE".into(),
            sessions_file.to_string_lossy().into_owned(),
        );
        let SessionListing::Listed(listed) = list_sessions(ready(command.clone()))
            .await
            .expect("listing")
        else {
            panic!("the mock agent lists its sessions");
        };
        let ids: Vec<&str> = listed
            .iter()
            .map(|session| &*session.session_id.0)
            .collect();
        assert_eq!(
            ids,
            ["listed-1", "listed-2", "listed-3", "listed-4", "listed-5"]
        );
        assert_eq!(listed[1].cwd, PathBuf::from("/projects/0"));
        assert_eq!(listed[1].title.as_deref(), Some("Session 2"));

        let mut thread = start(command.clone(), Some(acp::SessionId::new("listed-3")));
        thread
            .wait_until(|thread| thread.status() == &ConnectionStatus::Ready)
            .await;
        assert_eq!(
            thread.thread.entries(),
            [Entry::AgentMessage("Earlier 3".into())]
        );

        command.env.insert(
            "MOCK_LOGIN_FILE".into(),
            sessions_dir
                .path()
                .join("logged-in")
                .to_string_lossy()
                .into_owned(),
        );
        let logged_out = list_sessions(ready(command)).await.expect("listing");
        assert!(matches!(logged_out, SessionListing::LoggedOut));
    }

    /// Pauses the thread, hands its agent to a new thread as another server would, and
    /// drops the old one.
    async fn hand_to_new_thread(mut old: TestThread) -> TestThread {
        old.update(|thread| assert!(thread.pause(), "pausing starts"));
        old.wait_until(|thread| thread.wire.as_ref().is_some_and(Wire::is_paused))
            .await;
        assert!(
            old.events
                .iter()
                .any(|event| matches!(event, AgentThreadEvent::Paused))
        );
        let handed_off = old.thread.hand_off().expect("the thread can be handed off");
        old.thread.release();
        // As the snapshot crosses to the other server.
        let snapshot: AgentSnapshot =
            serde_json::from_str(&serde_json::to_string(&handed_off.snapshot).unwrap()).unwrap();
        drop(old);
        TestThread::new(
            AgentThread::adopt(
                tokio::runtime::Handle::current(),
                snapshot,
                handed_off.stdin,
                handed_off.stdout,
                handed_off.stderr,
                None,
            )
            .expect("the agent is adopted"),
        )
    }

    fn agent_text(thread: &AgentThread) -> String {
        thread
            .entries()
            .iter()
            .filter_map(|entry| match entry {
                Entry::AgentMessage(text) => Some(text.as_str()),
                _ => None,
            })
            .collect()
    }

    /// A turn streaming when its agent is handed off goes on and ends on the new connection.
    #[tokio::test(flavor = "multi_thread")]
    async fn hands_off_a_turn_in_progress() {
        let Some(mut command) = mock_agent(&[]) else {
            return;
        };
        let closed_dir = tempfile::tempdir().expect("temp dir");
        let closed_file = closed_dir.path().join("closed");
        command.env.insert(
            "MOCK_CLOSED_FILE".into(),
            closed_file.to_string_lossy().into_owned(),
        );
        let mut thread = start(command, None);
        thread
            .wait_until(|thread| thread.status() == &ConnectionStatus::Ready)
            .await;
        thread.update(|thread| thread.send("slow".into()));
        thread
            .wait_until(|thread| agent_text(thread).contains("One"))
            .await;
        assert!(thread.thread.can_hand_off());

        let mut thread = hand_to_new_thread(thread).await;
        assert!(thread.thread.is_working(), "the turn goes on");
        thread.wait_until(|thread| !thread.is_working()).await;
        assert!(
            thread
                .events
                .iter()
                .any(|event| matches!(event, AgentThreadEvent::WorkingChanged(true)))
        );
        assert_eq!(agent_text(&thread.thread), "One two three four five");
        assert_eq!(
            thread.thread.last_stop_reason(),
            Some(&acp::StopReason::EndTurn)
        );

        // The new connection works like any other.
        thread.update(|thread| thread.send("hello".into()));
        thread.wait_until(|thread| !thread.is_working()).await;
        assert!(agent_text(&thread.thread).ends_with("Echo: hello"));
        // The thread that handed the agent off left its session open.
        assert_eq!(
            std::fs::read_to_string(&closed_file).unwrap_or_default(),
            ""
        );
    }

    /// A permission request the agent waits on is asked again on the new connection, which
    /// answers it.
    #[tokio::test(flavor = "multi_thread")]
    async fn hands_off_a_waiting_permission_request() {
        let Some(command) = mock_agent(&[]) else {
            return;
        };
        let mut thread = start(command, None);
        thread
            .wait_until(|thread| thread.status() == &ConnectionStatus::Ready)
            .await;
        thread.update(|thread| thread.send("permission".into()));
        thread
            .wait_until(|thread| !thread.state.permission_requests.is_empty())
            .await;

        let mut thread = hand_to_new_thread(thread).await;
        thread
            .wait_until(|thread| !thread.state.permission_requests.is_empty())
            .await;
        assert_eq!(thread.thread.state.permission_requests.len(), 1);
        thread.update(|thread| {
            thread.respond_to_permission(
                &acp::ToolCallId::new("call-2"),
                acp::PermissionOptionId::new("allow"),
            )
        });
        thread.wait_until(|thread| !thread.is_working()).await;
        assert!(
            agent_text(&thread.thread).ends_with("(chose allow)"),
            "{:?} {:?} {:?}",
            thread.thread.entries(),
            thread.thread.turn_error(),
            thread.thread.status()
        );
    }

    /// A pause that's called off loses nothing.
    #[tokio::test(flavor = "multi_thread")]
    async fn resumes_after_a_pause() {
        let Some(command) = mock_agent(&[]) else {
            return;
        };
        let mut thread = start(command, None);
        thread
            .wait_until(|thread| thread.status() == &ConnectionStatus::Ready)
            .await;
        thread.update(|thread| thread.send("slow".into()));
        thread
            .wait_until(|thread| agent_text(thread).contains("One"))
            .await;
        thread.update(|thread| assert!(thread.pause()));
        thread
            .wait_until(|thread| thread.wire.as_ref().is_some_and(Wire::is_paused))
            .await;
        thread.update(AgentThread::resume);
        assert!(!thread.thread.is_paused());
        thread.wait_until(|thread| !thread.is_working()).await;
        assert_eq!(agent_text(&thread.thread), "One two three four five");
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
