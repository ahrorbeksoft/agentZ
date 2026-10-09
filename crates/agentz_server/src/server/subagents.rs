//! The agent's own subagents (Claude Agent's native subagent sessions) as subthreads. Each is a
//! thread with no agent of its own, which shows what the parent's agent sends in the
//! subagent's session, and a card in the parent's conversation opens it. The agent hears of
//! its subagents' ends itself, so they're never announced to it.

use std::path::PathBuf;
use std::time::SystemTime;

use agent_thread::{AgentThread, AgentThreadEvent, Attachments, Subagent};
use agentz_protocol::ConnectionId;
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
                let Some(subthread) = self.subagent_thread(thread_id, &session) else {
                    return;
                };
                // Its thread went with the server that handed the agent on.
                if !self.threads.contains_key(&subthread) {
                    let Some(thread) = self.start_thread(subthread).log_err() else {
                        return;
                    };
                    self.threads.insert(subthread, thread);
                    self.thread_changed(ConnectionId::Thread(subthread));
                }
                if let Some(thread) = self.threads.get_mut(&subthread) {
                    thread.apply_subagent_update(update);
                    self.changed_connections
                        .insert(ConnectionId::Thread(subthread));
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
                prompt: subagent.task.clone(),
                role: None,
                client_request_id: None,
                outcome: None,
                delivered: false,
                agent_session: Some(subagent.session.clone()),
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
        thread.start_subagent(Some((subagent.task.clone(), ThreadCreator::Thread(parent))));
        self.threads.insert(subthread, thread);
        self.thread_changed(ConnectionId::Thread(subthread));
        if let Some(parent_thread) = self.threads.get_mut(&parent) {
            parent_thread.add_subagent_card(&subagent, subthread);
            self.changed_connections
                .insert(ConnectionId::Thread(parent));
        }
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
