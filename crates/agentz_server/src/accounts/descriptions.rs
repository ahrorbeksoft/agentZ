//! What agentZ knows about each agent it can run on several accounts.

use std::collections::BTreeMap;
use std::path::Path;

use agentz_protocol::agents::AgentCommand;
use serde::{Deserialize, Serialize};

use super::LoginCheck;

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
    /// Variables the agent takes as a login, which would override the account's own.
    pub login_variables: Vec<String>,
    pub login_check: LoginCheck,
}

/// The description of a registry agent, by its id.
pub fn built_in(agent_id: &str) -> Option<AgentDescription> {
    match agent_id {
        "factory-droid" => Some(super::droid::description()),
        _ => None,
    }
}

impl AgentDescription {
    /// Makes `command` run on an agentZ account whose folder is `home`: the server's
    /// environment without the agent's login variables, then the account's Environment
    /// (`account_env`), then the home variables and file storage switches. A login variable set
    /// in the account's Environment stays, since the user set it on purpose.
    pub fn apply(
        &self,
        command: &mut AgentCommand,
        account_env: BTreeMap<String, String>,
        home: &Path,
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
            login_variables: vec!["AGENT_API_KEY".into(), "GITHUB_TOKEN".into()],
            login_check: LoginCheck::Session,
        };
        let mut command = AgentCommand {
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
        description.apply(
            &mut command,
            account_env,
            Path::new("/data/accounts/agent/1"),
        );

        let env = |variable: &str| command.env.get(variable).map(String::as_str);
        assert_eq!(env("AGENT_API_KEY"), None);
        assert_eq!(env("GITHUB_TOKEN"), Some("set-on-purpose"));
        assert_eq!(env("NODE_OPTIONS"), Some("--no-warnings"));
        assert_eq!(env("AGENT_HOME"), Some("/data/accounts/agent/1"));
        assert_eq!(env("XDG_DATA_HOME"), Some("/data/accounts/agent/1/data"));
        assert_eq!(env("AGENT_KEYRING"), Some("file"));
        assert_eq!(command.env_remove, ["AGENT_API_KEY"]);
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
}
