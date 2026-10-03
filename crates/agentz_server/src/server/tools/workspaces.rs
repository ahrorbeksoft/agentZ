//! The workspace tools, after t3code's worktree tools: an agent reads where its thread works,
//! lists the repository's branches and workspaces, moves its thread into a new worktree or
//! pasture, and syncs a pasture or brings its branch back. `workspaceStrategy` lets launched
//! threads and delegated tasks start in one.

use std::path::PathBuf;

use agentz_protocol::ConnectionId;
use agentz_protocol::workspace::{PastureSupport, WorkspaceChoice};
use futures::FutureExt as _;
use futures::future::BoxFuture;
use projects::{ThreadCreator, ThreadId, Workspace, WorkspaceKind};
use serde_json::{Value, json};
use util::ResultExt as _;

use super::{
    Arguments, Caller, Continuation, Failure, MAX_PROMPT_CHARS, Outcome, Server, Step, failure,
    invalid,
};
use crate::server::workspace_requests::PreparedWorkspace;
use crate::workspaces;

const MAX_BRANCH_CHARS: usize = 256;
const MAX_PATH_CHARS: usize = 4096;
const DEFAULT_BRANCH_LIMIT: u64 = 100;

/// Where a launched thread works.
#[derive(Clone, Debug, Default)]
pub(super) enum Placement {
    /// The tool's default: the project's checkout for a launched thread, the parent's folder
    /// for a delegated task.
    #[default]
    Default,
    Choice(WorkspaceChoice),
}

/// A launched thread's folder, once any new workspace is made.
pub(super) enum Folder {
    Default,
    /// The project's checkout (`None`) or a workspace.
    Chosen(Option<PathBuf>),
}

enum Preparing {
    Ready(Folder),
    Create(BoxFuture<'static, anyhow::Result<Workspace>>),
}

enum Made {
    Ready(Folder),
    Created(anyhow::Result<Workspace>),
}

pub(super) fn kind_name(kind: WorkspaceKind) -> &'static str {
    match kind {
        WorkspaceKind::Worktree => "worktree",
        WorkspaceKind::Pasture => "pasture",
    }
}

fn workspace_failure(error: anyhow::Error) -> Failure {
    failure("operation_failed", format!("{error:#}"))
}

/// The `workspaceStrategy` argument, as in t3code: `root`, `worktree` or `pasture` with an
/// optional `baseRef` and `branch`, or `existing` with a `path`.
pub(super) fn workspace_strategy(arguments: &Arguments) -> Result<Placement, Failure> {
    let strategy = match arguments.0.get("workspaceStrategy") {
        None | Some(Value::Null) => return Ok(Placement::Default),
        Some(Value::Object(strategy)) => Arguments(strategy),
        Some(_) => return Err(invalid("workspaceStrategy must be an object.")),
    };
    let choice = match strategy.string("type", 16)? {
        Some("root") => WorkspaceChoice::Checkout,
        Some(kind @ ("worktree" | "pasture")) => WorkspaceChoice::New {
            kind: if kind == "worktree" {
                WorkspaceKind::Worktree
            } else {
                WorkspaceKind::Pasture
            },
            base: strategy
                .string("baseRef", MAX_BRANCH_CHARS)?
                .map(String::from),
            branch: strategy
                .string("branch", MAX_BRANCH_CHARS)?
                .map(String::from),
        },
        Some("existing") => WorkspaceChoice::Existing(
            strategy
                .string("path", MAX_PATH_CHARS)?
                .map(PathBuf::from)
                .ok_or_else(|| invalid("workspaceStrategy type existing needs a path."))?,
        ),
        Some(kind) => {
            return Err(invalid(format!(
                "Unknown workspaceStrategy type {kind}. Types: root, worktree, pasture, existing."
            )));
        }
        None => return Err(invalid("workspaceStrategy needs a type.")),
    };
    Ok(Placement::Choice(choice))
}

