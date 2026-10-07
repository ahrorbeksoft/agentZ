//! Continue at reset: a thread stopped by its account's limit gets "Continue." when the limit
//! resets, as t3code's "Resume at reset" does. The account's "When a limit is reached" choice
//! has every thread on it wait, and the limit notice's button one thread. The server keeps the
//! time in the thread ([`projects::Thread::continues_at`]) and sends the message itself, so it
//! goes with the app closed and after a restart.
//!
//! A limit is a turn that ended with an error while the account's next read has a window used
//! up, as the app's notice takes it: the agents' error texts aren't parsed. On Droid that's only
//! when Droid itself stops, after its own choice at the limit.

use std::time::{Duration, Instant, SystemTime};

use agentz_protocol::accounts::{AccountId, AtLimit, used_up_window};
use agentz_protocol::agents::AgentId;
use agentz_protocol::{ConnectionId, PromptPart, Request};
use anyhow::{Context as _, Result};
use projects::ThreadId;
use util::ResultExt as _;

use super::Server;

const CONTINUE_MESSAGE: &str = "Continue.";
/// The longest a wait sleeps before it looks at the clock again: the reset is a time of day,
/// and the clock a sleep runs on stops while the Mac sleeps.
const CLOCK_CHECK: Duration = Duration::from_secs(60);

impl Server {
    /// A turn that starts, by the user or by the wait, ends the wait.
    pub(super) fn turn_started(&mut self, thread_id: ThreadId) {
        self.failed_turns.remove(&thread_id);
        self.projects.set_continues_at(thread_id, None);
    }

    /// The turn ended with an error: the read that follows says whether a limit stopped it.
    pub(super) fn turn_failed(
        &mut self,
        thread_id: ThreadId,
        agent_id: &AgentId,
        account: Option<AccountId>,
    ) {
        // A subthread takes only its task, and its parent sees it stop.
        let takes_messages = self
            .projects
            .thread(thread_id)
            .is_some_and(|thread| thread.task.is_none());
        if takes_messages
            && self.continues_at_reset(agent_id, account)
            && self.usage_reader(agent_id, account).is_some()
        {
            self.failed_turns.insert(thread_id, Instant::now());
        }
    }

    /// After a read of the account that started at `started`: its threads whose turns failed
    /// before then wait for the reset, if the read has a window used up.
    pub(super) fn wait_for_limits_found(
        &mut self,
        agent_id: &AgentId,
        account: Option<AccountId>,
        started: Instant,
    ) {
        let projects = &self.projects;
        self.failed_turns
            .retain(|thread_id, _| projects.thread(*thread_id).is_some());
        let failed: Vec<(ThreadId, Instant)> = self
            .failed_turns
            .iter()
            .filter(|(thread_id, _)| {
                self.projects.thread(**thread_id).is_some_and(|thread| {
                    thread.agent_id.as_deref() == Some(&*agent_id.0) && thread.account == account
                })
            })
            .map(|(thread_id, failed_at)| (*thread_id, *failed_at))
            .collect();
        if failed.is_empty() {
            return;
        }
        let resets_at = self.used_up_until(agent_id, account);
        let continues = self.continues_at_reset(agent_id, account);
        let mut read_again = false;
        for (thread_id, failed_at) in failed {
            // A read under way when the turn ended was the only one, and may have missed the
            // limit.
            if failed_at > started {
                read_again = true;
                continue;
            }
            self.failed_turns.remove(&thread_id);
            if continues && let Some(resets_at) = resets_at {
                self.continue_at(thread_id, resets_at);
            }
        }
        if read_again {
            self.read_account_if_it_can(agent_id, account);
        }
    }

    /// [`Request::ContinueAtReset`].
    pub(super) fn continue_at_reset(&mut self, thread_id: ThreadId, on: bool) -> Result<()> {
        self.queue_thread(ConnectionId::Thread(thread_id))?;
        if !on {
            self.projects.set_continues_at(thread_id, None);
            return Ok(());
        }
        let thread = self.projects.thread(thread_id).context("no such thread")?;
        let account = thread.account;
        let agent_id = thread
            .agent_id
            .clone()
            .map(AgentId::new)
            .context("the thread has no agent")?;
        let resets_at = self
            .used_up_until(&agent_id, account)
            .context("the account isn't at a limit that resets")?;
        self.continue_at(thread_id, resets_at);
        Ok(())
    }

    /// The waits of threads that were waiting when the server stopped. Those past their time
    /// continue now.
    pub(super) fn resume_limit_waits(&self) {
        for thread in self.projects.threads() {
            if let Some(at) = thread.continues_at {
                self.wait_until(thread.id, at);
            }
        }
    }

    fn continues_at_reset(&self, agent_id: &AgentId, account: Option<AccountId>) -> bool {
        self.accounts
            .get(agent_id)
            .choices(account)
            .is_some_and(|choices| choices.at_limit == AtLimit::ContinueAtReset)
    }

    /// When the account's last read says the window that stops it resets.
    fn used_up_until(&self, agent_id: &AgentId, account: Option<AccountId>) -> Option<SystemTime> {
        let accounts = self.accounts.get(agent_id);
        let read = accounts.status(account)?;
        used_up_window(&read.status.windows, SystemTime::now())?.resets_at
    }

    fn continue_at(&mut self, thread_id: ThreadId, at: SystemTime) {
        self.projects.set_continues_at(thread_id, Some(at));
        self.wait_until(thread_id, at);
    }

    fn wait_until(&self, thread_id: ThreadId, at: SystemTime) {
        let wait = async move {
            loop {
                let remaining = at.duration_since(SystemTime::now()).unwrap_or_default();
                if remaining.is_zero() {
                    break;
                }
                tokio::time::sleep(remaining.min(CLOCK_CHECK)).await;
            }
        };
        self.spawn_then(wait, move |server, ()| {
            server.continue_if_due(thread_id, at)
        });
    }

    /// Queues "Continue." in the thread, unless its wait was cancelled or moved meanwhile. The
    /// queue starts its agent if it was stopped, and sends the message once it's free. An
    /// archived thread doesn't continue.
    fn continue_if_due(&mut self, thread_id: ThreadId, at: SystemTime) {
        let Some(thread) = self.projects.thread(thread_id) else {
            return;
        };
        if thread.continues_at != Some(at) {
            return;
        }
        let is_archived = thread.archived_at.is_some();
        self.projects.set_continues_at(thread_id, None);
        if is_archived {
            return;
        }
        self.queue_request(Request::QueueMessage {
            connection: ConnectionId::Thread(thread_id),
            prompt: PromptPart::text(CONTINUE_MESSAGE),
        })
        .context("continuing at the reset")
        .log_err();
    }
}
