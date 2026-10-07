//! Google Antigravity's accounts.

use std::collections::BTreeMap;

use super::{AgentDescription, KeyLogin, LoginCheck};

/// The ACP server keeps its login, sessions, MCP servers, hooks and skills in `GEMINI_HOME`, or
/// else `~/.gemini`.
pub(super) fn description() -> AgentDescription {
    AgentDescription {
        home_variables: BTreeMap::from([("GEMINI_HOME".into(), String::new())]),
        shared_folders: BTreeMap::new(),
        external_links: Vec::new(),
        // Without it, the login goes to one keychain entry every home shares, where the
        // user's own is. With it, the login is `antigravity-acp/acp_token.json` in the home.
        file_storage: BTreeMap::from([("AGY_ACP_FORCE_FILE_STORAGE".into(), "1".into())]),
        // Never its `antigravity-acp/settings.json`: with a login method named there and none
        // stored, `session/new` waits up to 5 minutes on a browser login.
        home_files: BTreeMap::new(),
        // Its MCP servers and hooks. Its own `settings.json` holds only the login's method and
        // project.
        settings_files: vec!["config/mcp_config.json".into(), "config/hooks.json".into()],
        login_settings: BTreeMap::new(),
        normal_home: ".gemini".into(),
        // Each logs it in by another method than Google's.
        login_variables: [
            "GEMINI_API_KEY",
            "GOOGLE_API_KEY",
            "GOOGLE_APPLICATION_CREDENTIALS",
            "GOOGLE_CLOUD_PROJECT",
            "GOOGLE_CLOUD_LOCATION",
            "AGY_LLM_GATEWAY_URL",
            "AGY_LLM_GATEWAY_API_KEY",
            "AGY_LLM_GATEWAY_HEADERS",
            "AGY_GATEWAY_URL",
            "AGY_GATEWAY_API_KEY",
            "AGY_GATEWAY_HEADERS",
            "AGY_ACP_CCPA_PROJECT",
        ]
        .map(String::from)
        .to_vec(),
        skills_folders: vec!["config/skills".into(), "antigravity-cli/skills".into()],
        // It reads no skills outside its home but the project's.
        outside_skills_folders: Vec::new(),
        // Logged out, `session/new` fails with "Authentication required".
        login_check: LoginCheck::Session,
        // Nothing over ACP gives the identity or quota, and Google's APIs would need an access
        // token, which it keeps only in memory: reading them would mean renewing its login.
        reader: None,
        // "Gemini API key" reads `GEMINI_API_KEY` when `authenticate` brings none.
        key_login: Some(KeyLogin {
            method: "gemini-api-key".into(),
            variable: "GEMINI_API_KEY".into(),
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

    /// What the ACP server 1.3.0 answered in a new home.
    const INITIALIZE: &str = include_str!("antigravity_reads/initialize.json");

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
                ("GEMINI_API_KEY".to_string(), "outside".to_string()),
                ("GOOGLE_CLOUD_PROJECT".to_string(), "outside".to_string()),
            ]
            .into_iter()
            .collect(),
            ..AgentCommand::default()
        };
        let home = Path::new("/data/accounts/antigravity-acp/2");
        description().apply(&mut command, BTreeMap::new(), home, Some("key".into()));
        assert_eq!(
            command.env.get("GEMINI_HOME").map(String::as_str),
            Some("/data/accounts/antigravity-acp/2")
        );
        assert_eq!(
            command
                .env
                .get("AGY_ACP_FORCE_FILE_STORAGE")
                .map(String::as_str),
            Some("1")
        );
        assert_eq!(
            command.env.get("GEMINI_API_KEY").map(String::as_str),
            Some("key")
        );
        assert!(!command.env.contains_key("GOOGLE_CLOUD_PROJECT"));
        assert!(
            command
                .env_remove
                .contains(&"GOOGLE_CLOUD_PROJECT".to_string())
        );
        // Nothing names a login method before one is used.
        let description = description();
        assert!(description.home_files.is_empty());
        assert!(
            !description
                .settings_files
                .iter()
                .any(|path| path.starts_with("antigravity-acp"))
        );
    }
}
