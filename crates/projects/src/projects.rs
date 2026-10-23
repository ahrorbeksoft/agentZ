//! The set of project folders the user has opened, their threads, and which of them the
//! window is currently showing ("All projects" or a single project).
//!
//! Plain Rust with no UI framework, so the server can own it. Whoever owns the store learns of
//! changes through [`ProjectStore::revision`].

pub mod order_key;

use std::collections::BTreeMap;
use std::io::Write as _;
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, SystemTime};

use anyhow::{Context as _, Result};
use collections::HashSet;
use gpui_shared_string::SharedString;
use serde::{Deserialize, Serialize};
use util::ResultExt as _;

const SAVE_DEBOUNCE: Duration = Duration::from_millis(200);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ProjectId(pub u64);

impl ProjectId {
    /// Where threads started in a workspace pane belong: no project of the user's, so they're
    /// listed in the sidebar's Workspaces section rather than among the project's threads.
    /// Each works in its [`Thread::workspace`], any folder on its machine.
    pub const WORKSPACES: ProjectId = ProjectId(u64::MAX);
    /// Where chats belong (t3code's threads with no project): conversations outside every
    /// project, listed among the sidebar's threads. Each works in a folder of its own in the
    /// data directory, made with its first message ([`ProjectStore::set_chats_folder`]).
    pub const CHATS: ProjectId = ProjectId(u64::MAX - 1);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ThreadId(pub u64);

/// One of an agent's agentZ accounts, a login in its own home folder. Never reused within an
/// agent, so the threads of a removed account can't land on a newer one.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AccountId(pub u64);

impl std::fmt::Display for AccountId {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}", self.0)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Project {
    pub id: ProjectId,
    pub path: PathBuf,
    /// A name chosen in the project's settings, shown instead of the folder name.
    #[serde(default)]
    pub custom_name: Option<String>,
    /// An icon chosen in the project's settings, shown instead of the detected one.
    #[serde(default)]
    pub icon: Option<ProjectIcon>,
    /// Worktrees and pastures made for its threads, oldest first.
    #[serde(default)]
    pub workspaces: Vec<Workspace>,
    /// The repository the folder is in, by its primary remote, which projects on other
    /// machines and other checkouts are matched by. Kept up to date by the server.
    #[serde(default)]
    pub repository: Option<RepositoryIdentity>,
}

/// t3code's `RepositoryIdentity`, from a git remote.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RepositoryIdentity {
    /// The remote's URL normalized, as `host/owner/repo`. Equal for every clone.
    pub canonical_key: String,
    /// The repository's top folder on its machine.
    pub root_path: PathBuf,
    pub remote_name: String,
    pub remote_url: String,
    /// The key without its host, as `owner/repo`.
    #[serde(default)]
    pub display_name: Option<String>,
    #[serde(default)]
    pub owner: Option<String>,
    /// The last part of the key.
    #[serde(default)]
    pub name: Option<String>,
}

impl Project {
    pub fn name(&self) -> SharedString {
        match &self.custom_name {
            Some(name) => name.clone().into(),
            None => self.folder_name(),
        }
    }

    pub fn folder_name(&self) -> SharedString {
        project_name(&self.path)
    }

    /// The image file chosen for its icon, on its machine.
    pub fn icon_file(&self) -> Option<PathBuf> {
        match &self.icon {
            // An absolute path replaces the folder it's joined to.
            Some(ProjectIcon::Image { path }) => Some(self.path.join(path)),
            _ => None,
        }
    }
}

/// How a [`Workspace`] shares the project's repository.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkspaceKind {
    /// A `git worktree`: its own branch and folder, sharing the project's `.git`.
    Worktree,
    /// cow's copy-on-write clone of the whole folder, `.git`, dependencies and `.env`
    /// included, on its own branch.
    Pasture,
}

impl WorkspaceKind {
    pub fn label(self) -> &'static str {
        match self {
            WorkspaceKind::Worktree => "Worktree",
            WorkspaceKind::Pasture => "Pasture",
        }
    }
}

/// A checkout of the project besides its own folder, where threads can work apart from it.
/// It stays part of the project, never a project of its own.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Workspace {
    pub kind: WorkspaceKind,
    pub path: PathBuf,
    /// The branch it was made with. Whoever works there may have switched since.
    #[serde(default)]
    pub branch: Option<String>,
    /// What the branch started from.
    #[serde(default)]
    pub base: Option<String>,
    pub created_at: SystemTime,
}

/// Where a fork came from ([`Thread::forked_from`]): its source, and how many of the source's
/// entries it began with, those up to the end of the turn it was forked from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ForkPoint {
    pub thread: ThreadId,
    pub entries: usize,
}

/// A new worktree or pasture for a draft, made as its first message is sent (t3code's thread
/// setup).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlannedWorkspace {
    pub kind: WorkspaceKind,
    /// What its branch starts from: what the folder has checked out when `None`.
    #[serde(default)]
    pub base: Option<String>,
    /// Its branch: a temporary name, renamed after the thread's title, when `None`.
    #[serde(default)]
    pub branch: Option<String>,
}

/// A project icon picked by the user, as in t3code's project settings.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ProjectIcon {
    /// One of the app's icons (Zed's), by its snake_case name, in the named color.
    Icon {
        name: String,
        color: String,
    },
    Emoji {
        emoji: String,
    },
    /// Up to two letters on a tile of the named color.
    Monogram {
        text: String,
        color: String,
    },
    /// An image file on the project's machine: relative to the project's folder when it's
    /// inside it, so each copy of a combined project finds its own.
    Image {
        path: PathBuf,
    },
}

/// A monogram's letters as its tile shows them.
pub fn monogram_text(text: &str) -> String {
    text.chars().take(2).collect::<String>().to_uppercase()
}

/// Two letters for a project: the first letter, then a digit from the first word, the first
/// letter of the last word, or the first word's last letter (t3code's rule).
pub fn monogram(name: &str) -> String {
    let words: Vec<&str> = name
        .split(|c: char| !c.is_alphanumeric())
        .filter(|word| !word.is_empty())
        .collect();
    let Some(first_glyph) = words.first().and_then(|word| word.chars().next()) else {
        return "PR".to_string();
    };
    let first_word: Vec<char> = words
        .first()
        .map(|word| word.chars().collect())
        .unwrap_or_default();
    let second_glyph = first_word
        .iter()
        .skip(1)
        .find(|glyph| glyph.is_numeric())
        .copied()
        .or_else(|| {
            if words.len() > 1 {
                words.last().and_then(|word| word.chars().next())
            } else {
                first_word.last().copied()
            }
        })
        .unwrap_or(first_glyph);
    format!("{first_glyph}{second_glyph}")
        .to_uppercase()
        .chars()
        .take(2)
        .collect()
}

/// t3code's project colors: Tailwind's 600 shades on light themes and 400 shades on dark ones.
pub const MONOGRAM_COLORS: [(&str, u32, u32); 18] = [
    ("gray", 0x4b5563, 0x9ca3af),
    ("red", 0xdc2626, 0xf87171),
    ("orange", 0xea580c, 0xfb923c),
    ("amber", 0xd97706, 0xfbbf24),
    ("yellow", 0xca8a04, 0xfacc15),
    ("lime", 0x65a30d, 0xa3e635),
    ("green", 0x16a34a, 0x4ade80),
    ("emerald", 0x059669, 0x34d399),
    ("teal", 0x0d9488, 0x2dd4bf),
    ("cyan", 0x0891b2, 0x22d3ee),
    ("sky", 0x0284c7, 0x38bdf8),
    ("blue", 0x2563eb, 0x60a5fa),
    ("indigo", 0x4f46e5, 0x818cf8),
    ("violet", 0x7c3aed, 0xa78bfa),
    ("purple", 0x9333ea, 0xc084fc),
    ("fuchsia", 0xc026d3, 0xe879f9),
    ("pink", 0xdb2777, 0xf472b6),
    ("rose", 0xe11d48, 0xfb7185),
];

/// The project's automatic color, one of [`MONOGRAM_COLORS`].
pub fn monogram_color(name: &str) -> (&'static str, u32, u32) {
    MONOGRAM_COLORS
        .get(monogram_color_index(name))
        .copied()
        .unwrap_or(("blue", 0x2563eb, 0x60a5fa))
}

/// A stable color per project name, hashed the way t3code does.
fn monogram_color_index(name: &str) -> usize {
    let seed = name.trim().to_lowercase();
    let seed = if seed.is_empty() { "project" } else { &seed };
    seed.chars().fold(0, |index, glyph| {
        (index * 31 + glyph as usize) % MONOGRAM_COLORS.len()
    })
}

/// A thread's title until its first prompt names it.
pub const NEW_THREAD_TITLE: &str = "New thread";

/// What a terminal thread runs (herdr's panes).
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TerminalCommand {
    /// A command line for the user's login shell to run, such as `claude`. `None` is the login
    /// shell itself.
    #[serde(default)]
    pub command: Option<String>,
}

impl TerminalCommand {
    /// What the thread is called until the user renames it.
    pub fn title(&self) -> String {
        match &self.command {
            Some(command) => command.clone(),
            None => "Terminal".to_string(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Thread {
    pub id: ThreadId,
    pub project_id: ProjectId,
    pub title: String,
    /// The registry id of the thread's agent. It can change between turns
    /// ([`ProjectStore::switch_thread_agent`]).
    #[serde(default)]
    pub agent_id: Option<String>,
    /// The agent's account the thread runs on. `None` is the External account, the agent's
    /// own login in its normal home. Accounts share no sessions, so another account goes on
    /// in a session of its own.
    #[serde(default)]
    pub account: Option<AccountId>,
    /// When the thread was created or its agent last did something.
    #[serde(default)]
    pub last_activity_at: Option<SystemTime>,
    /// Orders threads newest first across machines, where ids don't compare. Missing on
    /// threads made before it was recorded.
    #[serde(default)]
    pub created_at: Option<SystemTime>,
    /// The agent's ACP session, so the conversation can be restored after a restart.
    #[serde(default)]
    pub session_id: Option<String>,
    /// The login the agent reported while that session was in use. A session that doesn't
    /// load while the agent reports another one was lost to the account change.
    #[serde(default)]
    pub session_login: Option<LoginIdentity>,
    /// The sessions of the agents and accounts the thread went on from, to go back to.
    #[serde(default)]
    pub other_sessions: Vec<AgentSession>,
    /// Set when the thread is archived; archived threads only show in Thread History.
    #[serde(default)]
    pub archived_at: Option<SystemTime>,
    /// Set while the thread is pinned (t3code's `pinnedAt`): the sidebar lists it above the
    /// rest.
    #[serde(default)]
    pub pinned_at: Option<SystemTime>,
    /// Where the user put the pinned thread among the pinned ones, as an [`order_key`]. Pinned
    /// threads without one follow those with one.
    #[serde(default)]
    pub pin_order_key: Option<String>,
    /// Where the user put the unpinned thread among the rest, as an [`order_key`]. Those
    /// without one, like new threads, lead.
    #[serde(default)]
    pub active_order_key: Option<String>,
    /// The user renamed the thread, so automatic titles no longer replace it.
    #[serde(default)]
    pub has_custom_title: bool,
    /// The latest automatic title (from the first prompt, the agent, a shell's folder), kept
    /// while the user's own shows, so clearing that brings it back. Missing on threads from
    /// before it was kept.
    #[serde(default)]
    pub automatic_title: Option<String>,
    /// The model last selected in the thread, as the agent names it.
    #[serde(default)]
    pub model: Option<String>,
    /// When the agent last finished a turn. A client shows the thread as done until it has
    /// displayed this completion.
    #[serde(default)]
    pub completed_at: Option<SystemTime>,
    /// Set while a limit of the thread's account stopped it and agentZ sends "Continue." when
    /// the limit resets, at this time. A turn that starts by then cancels it.
    #[serde(default)]
    pub continues_at: Option<SystemTime>,
    /// Set while the thread is snoozed (t3code's `snoozedUntil`): it leaves the thread list
    /// until then, unless it needs the user or finishes a turn first
    /// ([`ProjectStore::is_thread_snoozed`]). It stays once the thread wakes, which clients
    /// mark until the user opens it and wakes it for good ([`ProjectStore::wake_thread`]).
    #[serde(default)]
    pub snoozed_until: Option<SystemTime>,
    /// When the thread was snoozed: a turn it finishes after this wakes it.
    #[serde(default)]
    pub snoozed_at: Option<SystemTime>,
    /// When the user last woke the thread (t3code's `lastSnoozeWakeAt`). Auto-archive counts
    /// its inactivity from then, without moving it in the list as activity would.
    #[serde(default)]
    pub woke_at: Option<SystemTime>,
    /// Who started the thread, when it wasn't the user.
    #[serde(default)]
    pub created_by: Option<ThreadCreator>,
    /// Set on a subthread: a task another thread's agent delegated (t3code's `subagent`
    /// lineage). Subthreads show in their parent rather than in the thread list.
    #[serde(default)]
    pub task: Option<Task>,
    /// The worktree or pasture the thread works in, one of its project's workspaces, or
    /// another folder inside the project. `None` is the project's own folder. A Workspaces
    /// thread ([`ProjectId::WORKSPACES`]) always has one, any folder.
    #[serde(default)]
    pub workspace: Option<PathBuf>,
    /// The folder a Workspaces thread was started in, when it works in a worktree or pasture
    /// made from that folder's repository, so its draft can go back there.
    #[serde(default)]
    pub started_in: Option<PathBuf>,
    /// The new worktree or pasture a draft will work in, made from the folder it's in once its
    /// first message is sent.
    #[serde(default)]
    pub planned_workspace: Option<PlannedWorkspace>,
    /// Set on a terminal thread, which runs this instead of an ACP agent.
    #[serde(default)]
    pub terminal: Option<TerminalCommand>,
    /// The thread this one is a fork of, and where: it began with that thread's conversation up
    /// to there.
    #[serde(default)]
    pub forked_from: Option<ForkPoint>,
    /// A thread the user started and hasn't sent anything in yet: t3code's draft thread. It's
    /// left out of the thread list, and removed once the user leaves it with nothing typed.
    #[serde(default)]
    pub is_draft: bool,
    /// What's typed in the thread's composer and not sent yet (t3code's composer draft), kept
    /// while the user is elsewhere. Its mentions are written as their `@name`.
    #[serde(default)]
    pub unsent_text: Option<String>,
    /// The mentions in [`Thread::unsent_text`], in order, so they come back as chips.
    #[serde(default)]
    pub unsent_mentions: Vec<UnsentMention>,
}

/// The session an agent on one of its accounts had in a thread before the thread went on with
/// another ([`Thread::other_sessions`]).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AgentSession {
    pub agent_id: String,
    pub account: Option<AccountId>,
    pub session_id: String,
    #[serde(default)]
    pub session_login: Option<LoginIdentity>,
    /// How many of the thread's entries the session has seen: the ones after are new to it.
    pub seen: usize,
}

/// What tells one of an agent's logins from another, from what the agent reports of it: the
/// account's email or where an API key comes from, or the kind of login alone.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoginIdentity {
    pub kind: String,
    /// The email or key source, when the agent says which.
    #[serde(default)]
    pub key: Option<String>,
}

impl LoginIdentity {
    /// Whether `other` is a different login. One the agent reported without its email (Claude
    /// Agent's CLI check leaves it out at times) can't be told from one with it.
    pub fn differs_from(&self, other: &LoginIdentity) -> bool {
        self.kind != other.kind
            || matches!((&self.key, &other.key), (Some(key), Some(other)) if key != other)
    }

    /// Whether `other` is this login, saying more about it.
    pub fn is_completed_by(&self, other: &LoginIdentity) -> bool {
        !self.differs_from(other) && self.key.is_none() && other.key.is_some()
    }
}

/// A mention in a thread's [`Thread::unsent_text`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct UnsentMention {
    /// Where its `@name` is in the text, in bytes.
    pub range: Range<usize>,
    pub target: Mentioned,
}

/// What a mention names, as the protocol's prompt parts do, which this crate is below.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mentioned {
    /// A file or folder on the thread's machine.
    Path(PathBuf),
    Thread(ThreadId),
    /// An image kept for the thread, by its attachment id.
    Image(String),
}

