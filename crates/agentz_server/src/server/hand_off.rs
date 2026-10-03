//! Updating the server without ending its terminals or agents:
//! [`agentz_protocol::Request::HandOff`] starts the binary installed now and hands it the
//! terminals and the agents' connections (see [`crate::handoff`]). Each thread's connection
//! first pauses between messages (see [`AgentThread::pause`]); one that can't be handed over
//! as it is (its agent is starting, or busy with a request other than its turn) ends with this
//! server, and its agent starts again in the new one, loading its session.

use std::os::fd::{BorrowedFd, OwnedFd};
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::time::Duration;

use agent_thread::AgentThread;
use agentz_protocol::Response;
use agentz_protocol::terminal::TerminalKey;
use anyhow::{Context as _, Result, anyhow};
use collections::HashSet;
use projects::ThreadId;

use super::{ClientId, ConnectionId, Server};
use crate::handoff::{self, AgentPipes, HandedOffThread, Handover, Manifest};

/// How long threads get to pause; those that haven't by then end with this server.
const PAUSE_TIMEOUT: Duration = Duration::from_secs(5);

/// A handoff waiting for threads to pause.
pub(crate) struct PendingHandOff {
    client: ClientId,
    id: u64,
    executable: PathBuf,
    listener: OwnedFd,
    /// The threads asked to pause.
    threads: Vec<ThreadId>,
}

impl Server {
    pub(super) fn hand_off(&mut self, client: ClientId, id: u64, stop_running_turns: bool) {
        if let Err(error) = self.start_hand_off(client, id, stop_running_turns) {
            self.respond(client, id, Err(error));
        }
    }

