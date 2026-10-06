//! Adding, removing and changing an agent's accounts, and the account a new thread runs on.

use agentz_protocol::accounts::{AccountChoice, AccountId, AgentAccounts};
use agentz_protocol::agents::AgentId;
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
}
