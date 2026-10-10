//! The app's copy of a thread on the server, or of an agent connection opened from settings to
//! log in or out. It reads like the server's `AgentThread`; its actions are requests.

use std::ops::Deref;

use agent_client_protocol::schema::v1 as acp;
use agentz_protocol::accounts::AccountId;
use agentz_protocol::agents::AgentId;
use agentz_protocol::thread::{ConnectionStatus, ThreadState, ThreadUpdate, ThreadView};
use agentz_protocol::{ConnectionId, Request, Response};
use gpui::{App, AppContext as _, Context, Entity, SharedString, Task};
use projects::ThreadId;

use crate::server_client::ServerClient;

pub struct AgentThread {
    /// The server of the thread's machine.
    client: Entity<ServerClient>,
    /// `None` until the server has opened a login session.
    connection: Option<ConnectionId>,
    view: ThreadView,
    /// A revision for each entry, raised when it changes, so views redo only what changed.
    entry_revisions: Vec<u64>,
    next_revision: u64,
    /// Updates that arrived while the snapshot they follow was on its way.
    queued_updates: Option<Vec<ThreadUpdate>>,
    /// Kept to unsubscribe on drop, when there's no context to look it up.
    server: Option<agentz_client::Connection>,
    _subscribe: Task<()>,
}

impl Deref for AgentThread {
    type Target = ThreadView;

    fn deref(&self) -> &Self::Target {
        &self.view
    }
}

impl Drop for AgentThread {
    fn drop(&mut self) {
        let (Some(server), Some(connection)) = (&self.server, self.connection) else {
            return;
        };
        // Sent now; nobody waits for the answers.
        drop(server.request(Request::UnsubscribeThread(connection)));
        if let ConnectionId::LoginSession(login_session_id) = connection {
            drop(server.request(Request::CloseLoginSession(login_session_id)));
        }
    }
}

impl AgentThread {
    fn new(client: Entity<ServerClient>, agent_name: SharedString) -> Self {
        Self {
            client,
            connection: None,
            view: ThreadView {
                state: ThreadState {
                    agent_name,
                    ..ThreadState::default()
                },
                entries: Vec::new(),
            },
            entry_revisions: Vec::new(),
            next_revision: 0,
            queued_updates: None,
            server: None,
            _subscribe: Task::ready(()),
        }
    }

    /// Follows the thread's conversation. The server starts its agent if it isn't running.
    pub fn open(
        client: Entity<ServerClient>,
        thread_id: ThreadId,
        agent_name: SharedString,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut this = Self::new(client, agent_name);
        this.attach(ConnectionId::Thread(thread_id), cx);
        this
    }

    /// The app's one copy of the thread, shared by every view showing it: the server sends a
    /// client each thread's updates once.
    pub fn shared(
        client: &Entity<ServerClient>,
        thread_id: ThreadId,
        cx: &mut App,
    ) -> Entity<Self> {
        if let Some(thread) = client.read(cx).thread(ConnectionId::Thread(thread_id)) {
            return thread;
        }
        let agent_id = client
            .read(cx)
            .projects()
            .read(cx)
            .thread(thread_id)
            .and_then(|thread| thread.agent_id.clone())
            .map(AgentId::new);
        let agent_name = agent_id
            .as_ref()
            .and_then(|agent_id| {
                client
                    .read(cx)
                    .registry()
                    .read(cx)
                    .agent(agent_id)
                    .map(|agent| agent.name().clone())
            })
            .or_else(|| agent_id.as_ref().map(|agent_id| agent_id.0.clone()))
            .unwrap_or_else(|| "Agent".into());
        let client = client.clone();
        cx.new(|cx| Self::open(client, thread_id, agent_name, cx))
    }

    /// Starts the agent only to log the account in or out, `None` being the External account.
    /// It stops when this is dropped.
    pub fn open_login_session(
        client: Entity<ServerClient>,
        agent_id: AgentId,
        account: Option<AccountId>,
        agent_name: SharedString,
        cx: &mut Context<Self>,
    ) -> Self {
        let response = client
            .read(cx)
            .request(Request::OpenLoginSession { agent_id, account });
        let mut this = Self::new(client, agent_name);
        this._subscribe = cx.spawn(async move |this, cx| {
            let result = match response.await {
                Ok(Response::LoginSessionOpened(login_session_id)) => Ok(login_session_id),
                Ok(response) => Err(anyhow::anyhow!("unexpected response: {response:?}")),
                Err(error) => Err(error),
            };
            this.update(cx, |this, cx| match result {
                Ok(login_session_id) => {
                    this.attach(ConnectionId::LoginSession(login_session_id), cx)
                }
                Err(error) => this.fail(format!("{error:#}"), cx),
            })
            .ok();
        });
        this
    }

