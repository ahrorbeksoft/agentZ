//! What agentZ knows about each agent it can run on several accounts.

use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};

use agentz_protocol::accounts::AccountSupport;
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
    /// Folders those variables move that hold other programs' files too (`XDG_CONFIG_HOME`'s),
    /// by their path in the home, each with the agent's own entries there. Every other entry of
    /// the user's folder is linked into the home's, so the programs the agent runs still find
    /// theirs (`gh`'s login, `git`'s config).
    pub shared_folders: BTreeMap<String, Vec<String>>,
    /// Switches that keep the login in a file in the home, where it would otherwise go to a
    /// keychain entry every home shares.
    pub file_storage: BTreeMap<String, String>,
    /// Files a new account's home starts with, by their path in it, such as settings that differ
    /// from the agent's defaults.
    pub home_files: BTreeMap<String, String>,
    /// The agent's own settings files, by their path in a home, which "Copy settings from"
    /// copies. Never its login.
    pub settings_files: Vec<String>,
    /// Settings in those files that belong to the login they were made with, by file and JSON
    /// pointer, which a copy leaves out: Devin's organization.
    pub login_settings: BTreeMap<String, Vec<String>>,
    /// Where the agent keeps what a home variable would move, when none is set: the External
    /// account's home, relative to the user's home.
    pub normal_home: String,
    /// Variables the agent takes as a login, which would override the account's own.
    pub login_variables: Vec<String>,
    pub login_check: LoginCheck,
    /// How agentZ reads the account's identity and limits, if it can.
    pub reader: Option<Reader>,
    /// The agent's login method that takes a key from a variable, if it has one.
    pub key_login: Option<KeyLogin>,
    /// The vendor's page for an account's usage or billing (Open Usage Page).
    pub usage_page: Option<String>,
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
        "claude-acp" => Some(super::claude::description()),
        "codex-acp" => Some(super::codex::description()),
        "devin" => Some(super::devin::description()),
        _ => None,
    }
}

impl AgentDescription {
    /// What clients learn of it, with its accounts' folders in `folder`.
    pub fn support(&self, folder: PathBuf) -> AccountSupport {
        AccountSupport {
            folder,
            reads_usage: self.reader.is_some()
                || self
                    .key_login
                    .as_ref()
                    .is_some_and(|key_login| key_login.reader.is_some()),
            usage_page: self.usage_page.clone(),
            copies_settings_files: !self.settings_files.is_empty(),
        }
    }

    /// Writes the files a new account's `home` starts with.
    pub fn start_home(&self, home: &Path) -> Result<()> {
        for (path, contents) in &self.home_files {
            write_file(&inside(home, path)?, contents)?;
        }
        Ok(())
    }

    /// The External account's home: the folder the server's environment moves the whole home
    /// to, as Droid takes `FACTORY_HOME_OVERRIDE`, else the agent's normal one.
    pub fn external_home(&self) -> PathBuf {
        self.home_variables
            .iter()
            .filter(|(_, folder)| folder.is_empty())
            .find_map(|(variable, _)| std::env::var_os(variable))
            .map(PathBuf::from)
            .unwrap_or_else(|| util::paths::home_dir().join(&self.normal_home))
    }

