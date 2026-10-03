//! The app's copy of a thread on the server, or of an agent connection opened from settings to
//! log in or out. It reads like the server's `AgentThread`; its actions are requests.

use std::ops::Deref;

use agent_client_protocol::schema::v1 as acp;
use agentz_protocol::agents::AgentId;
use agentz_protocol::thread::{ConnectionStatus, ThreadState, ThreadUpdate, ThreadView};
use agentz_protocol::{ConnectionId, Request, Response};
use gpui::{App, AppContext as _, Context, Entity, SharedString, Task};
use projects::ThreadId;

use crate::server_client::ServerClient;

pub struct AgentThread {
    /// The server of the thread's machine.
    client: Entity<ServerClient>,
    /// `None` until the server has opened an account connection.
    connection: Option<ConnectionId>,
    view: ThreadView,
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
        if let ConnectionId::Account(account_id) = connection {
            drop(server.request(Request::CloseAccount(account_id)));
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

    /// Starts the agent only to log in or out. It stops when this is dropped.
    pub fn open_account(
        client: Entity<ServerClient>,
        agent_id: AgentId,
        agent_name: SharedString,
        cx: &mut Context<Self>,
    ) -> Self {
        let response = client.read(cx).request(Request::OpenAccount(agent_id));
        let mut this = Self::new(client, agent_name);
        this._subscribe = cx.spawn(async move |this, cx| {
            let result = match response.await {
                Ok(Response::AccountOpened(account_id)) => Ok(account_id),
                Ok(response) => Err(anyhow::anyhow!("unexpected response: {response:?}")),
                Err(error) => Err(error),
            };
            this.update(cx, |this, cx| match result {
                Ok(account_id) => this.attach(ConnectionId::Account(account_id), cx),
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
                    for update in this.queued_updates.take().unwrap_or_default() {
                        this.view.apply(update);
                    }
                    cx.notify();
                }
                Ok(response) => this.fail(format!("unexpected response: {response:?}"), cx),
                Err(error) => this.fail(format!("{error:#}"), cx),
            })
            .ok();
        });
    }

    /// The server is back. Threads pick up where they are; account connections ended with the
    /// old connection.
    pub fn client(&self) -> &Entity<ServerClient> {
        &self.client
    }

    pub(crate) fn reconnected(&mut self, cx: &mut Context<Self>) {
        match self.connection {
            Some(ConnectionId::Thread(_)) => self.subscribe(cx),
            Some(ConnectionId::Account(_)) => self.closed(cx),
            None => {}
        }
    }

    pub(crate) fn apply_update(&mut self, update: ThreadUpdate, cx: &mut Context<Self>) {
        match &mut self.queued_updates {
            Some(queued) => queued.push(update),
            None => {
                self.view.apply(update);
                cx.notify();
            }
        }
    }

    pub(crate) fn closed(&mut self, cx: &mut Context<Self>) {
        self.fail("The agent's connection closed.".to_string(), cx);
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

    pub fn send(&mut self, text: String, cx: &mut Context<Self>) {
        self.request(|connection| Request::Prompt { connection, text }, cx)
    }

    pub fn cancel(&mut self, cx: &mut Context<Self>) {
        self.request(Request::Cancel, cx)
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
}