    fn attach(&mut self, connection: ConnectionId, cx: &mut Context<Self>) {
        self.connection = Some(connection);
        let this = cx.weak_entity();
        self.client
            .update(cx, |client, _| client.register_thread(connection, this));
        self.subscribe(cx);
    }

    fn subscribe(&mut self, cx: &mut Context<Self>) {
        let Some(connection) = self.connection else {
            return;
        };
        let client = self.client.clone();
        self.server = client.read(cx).connection().cloned();
        self.queued_updates = Some(Vec::new());
        let response = client
            .read(cx)
            .request(Request::SubscribeThread(connection));
        self._subscribe = cx.spawn(async move |this, cx| {
            let response = response.await;
            this.update(cx, |this, cx| match response {
                Ok(Response::Thread(view)) => {
                    this.view = view;
                    this.entry_revisions.clear();
                    this.note_changed(0..this.view.entries.len());
                    for update in this.queued_updates.take().unwrap_or_default() {
                        this.apply(update);
                    }
                    cx.notify();
                }
                Ok(response) => this.fail(format!("unexpected response: {response:?}"), cx),
                Err(error) => this.fail(format!("{error:#}"), cx),
            })
            .ok();
        });
    }

    /// The server is back. Threads pick up where they are; login sessions ended with the
    /// old connection.
    pub fn client(&self) -> &Entity<ServerClient> {
        &self.client
    }

    pub(crate) fn reconnected(&mut self, cx: &mut Context<Self>) {
        match self.connection {
            Some(ConnectionId::Thread(_)) => self.subscribe(cx),
            Some(ConnectionId::LoginSession(_)) => self.closed(cx),
            None => {}
        }
    }

    pub(crate) fn apply_update(&mut self, update: ThreadUpdate, cx: &mut Context<Self>) {
        match &mut self.queued_updates {
            Some(queued) => queued.push(update),
            None => {
                self.apply(update);
                cx.notify();
            }
        }
    }

    fn apply(&mut self, update: ThreadUpdate) {
        let changed: Vec<usize> = update
            .entries
            .iter()
            .map(|(index, _)| *index)
            .chain(update.appended.iter().map(|(index, _)| *index))
            .collect();
        self.view.apply(update);
        self.entry_revisions.truncate(self.view.entries.len());
        for index in changed {
            self.note_changed(index..index + 1);
        }
    }

    fn note_changed(&mut self, range: std::ops::Range<usize>) {
        for index in range {
            if index >= self.view.entries.len() {
                break;
            }
            self.next_revision += 1;
            if index < self.entry_revisions.len() {
                self.entry_revisions[index] = self.next_revision;
            } else {
                self.entry_revisions.resize(index, 0);
                self.entry_revisions.push(self.next_revision);
            }
        }
    }

    /// Each entry's revision: a view has an entry as it is when it has its revision.
    pub fn entry_revisions(&self) -> &[u64] {
        &self.entry_revisions
    }

    pub(crate) fn closed(&mut self, cx: &mut Context<Self>) {
        self.fail("The agent's connection closed.".to_string(), cx);
    }

    #[cfg(test)]
    pub(crate) fn set_entries_for_test(
        &mut self,
        entries: Vec<agentz_protocol::thread::Entry>,
        cx: &mut Context<Self>,
    ) {
        self.view.entries = entries;
        self.entry_revisions.clear();
        self.note_changed(0..self.view.entries.len());
        cx.notify();
    }

    /// Adds an entry as one streaming in would: the others keep their revisions.
    #[cfg(test)]
    pub(crate) fn push_entry_for_test(
        &mut self,
        entry: agentz_protocol::thread::Entry,
        cx: &mut Context<Self>,
    ) {
        self.view.entries.push(entry);
        let index = self.view.entries.len() - 1;
        self.note_changed(index..index + 1);
        cx.notify();
    }

