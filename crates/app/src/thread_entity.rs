//! The app's handle on an [`agent_thread::AgentThread`]: a GPUI entity that reads through to the
//! thread, feeds it the results of its background work, and passes its events on as GPUI events.

use std::ops::Deref;
use std::path::PathBuf;

use agent_client_protocol::schema::v1 as acp;
use agent_thread::{AgentThreadEvent, CommandFuture, SessionDefaults, ThreadInbox};
use futures::StreamExt as _;
use gpui::{Context, EventEmitter, SharedString, Task};

pub struct AgentThread {
    thread: agent_thread::AgentThread,
    _messages: Task<()>,
}

impl EventEmitter<AgentThreadEvent> for AgentThread {}

impl Deref for AgentThread {
    type Target = agent_thread::AgentThread;

    fn deref(&self) -> &Self::Target {
        &self.thread
    }
}

impl AgentThread {
    pub fn start(
        agent_name: SharedString,
        command: CommandFuture,
        cwd: PathBuf,
        previous_session: Option<acp::SessionId>,
        cx: &mut Context<Self>,
    ) -> Self {
        let (thread, inbox) = agent_thread::AgentThread::start(
            reqwest_client::runtime().handle().clone(),
            agent_name,
            command,
            cwd,
            previous_session,
        );
        Self::new(thread, inbox, cx)
    }

    pub fn start_for_account(
        agent_name: SharedString,
        command: CommandFuture,
        cx: &mut Context<Self>,
    ) -> Self {
        let (thread, inbox) = agent_thread::AgentThread::start_for_account(
            reqwest_client::runtime().handle().clone(),
            agent_name,
            command,
        );
        Self::new(thread, inbox, cx)
    }

    pub fn failed(agent_name: SharedString, error: impl Into<SharedString>) -> Self {
        Self {
            thread: agent_thread::AgentThread::failed(agent_name, error),
            _messages: Task::ready(()),
        }
    }

    fn new(
        thread: agent_thread::AgentThread,
        mut inbox: ThreadInbox,
        cx: &mut Context<Self>,
    ) -> Self {
        let messages = cx.spawn(async move |this, cx| {
            while let Some(message) = inbox.next().await {
                let delivered = this.update(cx, |this, cx| {
                    this.change(cx, |thread| thread.handle(message))
                });
                if delivered.is_err() {
                    break;
                }
            }
        });
        Self {
            thread,
            _messages: messages,
        }
    }

    fn change<R>(
        &mut self,
        cx: &mut Context<Self>,
        change: impl FnOnce(&mut agent_thread::AgentThread) -> R,
    ) -> R {
        let result = change(&mut self.thread);
        for event in self.thread.take_events() {
            cx.emit(event);
        }
        cx.notify();
        result
    }

    pub fn reload(&mut self, cx: &mut Context<Self>) {
        self.change(cx, |thread| thread.reload())
    }

    pub fn reauthenticate(&mut self, cx: &mut Context<Self>) {
        self.change(cx, |thread| thread.reauthenticate())
    }

    pub fn logout(&mut self, cx: &mut Context<Self>) {
        self.change(cx, |thread| thread.logout())
    }

    pub fn check_login(&mut self, cx: &mut Context<Self>) {
        self.change(cx, |thread| thread.check_login())
    }

    pub fn set_defaults(&mut self, defaults: SessionDefaults) {
        self.thread.set_defaults(defaults)
    }

    pub fn authenticate(&mut self, method_id: acp::AuthMethodId, cx: &mut Context<Self>) {
        self.change(cx, |thread| thread.authenticate(method_id))
    }

    pub fn retry_session(&mut self, cx: &mut Context<Self>) {
        self.change(cx, |thread| thread.retry_session())
    }

    pub fn clear_plan(&mut self, cx: &mut Context<Self>) {
        self.change(cx, |thread| thread.clear_plan())
    }

    pub fn set_config_option(
        &mut self,
        config_id: acp::SessionConfigId,
        value: acp::SessionConfigOptionValue,
        cx: &mut Context<Self>,
    ) {
        self.change(cx, |thread| thread.set_config_option(config_id, value))
    }

    pub fn set_mode(&mut self, mode_id: acp::SessionModeId, cx: &mut Context<Self>) {
        self.change(cx, |thread| thread.set_mode(mode_id))
    }

    pub fn send(&mut self, text: String, cx: &mut Context<Self>) {
        self.change(cx, |thread| thread.send(text))
    }

    pub fn cancel(&mut self, cx: &mut Context<Self>) {
        self.change(cx, |thread| thread.cancel())
    }

    pub fn respond_to_permission(
        &mut self,
        tool_call_id: &acp::ToolCallId,
        option_id: acp::PermissionOptionId,
        cx: &mut Context<Self>,
    ) {
        self.change(cx, |thread| {
            thread.respond_to_permission(tool_call_id, option_id)
        })
    }
}
