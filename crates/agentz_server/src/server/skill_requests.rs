//! Adding and deleting agentZ's skills, and keeping their links in every account's skills
//! folder ([`crate::skills`]).

use std::collections::BTreeSet;

use agentz_protocol::agents::{AgentId, InstallState};
use agentz_protocol::{Request, Response};
use anyhow::{Result, anyhow};
use util::ResultExt as _;

use super::Server;
use crate::accounts;
use crate::skills::{self, SkillTarget};

impl Server {
    pub(super) fn skill_request(&mut self, request: Request) -> Result<Response> {
        let folder = skills::folder(&self.data_dir);
        match request {
            Request::AddSkill(files) => {
                skills::add(&folder, &files)?;
            }
            Request::CreateSkill {
                name,
                description,
                body,
            } => skills::create(&folder, &name, &description, &body)?,
            Request::DeleteSkill(name) => skills::delete(&folder, &name)?,
            request => return Err(anyhow!("not a skill request: {request:?}")),
        }
        self.list_skills();
        Ok(Response::Ok)
    }

    /// Reads the skills folder again. Their links, and who skips each, follow with the next
    /// changes sent.
    pub(super) fn list_skills(&mut self) {
        self.skills = skills::list(&skills::folder(&self.data_dir))
            .log_err()
            .unwrap_or_default();
        self.skills_synced = false;
    }

    /// Links the skills into every account, when they or the accounts that load them changed
    /// since the last sync. Not before the registry knows which agents are installed: until
    /// then, every one reads as not installed, and its links would be removed.
    pub(super) fn sync_skills(&mut self, registry_changed: bool) {
        if !self.registry.knows_installed()
            || (self.skills_synced
                && !registry_changed
                && self.accounts.revision() == self.skill_accounts_revision)
        {
            return;
        }
        self.skill_accounts_revision = self.accounts.revision();
        let targets = self.skill_targets();
        if self.skills_synced && targets == self.skill_targets_synced {
            return;
        }
        let folder = skills::folder(&self.data_dir);
        let listed = skills::list(&folder).log_err().unwrap_or_default();
        self.skills = skills::sync(&folder, listed, &targets);
        self.skill_targets_synced = targets;
        self.skills_synced = true;
    }

    /// Every account of the agents agentZ can run on accounts, External ones included. Those of
    /// agents that aren't installed, and External accounts that aren't listed, don't load the
    /// skills.
    fn skill_targets(&self) -> Vec<SkillTarget> {
        let agent_ids: BTreeSet<AgentId> = self
            .custom_agents
            .keys()
            .cloned()
            .chain(accounts::BUILT_IN_DESCRIPTIONS.map(AgentId::new))
            .collect();
        let mut targets = Vec::new();
        for agent_id in agent_ids {
            let Some(description) = self.account_description(&agent_id) else {
                continue;
            };
            let installed = self.custom_agents.contains_key(&agent_id)
                || matches!(
                    self.registry.install_state(&agent_id),
                    InstallState::Installed { .. } | InstallState::Installing
                );
            let agent_accounts = self.accounts.get(&agent_id);
            let accounts = std::iter::once((None, agent_accounts.lists_external())).chain(
                agent_accounts
                    .accounts
                    .iter()
                    .map(|account| (Some(account.id), true)),
            );
            for (account, listed) in accounts {
                let folders = account
                    .map(|id| accounts::home(&self.data_dir, &agent_id, id))
                    .transpose()
                    .and_then(|home| description.skill_folders(home.as_deref()));
                match folders {
                    Ok(Some((folder, other_folders))) => targets.push(SkillTarget {
                        agent_id: agent_id.clone(),
                        account,
                        folder,
                        other_folders,
                        loads: installed && listed,
                    }),
                    Ok(None) => {}
                    Err(error) => log::error!("finding {agent_id}'s skills folders: {error:#}"),
                }
            }
        }
        targets
    }
}
