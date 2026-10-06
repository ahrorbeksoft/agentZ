//! Each agent's accounts, kept by the server in `agents/accounts.json`, and their home folders
//! in `accounts/<agent id>/<account id>/`.

mod descriptions;
mod droid;
mod login_checks;
mod readers;

use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};

use agentz_protocol::accounts::{AccountId, AgentAccounts};
use agentz_protocol::agents::AgentId;
use anyhow::{Context as _, Result};
use util::ResultExt as _;

use crate::agent_settings::{read_json, write_json};

pub use descriptions::{AgentDescription, built_in as built_in_description};
pub use login_checks::{LoginCheck, StatusCommand};
pub use readers::{Reader, ReaderCommand};

pub struct AccountStore {
    accounts: BTreeMap<AgentId, AgentAccounts>,
    /// `None` keeps the accounts in memory only.
    path: Option<PathBuf>,
    revision: u64,
}

impl AccountStore {
    pub fn load(path: Option<PathBuf>) -> Self {
        let accounts = path
            .as_deref()
            .and_then(|path| read_json(path).log_err().flatten())
            .unwrap_or_default();
        Self {
            accounts,
            path,
            revision: 0,
        }
    }

    pub fn all(&self) -> &BTreeMap<AgentId, AgentAccounts> {
        &self.accounts
    }

    pub fn get(&self, agent_id: &AgentId) -> AgentAccounts {
        self.accounts.get(agent_id).cloned().unwrap_or_default()
    }

    /// Bumped by every change.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// Saves what `change` changed, even when it fails partway.
    pub fn update<T>(
        &mut self,
        agent_id: &AgentId,
        change: impl FnOnce(&mut AgentAccounts) -> T,
    ) -> T {
        let accounts = self.accounts.entry(agent_id.clone()).or_default();
        let previous = accounts.clone();
        let result = change(accounts);
        if *accounts != previous {
            self.revision += 1;
            self.save();
        }
        result
    }

    fn save(&self) {
        let Some(path) = &self.path else {
            return;
        };
        write_json(path, &self.accounts).log_err();
    }
}

/// The folder an agentZ account keeps its login, sessions and settings in.
pub fn home(data_dir: &Path, agent_id: &AgentId, account: AccountId) -> Result<PathBuf> {
    Ok(agent_folder(data_dir, agent_id)?.join(account.to_string()))
}

/// Where the agent's accounts have their folders.
pub fn agent_folder(data_dir: &Path, agent_id: &AgentId) -> Result<PathBuf> {
    let name: &str = &agent_id.0;
    // Agent ids name a folder here, so one must not reach outside `accounts`.
    let mut components = Path::new(name).components();
    anyhow::ensure!(
        matches!(
            (components.next(), components.next()),
            (Some(Component::Normal(_)), None)
        ),
        "{agent_id} can't name a folder"
    );
    Ok(data_dir.join("accounts").join(name))
}

/// Deletes an account's folder, if it has one yet.
pub fn remove_home(data_dir: &Path, agent_id: &AgentId, account: AccountId) -> Result<()> {
    let home = home(data_dir, agent_id, account)?;
    match std::fs::remove_dir_all(&home) {
        Err(error) if error.kind() != std::io::ErrorKind::NotFound => {
            Err(error).with_context(|| format!("removing {}", home.display()))
        }
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use agentz_protocol::accounts::AccountChange;

    use super::*;

    #[test]
    fn keeps_accounts_across_loads() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("agents").join("accounts.json");
        let mock = AgentId::new("mock");

        let mut store = AccountStore::load(Some(path.clone()));
        assert_eq!(store.get(&mock), AgentAccounts::default());
        let id = store.update(&mock, AgentAccounts::add);
        assert_eq!(store.revision(), 1);
        let failed = store.update(&mock, |accounts| {
            accounts.change(Some(AccountId(9)), AccountChange::MakeDefault)
        });
        assert!(failed.is_err());
        assert_eq!(store.revision(), 1);

        let store = AccountStore::load(Some(path));
        assert!(store.get(&mock).account(id).is_some());
    }

    #[test]
    fn homes_stay_inside_the_data_directory() {
        let data_dir = Path::new("/data");
        assert_eq!(
            home(data_dir, &AgentId::new("claude-acp"), AccountId(2)).expect("home"),
            Path::new("/data/accounts/claude-acp/2")
        );
        for agent_id in ["..", "a/b", "/etc", ""] {
            assert!(home(data_dir, &AgentId::new(agent_id), AccountId(1)).is_err());
        }

        let dir = tempfile::tempdir().expect("temp dir");
        let mock = AgentId::new("mock");
        remove_home(dir.path(), &mock, AccountId(1)).expect("nothing to remove");
        let home = home(dir.path(), &mock, AccountId(1)).expect("home");
        std::fs::create_dir_all(home.join(".factory")).expect("create");
        remove_home(dir.path(), &mock, AccountId(1)).expect("remove");
        assert!(!home.exists());
        assert!(dir.path().join("accounts").join("mock").exists());
    }
}
