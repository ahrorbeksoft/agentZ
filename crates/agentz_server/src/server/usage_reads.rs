//! Reading each account's identity and limits with its agent's reader: every 5 minutes while an
//! app is open (t3code's interval), after each turn on the account, and on demand (Refresh
//! Usage).

use std::time::{Duration, SystemTime};

use agentz_protocol::accounts::{AccountId, StatusRead};
use agentz_protocol::agents::{AgentId, InstallState};
use anyhow::Context as _;
use util::ResultExt as _;

use super::{Input, Server};
use crate::accounts::{self, Reader};

const REFRESH_INTERVAL: Duration = Duration::from_secs(5 * 60);
/// A refresh reads the accounts not read for this long. Less than the interval, so an account
/// read late in one refresh is still read in the next.
const STALE_AFTER: Duration = Duration::from_secs(150);
/// Between the reads of one refresh, so the agents don't all start at once.
const STAGGER: Duration = Duration::from_secs(3);

impl Server {
    pub(super) fn start_usage_refreshes(&self) {
        let inputs = self.inputs.clone();
        self.runtime.spawn(async move {
            loop {
                tokio::time::sleep(REFRESH_INTERVAL).await;
                let refresh = Input::Run(Box::new(|server: &mut Server| server.refresh_usage()));
                if inputs.unbounded_send(refresh).is_err() {
                    break;
                }
            }
        });
    }

    /// Reads every account not read lately, while an app is open: those of each installed agent
    /// with a reader. An External account found logged out is checked instead, so it's listed
    /// again once it's logged in.
    pub(super) fn refresh_usage(&mut self) {
        if !self
            .clients
            .values()
            .any(|client| client.subscribed_to_session)
        {
            return;
        }
        let now = SystemTime::now();
        let is_stale = |read: Option<&StatusRead>| {
            read.is_none_or(|read| {
                now.duration_since(read.read_at)
                    .is_ok_and(|age| age >= STALE_AFTER)
            })
        };
        let mut due = Vec::new();
        for agent_id in self.agents_with_readers() {
            let accounts = self.accounts.get(&agent_id);
            if self.usage_reader(&agent_id, None).is_some() {
                if !accounts.lists_external() {
                    self.check_login(&agent_id, None);
                } else if is_stale(accounts.status(None)) {
                    due.push((agent_id.clone(), None));
                }
            }
            for account in &accounts.accounts {
                if is_stale(account.status.as_ref())
                    && self.usage_reader(&agent_id, Some(account.id)).is_some()
                {
                    due.push((agent_id.clone(), Some(account.id)));
                }
            }
        }
        for (index, (agent_id, account)) in due.into_iter().enumerate() {
            self.spawn_then(
                tokio::time::sleep(STAGGER * index as u32),
                move |server, ()| server.read_account_if_it_can(&agent_id, account),
            );
        }
    }

    /// Reads the account if its agent has a reader and it's still there.
    pub(super) fn read_account_if_it_can(
        &mut self,
        agent_id: &AgentId,
        account: Option<AccountId>,
    ) {
        let is_there = account.is_none_or(|id| self.accounts.get(agent_id).account(id).is_some());
        if let Some(reader) = self.usage_reader(agent_id, account)
            && is_there
        {
            self.read_account(agent_id, account, reader);
        }
    }

    /// Reads the account now, unless a read of it is under way. A read that fails leaves the
    /// last one; a logged-out account keeps what it had too.
    pub(super) fn read_account(
        &mut self,
        agent_id: &AgentId,
        account: Option<AccountId>,
        reader: Reader,
    ) {
        if !self.reading_accounts.insert((agent_id.clone(), account)) {
            return;
        }
        let command = self.agent_command(agent_id, account, true);
        let http = self.http_client.clone();
        let folder = accounts::reader_folder(&self.data_dir, agent_id);
        let read = async move { reader.read(command.await?, http, &folder?).await };
        let agent_id = agent_id.clone();
        self.spawn_then(read, move |server, read| {
            server.reading_accounts.remove(&(agent_id.clone(), account));
            let Some(read) = read
                .with_context(|| format!("reading {agent_id}'s usage"))
                .log_err()
            else {
                return;
            };
            server.accounts.update(&agent_id, |accounts| {
                if let Some(logged_in) = read.logged_in {
                    accounts.set_logged_in(account, logged_in);
                }
                if read.logged_in != Some(false) {
                    accounts.set_status(
                        account,
                        StatusRead {
                            status: read.status,
                            read_at: SystemTime::now(),
                        },
                    );
                }
            });
        });
    }

    /// The account's reader, `None` being the External account: an account that logs in with a
    /// key has its key login's, if that has one.
    pub(super) fn usage_reader(
        &self,
        agent_id: &AgentId,
        account: Option<AccountId>,
    ) -> Option<Reader> {
        let description = self.account_description(agent_id)?;
        let key_reader = description
            .key_login
            .and_then(|key_login| key_login.reader)
            .filter(|_| self.accounts.get(agent_id).logs_in_with_key(account));
        key_reader.or(description.reader)
    }

    /// The custom agents and installed registry agents whose usage agentZ can read, on any of
    /// their accounts.
    fn agents_with_readers(&self) -> Vec<AgentId> {
        let custom = self.custom_agents.keys().cloned();
        let installed = self
            .registry
            .agents()
            .iter()
            .map(|agent| agent.id().clone())
            .filter(|agent_id| {
                !self.custom_agents.contains_key(agent_id)
                    && matches!(
                        self.registry.install_state(agent_id),
                        InstallState::Installed { .. }
                    )
            });
        custom
            .chain(installed)
            .filter(|agent_id| {
                self.account_description(agent_id)
                    .is_some_and(|description| {
                        description.reader.is_some()
                            || description
                                .key_login
                                .is_some_and(|key_login| key_login.reader.is_some())
                    })
            })
            .collect()
    }
}
