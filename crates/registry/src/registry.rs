//! The ACP agent registry: fetching the list of available agents, installing them, and
//! building the command that launches an installed agent.
//!
//! Ported from Zed's `project::agent_registry_store` and the registry parts of
//! `project::agent_server_store`, without Zed's settings and fs layers. npx agents run with
//! the system's `node`/`npm`.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{Context as _, Result, anyhow, bail};
use collections::HashMap;
use futures::AsyncReadExt as _;
use futures::future::{Shared, join_all};
use gpui::{
    App, AppContext as _, BackgroundExecutor, Context, Entity, FutureExt as _, Global,
    SharedString, Task, TaskExt as _,
};
use http_client::github::AssetKind;
use http_client::{AsyncBody, HttpClient, StatusCode};
use percent_encoding::percent_decode_str;
use serde::Deserialize;
use url::Url;
use util::ResultExt as _;

const REGISTRY_URL: &str = "https://cdn.agentclientprotocol.com/registry/v1/latest/registry.json";
const REFRESH_THROTTLE_DURATION: Duration = Duration::from_secs(60 * 60);
// Bounds the whole request including the body; the HTTP client only has a connect timeout.
const REGISTRY_FETCH_TIMEOUT: Duration = Duration::from_secs(30);
const REGISTRY_ICON_FETCH_TIMEOUT: Duration = Duration::from_secs(10);
const NPX_DIR_NAME: &str = "npx";

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct AgentId(pub SharedString);

impl AgentId {
    pub fn new(id: impl Into<SharedString>) -> Self {
        AgentId(id.into())
    }
}

impl std::fmt::Display for AgentId {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}", self.0)
    }
}

#[derive(Clone, Debug)]
pub struct RegistryAgentMetadata {
    pub id: AgentId,
    pub name: SharedString,
    pub description: SharedString,
    pub version: SharedString,
    pub repository: Option<SharedString>,
    pub website: Option<SharedString>,
    pub license_url: Option<SharedString>,
    /// Absolute path of the cached SVG icon.
    pub icon_path: Option<SharedString>,
}

#[derive(Clone, Debug)]
pub struct RegistryBinaryAgent {
    pub metadata: RegistryAgentMetadata,
    pub targets: HashMap<String, RegistryTargetConfig>,
    pub supports_current_platform: bool,
}

#[derive(Clone, Debug)]
pub struct RegistryNpxAgent {
    pub metadata: RegistryAgentMetadata,
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

