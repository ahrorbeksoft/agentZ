//! agentZ's own skills (design/accounts decisions.md §17): folders with a `SKILL.md` in
//! `skills/` in the data directory. Each is linked on its own into the skills folder of every
//! account of the agents agentZ can run on accounts, so the agents' own skills stay beside
//! them, and skipped where the agent has a skill of its own by that name. Only links into
//! `skills/` are ever removed.

use std::ffi::OsStr;
use std::path::{Component, Path, PathBuf};

use agentz_protocol::accounts::AccountId;
use agentz_protocol::agents::AgentId;
use agentz_protocol::skills::{
    FOLDER_TOO_LARGE, MAX_FOLDER_SIZE, Skill, SkillFile, SkippedSkill, validate_description,
    validate_name,
};
use anyhow::{Context as _, Result, bail};
use base64::Engine as _;
use serde::{Deserialize, Serialize};

pub const SKILL_FILE_NAME: &str = "SKILL.md";
/// Zed's limit.
const MAX_SKILL_FILE_SIZE: usize = 100 * 1024;

/// Where agentZ keeps its skills.
pub fn folder(data_dir: &Path) -> PathBuf {
    data_dir.join("skills")
}

/// A `SKILL.md`'s frontmatter, as Zed reads and writes it.
#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct Frontmatter {
    name: String,
    description: String,
}

/// The skills in `folder`, by name, with no account skipping them yet. A folder whose
/// `SKILL.md` can't be read is left out, as Zed leaves it out.
pub fn list(folder: &Path) -> Result<Vec<Skill>> {
    let entries = match std::fs::read_dir(folder) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error).with_context(|| format!("reading {}", folder.display())),
    };
    let mut skills = Vec::new();
    for entry in entries {
        let entry = entry.with_context(|| format!("reading {}", folder.display()))?;
        let Some(name) = entry.file_name().to_str().map(str::to_string) else {
            continue;
        };
        // Adding a skill writes it in a hidden folder first.
        if name.starts_with('.') || !entry.path().is_dir() {
            continue;
        }
        let path = entry.path().join(SKILL_FILE_NAME);
        match read_frontmatter(&path) {
            Ok(frontmatter) => skills.push(Skill {
                name,
                description: frontmatter.description,
                path,
                skipped: Vec::new(),
            }),
            Err(error) => log::warn!("leaving out the skill in {}: {error:#}", path.display()),
        }
    }
    skills.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(skills)
}

fn read_frontmatter(path: &Path) -> Result<Frontmatter> {
    let content =
        std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    parse_frontmatter(&content)
}

/// Zed's `extract_frontmatter`: the YAML between the first `---` line and the `---` line that
/// ends it, with Zed's checks of the name and description.
fn parse_frontmatter(content: &str) -> Result<Frontmatter> {
    if content.len() > MAX_SKILL_FILE_SIZE {
        bail!(
            "SKILL.md file exceeds maximum size of {}KB",
            MAX_SKILL_FILE_SIZE / 1024
        );
    }
    let content = content.trim_start();
    if !content.starts_with("---") {
        bail!("SKILL.md must start with YAML frontmatter (---)");
    }
    let bytes = content.as_bytes();
    let mut ends = Vec::new();
    for (index, byte) in bytes.iter().enumerate() {
        let line_start = index + 1;
        if *byte != b'\n' || bytes.get(line_start..line_start + 3) != Some(b"---") {
            continue;
        }
        let after = line_start + 3;
        match &bytes[after..] {
            [] => ends.push(after),
            [b'\n', ..] => ends.push(after + 1),
            [b'\r', b'\n', ..] => ends.push(after + 2),
            _ => {}
        }
    }
    if ends.is_empty() {
        bail!("SKILL.md missing closing frontmatter delimiter (---)");
    }
    // A `---` line can sit inside a quoted value, so the first end that parses is the real one.
    let mut last_error = None;
    for end in ends {
        let Some(document) = serde_yaml_ng::Deserializer::from_str(&content[..end]).next() else {
            continue;
        };
        match Frontmatter::deserialize(document) {
            Ok(frontmatter) => {
                validate_name(&frontmatter.name).map_err(anyhow::Error::msg)?;
                if frontmatter.description.trim().is_empty() {
                    bail!("Skill description cannot be empty");
                }
                return Ok(frontmatter);
            }
            Err(error) => last_error = Some(anyhow::Error::new(error)),
        }
    }
    Err(last_error
        .unwrap_or_else(|| anyhow::anyhow!("could not parse YAML frontmatter"))
        .context("Invalid YAML frontmatter"))
}