    /// Copy settings from: the settings files of the home `from` (none for Nothing) in place
    /// of `home`'s. A file the home starts with keeps its settings over a copied one, when both
    /// are JSON objects: a new Droid account still keeps its sessions off Factory's cloud.
    pub fn copy_settings_files(&self, from: Option<&Path>, home: &Path) -> Result<()> {
        for path in &self.settings_files {
            let target = inside(home, path)?;
            let source = from
                .map(|from| inside(from, path))
                .transpose()?
                .filter(|source| source.is_file());
            let start = self.home_files.get(path);
            let Some(source) = source else {
                match start {
                    Some(start) => write_file(&target, start)?,
                    None => remove_file(&target)?,
                }
                continue;
            };
            if let Some(folder) = target.parent() {
                std::fs::create_dir_all(folder)
                    .with_context(|| format!("creating {}", folder.display()))?;
            }
            // A copy keeps the file's permissions, since settings can hold keys.
            std::fs::copy(&source, &target)
                .with_context(|| format!("copying {} to {}", source.display(), target.display()))?;
            let login_settings = self.login_settings.get(path);
            if start.is_none() && login_settings.is_none() {
                continue;
            }
            let copied = std::fs::read_to_string(&target)
                .with_context(|| format!("reading {}", target.display()))?;
            let mut changed = start.and_then(|start| keep_settings(&copied, start));
            if let Some(pointers) = login_settings {
                let settings = changed.as_deref().unwrap_or(&copied);
                changed = without_settings(settings, pointers).or(changed);
            }
            if let Some(changed) = changed {
                write_file(&target, &changed)?;
            }
        }
        Ok(())
    }

