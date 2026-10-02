//! The set of project folders the user has opened, their threads, and which of them the
//! window is currently showing ("All projects" or a single project).

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use anyhow::{Context as _, Result};
use collections::HashSet;
use gpui::{App, AppContext as _, Context, Entity, Global, SharedString, Task};
use serde::{Deserialize, Serialize};
use util::ResultExt as _;

const SAVE_DEBOUNCE: Duration = Duration::from_millis(200);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ProjectId(pub u64);

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

/// A project icon picked by the user, as in t3code's project settings.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ProjectIcon {
    /// Up to two letters on a tile of the named color.
    Monogram { text: String, color: String },
    /// An image file anywhere on disk.
    Image { path: PathBuf },
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
    /// The agent's ACP session, so the conversation can be restored after a restart.
    #[serde(default)]
    pub session_id: Option<String>,
    /// Set when the thread is archived; archived threads only show in Thread History.
    #[serde(default)]
    pub archived_at: Option<SystemTime>,
    /// The user renamed the thread, so automatic titles no longer replace it.
    #[serde(default)]
    pub has_custom_title: bool,
    /// The model last selected in the thread, as the agent names it.
    #[serde(default)]
    pub model: Option<String>,
}

/// How threads (and, in "All projects", the projects themselves) are ordered.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ThreadOrder {
    /// Most recent activity first.
    #[default]
    LastActivity,
    /// Newest thread first.
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

#[derive(Default, Serialize, Deserialize)]
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
}

pub struct ProjectStore {
    next_id: u64,
    projects: Vec<Project>,
    threads: Vec<Thread>,
    scope: ProjectScope,
    thread_order: ThreadOrder,
    /// Whether the sidebar's Archived shelf is open.
    archived_expanded: bool,
    /// Threads whose agent is currently running. Not persisted: nothing is running after a
    /// restart.
    working_threads: HashSet<ThreadId>,
    state_path: Option<PathBuf>,
    _save_task: Option<Task<()>>,
}

struct GlobalProjectStore(Entity<ProjectStore>);

impl Global for GlobalProjectStore {}

pub fn init(cx: &mut App) {
    let store = cx.new(|_| ProjectStore::load(Some(paths::state_file())));
    cx.set_global(GlobalProjectStore(store));
}

impl ProjectStore {
    pub fn global(cx: &App) -> Entity<Self> {
        cx.global::<GlobalProjectStore>().0.clone()
    }