    #[cfg(test)]
    pub(crate) fn set_working_for_test(&mut self, working: bool, cx: &mut Context<Self>) {
        self.view.state.turn_started_at = working.then(std::time::SystemTime::now);
        cx.notify();
    }

    #[cfg(test)]
    pub(crate) fn set_turn_error_for_test(&mut self, error: &str, cx: &mut Context<Self>) {
        self.view.state.turn_error = Some(error.to_string().into());
        cx.notify();
    }

    #[cfg(test)]
    pub(crate) fn update_state_for_test(
        &mut self,
        update: impl FnOnce(&mut agentz_protocol::thread::ThreadState),
        cx: &mut Context<Self>,
    ) {
        update(&mut self.view.state);
        cx.notify();
    }

    #[cfg(test)]
    pub(crate) fn set_status_for_test(&mut self, status: ConnectionStatus, cx: &mut Context<Self>) {
        self.view.state.status = status;
        cx.notify();
    }

    /// Sets the permission requests as a state update would: no entry changes with them.
    #[cfg(test)]
    pub(crate) fn set_permission_requests_for_test(
        &mut self,
        requests: Vec<agentz_protocol::thread::PermissionRequest>,
        cx: &mut Context<Self>,
    ) {
        self.view.state.permission_requests = requests;
        cx.notify();
    }

    #[cfg(test)]
    pub(crate) fn set_background_tasks_for_test(
        &mut self,
        tasks: Vec<agentz_protocol::thread::BackgroundTask>,
        cx: &mut Context<Self>,
    ) {
        self.view.state.background_tasks = tasks;
        cx.notify();
    }

    #[cfg(test)]
    pub(crate) fn set_queued_messages_for_test(
        &mut self,
        messages: Vec<agentz_protocol::thread::QueuedMessage>,
        steering: bool,
        cx: &mut Context<Self>,
    ) {
        self.view.state.queued_messages = messages;
        self.view.state.steering_queued = steering;
        cx.notify();
    }

    fn fail(&mut self, error: String, cx: &mut Context<Self>) {
        self.queued_updates = None;
        self.view.state.status = ConnectionStatus::Failed(error.into());
        self.view.state.turn_started_at = None;
        cx.notify();
    }

    fn request(&self, request: impl FnOnce(ConnectionId) -> Request, cx: &App) {
        match self.connection {
            Some(connection) => self.client.read(cx).send(request(connection), cx),
            None => log::warn!("the agent's connection hasn't opened yet"),
        }
    }

    pub fn reload(&mut self, cx: &mut Context<Self>) {
        self.request(Request::Reload, cx)
    }

    pub fn reauthenticate(&mut self, cx: &mut Context<Self>) {
        self.request(Request::Reauthenticate, cx)
    }

    pub fn logout(&mut self, cx: &mut Context<Self>) {
        self.request(Request::Logout, cx)
    }

    pub fn check_login(&mut self, cx: &mut Context<Self>) {
        self.request(Request::CheckLogin, cx)
    }

    pub fn authenticate(
        &mut self,
        method_id: acp::AuthMethodId,
        meta: Option<acp::Meta>,
        cx: &mut Context<Self>,
    ) {
        self.request(
            |connection| Request::Authenticate {
                connection,
                method_id,
                meta,
            },
            cx,
        )
    }

    pub fn cancel_authentication(&mut self, cx: &mut Context<Self>) {
        self.request(Request::CancelAuthentication, cx)
    }

    pub fn respond_to_elicitation(
        &mut self,
        elicitation: u64,
        action: acp::ElicitationAction,
        cx: &mut Context<Self>,
    ) {
        self.request(
            |connection| Request::RespondToElicitation {
                connection,
                elicitation,
                action,
            },
            cx,
        )
    }

    pub fn dismiss_elicitation(&mut self, elicitation: u64, cx: &mut Context<Self>) {
        self.request(
            |connection| Request::DismissElicitation {
                connection,
                elicitation,
            },
            cx,
        )
    }

    /// The server's id for the connection, once it's open.
    pub fn connection(&self) -> Option<ConnectionId> {
        self.connection
    }