/// An agent's session to add as a thread: [`ProjectStore::add_imported_thread`].
#[derive(Clone, Debug, PartialEq)]
pub struct ImportedSession {
    pub project_id: ProjectId,
    /// The project's worktree or pasture the session ran in, or `None` for its own folder.
    pub workspace: Option<PathBuf>,
    pub agent_id: String,
    /// The account whose home keeps the session, `None` being the External one.
    pub account: Option<AccountId>,
    pub session_id: String,
    pub title: String,
    /// When the agent last worked on it, which orders it among the other threads.
    pub updated_at: Option<SystemTime>,
    /// Imported straight into Archived.
    pub archived: bool,
}

impl Thread {
    /// The thread that delegated this one, for a subthread of a thread on this machine.
    pub fn parent(&self) -> Option<ThreadId> {
        self.task
            .as_ref()
            .filter(|task| task.parent_machine.is_none())
            .map(|task| task.parent)
    }

    /// The thread on another machine that delegated this one ([`Task::parent_machine`]).
    pub fn remote_parent(&self) -> Option<RemoteThread> {
        let task = self.task.as_ref()?;
        Some(RemoteThread {
            machine: task.parent_machine.clone()?,
            thread: task.parent,
        })
    }

    /// Where the task this thread stands for works, for a task delegated to another machine
    /// ([`Task::runs_on`]).
    pub fn runs_on(&self) -> Option<&RemoteThread> {
        self.task.as_ref()?.runs_on.as_ref()
    }

    /// Started in a workspace pane, so listed in the Workspaces section.
    pub fn in_workspaces(&self) -> bool {
        self.project_id == ProjectId::WORKSPACES
    }

    /// A chat ([`ProjectId::CHATS`]): a thread outside every project, listed among the rest.
    pub fn is_chat(&self) -> bool {
        self.project_id == ProjectId::CHATS
    }

    pub fn is_pinned(&self) -> bool {
        self.pinned_at.is_some()
    }

    /// Listed among the threads, where the user can arrange it: not a subthread, a draft or a
    /// Workspaces thread, which have places of their own.
    pub fn can_arrange(&self) -> bool {
        self.task.is_none() && !self.is_draft && !self.in_workspaces()
    }

    /// An agent thread among the threads, which the user can pin. Terminal threads aren't: a
    /// shell can't, and an agent CLI running in one only arranges among the rest.
    pub fn can_pin(&self) -> bool {
        self.can_arrange() && self.terminal.is_none()
    }

    /// An agent thread among the threads, which the user can snooze as they can archive it.
    pub fn can_snooze(&self) -> bool {
        self.can_arrange() && self.terminal.is_none() && self.archived_at.is_none()
    }

    /// For a Workspaces thread, the folder it was started in, which may differ from the
    /// worktree or pasture it works in.
    pub fn starting_folder(&self) -> Option<&PathBuf> {
        self.started_in.as_ref().or(self.workspace.as_ref())
    }
}

/// A task delegated to a subthread, and its result once it ends.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Task {
    pub parent: ThreadId,
    /// What the task's agent was asked to do, its only prompt from the parent.
    pub prompt: String,
    /// The kind of work, as the parent described it: implementation, research, review…
    #[serde(default)]
    pub role: Option<String>,
    /// The parent's idempotency key, so a retried delegation finds this task.
    #[serde(default)]
    pub client_request_id: Option<String>,
    #[serde(default)]
    pub outcome: Option<TaskOutcome>,
    /// The parent has the outcome: it waited for it, read it, cancelled the task, or its agent
    /// took word that it ended.
    #[serde(default)]
    pub delivered: bool,
    /// Set on the agent's own subagent, by its session under the parent's (Claude Agent's
    /// subagent sessions): the parent's agent runs it and hears its end itself, so it has no
    /// agent of its own.
    #[serde(default)]
    pub agent_session: Option<String>,
    /// Set on a task delegated to another machine: its thread works there, and this one, which
    /// has no agent of its own, stands for it under the parent.
    #[serde(default)]
    pub runs_on: Option<RemoteThread>,
    /// Set on a task delegated from another machine, by the name the app gives that machine:
    /// `parent` is a thread there.
    #[serde(default)]
    pub parent_machine: Option<String>,
    /// Set on the agent's own subagent that its agent lets agentZ stop (ACP's subagent
    /// `cancel` capability).
    #[serde(default)]
    pub stoppable: bool,
    /// Set while the task waits for its turn: its parent already runs as many subthreads as
    /// the server's limit allows. It starts, in order, as others end.
    #[serde(default)]
    pub queued: Option<QueuedStart>,
    /// The parent let it delegate tasks of its own (`canDelegate`).
    #[serde(default)]
    pub can_delegate: bool,
    /// What its thread changed, from its checkpoints, as of its end. `None` while it runs,
    /// and when it couldn't be read: outside git, or before this was kept.
    #[serde(default)]
    pub changes: Option<TaskChanges>,
}

/// How a queued task starts once its turn comes: the settings it was delegated with, which
/// its agent only takes as it starts.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct QueuedStart {
    /// The model's setting and value.
    #[serde(default)]
    pub model: Option<(String, String)>,
    /// The effort's setting and value, from the user's pick for the task's role.
    #[serde(default)]
    pub effort: Option<(String, String)>,
    /// The parent's mode, which it may not go beyond.
    #[serde(default)]
    pub mode: Option<String>,
    /// The parent's mode, for agents that keep it in a setting: the setting and value.
    #[serde(default)]
    pub mode_option: Option<(String, String)>,
}

/// The files a task's thread changed over its turns, as its changes panel counts them.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskChanges {
    pub files: u32,
    pub additions: u32,
    pub deletions: u32,
}

/// A thread on another machine, by the name the app gives that machine.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteThread {
    pub machine: String,
    pub thread: ThreadId,
}

impl Task {
    /// One of the parent's agent's own subagents ([`Task::agent_session`]).
    pub fn is_agents_own(&self) -> bool {
        self.agent_session.is_some()
    }

    /// Listed with the parent's subthreads: the agent's own subagents only while they run.
    pub fn is_listed(&self) -> bool {
        !self.is_agents_own() || self.outcome.is_none()
    }

    /// Whether it runs and can be stopped: the agent's own subagent only when its agent lets
    /// agentZ stop it.
    pub fn can_stop(&self) -> bool {
        self.outcome.is_none() && (!self.is_agents_own() || self.stoppable)
    }

    /// Waiting for its turn to start ([`Task::queued`]).
    pub fn is_queued(&self) -> bool {
        self.outcome.is_none() && self.queued.is_some()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TaskOutcome {
    pub end: TaskEnd,
    /// The task's result: the agent's last message, or the error.
    pub summary: Option<String>,
    pub ended_at: SystemTime,
}

/// How a delegated task ended, with t3code's terminal task statuses.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskEnd {
    Completed,
    Failed,
    /// Stopped by the parent.
    Cancelled,
    /// Stopped some other way: by the user, or because the server stopped.
    Interrupted,
}

impl TaskEnd {
    pub fn as_str(self) -> &'static str {
        match self {
            TaskEnd::Completed => "completed",
            TaskEnd::Failed => "failed",
            TaskEnd::Cancelled => "cancelled",
            TaskEnd::Interrupted => "interrupted",
        }
    }

    /// The end [`Self::as_str`] names.
    pub fn parse(status: &str) -> Option<Self> {
        [
            TaskEnd::Completed,
            TaskEnd::Failed,
            TaskEnd::Cancelled,
            TaskEnd::Interrupted,
        ]
        .into_iter()
        .find(|end| end.as_str() == status)
    }
}

/// An agent that started a thread or sent it a message, as opposed to the user (t3code's
/// `createdBy: agent`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ThreadCreator {
    /// Another thread's agent, through MCP or the CLI.
    Thread(ThreadId),
    /// The CLI run outside any thread, e.g. by an agent in a terminal or a script.
    Command,
}

/// How threads (and, in "All projects", the projects themselves) are ordered.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ThreadOrder {
    /// Most recent activity first.
    LastActivity,
    /// Newest thread first.
    #[default]
    Created,
}

/// The parts of the thread list the user arranges by hand, each with its own order keys.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ThreadSection {
    Pinned,
    /// The unpinned threads.
    Active,
}

/// Which projects the sidebar shows threads for.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "project")]
pub enum ProjectScope {
    #[default]
    All,
    Project(ProjectId),
}

impl ProjectScope {
    /// The selected project, or `None` for all projects.
    pub fn project(self) -> Option<ProjectId> {
        match self {
            ProjectScope::All => None,
            ProjectScope::Project(id) => Some(id),
        }
    }
}

/// The whole store as a server sends it to clients.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ProjectsSnapshot {
    pub projects: Vec<Project>,
    pub threads: Vec<Thread>,
    pub scope: ProjectScope,
    pub thread_order: ThreadOrder,
    pub archived_expanded: bool,
    pub workspaces_expanded: bool,
    pub working_threads: Vec<ThreadId>,
    /// Threads waiting for the user to answer a permission request.
    pub blocked_threads: Vec<ThreadId>,
    /// Threads whose agent asked for input (ACP's elicitation) that the user hasn't given.
    pub awaiting_input_threads: Vec<ThreadId>,
    /// Threads whose turn ended with their work still going: a background task, or tasks
    /// delegated to subthreads. They complete once it's over.
    pub waiting_threads: Vec<ThreadId>,
    /// When each working or waiting thread started to be: its turn's start, held while the
    /// work goes on after it and through the turns that work brings.
    pub busy_since: Vec<(ThreadId, SystemTime)>,
    /// Terminal threads running an agent CLI, with its name. The rest are plain shells.
    pub terminal_agents: Vec<(ThreadId, String)>,
    /// Terminal threads running a program in front of their shell, with its name.
    pub terminal_commands: Vec<(ThreadId, String)>,
    /// Where each terminal thread's foreground process is.
    pub terminal_folders: Vec<(ThreadId, TerminalFolder)>,
    /// The branches checked out where projects and threads work, by folder: projects' own
    /// folders, their worktrees and pastures, and Workspaces threads' folders. Folders outside
    /// git have none.
    pub git_heads: Vec<(PathBuf, GitHead)>,
    /// The icon file of each project on its server's machine: the one chosen
    /// ([`Project::icon_file`]) while it's there, else the one found in its folder (t3code's
    /// favicon scan). Projects without one aren't listed.
    pub favicons: Vec<(ProjectId, PathBuf)>,
    /// How each project with copies on other machines stands in git, read with the branches.
    /// Other projects aren't listed.
    pub copy_statuses: Vec<(ProjectId, CopyStatus)>,
    /// Drawer terminals running a program in front of their shell: thread, terminal number,
    /// program.
    pub drawer_commands: Vec<(ThreadId, u32, String)>,
}

/// What a server sends its clients as the store changes, after the whole of it once: every
/// part of [`ProjectsSnapshot`] but the threads, of which only those that changed come, with
/// every thread's id in order. A parent with hundreds of subthreads made each change send them
/// all, their task prompts and results too.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ProjectsUpdate {
    /// The store, with only the threads that changed or are new.
    pub changed: ProjectsSnapshot,
    /// Every thread's id, in the store's order: those not listed are gone.
    pub thread_ids: Vec<ThreadId>,
}

impl ProjectsSnapshot {
    /// What changed from this snapshot to `newer`.
    pub fn update_to(&self, newer: &ProjectsSnapshot) -> ProjectsUpdate {
        let before: collections::HashMap<ThreadId, &Thread> = self
            .threads
            .iter()
            .map(|thread| (thread.id, thread))
            .collect();
        let threads = newer
            .threads
            .iter()
            .filter(|thread| before.get(&thread.id) != Some(thread))
            .cloned()
            .collect();
        ProjectsUpdate {
            changed: ProjectsSnapshot {
                threads,
                ..newer.without_threads()
            },
            thread_ids: newer.threads.iter().map(|thread| thread.id).collect(),
        }
    }

    /// The snapshot with `update` made to it.
    pub fn apply(&mut self, update: ProjectsUpdate) {
        let ProjectsUpdate {
            mut changed,
            thread_ids,
        } = update;
        let mut threads: collections::HashMap<ThreadId, Thread> = std::mem::take(&mut self.threads)
            .into_iter()
            .chain(std::mem::take(&mut changed.threads))
            .map(|thread| (thread.id, thread))
            .collect();
        changed.threads = thread_ids
            .into_iter()
            .filter_map(|id| threads.remove(&id))
            .collect();
        *self = changed;
    }

    fn without_threads(&self) -> ProjectsSnapshot {
        ProjectsSnapshot {
            projects: self.projects.clone(),
            threads: Vec::new(),
            scope: self.scope,
            thread_order: self.thread_order,
            archived_expanded: self.archived_expanded,
            workspaces_expanded: self.workspaces_expanded,
            working_threads: self.working_threads.clone(),
            blocked_threads: self.blocked_threads.clone(),
            awaiting_input_threads: self.awaiting_input_threads.clone(),
            waiting_threads: self.waiting_threads.clone(),
            busy_since: self.busy_since.clone(),
            terminal_agents: self.terminal_agents.clone(),
            terminal_commands: self.terminal_commands.clone(),
            terminal_folders: self.terminal_folders.clone(),
            git_heads: self.git_heads.clone(),
            favicons: self.favicons.clone(),
            copy_statuses: self.copy_statuses.clone(),
            drawer_commands: self.drawer_commands.clone(),
        }
    }
}

/// How a project's folder stands in git, as its machine's server reads it, so the copies of a
/// project on several machines can be told apart where a new thread picks one.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CopyStatus {
    /// `None` when detached.
    pub branch: Option<String>,
    /// The commit checked out, abbreviated.
    pub commit: Option<String>,
    /// What the branch tracks, such as `origin/main`.
    pub upstream: Option<String>,
    /// Commits ahead of and behind the upstream as of the machine's last fetch: nothing is
    /// fetched to read them. Without an upstream, ahead counts the commits no remote has.
    pub ahead: u32,
    pub behind: u32,
    /// Files changed or new, and the lines added and removed, not committed.
    pub changed_files: u32,
    pub added_lines: u32,
    pub removed_lines: u32,
    pub stashes: u32,
    /// When the repository last fetched, by its `FETCH_HEAD`.
    pub fetched_at: Option<SystemTime>,
}

/// The folder a terminal's foreground process works in, and its git branch.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TerminalFolder {
    pub path: PathBuf,
    /// `None` outside a git repository, or on a detached head.
    #[serde(default)]
    pub branch: Option<String>,
    #[serde(default)]
    pub is_repository: bool,
    /// The repository's name: its main checkout's folder, also for a worktree of it.
    #[serde(default)]
    pub repository: Option<String>,
    /// The path as its machine's user would write it, `~` for home, since a client can't
    /// tell another machine's home.
    #[serde(default)]
    pub display_path: Option<String>,
}

