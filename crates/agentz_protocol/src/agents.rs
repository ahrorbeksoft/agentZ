//! Agents: the registry's listing, what's installed, how to start one, each agent's settings,
//! and the sessions it keeps.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::SystemTime;

use agent_client_protocol::schema::v1 as acp;
use collections::HashMap;
use gpui_shared_string::SharedString;
use projects::{ProjectId, ThreadId};
use serde::{Deserialize, Serialize};

use crate::accounts::AccountSupport;
use crate::thread::{AuthStatus, LoginIdentity, SessionDefaults};

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AgentId(pub SharedString);

impl AgentId {
    pub fn new(id: impl Into<SharedString>) -> Self {
        AgentId(id.into())
    }
}

impl std::fmt::Display for AgentId {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}", self.0)
    }
}

/// How to start an installed agent.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct AgentCommand {
    pub path: PathBuf,
    pub args: Vec<String>,
    pub env: HashMap<String, String>,
    /// Left out of the environment the agent inherits, unless `env` sets them.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub env_remove: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RegistryAgentMetadata {
    pub id: AgentId,
    pub name: SharedString,
    pub description: SharedString,
    pub version: SharedString,
    #[serde(default)]
    pub repository: Option<SharedString>,
    #[serde(default)]
    pub website: Option<SharedString>,
    #[serde(default)]
    pub license_url: Option<SharedString>,
    /// The agent's icon, which [`crate::Request::AgentIcons`] fetches. Every machine reads the
    /// same registry, so a client fetches it once for all of them.
    #[serde(default)]
    pub icon: Option<IconId>,
}

/// An icon, by a hash of its SVG: the same on every machine that has it.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct IconId(pub SharedString);

/// An icon's SVG markup, which clients draw in the text color (the ACP Registry's icons use
/// `currentColor`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AgentIcon {
    pub id: IconId,
    pub svg: SharedString,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum InstallState {
    NotInstalled,
    Installing,
    Installed {
        version: SharedString,
        update_available: bool,
    },
    Failed(SharedString),
}

/// An agent in the registry, as clients list it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AgentListing {
    pub metadata: RegistryAgentMetadata,
    pub supports_current_platform: bool,
    pub install_state: InstallState,
    /// How a custom agent starts, to edit it. `None` for the registry's agents.
    #[serde(default)]
    pub custom_command: Option<AgentCommand>,
    /// What it offers for accounts, if it can have more than its own login.
    #[serde(default)]
    pub accounts: Option<AccountSupport>,
}

/// A custom agent as Settings › Agents adds or changes it ([`crate::Request::SaveCustomAgent`]).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CustomAgentChange {
    /// `None` adds one.
    pub agent_id: Option<AgentId>,
    /// Blank takes the name the agent gives itself.
    pub name: String,
    /// Its `env` becomes the agent's settings' environment.
    pub command: AgentCommand,
}

impl AgentListing {
    pub fn id(&self) -> &AgentId {
        &self.metadata.id
    }

    pub fn name(&self) -> &SharedString {
        &self.metadata.name
    }

    pub fn description(&self) -> &SharedString {
        &self.metadata.description
    }

    pub fn version(&self) -> &SharedString {
        &self.metadata.version
    }

    pub fn icon(&self) -> Option<&IconId> {
        self.metadata.icon.as_ref()
    }

    pub fn supports_current_platform(&self) -> bool {
        self.supports_current_platform
    }

    pub fn is_custom(&self) -> bool {
        self.custom_command.is_some()
    }
}

/// The registry as clients see it.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct RegistrySnapshot {
    pub agents: Vec<AgentListing>,
    pub is_fetching: bool,
    pub fetch_error: Option<SharedString>,
}

impl RegistrySnapshot {
    pub fn agents(&self) -> &[AgentListing] {
        &self.agents
    }

    pub fn agent(&self, id: &AgentId) -> Option<&AgentListing> {
        self.agents.iter().find(|agent| agent.id() == id)
    }

    pub fn is_fetching(&self) -> bool {
        self.is_fetching
    }

    pub fn fetch_error(&self) -> Option<SharedString> {
        self.fetch_error.clone()
    }

    pub fn install_state(&self, id: &AgentId) -> InstallState {
        self.agent(id)
            .map(|agent| agent.install_state.clone())
            .unwrap_or(InstallState::NotInstalled)
    }
}