impl Server {
    /// Makes the new workspaces the placements ask for, then calls `then` with each folder.
    /// All choices are checked first, so nothing is made for a call that would fail.
    pub(super) fn with_folders(
        &mut self,
        caller: Caller,
        placements: Vec<Placement>,
        then: impl FnOnce(&mut Server, Vec<Folder>) -> Outcome + Send + 'static,
    ) -> Outcome {
        let mut preparing = Vec::new();
        for placement in placements {
            preparing.push(match placement {
                Placement::Default => Preparing::Ready(Folder::Default),
                Placement::Choice(choice) => match self
                    .prepare_workspace(caller.project_id, choice)
                    .map_err(|error| invalid(format!("{error:#}")))?
                {
                    PreparedWorkspace::Ready(folder) => Preparing::Ready(Folder::Chosen(folder)),
                    PreparedWorkspace::Create(work) => Preparing::Create(work),
                },
            });
        }
        if preparing
            .iter()
            .all(|preparing| matches!(preparing, Preparing::Ready(_)))
        {
            let folders = preparing
                .into_iter()
                .filter_map(|preparing| match preparing {
                    Preparing::Ready(folder) => Some(folder),
                    Preparing::Create(_) => None,
                })
                .collect();
            return then(self, folders);
        }
        let project_id = caller.project_id;
        Ok(Step::Then(
            async move {
                let made =
                    futures::future::join_all(preparing.into_iter().map(|preparing| async move {
                        match preparing {
                            Preparing::Ready(folder) => Made::Ready(folder),
                            Preparing::Create(work) => Made::Created(work.await),
                        }
                    }))
                    .await;
                Box::new(move |server: &mut Server| {
                    let mut folders = Vec::new();
                    let mut error = None;
                    for made in made {
                        match made {
                            Made::Ready(folder) => folders.push(folder),
                            // Kept even if another failed, so it shows in Checkouts rather
                            // than being left behind unseen.
                            Made::Created(Ok(workspace)) => {
                                let folder = server.adopt_workspace(project_id, workspace);
                                folders.push(Folder::Chosen(Some(folder)));
                            }
                            Made::Created(Err(failed)) => error = Some(failed),
                        }
                    }
                    match error {
                        Some(error) => Err(workspace_failure(error)),
                        None => then(server, folders),
                    }
                }) as Continuation
            }
            .boxed(),
        ))
    }

    pub(super) fn workspace_status(&self, caller: Caller) -> Outcome {
        let thread_id = calling_thread(caller)?;
        let workspace = self.projects.thread_workspace(thread_id).cloned();
        let folder = self
            .projects
            .thread_folder(thread_id)
            .ok_or_else(|| failure("thread_not_found", "This thread was deleted."))?;
        let root = self
            .project_path(caller.project_id)
            .map_err(|_| failure("project_not_found", "This thread's project was removed."))?;
        Ok(Step::Background(
            async move {
                let branch = workspaces::current_branch(&folder).await.ok().flatten();
                Ok(json!({
                    "threadId": thread_id.0,
                    "kind": workspace.as_ref().map_or("checkout", |workspace| kind_name(workspace.kind)),
                    "attached": workspace.is_some(),
                    "folder": folder,
                    "branch": branch.or_else(|| workspace.as_ref().and_then(|workspace| workspace.branch.clone())),
                    "baseRef": workspace.as_ref().and_then(|workspace| workspace.base.clone()),
                    "projectWorkspaceRoot": root,
                }))
            }
            .boxed(),
        ))
    }

