//! Each thread's queue (Zed's message queue): messages the user sends while its agent works,
//! sent one at a time as each turn ends. The server keeps them, so they outlive the app, and
//! saves them in `queues.json` in the data directory, so they outlive the server too.

use std::path::PathBuf;

use agent_client_protocol::schema::v1 as acp;
use agent_thread::{AgentThread, QueuedMessage};
use agentz_protocol::thread::{ConnectionStatus, Entry};
use agentz_protocol::{ConnectionId, PromptPart, Request, Response};
use anyhow::{Context as _, Result, anyhow};
use collections::HashMap;
use projects::ThreadId;
use serde::{Deserialize, Serialize};
use util::ResultExt as _;

use super::Server;

#[derive(Clone, Default, Serialize, Deserialize)]
struct Queue {
    messages: Vec<QueuedMessage>,
    /// The first message steers ([`agentz_protocol::thread::ThreadState::steering_queued`]).
    steering: bool,
}

/// The threads' queues.
pub(super) struct Queues {
    path: PathBuf,
    queues: HashMap<ThreadId, Queue>,
    next_id: u64,
}

impl Queues {
    pub(super) fn load(path: PathBuf) -> Self {
        let queues: Vec<(ThreadId, Queue)> = match std::fs::read(&path) {
            Ok(json) => serde_json::from_slice(&json)
                .with_context(|| format!("reading {}", path.display()))
                .log_err()
                .unwrap_or_default(),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Vec::new(),
            Err(error) => {
                log::error!("failed to read {}: {error}", path.display());
                Vec::new()
            }
        };
        let next_id = queues
            .iter()
            .flat_map(|(_, queue)| &queue.messages)
            .map(|message| message.id + 1)
            .max()
            .unwrap_or(0);
        Self {
            path,
            queues: queues.into_iter().collect(),
            next_id,
        }
    }

    fn save(&self) -> Result<()> {
        let mut queues: Vec<(&ThreadId, &Queue)> = self
            .queues
            .iter()
            .filter(|(_, queue)| !queue.messages.is_empty())
            .collect();
        queues.sort_by_key(|(thread_id, _)| **thread_id);
        let json = serde_json::to_vec(&queues).context("encoding the queues")?;
        let directory = self.path.parent().context("no folder")?;
        std::fs::create_dir_all(directory)
            .with_context(|| format!("creating {}", directory.display()))?;
        // Written beside it first, so a crash doesn't leave half the queues.
        let temporary = self.path.with_extension("json.tmp");
        std::fs::write(&temporary, json)
            .with_context(|| format!("writing {}", temporary.display()))?;
        std::fs::rename(&temporary, &self.path)
            .with_context(|| format!("writing {}", self.path.display()))
    }

    fn queue(&self, thread_id: ThreadId) -> Option<&Queue> {
        self.queues
            .get(&thread_id)
            .filter(|queue| !queue.messages.is_empty())
    }

    pub(super) fn has_messages(&self, thread_id: ThreadId) -> bool {
        self.queue(thread_id).is_some()
    }

    /// The queue's messages and whether the first steers, for the thread's clients.
    pub(super) fn state(&self, thread_id: ThreadId) -> (Vec<QueuedMessage>, bool) {
        self.queue(thread_id)
            .map(|queue| (queue.messages.clone(), queue.steering))
            .unwrap_or_default()
    }

    fn threads(&self) -> Vec<ThreadId> {
        self.queues
            .iter()
            .filter(|(_, queue)| !queue.messages.is_empty())
            .map(|(thread_id, _)| *thread_id)
            .collect()
    }

    /// Forgets the queues of threads that are gone.
    pub(super) fn retain(&mut self, mut keep: impl FnMut(ThreadId) -> bool) {
        let before = self.queues.len();
        self.queues.retain(|thread_id, _| keep(*thread_id));
        if self.queues.len() != before {
            self.save().log_err();
        }
    }

    pub(super) fn remove(&mut self, thread_id: ThreadId) {
        if self.queues.remove(&thread_id).is_some() {
            self.save().log_err();
        }
    }
}