    /// Runs a terminal login method on the agent's machine, in [`TerminalKey::Login`].
    ///
    /// [`TerminalKey::Login`]: agentz_protocol::terminal::TerminalKey::Login
    pub fn terminal_login(&mut self, method_id: acp::AuthMethodId, cx: &mut Context<Self>) {
        self.request(
            |connection| Request::TerminalLogin {
                connection,
                method_id,
            },
            cx,
        )
    }

    pub fn retry_session(&mut self, cx: &mut Context<Self>) {
        self.request(Request::RetrySession, cx)
    }

    /// Sends the message that didn't get through again.
    pub fn retry_message(&mut self, cx: &mut Context<Self>) {
        self.request(Request::RetryMessage, cx)
    }

    /// Makes the thread's new worktree or pasture again, from the step that failed.
    pub fn retry_workspace_setup(&mut self, cx: &mut Context<Self>) {
        self.thread_request(Request::RetryWorkspaceSetup, cx)
    }

    /// Gives up on the thread's new worktree or pasture: its first message goes to the agent
    /// in the folder it's in.
    pub fn use_local(&mut self, cx: &mut Context<Self>) {
        self.thread_request(Request::UseLocal, cx)
    }

    /// A request about the thread itself, which a login session has none of.
    fn thread_request(&self, request: impl FnOnce(ThreadId) -> Request, cx: &App) {
        if let Some(ConnectionId::Thread(thread_id)) = self.connection {
            self.client.read(cx).send(request(thread_id), cx);
        }
    }

    /// Starts this continued thread without the conversation it would have brought.
    pub fn drop_handoff(&mut self, cx: &mut Context<Self>) {
        self.request(Request::DropHandoff, cx)
    }

    pub fn clear_plan(&mut self, cx: &mut Context<Self>) {
        self.request(Request::ClearPlan, cx)
    }

    pub fn set_config_option(
        &mut self,
        config_id: acp::SessionConfigId,
        value: acp::SessionConfigOptionValue,
        cx: &mut Context<Self>,
    ) {
        self.request(
            |connection| Request::SetConfigOption {
                connection,
                config_id,
                value,
            },
            cx,
        )
    }

    pub fn set_mode(&mut self, mode_id: acp::SessionModeId, cx: &mut Context<Self>) {
        self.request(
            |connection| Request::SetMode {
                connection,
                mode_id,
            },
            cx,
        )
    }

    pub fn send(&mut self, prompt: Vec<agentz_protocol::PromptPart>, cx: &mut Context<Self>) {
        self.request(|connection| Request::Prompt { connection, prompt }, cx)
    }

    pub fn cancel(&mut self, cx: &mut Context<Self>) {
        self.request(Request::Cancel, cx)
    }

    /// Adds the message to the queue the server keeps for the thread
    /// ([`ThreadState::queued_messages`]).
    pub fn queue_message(
        &mut self,
        prompt: Vec<agentz_protocol::PromptPart>,
        cx: &mut Context<Self>,
    ) {
        self.request(
            |connection| Request::QueueMessage { connection, prompt },
            cx,
        )
    }

    pub fn remove_queued_message(&mut self, id: u64, cx: &mut Context<Self>) {
        self.request(
            |connection| Request::RemoveQueuedMessage { connection, id },
            cx,
        )
    }

    pub fn steer_queued_message(&mut self, id: u64, cx: &mut Context<Self>) {
        self.request(
            |connection| Request::SteerQueuedMessage { connection, id },
            cx,
        )
    }

    pub fn send_queued_message_now(&mut self, id: u64, cx: &mut Context<Self>) {
        self.request(
            |connection| Request::SendQueuedMessageNow { connection, id },
            cx,
        )
    }

    pub fn clear_queue(&mut self, cx: &mut Context<Self>) {
        self.request(Request::ClearQueue, cx)
    }

    pub fn respond_to_permission(
        &mut self,
        tool_call_id: &acp::ToolCallId,
        option_id: acp::PermissionOptionId,
        cx: &mut Context<Self>,
    ) {
        let tool_call_id = tool_call_id.clone();
        self.request(
            |connection| Request::RespondToPermission {
                connection,
                tool_call_id,
                option_id,
            },
            cx,
        )
    }

    pub fn stop_background_task(&mut self, task_id: &str, cx: &mut Context<Self>) {
        let task_id = task_id.to_string();
        self.request(
            |connection| Request::StopBackgroundTask {
                connection,
                task_id,
            },
            cx,
        )
    }
}