    pub(super) fn workspace_list(&self, caller: Caller, arguments: &Arguments) -> Outcome {
        let query = arguments
            .string("query", MAX_BRANCH_CHARS)?
            .map(str::to_lowercase);
        let limit = arguments
            .number("limit")?
            .unwrap_or(DEFAULT_BRANCH_LIMIT)
            .clamp(1, 1_000) as usize;
        let root = self
            .project_path(caller.project_id)
            .map_err(|_| failure("project_not_found", "This thread's project was removed."))?;
        let data_dir = self.data_dir.clone();
        let workspaces: Vec<(Workspace, Vec<ThreadId>)> = self
            .projects
            .project(caller.project_id)
            .map(|project| {
                project
                    .workspaces
                    .iter()
                    .map(|workspace| {
                        let threads = self.projects.threads_in_folder(&workspace.path);
                        (workspace.clone(), threads)
                    })
                    .collect()
            })
            .unwrap_or_default();
        Ok(Step::Background(
            async move {
                let git = workspaces::project_git(&root, &data_dir).await;
                if !git.is_repository {
                    return Ok(json!({
                        "isRepository": false,
                        "projectWorkspaceRoot": root,
                        "branches": [],
                        "workspaces": [],
                    }));
                }
                let mut checkouts = vec![("checkout", root.clone(), git.branch.clone())];
                let mut listed = Vec::new();
                for (workspace, threads) in workspaces {
                    let branch = workspaces::current_branch(&workspace.path)
                        .await
                        .ok()
                        .flatten();
                    checkouts.push((kind_name(workspace.kind), workspace.path.clone(), branch.clone()));
                    listed.push(json!({
                        "kind": kind_name(workspace.kind),
                        "path": workspace.path,
                        "branch": branch.or(workspace.branch),
                        "baseRef": workspace.base,
                        "exists": workspace.path.exists(),
                        "threadIds": threads.iter().map(|thread| thread.0).collect::<Vec<_>>(),
                    }));
                }
                let matching: Vec<&String> = git
                    .branches
                    .iter()
                    .filter(|branch| {
                        query
                            .as_ref()
                            .is_none_or(|query| branch.to_lowercase().contains(query))
                    })
                    .collect();
                let branches: Vec<Value> = matching
                    .iter()
                    .take(limit)
                    .map(|branch| {
                        let checkouts: Vec<Value> = checkouts
                            .iter()
                            .filter(|(_, _, checked_out)| checked_out.as_ref() == Some(*branch))
                            .map(|(kind, path, _)| json!({"kind": kind, "path": path}))
                            .collect();
                        json!({"name": branch, "checkouts": checkouts})
                    })
                    .collect();
                Ok(json!({
                    "isRepository": true,
                    "projectWorkspaceRoot": root,
                    "currentBranch": git.branch,
                    "branches": branches,
                    "hasMoreBranches": matching.len() > limit,
                    "workspaces": listed,
                    "pastures": match git.pastures {
                        PastureSupport::CopyOnWrite => "copy_on_write",
                        PastureSupport::FullCopy => "full_copy",
                        PastureSupport::Unsupported(_) | PastureSupport::Unknown(_) => "unsupported",
                    },
                }))
            }
            .boxed(),
        ))
    }

    pub(super) fn workspace_handoff(&mut self, caller: Caller, arguments: &Arguments) -> Outcome {
        let thread_id = calling_thread(caller)?;
        if let Some(workspace) = self.projects.thread_workspace(thread_id) {
            return Err(failure(
                "already_in_workspace",
                format!(
                    "This thread already works in a {} at {}.",
                    kind_name(workspace.kind),
                    workspace.path.display()
                ),
            ));
        }
        if self.moving_threads.contains_key(&thread_id) {
            return Err(failure(
                "handoff_in_progress",
                "This thread is already moving to a new workspace.",
            ));
        }
        let kind = match arguments.string("type", 16)? {
            Some("worktree") => WorkspaceKind::Worktree,
            Some("pasture") => WorkspaceKind::Pasture,
            Some(kind) => {
                return Err(invalid(format!(
                    "Unknown type {kind}. Types: worktree, pasture."
                )));
            }
            None => return Err(invalid("type is required.")),
        };
        let choice = WorkspaceChoice::New {
            kind,
            base: arguments
                .string("baseRef", MAX_BRANCH_CHARS)?
                .map(String::from),
            branch: arguments
                .string("branch", MAX_BRANCH_CHARS)?
                .map(String::from),
        };
        let continuation = arguments
            .string("continuationPrompt", MAX_PROMPT_CHARS)?
            .map(String::from);
        let project_id = caller.project_id;
        self.with_folders(caller, vec![Placement::Choice(choice)], move |server, folders| {
            let Some(Folder::Chosen(Some(path))) = folders.into_iter().next() else {
                return Err(failure("operation_failed", "No workspace was made."));
            };
            if server.projects.thread(thread_id).is_none() {
                return Err(failure("thread_not_found", "This thread was deleted."));
            }
            server.projects.set_thread_workspace(thread_id, Some(path.clone()));
            let scheduled = continuation.is_some();
            server.moving_threads.insert(thread_id, continuation);
            let workspace = server.projects.workspace(project_id, &path);
            Ok(Step::Done(json!({
                "kind": kind_name(kind),
                "workspacePath": path,
                "branch": workspace.and_then(|workspace| workspace.branch.clone()),
                "baseRef": workspace.and_then(|workspace| workspace.base.clone()),
                "continuation": if scheduled { "scheduled" } else { "none" },
                "note": if scheduled {
                    "The thread moves once this turn ends, and continuationPrompt starts its next turn there. End this turn now."
                } else {
                    "The thread moves once this turn ends, and stays idle there until its next message. End this turn now."
                },
            })))
        })
    }