    pub fn icon_path(&self) -> Option<&SharedString> {
        self.metadata().icon_path.as_ref()
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

#[derive(Clone, Debug, PartialEq)]
pub enum InstallState {
    NotInstalled,
    Installing,
    Installed {
        version: SharedString,
        update_available: bool,
    },
    Failed(SharedString),
}

/// How to start an installed agent.
#[derive(Clone, Debug)]
pub struct AgentCommand {
    pub path: PathBuf,
    pub args: Vec<String>,
    pub env: HashMap<String, String>,
}

/// Resolves once the user's login-shell environment (and with it `PATH`) has been loaded, so
/// `npm` and `node` can be found when the app was started from Finder.
pub type ShellEnvironmentReady = Shared<Task<()>>;

struct GlobalAgentRegistryStore(Entity<AgentRegistryStore>);

impl Global for GlobalAgentRegistryStore {}

pub struct AgentRegistryStore {
    http_client: Arc<dyn HttpClient>,
    shell_environment_ready: ShellEnvironmentReady,
    registry_dir: PathBuf,
    agents: Vec<RegistryAgent>,
    installed_versions: HashMap<AgentId, SharedString>,
    installing: HashMap<AgentId, Task<()>>,
    install_errors: HashMap<AgentId, SharedString>,
    is_fetching: bool,
    fetch_error: Option<SharedString>,
    pending_refresh: Option<Task<()>>,
    last_refresh: Option<Instant>,
}

pub fn init(
    http_client: Arc<dyn HttpClient>,
    shell_environment_ready: ShellEnvironmentReady,
    cx: &mut App,
) {
    let store = cx.new(|cx| {
        AgentRegistryStore::new(
            http_client,
            shell_environment_ready,
            paths::registry_dir(),
            cx,
        )
    });
    cx.set_global(GlobalAgentRegistryStore(store.clone()));
    store.update(cx, |store, cx| store.refresh_if_stale(cx));
}

impl AgentRegistryStore {
    pub fn global(cx: &App) -> Entity<Self> {
        cx.global::<GlobalAgentRegistryStore>().0.clone()
    }

    fn new(
        http_client: Arc<dyn HttpClient>,
        shell_environment_ready: ShellEnvironmentReady,
        registry_dir: PathBuf,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut store = Self {
            http_client,
            shell_environment_ready,
            registry_dir,
            agents: Vec::new(),
            installed_versions: HashMap::default(),
            installing: HashMap::default(),
            install_errors: HashMap::default(),
            is_fetching: false,
            fetch_error: None,
            pending_refresh: None,
            last_refresh: None,
        };
        store.load_cached_registry(cx);
        store
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

    pub fn fetch_error(&self) -> Option<SharedString> {
        self.fetch_error.clone()
    }

    pub fn install_state(&self, id: &AgentId) -> InstallState {
        if self.installing.contains_key(id) {
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

    /// Fetches the latest registry from the network and updates the cache.
    pub fn refresh(&mut self, cx: &mut Context<Self>) {
        if self.pending_refresh.is_some() {
            return;
        }

        self.is_fetching = true;
        self.fetch_error = None;
        self.last_refresh = Some(Instant::now());
        cx.notify();

        let http_client = self.http_client.clone();
        let registry_dir = self.registry_dir.clone();
        let executor = cx.background_executor().clone();

        self.pending_refresh = Some(cx.spawn(async move |this, cx| {
            let result = match fetch_registry_index(http_client.clone(), &executor).await {
                Ok(data) => {
                    build_registry_agents(
                        http_client,
                        registry_dir.clone(),
                        data.index,
                        Some(data.raw_body),
                        &executor,
                    )
                    .await
                }
                Err(error) => {
                    log::error!("failed to fetch the ACP registry: {error:#}");
                    Err(error)
                }
            };
            let installed_versions = executor
                .spawn(async move { scan_installed_versions(&registry_dir) })
                .await;

            this.update(cx, |this, cx| {
                this.pending_refresh = None;
                this.is_fetching = false;
                match result {
                    Ok(agents) => {
                        this.agents = agents;
                        this.fetch_error = None;
                    }
                    Err(error) => {
                        this.fetch_error = Some(SharedString::from(format!("{error:#}")));
                    }
                }
                this.set_installed_versions(installed_versions);
                cx.notify();
            })
            .ok();
        }));
    }

    /// Refreshes at most once an hour.
    pub fn refresh_if_stale(&mut self, cx: &mut Context<Self>) {
        let should_refresh = self
            .last_refresh
            .map(|last| last.elapsed() >= REFRESH_THROTTLE_DURATION)
            .unwrap_or(true);
        if should_refresh {
            self.refresh(cx);
        }
    }

    /// Installs (or updates to) the registry's current version of the agent.
    pub fn install(&mut self, id: &AgentId, cx: &mut Context<Self>) {
        if self.installing.contains_key(id) {
            return;
        }
        let Some(agent) = self.agent(id).cloned() else {
            return;
        };
        self.install_errors.remove(id);

        let http_client = self.http_client.clone();
        let registry_dir = self.registry_dir.clone();
        let shell_environment_ready = self.shell_environment_ready.clone();
        let executor = cx.background_executor().clone();
        let id = id.clone();
        let task = cx.spawn({
            let id = id.clone();
            async move |this, cx| {
                shell_environment_ready.await;
                let install_result = {
                    let registry_dir = registry_dir.clone();
                    executor
                        .spawn(async move {
                            install_agent(&agent, &registry_dir, http_client.as_ref()).await
                        })
                        .await
                };
                let installed_versions = executor
                    .spawn(async move { scan_installed_versions(&registry_dir) })
                    .await;

                this.update(cx, |this, cx| {
                    this.installing.remove(&id);
                    if let Err(error) = install_result {
                        log::error!("failed to install agent {id}: {error:#}");
                        this.install_errors
                            .insert(id.clone(), SharedString::from(format!("{error:#}")));
                    }
                    this.set_installed_versions(installed_versions);
                    cx.notify();
                })
                .ok();
            }
        });
        self.installing.insert(id, task);
        cx.notify();
    }

    /// Builds the command that starts an installed agent.
    pub fn command(&self, id: &AgentId, cx: &App) -> Task<Result<AgentCommand>> {
        let Some(agent) = self.agent(id).cloned() else {
            return Task::ready(Err(anyhow!("agent {id} is not in the registry")));
        };
        let Some(installed_version) = self.installed_versions.get(id).cloned() else {
            return Task::ready(Err(anyhow!("agent {id} is not installed")));
        };
        let registry_dir = self.registry_dir.clone();
        let shell_environment_ready = self.shell_environment_ready.clone();
        cx.background_spawn(async move {
            shell_environment_ready.await;
            agent_command(&agent, &installed_version, &registry_dir)
        })
    }

    fn set_installed_versions(&mut self, installed_versions: HashMap<AgentId, SharedString>) {
        for id in installed_versions.keys() {
            self.install_errors.remove(id);
        }
        self.installed_versions = installed_versions;
    }

    fn load_cached_registry(&mut self, cx: &mut Context<Self>) {
        let http_client = self.http_client.clone();
        let registry_dir = self.registry_dir.clone();
        let executor = cx.background_executor().clone();
        cx.spawn(async move |this, cx| -> Result<()> {
            let (bytes, installed_versions) = {
                let registry_dir = registry_dir.clone();
                executor
                    .spawn(async move {
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
                    .await
            };

            let agents = match bytes {
                Some(bytes) => {
                    let index: RegistryIndex =
                        serde_json::from_slice(&bytes).context("parsing cached registry")?;
                    Some(
                        build_registry_agents(http_client, registry_dir, index, None, &executor)
                            .await?,
                    )
                }
                None => None,
            };

            this.update(cx, |this, cx| {
                // A network refresh may have finished first; its list is newer.
                if let Some(agents) = agents
                    && this.agents.is_empty()
                {
                    this.agents = agents;
                }
                this.set_installed_versions(installed_versions);
                cx.notify();
            })?;
            Ok(())
        })
        .detach_and_log_err(cx);
    }
}

struct RegistryFetchResult {
    index: RegistryIndex,
    raw_body: Vec<u8>,
}

async fn fetch_registry_index(
    http_client: Arc<dyn HttpClient>,
    executor: &BackgroundExecutor,
) -> Result<RegistryFetchResult> {
    let (status, body) =
        fetch_url_body(http_client, REGISTRY_URL, REGISTRY_FETCH_TIMEOUT, executor)
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
    executor: &BackgroundExecutor,
) -> Result<Vec<RegistryAgent>> {
    let icons_dir = registry_dir.join("icons");
    let update_cache = raw_body.is_some();
    if let Some(raw_body) = raw_body {
        let registry_dir = registry_dir.clone();
        executor
            .spawn(async move {
                std::fs::create_dir_all(registry_dir.join("icons"))
                    .with_context(|| format!("creating {}", registry_dir.display()))?;
                std::fs::write(registry_dir.join("registry.json"), raw_body)
                    .context("writing registry cache")
            })
            .await?;
    }

    let icon_paths = join_all(index.agents.iter().map(|entry| {
        let http_client = http_client.clone();
        let icons_dir = icons_dir.clone();
        async move {
            resolve_icon_path(entry, &icons_dir, update_cache, http_client, executor)
                .await
                .log_err()
                .flatten()
        }
    }))
    .await;

    Ok(registry_agents_from_index(index, icon_paths))
}

fn registry_agents_from_index(
    index: RegistryIndex,
    icon_paths: Vec<Option<SharedString>>,
) -> Vec<RegistryAgent> {
    let current_platform = current_platform_key();
    let mut agents = Vec::new();
    for (entry, icon_path) in index.agents.into_iter().zip(icon_paths) {
        let metadata = RegistryAgentMetadata {
            id: AgentId::new(entry.id),
            name: entry.name.into(),
            description: entry.description.into(),
            version: entry.version.into(),
            repository: entry.repository.map(Into::into),
            website: entry.website.map(Into::into),
            license_url: entry.license_url.map(Into::into),
            icon_path,
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
                targets,
                supports_current_platform,
            })
        });

        let npx_agent = entry.distribution.npx.as_ref().map(|npx| RegistryNpxAgent {
            metadata: metadata.clone(),
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

async fn resolve_icon_path(
    entry: &RegistryEntry,
    icons_dir: &Path,
    update_cache: bool,
    http_client: Arc<dyn HttpClient>,
    executor: &BackgroundExecutor,
) -> Result<Option<SharedString>> {
    let Some(icon_url) = resolve_icon_url(entry) else {
        return Ok(None);
    };

    let icon_path = icons_dir.join(format!("{}.svg", sanitize_path_component(&entry.id)));
    if update_cache && !icon_path.is_file() {
        let download = async {
            let (status, body) = fetch_url_body(
                http_client,
                &icon_url,
                REGISTRY_ICON_FETCH_TIMEOUT,
                executor,
            )
            .await?;
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

    Ok(icon_path
        .is_file()
        .then(|| SharedString::from(icon_path.to_string_lossy().into_owned())))
}

async fn fetch_url_body(
    http_client: Arc<dyn HttpClient>,
    url: &str,
    timeout: Duration,
    executor: &BackgroundExecutor,
) -> Result<(StatusCode, Vec<u8>)> {
    async {
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
    }
    .with_timeout(timeout, executor)
    .await
    .map_err(|_| {
        anyhow!(
            "timed out after {}s while fetching {url}",
            timeout.as_secs()
        )
    })?
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
) -> Result<()> {
    match agent {
        RegistryAgent::Binary(agent) => {
            install_binary_agent(agent, registry_dir, http_client).await
        }
        RegistryAgent::Npx(agent) => install_npx_agent(agent, registry_dir).await,
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

async fn install_npx_agent(agent: &RegistryNpxAgent, registry_dir: &Path) -> Result<()> {
    let install_dir = npx_agent_dir(registry_dir, &agent.metadata.id);
    std::fs::create_dir_all(&install_dir)
        .with_context(|| format!("creating {}", install_dir.display()))?;
    let manifest_path = install_dir.join("package.json");
    if !manifest_path.exists() {
        std::fs::write(&manifest_path, "{\"private\": true}\n")
            .with_context(|| format!("writing {}", manifest_path.display()))?;
    }

    let (_, package_spec) = bounded_npm_package_spec(&agent.package);
    let output = smol::process::Command::new(find_program("npm")?)
        .args([
            "install",
            package_spec.as_str(),
            "--save-exact",
            "--no-fund",
            "--no-audit",
        ])
        .current_dir(&install_dir)
        .output()
        .await
        .context("running npm install")?;
    if !output.status.success() {
        bail!(
            "npm install {} failed: {}",
            agent.package,
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(())
}

fn agent_command(
    agent: &RegistryAgent,
    installed_version: &str,
    registry_dir: &Path,
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
            let mut args = vec![executable.to_string_lossy().into_owned()];
            args.extend(agent.args.iter().cloned());
            Ok(AgentCommand {
                path: find_program("node")?,
                args,
                env: agent.env.clone(),
            })
        }
    }
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

fn find_program(name: &str) -> Result<PathBuf> {
    let path = std::env::var_os("PATH").unwrap_or_default();
    std::env::split_paths(&path)
        .map(|dir| dir.join(name))
        .find(|candidate| candidate.is_file())
        .with_context(|| {
            format!("could not find `{name}` on PATH; install Node.js to use this agent")
        })
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

    const SAMPLE_INDEX: &str = r#"{
        "version": "1.0.0",
        "agents": [
            {
                "id": "npx-agent", "name": "Npx Agent", "version": "1.2.3",
                "description": "runs with npx",
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
    fn parses_registry_index() {
        let index: RegistryIndex = serde_json::from_str(SAMPLE_INDEX).expect("valid index");
        let icon_paths = vec![None; index.agents.len()];
        let agents = registry_agents_from_index(index, icon_paths);
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

    #[test]
    fn builds_launch_commands() {
        let dir = tempfile::tempdir().expect("temp dir");
        let index: RegistryIndex = serde_json::from_str(SAMPLE_INDEX).expect("valid index");
        let agents = registry_agents_from_index(index, vec![None; 3]);

        let binary = &agents[1];
        let command = agent_command(binary, "0.9.0", dir.path()).expect("binary command");
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
