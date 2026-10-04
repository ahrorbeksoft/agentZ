//! The agentZ server: it owns the projects, the agent registry, agent settings and the agents
//! themselves, so threads keep working while no app is connected (herdr's model). Clients talk
//! to it in [`agentz_protocol`] over any byte stream.
//!
//! One task owns all the state. Client requests and the results of background work arrive on
//! its channel; after each batch it sends subscribers what changed.

mod agent_settings;
pub mod browser;
mod checkpoints;
mod connection;
mod continuations;
mod detect;
mod directories;
mod git;
#[cfg(unix)]
pub mod handoff;
mod machine_kind;
mod repositories;
mod server;
mod spaces;
mod terminal_programs;
mod terminals;
mod workspaces;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use agentz_protocol::agents::AgentId;
use agentz_protocol::{MachineInfo, PROTOCOL_VERSION, ServerWelcome};
use anyhow::{Context as _, Result};
use futures::channel::mpsc;
use gpui_shared_string::SharedString;
use http_client::HttpClient;
use registry::{AgentCommand, ShellEnvironmentReady};
use serde::{Deserialize, Serialize};
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
    /// How agents reach the agent-control tools. `None` leaves them out.
    pub agent_control: Option<AgentControl>,
    /// Agents' login pages go to the clients ([`browser`]), which needs `agent_control`.
    pub hands_pages_to_clients: bool,
    /// The shell terminals run, and run commands with (`-c`). `None` is the user's login
    /// shell, which tests avoid since it reads the user's own setup.
    pub terminal_shell: Option<String>,
    /// The socket clients connect to, handed with the terminals to a newer server
    /// ([`agentz_protocol::Request::HandOff`]). `None` can't hand off.
    #[cfg(unix)]
    pub listener: Option<std::os::fd::RawFd>,
    /// The terminals the server before this one handed over.
    #[cfg(unix)]
    pub handed_over: Option<handoff::HandedOver>,
}

/// What agents are given to manage threads: the `agentz` MCP server in every session, and the
/// environment for the CLI.
#[derive(Clone, Debug)]
pub struct AgentControl {
    /// The `agentz-server` binary, run as `mcp-bridge` and `call`.
    pub executable: PathBuf,
    /// The socket the bridge and the CLI connect to.
    pub socket: PathBuf,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CustomAgent {
    pub name: SharedString,
    pub command: AgentCommand,
    /// What the agent said it is (ACP's `agentInfo`) when it was last saved from Settings.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub info: Option<agent_client_protocol::schema::v1::Implementation>,
}

/// A running server. Cloning it is cheap.
#[derive(Clone)]
pub struct ServerHandle {
    runtime: tokio::runtime::Handle,
    inputs: mpsc::UnboundedSender<Input>,
    welcome: Arc<ServerWelcome>,
    next_client_id: Arc<AtomicU64>,
    stopped: tokio::sync::watch::Receiver<bool>,
    handed_off: Arc<AtomicBool>,
}

/// Reads `agents/custom.json`: agent ids, each with a `name` and a `command` (`path`, `args`,
/// `env`).
pub fn load_custom_agents(data_dir: &Path) -> Result<BTreeMap<AgentId, CustomAgent>> {
    let path = custom_agents_path(data_dir);
    match std::fs::read(&path) {
        Ok(bytes) => {
            serde_json::from_slice(&bytes).with_context(|| format!("parsing {}", path.display()))
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(BTreeMap::new()),
        Err(error) => Err(error).with_context(|| format!("reading {}", path.display())),
    }
}

fn custom_agents_path(data_dir: &Path) -> PathBuf {
    data_dir.join("agents").join("custom.json")
}

/// The hash the SSH installer wrote beside this binary. Read at start, because a newer
/// install replaces the file while this server keeps running.
fn installed_build() -> Option<String> {
    let executable = std::env::current_exe().ok()?;
    let hash = std::fs::read_to_string(executable.with_file_name("agentz-server.sha256")).ok()?;
    Some(hash.trim().to_string()).filter(|hash| !hash.is_empty())
}

/// When this binary was last modified, read at start for the same reason.
fn binary_modified() -> Option<u64> {
    let modified = std::fs::metadata(std::env::current_exe().ok()?)
        .ok()?
        .modified()
        .ok()?;
    let since_epoch = modified.duration_since(std::time::UNIX_EPOCH).ok()?;
    u64::try_from(since_epoch.as_millis()).ok()
}

/// Starts the server on the runtime.
pub fn start(runtime: tokio::runtime::Handle, config: ServerConfig) -> Result<ServerHandle> {
    let machine = machine_info(&config.data_dir)?;
    #[cfg(unix)]
    let can_hand_off = config.listener.is_some() && config.agent_control.is_some();
    #[cfg(not(unix))]
    let can_hand_off = false;
    let welcome = ServerWelcome {
        protocol_version: PROTOCOL_VERSION,
        server_version: config.version.clone(),
        machine,
        pid: std::process::id(),
        capabilities: vec![
            agentz_protocol::CAPABILITY_THREAD_DIFF.to_string(),
            agentz_protocol::CAPABILITY_WORKSPACES.to_string(),
            agentz_protocol::CAPABILITY_TERMINALS.to_string(),
            agentz_protocol::CAPABILITY_BROWSE_DIRECTORIES.to_string(),
            agentz_protocol::CAPABILITY_RELAY.to_string(),
            agentz_protocol::CAPABILITY_SPACES.to_string(),
            agentz_protocol::CAPABILITY_MACHINE_ICON.to_string(),
            agentz_protocol::CAPABILITY_DRAWER_TERMINALS.to_string(),
            agentz_protocol::CAPABILITY_IMPORT_SESSIONS.to_string(),
        ]
        .into_iter()
        .chain(can_hand_off.then(|| agentz_protocol::CAPABILITY_HAND_OFF.to_string()))
        .collect(),
        build: installed_build(),
        binary_modified: binary_modified(),
        error: None,
    };
    let (inputs, inbox) = mpsc::unbounded();
    let (stopped_sender, stopped) = tokio::sync::watch::channel(false);
    let handed_off = Arc::new(AtomicBool::new(false));
    let server = {
        // The stores spawn their background work onto the runtime as they're created.
        let _guard = runtime.enter();
        Server::new(
            runtime.clone(),
            config,
            welcome.machine.clone(),
            inputs.clone(),
            handed_off.clone(),
        )
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
        handed_off,
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

    /// Whether the server stopped because it handed its terminals and socket to a newer one,
    /// which listens on the socket now.
    pub fn handed_off(&self) -> bool {
        self.handed_off.load(Ordering::Acquire)
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
