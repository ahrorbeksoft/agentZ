//! Reading each account's identity and limits with its agent's reader: every 5 minutes while an
//! app is open (t3code's interval), after each turn on the account, and on demand (Refresh
//! Usage).

use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};

use agentz_protocol::Response;
use agentz_protocol::accounts::{AccountId, OveragePreference, StatusRead};
use agentz_protocol::agents::{AgentId, InstallState};
use anyhow::{Context as _, Result};
use util::ResultExt as _;

use super::{ClientId, Input, Server};
use crate::accounts::{self, Read, Reader};

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
        let started = Instant::now();
        let command = self.agent_command(agent_id, account, true);
        let http = self.http_client.clone();
        let folder = accounts::reader_folder(&self.data_dir, agent_id);
        let lock = self.account_lock(agent_id, account);
        let read = async move {
            let _held = lock.lock().await;
            reader.read(command.await?, http, &folder?).await
        };
        let agent_id = agent_id.clone();
        self.spawn_then(read, move |server, read| {
            server.reading_accounts.remove(&(agent_id.clone(), account));
            if let Some(read) = read
                .with_context(|| format!("reading {agent_id}'s usage"))
                .log_err()
            {
                server.keep_read(&agent_id, account, read);
            }
            server.wait_for_limits_found(&agent_id, account, started);
        });
    }

    /// Droid's "Switch to Droid Core" on the account, through its reader, answered once the
    /// read that follows says it's chosen.
    pub(super) fn switch_to_droid_core(
        &mut self,
        client: ClientId,
        id: u64,
        agent_id: AgentId,
        account: Option<AccountId>,
    ) {
        let reader = self.overage_reader(&agent_id, account);
        let reader = match reader {
            Ok(reader) => reader,
            Err(error) => return self.respond(client, id, Err(error)),
        };
        let command = self.agent_command(&agent_id, account, true);
        let http = self.http_client.clone();
        let folder = accounts::reader_folder(&self.data_dir, &agent_id);
        let lock = self.account_lock(&agent_id, account);
        let switch = async move {
            let _held = lock.lock().await;
            reader
                .switch_to_droid_core(command.await?, http, &folder?)
                .await
        };
        self.spawn_then(switch, move |server, read: Result<Read>| {
            let switched = read.and_then(|read| {
                let preference = read.status.overage.and_then(|overage| overage.preference);
                server.keep_read(&agent_id, account, read);
                anyhow::ensure!(
                    preference == Some(OveragePreference::DroidCore),
                    "Droid didn't save the choice."
                );
                Ok(Response::Ok)
            });
            server.respond(client, id, switched);
        });
    }

    /// The reader of an account whose last read says it can change what Droid does at a limit.
    fn overage_reader(&self, agent_id: &AgentId, account: Option<AccountId>) -> Result<Reader> {
        let accounts = self.accounts.get(agent_id);
        if let Some(id) = account {
            accounts.account(id).context("there's no such account")?;
        }
        let overage = accounts
            .status(account)
            .and_then(|read| read.status.overage)
            .with_context(|| format!("{} has no choice at a limit", self.agent_name(agent_id)))?;
        anyhow::ensure!(
            overage.can_change,
            "This login can't change it: its organization sets it."
        );
        self.usage_reader(agent_id, account)
            .with_context(|| format!("agentZ can't read {}'s usage", self.agent_name(agent_id)))
    }

    /// Keeps what a read found. A logged-out account keeps what it had.
    fn keep_read(&mut self, agent_id: &AgentId, account: Option<AccountId>, read: Read) {
        self.accounts.update(agent_id, |accounts| {
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
    }

    fn account_lock(
        &mut self,
        agent_id: &AgentId,
        account: Option<AccountId>,
    ) -> Arc<tokio::sync::Mutex<()>> {
        self.account_locks
            .entry((agent_id.clone(), account))
            .or_default()
            .clone()
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
