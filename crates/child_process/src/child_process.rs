//! Running a command for what it prints, without waiting on its pipes once it has exited.
//!
//! On macOS, which lacks `pipe2`, Rust makes a pipe close-on-exec only after creating it, so a
//! process another thread starts at that moment (an agent, a terminal's shell, an `ssh`)
//! inherits the pipe's end and keeps it open for as long as it runs. The end of a command's
//! output then comes long after the command exited, or never, and `output()` waits for it: a
//! `git rev-parse` reading a task's changes hung until its parent never heard of the task. And
//! a command that reads its input to the end never sees the end of a pipe we closed. So output
//! is read only [`OUTPUT_AFTER_EXIT`] past the exit, and input goes in a file.

use std::io::{self, Seek as _, SeekFrom, Write as _};
use std::process::{ExitStatus, Output, Stdio};
use std::time::Duration;

use tokio::io::{AsyncRead, AsyncReadExt as _};
use tokio::process::{Child, Command};

/// How long a command's output is still read once it has exited. Everything it wrote is in the
/// pipe by then, so what holds the pipe open after that is a process that inherited its end.
pub const OUTPUT_AFTER_EXIT: Duration = Duration::from_millis(200);

/// [`Command::output`], reading the output only [`OUTPUT_AFTER_EXIT`] past the exit. The
/// command gets no input, as `output` gives it by default.
pub async fn output(command: &mut Command) -> io::Result<Output> {
    let child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    wait_with_output(child).await
}

/// [`Child::wait_with_output`], reading the output only [`OUTPUT_AFTER_EXIT`] past the exit.
pub async fn wait_with_output(child: Child) -> io::Result<Output> {
    let output = wait_with_limited_output(child, usize::MAX).await?;
    let status = output
        .status
        .ok_or_else(|| io::Error::other("the command was stopped"))?;
    Ok(Output {
        status,
        stdout: output.stdout,
        stderr: output.stderr,
    })
}

/// What a command printed, and how it exited: `None` when it was stopped for printing more than
/// its limit.
pub struct LimitedOutput {
    pub status: Option<ExitStatus>,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

enum Happened {
    Printed(io::Result<usize>),
    Complained(io::Result<usize>),
    Exited(io::Result<ExitStatus>),
    OutputHeld,
}

/// [`wait_with_output`], stopping the command once it has printed more than `limit` bytes, of
/// which it keeps the first `limit`.
pub async fn wait_with_limited_output(mut child: Child, limit: usize) -> io::Result<LimitedOutput> {
    // As `wait_with_output` does, so a command reading its input isn't left waiting for it.
    drop(child.stdin.take());
    let mut stdout = child.stdout.take();
    let mut stderr = child.stderr.take();
    let mut output = LimitedOutput {
        status: None,
        stdout: Vec::new(),
        stderr: Vec::new(),
    };
    let mut stdout_buffer = vec![0; 64 * 1024];
    let mut stderr_buffer = vec![0; 8 * 1024];
    let mut reading_stdout = stdout.is_some();
    let mut reading_stderr = stderr.is_some();
    let mut read_until = tokio::time::Instant::now();
    while output.status.is_none() || reading_stdout || reading_stderr {
        let happened = tokio::select! {
            biased;
            read = read_some(&mut stdout, &mut stdout_buffer), if reading_stdout => {
                Happened::Printed(read)
            }
            read = read_some(&mut stderr, &mut stderr_buffer), if reading_stderr => {
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
                read if read > limit - output.stdout.len() => {
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

async fn read_some(
    pipe: &mut Option<impl AsyncRead + Unpin>,
    buffer: &mut [u8],
) -> io::Result<usize> {
    match pipe {
        Some(pipe) => pipe.read(buffer).await,
        None => Ok(0),
    }
}

/// What's left to read, up to `limit` bytes, of the output of a command that has exited: up to
/// its end, or for [`OUTPUT_AFTER_EXIT`].
pub async fn read_rest(pipe: &mut (impl AsyncRead + Unpin), limit: usize) -> Vec<u8> {
    let mut rest = Vec::new();
    let mut buffer = vec![0; 8 * 1024];
    let read_until = tokio::time::Instant::now() + OUTPUT_AFTER_EXIT;
    while rest.len() < limit {
        match tokio::time::timeout_at(read_until, pipe.read(&mut buffer)).await {
            Ok(Ok(read @ 1..)) => {
                rest.extend_from_slice(&buffer[..read.min(limit - rest.len())]);
            }
            _ => break,
        }
    }
    rest
}

/// `bytes` as a command's input, in a file of its own: the command sees its end once it has
/// read it all, which a pipe another process inherited would keep from it.
pub fn input(bytes: &[u8]) -> io::Result<Stdio> {
    let mut file = tempfile::tempfile()?;
    file.write_all(bytes)?;
    file.seek(SeekFrom::Start(0))?;
    Ok(Stdio::from(file))
}

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use super::*;

    /// A process the command leaves behind with its output open, as a process started at the
    /// same moment elsewhere can be, doesn't hold its output up.
    #[tokio::test]
    async fn output_held_open_past_the_exit_doesnt_hold_it_up() {
        let started = Instant::now();
        let output = output(Command::new("/bin/sh").args(["-c", "echo held; sleep 5 &"]))
            .await
            .expect("runs");
        assert!(output.status.success());
        assert_eq!(output.stdout, b"held\n");
        assert!(
            started.elapsed() < Duration::from_secs(3),
            "{:?}",
            started.elapsed()
        );
    }

    /// Output past the limit stops the command, keeping what came before it.
    #[tokio::test]
    async fn stops_a_command_printing_past_its_limit() {
        let child = Command::new("/bin/sh")
            .args(["-c", "echo 1234567890; sleep 5"])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .expect("runs");
        let started = Instant::now();
        let output = wait_with_limited_output(child, 4).await.expect("waits");
        assert!(output.status.is_none());
        assert_eq!(output.stdout, b"1234");
        assert!(started.elapsed() < Duration::from_secs(3));
    }

    /// Input in a file ends where the file does, so a command reading it all finishes.
    #[tokio::test]
    async fn input_ends_where_its_bytes_do() {
        let child = Command::new("/bin/cat")
            .stdin(input(b"all of it").expect("writes the input"))
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("runs");
        let output = wait_with_output(child).await.expect("waits");
        assert_eq!(output.stdout, b"all of it");
    }

    /// What's left of a pipe ends with it, or soon after when another process holds it.
    #[tokio::test]
    async fn reads_the_rest_of_a_pipe_held_open() {
        let mut child = Command::new("/bin/sh")
            .args(["-c", "echo said >&2; sleep 5 &"])
            .stderr(Stdio::piped())
            .spawn()
            .expect("runs");
        let mut stderr = child.stderr.take().expect("its errors");
        child.wait().await.expect("exits");
        let started = Instant::now();
        assert_eq!(read_rest(&mut stderr, 1024).await, b"said\n");
        assert!(started.elapsed() < Duration::from_secs(3));
    }
}
