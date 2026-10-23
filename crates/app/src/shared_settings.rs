//! Settings machines share (design/settings-2, topic 2): the Shared entry of Settings' machine
//! picker. A machine follows Shared for its thread titles, cleanup rules, skills and MCP servers
//! unless it sets its own of that section. The app keeps Shared and gives it to the machines
//! that follow it as they connect, so a machine added later gets it too.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use agentz_protocol::mcp_servers::McpServer;
use agentz_protocol::skills::{
    FOLDER_TOO_LARGE, MAX_FOLDER_SIZE, SKILL_FILE_NAME, Skill, SkillFile, format_skill_file,
    parse_frontmatter, validate_description, validate_name,
};
use agentz_protocol::storage::CleanupRules;
use agentz_protocol::title_generation::TitleGeneration;
use anyhow::{Context as _, Result, bail};
use base64::Engine as _;
use serde::{Deserialize, Serialize};

use crate::machines::MachineId;

/// The parts of a machine's settings it can take from Shared.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SharedSection {
    ThreadTitles,
    CleanupRules,
    Skills,
    McpServers,
}

impl SharedSection {
    pub const ALL: [Self; 4] = [
        Self::ThreadTitles,
        Self::CleanupRules,
        Self::Skills,
        Self::McpServers,
    ];

    /// Whether its values replace the machine's: a list's items are added beside its own.
    fn is_list(self) -> bool {
        matches!(self, Self::Skills | Self::McpServers)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SharedMachineSettings {
    /// `None` until the first machine seen gives its own as the start.
    pub title_generation: Option<TitleGeneration>,
    pub cleanup_rules: Option<CleanupRules>,
    /// Their files are in the app's data directory ([`crate::app_settings::AppSettingsStore::shared_skills_folder`]).
    pub skills: Vec<SharedSkill>,
    /// Never agentZ's own server, which each machine lists itself.
    pub mcp_servers: Vec<McpServer>,
    /// By [`MachineId::slug`].
    pub machines: BTreeMap<String, MachineSharing>,
}

/// One of Shared's skills.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SharedSkill {
    pub name: String,
    pub description: String,
    /// When its files were written, in milliseconds since the epoch, so a machine given an
    /// earlier skill of the name gets this one.
    pub revision: u64,
}

/// What a machine takes from Shared.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct MachineSharing {
    /// `true` for a section the machine sets its own of. Thread titles and cleanup rules are
    /// missing until decided: a machine saved before Shared keeps values of its own.
    pub own: BTreeMap<SharedSection, bool>,
    /// The shared skills given to it, by name, with their revisions, so those taken out of
    /// Shared go from it too.
    pub skills_given: BTreeMap<String, u64>,
    pub mcp_servers_given: BTreeSet<String>,
}

impl MachineSharing {
    /// A machine added now, which takes everything from Shared.
    pub fn following() -> Self {
        Self {
            own: SharedSection::ALL
                .into_iter()
                .map(|section| (section, false))
                .collect(),
            ..Self::default()
        }
    }

    /// Notes what a step of [`list_steps`] did.
    pub fn record(&mut self, step: &ListStep) {
        match step {
            ListStep::AddSkill(skill) => {
                self.skills_given.insert(skill.name.clone(), skill.revision);
            }
            ListStep::DeleteSkill(name) | ListStep::ForgetSkill(name) => {
                self.skills_given.remove(name);
            }
            ListStep::SaveMcpServer { server, .. } => {
                self.mcp_servers_given.insert(server.name.clone());
            }
            ListStep::KeepMcpServer(name) => {
                self.mcp_servers_given.insert(name.clone());
            }
            ListStep::DeleteMcpServer(name) | ListStep::ForgetMcpServer(name) => {
                self.mcp_servers_given.remove(name);
            }
        }
    }
}

impl SharedMachineSettings {
    pub fn sharing(&self, machine: MachineId) -> Option<&MachineSharing> {
        self.machines.get(&machine.slug())
    }

    pub fn sharing_mut(&mut self, machine: MachineId) -> &mut MachineSharing {
        self.machines.entry(machine.slug()).or_default()
    }

    /// Whether the machine has decided the section: `Some(true)` sets its own.
    pub fn decision(&self, machine: MachineId, section: SharedSection) -> Option<bool> {
        let decided = self
            .sharing(machine)
            .and_then(|sharing| sharing.own.get(&section).copied());
        if section.is_list() {
            Some(decided.unwrap_or(false))
        } else {
            decided
        }
    }

