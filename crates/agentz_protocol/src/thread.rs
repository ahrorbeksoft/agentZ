//! A thread's conversation as the server keeps it and clients see it.

use std::path::PathBuf;
use std::time::SystemTime;

use agent_client_protocol::schema::v1 as acp;
use gpui_shared_string::SharedString;
use serde::{Deserialize, Serialize};

use crate::agents::AgentCommand;
use projects::ThreadCreator;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub enum ConnectionStatus {
    #[default]
    Connecting,
    /// The agent is running but needs the user to log in before a session can start.
    AuthRequired,
    Ready,
    Failed(SharedString),
}

/// Context-window usage reported by the agent.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct ContextUsage {
    pub used: u64,
    pub size: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Entry {
    UserMessage(String),
    AgentMessage(String),
    AgentThought(String),
    ToolCall(ToolCall),
    /// Where the plan first appeared; the plan itself is kept up to date in [`ThreadView::plan`].
    Plan,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
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
    /// Terminals the agent runs the tool in (ACP's `terminal/create`), by the ids it got:
    /// [`crate::terminal::TerminalKey::Agent`].
    #[serde(default)]
    pub terminals: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DiffLineKind {
    Context,
    Removed,
    Added,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PlanItem {
    pub content: String,
    pub status: acp::PlanEntryStatus,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PermissionOption {
    pub id: acp::PermissionOptionId,
    pub name: String,
    pub kind: acp::PermissionOptionKind,
}

/// A permission request waiting for an answer.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PermissionRequest {
    pub tool_call_id: acp::ToolCallId,
    pub title: String,
    pub options: Vec<PermissionOption>,
}

/// A request for input from the agent (ACP's `elicitation/create`), waiting on the user.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Elicitation {
    /// The thread's own id for it, to answer with.
    pub id: u64,
    pub request: acp::CreateElicitationRequest,
    /// The user opened its URL (which answered it), and the agent hasn't said it's done yet.
    #[serde(default)]
    pub opened: bool,
}

impl Elicitation {
    /// The URL to open, for a URL elicitation.
    pub fn url(&self) -> Option<&str> {
        match &self.request.mode {
            acp::ElicitationMode::Url(url) => Some(&url.url),
            _ => None,
        }
    }
}

/// The account an agent says it's logged in to, from its `_auth/status_update` notifications
/// (Claude Agent and Codex send them). Kept as the agent words it.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct AuthStatus {
    /// `none` when logged out; otherwise how it's logged in (`account`, `api_key`, …).
    pub kind: String,
    pub label: Option<String>,
    pub account: Option<AuthAccount>,
    pub detail: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct AuthAccount {
    pub email: Option<String>,
    pub plan: Option<String>,
    pub organization: Option<String>,
}

impl AuthStatus {
    pub fn is_logged_in(&self) -> bool {
        !self.kind.is_empty() && self.kind != "none"
    }

    /// Which login this is, compared as Claude Agent compares them: by the account's email,
    /// by where an API key comes from, or by the kind alone.
    pub fn identity(&self) -> LoginIdentity {
        let key = match self.kind.as_str() {
            "account" => self
                .account
                .as_ref()
                .and_then(|account| account.email.clone()),
            "api_key" => self.detail.clone(),
            _ => None,
        };
        LoginIdentity {
            kind: self.kind.clone(),
            key: key.filter(|key| !key.trim().is_empty()),
        }
    }
}

/// What tells one login from another: see [`AuthStatus::identity`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoginIdentity {
    pub kind: String,
    /// The email or key source, when the agent says which.
    #[serde(default)]
    pub key: Option<String>,
}

impl LoginIdentity {
    /// Whether `other` is a different login. One the agent reported without its email (Claude
    /// Agent's CLI check leaves it out at times) can't be told from one with it.
    pub fn differs_from(&self, other: &LoginIdentity) -> bool {
        self.kind != other.kind
            || matches!((&self.key, &other.key), (Some(key), Some(other)) if key != other)
    }
}

/// How the thread's ACP session was set up when the agent started.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
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

/// Settings applied to new sessions (not to loaded ones), as Zed's per-agent defaults are.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SessionDefaults {
    pub mode: Option<acp::SessionModeId>,
    pub config_options: Vec<(acp::SessionConfigId, acp::SessionConfigOptionValue)>,
}

