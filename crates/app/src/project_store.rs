//! The app's copy of the server's projects and threads. Reads go to the copy; changes go to the
//! server, and come back as a new snapshot.

use std::collections::BTreeMap;
use std::ops::Deref;
use std::path::PathBuf;
use std::time::SystemTime;

use agentz_protocol::agents::AgentId;
use agentz_protocol::workspace::{ProjectGit, WorkspaceChoice, WorkspaceRemoval};
use agentz_protocol::{Request, Response};
use anyhow::{Context as _, Result, anyhow};
use gpui::{App, AppContext as _, Context, Entity, EventEmitter, Global, Task};
use projects::{
    ProjectIcon, ProjectId, ProjectScope, ProjectsSnapshot, ThreadCreator, ThreadId, ThreadOrder,
};
use util::ResultExt as _;

use crate::server_client::ServerClient;

/// What a thread needs from the user, as t3code's sidebar shows it. Ordered by priority, so a
/// project shows the most pressing status of its threads.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ThreadStatus {
    /// The agent finished a turn this client hasn't displayed yet.
    Completed,
    Working,
    /// A permission request is waiting.
    PendingApproval,
}

pub enum ProjectStoreEvent {
    /// The thread finished a turn or started waiting for a permission answer.
    NeedsAttention(ThreadId, ThreadStatus),
}

pub struct ProjectStore {
    store: projects::ProjectStore,
    has_snapshot: bool,
    /// The last completion this client displayed, per thread. Only this client's, so it's kept
    /// in the app's own file rather than on the server (herdr).
    viewed: BTreeMap<u64, SystemTime>,
    viewed_path: Option<PathBuf>,
    _save_viewed: Option<Task<()>>,
}

impl EventEmitter<ProjectStoreEvent> for ProjectStore {}

struct GlobalProjectStore(Entity<ProjectStore>);

impl Global for GlobalProjectStore {}

