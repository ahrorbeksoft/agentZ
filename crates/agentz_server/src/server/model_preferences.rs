//! Shared's model preferences for each agent (`Request::SetModelPreferences`): the models its
//! pickers leave out, its favorites and their order. The setting is the app's; the server
//! keeps its own copy so `orchestrator_capabilities` names only the models the user shows,
//! with the app closed too.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use agentz_protocol::accounts::AccountId;
use agentz_protocol::agents::AgentId;
use agentz_protocol::models::{self, ModelPreferences};
use anyhow::Result;

use super::Server;
use crate::agent_settings::{read_json, write_json};

fn path(data_dir: &Path) -> PathBuf {
    data_dir.join("model-preferences.json")
}

pub(crate) fn load(data_dir: &Path) -> Result<BTreeMap<AgentId, ModelPreferences>> {
    Ok(read_json(&path(data_dir))?.unwrap_or_default())
}

impl Server {
    pub(super) fn set_model_preferences(
        &mut self,
        preferences: BTreeMap<AgentId, ModelPreferences>,
    ) -> Result<()> {
        if preferences == self.model_preferences {
            return Ok(());
        }
        write_json(&path(&self.data_dir), &preferences)?;
        self.model_preferences = preferences;
        Ok(())
    }

    /// The model preferences the account's threads follow: its own, or Shared's.
    pub(super) fn followed_model_preferences(
        &self,
        agent_id: &AgentId,
        account: Option<AccountId>,
    ) -> ModelPreferences {
        let own = self.account_settings(agent_id, account).models;
        models::followed(
            &self.accounts.get(agent_id),
            own.as_ref(),
            self.model_preferences.get(agent_id),
        )
        .cloned()
        .unwrap_or_default()
    }
}
