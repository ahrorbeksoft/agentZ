//! Requests about worktrees and pastures, which take git work off the server's task and come
//! back to change its state.

use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime};

use agent_client_protocol::schema::v1 as acp;
use agentz_protocol::accounts::{AccountChoice, AccountId};
use agentz_protocol::agents::{AgentId, InstallState};
use agentz_protocol::terminal::{TerminalCommand, TerminalKey};
use agentz_protocol::thread::{
    ConnectionStatus, Entry, FailedMessage, HandoffSource, PendingHandoff, ThreadAgent,
    ThreadState, ThreadView, fork_handoff, merge_handoff, session_handoff, switch_handoff,
};
use agentz_protocol::workspace::{RepositoryCheckouts, WorkspaceChoice, WorkspaceRemoval};
use agentz_protocol::{ConnectionId, Request, Response};
use anyhow::{Context as _, Result, anyhow};
use futures::FutureExt as _;
use futures::future::BoxFuture;
use projects::{ProjectId, ThreadId, Workspace, WorkspaceKind};

use super::{ClientId, OPEN_GRACE, Server};
use crate::workspaces::{self, NewWorkspace};
use crate::{continuations, transcripts};
use util::ResultExt as _;

/// What a new thread runs.
pub(super) enum NewThread {
    /// On the agent's account, `None` being the External one.
    Agent(AgentId, Option<AccountId>),
    Terminal(TerminalCommand),
}

/// Where a new thread will work, once any new workspace is made.
pub(super) enum PreparedWorkspace {
    /// The project's folder (`None`) or an existing workspace.
    Ready(Option<PathBuf>),
    Create(BoxFuture<'static, Result<Workspace>>),
}

impl Server {
    pub(super) fn workspace_request(&mut self, client: ClientId, id: u64, request: Request) {
        match request {
            Request::CreateThread {
                project_id,
                agent_id,
                workspace,
                account,
            } => {
                let account = match self.choose_account(&agent_id, account) {
                    Ok(account) => account,
                    Err(error) => return self.respond(client, id, Err(error)),
                };
                self.create_thread(
                    client,
                    id,
                    project_id,
                    NewThread::Agent(agent_id, account),
                    workspace,
                )
            }
            Request::CreateTerminalThread {
                project_id,
                command,
                workspace,
            } => self.create_thread(
                client,
                id,
                project_id,
                NewThread::Terminal(command),
                workspace,
            ),
            Request::CreateWorkspacesThread {
                folder,
                agent_id,
                workspace,
                account,
            } => self.create_workspaces_thread(client, id, folder, agent_id, workspace, account),
            Request::ProjectGit(project_id) => {
                let repo = match self.project_path(project_id) {
                    Ok(repo) => repo,
                    Err(error) => return self.respond(client, id, Err(error)),
                };
                let data_dir = self.data_dir.clone();
                self.spawn_then(
                    async move { workspaces::project_git(&repo, &data_dir).await },
                    move |server, git| server.respond(client, id, Ok(Response::ProjectGit(git))),
                );
            }
            Request::RepositoryCheckouts(folder) => {
                let data_dir = self.data_dir.clone();
                let project_workspaces: Vec<(PathBuf, Vec<Workspace>)> = self
                    .projects
                    .projects()
                    .iter()
                    .map(|project| (project.path.clone(), project.workspaces.clone()))
                    .collect();
                self.spawn_then(
                    async move {
                        workspaces::repository_checkouts(&folder, &data_dir, &project_workspaces)
                            .await
                    },
                    move |server, checkouts| {
                        let data_dir = super::terminal_requests::home_relative(&server.data_dir);
                        let checkouts = checkouts.map(|checkouts| RepositoryCheckouts {
                            data_dir,
                            ..checkouts
                        });
                        server.respond(client, id, checkouts.map(Response::RepositoryCheckouts))
                    },
                );
            }
            Request::FetchOrigin(folder) => {
                let data_dir = self.data_dir.clone();
                self.spawn_then(
                    async move {
                        workspaces::fetch_origin(&folder).await?;
                        let root = workspaces::main_checkout(&folder).await?;
                        let mut git = workspaces::project_git(&root, &data_dir).await;
                        git.branch = workspaces::current_branch(&folder).await.ok().flatten();
                        anyhow::Ok(git)
                    },
                    move |server, git| server.respond(client, id, git.map(Response::ProjectGit)),
                );
            }
            Request::CreateWorkspace {
                folder,
                kind,
                base,
                branch,
            } => {
                let data_dir = self.data_dir.clone();
                self.spawn_then(
                    async move {
                        workspaces::create_from(&folder, kind, base, branch, data_dir).await
                    },
                    move |server, created| {
                        let folder = created
                            .map(|(repo, workspace)| server.adopt_repository_workspace(&repo, workspace));
                        server.respond(client, id, folder.map(Response::WorkspaceCreated));
                    },
                );
            }
            Request::RemoveWorkspace { path, force } => {
                self.remove_workspace(client, id, path, force)
            }
            Request::SyncWorkspace {
                project_id,
                path,
                branch,
                merge,
            } => match self.sync_workspace(project_id, &path, branch, merge) {
                Ok(work) => self.respond_later(client, id, work),
                Err(error) => self.respond(client, id, Err(error)),
            },
            Request::BringBackWorkspace {
                project_id,
                path,
                branch,
            } => match self.bring_back_workspace(project_id, &path, branch) {
                Ok(work) => self.respond_later(client, id, work),
                Err(error) => self.respond(client, id, Err(error)),
            },
            request => self.respond(
                client,
                id,
                Err(anyhow!("not a workspace request: {request:?}")),
            ),
        }
    }

