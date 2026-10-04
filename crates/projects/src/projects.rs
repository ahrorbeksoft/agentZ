//! The set of project folders the user has opened, their threads, and which of them the
//! window is currently showing ("All projects" or a single project).
//!
//! Plain Rust with no UI framework, so the server can own it. Whoever owns the store learns of
//! changes through [`ProjectStore::revision`].

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
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
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ThreadId(pub u64);

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

/// A project icon picked by the user, as in t3code's project settings.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ProjectIcon {
    /// Up to two letters on a tile of the named color.
    Monogram { text: String, color: String },
    /// An image file anywhere on disk.
    Image { path: PathBuf },
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
    /// The registry id of the agent the thread was started with.
    #[serde(default)]
    pub agent_id: Option<String>,
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
    /// Set when the thread is archived; archived threads only show in Thread History.
    #[serde(default)]
    pub archived_at: Option<SystemTime>,
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
    /// Set on a terminal thread, which runs this instead of an ACP agent.
    #[serde(default)]
    pub terminal: Option<TerminalCommand>,
    /// The thread this one continues with another agent, whose conversation went with its
    /// first message.
    #[serde(default)]
    pub continued_from: Option<ThreadId>,
    /// A thread the user started and hasn't sent anything in yet: t3code's draft thread. It's
    /// left out of the thread list, and removed once the user leaves it with nothing typed.
    #[serde(default)]
    pub is_draft: bool,
    /// What's typed in the thread's composer and not sent yet (t3code's composer draft), kept
    /// while the user is elsewhere.
    #[serde(default)]
    pub unsent_text: Option<String>,
}

/// An agent's session to add as a thread: [`ProjectStore::add_imported_thread`].
#[derive(Clone, Debug, PartialEq)]
pub struct ImportedSession {
    pub project_id: ProjectId,
    /// The project's worktree or pasture the session ran in, or `None` for its own folder.
    pub workspace: Option<PathBuf>,
    pub agent_id: String,
    pub session_id: String,
    pub title: String,
    /// When the agent last worked on it, which orders it among the other threads.
    pub updated_at: Option<SystemTime>,
    /// Imported straight into Archived.
    pub archived: bool,
}

impl Thread {
    /// The thread that delegated this one, for a subthread.
    pub fn parent(&self) -> Option<ThreadId> {
        self.task.as_ref().map(|task| task.parent)
    }

