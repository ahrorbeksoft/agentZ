//! Adding, removing and changing an agent's accounts, the account a new thread runs on, each
//! account's settings, and the home its agent runs in.

use std::path::PathBuf;

use agentz_protocol::accounts::{AccountChoice, AccountId, AgentAccounts};
use agentz_protocol::agents::{AgentId, AgentSettings};
use agentz_protocol::{ConnectionId, Request, Response};
use anyhow::{Context as _, Result, anyhow, bail};
use projects::ThreadId;
use util::ResultExt as _;

use super::Server;
use crate::accounts::{self, AgentDescription};

impl Server {
    pub(super) fn account_request(&mut self, request: Request) -> Result<Response> {
        match request {
            Request::AddAccount(agent_id) => {
                if self.account_description(&agent_id).is_none() {
                    bail!(
                        "{} can't have more than one account",
                        self.agent_name(&agent_id)
                    );
                }
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
                // They write into its folder.
                let stopped = self.stop_account_agents(&agent_id, account);
                // The folder first: if it can't go, the account stays to try again.
                accounts::remove_home(&self.data_dir, &agent_id, account)?;
                self.accounts
                    .update(&agent_id, |accounts| accounts.remove(account))?;
                // Threads that are open start again, to say why they can't go on.
                let watched = self.watched_threads();
                for thread_id in stopped {
                    if watched.contains(&thread_id) {
                        self.update_thread(ConnectionId::Thread(thread_id), |_| {})
                            .log_err();
                    }
                }
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
            Request::RefreshUsage { agent_id, account } => {
                if let Some(id) = account {
                    self.accounts
                        .get(&agent_id)
                        .account(id)
                        .context("there's no such account")?;
                }
                let reader = self
                    .account_description(&agent_id)
                    .and_then(|description| description.reader)
                    .with_context(|| {
                        format!("agentZ can't read {}'s usage", self.agent_name(&agent_id))
                    })?;
                self.read_account(&agent_id, account, reader);
                Ok(Response::Ok)
            }
            request => Err(anyhow!("not an account request: {request:?}")),
        }
    }

    /// How the agent keeps an account in a folder of agentZ's: a custom agent's from
    /// `custom.json`, else agentZ's own. Without one, the agent has only the External account.
    pub(super) fn account_description(&self, agent_id: &AgentId) -> Option<AgentDescription> {
        match self.custom_agents.get(agent_id) {
            Some(agent) => agent.accounts.clone(),
            None => accounts::built_in_description(&agent_id.0),
        }
    }

    /// An agentZ account's description and home, while the account is there.
    pub(super) fn account_home(
        &self,
        agent_id: &AgentId,
        account: AccountId,
    ) -> Result<(AgentDescription, PathBuf)> {
        self.accounts
            .get(agent_id)
            .account(account)
            .context("The account was removed.")?;
        let description = self.account_description(agent_id).with_context(|| {
            format!(
                "{} can't have more than one account",
                self.agent_name(agent_id)
            )
        })?;
        Ok((
            description,
            accounts::home(&self.data_dir, agent_id, account)?,
        ))
    }

    /// Stops the agents running on the account: its threads' and its login sessions'. Returns
    /// the threads.
    fn stop_account_agents(&mut self, agent_id: &AgentId, account: AccountId) -> Vec<ThreadId> {
        let on_account = |thread: &projects::Thread| {
            thread.agent_id.as_deref() == Some(&*agent_id.0) && thread.account == Some(account)
        };
        let stopped: Vec<ThreadId> = self
            .threads
            .keys()
            .copied()
            .filter(|thread_id| self.projects.thread(*thread_id).is_some_and(on_account))
            .collect();
        for thread_id in &stopped {
            self.stop_agent(*thread_id);
        }
        self.login_sessions.retain(|_, login_session| {
            login_session.agent_id != *agent_id || login_session.account != Some(account)
        });
        stopped
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
