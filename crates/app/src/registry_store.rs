//! The app's copy of the server's agent registry. Refreshing, installing and uninstalling are
//! requests to the server.

use std::ops::Deref;

use agentz_protocol::Request;
use agentz_protocol::agents::{AgentId, RegistrySnapshot};
use gpui::{App, Context, WeakEntity};

use crate::server_client::ServerClient;

/// One machine's agents.
pub struct AgentRegistryStore {
    snapshot: RegistrySnapshot,
    client: WeakEntity<ServerClient>,
}

impl Deref for AgentRegistryStore {
    type Target = RegistrySnapshot;

    fn deref(&self) -> &Self::Target {
        &self.snapshot
    }
}

impl AgentRegistryStore {
    pub(crate) fn new(client: WeakEntity<ServerClient>) -> Self {
        Self {
            snapshot: RegistrySnapshot::default(),
            client,
        }
    }

    pub(crate) fn set_snapshot(&mut self, snapshot: RegistrySnapshot, cx: &mut Context<Self>) {
        if snapshot != self.snapshot {
            self.snapshot = snapshot;
            cx.notify();
        }
    }

    fn send(&self, request: Request, cx: &App) {
        if let Some(client) = self.client.upgrade() {
            client.read(cx).send(request, cx);
        }
    }

    pub fn refresh(&mut self, cx: &mut Context<Self>) {
        self.send(Request::RefreshRegistry { if_stale: false }, cx)
    }

    pub fn refresh_if_stale(&mut self, cx: &mut Context<Self>) {
        self.send(Request::RefreshRegistry { if_stale: true }, cx)
    }

    pub fn install(&mut self, id: &AgentId, cx: &mut Context<Self>) {
        self.send(Request::InstallAgent(id.clone()), cx)
    }

    pub fn uninstall(&mut self, id: &AgentId, cx: &mut Context<Self>) {
        self.send(Request::UninstallAgent(id.clone()), cx)
    }
}
