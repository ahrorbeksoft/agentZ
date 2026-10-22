//! A draft's new worktree or pasture, made as its first message is sent (t3code's thread
//! setup). The message waits while the steps run, shown in the thread; then the agent starts
//! there and gets it. A failed step waits for Retry or Use Local. Once the thread has a title,
//! the workspace's temporary branch is renamed after it.
//!
//! A workspace a cleanup removed (workspace_cleanup.rs) is made again the same way with its
//! thread's next message, at its folder and on its branch, and the agent goes on in its
//! session there. Until then the thread shows its conversation without an agent.

use std::path::PathBuf;
use std::time::Instant;

use agent_thread::{AgentThread, Attachments};
use agentz_protocol::accounts::AccountId;
use agentz_protocol::agents::AgentId;
use agentz_protocol::thread::ThreadAgent;
use agentz_protocol::workspace::{SetupStepKind, SetupStepState, WorkspaceSetup, remote_branch};
use agentz_protocol::{ConnectionId, PromptPart, Response};
use anyhow::{Context as _, Result, anyhow};
use futures::FutureExt as _;
use futures::future::BoxFuture;
use projects::{PlannedWorkspace, ThreadId, Workspace};
use util::ResultExt as _;

use super::Server;
use crate::workspaces::{self, NewWorkspace};
use crate::{continuations, transcripts};

/// A first message waiting for its thread's workspace.
pub(super) struct SetupRun {
    /// The folder the draft is in, which the workspace is made from.
    folder: PathBuf,
    plan: PlannedWorkspace,
    prompt: Vec<PromptPart>,
    setup: WorkspaceSetup,
    /// The repository to make it in and what its branch starts from, once known.
    resolved: Option<(PathBuf, String)>,
    made: Option<Workspace>,
    step_started: Instant,
    /// A removed workspace to make again, with the commit it was at.
    remake: Option<(Workspace, Option<String>)>,
}

impl Server {
    /// Where a draft will work ([`agentz_protocol::Request::PlanWorkspace`]).
    pub(super) fn plan_workspace(
        &mut self,
        thread_id: ThreadId,
        plan: Option<PlannedWorkspace>,
    ) -> Result<Response> {
        let thread = self.projects.thread(thread_id).context("no such thread")?;
        anyhow::ensure!(
            thread.is_draft && thread.terminal.is_none(),
            "only a new thread's workspace can be chosen"
        );
        let in_own_folder = match &thread.workspace {
            None => true,
            Some(_) => thread.in_workspaces() && thread.started_in.is_none(),
        };
        anyhow::ensure!(
            in_own_folder,
            "the thread works in an existing worktree or pasture; start a new thread instead"
        );
        self.projects.set_planned_workspace(thread_id, plan);
        Ok(Response::Ok)
    }

    /// Whether the thread's first message waits for its workspace.
    pub(super) fn has_workspace_setup(&self, thread_id: ThreadId) -> bool {
        self.workspace_setups.contains_key(&thread_id)
    }

    /// Holds a draft's first message and starts making its planned workspace. The draft
    /// becomes a thread now, titled after the message, as sending makes one.
    pub(super) fn set_up_workspace(
        &mut self,
        thread_id: ThreadId,
        plan: PlannedWorkspace,
        prompt: Vec<PromptPart>,
    ) -> Result<()> {
        let folder = self
            .projects
            .thread_folder(thread_id)
            .context("no such thread")?;
        let message = prompt_text(&prompt);
        let setup = WorkspaceSetup::new(
            plan.kind,
            plan.base.clone().unwrap_or_default(),
            message.clone(),
            workspaces::has_submodules(&folder),
        );
        self.workspace_setups.insert(
            thread_id,
            SetupRun {
                folder,
                plan,
                prompt,
                setup,
                resolved: None,
                made: None,
                step_started: Instant::now(),
                remake: None,
            },
        );
        if !message.is_empty() {
            self.projects
                .rename_thread(thread_id, super::thread_title_from_prompt(&message));
        }
        self.projects.set_draft(thread_id, false);
        self.draft_due.remove(&thread_id);
        self.projects.set_thread_working(thread_id, true);
        self.advance_workspace_setup(thread_id);
        Ok(())
    }

