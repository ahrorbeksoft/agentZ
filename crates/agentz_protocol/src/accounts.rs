//! An agent's accounts: its own login (the External account) and the logins made in agentZ,
//! each in its own home folder, with its own sessions, history and settings.

use std::path::PathBuf;
use std::time::{Duration, SystemTime};

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
    pub external_status: Option<StatusRead>,
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
    /// Whether its home was logged in when last checked.
    #[serde(default)]
    pub logged_in: Option<bool>,
    /// Whether it logs in with an API key agentZ keeps in its folder, which Log Out forgets.
    #[serde(default)]
    pub logs_in_with_key: bool,
    #[serde(default)]
    pub status: Option<StatusRead>,
}

/// An account's last identity and quota read. A read that fails leaves it, so the account
/// keeps its last numbers, with when they were read.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StatusRead {
    pub status: AccountStatus,
    pub read_at: SystemTime,
}

/// Who the account is and how much of its limits is used, as its agent's reader found.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AccountStatus {
    pub email: Option<String>,
    pub name: Option<String>,
    pub plan: Option<String>,
    /// Empty for a login with no usage to read (an API key, say).
    pub windows: Vec<LimitWindow>,
    /// What's left to spend beyond the plan's limits, as the agent words it ("$12.40").
    pub credits: Option<String>,
}

/// One of an account's limits, such as the 5-hour or the weekly one.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LimitWindow {
    /// As the agent names it.
    pub label: String,
    pub used_percent: f64,
    #[serde(default)]
    pub resets_at: Option<SystemTime>,
    /// How long the window runs, which with its reset says how much of it has gone.
    #[serde(default)]
    pub length: Option<Duration>,
}

impl LimitWindow {
    /// What's left, in whole percent: bars and labels show what remains, as Codex and t3code
    /// do.
    pub fn left_percent(&self) -> u8 {
        (100. - self.used_percent.clamp(0., 100.)).round() as u8
    }

    /// The share of the window still to come at `now`, 0 to 1: where even spending would
    /// leave what's left. `None` without its length or reset.
    pub fn time_left(&self, now: SystemTime) -> Option<f64> {
        let length = self.length.filter(|length| !length.is_zero())?;
        let remaining = self
            .resets_at?
            .duration_since(now)
            .unwrap_or(Duration::ZERO);
        Some((remaining.as_secs_f64() / length.as_secs_f64()).clamp(0., 1.))
    }
}

/// What an agent that can have accounts offers for them, in its listing
/// ([`crate::agents::AgentListing::accounts`]).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AccountSupport {
    /// Where its agentZ accounts have their folders, each named by its id, on the server's
    /// machine.
    pub folder: PathBuf,
    /// Whether agentZ can read its accounts' identity and limits.
    pub reads_usage: bool,
    /// The vendor's page for an account's usage or billing.
    pub usage_page: Option<String>,
}

impl AccountSupport {
    /// The account's folder.
    pub fn home(&self, account: AccountId) -> PathBuf {
        self.folder.join(account.to_string())
    }
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

    /// The accounts in the order they're listed: the External one (`None`) first while it's
    /// listed, then agentZ's in the order they were added.
    pub fn listed(&self) -> Vec<Option<AccountId>> {
        self.lists_external()
            .then_some(None)
            .into_iter()
            .chain(self.accounts.iter().map(|account| Some(account.id)))
            .collect()
    }

    /// What the user chose for `account`, `None` being the External one.
    pub fn choices(&self, account: Option<AccountId>) -> Option<&AccountChoices> {
        match account {
            None => Some(&self.external),
            Some(id) => self.account(id).map(|account| &account.choices),
        }
    }