/// The branch checked out in a folder, as its machine's server reads it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitHead {
    /// The branch name, or the abbreviated commit when HEAD is detached.
    pub branch: String,
    /// Set when the folder is in a linked worktree rather than the main checkout.
    #[serde(default)]
    pub worktree: Option<PathBuf>,
}

#[derive(Clone, Default, Serialize, Deserialize)]
struct PersistedState {
    #[serde(default)]
    next_id: u64,
    #[serde(default)]
    projects: Vec<Project>,
    #[serde(default)]
    threads: Vec<Thread>,
    #[serde(default)]
    scope: ProjectScope,
    #[serde(default)]
    thread_order: ThreadOrder,
    #[serde(default)]
    archived_expanded: bool,
    #[serde(default)]
    workspaces_expanded: bool,
}

pub struct ProjectStore {
    next_id: u64,
    projects: Vec<Project>,
    threads: Vec<Thread>,
    scope: ProjectScope,
    thread_order: ThreadOrder,
    /// Whether the sidebar's Archived shelf is open.
    archived_expanded: bool,
    /// Whether the sidebar's Workspaces section is open.
    workspaces_expanded: bool,
    /// Where a chat works until its first message makes its own folder: the folder those are
    /// made in. Set by the server, which knows its data directory. Not persisted.
    chats_folder: Option<PathBuf>,
    /// Threads whose agent is currently running. Not persisted: nothing is running after a
    /// restart.
    working_threads: HashSet<ThreadId>,
    /// Threads with a permission request waiting. Not persisted either.
    blocked_threads: HashSet<ThreadId>,
    /// Threads with a request for input waiting. Not persisted either.
    awaiting_input_threads: HashSet<ThreadId>,
    /// Threads whose work goes on after their turn. Not persisted either.
    waiting_threads: HashSet<ThreadId>,
    /// [`ProjectsSnapshot::busy_since`]. Not persisted either.
    busy_since: BTreeMap<ThreadId, SystemTime>,
    /// Terminal threads running an agent CLI, by its name. Not persisted either.
    terminal_agents: BTreeMap<ThreadId, String>,
    /// Terminal threads running a program in front of their shell. Not persisted either.
    terminal_commands: BTreeMap<ThreadId, String>,
    /// Where terminal threads' foreground processes are. Not persisted either.
    terminal_folders: BTreeMap<ThreadId, TerminalFolder>,
    /// The branches of [`ProjectStore::git_head_folders`]. Not persisted either.
    git_heads: BTreeMap<PathBuf, GitHead>,
    /// [`ProjectsSnapshot::favicons`]. Not persisted either.
    favicons: BTreeMap<ProjectId, PathBuf>,
    /// [`ProjectsSnapshot::copy_statuses`]. Not persisted either.
    copy_statuses: BTreeMap<ProjectId, CopyStatus>,
    /// Drawer terminals running a program in front of their shell. Not persisted either.
    drawer_commands: BTreeMap<(ThreadId, u32), String>,
    /// Counts changes, so the owner can tell whether a call changed anything.
    revision: u64,
    saver: Option<Saver<PersistedState>>,
}

impl ProjectStore {
    /// Loads the store from `state_path`, and saves every change back to it. With `None`,
    /// nothing is read or saved. A file that's there but can't be read is an error, never an
    /// empty store that would be saved over it.
    pub fn load(state_path: Option<PathBuf>) -> Result<Self> {
        let state = match &state_path {
            Some(path) => read_state::<PersistedState>(path)?.unwrap_or_default(),
            None => PersistedState::default(),
        };
        Ok(Self::from_state(state, state_path))
    }

    fn from_state(state: PersistedState, state_path: Option<PathBuf>) -> Self {
        let mut this = Self {
            next_id: state.next_id,
            projects: state.projects,
            threads: state.threads,
            scope: state.scope,
            thread_order: state.thread_order,
            archived_expanded: state.archived_expanded,
            workspaces_expanded: state.workspaces_expanded,
            chats_folder: None,
            working_threads: HashSet::default(),
            blocked_threads: HashSet::default(),
            awaiting_input_threads: HashSet::default(),
            waiting_threads: HashSet::default(),
            busy_since: BTreeMap::new(),
            terminal_agents: BTreeMap::new(),
            terminal_commands: BTreeMap::new(),
            terminal_folders: BTreeMap::new(),
            git_heads: BTreeMap::new(),
            favicons: BTreeMap::new(),
            copy_statuses: BTreeMap::new(),
            drawer_commands: BTreeMap::new(),
            revision: 0,
            saver: state_path.map(|path| Saver::new(path, "projects-saver")),
        };
        let highest_id = this
            .projects
            .iter()
            .map(|project| project.id.0)
            .chain(this.threads.iter().map(|thread| thread.id.0))
            .max();
        if let Some(highest_id) = highest_id {
            this.next_id = this.next_id.max(highest_id + 1);
        }
        this.threads.retain(|thread| {
            thread.in_workspaces()
                || thread.is_chat()
                || this.projects.iter().any(|p| p.id == thread.project_id)
        });
        if let ProjectScope::Project(id) = this.scope
            && this.project(id).is_none()
        {
            this.scope = ProjectScope::All;
        }
        this
    }