    fn create_thread(
        &mut self,
        client: ClientId,
        id: u64,
        project_id: ProjectId,
        new: NewThread,
        workspace: WorkspaceChoice,
    ) {
        if project_id == ProjectId::CHATS && matches!(new, NewThread::Terminal(_)) {
            let error = anyhow!("a chat is a conversation with an agent, not a terminal");
            return self.respond(client, id, Err(error));
        }
        // An agent's draft starts in the project's folder, and its first message makes the
        // new workspace (`workspace_setup`).
        if let (NewThread::Agent(..), Some(plan)) = (&new, workspace.plan())
            && project_id != ProjectId::WORKSPACES
            && project_id != ProjectId::CHATS
        {
            let result = self
                .create_thread_in(project_id, new, None)
                .inspect(|thread_id| {
                    self.projects.set_planned_workspace(*thread_id, Some(plan));
                });
            return self.respond(client, id, result.map(Response::ThreadCreated));
        }
        match self.prepare_workspace(project_id, workspace) {
            Ok(PreparedWorkspace::Ready(folder)) => {
                let result = self.create_thread_in(project_id, new, folder);
                self.respond(client, id, result.map(Response::ThreadCreated));
            }
            Ok(PreparedWorkspace::Create(work)) => {
                self.spawn_then(work, move |server, workspace| {
                    let result = workspace.and_then(|workspace| {
                        let folder = server.adopt_workspace(project_id, workspace);
                        server.create_thread_in(project_id, new, Some(folder))
                    });
                    server.respond(client, id, result.map(Response::ThreadCreated));
                });
            }
            Err(error) => self.respond(client, id, Err(error)),
        }
    }