    pub(super) fn workspace_sync(&self, caller: Caller, arguments: &Arguments) -> Outcome {
        let path = self.calling_pasture(caller)?;
        let branch = arguments
            .string("branch", MAX_BRANCH_CHARS)?
            .map(String::from);
        let merge = match arguments.string("strategy", 16)? {
            None | Some("rebase") => false,
            Some("merge") => true,
            Some(strategy) => {
                return Err(invalid(format!(
                    "Unknown strategy {strategy}. Strategies: rebase, merge."
                )));
            }
        };
        let work = self
            .sync_workspace(caller.project_id, &path, branch, merge)
            .map_err(|error| invalid(format!("{error:#}")))?;
        Ok(message_in_background(work))
    }

    pub(super) fn workspace_bring_back(&self, caller: Caller, arguments: &Arguments) -> Outcome {
        let path = self.calling_pasture(caller)?;
        let branch = arguments
            .string("branch", MAX_BRANCH_CHARS)?
            .map(String::from);
        let work = self
            .bring_back_workspace(caller.project_id, &path, branch)
            .map_err(|error| invalid(format!("{error:#}")))?;
        Ok(message_in_background(work))
    }

    fn calling_pasture(&self, caller: Caller) -> Result<PathBuf, Failure> {
        let thread_id = calling_thread(caller)?;
        match self.projects.thread_workspace(thread_id) {
            Some(workspace) if workspace.kind == WorkspaceKind::Pasture => {
                Ok(workspace.path.clone())
            }
            Some(_) => Err(failure(
                "not_in_pasture",
                "This thread works in a worktree, whose branches are already in the project.",
            )),
            None => Err(failure(
                "not_in_pasture",
                "This thread works in the project's own checkout.",
            )),
        }
    }

    /// Restarts the agents of threads that moved to a new workspace once their turn has ended,
    /// so their sessions reopen there, and sends any continuation prompt.
    pub(in crate::server) fn move_threads(&mut self) {
        let ready: Vec<ThreadId> = self
            .moving_threads
            .keys()
            .filter(|thread_id| {
                !self
                    .threads
                    .get(thread_id)
                    .is_some_and(|thread| thread.is_working())
            })
            .copied()
            .collect();
        for thread_id in ready {
            let continuation = self.moving_threads.remove(&thread_id).flatten();
            if self.projects.thread(thread_id).is_none() {
                continue;
            }
            self.threads.remove(&thread_id);
            self.tool_sessions
                .retain(|_, session_thread| *session_thread != thread_id);
            self.update_thread(ConnectionId::Thread(thread_id), |thread| {
                if let Some(prompt) = continuation {
                    thread.send_from(prompt, ThreadCreator::Thread(thread_id));
                }
            })
            .log_err();
        }
    }
}

fn calling_thread(caller: Caller) -> Result<ThreadId, Failure> {
    caller.thread_id.ok_or_else(|| {
        failure(
            "capability_denied",
            "Only a thread's agent has a workspace. From a shell, pass --thread.",
        )
    })
}

fn message_in_background(work: BoxFuture<'static, anyhow::Result<String>>) -> Step {
    Step::Background(
        async move {
            work.await
                .map(|message| json!({"message": message}))
                .map_err(workspace_failure)
        }
        .boxed(),
    )
}