    /// Whether the thread's worktree or pasture is gone, so its agent can't start: a cleanup
    /// removed it, or is removing it.
    pub(super) fn workspace_is_gone(&self, thread_id: ThreadId) -> bool {
        self.projects
            .thread_workspace(thread_id)
            .is_some_and(|workspace| {
                !workspace.path.exists() || self.cleanups.removing.contains(&workspace.path)
            })
    }

    /// The thread while its workspace is gone: its conversation as kept, with no agent.
    pub(super) fn thread_without_folder(
        &mut self,
        thread_id: ThreadId,
        agent_id: &AgentId,
        account: Option<AccountId>,
        cwd: PathBuf,
    ) -> AgentThread {
        let agent_name = self.agent_name(agent_id);
        let mut agent_thread = AgentThread::without_agent(agent_name.clone(), cwd);
        agent_thread.set_agent(ThreadAgent {
            agent_id: agent_id.0.to_string(),
            name: agent_name.to_string(),
            account,
        });
        if let Some(handoffs) = continuations::load(&self.data_dir, thread_id).log_err() {
            agent_thread.set_handoffs(handoffs);
        }
        if let Some(transcript) = transcripts::load(&self.data_dir, thread_id)
            .log_err()
            .flatten()
        {
            agent_thread.restore_transcript(transcript);
        }
        self.end_subagent_cards(thread_id, &mut agent_thread);
        self.saved_transcripts.remove(&thread_id);
        if let Some(revision) = agent_thread.conversation_revision() {
            self.saved_transcripts.insert(thread_id, revision);
        }
        agent_thread.set_attachments(Attachments::for_thread(&self.data_dir, thread_id));
        let (queued_messages, steering) = self.queues.state(thread_id);
        agent_thread.set_queued_messages(queued_messages, steering);
        agent_thread.set_workspace_setup(self.workspace_setup(thread_id));
        agent_thread
    }

    /// Makes the thread's workspace again if a cleanup removed it, holding the message until
    /// it's there, as t3code's next turn does. The message comes back when there's nothing to
    /// make.
    pub(super) fn remake_workspace(
        &mut self,
        thread_id: ThreadId,
        prompt: Vec<PromptPart>,
    ) -> Result<Option<Vec<PromptPart>>> {
        if !self.workspace_is_gone(thread_id) {
            return Ok(Some(prompt));
        }
        let workspace = self
            .projects
            .thread_workspace(thread_id)
            .cloned()
            .context("the thread has no workspace")?;
        anyhow::ensure!(
            !self.cleanups.removing.contains(&workspace.path),
            "the thread's {} is being removed by a cleanup; send the message again in a moment",
            workspace.kind.label().to_lowercase()
        );
        let branch = workspace
            .branch
            .clone()
            .context("the thread's workspace had no branch to make it again on")?;
        let repo = self
            .projects
            .projects()
            .iter()
            .find(|project| {
                project
                    .workspaces
                    .iter()
                    .any(|kept| kept.path == workspace.path)
            })
            .map(|project| project.path.clone())
            .context("the workspace's project is gone")?;
        let setup = WorkspaceSetup::remake(
            workspace.kind,
            branch,
            prompt_text(&prompt),
            workspaces::has_submodules(&repo),
        );
        let head = self.cleanups.removed_head(&workspace.path);
        self.workspace_setups.insert(
            thread_id,
            SetupRun {
                folder: repo.clone(),
                plan: PlannedWorkspace {
                    kind: workspace.kind,
                    base: workspace.base.clone(),
                    branch: workspace.branch.clone(),
                },
                prompt,
                setup,
                resolved: Some((repo, workspace.base.clone().unwrap_or_default())),
                made: None,
                step_started: Instant::now(),
                remake: Some((workspace, head)),
            },
        );
        self.projects.set_thread_working(thread_id, true);
        self.advance_workspace_setup(thread_id);
        Ok(None)
    }

