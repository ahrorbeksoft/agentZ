//! Worktrees and pastures: where a new thread works, and what the server reports about a
//! project's repository to offer them.

use std::path::PathBuf;

use projects::WorkspaceKind;
use serde::{Deserialize, Serialize};

/// Where [`crate::Request::CreateThread`]'s thread works.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub enum WorkspaceChoice {
    /// The project's own folder.
    #[default]
    Checkout,
    /// A new worktree or pasture on a new branch.
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

/// A project's repository, for New Thread's workspace step: [`crate::Response::ProjectGit`].
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ProjectGit {
    pub is_repository: bool,
    /// The checkout's branch, `None` when detached.
    pub branch: Option<String>,
    /// Local branches, most recently committed first.
    pub branches: Vec<String>,
    pub pastures: PastureSupport,
}

/// [`crate::Request::RepositoryCheckouts`]'s answer.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct RepositoryCheckouts {
    /// As for a project, but `branch` is what the asked-about folder has checked out.
    pub git: ProjectGit,
    /// The main checkout first, then its worktrees (`git worktree list`), then the pastures
    /// of the project it is, if any.
    pub checkouts: Vec<Checkout>,
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
