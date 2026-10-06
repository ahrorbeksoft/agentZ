//! Adding, removing and changing an agent's accounts, the account a new thread runs on, and
//! each account's settings.

use agentz_protocol::accounts::{AccountChoice, AccountId, AgentAccounts};
use agentz_protocol::agents::{AgentId, AgentSettings};
use agentz_protocol::{Request, Response};
use anyhow::{Context as _, Result, anyhow};

use super::Server;
use crate::accounts;

impl Server {
    pub(super) fn account_request(&mut self, request: Request) -> Result<Response> {
        match request {
            Request::AddAccount(agent_id) => {
                // Every account gets a folder, named after its agent.
                accounts::agent_folder(&self.data_dir, &agent_id)?;
                let id = self.accounts.update(&agent_id, AgentAccounts::add);
                Ok(Response::AccountAdded(id))
            }
            Request::RemoveAccount { agent_id, account } => {
                self.accounts
                    .get(&agent_id)
                    .account(account)
                    .context("there's no such account")?;
                // The folder first: if it can't go, the account stays to try again.
                accounts::remove_home(&self.data_dir, &agent_id, account)?;
                self.accounts
                    .update(&agent_id, |accounts| accounts.remove(account))?;
                Ok(Response::Ok)
            }
            Request::UpdateAccount {
                agent_id,
                account,
                change,
            } => {
                self.accounts
                    .update(&agent_id, |accounts| accounts.change(account, change))?;
                Ok(Response::Ok)
            }
            request => Err(anyhow!("not an account request: {request:?}")),
        }
    }

    /// The account a new thread with `agent_id` runs on, `None` being the External one.
    pub(super) fn choose_account(
        &self,
        agent_id: &AgentId,
        choice: AccountChoice,
    ) -> Result<Option<AccountId>> {
        self.accounts.get(agent_id).choose(choice)
    }

    /// The account's settings: the agent's for the External account (`None`), which keeps what
    /// was set before accounts. A removed account has none.
    pub(super) fn account_settings(
        &self,
        agent_id: &AgentId,
        account: Option<AccountId>,
    ) -> AgentSettings {
        match account {
            None => self.agent_settings.get(agent_id),
            Some(id) => self
                .accounts
                .get(agent_id)
                .account(id)
                .map(|account| account.settings.clone())
                .unwrap_or_default(),
        }
    }

    /// The settings of the account a new thread with `agent_id` runs on.
    pub(super) fn new_thread_settings(&self, agent_id: &AgentId) -> AgentSettings {
        let account = self.accounts.get(agent_id).new_thread_account();
        self.account_settings(agent_id, account)
    }

    /// Changes the account's settings, if it's still there.
    pub(super) fn update_account_settings(
        &mut self,
        agent_id: &AgentId,
        account: Option<AccountId>,
        change: impl FnOnce(&mut AgentSettings),
    ) {
        match account {
            None => self.agent_settings.update(agent_id, change),
            Some(id) => self.accounts.update(agent_id, |accounts| {
                if let Some(account) = accounts.account_mut(id) {
                    change(&mut account.settings);
                }
            }),
        }
    }
}