    /// [`agentz_protocol::Request::RetryWorkspaceSetup`]: from the step that failed.
    pub(super) fn retry_workspace_setup(&mut self, thread_id: ThreadId) -> Result<Response> {
        let run = self
            .workspace_setups
            .get_mut(&thread_id)
            .context("the thread's workspace isn't being made")?;
        anyhow::ensure!(run.setup.is_failed(), "the workspace is still being made");
        run.setup.error = None;
        for step in &mut run.setup.steps {
            if step.state == SetupStepState::Failed {
                step.state = SetupStepState::Waiting;
            }
        }
        self.projects.set_thread_working(thread_id, true);
        self.advance_workspace_setup(thread_id);
        Ok(Response::Ok)
    }

    /// [`agentz_protocol::Request::UseLocal`]: the message goes to the agent where the draft
    /// was.
    pub(super) fn use_local(&mut self, thread_id: ThreadId) -> Result<Response> {
        let run = self
            .workspace_setups
            .get(&thread_id)
            .context("the thread's workspace isn't being made")?;
        anyhow::ensure!(
            run.remake.is_none(),
            "the thread's {} is made again where it was; retry instead",
            run.setup.kind.label().to_lowercase()
        );
        anyhow::ensure!(run.setup.is_failed(), "the workspace is still being made");
        let run = self
            .workspace_setups
            .remove(&thread_id)
            .context("the thread's workspace isn't being made")?;
        self.projects.set_planned_workspace(thread_id, None);
        // The draft's session never had a message, so there's nothing for a new agent to load.
        if !self.threads.contains_key(&thread_id) {
            self.projects.forget_thread_session(thread_id);
        }
        let connection = ConnectionId::Thread(thread_id);
        self.update_thread(connection, |thread| thread.set_workspace_setup(None))?;
        self.prompt(connection, run.prompt, None)
    }

