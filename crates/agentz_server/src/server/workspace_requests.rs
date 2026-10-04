//! Requests about worktrees and pastures, which take git work off the server's task and come
//! back to change its state.

use std::path::{Path, PathBuf};

use agentz_protocol::agents::AgentId;
use agentz_protocol::terminal::{TerminalCommand, TerminalKey};
use agentz_protocol::thread::{ConnectionStatus, handoff};
use agentz_protocol::workspace::{WorkspaceChoice, WorkspaceRemoval};
use agentz_protocol::{ConnectionId, Request, Response};
use anyhow::{Context as _, Result, anyhow};
use futures::FutureExt as _;
use futures::future::BoxFuture;
use projects::{ProjectId, ThreadId, Workspace, WorkspaceKind};

use super::{ClientId, Server};
use crate::continuations;
use crate::workspaces::{self, NewWorkspace};
use util::ResultExt as _;

/// What a new thread runs.
pub(super) enum NewThread {
    Agent(AgentId),
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
            } => self.create_thread(
                client,
                id,
                project_id,
                NewThread::Agent(agent_id),
                workspace,
            ),
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
                        server.respond(client, id, checkouts.map(Response::RepositoryCheckouts))
                    },
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
                        // A project's worktree is one of its workspaces, for its threads.
                        let folder = created.map(|(repo, workspace)| {
                            let project_id = server
                                .projects
                                .projects()
                                .iter()
                                .find(|project| project.path == repo)
                                .map(|project| project.id);
                            match project_id {
                                Some(project_id) => server.adopt_workspace(project_id, workspace),
                                None => workspace.path,
                            }
                        });
                        server.respond(client, id, folder.map(Response::WorkspaceCreated));
                    },
                );
            }
            Request::RemoveWorkspace {
                project_id,
                path,
                force,
            } => self.remove_workspace(client, id, project_id, path, force),
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

    /// Checks the choice now; a new workspace is made by the returned work.
    pub(super) fn prepare_workspace(
        &self,
        project_id: ProjectId,
        choice: WorkspaceChoice,
    ) -> Result<PreparedWorkspace> {
        let repo = self.project_path(project_id)?;
        match choice {
            WorkspaceChoice::Checkout => Ok(PreparedWorkspace::Ready(None)),
            WorkspaceChoice::Existing(path) if path == repo => Ok(PreparedWorkspace::Ready(None)),
            WorkspaceChoice::Existing(path) => {
                self.projects
                    .workspace(project_id, &path)
                    .with_context(|| {
                        format!("{} isn't one of the project's workspaces", path.display())
                    })?;
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
    /// or terminal, so it's ready by the time the user has typed something.
    pub(super) fn create_thread_in(
        &mut self,
        project_id: ProjectId,
        new: NewThread,
        folder: Option<PathBuf>,
    ) -> Result<ThreadId> {
        let thread_id = match &new {
            NewThread::Agent(agent_id) => self.projects.add_thread(
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
            NewThread::Agent(_) => {
                self.update_thread(ConnectionId::Thread(thread_id), |_| {})?;
            }
            NewThread::Terminal(_) => {
                self.ensure_terminal(&TerminalKey::Thread(thread_id))?;
            }
        }
        Ok(thread_id)
    }

    /// "Continue with another agent": a thread with `agent_id` in `thread_id`'s workspace,
    /// whose first message brings `thread_id`'s conversation ([`handoff`]), t3code's context
    /// handoff. The conversation is the one the running agent replayed, so the thread is open.
    pub(super) fn continue_thread(
        &mut self,
        thread_id: ThreadId,
        agent_id: AgentId,
    ) -> Result<ThreadId> {
        let thread = self
            .projects
            .thread(thread_id)
            .context("no such thread")?
            .clone();
        anyhow::ensure!(
            thread.terminal.is_none(),
            "a terminal thread has no conversation to continue"
        );
        let running = self
            .threads
            .get(&thread_id)
            .context("the thread's agent isn't running; open the thread first")?;
        anyhow::ensure!(
            *running.status() != ConnectionStatus::Connecting,
            "the thread is still loading its conversation"
        );
        let from_agent = thread
            .agent_id
            .clone()
            .map(|agent_id| self.agent_name(&AgentId::new(agent_id)))
            .unwrap_or_else(|| running.agent_name().clone());
        let handoff = handoff(running, &from_agent, &thread.title);
        anyhow::ensure!(
            handoff.messages > 0,
            "the thread has no conversation to continue yet"
        );
        let choice = thread
            .workspace
            .clone()
            .map_or(WorkspaceChoice::Checkout, WorkspaceChoice::Existing);
        let PreparedWorkspace::Ready(folder) = self.prepare_workspace(thread.project_id, choice)?
        else {
            return Err(anyhow!("the thread's workspace isn't ready"));
        };
        let new_thread =
            self.create_thread_in(thread.project_id, NewThread::Agent(agent_id), folder)?;
        self.projects.set_continued_from(new_thread, thread_id);
        continuations::save(&self.data_dir, new_thread, &handoff).log_err();
        self.update_thread(ConnectionId::Thread(new_thread), |thread| {
            thread.set_handoff(Some(handoff))
        })?;
        Ok(new_thread)
    }

    pub(super) fn project_path(&self, project_id: ProjectId) -> Result<PathBuf> {
        Ok(self
            .projects
            .project(project_id)
            .context("no such project")?
            .path
            .clone())
    }

    fn remove_workspace(
        &mut self,
        client: ClientId,
        id: u64,
        project_id: ProjectId,
        path: PathBuf,
        force: bool,
    ) {
        let prepared = (|| {
            let repo = self.project_path(project_id)?;
            let workspace = self
                .projects
                .workspace(project_id, &path)
                .cloned()
                .with_context(|| {
                    format!("{} isn't one of the project's workspaces", path.display())
                })?;
            anyhow::ensure!(
                !self
                    .projects
                    .threads_in_folder(&path)
                    .into_iter()
                    .any(|thread_id| self.projects.is_thread_working(thread_id)),
                "a thread is working there; stop it before removing the {}",
                workspace.kind.label().to_lowercase()
            );
            Ok((repo, workspace))
        })();
        let (repo, workspace) = match prepared {
            Ok(prepared) => prepared,
            Err(error) => return self.respond(client, id, Err(error)),
        };
        self.spawn_then(
            async move { workspaces::remove(&repo, &workspace, force).await },
            move |server, removal| {
                if let Ok(WorkspaceRemoval::Removed) = &removal {
                    server.projects.remove_workspace(project_id, &path);
                    // Their agents would be left in a deleted folder.
                    for thread_id in server.projects.threads_in_folder(&path) {
                        server.threads.remove(&thread_id);
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