    /// Rename's name for the account, else the email or name its last read found.
    pub fn name(&self, account: Option<AccountId>) -> Option<String> {
        let label = self.choices(account)?.label.clone();
        label.or_else(|| {
            let status = &self.status(account)?.status;
            status.email.clone().or_else(|| status.name.clone())
        })
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
            logged_in: None,
            logs_in_with_key: false,
            status: None,
        });
        id
    }

    /// Whether the account logs in with a key agentZ keeps. The External account never does.
    pub fn logs_in_with_key(&self, account: Option<AccountId>) -> bool {
        account
            .and_then(|id| self.account(id))
            .is_some_and(|account| account.logs_in_with_key)
    }

    /// The account's last read, `None` being the External account.
    pub fn status(&self, account: Option<AccountId>) -> Option<&StatusRead> {
        match account {
            None => self.external_status.as_ref(),
            Some(id) => self.account(id).and_then(|account| account.status.as_ref()),
        }
    }

    /// Keeps a read of `account`, `None` being the External one. A removed account is left
    /// removed.
    pub fn set_status(&mut self, account: Option<AccountId>, read: StatusRead) {
        match account {
            None => self.external_status = Some(read),
            Some(id) => {
                if let Some(account) = self.account_mut(id) {
                    account.status = Some(read);
                }
            }
        }
    }

    /// What the last login check found for `account`, `None` being the External one.
    pub fn logged_in(&self, account: Option<AccountId>) -> Option<bool> {
        match account {
            None => self.external_logged_in,
            Some(id) => self.account(id).and_then(|account| account.logged_in),
        }
    }

    /// What a login check found for `account`, `None` being the External one. A removed
    /// account is left removed.
    pub fn set_logged_in(&mut self, account: Option<AccountId>, logged_in: bool) {
        match account {
            None => self.external_logged_in = Some(logged_in),
            Some(id) => {
                if let Some(account) = self.account_mut(id) {
                    account.logged_in = Some(logged_in);
                }
            }
        }
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

        accounts.set_logged_in(None, false);
        assert_eq!(accounts.new_thread_account(), Some(first));
        accounts.set_logged_in(Some(first), true);
        accounts.set_logged_in(Some(AccountId(9)), true);
        assert_eq!(accounts.logged_in(Some(first)), Some(true));
        assert_eq!(accounts.logged_in(Some(second)), None);
        assert_eq!(accounts.logged_in(None), Some(false));

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
    fn names_accounts_and_lists_the_external_one_first() {
        let mut accounts = AgentAccounts::default();
        let id = accounts.add();
        assert_eq!(accounts.listed(), [None, Some(id)]);
        assert_eq!(accounts.name(Some(id)), None);
        let read = |email: &str| StatusRead {
            status: AccountStatus {
                email: Some(email.to_string()),
                ..AccountStatus::default()
            },
            read_at: SystemTime::UNIX_EPOCH,
        };
        accounts.set_status(Some(id), read("work@example.com"));
        assert_eq!(accounts.name(Some(id)).as_deref(), Some("work@example.com"));
        accounts
            .change(Some(id), AccountChange::Rename(Some("Work".into())))
            .expect("rename");
        assert_eq!(accounts.name(Some(id)).as_deref(), Some("Work"));
        assert_eq!(accounts.name(Some(AccountId(9))), None);

        accounts.set_logged_in(None, false);
        assert_eq!(accounts.listed(), [Some(id)]);
    }

    #[test]
    fn a_window_says_whats_left_of_it_and_of_its_time() {
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(1_000_000);
        let window = LimitWindow {
            label: "5-hour".into(),
            used_percent: 37.6,
            resets_at: Some(now + Duration::from_secs(2 * 3600)),
            length: Some(Duration::from_secs(5 * 3600)),
        };
        assert_eq!(window.left_percent(), 62);
        assert_eq!(window.time_left(now), Some(0.4));
        // Past its reset, none of it is left.
        assert_eq!(
            window.time_left(now + Duration::from_secs(3 * 3600)),
            Some(0.)
        );
        let without_length = LimitWindow {
            length: None,
            ..window.clone()
        };
        assert_eq!(without_length.time_left(now), None);
        let overspent = LimitWindow {
            used_percent: 112.,
            ..window
        };
        assert_eq!(overspent.left_percent(), 0);
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