    pub fn sets_own(&self, machine: MachineId, section: SharedSection) -> bool {
        self.decision(machine, section) == Some(true)
    }

    /// Keeps a skill just written, in place of an earlier one of its name, in name order.
    pub fn put_skill(&mut self, skill: SharedSkill) {
        self.skills.retain(|other| other.name != skill.name);
        let index = self.skills.partition_point(|other| other.name < skill.name);
        self.skills.insert(index, skill);
    }

    /// Its "Use shared settings" switch. For a list, either way what it was given is
    /// forgotten: its copies stay as its own, and following again gives it Shared's afresh.
    pub fn set_own(&mut self, machine: MachineId, section: SharedSection, own: bool) {
        let sharing = self.sharing_mut(machine);
        sharing.own.insert(section, own);
        match section {
            SharedSection::Skills => sharing.skills_given.clear(),
            SharedSection::McpServers => sharing.mcp_servers_given.clear(),
            SharedSection::ThreadTitles | SharedSection::CleanupRules => {}
        }
    }
}

/// What making a machine's skills and MCP servers match Shared's takes, a request at a time,
/// besides the steps that only note what it has.
#[derive(Clone, Debug, PartialEq)]
pub enum ListStep {
    AddSkill(SharedSkill),
    DeleteSkill(String),
    /// A skill taken out of Shared that the machine doesn't have any more.
    ForgetSkill(String),
    SaveMcpServer {
        replacing: Option<String>,
        server: McpServer,
    },
    /// A server the machine has as Shared has it.
    KeepMcpServer(String),
    DeleteMcpServer(String),
    ForgetMcpServer(String),
}

/// The steps that give a machine following Shared its skills and MCP servers: what was taken
/// out of Shared goes, and what's new or changed replaces the machine's of its name.
pub fn list_steps(
    shared: &SharedMachineSettings,
    machine: MachineId,
    skills: &[Skill],
    mcp_servers: &[McpServer],
) -> Vec<ListStep> {
    let default = MachineSharing::default();
    let sharing = shared.sharing(machine).unwrap_or(&default);
    let mut steps = Vec::new();
    if !shared.sets_own(machine, SharedSection::Skills) {
        let has_skill = |name: &str| skills.iter().any(|skill| skill.name == name);
        for name in sharing.skills_given.keys() {
            if !shared.skills.iter().any(|skill| skill.name == *name) {
                steps.push(if has_skill(name) {
                    ListStep::DeleteSkill(name.clone())
                } else {
                    ListStep::ForgetSkill(name.clone())
                });
            }
        }
        for skill in &shared.skills {
            let is_current = sharing.skills_given.get(&skill.name) == Some(&skill.revision);
            match (has_skill(&skill.name), is_current) {
                (true, true) => {}
                (true, false) => {
                    steps.push(ListStep::DeleteSkill(skill.name.clone()));
                    steps.push(ListStep::AddSkill(skill.clone()));
                }
                (false, _) => steps.push(ListStep::AddSkill(skill.clone())),
            }
        }
    }
    if !shared.sets_own(machine, SharedSection::McpServers) {
        let listed = |name: &str| mcp_servers.iter().find(|server| server.name == name);
        for name in &sharing.mcp_servers_given {
            if !shared.mcp_servers.iter().any(|server| server.name == *name) {
                steps.push(match listed(name) {
                    Some(_) => ListStep::DeleteMcpServer(name.clone()),
                    None => ListStep::ForgetMcpServer(name.clone()),
                });
            }
        }
        for server in &shared.mcp_servers {
            match listed(&server.name) {
                Some(listed)
                    if listed.transport == server.transport && listed.enabled == server.enabled =>
                {
                    if !sharing.mcp_servers_given.contains(&server.name) {
                        steps.push(ListStep::KeepMcpServer(server.name.clone()));
                    }
                }
                listed => steps.push(ListStep::SaveMcpServer {
                    replacing: listed.map(|listed| listed.name.clone()),
                    // Which of the machine's accounts don't get it stays the machine's.
                    server: McpServer {
                        kept_off: listed
                            .map(|listed| listed.kept_off.clone())
                            .unwrap_or_default(),
                        ..server.clone()
                    },
                }),
            }
        }
    }
    steps
}