/// Everything about a thread except its entries, which are sent separately because they grow.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ThreadState {
    pub agent_name: SharedString,
    pub status: ConnectionStatus,
    pub plan: Vec<PlanItem>,
    /// Settings the agent exposes for this session (model, effort, mode, …).
    pub config_options: Vec<acp::SessionConfigOption>,
    /// Session modes from agents that predate config options.
    pub modes: Option<acp::SessionModeState>,
    pub session_restore: Option<SessionRestore>,
    pub permission_requests: Vec<PermissionRequest>,
    pub capabilities: acp::AgentCapabilities,
    pub auth_methods: Vec<acp::AuthMethod>,
    pub auth_error: Option<SharedString>,
    /// What the agent said when it asked for a login, if more than "authentication
    /// required": Factory Droid gives its pairing code here.
    pub auth_description: Option<SharedString>,
    /// The login method whose `authenticate` is in flight. Browser logins wait in it until
    /// the user finishes in the browser.
    pub authenticating: Option<acp::AuthMethodId>,
    /// Links the agent printed while logging in. They are how to finish when the agent's
    /// machine can't open a browser itself (a remote machine).
    pub auth_links: Vec<SharedString>,
    /// A one-time code the agent printed while logging in, to enter on the page it links to
    /// (a device login).
    pub auth_code: Option<SharedString>,
    /// The conversation this thread continues, to go with its first message, while it waits
    /// for one ("Continue with another agent").
    pub handoff: Option<PendingHandoff>,
    /// The page the agent tried to open in a browser while logging in, on a machine agentZ
    /// reaches over SSH. A browser there isn't one the user sees, so agentZ gives agents its own
    /// `xdg-open`, which hands the page to the clients to open instead.
    pub login_page: Option<SharedString>,
    /// The account the agent reported, if it reports one.
    pub auth_status: Option<AuthStatus>,
    /// Requests for input from the agent, oldest first.
    pub elicitations: Vec<Elicitation>,
    /// The command the agent was started with, for its terminal login methods.
    pub command: Option<AgentCommand>,
    pub cwd: PathBuf,
    pub usage: Option<ContextUsage>,
    pub cost: Option<acp::Cost>,
    pub available_commands: Vec<acp::AvailableCommand>,
    pub turn_started_at: Option<SystemTime>,
    pub last_stop_reason: Option<acp::StopReason>,
    pub turn_error: Option<SharedString>,
    /// The outcome of the last log in or out on a connection made only for that.
    pub account_notice: Option<SharedString>,
    /// What the agent says about itself when it starts.
    pub agent_info: Option<acp::Implementation>,
    /// Whether the agent let a session open (logged in) or asked for a login. ACP has no way to
    /// ask directly, so this is the closest status there is. `None` until known.
    pub logged_in: Option<bool>,
    /// User messages that an agent sent (through MCP or the CLI) rather than the user, by
    /// entry index, in order. A list rather than a map: integer map keys don't survive serde's
    /// buffering of the protocol's untagged fallbacks.
    pub prompts_from_agents: Vec<(usize, ThreadCreator)>,
}

/// A thread's state and entries, with the read API both the server's thread and the clients'
/// copies offer.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ThreadView {
    pub state: ThreadState,
    pub entries: Vec<Entry>,
}

/// What changed in a [`ThreadView`], from [`ThreadView::changes_since`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ThreadUpdate {
    /// The new state, if it changed.
    #[serde(default)]
    pub state: Option<ThreadState>,
    pub entry_count: usize,
    /// Entries that changed or were added, by index.
    #[serde(default)]
    pub entries: Vec<(usize, Entry)>,
}

impl ThreadView {
    /// What changed since `previous`, or `None` if nothing did.
    pub fn changes_since(&self, previous: &ThreadView) -> Option<ThreadUpdate> {
        let state = (self.state != previous.state).then(|| self.state.clone());
        let entries: Vec<_> = self
            .entries
            .iter()
            .enumerate()
            .filter(|(index, entry)| previous.entries.get(*index) != Some(entry))
            .map(|(index, entry)| (index, entry.clone()))
            .collect();
        if state.is_none() && entries.is_empty() && self.entries.len() == previous.entries.len() {
            return None;
        }
        Some(ThreadUpdate {
            state,
            entry_count: self.entries.len(),
            entries,
        })
    }

