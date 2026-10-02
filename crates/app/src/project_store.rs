//! The app's copy of the server's projects and threads. Reads go to the copy; changes go to the
//! server, and come back as a new snapshot.

use std::ops::Deref;
use std::path::PathBuf;

use agentz_protocol::agents::AgentId;
use agentz_protocol::{Request, Response};
use anyhow::{Result, anyhow};
use gpui::{App, AppContext as _, Context, Entity, Global, Task};
use projects::{ProjectIcon, ProjectId, ProjectScope, ProjectsSnapshot, ThreadId, ThreadOrder};

use crate::server_client::ServerClient;

pub struct ProjectStore {
    store: projects::ProjectStore,
}

struct GlobalProjectStore(Entity<ProjectStore>);

impl Global for GlobalProjectStore {}

pub fn init(cx: &mut App) {
    let store = cx.new(|_| ProjectStore {
        store: projects::ProjectStore::from_snapshot(ProjectsSnapshot::default()),
    });
    cx.set_global(GlobalProjectStore(store));
}

impl Deref for ProjectStore {
    type Target = projects::ProjectStore;

    fn deref(&self) -> &Self::Target {
        &self.store
    }
}

impl ProjectStore {
    pub fn global(cx: &App) -> Entity<Self> {
        cx.global::<GlobalProjectStore>().0.clone()
    }

    pub(crate) fn set_snapshot(&mut self, snapshot: ProjectsSnapshot, cx: &mut Context<Self>) {
        self.store = projects::ProjectStore::from_snapshot(snapshot);
        cx.notify();
    }

    fn send(&self, request: Request, cx: &App) {
        ServerClient::global(cx).read(cx).send(request, cx);
    }

    pub fn toggle_archived_expanded(&mut self, cx: &mut Context<Self>) {
        self.send(Request::ToggleArchivedExpanded, cx)
    }

    pub fn set_thread_order(&mut self, order: ThreadOrder, cx: &mut Context<Self>) {
        self.send(Request::SetThreadOrder(order), cx)
    }

    pub fn set_scope(&mut self, scope: ProjectScope, cx: &mut Context<Self>) {
        self.send(Request::SetScope(scope), cx)
    }

    /// Resolves once the project is in this copy.
    pub fn add_project(
        &mut self,
        path: PathBuf,
        cx: &mut Context<Self>,
    ) -> Task<Result<ProjectId>> {
        let response = ServerClient::global(cx)
            .read(cx)
            .request(Request::AddProject { path });
        cx.background_spawn(async move {
            match response.await? {
                Response::ProjectAdded(project_id) => Ok(project_id),
                response => Err(anyhow!("unexpected response: {response:?}")),
            }
        })
    }

    pub fn set_project_name(&mut self, id: ProjectId, name: &str, cx: &mut Context<Self>) {
        self.send(
            Request::SetProjectName {
                project_id: id,
                name: name.to_string(),
            },
            cx,
        )
    }

    pub fn set_project_icon(
        &mut self,
        id: ProjectId,
        icon: Option<ProjectIcon>,
        cx: &mut Context<Self>,
    ) {
        self.send(
            Request::SetProjectIcon {
                project_id: id,
                icon,
            },
            cx,
        )
    }

    pub fn remove_project(&mut self, id: ProjectId, cx: &mut Context<Self>) {
        self.send(Request::RemoveProject(id), cx)
    }

    /// Resolves once the thread is in this copy. The server starts its agent right away.
    pub fn create_thread(
        &mut self,
        project_id: ProjectId,
        agent_id: AgentId,
        cx: &mut Context<Self>,
    ) -> Task<Result<ThreadId>> {
        let response = ServerClient::global(cx)
            .read(cx)
            .request(Request::CreateThread {
                project_id,
                agent_id,
            });
        cx.background_spawn(async move {
            match response.await? {
                Response::ThreadCreated(thread_id) => Ok(thread_id),
                response => Err(anyhow!("unexpected response: {response:?}")),
            }
        })
    }

    pub fn archive_thread(&mut self, id: ThreadId, cx: &mut Context<Self>) {
        self.send(Request::ArchiveThread(id), cx)
    }

    pub fn unarchive_thread(&mut self, id: ThreadId, cx: &mut Context<Self>) {
        self.send(Request::UnarchiveThread(id), cx)
    }

    pub fn delete_thread(&mut self, id: ThreadId, cx: &mut Context<Self>) {
        self.send(Request::DeleteThread(id), cx)
    }

    pub fn set_custom_title(&mut self, id: ThreadId, title: String, cx: &mut Context<Self>) {
        self.send(
            Request::RenameThread {
                thread_id: id,
                title,
            },
            cx,
        )
    }
}
