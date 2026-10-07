//! What agentZ knows about each agent it can run on several accounts.

use std::collections::BTreeMap;
use std::path::{Component, Path};

use agentz_protocol::agents::AgentCommand;
use anyhow::{Context as _, Result};
use serde::{Deserialize, Serialize};

use super::{LoginCheck, Reader};

/// How an agent keeps its login, sessions and settings in a folder agentZ chooses. A custom
/// agent can have one in `agents/custom.json`, under `accounts`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AgentDescription {
    /// Variables that move the agent's config, login and sessions, each to a folder in the
    /// account's home (`""` being the home itself).
    pub home_variables: BTreeMap<String, String>,
    /// Switches that keep the login in a file in the home, where it would otherwise go to a
    /// keychain entry every home shares.
    pub file_storage: BTreeMap<String, String>,
    /// Files a new account's home starts with, by their path in it, such as settings that differ
    /// from the agent's defaults.
    pub home_files: BTreeMap<String, String>,
    /// Variables the agent takes as a login, which would override the account's own.
    pub login_variables: Vec<String>,
    pub login_check: LoginCheck,
    /// How agentZ reads the account's identity and limits, if it can.
    pub reader: Option<Reader>,
    /// The agent's login method that takes a key from a variable, if it has one.
    pub key_login: Option<KeyLogin>,
}

/// A login method that reads its key from the agent's environment rather than from
/// `authenticate` (Droid's "Factory API Key"). agentZ keeps an account's key in its folder and
/// starts the account's agent with the key in `variable`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct KeyLogin {
    /// The method's id among the agent's login methods.
    pub method: String,
    pub variable: String,
    /// How agentZ reads an account that logs in with a key, in place of the agent's `reader`.
    pub reader: Option<Reader>,
}

/// The description of a registry agent, by its id.
pub fn built_in(agent_id: &str) -> Option<AgentDescription> {
    match agent_id {
        "factory-droid" => Some(super::droid::description()),
        _ => None,
    }
}

impl AgentDescription {
    /// Writes the files a new account's `home` starts with.
    pub fn start_home(&self, home: &Path) -> Result<()> {
        for (path, contents) in &self.home_files {
            // A custom agent names these, so one must not reach outside the home.
            anyhow::ensure!(
                !path.is_empty()
                    && Path::new(path)
                        .components()
                        .all(|component| matches!(component, Component::Normal(_))),
                "{path} isn't a path inside an account's home"
            );
            let path = home.join(path);
            if let Some(folder) = path.parent() {
                std::fs::create_dir_all(folder)
                    .with_context(|| format!("creating {}", folder.display()))?;
            }
            std::fs::write(&path, contents)
                .with_context(|| format!("writing {}", path.display()))?;
        }
        Ok(())
    }

