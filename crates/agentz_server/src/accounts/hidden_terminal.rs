//! A terminal nobody sees, for readers that type a command into an agent's terminal UI and read
//! the screen (plan.md, reader kind 5). It ends its program when dropped.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use agentz_protocol::agents::AgentCommand;
use alacritty_terminal::event::Event as AlacEvent;
use anyhow::{Result, bail};
use futures::StreamExt as _;
use futures::channel::mpsc;

use crate::terminals::{Terminal, TerminalSize, TerminalSpawn, without_variables};

pub(super) struct HiddenTerminal {
    terminal: Terminal,
    events: mpsc::UnboundedReceiver<AlacEvent>,
}

impl HiddenTerminal {
    /// Starts the account's agent program (`agent`'s, with its environment) with `args` in
    /// `cwd`.
    pub(super) fn start(
        agent: &AgentCommand,
        args: Vec<String>,
        cwd: PathBuf,
        size: TerminalSize,
    ) -> Result<Self> {
        let env: std::collections::HashMap<String, String> = agent
            .env
            .iter()
            .map(|(variable, value)| (variable.clone(), value.clone()))
            .collect();
        let removed = agent
            .env_remove
            .iter()
            .filter(|variable| !env.contains_key(*variable))
            .cloned()
            .collect();
        let program = without_variables(agent.path.to_string_lossy().into_owned(), args, removed);
        let (sender, events) = mpsc::unbounded();
        let terminal = Terminal::start(
            TerminalSpawn {
                program: Some(program),
                cwd,
                env,
            },
            size,
            None,
            Arc::new(move |event| {
                sender.unbounded_send(event).ok();
            }),
        )?;
        Ok(Self { terminal, events })
    }

    /// Waits up to `timeout` for `found` to find what it looks for on the screen; `what` names
    /// it for the error otherwise.
    pub(super) async fn wait_for<T>(
        &mut self,
        what: &str,
        timeout: Duration,
        mut found: impl FnMut(&str) -> Option<T>,
    ) -> Result<T> {
        let deadline = tokio::time::Instant::now() + timeout;
        loop {
            self.terminal.take_wakeup();
            let screen = self.terminal.screen_text();
            if let Some(found) = found(&screen) {
                return Ok(found);
            }
            // What it last wrote, for the error.
            let last_line = screen
                .lines()
                .rev()
                .map(str::trim)
                .find(|line| !line.is_empty())
                .unwrap_or_default()
                .to_string();
            if self.terminal.exit().is_some() {
                bail!("it exited before showing {what} (\"{last_line}\")");
            }
            match tokio::time::timeout_at(deadline, self.events.next()).await {
                Ok(Some(event)) => {
                    self.terminal.handle_event(event);
                }
                Ok(None) => bail!("its terminal stopped before showing {what}"),
                Err(_) => bail!(
                    "it didn't show {what} within {}s (\"{last_line}\")",
                    timeout.as_secs()
                ),
            }
        }
    }

    pub(super) fn write(&self, text: &str) {
        self.terminal.write(text.as_bytes().to_vec());
    }

    /// Ends the program, and waits up to `timeout` for it to be gone, so it's done writing its
    /// files.
    pub(super) async fn end(mut self, timeout: Duration) {
        let pid = self.terminal.child_pid();
        self.terminal.kill();
        #[cfg(unix)]
        if let Some(pid) = pid {
            let deadline = tokio::time::Instant::now() + timeout;
            while crate::terminals::process_exists(pid) && tokio::time::Instant::now() < deadline {
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        }
    }
}
