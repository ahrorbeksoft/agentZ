//! agentZ's own skills (design/accounts decisions.md §17, §18): folders with a `SKILL.md`, kept
//! in `skills/` in the server's data directory and linked into every agent's accounts.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::accounts::AccountId;
use crate::agents::AgentId;

/// Zed's limits.
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
