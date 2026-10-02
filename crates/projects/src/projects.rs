//! The set of project folders the user has opened, their threads, and which of them the
//! window is currently showing ("All projects" or a single project).

use std::path::{Path, PathBuf};
use std::time::Duration;

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
}

impl Project {
    pub fn name(&self) -> SharedString {
        project_name(&self.path)
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
}

/// Which projects the sidebar shows threads for.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "project")]
pub enum ProjectScope {
    #[default]
    All,
    Project(ProjectId),
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
    collapsed: Vec<ProjectId>,
}

pub struct ProjectStore {
    next_id: u64,
    projects: Vec<Project>,
    threads: Vec<Thread>,
    scope: ProjectScope,
    collapsed: HashSet<ProjectId>,
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
            collapsed: state.collapsed.into_iter().collect(),
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

    pub fn threads_for(&self, project_id: ProjectId) -> impl Iterator<Item = &Thread> {
        self.threads
            .iter()
            .filter(move |thread| thread.project_id == project_id)
    }

    pub fn thread_count(&self, project_id: ProjectId) -> usize {
        self.threads_for(project_id).count()
    }

    pub fn scope(&self) -> ProjectScope {
        self.scope
    }

    /// The projects the current scope shows, in the order they were added.
    pub fn visible_projects(&self) -> impl Iterator<Item = &Project> {
        let scope = self.scope;
        self.projects.iter().filter(move |project| match scope {
            ProjectScope::All => true,
            ProjectScope::Project(id) => project.id == id,
        })
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

    pub fn is_collapsed(&self, id: ProjectId) -> bool {
        self.collapsed.contains(&id)
    }

    pub fn toggle_collapsed(&mut self, id: ProjectId, cx: &mut Context<Self>) {
        if !self.collapsed.remove(&id) {
            self.collapsed.insert(id);
        }
        self.changed(cx);
    }

    /// Adds the folder at `path` (or finds it if it was already added) and returns its id.
    pub fn add_project(&mut self, path: PathBuf, cx: &mut Context<Self>) -> ProjectId {
        let path = std::fs::canonicalize(&path).unwrap_or(path);
        if let Some(existing) = self.projects.iter().find(|project| project.path == path) {
            let id = existing.id;
            if self.collapsed.remove(&id) {
                self.changed(cx);
            }
            return id;
        }
        let id = ProjectId(self.allocate_id());
        self.projects.push(Project { id, path });
        self.changed(cx);
        id
    }

    /// Removes the project from the list. Nothing on disk is touched.
    pub fn remove_project(&mut self, id: ProjectId, cx: &mut Context<Self>) {
        let count_before = self.projects.len();
        self.projects.retain(|project| project.id != id);
        if self.projects.len() == count_before {
            return;
        }
        self.threads.retain(|thread| thread.project_id != id);
        self.collapsed.remove(&id);
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
        });
        self.changed(cx);
        Some(id)
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
            collapsed: {
                let mut collapsed: Vec<_> = self.collapsed.iter().copied().collect();
                collapsed.sort();
                collapsed
            },
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

        store.update(cx, |store, cx| store.remove_project(second, cx));
        store.read_with(cx, |store, _| {
            assert_eq!(store.scope(), ProjectScope::All);
            assert_eq!(store.projects().len(), 1);
        });
    }
}