    pub fn apply(&mut self, update: ThreadUpdate) {
        if let Some(state) = update.state {
            self.state = state;
        }
        self.entries.truncate(update.entry_count);
        for (index, entry) in update.entries {
            if index < self.entries.len() {
                self.entries[index] = entry;
            } else if index == self.entries.len() {
                self.entries.push(entry);
            } else {
                log::error!("thread update skipped entries before {index}");
            }
        }
    }

    pub fn agent_name(&self) -> &SharedString {
        &self.state.agent_name
    }

    /// The agent that sent the user message at `index`, if one did.
    pub fn prompt_sender(&self, index: usize) -> Option<ThreadCreator> {
        self.state
            .prompts_from_agents
            .iter()
            .find(|(prompt_index, _)| *prompt_index == index)
            .map(|(_, sender)| *sender)
    }

    pub fn status(&self) -> &ConnectionStatus {
        &self.state.status
    }

    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    pub fn plan(&self) -> &[PlanItem] {
        &self.state.plan
    }

    pub fn session_restore(&self) -> Option<SessionRestore> {
        self.state.session_restore
    }

    pub fn config_options(&self) -> &[acp::SessionConfigOption] {
        &self.state.config_options
    }

    pub fn modes(&self) -> Option<&acp::SessionModeState> {
        self.state.modes.as_ref()
    }

    /// Whether the agent advertises ACP's logout method.
    pub fn supports_logout(&self) -> bool {
        self.state.capabilities.auth.logout.is_some()
    }

    pub fn supports_images(&self) -> bool {
        self.state.capabilities.prompt_capabilities.image
    }

    pub fn auth_methods(&self) -> &[acp::AuthMethod] {
        &self.state.auth_methods
    }

    pub fn auth_error(&self) -> Option<&SharedString> {
        self.state.auth_error.as_ref()
    }

    pub fn auth_description(&self) -> Option<&SharedString> {
        self.state.auth_description.as_ref()
    }

    /// The login method being authenticated, while `authenticate` is in flight.
    pub fn authenticating(&self) -> Option<&acp::AuthMethod> {
        let method_id = self.state.authenticating.as_ref()?;
        self.state
            .auth_methods
            .iter()
            .find(|method| method.id() == method_id)
    }

    pub fn is_authenticating(&self) -> bool {
        self.state.authenticating.is_some()
    }

    pub fn auth_links(&self) -> &[SharedString] {
        &self.state.auth_links
    }

    pub fn auth_code(&self) -> Option<&SharedString> {
        self.state.auth_code.as_ref()
    }

    pub fn login_page(&self) -> Option<&SharedString> {
        self.state.login_page.as_ref()
    }

    pub fn pending_handoff(&self) -> Option<&PendingHandoff> {
        self.state.handoff.as_ref()
    }

    /// Whether the agent is waiting on the user to answer a request for input.
    pub fn is_awaiting_input(&self) -> bool {
        self.state
            .elicitations
            .iter()
            .any(|elicitation| !elicitation.opened)
    }

    pub fn auth_status(&self) -> Option<&AuthStatus> {
        self.state.auth_status.as_ref()
    }

    pub fn elicitations(&self) -> &[Elicitation] {
        &self.state.elicitations
    }

    /// What happened on the last log in or out of an account connection.
    pub fn account_notice(&self) -> Option<&SharedString> {
        self.state.account_notice.as_ref()
    }

    pub fn agent_info(&self) -> Option<&acp::Implementation> {
        self.state.agent_info.as_ref()
    }

    /// Whether the agent is logged in, as far as ACP can tell; see [`ThreadState::logged_in`].
    pub fn logged_in(&self) -> Option<bool> {
        self.state.logged_in
    }

    pub fn cwd(&self) -> &PathBuf {
        &self.state.cwd
    }

    pub fn context_usage(&self) -> Option<ContextUsage> {
        self.state.usage
    }

    pub fn cost(&self) -> Option<&acp::Cost> {
        self.state.cost.as_ref()
    }

    pub fn available_commands(&self) -> &[acp::AvailableCommand] {
        &self.state.available_commands
    }

    pub fn is_working(&self) -> bool {
        self.state.turn_started_at.is_some()
    }

    pub fn turn_started_at(&self) -> Option<SystemTime> {
        self.state.turn_started_at
    }

