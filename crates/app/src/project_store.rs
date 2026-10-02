//! The app's handle on [`projects::ProjectStore`]: a GPUI entity that reads through to the store
//! and notifies observers when a call changes it.

use std::ops::Deref;
use std::path::PathBuf;

use gpui::{App, AppContext as _, Context, Entity, Global};
use projects::{ProjectIcon, ProjectId, ProjectScope, ThreadId, ThreadOrder};

pub struct ProjectStore {
    store: projects::ProjectStore,
}

struct GlobalProjectStore(Entity<ProjectStore>);

impl Global for GlobalProjectStore {}

pub fn init(cx: &mut App) {
    let store =
        cx.new(|_| ProjectStore::new(projects::ProjectStore::load(Some(paths::state_file()))));
    cx.set_global(GlobalProjectStore(store));
}

impl Deref for ProjectStore {
    type Target = projects::ProjectStore;

    fn deref(&self) -> &Self::Target {
        &self.store
    }
}

impl ProjectStore {
    pub fn new(store: projects::ProjectStore) -> Self {
        Self { store }
    }

    pub fn global(cx: &App) -> Entity<Self> {
        cx.global::<GlobalProjectStore>().0.clone()
    }

    fn change<R>(
        &mut self,
        cx: &mut Context<Self>,
        change: impl FnOnce(&mut projects::ProjectStore) -> R,
    ) -> R {
        let revision = self.store.revision();
        let result = change(&mut self.store);
        if self.store.revision() != revision {
            cx.notify();
        }
        result
    }

    pub fn toggle_archived_expanded(&mut self, cx: &mut Context<Self>) {
        self.change(cx, |store| store.toggle_archived_expanded())
    }

    pub fn set_thread_order(&mut self, order: ThreadOrder, cx: &mut Context<Self>) {
        self.change(cx, |store| store.set_thread_order(order))
    }

    pub fn set_scope(&mut self, scope: ProjectScope, cx: &mut Context<Self>) {
        self.change(cx, |store| store.set_scope(scope))
    }

    pub fn add_project(&mut self, path: PathBuf, cx: &mut Context<Self>) -> ProjectId {
        self.change(cx, |store| store.add_project(path))
    }

    pub fn set_project_name(&mut self, id: ProjectId, name: &str, cx: &mut Context<Self>) {
        self.change(cx, |store| store.set_project_name(id, name))
    }

    pub fn set_project_icon(
        &mut self,
        id: ProjectId,
        icon: Option<ProjectIcon>,
        cx: &mut Context<Self>,
    ) {
        self.change(cx, |store| store.set_project_icon(id, icon))
    }

    pub fn remove_project(&mut self, id: ProjectId, cx: &mut Context<Self>) {
        self.change(cx, |store| store.remove_project(id))
    }

    pub fn add_thread(
        &mut self,
        project_id: ProjectId,
        title: impl Into<String>,
        agent_id: Option<String>,
        cx: &mut Context<Self>,
    ) -> Option<ThreadId> {
        self.change(cx, |store| store.add_thread(project_id, title, agent_id))
    }

    pub fn archive_thread(&mut self, id: ThreadId, cx: &mut Context<Self>) {
        self.change(cx, |store| store.archive_thread(id))
    }

    pub fn unarchive_thread(&mut self, id: ThreadId, cx: &mut Context<Self>) {
        self.change(cx, |store| store.unarchive_thread(id))
    }

    pub fn delete_thread(&mut self, id: ThreadId, cx: &mut Context<Self>) {
        self.change(cx, |store| store.delete_thread(id))
    }

    pub fn set_thread_session(&mut self, id: ThreadId, session_id: String, cx: &mut Context<Self>) {
        self.change(cx, |store| store.set_thread_session(id, session_id))
    }

    pub fn set_thread_model(&mut self, id: ThreadId, model: String, cx: &mut Context<Self>) {
        self.change(cx, |store| store.set_thread_model(id, model))
    }

    pub fn rename_thread(&mut self, id: ThreadId, title: String, cx: &mut Context<Self>) {
        self.change(cx, |store| store.rename_thread(id, title))
    }

    pub fn set_custom_title(&mut self, id: ThreadId, title: String, cx: &mut Context<Self>) {
        self.change(cx, |store| store.set_custom_title(id, title))
    }

    pub fn set_thread_working(&mut self, id: ThreadId, working: bool, cx: &mut Context<Self>) {
        self.change(cx, |store| store.set_thread_working(id, working))
    }
}
