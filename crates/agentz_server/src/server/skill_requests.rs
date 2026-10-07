//! Adding and deleting agentZ's skills, and keeping their links in every account's skills
//! folder ([`crate::skills`]).

use std::collections::BTreeSet;

use agentz_protocol::agents::{AgentId, InstallState};
use agentz_protocol::skills::Skill;
use agentz_protocol::{Request, Response};
use anyhow::{Result, anyhow};
use util::ResultExt as _;

use super::Server;
use crate::accounts;
use crate::skills::{self, SkillTarget};

impl Server {
    pub(super) fn skill_request(&mut self, request: Request) -> Result<Response> {
        let folder = skills::folder(&self.data_dir);
        // A new skill loads on every account, whatever was chosen for one of its name before.
        let forgotten = match request {
            Request::AddSkill(files) => skills::add(&folder, &files)?,
            Request::CreateSkill {
                name,
                description,
                body,
            } => {
                skills::create(&folder, &name, &description, &body)?;
                name
            }
            Request::DeleteSkill(name) => {
                skills::delete(&folder, &name)?;
                name
            }
            Request::SetSkillKeptOff { name, kept_off } => {
                anyhow::ensure!(
                    self.skills.iter().any(|skill| skill.name == name),
                    "There's no skill named \"{name}\"."
                );
                skills::set_kept_off(&self.data_dir, &name, kept_off)?;
                self.list_skills();
                return Ok(Response::Ok);
            }
            request => return Err(anyhow!("not a skill request: {request:?}")),
        };
        skills::set_kept_off(&self.data_dir, &forgotten, Vec::new()).log_err();
        self.list_skills();
        Ok(Response::Ok)
    }

    /// Reads the skills folder again. Their links, and who skips each, follow with the next
    /// changes sent.
    pub(super) fn list_skills(&mut self) {
        self.skills = self.listed_skills();
        self.skills_synced = false;
    }

    /// The skills in the folder, with the accounts each is kept off.
    fn listed_skills(&self) -> Vec<Skill> {
        let mut listed = skills::list(&skills::folder(&self.data_dir))
            .log_err()
            .unwrap_or_default();
        let mut choices = skills::load_choices(&self.data_dir)
            .log_err()
            .unwrap_or_default();
        for skill in &mut listed {
            if let Some(choices) = choices.remove(&skill.name) {
                skill.kept_off = choices.kept_off;
            }
        }
        listed
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
        let listed = self.listed_skills();
        self.skills = skills::sync(&skills::folder(&self.data_dir), listed, &targets);
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
