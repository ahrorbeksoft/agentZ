//! Keeps the machine awake while its agents work, as Zed holds off idle sleep while an agent
//! thread runs (`prevent_idle_sleep`). The server holds it, so it goes on with the app closed,
//! and each SSH machine keeps itself awake for its own agents.
//!
//! A system command holds it for as long as it runs: `caffeinate` on macOS, `systemd-inhibit`
//! on Linux. Each runs `cat` on a pipe from the server, so it ends as soon as the pipe closes,
//! which a server that dies does too.

use std::future::Future;
use std::process::Stdio;
use std::time::Duration;

use anyhow::{Context as _, Result, bail};
use futures::channel::oneshot;
use util::ResultExt as _;

/// Zed's reason, which `systemd-inhibit --list` shows.
const REASON: &str = "Agent thread in progress";
/// How long a released command has to end once its pipe closes, before it's killed.
const RELEASE_GRACE: Duration = Duration::from_secs(5);
/// How much of the hold's error is kept, when it ends by itself.
const STDERR_LIMIT: usize = 16 * 1024;

/// A command that keeps the machine awake while it runs, and ends once its stdin closes.
#[derive(Clone, Debug)]
pub struct KeepAwakeCommand {
    pub program: String,
    pub args: Vec<String>,
}

impl KeepAwakeCommand {
    /// This system's, in the order they're tried.
    pub fn system() -> Vec<Self> {
        if cfg!(target_os = "macos") {
            // Idle sleep, as Zed's activity holds off: the display still sleeps.
            vec![Self::new("/usr/bin/caffeinate", ["-i", "cat"])]
        } else if cfg!(target_os = "linux") {
            // Blocking sleep is for the local session's processes alone (polkit's default), so
            // a server started over SSH gets the idle inhibitor alone.
            ["idle:sleep", "idle"]
                .into_iter()
                .map(|what| {
                    Self::new(
                        "systemd-inhibit",
                        [
                            format!("--what={what}"),
                            "--who=agentZ".into(),
                            format!("--why={REASON}"),
                            "--mode=block".into(),
                            "cat".into(),
                        ],
                    )
                })
                .collect()
        } else {
            Vec::new()
        }
    }

    pub fn new(program: &str, args: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            program: program.into(),
            args: args.into_iter().map(Into::into).collect(),
        }
    }
}

/// Holds the machine awake while [`Self::update`] is told agents work.
pub(crate) struct KeepAwake {
    /// What's left to try. One that couldn't hold is dropped for the next.
    commands: Vec<KeepAwakeCommand>,
    hold: Hold,
    holds_taken: u64,
}

enum Hold {
    Released,
    /// Dropping the sender lets the machine sleep.
    Held {
        id: u64,
        _release: oneshot::Sender<()>,
    },
}

impl KeepAwake {
    pub(crate) fn new(commands: Vec<KeepAwakeCommand>) -> Self {
        Self {
            commands,
            hold: Hold::Released,
            holds_taken: 0,
        }
    }

    /// Takes or lets go of the hold, for whether agents work. A hold to take comes back to be
    /// run, with the id to give [`Self::ended`] once it's over.
    pub(crate) fn update(
        &mut self,
        working: bool,
    ) -> Option<(u64, impl Future<Output = Result<()>> + Send + 'static)> {
        match (&self.hold, working) {
            (Hold::Released, true) => {
                let command = self.commands.first()?.clone();
                let (release, released) = oneshot::channel();
                self.holds_taken += 1;
                self.hold = Hold::Held {
                    id: self.holds_taken,
                    _release: release,
                };
                Some((self.holds_taken, hold(command, released)))
            }
            (Hold::Held { .. }, false) => {
                self.hold = Hold::Released;
                None
            }
            _ => None,
        }
    }

    /// A hold is over. One that ended while it was still wanted couldn't hold, so the next
    /// command is tried from the next [`Self::update`].
    pub(crate) fn ended(&mut self, id: u64, result: Result<()>) {
        let is_current = matches!(self.hold, Hold::Held { id: held, .. } if held == id);
        if !is_current {
            result.log_err();
            return;
        }
        self.hold = Hold::Released;
        if self.commands.is_empty() {
            return;
        }
        let command = self.commands.remove(0);
        let error = result.err().unwrap_or_else(|| anyhow::anyhow!("it ended"));
        log::warn!(
            "{} can't keep the machine awake: {error:#}",
            command.program
        );
        if self.commands.is_empty() {
            log::warn!("nothing keeps this machine awake while its agents work");
        }
    }
}

/// Runs the command until `released`. It ending first means it couldn't hold.
async fn hold(command: KeepAwakeCommand, released: oneshot::Receiver<()>) -> Result<()> {
    let mut child = tokio::process::Command::new(&command.program)
        .args(&command.args)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .with_context(|| format!("running {}", command.program))?;
    let pipe = child.stdin.take();
    tokio::select! {
        _ = released => {
            drop(pipe);
            if tokio::time::timeout(RELEASE_GRACE, child.wait()).await.is_err() {
                child.kill().await.context("stopping it")?;
            }
            Ok(())
        }
        status = child.wait() => {
            // Its `cat`, if it got that far, holds the error output open until this closes.
            drop(pipe);
            let output = match child.stderr.take() {
                Some(mut stderr) => child_process::read_rest(&mut stderr, STDERR_LIMIT).await,
                None => Vec::new(),
            };
            bail!(
                "it ended ({}): {}",
                status?,
                String::from_utf8_lossy(&output).trim()
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A command that can't hold gives way to the next, as `systemd-inhibit` blocking sleep,
    /// refused over SSH, gives way to the idle inhibitor alone.
    #[tokio::test]
    async fn falls_back_to_the_next_command() {
        let refused = KeepAwakeCommand::new(
            "/bin/sh",
            ["-c", "echo 'Failed to inhibit: Access denied' >&2; exit 1"],
        );
        let holds = KeepAwakeCommand::new("/bin/sh", ["-c", "cat"]);
        let mut keep_awake = KeepAwake::new(vec![refused, holds]);

        let (id, hold) = keep_awake.update(true).expect("a hold to take");
        let error = hold.await.expect_err("the first can't hold");
        assert!(format!("{error:#}").contains("Access denied"), "{error:#}");
        keep_awake.ended(id, Err(error));

        let (id, hold) = keep_awake.update(true).expect("the next is tried");
        let hold = tokio::spawn(hold);
        assert!(keep_awake.update(true).is_none(), "it's held once");
        assert!(keep_awake.update(false).is_none());
        let released = tokio::time::timeout(Duration::from_secs(10), hold)
            .await
            .expect("it lets go")
            .expect("the hold runs");
        released.expect("it held until let go");
        keep_awake.ended(id, Ok(()));
        assert_eq!(keep_awake.commands.len(), 1, "the one that held stays");
    }
}
