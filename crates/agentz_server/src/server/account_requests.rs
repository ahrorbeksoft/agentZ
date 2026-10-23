//! Adding, removing and changing an agent's accounts, the account a new thread runs on, each
//! account's settings, and the home its agent runs in.

use std::path::PathBuf;

use agentz_protocol::accounts::{
    AccountChoice, AccountId, AccountSupport, AgentAccounts, SettingsSource, SharedAccountSection,
    SharedAccountSettings,
};
use agentz_protocol::agents::{AgentId, AgentSettings};
use agentz_protocol::{AgentSettingsChange, ConnectionId, Request, Response};
use anyhow::{Context as _, Result, anyhow, bail};
use projects::ThreadId;
use util::ResultExt as _;

use super::Server;
use crate::accounts::{self, AgentDescription};

impl Server {
    pub(super) fn account_request(&mut self, request: Request) -> Result<Response> {
        match request {
            Request::AddAccount(agent_id) => {
                let Some(description) = self.account_description(&agent_id) else {
                    bail!(
                        "{} can't have more than one account",
                        self.agent_name(&agent_id)
                    );
                };
                // Every account gets a folder, named after its agent.
                accounts::agent_folder(&self.data_dir, &agent_id)?;
                self.start_account_sharing(&agent_id);
                let source = self.accounts.get(&agent_id).default_settings_source();
                let id = self.accounts.update(&agent_id, AgentAccounts::add);
                let started = accounts::home(&self.data_dir, &agent_id, id)
                    .and_then(|home| description.start_home(&home));
                if let Err(error) = started {
                    accounts::remove_home(&self.data_dir, &agent_id, id).log_err();
                    self.accounts
                        .update(&agent_id, |accounts| accounts.remove(id))
                        .log_err();
                    return Err(error);
                }
                // A copy that fails leaves the account with nothing copied, to pick again.
                self.copy_account_settings(&agent_id, id, source).log_err();
                self.added_accounts.insert((agent_id, id));
                Ok(Response::AccountAdded(id))
            }
            Request::CopyAccountSettings {
                agent_id,
                account,
                from,
            } => {
                self.copy_account_settings(&agent_id, account, from)?;
                Ok(Response::Ok)
            }
            Request::UpdateSharedAccountSettings { agent_id, change } => {
                self.update_shared_account_settings(&agent_id, change)?;
                Ok(Response::Ok)
            }
            Request::UseSharedAccountSettings {
                agent_id,
                account,
                section,
                on,
            } => {
                self.use_shared_account_settings(&agent_id, account, section, on)?;
                Ok(Response::Ok)
            }
            Request::RemoveAccount { agent_id, account } => {
                self.remove_account(&agent_id, account)?;
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
                let reader = self.usage_reader(&agent_id, account).with_context(|| {
                    format!("agentZ can't read {}'s usage", self.agent_name(&agent_id))
                })?;
                self.read_account(&agent_id, account, reader);
                Ok(Response::Ok)
            }
            request => Err(anyhow!("not an account request: {request:?}")),
        }
    }

    /// Removes the account with its folder, stopping what runs on it.
    pub(super) fn remove_account(&mut self, agent_id: &AgentId, account: AccountId) -> Result<()> {
        self.accounts
            .get(agent_id)
            .account(account)
            .context("there's no such account")?;
        // They write into its folder.
        let stopped = self.stop_account_agents(agent_id, account);
        // The folder first: if it can't go, the account stays to try again.
        accounts::remove_home(&self.data_dir, agent_id, account)?;
        self.accounts
            .update(agent_id, |accounts| accounts.remove(account))?;
        self.added_accounts.remove(&(agent_id.clone(), account));
        // Threads that are open start again, to say why they can't go on.
        let watched = self.watched_threads();
        for thread_id in stopped {
            if watched.contains(&thread_id) {
                self.update_thread(ConnectionId::Thread(thread_id), |_| {})
                    .log_err();
            }
        }
        Ok(())
    }

    /// How the agent keeps an account in a folder of agentZ's: a custom agent's from
    /// `custom.json`, else agentZ's own. Without one, the agent has only the External account.
    pub(super) fn account_description(&self, agent_id: &AgentId) -> Option<AgentDescription> {
        match self.custom_agents.get(agent_id) {
            Some(agent) => agent.accounts.clone(),
            None => accounts::built_in_description(&agent_id.0),
        }
    }

