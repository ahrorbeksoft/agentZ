//! The ACP agent registry: fetching the list of available agents, installing them, and
//! building the command that launches an installed agent.
//!
//! Ported from Zed's `project::agent_registry_store` and the registry parts of
//! `project::agent_server_store`, without Zed's settings and fs layers. npx agents run with
//! the system's Node.js, or one downloaded for them ([`node_runtime`]).
//!
//! Plain Rust on tokio, so the server can own it. Background work reports back as
//! [`RegistryMessage`]s, which the store's owner passes to [`AgentRegistryStore::handle`].

mod node_runtime;

use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{Context as _, Result, anyhow, bail};
use collections::{HashMap, HashSet};
use futures::AsyncReadExt as _;
use futures::channel::{mpsc, oneshot};
use futures::future::{BoxFuture, FutureExt as _, Shared, join_all};
use gpui_shared_string::SharedString;
use http_client::github::AssetKind;
use http_client::{AsyncBody, HttpClient, StatusCode};
use percent_encoding::percent_decode_str;
use serde::Deserialize;
use sha2::{Digest as _, Sha256};
use tokio::task::JoinSet;
use url::Url;
use util::ResultExt as _;

use crate::node_runtime::{Node, NodeRuntime};

pub use agentz_protocol::agents::{
    AgentCommand, AgentIcon, AgentId, AgentListing, IconId, InstallState, RegistryAgentMetadata,
    RegistrySnapshot,
};

const REGISTRY_URL: &str = "https://cdn.agentclientprotocol.com/registry/v1/latest/registry.json";
const REFRESH_THROTTLE_DURATION: Duration = Duration::from_secs(60 * 60);
// Bounds the whole request including the body; the HTTP client only has a connect timeout.
const REGISTRY_FETCH_TIMEOUT: Duration = Duration::from_secs(30);
const REGISTRY_ICON_FETCH_TIMEOUT: Duration = Duration::from_secs(10);
const NPX_DIR_NAME: &str = "npx";

#[derive(Clone, Debug)]
pub struct RegistryBinaryAgent {
    pub metadata: RegistryAgentMetadata,
    pub icon: Option<AgentIcon>,
    pub targets: HashMap<String, RegistryTargetConfig>,
    pub supports_current_platform: bool,
}

#[derive(Clone, Debug)]
pub struct RegistryNpxAgent {
    pub metadata: RegistryAgentMetadata,
    pub icon: Option<AgentIcon>,
    pub package: SharedString,
    pub args: Vec<String>,
    pub env: HashMap<String, String>,
}

#[derive(Clone, Debug)]
pub enum RegistryAgent {
    Binary(RegistryBinaryAgent),
    Npx(RegistryNpxAgent),
}

impl RegistryAgent {
    pub fn metadata(&self) -> &RegistryAgentMetadata {
        match self {
            RegistryAgent::Binary(agent) => &agent.metadata,
            RegistryAgent::Npx(agent) => &agent.metadata,
        }
    }

    pub fn id(&self) -> &AgentId {
        &self.metadata().id
    }

    pub fn name(&self) -> &SharedString {
        &self.metadata().name
    }

    pub fn description(&self) -> &SharedString {
        &self.metadata().description
    }

    pub fn version(&self) -> &SharedString {
        &self.metadata().version
    }

    pub fn icon(&self) -> Option<&AgentIcon> {
        match self {
            RegistryAgent::Binary(agent) => agent.icon.as_ref(),
            RegistryAgent::Npx(agent) => agent.icon.as_ref(),
        }
    }

    pub fn supports_current_platform(&self) -> bool {
        match self {
            RegistryAgent::Binary(agent) => agent.supports_current_platform,
            RegistryAgent::Npx(_) => true,
        }
    }
}

#[derive(Clone, Debug)]
pub struct RegistryTargetConfig {
    pub archive: String,
    pub cmd: String,
    pub args: Vec<String>,
    pub sha256: Option<String>,
    pub env: HashMap<String, String>,
}

/// Resolves once the user's login-shell environment (and with it `PATH`) has been loaded, so
/// `npm` and `node` can be found when the app was started from Finder.
pub type ShellEnvironmentReady = Shared<BoxFuture<'static, ()>>;

/// Resolves to the command that starts an agent.
pub type CommandFuture = BoxFuture<'static, Result<AgentCommand>>;

/// The result of background work, for [`AgentRegistryStore::handle`].
pub struct RegistryMessage(MessageKind);

pub type RegistryInbox = mpsc::UnboundedReceiver<RegistryMessage>;

enum MessageKind {
    CacheLoaded(Result<(Option<Vec<RegistryAgent>>, HashMap<AgentId, SharedString>)>),
    Refreshed {
        agents: Result<Vec<RegistryAgent>>,
        installed_versions: HashMap<AgentId, SharedString>,
    },
    Installed {
        id: AgentId,
        result: Result<()>,
        installed_versions: HashMap<AgentId, SharedString>,
    },
    Uninstalled(HashMap<AgentId, SharedString>),
}

pub struct AgentRegistryStore {
    runtime: tokio::runtime::Handle,
    http_client: Arc<dyn HttpClient>,
    shell_environment_ready: ShellEnvironmentReady,
    registry_dir: PathBuf,
    node_runtime: NodeRuntime,
    agents: Vec<RegistryAgent>,
    installed_versions: HashMap<AgentId, SharedString>,
    installing: HashSet<AgentId>,
    install_errors: HashMap<AgentId, SharedString>,
    is_fetching: bool,
    fetch_error: Option<SharedString>,
    last_refresh: Option<Instant>,
    /// Set once the agent list is known, from the cache or the network.
    has_loaded: bool,
    loaded_waiters: Vec<(AgentId, oneshot::Sender<CommandFuture>)>,
    messages: mpsc::UnboundedSender<RegistryMessage>,
    /// Dropping the store cancels its background work.
    tasks: JoinSet<()>,
}