/// Where a shared skill's files are.
pub fn skill_folder(root: &Path, name: &str) -> PathBuf {
    root.join(name)
}

/// Keeps a skill folder's files as one of Shared's skills in `root`, named as its `SKILL.md`
/// says.
pub fn add_skill(root: &Path, files: &[SkillFile]) -> Result<SharedSkill> {
    let skill_file = files
        .iter()
        .find(|file| file.path == SKILL_FILE_NAME)
        .context("The folder has no SKILL.md.")?;
    let content = String::from_utf8(decode(&skill_file.data)?).context("reading SKILL.md")?;
    let frontmatter = parse_frontmatter(&content)?;
    validate_description(&frontmatter.description).map_err(anyhow::Error::msg)?;
    write_skill(root, &frontmatter.name, files)?;
    Ok(SharedSkill {
        name: frontmatter.name,
        description: frontmatter.description,
        revision: now_millis(),
    })
}

/// Create a Skill for Shared: a new `SKILL.md`, as Zed's form writes it.
pub fn create_skill(root: &Path, name: &str, description: &str, body: &str) -> Result<SharedSkill> {
    validate_name(name).map_err(anyhow::Error::msg)?;
    validate_description(description).map_err(anyhow::Error::msg)?;
    if body.trim().is_empty() {
        bail!("Body is required.");
    }
    let content = format_skill_file(name, description, body)?;
    let file = SkillFile {
        path: SKILL_FILE_NAME.to_string(),
        data: base64::engine::general_purpose::STANDARD.encode(content),
        executable: false,
    };
    write_skill(root, name, &[file])?;
    Ok(SharedSkill {
        name: name.to_string(),
        description: description.to_string(),
        revision: now_millis(),
    })
}

/// Writes the files in a hidden folder first, then moves it in place, so a machine is never
/// given a skill half written.
fn write_skill(root: &Path, name: &str, files: &[SkillFile]) -> Result<()> {
    let target = skill_folder(root, name);
    if target.exists() {
        bail!("A shared skill named \"{name}\" already exists. Pick a different name.");
    }
    let staging = root.join(format!(".adding-{name}-{}", now_millis()));
    let written = (|| {
        for file in files {
            anyhow::ensure!(
                !file.path.is_empty()
                    && Path::new(&file.path)
                        .components()
                        .all(|component| matches!(component, Component::Normal(_))),
                "{} isn't a path inside a skill's folder",
                file.path
            );
            let path = staging.join(&file.path);
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)
                    .with_context(|| format!("creating {}", parent.display()))?;
            }
            std::fs::write(&path, decode(&file.data)?)
                .with_context(|| format!("writing {}", path.display()))?;
            #[cfg(unix)]
            if file.executable {
                use std::os::unix::fs::PermissionsExt as _;
                std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))
                    .with_context(|| format!("making {} executable", path.display()))?;
            }
        }
        std::fs::rename(&staging, &target)
            .with_context(|| format!("moving the skill to {}", target.display()))
    })();
    if written.is_err() {
        std::fs::remove_dir_all(&staging).ok();
    }
    written
}

/// Deletes a shared skill's files.
pub fn delete_skill(root: &Path, name: &str) -> Result<()> {
    anyhow::ensure!(
        validate_name(name).is_ok(),
        "There's no shared skill named \"{name}\"."
    );
    let folder = skill_folder(root, name);
    match std::fs::remove_dir_all(&folder) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error).with_context(|| format!("removing {}", folder.display())),
    }
}

fn decode(data: &str) -> Result<Vec<u8>> {
    base64::engine::general_purpose::STANDARD
        .decode(data)
        .context("decoding a skill's file")
}

fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_millis() as u64)
}