    /// A Workspaces thread started from `folder`: working there, or in a worktree or pasture of
    /// its repository, which remembers `folder`.
    fn create_workspaces_thread(
        &mut self,
        client: ClientId,
        id: u64,
        folder: PathBuf,
        agent_id: AgentId,
        workspace: WorkspaceChoice,
        account: AccountChoice,
    ) {
        let folder = std::fs::canonicalize(&folder).unwrap_or(folder);
        if !folder.is_dir() {
            let error = anyhow!("{} isn't a folder here", folder.display());
            return self.respond(client, id, Err(error));
        }
        let account = match self.choose_account(&agent_id, account) {
            Ok(account) => account,
            Err(error) => return self.respond(client, id, Err(error)),
        };
        let new = NewThread::Agent(agent_id, account);
        let start = {
            let folder = folder.clone();
            move |server: &mut Server, path: PathBuf| {
                let started_in = (path != folder).then_some(folder);
                let thread_id = server.create_thread_in(ProjectId::WORKSPACES, new, Some(path))?;
                server.projects.set_started_in(thread_id, started_in);
                anyhow::Ok(thread_id)
            }
        };
        let plan = workspace.plan();
        let path = match workspace {
            WorkspaceChoice::Checkout | WorkspaceChoice::New { .. } => folder,
            WorkspaceChoice::Existing(path) => path,
        };
        // Its first message makes the new workspace from `folder` (`workspace_setup`).
        if let Some(plan) = plan {
            return self.spawn_then(
                async move { workspaces::main_checkout(&path).await.map(|_| path) },
                move |server, path| {
                    let result = path.and_then(|path| {
                        let thread_id = start(server, path)?;
                        server.projects.set_planned_workspace(thread_id, Some(plan));
                        anyhow::Ok(thread_id)
                    });
                    server.respond(client, id, result.map(Response::ThreadCreated));
                },
            );
        }
        let result = if path.is_dir() {
            start(self, path)
        } else {
            Err(anyhow!("{} was removed", path.display()))
        };
        self.respond(client, id, result.map(Response::ThreadCreated));
    }

    /// Records a worktree or pasture made from a repository as one of the workspaces of the
    /// project at the repository's main checkout, if there's one, for its threads. Returns its
    /// folder.
    pub(super) fn adopt_repository_workspace(
        &mut self,
        repo: &Path,
        workspace: Workspace,
    ) -> PathBuf {
        let project_id = self
            .projects
            .projects()
            .iter()
            .find(|project| project.path == repo)
            .map(|project| project.id);
        match project_id {
            Some(project_id) => self.adopt_workspace(project_id, workspace),
            None => workspace.path,
        }
    }

    /// Checks the choice now; a new workspace is made by the returned work. A Workspaces
    /// thread's is any existing folder.
    pub(super) fn prepare_workspace(
        &self,
        project_id: ProjectId,
        choice: WorkspaceChoice,
    ) -> Result<PreparedWorkspace> {
        if project_id == ProjectId::WORKSPACES {
            let WorkspaceChoice::Existing(path) = choice else {
                anyhow::bail!(
                    "a thread started in a workspace pane belongs to no project, so it has no \
                     checkout of its own; name a folder"
                );
            };
            anyhow::ensure!(path.is_dir(), "{} was removed", path.display());
            return Ok(PreparedWorkspace::Ready(Some(path)));
        }
        // A chat's folder is made with its first message (`chats`); one continuing a chat
        // works in that chat's.
        if project_id == ProjectId::CHATS {
            let root = self
                .projects
                .chats_folder()
                .context("chats have no folder")?;
            std::fs::create_dir_all(root)
                .with_context(|| format!("creating {}", root.display()))?;
            return match choice {
                WorkspaceChoice::Checkout => Ok(PreparedWorkspace::Ready(None)),
                WorkspaceChoice::Existing(path) if path.parent() == Some(root) && path.is_dir() => {
                    Ok(PreparedWorkspace::Ready(Some(path)))
                }
                _ => Err(anyhow!(
                    "a chat works in a folder of its own, which its first message makes"
                )),
            };
        }
        let repo = self.project_path(project_id)?;
        match choice {
            WorkspaceChoice::Checkout => Ok(PreparedWorkspace::Ready(None)),
            WorkspaceChoice::Existing(path) if path == repo => Ok(PreparedWorkspace::Ready(None)),
            // A thread moved from the Workspaces section may work in a folder inside the project.
            WorkspaceChoice::Existing(path) => {
                anyhow::ensure!(
                    projects::project_at(self.projects.projects(), &path)
                        .is_some_and(|project| project.id == project_id),
                    "{} isn't one of the project's workspaces",
                    path.display()
                );
                anyhow::ensure!(path.exists(), "{} was removed", path.display());
                Ok(PreparedWorkspace::Ready(Some(path)))
            }
            WorkspaceChoice::New { kind, base, branch } => {
                let new = NewWorkspace {
                    kind,
                    repo,
                    data_dir: self.data_dir.clone(),
                    base,
                    branch,
                    submodules: true,
                };
                Ok(PreparedWorkspace::Create(workspaces::create(new).boxed()))
            }
        }
    }

