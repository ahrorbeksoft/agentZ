//! Running a one-off command for its output and exit code, as an agent's own shell tool does,
//! on any machine the app reaches. herdr has no such call: its agents type into a pane and
//! read it back, which for a quick command on another machine took a terminal, a send, a wait
//! and a read. The command runs without a terminal and ends with the call: once its time is up,
//! it's stopped along with what it started.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use futures::FutureExt as _;
use serde_json::{Value, json};
use tokio::io::AsyncReadExt as _;

use super::{Arguments, Caller, Failure, MAX_WAIT, Outcome, Server, Step, failure, invalid};

const MAX_COMMAND_CHARS: usize = 8_000;
const MAX_FOLDER_CHARS: usize = 4_096;
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(60);
/// How much of the output is kept, from its end, where a build's or a test run's summary is.
const MAX_OUTPUT_BYTES: usize = 50_000;
/// How long the output is still read once the command has exited: something it left running
/// in the background may keep the output open for good.
const OUTPUT_AFTER_EXIT: Duration = Duration::from_millis(200);
/// How long a stopped command gets to exit.
const STOP_WAIT: Duration = Duration::from_secs(5);

impl Server {
    pub(super) fn command_run(&mut self, caller: Caller, arguments: &Arguments) -> Outcome {
        let command = arguments
            .string("command", MAX_COMMAND_CHARS)?
            .filter(|command| !command.trim().is_empty())
            .ok_or_else(|| invalid("command is required."))?
            .to_string();
        let folder = match arguments.string("folder", MAX_FOLDER_CHARS)? {
            Some(folder) => {
                let folder = crate::directories::expand_home(Path::new(folder));
                if !folder.is_absolute() {
                    return Err(invalid("folder must be absolute or start with ~."));
                }
                if !folder.is_dir() {
                    return Err(invalid(format!(
                        "{} isn't a folder on this machine.",
                        folder.display()
                    )));
                }
                folder
            }
            None => caller
                .thread_id
                .and_then(|thread_id| self.projects.thread_folder(thread_id))
                .or_else(|| {
                    let project = self.projects.project(caller.project_id?)?;
                    Some(project.path.clone())
                })
                .unwrap_or_else(|| util::paths::home_dir().clone()),
        };
        let timeout = arguments
            .number("timeoutMs")?
            .map_or(DEFAULT_TIMEOUT, Duration::from_millis)
            .min(MAX_WAIT);
        // Not a login shell, as terminals' commands are: the server already has the login
        // shell's environment, and an interactive shell without a terminal complains on stderr.
        let shell = self
            .terminal_shell
            .clone()
            .unwrap_or_else(util::shell::get_system_shell);
        let mut process = tokio::process::Command::new(shell);
        process
            .arg("-c")
            .arg(command)
            .current_dir(&folder)
            .envs(self.terminal_env(caller.thread_id))
            .stdin(Stdio::null())
            .kill_on_drop(true)
            // Its own group, so stopping it stops what it started.
            .process_group(0);
        let shell_environment_ready = self.shell_environment_ready.clone();
        Ok(Step::Background(
            async move {
                shell_environment_ready.await;
                run(process, folder, timeout).await
            }
            .boxed(),
        ))
    }
}

enum Happened {
    Read(std::io::Result<usize>),
    Exited(std::io::Result<std::process::ExitStatus>),
    TimeUp,
}

async fn run(
    mut process: tokio::process::Command,
    folder: PathBuf,
    timeout: Duration,
) -> Result<Value, Failure> {
    let could_not_run = |error: std::io::Error| {
        failure(
            "operation_failed",
            format!("Couldn't run the command: {error}"),
        )
    };
    // One pipe for both, so the output reads in the order it was written, as in a terminal.
    let (reader, writer) = std::io::pipe().map_err(could_not_run)?;
    process
        .stdout(writer.try_clone().map_err(could_not_run)?)
        .stderr(writer);
    let mut child = process.spawn().map_err(could_not_run)?;
    // Its copies of the pipe's end would keep the output open after the command ends.
    drop(process);
    let mut reader =
        tokio::net::unix::pipe::Receiver::from_owned_fd(reader.into()).map_err(could_not_run)?;
    let group = child.id();
    let mut buffer = vec![0; 8192];
    let mut output = Tail::default();
    let mut reading = true;
    let mut status = None;
    let mut timed_out = false;
    let mut ends_at = tokio::time::Instant::now() + timeout;
    while status.is_none() || reading {
        let happened = tokio::select! {
            read = reader.read(&mut buffer), if reading => Happened::Read(read),
            exited = child.wait(), if status.is_none() => Happened::Exited(exited),
            () = tokio::time::sleep_until(ends_at) => Happened::TimeUp,
        };
        match happened {
            Happened::Read(Ok(read @ 1..)) => output.push(&buffer[..read]),
            Happened::Read(_) => reading = false,
            Happened::Exited(exited) => {
                status = Some(exited.map_err(|error| {
                    failure(
                        "operation_failed",
                        format!("Couldn't wait for the command: {error}"),
                    )
                })?);
                ends_at = tokio::time::Instant::now() + OUTPUT_AFTER_EXIT;
            }
            // What it left running holds the output open.
            Happened::TimeUp if status.is_some() => break,
            Happened::TimeUp if !timed_out => {
                timed_out = true;
                if let Some(group) = group {
                    // SAFETY: only sends a signal, to the group the command leads.
                    unsafe { libc::kill(-(group as libc::pid_t), libc::SIGKILL) };
                }
                ends_at = tokio::time::Instant::now() + STOP_WAIT;
            }
            Happened::TimeUp => break,
        }
    }
    let (text, truncated) = output.text();
    let (exit_code, signal) = match status {
        Some(status) => exit_of(status),
        None => (None, None),
    };
    Ok(json!({
        "exitCode": exit_code,
        "signal": signal,
        "timedOut": timed_out,
        "output": text,
        "outputTruncated": truncated,
        "folder": folder,
    }))
}

