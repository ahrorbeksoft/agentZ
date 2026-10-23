//! What each kind of delegated task runs on (Settings › Subthreads): the four roles
//! agents delegate by, each with the user's picks of agent, accounts, model and effort in the
//! order they were added. Agents see the picks (`orchestrator_capabilities`) and name an agent
//! or model only when a role has none, or when the user asked for other agents. Of the picks'
//! accounts, the one with the most usage to spare runs the task
//! ([`crate::accounts::spare_per_hour`]), or with [`SubthreadRoles::by_usage`] off, the first
//! in order with usage left.

use serde::{Deserialize, Serialize};

use crate::accounts::AccountId;
use crate::agents::AgentId;

/// The kind of work a delegated task is. A task the parent gives no role is [`Self::Other`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TaskRole {
    Researcher,
    Reviewer,
    Worker,
    #[default]
    Other,
}

impl TaskRole {
    pub const ALL: [TaskRole; 4] = [
        TaskRole::Researcher,
        TaskRole::Reviewer,
        TaskRole::Worker,
        TaskRole::Other,
    ];

    /// As agents pass it (`role`), and as tasks keep it.
    pub fn id(self) -> &'static str {
        match self {
            TaskRole::Researcher => "researcher",
            TaskRole::Reviewer => "reviewer",
            TaskRole::Worker => "worker",
            TaskRole::Other => "other",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|role| role.id() == id)
    }
}

/// The user's picks for each role, in the order they were added. An empty list leaves the role
/// to the agent that delegates.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SubthreadRoles {
    pub researcher: Vec<RolePick>,
    pub reviewer: Vec<RolePick>,
    pub worker: Vec<RolePick>,
    pub other: Vec<RolePick>,
    /// Of every account a role's picks may use, the one with the most usage to spare runs its
    /// task. Off, the picks and their accounts are tried in order, and the first with usage
    /// left runs it.
    pub by_usage: bool,
}

impl Default for SubthreadRoles {
    fn default() -> Self {
        Self {
            researcher: Vec::new(),
            reviewer: Vec::new(),
            worker: Vec::new(),
            other: Vec::new(),
            by_usage: true,
        }
    }
}

impl SubthreadRoles {
    pub fn picks(&self, role: TaskRole) -> &[RolePick] {
        match role {
            TaskRole::Researcher => &self.researcher,
            TaskRole::Reviewer => &self.reviewer,
            TaskRole::Worker => &self.worker,
            TaskRole::Other => &self.other,
        }
    }

    pub fn picks_mut(&mut self, role: TaskRole) -> &mut Vec<RolePick> {
        match role {
            TaskRole::Researcher => &mut self.researcher,
            TaskRole::Reviewer => &mut self.reviewer,
            TaskRole::Worker => &mut self.worker,
            TaskRole::Other => &mut self.other,
        }
    }
}

/// One choice for a role: an agent, which of its accounts, and the model and effort to run it
/// with.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RolePick {
    pub agent: AgentId,
    /// The accounts it may run on, `None` in the list being the External account; `None` is
    /// all of them. They're tried in the order the agent lists them. Accounts are each
    /// machine's, so only This Mac's server is sent the user's choice; others use all of theirs.
    #[serde(default)]
    pub accounts: Option<Vec<Option<AccountId>>>,
    /// The model's value, as the agent's model setting names it. `None` is the account's
    /// default model.
    #[serde(default)]
    pub model: Option<String>,
    /// The effort's value, as the agent's effort setting (ACP's thought level) names it.
    /// `None` is the model's default.
    #[serde(default)]
    pub effort: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roles_go_by_their_ids() {
        for role in TaskRole::ALL {
            assert_eq!(TaskRole::from_id(role.id()), Some(role));
            assert_eq!(
                serde_json::to_value(role).ok(),
                Some(serde_json::json!(role.id()))
            );
        }
        assert_eq!(TaskRole::from_id("research"), None);
    }
}