    /// Goes up by at least one with every change.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn projects(&self) -> &[Project] {
        &self.projects
    }

    pub fn project(&self, id: ProjectId) -> Option<&Project> {
        self.projects.iter().find(|project| project.id == id)
    }

    /// The project's threads in the current [`ThreadOrder`].
    pub fn threads_for(&self, project_id: ProjectId) -> impl Iterator<Item = &Thread> {
        let mut threads: Vec<&Thread> = self
            .threads
            .iter()
            .filter(|thread| {
                thread.project_id == project_id
                    && thread.archived_at.is_none()
                    && thread.task.is_none()
            })
            .collect();
        match self.thread_order {
            ThreadOrder::LastActivity => threads.sort_by(|a, b| {
                b.last_activity_at
                    .cmp(&a.last_activity_at)
                    .then(b.id.cmp(&a.id))
            }),
            ThreadOrder::Created => threads.sort_by_key(|thread| std::cmp::Reverse(thread.id)),
        }
        threads.into_iter()
    }

    pub fn archived_expanded(&self) -> bool {
        self.archived_expanded
    }

    pub fn toggle_archived_expanded(&mut self) {
        self.archived_expanded = !self.archived_expanded;
        self.changed();
    }

    pub fn workspaces_expanded(&self) -> bool {
        self.workspaces_expanded
    }

    pub fn toggle_workspaces_expanded(&mut self) {
        self.workspaces_expanded = !self.workspaces_expanded;
        self.changed();
    }

    /// Where chats' own folders are made, and where a chat works until it has one.
    pub fn set_chats_folder(&mut self, folder: PathBuf) {
        self.chats_folder = Some(folder);
    }

    pub fn chats_folder(&self) -> Option<&Path> {
        self.chats_folder.as_deref()
    }

    /// Unarchived threads of every visible project, in the current [`ThreadOrder`].
    pub fn active_threads(&self) -> Vec<&Thread> {
        let mut threads: Vec<&Thread> = self
            .visible_projects()
            .flat_map(|project| self.threads_for(project.id))
            .collect();
        match self.thread_order {
            ThreadOrder::LastActivity => threads.sort_by(|a, b| {
                b.last_activity_at
                    .cmp(&a.last_activity_at)
                    .then(b.id.cmp(&a.id))
            }),
            ThreadOrder::Created => threads.sort_by_key(|thread| std::cmp::Reverse(thread.id)),
        }
        threads
    }

    pub fn thread_order(&self) -> ThreadOrder {
        self.thread_order
    }

    pub fn set_thread_order(&mut self, order: ThreadOrder) {
        if self.thread_order != order {
            self.thread_order = order;
            self.changed();
        }
    }

    fn latest_activity(&self, project_id: ProjectId) -> Option<SystemTime> {
        self.threads
            .iter()
            .filter(|thread| thread.project_id == project_id)
            .filter_map(|thread| thread.last_activity_at)
            .max()
    }

    pub fn thread_count(&self, project_id: ProjectId) -> usize {
        self.threads_for(project_id).count()
    }

    pub fn scope(&self) -> ProjectScope {
        self.scope
    }

    /// The projects the current scope shows. With [`ThreadOrder::LastActivity`] the most
    /// recently active project comes first; otherwise projects keep the order they were added.
    pub fn visible_projects(&self) -> impl Iterator<Item = &Project> {
        let scope = self.scope;
        let mut projects: Vec<&Project> = self
            .projects
            .iter()
            .filter(|project| match scope {
                ProjectScope::All => true,
                ProjectScope::Project(id) => project.id == id,
            })
            .collect();
        if self.thread_order == ThreadOrder::LastActivity {
            // Stable sort, so projects without threads keep their added order at the end.
            projects.sort_by_key(|project| std::cmp::Reverse(self.latest_activity(project.id)));
        }
        projects.into_iter()
    }

    pub fn set_scope(&mut self, scope: ProjectScope) {
        if let ProjectScope::Project(id) = scope
            && self.project(id).is_none()
        {
            return;
        }
        if self.scope != scope {
            self.scope = scope;
            self.changed();
        }
    }

    /// Adds the folder at `path` (or finds it if it was already added) and returns its id.
    pub fn add_project(&mut self, path: PathBuf) -> ProjectId {
        let path = std::fs::canonicalize(&path).unwrap_or(path);
        if let Some(existing) = self.projects.iter().find(|project| project.path == path) {
            return existing.id;
        }
        let id = ProjectId(self.allocate_id());
        self.projects.push(Project {
            id,
            path,
            custom_name: None,
            icon: None,
            workspaces: Vec::new(),
            repository: None,
        });
        self.changed();
        id
    }

    /// Removes the project from the list. Nothing on disk is touched.
    /// Sets the project's display name; an empty name goes back to the folder name.
    pub fn set_project_name(&mut self, id: ProjectId, name: &str) {
        let name = name.trim();
        let Some(project) = self.projects.iter_mut().find(|project| project.id == id) else {
            return;
        };
        let custom_name =
            (!name.is_empty() && *name != *project.folder_name()).then(|| name.to_string());
        if project.custom_name != custom_name {
            project.custom_name = custom_name;
            self.changed();
        }
    }

    pub fn set_project_icon(&mut self, id: ProjectId, icon: Option<ProjectIcon>) {
        if let Some(project) = self.projects.iter_mut().find(|project| project.id == id)
            && project.icon != icon
        {
            project.icon = icon;
            self.changed();
        }
    }

    pub fn set_project_repository(
        &mut self,
        id: ProjectId,
        repository: Option<RepositoryIdentity>,
    ) {
        if let Some(project) = self.projects.iter_mut().find(|project| project.id == id)
            && project.repository != repository
        {
            project.repository = repository;
            self.changed();
        }
    }

    pub fn remove_project(&mut self, id: ProjectId) {
        let count_before = self.projects.len();
        self.projects.retain(|project| project.id != id);
        if self.projects.len() == count_before {
            return;
        }
        self.threads.retain(|thread| thread.project_id != id);
        let threads = &self.threads;
        self.working_threads
            .retain(|thread_id| threads.iter().any(|thread| thread.id == *thread_id));
        self.blocked_threads
            .retain(|thread_id| threads.iter().any(|thread| thread.id == *thread_id));
        self.awaiting_input_threads
            .retain(|thread_id| threads.iter().any(|thread| thread.id == *thread_id));
        self.waiting_threads
            .retain(|thread_id| threads.iter().any(|thread| thread.id == *thread_id));
        self.busy_since
            .retain(|thread_id, _| threads.iter().any(|thread| thread.id == *thread_id));
        self.terminal_agents
            .retain(|thread_id, _| threads.iter().any(|thread| thread.id == *thread_id));
        self.terminal_commands
            .retain(|thread_id, _| threads.iter().any(|thread| thread.id == *thread_id));
        self.terminal_folders
            .retain(|thread_id, _| threads.iter().any(|thread| thread.id == *thread_id));
        self.drawer_commands
            .retain(|(thread_id, _), _| threads.iter().any(|thread| thread.id == *thread_id));
        if self.scope == ProjectScope::Project(id) {
            self.scope = ProjectScope::All;
        }
        self.changed();
    }

    pub fn add_thread(
        &mut self,
        project_id: ProjectId,
        title: impl Into<String>,
        agent_id: Option<String>,
    ) -> Option<ThreadId> {
        if project_id != ProjectId::WORKSPACES && project_id != ProjectId::CHATS {
            self.project(project_id)?;
        }
        let id = ThreadId(self.allocate_id());
        let now = SystemTime::now();
        let title = title.into();
        self.threads.push(Thread {
            id,
            project_id,
            automatic_title: Some(title.clone()),
            title,
            agent_id,
            account: None,
            last_activity_at: Some(now),
            created_at: Some(now),
            session_id: None,
            session_login: None,
            other_sessions: Vec::new(),
            archived_at: None,
            pinned_at: None,
            pin_order_key: None,
            active_order_key: None,
            has_custom_title: false,
            model: None,
            completed_at: None,
            continues_at: None,
            snoozed_until: None,
            snoozed_at: None,
            woke_at: None,
            created_by: None,
            task: None,
            workspace: None,
            started_in: None,
            planned_workspace: None,
            terminal: None,
            forked_from: None,
            is_draft: false,
            unsent_text: None,
            unsent_mentions: Vec::new(),
        });
        self.changed();
        Some(id)
    }

    /// Adds a terminal thread, titled after its command.
    pub fn add_terminal_thread(
        &mut self,
        project_id: ProjectId,
        command: TerminalCommand,
    ) -> Option<ThreadId> {
        let id = self.add_thread(project_id, command.title(), None)?;
        if let Some(thread) = self.threads.iter_mut().find(|thread| thread.id == id) {
            thread.terminal = Some(command);
        }
        self.changed();
        Some(id)
    }

    /// Adds a subthread of `task.parent`, in the parent's project and workspace.
    pub fn add_subthread(&mut self, task: Task, agent_id: Option<String>) -> Option<ThreadId> {
        let parent_thread = self.thread(task.parent)?;
        let project_id = parent_thread.project_id;
        let workspace = parent_thread.workspace.clone();
        let parent = task.parent;
        let id = self.add_thread(project_id, NEW_THREAD_TITLE, agent_id)?;
        if let Some(thread) = self.threads.iter_mut().find(|thread| thread.id == id) {
            thread.created_by = Some(ThreadCreator::Thread(parent));
            thread.task = Some(task);
            thread.workspace = workspace;
        }
        self.changed();
        Some(id)
    }

    /// Adds the thread of a task a thread on another machine delegated
    /// ([`Task::parent_machine`]), in the project's folder.
    pub fn add_task_from_elsewhere(
        &mut self,
        project_id: ProjectId,
        task: Task,
        agent_id: Option<String>,
    ) -> Option<ThreadId> {
        let id = self.add_thread(project_id, NEW_THREAD_TITLE, agent_id)?;
        if let Some(thread) = self.threads.iter_mut().find(|thread| thread.id == id) {
            thread.task = Some(task);
        }
        self.changed();
        Some(id)
    }

    /// Adds a thread for a conversation the agent already keeps (from ACP's `session/list`).
    /// The agent loads it when the thread opens.
    pub fn add_imported_thread(&mut self, session: ImportedSession) -> Option<ThreadId> {
        let id = self.add_thread(session.project_id, session.title, Some(session.agent_id))?;
        let archived_at = session.archived.then(SystemTime::now);
        let thread = self.threads.iter_mut().find(|thread| thread.id == id)?;
        thread.account = session.account;
        thread.session_id = Some(session.session_id);
        thread.workspace = session.workspace;
        if let Some(updated_at) = session.updated_at {
            thread.last_activity_at = Some(updated_at);
            thread.created_at = Some(updated_at);
        }
        thread.archived_at = archived_at;
        self.changed();
        Some(id)
    }

    /// The thread that has the session the agent keeps for the account.
    pub fn thread_for_session(
        &self,
        agent_id: &str,
        account: Option<AccountId>,
        session_id: &str,
    ) -> Option<ThreadId> {
        self.threads
            .iter()
            .find(|thread| {
                thread.agent_id.as_deref() == Some(agent_id)
                    && thread.account == account
                    && thread.session_id.as_deref() == Some(session_id)
            })
            .map(|thread| thread.id)
    }

    /// The project whose folder `folder` is, or one of whose worktrees or pastures, with that
    /// workspace. Both are compared as given, so pass canonical paths.
    pub fn folder_owner(&self, folder: &Path) -> Option<(ProjectId, Option<PathBuf>)> {
        self.projects.iter().find_map(|project| {
            if project.path == folder {
                return Some((project.id, None));
            }
            project
                .workspaces
                .iter()
                .find(|workspace| workspace.path == folder)
                .map(|workspace| (project.id, Some(workspace.path.clone())))
        })
    }

    /// The thread's subthreads, newest first.
    pub fn subthreads(&self, parent: ThreadId) -> Vec<&Thread> {
        let mut threads: Vec<&Thread> = self
            .threads
            .iter()
            .filter(|thread| thread.parent() == Some(parent))
            .collect();
        threads.sort_by_key(|thread| std::cmp::Reverse(thread.id));
        threads
    }

    /// The thread itself, its subthreads, theirs, and so on.
    pub fn thread_and_subthreads(&self, id: ThreadId) -> Vec<ThreadId> {
        let mut ids = vec![id];
        let mut index = 0;
        while let Some(&parent) = ids.get(index) {
            ids.extend(
                self.threads
                    .iter()
                    .filter(|thread| thread.parent() == Some(parent))
                    .map(|thread| thread.id),
            );
            index += 1;
        }
        ids
    }

    /// The top-level thread a subthread belongs to, or the thread itself.
    pub fn root_thread(&self, id: ThreadId) -> ThreadId {
        let mut id = id;
        // Bounded, in case a broken state file has a cycle.
        for _ in 0..self.threads.len() {
            match self.thread(id).and_then(Thread::parent) {
                Some(parent) => id = parent,
                None => break,
            }
        }
        id
    }

    pub fn update_task(&mut self, id: ThreadId, update: impl FnOnce(&mut Task)) {
        let Some(task) = self
            .threads
            .iter_mut()
            .find(|thread| thread.id == id)
            .and_then(|thread| thread.task.as_mut())
        else {
            return;
        };
        let before = task.clone();
        update(task);
        if *task != before {
            self.changed();
        }
    }

    /// Every thread, archived or not, in no particular order.
    pub fn threads(&self) -> &[Thread] {
        &self.threads
    }

    pub fn thread(&self, id: ThreadId) -> Option<&Thread> {
        self.threads.iter().find(|thread| thread.id == id)
    }

    /// Every thread of the visible projects, archived or not, most recent activity first.
    /// Archived threads in the current scope, most recently archived first.
    pub fn archived_threads(&self) -> Vec<&Thread> {
        let mut threads: Vec<&Thread> = self
            .threads
            .iter()
            .filter(|thread| thread.archived_at.is_some() && thread.task.is_none())
            .filter(|thread| match self.scope {
                ProjectScope::All => true,
                ProjectScope::Project(id) => thread.project_id == id,
            })
            .collect();
        threads.sort_by_key(|thread| std::cmp::Reverse((thread.archived_at, thread.id)));
        threads
    }

    pub fn archive_thread(&mut self, id: ThreadId) {
        if let Some(thread) = self.threads.iter_mut().find(|thread| thread.id == id)
            && thread.archived_at.is_none()
        {
            thread.archived_at = Some(SystemTime::now());
            // As t3code's settling does, so a thread brought back starts over among the rest,
            // neither pinned nor snoozed.
            thread.pinned_at = None;
            thread.pin_order_key = None;
            thread.active_order_key = None;
            thread.snoozed_until = None;
            thread.snoozed_at = None;
            self.working_threads.remove(&id);
            self.blocked_threads.remove(&id);
            self.awaiting_input_threads.remove(&id);
            self.waiting_threads.remove(&id);
            self.busy_since.remove(&id);
            self.changed();
        }
    }

    /// The agent threads in the thread list, neither pinned nor busy, whose last activity was
    /// before `cutoff`: the ones auto-archive archives. A snooze counts as activity until it
    /// ends, as waking a thread does. A thread with no recorded activity or creation time is
    /// never stale.
    pub fn stale_threads(&self, cutoff: SystemTime) -> Vec<ThreadId> {
        self.threads
            .iter()
            .filter(|thread| {
                thread.can_arrange()
                    && thread.terminal.is_none()
                    && thread.archived_at.is_none()
                    && !thread.is_pinned()
                    && !self.working_threads.contains(&thread.id)
                    && !self.blocked_threads.contains(&thread.id)
                    && !self.awaiting_input_threads.contains(&thread.id)
                    && !self.waiting_threads.contains(&thread.id)
                    && thread
                        .last_activity_at
                        .or(thread.created_at)
                        .is_some_and(|last| {
                            [thread.snoozed_until, thread.woke_at]
                                .into_iter()
                                .flatten()
                                .fold(last, SystemTime::max)
                                < cutoff
                        })
            })
            .map(|thread| thread.id)
            .collect()
    }

    pub fn unarchive_thread(&mut self, id: ThreadId) {
        if let Some(thread) = self.threads.iter_mut().find(|thread| thread.id == id)
            && thread.archived_at.is_some()
        {
            thread.archived_at = None;
            self.changed();
        }
    }

    /// Pins the thread, at `order_key` among the pinned ones, or without one after those the
    /// user arranged. An archived thread comes back to be pinned, as a settled one does in
    /// t3code, and a snoozed one wakes. Pinning a pinned thread moves it to `order_key`.
    pub fn pin_thread(&mut self, id: ThreadId, order_key: Option<String>) -> Result<()> {
        if order_key
            .as_deref()
            .is_some_and(|key| !order_key::is_valid(key))
        {
            anyhow::bail!("not an order key: {order_key:?}");
        }
        let thread = self
            .threads
            .iter_mut()
            .find(|thread| thread.id == id)
            .context("no such thread")?;
        if !thread.can_pin() {
            anyhow::bail!("only an agent thread in the thread list can be pinned");
        }
        let pinned_at = thread.pinned_at.or_else(|| Some(SystemTime::now()));
        let pin_order_key = order_key.or_else(|| thread.pin_order_key.clone());
        if thread.pinned_at == pinned_at
            && thread.pin_order_key == pin_order_key
            && thread.archived_at.is_none()
            && thread.snoozed_until.is_none()
        {
            return Ok(());
        }
        thread.pinned_at = pinned_at;
        thread.pin_order_key = pin_order_key;
        thread.archived_at = None;
        self.changed();
        self.wake_thread(id);
        Ok(())
    }

    /// Snoozes the thread until `until` (t3code's `thread.snooze`). A thread waiting on the
    /// user can't be: that would hide what it asks. Snoozing again until the same time keeps
    /// when it was snoozed, so a turn it finished since still wakes it.
    pub fn snooze_thread(&mut self, id: ThreadId, until: SystemTime) -> Result<()> {
        let now = SystemTime::now();
        if until <= now {
            anyhow::bail!("the wake time has passed");
        }
        if self.is_thread_or_subthread_blocked(id) || self.is_thread_awaiting_input(id) {
            anyhow::bail!("the thread is waiting for your answer, so it can't be snoozed");
        }
        let thread = self
            .threads
            .iter_mut()
            .find(|thread| thread.id == id)
            .context("no such thread")?;
        if !thread.can_snooze() {
            anyhow::bail!("only an agent thread in the thread list can be snoozed");
        }
        if thread.snoozed_until == Some(until) {
            return Ok(());
        }
        thread.snoozed_until = Some(until);
        thread.snoozed_at = Some(now);
        self.changed();
        Ok(())
    }

    /// Wakes the thread (t3code's `thread.unsnooze`): a snoozed one comes back now, and one
    /// that woke is no longer marked.
    pub fn wake_thread(&mut self, id: ThreadId) {
        if let Some(thread) = self.threads.iter_mut().find(|thread| thread.id == id)
            && thread.snoozed_until.is_some()
        {
            thread.snoozed_until = None;
            thread.snoozed_at = None;
            thread.woke_at = Some(SystemTime::now());
            self.changed();
        }
    }

    /// The thread is snoozed (t3code's `effectiveSnoozed`): its wake time is after `now`,
    /// and since it was snoozed it hasn't asked the user anything or finished a turn.
    pub fn is_thread_snoozed(&self, id: ThreadId, now: SystemTime) -> bool {
        self.thread(id)
            .and_then(|thread| thread.snoozed_until)
            .is_some_and(|until| until > now)
            && !self.raised_hand_while_snoozed(id)
    }

    /// The snoozed thread woke, on time or early, and the user hasn't woken it since: clients
    /// mark it Woke (t3code's `threadWokeAt`).
    pub fn has_thread_woken(&self, id: ThreadId, now: SystemTime) -> bool {
        self.thread(id)
            .is_some_and(|thread| thread.snoozed_until.is_some())
            && !self.is_thread_snoozed(id, now)
    }

    /// Something outranks the snooze (t3code's `threadRaisedHandWhileSnoozed`): the thread
    /// waits on the user, or finished a turn after it was snoozed, failed ones included.
    fn raised_hand_while_snoozed(&self, id: ThreadId) -> bool {
        let finished_since = self.thread(id).is_some_and(|thread| {
            thread
                .completed_at
                .zip(thread.snoozed_at)
                .is_some_and(|(completed_at, snoozed_at)| completed_at > snoozed_at)
        });
        finished_since
            || self.is_thread_or_subthread_blocked(id)
            || self.is_thread_awaiting_input(id)
    }

    /// Unpins the thread, which goes back to where it was among the rest.
    pub fn unpin_thread(&mut self, id: ThreadId) {
        if let Some(thread) = self.threads.iter_mut().find(|thread| thread.id == id)
            && thread.is_pinned()
        {
            thread.pinned_at = None;
            thread.pin_order_key = None;
            self.changed();
        }
    }

    /// Writes the order keys of a reorder in `section` ([`order_key::plan_reorder`]): all of
    /// them, or none if one of the threads isn't in that section.
    pub fn set_order_keys(
        &mut self,
        section: ThreadSection,
        keys: Vec<(ThreadId, String)>,
    ) -> Result<()> {
        for (id, key) in &keys {
            if !order_key::is_valid(key) {
                anyhow::bail!("not an order key: {key:?}");
            }
            let thread = self.thread(*id).context("no such thread")?;
            let (in_section, name) = match section {
                ThreadSection::Pinned => (thread.is_pinned(), "pinned"),
                ThreadSection::Active => (!thread.is_pinned(), "unpinned"),
            };
            if !thread.can_arrange() || thread.archived_at.is_some() || !in_section {
                anyhow::bail!("thread {} isn't among the {name} threads", id.0);
            }
        }
        let mut changed = false;
        for (id, key) in keys {
            if let Some(thread) = self.threads.iter_mut().find(|thread| thread.id == id) {
                let slot = match section {
                    ThreadSection::Pinned => &mut thread.pin_order_key,
                    ThreadSection::Active => &mut thread.active_order_key,
                };
                if slot.as_ref() != Some(&key) {
                    *slot = Some(key);
                    changed = true;
                }
            }
        }
        if changed {
            self.changed();
        }
        Ok(())
    }

    /// Removes the thread for good, with its subthreads.
    pub fn delete_thread(&mut self, id: ThreadId) {
        let ids = self.thread_and_subthreads(id);
        let count_before = self.threads.len();
        self.threads.retain(|thread| !ids.contains(&thread.id));
        if self.threads.len() != count_before {
            for id in ids {
                self.working_threads.remove(&id);
                self.blocked_threads.remove(&id);
                self.awaiting_input_threads.remove(&id);
                self.waiting_threads.remove(&id);
                self.busy_since.remove(&id);
                self.terminal_agents.remove(&id);
                self.terminal_commands.remove(&id);
                self.terminal_folders.remove(&id);
                self.drawer_commands
                    .retain(|(thread_id, _), _| *thread_id != id);
            }
            self.changed();
        }
    }

    pub fn set_thread_creator(&mut self, id: ThreadId, creator: ThreadCreator) {
        if let Some(thread) = self.threads.iter_mut().find(|thread| thread.id == id)
            && thread.created_by != Some(creator)
        {
            thread.created_by = Some(creator);
            self.changed();
        }
    }

    /// Records a worktree or pasture made for the project's threads.
    pub fn add_workspace(&mut self, project_id: ProjectId, workspace: Workspace) {
        let Some(project) = self
            .projects
            .iter_mut()
            .find(|project| project.id == project_id)
        else {
            return;
        };
        project
            .workspaces
            .retain(|existing| existing.path != workspace.path);
        project.workspaces.push(workspace);
        self.changed();
    }

    /// Forgets a workspace that was removed from disk. Threads that worked there keep its path,
    /// and fail to start until they move.
    pub fn remove_workspace(&mut self, project_id: ProjectId, path: &Path) {
        if let Some(project) = self
            .projects
            .iter_mut()
            .find(|project| project.id == project_id)
        {
            let count_before = project.workspaces.len();
            project
                .workspaces
                .retain(|workspace| workspace.path != path);
            if project.workspaces.len() != count_before {
                self.changed();
            }
        }
    }

    pub fn workspace(&self, project_id: ProjectId, path: &Path) -> Option<&Workspace> {
        self.project(project_id)?
            .workspaces
            .iter()
            .find(|workspace| workspace.path == path)
    }

    /// The worktree or pasture the thread works in, if it isn't the project's own folder. A
    /// Workspaces thread's is any project's.
    pub fn thread_workspace(&self, id: ThreadId) -> Option<&Workspace> {
        let thread = self.thread(id)?;
        let path = thread.workspace.as_deref()?;
        if thread.in_workspaces() {
            return self
                .projects
                .iter()
                .flat_map(|project| &project.workspaces)
                .find(|workspace| workspace.path == path);
        }
        self.workspace(thread.project_id, path)
    }

    /// The project a thread is listed under: its own, or for a Workspaces thread the one its
    /// folder is in, if any.
    pub fn thread_project(&self, id: ThreadId) -> Option<ProjectId> {
        let thread = self.thread(id)?;
        if !thread.in_workspaces() {
            return Some(thread.project_id);
        }
        project_at(&self.projects, thread.workspace.as_deref()?).map(|project| project.id)
    }

    /// Makes a Workspaces thread, with its subthreads, a thread of the project, still working
    /// in its folder.
    pub fn move_thread_to_project(&mut self, id: ThreadId, project_id: ProjectId) {
        let Some(project_path) = self.project(project_id).map(|project| project.path.clone())
        else {
            return;
        };
        let ids = self.thread_and_subthreads(id);
        for thread in &mut self.threads {
            if ids.contains(&thread.id) {
                thread.project_id = project_id;
                thread.started_in = None;
                if thread.workspace.as_ref() == Some(&project_path) {
                    thread.workspace = None;
                }
            }
        }
        self.changed();
    }

    /// Where the thread's agent works: its workspace, or its project's folder. A chat without a
    /// folder of its own yet works in [`ProjectStore::chats_folder`].
    pub fn thread_folder(&self, id: ThreadId) -> Option<PathBuf> {
        let thread = self.thread(id)?;
        match &thread.workspace {
            Some(path) => Some(path.clone()),
            None if thread.is_chat() => self.chats_folder.clone(),
            None => Some(self.project(thread.project_id)?.path.clone()),
        }
    }

    /// Threads working in the folder: a workspace, or a project's own folder.
    pub fn threads_in_folder(&self, folder: &Path) -> Vec<ThreadId> {
        self.threads
            .iter()
            .filter(|thread| self.thread_folder(thread.id).as_deref() == Some(folder))
            .map(|thread| thread.id)
            .collect()
    }

    /// Moves the thread to one of its project's workspaces, or with `None` to the project's
    /// own folder.
    pub fn set_thread_workspace(&mut self, id: ThreadId, workspace: Option<PathBuf>) {
        if let Some(thread) = self.threads.iter_mut().find(|thread| thread.id == id)
            && thread.workspace != workspace
        {
            thread.workspace = workspace;
            self.changed();
        }
    }

    /// Before its agent starts, since the account decides where it runs.
    pub fn set_thread_account(&mut self, id: ThreadId, account: Option<AccountId>) {
        if let Some(thread) = self.threads.iter_mut().find(|thread| thread.id == id)
            && thread.account != account
        {
            thread.account = account;
            self.changed();
        }
    }

    pub fn set_started_in(&mut self, id: ThreadId, folder: Option<PathBuf>) {
        if let Some(thread) = self.threads.iter_mut().find(|thread| thread.id == id)
            && thread.started_in != folder
        {
            thread.started_in = folder;
            self.changed();
        }
    }

    pub fn set_planned_workspace(&mut self, id: ThreadId, plan: Option<PlannedWorkspace>) {
        if let Some(thread) = self.threads.iter_mut().find(|thread| thread.id == id)
            && thread.planned_workspace != plan
        {
            thread.planned_workspace = plan;
            self.changed();
        }
    }

    /// Records a workspace's branch after it was renamed, in whichever project has it.
    pub fn set_workspace_branch(&mut self, path: &Path, branch: String) {
        let workspace = self
            .projects
            .iter_mut()
            .flat_map(|project| project.workspaces.iter_mut())
            .find(|workspace| workspace.path == path);
        if let Some(workspace) = workspace
            && workspace.branch.as_ref() != Some(&branch)
        {
            workspace.branch = Some(branch);
            self.changed();
        }
    }

    pub fn set_draft(&mut self, id: ThreadId, is_draft: bool) {
        if let Some(thread) = self.threads.iter_mut().find(|thread| thread.id == id)
            && thread.is_draft != is_draft
        {
            thread.is_draft = is_draft;
            self.changed();
        }
    }

    /// Keeps what's typed in the thread's composer, and the mentions in it; blank text is none.
    /// Mentions out of order or outside the text are dropped.
    pub fn set_unsent_text(
        &mut self,
        id: ThreadId,
        text: Option<String>,
        mentions: Vec<UnsentMention>,
    ) {
        let text = text.filter(|text| !text.trim().is_empty());
        let mut end = 0;
        let mentions: Vec<UnsentMention> = mentions
            .into_iter()
            .filter(|mention| {
                let fits = mention.range.start >= end
                    && !mention.range.is_empty()
                    && text
                        .as_deref()
                        .is_some_and(|text| text.get(mention.range.clone()).is_some());
                if fits {
                    end = mention.range.end;
                }
                fits
            })
            .collect();
        if let Some(thread) = self.threads.iter_mut().find(|thread| thread.id == id)
            && (thread.unsent_text != text || thread.unsent_mentions != mentions)
        {
            thread.unsent_text = text;
            thread.unsent_mentions = mentions;
            self.changed();
        }
    }

    /// [`Thread::continues_at`].
    pub fn set_continues_at(&mut self, id: ThreadId, at: Option<SystemTime>) {
        if let Some(thread) = self.threads.iter_mut().find(|thread| thread.id == id)
            && thread.continues_at != at
        {
            thread.continues_at = at;
            self.changed();
        }
    }

    pub fn set_forked_from(&mut self, id: ThreadId, from: ForkPoint) {
        if let Some(thread) = self.threads.iter_mut().find(|thread| thread.id == id) {
            thread.forked_from = Some(from);
            self.changed();
        }
    }

    /// The forks of this thread, oldest first.
    pub fn forks(&self, id: ThreadId) -> impl Iterator<Item = &Thread> {
        self.threads.iter().filter(move |thread| {
            thread
                .forked_from
                .is_some_and(|fork_point| fork_point.thread == id)
        })
    }

    pub fn set_thread_session(&mut self, id: ThreadId, session_id: String) {
        if let Some(thread) = self.threads.iter_mut().find(|thread| thread.id == id)
            && thread.session_id.as_deref() != Some(session_id.as_str())
        {
            thread.session_id = Some(session_id);
            // The new session's login comes once the agent reports it.
            thread.session_login = None;
            self.changed();
        }
    }

    /// The thread's agent starts a new session next time, as one moved to another folder must.
    /// The sessions of the agents it went on from are forgotten too.
    pub fn forget_thread_session(&mut self, id: ThreadId) {
        if let Some(thread) = self.threads.iter_mut().find(|thread| thread.id == id)
            && (thread.session_id.is_some() || !thread.other_sessions.is_empty())
        {
            thread.session_id = None;
            thread.session_login = None;
            thread.other_sessions.clear();
            self.changed();
        }
    }

    /// The thread goes on with `agent_id` on `account`. The session it had keeps, with the
    /// `seen` entries it has seen, to go back to; the one the new agent and account had, if
    /// they were in the thread before, is the thread's again. Returns that one.
    pub fn switch_thread_agent(
        &mut self,
        id: ThreadId,
        agent_id: String,
        account: Option<AccountId>,
        seen: usize,
    ) -> Option<AgentSession> {
        let thread = self.threads.iter_mut().find(|thread| thread.id == id)?;
        if let (Some(current), Some(session_id)) =
            (thread.agent_id.clone(), thread.session_id.take())
        {
            let account = thread.account;
            thread
                .other_sessions
                .retain(|session| !(session.agent_id == current && session.account == account));
            thread.other_sessions.push(AgentSession {
                agent_id: current,
                account,
                session_id,
                session_login: thread.session_login.take(),
                seen,
            });
        }
        let returning = thread
            .other_sessions
            .iter()
            .position(|session| session.agent_id == agent_id && session.account == account)
            .map(|index| thread.other_sessions.remove(index));
        thread.session_id = returning.as_ref().map(|session| session.session_id.clone());
        thread.session_login = returning
            .as_ref()
            .and_then(|session| session.session_login.clone());
        thread.agent_id = Some(agent_id);
        thread.account = account;
        // Filled in once the agent's session opens.
        thread.model = None;
        self.changed();
        returning
    }

    pub fn set_session_login(&mut self, id: ThreadId, login: LoginIdentity) {
        if let Some(thread) = self.threads.iter_mut().find(|thread| thread.id == id)
            && thread.session_login.as_ref() != Some(&login)
        {
            thread.session_login = Some(login);
            self.changed();
        }
    }

    pub fn set_thread_model(&mut self, id: ThreadId, model: String) {
        if let Some(thread) = self.threads.iter_mut().find(|thread| thread.id == id)
            && thread.model.as_deref() != Some(model.as_str())
        {
            thread.model = Some(model);
            self.changed();
        }
    }

    /// Sets an automatic title (from the first prompt, the agent, or a shell's folder). It
    /// shows unless the user renamed the thread, and is kept for when they clear their title.
    pub fn rename_thread(&mut self, id: ThreadId, title: String) {
        let Some(thread) = self.threads.iter_mut().find(|thread| thread.id == id) else {
            return;
        };
        let mut changed = false;
        if thread.automatic_title.as_ref() != Some(&title) {
            thread.automatic_title = Some(title.clone());
            changed = true;
        }
        if !thread.has_custom_title && thread.title != title {
            thread.title = title;
            changed = true;
        }
        if changed {
            self.changed();
        }
    }

    /// Sets a title chosen by the user. An empty one goes back to the automatic title.
    pub fn set_custom_title(&mut self, id: ThreadId, title: String) {
        let Some(thread) = self.threads.iter_mut().find(|thread| thread.id == id) else {
            return;
        };
        let title = title.trim();
        if title.is_empty() {
            if !thread.has_custom_title {
                return;
            }
            thread.has_custom_title = false;
            thread.title = thread
                .automatic_title
                .clone()
                .unwrap_or_else(|| NEW_THREAD_TITLE.to_string());
        } else {
            if thread.has_custom_title && thread.title == title {
                return;
            }
            // A thread from before automatic titles were kept: its title was the automatic one.
            if thread.automatic_title.is_none() && !thread.has_custom_title {
                thread.automatic_title = Some(thread.title.clone());
            }
            thread.title = title.to_string();
            thread.has_custom_title = true;
        }
        self.changed();
    }

    pub fn is_thread_working(&self, id: ThreadId) -> bool {
        self.working_threads.contains(&id)
    }

    /// Marks whether the thread's agent is running; either change counts as activity, and
    /// stopping completes the turn, unless its work goes on ([`Self::set_thread_waiting`]).
    pub fn set_thread_working(&mut self, id: ThreadId, working: bool) {
        let changed = if working {
            self.working_threads.insert(id)
        } else {
            self.working_threads.remove(&id)
        };
        if changed {
            self.update_busy_since(id);
            if !working && !self.waiting_threads.contains(&id) {
                self.complete_thread(id);
            }
            self.record_thread_activity(id);
        }
    }

    pub fn is_thread_waiting(&self, id: ThreadId) -> bool {
        self.waiting_threads.contains(&id)
    }

    /// Marks whether the thread's work goes on after its turn. Set before the turn ends, it
    /// holds the completion back until it's cleared.
    pub fn set_thread_waiting(&mut self, id: ThreadId, waiting: bool) {
        let changed = if waiting {
            self.thread(id).is_some() && self.waiting_threads.insert(id)
        } else {
            self.waiting_threads.remove(&id)
        };
        if !changed {
            return;
        }
        self.update_busy_since(id);
        if !waiting && !self.working_threads.contains(&id) {
            self.complete_thread(id);
            self.record_thread_activity(id);
        } else {
            self.changed();
        }
    }

    /// The threads whose work goes on after their turn.
    pub fn waiting_threads(&self) -> impl Iterator<Item = ThreadId> + '_ {
        self.waiting_threads.iter().copied()
    }

    /// When the thread started working, if it's working or waiting: a thread waiting on its
    /// subthreads goes on into the turn their results start without a pause, so this holds
    /// from the turn that started them until the one that takes the last result ends.
    pub fn busy_since(&self, id: ThreadId) -> Option<SystemTime> {
        self.busy_since.get(&id).copied()
    }

    fn update_busy_since(&mut self, id: ThreadId) {
        if self.working_threads.contains(&id) || self.waiting_threads.contains(&id) {
            self.busy_since.entry(id).or_insert_with(SystemTime::now);
        } else {
            self.busy_since.remove(&id);
        }
    }

    fn complete_thread(&mut self, id: ThreadId) {
        if let Some(thread) = self.threads.iter_mut().find(|thread| thread.id == id) {
            thread.completed_at = Some(SystemTime::now());
        }
    }

    pub fn is_thread_blocked(&self, id: ThreadId) -> bool {
        self.blocked_threads.contains(&id)
    }

    /// The thread, or a subthread of it at any depth, is waiting for a permission answer. A
    /// subthread's requests are answered from its parent.
    pub fn is_thread_or_subthread_blocked(&self, id: ThreadId) -> bool {
        self.thread_and_subthreads(id)
            .into_iter()
            .any(|id| self.blocked_threads.contains(&id))
    }

    /// Marks whether the thread is waiting for a permission answer.
    pub fn set_thread_blocked(&mut self, id: ThreadId, blocked: bool) {
        let changed = if blocked {
            self.thread(id).is_some() && self.blocked_threads.insert(id)
        } else {
            self.blocked_threads.remove(&id)
        };
        if changed {
            self.changed();
        }
    }

    pub fn is_thread_awaiting_input(&self, id: ThreadId) -> bool {
        self.awaiting_input_threads.contains(&id)
    }

    /// Marks whether the thread's agent waits for the user to answer a request for input.
    pub fn set_thread_awaiting_input(&mut self, id: ThreadId, awaiting_input: bool) {
        let changed = if awaiting_input {
            self.thread(id).is_some() && self.awaiting_input_threads.insert(id)
        } else {
            self.awaiting_input_threads.remove(&id)
        };
        if changed {
            self.changed();
        }
    }

    /// The agent CLI a terminal thread runs, if any.
    pub fn terminal_agent(&self, id: ThreadId) -> Option<&str> {
        self.terminal_agents.get(&id).map(String::as_str)
    }

    /// Records the agent CLI a terminal thread runs, or that it's back to a plain shell.
    pub fn set_terminal_agent(&mut self, id: ThreadId, agent: Option<String>) {
        let changed = match agent {
            Some(agent) if self.thread(id).is_some() => {
                self.terminal_agents.insert(id, agent.clone()) != Some(agent)
            }
            Some(_) => false,
            None => self.terminal_agents.remove(&id).is_some(),
        };
        if changed {
            self.changed();
        }
    }

    /// The program a terminal thread runs in front of its shell, if any.
    pub fn terminal_command(&self, id: ThreadId) -> Option<&str> {
        self.terminal_commands.get(&id).map(String::as_str)
    }

    /// Records the program a terminal thread runs in front of its shell, or that the shell is.
    pub fn set_terminal_command(&mut self, id: ThreadId, command: Option<String>) {
        let changed = match command {
            Some(command) if self.thread(id).is_some() => {
                self.terminal_commands.insert(id, command.clone()) != Some(command)
            }
            Some(_) => false,
            None => self.terminal_commands.remove(&id).is_some(),
        };
        if changed {
            self.changed();
        }
    }

    /// The thread's drawer terminals running a program in front of their shell, by number.
    pub fn drawer_commands(&self, id: ThreadId) -> impl Iterator<Item = (u32, &str)> {
        self.drawer_commands
            .range((id, 0)..=(id, u32::MAX))
            .map(|((_, number), command)| (*number, command.as_str()))
    }

    /// Records the program a drawer terminal runs in front of its shell, or that the shell is.
    pub fn set_drawer_command(&mut self, id: ThreadId, number: u32, command: Option<String>) {
        let changed = match command {
            Some(command) if self.thread(id).is_some() => {
                self.drawer_commands.insert((id, number), command.clone()) != Some(command)
            }
            Some(_) => false,
            None => self.drawer_commands.remove(&(id, number)).is_some(),
        };
        if changed {
            self.changed();
        }
    }

    /// Where a terminal thread's foreground process is, if known.
    pub fn terminal_folder(&self, id: ThreadId) -> Option<&TerminalFolder> {
        self.terminal_folders.get(&id)
    }

    pub fn terminal_folders(&self) -> impl Iterator<Item = (ThreadId, &TerminalFolder)> {
        self.terminal_folders
            .iter()
            .map(|(id, folder)| (*id, folder))
    }

    pub fn set_terminal_folder(&mut self, id: ThreadId, folder: Option<TerminalFolder>) {
        let changed = match folder {
            Some(folder) if self.thread(id).is_some() => {
                self.terminal_folders.insert(id, folder.clone()) != Some(folder)
            }
            Some(_) => false,
            None => self.terminal_folders.remove(&id).is_some(),
        };
        if changed {
            self.changed();
        }
    }

    /// The branch checked out in one of [`Self::git_head_folders`], once its server has read it.
    pub fn git_head(&self, folder: &Path) -> Option<&GitHead> {
        self.git_heads.get(folder)
    }

    /// The folders whose branches are shown: projects' own folders, their worktrees and
    /// pastures, and where threads work outside their project's folder.
    pub fn git_head_folders(&self) -> impl Iterator<Item = &PathBuf> {
        self.projects
            .iter()
            .flat_map(|project| {
                std::iter::once(&project.path)
                    .chain(project.workspaces.iter().map(|workspace| &workspace.path))
            })
            .chain(
                self.threads
                    .iter()
                    .filter_map(|thread| thread.workspace.as_ref()),
            )
    }

    pub fn set_git_heads(&mut self, heads: BTreeMap<PathBuf, GitHead>) {
        if self.git_heads != heads {
            self.git_heads = heads;
            self.changed();
        }
    }

    /// The project's icon file on its server's machine, if any ([`ProjectsSnapshot::favicons`]).
    pub fn favicon(&self, project: ProjectId) -> Option<&Path> {
        self.favicons.get(&project).map(PathBuf::as_path)
    }

    pub fn set_favicons(&mut self, mut favicons: BTreeMap<ProjectId, PathBuf>) {
        // A project removed while its folder was read has no icon to show.
        favicons.retain(|id, _| self.project(*id).is_some());
        if self.favicons != favicons {
            self.favicons = favicons;
            self.changed();
        }
    }

    /// How the project's folder stands in git, for a project with copies on other machines
    /// ([`ProjectsSnapshot::copy_statuses`]).
    pub fn copy_status(&self, project: ProjectId) -> Option<&CopyStatus> {
        self.copy_statuses.get(&project)
    }

    pub fn set_copy_statuses(&mut self, mut statuses: BTreeMap<ProjectId, CopyStatus>) {
        statuses.retain(|id, _| self.project(*id).is_some());
        if self.copy_statuses != statuses {
            self.copy_statuses = statuses;
            self.changed();
        }
    }

    pub fn record_thread_activity(&mut self, id: ThreadId) {
        if let Some(thread) = self.threads.iter_mut().find(|thread| thread.id == id) {
            thread.last_activity_at = Some(SystemTime::now());
            self.changed();
        }
    }

    /// Everything in the store, for a client's copy.
    pub fn snapshot(&self) -> ProjectsSnapshot {
        let mut working_threads: Vec<_> = self.working_threads.iter().copied().collect();
        working_threads.sort();
        let mut blocked_threads: Vec<_> = self.blocked_threads.iter().copied().collect();
        blocked_threads.sort();
        let mut awaiting_input_threads: Vec<_> =
            self.awaiting_input_threads.iter().copied().collect();
        awaiting_input_threads.sort();
        let mut waiting_threads: Vec<_> = self.waiting_threads.iter().copied().collect();
        waiting_threads.sort();
        let busy_since = self
            .busy_since
            .iter()
            .map(|(id, since)| (*id, *since))
            .collect();
        ProjectsSnapshot {
            projects: self.projects.clone(),
            threads: self.threads.clone(),
            scope: self.scope,
            thread_order: self.thread_order,
            archived_expanded: self.archived_expanded,
            workspaces_expanded: self.workspaces_expanded,
            working_threads,
            blocked_threads,
            awaiting_input_threads,
            waiting_threads,
            busy_since,
            terminal_agents: self
                .terminal_agents
                .iter()
                .map(|(id, agent)| (*id, agent.clone()))
                .collect(),
            terminal_commands: self
                .terminal_commands
                .iter()
                .map(|(id, command)| (*id, command.clone()))
                .collect(),
            terminal_folders: self
                .terminal_folders
                .iter()
                .map(|(id, folder)| (*id, folder.clone()))
                .collect(),
            git_heads: self
                .git_heads
                .iter()
                .map(|(folder, head)| (folder.clone(), head.clone()))
                .collect(),
            favicons: self
                .favicons
                .iter()
                .map(|(id, path)| (*id, path.clone()))
                .collect(),
            copy_statuses: self
                .copy_statuses
                .iter()
                .map(|(id, status)| (*id, status.clone()))
                .collect(),
            drawer_commands: self
                .drawer_commands
                .iter()
                .map(|((id, number), command)| (*id, *number, command.clone()))
                .collect(),
        }
    }

    /// A client's read-only copy of a server's store. Nothing is saved.
    pub fn from_snapshot(snapshot: ProjectsSnapshot) -> Self {
        let mut this = Self::from_state(
            PersistedState {
                next_id: 0,
                projects: snapshot.projects,
                threads: snapshot.threads,
                scope: snapshot.scope,
                thread_order: snapshot.thread_order,
                archived_expanded: snapshot.archived_expanded,
                workspaces_expanded: snapshot.workspaces_expanded,
            },
            None,
        );
        this.working_threads = snapshot.working_threads.into_iter().collect();
        this.blocked_threads = snapshot.blocked_threads.into_iter().collect();
        this.awaiting_input_threads = snapshot.awaiting_input_threads.into_iter().collect();
        this.waiting_threads = snapshot.waiting_threads.into_iter().collect();
        this.busy_since = snapshot.busy_since.into_iter().collect();
        this.terminal_agents = snapshot.terminal_agents.into_iter().collect();
        this.terminal_commands = snapshot.terminal_commands.into_iter().collect();
        this.terminal_folders = snapshot.terminal_folders.into_iter().collect();
        this.git_heads = snapshot.git_heads.into_iter().collect();
        this.favicons = snapshot.favicons.into_iter().collect();
        this.copy_statuses = snapshot.copy_statuses.into_iter().collect();
        this.drawer_commands = snapshot
            .drawer_commands
            .into_iter()
            .map(|(id, number, command)| ((id, number), command))
            .collect();
        this
    }

    /// Writes the state now, and returns once it's written.
    pub fn flush_saves(&self) {
        if let Some(saver) = &self.saver {
            saver.flush();
        }
    }

    /// Stops writing the state: another process owns it now.
    pub fn stop_saving(&mut self) {
        if let Some(saver) = self.saver.take() {
            saver.discard();
        }
    }

    fn allocate_id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    fn changed(&mut self) {
        self.revision += 1;
        let Some(saver) = &self.saver else {
            return;
        };
        saver.save(PersistedState {
            next_id: self.next_id,
            projects: self.projects.clone(),
            threads: self.threads.clone(),
            scope: self.scope,
            thread_order: self.thread_order,
            archived_expanded: self.archived_expanded,
            workspaces_expanded: self.workspaces_expanded,
        });
    }
}