    /// Records a workspace that was just made, and returns its folder.
    pub(super) fn adopt_workspace(
        &mut self,
        project_id: ProjectId,
        workspace: Workspace,
    ) -> PathBuf {
        let path = workspace.path.clone();
        self.projects.add_workspace(project_id, workspace);
        path
    }

    /// Adds a thread working in `folder` (the project's own with `None`) and starts its agent
    /// or terminal, so it's ready by the time the user has typed something. An agent's thread
    /// is a draft until its first message ([`Self::sweep_drafts`]).
    pub(super) fn create_thread_in(
        &mut self,
        project_id: ProjectId,
        new: NewThread,
        folder: Option<PathBuf>,
    ) -> Result<ThreadId> {
        let thread_id = match &new {
            NewThread::Agent(agent_id, _) => self.projects.add_thread(
                project_id,
                projects::NEW_THREAD_TITLE,
                Some(agent_id.0.to_string()),
            ),
            NewThread::Terminal(command) => self
                .projects
                .add_terminal_thread(project_id, command.clone()),
        }
        .context("no such project")?;
        self.projects.set_thread_workspace(thread_id, folder);
        match new {
            NewThread::Agent(_, account) => {
                self.projects.set_thread_account(thread_id, account);
                self.projects.set_draft(thread_id, true);
                self.draft_due
                    .insert(thread_id, Instant::now() + OPEN_GRACE);
                self.update_thread(ConnectionId::Thread(thread_id), |_| {})?;
                self.sweep_drafts();
            }
            NewThread::Terminal(_) => {
                self.ensure_terminal(&TerminalKey::Thread(thread_id))?;
            }
        }
        Ok(thread_id)
    }

    /// Retry ([`Request::RetryMessage`]). A message that failed for the lost conversation is
    /// resent with it (Resend with Context): the session the agent opened in its place is told
    /// the conversation before the message ([`session_handoff`]).
    pub(super) fn retry_message(&mut self, connection: ConnectionId) -> Result<()> {
        if let ConnectionId::Thread(thread_id) = connection
            && let Some(running) = self.threads.get(&thread_id)
            && running.failed_message() == Some(FailedMessage::LostHistory)
            && let Some(thread) = self.projects.thread(thread_id)
        {
            let before = running
                .entries
                .iter()
                .rposition(|entry| matches!(entry, Entry::UserMessage(_)))
                .unwrap_or(running.entries.len());
            let view = ThreadView {
                entries: running.entries[..before].to_vec(),
                state: running.state.clone(),
            };
            let agent_name = thread
                .agent_id
                .clone()
                .map(|agent_id| self.agent_name(&AgentId::new(agent_id)).to_string())
                .unwrap_or_else(|| running.agent_name().to_string());
            if let Some(handoff) = session_handoff(&view, thread_id, &thread.title, &agent_name) {
                let handoffs = self.update_thread(connection, |thread| {
                    thread.add_handoff(handoff);
                    thread.state.pending_handoffs.clone()
                })?;
                continuations::save(&self.data_dir, thread_id, &handoffs).log_err();
            }
        }
        self.update_thread(connection, agent_thread::AgentThread::retry_message)
    }