    fn load(state_path: Option<PathBuf>) -> Self {
        let state = state_path
            .as_deref()
            .and_then(|path| read_state(path).log_err())
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
            working_threads: HashSet::default(),
            state_path,
            _save_task: None,
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
        this.threads
            .retain(|thread| this.projects.iter().any(|p| p.id == thread.project_id));
        if let ProjectScope::Project(id) = this.scope
            && this.project(id).is_none()
        {
            this.scope = ProjectScope::All;
        }
        this
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
            .filter(|thread| thread.project_id == project_id && thread.archived_at.is_none())
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

    pub fn toggle_archived_expanded(&mut self, cx: &mut Context<Self>) {
        self.archived_expanded = !self.archived_expanded;
        self.changed(cx);
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

    pub fn set_thread_order(&mut self, order: ThreadOrder, cx: &mut Context<Self>) {
        if self.thread_order != order {
            self.thread_order = order;
            self.changed(cx);
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

    pub fn set_scope(&mut self, scope: ProjectScope, cx: &mut Context<Self>) {
        if let ProjectScope::Project(id) = scope
            && self.project(id).is_none()
        {
            return;
        }
        if self.scope != scope {
            self.scope = scope;
            self.changed(cx);
        }
    }

    /// Adds the folder at `path` (or finds it if it was already added) and returns its id.
    pub fn add_project(&mut self, path: PathBuf, cx: &mut Context<Self>) -> ProjectId {
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
        });
        self.changed(cx);
        id
    }

    /// Removes the project from the list. Nothing on disk is touched.
    /// Sets the project's display name; an empty name goes back to the folder name.
    pub fn set_project_name(&mut self, id: ProjectId, name: &str, cx: &mut Context<Self>) {
        let name = name.trim();
        let Some(project) = self.projects.iter_mut().find(|project| project.id == id) else {
            return;
        };
        let custom_name =
            (!name.is_empty() && *name != *project.folder_name()).then(|| name.to_string());
        if project.custom_name != custom_name {
            project.custom_name = custom_name;
            self.changed(cx);
        }
    }

    pub fn set_project_icon(
        &mut self,
        id: ProjectId,
        icon: Option<ProjectIcon>,
        cx: &mut Context<Self>,
    ) {
        if let Some(project) = self.projects.iter_mut().find(|project| project.id == id)
            && project.icon != icon
        {
            project.icon = icon;
            self.changed(cx);
        }
    }

    pub fn remove_project(&mut self, id: ProjectId, cx: &mut Context<Self>) {
        let count_before = self.projects.len();
        self.projects.retain(|project| project.id != id);
        if self.projects.len() == count_before {
            return;
        }
        self.threads.retain(|thread| thread.project_id != id);
        let threads = &self.threads;
        self.working_threads
            .retain(|thread_id| threads.iter().any(|thread| thread.id == *thread_id));
        if self.scope == ProjectScope::Project(id) {
            self.scope = ProjectScope::All;
        }
        self.changed(cx);
    }

    pub fn add_thread(
        &mut self,
        project_id: ProjectId,
        title: impl Into<String>,
        agent_id: Option<String>,
        cx: &mut Context<Self>,
    ) -> Option<ThreadId> {
        self.project(project_id)?;
        let id = ThreadId(self.allocate_id());
        self.threads.push(Thread {
            id,
            project_id,
            title: title.into(),
            agent_id,
            last_activity_at: Some(SystemTime::now()),
            session_id: None,
            archived_at: None,
            has_custom_title: false,
            model: None,
        });
        self.changed(cx);
        Some(id)
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
            .filter(|thread| thread.archived_at.is_some())
            .filter(|thread| match self.scope {
                ProjectScope::All => true,
                ProjectScope::Project(id) => thread.project_id == id,
            })
            .collect();
        threads.sort_by_key(|thread| std::cmp::Reverse((thread.archived_at, thread.id)));
        threads
    }

    pub fn archive_thread(&mut self, id: ThreadId, cx: &mut Context<Self>) {
        if let Some(thread) = self.threads.iter_mut().find(|thread| thread.id == id)
            && thread.archived_at.is_none()
        {
            thread.archived_at = Some(SystemTime::now());
            self.working_threads.remove(&id);
            self.changed(cx);
        }
    }

    pub fn unarchive_thread(&mut self, id: ThreadId, cx: &mut Context<Self>) {
        if let Some(thread) = self.threads.iter_mut().find(|thread| thread.id == id)
            && thread.archived_at.is_some()
        {
            thread.archived_at = None;
            self.changed(cx);
        }
    }

    /// Removes the thread for good.
    pub fn delete_thread(&mut self, id: ThreadId, cx: &mut Context<Self>) {
        let count_before = self.threads.len();
        self.threads.retain(|thread| thread.id != id);
        if self.threads.len() != count_before {
            self.working_threads.remove(&id);
            self.changed(cx);
        }
    }

    pub fn set_thread_session(&mut self, id: ThreadId, session_id: String, cx: &mut Context<Self>) {
        if let Some(thread) = self.threads.iter_mut().find(|thread| thread.id == id)
            && thread.session_id.as_deref() != Some(session_id.as_str())
        {
            thread.session_id = Some(session_id);
            self.changed(cx);
        }
    }

    pub fn set_thread_model(&mut self, id: ThreadId, model: String, cx: &mut Context<Self>) {
        if let Some(thread) = self.threads.iter_mut().find(|thread| thread.id == id)
            && thread.model.as_deref() != Some(model.as_str())
        {
            thread.model = Some(model);
            self.changed(cx);
        }
    }

    /// Sets an automatic title (from the first prompt or the agent), unless the user renamed
    /// the thread.
    pub fn rename_thread(&mut self, id: ThreadId, title: String, cx: &mut Context<Self>) {
        if let Some(thread) = self.threads.iter_mut().find(|thread| thread.id == id)
            && !thread.has_custom_title
            && thread.title != title
        {
            thread.title = title;
            self.changed(cx);
        }
    }

    /// Sets a title chosen by the user.
    pub fn set_custom_title(&mut self, id: ThreadId, title: String, cx: &mut Context<Self>) {
        if let Some(thread) = self.threads.iter_mut().find(|thread| thread.id == id) {
            thread.title = title;
            thread.has_custom_title = true;
            self.changed(cx);
        }
    }

    pub fn is_thread_working(&self, id: ThreadId) -> bool {
        self.working_threads.contains(&id)
    }

    /// Marks whether the thread's agent is running; either change counts as activity.
    pub fn set_thread_working(&mut self, id: ThreadId, working: bool, cx: &mut Context<Self>) {
        let changed = if working {
            self.working_threads.insert(id)
        } else {
            self.working_threads.remove(&id)
        };
        if changed {
            self.record_thread_activity(id, cx);
        }
    }

    pub fn record_thread_activity(&mut self, id: ThreadId, cx: &mut Context<Self>) {
        if let Some(thread) = self.threads.iter_mut().find(|thread| thread.id == id) {
            thread.last_activity_at = Some(SystemTime::now());
            self.changed(cx);
        }
    }

    fn allocate_id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    fn changed(&mut self, cx: &mut Context<Self>) {
        cx.notify();
        let Some(state_path) = self.state_path.clone() else {
            return;
        };
        let state = PersistedState {
            next_id: self.next_id,
            projects: self.projects.clone(),
            threads: self.threads.clone(),
            scope: self.scope,
            thread_order: self.thread_order,
            archived_expanded: self.archived_expanded,
        };
        let executor = cx.background_executor().clone();
        self._save_task = Some(cx.background_spawn(async move {
            executor.timer(SAVE_DEBOUNCE).await;
            write_state(&state_path, &state).log_err();
        }));
    }
}

fn project_name(path: &Path) -> SharedString {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string_lossy().into_owned())
        .into()
}

fn read_state(path: &Path) -> Result<Option<PersistedState>> {
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

fn write_state(path: &Path, state: &PersistedState) -> Result<()> {
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
    use gpui::TestAppContext;

    #[gpui::test]
    async fn adding_removing_and_scoping_projects(cx: &mut TestAppContext) {
        let dir = tempfile::tempdir().expect("temp dir");
        let state_path = dir.path().join("state.json");
        let first_folder = dir.path().join("first");
        let second_folder = dir.path().join("second");
        std::fs::create_dir_all(&first_folder).expect("create first");
        std::fs::create_dir_all(&second_folder).expect("create second");

        let store = cx.new(|_| ProjectStore::load(Some(state_path.clone())));
        let (first, second) = store.update(cx, |store, cx| {
            let first = store.add_project(first_folder.clone(), cx);
            let second = store.add_project(second_folder.clone(), cx);
            assert_eq!(store.add_project(first_folder.clone(), cx), first);
            store.add_thread(first, "Fix login bug", None, cx);
            store.set_scope(ProjectScope::Project(second), cx);
            (first, second)
        });

        store.read_with(cx, |store, _| {
            assert_eq!(store.projects().len(), 2);
            assert_eq!(store.thread_count(first), 1);
            let visible: Vec<_> = store.visible_projects().map(|p| p.id).collect();
            assert_eq!(visible, vec![second]);
        });

        cx.executor().advance_clock(SAVE_DEBOUNCE * 2);
        cx.run_until_parked();
        let reloaded = ProjectStore::load(Some(state_path.clone()));
        assert_eq!(reloaded.projects().len(), 2);
        assert_eq!(reloaded.scope(), ProjectScope::Project(second));
        assert_eq!(
            reloaded.project(first).map(|p| p.name()),
            Some("first".into())
        );

        store.update(cx, |store, cx| {
            let older = store.add_thread(first, "Older", None, cx).expect("thread");
            let newer = store.add_thread(first, "Newer", None, cx).expect("thread");
            store.set_thread_working(older, true, cx);
            let order: Vec<_> = store.threads_for(first).map(|thread| thread.id).collect();
            assert_eq!(order[0], older, "most recent activity first");
            store.set_thread_order(ThreadOrder::Created, cx);
            let order: Vec<_> = store.threads_for(first).map(|thread| thread.id).collect();
            assert_eq!(order[0], newer, "newest thread first");
            store.set_thread_session(newer, "session-7".into(), cx);
            assert_eq!(
                store
                    .thread(newer)
                    .and_then(|thread| thread.session_id.clone()),
                Some("session-7".into())
            );
        });

        store.update(cx, |store, cx| {
            store.set_scope(ProjectScope::All, cx);
            let thread = store
                .add_thread(first, "To archive", None, cx)
                .expect("thread");
            store.archive_thread(thread, cx);
            assert!(store.threads_for(first).all(|t| t.id != thread));
            assert!(store.archived_threads().iter().any(|t| t.id == thread));
            store.unarchive_thread(thread, cx);
            assert!(store.threads_for(first).any(|t| t.id == thread));
            store.set_custom_title(thread, "Mine".into(), cx);
            store.rename_thread(thread, "Automatic".into(), cx);
            assert_eq!(store.thread(thread).map(|t| t.title.as_str()), Some("Mine"));
            store.delete_thread(thread, cx);
            assert!(store.thread(thread).is_none());
        });

        store.update(cx, |store, cx| {
            store.set_project_name(first, "  Renamed ", cx);
            assert_eq!(
                store.project(first).map(|p| p.name()),
                Some("Renamed".into())
            );
            store.set_project_name(first, "first", cx);
            assert_eq!(
                store.project(first).and_then(|p| p.custom_name.clone()),
                None
            );
            let icon = ProjectIcon::Monogram {
                text: "FI".into(),
                color: "teal".into(),
            };
            store.set_project_icon(first, Some(icon.clone()), cx);
            assert_eq!(
                store.project(first).and_then(|p| p.icon.clone()),
                Some(icon)
            );
        });

        store.update(cx, |store, cx| store.remove_project(second, cx));
        store.read_with(cx, |store, _| {
            assert_eq!(store.scope(), ProjectScope::All);
            assert_eq!(store.projects().len(), 1);
        });
    }
}
