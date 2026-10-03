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
            .find_map(|method| match method {
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
}
