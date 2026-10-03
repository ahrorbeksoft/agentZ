//! Updating the server without ending its terminals: [`agentz_protocol::Request::HandOff`] starts the binary
//! installed now and hands it the terminals (see [`crate::handoff`]). Agents end with this
//! server and start again in the new one, loading their sessions.

use std::os::fd::BorrowedFd;
use std::sync::atomic::Ordering;

use agentz_protocol::Response;
use agentz_protocol::terminal::TerminalKey;
use anyhow::{Context as _, Result, anyhow};

use super::{ClientId, Server};
use crate::handoff::{self, Handover, Manifest};

impl Server {
    pub(super) fn hand_off(&mut self, client: ClientId, id: u64, stop_running_turns: bool) {
        match self.start_hand_off(client, id, stop_running_turns) {
            Ok(Some(response)) => self.respond(client, id, Ok(response)),
            Ok(None) => {}
            Err(error) => self.respond(client, id, Err(error)),
        }
    }

    /// Answers at once when it can't start, or when turns are running.
    fn start_hand_off(
        &mut self,
        client: ClientId,
        id: u64,
        stop_running_turns: bool,
    ) -> Result<Option<Response>> {
        if self.handing_off {
            return Err(anyhow!("the server is already being updated"));
        }
        let executable = self
            .agent_control
            .as_ref()
            .map(|control| control.executable.clone())
            .context("this server doesn't know its binary")?;
        let listener = self
            .listener
            .context("this server doesn't know its socket")?;
        if !stop_running_turns {
            let mut running: Vec<String> = self
                .threads
                .keys()
                .filter(|thread_id| self.is_busy(**thread_id))
                .filter_map(|thread_id| self.projects.thread(*thread_id))
                .map(|thread| thread.title.to_string())
                .collect();
            if !running.is_empty() {
                running.sort();
                return Ok(Some(Response::TurnsRunning(running)));
            }
        }
        // SAFETY: the socket stays open while the server runs.
        let listener = unsafe { BorrowedFd::borrow_raw(listener) }
            .try_clone_to_owned()
            .context("handing off the socket")?;
        // The new server reads the state as it starts.
        self.projects.flush_saves();
        self.spaces.flush_saves();
        let (terminals, ptys): (Vec<_>, Vec<_>) = self.pause_terminals().into_iter().unzip();
        let keys: Vec<TerminalKey> = terminals
            .iter()
            .map(|terminal| terminal.key.clone())
            .collect();
        let manifest = Manifest {
            terminals,
            palette: self.terminals.palette.clone(),
        };
        self.handing_off = true;
        log::info!(
            "handing {} terminals to {}",
            keys.len(),
            executable.display()
        );
        self.spawn_then(
            async move {
                tokio::task::spawn_blocking(move || {
                    handoff::send(&executable, listener, &manifest, ptys)
                })
                .await
                .map_err(anyhow::Error::from)
                .and_then(|handover| handover)
            },
            move |server, handover| server.finish_hand_off(client, id, keys, handover),
        );
        Ok(None)
    }

    fn finish_hand_off(
        &mut self,
        client: ClientId,
        id: u64,
        keys: Vec<TerminalKey>,
        handover: Result<Handover>,
    ) {
        self.handing_off = false;
        match handover.and_then(Handover::commit) {
            Ok(()) => {
                self.detach_terminals(&keys);
                // The new server owns the state files now.
                self.projects.stop_saving();
                self.spaces.stop_saving();
                self.handed_off.store(true, Ordering::Release);
                log::info!("handed off to the new server; stopping");
                self.respond(client, id, Ok(Response::Ok));
                self.stopping = true;
            }
            Err(error) => {
                log::error!("failed to hand off: {error:#}");
                self.resume_terminals(&keys);
                self.respond(
                    client,
                    id,
                    Err(error.context("handing off to the new server")),
                );
            }
        }
    }
}
