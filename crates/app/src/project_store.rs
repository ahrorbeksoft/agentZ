//! The app's copy of one machine's projects and threads. Reads go to the copy; changes go to
//! that machine's server, and come back as a new snapshot.

use std::collections::BTreeMap;
use std::ops::Deref;
use std::path::PathBuf;
use std::time::SystemTime;

use agentz_protocol::accounts::{AccountChoice, AccountId};
use agentz_protocol::agents::{AgentId, AgentSession, AgentSessions};
use agentz_protocol::terminal::TerminalCommand;
use agentz_protocol::workspace::{
    ProjectGit, RepositoryCheckouts, WorkspaceChoice, WorkspaceRemoval,
};
use agentz_protocol::{Request, Response};
use anyhow::{Context as _, Result, anyhow};
use futures::FutureExt as _;
use futures::future::BoxFuture;
use gpui::{App, AppContext as _, Context, EventEmitter, Task, WeakEntity};
use projects::{
    ProjectIcon, ProjectId, ProjectsSnapshot, ThreadCreator, ThreadId, ThreadOrder, ThreadSection,
    UnsentMention, WorkspaceKind,
};
use util::ResultExt as _;

use crate::machines::MachineId;
use crate::server_client::ServerClient;

/// What a thread needs from the user, as t3code's sidebar shows it. Ordered by priority, so a
/// project shows the most pressing status of its threads.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ThreadStatus {
    /// The agent finished a turn this client hasn't displayed yet.
    Completed,
    /// The agent's turn ended with its work still going: a background task.
    Waiting,
    Working,
    /// The agent asked for input (ACP's elicitation) that the user hasn't given yet.
    AwaitingInput,
    /// A permission request is waiting.
    PendingApproval,
}

pub enum ProjectStoreEvent {
    /// The thread finished a turn, or started waiting for a permission answer or input.
    NeedsAttention(ThreadId, ThreadStatus),
    /// The user archived the thread from this app, rather than an agent or another app.
    Archiving(ThreadId),
}

pub struct ProjectStore {
    machine: MachineId,
    client: WeakEntity<ServerClient>,
    store: projects::ProjectStore,
    has_snapshot: bool,
    /// The last completion this client displayed, per thread. Only this client's, so it's kept
    /// in the app's own file rather than on the server (herdr).
    viewed: BTreeMap<u64, SystemTime>,
    viewed_path: Option<PathBuf>,
    _save_viewed: Option<Task<()>>,
}

impl EventEmitter<ProjectStoreEvent> for ProjectStore {}

impl Deref for ProjectStore {
    type Target = projects::ProjectStore;

    fn deref(&self) -> &Self::Target {
        &self.store
    }
}

impl ProjectStore {
    pub(crate) fn new(machine: MachineId, client: WeakEntity<ServerClient>) -> Self {
        let viewed_path = match machine {
            MachineId::Local => paths::data_dir().join("viewed.json"),
            MachineId::Remote(_) => paths::data_dir()
                .join("machines")
                .join(format!("viewed-{}.json", machine.slug())),
        };
        let viewed = read_viewed(&viewed_path).log_err().unwrap_or_default();
        Self {
            machine,
            client,
            store: projects::ProjectStore::from_snapshot(ProjectsSnapshot::default()),
            has_snapshot: false,
            viewed,
            viewed_path: Some(viewed_path),
            _save_viewed: None,
        }
    }

    pub fn machine(&self) -> MachineId {
        self.machine
    }

    /// Whether the server's projects and threads have arrived, so an empty list means none.
    pub fn has_snapshot(&self) -> bool {
        self.has_snapshot
    }