/// The files of a skill's folder, as the server takes them. A clone's `.git` and Finder's
/// `.DS_Store` aren't part of the skill, and linked folders aren't followed, so a link back up
/// can't loop.
pub fn read_skill_folder(folder: &Path) -> Result<Vec<SkillFile>> {
    let mut files = Vec::new();
    let mut size = 0;
    let mut pending = vec![PathBuf::new()];
    while let Some(relative) = pending.pop() {
        let directory = folder.join(&relative);
        let entries = std::fs::read_dir(&directory)
            .with_context(|| format!("reading {}", directory.display()))?;
        for entry in entries {
            let entry = entry.with_context(|| format!("reading {}", directory.display()))?;
            let name = entry.file_name();
            if name == ".git" || name == ".DS_Store" {
                continue;
            }
            let path = entry.path();
            let metadata =
                std::fs::metadata(&path).with_context(|| format!("reading {}", path.display()))?;
            if metadata.is_dir() {
                if !entry.file_type().is_ok_and(|kind| kind.is_symlink()) {
                    pending.push(relative.join(&name));
                }
                continue;
            }
            if !metadata.is_file() {
                continue;
            }
            size += metadata.len() as usize;
            anyhow::ensure!(size <= MAX_FOLDER_SIZE, FOLDER_TOO_LARGE);
            let data =
                std::fs::read(&path).with_context(|| format!("reading {}", path.display()))?;
            files.push(SkillFile {
                path: slash_path(&relative.join(&name))?,
                data: base64::engine::general_purpose::STANDARD.encode(data),
                executable: is_executable(&metadata),
            });
        }
    }
    files.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(files)
}

