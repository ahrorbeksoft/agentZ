//! Running git for checkpoints and workspaces.

use std::path::Path;
use std::process::Stdio;

use anyhow::{Context as _, Result, anyhow};
use tokio::process::Command;

/// Set by a git hook or an outer git command, they would point git at another repository.
const INHERITED_GIT_VARIABLES: [&str; 6] = [
    "GIT_DIR",
    "GIT_WORK_TREE",
    "GIT_INDEX_FILE",
    "GIT_COMMON_DIR",
    "GIT_OBJECT_DIRECTORY",
    "GIT_ALTERNATE_OBJECT_DIRECTORIES",
];

pub(crate) async fn is_repository(cwd: &Path) -> bool {
    git(cwd, &["rev-parse", "--is-inside-work-tree"], &[])
        .await
        .is_ok_and(|output| output.trim() == "true")
}

pub(crate) fn command(cwd: &Path, args: &[&str], env: &[(&str, &str)]) -> Command {
    let mut command = Command::new("git");
    command
        .args(args)
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    for variable in INHERITED_GIT_VARIABLES {
        command.env_remove(variable);
    }
    command.envs(env.iter().copied());
    command
}

/// Runs git and returns what it printed, or an error with its message.
pub(crate) async fn git(cwd: &Path, args: &[&str], env: &[(&str, &str)]) -> Result<String> {
    let output = child_process::output(&mut command(cwd, args, env))
        .await
        .context("running git")?;
    if !output.status.success() {
        return Err(anyhow!(
            "git {} failed: {}",
            args.iter()
                .find(|arg| !arg.starts_with('-') && !arg.contains('='))
                .unwrap_or(&""),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// Like [`git`], stopping after `limit` bytes. Returns whether the output was cut short.
pub(crate) async fn git_limited(cwd: &Path, args: &[&str], limit: usize) -> Result<(String, bool)> {
    let child = command(cwd, args, &[]).spawn().context("running git")?;
    let mut output = child_process::wait_with_limited_output(child, limit).await?;
    // Stopped for printing more.
    let truncated = output.status.is_none();
    if truncated {
        // Up to the last whole line, so no line is cut in half.
        if let Some(end) = output.stdout.iter().rposition(|byte| *byte == b'\n') {
            output.stdout.truncate(end + 1);
        }
    } else if !output.status.is_some_and(|status| status.success()) {
        return Err(anyhow!(
            "git diff failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok((
        String::from_utf8_lossy(&output.stdout).into_owned(),
        truncated,
    ))
}
