//! Running git for checkpoints and workspaces.

use std::path::Path;
use std::process::Stdio;

use anyhow::{Context as _, Result, anyhow};
use tokio::io::AsyncReadExt as _;
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
    let output = command(cwd, args, env)
        .output()
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
    let mut child = command(cwd, args, &[]).spawn().context("running git")?;
    let mut stdout = child.stdout.take().context("git's output")?;
    let mut output = Vec::new();
    let mut buffer = vec![0; 64 * 1024];
    let mut truncated = false;
    loop {
        let read = stdout.read(&mut buffer).await?;
        if read == 0 {
            break;
        }
        if output.len() + read > limit {
            output.extend_from_slice(&buffer[..limit - output.len()]);
            truncated = true;
            break;
        }
        output.extend_from_slice(&buffer[..read]);
    }
    if truncated {
        child.kill().await.ok();
        // Up to the last whole line, so no line is cut in half.
        if let Some(end) = output.iter().rposition(|byte| *byte == b'\n') {
            output.truncate(end + 1);
        }
    } else {
        let result = child.wait_with_output().await?;
        if !result.status.success() {
            return Err(anyhow!(
                "git diff failed: {}",
                String::from_utf8_lossy(&result.stderr).trim()
            ));
        }
    }
    Ok((String::from_utf8_lossy(&output).into_owned(), truncated))
}
