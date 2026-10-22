//! The agent's own subagents (ACP's subagent sessions, or Claude Agent's in AIR's draft of them)
//! as subthreads. Each is a thread with no agent of its own, which shows what the parent's
//! agent sends in the subagent's session, and a card in the parent's conversation opens it. The
//! agent hears of its subagents' ends itself, so they're never announced to it.

use std::path::PathBuf;
use std::time::SystemTime;

use agent_client_protocol::schema::v1 as acp;
use agent_thread::{AgentThread, AgentThreadEvent, Attachments, Subagent};
use agentz_protocol::ConnectionId;
use agentz_protocol::thread::Entry;
use gpui_shared_string::SharedString;
use projects::{Task, TaskEnd, TaskOutcome, ThreadCreator, ThreadId};
use util::ResultExt as _;

use crate::transcripts;

use super::tools::{MAX_LAST_MESSAGE_CHARS, last_agent_message, truncate};
use super::{Server, thread_title_from_prompt};

impl Server {
    /// What the thread's agent reported about its subagents.
    pub(super) fn subagent_event(&mut self, thread_id: ThreadId, event: AgentThreadEvent) {
        match event {
            AgentThreadEvent::SubagentStarted(subagent) => {
                self.subagent_started(thread_id, subagent)
            }
            AgentThreadEvent::SubagentUpdate { session, update } => {
                let Some(subthread) = self.loaded_subagent_thread(thread_id, &session) else {
                    return;
                };
                let streams_prompt = matches!(update, acp::SessionUpdate::UserMessageChunk(_));
                let Some(thread) = self.threads.get_mut(&subthread) else {
                    return;
                };
                thread.apply_subagent_update(update);
                self.changed_connections
                    .insert(ConnectionId::Thread(subthread));
                // The rest of the first prompt it was sent, streamed.
                let mut prompts = thread.entries().iter().filter_map(|entry| match entry {
                    Entry::UserMessage(text) => Some(text),
                    _ => None,
                });
                if streams_prompt && let (Some(prompt), None) = (prompts.next(), prompts.next()) {
                    let prompt = prompt.clone();
                    self.projects
                        .update_task(subthread, |task| task.prompt = prompt);
                }
            }
            AgentThreadEvent::SubagentPrompted { session, prompt } => {
                let Some(subthread) = self.loaded_subagent_thread(thread_id, &session) else {
                    return;
                };
                let Some(parent) = self
                    .projects
                    .thread(subthread)
                    .and_then(|thread| thread.task.as_ref())
                    .map(|task| task.parent)
                else {
                    return;
                };
                self.projects.update_task(subthread, |task| {
                    if task.prompt.is_empty() {
                        task.prompt = prompt.clone();
                    }
                });
                if let Some(thread) = self.threads.get_mut(&subthread) {
                    thread.start_subagent(Some((prompt, ThreadCreator::Thread(parent))));
                    self.thread_changed(ConnectionId::Thread(subthread));
                }
            }
            AgentThreadEvent::SubagentEnded { session, end } => {
                if let Some(subthread) = self.subagent_thread(thread_id, &session) {
                    self.end_subagent(subthread, end, None);
                }
            }
            _ => {}
        }
    }

