//! The app's handle on [`registry::AgentRegistryStore`]: a GPUI entity that reads through to the
//! store, feeds it the results of its background work and notifies observers.

use std::ops::Deref;
use std::sync::Arc;

use futures::StreamExt as _;
use gpui::{App, AppContext as _, Context, Entity, Global, Task};
use http_client::HttpClient;
use registry::{AgentId, CommandFuture, ShellEnvironmentReady};

pub struct AgentRegistryStore {
    store: registry::AgentRegistryStore,
    _messages: Task<()>,
}

struct GlobalAgentRegistryStore(Entity<AgentRegistryStore>);

impl Global for GlobalAgentRegistryStore {}

pub fn init(
    http_client: Arc<dyn HttpClient>,
    shell_environment_ready: ShellEnvironmentReady,
    cx: &mut App,
) {
    let store = cx.new(|cx| {
        let (store, mut inbox) = registry::AgentRegistryStore::new(
            reqwest_client::runtime().handle().clone(),
            http_client,
            shell_environment_ready,
            paths::registry_dir(),
        );
        let messages = cx.spawn(async move |this, cx| {
            while let Some(message) = inbox.next().await {
                let delivered = this.update(cx, |this: &mut AgentRegistryStore, cx| {
                    this.store.handle(message);
                    cx.notify();
                });
                if delivered.is_err() {
                    break;
                }
            }
        });
        AgentRegistryStore {
            store,
            _messages: messages,
        }
    });
    cx.set_global(GlobalAgentRegistryStore(store.clone()));
    store.update(cx, |store, cx| store.refresh_if_stale(cx));
}

impl Deref for AgentRegistryStore {
    type Target = registry::AgentRegistryStore;

    fn deref(&self) -> &Self::Target {
        &self.store
    }
}

impl AgentRegistryStore {
    pub fn global(cx: &App) -> Entity<Self> {
        cx.global::<GlobalAgentRegistryStore>().0.clone()
    }

    pub fn refresh(&mut self, cx: &mut Context<Self>) {
        self.store.refresh();
        cx.notify();
    }

    pub fn refresh_if_stale(&mut self, cx: &mut Context<Self>) {
        self.store.refresh_if_stale();
        cx.notify();
    }

    pub fn install(&mut self, id: &AgentId, cx: &mut Context<Self>) {
        self.store.install(id);
        cx.notify();
    }

    pub fn uninstall(&mut self, id: &AgentId, cx: &mut Context<Self>) {
        self.store.uninstall(id);
        cx.notify();
    }

    pub fn command_when_loaded(&mut self, id: &AgentId) -> CommandFuture {
        self.store.command_when_loaded(id)
    }
}