/// `path`'s names with `/` between them, as the server takes them on any machine.
fn slash_path(path: &Path) -> Result<String> {
    let names = path
        .components()
        .map(|component| match component {
            Component::Normal(name) => name
                .to_str()
                .with_context(|| format!("{} isn't a name agentZ can send", path.display())),
            _ => anyhow::bail!("{} isn't a path in the folder", path.display()),
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(names.join("/"))
}

#[cfg(unix)]
fn is_executable(metadata: &std::fs::Metadata) -> bool {
    use std::os::unix::fs::PermissionsExt as _;
    metadata.permissions().mode() & 0o111 != 0
}

#[cfg(not(unix))]
fn is_executable(_: &std::fs::Metadata) -> bool {
    false
}

#[cfg(test)]
mod tests {
    use agentz_protocol::mcp_servers::McpTransport;

    use super::*;

    fn skill(name: &str, revision: u64) -> SharedSkill {
        SharedSkill {
            name: name.into(),
            description: format!("{name}."),
            revision,
        }
    }

    fn listed_skill(name: &str) -> Skill {
        Skill {
            name: name.into(),
            description: format!("{name}."),
            path: format!("/tmp/skills/{name}/SKILL.md").into(),
            skipped: Vec::new(),
            kept_off: Vec::new(),
        }
    }

    fn server(name: &str, command: &str) -> McpServer {
        McpServer {
            name: name.into(),
            enabled: true,
            transport: McpTransport::Local {
                command: command.into(),
                args: Vec::new(),
                env: Vec::new(),
            },
            kept_off: Vec::new(),
        }
    }

    #[test]
    fn a_following_machine_gets_what_shared_has_and_loses_what_it_lost() {
        let devbox = MachineId::Remote(1);
        let mut shared = SharedMachineSettings {
            skills: vec![skill("review", 2), skill("notes", 1)],
            mcp_servers: vec![server("github", "npx")],
            ..SharedMachineSettings::default()
        };
        let sharing = shared.sharing_mut(devbox);
        sharing.skills_given = [("review".to_string(), 1), ("old".to_string(), 1)].into();
        sharing.mcp_servers_given = ["linear".to_string()].into();
        let skills = [
            listed_skill("review"),
            listed_skill("old"),
            listed_skill("mine"),
        ];
        let mut github = server("github", "node");
        github.kept_off = vec![agentz_protocol::accounts::AgentAccount {
            agent_id: agentz_protocol::agents::AgentId::new("mock"),
            account: None,
        }];
        let servers = [
            github.clone(),
            server("linear", "npx"),
            server("own", "npx"),
        ];

        let steps = list_steps(&shared, devbox, &skills, &servers);
        assert_eq!(
            steps,
            [
                ListStep::DeleteSkill("old".into()),
                // A newer revision replaces the machine's copy.
                ListStep::DeleteSkill("review".into()),
                ListStep::AddSkill(skill("review", 2)),
                ListStep::AddSkill(skill("notes", 1)),
                ListStep::DeleteMcpServer("linear".into()),
                ListStep::SaveMcpServer {
                    replacing: Some("github".into()),
                    server: McpServer {
                        kept_off: github.kept_off.clone(),
                        ..server("github", "npx")
                    },
                },
            ]
        );

        // Once done, nothing's left to do, and the machine's own stay.
        let mut sharing = shared.sharing(devbox).cloned().unwrap_or_default();
        for step in &steps {
            sharing.record(step);
        }
        *shared.sharing_mut(devbox) = sharing;
        let skills = [
            listed_skill("review"),
            listed_skill("notes"),
            listed_skill("mine"),
        ];
        let servers = [
            McpServer {
                kept_off: github.kept_off,
                ..server("github", "npx")
            },
            server("own", "npx"),
        ];
        assert_eq!(list_steps(&shared, devbox, &skills, &servers), []);

        // Setting its own stops it all, and keeps what it has.
        shared.set_own(devbox, SharedSection::Skills, true);
        shared.set_own(devbox, SharedSection::McpServers, true);
        shared.skills.clear();
        shared.mcp_servers.clear();
        assert_eq!(list_steps(&shared, devbox, &skills, &servers), []);
        assert!(shared.sets_own(devbox, SharedSection::Skills));
    }

    #[test]
    fn titles_and_cleanup_are_undecided_on_a_machine_saved_before_shared() {
        let mut shared = SharedMachineSettings::default();
        let devbox = MachineId::Remote(1);
        assert_eq!(shared.decision(devbox, SharedSection::ThreadTitles), None);
        assert_eq!(shared.decision(devbox, SharedSection::Skills), Some(false));
        *shared.sharing_mut(MachineId::Remote(2)) = MachineSharing::following();
        assert_eq!(
            shared.decision(MachineId::Remote(2), SharedSection::CleanupRules),
            Some(false)
        );
    }

    #[test]
    fn shared_skills_are_kept_in_their_folders() -> Result<()> {
        let root = tempfile::tempdir()?;
        let created = create_skill(root.path(), "review", "Reviews a change.", "Read it.")?;
        assert_eq!(created.name, "review");
        let files = read_skill_folder(&skill_folder(root.path(), "review"))?;
        assert_eq!(files.len(), 1);
        let error = create_skill(root.path(), "review", "Again.", "Read it.")
            .expect_err("a second skill of the name");
        assert!(format!("{error:#}").contains("already exists"));

        let source = tempfile::tempdir()?;
        std::fs::write(
            source.path().join("SKILL.md"),
            "---\nname: notes\ndescription: Writes notes.\n---\nBody.\n",
        )?;
        std::fs::create_dir_all(source.path().join("scripts"))?;
        std::fs::write(source.path().join("scripts/run.sh"), "#!/bin/sh\n")?;
        let added = add_skill(root.path(), &read_skill_folder(source.path())?)?;
        assert_eq!(
            (added.name.as_str(), added.description.as_str()),
            ("notes", "Writes notes.")
        );
        assert!(root.path().join("notes/scripts/run.sh").exists());

        delete_skill(root.path(), "notes")?;
        assert!(!root.path().join("notes").exists());
        Ok(())
    }

    #[test]
    fn a_folder_is_read_without_git_finder_or_linked_folders() -> Result<()> {
        let folder = tempfile::tempdir()?;
        let outside = tempfile::tempdir()?;
        std::fs::write(folder.path().join("SKILL.md"), "---\nname: x\n---\n")?;
        std::fs::create_dir_all(folder.path().join("scripts"))?;
        std::fs::write(folder.path().join("scripts/run.sh"), "#!/bin/sh\n")?;
        std::fs::create_dir_all(folder.path().join(".git"))?;
        std::fs::write(folder.path().join(".git/config"), "")?;
        std::fs::write(folder.path().join(".DS_Store"), "")?;
        std::fs::write(outside.path().join("secret.md"), "")?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            std::fs::set_permissions(
                folder.path().join("scripts/run.sh"),
                std::fs::Permissions::from_mode(0o755),
            )?;
            std::os::unix::fs::symlink(outside.path(), folder.path().join("linked"))?;
        }

        let files = read_skill_folder(folder.path())?;
        let paths: Vec<&str> = files.iter().map(|file| file.path.as_str()).collect();
        assert_eq!(paths, ["SKILL.md", "scripts/run.sh"]);
        assert!(!files[0].executable);
        assert_eq!(files[1].executable, cfg!(unix));
        assert_eq!(files[1].data, "IyEvYmluL3NoCg==");
        Ok(())
    }
}