    /// What the agent's listing says it offers for accounts, if it can have them.
    pub(super) fn account_support(&self, agent_id: &AgentId) -> Option<AccountSupport> {
        let description = self.account_description(agent_id)?;
        let folder = accounts::agent_folder(&self.data_dir, agent_id).log_err()?;
        Some(description.support(folder))
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

    /// Copy settings from: `from`'s settings files in place of the account's, and its
    /// Environment and defaults, each as `from` has it: Shared's, or a copy of its own.
    /// Nothing leaves both to Shared.
    fn copy_account_settings(
        &mut self,
        agent_id: &AgentId,
        account: AccountId,
        from: SettingsSource,
    ) -> Result<()> {
        let (description, home) = self.account_home(agent_id, account)?;
        let accounts = self.accounts.get(agent_id);
        let (source_home, mut settings, source) = match from {
            SettingsSource::Nothing => (None, AgentSettings::default(), None),
            SettingsSource::External => (
                Some(description.external_home()),
                self.account_settings(agent_id, None),
                Some(None),
            ),
            SettingsSource::Account(id) => {
                anyhow::ensure!(id != account, "An account can't copy its own settings.");
                let (_, source_home) = self.account_home(agent_id, id)?;
                (
                    Some(source_home),
                    self.account_settings(agent_id, Some(id)),
                    Some(Some(id)),
                )
            }
        };
        let shares = |section| {
            accounts.shared.is_some()
                && source.is_none_or(|source| !accounts.sets_own(source, section))
        };
        let (shares_env, shares_defaults) = (
            shares(SharedAccountSection::Environment),
            shares(SharedAccountSection::Defaults),
        );
        // A login variable in the Environment is a login, which is never copied.
        settings
            .env
            .retain(|variable, _| !description.login_variables.contains(variable));
        description.copy_settings_files(source_home.as_deref(), &home)?;
        self.accounts.update(agent_id, |accounts| {
            if let Some(account) = accounts.account_mut(account) {
                account.settings.copy_settings(&settings);
                if shares_env {
                    account.settings.env.clear();
                }
                if shares_defaults {
                    account.settings.clear_defaults();
                }
                account.choices.own_env = !shares_env;
                account.choices.own_defaults = !shares_defaults;
                account.settings_from = from;
            }
        });
        Ok(())
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

    /// What the account runs with: its settings, the agent's for the External account
    /// (`None`), which keeps what was set before accounts, with Shared's Environment and
    /// defaults where it uses them. A removed account has none.
    pub(super) fn account_settings(
        &self,
        agent_id: &AgentId,
        account: Option<AccountId>,
    ) -> AgentSettings {
        let accounts = self.accounts.get(agent_id);
        let own = match account {
            None => self.agent_settings.get(agent_id),
            Some(id) => accounts
                .account(id)
                .map(|account| account.settings.clone())
                .unwrap_or_default(),
        };
        accounts.with_shared(account, own)
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

    /// Changes the Environment or defaults the account runs with: Shared's where it uses
    /// them, else its own.
    pub(super) fn change_account_choices(
        &mut self,
        agent_id: &AgentId,
        account: Option<AccountId>,
        change: impl FnOnce(&mut AgentSettings),
    ) {
        let accounts = self.accounts.get(agent_id);
        let mut settings = self.account_settings(agent_id, account);
        change(&mut settings);
        let shares_env = !accounts.sets_own(account, SharedAccountSection::Environment);
        let shares_defaults = !accounts.sets_own(account, SharedAccountSection::Defaults);
        if shares_env || shares_defaults {
            self.accounts.update(agent_id, |accounts| {
                if let Some(shared) = &mut accounts.shared {
                    if shares_env {
                        shared.env = settings.env.clone();
                    }
                    if shares_defaults {
                        shared.default_mode = settings.default_mode.clone();
                        shared.default_config_options = settings.default_config_options.clone();
                    }
                }
            });
        }
        if !shares_env || !shares_defaults {
            self.update_account_settings(agent_id, account, |own| {
                if !shares_env {
                    own.env = settings.env;
                }
                if !shares_defaults {
                    own.default_mode = settings.default_mode;
                    own.default_config_options = settings.default_config_options;
                }
            });
        }
    }

    /// Starts what the agent's accounts share ([`AgentAccounts::start_sharing`]), once it has
    /// accounts of agentZ's.
    pub(super) fn start_account_sharing(&mut self, agent_id: &AgentId) {
        if self.accounts.get(agent_id).shared.is_some() {
            return;
        }
        let external = self.agent_settings.get(agent_id);
        let login_variables = self
            .account_description(agent_id)
            .map(|description| description.login_variables)
            .unwrap_or_default();
        self.accounts.update(agent_id, |accounts| {
            accounts.start_sharing(&external, &login_variables)
        });
    }

    /// "Use shared settings" on the account's Defaults or Environment tab: off, it gets a copy
    /// of Shared's to change; back on, its copy goes.
    fn use_shared_account_settings(
        &mut self,
        agent_id: &AgentId,
        account: Option<AccountId>,
        section: SharedAccountSection,
        on: bool,
    ) -> Result<()> {
        let accounts = self.accounts.get(agent_id);
        accounts
            .choices(account)
            .context("there's no such account")?;
        let shared = accounts
            .shared
            .clone()
            .context("The agent's accounts don't share settings yet.")?;
        if accounts.sets_own(account, section) != on {
            return Ok(());
        }
        self.update_account_settings(agent_id, account, |settings| match (section, on) {
            (SharedAccountSection::Environment, false) => settings.env = shared.env,
            (SharedAccountSection::Environment, true) => settings.env.clear(),
            (SharedAccountSection::Defaults, false) => {
                settings.copy_defaults(shared.default_mode, shared.default_config_options)
            }
            (SharedAccountSection::Defaults, true) => settings.clear_defaults(),
        });
        self.accounts
            .update(agent_id, |accounts| accounts.set_own(account, section, !on))
    }

    /// A change to what the agent's accounts share, as [`Request::UpdateAgentSettings`] makes
    /// it to an account's.
    fn update_shared_account_settings(
        &mut self,
        agent_id: &AgentId,
        change: AgentSettingsChange,
    ) -> Result<()> {
        match &change {
            AgentSettingsChange::SetLoginMethod(_) => bail!("Shared settings have no login."),
            AgentSettingsChange::SetModels(_) => bail!("Shared's models are the app's."),
            AgentSettingsChange::Unknown(change) => {
                bail!("unsupported agent settings change: {change}")
            }
            _ => {}
        }
        self.accounts.update(agent_id, |accounts| {
            let shared = accounts
                .shared
                .as_mut()
                .context("The agent's accounts don't share settings yet.")?;
            let mut settings = shared.to_settings();
            change.apply(&mut settings);
            *shared = SharedAccountSettings::of(&settings);
            Ok(())
        })
    }
}
