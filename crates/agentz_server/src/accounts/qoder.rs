//! Qoder's accounts.

use std::collections::BTreeMap;

use super::{AgentDescription, KeyLogin, LoginCheck, SHARED_SKILLS_FOLDER};

/// Qoder keeps its login (`.auth/`), settings, sessions and skills in `QODER_CONFIG_DIR`, or
/// else `~/.qoder`.
pub(super) fn description() -> AgentDescription {
    AgentDescription {
        home_variables: BTreeMap::from([("QODER_CONFIG_DIR".into(), String::new())]),
        shared_folders: BTreeMap::new(),
        external_links: Vec::new(),
        // Its login is a file in `.auth/`, encrypted with the machine id beside it.
        file_storage: BTreeMap::new(),
        home_files: BTreeMap::new(),
        // Its MCP servers, hooks and permissions, and the login method last used, which a home
        // with no login ignores.
        settings_files: vec!["settings.json".into()],
        login_settings: BTreeMap::new(),
        normal_home: ".qoder".into(),
        login_variables: [
            "QODER_PERSONAL_ACCESS_TOKEN",
            "QODER_PAT",
            "QODER_JOB_TOKEN",
        ]
        .map(String::from)
        .to_vec(),
        skills_folders: vec!["skills".into()],
        // Whatever its home.
        outside_skills_folders: vec![SHARED_SKILLS_FOLDER.into()],
        // Logged out, `session/new` fails with "Authentication required".
        login_check: LoginCheck::Session,
        // It has no quota to read but in its terminal UI, and its `status` would renew an
        // expired login itself, in the home a session may be using.
        reader: None,
        // "Use QODER_PERSONAL_ACCESS_TOKEN" logs in with that variable.
        key_login: Some(KeyLogin {
            method: "qoder-personal-access-token".into(),
            variable: "QODER_PERSONAL_ACCESS_TOKEN".into(),
            reader: None,
        }),
        usage_page: None,
        extra_usage_page: None,
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use agentz_protocol::agents::AgentCommand;

    use super::*;

    /// What Qoder 0.2.14 answered in a new home.
    const INITIALIZE: &str = include_str!("qoder_reads/initialize.json");

    #[test]
    fn its_key_login_is_one_of_its_methods() {
        let initialize: serde_json::Value = serde_json::from_str(INITIALIZE).expect("json");
        let methods: Vec<&str> = initialize["authMethods"]
            .as_array()
            .expect("methods")
            .iter()
            .filter_map(|method| method["id"].as_str())
            .collect();
        let key_login = description().key_login.expect("key login");
        assert!(methods.contains(&key_login.method.as_str()), "{methods:?}");
    }

    #[test]
    fn accounts_keep_their_login_in_their_home() {
        let mut command = AgentCommand {
            env: [
                ("QODER_PAT".to_string(), "outside".to_string()),
                (
                    "QODER_PERSONAL_ACCESS_TOKEN".to_string(),
                    "outside".to_string(),
                ),
            ]
            .into_iter()
            .collect(),
            ..AgentCommand::default()
        };
        let home = Path::new("/data/accounts/qoder/2");
        description().apply(&mut command, BTreeMap::new(), home, Some("token".into()));
        assert_eq!(
            command.env.get("QODER_CONFIG_DIR").map(String::as_str),
            Some("/data/accounts/qoder/2")
        );
        // Qoder reads `QODER_PAT` first, so the user's must not shadow the account's token.
        assert!(!command.env.contains_key("QODER_PAT"));
        assert!(command.env_remove.contains(&"QODER_PAT".to_string()));
        assert_eq!(
            command
                .env
                .get("QODER_PERSONAL_ACCESS_TOKEN")
                .map(String::as_str),
            Some("token")
        );
    }
}