    /// Started in a workspace pane, so listed in the Workspaces section.
    pub fn in_workspaces(&self) -> bool {
        self.project_id == ProjectId::WORKSPACES
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
    /// The parent has the outcome: it waited for it, read it, cancelled the task, or was sent
    /// word that it ended.
    #[serde(default)]
    pub delivered: bool,
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
    /// Terminal threads running an agent CLI, with its name. The rest are plain shells.
    pub terminal_agents: Vec<(ThreadId, String)>,
    /// Terminal threads running a program in front of their shell, with its name.
    pub terminal_commands: Vec<(ThreadId, String)>,
    /// Where each terminal thread's foreground process is.
    pub terminal_folders: Vec<(ThreadId, TerminalFolder)>,
    /// Drawer terminals running a program in front of their shell: thread, terminal number,
    /// program.
    pub drawer_commands: Vec<(ThreadId, u32, String)>,
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
    /// Threads whose agent is currently running. Not persisted: nothing is running after a
    /// restart.
    working_threads: HashSet<ThreadId>,
    /// Threads with a permission request waiting. Not persisted either.
    blocked_threads: HashSet<ThreadId>,
    /// Threads with a request for input waiting. Not persisted either.
    awaiting_input_threads: HashSet<ThreadId>,
    /// Terminal threads running an agent CLI, by its name. Not persisted either.
    terminal_agents: BTreeMap<ThreadId, String>,
    /// Terminal threads running a program in front of their shell. Not persisted either.
    terminal_commands: BTreeMap<ThreadId, String>,
    /// Where terminal threads' foreground processes are. Not persisted either.
    terminal_folders: BTreeMap<ThreadId, TerminalFolder>,
    /// Drawer terminals running a program in front of their shell. Not persisted either.
    drawer_commands: BTreeMap<(ThreadId, u32), String>,
    /// Counts changes, so the owner can tell whether a call changed anything.
    revision: u64,
    saver: Option<Saver<PersistedState>>,
}

impl ProjectStore {
    /// Loads the store from `state_path`, and saves every change back to it. With `None`,
    /// nothing is read or saved.
    pub fn load(state_path: Option<PathBuf>) -> Self {
        let state = state_path
            .as_deref()
            .and_then(|path| read_state::<PersistedState>(path).log_err())
            .flatten()
            .unwrap_or_default();
        Self::from_state(state, state_path)
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
            working_threads: HashSet::default(),
            blocked_threads: HashSet::default(),
            awaiting_input_threads: HashSet::default(),
            terminal_agents: BTreeMap::new(),
            terminal_commands: BTreeMap::new(),
            terminal_folders: BTreeMap::new(),
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
            thread.in_workspaces() || this.projects.iter().any(|p| p.id == thread.project_id)
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
        if project_id != ProjectId::WORKSPACES {
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
            last_activity_at: Some(now),
            created_at: Some(now),
            session_id: None,
            archived_at: None,
            has_custom_title: false,
            model: None,
            completed_at: None,
            created_by: None,
            task: None,
            workspace: None,
            started_in: None,
            terminal: None,
            continued_from: None,
            is_draft: false,
            unsent_text: None,
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

    /// Adds a thread for a conversation the agent already keeps (from ACP's `session/list`).
    /// The agent loads it when the thread opens.
    pub fn add_imported_thread(&mut self, session: ImportedSession) -> Option<ThreadId> {
        let id = self.add_thread(session.project_id, session.title, Some(session.agent_id))?;
        let archived_at = session.archived.then(SystemTime::now);
        let thread = self.threads.iter_mut().find(|thread| thread.id == id)?;
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

    /// The thread that has the agent's session.
    pub fn thread_for_session(&self, agent_id: &str, session_id: &str) -> Option<ThreadId> {
        self.threads
            .iter()
            .find(|thread| {
                thread.agent_id.as_deref() == Some(agent_id)
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
            self.working_threads.remove(&id);
            self.blocked_threads.remove(&id);
            self.awaiting_input_threads.remove(&id);
            self.changed();
        }
    }

    pub fn unarchive_thread(&mut self, id: ThreadId) {
        if let Some(thread) = self.threads.iter_mut().find(|thread| thread.id == id)
            && thread.archived_at.is_some()
        {
            thread.archived_at = None;
            self.changed();
        }
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

    /// Where the thread's agent works: its workspace, or its project's folder.
    pub fn thread_folder(&self, id: ThreadId) -> Option<PathBuf> {
        let thread = self.thread(id)?;
        match &thread.workspace {
            Some(path) => Some(path.clone()),
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

    pub fn set_started_in(&mut self, id: ThreadId, folder: Option<PathBuf>) {
        if let Some(thread) = self.threads.iter_mut().find(|thread| thread.id == id)
            && thread.started_in != folder
        {
            thread.started_in = folder;
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

    /// Keeps what's typed in the thread's composer; blank text is none.
    pub fn set_unsent_text(&mut self, id: ThreadId, text: Option<String>) {
        let text = text.filter(|text| !text.trim().is_empty());
        if let Some(thread) = self.threads.iter_mut().find(|thread| thread.id == id)
            && thread.unsent_text != text
        {
            thread.unsent_text = text;
            self.changed();
        }
    }

    pub fn set_continued_from(&mut self, id: ThreadId, from: ThreadId) {
        if let Some(thread) = self.threads.iter_mut().find(|thread| thread.id == id) {
            thread.continued_from = Some(from);
            self.changed();
        }
    }

    /// The threads that continue this one with other agents, oldest first.
    pub fn continuations(&self, id: ThreadId) -> impl Iterator<Item = &Thread> {
        self.threads
            .iter()
            .filter(move |thread| thread.continued_from == Some(id))
    }

    pub fn set_thread_session(&mut self, id: ThreadId, session_id: String) {
        if let Some(thread) = self.threads.iter_mut().find(|thread| thread.id == id)
            && thread.session_id.as_deref() != Some(session_id.as_str())
        {
            thread.session_id = Some(session_id);
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
    /// stopping completes the turn.
    pub fn set_thread_working(&mut self, id: ThreadId, working: bool) {
        let changed = if working {
            self.working_threads.insert(id)
        } else {
            self.working_threads.remove(&id)
        };
        if changed {
            if !working && let Some(thread) = self.threads.iter_mut().find(|thread| thread.id == id)
            {
                thread.completed_at = Some(SystemTime::now());
            }
            self.record_thread_activity(id);
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
        this.terminal_agents = snapshot.terminal_agents.into_iter().collect();
        this.terminal_commands = snapshot.terminal_commands.into_iter().collect();
        this.terminal_folders = snapshot.terminal_folders.into_iter().collect();
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
    let temporary_path = path.with_extension("json.tmp");
    std::fs::write(&temporary_path, json)
        .with_context(|| format!("writing {}", temporary_path.display()))?;
    std::fs::rename(&temporary_path, path)
        .with_context(|| format!("replacing {}", path.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adding_removing_and_scoping_projects() {
        let dir = tempfile::tempdir().expect("temp dir");
        let state_path = dir.path().join("state.json");
        let first_folder = dir.path().join("first");
        let second_folder = dir.path().join("second");
        std::fs::create_dir_all(&first_folder).expect("create first");
        std::fs::create_dir_all(&second_folder).expect("create second");

        let mut store = ProjectStore::load(Some(state_path.clone()));
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
        let mut store = ProjectStore::load(Some(state_path));
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
    fn subthreads_belong_to_their_parent() {
        let dir = tempfile::tempdir().expect("temp dir");
        let mut store = ProjectStore::load(None);
        let project = store.add_project(dir.path().to_path_buf());
        let parent = store.add_thread(project, "Parent", None).expect("thread");
        let task = |parent| Task {
            parent,
            prompt: "Look into it".into(),
            role: None,
            client_request_id: None,
            outcome: None,
            delivered: false,
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

    #[test]
    fn threads_await_input_until_answered_or_deleted() {
        let dir = tempfile::tempdir().expect("temp dir");
        let mut store = ProjectStore::load(None);
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
    fn saves_after_changes_pause() {
        let dir = tempfile::tempdir().expect("temp dir");
        let state_path = dir.path().join("state.json");
        let mut store = ProjectStore::load(Some(state_path.clone()));
        store.add_project(dir.path().to_path_buf());
        assert!(!state_path.exists(), "saving waits for changes to pause");
        std::thread::sleep(SAVE_DEBOUNCE * 3);
        let saved: PersistedState = read_state(&state_path).expect("readable").expect("saved");
        assert_eq!(saved.projects.len(), 1);
    }

    #[test]
    fn threads_work_in_workspaces() {
        let dir = tempfile::tempdir().expect("temp dir");
        let mut store = ProjectStore::load(None);
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
        let mut store = ProjectStore::load(Some(state_path.clone()));
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
        let mut store = ProjectStore::load(Some(state_path));
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
    fn imported_sessions_become_threads_in_their_folder() {
        let dir = tempfile::tempdir().expect("temp dir");
        let mut store = ProjectStore::load(None);
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
            store.thread_for_session("codex", "session-a"),
            Some(imported)
        );
        assert_eq!(store.thread_for_session("claude", "session-a"), None);

        let archived = store
            .add_imported_thread(ImportedSession {
                project_id: project,
                workspace: None,
                agent_id: "codex".into(),
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
    }
}