impl Server {
    pub(super) fn queue_request(&mut self, request: Request) -> Result<Response> {
        match request {
            Request::QueueMessage { connection, prompt } => {
                let thread_id = self.queue_thread(connection)?;
                anyhow::ensure!(
                    prompt.iter().any(|part| match part {
                        PromptPart::Text(text) => !text.trim().is_empty(),
                        _ => true,
                    }),
                    "the message is empty"
                );
                // The agent sends the queue once it's running, and the queue may be all it has
                // to do.
                if !self.threads.contains_key(&thread_id) {
                    self.update_thread(connection, |_| ())?;
                }
                let id = self.queues.next_id;
                self.queues.next_id += 1;
                self.queues
                    .queues
                    .entry(thread_id)
                    .or_default()
                    .messages
                    .push(QueuedMessage { id, prompt });
                self.queue_changed(thread_id);
            }
            Request::RemoveQueuedMessage { connection, id } => {
                let thread_id = self.queue_thread(connection)?;
                let (queue, index) = self.queued_message(thread_id, id)?;
                queue.messages.remove(index);
                if index == 0 {
                    queue.steering = false;
                }
                self.queue_changed(thread_id);
            }
            Request::SteerQueuedMessage { connection, id } => {
                let thread_id = self.queue_thread(connection)?;
                self.steer_queued_message(thread_id, id)?;
            }
            Request::SendQueuedMessageNow { connection, id } => {
                let thread_id = self.queue_thread(connection)?;
                let (queue, index) = self.queued_message(thread_id, id)?;
                let message = queue.messages.remove(index);
                queue.messages.insert(0, message);
                queue.steering = false;
                self.queue_changed(thread_id);
                // Once the turn has ended, the queue sends it.
                if self
                    .threads
                    .get(&thread_id)
                    .is_some_and(|thread| thread.is_working())
                {
                    self.update_thread(connection, AgentThread::cancel)?;
                }
            }
            Request::ClearQueue(connection) => {
                let thread_id = self.queue_thread(connection)?;
                if let Some(queue) = self.queues.queues.get_mut(&thread_id) {
                    queue.messages.clear();
                    queue.steering = false;
                }
                self.queue_changed(thread_id);
            }
            _ => return Err(anyhow!("not a queue request")),
        }
        Ok(Response::Ok)
    }

    /// The thread whose queue a request changes: one the user messages, not a subthread.
    pub(super) fn queue_thread(&self, connection: ConnectionId) -> Result<ThreadId> {
        let ConnectionId::Thread(thread_id) = connection else {
            return Err(anyhow!("only threads have queues"));
        };
        let thread = self.projects.thread(thread_id).context("no such thread")?;
        anyhow::ensure!(
            thread.task.is_none(),
            "a subthread only takes its task; message its parent instead"
        );
        Ok(thread_id)
    }

    fn queued_message(&mut self, thread_id: ThreadId, id: u64) -> Result<(&mut Queue, usize)> {
        let queue = self
            .queues
            .queues
            .get_mut(&thread_id)
            .context("the message was already sent")?;
        let index = queue
            .messages
            .iter()
            .position(|message| message.id == id)
            .context("the message was already sent")?;
        Ok((queue, index))
    }

    /// Zed's Steer: an agent that takes messages into its turn gets the message at once. For
    /// any other, the message goes first, to go once the agent's current step is done.
    /// Steering the steering message again stops it.
    fn steer_queued_message(&mut self, thread_id: ThreadId, id: u64) -> Result<()> {
        let takes_steering = self.threads.get(&thread_id).is_some_and(|thread| {
            thread.state.supports_steering
                && thread.is_working()
                && *thread.status() == ConnectionStatus::Ready
        });
        let (queue, index) = self.queued_message(thread_id, id)?;
        if takes_steering {
            let message = queue.messages.remove(index);
            if index == 0 {
                queue.steering = false;
            }
            self.queue_changed(thread_id);
            self.prompt(ConnectionId::Thread(thread_id), message.prompt, true)?;
            return Ok(());
        }
        if index == 0 && queue.steering {
            queue.steering = false;
        } else {
            let message = queue.messages.remove(index);
            queue.messages.insert(0, message);
            queue.steering = true;
        }
        self.queue_changed(thread_id);
        self.steer_if_due(thread_id);
        Ok(())
    }