/// Zed's `format_skill_file`.
fn format_skill_file(name: &str, description: &str, body: &str) -> Result<String> {
    let frontmatter = serde_yaml_ng::to_string(&Frontmatter {
        name: name.to_string(),
        description: description.to_string(),
    })
    .context("failed to serialize skill frontmatter as YAML")?;
    let mut content = format!("---\n{frontmatter}---\n");
    let body = body.trim();
    if !body.is_empty() {
        content.push('\n');
        content.push_str(body);
        content.push('\n');
    }
    Ok(content)
}

/// Add from Folder…: `files` as a skill in `folder`, named as their `SKILL.md` says. Returns
/// its name.
pub fn add(folder: &Path, files: &[SkillFile]) -> Result<String> {
    let size: usize = files.iter().map(|file| file.data.len() / 4 * 3).sum();
    anyhow::ensure!(size <= MAX_FOLDER_SIZE, FOLDER_TOO_LARGE);
    let skill_file = files
        .iter()
        .find(|file| file.path == SKILL_FILE_NAME)
        .context("The folder has no SKILL.md.")?;
    let content = String::from_utf8(decode(&skill_file.data)?).context("reading SKILL.md")?;
    let frontmatter = parse_frontmatter(&content)?;
    validate_description(&frontmatter.description).map_err(anyhow::Error::msg)?;
    write_skill(folder, &frontmatter.name, |staging| {
        for file in files {
            let path = inside(staging, &file.path)?;
            write_file(&path, &decode(&file.data)?, file.executable)?;
        }
        Ok(())
    })?;
    Ok(frontmatter.name)
}

/// Create a Skill: a skill in `folder` with a new `SKILL.md`, as Zed's form writes it.
pub fn create(folder: &Path, name: &str, description: &str, body: &str) -> Result<()> {
    validate_name(name).map_err(anyhow::Error::msg)?;
    validate_description(description).map_err(anyhow::Error::msg)?;
    if body.trim().is_empty() {
        bail!("Body is required.");
    }
    let content = format_skill_file(name, description, body)?;
    write_skill(folder, name, |staging| {
        write_file(&staging.join(SKILL_FILE_NAME), content.as_bytes(), false)
    })
}

/// Writes a skill's files into a hidden folder, then moves it in place, so a skill is never
/// linked half written.
fn write_skill(folder: &Path, name: &str, write: impl FnOnce(&Path) -> Result<()>) -> Result<()> {
    let target = folder.join(name);
    match std::fs::symlink_metadata(&target) {
        Ok(metadata) if metadata.is_dir() => bail!(
            "A skill named \"{name}\" already exists at {}. Pick a different name.",
            target.display()
        ),
        Ok(_) => bail!(
            "A file (not a skill directory) already exists at {}. Delete it or pick a different \
             skill name.",
            target.display()
        ),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(error).with_context(|| format!("checking {}", target.display()));
        }
    }
    let staging = folder.join(format!(".adding-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&staging).with_context(|| format!("creating {}", staging.display()))?;
    let written = write(&staging).and_then(|()| {
        std::fs::rename(&staging, &target)
            .with_context(|| format!("moving the skill to {}", target.display()))
    });
    if written.is_err() {
        std::fs::remove_dir_all(&staging).ok();
    }
    written
}

/// Deletes the skill named `name` in `folder`. Its links go with the next sync.
pub fn delete(folder: &Path, name: &str) -> Result<()> {
    let path = inside(folder, name)?;
    anyhow::ensure!(
        !name.starts_with('.') && !name.contains('/'),
        "There's no skill named \"{name}\"."
    );
    let metadata = match std::fs::symlink_metadata(&path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            bail!("There's no skill named \"{name}\".")
        }
        Err(error) => return Err(error).with_context(|| format!("checking {}", path.display())),
    };
    if metadata.is_dir() {
        std::fs::remove_dir_all(&path)
    } else {
        std::fs::remove_file(&path)
    }
    .with_context(|| format!("removing {}", path.display()))
}

fn decode(data: &str) -> Result<Vec<u8>> {
    base64::engine::general_purpose::STANDARD
        .decode(data)
        .context("decoding a skill's file")
}

/// `path` in `folder`. The client names these paths, so one must not reach outside it.
fn inside(folder: &Path, path: &str) -> Result<PathBuf> {
    anyhow::ensure!(
        !path.is_empty()
            && Path::new(path)
                .components()
                .all(|component| matches!(component, Component::Normal(_))),
        "{path} isn't a path inside a skill's folder"
    );
    Ok(folder.join(path))
}

