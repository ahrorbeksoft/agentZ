//! agentZ's own skills (design/accounts decisions.md §17, §18): folders with a `SKILL.md`, kept
//! in `skills/` in the server's data directory and linked into every agent's accounts.

use std::path::PathBuf;

use anyhow::{Context as _, Result, bail};
use serde::{Deserialize, Serialize};

use crate::accounts::{AccountId, AgentAccount};
use crate::agents::AgentId;

pub const SKILL_FILE_NAME: &str = "SKILL.md";
/// Zed's limits.
pub const MAX_SKILL_FILE_SIZE: usize = 100 * 1024;
pub const MAX_NAME_LEN: usize = 64;
pub const MAX_DESCRIPTION_LEN: usize = 1024;
/// What Add from Folder… sends at most, well inside a message once in base64.
pub const MAX_FOLDER_SIZE: usize = 32 * 1024 * 1024;
pub const FOLDER_TOO_LARGE: &str = "The folder is larger than 32 MB.";

/// One of agentZ's skills, as Settings › Skills lists it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Skill {
    /// Its folder's name, which agents load it by.
    pub name: String,
    pub description: String,
    /// Its `SKILL.md`, on the server's machine.
    pub path: PathBuf,
    /// The accounts that keep a skill of their own by this name instead.
    #[serde(default)]
    pub skipped: Vec<SkippedSkill>,
    /// The accounts its accounts menu keeps it off, so an account added later loads it.
    #[serde(default)]
    pub kept_off: Vec<AgentAccount>,
}

/// An account that doesn't load one of agentZ's skills, since its agent has its own skill of
/// that name.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SkippedSkill {
    pub agent_id: AgentId,
    /// `None` is the External account.
    pub account: Option<AccountId>,
    /// The agent's own skill.
    pub own: PathBuf,
}

/// A file of a skill's folder, sent from the client's machine ([`crate::Request::AddSkill`]).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SkillFile {
    /// Relative to the folder, with `/` between names.
    pub path: String,
    /// Its bytes, in base64.
    pub data: String,
    #[serde(default)]
    pub executable: bool,
}

/// Zed's rules for a skill's name.
pub fn validate_name(name: &str) -> Result<(), &'static str> {
    if name.is_empty() {
        return Err("Skill name cannot be empty");
    }
    if name.len() > MAX_NAME_LEN {
        return Err("Skill name must be at most 64 characters");
    }
    if name.starts_with('-') || name.ends_with('-') {
        return Err("Skill name must not start or end with a hyphen");
    }
    if !name
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    {
        return Err("Skill name must contain only lowercase letters, numbers, and hyphens");
    }
    Ok(())
}

/// Zed's rules for a description written by its form.
pub fn validate_description(description: &str) -> Result<(), &'static str> {
    if description.trim().is_empty() {
        return Err("Skill description cannot be empty");
    }
    if description.chars().count() > MAX_DESCRIPTION_LEN {
        return Err("Skill description must be at most 1024 characters");
    }
    Ok(())
}

/// A `SKILL.md`'s frontmatter, as Zed reads and writes it.
#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct Frontmatter {
    pub name: String,
    pub description: String,
}

/// Zed's `extract_frontmatter`: the YAML between the first `---` line and the `---` line that
/// ends it, with Zed's checks of the name and description.
pub fn parse_frontmatter(content: &str) -> Result<Frontmatter> {
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
pub fn format_skill_file(name: &str, description: &str, body: &str) -> Result<String> {
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