    /// For an agent that can't take a message into its turn, a steering message ends the turn
    /// at the next step: once no tool call is running (one waiting for approval is between
    /// steps). The queue then sends it, as at any turn's end.
    fn steer_if_due(&mut self, thread_id: ThreadId) {
        if !self
            .queues
            .queue(thread_id)
            .is_some_and(|queue| queue.steering)
        {
            return;
        }
        let Some(thread) = self.threads.get(&thread_id) else {
            return;
        };
        if !thread.is_working() {
            return;
        }
        let entries = thread.entries();
        let turn_start = entries
            .iter()
            .rposition(|entry| matches!(entry, Entry::UserMessage(_)))
            .unwrap_or(0);
        let is_running = entries[turn_start..].iter().any(|entry| {
            matches!(entry, Entry::ToolCall(tool_call)
                if tool_call.status == acp::ToolCallStatus::InProgress)
        });
        if is_running && thread.state.permission_requests.is_empty() {
            return;
        }
        if let Some(queue) = self.queues.queues.get_mut(&thread_id) {
            queue.steering = false;
        }
        self.queue_changed(thread_id);
        self.update_thread(ConnectionId::Thread(thread_id), AgentThread::cancel)
            .log_err();
    }

    /// Sends the first queued message of each thread whose agent is free, and ends the turns
    /// that steering messages wait on.
    pub(super) fn send_queued_messages(&mut self) {
        for thread_id in self.queues.threads() {
            self.steer_if_due(thread_id);
            let Some(thread) = self.threads.get(&thread_id) else {
                continue;
            };
            // Archived threads take no messages until they're unarchived, and paused ones are
            // handed to the next server. One that lost its conversation sends its first message
            // only to fail it, for the user's Send Anyway.
            let is_archived = self
                .projects
                .thread(thread_id)
                .is_some_and(|thread| thread.archived_at.is_some());
            if is_archived
                || thread.is_paused()
                || thread.is_working()
                || *thread.status() != ConnectionStatus::Ready
                || thread.waits_for_send_anyway()
                || self.has_waiting_prompt(thread_id)
            {
                continue;
            }
            let Some(queue) = self.queues.queues.get_mut(&thread_id) else {
                continue;
            };
            if queue.messages.is_empty() {
                continue;
            }
            let message = queue.messages.remove(0);
            queue.steering = false;
            self.queue_changed(thread_id);
            self.prompt(ConnectionId::Thread(thread_id), message.prompt, false)
                .log_err();
        }
    }

    /// Whether the thread has queued messages to send once its agent is free.
    pub(super) fn has_queued_messages(&self, thread_id: ThreadId) -> bool {
        self.queues.has_messages(thread_id)
            && self
                .projects
                .thread(thread_id)
                .is_some_and(|thread| thread.archived_at.is_none())
    }

    /// Saves the queue, and shows it to the thread's clients.
    fn queue_changed(&mut self, thread_id: ThreadId) {
        if self
            .queues
            .queues
            .get(&thread_id)
            .is_some_and(|queue| queue.messages.is_empty())
        {
            self.queues.queues.remove(&thread_id);
        }
        self.queues.save().log_err();
        self.show_queue(thread_id);
    }

    /// Puts the queue in the thread's state, which its clients get.
    pub(super) fn show_queue(&mut self, thread_id: ThreadId) {
        let (messages, steering) = self.queues.state(thread_id);
        if let Some(thread) = self.threads.get_mut(&thread_id) {
            thread.set_queued_messages(messages, steering);
            self.thread_changed(ConnectionId::Thread(thread_id));
        }
    }
}
