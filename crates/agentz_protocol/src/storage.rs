//! What agentZ keeps on a machine and how much room each part takes, for Settings › Storage
//! (design/storage). The machine's server measures it in the background as threads change, so
//! the page opens with current sizes.

use std::path::PathBuf;
use std::time::{Duration, SystemTime};

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
    /// Settings › Storage's Cleaning up. Kept by the server, not measured.
    #[serde(default)]
    pub cleanup: WorkspaceCleanup,
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
    /// The project keeps it for a thread that was deleted, rather than for no thread at all.
    #[serde(default)]
    pub thread_deleted: bool,
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

/// Settings › Storage's Cleaning up (design/archive-cleanup): rules that remove worktrees and
/// pastures on their own, t3code's storage cleanup. Every rule starts off.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CleanupRules {
    /// How long after its thread is archived a workspace is removed.
    #[serde(default)]
    pub archived: ArchivedCleanup,
    /// Deleting a thread removes its workspace, once no other thread uses it.
    #[serde(default)]
    pub deleted: bool,
    /// Days without activity after which a thread's workspace is removed, archived or not.
    #[serde(default)]
    pub inactive_days: Option<u32>,
    /// Worktrees whose branch is merged into the project's default branch.
    #[serde(default)]
    pub merged: bool,
    /// Worktrees and pastures with no commits beyond the default branch and no changes.
    #[serde(default)]
    pub unchanged: bool,
}

impl CleanupRules {
    /// t3code's "Delete inactive worktrees" days, once turned on.
    pub const DEFAULT_INACTIVE_DAYS: u32 = 8;
    pub const MAX_INACTIVE_DAYS: u32 = 3650;

    pub fn any(&self) -> bool {
        self.archived != ArchivedCleanup::Never
            || self.deleted
            || self.inactive_days.is_some()
            || self.merged
            || self.unchanged
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArchivedCleanup {
    #[default]
    Never,
    RightAway,
    AfterDay,
    AfterWeek,
    AfterMonth,
}

impl ArchivedCleanup {
    pub const ALL: [ArchivedCleanup; 5] = [
        ArchivedCleanup::Never,
        ArchivedCleanup::RightAway,
        ArchivedCleanup::AfterDay,
        ArchivedCleanup::AfterWeek,
        ArchivedCleanup::AfterMonth,
    ];

    pub fn label(self) -> &'static str {
        match self {
            ArchivedCleanup::Never => "Never",
            ArchivedCleanup::RightAway => "Right away",
            ArchivedCleanup::AfterDay => "After a day",
            ArchivedCleanup::AfterWeek => "After a week",
            ArchivedCleanup::AfterMonth => "After a month",
        }
    }

    /// How long a thread stays archived before its workspace goes; `None` for never.
    pub fn after(self) -> Option<Duration> {
        const DAY: u64 = 24 * 60 * 60;
        match self {
            ArchivedCleanup::Never => None,
            ArchivedCleanup::RightAway => Some(Duration::ZERO),
            ArchivedCleanup::AfterDay => Some(Duration::from_secs(DAY)),
            ArchivedCleanup::AfterWeek => Some(Duration::from_secs(7 * DAY)),
            ArchivedCleanup::AfterMonth => Some(Duration::from_secs(30 * DAY)),
        }
    }
}

/// The cleanup rules, and how the last run went.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct WorkspaceCleanup {
    pub rules: CleanupRules,
    pub last_run: Option<CleanupReport>,
    /// A run is going now.
    pub running: bool,
}

/// What a cleanup run did with each workspace its rules looked at (t3code's cleanup report).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CleanupReport {
    pub finished_at: SystemTime,
    pub entries: Vec<CleanupEntry>,
}

impl CleanupReport {
    pub fn count(&self, outcome: CleanupOutcome) -> usize {
        self.entries
            .iter()
            .filter(|entry| entry.outcome == outcome)
            .count()
    }

    pub fn bytes_freed(&self) -> u64 {
        self.entries
            .iter()
            .filter(|entry| entry.outcome == CleanupOutcome::Removed)
            .map(|entry| entry.bytes)
            .sum()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CleanupEntry {
    pub path: PathBuf,
    pub kind: WorkspaceKind,
    /// The thread that works there, or worked there before it was deleted.
    pub thread_title: Option<String>,
    pub outcome: CleanupOutcome,
    /// Why, as a sentence: "Archived for 9 days", "Has uncommitted changes (3 files)", or the
    /// error for a failure.
    pub reason: String,
    pub bytes: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CleanupOutcome {
    Removed,
    Kept,
    Failed,
}