/// Writes a state as JSON on its own thread, once changes have paused for [`SAVE_DEBOUNCE`].
/// Dropping it writes any pending state before returning, so nothing is lost on quit.
pub struct Saver<T> {
    sender: Option<mpsc::Sender<SaverMessage<T>>>,
    thread: Option<std::thread::JoinHandle<()>>,
}

enum SaverMessage<T> {
    Save(T),
    /// Write what's pending now, then answer.
    Flush(mpsc::Sender<()>),
    /// Stop without writing what's pending.
    Discard,
}

impl<T: Serialize + Send + 'static> Saver<T> {
    pub fn new(state_path: PathBuf, thread_name: &str) -> Self {
        let (sender, receiver) = mpsc::channel::<SaverMessage<T>>();
        let thread = std::thread::Builder::new()
            .name(thread_name.into())
            .spawn(move || {
                let mut pending: Option<T> = None;
                loop {
                    let message = if pending.is_some() {
                        receiver.recv_timeout(SAVE_DEBOUNCE)
                    } else {
                        receiver
                            .recv()
                            .map_err(|_| mpsc::RecvTimeoutError::Disconnected)
                    };
                    match message {
                        Ok(SaverMessage::Save(state)) => pending = Some(state),
                        Ok(SaverMessage::Flush(done)) => {
                            if let Some(state) = pending.take() {
                                write_state(&state_path, &state).log_err();
                            }
                            done.send(()).ok();
                        }
                        Ok(SaverMessage::Discard) => return,
                        Err(mpsc::RecvTimeoutError::Timeout) => {
                            if let Some(state) = pending.take() {
                                write_state(&state_path, &state).log_err();
                            }
                        }
                        Err(mpsc::RecvTimeoutError::Disconnected) => {
                            if let Some(state) = pending.take() {
                                write_state(&state_path, &state).log_err();
                            }
                            return;
                        }
                    }
                }
            })
            .log_err();
        Self {
            sender: thread.is_some().then_some(sender),
            thread,
        }
    }

    pub fn save(&self, state: T) {
        if let Some(sender) = &self.sender {
            sender.send(SaverMessage::Save(state)).log_err();
        }
    }

    /// Writes the pending state now, and returns once it's written.
    pub fn flush(&self) {
        let Some(sender) = &self.sender else {
            return;
        };
        let (done, written) = mpsc::channel();
        if sender.send(SaverMessage::Flush(done)).log_err().is_some() {
            written.recv().log_err();
        }
    }

    /// Stops saving, dropping what's pending: another process owns the file now.
    pub fn discard(mut self) {
        if let Some(sender) = self.sender.take() {
            sender.send(SaverMessage::Discard).log_err();
        }
    }
}

