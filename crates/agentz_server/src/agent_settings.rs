//! Each agent's settings, kept by the server in `agents/settings.json` so every client sees the
//! same ones.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use agentz_protocol::agents::{AgentId, AgentSettings};
use anyhow::{Context as _, Result};
use serde::Deserialize;
use util::ResultExt as _;

pub struct AgentSettingsStore {
    settings: BTreeMap<AgentId, AgentSettings>,
    /// `None` keeps the settings in memory only.
    path: Option<PathBuf>,
    revision: u64,
}

/// Where the app kept agent settings before the server did.
#[derive(Deserialize)]
struct LegacyAppSettings {
    #[serde(default)]
    agents: BTreeMap<AgentId, AgentSettings>,
}

impl AgentSettingsStore {
    /// Loads the settings, or takes them from the app's old settings file the first time.
    pub fn load(path: Option<PathBuf>, legacy_app_settings: Option<&Path>) -> Self {
        let mut store = Self {
            settings: BTreeMap::new(),
            path,
            revision: 0,
        };
        let Some(path) = store.path.clone() else {
            return store;
        };
        if let Some(settings) = read_json::<BTreeMap<AgentId, AgentSettings>>(&path).log_err() {
            match settings {
                Some(settings) => store.settings = settings,
                None => {
                    let legacy = legacy_app_settings
                        .and_then(|path| read_json::<LegacyAppSettings>(path).log_err().flatten());
                    if let Some(legacy) = legacy
                        && !legacy.agents.is_empty()
                    {
                        store.settings = legacy.agents;
                        store.save();
                    }
                }
            }
        }
        store
    }

    pub fn all(&self) -> &BTreeMap<AgentId, AgentSettings> {
        &self.settings
    }

    pub fn get(&self, agent_id: &AgentId) -> AgentSettings {
        self.settings.get(agent_id).cloned().unwrap_or_default()
    }

    /// Bumped by every change.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn update(&mut self, agent_id: &AgentId, change: impl FnOnce(&mut AgentSettings)) {
        let settings = self.settings.entry(agent_id.clone()).or_default();
        let previous = settings.clone();
        change(settings);
        if *settings != previous {
            self.revision += 1;
            self.save();
        }
    }

    fn save(&self) {
        let Some(path) = &self.path else {
            return;
        };
        write_json(path, &self.settings).log_err();
    }
}

pub(crate) fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Result<Option<T>> {
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error).with_context(|| format!("reading {}", path.display())),
    };
    let value =
        serde_json::from_slice(&bytes).with_context(|| format!("parsing {}", path.display()))?;
    Ok(Some(value))
}

pub(crate) fn write_json(path: &Path, value: &impl serde::Serialize) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("creating {}", parent.display()))?;
    }
    let json = serde_json::to_vec_pretty(value)?;
    std::fs::write(path, json).with_context(|| format!("writing {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn takes_settings_from_the_app_once() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("agents").join("settings.json");
        let legacy = dir.path().join("settings.json");
        std::fs::write(
            &legacy,
            r#"{"theme_mode":"dark","agents":{"claude":{"env":{"A":"1"}}}}"#,
        )
        .expect("write");

        let store = AgentSettingsStore::load(Some(path.clone()), Some(&legacy));
        let claude = AgentId::new("claude");
        assert_eq!(
            store.get(&claude).env.get("A").map(String::as_str),
            Some("1")
        );
        assert!(path.exists());

        let mut store = AgentSettingsStore::load(Some(path.clone()), None);
        assert_eq!(store.get(&claude).env.len(), 1);
        let revision = store.revision();
        store.update(&claude, |settings| {
            settings.login_method = Some("oauth".into())
        });
        assert_eq!(store.revision(), revision + 1);
        store.update(&claude, |settings| {
            settings.login_method = Some("oauth".into())
        });
        assert_eq!(store.revision(), revision + 1);

        let store = AgentSettingsStore::load(Some(path), Some(&legacy));
        assert_eq!(store.get(&claude).login_method.as_deref(), Some("oauth"));
    }
}