fn write_file(path: &Path, contents: &[u8], executable: bool) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("creating {}", parent.display()))?;
    }
    std::fs::write(path, contents).with_context(|| format!("writing {}", path.display()))?;
    #[cfg(unix)]
    if executable {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))
            .with_context(|| format!("making {} executable", path.display()))?;
    }
    #[cfg(not(unix))]
    let _ = executable;
    Ok(())
}

/// Where one account's agent loads skills from.
#[derive(Clone, Debug, PartialEq)]
pub struct SkillTarget {
    pub agent_id: AgentId,
    /// `None` is the External account.
    pub account: Option<AccountId>,
    /// The folder agentZ links its skills into.
    pub folder: PathBuf,
    /// The agent's other skills folders, in its home and outside it.
    pub other_folders: Vec<PathBuf>,
    /// Whether the account loads agentZ's skills. Its agent may not be installed, or the
    /// External account not listed; then agentZ's links are only removed.
    pub loads: bool,
}

/// Links each of `skills` (in `folder`) into each target, except where the agent has a skill of
/// its own by that name or already loads ours from another of its folders, and removes
/// agentZ's links that aren't wanted any more. Returns the skills with the accounts that
/// skipped them.
pub fn sync(folder: &Path, mut skills: Vec<Skill>, targets: &[SkillTarget]) -> Vec<Skill> {
    // Folders other agents read too (Claude's, which Devin reads) go first, so those agents
    // see the links they end up with.
    let mut ordered: Vec<&SkillTarget> = targets.iter().collect();
    ordered.sort_by_key(|target| {
        !targets
            .iter()
            .any(|other| other.other_folders.contains(&target.folder))
    });
    for target in ordered {
        let skipped = match sync_target(folder, &skills, target) {
            Ok(skipped) => skipped,
            Err(error) => {
                log::error!("linking skills into {}: {error:#}", target.folder.display());
                continue;
            }
        };
        for (index, own) in skipped {
            skills[index].skipped.push(SkippedSkill {
                agent_id: target.agent_id.clone(),
                account: target.account,
                own,
            });
        }
    }
    skills
}

