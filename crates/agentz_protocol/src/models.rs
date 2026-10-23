//! Which of an agent's models its pickers offer, and in what order (Settings › Agents › an
//! agent › Models, `design/model-visibility`), as t3code keeps a provider's `hiddenModels`,
//! `favoriteModels` and `modelOrder`. Shared's, which every account follows unless it sets its
//! own, are the app's, given to every machine's server ([`crate::Request::SetModelPreferences`]);
//! an account that sets its own keeps them in its settings
//! ([`crate::agents::AgentSettings::models`]), on its machine.

use std::collections::{BTreeSet, HashMap};

use serde::{Deserialize, Serialize};

use crate::accounts::AgentAccounts;

/// The user's choices for one agent's models, by the value ids of its model setting.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ModelPreferences {
    /// Left out of the pickers. A model the agent adds later isn't here, so it's shown.
    pub hidden: BTreeSet<String>,
    /// Listed first in the picker, and what Cycle Favorite Models goes through.
    pub favorites: BTreeSet<String>,
    /// The user's order. Models not in it follow those that are, in the agent's order.
    pub order: Vec<String>,
}

impl ModelPreferences {
    pub fn is_shown(&self, model: &str) -> bool {
        !self.hidden.contains(model)
    }

    pub fn is_favorite(&self, model: &str) -> bool {
        self.favorites.contains(model)
    }

    pub fn set_shown(&mut self, model: &str, shown: bool) {
        if shown {
            self.hidden.remove(model);
        } else {
            self.hidden.insert(model.to_string());
        }
    }

    pub fn set_favorite(&mut self, model: &str, favorite: bool) {
        if favorite {
            self.favorites.insert(model.to_string());
        } else {
            self.favorites.remove(model);
        }
    }

    /// Sorts `models` in the user's order: those in [`Self::order`] as there, then the rest as
    /// they were (t3code's `sortModelsForProviderInstance`).
    pub fn sort<T>(&self, models: &mut [T], id: impl Fn(&T) -> &str) {
        let rank: HashMap<&str, usize> = self
            .order
            .iter()
            .enumerate()
            .map(|(index, model)| (model.as_str(), index))
            .collect();
        models.sort_by_key(|model| rank.get(id(model)).copied().unwrap_or(usize::MAX));
    }

    /// Puts `model` where `target` is among `models`, the agent's models as the list shows
    /// them, and keeps that whole order, as t3code keeps the order it shows. Models this
    /// machine's agent doesn't offer keep their places after them.
    pub fn move_model(&mut self, models: &[String], model: &str, target: &str) {
        let mut order = models.to_vec();
        let (Some(from), Some(to)) = (
            order.iter().position(|id| id == model),
            order.iter().position(|id| id == target),
        ) else {
            return;
        };
        let moved = order.remove(from);
        order.insert(to, moved);
        order.extend(self.order.iter().filter(|id| !models.contains(id)).cloned());
        self.order = order;
    }
}

/// The preferences an account's pickers follow: its own (`own`) while it keeps them and the
/// agent lists other accounts too, else Shared's (`shared`). An agent with one account has no
/// Account menu, so its Models tab changes Shared's.
pub fn followed<'a>(
    accounts: &AgentAccounts,
    own: Option<&'a ModelPreferences>,
    shared: Option<&'a ModelPreferences>,
) -> Option<&'a ModelPreferences> {
    match own {
        Some(own) if accounts.listed().len() > 1 => Some(own),
        _ => shared,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids(models: &[&str]) -> Vec<String> {
        models.iter().map(|model| model.to_string()).collect()
    }

    #[test]
    fn models_sort_in_the_users_order_then_the_agents() {
        let preferences = ModelPreferences {
            order: ids(&["sonnet", "opus"]),
            ..ModelPreferences::default()
        };
        let mut models = vec!["auto", "opus", "haiku", "sonnet", "fable"];
        preferences.sort(&mut models, |model| model);
        assert_eq!(models, ["sonnet", "opus", "auto", "haiku", "fable"]);
    }

    #[test]
    fn a_moved_model_takes_its_targets_place() {
        let shown = ids(&["auto", "opus", "sonnet", "haiku"]);
        let mut preferences = ModelPreferences {
            order: ids(&["gone", "opus"]),
            ..ModelPreferences::default()
        };
        preferences.move_model(&shown, "opus", "haiku");
        assert_eq!(
            preferences.order,
            ids(&["auto", "sonnet", "haiku", "opus", "gone"])
        );
        preferences.move_model(&preferences.order.clone(), "haiku", "auto");
        assert_eq!(
            preferences.order,
            ids(&["haiku", "auto", "sonnet", "opus", "gone"])
        );
        // A model that isn't listed moves nothing.
        preferences.move_model(&shown, "new", "auto");
        assert_eq!(
            preferences.order,
            ids(&["haiku", "auto", "sonnet", "opus", "gone"])
        );
    }

    #[test]
    fn new_models_are_shown() {
        let mut preferences = ModelPreferences::default();
        preferences.set_shown("opus-4", false);
        assert!(!preferences.is_shown("opus-4"));
        assert!(preferences.is_shown("opus-6"));
        preferences.set_shown("opus-4", true);
        assert_eq!(preferences, ModelPreferences::default());
    }
}
