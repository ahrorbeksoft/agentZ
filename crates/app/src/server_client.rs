//! The app's connection to `agentz-server`. It starts the server if it isn't running, keeps the
//! app's copies of the session (projects, registry, agent settings) and of open threads up to
//! date, and reconnects when the connection drops.

use std::path::PathBuf;
use std::time::Duration;

use agentz_client::Connection;
use agentz_protocol::{ClientKind, ConnectionId, Event, Request, Response};
use anyhow::{Context as _, Result, anyhow};
use collections::HashMap;
use futures::FutureExt as _;
use futures::future::BoxFuture;
use gpui::{App, AppContext as _, AsyncApp, Context, Entity, Global, Task, WeakEntity};
use ui::SharedString;

use crate::app_settings::AppSettingsStore;
use crate::project_store::ProjectStore;
use crate::registry_store::AgentRegistryStore;
use crate::thread_entity::AgentThread;

const MIN_RETRY_DELAY: Duration = Duration::from_millis(250);
const MAX_RETRY_DELAY: Duration = Duration::from_secs(5);
const SERVER_BINARY_ENV_VAR: &str = "AGENTZ_SERVER_BIN";

#[derive(Clone, Debug, PartialEq)]
pub enum ServerStatus {
    Connecting,
    Connected,
    /// Retrying after the error.
    Disconnected(SharedString),
}

pub struct ServerClient {
    connection: Option<Connection>,
    status: ServerStatus,
    /// Open threads and account connections, which get the server's updates.
    threads: HashMap<ConnectionId, WeakEntity<AgentThread>>,
    /// Session events that arrived while the session snapshot was on its way.
    queued_session_events: Option<Vec<Event>>,
    _maintain_connection: Task<()>,
}

struct GlobalServerClient(Entity<ServerClient>);

impl Global for GlobalServerClient {}

pub fn init(cx: &mut App) {
    let client = cx.new(|cx| ServerClient {
        connection: None,
        status: ServerStatus::Connecting,
        threads: HashMap::default(),
        queued_session_events: None,
        _maintain_connection: cx.spawn(async move |this, cx| maintain_connection(this, cx).await),
    });
    cx.set_global(GlobalServerClient(client));
}

impl ServerClient {
    pub fn global(cx: &App) -> Entity<Self> {
        cx.global::<GlobalServerClient>().0.clone()
    }

    pub fn status(&self) -> &ServerStatus {
        &self.status
    }

    pub fn connection(&self) -> Option<&Connection> {
        self.connection.as_ref()
    }

    /// Sends a request now; the future waits for the answer. It fails at once while
    /// disconnected.
    pub fn request(&self, request: Request) -> BoxFuture<'static, Result<Response>> {
        match &self.connection {
            Some(connection) => connection.request(request).boxed(),
            None => futures::future::ready(Err(anyhow!("not connected to agentz-server"))).boxed(),
        }
    }

    /// Sends a request whose answer only matters if it's an error.
    pub fn send(&self, request: Request, cx: &App) {
        let description = request_name(&request);
        let response = self.request(request);
        cx.background_spawn(async move {
            if let Err(error) = response.await {
                log::error!("{description} failed: {error:#}");
            }
        })
        .detach();
    }

    pub(crate) fn register_thread(
        &mut self,
        connection: ConnectionId,
        thread: WeakEntity<AgentThread>,
    ) {
        self.threads.retain(|_, thread| thread.upgrade().is_some());
        self.threads.insert(connection, thread);
    }

    fn connected(&mut self, connection: Connection, cx: &mut Context<Self>) {
        log::info!(
            "connected to agentz-server {} (pid {})",
            connection.welcome().server_version,
            connection.welcome().pid
        );
        self.connection = Some(connection);
        self.status = ServerStatus::Connected;
        self.queued_session_events = Some(Vec::new());
        let threads: Vec<_> = self
            .threads
            .values()
            .filter_map(|thread| thread.upgrade())
            .collect();
        // Deferred: the threads read this client, which is being updated.
        cx.defer(move |cx| {
            for thread in threads {
                thread.update(cx, |thread, cx| thread.reconnected(cx));
            }
        });
        cx.notify();
    }

    fn disconnected(&mut self, error: SharedString, cx: &mut Context<Self>) {
        self.connection = None;
        self.status = ServerStatus::Disconnected(error);
        self.queued_session_events = None;
        cx.notify();
    }

    fn apply_session(&mut self, response: Result<Response>, cx: &mut Context<Self>) {
        let session = match response {
            Ok(Response::Session(session)) => session,
            Ok(response) => {
                log::error!("expected a session snapshot, got {response:?}");
                return;
            }
            Err(error) => {
                log::error!("failed to subscribe to the session: {error:#}");
                return;
            }
        };
        ProjectStore::global(cx).update(cx, |store, cx| store.set_snapshot(session.projects, cx));
        AgentRegistryStore::global(cx).update(cx, |registry, cx| {
            registry.set_snapshot(session.registry, cx)
        });
        AppSettingsStore::global(cx).update(cx, |settings, cx| {
            settings.set_agent_settings(session.agent_settings, cx)
        });
        for event in self.queued_session_events.take().unwrap_or_default() {
            self.handle_event(event, cx);
        }
    }

    fn handle_event(&mut self, event: Event, cx: &mut Context<Self>) {
        if let Some(queued) = &mut self.queued_session_events
            && matches!(
                event,
                Event::Projects(_) | Event::Registry(_) | Event::AgentSettings(_)
            )
        {
            queued.push(event);
            return;
        }
        match event {
            Event::Projects(projects) => {
                ProjectStore::global(cx).update(cx, |store, cx| store.set_snapshot(projects, cx))
            }
            Event::Registry(registry) => AgentRegistryStore::global(cx)
                .update(cx, |store, cx| store.set_snapshot(registry, cx)),
            Event::AgentSettings(agent_settings) => AppSettingsStore::global(cx)
                .update(cx, |settings, cx| {
                    settings.set_agent_settings(agent_settings, cx)
                }),
            Event::Thread { connection, update } => {
                if let Some(thread) = self.threads.get(&connection).and_then(|t| t.upgrade()) {
                    thread.update(cx, |thread, cx| thread.apply_update(update, cx));
                }
            }
            Event::ConnectionClosed(connection) => {
                if let Some(thread) = self.threads.remove(&connection).and_then(|t| t.upgrade()) {
                    thread.update(cx, |thread, cx| thread.closed(cx));
                }
            }
            Event::Unknown(event) => log::warn!("unknown event from the server: {event}"),
        }
    }
}