    /// Fork from This Response ([`Request::ForkThread`]): a thread on the same agent and
    /// account, in the same folder, that begins with the conversation up to the end of the
    /// turn of the reply at `entry`, as t3code forks a finished run. Its first message brings
    /// that conversation ([`fork_handoff`]), unless its agent forks the thread's session
    /// ([`agent_thread::AgentThread::fork_session`]), which it can only when the fork has all
    /// of it.
    pub(super) fn fork_thread(&mut self, thread_id: ThreadId, entry: usize) -> Result<ThreadId> {
        let thread = self
            .projects
            .thread(thread_id)
            .context("no such thread")?
            .clone();
        anyhow::ensure!(
            thread.terminal.is_none(),
            "a terminal thread has no conversation to fork"
        );
        let agent_id = thread
            .agent_id
            .clone()
            .map(AgentId::new)
            .context("the thread has no agent")?;
        let connection = ConnectionId::Thread(thread_id);
        if !self.threads.contains_key(&thread_id) {
            self.update_thread(connection, |_| {})?;
        }
        let running = self.threads.get(&thread_id).context("no such thread")?;
        anyhow::ensure!(
            running.conversation_revision().is_some(),
            "the thread is still loading its conversation"
        );
        anyhow::ensure!(
            matches!(running.entries.get(entry), Some(Entry::AgentMessage(_))),
            "only an agent's reply can be forked"
        );
        let end = match running.turn_record(entry) {
            Some(turn) => turn.entries_end.min(running.entries.len()),
            None => match running.entries[entry + 1..]
                .iter()
                .position(|entry| matches!(entry, Entry::UserMessage(_)))
            {
                Some(offset) => entry + 1 + offset,
                None => {
                    anyhow::ensure!(
                        !running.is_working(),
                        "the reply's turn hasn't finished yet"
                    );
                    running.entries.len()
                }
            },
        };
        // The agent's session stands in for the conversation only when the fork has all of it
        // and the agent knows it all: nothing waits to be told to it, and its session loaded.
        let source_session = thread
            .session_id
            .clone()
            .filter(|_| {
                end == running.entries.len()
                    && !running.is_working()
                    && running.state.pending_handoffs.is_empty()
                    && running.state.lost_history.is_none()
            })
            .map(acp::SessionId::new);
        let state = &running.state;
        let fork_view = ThreadView {
            entries: running.entries[..end].to_vec(),
            state: ThreadState {
                finished_turns: state
                    .finished_turns
                    .iter()
                    .filter(|turn| turn.entries_end <= end)
                    .cloned()
                    .collect(),
                prompts_from_agents: state
                    .prompts_from_agents
                    .iter()
                    .filter(|(index, _)| *index < end)
                    .cloned()
                    .collect(),
                task_notices: state
                    .task_notices
                    .iter()
                    .copied()
                    .filter(|index| *index < end)
                    .collect(),
                sent_times: state
                    .sent_times
                    .iter()
                    .copied()
                    .filter(|(index, _)| *index < end)
                    .collect(),
                handoffs: state
                    .handoffs
                    .iter()
                    .filter(|mark| mark.entry < end)
                    .cloned()
                    .collect(),
                ..ThreadState::default()
            },
        };
        let agent_name = self.agent_name(&agent_id).to_string();
        let choice = thread
            .workspace
            .clone()
            .map_or(WorkspaceChoice::Checkout, WorkspaceChoice::Existing);
        let PreparedWorkspace::Ready(folder) = self.prepare_workspace(thread.project_id, choice)?
        else {
            return Err(anyhow!("the thread's workspace isn't ready"));
        };
        let fork = self.create_thread_in(
            thread.project_id,
            NewThread::Agent(agent_id, thread.account),
            folder,
        )?;
        // It has a conversation already, so it's no draft.
        self.projects.set_draft(fork, false);
        self.draft_due.remove(&fork);
        self.projects
            .rename_thread(fork, format!("{} fork", thread.title));
        self.projects.set_started_in(fork, thread.started_in);
        self.projects.set_forked_from(
            fork,
            projects::ForkPoint {
                thread: thread_id,
                entries: end,
            },
        );
        let handoffs: Vec<PendingHandoff> =
            fork_handoff(&fork_view, fork, thread_id, &thread.title, &agent_name)
                .into_iter()
                .collect();
        let transcript = agent_thread::TranscriptRef {
            entries: &fork_view.entries,
            plan: &fork_view.state.plan,
            finished_turns: &fork_view.state.finished_turns,
            prompts_from_agents: &fork_view.state.prompts_from_agents,
            task_notices: &fork_view.state.task_notices,
            sent_times: &fork_view.state.sent_times,
            handoffs: &fork_view.state.handoffs,
        };
        transcripts::save(&self.data_dir, fork, transcript).log_err();
        continuations::save(&self.data_dir, fork, &handoffs).log_err();
        let ThreadView { entries, state } = fork_view;
        self.update_thread(ConnectionId::Thread(fork), |thread| {
            thread.restore_transcript(agent_thread::Transcript {
                entries,
                plan: state.plan,
                finished_turns: state.finished_turns,
                prompts_from_agents: state.prompts_from_agents,
                task_notices: state.task_notices,
                sent_times: state.sent_times,
                handoffs: state.handoffs,
            });
            thread.set_handoffs(handoffs);
            if let Some(source_session) = source_session {
                thread.fork_session(source_session);
            }
        })?;
        Ok(fork)
    }

