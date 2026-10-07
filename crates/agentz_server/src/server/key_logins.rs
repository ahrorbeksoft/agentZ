//! API-key accounts (plan.md › API-key accounts): an agentZ account that logs in with a key its
//! agent reads from a variable (Droid's "Factory API Key"). agentZ keeps the key in the
//! account's folder and starts the account's agent with it, so the agent restarts for a login
//! with another key, or with another method, which the key would override.

use agent_client_protocol::schema::v1 as acp;
use agent_thread::AgentThread;
use agentz_protocol::ConnectionId;
use agentz_protocol::accounts::AccountId;
use agentz_protocol::agents::{AgentCommand, AgentId};
use anyhow::{Context as _, Result};
use util::ResultExt as _;

use super::Server;
use crate::accounts::{self, KeyLogin};

impl Server {
    /// The agent's key login, on an agentZ account (`Some`) of an agent that has one.
    pub(super) fn key_login(
        &self,
        agent_id: &AgentId,
        account: Option<AccountId>,
    ) -> Option<KeyLogin> {
        account?;
        self.account_description(agent_id)?.key_login
    }

    /// Logs the connection's agent in with one of its methods. On an account that can log in
    /// with a key, an agent started with another key than the login needs (the one entered, or
    /// none for the agent's other methods) restarts with it first.
    pub(super) fn authenticate(
        &mut self,
        connection: ConnectionId,
        method_id: acp::AuthMethodId,
        meta: Option<acp::Meta>,
    ) -> Result<()> {
        let key_login = self
            .connection_account(connection)
            .and_then(|(agent_id, account)| self.key_login(&agent_id, account));
        let Some(key_login) = key_login else {
            self.update_thread(connection, |thread| thread.authenticate(method_id, meta))?;
            return Ok(());
        };
        let key = if *method_id.0 == *key_login.method {
            Some(entered_key(meta.as_ref())?)
        } else {
            None
        };
        self.update_thread(connection, |thread| {
            let started_with = started_key(thread, &key_login);
            match thread.state.command.clone() {
                Some(command) if started_with != key => thread
                    .restart_with(with_key(command, &key_login.variable, key), Some(method_id)),
                // The key reaches the agent through its environment, not `authenticate`.
                _ if key.is_some() => thread.authenticate(method_id, None),
                _ => thread.authenticate(method_id, meta),
            }
        })?;
        Ok(())
    }

    /// Logs the connection's agent out. An account that logs in with a key forgets it, and its
    /// agent starts again without it.
    pub(super) fn logout(&mut self, connection: ConnectionId) -> Result<()> {
        let key_account = self
            .connection_account(connection)
            .and_then(|(agent_id, account)| {
                let key_login = self.key_login(&agent_id, account)?;
                let id = account?;
                self.accounts
                    .get(&agent_id)
                    .logs_in_with_key(account)
                    .then_some((agent_id, id, key_login))
            });
        let Some((agent_id, account, key_login)) = key_account else {
            self.update_thread(connection, |thread| thread.logout())?;
            return Ok(());
        };
        let (_, home) = self.account_home(&agent_id, account)?;
        accounts::store_key(&home, None)?;
        self.accounts.update(&agent_id, |accounts| {
            if let Some(account) = accounts.account_mut(account) {
                account.logs_in_with_key = false;
            }
        });
        self.update_thread(connection, |thread| {
            if let Some(command) = thread.state.command.clone() {
                thread.restart_with(with_key(command, &key_login.variable, None), None);
            }
        })?;
        Ok(())
    }

    /// After a login made in agentZ, the account keeps the key its agent was started with, or
    /// none: the login that worked.
    pub(super) fn keep_key_of_login(
        &mut self,
        connection: ConnectionId,
        agent_id: &AgentId,
        account: Option<AccountId>,
    ) {
        let (Some(id), Some(key_login)) = (account, self.key_login(agent_id, account)) else {
            return;
        };
        let Some(key) = self
            .connection_thread(connection)
            .map(|thread| started_key(thread, &key_login))
        else {
            return;
        };
        let Some((_, home)) = self.account_home(agent_id, id).log_err() else {
            return;
        };
        if accounts::store_key(&home, key.as_deref())
            .with_context(|| format!("keeping the key of {agent_id}'s account {id}"))
            .log_err()
            .is_none()
        {
            return;
        }
        self.accounts.update(agent_id, |accounts| {
            if let Some(account) = accounts.account_mut(id) {
                account.logs_in_with_key = key.is_some();
            }
        });
        // Its reader may be another now.
        self.read_account_if_it_can(agent_id, account);
    }

    /// The agent and account a connection runs on.
    pub(super) fn connection_account(
        &self,
        connection: ConnectionId,
    ) -> Option<(AgentId, Option<AccountId>)> {
        match connection {
            ConnectionId::Thread(thread_id) => {
                let thread = self.projects.thread(thread_id)?;
                Some((AgentId::new(thread.agent_id.clone()?), thread.account))
            }
            ConnectionId::LoginSession(login_session_id) => {
                let login_session = self.login_sessions.get(&login_session_id)?;
                Some((login_session.agent_id.clone(), login_session.account))
            }
        }
    }

    fn connection_thread(&self, connection: ConnectionId) -> Option<&AgentThread> {
        match connection {
            ConnectionId::Thread(thread_id) => self.threads.get(&thread_id),
            ConnectionId::LoginSession(login_session_id) => self
                .login_sessions
                .get(&login_session_id)
                .map(|login_session| &login_session.thread),
        }
    }
}

/// What the user entered for an API-key login.
fn entered_key(meta: Option<&acp::Meta>) -> Result<String> {
    meta.and_then(|meta| meta.get("api-key"))
        .and_then(|api_key| api_key.get("apiKey"))
        .and_then(|key| key.as_str())
        .map(str::trim)
        .filter(|key| !key.is_empty())
        .map(str::to_string)
        .context("Enter the API key.")
}

/// The key the thread's agent was started with, if it's running.
fn started_key(thread: &AgentThread, key_login: &KeyLogin) -> Option<String> {
    thread
        .state
        .command
        .as_ref()
        .and_then(|command| command.env.get(&key_login.variable))
        .cloned()
}

fn with_key(mut command: AgentCommand, variable: &str, key: Option<String>) -> AgentCommand {
    match key {
        Some(key) => {
            command.env.insert(variable.to_string(), key);
            command.env_remove.retain(|removed| removed != variable);
        }
        None => {
            command.env.remove(variable);
            if !command.env_remove.iter().any(|removed| removed == variable) {
                command.env_remove.push(variable.to_string());
            }
        }
    }
    command
}