impl AgentRegistryStore {
    /// Creates the store and starts loading the cached registry. Pass what arrives on the
    /// returned inbox to [`Self::handle`]. Node.js is downloaded into `node_dir` when an npm
    /// agent needs it and the machine has none.
    pub fn new(
        runtime: tokio::runtime::Handle,
        http_client: Arc<dyn HttpClient>,
        shell_environment_ready: ShellEnvironmentReady,
        registry_dir: PathBuf,
        node_dir: PathBuf,
    ) -> (Self, RegistryInbox) {
        let (messages, inbox) = mpsc::unbounded();
        let node_runtime = NodeRuntime::new(node_dir, http_client.clone());
        let mut store = Self {
            runtime,
            http_client,
            shell_environment_ready,
            registry_dir,
            node_runtime,
            agents: Vec::new(),
            installed_versions: HashMap::default(),
            installing: HashSet::default(),
            install_errors: HashMap::default(),
            is_fetching: false,
            fetch_error: None,
            last_refresh: None,
            has_loaded: false,
            loaded_waiters: Vec::new(),
            messages,
            tasks: JoinSet::new(),
        };
        store.load_cached_registry();
        (store, inbox)
    }

    pub fn agents(&self) -> &[RegistryAgent] {
        &self.agents
    }

    pub fn agent(&self, id: &AgentId) -> Option<&RegistryAgent> {
        self.agents.iter().find(|agent| agent.id() == id)
    }

    pub fn is_fetching(&self) -> bool {
        self.is_fetching
    }

    pub fn icon(&self, id: &IconId) -> Option<&AgentIcon> {
        self.agents
            .iter()
            .filter_map(RegistryAgent::icon)
            .find(|icon| &icon.id == id)
    }

    pub fn fetch_error(&self) -> Option<SharedString> {
        self.fetch_error.clone()
    }

    pub fn install_state(&self, id: &AgentId) -> InstallState {
        if self.installing.contains(id) {
            return InstallState::Installing;
        }
        if let Some(version) = self.installed_versions.get(id) {
            let update_available = self
                .agent(id)
                .is_some_and(|agent| agent.version() != version);
            return InstallState::Installed {
                version: version.clone(),
                update_available,
            };
        }
        if let Some(error) = self.install_errors.get(id) {
            return InstallState::Failed(error.clone());
        }
        InstallState::NotInstalled
    }

    /// The registry as clients see it.
    pub fn snapshot(&self) -> RegistrySnapshot {
        RegistrySnapshot {
            agents: self
                .agents
                .iter()
                .map(|agent| AgentListing {
                    metadata: agent.metadata().clone(),
                    supports_current_platform: agent.supports_current_platform(),
                    install_state: self.install_state(agent.id()),
                    custom_command: None,
                })
                .collect(),
            is_fetching: self.is_fetching,
            fetch_error: self.fetch_error.clone(),
        }
    }

    /// Fetches the latest registry from the network and updates the cache.
    pub fn refresh(&mut self) {
        if self.is_fetching {
            return;
        }

        self.is_fetching = true;
        self.fetch_error = None;
        self.last_refresh = Some(Instant::now());

        let http_client = self.http_client.clone();
        let registry_dir = self.registry_dir.clone();
        self.spawn(async move {
            let agents = match fetch_registry_index(http_client.clone()).await {
                Ok(data) => {
                    build_registry_agents(
                        http_client,
                        registry_dir.clone(),
                        data.index,
                        Some(data.raw_body),
                    )
                    .await
                }
                Err(error) => {
                    log::error!("failed to fetch the ACP registry: {error:#}");
                    Err(error)
                }
            };
            let installed_versions = scan_installed_versions_in_background(registry_dir).await;
            MessageKind::Refreshed {
                agents,
                installed_versions,
            }
        });
    }

    /// Refreshes at most once an hour.
    pub fn refresh_if_stale(&mut self) {
        let should_refresh = self
            .last_refresh
            .map(|last| last.elapsed() >= REFRESH_THROTTLE_DURATION)
            .unwrap_or(true);
        if should_refresh {
            self.refresh();
        }
    }

    /// Installs (or updates to) the registry's current version of the agent.
    pub fn install(&mut self, id: &AgentId) {
        if self.installing.contains(id) {
            return;
        }
        let Some(agent) = self.agent(id).cloned() else {
            return;
        };
        self.install_errors.remove(id);
        self.installing.insert(id.clone());

        let http_client = self.http_client.clone();
        let registry_dir = self.registry_dir.clone();
        let shell_environment_ready = self.shell_environment_ready.clone();
        let node_runtime = self.node_runtime.clone();
        let id = id.clone();
        self.spawn(async move {
            shell_environment_ready.await;
            let result =
                install_agent(&agent, &registry_dir, http_client.as_ref(), &node_runtime).await;
            let installed_versions = scan_installed_versions_in_background(registry_dir).await;
            MessageKind::Installed {
                id,
                result,
                installed_versions,
            }
        });
    }

    /// Deletes an installed agent's files.
    pub fn uninstall(&mut self, id: &AgentId) {
        if self.installing.contains(id) || !self.installed_versions.contains_key(id) {
            return;
        }
        let registry_dir = self.registry_dir.clone();
        let id = id.clone();
        self.spawn(async move {
            let installed_versions = tokio::task::spawn_blocking(move || {
                // Binary agents live in `<id>/<version>`, npx agents in `npx/<id>`.
                for dir in [
                    registry_dir.join(&*id.0),
                    registry_dir.join(NPX_DIR_NAME).join(&*id.0),
                ] {
                    if dir.exists() {
                        std::fs::remove_dir_all(&dir)
                            .with_context(|| format!("removing {}", dir.display()))
                            .log_err();
                    }
                }
                scan_installed_versions(&registry_dir)
            })
            .await
            .unwrap_or_default();
            MessageKind::Uninstalled(installed_versions)
        });
    }

    /// Builds the command that starts an installed agent.
    pub fn command(&self, id: &AgentId) -> CommandFuture {
        let Some(agent) = self.agent(id).cloned() else {
            return futures::future::ready(Err(anyhow!("agent {id} is not in the registry")))
                .boxed();
        };
        let Some(installed_version) = self.installed_versions.get(id).cloned() else {
            return futures::future::ready(Err(anyhow!("agent {id} is not installed"))).boxed();
        };
        let registry_dir = self.registry_dir.clone();
        let shell_environment_ready = self.shell_environment_ready.clone();
        let node_runtime = self.node_runtime.clone();
        async move {
            shell_environment_ready.await;
            agent_command(&agent, &installed_version, &registry_dir, &node_runtime).await
        }
        .boxed()
    }

