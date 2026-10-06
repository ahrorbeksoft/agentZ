//! An agent's accounts: its own login (the External account) and the logins made in agentZ,
//! each in its own home folder, with its own sessions, history and settings.

use anyhow::{Context as _, Result};
pub use projects::AccountId;
use serde::{Deserialize, Serialize};

use crate::agents::AgentSettings;

/// An agent's accounts, as `agents/accounts.json` keeps them. The External account's settings
/// are the agent's settings ([`AgentSettings`] in `agents/settings.json`).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AgentAccounts {
    /// In the order they were added.
    pub accounts: Vec<Account>,
    /// Use for New Threads. `None` leaves it to the External account while it's listed, or
    /// else the first one.
    pub default_account: Option<AccountId>,
    /// What the user chose for the External account, kept while it isn't listed.
    pub external: AccountChoices,
    /// Whether the agent's normal home was logged in when last checked. The External account is
    /// listed unless it wasn't.
    pub external_logged_in: Option<bool>,
    /// The last id given out, so none is given twice.
    pub last_id: u64,
}

/// A login made in agentZ, in `accounts/<agent id>/<account id>/` in the data directory.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Account {
    pub id: AccountId,
    #[serde(flatten)]
    pub choices: AccountChoices,
    #[serde(default)]
    pub settings: AgentSettings,
}

/// What the user chooses for any account, the External one included.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AccountChoices {
    /// Rename's short name, shown instead of the email.
    pub label: Option<String>,
    /// t3code's accent color, as `#rrggbb`: it tints the agent's icon on the account's threads.
    pub color: Option<String>,
    pub at_limit: AtLimit,
}

/// What a thread does when its account reaches a limit.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum AtLimit {
    #[default]
    Stop,
    /// Sends the message again when the limit resets.
    ContinueAtReset,
}

/// The account a new thread runs on.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum AccountChoice {
    /// The agent's account for new threads ([`AgentAccounts::new_thread_account`]).
    #[default]
    Default,
    External,
    Account(AccountId),
}

/// [`crate::Request::UpdateAccount`]: a change to one account.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum AccountChange {
    /// `None` shows the email again.
    Rename(Option<String>),
    SetColor(Option<String>),
    SetAtLimit(AtLimit),
    /// Use for New Threads.
    MakeDefault,
    /// From a newer version.
    #[serde(untagged)]
    Unknown(serde_json::Value),
}

impl AgentAccounts {
    pub fn account(&self, id: AccountId) -> Option<&Account> {
        self.accounts.iter().find(|account| account.id == id)
    }

    pub fn account_mut(&mut self, id: AccountId) -> Option<&mut Account> {
        self.accounts.iter_mut().find(|account| account.id == id)
    }

    /// Unless the normal home was found logged out. Before it's checked, it's listed as the
    /// agent's login was shown before it had accounts.
    pub fn lists_external(&self) -> bool {
        self.external_logged_in != Some(false)
    }

    /// Use for New Threads' account, else the External account while it's listed, else the
    /// first one. `None` is the External account.
    pub fn new_thread_account(&self) -> Option<AccountId> {
        if let Some(id) = self.default_account
            && self.account(id).is_some()
        {
            return Some(id);
        }
        if self.lists_external() {
            return None;
        }
        self.accounts.first().map(|account| account.id)
    }

    /// The account a new thread runs on, `None` being the External one.
    pub fn choose(&self, choice: AccountChoice) -> Result<Option<AccountId>> {
        match choice {
            AccountChoice::Default => Ok(self.new_thread_account()),
            AccountChoice::External => Ok(None),
            AccountChoice::Account(id) => {
                self.account(id).context("the account was removed")?;
                Ok(Some(id))
            }
        }
    }

    /// A new account, logged out until it logs in.
    pub fn add(&mut self) -> AccountId {
        self.last_id += 1;
        let id = AccountId(self.last_id);
        self.accounts.push(Account {
            id,
            choices: AccountChoices::default(),
            settings: AgentSettings::default(),
        });
        id
    }