    /// Makes `command` run on an agentZ account whose folder is `home`: the server's
    /// environment without the agent's login variables, then the account's Environment
    /// (`account_env`), then the home variables and file storage switches, then the key it logs
    /// in with, if any. A login variable set in the account's Environment stays, since the user
    /// set it on purpose.
    pub fn apply(
        &self,
        command: &mut AgentCommand,
        account_env: BTreeMap<String, String>,
        home: &Path,
        key: Option<String>,
    ) {
        for variable in &self.login_variables {
            command.env.remove(variable);
        }
        command.env.extend(account_env);
        for (variable, folder) in &self.home_variables {
            let path = if folder.is_empty() {
                home.to_path_buf()
            } else {
                home.join(folder)
            };
            command
                .env
                .insert(variable.clone(), path.to_string_lossy().into_owned());
        }
        command.env.extend(
            self.file_storage
                .iter()
                .map(|(variable, value)| (variable.clone(), value.clone())),
        );
        if let (Some(key_login), Some(key)) = (&self.key_login, key) {
            command.env.insert(key_login.variable.clone(), key);
        }
        let env = &command.env;
        command.env_remove.extend(
            self.login_variables
                .iter()
                .filter(|variable| !env.contains_key(*variable))
                .cloned(),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accounts_get_their_home_and_no_outside_login() {
        let description = AgentDescription {
            home_variables: BTreeMap::from([
                ("XDG_DATA_HOME".into(), "data".into()),
                ("AGENT_HOME".into(), String::new()),
            ]),
            file_storage: BTreeMap::from([("AGENT_KEYRING".into(), "file".into())]),
            home_files: BTreeMap::new(),
            login_variables: vec!["AGENT_API_KEY".into(), "GITHUB_TOKEN".into()],
            login_check: LoginCheck::Session,
            reader: None,
            key_login: Some(KeyLogin {
                method: "agent-api-key".into(),
                variable: "AGENT_API_KEY".into(),
                reader: None,
            }),
        };
        let registry_command = AgentCommand {
            env: [
                ("AGENT_API_KEY".to_string(), "from-the-registry".to_string()),
                ("NODE_OPTIONS".to_string(), "--no-warnings".to_string()),
            ]
            .into_iter()
            .collect(),
            ..AgentCommand::default()
        };
        let account_env = BTreeMap::from([
            ("GITHUB_TOKEN".to_string(), "set-on-purpose".to_string()),
            ("AGENT_HOME".to_string(), "/elsewhere".to_string()),
        ]);
        let mut command = registry_command.clone();
        description.apply(
            &mut command,
            account_env.clone(),
            Path::new("/data/accounts/agent/1"),
            None,
        );

        let env = |variable: &str| command.env.get(variable).map(String::as_str);
        assert_eq!(env("AGENT_API_KEY"), None);
        assert_eq!(env("GITHUB_TOKEN"), Some("set-on-purpose"));
        assert_eq!(env("NODE_OPTIONS"), Some("--no-warnings"));
        assert_eq!(env("AGENT_HOME"), Some("/data/accounts/agent/1"));
        assert_eq!(env("XDG_DATA_HOME"), Some("/data/accounts/agent/1/data"));
        assert_eq!(env("AGENT_KEYRING"), Some("file"));
        assert_eq!(command.env_remove, ["AGENT_API_KEY"]);

        // An API-key account's own key goes in its place.
        let mut command = registry_command;
        description.apply(
            &mut command,
            account_env,
            Path::new("/data/accounts/agent/2"),
            Some("sk-account".into()),
        );
        assert_eq!(
            command.env.get("AGENT_API_KEY").map(String::as_str),
            Some("sk-account")
        );
        assert!(command.env_remove.is_empty());
    }

    #[test]
    fn custom_agents_describe_their_accounts() {
        let description: AgentDescription = serde_json::from_str(
            r#"{"home_variables": {"MOCK_HOME": ""}, "login_variables": ["MOCK_API_KEY"]}"#,
        )
        .expect("parse");
        assert_eq!(description.home_variables["MOCK_HOME"], "");
        assert!(description.file_storage.is_empty());
        assert_eq!(description.login_check, LoginCheck::Session);
        assert!(built_in("factory-droid").is_some());
        assert!(built_in("mock").is_none());
    }

    #[test]
    fn new_homes_start_with_their_files() {
        let dir = tempfile::tempdir().expect("temp dir");
        let home = dir.path().join("1");
        let description = AgentDescription {
            home_files: BTreeMap::from([(
                ".agent/settings.json".into(),
                r#"{"sync": false}"#.into(),
            )]),
            ..AgentDescription::default()
        };
        description.start_home(&home).expect("start");
        assert_eq!(
            std::fs::read_to_string(home.join(".agent/settings.json")).expect("read"),
            r#"{"sync": false}"#
        );

        for outside in ["../escape.json", "/etc/escape.json", ""] {
            let description = AgentDescription {
                home_files: BTreeMap::from([(outside.into(), "{}".into())]),
                ..AgentDescription::default()
            };
            assert!(description.start_home(&home).is_err(), "{outside}");
        }
        assert!(!dir.path().join("escape.json").exists());
    }
}