    /// Links the user's entries into the account `home`'s shared folders, and removes links
    /// to entries that are gone. Run before each start of the account's agent, so it finds
    /// what the user added since.
    pub fn link_shared_folders(&self, home: &Path) -> Result<()> {
        for (path, own) in &self.shared_folders {
            let folder = inside(home, path)?;
            let user_folder = self
                .home_variables
                .iter()
                .filter(|(_, moved)| moved.as_str() == path.as_str())
                .find_map(|(variable, _)| std::env::var_os(variable))
                .map(PathBuf::from)
                .unwrap_or_else(|| util::paths::home_dir().join(path));
            link_entries(&folder, &user_folder, own)?;
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

/// `path` in `home`. A custom agent names these paths, so one must not reach outside it.
fn inside(home: &Path, path: &str) -> Result<PathBuf> {
    anyhow::ensure!(
        !path.is_empty()
            && Path::new(path)
                .components()
                .all(|component| matches!(component, Component::Normal(_))),
        "{path} isn't a path inside an account's home"
    );
    Ok(home.join(path))
}

pub(super) fn write_file(path: &Path, contents: &str) -> Result<()> {
    if let Some(folder) = path.parent() {
        std::fs::create_dir_all(folder)
            .with_context(|| format!("creating {}", folder.display()))?;
    }
    std::fs::write(path, contents).with_context(|| format!("writing {}", path.display()))
}

pub(super) fn remove_file(path: &Path) -> Result<()> {
    match std::fs::remove_file(path) {
        Err(error) if error.kind() != std::io::ErrorKind::NotFound => {
            Err(error).with_context(|| format!("removing {}", path.display()))
        }
        _ => Ok(()),
    }
}

/// `settings` without what `pointers` point to, when it's a JSON object that has any of them.
fn without_settings(settings: &str, pointers: &[String]) -> Option<String> {
    let mut settings: serde_json::Value = serde_json::from_str(settings).ok()?;
    settings.as_object()?;
    let mut removed = false;
    for pointer in pointers {
        let Some((parent, key)) = pointer.rsplit_once('/') else {
            continue;
        };
        removed |= settings
            .pointer_mut(parent)
            .and_then(serde_json::Value::as_object_mut)
            .and_then(|parent| parent.remove(key))
            .is_some();
    }
    if !removed {
        return None;
    }
    let mut text = serde_json::to_string_pretty(&settings).ok()?;
    text.push('\n');
    Some(text)
}

/// Links each entry of `user_folder` but the agent's `own` into `folder` where nothing of its
/// name is, and removes links into `user_folder` whose entry is gone. Nothing else in `folder`
/// changes: what the agent, or a program it ran, made there stays.
fn link_entries(folder: &Path, user_folder: &Path, own: &[String]) -> Result<()> {
    if folder == user_folder {
        return Ok(());
    }
    std::fs::create_dir_all(folder).with_context(|| format!("creating {}", folder.display()))?;
    for entry in
        std::fs::read_dir(folder).with_context(|| format!("reading {}", folder.display()))?
    {
        let link = entry
            .with_context(|| format!("reading {}", folder.display()))?
            .path();
        let Ok(target) = std::fs::read_link(&link) else {
            continue;
        };
        if target.parent() == Some(user_folder) && std::fs::symlink_metadata(&target).is_err() {
            remove_file(&link)?;
        }
    }
    let entries = match std::fs::read_dir(user_folder) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => {
            return Err(error).with_context(|| format!("reading {}", user_folder.display()));
        }
    };
    for entry in entries {
        let name = entry
            .with_context(|| format!("reading {}", user_folder.display()))?
            .file_name();
        if own.iter().any(|own| std::ffi::OsStr::new(own) == name) {
            continue;
        }
        let link = folder.join(&name);
        if std::fs::symlink_metadata(&link).is_ok() {
            continue;
        }
        let target = user_folder.join(&name);
        #[cfg(unix)]
        std::os::unix::fs::symlink(&target, &link)
            .with_context(|| format!("linking {} to {}", link.display(), target.display()))?;
        #[cfg(not(unix))]
        anyhow::bail!("can't link {} to {}", link.display(), target.display());
    }
    Ok(())
}

/// `copied` with the settings `start` sets, when both are JSON objects and they differ.
fn keep_settings(copied: &str, start: &str) -> Option<String> {
    let serde_json::Value::Object(mut settings) = serde_json::from_str(copied).ok()? else {
        return None;
    };
    let serde_json::Value::Object(start) = serde_json::from_str(start).ok()? else {
        return None;
    };
    if start
        .iter()
        .all(|(key, value)| settings.get(key) == Some(value))
    {
        return None;
    }
    settings.extend(start);
    let mut merged = serde_json::to_string_pretty(&settings).ok()?;
    merged.push('\n');
    Some(merged)
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
            shared_folders: BTreeMap::new(),
            file_storage: BTreeMap::from([("AGENT_KEYRING".into(), "file".into())]),
            home_files: BTreeMap::new(),
            settings_files: Vec::new(),
            login_settings: BTreeMap::new(),
            normal_home: String::new(),
            login_variables: vec!["AGENT_API_KEY".into(), "GITHUB_TOKEN".into()],
            login_check: LoginCheck::Session,
            reader: None,
            key_login: Some(KeyLogin {
                method: "agent-api-key".into(),
                variable: "AGENT_API_KEY".into(),
                reader: None,
            }),
            usage_page: None,
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

    #[test]
    fn copies_settings_files_but_keeps_what_homes_start_with() {
        let dir = tempfile::tempdir().expect("temp dir");
        let from = dir.path().join("from");
        let home = dir.path().join("1");
        let description = AgentDescription {
            home_files: BTreeMap::from([(
                ".agent/settings.json".into(),
                "{\"sync\": false}\n".into(),
            )]),
            settings_files: vec![".agent/settings.json".into(), ".agent/config.toml".into()],
            ..AgentDescription::default()
        };
        description.start_home(&home).expect("start");
        write_file(
            &from.join(".agent/settings.json"),
            r#"{"model": "opus", "sync": true}"#,
        )
        .expect("write");
        write_file(&from.join(".agent/config.toml"), "model = \"opus\"\n").expect("write");
        write_file(&from.join(".agent/login.json"), "{}").expect("write");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            std::fs::set_permissions(
                from.join(".agent/config.toml"),
                std::fs::Permissions::from_mode(0o600),
            )
            .expect("permissions");
        }

        description
            .copy_settings_files(Some(&from), &home)
            .expect("copy");
        let read = |path: &str| std::fs::read_to_string(home.join(path)).ok();
        let settings: serde_json::Value =
            serde_json::from_str(&read(".agent/settings.json").expect("settings")).expect("json");
        assert_eq!(
            settings,
            serde_json::json!({"model": "opus", "sync": false})
        );
        assert_eq!(
            read(".agent/config.toml").as_deref(),
            Some("model = \"opus\"\n")
        );
        assert_eq!(read(".agent/login.json"), None);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            let mode = std::fs::metadata(home.join(".agent/config.toml"))
                .expect("metadata")
                .permissions()
                .mode();
            assert_eq!(mode & 0o777, 0o600);
        }

        // Nothing puts back what a new home has.
        description.copy_settings_files(None, &home).expect("copy");
        assert_eq!(
            read(".agent/settings.json").as_deref(),
            Some("{\"sync\": false}\n")
        );
        assert_eq!(read(".agent/config.toml"), None);
    }

