//! What agentZ keeps on a machine and how much room each part takes, for Settings › Storage
//! (design/storage). The machine's server measures it in the background as threads change, so
//! the page opens with current sizes.

use std::path::PathBuf;

use projects::{ProjectId, ThreadId, WorkspaceKind};
use serde::{Deserialize, Serialize};

use crate::agents::AgentId;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Storage {
    /// The data folder, with the home folder written `~`.
    pub data_folder: String,
    /// Set once everything was measured since the server started.
    pub measured: bool,
    /// Each thread and chat the thread lists show, with what its subthreads keep: their
    /// conversations, images, uploaded files and handoffs, and a chat's folder.
    pub threads: Vec<ThreadStorage>,
    /// The worktrees and pastures agentZ made, kept by a project or left in the data folder.
    pub checkouts: Vec<CheckoutStorage>,
    /// Agents installed from the ACP Registry.
    pub agents: Vec<AgentStorage>,
    /// The ACP Registry's list and icons, fetched again when needed.
    pub registry_cache: u64,
    /// Node.js downloaded for npm agents, when the machine had none new enough.
    pub node: Option<NodeStorage>,
    pub server_log: u64,
}

impl Storage {
    pub fn thread(&self, thread_id: ThreadId) -> Option<&ThreadStorage> {
        self.threads
            .iter()
            .find(|thread| thread.thread_id == thread_id)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ThreadStorage {
    pub thread_id: ThreadId,
    pub bytes: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CheckoutStorage {
    pub path: PathBuf,
    pub kind: WorkspaceKind,
    /// The project that keeps it, if one still does.
    pub project_id: Option<ProjectId>,
    /// The folder name of the repository it was made from.
    pub repository: String,
    /// What's checked out in it.
    pub branch: Option<String>,
    pub bytes: u64,
    /// Files with uncommitted changes, new ones included, which deleting it would lose.
    pub changed_files: u32,
    /// A pasture's commits that the project doesn't have, which deleting it would lose.
    pub has_own_commits: bool,
}

impl CheckoutStorage {
    /// Deleting it would lose work, so the page keeps it.
    pub fn has_changes(&self) -> bool {
        self.changed_files > 0 || self.has_own_commits
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AgentStorage {
    pub agent_id: AgentId,
    pub bytes: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NodeStorage {
    /// As nodejs.org names it: `v24.11.0`.
    pub version: String,
    pub bytes: u64,
    /// An agent running now uses it, so it can't be deleted.
    pub in_use: bool,
}

/// What Settings › Storage's Delete and Clear free besides threads and checkouts.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StorageCache {
    /// Downloaded Node.js, downloaded again when an agent needs it.
    Node,
    ServerLog,
    /// The ACP Registry's list and icons, fetched again at once.
    RegistryCache,
}
