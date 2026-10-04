//! The agent CLIs agent detection names. The list and executables are herdr's
//! (`interactive_agent_executable` in `src/detect/mod.rs`).

/// Each CLI's command, its name, and the ACP Registry agent whose icon stands for it.
const AGENT_CLIS: &[(&str, &str, Option<&str>)] = &[
    ("claude", "Claude Code", Some("claude-acp")),
    ("codex", "Codex", Some("codex-acp")),
    ("gemini", "Gemini CLI", Some("gemini")),
    ("opencode", "OpenCode", Some("opencode")),
    ("droid", "Droid", Some("factory-droid")),
    ("amp", "Amp", Some("amp-acp")),
    ("cursor-agent", "Cursor Agent", Some("cursor")),
    ("copilot", "GitHub Copilot", Some("github-copilot-cli")),
    ("devin", "Devin", Some("devin")),
    ("agy", "Antigravity", Some("antigravity-acp")),
    ("cline", "Cline", Some("cline")),
    ("kimi", "Kimi Code", Some("kimi")),
    ("kiro-cli", "Kiro", None),
    ("grok", "Grok", Some("grok-build")),
    ("hermes", "Hermes", None),
    ("kilo", "Kilo Code", Some("kilo")),
    ("qodercli", "Qoder", Some("qoder")),
    ("qwen", "Qwen Code", Some("qwen-code")),
    ("letta", "Letta Code", None),
    ("pi", "Pi", Some("pi-acp")),
    ("maki", "Maki", None),
    ("muse", "Muse", None),
];

/// The name of an agent CLI, such as "Claude Code" for `claude`.
pub(crate) fn label(command: &str) -> Option<&'static str> {
    AGENT_CLIS
        .iter()
        .find(|(name, _, _)| *name == command)
        .map(|(_, label, _)| *label)
}

/// The ACP Registry agent whose icon stands for an agent CLI, such as `claude-acp` for `claude`.
pub(crate) fn registry_agent(command: &str) -> Option<&'static str> {
    AGENT_CLIS
        .iter()
        .find(|(name, _, _)| *name == command)
        .and_then(|(_, _, agent)| *agent)
}