pub fn init(cx: &mut App) {
    let viewed_path = paths::data_dir().join("viewed.json");
    let viewed = read_viewed(&viewed_path).log_err().unwrap_or_default();
    let store = cx.new(|_| ProjectStore {
        store: projects::ProjectStore::from_snapshot(ProjectsSnapshot::default()),
        has_snapshot: false,
        viewed,
        viewed_path: Some(viewed_path),
        _save_viewed: None,
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
        let store = projects::ProjectStore::from_snapshot(snapshot);
        // The first snapshot only shows where things are; changes after it are news.
        if std::mem::replace(&mut self.has_snapshot, true) {
            for thread in store.threads() {
                let became_blocked =
                    store.is_thread_blocked(thread.id) && !self.store.is_thread_blocked(thread.id);
                // A subthread's requests are answered in its top-level thread, and its
                // completion is the parent's business.
                if thread.task.is_some() {
                    if became_blocked {
                        cx.emit(ProjectStoreEvent::NeedsAttention(
                            store.root_thread(thread.id),
                            ThreadStatus::PendingApproval,
                        ));
                    }
                    continue;
                }
                let completed = !store.is_thread_working(thread.id)
                    && thread.completed_at.is_some()
                    && thread.completed_at
                        != self
                            .store
                            .thread(thread.id)
                            .and_then(|thread| thread.completed_at);
                if became_blocked {
                    cx.emit(ProjectStoreEvent::NeedsAttention(
                        thread.id,
                        ThreadStatus::PendingApproval,
                    ));
                } else if completed {
                    cx.emit(ProjectStoreEvent::NeedsAttention(
                        thread.id,
                        ThreadStatus::Completed,
                    ));
                }
            }
        }
        self.store = store;
        cx.notify();
    }

    /// A thread's status counts its subthreads': it's where their requests are answered, and
    /// it works while they do.
    pub fn thread_status(&self, id: ThreadId) -> Option<ThreadStatus> {
        if self.store.is_thread_or_subthread_blocked(id) {
            return Some(ThreadStatus::PendingApproval);
        }
        if self
            .store
            .thread_and_subthreads(id)
            .into_iter()
            .any(|id| self.store.is_thread_working(id))
        {
            return Some(ThreadStatus::Working);
        }
        let completed_at = self.store.thread(id)?.completed_at?;
        let is_unseen = self
            .viewed
            .get(&id.0)
            .is_none_or(|viewed| *viewed < completed_at);
        is_unseen.then_some(ThreadStatus::Completed)
    }

    /// Who an agent-started thread or agent-sent message came from, to follow "Started by" or
    /// "Sent by".
    pub fn describe_creator(&self, creator: ThreadCreator) -> String {
        match creator {
            ThreadCreator::Thread(id) => match self.store.thread(id) {
                Some(thread) => format!("the agent in “{}”", thread.title),
                None => "an agent in a deleted thread".to_string(),
            },
            ThreadCreator::Command => "the agentZ CLI".to_string(),
        }
    }

    /// The most pressing status among the project's threads that aren't archived.
    pub fn project_status(&self, id: ProjectId) -> Option<ThreadStatus> {
        self.store
            .threads()
            .iter()
            .filter(|thread| {
                thread.project_id == id && thread.archived_at.is_none() && thread.task.is_none()
            })
            .filter_map(|thread| self.thread_status(thread.id))
            .max()
    }

    /// Records that this client displayed the thread's latest completion.
    pub fn mark_viewed(&mut self, id: ThreadId, cx: &mut Context<Self>) {
        let Some(completed_at) = self.store.thread(id).and_then(|thread| thread.completed_at)
        else {
            return;
        };
        if self.viewed.get(&id.0) == Some(&completed_at) {
            return;
        }
        self.viewed.insert(id.0, completed_at);
        let threads = self.store.threads();
        self.viewed
            .retain(|id, _| threads.iter().any(|thread| thread.id.0 == *id));
        if let Some(path) = self.viewed_path.clone() {
            let viewed = self.viewed.clone();
            self._save_viewed = Some(cx.background_spawn(async move {
                write_viewed(&path, &viewed).log_err();
            }));
        }
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

    /// Resolves once the thread is in this copy, after any new workspace is made. The server
    /// starts its agent right away.
    pub fn create_thread(
        &mut self,
        project_id: ProjectId,
        agent_id: AgentId,
        workspace: WorkspaceChoice,
        cx: &mut Context<Self>,
    ) -> Task<Result<ThreadId>> {
        self.request(
            Request::CreateThread {
                project_id,
                agent_id,
                workspace,
            },
            |response| match response {
                Response::ThreadCreated(thread_id) => Some(thread_id),
                _ => None,
            },
            cx,
        )
    }

    /// The project's branches and whether pastures can be made, for New Thread.
    pub fn project_git(&self, project_id: ProjectId, cx: &App) -> Task<Result<ProjectGit>> {
        self.request(
            Request::ProjectGit(project_id),
            |response| match response {
                Response::ProjectGit(git) => Some(git),
                _ => None,
            },
            cx,
        )
    }

    /// Removes a worktree or pasture; without `force`, work it would lose is reported first.
    pub fn remove_workspace(
        &self,
        project_id: ProjectId,
        path: PathBuf,
        force: bool,
        cx: &App,
    ) -> Task<Result<WorkspaceRemoval>> {
        self.request(
            Request::RemoveWorkspace {
                project_id,
                path,
                force,
            },
            |response| match response {
                Response::WorkspaceRemoval(removal) => Some(removal),
                _ => None,
            },
            cx,
        )
    }

    /// Rebases a pasture onto the branch it started from, and says how it went.
    pub fn sync_workspace(
        &self,
        project_id: ProjectId,
        path: PathBuf,
        cx: &App,
    ) -> Task<Result<String>> {
        self.request(
            Request::SyncWorkspace {
                project_id,
                path,
                branch: None,
                merge: false,
            },
            message_response,
            cx,
        )
    }

    /// Creates the pasture's branch in the project's checkout, and says how it went.
    pub fn bring_back_workspace(
        &self,
        project_id: ProjectId,
        path: PathBuf,
        cx: &App,
    ) -> Task<Result<String>> {
        self.request(
            Request::BringBackWorkspace {
                project_id,
                path,
                branch: None,
            },
            message_response,
            cx,
        )
    }

    fn request<T: Send + 'static>(
        &self,
        request: Request,
        answer: fn(Response) -> Option<T>,
        cx: &App,
    ) -> Task<Result<T>> {
        let response = ServerClient::global(cx).read(cx).request(request);
        cx.background_spawn(async move {
            let response = response.await?;
            let description = format!("{response:?}");
            answer(response).ok_or_else(|| anyhow!("unexpected response: {description}"))
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

fn message_response(response: Response) -> Option<String> {
    match response {
        Response::Message(message) => Some(message),
        _ => None,
    }
}

fn read_viewed(path: &std::path::Path) -> Result<BTreeMap<u64, SystemTime>> {
    match std::fs::read(path) {
        Ok(bytes) => {
            serde_json::from_slice(&bytes).with_context(|| format!("parsing {}", path.display()))
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(BTreeMap::new()),
        Err(error) => Err(error).with_context(|| format!("reading {}", path.display())),
    }
}

fn write_viewed(path: &std::path::Path, viewed: &BTreeMap<u64, SystemTime>) -> Result<()> {
    let json = serde_json::to_vec(viewed)?;
    let temporary = path.with_extension("json.tmp");
    std::fs::write(&temporary, json).with_context(|| format!("writing {}", temporary.display()))?;
    std::fs::rename(&temporary, path).with_context(|| format!("replacing {}", path.display()))
}