    /// Merge Back ([`Request::MergeBack`]): what happened in the fork since it was forked waits
    /// in the thread it was forked from, for that thread's next message ([`merge_handoff`]),
    /// in place of an earlier merge of the fork that hasn't gone yet, as t3code supersedes it.
    pub(super) fn merge_back(&mut self, fork_id: ThreadId) -> Result<()> {
        let fork = self
            .projects
            .thread(fork_id)
            .context("no such thread")?
            .clone();
        let fork_point = fork.forked_from.context("the thread isn't a fork")?;
        anyhow::ensure!(
            self.projects.thread(fork_point.thread).is_some(),
            "the thread it was forked from was deleted"
        );
        let connection = ConnectionId::Thread(fork_id);
        if !self.threads.contains_key(&fork_id) {
            self.update_thread(connection, |_| {})?;
        }
        let running = self.threads.get(&fork_id).context("no such thread")?;
        anyhow::ensure!(
            !running.is_working(),
            "the fork is working; wait for it to finish first"
        );
        anyhow::ensure!(
            running.conversation_revision().is_some(),
            "the fork is still loading its conversation"
        );
        let agent_name = fork
            .agent_id
            .clone()
            .map(|agent_id| self.agent_name(&AgentId::new(agent_id)).to_string())
            .unwrap_or_else(|| running.agent_name().to_string());
        let handoff = merge_handoff(
            running,
            fork_id,
            &fork.title,
            fork_point.entries,
            &agent_name,
        )
        .context("nothing has happened in the fork to merge back yet")?;
        let source = fork_point.thread;
        let handoffs = self.update_thread(ConnectionId::Thread(source), |thread| {
            thread.add_handoff(handoff);
            thread.state.pending_handoffs.clone()
        })?;
        continuations::save(&self.data_dir, source, &handoffs).log_err();
        Ok(())
    }