async fn maintain_connection(this: WeakEntity<ServerClient>, cx: &mut AsyncApp) {
    let runtime = reqwest_client::runtime().handle().clone();
    let mut delay = MIN_RETRY_DELAY;
    loop {
        let error = match connect(&runtime).await {
            Ok((connection, mut events)) => {
                delay = MIN_RETRY_DELAY;
                let session = connection.request(Request::SubscribeSession);
                if this
                    .update(cx, |this, cx| this.connected(connection, cx))
                    .is_err()
                {
                    return;
                }
                let session_applied = this.clone();
                cx.spawn(async move |cx| {
                    let response = session.await;
                    session_applied
                        .update(cx, |this, cx| this.apply_session(response, cx))
                        .ok();
                })
                .detach();
                while let Some(event) = events.next().await {
                    if this
                        .update(cx, |this, cx| this.handle_event(event, cx))
                        .is_err()
                    {
                        return;
                    }
                }
                "the connection to agentz-server closed".to_string()
            }
            Err(error) => format!("{error:#}"),
        };
        log::warn!("{error}; retrying in {delay:?}");
        if this
            .update(cx, |this, cx| this.disconnected(error.into(), cx))
            .is_err()
        {
            return;
        }
        cx.background_executor().timer(delay).await;
        delay = (delay * 2).min(MAX_RETRY_DELAY);
    }
}

/// Connects to the local server, starting it first if nothing is listening.
async fn connect(runtime: &tokio::runtime::Handle) -> Result<(Connection, agentz_client::Events)> {
    let socket = paths::server_socket();
    let version = env!("CARGO_PKG_VERSION").to_string();
    match agentz_client::connect_local(runtime, &socket, ClientKind::App, version.clone()).await {
        Ok(connected) => return Ok(connected),
        Err(error) => log::info!("starting agentz-server ({error:#})"),
    }
    agentz_client::start_local_server(runtime, &server_binary()?).await?;
    agentz_client::connect_local(runtime, &socket, ClientKind::App, version).await
}

/// `agentz-server` next to the app's executable, as `cargo build` and the app bundle place it.
fn server_binary() -> Result<PathBuf> {
    if let Some(path) = std::env::var_os(SERVER_BINARY_ENV_VAR) {
        return Ok(PathBuf::from(path));
    }
    let executable = std::env::current_exe().context("finding the app's executable")?;
    let path = executable
        .parent()
        .context("finding the app's directory")?
        .join("agentz-server");
    anyhow::ensure!(
        path.exists(),
        "{} is missing; build it with `cargo build`",
        path.display()
    );
    Ok(path)
}

/// The request's variant, for logs.
fn request_name(request: &Request) -> String {
    let debug = format!("{request:?}");
    debug
        .split(|character: char| !character.is_alphanumeric())
        .next()
        .unwrap_or_default()
        .to_string()
}
