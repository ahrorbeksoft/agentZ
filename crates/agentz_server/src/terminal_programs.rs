//! Agent CLIs a terminal thread can run, found on the server's `PATH`. The list and
//! executables are herdr's (`interactive_agent_executable` in `src/detect/mod.rs`).

use std::path::{Path, PathBuf};

use agentz_protocol::terminal::TerminalProgram;

const AGENT_CLIS: &[(&str, &str)] = &[
    ("claude", "Claude Code"),
    ("codex", "Codex"),
    ("gemini", "Gemini CLI"),
    ("opencode", "OpenCode"),
    ("droid", "Droid"),
    ("amp", "Amp"),
    ("cursor-agent", "Cursor Agent"),
    ("copilot", "GitHub Copilot"),
    ("devin", "Devin"),
    ("agy", "Antigravity"),
    ("cline", "Cline"),
    ("kimi", "Kimi Code"),
    ("kiro-cli", "Kiro"),
    ("grok", "Grok"),
    ("hermes", "Hermes"),
    ("kilo", "Kilo Code"),
    ("qodercli", "Qoder"),
    ("qwen", "Qwen Code"),
    ("letta", "Letta Code"),
    ("pi", "Pi"),
    ("maki", "Maki"),
    ("muse", "Muse"),
];

/// The agent CLIs on `PATH`, in herdr's order.
pub(crate) fn find_on_path() -> Vec<TerminalProgram> {
    let Some(path) = std::env::var_os("PATH") else {
        return Vec::new();
    };
    let dirs: Vec<PathBuf> = std::env::split_paths(&path).collect();
    AGENT_CLIS
        .iter()
        .filter_map(|(command, label)| {
            let path = dirs
                .iter()
                .map(|dir| dir.join(command))
                .find(|path| is_executable(path))?;
            Some(TerminalProgram {
                command: command.to_string(),
                label: label.to_string(),
                path,
            })
        })
        .collect()
}

#[cfg(unix)]
fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt as _;
    std::fs::metadata(path)
        .is_ok_and(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
}

#[cfg(not(unix))]
fn is_executable(path: &Path) -> bool {
    path.is_file()
}