/// Returns the skills the target skips, by index, with the agent's own skill of the name.
fn sync_target(
    folder: &Path,
    skills: &[Skill],
    target: &SkillTarget,
) -> Result<Vec<(usize, PathBuf)>> {
    let ours = |path: &Path| {
        std::fs::read_link(path)
            .ok()
            .filter(|to| to.parent() == Some(folder))
    };
    match std::fs::read_dir(&target.folder) {
        Ok(entries) => {
            for entry in entries {
                let link = entry
                    .with_context(|| format!("reading {}", target.folder.display()))?
                    .path();
                let Some(to) = ours(&link) else {
                    continue;
                };
                let is_wanted = target.loads
                    && link.file_name() == to.file_name()
                    && skills
                        .iter()
                        .any(|skill| to.file_name() == Some(OsStr::new(&skill.name)));
                if !is_wanted {
                    remove_link(&link)?;
                }
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(error).with_context(|| format!("reading {}", target.folder.display()));
        }
    }
    if !target.loads {
        return Ok(Vec::new());
    }
    let mut skipped = Vec::new();
    for (index, skill) in skills.iter().enumerate() {
        let link = target.folder.join(&skill.name);
        // agentZ's links elsewhere (an External account's, which another agent reads too)
        // aren't the agent's own.
        let own = std::iter::once(&target.folder)
            .chain(&target.other_folders)
            .map(|other| other.join(&skill.name))
            .find(|path| std::fs::symlink_metadata(path).is_ok() && ours(path).is_none());
        let to = folder.join(&skill.name);
        // Linked into a folder it reads anyway, it would load the skill twice.
        let reached = target
            .other_folders
            .iter()
            .any(|other| ours(&other.join(&skill.name)).as_ref() == Some(&to));
        if own.is_some() || reached {
            if ours(&link).is_some() {
                remove_link(&link)?;
            }
            if let Some(own) = own {
                skipped.push((index, own));
            }
            continue;
        }
        if ours(&link).is_some() {
            continue;
        }
        std::fs::create_dir_all(&target.folder)
            .with_context(|| format!("creating {}", target.folder.display()))?;
        #[cfg(unix)]
        std::os::unix::fs::symlink(&to, &link)
            .with_context(|| format!("linking {} to {}", link.display(), to.display()))?;
        #[cfg(not(unix))]
        bail!("can't link {} to {}", link.display(), to.display());
    }
    Ok(skipped)
}

fn remove_link(link: &Path) -> Result<()> {
    std::fs::remove_file(link).with_context(|| format!("removing {}", link.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(path: &str, contents: &str) -> SkillFile {
        SkillFile {
            path: path.into(),
            data: base64::engine::general_purpose::STANDARD.encode(contents),
            executable: false,
        }
    }

    fn names(skills: &[Skill]) -> Vec<&str> {
        skills.iter().map(|skill| skill.name.as_str()).collect()
    }

    /// The agents skipping the skill `name`, with their own skill of the name.
    fn skipped_by(skills: &[Skill], name: &str) -> Vec<(String, PathBuf)> {
        skills
            .iter()
            .find(|skill| skill.name == name)
            .expect("listed")
            .skipped
            .iter()
            .map(|skipped| (skipped.agent_id.0.to_string(), skipped.own.clone()))
            .collect()
    }

    #[test]
    fn reads_the_frontmatter_as_zed_does() {
        let frontmatter = parse_frontmatter(
            "---\nname: review\ndescription: >\n  Review a diff\n  for bugs.\n---\n\nBody.\n",
        )
        .expect("parse");
        assert_eq!(frontmatter.name, "review");
        assert_eq!(frontmatter.description, "Review a diff for bugs.\n");

        // Only a line of exactly `---` ends it.
        let frontmatter =
            parse_frontmatter("---\nname: notes\ndescription: \"---trailing\"\n---\nBody.\n")
                .expect("parse");
        assert_eq!(frontmatter.description, "---trailing");

        for (content, error) in [
            (
                "---\nname: foo\ndescription: bar\n----\nBody.\n",
                "missing closing frontmatter",
            ),
            ("No frontmatter.", "must start with YAML frontmatter"),
            ("---\nname: review\n", "missing closing frontmatter"),
            (
                "---\nname: Review\ndescription: x\n---\n",
                "only lowercase letters",
            ),
            (
                "---\nname: review\ndescription: ' '\n---\n",
                "cannot be empty",
            ),
        ] {
            let message = format!("{:#}", parse_frontmatter(content).expect_err(content));
            assert!(message.contains(error), "{content}: {message}");
        }
    }

    #[test]
    fn skills_are_added_created_and_deleted() {
        let dir = tempfile::tempdir().expect("temp dir");
        let folder = dir.path().join("skills");
        assert!(list(&folder).expect("list").is_empty());

        let mut script = file("scripts/run.sh", "#!/bin/sh\necho hi\n");
        script.executable = true;
        let files = vec![
            file(
                SKILL_FILE_NAME,
                "---\nname: release-notes\ndescription: Write release notes.\n---\nSteps.\n",
            ),
            script,
        ];
        assert_eq!(add(&folder, &files).expect("add"), "release-notes");
        let added = folder.join("release-notes");
        assert_eq!(
            std::fs::read_to_string(added.join("scripts/run.sh")).expect("read"),
            "#!/bin/sh\necho hi\n"
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            let mode = std::fs::metadata(added.join("scripts/run.sh"))
                .expect("metadata")
                .permissions()
                .mode();
            assert_eq!(mode & 0o111, 0o111);
        }
        let error = add(&folder, &files).expect_err("added twice");
        assert!(format!("{error:#}").contains("already exists"), "{error:#}");
        let error = add(&folder, &[file("README.md", "")]).expect_err("no SKILL.md");
        assert_eq!(error.to_string(), "The folder has no SKILL.md.");
        let outside = [files[0].clone(), file("../outside.txt", "")];
        assert!(add(&folder, &outside).is_err());
        assert!(!dir.path().join("outside.txt").exists());

        create(
            &folder,
            "review",
            "Review a diff: bugs first.",
            "Read the diff.",
        )
        .expect("create");
        assert_eq!(
            std::fs::read_to_string(folder.join("review").join(SKILL_FILE_NAME)).expect("read"),
            "---\nname: review\ndescription: 'Review a diff: bugs first.'\n---\n\nRead the diff.\n"
        );
        let error = create(&folder, "Review", "x", "y").expect_err("bad name");
        assert_eq!(
            error.to_string(),
            "Skill name must contain only lowercase letters, numbers, and hyphens"
        );
        let error = create(&folder, "empty", "x", " ").expect_err("no body");
        assert_eq!(error.to_string(), "Body is required.");

        // Folders without a SKILL.md that reads, and failed adds, aren't skills.
        std::fs::create_dir_all(folder.join("broken")).expect("create");
        std::fs::write(
            folder.join("broken").join(SKILL_FILE_NAME),
            "no frontmatter",
        )
        .expect("write");
        let skills = list(&folder).expect("list");
        assert_eq!(names(&skills), ["release-notes", "review"]);
        assert_eq!(skills[1].description, "Review a diff: bugs first.");
        assert_eq!(skills[1].path, folder.join("review").join(SKILL_FILE_NAME));

        delete(&folder, "review").expect("delete");
        assert!(delete(&folder, "review").is_err());
        assert!(delete(&folder, "..").is_err());
        assert!(delete(&folder, "../skills").is_err());
        assert_eq!(names(&list(&folder).expect("list")), ["release-notes"]);
    }

    #[cfg(unix)]
    #[test]
    fn links_skills_beside_the_agents_own() {
        let dir = tempfile::tempdir().expect("temp dir");
        let folder = dir.path().join("skills");
        for name in ["review", "frontend-design", "release-notes"] {
            create(&folder, name, "A skill.", "Do it.").expect("create");
        }
        let target = |name: &str, other_folders: Vec<PathBuf>, loads: bool| SkillTarget {
            agent_id: AgentId::new(name.to_string()),
            account: None,
            folder: dir.path().join(name).join("skills"),
            other_folders,
            loads,
        };
        let shared = dir.path().join("shared/skills");
        // Claude has its own frontend-design, and a skill of its own beside.
        let claude = target("claude", Vec::new(), true);
        for own in ["frontend-design", "mine"] {
            std::fs::create_dir_all(claude.folder.join(own)).expect("create");
        }
        // Devin reads Claude's folder and the shared one too: Claude's frontend-design and the
        // shared one's review are its own, and it loads the rest through Claude's links.
        let devin = target("devin", vec![claude.folder.clone(), shared.clone()], true);
        std::fs::create_dir_all(shared.join("review")).expect("create");
        // Codex isn't installed, and had a link from before.
        let codex = target("codex", Vec::new(), false);
        std::fs::create_dir_all(&codex.folder).expect("create");
        std::os::unix::fs::symlink(folder.join("review"), codex.folder.join("review"))
            .expect("link");
        // Claude's folder is synced first wherever it's listed.
        let targets = [devin.clone(), claude.clone(), codex.clone()];

        let skills = sync(&folder, list(&folder).expect("list"), &targets);
        let linked = |target: &SkillTarget, name: &str| {
            std::fs::read_link(target.folder.join(name)).ok() == Some(folder.join(name))
        };
        assert!(linked(&claude, "review") && linked(&claude, "release-notes"));
        assert!(!linked(&claude, "frontend-design"));
        assert!(claude.folder.join("mine").is_dir());
        assert!(!devin.folder.exists());
        assert!(!codex.folder.join("review").exists());
        let skipped = |name: &str| skipped_by(&skills, name);
        assert_eq!(
            skipped("frontend-design"),
            [
                ("claude".into(), claude.folder.join("frontend-design")),
                ("devin".into(), claude.folder.join("frontend-design"))
            ]
        );
        assert_eq!(skipped("review"), [("devin".into(), shared.join("review"))]);
        assert!(skipped("release-notes").is_empty());

        // A skill deleted, Devin's own review gone, and Claude no longer loading ours: Devin
        // links what it no longer gets through Claude's folder, and Claude keeps only its own.
        delete(&folder, "release-notes").expect("delete");
        std::fs::remove_dir_all(shared.join("review")).expect("remove");
        let targets = [
            devin.clone(),
            SkillTarget {
                loads: false,
                ..claude.clone()
            },
        ];
        let skills = sync(&folder, list(&folder).expect("list"), &targets);
        assert!(linked(&devin, "review"));
        assert!(!devin.folder.join("release-notes").exists());
        assert!(!devin.folder.join("frontend-design").exists());
        assert!(skills.iter().all(|skill| skill.name != "release-notes"));
        assert!(skipped_by(&skills, "review").is_empty());
        assert_eq!(
            skipped_by(&skills, "frontend-design"),
            [("devin".into(), claude.folder.join("frontend-design"))]
        );
        let mut left: Vec<String> = std::fs::read_dir(&claude.folder)
            .expect("read")
            .map(|entry| {
                entry
                    .expect("entry")
                    .file_name()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();
        left.sort();
        assert_eq!(left, ["frontend-design", "mine"]);
    }
}