    /// The thread goes on with `agent_id` on `account` ([`Request::SwitchAgent`]): its agent
    /// stops, keeping the conversation, and the new one starts in the session it had in the
    /// thread, if it was there before, or a new one. Its next message brings what that session
    /// hasn't seen ([`switch_handoff`]).
    pub(super) fn switch_agent(
        &mut self,
        thread_id: ThreadId,
        agent_id: AgentId,
        account: AccountChoice,
    ) -> Result<()> {
        let thread = self
            .projects
            .thread(thread_id)
            .context("no such thread")?
            .clone();
        anyhow::ensure!(thread.terminal.is_none(), "a terminal thread has no agent");
        anyhow::ensure!(
            thread.task.is_none(),
            "a task stays with the agent it was given to"
        );
        anyhow::ensure!(
            self.custom_agents.contains_key(&agent_id)
                || matches!(
                    self.registry.install_state(&agent_id),
                    InstallState::Installed { .. }
                ),
            "{} isn't installed",
            self.agent_name(&agent_id)
        );
        let account = self.choose_account(&agent_id, account)?;
        let to = ThreadAgent {
            agent_id: agent_id.0.to_string(),
            name: self.agent_name(&agent_id).to_string(),
            account,
        };
        let current = thread.agent_id.clone().map(|current| ThreadAgent {
            name: self.agent_name(&AgentId::new(current.clone())).to_string(),
            agent_id: current,
            account: thread.account,
        });
        if current.as_ref().is_some_and(|current| current.is(&to)) {
            return Ok(());
        }
        let connection = ConnectionId::Thread(thread_id);
        // Its conversation is what the new agent is told.
        if !self.threads.contains_key(&thread_id) {
            self.update_thread(connection, |_| {})?;
        }
        let running = self.threads.get(&thread_id).context("no such thread")?;
        anyhow::ensure!(
            !running.is_working(),
            "the thread is working; stop it or wait for it to finish first"
        );
        anyhow::ensure!(
            !(*running.status() == ConnectionStatus::Connecting
                && running.conversation_revision().is_none()
                && thread.session_id.is_some()),
            "the thread is still loading its conversation"
        );
        let pending = running.state.pending_handoffs.clone();
        let tells_everything = pending
            .iter()
            .any(|handoff| handoff.source.tells_everything());
        // A session that was still to be told what it missed has seen only what it saw then,
        // and one whose whole conversation was still to go has seen none of it.
        let seen = if tells_everything {
            0
        } else {
            pending
                .iter()
                .find_map(|handoff| match handoff.source {
                    HandoffSource::Agent { since, .. } => Some(since),
                    _ => None,
                })
                .unwrap_or(running.entries.len())
        };
        let returning =
            self.projects
                .switch_thread_agent(thread_id, to.agent_id.clone(), to.account, seen);
        // The other conversations still go: forks merged back, and a whole conversation this
        // thread hasn't started on yet, which tells the new agent everything.
        let mut handoffs: Vec<PendingHandoff> = pending
            .into_iter()
            .filter(|handoff| !matches!(handoff.source, HandoffSource::Agent { .. }))
            .collect();
        if !tells_everything {
            // From the agent that answered last, which isn't the current one when it was
            // switched to and never answered.
            let from = running
                .state
                .finished_turns
                .iter()
                .rev()
                .find_map(|turn| turn.agent.clone())
                .or(current)
                .unwrap_or_else(|| ThreadAgent {
                    name: running.agent_name().to_string(),
                    ..ThreadAgent::default()
                });
            let since = returning.as_ref().map_or(0, |session| session.seen);
            handoffs.splice(
                0..0,
                switch_handoff(running, thread_id, &thread.title, from, to, since),
            );
        }
        self.stop_agent(thread_id);
        self.tool_sessions
            .retain(|_, session_thread| *session_thread != thread_id);
        continuations::save(&self.data_dir, thread_id, &handoffs).log_err();
        // Starts the new agent, which takes the handoff from its file.
        self.update_thread(connection, |_| {})
    }

    pub(super) fn project_path(&self, project_id: ProjectId) -> Result<PathBuf> {
        Ok(self
            .projects
            .project(project_id)
            .context("no such project")?
            .path
            .clone())
    }