    /// Like [`Self::command`], but waits until the agent list has loaded, so threads opened
    /// right after launch can still find their agent.
    pub fn command_when_loaded(&mut self, id: &AgentId) -> CommandFuture {
        if self.has_loaded {
            return self.command(id);
        }
        let (sender, receiver) = oneshot::channel();
        self.loaded_waiters.push((id.clone(), sender));
        async move {
            let command = receiver
                .await
                .map_err(|_| anyhow!("the agent registry closed before loading"))?;
            command.await
        }
        .boxed()
    }

    /// Applies the result of background work.
    pub fn handle(&mut self, message: RegistryMessage) {
        match message.0 {
            MessageKind::CacheLoaded(Err(error)) => {
                log::error!("failed to load the cached registry: {error:#}")
            }
            MessageKind::CacheLoaded(Ok((agents, installed_versions))) => {
                // A network refresh may have finished first; its list is newer.
                if let Some(agents) = agents
                    && self.agents.is_empty()
                {
                    self.agents = agents;
                }
                self.set_installed_versions(installed_versions);
                if !self.agents.is_empty() {
                    self.mark_loaded();
                }
            }
            MessageKind::Refreshed {
                agents,
                installed_versions,
            } => {
                self.is_fetching = false;
                match agents {
                    Ok(agents) => {
                        self.agents = agents;
                        self.fetch_error = None;
                    }
                    Err(error) => {
                        self.fetch_error = Some(SharedString::from(format!("{error:#}")));
                    }
                }
                self.set_installed_versions(installed_versions);
                self.mark_loaded();
            }
            MessageKind::Installed {
                id,
                result,
                installed_versions,
            } => {
                self.installing.remove(&id);
                if let Err(error) = result {
                    log::error!("failed to install agent {id}: {error:#}");
                    self.install_errors
                        .insert(id, SharedString::from(format!("{error:#}")));
                }
                self.set_installed_versions(installed_versions);
            }
            MessageKind::Uninstalled(installed_versions) => {
                self.set_installed_versions(installed_versions)
            }
        }
    }

    fn spawn(&mut self, work: impl Future<Output = MessageKind> + Send + 'static) {
        while self.tasks.try_join_next().is_some() {}
        let messages = self.messages.clone();
        self.tasks.spawn_on(
            async move {
                let message = work.await;
                messages.unbounded_send(RegistryMessage(message)).ok();
            },
            &self.runtime,
        );
    }

    fn mark_loaded(&mut self) {
        self.has_loaded = true;
        for (id, waiter) in std::mem::take(&mut self.loaded_waiters) {
            waiter.send(self.command(&id)).ok();
        }
    }

    fn set_installed_versions(&mut self, installed_versions: HashMap<AgentId, SharedString>) {
        for id in installed_versions.keys() {
            self.install_errors.remove(id);
        }
        self.installed_versions = installed_versions;
    }

    fn load_cached_registry(&mut self) {
        let http_client = self.http_client.clone();
        let registry_dir = self.registry_dir.clone();
        self.spawn(async move {
            let result = async {
                let (bytes, installed_versions) = {
                    let registry_dir = registry_dir.clone();
                    tokio::task::spawn_blocking(move || {
                        let cache_path = registry_dir.join("registry.json");
                        let bytes = match std::fs::read(&cache_path) {
                            Ok(bytes) => Some(bytes),
                            Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
                            Err(error) => {
                                log::error!("reading {}: {error}", cache_path.display());
                                None
                            }
                        };
                        (bytes, scan_installed_versions(&registry_dir))
                    })
                    .await?
                };
                let agents = match bytes {
                    Some(bytes) => {
                        let index: RegistryIndex =
                            serde_json::from_slice(&bytes).context("parsing cached registry")?;
                        Some(build_registry_agents(http_client, registry_dir, index, None).await?)
                    }
                    None => None,
                };
                anyhow::Ok((agents, installed_versions))
            }
            .await;
            MessageKind::CacheLoaded(result)
        });
    }
}

async fn scan_installed_versions_in_background(
    registry_dir: PathBuf,
) -> HashMap<AgentId, SharedString> {
    tokio::task::spawn_blocking(move || scan_installed_versions(&registry_dir))
        .await
        .unwrap_or_default()
}

struct RegistryFetchResult {
    index: RegistryIndex,
    raw_body: Vec<u8>,
}

async fn fetch_registry_index(http_client: Arc<dyn HttpClient>) -> Result<RegistryFetchResult> {
    let (status, body) = fetch_url_body(http_client, REGISTRY_URL, REGISTRY_FETCH_TIMEOUT)
        .await
        .context("fetching ACP registry")?;

    if !status.is_success() {
        let text = String::from_utf8_lossy(body.as_slice());
        bail!(
            "registry status error {}, response: {text:?}",
            status.as_u16()
        );
    }

    let index: RegistryIndex = serde_json::from_slice(&body).context("parsing ACP registry")?;
    Ok(RegistryFetchResult {
        index,
        raw_body: body,
    })
}

/// Turns the registry index into agents, caching the index and downloading missing icons
/// when `raw_body` (a freshly fetched index) is given.
async fn build_registry_agents(
    http_client: Arc<dyn HttpClient>,
    registry_dir: PathBuf,
    index: RegistryIndex,
    raw_body: Option<Vec<u8>>,
) -> Result<Vec<RegistryAgent>> {
    let icons_dir = registry_dir.join("icons");
    let update_cache = raw_body.is_some();
    if let Some(raw_body) = raw_body {
        let registry_dir = registry_dir.clone();
        tokio::task::spawn_blocking(move || {
            std::fs::create_dir_all(registry_dir.join("icons"))
                .with_context(|| format!("creating {}", registry_dir.display()))?;
            std::fs::write(registry_dir.join("registry.json"), raw_body)
                .context("writing registry cache")
        })
        .await??;
    }

    let icons = join_all(index.agents.iter().map(|entry| {
        let http_client = http_client.clone();
        let icons_dir = icons_dir.clone();
        async move {
            resolve_icon(entry, &icons_dir, update_cache, http_client)
                .await
                .log_err()
                .flatten()
        }
    }))
    .await;

    Ok(registry_agents_from_index(index, icons))
}

