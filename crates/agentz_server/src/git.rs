//! Running git for checkpoints and workspaces.

use std::path::Path;
use std::process::{ExitStatus, Stdio};
use std::time::Duration;

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

/// How long git's output is still read once git has exited. On macOS, which lacks `pipe2`,
/// Rust makes a pipe close-on-exec only after creating it, so a process started at that
/// moment inherits its end, and an agent then holds the output open for as long as it runs.
const OUTPUT_AFTER_EXIT: Duration = Duration::from_millis(200);

/// Runs git and returns what it printed, or an error with its message.
pub(crate) async fn git(cwd: &Path, args: &[&str], env: &[(&str, &str)]) -> Result<String> {
    let output = run(cwd, args, env, usize::MAX).await?;
    if !output.status.is_some_and(|status| status.success()) {
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
    let mut output = run(cwd, args, &[], limit).await?;
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

/// What git printed, and how it exited: `None` when it was stopped for printing more than
/// its limit.
struct Output {
    status: Option<ExitStatus>,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

enum Happened {
    Printed(std::io::Result<usize>),
    Complained(std::io::Result<usize>),
    Exited(std::io::Result<ExitStatus>),
    OutputHeld,
}

/// Runs git, reading its output only [`OUTPUT_AFTER_EXIT`] past its exit, and stopping it
/// once it has printed more than `limit` bytes.
async fn run(cwd: &Path, args: &[&str], env: &[(&str, &str)], limit: usize) -> Result<Output> {
    let mut child = command(cwd, args, env).spawn().context("running git")?;
    let mut stdout = child.stdout.take().context("git's output")?;
    let mut stderr = child.stderr.take().context("git's errors")?;
    let mut output = Output {
        status: None,
        stdout: Vec::new(),
        stderr: Vec::new(),
    };
    let mut stdout_buffer = vec![0; 64 * 1024];
    let mut stderr_buffer = vec![0; 8 * 1024];
    let mut reading_stdout = true;
    let mut reading_stderr = true;
    let mut read_until = tokio::time::Instant::now();
    while output.status.is_none() || reading_stdout || reading_stderr {
        let happened = tokio::select! {
            biased;
            read = stdout.read(&mut stdout_buffer), if reading_stdout => Happened::Printed(read),
            read = stderr.read(&mut stderr_buffer), if reading_stderr => {
                Happened::Complained(read)
            }
            status = child.wait(), if output.status.is_none() => Happened::Exited(status),
            () = tokio::time::sleep_until(read_until), if output.status.is_some() => {
                Happened::OutputHeld
            }
        };
        match happened {
            Happened::Printed(read) => match read? {
                0 => reading_stdout = false,
                read if output.stdout.len() + read > limit => {
                    let kept = limit - output.stdout.len();
                    output.stdout.extend_from_slice(&stdout_buffer[..kept]);
                    child.kill().await.ok();
                    output.status = None;
                    return Ok(output);
                }
                read => output.stdout.extend_from_slice(&stdout_buffer[..read]),
            },
            Happened::Complained(read) => match read? {
                0 => reading_stderr = false,
                read => output.stderr.extend_from_slice(&stderr_buffer[..read]),
            },
            Happened::Exited(status) => {
                output.status = Some(status?);
                read_until = tokio::time::Instant::now() + OUTPUT_AFTER_EXIT;
            }
            Happened::OutputHeld => break,
        }
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use super::*;

    /// A process that outlives git with its output open, as an agent started at the same
    /// moment can, doesn't hold git's answer up.
    #[tokio::test]
    async fn output_held_open_past_gits_exit_doesnt_hold_up_the_answer() {
        let folder = tempfile::tempdir().expect("temp dir");
        let started = Instant::now();
        let output = git(
            folder.path(),
            &["-c", "alias.hold=!echo held; sleep 5 &", "hold"],
            &[],
        )
        .await
        .expect("git runs");
        assert_eq!(output, "held\n");
        assert!(
            started.elapsed() < Duration::from_secs(3),
            "{:?}",
            started.elapsed()
        );
    }
}