    pub fn remove(&mut self, id: AccountId) -> Result<()> {
        let index = self
            .accounts
            .iter()
            .position(|account| account.id == id)
            .context("there's no such account")?;
        self.accounts.remove(index);
        if self.default_account == Some(id) {
            self.default_account = None;
        }
        Ok(())
    }

    /// Changes `account`, `None` being the External one.
    pub fn change(&mut self, account: Option<AccountId>, change: AccountChange) -> Result<()> {
        if let AccountChange::MakeDefault = change {
            if let Some(id) = account {
                self.account(id).context("there's no such account")?;
            }
            // Without a default, new threads take the External account while it's listed.
            self.default_account = account;
            return Ok(());
        }
        let choices = match account {
            None => &mut self.external,
            Some(id) => {
                &mut self
                    .account_mut(id)
                    .context("there's no such account")?
                    .choices
            }
        };
        match change {
            AccountChange::Rename(label) => {
                choices.label = label
                    .map(|label| label.trim().to_string())
                    .filter(|label| !label.is_empty());
            }
            AccountChange::SetColor(color) => choices.color = color,
            AccountChange::SetAtLimit(at_limit) => choices.at_limit = at_limit,
            AccountChange::MakeDefault => {}
            AccountChange::Unknown(change) => {
                anyhow::bail!("unsupported account change: {change}")
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_threads_take_the_default_then_external_then_the_first() {
        let mut accounts = AgentAccounts::default();
        assert_eq!(accounts.new_thread_account(), None);

        let first = accounts.add();
        let second = accounts.add();
        assert_eq!((first, second), (AccountId(1), AccountId(2)));
        assert_eq!(accounts.new_thread_account(), None);

        accounts.external_logged_in = Some(false);
        assert_eq!(accounts.new_thread_account(), Some(first));

        accounts
            .change(Some(second), AccountChange::MakeDefault)
            .expect("change");
        assert_eq!(accounts.new_thread_account(), Some(second));
        assert_eq!(
            accounts.choose(AccountChoice::External).expect("choose"),
            None
        );

        accounts.remove(second).expect("remove");
        assert_eq!(accounts.default_account, None);
        assert_eq!(accounts.new_thread_account(), Some(first));
        assert!(accounts.choose(AccountChoice::Account(second)).is_err());

        // Ids aren't given twice.
        assert_eq!(accounts.add(), AccountId(3));
    }

    #[test]
    fn changes_the_external_account_too() {
        let mut accounts = AgentAccounts::default();
        let id = accounts.add();
        accounts
            .change(None, AccountChange::Rename(Some("  Work ".into())))
            .expect("rename");
        accounts
            .change(None, AccountChange::SetAtLimit(AtLimit::ContinueAtReset))
            .expect("at limit");
        accounts
            .change(Some(id), AccountChange::SetColor(Some("#3b82f6".into())))
            .expect("color");
        accounts
            .change(Some(id), AccountChange::Rename(Some(" ".into())))
            .expect("rename");
        assert_eq!(accounts.external.label.as_deref(), Some("Work"));
        assert_eq!(accounts.external.at_limit, AtLimit::ContinueAtReset);
        let account = accounts.account(id).expect("account");
        assert_eq!(account.choices.color.as_deref(), Some("#3b82f6"));
        assert_eq!(account.choices.label, None);
        assert!(
            accounts
                .change(Some(AccountId(9)), AccountChange::MakeDefault)
                .is_err()
        );
    }

    #[test]
    fn keeps_choices_beside_the_id() {
        let mut accounts = AgentAccounts::default();
        let id = accounts.add();
        accounts
            .change(Some(id), AccountChange::Rename(Some("Side".into())))
            .expect("rename");
        let json = serde_json::to_value(&accounts).expect("serialize");
        assert_eq!(json["accounts"][0]["id"], 1);
        assert_eq!(json["accounts"][0]["label"], "Side");
        let read: AgentAccounts = serde_json::from_value(json).expect("deserialize");
        assert_eq!(read, accounts);
        let old: AgentAccounts = serde_json::from_str("{}").expect("deserialize");
        assert_eq!(old, AgentAccounts::default());
    }
}