fn registry_agents_from_index(
    index: RegistryIndex,
    icons: Vec<Option<AgentIcon>>,
) -> Vec<RegistryAgent> {
    let current_platform = current_platform_key();
    let mut agents = Vec::new();
    for (entry, icon) in index.agents.into_iter().zip(icons) {
        let metadata = RegistryAgentMetadata {
            id: AgentId::new(entry.id),
            name: entry.name.into(),
            description: entry.description.into(),
            version: entry.version.into(),
            repository: entry.repository.map(Into::into),
            website: entry.website.map(Into::into),
            license_url: entry.license_url.map(Into::into),
            icon: icon.as_ref().map(|icon| icon.id.clone()),
        };

        let binary_agent = entry.distribution.binary.as_ref().and_then(|binary| {
            if binary.is_empty() {
                return None;
            }

            let mut targets = HashMap::default();
            for (platform, target) in binary.iter() {
                targets.insert(
                    platform.clone(),
                    RegistryTargetConfig {
                        archive: target.archive.clone(),
                        cmd: target.cmd.clone(),
                        args: target.args.clone(),
                        sha256: target.sha256.clone(),
                        env: target.env.clone(),
                    },
                );
            }

            let supports_current_platform =
                current_platform.is_some_and(|platform| targets.contains_key(platform));

            Some(RegistryBinaryAgent {
                metadata: metadata.clone(),
                icon: icon.clone(),
                targets,
                supports_current_platform,
            })
        });

        let npx_agent = entry.distribution.npx.as_ref().map(|npx| RegistryNpxAgent {
            metadata: metadata.clone(),
            icon: icon.clone(),
            package: npx.package.clone().into(),
            args: npx.args.clone(),
            env: npx.env.clone(),
        });

        let agent = match (binary_agent, npx_agent) {
            (Some(binary_agent), Some(npx_agent)) => {
                if binary_agent.supports_current_platform {
                    RegistryAgent::Binary(binary_agent)
                } else {
                    RegistryAgent::Npx(npx_agent)
                }
            }
            (Some(binary_agent), None) => RegistryAgent::Binary(binary_agent),
            (None, Some(npx_agent)) => RegistryAgent::Npx(npx_agent),
            (None, None) => continue,
        };

        agents.push(agent);
    }
    agents
}

/// The agent's icon, downloaded into `icons_dir` once.
async fn resolve_icon(
    entry: &RegistryEntry,
    icons_dir: &Path,
    update_cache: bool,
    http_client: Arc<dyn HttpClient>,
) -> Result<Option<AgentIcon>> {
    let Some(icon_url) = resolve_icon_url(entry) else {
        return Ok(None);
    };

    let icon_path = icons_dir.join(format!("{}.svg", sanitize_path_component(&entry.id)));
    if update_cache && !icon_path.is_file() {
        let download = async {
            let (status, body) =
                fetch_url_body(http_client, &icon_url, REGISTRY_ICON_FETCH_TIMEOUT).await?;
            if !status.is_success() {
                bail!("icon status error {}", status.as_u16());
            }
            std::fs::write(&icon_path, &body)
                .with_context(|| format!("writing {}", icon_path.display()))
        };
        if let Err(error) = download.await {
            log::warn!(
                "failed to download ACP registry icon for {}: {error:#}",
                entry.id
            );
        }
    }

    if !icon_path.is_file() {
        return Ok(None);
    }
    let svg = std::fs::read_to_string(&icon_path)
        .with_context(|| format!("reading {}", icon_path.display()))?;
    Ok(Some(agent_icon(svg)))
}

async fn fetch_url_body(
    http_client: Arc<dyn HttpClient>,
    url: &str,
    timeout: Duration,
) -> Result<(StatusCode, Vec<u8>)> {
    let request = async {
        let mut response = http_client
            .get(url, AsyncBody::default(), true)
            .await
            .with_context(|| format!("requesting {url}"))?;

        let status = response.status();
        let mut body = Vec::new();
        response
            .body_mut()
            .read_to_end(&mut body)
            .await
            .with_context(|| format!("reading response from {url}"))?;

        Ok((status, body))
    };
    tokio::time::timeout(timeout, request).await.map_err(|_| {
        anyhow!(
            "timed out after {}s while fetching {url}",
            timeout.as_secs()
        )
    })?
}

/// The icon with its id, a hash of the SVG, so machines that have the same icon name it alike.
pub fn agent_icon(svg: String) -> AgentIcon {
    let hash = Sha256::digest(svg.as_bytes());
    AgentIcon {
        id: IconId(hex::encode(hash).into()),
        svg: svg.into(),
    }
}

fn resolve_icon_url(entry: &RegistryEntry) -> Option<String> {
    let icon = entry.icon.as_ref()?;
    if icon.starts_with("https://") || icon.starts_with("http://") {
        return Some(icon.to_string());
    }

    let relative_icon = icon.trim_start_matches("./");
    Some(format!(
        "https://raw.githubusercontent.com/agentclientprotocol/registry/main/{}/{relative_icon}",
        entry.id
    ))
}

fn current_platform_key() -> Option<&'static str> {
    Some(match (std::env::consts::OS, std::env::consts::ARCH) {
        ("macos", "aarch64") => "darwin-aarch64",
        ("macos", "x86_64") => "darwin-x86_64",
        ("linux", "aarch64") => "linux-aarch64",
        ("linux", "x86_64") => "linux-x86_64",
        ("windows", "aarch64") => "windows-aarch64",
        ("windows", "x86_64") => "windows-x86_64",
        _ => return None,
    })
}

fn sanitize_path_component(input: &str) -> String {
    let sanitized = input
        .chars()
        .map(|character| match character {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '.' | '_' | '-' => character,
            _ => '-',
        })
        .collect::<String>();

    if sanitized.is_empty() || sanitized == "." || sanitized == ".." {
        "unknown".to_string()
    } else {
        sanitized
    }
}

fn binary_agent_dir(registry_dir: &Path, id: &AgentId) -> PathBuf {
    registry_dir.join(sanitize_path_component(&id.0))
}

fn npx_agent_dir(registry_dir: &Path, id: &AgentId) -> PathBuf {
    registry_dir
        .join(NPX_DIR_NAME)
        .join(sanitize_path_component(&id.0))
}

