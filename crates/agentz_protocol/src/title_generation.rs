//! Generated thread titles (t3code's text generation): a machine's server can title threads
//! whose agent gives them none from their first message, with a coding agent's CLI installed
//! there. Off unless the user turns it on, and kept per machine in `title-generation.json`.

use serde::{Deserialize, Serialize};

/// A CLI that can write a title, run as t3code runs it for its text generation.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TitleProvider {
    /// t3code's default text generation provider.
    #[default]
    Codex,
    Claude,
    Antigravity,
    /// From a newer version.
    #[serde(untagged)]
    Unknown(serde_json::Value),
}

impl TitleProvider {
    pub const ALL: [TitleProvider; 3] = [
        TitleProvider::Codex,
        TitleProvider::Claude,
        TitleProvider::Antigravity,
    ];

    pub fn label(&self) -> &'static str {
        match self {
            TitleProvider::Codex => "Codex",
            TitleProvider::Claude => "Claude",
            TitleProvider::Antigravity => "Antigravity",
            TitleProvider::Unknown(_) => "Unknown",
        }
    }

    /// The program looked for on the machine's `PATH`.
    pub fn program(&self) -> Option<&'static str> {
        match self {
            TitleProvider::Codex => Some("codex"),
            TitleProvider::Claude => Some("claude"),
            TitleProvider::Antigravity => Some("agy"),
            TitleProvider::Unknown(_) => None,
        }
    }

    /// t3code's text generation model for it (`DEFAULT_TEXT_GENERATION_MODEL_BY_PROVIDER`).
    pub fn default_model(&self) -> &'static str {
        match self {
            TitleProvider::Codex => "gpt-6-luna",
            TitleProvider::Claude => "claude-haiku-4-5",
            TitleProvider::Antigravity | TitleProvider::Unknown(_) => ANTIGRAVITY_DEFAULT_MODEL,
        }
    }

    /// The reasoning effort used when none is chosen: t3code's `low` for Codex; Claude's is
    /// left to the CLI.
    pub fn default_effort(&self) -> Option<&'static str> {
        match self {
            TitleProvider::Codex => Some("low"),
            _ => None,
        }
    }
}

/// t3code's "keep the CLI's current model" for Antigravity: no `--model` is passed.
pub const ANTIGRAVITY_DEFAULT_MODEL: &str = "antigravity-default";

/// What the user chose, as the machine's server keeps it.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TitleGeneration {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub provider: TitleProvider,
    /// `None` is the provider's [`TitleProvider::default_model`].
    #[serde(default)]
    pub model: Option<String>,
    /// `None` is the provider's [`TitleProvider::default_effort`].
    #[serde(default)]
    pub effort: Option<String>,
}

impl TitleGeneration {
    pub fn model(&self) -> &str {
        self.model
            .as_deref()
            .unwrap_or_else(|| self.provider.default_model())
    }

    pub fn effort(&self) -> Option<&str> {
        self.effort
            .as_deref()
            .or_else(|| self.provider.default_effort())
    }
}

/// A model a provider offers for titles.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TitleModel {
    pub id: String,
    pub name: String,
    /// The reasoning efforts it takes, in order; empty when it takes none.
    #[serde(default)]
    pub efforts: Vec<String>,
}

/// A provider as the machine has it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TitleProviderInfo {
    pub provider: TitleProvider,
    /// Its CLI is on the machine's `PATH`.
    pub installed: bool,
    pub models: Vec<TitleModel>,
}

/// The machine's setting, with the providers it has, for Settings › General.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TitleGenerationState {
    #[serde(default)]
    pub settings: TitleGeneration,
    /// Every provider, installed or not; empty until the server has looked.
    #[serde(default)]
    pub providers: Vec<TitleProviderInfo>,
}

/// t3code's names for reasoning efforts (`REASONING_EFFORT_LABELS`).
pub fn effort_label(effort: &str) -> String {
    match effort {
        "none" => "None".to_string(),
        "minimal" => "Minimal".to_string(),
        "low" => "Low".to_string(),
        "medium" => "Medium".to_string(),
        "high" => "High".to_string(),
        "xhigh" => "Extra High".to_string(),
        "max" => "Max".to_string(),
        "ultra" => "Ultra".to_string(),
        other => other.to_string(),
    }
}