    /// Answers at once when it can't start, or when turns that can't be handed over run.
    fn start_hand_off(
        &mut self,
        client: ClientId,
        id: u64,
        stop_running_turns: bool,
    ) -> Result<()> {
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
                .iter()
                .filter(|(thread_id, thread)| self.is_busy(**thread_id) && !thread.can_hand_off())
                .filter_map(|(thread_id, _)| self.projects.thread(*thread_id))
                .map(|thread| thread.title.to_string())
                .collect();
            if !running.is_empty() {
                running.sort();
                self.respond(client, id, Ok(Response::TurnsRunning(running)));
                return Ok(());
            }
        }
        // SAFETY: the socket stays open while the server runs.
        let listener = unsafe { BorrowedFd::borrow_raw(listener) }
            .try_clone_to_owned()
            .context("handing off the socket")?;
        self.handing_off = true;
        let mut threads = Vec::new();
        for (thread_id, thread) in &mut self.threads {
            if thread.can_hand_off() && thread.pause() {
                threads.push(*thread_id);
            }
        }
        self.pausing_threads = threads.iter().copied().collect();
        self.pending_hand_off = Some(PendingHandOff {
            client,
            id,
            executable,
            listener,
            threads,
        });
        if self.pausing_threads.is_empty() {
            self.continue_hand_off();
        } else {
            self.spawn_then(tokio::time::sleep(PAUSE_TIMEOUT), |server, ()| {
                server.continue_hand_off()
            });
        }
        Ok(())
    }

    /// A thread paused for the handoff; once all have, it goes on.
    pub(super) fn thread_paused(&mut self, thread_id: ThreadId) {
        if self.pausing_threads.remove(&thread_id) && self.pausing_threads.is_empty() {
            self.continue_hand_off();
        }
    }

    /// Sends what can be handed over to the new server.
    fn continue_hand_off(&mut self) {
        let Some(pending) = self.pending_hand_off.take() else {
            return;
        };
        let still_pausing = std::mem::take(&mut self.pausing_threads);
        let mut agents = Vec::new();
        let mut agent_pipes = Vec::new();
        let mut handed_threads = HashSet::default();
        for thread_id in &pending.threads {
            let Some(thread) = self.threads.get_mut(thread_id) else {
                continue;
            };
            if still_pausing.contains(thread_id) {
                log::warn!("thread {} didn't pause in time", thread_id.0);
                thread.resume();
                continue;
            }
            match thread.hand_off() {
                Ok(handed) => {
                    let token = self
                        .tool_sessions
                        .iter()
                        .find(|(_, session_thread)| *session_thread == thread_id)
                        .map(|(token, _)| token.clone());
                    agents.push(HandedOffThread {
                        thread_id: *thread_id,
                        agent: handed.snapshot,
                        token,
                    });
                    agent_pipes.push(AgentPipes {
                        stdin: handed.stdin,
                        stdout: handed.stdout,
                        stderr: handed.stderr,
                    });
                    handed_threads.insert(*thread_id);
                }
                Err(error) => {
                    log::info!(
                        "thread {}'s agent starts again after the update: {error:#}",
                        thread_id.0
                    );
                    thread.resume();
                }
            }
        }
        // The new server reads the state as it starts.
        self.projects.flush_saves();
        self.spaces.flush_saves();
        let (terminals, ptys): (Vec<_>, Vec<_>) =
            self.pause_terminals(&handed_threads).into_iter().unzip();
        let keys: Vec<TerminalKey> = terminals
            .iter()
            .map(|terminal| terminal.key.clone())
            .collect();
        let follow_ups = self
            .follow_ups
            .iter_mut()
            .map(|(thread_id, follow_ups)| (*thread_id, follow_ups.drain(..).collect()))
            .collect();
        let manifest = Manifest {
            terminals,
            palette: self.terminals.palette.clone(),
            agents,
            follow_ups,
        };
        log::info!(
            "handing {} terminals and {} agents to {}",
            keys.len(),
            handed_threads.len(),
            pending.executable.display()
        );
        let PendingHandOff {
            client,
            id,
            executable,
            listener,
            ..
        } = pending;
        let threads: Vec<ThreadId> = handed_threads.into_iter().collect();
        self.spawn_then(
            async move {
                tokio::task::spawn_blocking(move || {
                    handoff::send(&executable, listener, &manifest, ptys, agent_pipes)
                })
                .await
                .map_err(anyhow::Error::from)
                .and_then(|handover| handover)
            },
            move |server, handover| server.finish_hand_off(client, id, keys, threads, handover),
        );
    }

    fn finish_hand_off(
        &mut self,
        client: ClientId,
        id: u64,
        keys: Vec<TerminalKey>,
        threads: Vec<ThreadId>,
        handover: Result<(Handover, bool)>,
    ) {
        self.handing_off = false;
        let result =
            handover.and_then(|(handover, took_agents)| handover.commit().map(|()| took_agents));
        match result {
            Ok(took_agents) => {
                self.detach_terminals(&keys);
                // A server from before agents were handed over didn't take them: they end with
                // this one, as they did then.
                if took_agents {
                    for thread_id in &threads {
                        if let Some(thread) = self.threads.get_mut(thread_id) {
                            thread.release();
                        }
                    }
                }
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
                for thread_id in &threads {
                    if let Some(thread) = self.threads.get_mut(thread_id) {
                        thread.resume();
                    }
                }
                self.resume_terminals(&keys);
                self.respond(
                    client,
                    id,
                    Err(error.context("handing off to the new server")),
                );
            }
        }
    }

    /// Takes over the agents the server before handed over, as threads that go on with their
    /// sessions and turns.
    pub(super) fn adopt_agents(&mut self, agents: Vec<HandedOffThread>, pipes: Vec<AgentPipes>) {
        for (handed, pipes) in agents.into_iter().zip(pipes) {
            let thread_id = handed.thread_id;
            let terminal_host = self.agent_terminal_host(thread_id);
            let adopted = AgentThread::adopt(
                self.runtime.clone(),
                handed.agent,
                pipes.stdin,
                pipes.stdout,
                pipes.stderr,
                Some(terminal_host),
            );
            let (mut thread, inbox) = match adopted {
                Ok(adopted) => adopted,
                Err(error) => {
                    log::error!(
                        "failed to take over thread {}'s agent: {error:#}",
                        thread_id.0
                    );
                    continue;
                }
            };
            if let Some(cwd) = self.projects.thread_folder(thread_id) {
                thread.set_turn_hook(self.turn_hook(cwd, thread_id));
            }
            if let Some(token) = handed.token {
                self.tool_sessions.insert(token, thread_id);
            }
            let connection = ConnectionId::Thread(thread_id);
            self.forward(inbox, move |message| {
                super::Input::Thread(connection, message)
            });
            self.threads.insert(thread_id, thread);
            self.thread_changed(connection);
        }
    }
}