/// Finds the installed version of every agent by looking at what is on disk.
///
/// Binary agents live in `<registry>/<id>/<version>/`; npx agents in
/// `<registry>/npx/<id>/` with the package's own `package.json` recording the version.
fn scan_installed_versions(registry_dir: &Path) -> HashMap<AgentId, SharedString> {
    let mut installed = HashMap::default();
    let Ok(entries) = std::fs::read_dir(registry_dir) else {
        return installed;
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        let path = entry.path();
        if !path.is_dir() || name == "icons" {
            continue;
        }
        if name == NPX_DIR_NAME {
            let Ok(npx_entries) = std::fs::read_dir(&path) else {
                continue;
            };
            for npx_entry in npx_entries.flatten() {
                let id = npx_entry.file_name().to_string_lossy().into_owned();
                if let Some(version) = installed_npx_version(&npx_entry.path()) {
                    installed.insert(AgentId::new(id), version.into());
                }
            }
            continue;
        }
        let newest_version = std::fs::read_dir(&path)
            .into_iter()
            .flatten()
            .flatten()
            .filter(|version_entry| version_entry.path().is_dir())
            .filter_map(|version_entry| {
                let modified = version_entry.metadata().ok()?.modified().ok()?;
                let version = version_entry.file_name().to_string_lossy().into_owned();
                (!version.starts_with('.')).then_some((modified, version))
            })
            .max();
        if let Some((_, version)) = newest_version {
            installed.insert(AgentId::new(name), version.into());
        }
    }
    installed
}

fn installed_npx_version(install_dir: &Path) -> Option<String> {
    let manifest = std::fs::read(install_dir.join("package.json")).ok()?;
    let manifest: serde_json::Value = serde_json::from_slice(&manifest).ok()?;
    let (package_name, _) = manifest.get("dependencies")?.as_object()?.iter().next()?;
    let package_manifest = std::fs::read(
        install_dir
            .join("node_modules")
            .join(package_name)
            .join("package.json"),
    )
    .ok()?;
    let package_manifest: serde_json::Value = serde_json::from_slice(&package_manifest).ok()?;
    Some(package_manifest.get("version")?.as_str()?.to_string())
}

async fn install_agent(
    agent: &RegistryAgent,
    registry_dir: &Path,
    http_client: &dyn HttpClient,
    node_runtime: &NodeRuntime,
) -> Result<()> {
    match agent {
        RegistryAgent::Binary(agent) => {
            install_binary_agent(agent, registry_dir, http_client).await
        }
        RegistryAgent::Npx(agent) => install_npx_agent(agent, registry_dir, node_runtime).await,
    }
}

async fn install_binary_agent(
    agent: &RegistryBinaryAgent,
    registry_dir: &Path,
    http_client: &dyn HttpClient,
) -> Result<()> {
    let target = current_target(agent)?;
    let agent_dir = binary_agent_dir(registry_dir, &agent.metadata.id);
    let version_dir = agent_dir.join(sanitize_path_component(&agent.metadata.version));
    if version_dir.is_dir() {
        return Ok(());
    }
    std::fs::create_dir_all(&agent_dir)
        .with_context(|| format!("creating {}", agent_dir.display()))?;

    let archive_url = &target.archive;
    match registry_archive_kind_for_url(archive_url)? {
        RegistryArchiveKind::Archive(asset_kind) => {
            http_client::github_download::download_server_binary(
                http_client,
                archive_url,
                target.sha256.as_deref(),
                &version_dir,
                asset_kind,
            )
            .await?;
        }
        RegistryArchiveKind::RawBinary { file_name } => {
            http_client::github_download::download_server_raw_binary(
                http_client,
                archive_url,
                target.sha256.as_deref(),
                &version_dir,
                &file_name,
            )
            .await?;
        }
    }

    let command_path = binary_command_path(&version_dir, &target.cmd)?;
    anyhow::ensure!(
        command_path.is_file(),
        "missing command {} after extraction",
        command_path.display()
    );

    remove_other_versions(&agent_dir, &version_dir).log_err();
    Ok(())
}

fn current_target(agent: &RegistryBinaryAgent) -> Result<&RegistryTargetConfig> {
    let platform_key = current_platform_key().context("unsupported platform")?;
    agent.targets.get(platform_key).with_context(|| {
        let mut available: Vec<_> = agent.targets.keys().map(String::as_str).collect();
        available.sort();
        format!(
            "{} has no build for {platform_key}. Available platforms: {}",
            agent.metadata.name,
            available.join(", ")
        )
    })
}

fn binary_command_path(version_dir: &Path, cmd: &str) -> Result<PathBuf> {
    anyhow::ensure!(
        !cmd.contains(".."),
        "command path cannot contain '..': {cmd}"
    );
    let relative = cmd
        .strip_prefix("./")
        .or_else(|| cmd.strip_prefix(".\\"))
        .with_context(|| format!("command must be relative (start with './'): {cmd}"))?;
    Ok(version_dir.join(relative))
}

fn remove_other_versions(agent_dir: &Path, current_version_dir: &Path) -> Result<()> {
    for entry in std::fs::read_dir(agent_dir)? {
        let path = entry?.path();
        if path != current_version_dir && path.is_dir() {
            std::fs::remove_dir_all(&path)
                .with_context(|| format!("removing old version {}", path.display()))?;
        }
    }
    Ok(())
}

async fn install_npx_agent(
    agent: &RegistryNpxAgent,
    registry_dir: &Path,
    node_runtime: &NodeRuntime,
) -> Result<()> {
    let node = node_runtime.node().await?;
    let install_dir = npx_agent_dir(registry_dir, &agent.metadata.id);
    std::fs::create_dir_all(&install_dir)
        .with_context(|| format!("creating {}", install_dir.display()))?;
    let manifest_path = install_dir.join("package.json");
    if !manifest_path.exists() {
        std::fs::write(&manifest_path, "{\"private\": true}\n")
            .with_context(|| format!("writing {}", manifest_path.display()))?;
    }
    install_npm_package(&node, &install_dir, &agent.package).await
}

