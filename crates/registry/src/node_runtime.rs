//! Finds Node.js for npm agents: the machine's own when it's new enough, otherwise a copy
//! downloaded from nodejs.org into the data directory. Ported from Zed's `node_runtime`.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context as _, Result, anyhow, bail};
use collections::HashMap;
use http_client::HttpClient;
use http_client::github::AssetKind;
use semver::Version;

const MANAGED_VERSION: &str = "v24.11.0";
const MIN_SYSTEM_VERSION: Version = Version::new(22, 0, 0);
const NODE_CA_CERTS_ENV_VAR: &str = "NODE_EXTRA_CA_CERTS";
const CHECKSUMS_FETCH_TIMEOUT: Duration = Duration::from_secs(30);

/// A Node.js installation that has been checked to run.
#[derive(Clone, Debug)]
pub(crate) enum Node {
    System { node: PathBuf, npm: PathBuf },
    Managed { home: PathBuf },
}

impl Node {
    pub(crate) fn node_path(&self) -> PathBuf {
        match self {
            Node::System { node, .. } => node.clone(),
            Node::Managed { home } => home.join("bin").join("node"),
        }
    }

    /// Runs npm with `subcommand` and `args`. A managed npm gets its own cache and blank
    /// configs, as in Zed, so the user's `.npmrc` (made for their own Node) doesn't apply.
    pub(crate) fn npm(&self, subcommand: &str, args: &[&str]) -> tokio::process::Command {
        match self {
            Node::System { npm, .. } => {
                let mut command = tokio::process::Command::new(npm);
                command.arg(subcommand).args(args);
                command
            }
            Node::Managed { home } => {
                let mut command = tokio::process::Command::new(self.node_path());
                command
                    .arg(managed_npm_cli(home))
                    .arg(subcommand)
                    .arg(format!("--cache={}", home.join("cache").display()))
                    .arg("--userconfig")
                    .arg(home.join("blank_user_npmrc"))
                    .arg("--globalconfig")
                    .arg(home.join("blank_global_npmrc"))
                    .args(args)
                    .envs(self.env());
                command
            }
        }
    }

    /// Environment for processes that run on this Node, so the scripts they start find the
    /// same `node`.
    pub(crate) fn env(&self) -> HashMap<String, String> {
        let mut env = HashMap::default();
        if let Node::Managed { .. } = self {
            if let Some(path) = path_with_dir_prepended(&self.node_path()) {
                env.insert("PATH".into(), path.to_string_lossy().into_owned());
            }
            if let Ok(certs) = std::env::var(NODE_CA_CERTS_ENV_VAR)
                && !certs.is_empty()
            {
                env.insert(NODE_CA_CERTS_ENV_VAR.into(), certs);
            }
        }
        env
    }

    fn exists(&self) -> bool {
        self.node_path().is_file()
    }
}

/// Finds Node.js once and remembers it; downloads happen at most once at a time.
#[derive(Clone)]
pub(crate) struct NodeRuntime {
    dir: PathBuf,
    http_client: Arc<dyn HttpClient>,
    found: Arc<tokio::sync::Mutex<Option<Node>>>,
}

impl NodeRuntime {
    pub(crate) fn new(dir: PathBuf, http_client: Arc<dyn HttpClient>) -> Self {
        Self {
            dir,
            http_client,
            found: Arc::default(),
        }
    }

    pub(crate) fn dir(&self) -> &Path {
        &self.dir
    }

    /// Whether the Node.js found is the downloaded one, or one is being looked for (and may be
    /// downloaded) right now.
    pub(crate) fn uses_download(&self) -> bool {
        match self.found.try_lock() {
            Ok(found) => matches!(*found, Some(Node::Managed { .. })),
            Err(_) => true,
        }
    }

    /// Deletes the downloaded Node.js, which is downloaded again when it's next needed.
    pub(crate) async fn delete_download(&self) -> Result<()> {
        // Held throughout, so no agent starts on it or downloads it meanwhile.
        let mut found = self.found.lock().await;
        if matches!(*found, Some(Node::Managed { .. })) {
            *found = None;
        }
        let dir = self.dir.clone();
        let removed = tokio::task::spawn_blocking(move || std::fs::remove_dir_all(&dir)).await?;
        match removed {
            Err(error) if error.kind() != std::io::ErrorKind::NotFound => {
                Err(error).with_context(|| format!("removing {}", self.dir.display()))
            }
            _ => Ok(()),
        }
    }

