//! Whether each account is logged in: from its threads and login sessions, or from the check
//! its agent's description names (an empty session, or the agent's own status command).

use std::time::Duration;

use agentz_protocol::accounts::AccountId;
use agentz_protocol::agents::AgentId;
use anyhow::Context as _;
use util::ResultExt as _;

use super::Server;
use crate::accounts::{LoginCheck, StatusCommand};

/// How long the server's own login check may take to open its session before it's given up.
const LOGIN_CHECK_TIMEOUT: Duration = Duration::from_secs(120);

impl Server {
    /// Checks the normal home's login of each agent that has agentZ accounts, which decides
    /// whether the External account is listed beside them.
    pub(super) fn check_external_logins(&mut self) {
        let agents: Vec<AgentId> = self
            .accounts
            .all()
            .iter()
            .filter(|(_, accounts)| !accounts.accounts.is_empty())
            .map(|(agent_id, _)| agent_id.clone())
            .collect();
        for agent_id in agents {
            self.check_login(&agent_id, None);
        }
    }

    /// Runs the account's login check, `None` being the External account.
    pub(super) fn check_login(&mut self, agent_id: &AgentId, account: Option<AccountId>) {
        match self.login_check(agent_id) {
            LoginCheck::Session => {
                let login_session_id = self.open_login_session(agent_id.clone(), account, None);
                self.spawn_then(
                    tokio::time::sleep(LOGIN_CHECK_TIMEOUT),
                    move |server, ()| server.finish_login_check(login_session_id),
                );
            }
            LoginCheck::Command(status) => self.run_status_command(agent_id, account, status),
        }
    }

    /// Agents without a description have their sessions tell.
    fn login_check(&self, agent_id: &AgentId) -> LoginCheck {
        self.account_description(agent_id)
            .map(|description| description.login_check)
            .unwrap_or_default()
    }

    /// Where sessions open logged out too, a login session's own session doesn't tell, so the
    /// status command runs as the login session opens or checks again.
    pub(super) fn check_login_session_with_command(&mut self, login_session_id: u64) {
        let Some(login_session) = self.login_sessions.get(&login_session_id) else {
            return;
        };
        let (agent_id, account) = (login_session.agent_id.clone(), login_session.account);
        if let LoginCheck::Command(status) = self.login_check(&agent_id) {
            self.run_status_command(&agent_id, account, status);
        }
    }

    /// Records what a thread or login session on the account found. A session that opened
    /// says it's logged in, unless the agent's sessions open logged out too: then only the
    /// agent's own report counts, and a login or logout is checked with its status command.
    pub(super) fn thread_login_changed(
        &mut self,
        agent_id: &AgentId,
        account: Option<AccountId>,
        logged_in: Option<bool>,
        logged_in_or_out: bool,
        reported_login: Option<bool>,
    ) {
        // This runs for every message an agent sends.
        let recorded = self
            .accounts
            .all()
            .get(agent_id)
            .and_then(|accounts| accounts.logged_in(account));
        if !logged_in_or_out
            && reported_login.is_none()
            && logged_in.is_none_or(|logged_in| recorded == Some(logged_in))
        {
            return;
        }
        let found = match self.login_check(agent_id) {
            LoginCheck::Session => logged_in,
            LoginCheck::Command(status) => {
                if logged_in_or_out {
                    self.run_status_command(agent_id, account, status);
                }
                reported_login
            }
        };
        if let Some(logged_in) = found {
            self.accounts.update(agent_id, |accounts| {
                accounts.set_logged_in(account, logged_in)
            });
        }
    }

    /// Closes the server's own login check, which has its answer or is given up.
    pub(super) fn finish_login_check(&mut self, login_session_id: u64) {
        if self
            .login_sessions
            .get(&login_session_id)
            .is_some_and(|login_session| login_session.owner.is_none())
        {
            self.login_sessions.remove(&login_session_id);
        }
    }

    fn run_status_command(
        &mut self,
        agent_id: &AgentId,
        account: Option<AccountId>,
        status: StatusCommand,
    ) {
        let command = self.agent_command(agent_id, account, true);
        let checked = async move { status.run(command.await?).await };
        let agent_id = agent_id.clone();
        self.spawn_then(checked, move |server, logged_in| {
            let Some(logged_in) = logged_in
                .with_context(|| format!("checking {agent_id}'s login"))
                .log_err()
            else {
                return;
            };
            server.accounts.update(&agent_id, |accounts| {
                accounts.set_logged_in(account, logged_in)
            });
        });
    }
}