impl<T> Drop for Saver<T> {
    fn drop(&mut self) {
        self.sender.take();
        if let Some(thread) = self.thread.take()
            && thread.join().is_err()
        {
            log::error!("a saver thread panicked");
        }
    }
}

/// The project a folder is in: the deepest whose folder, worktree or pasture holds it.
pub fn project_at<'a>(projects: &'a [Project], folder: &Path) -> Option<&'a Project> {
    projects
        .iter()
        .flat_map(|project| {
            std::iter::once(&project.path)
                .chain(project.workspaces.iter().map(|workspace| &workspace.path))
                .filter(|root| folder.starts_with(root))
                .map(move |root| (root.components().count(), project))
        })
        .max_by_key(|(depth, _)| *depth)
        .map(|(_, project)| project)
}

fn project_name(path: &Path) -> SharedString {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string_lossy().into_owned())
        .into()
}

/// Reads a state [`Saver`] wrote, or `None` when there's none yet.
pub fn read_state<T: serde::de::DeserializeOwned>(path: &Path) -> Result<Option<T>> {
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(error).with_context(|| format!("reading {}", path.display()));
        }
    };
    let state =
        serde_json::from_slice(&bytes).with_context(|| format!("parsing {}", path.display()))?;
    Ok(Some(state))
}

fn write_state(path: &Path, state: &impl Serialize) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("creating {}", parent.display()))?;
    }
    let json = serde_json::to_vec_pretty(state)?;
    write_durably(path, &path.with_extension("json.tmp"), |file| {
        file.write_all(&json)
    })
}

/// Replaces `path` as herdr saves its state: `write` fills `temporary`, beside it, which is
/// synced to disk before it's renamed into place, and the folder is synced after. A crash or
/// power loss then leaves the old file or the whole new one, never a rename whose contents
/// weren't written yet. A failed write removes `temporary`.
pub fn write_durably(
    path: &Path,
    temporary: &Path,
    write: impl FnOnce(&mut std::fs::File) -> std::io::Result<()>,
) -> Result<()> {
    let written = std::fs::File::create(temporary)
        .and_then(|mut file| {
            // Only its owner reads what agentZ saves: agents' settings and MCP servers keep
            // API keys. Set before anything is written, and on a temporary file a crash left.
            #[cfg(unix)]
            file.set_permissions(
                <std::fs::Permissions as std::os::unix::fs::PermissionsExt>::from_mode(0o600),
            )?;
            write(&mut file)?;
            file.sync_all()
        })
        .with_context(|| format!("writing {}", temporary.display()))
        .and_then(|()| {
            std::fs::rename(temporary, path)
                .with_context(|| format!("replacing {}", path.display()))
        });
    if let Err(error) = written {
        match std::fs::remove_file(temporary) {
            Ok(()) => {}
            Err(removing) if removing.kind() == std::io::ErrorKind::NotFound => {}
            Err(removing) => log::warn!("removing {}: {removing}", temporary.display()),
        }
        return Err(error);
    }
    sync_folder_of(path)
}

/// Writes of one file from background tasks, one at a time and in the order they were asked
/// for: a write that a newer one overtook is skipped. Two at once would share
/// [`write_durably`]'s temporary file, and the older could land last, or inside the newer.
#[derive(Default)]
pub struct OrderedWrites {
    asked: u64,
    written: Arc<Mutex<u64>>,
}

impl OrderedWrites {
    /// The next write's turn, to make on another thread.
    pub fn next(&mut self) -> OrderedWrite {
        self.asked += 1;
        OrderedWrite {
            turn: self.asked,
            written: self.written.clone(),
        }
    }
}

pub struct OrderedWrite {
    turn: u64,
    written: Arc<Mutex<u64>>,
}

impl OrderedWrite {
    pub fn write(self, write: impl FnOnce() -> Result<()>) -> Result<()> {
        let mut written = self
            .written
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if *written > self.turn {
            return Ok(());
        }
        *written = self.turn;
        write()
    }
}

