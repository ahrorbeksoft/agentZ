//! What each role of delegated task runs on (`Request::SetSubthreadRoles`): the user's picks of
//! agent, accounts, model and effort. Of every account the role's picks may use, the one with
//! the most usage to spare runs the task, or, with the user's switch off, the first in order
//! with usage left. The setting is the app's; the server keeps its own copy so tasks delegated
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

/// One of a pick's accounts that can run a task now.
struct UsableAccount {
    /// `None` is the External account.
    account: Option<AccountId>,
    spare: Option<f64>,
    /// The agent's account for new threads.
    is_default: bool,
}

impl Server {
    pub(super) fn set_subthread_roles(&mut self, roles: SubthreadRoles) -> Result<()> {
        if roles == self.subthread_roles {
            return Ok(());
        }
        write_json(&path(&self.data_dir), &roles)?;
        self.subthread_roles = roles;
        Ok(())
    }

    /// The pick and account a role's task runs on. Every pick's accounts that can run it count:
    /// installed here, logged in, not used up, offering the pick's model, and among the pick's
    /// accounts. Of those, the one with the most usage to spare ([`spare_per_hour`]), ties
    /// going to the earlier pick, then the account for new threads; with
    /// [`SubthreadRoles::by_usage`] off, the first in order. `None` when the role has no
    /// picks, so the delegating agent chooses.
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
        let mut candidates = Vec::new();
        for (place, pick) in picks.iter().enumerate() {
            let name = self.agent_name(&pick.agent).to_string();
            if !matches!(
                registry.install_state(&pick.agent),
                InstallState::Installed { .. }
            ) {
                reasons.push(format!("{name} isn't installed on this machine."));
                continue;
            }
            match self.usable_accounts(pick, now) {
                Ok(accounts) => {
                    candidates.extend(accounts.into_iter().map(|account| (place, account)))
                }
                Err(reason) => reasons.push(format!("{name}: {reason}")),
            }
        }
        if self.subthread_roles.by_usage {
            // Known spare first, the most first; then the order they're listed in, with the
            // agent's account for new threads first among its own.
            candidates.sort_by(|(a_place, a), (b_place, b)| {
                b.spare
                    .is_some()
                    .cmp(&a.spare.is_some())
                    .then(b.spare.unwrap_or(0.).total_cmp(&a.spare.unwrap_or(0.)))
                    .then(a_place.cmp(b_place))
                    .then(b.is_default.cmp(&a.is_default))
            });
        }
        match candidates.first() {
            Some((place, usable)) => Some(Ok(RoleChoice {
                pick: picks[*place].clone(),
                account: usable.account,
            })),
            None => Some(Err(RoleUnavailable(reasons))),
        }
    }

    /// The pick's accounts that can run its task, in the order the agent lists them, or why
    /// none can.
    fn usable_accounts(
        &self,
        pick: &RolePick,
        now: SystemTime,
    ) -> Result<Vec<UsableAccount>, String> {
        let agent = &pick.agent;
        let model = pick.model.as_deref();
        let accounts = self.accounts.get(agent);
        let default = accounts.new_thread_account();
        let considered: Vec<Option<AccountId>> = accounts
            .listed()
            .into_iter()
            .filter(|account| {
                pick.accounts
                    .as_ref()
                    .is_none_or(|chosen| chosen.contains(account))
            })
            .collect();
        let mut usable = Vec::new();
        let mut used_up = 0;
        let mut without_model = 0;
        for account in considered.iter().copied() {
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
            usable.push(UsableAccount {
                account,
                spare: spare_per_hour(windows, now),
                is_default: account == default,
            });
        }
        if !usable.is_empty() {
            return Ok(usable);
        }
        let which = if pick.accounts.is_some() {
            "of the accounts chosen for it"
        } else {
            "account"
        };
        Err(if considered.is_empty() {
            "the accounts chosen for it are gone.".to_string()
        } else if used_up > 0 && used_up + without_model == considered.len() {
            format!("every {which} is used up until its limit resets.")
        } else if without_model > 0 {
            format!("no logged-in {which} offers {}.", model.unwrap_or_default())
        } else {
            format!("no {which} is logged in.")
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