    /// A project's worktree or pasture, or any linked worktree (herdr's Delete worktree
    /// checkout), found from the folder alone.
    fn remove_workspace(&mut self, client: ClientId, id: u64, path: PathBuf, force: bool) {
        if self
            .projects
            .threads_in_folder(&path)
            .into_iter()
            .any(|thread_id| self.projects.is_thread_working(thread_id))
        {
            return self.respond(
                client,
                id,
                Err(anyhow!(
                    "a thread is working there; stop it before removing it"
                )),
            );
        }
        let project_workspace = self.projects.projects().iter().find_map(|project| {
            let workspace = project
                .workspaces
                .iter()
                .find(|workspace| workspace.path == path)?;
            Some((project.id, project.path.clone(), workspace.clone()))
        });
        let project_id = project_workspace
            .as_ref()
            .map(|(project_id, _, _)| *project_id);
        let removing = path.clone();
        let pastures = self
            .data_dir
            .join(workspaces::folder_name(WorkspaceKind::Pasture));
        self.spawn_then(
            async move {
                let (repo, workspace) = match project_workspace {
                    Some((_, repo, workspace)) => (repo, workspace),
                    // A Workspaces thread's pasture, or a removed project's, which is a
                    // repository of its own.
                    None if is_inside(&removing, &pastures).await => {
                        if !force && workspaces::has_unpushed_commits(&removing).await {
                            return Ok(WorkspaceRemoval::NeedsConfirmation(
                                "This pasture has commits no remote has. Removing it loses \
                                 them."
                                    .to_string(),
                            ));
                        }
                        let workspace = Workspace {
                            kind: WorkspaceKind::Pasture,
                            path: removing.clone(),
                            branch: None,
                            base: None,
                            created_at: SystemTime::now(),
                        };
                        // Its own commits were checked above.
                        (removing, workspace)
                    }
                    None => {
                        let repo = workspaces::main_checkout(&removing).await?;
                        anyhow::ensure!(
                            repo != removing,
                            "{} is a repository's main checkout, not a worktree",
                            removing.display()
                        );
                        let workspace = Workspace {
                            kind: WorkspaceKind::Worktree,
                            path: removing,
                            branch: None,
                            base: None,
                            created_at: SystemTime::now(),
                        };
                        (repo, workspace)
                    }
                };
                workspaces::remove(&repo, &workspace, force).await
            },
            move |server, removal| {
                if let Ok(WorkspaceRemoval::Removed) = &removal {
                    if let Some(project_id) = project_id {
                        server.projects.remove_workspace(project_id, &path);
                    }
                    server
                        .storage
                        .checkouts
                        .retain(|checkout| checkout.path != path);
                    // Their agents would be left in a deleted folder.
                    for thread_id in server.projects.threads_in_folder(&path) {
                        server.stop_agent(thread_id);
                    }
                }
                server.respond(client, id, removal.map(Response::WorkspaceRemoval));
            },
        );
    }

    pub(super) fn pasture(
        &self,
        project_id: ProjectId,
        path: &Path,
    ) -> Result<(PathBuf, Workspace)> {
        let repo = self.project_path(project_id)?;
        let workspace = self
            .projects
            .workspace(project_id, path)
            .cloned()
            .with_context(|| format!("{} isn't one of the project's workspaces", path.display()))?;
        anyhow::ensure!(
            workspace.kind == WorkspaceKind::Pasture,
            "worktrees share the project's branches, so there's nothing to sync or bring back"
        );
        Ok((repo, workspace))
    }

    pub(super) fn sync_workspace(
        &self,
        project_id: ProjectId,
        path: &Path,
        branch: Option<String>,
        merge: bool,
    ) -> Result<BoxFuture<'static, Result<String>>> {
        let (repo, workspace) = self.pasture(project_id, path)?;
        Ok(async move {
            let branch = match branch.or(workspace.base) {
                Some(branch) => branch,
                None => workspaces::current_branch(&repo)
                    .await?
                    .context("the project's checkout isn't on a branch; name one to sync from")?,
            };
            workspaces::sync(&repo, &workspace.path, &branch, merge).await
        }
        .boxed())
    }

    pub(super) fn bring_back_workspace(
        &self,
        project_id: ProjectId,
        path: &Path,
        branch: Option<String>,
    ) -> Result<BoxFuture<'static, Result<String>>> {
        let (repo, workspace) = self.pasture(project_id, path)?;
        Ok(async move {
            let branch = match branch {
                Some(branch) => branch,
                None => workspaces::current_branch(&workspace.path)
                    .await?
                    .or(workspace.branch)
                    .context("the pasture isn't on a branch; name the branch to create")?,
            };
            workspaces::bring_back(&repo, &workspace.path, &branch).await
        }
        .boxed())
    }

    fn respond_later(&self, client: ClientId, id: u64, work: BoxFuture<'static, Result<String>>) {
        self.spawn_then(work, move |server, result| {
            server.respond(client, id, result.map(Response::Message))
        });
    }
}

/// Whether `path` is in `folder`, both resolved.
async fn is_inside(path: &Path, folder: &Path) -> bool {
    match (
        tokio::fs::canonicalize(path).await,
        tokio::fs::canonicalize(folder).await,
    ) {
        (Ok(path), Ok(folder)) => path != folder && path.starts_with(&folder),
        _ => false,
    }
}