/// Syncs the folder `path` is in, so what was renamed into it is on disk too.
pub fn sync_folder_of(path: &Path) -> Result<()> {
    let Some(folder) = path.parent() else {
        return Ok(());
    };
    match std::fs::File::open(folder).and_then(|folder| folder.sync_all()) {
        Ok(()) => Ok(()),
        // Some file systems can't sync a folder (herdr's `directory_sync_unsupported`).
        Err(error)
            if matches!(
                error.kind(),
                std::io::ErrorKind::Unsupported
                    | std::io::ErrorKind::InvalidInput
                    | std::io::ErrorKind::PermissionDenied
            ) =>
        {
            Ok(())
        }
        Err(error) => Err(error).with_context(|| format!("syncing {}", folder.display())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn durable_writes_replace_the_file_or_leave_it() {
        let folder = tempfile::tempdir().expect("temp dir");
        let path = folder.path().join("state.json");
        let temporary = path.with_extension("json.tmp");
        write_durably(&path, &temporary, |file| file.write_all(b"one")).expect("writes");
        write_durably(&path, &temporary, |file| file.write_all(b"two")).expect("replaces");
        assert_eq!(std::fs::read(&path).expect("reads"), b"two");
        assert!(!temporary.exists());

        let failed = write_durably(&path, &temporary, |file| {
            file.write_all(b"thr")?;
            Err(std::io::Error::other("the disk is full"))
        });
        assert!(format!("{:#}", failed.expect_err("fails")).contains("the disk is full"));
        assert_eq!(std::fs::read(&path).expect("reads"), b"two");
        assert!(!temporary.exists());
    }

    #[test]
    fn an_unreadable_state_is_an_error_and_stays() {
        let dir = tempfile::tempdir().expect("temp dir");
        let state_path = dir.path().join("state.json");
        std::fs::write(&state_path, b"{\"projects\": 7}").expect("writes");
        assert!(ProjectStore::load(Some(state_path.clone())).is_err());
        assert_eq!(
            std::fs::read(&state_path).expect("reads"),
            b"{\"projects\": 7}"
        );
        // One that isn't there yet is a new, empty store.
        let store = ProjectStore::load(Some(dir.path().join("new.json"))).expect("loads");
        assert!(store.projects().is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn durable_writes_are_for_their_owner_only() {
        use std::os::unix::fs::PermissionsExt as _;
        let folder = tempfile::tempdir().expect("temp dir");
        let path = folder.path().join("mcp-servers.json");
        let temporary = path.with_extension("json.tmp");
        // Left by a crash, as anyone could read it.
        std::fs::write(&temporary, b"").expect("writes");
        std::fs::set_permissions(&temporary, std::fs::Permissions::from_mode(0o644))
            .expect("opens it up");
        write_durably(&path, &temporary, |file| {
            file.write_all(b"{\"API_KEY\": \"x\"}")
        })
        .expect("writes");
        let mode = std::fs::metadata(&path)
            .expect("there")
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600);
    }

    /// An older write that runs after a newer one, as background tasks can, is skipped.
    #[test]
    fn ordered_writes_never_put_an_older_over_a_newer() {
        let mut writes = OrderedWrites::default();
        let older = writes.next();
        let newer = writes.next();
        let written = std::cell::RefCell::new(Vec::new());
        let record = |name: &'static str| {
            written.borrow_mut().push(name);
            Ok(())
        };
        newer.write(|| record("newer")).expect("writes");
        older.write(|| record("older")).expect("skips");
        writes.next().write(|| record("newest")).expect("writes");
        assert_eq!(*written.borrow(), ["newer", "newest"]);
    }

    #[test]
    fn monograms() {
        assert_eq!(monogram("dramabot"), "DT");
        assert_eq!(monogram("shop-landing"), "SL");
        assert_eq!(monogram("web3app"), "W3");
        assert_eq!(monogram("x"), "XX");
        assert_eq!(monogram("--"), "PR");
    }

    #[test]
    fn monogram_colors_are_stable() {
        assert_eq!(
            monogram_color_index("agentZ"),
            monogram_color_index("AGENTZ")
        );
        assert!(monogram_color_index("anything") < MONOGRAM_COLORS.len());
    }

    #[test]
    fn adding_removing_and_scoping_projects() {
        let dir = tempfile::tempdir().expect("temp dir");
        let state_path = dir.path().join("state.json");
        let first_folder = dir.path().join("first");
        let second_folder = dir.path().join("second");
        std::fs::create_dir_all(&first_folder).expect("create first");
        std::fs::create_dir_all(&second_folder).expect("create second");

        let mut store = ProjectStore::load(Some(state_path.clone())).expect("loads");
        let revision = store.revision();
        let first = store.add_project(first_folder.clone());
        let second = store.add_project(second_folder);
        assert!(store.revision() > revision);
        let revision = store.revision();
        assert_eq!(store.add_project(first_folder), first);
        assert_eq!(store.revision(), revision, "re-adding changes nothing");
        store.add_thread(first, "Fix login bug", None);
        store.set_scope(ProjectScope::Project(second));

        assert_eq!(store.projects().len(), 2);
        assert_eq!(store.thread_count(first), 1);
        let visible: Vec<_> = store.visible_projects().map(|p| p.id).collect();
        assert_eq!(visible, vec![second]);

        // Dropping the store writes what's pending.
        drop(store);
        let mut store = ProjectStore::load(Some(state_path)).expect("loads");
        assert_eq!(store.projects().len(), 2);
        assert_eq!(store.scope(), ProjectScope::Project(second));
        assert_eq!(store.project(first).map(|p| p.name()), Some("first".into()));

        let older = store.add_thread(first, "Older", None).expect("thread");
        let newer = store.add_thread(first, "Newer", None).expect("thread");
        store.set_thread_order(ThreadOrder::LastActivity);
        store.set_thread_working(older, true);
        let order: Vec<_> = store.threads_for(first).map(|thread| thread.id).collect();
        assert_eq!(order[0], older, "most recent activity first");
        store.set_thread_order(ThreadOrder::Created);
        let order: Vec<_> = store.threads_for(first).map(|thread| thread.id).collect();
        assert_eq!(order[0], newer, "newest thread first");
        store.set_thread_session(newer, "session-7".into());
        assert_eq!(
            store
                .thread(newer)
                .and_then(|thread| thread.session_id.clone()),
            Some("session-7".into())
        );

        store.set_scope(ProjectScope::All);
        let thread = store.add_thread(first, "To archive", None).expect("thread");
        store.archive_thread(thread);
        assert!(store.threads_for(first).all(|t| t.id != thread));
        assert!(store.archived_threads().iter().any(|t| t.id == thread));
        store.unarchive_thread(thread);
        assert!(store.threads_for(first).any(|t| t.id == thread));
        store.set_custom_title(thread, "Mine".into());
        store.rename_thread(thread, "Automatic".into());
        assert_eq!(store.thread(thread).map(|t| t.title.as_str()), Some("Mine"));
        // Automatic titles keep coming in underneath, and clearing the user's shows the latest.
        store.rename_thread(thread, "Later".into());
        store.set_custom_title(thread, " ".into());
        let renamed = store.thread(thread).expect("thread");
        assert_eq!(
            (renamed.title.as_str(), renamed.has_custom_title),
            ("Later", false)
        );
        store.rename_thread(thread, "Latest".into());
        assert_eq!(
            store.thread(thread).map(|t| t.title.as_str()),
            Some("Latest")
        );
        store.delete_thread(thread);
        assert!(store.thread(thread).is_none());

        store.set_project_name(first, "  Renamed ");
        assert_eq!(
            store.project(first).map(|p| p.name()),
            Some("Renamed".into())
        );
        store.set_project_name(first, "first");
        assert_eq!(
            store.project(first).and_then(|p| p.custom_name.clone()),
            None
        );
        let icon = ProjectIcon::Monogram {
            text: "FI".into(),
            color: "teal".into(),
        };
        store.set_project_icon(first, Some(icon.clone()));
        assert_eq!(
            store.project(first).and_then(|p| p.icon.clone()),
            Some(icon)
        );

        store.remove_project(second);
        assert_eq!(store.scope(), ProjectScope::All);
        assert_eq!(store.projects().len(), 1);
    }

    #[test]
    fn pinning_and_arranging_threads() {
        let dir = tempfile::tempdir().expect("temp dir");
        let mut store = ProjectStore::load(None).expect("loads");
        let project = store.add_project(dir.path().to_path_buf());
        let first = store.add_thread(project, "First", None).expect("thread");
        let second = store.add_thread(project, "Second", None).expect("thread");
        let pinned = |store: &ProjectStore, id| {
            store
                .thread(id)
                .map(|thread| (thread.is_pinned(), thread.pin_order_key.clone()))
        };

        store.pin_thread(first, None).expect("pin");
        assert_eq!(pinned(&store, first), Some((true, None)));
        let pinned_at = store.thread(first).and_then(|thread| thread.pinned_at);
        store.pin_thread(first, Some("m".into())).expect("move");
        assert_eq!(pinned(&store, first), Some((true, Some("m".into()))));
        assert_eq!(
            store.thread(first).and_then(|thread| thread.pinned_at),
            pinned_at,
            "moving keeps when it was pinned"
        );
        assert!(store.pin_thread(second, Some("ma".into())).is_err());

        // All the keys or none.
        let revision = store.revision();
        assert!(
            store
                .set_order_keys(
                    ThreadSection::Pinned,
                    vec![(first, "g".into()), (second, "t".into())],
                )
                .is_err()
        );
        assert_eq!(store.revision(), revision);
        assert_eq!(pinned(&store, first), Some((true, Some("m".into()))));
        store
            .set_order_keys(ThreadSection::Active, vec![(second, "t".into())])
            .expect("arrange");
        assert_eq!(
            store
                .thread(second)
                .and_then(|thread| thread.active_order_key.clone()),
            Some("t".into())
        );

        // Unpinning keeps its place among the rest; archiving forgets both.
        store
            .set_order_keys(ThreadSection::Active, vec![(first, "f".into())])
            .expect_err("pinned");
        store.unpin_thread(first);
        assert_eq!(pinned(&store, first), Some((false, None)));
        store.pin_thread(second, Some("m".into())).expect("pin");
        store.archive_thread(second);
        let archived = store.thread(second).expect("thread");
        assert_eq!(
            (archived.is_pinned(), archived.active_order_key.clone()),
            (false, None)
        );
        store.pin_thread(second, None).expect("pin from archived");
        let thread = store.thread(second).expect("thread");
        assert!(thread.is_pinned() && thread.archived_at.is_none());

        let shell = store
            .add_terminal_thread(project, TerminalCommand::default())
            .expect("shell");
        assert!(store.pin_thread(shell, None).is_err());
        store
            .set_order_keys(ThreadSection::Active, vec![(shell, "f".into())])
            .expect("an agent CLI arranges among the rest");
        let draft = store.add_thread(project, "Draft", None).expect("thread");
        store.set_draft(draft, true);
        assert!(store.pin_thread(draft, None).is_err());
    }

    #[test]
    fn subthreads_belong_to_their_parent() {
        let dir = tempfile::tempdir().expect("temp dir");
        let mut store = ProjectStore::load(None).expect("loads");
        let project = store.add_project(dir.path().to_path_buf());
        let parent = store.add_thread(project, "Parent", None).expect("thread");
        let task = |parent| Task {
            parent,
            prompt: "Look into it".into(),
            role: None,
            client_request_id: None,
            outcome: None,
            delivered: false,
            agent_session: None,
            runs_on: None,
            parent_machine: None,
            stoppable: false,
            queued: None,
            can_delegate: false,
            changes: None,
        };
        let child = store.add_subthread(task(parent), None).expect("subthread");
        let grandchild = store.add_subthread(task(child), None).expect("subthread");

        let listed: Vec<_> = store.threads_for(project).map(|thread| thread.id).collect();
        assert_eq!(listed, vec![parent]);
        assert_eq!(store.thread(child).and_then(Thread::parent), Some(parent));
        assert_eq!(store.root_thread(grandchild), parent);
        assert_eq!(
            store.thread_and_subthreads(parent),
            vec![parent, child, grandchild]
        );
        store.set_thread_blocked(grandchild, true);
        assert!(store.is_thread_or_subthread_blocked(parent));
        assert!(!store.is_thread_blocked(parent));

        store.delete_thread(parent);
        assert!(store.threads().is_empty());
        assert!(!store.is_thread_blocked(grandchild));
    }

    /// An update carries only the threads that changed, and brings an older snapshot to the
    /// newer one: threads changed, added and gone, in the newer one's order.
    #[test]
    fn updates_carry_only_the_threads_that_changed() {
        let dir = tempfile::tempdir().expect("temp dir");
        let mut store = ProjectStore::load(None).expect("loads");
        let project = store.add_project(dir.path().to_path_buf());
        let first = store.add_thread(project, "First", None).expect("thread");
        let second = store.add_thread(project, "Second", None).expect("thread");
        let third = store.add_thread(project, "Third", None).expect("thread");
        let before = store.snapshot();

        store.rename_thread(second, "Second, renamed".into());
        store.delete_thread(third);
        let fourth = store.add_thread(project, "Fourth", None).expect("thread");
        store.set_thread_blocked(first, true);
        let after = store.snapshot();

        let update = before.update_to(&after);
        let changed: Vec<ThreadId> = update
            .changed
            .threads
            .iter()
            .map(|thread| thread.id)
            .collect();
        assert_eq!(changed, [second, fourth]);
        assert_eq!(update.changed.blocked_threads, [first]);
        let mut applied = before;
        applied.apply(update);
        assert_eq!(applied, after);
    }

    #[test]
    fn threads_await_input_until_answered_or_deleted() {
        let dir = tempfile::tempdir().expect("temp dir");
        let mut store = ProjectStore::load(None).expect("loads");
        let project = store.add_project(dir.path().to_path_buf());
        let thread = store.add_thread(project, "Thread", None).expect("thread");

        store.set_thread_awaiting_input(thread, true);
        let copy = ProjectStore::from_snapshot(store.snapshot());
        assert!(copy.is_thread_awaiting_input(thread));
        store.set_thread_awaiting_input(thread, false);
        assert!(!store.is_thread_awaiting_input(thread));

        store.set_thread_awaiting_input(thread, true);
        store.delete_thread(thread);
        assert!(!store.is_thread_awaiting_input(thread));
        assert!(store.snapshot().awaiting_input_threads.is_empty());
    }

    #[test]
    fn a_waiting_thread_completes_once_its_work_is_over() {
        let dir = tempfile::tempdir().expect("temp dir");
        let mut store = ProjectStore::load(None).expect("loads");
        let project = store.add_project(dir.path().to_path_buf());
        let thread = store.add_thread(project, "Thread", None).expect("thread");
        let completed_at = |store: &ProjectStore| store.thread(thread)?.completed_at;

        store.set_thread_working(thread, true);
        let since = store.busy_since(thread);
        assert!(since.is_some());
        store.set_thread_waiting(thread, true);
        store.set_thread_working(thread, false);
        assert_eq!(completed_at(&store), None);
        let copy = ProjectStore::from_snapshot(store.snapshot());
        assert!(copy.is_thread_waiting(thread));
        assert_eq!(copy.busy_since(thread), since);

        // Work it goes on with doesn't complete it either, until it ends, and it's been busy
        // since the first turn.
        std::thread::sleep(Duration::from_millis(2));
        store.set_thread_working(thread, true);
        store.set_thread_waiting(thread, false);
        assert_eq!(completed_at(&store), None);
        assert_eq!(store.busy_since(thread), since);
        store.set_thread_working(thread, false);
        assert!(completed_at(&store).is_some());
        assert_eq!(store.busy_since(thread), None);

        // Without a turn after it, the end of the wait completes it.
        let first = completed_at(&store);
        store.set_thread_waiting(thread, true);
        std::thread::sleep(Duration::from_millis(2));
        store.set_thread_waiting(thread, false);
        assert!(completed_at(&store) > first);
    }

    #[test]
    fn unsent_text_keeps_only_mentions_in_it() {
        let dir = tempfile::tempdir().expect("temp dir");
        let mut store = ProjectStore::load(None).expect("loads");
        let project = store.add_project(dir.path().to_path_buf());
        let thread = store.add_thread(project, "Thread", None).expect("thread");
        let mention = |range: Range<usize>| UnsentMention {
            range,
            target: Mentioned::Image("image.png".into()),
        };

        // "see @Image and é", where é takes two bytes.
        store.set_unsent_text(
            thread,
            Some("see @Image and é".into()),
            vec![
                mention(4..10),
                mention(2..6),
                mention(16..17),
                mention(15..15),
            ],
        );
        let unsent = store.thread(thread).expect("thread");
        assert_eq!(unsent.unsent_mentions, vec![mention(4..10)]);

        store.set_unsent_text(thread, Some("  ".into()), vec![mention(0..1)]);
        let unsent = store.thread(thread).expect("thread");
        assert_eq!(unsent.unsent_text, None);
        assert!(unsent.unsent_mentions.is_empty());
    }

    #[test]
    fn saves_after_changes_pause() {
        let dir = tempfile::tempdir().expect("temp dir");
        let state_path = dir.path().join("state.json");
        let mut store = ProjectStore::load(Some(state_path.clone())).expect("loads");
        store.add_project(dir.path().to_path_buf());
        assert!(!state_path.exists(), "saving waits for changes to pause");
        std::thread::sleep(SAVE_DEBOUNCE * 3);
        let saved: PersistedState = read_state(&state_path).expect("readable").expect("saved");
        assert_eq!(saved.projects.len(), 1);
    }

    #[test]
    fn threads_work_in_workspaces() {
        let dir = tempfile::tempdir().expect("temp dir");
        let mut store = ProjectStore::load(None).expect("loads");
        let project = store.add_project(dir.path().to_path_buf());
        let project_path = store.project(project).expect("project").path.clone();
        let pasture = dir.path().join("pastures/demo/agentz-1");
        store.add_workspace(
            project,
            Workspace {
                kind: WorkspaceKind::Pasture,
                path: pasture.clone(),
                branch: Some("agentz/1".into()),
                base: Some("main".into()),
                created_at: SystemTime::now(),
            },
        );
        let lead = store.add_thread(project, "Lead", None).expect("thread");
        assert_eq!(store.thread_folder(lead), Some(project_path.clone()));
        store.set_thread_workspace(lead, Some(pasture.clone()));
        assert_eq!(store.thread_folder(lead), Some(pasture.clone()));
        assert_eq!(
            store.thread_workspace(lead).map(|workspace| workspace.kind),
            Some(WorkspaceKind::Pasture)
        );

        let child = store
            .add_subthread(
                Task {
                    parent: lead,
                    prompt: "Help".into(),
                    role: None,
                    client_request_id: None,
                    outcome: None,
                    delivered: false,
                    agent_session: None,
                    runs_on: None,
                    parent_machine: None,
                    stoppable: false,
                    queued: None,
                    can_delegate: false,
                    changes: None,
                },
                None,
            )
            .expect("subthread");
        assert_eq!(store.thread_folder(child), Some(pasture.clone()));
        assert_eq!(store.threads_in_folder(&pasture), vec![lead, child]);
        assert!(store.threads_in_folder(&project_path).is_empty());

        store.remove_workspace(project, &pasture);
        assert!(store.thread_workspace(lead).is_none());
        assert_eq!(store.thread_folder(lead), Some(pasture), "keeps its folder");
    }

    #[test]
    fn workspaces_threads_work_in_any_folder_until_moved_to_a_project() {
        let dir = tempfile::tempdir().expect("temp dir");
        let state_path = dir.path().join("state.json");
        let project_folder = dir.path().join("storefront");
        let outside = dir.path().join("docs");
        std::fs::create_dir_all(project_folder.join("src")).expect("create project");
        let mut store = ProjectStore::load(Some(state_path.clone())).expect("loads");
        let project = store.add_project(project_folder);
        let project_path = store.project(project).expect("project").path.clone();
        let worktree = dir.path().join("worktrees/storefront/agentz-1");
        store.add_workspace(
            project,
            Workspace {
                kind: WorkspaceKind::Worktree,
                path: worktree.clone(),
                branch: Some("agentz/1".into()),
                base: None,
                created_at: SystemTime::now(),
            },
        );

        let in_src = store
            .add_thread(ProjectId::WORKSPACES, NEW_THREAD_TITLE, None)
            .expect("thread");
        store.set_thread_workspace(in_src, Some(project_path.join("src")));
        let in_worktree = store
            .add_thread(ProjectId::WORKSPACES, NEW_THREAD_TITLE, None)
            .expect("thread");
        store.set_thread_workspace(in_worktree, Some(worktree));
        let in_docs = store
            .add_thread(ProjectId::WORKSPACES, NEW_THREAD_TITLE, None)
            .expect("thread");
        store.set_thread_workspace(in_docs, Some(outside.clone()));

        assert!(store.threads_for(project).next().is_none(), "not listed");
        assert_eq!(store.thread_folder(in_src), Some(project_path.join("src")));
        assert_eq!(store.thread_project(in_src), Some(project));
        assert_eq!(store.thread_project(in_worktree), Some(project));
        assert_eq!(
            store.thread_workspace(in_worktree).map(|w| w.kind),
            Some(WorkspaceKind::Worktree)
        );
        assert_eq!(store.thread_project(in_docs), None);

        // They outlive a restart, and so does whether the section is open.
        assert!(!store.workspaces_expanded());
        store.toggle_workspaces_expanded();
        drop(store);
        let mut store = ProjectStore::load(Some(state_path)).expect("loads");
        assert_eq!(store.threads().len(), 3);
        assert!(store.workspaces_expanded());

        let child = store
            .add_subthread(
                Task {
                    parent: in_src,
                    prompt: "Help".into(),
                    role: None,
                    client_request_id: None,
                    outcome: None,
                    delivered: false,
                    agent_session: None,
                    runs_on: None,
                    parent_machine: None,
                    stoppable: false,
                    queued: None,
                    can_delegate: false,
                    changes: None,
                },
                None,
            )
            .expect("subthread");
        assert!(store.thread(child).is_some_and(Thread::in_workspaces));
        store.move_thread_to_project(in_src, project);
        for id in [in_src, child] {
            let thread = store.thread(id).expect("thread");
            assert_eq!(thread.project_id, project);
            assert_eq!(store.thread_folder(id), Some(project_path.join("src")));
        }
        assert_eq!(
            store.threads_for(project).map(|t| t.id).collect::<Vec<_>>(),
            vec![in_src]
        );

        let docs = store.add_project(outside.clone());
        store.move_thread_to_project(in_docs, docs);
        let thread = store.thread(in_docs).expect("thread");
        assert_eq!(thread.workspace, None, "works in the project's own folder");
        assert_eq!(store.thread_folder(in_docs), Some(outside));
    }

    #[test]
    fn chats_are_kept_apart_from_projects_and_archived_like_threads() {
        let dir = tempfile::tempdir().expect("temp dir");
        let state_path = dir.path().join("state.json");
        let chats_folder = dir.path().join("chats");
        let mut store = ProjectStore::load(Some(state_path.clone())).expect("loads");
        store.set_chats_folder(chats_folder.clone());
        let chat = store
            .add_thread(ProjectId::CHATS, "Async Rust", None)
            .expect("chat");
        let draft = store
            .add_thread(ProjectId::CHATS, NEW_THREAD_TITLE, None)
            .expect("chat");
        store.set_draft(draft, true);

        assert!(store.thread(chat).is_some_and(Thread::is_chat));
        assert!(store.thread(chat).is_some_and(Thread::can_arrange));
        assert!(
            !store.thread(draft).is_some_and(Thread::can_arrange),
            "drafts aren't listed"
        );

        // Until its first message, a chat works where its folder will be made.
        assert_eq!(store.thread_folder(chat), Some(chats_folder.clone()));
        let own = chats_folder.join("2026-01-01-async-rust-1");
        store.set_thread_workspace(chat, Some(own.clone()));
        assert_eq!(store.thread_folder(chat), Some(own));

        let in_an_hour = SystemTime::now() + Duration::from_secs(3600);
        store.snooze_thread(chat, in_an_hour).expect("snooze");
        store.pin_thread(chat, None).expect("pin");
        store.archive_thread(chat);
        let archived = |store: &ProjectStore| {
            store
                .archived_threads()
                .iter()
                .map(|t| t.id)
                .collect::<Vec<_>>()
        };
        assert_eq!(archived(&store), vec![chat]);
        assert!(
            store
                .thread(chat)
                .is_some_and(|t| !t.is_pinned() && t.snoozed_until.is_none())
        );

        // It outlives a restart, archived, and comes back as a thread does.
        drop(store);
        let mut store = ProjectStore::load(Some(state_path)).expect("loads");
        assert_eq!(archived(&store), vec![chat]);
        store.unarchive_thread(chat);
        assert!(archived(&store).is_empty());
    }

    #[test]
    fn only_idle_unpinned_threads_left_alone_are_stale() {
        let dir = tempfile::tempdir().expect("temp dir");
        let mut store = ProjectStore::load(Some(dir.path().join("state.json"))).expect("loads");
        let project = store.add_project(dir.path().join("storefront"));
        let long_ago = SystemTime::now() - std::time::Duration::from_secs(5 * 86_400);
        let cutoff = SystemTime::now() - std::time::Duration::from_secs(3 * 86_400);
        let old_thread = |store: &mut ProjectStore, title: &str| {
            let id = store.add_thread(project, title, None).expect("thread");
            let thread = store
                .threads
                .iter_mut()
                .find(|thread| thread.id == id)
                .expect("the thread");
            thread.last_activity_at = Some(long_ago);
            id
        };
        let stale = old_thread(&mut store, "Fix the cart");
        let pinned = old_thread(&mut store, "Pinned");
        store.pin_thread(pinned, None).expect("pin");
        let working = old_thread(&mut store, "Working");
        store.set_thread_working(working, true);
        let archived = old_thread(&mut store, "Archived");
        store.archive_thread(archived);
        let snoozed = old_thread(&mut store, "Snoozed");
        let in_an_hour = SystemTime::now() + Duration::from_secs(3600);
        store.snooze_thread(snoozed, in_an_hour).expect("snooze");
        let chat = store
            .add_thread(ProjectId::CHATS, "Trip ideas", None)
            .expect("chat");
        store
            .threads
            .iter_mut()
            .find(|thread| thread.id == chat)
            .expect("the chat")
            .last_activity_at = Some(long_ago);
        let recent = store.add_thread(project, "Just now", None).expect("thread");

        assert_eq!(store.stale_threads(cutoff), vec![stale, chat]);
        assert!(store.thread(recent).is_some());
        store.set_thread_working(working, false);
        assert_eq!(
            store.stale_threads(cutoff),
            vec![stale, chat],
            "just worked"
        );
        store.wake_thread(snoozed);
        assert_eq!(store.stale_threads(cutoff), vec![stale, chat], "just woken");
    }

    #[test]
    fn snoozed_threads_wake_on_time_or_once_they_need_the_user() {
        let dir = tempfile::tempdir().expect("temp dir");
        let state_path = dir.path().join("state.json");
        let mut store = ProjectStore::load(Some(state_path.clone())).expect("loads");
        let project = store.add_project(dir.path().join("storefront"));
        let thread = store
            .add_thread(project, "Fix the cart", None)
            .expect("thread");
        let now = SystemTime::now();
        let hour = Duration::from_secs(3600);
        assert!(
            store.snooze_thread(thread, now - hour).is_err(),
            "in the past"
        );
        store.snooze_thread(thread, now + hour).expect("snooze");
        assert!(store.is_thread_snoozed(thread, now));
        assert!(!store.has_thread_woken(thread, now));
        assert!(!store.is_thread_snoozed(thread, now + 2 * hour), "on time");
        assert!(store.has_thread_woken(thread, now + 2 * hour));

        store.set_thread_blocked(thread, true);
        assert!(!store.is_thread_snoozed(thread, now), "asks for permission");
        assert!(store.snooze_thread(thread, now + 2 * hour).is_err());
        store.set_thread_blocked(thread, false);
        store.set_thread_awaiting_input(thread, true);
        assert!(!store.is_thread_snoozed(thread, now), "asks for input");
        store.set_thread_awaiting_input(thread, false);
        store.set_thread_working(thread, true);
        assert!(
            store.is_thread_snoozed(thread, now),
            "works on while snoozed"
        );
        store.set_thread_working(thread, false);
        assert!(!store.is_thread_snoozed(thread, now), "finished its turn");
        assert!(store.has_thread_woken(thread, now));
        store.wake_thread(thread);
        assert!(!store.has_thread_woken(thread, now));
        assert!(store.thread(thread).expect("thread").woke_at.is_some());

        store
            .snooze_thread(thread, now + hour)
            .expect("snooze again");
        assert!(
            store.is_thread_snoozed(thread, now),
            "an earlier turn doesn't wake it"
        );
        drop(store);
        let mut store = ProjectStore::load(Some(state_path)).expect("loads");
        assert!(store.is_thread_snoozed(thread, now), "kept");
        store.pin_thread(thread, None).expect("pin");
        assert!(!store.has_thread_woken(thread, now), "pinning wakes it");

        store
            .snooze_thread(thread, now + hour)
            .expect("snooze once more");
        store.archive_thread(thread);
        assert_eq!(store.thread(thread).expect("thread").snoozed_until, None);
        assert!(
            store.snooze_thread(thread, now + hour).is_err(),
            "archived threads aren't snoozed"
        );
    }

    #[test]
    fn imported_sessions_become_threads_in_their_folder() {
        let dir = tempfile::tempdir().expect("temp dir");
        let mut store = ProjectStore::load(None).expect("loads");
        let project = store.add_project(dir.path().to_path_buf());
        let project_path = store.project(project).expect("project").path.clone();
        let worktree = dir.path().join("worktrees/demo/agentz-1");
        store.add_workspace(
            project,
            Workspace {
                kind: WorkspaceKind::Worktree,
                path: worktree.clone(),
                branch: Some("agentz/1".into()),
                base: None,
                created_at: SystemTime::now(),
            },
        );
        assert_eq!(store.folder_owner(&project_path), Some((project, None)));
        assert_eq!(
            store.folder_owner(&worktree),
            Some((project, Some(worktree.clone())))
        );
        assert_eq!(store.folder_owner(&dir.path().join("elsewhere")), None);

        let earlier = SystemTime::UNIX_EPOCH + Duration::from_secs(1_000_000);
        let recent = store.add_thread(project, "Recent", None).expect("thread");
        let imported = store
            .add_imported_thread(ImportedSession {
                project_id: project,
                workspace: Some(worktree.clone()),
                agent_id: "codex".into(),
                account: None,
                session_id: "session-a".into(),
                title: "Fix the login".into(),
                updated_at: Some(earlier),
                archived: false,
            })
            .expect("imported");
        let thread = store.thread(imported).expect("thread");
        assert_eq!(thread.session_id.as_deref(), Some("session-a"));
        assert_eq!(thread.title, "Fix the login");
        assert_eq!(thread.last_activity_at, Some(earlier));
        assert_eq!(store.thread_folder(imported), Some(worktree));
        store.set_thread_order(ThreadOrder::LastActivity);
        let listed: Vec<_> = store.threads_for(project).map(|thread| thread.id).collect();
        assert_eq!(
            listed,
            vec![recent, imported],
            "ordered by when it was active"
        );

        assert_eq!(
            store.thread_for_session("codex", None, "session-a"),
            Some(imported)
        );
        assert_eq!(store.thread_for_session("claude", None, "session-a"), None);
        assert_eq!(
            store.thread_for_session("codex", Some(AccountId(1)), "session-a"),
            None
        );

        let archived = store
            .add_imported_thread(ImportedSession {
                project_id: project,
                workspace: None,
                agent_id: "codex".into(),
                account: Some(AccountId(1)),
                session_id: "session-b".into(),
                title: "Old work".into(),
                updated_at: None,
                archived: true,
            })
            .expect("imported");
        assert_eq!(
            store
                .archived_threads()
                .iter()
                .map(|thread| thread.id)
                .collect::<Vec<_>>(),
            vec![archived]
        );
        assert_eq!(
            store.thread(archived).and_then(|thread| thread.account),
            Some(AccountId(1))
        );
    }
}