/// What Zed keeps for each external agent: its environment and the defaults for new sessions,
/// which follow the user's last choices in a thread.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AgentSettings {
    pub env: BTreeMap<String, String>,
    pub default_mode: Option<acp::SessionModeId>,
    pub default_config_options: BTreeMap<String, acp::SessionConfigOptionValue>,
    /// The settings and modes the agent last offered, so its settings page can list them
    /// without starting a session.
    pub known_config_options: Vec<acp::SessionConfigOption>,
    pub known_modes: Option<acp::SessionModeState>,
    /// The method of the login agentZ made, while the agent still has it, to say how it's
    /// logged in. ACP has no way to ask the agent. `None` while it's logged in means it was
    /// logged in outside agentZ, as by its own CLI.
    pub login_method: Option<String>,
    /// The account the agent reported after agentZ logged it in. Another one later means it was
    /// logged in again outside agentZ.
    pub login_identity: Option<LoginIdentity>,
}

/// [`crate::Request::ListAgentSessions`]'s answer: the conversations an agent keeps on the
/// server's machine, from ACP's `session/list`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum AgentSessions {
    /// As the agent orders them.
    Listed(Vec<AgentSession>),
    /// The agent doesn't support `session/list`.
    Unsupported,
    /// The agent wants a login first.
    LoggedOut,
    /// From a newer version.
    #[serde(untagged)]
    Unknown(serde_json::Value),
}

/// One of an agent's sessions, and where it would go as a thread.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AgentSession {
    pub session_id: String,
    /// The folder it ran in, as the agent reports it.
    pub cwd: PathBuf,
    #[serde(default)]
    pub title: Option<String>,
    /// When the agent last worked on it.
    #[serde(default)]
    pub updated_at: Option<SystemTime>,
    /// The project whose folder, or one of whose worktrees or pastures, `cwd` is. Only these
    /// sessions can be imported, since a thread belongs to a project.
    #[serde(default)]
    pub project_id: Option<ProjectId>,
    /// The project's worktree or pasture `cwd` is, or `None` for the project's own folder.
    #[serde(default)]
    pub workspace: Option<PathBuf>,
    /// The thread that has it already.
    #[serde(default)]
    pub thread_id: Option<ThreadId>,
}

impl AgentSettings {
    /// agentZ logged the agent in with `method`.
    pub fn logged_in(&mut self, method: String) {
        self.login_method = Some(method);
        self.login_identity = None;
    }

    /// The agent is logged out, whoever logged it out.
    pub fn logged_out(&mut self) {
        self.login_method = None;
        self.login_identity = None;
    }

    /// Follows the account the agent reports. The first one after agentZ logged it in is the
    /// login agentZ made; a different one means it was logged in again outside agentZ.
    pub fn account_reported(&mut self, status: &AuthStatus) {
        if !status.is_logged_in() {
            self.logged_out();
            return;
        }
        if self.login_method.is_none() {
            return;
        }
        let identity = status.identity();
        match &self.login_identity {
            Some(known) if known.differs_from(&identity) => self.logged_out(),
            // Keep the one that says more.
            Some(known) if known.key.is_some() => {}
            _ => self.login_identity = Some(identity),
        }
    }

    pub fn session_defaults(&self) -> SessionDefaults {
        SessionDefaults {
            mode: self.default_mode.clone(),
            config_options: self
                .default_config_options
                .iter()
                .map(|(id, value)| (acp::SessionConfigId::new(id.clone()), value.clone()))
                .collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::thread::AuthAccount;

    fn account(email: Option<&str>) -> AuthStatus {
        AuthStatus {
            kind: "account".into(),
            label: Some("Claude Max".into()),
            account: Some(AuthAccount {
                email: email.map(str::to_string),
                ..Default::default()
            }),
            detail: None,
        }
    }

    #[test]
    fn logins_made_outside_agentz() {
        let mut settings = AgentSettings::default();
        // Logged in before agentZ ever logged it in.
        settings.account_reported(&account(Some("a@example.com")));
        assert_eq!(settings.login_method, None);

        settings.logged_in("Claude Subscription".into());
        // Claude Agent's CLI check leaves the email out, which doesn't make it another login.
        settings.account_reported(&account(None));
        settings.account_reported(&account(Some("a@example.com")));
        settings.account_reported(&account(None));
        assert_eq!(
            settings.login_method.as_deref(),
            Some("Claude Subscription")
        );

        // Logged in again as someone else, in a terminal.
        settings.account_reported(&account(Some("b@example.com")));
        assert_eq!(settings.login_method, None);

        settings.logged_in("Claude Subscription".into());
        settings.account_reported(&account(Some("b@example.com")));
        // An API key from the environment now pays instead.
        settings.account_reported(&AuthStatus {
            kind: "api_key".into(),
            detail: Some("ANTHROPIC_API_KEY".into()),
            ..Default::default()
        });
        assert_eq!(settings.login_method, None);

        settings.logged_in("Claude Subscription".into());
        settings.account_reported(&AuthStatus {
            kind: "none".into(),
            ..Default::default()
        });
        assert_eq!(settings.login_method, None);
    }
}