    /// Runs the next step, or starts the agent in the workspace once they're all done.
    fn advance_workspace_setup(&mut self, thread_id: ThreadId) {
        let data_dir = self.data_dir.clone();
        let in_workspaces = self
            .projects
            .thread(thread_id)
            .is_some_and(|thread| thread.in_workspaces());
        let Some(run) = self.workspace_setups.get_mut(&thread_id) else {
            return;
        };
        if run.setup.is_failed() {
            return;
        }
        let Some((repo, base)) = run.resolved.clone() else {
            let folder = run.folder.clone();
            let base = run.plan.base.clone();
            self.spawn_then(
                async move {
                    let (main_checkout, base) =
                        workspaces::repository_and_base(&folder, base).await?;
                    // A project's are made from its folder, as agents' are.
                    let repo = if in_workspaces { main_checkout } else { folder };
                    anyhow::Ok((repo, base))
                },
                move |server, resolved| {
                    let Some(run) = server.workspace_setups.get_mut(&thread_id) else {
                        return;
                    };
                    match resolved {
                        Ok((repo, base)) => {
                            run.setup.base = base.clone();
                            run.resolved = Some((repo, base));
                            server.advance_workspace_setup(thread_id);
                        }
                        Err(error) => server.workspace_setup_failed(thread_id, 0, error),
                    }
                },
            );
            return;
        };
        let Some(index) = run
            .setup
            .steps
            .iter()
            .position(|step| step.state != SetupStepState::Done)
        else {
            return self.finish_workspace_setup(thread_id);
        };
        let step = &mut run.setup.steps[index];
        step.state = SetupStepState::Running;
        let kind = step.kind;
        run.step_started = Instant::now();
        let work: BoxFuture<'static, Result<Option<Workspace>>> = match kind {
            SetupStepKind::Fetch => {
                let folder = run.folder.clone();
                let branch = remote_branch(&base).unwrap_or(&base).to_string();
                async move {
                    workspaces::fetch_base(&folder, &branch).await?;
                    Ok(None)
                }
                .boxed()
            }
            SetupStepKind::CheckOut if run.remake.is_some() => {
                let remake = run.remake.clone();
                async move {
                    let (workspace, head) = remake.context("nothing to make again")?;
                    workspaces::recreate(&repo, &workspace, head.as_deref()).await?;
                    Ok(Some(workspace))
                }
                .boxed()
            }
            SetupStepKind::CheckOut => {
                let new = NewWorkspace {
                    kind: run.plan.kind,
                    repo,
                    data_dir,
                    base: Some(base),
                    branch: run.plan.branch.clone(),
                    submodules: false,
                };
                async move { workspaces::create(new).await.map(Some) }.boxed()
            }
            SetupStepKind::Submodules => {
                let path = run.made.as_ref().map(|workspace| workspace.path.clone());
                async move {
                    // Best effort, as for every new workspace.
                    if let Some(path) = path
                        && let Err(error) = workspaces::init_submodules(&path).await
                    {
                        log::warn!("submodules in {} are empty: {error:#}", path.display());
                    }
                    Ok(None)
                }
                .boxed()
            }
        };
        self.show_workspace_setup(thread_id);
        self.spawn_then(work, move |server, result| {
            server.workspace_setup_step_ended(thread_id, index, result)
        });
    }

    fn workspace_setup_step_ended(
        &mut self,
        thread_id: ThreadId,
        index: usize,
        result: Result<Option<Workspace>>,
    ) {
        let thread_is_gone = self.projects.thread(thread_id).is_none();
        if thread_is_gone {
            if let Some(mut run) = self.workspace_setups.remove(&thread_id) {
                if let Ok(Some(workspace)) = result {
                    run.made = Some(workspace);
                }
                self.keep_unused_workspace(run);
            }
            return;
        }
        let Some(run) = self.workspace_setups.get_mut(&thread_id) else {
            return;
        };
        match result {
            Ok(made) => {
                if let Some(step) = run.setup.steps.get_mut(index) {
                    step.state = SetupStepState::Done;
                    step.took = Some(run.step_started.elapsed());
                }
                if made.is_some() {
                    run.made = made;
                }
                self.advance_workspace_setup(thread_id);
            }
            Err(error) => self.workspace_setup_failed(thread_id, index, error),
        }
    }

    /// Drops a deleted thread's setup. One with a step running ends when the step does.
    pub(super) fn drop_workspace_setup(&mut self, thread_id: ThreadId) {
        let is_running = self.workspace_setups.get(&thread_id).is_some_and(|run| {
            run.setup
                .steps
                .iter()
                .any(|step| step.state == SetupStepState::Running)
        });
        if !is_running && let Some(run) = self.workspace_setups.remove(&thread_id) {
            self.keep_unused_workspace(run);
        }
    }

    /// A workspace made for a thread that's gone is kept as one of the project's, where it can
    /// be removed.
    fn keep_unused_workspace(&mut self, run: SetupRun) {
        if let Some(workspace) = run.made {
            let repo = run.resolved.map(|(repo, _)| repo).unwrap_or(run.folder);
            self.adopt_repository_workspace(&repo, workspace);
        }
    }

    fn workspace_setup_failed(&mut self, thread_id: ThreadId, index: usize, error: anyhow::Error) {
        let Some(run) = self.workspace_setups.get_mut(&thread_id) else {
            return;
        };
        log::warn!(
            "couldn't make thread {}'s {}: {error:#}",
            thread_id.0,
            run.setup.kind.label().to_lowercase()
        );
        if let Some(step) = run.setup.steps.get_mut(index) {
            step.state = SetupStepState::Failed;
        }
        run.setup.error = Some(format!("{error:#}"));
        self.projects.set_thread_working(thread_id, false);
        self.show_workspace_setup(thread_id);
    }

    /// Moves the thread into its new workspace, and sends its message to an agent started
    /// there, as a thread handed off to a workspace moves.
    fn finish_workspace_setup(&mut self, thread_id: ThreadId) {
        let Some(run) = self.workspace_setups.remove(&thread_id) else {
            return;
        };
        if let Some((workspace, _)) = run.remake {
            self.forget_removed_workspace(&workspace.path);
            // The agent starts there now, and goes on in its session.
            self.stop_agent(thread_id);
            self.prompt(ConnectionId::Thread(thread_id), run.prompt, None)
                .log_err();
            return;
        }
        let (Some((repo, _)), Some(workspace)) = (run.resolved, run.made) else {
            log::error!(
                "thread {}'s workspace setup ended without a workspace",
                thread_id.0
            );
            return;
        };
        let Some(thread) = self.projects.thread(thread_id) else {
            return;
        };
        let project_id = thread.project_id;
        let in_workspaces = thread.in_workspaces();
        let path = if in_workspaces {
            self.adopt_repository_workspace(&repo, workspace)
        } else {
            self.adopt_workspace(project_id, workspace)
        };
        self.projects
            .set_thread_workspace(thread_id, Some(path.clone()));
        if in_workspaces {
            self.projects.set_started_in(thread_id, Some(run.folder));
        }
        self.projects.set_planned_workspace(thread_id, None);
        if run.plan.branch.is_none() {
            self.branches_to_name.insert(thread_id, path);
        }
        self.stop_agent(thread_id);
        self.tool_sessions
            .retain(|_, session_thread| *session_thread != thread_id);
        // Its session was opened in the old folder, and never had a message.
        self.projects.forget_thread_session(thread_id);
        self.prompt(ConnectionId::Thread(thread_id), run.prompt, None)
            .log_err();
    }

    /// Puts the setup in the thread's state, which its clients get.
    pub(super) fn show_workspace_setup(&mut self, thread_id: ThreadId) {
        let setup = self
            .workspace_setups
            .get(&thread_id)
            .map(|run| run.setup.clone());
        self.update_thread(ConnectionId::Thread(thread_id), |thread| {
            thread.set_workspace_setup(setup)
        })
        .log_err();
    }

    /// The setup to show in a thread whose agent is starting.
    pub(super) fn workspace_setup(&self, thread_id: ThreadId) -> Option<WorkspaceSetup> {
        self.workspace_setups
            .get(&thread_id)
            .map(|run| run.setup.clone())
    }

    /// Renames a workspace made with its thread's first message after the thread's title, once
    /// it has one (the agent's, or the title generator's): t3code's branch naming.
    pub(super) fn name_branch_after_title(&mut self, thread_id: ThreadId) {
        let Some(path) = self.branches_to_name.remove(&thread_id) else {
            return;
        };
        let Some(thread) = self.projects.thread(thread_id) else {
            return;
        };
        let title = thread.title.clone();
        let Some(workspace) = self
            .projects
            .thread_workspace(thread_id)
            .filter(|workspace| workspace.path == path)
        else {
            return;
        };
        let Some(from) = workspace.branch.clone() else {
            return;
        };
        let Some(repo) = self
            .projects
            .projects()
            .iter()
            .find(|project| {
                project
                    .workspaces
                    .iter()
                    .any(|workspace| workspace.path == path)
            })
            .map(|project| project.path.clone())
        else {
            return;
        };
        let to = workspaces::branch_for_title(&title);
        let renaming = path.clone();
        self.spawn_then(
            async move {
                let renamed = workspaces::rename_branch(&repo, &renaming, &from, &to).await?;
                anyhow::Ok(renamed.then_some(to))
            },
            move |server, renamed| match renamed {
                Ok(Some(branch)) => server.projects.set_workspace_branch(&path, branch),
                Ok(None) => {}
                Err(error) => log::warn!(
                    "couldn't rename the branch of {}: {error:#}",
                    path.display()
                ),
            },
        );
    }

    /// A message sent while the first one waits for the workspace.
    pub(super) fn refuse_during_setup(&self, thread_id: ThreadId) -> Result<()> {
        match self.workspace_setups.get(&thread_id) {
            Some(run) if run.setup.is_failed() => Err(anyhow!(
                "the thread's {} couldn't be made; retry, or use the local checkout",
                run.setup.kind.label().to_lowercase()
            )),
            Some(run) => Err(anyhow!(
                "the thread's {} is still being made; queue the message",
                run.setup.kind.label().to_lowercase()
            )),
            None => Ok(()),
        }
    }
}

/// A message's text, shown as the user's while it waits.
fn prompt_text(prompt: &[PromptPart]) -> String {
    prompt
        .iter()
        .filter_map(|part| match part {
            PromptPart::Text(text) => Some(text.as_str()),
            _ => None,
        })
        .collect::<String>()
        .trim()
        .to_string()
}
