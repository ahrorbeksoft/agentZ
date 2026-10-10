//! Worktrees and pastures: where a new thread works, and what the server reports about a
//! project's repository to offer them.

use std::path::PathBuf;
use std::time::Duration;

use projects::{PlannedWorkspace, WorkspaceKind};
use serde::{Deserialize, Serialize};

/// Where [`crate::Request::CreateThread`]'s thread works.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub enum WorkspaceChoice {
    /// The project's own folder.
    #[default]
    Checkout,
    /// A new worktree or pasture on a new branch. A thread the user starts is a draft in the
    /// project's folder until its first message, which makes it
    /// ([`projects::Thread::planned_workspace`]); one an agent starts gets it at once.
    New {
        kind: WorkspaceKind,
        /// What the branch starts from: the checkout's current branch by default.
        #[serde(default)]
        base: Option<String>,
        /// `agentz/<short id>` by default.
        #[serde(default)]
        branch: Option<String>,
    },
    /// One of the project's worktrees or pastures, by folder.
    Existing(PathBuf),
}

impl WorkspaceChoice {
    /// What a draft records for a new worktree or pasture, if that's the choice.
    pub fn plan(&self) -> Option<PlannedWorkspace> {
        match self {
            WorkspaceChoice::New { kind, base, branch } => Some(PlannedWorkspace {
                kind: *kind,
                base: base.clone(),
                branch: branch.clone(),
            }),
            _ => None,
        }
    }
}

/// A project's repository, for New Thread's workspace step: [`crate::Response::ProjectGit`].
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ProjectGit {
    pub is_repository: bool,
    /// The checkout's branch, `None` when detached.
    pub branch: Option<String>,
    /// origin's default branch, as `origin/HEAD` names it, or else `main` or `master` if the
    /// repository has one (t3code's).
    pub default_branch: Option<String>,
    /// Whether there's an `origin` remote to fetch.
    pub has_origin: bool,
    /// Local branches, most recently committed first, then origin's.
    pub branches: Vec<GitBranch>,
    pub pastures: PastureSupport,
}

/// One of a repository's branches, to start a new one from.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitBranch {
    /// As git writes it: `main`, or `origin/main` for origin's.
    pub name: String,
    pub is_remote: bool,
    /// Checked out in one of the repository's checkouts.
    pub is_checked_out: bool,
}

impl ProjectGit {
    /// What a new branch starts from unless another is picked: what's checked out.
    pub fn default_base(&self) -> String {
        self.branch.clone().unwrap_or_else(|| "HEAD".into())
    }

    /// The faint word at the end of a branch's row (t3code's `BranchPickerRefItem`).
    pub fn mark(&self, branch: &GitBranch) -> Option<&'static str> {
        if branch.is_remote {
            Some("remote")
        } else if self.branch.as_ref() == Some(&branch.name) {
            Some("current")
        } else if self.default_branch.as_ref() == Some(&branch.name) {
            Some("default")
        } else if branch.is_checked_out {
            Some("worktree")
        } else {
            None
        }
    }
}

/// A thread's worktree or pasture being made as its first message goes, before its agent
/// starts there (t3code's worktree setup): [`crate::thread::ThreadState::workspace_setup`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WorkspaceSetup {
    pub kind: WorkspaceKind,
    /// What its branch starts from: `main`, or `origin/main`.
    pub base: String,
    /// The first message's text, shown as the user's until the agent has it.
    pub message: String,
    pub steps: Vec<SetupStep>,
    /// Why it stopped. The message waits for [`crate::Request::RetryWorkspaceSetup`] or
    /// [`crate::Request::UseLocal`].
    pub error: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SetupStep {
    pub kind: SetupStepKind,
    pub state: SetupStepState,
    /// How long it took, once done.
    pub took: Option<Duration>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SetupStepKind {
    /// `git fetch` of a base on origin.
    Fetch,
    /// `git worktree add`, or a pasture's copy switched to the base.
    CheckOut,
    Submodules,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SetupStepState {
    Waiting,
    Running,
    Done,
    Failed,
}

impl WorkspaceSetup {
    /// The steps for a new workspace from `base`: a fetch first when it's origin's.
    pub fn new(kind: WorkspaceKind, base: String, message: String, has_submodules: bool) -> Self {
        let mut steps = Vec::new();
        if remote_branch(&base).is_some() {
            steps.push(SetupStepKind::Fetch);
        }
        steps.push(SetupStepKind::CheckOut);
        if has_submodules {
            steps.push(SetupStepKind::Submodules);
        }
        WorkspaceSetup {
            kind,
            base,
            message,
            steps: steps
                .into_iter()
                .map(|kind| SetupStep {
                    kind,
                    state: SetupStepState::Waiting,
                    took: None,
                })
                .collect(),
            error: None,
        }
    }

    pub fn is_failed(&self) -> bool {
        self.error.is_some()
    }
}

/// `origin/main`'s branch on origin, `main`; `None` for a local branch.
pub fn remote_branch(base: &str) -> Option<&str> {
    base.strip_prefix("origin/")
        .filter(|branch| !branch.is_empty())
}

/// [`crate::Request::RepositoryCheckouts`]'s answer.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct RepositoryCheckouts {
    /// As for a project, but `branch` is what the asked-about folder has checked out.
    pub git: ProjectGit,
    /// The main checkout first, then its worktrees (`git worktree list`), then the pastures
    /// of the project it is, if any.
    pub checkouts: Vec<Checkout>,
    /// Where the server makes worktrees and pastures (its data folder), as its user writes it:
    /// `~/.agentz`.
    #[serde(default)]
    pub data_dir: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Checkout {
    pub path: PathBuf,
    /// `None` when detached.
    pub branch: Option<String>,
    /// `None` for the main checkout.
    pub kind: Option<WorkspaceKind>,
}

/// How a pasture of the project would be made on its machine.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum PastureSupport {
    /// A copy-on-write clone: instant, and only changed blocks use disk.
    CopyOnWrite,
    /// A full copy (cow's fallback on Linux without reflinks): slower and as big as the project.
    FullCopy,
    /// No pastures, with the reason.
    Unsupported(String),
    /// From a newer version.
    #[serde(untagged)]
    Unknown(serde_json::Value),
}

impl Default for PastureSupport {
    fn default() -> Self {
        PastureSupport::Unsupported("Not a git repository.".into())
    }
}

/// [`crate::Request::RemoveWorkspace`]'s outcome.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum WorkspaceRemoval {
    Removed,
    /// It has work that removing would lose; ask, then remove with `force`.
    NeedsConfirmation(String),
    /// From a newer version.
    #[serde(untagged)]
    Unknown(serde_json::Value),
}