    pub fn turn_error(&self) -> Option<&SharedString> {
        self.state.turn_error.as_ref()
    }

    pub fn last_stop_reason(&self) -> Option<&acp::StopReason> {
        self.state.last_stop_reason.as_ref()
    }

    pub fn permission_request(&self, tool_call_id: &acp::ToolCallId) -> Option<&PermissionRequest> {
        self.state
            .permission_requests
            .iter()
            .find(|request| &request.tool_call_id == tool_call_id)
    }

    /// Permission requests whose tool call isn't shown as an entry.
    pub fn orphan_permission_requests(&self) -> impl Iterator<Item = &PermissionRequest> {
        self.state.permission_requests.iter().filter(|request| {
            !self.entries.iter().any(|entry| {
                matches!(entry, Entry::ToolCall(tool_call) if tool_call.id == request.tool_call_id)
            })
        })
    }

    /// The display name of the model the agent's model selector currently has chosen.
    pub fn model_name(&self) -> Option<String> {
        self.state.config_options.iter().find_map(|option| {
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

    /// The command to run in a terminal for one of the agent's terminal login methods.
    pub fn terminal_auth_command(&self, method_id: &acp::AuthMethodId) -> Option<AgentCommand> {
        let command = self.state.command.as_ref()?;
        let method = self
            .state
            .auth_methods
            .iter()
            .find(|method| method.id() == method_id)?;
        terminal_login_command(command, method)
    }
}

/// What a login method takes from the user before `authenticate`, as its `_meta` says. Agents
/// read it back from `authenticate`'s `_meta` under the same key.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LoginInput {
    /// Nothing: the agent logs in by itself, often in a browser on its machine.
    Nothing,
    /// An API key: `_meta["api-key"]` (Codex's `api-key`), answered with
    /// `{"api-key": {"apiKey": …}}`, which Antigravity also reads.
    ApiKey,
    /// An LLM gateway: `_meta["gateway"]` (Claude Agent's and Codex's), answered with
    /// `{"gateway": {"baseUrl": …, "headers": {…}}}`.
    Gateway,
}

pub fn login_input(method: &acp::AuthMethod) -> LoginInput {
    let Some(meta) = method.meta() else {
        return LoginInput::Nothing;
    };
    if meta.contains_key("api-key") {
        LoginInput::ApiKey
    } else if meta.contains_key("gateway") {
        LoginInput::Gateway
    } else {
        LoginInput::Nothing
    }
}

/// `authenticate`'s `_meta` for an [`LoginInput::ApiKey`] method.
pub fn api_key_meta(api_key: &str) -> acp::Meta {
    acp::Meta::from_iter([(
        "api-key".to_string(),
        serde_json::json!({ "apiKey": api_key }),
    )])
}

/// `authenticate`'s `_meta` for a [`LoginInput::Gateway`] method.
pub fn gateway_meta(base_url: &str, headers: &[(String, String)]) -> acp::Meta {
    let headers: serde_json::Map<String, serde_json::Value> = headers
        .iter()
        .map(|(name, value)| (name.clone(), value.clone().into()))
        .collect();
    acp::Meta::from_iter([(
        "gateway".to_string(),
        serde_json::json!({ "baseUrl": base_url, "headers": headers }),
    )])
}

/// Opens the context a continued thread's first message brings: [`handoff`].
pub const HANDOFF_OPENING: &str = "<agentz-handoff";
const HANDOFF_CLOSING: &str = "</agentz-handoff>";
/// How much of the old conversation goes along, in characters: a long thread's gist without
/// filling much of the new agent's context.
const HANDOFF_BUDGET: usize = 40_000;
/// The most kept of any one message.
const HANDOFF_MESSAGE_LIMIT: usize = 6_000;
/// The most tool calls named for one of the agent's turns.
const HANDOFF_TOOL_LIMIT: usize = 20;

/// The conversation a thread continues with another agent, sent with its first message
/// ("Continue with another agent").
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PendingHandoff {
    /// The thread it continues, which this one links to once it's sent.
    pub from: projects::ThreadId,
    pub from_title: String,
    pub from_agent: String,
    pub text: String,
    /// How many messages (the user's and the agent's) the conversation had.
    pub messages: usize,
}

/// A thread's conversation for another agent to continue: the user's messages, the agent's
/// replies with the tools it used, and the plan, as t3code's deterministic handoff
/// summarizes. The first message (the goal) and the latest ones are kept when it's long.
pub fn handoff(
    view: &ThreadView,
    from: projects::ThreadId,
    agent_name: &str,
    title: &str,
) -> PendingHandoff {
    enum Part {
        User(String),
        Agent { text: String, tools: Vec<String> },
    }
    let mut parts: Vec<Part> = Vec::new();
    for entry in &view.entries {
        match entry {
            Entry::UserMessage(text) => {
                let text = without_handoff(text).trim();
                if !text.is_empty() {
                    parts.push(Part::User(text.to_string()));
                }
            }
            Entry::AgentMessage(text) => match parts.last_mut() {
                Some(Part::Agent { text: reply, .. }) => {
                    if !reply.is_empty() {
                        reply.push_str("\n\n");
                    }
                    reply.push_str(text.trim());
                }
                _ => parts.push(Part::Agent {
                    text: text.trim().to_string(),
                    tools: Vec::new(),
                }),
            },
            Entry::ToolCall(tool_call) => {
                let mut tool = tool_call.title.clone();
                if tool_call.status == acp::ToolCallStatus::Failed {
                    tool.push_str(" (failed)");
                }
                match parts.last_mut() {
                    Some(Part::Agent { tools, .. }) => tools.push(tool),
                    _ => parts.push(Part::Agent {
                        text: String::new(),
                        tools: vec![tool],
                    }),
                }
            }
            Entry::AgentThought(_) | Entry::Plan => {}
        }
    }
    let messages = parts.len();
    let attribute = |value: &str| value.replace('"', "'");
    // Tags rather than headings, which the messages' own markdown has too.
    let sections: Vec<String> = parts
        .into_iter()
        .map(|part| match part {
            Part::User(text) => format!(
                "<message from=\"user\">\n{}\n</message>",
                clip(&text, HANDOFF_MESSAGE_LIMIT)
            ),
            Part::Agent { text, tools } => {
                let mut section = format!("<message from=\"{}\">", attribute(agent_name));
                if !text.is_empty() {
                    section.push('\n');
                    section.push_str(&clip(&text, HANDOFF_MESSAGE_LIMIT));
                }
                if !tools.is_empty() {
                    let shown = tools.len().min(HANDOFF_TOOL_LIMIT);
                    section.push_str("\nTools: ");
                    section.push_str(&tools[..shown].join(" · "));
                    if tools.len() > shown {
                        section.push_str(&format!(" · and {} more", tools.len() - shown));
                    }
                }
                section.push_str("\n</message>");
                section
            }
        })
        .collect();

    // The first section is the goal; then as many of the latest as fit.
    let mut kept_latest: Vec<&String> = Vec::new();
    let mut used = sections.first().map_or(0, String::len);
    for section in sections.iter().skip(1).rev() {
        if used + section.len() > HANDOFF_BUDGET {
            break;
        }
        used += section.len();
        kept_latest.push(section);
    }
    kept_latest.reverse();
    let left_out = sections.len().saturating_sub(1 + kept_latest.len());
    let mut body: Vec<String> = sections.first().cloned().into_iter().collect();
    if left_out > 0 {
        body.push(format!("(… {left_out} messages left out …)"));
    }
    body.extend(kept_latest.into_iter().cloned());
    if !view.state.plan.is_empty() {
        let plan: Vec<String> = view
            .state
            .plan
            .iter()
            .map(|item| {
                let mark = match item.status {
                    acp::PlanEntryStatus::Completed => "x",
                    acp::PlanEntryStatus::InProgress => "~",
                    _ => " ",
                };
                format!("- [{mark}] {}", item.content)
            })
            .collect();
        body.push(format!("<plan>\n{}\n</plan>", plan.join("\n")));
    }

    let text = format!(
        "{HANDOFF_OPENING} from=\"{agent}\" thread=\"{title}\">\n\
         The user is continuing, with you, a conversation they had with {agent_name} in this \
         same folder. This is what happened there, oldest first. Files may have changed since, \
         so check them before relying on details. The user's message to you follows.\n\n\
         {body}\n{HANDOFF_CLOSING}",
        agent = attribute(agent_name),
        title = attribute(title),
        body = body.join("\n\n"),
    );
    PendingHandoff {
        from,
        from_title: title.to_string(),
        from_agent: agent_name.to_string(),
        text,
        messages,
    }
}

/// A user message without the handoff it began with, as an agent may replay it.
pub fn without_handoff(text: &str) -> &str {
    let trimmed = text.trim_start();
    if !trimmed.starts_with(HANDOFF_OPENING) {
        return text;
    }
    match trimmed.find(HANDOFF_CLOSING) {
        Some(end) => trimmed[end + HANDOFF_CLOSING.len()..].trim_start(),
        None => text,
    }
}

/// At most `limit` characters, ending in "…" when cut.
fn clip(text: &str, limit: usize) -> String {
    match text.char_indices().nth(limit) {
        Some((end, _)) => format!("{}…", &text[..end]),
        None => text.to_string(),
    }
}

/// A one-time code in what an agent says while logging in, as device logins print one ("Enter
/// this one-time code: ABCD-1234"). Only in text that talks about a code, and only a word shaped
/// like one: two or three groups of capitals and digits joined by dashes.
pub fn login_code(text: &str) -> Option<String> {
    let text = strip_ansi_escapes(text);
    if !text.to_lowercase().contains("code") {
        return None;
    }
    text.split_whitespace()
        .map(|word| word.trim_matches(|character: char| !character.is_ascii_alphanumeric()))
        .find(|word| is_login_code(word))
        .map(str::to_string)
}

fn is_login_code(word: &str) -> bool {
    let groups: Vec<&str> = word.split('-').collect();
    (2..=3).contains(&groups.len())
        && groups.iter().all(|group| {
            (3..=8).contains(&group.len())
                && group
                    .chars()
                    .all(|character| character.is_ascii_uppercase() || character.is_ascii_digit())
        })
}

/// Text without the terminal color and cursor sequences agents put in what they print.
fn strip_ansi_escapes(text: &str) -> String {
    let mut stripped = String::with_capacity(text.len());
    let mut characters = text.chars().peekable();
    while let Some(character) = characters.next() {
        if character != '\u{1b}' {
            stripped.push(character);
            continue;
        }
        if characters.peek() == Some(&'[') {
            characters.next();
            for character in characters.by_ref() {
                if character.is_ascii_alphabetic() || character == '~' {
                    break;
                }
            }
        }
    }
    stripped
}

/// Whether the login method runs in a terminal rather than through ACP's `authenticate`.
pub fn logs_in_through_terminal(method: &acp::AuthMethod) -> bool {
    matches!(method, acp::AuthMethod::Terminal(_)) || meta_terminal_auth(method).is_some()
}

/// What a terminal login method runs, given the agent's own command. A `terminal` method runs
/// the agent with its arguments and environment added. Agents from before terminal methods
/// were stabilized name a command in `_meta["terminal-auth"]` instead, which Zed still reads.
pub fn terminal_login_command(
    agent: &AgentCommand,
    method: &acp::AuthMethod,
) -> Option<AgentCommand> {
    if let acp::AuthMethod::Terminal(terminal) = method {
        let mut command = agent.clone();
        command.args.extend(terminal.args.iter().cloned());
        command.env.extend(
            terminal
                .env
                .iter()
                .map(|(key, value)| (key.clone(), value.clone())),
        );
        return Some(command);
    }
    let meta = meta_terminal_auth(method)?;
    // A bare `node` or `opencode` means the program the agent itself runs from, which may not
    // be on `PATH` (Zed swaps in its own Node the same way).
    let path = if agent
        .path
        .file_name()
        .is_some_and(|name| name.to_string_lossy() == meta.command)
    {
        agent.path.clone()
    } else {
        PathBuf::from(meta.command)
    };
    let mut env = agent.env.clone();
    env.extend(meta.env);
    Some(AgentCommand {
        path,
        args: meta.args,
        env,
    })
}

#[derive(Deserialize)]
struct MetaTerminalAuth {
    command: String,
    #[serde(default)]
    args: Vec<String>,
    #[serde(default)]
    env: std::collections::HashMap<String, String>,
}

fn meta_terminal_auth(method: &acp::AuthMethod) -> Option<MetaTerminalAuth> {
    let value = method.meta()?.get("terminal-auth")?.clone();
    serde_json::from_value(value).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tool_call(title: &str, status: acp::ToolCallStatus) -> Entry {
        Entry::ToolCall(ToolCall {
            id: acp::ToolCallId::new(title.to_string()),
            title: title.to_string(),
            kind: acp::ToolKind::Other,
            status,
            text: Vec::new(),
            diffs: Vec::new(),
            locations: Vec::new(),
            raw_input: None,
            terminals: Vec::new(),
        })
    }

    #[test]
    fn handoffs_tell_the_conversation() {
        let mut view = ThreadView::default();
        view.entries = vec![
            Entry::UserMessage("Build the checkout page".into()),
            Entry::AgentThought("The user wants a page.".into()),
            tool_call("Read src/cart.tsx", acp::ToolCallStatus::Completed),
            Entry::AgentMessage("It builds.".into()),
            tool_call("Run npm test", acp::ToolCallStatus::Failed),
            Entry::Plan,
        ];
        view.state.plan = vec![
            PlanItem {
                content: "Cart summary".into(),
                status: acp::PlanEntryStatus::Completed,
            },
            PlanItem {
                content: "Pay button".into(),
                status: acp::PlanEntryStatus::Pending,
            },
        ];
        let handoff = handoff(
            &view,
            projects::ThreadId(1),
            "Claude Agent",
            "Checkout \"page\"",
        );
        assert_eq!(handoff.messages, 2);
        assert!(
            handoff
                .text
                .starts_with("<agentz-handoff from=\"Claude Agent\" thread=\"Checkout 'page'\">")
        );
        assert!(handoff.text.contains(
            "<message from=\"user\">\nBuild the checkout page\n</message>\n\n<message \
             from=\"Claude Agent\">\nIt builds.\nTools: Read src/cart.tsx · Run npm test \
             (failed)\n</message>\n\n<plan>\n- [x] Cart summary\n- [ ] Pay button\n</plan>"
        ));
        // Thoughts stay with the agent that had them.
        assert!(!handoff.text.contains("wants a page"));

        // An agent may replay the first message with the handoff it brought.
        let replayed = format!("{}\n\nWire the pay button", handoff.text);
        assert_eq!(without_handoff(&replayed), "Wire the pay button");
        assert_eq!(without_handoff("Hello"), "Hello");
    }

    #[test]
    fn long_handoffs_keep_the_goal_and_the_latest() {
        let mut view = ThreadView::default();
        view.entries.push(Entry::UserMessage("The goal".into()));
        for turn in 0..40 {
            view.entries.push(Entry::UserMessage(format!(
                "Request {turn} {}",
                "x".repeat(2_000)
            )));
            view.entries.push(Entry::AgentMessage(format!(
                "Reply {turn} {}",
                "y".repeat(2_000)
            )));
        }
        let handoff = handoff(&view, projects::ThreadId(1), "Codex", "Long");
        assert_eq!(handoff.messages, 81);
        assert!(
            handoff
                .text
                .contains("<message from=\"user\">\nThe goal\n</message>")
        );
        assert!(handoff.text.contains("Reply 39"));
        assert!(!handoff.text.contains("Request 0 "));
        assert!(handoff.text.contains("messages left out"));
        assert!(handoff.text.len() < 45_000);
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
    fn login_codes() {
        assert_eq!(
            login_code(
                "2. Enter this one-time code (expires in 15 minutes)   \u{1b}[94mQXJ7-4KDM\u{1b}[0m"
            )
            .as_deref(),
            Some("QXJ7-4KDM")
        );
        assert_eq!(
            login_code("First copy your one-time code: WDJB-MJHT.").as_deref(),
            Some("WDJB-MJHT")
        );
        // Words shaped like codes, without talk of a code, and words that only look close.
        assert_eq!(login_code("Logging in to ABCD-EFGH"), None);
        assert_eq!(login_code("The code is UTF-8 encoded"), None);
        assert_eq!(
            login_code("code at https://auth.openai.com/codex/device"),
            None
        );
        assert_eq!(login_code("Enter the code abcd-efgh"), None);
    }

    #[test]
    fn thread_updates_carry_only_what_changed() {
        let mut client = ThreadView::default();
        let mut server = ThreadView::default();
        assert_eq!(server.changes_since(&client), None);

        server.entries.push(Entry::UserMessage("hello".into()));
        server.entries.push(Entry::AgentMessage("Ech".into()));
        server.state.turn_started_at = Some(SystemTime::UNIX_EPOCH);
        let update = server.changes_since(&client).expect("changes");
        assert!(update.state.is_some());
        assert_eq!(update.entries.len(), 2);
        let previous = server.clone();
        client.apply(update);
        assert_eq!(client, server);

        server.entries[1] = Entry::AgentMessage("Echo".into());
        let update = server.changes_since(&previous).expect("changes");
        assert_eq!(update.state, None);
        assert_eq!(
            update.entries,
            vec![(1, Entry::AgentMessage("Echo".into()))]
        );
        client.apply(update);
        assert_eq!(client, server);

        let previous = server.clone();
        server.entries.clear();
        let update = server.changes_since(&previous).expect("changes");
        assert_eq!(update.entry_count, 0);
        client.apply(update);
        assert_eq!(client, server);
    }

    /// The shapes Codex, Claude Agent and Antigravity read.
    #[test]
    fn logins_take_what_their_meta_asks_for() {
        let method = |meta: serde_json::Value| {
            acp::AuthMethod::Agent(
                acp::AuthMethodAgent::new("id", "Name")
                    .meta(serde_json::from_value::<acp::Meta>(meta).expect("meta is an object")),
            )
        };
        let api_key = method(serde_json::json!({"api-key": {"provider": "openai"}}));
        assert_eq!(login_input(&api_key), LoginInput::ApiKey);
        let gateway = method(serde_json::json!({"gateway": {"protocol": "anthropic"}}));
        assert_eq!(login_input(&gateway), LoginInput::Gateway);
        let browser = acp::AuthMethod::Agent(acp::AuthMethodAgent::new("chat-gpt", "ChatGPT"));
        assert_eq!(login_input(&browser), LoginInput::Nothing);

        assert_eq!(
            serde_json::Value::Object(api_key_meta("sk-1")),
            serde_json::json!({"api-key": {"apiKey": "sk-1"}})
        );
        assert_eq!(
            serde_json::Value::Object(gateway_meta(
                "https://gateway.example.com",
                &[("Authorization".into(), "Bearer 1".into())]
            )),
            serde_json::json!({"gateway": {
                "baseUrl": "https://gateway.example.com",
                "headers": {"Authorization": "Bearer 1"}
            }})
        );
    }

    #[test]
    fn terminal_logins_run_the_agent_or_the_command_in_its_meta() {
        let agent = AgentCommand {
            path: PathBuf::from("/opt/agents/node"),
            args: vec!["agent.js".into()],
            env: [("KEY".to_string(), "agent".to_string())]
                .into_iter()
                .collect(),
        };
        let terminal = acp::AuthMethod::Terminal(
            acp::AuthMethodTerminal::new("claude-ai-login", "Claude subscription")
                .args(vec!["--cli".into(), "auth".into(), "login".into()])
                .env(
                    [("MODE".to_string(), "login".to_string())]
                        .into_iter()
                        .collect(),
                ),
        );
        assert!(logs_in_through_terminal(&terminal));
        let command = terminal_login_command(&agent, &terminal).expect("a command");
        assert_eq!(command.path, agent.path);
        assert_eq!(command.args, ["agent.js", "--cli", "auth", "login"]);
        assert_eq!(command.env["MODE"], "login");
        assert_eq!(command.env["KEY"], "agent");

        let meta = |command: &str| {
            acp::AuthMethod::Agent(acp::AuthMethodAgent::new("login", "Log In").meta(
                acp::Meta::from_iter([(
                    "terminal-auth".to_string(),
                    serde_json::json!({"label": "Log In", "command": command, "args": ["auth", "login"]}),
                )]),
            ))
        };
        let command = terminal_login_command(&agent, &meta("node")).expect("a command");
        assert_eq!(command.path, agent.path);
        assert_eq!(command.args, ["auth", "login"]);
        let command = terminal_login_command(&agent, &meta("opencode")).expect("a command");
        assert_eq!(command.path, PathBuf::from("opencode"));
        assert!(logs_in_through_terminal(&meta("opencode")));

        let browser = acp::AuthMethod::Agent(acp::AuthMethodAgent::new("chatgpt", "ChatGPT"));
        assert!(!logs_in_through_terminal(&browser));
        assert_eq!(terminal_login_command(&agent, &browser), None);
    }
}