    fn subagent_started(&mut self, thread_id: ThreadId, subagent: Subagent) {
        // ACP's subagents are reused: one that was done goes on in its subthread.
        if let Some(subthread) = self.loaded_subagent_thread(thread_id, &subagent.session) {
            self.subagent_resumed(subthread, subagent);
            return;
        }
        let parent = subagent
            .parent_session
            .as_deref()
            .and_then(|session| self.subagent_thread(thread_id, session))
            .unwrap_or(thread_id);
        let Some(owner) = self.projects.thread(thread_id) else {
            return;
        };
        let agent_id = owner.agent_id.clone();
        let account = owner.account;
        let Some(agent_name) = self
            .threads
            .get(&thread_id)
            .map(|thread| thread.agent_name().clone())
        else {
            return;
        };
        let Some(subthread) = self.projects.add_subthread(
            Task {
                parent,
                prompt: subagent.task.clone().unwrap_or_default(),
                role: None,
                client_request_id: None,
                outcome: None,
                delivered: false,
                agent_session: Some(subagent.session.clone()),
                runs_on: None,
                parent_machine: None,
                stoppable: subagent.stoppable,
                queued: None,
                can_delegate: false,
                changes: None,
            },
            agent_id,
        ) else {
            return;
        };
        self.projects
            .set_custom_title(subthread, thread_title_from_prompt(&subagent.name));
        self.projects.set_thread_account(subthread, account);
        let Some(cwd) = self.projects.thread_folder(subthread) else {
            return;
        };
        let mut thread = AgentThread::subagent(agent_name, cwd);
        thread.set_attachments(Attachments::for_thread(&self.data_dir, subthread));
        thread.start_subagent(
            subagent
                .task
                .clone()
                .map(|task| (task, ThreadCreator::Thread(parent))),
        );
        self.threads.insert(subthread, thread);
        self.thread_changed(ConnectionId::Thread(subthread));
        if let Some(parent_thread) = self.threads.get_mut(&parent) {
            parent_thread.add_subagent_card(&subagent, subthread);
            self.changed_connections
                .insert(ConnectionId::Thread(parent));
        }
    }

    /// The subagent in `subthread` works again, with a new card in its parent's conversation
    /// if it was done.
    fn subagent_resumed(&mut self, subthread: ThreadId, subagent: Subagent) {
        let Some(thread) = self.projects.thread(subthread) else {
            return;
        };
        let Some(task) = thread.task.as_ref() else {
            return;
        };
        let parent = task.parent;
        let was_done = task.outcome.is_some();
        // Its agent may not name it again.
        let card = Subagent {
            name: thread.title.clone(),
            ..subagent.clone()
        };
        self.projects.update_task(subthread, |task| {
            task.outcome = None;
            task.delivered = false;
            task.stoppable = subagent.stoppable;
        });
        if let Some(thread) = self.threads.get_mut(&subthread) {
            thread.start_subagent(
                subagent
                    .task
                    .map(|task| (task, ThreadCreator::Thread(parent))),
            );
            self.thread_changed(ConnectionId::Thread(subthread));
        }
        if was_done && let Some(parent_thread) = self.threads.get_mut(&parent) {
            parent_thread.add_subagent_card(&card, subthread);
            self.changed_connections
                .insert(ConnectionId::Thread(parent));
        }
    }

    /// Asks the agent that runs the subagent in `subthread` to stop it, if it lets agentZ.
    /// Returns whether `subthread` is an agent's own subagent's, which only its agent stops.
    pub(super) fn stop_subagent(&mut self, subthread: ThreadId) -> bool {
        let Some(session) = self
            .projects
            .thread(subthread)
            .and_then(|thread| thread.task.as_ref())
            .and_then(|task| task.agent_session.clone())
        else {
            return false;
        };
        let owner = self.subagent_owner(subthread);
        if let Some(thread) = self.threads.get_mut(&owner) {
            thread.stop_subagent(&session);
        }
        true
    }

    /// Ends the subagent's subthread and its card, and the task, unless it ended already:
    /// with its report once it completed, or with `summary`.
    pub(super) fn end_subagent(
        &mut self,
        subthread: ThreadId,
        end: TaskEnd,
        summary: Option<String>,
    ) {
        let Some(task) = self
            .projects
            .thread(subthread)
            .and_then(|thread| thread.task.as_ref())
            .filter(|task| task.is_agents_own())
        else {
            return;
        };
        let parent = task.parent;
        let ended = task.outcome.is_some();
        let report = self
            .threads
            .get(&subthread)
            .filter(|_| end == TaskEnd::Completed)
            .and_then(|thread| last_agent_message(thread.entries()))
            .map(|message| truncate(message, MAX_LAST_MESSAGE_CHARS).0);
        if let Some(thread) = self.threads.get_mut(&subthread) {
            thread.end_subagent();
            self.thread_changed(ConnectionId::Thread(subthread));
        }
        if let Some(parent_thread) = self.threads.get_mut(&parent) {
            parent_thread.end_subagent_card(subthread, end, report.clone());
            self.changed_connections
                .insert(ConnectionId::Thread(parent));
        }
        if ended {
            return;
        }
        self.projects.update_task(subthread, |task| {
            task.outcome = Some(TaskOutcome {
                end,
                summary: report.or(summary),
                ended_at: SystemTime::now(),
            });
            // The parent's agent ran it, and knows.
            task.delivered = true;
        });
    }