/// Installs the registry's exact version, and the bounded range only when npm refuses that.
/// npm resolves a range to the `latest` dist-tag whenever `latest` satisfies it, so with the range
/// alone a package whose newest release sits under another tag (Grok's under `alpha`) never gets
/// past `latest`, and shows an update that installing doesn't bring.
async fn install_npm_package(node: &Node, install_dir: &Path, package: &str) -> Result<()> {
    let (_, bounded_spec) = bounded_npm_package_spec(package);
    let mut specs = vec![package.to_string()];
    if bounded_spec != package {
        specs.push(bounded_spec);
    }
    let mut last_error = None;
    for spec in specs {
        let output = node
            .npm(
                "install",
                &[spec.as_str(), "--save-exact", "--no-fund", "--no-audit"],
            )
            .current_dir(install_dir)
            .output()
            .await
            .context("running npm install")?;
        if output.status.success() {
            return Ok(());
        }
        let error = anyhow!(
            "npm install {spec} failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
        log::info!("{error:#}");
        last_error = Some(error);
    }
    Err(last_error.unwrap_or_else(|| anyhow!("nothing to install for {package}")))
}

async fn agent_command(
    agent: &RegistryAgent,
    installed_version: &str,
    registry_dir: &Path,
    node_runtime: &NodeRuntime,
) -> Result<AgentCommand> {
    match agent {
        RegistryAgent::Binary(agent) => {
            let target = current_target(agent)?;
            let version_dir = binary_agent_dir(registry_dir, &agent.metadata.id)
                .join(sanitize_path_component(installed_version));
            Ok(AgentCommand {
                path: binary_command_path(&version_dir, &target.cmd)?,
                args: target.args.clone(),
                env: target.env.clone(),
            })
        }
        RegistryAgent::Npx(agent) => {
            let (package_name, _) = bounded_npm_package_spec(&agent.package);
            let package_dir = npx_agent_dir(registry_dir, &agent.metadata.id)
                .join("node_modules")
                .join(package_name);
            let executable = read_package_executable(&package_dir)?;
            let node = node_runtime.node().await?;
            let mut env = node.env();
            env.extend(agent.env.clone());
            // Like npx, run the bin as what it is: some packages ship a native binary there.
            if !runs_with_node(&executable) {
                return Ok(AgentCommand {
                    path: executable,
                    args: agent.args.clone(),
                    env,
                });
            }
            let mut args = vec![executable.to_string_lossy().into_owned()];
            args.extend(agent.args.iter().cloned());
            Ok(AgentCommand {
                path: node.node_path(),
                args,
                env,
            })
        }
    }
}

/// Whether an npm bin is a Node script (a `node` shebang, or JavaScript without one) rather than
/// a native executable or a script for another interpreter.
fn runs_with_node(executable: &Path) -> bool {
    let Ok(mut file) = std::fs::File::open(executable) else {
        return true;
    };
    let mut head = [0u8; 256];
    let read = std::io::Read::read(&mut file, &mut head).unwrap_or(0);
    let head = &head[..read];
    if let Some(shebang) = head.strip_prefix(b"#!") {
        let line_end = shebang
            .iter()
            .position(|byte| *byte == b'\n')
            .unwrap_or(shebang.len());
        return String::from_utf8_lossy(&shebang[..line_end]).contains("node");
    }
    const NATIVE_MAGIC: [[u8; 4]; 6] = [
        [0x7f, b'E', b'L', b'F'],
        [0xfe, 0xed, 0xfa, 0xce],
        [0xfe, 0xed, 0xfa, 0xcf],
        [0xce, 0xfa, 0xed, 0xfe],
        [0xcf, 0xfa, 0xed, 0xfe],
        [0xca, 0xfe, 0xba, 0xbe],
    ];
    !NATIVE_MAGIC.iter().any(|magic| head.starts_with(magic))
}

/// Reads the first `bin` entry of an installed npm package.
fn read_package_executable(package_dir: &Path) -> Result<PathBuf> {
    let manifest_path = package_dir.join("package.json");
    let manifest = std::fs::read(&manifest_path)
        .with_context(|| format!("reading {}", manifest_path.display()))?;
    let manifest: serde_json::Value = serde_json::from_slice(&manifest)
        .with_context(|| format!("parsing {}", manifest_path.display()))?;
    let relative = match manifest.get("bin") {
        Some(serde_json::Value::String(path)) => path.clone(),
        Some(serde_json::Value::Object(bins)) => bins
            .values()
            .find_map(|path| path.as_str().map(str::to_string))
            .context("package has an empty bin map")?,
        _ => bail!("package at {} has no executable", package_dir.display()),
    };
    Ok(package_dir.join(relative))
}

/// Uses an npm range (`0.0.0 - <version>`) instead of an exact pin so npm can fall back to an
/// older release when a min-release-age policy hides the newest one. Ported from Zed.
fn bounded_npm_package_spec(package_spec: &str) -> (&str, String) {
    let Some((package_name, version)) = package_spec.rsplit_once('@') else {
        return (package_spec, package_spec.to_string());
    };
    if package_name.is_empty() {
        return (package_spec, package_spec.to_string());
    }
    if semver::Version::parse(version).is_err() {
        return (package_name, package_spec.to_string());
    }

    (package_name, format!("{package_name}@0.0.0 - {version}"))
}

enum RegistryArchiveKind {
    Archive(AssetKind),
    /// The registry schema allows an archive URL to point straight at an executable.
    RawBinary {
        file_name: String,
    },
}

fn registry_archive_kind_for_url(archive_url: &str) -> Result<RegistryArchiveKind> {
    const UNSUPPORTED_SUFFIXES: &[&str] = &[
        // Installer formats explicitly rejected by the registry schema.
        ".dmg",
        ".pkg",
        ".deb",
        ".rpm",
        ".msi",
        ".appimage",
        // Archive formats we cannot extract; treating them as raw binaries would produce a
        // broken install.
        ".tar.xz",
        ".txz",
        ".tar",
        ".gz",
        ".bz2",
        ".xz",
        ".7z",
    ];

    let archive_path = Url::parse(archive_url)
        .ok()
        .map(|url| url.path().to_string())
        .unwrap_or_else(|| archive_url.to_string());
    let lowercase_path = archive_path.to_lowercase();

    if lowercase_path.ends_with(".zip") {
        Ok(RegistryArchiveKind::Archive(AssetKind::Zip))
    } else if lowercase_path.ends_with(".tar.gz") || lowercase_path.ends_with(".tgz") {
        Ok(RegistryArchiveKind::Archive(AssetKind::TarGz))
    } else if lowercase_path.ends_with(".tar.bz2") || lowercase_path.ends_with(".tbz2") {
        Ok(RegistryArchiveKind::Archive(AssetKind::TarBz2))
    } else if let Some(suffix) = UNSUPPORTED_SUFFIXES
        .iter()
        .find(|suffix| lowercase_path.ends_with(*suffix))
    {
        bail!("unsupported archive type {suffix} in URL: {archive_url}");
    } else {
        let file_name = raw_binary_file_name(&archive_path)
            .with_context(|| format!("determining binary file name from URL: {archive_url}"))?;
        Ok(RegistryArchiveKind::RawBinary { file_name })
    }
}

fn raw_binary_file_name(archive_path: &str) -> Result<String> {
    let last_segment = archive_path
        .rsplit('/')
        .next()
        .filter(|segment| !segment.is_empty())
        .context("URL has no file name")?;
    let file_name = percent_decode_str(last_segment)
        .decode_utf8()
        .context("file name is not valid UTF-8")?
        .into_owned();
    anyhow::ensure!(
        !file_name.is_empty()
            && file_name != "."
            && file_name != ".."
            && !file_name.contains(['/', '\\'])
            && !file_name.contains('\0'),
        "invalid binary file name: {file_name}"
    );
    Ok(file_name)
}

#[derive(Deserialize)]
struct RegistryIndex {
    agents: Vec<RegistryEntry>,
}

#[derive(Deserialize)]
struct RegistryEntry {
    id: String,
    name: String,
    version: String,
    description: String,
    #[serde(default)]
    repository: Option<String>,
    #[serde(default)]
    website: Option<String>,
    #[serde(default)]
    license_url: Option<String>,
    #[serde(default)]
    icon: Option<String>,
    distribution: RegistryDistribution,
}

#[derive(Deserialize)]
struct RegistryDistribution {
    #[serde(default)]
    binary: Option<HashMap<String, RegistryBinaryTarget>>,
    #[serde(default)]
    npx: Option<RegistryNpxDistribution>,
}

#[derive(Deserialize)]
struct RegistryBinaryTarget {
    archive: String,
    cmd: String,
    #[serde(default)]
    args: Vec<String>,
    #[serde(default)]
    sha256: Option<String>,
    #[serde(default)]
    env: HashMap<String, String>,
}

#[derive(Deserialize)]
struct RegistryNpxDistribution {
    package: String,
    #[serde(default)]
    args: Vec<String>,
    #[serde(default)]
    env: HashMap<String, String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::StreamExt as _;

    const SAMPLE_INDEX: &str = r#"{
        "version": "1.0.0",
        "agents": [
            {
                "id": "npx-agent", "name": "Npx Agent", "version": "1.2.3",
                "description": "runs with npx", "icon": "./npx-agent/icon.svg",
                "distribution": { "npx": { "package": "npx-agent@1.2.3", "args": ["--acp"] } }
            },
            {
                "id": "binary-agent", "name": "Binary Agent", "version": "0.9.0",
                "description": "ships binaries",
                "distribution": { "binary": {
                    "darwin-aarch64": { "archive": "https://example.com/a.tar.gz", "cmd": "./agent" },
                    "darwin-x86_64": { "archive": "https://example.com/b.tar.gz", "cmd": "./agent" },
                    "linux-aarch64": { "archive": "https://example.com/c.tar.gz", "cmd": "./agent" },
                    "linux-x86_64": { "archive": "https://example.com/d.tar.gz", "cmd": "./agent" },
                    "windows-aarch64": { "archive": "https://example.com/e.zip", "cmd": "./agent.exe" },
                    "windows-x86_64": { "archive": "https://example.com/f.zip", "cmd": "./agent.exe" }
                } }
            },
            {
                "id": "uvx-agent", "name": "Uvx Agent", "version": "2.0.0",
                "description": "only uvx, which we skip",
                "distribution": { "uvx": { "package": "uvx-agent" } }
            }
        ],
        "extensions": []
    }"#;

    #[test]
    fn npm_bins_run_as_what_they_are() {
        let dir = tempfile::tempdir().expect("temp dir");
        let write = |name: &str, bytes: &[u8]| {
            let path = dir.path().join(name);
            std::fs::write(&path, bytes).expect("write");
            path
        };
        assert!(runs_with_node(&write(
            "cli.js",
            b"#!/usr/bin/env node\nconsole.log(1)"
        )));
        assert!(runs_with_node(&write("plain.js", b"module.exports = 1")));
        assert!(!runs_with_node(&write("tool.sh", b"#!/bin/sh\necho hi")));
        assert!(!runs_with_node(&write(
            "droid",
            &[0xcf, 0xfa, 0xed, 0xfe, 7, 0, 0, 1]
        )));
    }

    #[test]
    fn parses_registry_index() {
        let index: RegistryIndex = serde_json::from_str(SAMPLE_INDEX).expect("valid index");
        let icons = vec![None; index.agents.len()];
        let agents = registry_agents_from_index(index, icons);
        let ids: Vec<_> = agents
            .iter()
            .map(|agent| agent.id().0.to_string())
            .collect();
        assert_eq!(ids, vec!["npx-agent", "binary-agent"]);
        assert!(matches!(agents[0], RegistryAgent::Npx(_)));
        match &agents[1] {
            RegistryAgent::Binary(agent) => assert!(agent.supports_current_platform),
            RegistryAgent::Npx(_) => panic!("expected a binary agent"),
        }
    }

    #[test]
    fn npm_package_spec_is_bounded() {
        assert_eq!(
            bounded_npm_package_spec("@scope/agent@1.2.3"),
            ("@scope/agent", "@scope/agent@0.0.0 - 1.2.3".to_string())
        );
        assert_eq!(
            bounded_npm_package_spec("agent@latest"),
            ("agent", "agent@latest".to_string())
        );
        assert_eq!(
            bounded_npm_package_spec("agent"),
            ("agent", "agent".to_string())
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn npm_installs_the_registry_version_and_the_range_only_when_refused() {
        use std::os::unix::fs::PermissionsExt as _;

        let dir = tempfile::tempdir().expect("temp dir");
        let calls = dir.path().join("calls");
        let refuse_exact = dir.path().join("refuse-exact");
        // With `refuse-exact` present it refuses exact versions, as npm does under a
        // min-release-age policy.
        let fake_npm = dir.path().join("npm");
        std::fs::write(
            &fake_npm,
            format!(
                "#!/bin/sh\necho \"$2\" >> '{calls}'\ncase \"$2\" in *' - '*) exit 0 ;; esac\n\
                 if [ -f '{refuse_exact}' ]; then echo 'No matching version' >&2; exit 1; fi\n",
                calls = calls.display(),
                refuse_exact = refuse_exact.display(),
            ),
        )
        .expect("write");
        std::fs::set_permissions(&fake_npm, std::fs::Permissions::from_mode(0o755)).expect("chmod");
        let node = Node::System {
            node: PathBuf::from("/usr/bin/false"),
            npm: fake_npm,
        };
        let recorded_calls = || {
            let recorded = std::fs::read_to_string(&calls).expect("calls");
            std::fs::remove_file(&calls).expect("clear calls");
            recorded.lines().map(str::to_string).collect::<Vec<_>>()
        };

        install_npm_package(&node, dir.path(), "@scope/agent@1.2.3")
            .await
            .expect("install");
        assert_eq!(recorded_calls(), vec!["@scope/agent@1.2.3"]);

        std::fs::write(&refuse_exact, "").expect("write");
        install_npm_package(&node, dir.path(), "@scope/agent@1.2.3")
            .await
            .expect("install through the range");
        assert_eq!(
            recorded_calls(),
            vec!["@scope/agent@1.2.3", "@scope/agent@0.0.0 - 1.2.3"]
        );
    }

    #[test]
    fn archive_kinds() {
        assert!(matches!(
            registry_archive_kind_for_url("https://example.com/agent.tar.gz"),
            Ok(RegistryArchiveKind::Archive(AssetKind::TarGz))
        ));
        assert!(matches!(
            registry_archive_kind_for_url("https://example.com/agent-darwin"),
            Ok(RegistryArchiveKind::RawBinary { .. })
        ));
        assert!(registry_archive_kind_for_url("https://example.com/agent.dmg").is_err());
        assert!(binary_command_path(Path::new("/x"), "../evil").is_err());
        assert!(binary_command_path(Path::new("/x"), "agent").is_err());
    }

    #[tokio::test]
    async fn builds_launch_commands() {
        let dir = tempfile::tempdir().expect("temp dir");
        let index: RegistryIndex = serde_json::from_str(SAMPLE_INDEX).expect("valid index");
        let agents = registry_agents_from_index(index, vec![None; 3]);
        let node_runtime = NodeRuntime::new(
            dir.path().join("node"),
            Arc::new(http_client::BlockedHttpClient),
        );

        let binary = &agents[1];
        let command = agent_command(binary, "0.9.0", dir.path(), &node_runtime)
            .await
            .expect("binary command");
        let expected_name = if cfg!(windows) { "agent.exe" } else { "agent" };
        assert_eq!(
            command.path,
            dir.path()
                .join("binary-agent")
                .join("0.9.0")
                .join(expected_name)
        );

        let package_dir = dir
            .path()
            .join(NPX_DIR_NAME)
            .join("npx-agent")
            .join("node_modules")
            .join("npx-agent");
        std::fs::create_dir_all(&package_dir).expect("mkdir");
        std::fs::write(
            package_dir.join("package.json"),
            r#"{"bin": {"npx-agent": "dist/index.js"}}"#,
        )
        .expect("write");
        assert_eq!(
            read_package_executable(&package_dir).expect("executable"),
            package_dir.join("dist/index.js")
        );
        std::fs::write(package_dir.join("package.json"), r#"{"bin": "cli.js"}"#).expect("write");
        assert_eq!(
            read_package_executable(&package_dir).expect("executable"),
            package_dir.join("cli.js")
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn store_loads_the_cache_and_keeps_it_when_a_refresh_fails() {
        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::write(dir.path().join("registry.json"), SAMPLE_INDEX).expect("write");
        std::fs::create_dir_all(dir.path().join("binary-agent").join("0.9.0")).expect("mkdir");
        std::fs::create_dir_all(dir.path().join("icons")).expect("mkdir");
        let svg = r#"<svg fill="currentColor"/>"#;
        std::fs::write(dir.path().join("icons").join("npx-agent.svg"), svg).expect("write");

        let (mut store, mut inbox) = AgentRegistryStore::new(
            tokio::runtime::Handle::current(),
            Arc::new(http_client::BlockedHttpClient),
            futures::future::ready(()).boxed().shared(),
            dir.path().to_path_buf(),
            dir.path().join("node"),
        );
        let binary_agent = AgentId::new("binary-agent");
        let command = store.command_when_loaded(&binary_agent);
        assert_eq!(
            store.install_state(&binary_agent),
            InstallState::NotInstalled
        );

        let message = inbox.next().await.expect("cache message");
        store.handle(message);
        assert_eq!(store.agents().len(), 2);

        // Clients get the icon by a hash of its SVG, the same on every machine that has it.
        let listing = store.snapshot();
        let npx_agent = listing
            .agent(&AgentId::new("npx-agent"))
            .expect("npx agent");
        let icon_id = npx_agent.icon().expect("icon id").clone();
        assert_eq!(icon_id, agent_icon(svg.to_string()).id);
        assert_eq!(
            store.icon(&icon_id).map(|icon| icon.svg.clone()),
            Some(svg.into())
        );
        assert_eq!(
            listing.agent(&binary_agent).and_then(AgentListing::icon),
            None
        );
        assert_eq!(
            store.install_state(&binary_agent),
            InstallState::Installed {
                version: "0.9.0".into(),
                update_available: false,
            }
        );
        let command = command.await.expect("command");
        assert!(command.path.starts_with(dir.path().join("binary-agent")));

        store.refresh();
        assert!(store.is_fetching());
        let message = inbox.next().await.expect("refresh message");
        store.handle(message);
        assert!(!store.is_fetching());
        assert!(store.fetch_error().is_some());
        assert_eq!(store.agents().len(), 2);
    }

    #[test]
    fn scans_installed_versions() {
        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::create_dir_all(dir.path().join("binary-agent").join("0.9.0")).expect("mkdir");
        std::fs::create_dir_all(dir.path().join("icons")).expect("mkdir");
        let npx_dir = dir.path().join(NPX_DIR_NAME).join("npx-agent");
        let package_dir = npx_dir.join("node_modules").join("@scope").join("pkg");
        std::fs::create_dir_all(&package_dir).expect("mkdir");
        std::fs::write(
            npx_dir.join("package.json"),
            r#"{"dependencies": {"@scope/pkg": "0.0.0 - 1.2.3"}}"#,
        )
        .expect("write");
        std::fs::write(package_dir.join("package.json"), r#"{"version": "1.2.1"}"#).expect("write");

        let installed = scan_installed_versions(dir.path());
        assert_eq!(installed.len(), 2);
        assert_eq!(
            installed.get(&AgentId::new("binary-agent")),
            Some(&SharedString::from("0.9.0"))
        );
        assert_eq!(
            installed.get(&AgentId::new("npx-agent")),
            Some(&SharedString::from("1.2.1"))
        );
    }
}
