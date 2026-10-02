//! Agents: the registry's listing, what's installed, how to start one, and each agent's
//! settings.

use std::collections::BTreeMap;
use std::path::PathBuf;

use agent_client_protocol::schema::v1 as acp;
use collections::HashMap;
use gpui_shared_string::SharedString;
use serde::{Deserialize, Serialize};

use crate::thread::SessionDefaults;

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
    /// Absolute path of the cached SVG icon.
    #[serde(default)]
    pub icon_path: Option<SharedString>,
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

    pub fn icon_path(&self) -> Option<&SharedString> {
        self.metadata.icon_path.as_ref()
    }

    pub fn supports_current_platform(&self) -> bool {
        self.supports_current_platform
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
    /// The login method last used from agentZ, to say how the agent is logged in. ACP has no way
    /// to ask the agent.
    pub login_method: Option<String>,
}

impl AgentSettings {
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