/// The exit code, or the signal that ended the command.
fn exit_of(status: std::process::ExitStatus) -> (Option<i32>, Option<String>) {
    use std::os::unix::process::ExitStatusExt as _;
    match status.signal() {
        Some(signal) => {
            let name = nix::sys::signal::Signal::try_from(signal)
                .map(|signal| signal.as_str().to_string())
                .unwrap_or_else(|_| format!("Signal {signal}"));
            (None, Some(name))
        }
        None => (status.code(), None),
    }
}

/// The end of the command's output, stdout and stderr together.
#[derive(Default)]
struct Tail {
    bytes: Vec<u8>,
    total: usize,
}

impl Tail {
    fn push(&mut self, chunk: &[u8]) {
        self.total += chunk.len();
        self.bytes.extend_from_slice(chunk);
        // In batches, so a long output isn't moved for every chunk.
        if self.bytes.len() > 2 * MAX_OUTPUT_BYTES {
            self.bytes.drain(..self.bytes.len() - MAX_OUTPUT_BYTES);
        }
    }

    /// The text, and whether its start was cut.
    fn text(mut self) -> (String, bool) {
        if self.bytes.len() > MAX_OUTPUT_BYTES {
            self.bytes.drain(..self.bytes.len() - MAX_OUTPUT_BYTES);
        }
        let truncated = self.total > self.bytes.len();
        // From a character's start, not inside one.
        let start = if truncated {
            self.bytes
                .iter()
                .position(|byte| byte & 0xC0 != 0x80)
                .unwrap_or(0)
        } else {
            0
        };
        (
            String::from_utf8_lossy(&self.bytes[start..]).into_owned(),
            truncated,
        )
    }
}

pub(super) fn definitions() -> Vec<Value> {
    vec![json!({
        "name": "agentz_command_run",
        "title": "Run a command",
        "description": format!("Run a shell command and return its output and exit code, as your own command tool does, on this machine or, with machine, on another one, with this project there or not. It runs in the user's shell with -c, without a terminal or input, in folder: by default this thread's folder here, this project's checkout on another machine that has it, and the home folder otherwise. Nothing carries over between calls (cd, exports, shell variables). The call ends with the command: once timeoutMs (default 60 seconds) has passed, it's stopped along with what it started, and timedOut is true. exitCode is null when a signal ended it, which signal names. output is stdout and stderr together as they came, only its last {MAX_OUTPUT_BYTES} bytes when it's longer (outputTruncated). For long-running work such as dev servers or watchers, or a program that waits for input, use agentz_terminal_start instead. On this machine, prefer your own command tool."),
        "inputSchema": {
            "type": "object",
            "properties": {
                "command": {"type": "string", "maxLength": MAX_COMMAND_CHARS},
                "folder": {"type": "string", "maxLength": MAX_FOLDER_CHARS, "description": "Where it runs, an absolute path or one starting with ~ on the machine."},
                "timeoutMs": {"type": "integer", "minimum": 0, "maximum": MAX_WAIT.as_millis() as u64},
            },
            "required": ["command"],
            "additionalProperties": false,
        },
        "annotations": {"readOnlyHint": false, "destructiveHint": true, "openWorldHint": true},
    })]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn long_output_keeps_its_end_from_a_character() {
        let mut tail = Tail::default();
        tail.push("é".repeat(MAX_OUTPUT_BYTES).as_bytes());
        // The cut falls inside an é.
        tail.push(b"done.");
        let (text, truncated) = tail.text();
        assert!(truncated);
        assert!(text.ends_with("édone."));
        assert!(text.starts_with('é'));
        assert_eq!(text.len(), MAX_OUTPUT_BYTES - 1);

        let mut short = Tail::default();
        short.push(b"ok\n");
        assert_eq!(short.text(), ("ok\n".to_string(), false));
    }
}
