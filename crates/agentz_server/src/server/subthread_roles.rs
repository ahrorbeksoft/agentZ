//! What each role of delegated task runs on (`Request::SetSubthreadRoles`): the user's picks of
//! agent, model and effort, tried in order, and within a pick's agent the account with the most
//! usage to spare. The setting is the app's; the server keeps its own copy so tasks delegated
//! while the app is closed keep to it.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use agentz_protocol::accounts::{AccountId, spare_per_hour, used_up_window};
use agentz_protocol::agents::{AgentId, InstallState};
use agentz_protocol::subthread_roles::{RolePick, SubthreadRoles, TaskRole};
use anyhow::Result;

use super::Server;
use crate::agent_settings::{read_json, write_json};

fn path(data_dir: &Path) -> PathBuf {
    data_dir.join("subthread-roles.json")
}

pub(crate) fn load(data_dir: &Path) -> Result<SubthreadRoles> {
    Ok(read_json::<SubthreadRoles>(&path(data_dir))?.unwrap_or_default())
}

/// The pick a role's task runs on, with the account chosen for it.
pub(super) struct RoleChoice {
    pub pick: RolePick,
    /// `None` is the External account.
    pub account: Option<AccountId>,
}

/// Why none of a role's picks could run its task, one line each, for the agent that delegated.
pub(super) struct RoleUnavailable(pub Vec<String>);

impl Server {
    pub(super) fn set_subthread_roles(&mut self, roles: SubthreadRoles) -> Result<()> {
        if roles == self.subthread_roles {
            return Ok(());
        }
        write_json(&path(&self.data_dir), &roles)?;
        self.subthread_roles = roles;
        Ok(())
    }

    /// The first of the role's picks with an account to run it: installed here, logged in,
    /// not used up, and offering the pick's model. Of a pick's agent's accounts, the one with
    /// the most usage to spare ([`spare_per_hour`]), then the one for new threads. `None` when
    /// the role has no picks, so the delegating agent chooses.
    pub(super) fn choose_for_role(
        &self,
        role: TaskRole,
    ) -> Option<Result<RoleChoice, RoleUnavailable>> {
        let picks = self.subthread_roles.picks(role);
        if picks.is_empty() {
            return None;
        }
        let now = SystemTime::now();
        let registry = self.registry_snapshot();
        let mut reasons = Vec::new();
        for pick in picks {
            let name = self.agent_name(&pick.agent).to_string();
            if !matches!(
                registry.install_state(&pick.agent),
                InstallState::Installed { .. }
            ) {
                reasons.push(format!("{name} isn't installed on this machine."));
                continue;
            }
            match self.account_to_spare(&pick.agent, pick.model.as_deref(), now) {
                Ok(account) => {
                    return Some(Ok(RoleChoice {
                        pick: pick.clone(),
                        account,
                    }));
                }
                Err(reason) => reasons.push(format!("{name}: {reason}")),
            }
        }
        Some(Err(RoleUnavailable(reasons)))
    }

    /// The agent's account with the most usage to spare that can run `model`, or why none
    /// can.
    fn account_to_spare(
        &self,
        agent: &AgentId,
        model: Option<&str>,
        now: SystemTime,
    ) -> Result<Option<AccountId>, String> {
        let accounts = self.accounts.get(agent);
        let default = accounts.new_thread_account();
        let listed = accounts.listed();
        let mut candidates = Vec::new();
        let mut used_up = 0;
        let mut without_model = 0;
        for (order, account) in listed.iter().copied().enumerate() {
            if accounts.logged_in(account) == Some(false) {
                continue;
            }
            let windows = accounts
                .status(account)
                .map(|read| read.status.windows.as_slice())
                .unwrap_or_default();
            if used_up_window(windows, now).is_some() {
                used_up += 1;
                continue;
            }
            if let Some(model) = model
                && !self.account_offers_model(agent, account, model)
            {
                without_model += 1;
                continue;
            }
            let spare = spare_per_hour(windows, now);
            candidates.push((spare, account == default, order, account));
        }
        // Known spare first, the most first; then the account for new threads, then the order
        // they're listed in.
        candidates.sort_by(|a, b| {
            b.0.is_some()
                .cmp(&a.0.is_some())
                .then(b.0.unwrap_or(0.).total_cmp(&a.0.unwrap_or(0.)))
                .then(b.1.cmp(&a.1))
                .then(a.2.cmp(&b.2))
        });
        if let Some((_, _, _, account)) = candidates.first() {
            return Ok(*account);
        }
        Err(if used_up > 0 && used_up + without_model == listed.len() {
            "every account is used up until its limit resets.".to_string()
        } else if without_model > 0 {
            format!("no logged-in account offers {}.", model.unwrap_or_default())
        } else {
            "no account is logged in.".to_string()
        })
    }

    /// Whether the account offers the model, as far as it has said: one that hasn't reported
    /// its models yet is given the benefit of the doubt.
    fn account_offers_model(
        &self,
        agent: &AgentId,
        account: Option<AccountId>,
        model: &str,
    ) -> bool {
        let settings = self.account_settings(agent, account);
        match super::tools::model_choices(&settings.known_config_options) {
            Some((_, choices, _)) => choices.iter().any(|(id, name)| {
                id.eq_ignore_ascii_case(model) || name.eq_ignore_ascii_case(model)
            }),
            None => true,
        }
    }
}