    /// Ends the subagents whose agent stopped: it ran them.
    pub(super) fn end_orphaned_subagents(&mut self) {
        let running: Vec<ThreadId> = self
            .projects
            .threads()
            .iter()
            .filter(|thread| {
                thread
                    .task
                    .as_ref()
                    .is_some_and(|task| task.is_agents_own() && task.outcome.is_none())
            })
            .map(|thread| thread.id)
            .collect();
        for subthread in running {
            let owner = self.subagent_owner(subthread);
            let owner_runs = self.threads.get(&owner).is_some_and(|thread| {
                !matches!(
                    thread.status(),
                    agentz_protocol::thread::ConnectionStatus::Failed(_)
                )
            });
            if !owner_runs {
                self.end_subagent(
                    subthread,
                    TaskEnd::Interrupted,
                    Some("The agent stopped.".to_string()),
                );
            }
        }
    }

    /// The thread whose agent runs the subagent working in `thread_id`: the first one up from it
    /// that isn't a subagent's itself.
    fn subagent_owner(&self, thread_id: ThreadId) -> ThreadId {
        let mut owner = thread_id;
        for _ in 0..self.projects.threads().len() {
            match self
                .projects
                .thread(owner)
                .and_then(|thread| thread.task.as_ref())
                .filter(|task| task.is_agents_own())
            {
                Some(task) => owner = task.parent,
                None => break,
            }
        }
        owner
    }

    /// The subthread of the subagent with `session` that `owner`'s agent runs.
    fn subagent_thread(&self, owner: ThreadId, session: &str) -> Option<ThreadId> {
        self.projects
            .threads()
            .iter()
            .find(|thread| {
                thread
                    .task
                    .as_ref()
                    .and_then(|task| task.agent_session.as_deref())
                    == Some(session)
                    && self.subagent_owner(thread.id) == owner
            })
            .map(|thread| thread.id)
    }

    /// [`Self::subagent_thread`], started if its thread went with the server that handed the
    /// agent on.
    fn loaded_subagent_thread(&mut self, owner: ThreadId, session: &str) -> Option<ThreadId> {
        let subthread = self.subagent_thread(owner, session)?;
        if !self.threads.contains_key(&subthread) {
            let thread = self.start_thread(subthread).log_err()?;
            self.threads.insert(subthread, thread);
            self.thread_changed(ConnectionId::Thread(subthread));
        }
        Some(subthread)
    }

    /// A subagent's subthread as it was kept: working if the subagent still is.
    pub(super) fn restored_subagent_thread(
        &self,
        subthread: ThreadId,
        agent_name: SharedString,
        cwd: PathBuf,
        running: bool,
    ) -> AgentThread {
        let mut thread = AgentThread::subagent(agent_name, cwd);
        thread.set_attachments(Attachments::for_thread(&self.data_dir, subthread));
        if let Some(transcript) = transcripts::load(&self.data_dir, subthread)
            .log_err()
            .flatten()
        {
            thread.restore_transcript(transcript);
        }
        if running {
            thread.start_subagent(None);
        }
        thread
    }

    /// The cards of subagents that ended while the thread wasn't running, as they ended.
    pub(super) fn end_subagent_cards(&self, thread_id: ThreadId, thread: &mut AgentThread) {
        for subthread in self.projects.subthreads(thread_id) {
            let Some(outcome) = subthread
                .task
                .as_ref()
                .filter(|task| task.is_agents_own())
                .and_then(|task| task.outcome.as_ref())
            else {
                continue;
            };
            let report = outcome
                .summary
                .clone()
                .filter(|_| outcome.end == TaskEnd::Completed);
            thread.end_subagent_card(subthread.id, outcome.end, report);
        }
    }
}