    #[test]
    fn copies_leave_out_the_logins_settings() {
        let dir = tempfile::tempdir().expect("temp dir");
        let from = dir.path().join("from");
        let home = dir.path().join("1");
        let description = AgentDescription {
            settings_files: vec![".config/agent/config.json".into()],
            login_settings: BTreeMap::from([(
                ".config/agent/config.json".into(),
                vec!["/agent/org_id".into(), "/missing/key".into()],
            )]),
            ..AgentDescription::default()
        };
        let config = ".config/agent/config.json";
        write_file(
            &from.join(config),
            r#"{"model": "opus", "agent": {"org_id": "org-1", "theme": "dark"}}"#,
        )
        .expect("write");
        description
            .copy_settings_files(Some(&from), &home)
            .expect("copy");
        let read = || std::fs::read_to_string(home.join(config)).expect("read");
        let settings: serde_json::Value = serde_json::from_str(&read()).expect("json");
        assert_eq!(
            settings,
            serde_json::json!({"model": "opus", "agent": {"theme": "dark"}})
        );

        // A file without them, or one that isn't JSON, is copied as it is.
        for copied in [r#"{"model": "opus"}"#, "// comments\n{}"] {
            write_file(&from.join(config), copied).expect("write");
            description
                .copy_settings_files(Some(&from), &home)
                .expect("copy");
            assert_eq!(read(), copied);
        }
    }

    #[cfg(unix)]
    #[test]
    fn shared_folders_link_the_users_entries() {
        let dir = tempfile::tempdir().expect("temp dir");
        let user_folder = dir.path().join("user/.config");
        let folder = dir.path().join("1/.config");
        for entry in ["gh", "git", "agent"] {
            std::fs::create_dir_all(user_folder.join(entry)).expect("create");
        }
        write_file(&user_folder.join("gh/hosts.yml"), "github.com: {}\n").expect("write");
        write_file(&user_folder.join("starship.toml"), "").expect("write");
        // The agent's own, and what a program the agent ran wrote there.
        write_file(&folder.join("agent/config.json"), "{}").expect("write");
        write_file(&folder.join("git/config"), "").expect("write");
        let own = vec!["agent".to_string()];

        link_entries(&folder, &user_folder, &own).expect("link");
        let link = |name: &str| std::fs::read_link(folder.join(name)).ok();
        assert_eq!(link("gh"), Some(user_folder.join("gh")));
        assert_eq!(
            link("starship.toml"),
            Some(user_folder.join("starship.toml"))
        );
        assert_eq!(link("git"), None);
        assert_eq!(link("agent"), None);
        assert_eq!(
            std::fs::read_to_string(folder.join("gh/hosts.yml")).expect("through the link"),
            "github.com: {}\n"
        );

        // A link to an entry the user removed goes; a new entry gets one.
        std::fs::remove_file(user_folder.join("starship.toml")).expect("remove");
        std::fs::create_dir_all(user_folder.join("fish")).expect("create");
        link_entries(&folder, &user_folder, &own).expect("link again");
        assert_eq!(link("starship.toml"), None);
        assert!(!folder.join("starship.toml").exists());
        assert_eq!(link("fish"), Some(user_folder.join("fish")));
        // Without a folder of the user's, nothing is linked.
        link_entries(&folder, &dir.path().join("none"), &own).expect("no folder");
        assert_eq!(link("gh"), Some(user_folder.join("gh")));

        // Removing the account's home leaves the user's entries.
        std::fs::remove_dir_all(dir.path().join("1")).expect("remove the home");
        assert!(user_folder.join("gh/hosts.yml").is_file());
    }
}
