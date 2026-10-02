//! The agentZ server: it owns the projects, the agent registry, agent settings and the agents
//! themselves, so threads keep working while no app is connected (herdr's model). Clients talk
//! to it in [`agentz_protocol`] over any byte stream.
//!
//! One task owns all the state. Client requests and the results of background work arrive on
//! its channel; after each batch it sends subscribers what changed.

mod agent_settings;
mod connection;
mod server;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use agentz_protocol::agents::AgentId;
use agentz_protocol::{MachineInfo, PROTOCOL_VERSION, ServerWelcome};
use anyhow::{Context as _, Result};
use futures::channel::mpsc;
use gpui_shared_string::SharedString;
use http_client::HttpClient;
use registry::{AgentCommand, ShellEnvironmentReady};
use tokio::io::{AsyncRead, AsyncWrite};

pub use agent_settings::AgentSettingsStore;

use crate::server::{Input, Server};

pub struct ServerConfig {
    /// Holds the projects, agent settings and installed agents.
    pub data_dir: PathBuf,
    pub version: String,
    pub http_client: Arc<dyn HttpClient>,
    pub shell_environment_ready: ShellEnvironmentReady,
    /// Agents started from a fixed command rather than from the registry.
    pub custom_agents: BTreeMap<AgentId, CustomAgent>,
}

#[derive(Clone, Debug)]
pub struct CustomAgent {
    pub name: SharedString,
    pub command: AgentCommand,
}

/// A running server. Cloning it is cheap.
#[derive(Clone)]
pub struct ServerHandle {
    runtime: tokio::runtime::Handle,
    inputs: mpsc::UnboundedSender<Input>,
    welcome: Arc<ServerWelcome>,
    next_client_id: Arc<AtomicU64>,
    stopped: tokio::sync::watch::Receiver<bool>,
}

/// Starts the server on the runtime.
pub fn start(runtime: tokio::runtime::Handle, config: ServerConfig) -> Result<ServerHandle> {
    let machine = machine_info(&config.data_dir)?;
    let welcome = ServerWelcome {
        protocol_version: PROTOCOL_VERSION,
        server_version: config.version.clone(),
        machine,
        pid: std::process::id(),
        capabilities: Vec::new(),
        error: None,
    };
    let (inputs, inbox) = mpsc::unbounded();
    let (stopped_sender, stopped) = tokio::sync::watch::channel(false);
    let server = {
        // The stores spawn their background work onto the runtime as they're created.
        let _guard = runtime.enter();
        Server::new(runtime.clone(), config, inputs.clone())
    };
    runtime.spawn(async move {
        server.run(inbox).await;
        stopped_sender.send_replace(true);
    });
    Ok(ServerHandle {
        runtime,
        inputs,
        welcome: Arc::new(welcome),
        next_client_id: Arc::new(AtomicU64::new(1)),
        stopped,
    })
}

impl ServerHandle {
    pub fn machine(&self) -> &MachineInfo {
        &self.welcome.machine
    }

    /// Serves one client on the stream until either side closes it.
    pub fn serve(&self, stream: impl AsyncRead + AsyncWrite + Send + 'static) {
        let client = self.next_client_id.fetch_add(1, Ordering::Relaxed);
        let welcome = ServerWelcome::clone(&self.welcome);
        let inputs = self.inputs.clone();
        self.runtime.spawn(async move {
            if let Err(error) = connection::serve(stream, client, welcome, inputs).await {
                log::warn!("client {client}: {error:#}");
            }
        });
    }

    /// Stops the agents, saves, and ends the server.
    pub fn shut_down(&self) {
        self.inputs.unbounded_send(Input::Shutdown).ok();
    }

    /// Resolves once the server has shut down.
    pub async fn stopped(&self) {
        let mut stopped = self.stopped.clone();
        if stopped.wait_for(|stopped| *stopped).await.is_err() {
            log::error!("the server ended without saying so");
        }
    }
}

/// Identifies this machine to clients, however they reach it.
fn machine_info(data_dir: &Path) -> Result<MachineInfo> {
    let id_path = data_dir.join("machine-id");
    let id = match std::fs::read_to_string(&id_path) {
        Ok(id) if !id.trim().is_empty() => id.trim().to_string(),
        Ok(_) => write_machine_id(&id_path)?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => write_machine_id(&id_path)?,
        Err(error) => {
            return Err(error).with_context(|| format!("reading {}", id_path.display()));
        }
    };
    Ok(MachineInfo {
        id,
        hostname: hostname(),
        os: std::env::consts::OS.to_string(),
        arch: std::env::consts::ARCH.to_string(),
    })
}

fn write_machine_id(path: &Path) -> Result<String> {
    let id = uuid::Uuid::new_v4().to_string();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("creating {}", parent.display()))?;
    }
    std::fs::write(path, &id).with_context(|| format!("writing {}", path.display()))?;
    Ok(id)
}

fn hostname() -> String {
    #[cfg(unix)]
    match nix::unistd::gethostname() {
        Ok(name) => return name.to_string_lossy().into_owned(),
        Err(error) => log::error!("failed to read the hostname: {error}"),
    }
    std::env::var("COMPUTERNAME").unwrap_or_default()
}

#[cfg(test)]
mod tests;
