//! The app's copy of the server's agent registry. Refreshing, installing and uninstalling are
//! requests to the server.

use std::ops::Deref;

use agentz_protocol::Request;
use agentz_protocol::agents::{AgentId, RegistrySnapshot};
use gpui::{App, AppContext as _, Context, Entity, Global};

use crate::server_client::ServerClient;

pub struct AgentRegistryStore {
    snapshot: RegistrySnapshot,
}

struct GlobalAgentRegistryStore(Entity<AgentRegistryStore>);

impl Global for GlobalAgentRegistryStore {}

pub fn init(cx: &mut App) {
    let store = cx.new(|_| AgentRegistryStore {
        snapshot: RegistrySnapshot::default(),
    });
    cx.set_global(GlobalAgentRegistryStore(store));
}

impl Deref for AgentRegistryStore {
    type Target = RegistrySnapshot;

    fn deref(&self) -> &Self::Target {
        &self.snapshot
    }
}

impl AgentRegistryStore {
    pub fn global(cx: &App) -> Entity<Self> {
        cx.global::<GlobalAgentRegistryStore>().0.clone()
    }

    pub(crate) fn set_snapshot(&mut self, snapshot: RegistrySnapshot, cx: &mut Context<Self>) {
        if snapshot != self.snapshot {
            self.snapshot = snapshot;
            cx.notify();
        }
    }

    fn send(&self, request: Request, cx: &App) {
        ServerClient::global(cx).read(cx).send(request, cx);
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