    pub(crate) fn set_snapshot(&mut self, snapshot: ProjectsSnapshot, cx: &mut Context<Self>) {
        let store = projects::ProjectStore::from_snapshot(snapshot);
        // The first snapshot only shows where things are; changes after it are news.
        if std::mem::replace(&mut self.has_snapshot, true) {
            for thread in store.threads() {
                let became_blocked =
                    store.is_thread_blocked(thread.id) && !self.store.is_thread_blocked(thread.id);
                let became_awaiting_input = store.is_thread_awaiting_input(thread.id)
                    && !self.store.is_thread_awaiting_input(thread.id);
                // A subthread is its parent's business: its requests show on its top-level
                // thread without a sound, and its end reaches the parent, which completes
                // after its last subthread.
                if thread.task.is_some() {
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
                } else if became_awaiting_input {
                    cx.emit(ProjectStoreEvent::NeedsAttention(
                        thread.id,
                        ThreadStatus::AwaitingInput,
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
        if self.store.is_thread_awaiting_input(id) {
            return Some(ThreadStatus::AwaitingInput);
        }
        if self
            .store
            .thread_and_subthreads(id)
            .into_iter()
            .any(|id| self.store.is_thread_working(id))
        {
            return Some(ThreadStatus::Working);
        }
        if self.store.is_thread_waiting(id) {
            return Some(ThreadStatus::Waiting);
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
        if let Some(client) = self.client.upgrade() {
            client.read(cx).send(request, cx);
        }
    }

    fn server_request(&self, request: Request, cx: &App) -> BoxFuture<'static, Result<Response>> {
        match self.client.upgrade() {
            Some(client) => client.read(cx).request(request),
            None => futures::future::ready(Err(anyhow!("the machine was removed"))).boxed(),
        }
    }

    pub fn toggle_archived_expanded(&mut self, cx: &mut Context<Self>) {
        self.send(Request::ToggleArchivedExpanded, cx)
    }

    pub fn toggle_workspaces_expanded(&mut self, cx: &mut Context<Self>) {
        self.send(Request::ToggleWorkspacesExpanded, cx)
    }

    /// Makes a Workspaces thread one of the project its folder is in, adding the folder as a
    /// project when it's in none.
    pub fn move_to_agents(&mut self, id: ThreadId, cx: &mut Context<Self>) -> Task<Result<()>> {
        self.request(
            Request::MoveToAgents(id),
            |response| matches!(response, Response::Ok).then_some(()),
            cx,
        )
    }

    /// The limit notice's Continue at <reset>, or with `on: false` its cancel.
    pub fn continue_at_reset(
        &mut self,
        id: ThreadId,
        on: bool,
        cx: &mut Context<Self>,
    ) -> Task<Result<()>> {
        self.request(
            Request::ContinueAtReset { thread_id: id, on },
            |response| matches!(response, Response::Ok).then_some(()),
            cx,
        )
    }

    pub fn set_thread_order(&mut self, order: ThreadOrder, cx: &mut Context<Self>) {
        self.send(Request::SetThreadOrder(order), cx)
    }

    /// Resolves once the project is in this copy.
    pub fn add_project(
        &mut self,
        path: PathBuf,
        cx: &mut Context<Self>,
    ) -> Task<Result<ProjectId>> {
        let response = self.server_request(Request::AddProject { path }, cx);
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
        account: AccountChoice,
        cx: &mut Context<Self>,
    ) -> Task<Result<ThreadId>> {
        self.request(
            Request::CreateThread {
                project_id,
                agent_id,
                workspace,
                account,
            },
            |response| match response {
                Response::ThreadCreated(thread_id) => Some(thread_id),
                _ => None,
            },
            cx,
        )
    }

    /// A thread started in a workspace pane, working in `folder` or in a new worktree or
    /// pasture of its repository. Resolves once it's in this copy.
    pub fn create_workspaces_thread(
        &mut self,
        folder: PathBuf,
        agent_id: AgentId,
        workspace: WorkspaceChoice,
        account: AccountChoice,
        cx: &mut Context<Self>,
    ) -> Task<Result<ThreadId>> {
        self.request(
            Request::CreateWorkspacesThread {
                folder,
                agent_id,
                workspace,
                account,
            },
            |response| match response {
                Response::ThreadCreated(thread_id) => Some(thread_id),
                _ => None,
            },
            cx,
        )
    }

    /// "Continue with another agent": a thread with `agent_id` on `account` in the thread's
    /// workspace, whose first message brings the thread's conversation.
    pub fn continue_thread(
        &mut self,
        thread_id: ThreadId,
        agent_id: AgentId,
        account: AccountChoice,
        cx: &mut Context<Self>,
    ) -> Task<Result<ThreadId>> {
        self.request(
            Request::ContinueThread {
                thread_id,
                agent_id,
                account,
            },
            |response| match response {
                Response::ThreadCreated(thread_id) => Some(thread_id),
                _ => None,
            },
            cx,
        )
    }

    /// A thread that runs a terminal: a login shell, or `command` in one.
    pub fn create_terminal_thread(
        &mut self,
        project_id: ProjectId,
        command: TerminalCommand,
        workspace: WorkspaceChoice,
        cx: &mut Context<Self>,
    ) -> Task<Result<ThreadId>> {
        self.request(
            Request::CreateTerminalThread {
                project_id,
                command,
                workspace,
            },
            |response| match response {
                Response::ThreadCreated(thread_id) => Some(thread_id),
                _ => None,
            },
            cx,
        )
    }

    /// The conversations the agent keeps on this machine for the account (`None` being the
    /// External one), each with the project it would be imported into. The server starts the
    /// agent only to ask.
    pub fn list_agent_sessions(
        &self,
        agent_id: AgentId,
        account: Option<AccountId>,
        cx: &App,
    ) -> Task<Result<AgentSessions>> {
        self.request(
            Request::ListAgentSessions { agent_id, account },
            |response| match response {
                Response::AgentSessions(sessions) => Some(sessions),
                _ => None,
            },
            cx,
        )
    }

    /// Adds an archived thread on the account for each session that has none yet. Resolves
    /// once they're in this copy.
    pub fn import_agent_sessions(
        &self,
        agent_id: AgentId,
        account: Option<AccountId>,
        sessions: Vec<AgentSession>,
        cx: &App,
    ) -> Task<Result<Vec<ThreadId>>> {
        self.request(
            Request::ImportAgentSessions {
                agent_id,
                account,
                sessions,
                archived: true,
            },
            |response| match response {
                Response::ThreadsImported(threads) => Some(threads),
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

    /// The repository `folder` is in, a project or not, and its checkouts.
    pub fn repository_checkouts(
        &self,
        folder: PathBuf,
        cx: &App,
    ) -> Task<Result<RepositoryCheckouts>> {
        self.request(
            Request::RepositoryCheckouts(folder),
            |response| match response {
                Response::RepositoryCheckouts(checkouts) => Some(checkouts),
                _ => None,
            },
            cx,
        )
    }

    /// Makes a worktree or pasture of the repository `folder` is in, from what `folder` has
    /// checked out, with no thread in it. Resolves to its folder.
    pub fn create_workspace(
        &self,
        folder: PathBuf,
        kind: WorkspaceKind,
        base: Option<String>,
        branch: Option<String>,
        cx: &App,
    ) -> Task<Result<PathBuf>> {
        self.request(
            Request::CreateWorkspace {
                folder,
                kind,
                base,
                branch,
            },
            |response| match response {
                Response::WorkspaceCreated(folder) => Some(folder),
                _ => None,
            },
            cx,
        )
    }

    /// Removes a worktree or pasture; without `force`, work it would lose is reported first.
    pub fn remove_workspace(
        &self,
        path: PathBuf,
        force: bool,
        cx: &App,
    ) -> Task<Result<WorkspaceRemoval>> {
        self.request(
            Request::RemoveWorkspace { path, force },
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
        let response = self.server_request(request, cx);
        cx.background_spawn(async move {
            let response = response.await?;
            let description = format!("{response:?}");
            answer(response).ok_or_else(|| anyhow!("unexpected response: {description}"))
        })
    }

    // Archiving, pinning and arranging show at once, as the server will have them, so a
    // dropped card stays where it was let go until the server's snapshot replaces this copy.

    pub fn archive_thread(&mut self, id: ThreadId, cx: &mut Context<Self>) {
        self.store.archive_thread(id);
        cx.notify();
        self.send(Request::ArchiveThread(id), cx);
        cx.emit(ProjectStoreEvent::Archiving(id));
    }

    pub fn unarchive_thread(&mut self, id: ThreadId, cx: &mut Context<Self>) {
        self.store.unarchive_thread(id);
        cx.notify();
        self.send(Request::UnarchiveThread(id), cx)
    }

    pub fn pin_thread(&mut self, id: ThreadId, order_key: Option<String>, cx: &mut Context<Self>) {
        self.store.pin_thread(id, order_key.clone()).log_err();
        cx.notify();
        self.send(
            Request::PinThread {
                thread_id: id,
                order_key,
            },
            cx,
        )
    }

    pub fn unpin_thread(&mut self, id: ThreadId, cx: &mut Context<Self>) {
        self.store.unpin_thread(id);
        cx.notify();
        self.send(Request::UnpinThread(id), cx)
    }

    pub fn reorder_threads(
        &mut self,
        section: ThreadSection,
        keys: Vec<(ThreadId, String)>,
        cx: &mut Context<Self>,
    ) {
        if keys.is_empty() {
            return;
        }
        self.store.set_order_keys(section, keys.clone()).log_err();
        cx.notify();
        self.send(Request::ReorderThreads { section, keys }, cx)
    }

    pub fn delete_thread(&mut self, id: ThreadId, cx: &mut Context<Self>) {
        self.send(Request::DeleteThread(id), cx)
    }

    /// Keeps what's typed in the thread's composer, and the mentions in it, on its machine;
    /// `None` discards it. Only reads the app, so a closing view can still save.
    pub fn set_unsent_text(
        &self,
        id: ThreadId,
        text: Option<String>,
        mentions: Vec<UnsentMention>,
        cx: &App,
    ) {
        self.send(
            Request::SetUnsentText {
                thread_id: id,
                text,
                mentions,
            },
            cx,
        )
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
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("creating {}", parent.display()))?;
    }
    let json = serde_json::to_vec(viewed)?;
    let temporary = path.with_extension("json.tmp");
    std::fs::write(&temporary, json).with_context(|| format!("writing {}", temporary.display()))?;
    std::fs::rename(&temporary, path).with_context(|| format!("replacing {}", path.display()))
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use agentz_protocol::spaces::SpacesSnapshot;
    use gpui::TestAppContext;

    use super::*;

    #[gpui::test]
    fn a_thread_waiting_for_input_asks_for_attention(cx: &mut TestAppContext) {
        let client = cx.update(|cx| {
            crate::init_for_test(cx);
            ServerClient::new_for_test(
                MachineId::Local,
                "This Mac".into(),
                SpacesSnapshot::default(),
                cx,
            )
        });
        let project_store = client.read_with(cx, |client, _| client.projects().clone());
        let attention = Rc::new(RefCell::new(Vec::new()));
        cx.update(|cx| {
            let attention = attention.clone();
            cx.subscribe(&project_store, move |_, event: &ProjectStoreEvent, _| {
                if let ProjectStoreEvent::NeedsAttention(id, status) = event {
                    attention.borrow_mut().push((*id, *status));
                }
            })
            .detach();
        });

        let dir = tempfile::tempdir().expect("temp dir");
        let mut store = projects::ProjectStore::load(None);
        let project = store.add_project(dir.path().to_path_buf());
        let thread = store.add_thread(project, "Thread", None).expect("thread");
        let show = |store: &projects::ProjectStore, cx: &mut TestAppContext| {
            project_store.update(cx, |project_store, cx| {
                project_store.set_snapshot(store.snapshot(), cx);
                project_store.thread_status(thread)
            })
        };
        assert_eq!(show(&store, cx), None);

        store.set_thread_awaiting_input(thread, true);
        assert_eq!(show(&store, cx), Some(ThreadStatus::AwaitingInput));

        // A permission request is more pressing than a question.
        store.set_thread_blocked(thread, true);
        assert_eq!(show(&store, cx), Some(ThreadStatus::PendingApproval));

        store.set_thread_blocked(thread, false);
        store.set_thread_awaiting_input(thread, false);
        assert_eq!(show(&store, cx), None);
        assert_eq!(
            *attention.borrow(),
            vec![
                (thread, ThreadStatus::AwaitingInput),
                (thread, ThreadStatus::PendingApproval),
            ]
        );
    }

    /// A turn that left work in the background shows the thread waiting, and asks for
    /// attention only once that work is over.
    #[gpui::test]
    fn a_waiting_thread_asks_for_attention_once_done(cx: &mut TestAppContext) {
        let client = cx.update(|cx| {
            crate::init_for_test(cx);
            ServerClient::new_for_test(
                MachineId::Local,
                "This Mac".into(),
                SpacesSnapshot::default(),
                cx,
            )
        });
        let project_store = client.read_with(cx, |client, _| client.projects().clone());
        let attention = Rc::new(RefCell::new(Vec::new()));
        cx.update(|cx| {
            let attention = attention.clone();
            cx.subscribe(&project_store, move |_, event: &ProjectStoreEvent, _| {
                if let ProjectStoreEvent::NeedsAttention(id, status) = event {
                    attention.borrow_mut().push((*id, *status));
                }
            })
            .detach();
        });

        let dir = tempfile::tempdir().expect("temp dir");
        let mut store = projects::ProjectStore::load(None);
        let project = store.add_project(dir.path().to_path_buf());
        let thread = store.add_thread(project, "Thread", None).expect("thread");
        let show = |store: &projects::ProjectStore, cx: &mut TestAppContext| {
            project_store.update(cx, |project_store, cx| {
                project_store.set_snapshot(store.snapshot(), cx);
                project_store.thread_status(thread)
            })
        };
        assert_eq!(show(&store, cx), None);

        store.set_thread_working(thread, true);
        assert_eq!(show(&store, cx), Some(ThreadStatus::Working));
        store.set_thread_waiting(thread, true);
        store.set_thread_working(thread, false);
        assert_eq!(show(&store, cx), Some(ThreadStatus::Waiting));
        assert!(attention.borrow().is_empty());

        store.set_thread_waiting(thread, false);
        assert_eq!(show(&store, cx), Some(ThreadStatus::Completed));
        assert_eq!(*attention.borrow(), vec![(thread, ThreadStatus::Completed)]);
    }

    /// A subthread never asks for attention: its request shows on its parent, silently, and
    /// its end is the parent's to hear.
    #[gpui::test]
    fn subthreads_ask_for_no_attention(cx: &mut TestAppContext) {
        let client = cx.update(|cx| {
            crate::init_for_test(cx);
            ServerClient::new_for_test(
                MachineId::Local,
                "This Mac".into(),
                SpacesSnapshot::default(),
                cx,
            )
        });
        let project_store = client.read_with(cx, |client, _| client.projects().clone());
        let attention = Rc::new(RefCell::new(Vec::new()));
        cx.update(|cx| {
            let attention = attention.clone();
            cx.subscribe(&project_store, move |_, event: &ProjectStoreEvent, _| {
                if let ProjectStoreEvent::NeedsAttention(id, status) = event {
                    attention.borrow_mut().push((*id, *status));
                }
            })
            .detach();
        });

        let dir = tempfile::tempdir().expect("temp dir");
        let mut store = projects::ProjectStore::load(None);
        let project = store.add_project(dir.path().to_path_buf());
        let parent = store.add_thread(project, "Parent", None).expect("thread");
        let child = store
            .add_subthread(
                projects::Task {
                    parent,
                    prompt: "Look into it".into(),
                    role: None,
                    client_request_id: None,
                    outcome: None,
                    delivered: false,
                    agent_session: None,
                },
                None,
            )
            .expect("subthread");
        let show = |store: &projects::ProjectStore, cx: &mut TestAppContext| {
            project_store.update(cx, |project_store, cx| {
                project_store.set_snapshot(store.snapshot(), cx);
                project_store.thread_status(parent)
            })
        };
        assert_eq!(show(&store, cx), None);

        store.set_thread_working(child, true);
        assert_eq!(show(&store, cx), Some(ThreadStatus::Working));
        store.set_thread_blocked(child, true);
        assert_eq!(show(&store, cx), Some(ThreadStatus::PendingApproval));
        store.set_thread_awaiting_input(child, true);
        show(&store, cx);
        store.set_thread_blocked(child, false);
        store.set_thread_awaiting_input(child, false);
        store.set_thread_working(child, false);
        show(&store, cx);
        assert!(attention.borrow().is_empty());
    }
}
