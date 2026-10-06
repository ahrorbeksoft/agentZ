//! Custom agents: run from a command rather than the ACP Registry, and added or changed from
//! Settings › Agents as with Zed's Add Custom Agent. They live in `agents/custom.json`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use agent_client_protocol::schema::v1 as acp;
use agentz_protocol::Response;
use agentz_protocol::agents::{AgentCommand, AgentId, CustomAgentChange, IconId, InstallState};
use anyhow::{Context as _, Result, anyhow, bail};

use super::{ClientId, Server};
use crate::CustomAgent;

impl Server {
    /// Starts the agent once, to check that it runs and to learn what it calls itself, then
    /// keeps it.
    pub(super) fn save_custom_agent(
        &mut self,
        client: ClientId,
        id: u64,
        change: CustomAgentChange,
    ) {
        let CustomAgentChange {
            agent_id,
            name,
            mut command,
        } = change;
        if let Some(agent_id) = &agent_id
            && !self.custom_agents.contains_key(agent_id)
        {
            let error = anyhow!("there's no custom agent {agent_id}");
            return self.respond(client, id, Err(error));
        }
        if command.path.as_os_str().is_empty() {
            return self.respond(
                client,
                id,
                Err(anyhow!("Enter the command that starts it.")),
            );
        }
        command.path = expand_home(&command.path);
        let probe = command.clone();
        // The environment is the agent's settings', which its Environment tab edits too.
        let env: BTreeMap<String, String> = std::mem::take(&mut command.env).into_iter().collect();
        self.spawn_then(agent_thread::agent_info(probe), move |server, info| {
            let saved = info
                .context("It didn't start")
                .and_then(|info| server.keep_custom_agent(agent_id, &name, command, env, info));
            server.respond(client, id, saved.map(Response::CustomAgentSaved));
        });
    }

    fn keep_custom_agent(
        &mut self,
        agent_id: Option<AgentId>,
        name: &str,
        command: AgentCommand,
        env: BTreeMap<String, String>,
        info: Option<acp::Implementation>,
    ) -> Result<AgentId> {
        let name = match name.trim() {
            "" => info
                .as_ref()
                .map(|info| {
                    info.title
                        .as_deref()
                        .filter(|title| !title.trim().is_empty())
                        .unwrap_or(&info.name)
                        .trim()
                        .to_string()
                })
                .filter(|name| !name.is_empty())
                .context("The agent doesn't say its name. Give it one.")?,
            name => name.to_string(),
        };
        // Pickers would show two agents by the same name and icon.
        let name_is_taken = self.registry_snapshot().agents.iter().any(|agent| {
            Some(agent.id()) != agent_id.as_ref()
                && matches!(
                    agent.install_state,
                    InstallState::Installed { .. } | InstallState::Installing
                )
                && agent.name().eq_ignore_ascii_case(&name)
        });
        if name_is_taken {
            bail!("An installed agent is already called {name}. Give this one another name.");
        }
        // It may have been removed while it started.
        if let Some(agent_id) = &agent_id
            && !self.custom_agents.contains_key(agent_id)
        {
            bail!("there's no custom agent {agent_id}");
        }
        let agent_id = agent_id.unwrap_or_else(|| self.new_custom_agent_id(&name));
        let mut agents = self.custom_agents.clone();
        // Settings doesn't edit it, so it's kept from `custom.json`.
        let accounts = agents
            .get(&agent_id)
            .and_then(|agent| agent.accounts.clone());
        agents.insert(
            agent_id.clone(),
            CustomAgent {
                name: name.into(),
                command,
                info,
                accounts,
            },
        );
        write_custom_agents(&self.data_dir, &agents)?;
        self.custom_agents = agents;
        self.agent_settings
            .update(&agent_id, |settings| settings.env = env);
        self.registry_changed = true;
        Ok(agent_id)
    }

    pub(super) fn remove_custom_agent(&mut self, agent_id: &AgentId) -> Result<Response> {
        let mut agents = self.custom_agents.clone();
        agents
            .remove(agent_id)
            .with_context(|| format!("there's no custom agent {agent_id}"))?;
        write_custom_agents(&self.data_dir, &agents)?;
        self.custom_agents = agents;
        self.registry_changed = true;
        Ok(Response::Ok)
    }

    /// Fixed once it's added, since threads keep their agent's id. Never one of the registry's.
    fn new_custom_agent_id(&self, name: &str) -> AgentId {
        let slug: String = name
            .to_lowercase()
            .split(|character: char| !character.is_ascii_alphanumeric())
            .filter(|word| !word.is_empty())
            .collect::<Vec<_>>()
            .join("-");
        let base = if slug.is_empty() {
            "custom-agent".to_string()
        } else {
            format!("custom-{slug}")
        };
        let is_taken =
            |id: &AgentId| self.custom_agents.contains_key(id) || self.registry.agent(id).is_some();
        let mut candidate = AgentId::new(base.clone());
        let mut number = 2;
        while is_taken(&candidate) {
            candidate = AgentId::new(format!("{base}-{number}"));
            number += 1;
        }
        candidate
    }

    /// ACP gives agents no icon, so a custom agent shows the registry's icon for the agent it
    /// says it is, as OpenCode run from its own install does.
    pub(super) fn custom_agent_icon(&self, agent: &CustomAgent) -> Option<IconId> {
        let info = agent.info.as_ref()?;
        let names: Vec<&str> = [Some(info.name.as_str()), info.title.as_deref()]
            .into_iter()
            .flatten()
            .map(str::trim)
            .filter(|name| !name.is_empty())
            .collect();
        self.registry
            .agents()
            .iter()
            .find(|registry_agent| {
                names.iter().any(|name| {
                    name.eq_ignore_ascii_case(registry_agent.name())
                        || name.eq_ignore_ascii_case(&registry_agent.id().0)
                })
            })
            .and_then(|registry_agent| registry_agent.metadata().icon.clone())
    }
}

/// What a custom agent's listing shows for its version: the one it reported, if any.
pub(super) fn custom_agent_version(agent: &CustomAgent) -> String {
    agent
        .info
        .as_ref()
        .map(|info| info.version.trim().to_string())
        .filter(|version| !version.is_empty())
        .unwrap_or_else(|| "custom".to_string())
}

fn write_custom_agents(data_dir: &Path, agents: &BTreeMap<AgentId, CustomAgent>) -> Result<()> {
    let path = crate::custom_agents_path(data_dir);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("creating {}", parent.display()))?;
    }
    let temporary = path.with_extension("json.tmp");
    std::fs::write(&temporary, serde_json::to_vec_pretty(agents)?)
        .with_context(|| format!("writing {}", temporary.display()))?;
    std::fs::rename(&temporary, &path).with_context(|| format!("writing {}", path.display()))
}

/// People type `~/…` as in a shell, which the agent's process wouldn't expand.
fn expand_home(path: &Path) -> PathBuf {
    match path.strip_prefix("~") {
        Ok(rest) => util::paths::home_dir().join(rest),
        Err(_) => path.to_path_buf(),
    }
}