    /// Call once the login-shell `PATH` is loaded, or a system Node may be missed.
    pub(crate) async fn node(&self) -> Result<Node> {
        let mut found = self.found.lock().await;
        if let Some(node) = found.as_ref()
            && node.exists()
        {
            return Ok(node.clone());
        }
        let node = match system_node().await {
            Ok(node) => {
                log::info!("using Node.js found on PATH: {node:?}");
                node
            }
            Err(reason) => {
                let node = install_managed_node(&self.dir, &self.http_client)
                    .await
                    .context("downloading Node.js")?;
                log::info!("using managed Node.js ({node:?}) since {reason:#}");
                node
            }
        };
        *found = Some(node.clone());
        Ok(node)
    }
}

async fn system_node() -> Result<Node> {
    let node = find_program("node").context("system Node.js wasn't found on PATH")?;
    let npm = find_program("npm").context("system npm wasn't found on PATH")?;
    let output = tokio::process::Command::new(&node)
        .arg("--version")
        .kill_on_drop(true)
        .output()
        .await
        .with_context(|| format!("running {}", node.display()))?;
    if !output.status.success() {
        bail!(
            "{} --version failed: {}",
            node.display(),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    let version = parse_node_version(&String::from_utf8_lossy(&output.stdout))?;
    if version < MIN_SYSTEM_VERSION {
        bail!("system Node.js is {version}, older than {MIN_SYSTEM_VERSION}");
    }
    Ok(Node::System { node, npm })
}

fn parse_node_version(output: &str) -> Result<Version> {
    let output = output.trim();
    Version::parse(output.trim_start_matches('v'))
        .with_context(|| format!("Node.js reported an invalid version: {output:?}"))
}

fn find_program(name: &str) -> Result<PathBuf> {
    let path = std::env::var_os("PATH").unwrap_or_default();
    std::env::split_paths(&path)
        .map(|dir| dir.join(name))
        .find(|candidate| candidate.is_file())
        .with_context(|| format!("could not find `{name}` on PATH"))
}

/// The name nodejs.org gives the build for this machine, like `node-v24.11.0-darwin-arm64`.
fn managed_build_name() -> Result<String> {
    let os = match std::env::consts::OS {
        "macos" => "darwin",
        "linux" => "linux",
        other => bail!("no Node.js download for {other}"),
    };
    let arch = match std::env::consts::ARCH {
        "x86_64" => "x64",
        "aarch64" => "arm64",
        other => bail!("no Node.js download for {other}"),
    };
    Ok(format!("node-{MANAGED_VERSION}-{os}-{arch}"))
}

fn managed_npm_cli(home: &Path) -> PathBuf {
    home.join("lib")
        .join("node_modules")
        .join("npm")
        .join("bin")
        .join("npm-cli.js")
}

/// Installs `<dir>/<version>/<build>/`, keeping a working install and replacing a broken one.
async fn install_managed_node(dir: &Path, http_client: &Arc<dyn HttpClient>) -> Result<Node> {
    let build = managed_build_name()?;
    let version_dir = dir.join(MANAGED_VERSION);
    let node = Node::Managed {
        home: version_dir.join(&build),
    };

    if !managed_node_runs(&node).await {
        std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
        let file_name = format!("{build}.tar.gz");
        let base_url = format!("https://nodejs.org/dist/{MANAGED_VERSION}");
        let (status, checksums) = crate::fetch_url_body(
            http_client.clone(),
            &format!("{base_url}/SHASUMS256.txt"),
            CHECKSUMS_FETCH_TIMEOUT,
        )
        .await?;
        if !status.is_success() {
            bail!("fetching Node.js checksums: status {}", status.as_u16());
        }
        let digest = checksum_for(&String::from_utf8_lossy(&checksums), &file_name)
            .with_context(|| format!("no checksum for {file_name}"))?;
        http_client::github_download::download_server_binary(
            http_client.as_ref(),
            &format!("{base_url}/{file_name}"),
            Some(&digest),
            &version_dir,
            AssetKind::TarGz,
        )
        .await?;
        if !managed_node_runs(&node).await {
            bail!("the downloaded Node.js doesn't run on this machine");
        }
    }
    crate::remove_other_versions(dir, &version_dir).ok();

    let Node::Managed { home } = &node else {
        return Err(anyhow!("managed Node.js has no home"));
    };
    let cache = home.join("cache");
    if cache.exists() {
        std::fs::remove_dir_all(&cache).ok();
    }
    std::fs::create_dir_all(&cache).with_context(|| format!("creating {}", cache.display()))?;
    for config in ["blank_user_npmrc", "blank_global_npmrc"] {
        std::fs::write(home.join(config), [])
            .with_context(|| format!("writing {}", home.join(config).display()))?;
    }
    Ok(node)
}

async fn managed_node_runs(node: &Node) -> bool {
    let Node::Managed { home } = node else {
        return false;
    };
    if !node.exists() {
        return false;
    }
    match tokio::process::Command::new(node.node_path())
        .arg(managed_npm_cli(home))
        .arg("--version")
        .kill_on_drop(true)
        .output()
        .await
    {
        Ok(output) if output.status.success() => true,
        Ok(output) => {
            log::warn!(
                "managed Node.js at {} failed its check: {}",
                home.display(),
                String::from_utf8_lossy(&output.stderr).trim()
            );
            false
        }
        Err(error) => {
            log::warn!(
                "managed Node.js at {} failed to start: {error}",
                home.display()
            );
            false
        }
    }
}

/// Finds a file's SHA-256 in a `SHASUMS256.txt` (`<hex>  <file name>` per line).
fn checksum_for(checksums: &str, file_name: &str) -> Option<String> {
    checksums.lines().find_map(|line| {
        let (digest, name) = line.split_once(char::is_whitespace)?;
        (name.trim() == file_name).then(|| digest.to_string())
    })
}

fn path_with_dir_prepended(binary: &Path) -> Option<OsString> {
    let dir = binary.parent()?.to_path_buf();
    let existing = std::env::var_os("PATH").unwrap_or_default();
    std::env::join_paths(std::iter::once(dir).chain(std::env::split_paths(&existing))).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn node_versions_are_parsed() {
        assert_eq!(
            parse_node_version("v24.11.0\n").ok(),
            Some(Version::new(24, 11, 0))
        );
        assert!(parse_node_version("not a version").is_err());
    }

    #[test]
    fn checksums_are_found_by_file_name() {
        let checksums = "\
aaa  node-v24.11.0-darwin-arm64.tar.gz
bbb  node-v24.11.0-darwin-arm64.tar.xz
ccc  node-v24.11.0-linux-x64.tar.gz
";
        assert_eq!(
            checksum_for(checksums, "node-v24.11.0-linux-x64.tar.gz").as_deref(),
            Some("ccc")
        );
        assert_eq!(checksum_for(checksums, "node-v24.11.0-win-x64.zip"), None);
    }

    #[test]
    fn managed_npm_uses_its_own_cache_and_configs() {
        let node = Node::Managed {
            home: PathBuf::from("/data/node/v24/node-v24"),
        };
        let command = node.npm("install", &["agent@1.0.0"]);
        let command = command.as_std();
        assert_eq!(
            command.get_program(),
            Path::new("/data/node/v24/node-v24/bin/node")
        );
        let args: Vec<_> = command
            .get_args()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect();
        assert_eq!(
            args,
            [
                "/data/node/v24/node-v24/lib/node_modules/npm/bin/npm-cli.js",
                "install",
                "--cache=/data/node/v24/node-v24/cache",
                "--userconfig",
                "/data/node/v24/node-v24/blank_user_npmrc",
                "--globalconfig",
                "/data/node/v24/node-v24/blank_global_npmrc",
                "agent@1.0.0",
            ]
        );
        let path = node.env().get("PATH").cloned().unwrap_or_default();
        assert!(path.starts_with("/data/node/v24/node-v24/bin"));
    }

    /// Downloads Node.js (about 50 MB) into a scratch directory and installs a small package
    /// with its npm.
    #[tokio::test(flavor = "multi_thread")]
    #[ignore]
    async fn managed_node_downloads_and_runs() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let http_client: Arc<dyn HttpClient> = Arc::new(
            reqwest_client::ReqwestClient::user_agent("agentz-test")
                .map_err(|error| anyhow!("{error}"))?,
        );
        let node = install_managed_node(dir.path(), &http_client).await?;
        let project = dir.path().join("project");
        std::fs::create_dir_all(&project)?;
        std::fs::write(project.join("package.json"), "{\"private\": true}\n")?;
        let output = node
            .npm("install", &["is-number@7.0.0", "--no-fund", "--no-audit"])
            .current_dir(&project)
            .output()
            .await?;
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            project
                .join("node_modules/is-number/package.json")
                .is_file()
        );
        // A second call reuses the install.
        install_managed_node(dir.path(), &http_client).await?;
        Ok(())
    }
}
