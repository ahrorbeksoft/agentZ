//! Instabox's HTTP API, for what its CLI doesn't do: desktop links, CPU and memory, and running
//! time (plan.md, "Instabox's API"). agentZ makes its own API key once, through the signed-in
//! CLI, keeps it in `cloud/instabox/api-key` in the data directory, and makes another when it
//! stops working. It never reads the CLI's own login.

use std::io::Write as _;
use std::os::unix::fs::{OpenOptionsExt as _, PermissionsExt as _};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use agentz_protocol::cloud::{MetricSample, RunningTime};
use anyhow::{Context as _, Result, anyhow};
use futures::AsyncReadExt as _;
use http_client::{AsyncBody, HttpClient, Method, Request, StatusCode};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::instabox::{find_string, time};
use crate::{Cli, READ_TIMEOUT};

const DEFAULT_URL: &str = "https://api.instabox.sh";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
/// Starting the desktop answers once it runs; Instabox's SDK waits 90 s for it.
const DESKTOP_TIMEOUT: Duration = Duration::from_secs(90);
/// Installing the desktop takes about half a minute, once per machine.
const INSTALL_TIMEOUT: Duration = Duration::from_secs(10 * 60);
/// How long a desktop link lets new viewers in.
const LINK_SECONDS: u64 = 3600;
/// What the key is called in `keys ls` and Instabox's dashboard.
const KEY_NAME: &str = "agentZ";

pub struct InstaboxApi {
    http: Arc<dyn HttpClient>,
    url: String,
    key_file: PathBuf,
    /// Held while the key is read or made, so requests side by side don't make two.
    key: tokio::sync::Mutex<Option<ApiKey>>,
    poll_interval: Duration,
}

#[derive(Clone, PartialEq, Serialize, Deserialize)]
struct ApiKey {
    /// What `keys rm` takes, when `keys create` gave it.
    id: Option<String>,
    secret: String,
}

impl InstaboxApi {
    /// The API at `INSTABOX_API_URL`, where the CLI and SDK take it from, or Instabox's own, with
    /// agentZ's key kept under `data_dir`.
    pub fn new(http: Arc<dyn HttpClient>, data_dir: &Path) -> Self {
        let url = std::env::var("INSTABOX_API_URL")
            .ok()
            .filter(|url| !url.trim().is_empty())
            .unwrap_or_else(|| DEFAULT_URL.to_string());
        Self {
            http,
            url: url.trim().trim_end_matches('/').to_string(),
            key_file: data_dir.join("cloud").join("instabox").join("api-key"),
            key: tokio::sync::Mutex::new(None),
            poll_interval: Duration::from_secs(2),
        }
    }

    #[cfg(test)]
    pub(crate) fn polling_every(mut self, interval: Duration) -> Self {
        self.poll_interval = interval;
        self
    }

    /// CPU and memory over the last 5 minutes, oldest first.
    pub(crate) async fn metrics(&self, cli: &Cli, machine: &str) -> Result<Vec<MetricSample>> {
        let metrics: Metrics = self
            .request(
                cli,
                Method::GET,
                &format!("/v1/machines/{machine}/metrics"),
                None,
                REQUEST_TIMEOUT,
            )
            .await?;
        let mut samples: Vec<MetricSample> = metrics
            .metrics
            .into_iter()
            .filter_map(|metric| {
                Some(MetricSample {
                    at: time(Some(metric.timestamp))?,
                    cpu_percent: metric.cpu_used_pct,
                    memory_used_bytes: metric.mem_used as u64,
                    memory_total_bytes: metric.mem_total as u64,
                })
            })
            .collect();
        samples.sort_by_key(|sample| sample.at);
        Ok(samples)
    }

    /// How long it's run since the month began.
    pub(crate) async fn running_time(&self, cli: &Cli, machine: &str) -> Result<RunningTime> {
        let usage: Usage = self
            .request(
                cli,
                Method::GET,
                &format!("/v1/machines/{machine}/usage"),
                None,
                REQUEST_TIMEOUT,
            )
            .await?;
        Ok(RunningTime {
            since: time(Some(usage.from)).context("reading when its usage starts")?,
            seconds: usage.running_seconds as u64,
            vcpu_seconds: usage.vcpu_seconds.map(|seconds| seconds as u64),
        })
    }

    /// A link to the machine's desktop that works for an hour, to use it or only to watch it.
    /// The desktop is started first, and installed on a template that doesn't have it.
    pub(crate) async fn desktop_link(
        &self,
        cli: &Cli,
        machine: &str,
        view_only: bool,
    ) -> Result<String> {
        let mut status = self.desktop(cli, machine, "start").await?;
        if !status.installed {
            if !status.installing {
                status = self.desktop(cli, machine, "install").await?;
            }
            let deadline = Instant::now() + INSTALL_TIMEOUT;
            while !status.installed {
                if status.failed {
                    return Err(anyhow!(
                        "Instabox couldn't install the desktop: {}",
                        status
                            .log
                            .as_deref()
                            .unwrap_or("its log is /tmp/instabox-desktop-install.log there")
                    ));
                }
                anyhow::ensure!(
                    Instant::now() < deadline,
                    "Instabox didn't install the desktop in 10 minutes"
                );
                tokio::time::sleep(self.poll_interval).await;
                status = self.desktop(cli, machine, "status").await?;
            }
            status = self.desktop(cli, machine, "start").await?;
        }
        anyhow::ensure!(status.running, "the desktop didn't start");
        let link: DesktopLink = self
            .request(
                cli,
                Method::POST,
                &format!("/v1/machines/{machine}/desktop/stream"),
                Some(serde_json::json!({ "viewOnly": view_only, "ttlSeconds": LINK_SECONDS })),
                REQUEST_TIMEOUT,
            )
            .await?;
        Ok(link.url)
    }

    async fn desktop(&self, cli: &Cli, machine: &str, action: &str) -> Result<DesktopStatus> {
        self.request(
            cli,
            Method::POST,
            &format!("/v1/machines/{machine}/desktop"),
            Some(serde_json::json!({ "action": action })),
            DESKTOP_TIMEOUT,
        )
        .await
    }

    /// Sends a request with agentZ's key, made first if there's none, and again with a new one
    /// if Instabox turns it down.
    async fn request<T: DeserializeOwned>(
        &self,
        cli: &Cli,
        method: Method,
        path: &str,
        body: Option<serde_json::Value>,
        timeout: Duration,
    ) -> Result<T> {
        let key = self.key(cli, None).await?;
        let (mut status, mut answer) = self
            .send(&key, method.clone(), path, body.as_ref(), timeout)
            .await?;
        if status == StatusCode::UNAUTHORIZED || status == StatusCode::FORBIDDEN {
            // Revoked, expired, or made for an account the CLI has since signed out of.
            let key = self.key(cli, Some(&key)).await?;
            (status, answer) = self
                .send(&key, method.clone(), path, body.as_ref(), timeout)
                .await?;
        }
        if !status.is_success() {
            return Err(anyhow!(
                error_message(&answer)
                    .unwrap_or_else(|| format!("{method} {path} answered {status}"))
            ));
        }
        serde_json::from_slice(&answer).with_context(|| format!("reading {method} {path}"))
    }

    async fn send(
        &self,
        key: &ApiKey,
        method: Method,
        path: &str,
        body: Option<&serde_json::Value>,
        timeout: Duration,
    ) -> Result<(StatusCode, Vec<u8>)> {
        let url = format!("{}{path}", self.url);
        let mut request = Request::builder()
            .method(method)
            .uri(&url)
            .header("X-API-Key", &key.secret)
            .header("Accept", "application/json");
        if body.is_some() {
            request = request.header("Content-Type", "application/json");
        }
        let request = request.body(
            body.map(|body| AsyncBody::from(body.to_string()))
                .unwrap_or_default(),
        )?;
        let read = async {
            let mut response = self
                .http
                .send(request)
                .await
                .with_context(|| format!("requesting {url}"))?;
            let mut answer = Vec::new();
            response
                .body_mut()
                .read_to_end(&mut answer)
                .await
                .with_context(|| format!("reading {url}"))?;
            anyhow::Ok((response.status(), answer))
        };
        tokio::time::timeout(timeout, read)
            .await
            .map_err(|_| anyhow!("{url} didn't answer in {}s", timeout.as_secs()))?
    }

    /// The key agentZ made, or a new one when there's none yet or `refused` was turned down.
    /// The refused key is revoked once it's replaced.
    async fn key(&self, cli: &Cli, refused: Option<&ApiKey>) -> Result<ApiKey> {
        let mut kept = self.key.lock().await;
        if kept.is_none() {
            *kept = read_key(&self.key_file);
        }
        if let Some(key) = kept.as_ref()
            && refused.is_none_or(|refused| refused.secret != key.secret)
        {
            return Ok(key.clone());
        }
        let made = make_key(cli)
            .await
            .context("making agentZ's Instabox API key")?;
        write_key(&self.key_file, &made)?;
        if let Some(ApiKey { id: Some(id), .. }) = kept.replace(made.clone())
            && let Err(error) = cli.run(&["keys", "rm", &id], READ_TIMEOUT).await
        {
            log::warn!("revoking agentZ's previous Instabox API key: {error:#}");
        }
        Ok(made)
    }
}

async fn make_key(cli: &Cli) -> Result<ApiKey> {
    let stdout = cli
        .run(
            &[
                "keys",
                "create",
                "--name",
                KEY_NAME,
                "--scope",
                "machines:read",
                "--scope",
                "machines:write",
                "--json",
            ],
            READ_TIMEOUT,
        )
        .await?;
    parse_new_key(&stdout)
}

/// `keys create --json` prints the secret once. Its shape isn't documented, so the secret is
/// the string with Instabox's key prefix, and the id is beside it.
fn parse_new_key(json: &str) -> Result<ApiKey> {
    let value: serde_json::Value = serde_json::from_str(json).context("parsing the new key")?;
    let secret = find_string(&value, &|text| text.starts_with("ibk_"))
        .context("the new key has no secret")?
        .to_string();
    let id = key_id(&value, &secret).map(str::to_string);
    if id.is_none() {
        log::warn!("Instabox's new API key has no id, so it can't be revoked once replaced");
    }
    Ok(ApiKey { id, secret })
}

/// The `id` of the object that holds the secret, or of the answer itself.
fn key_id<'a>(value: &'a serde_json::Value, secret: &str) -> Option<&'a str> {
    let object = value.as_object()?;
    let holds_secret = object.values().any(|field| field.as_str() == Some(secret));
    let nested = || {
        object
            .values()
            .filter(|field| field.is_object())
            .find_map(|field| key_id(field, secret))
    };
    match object.get("id").and_then(serde_json::Value::as_str) {
        Some(id) if holds_secret => Some(id),
        Some(id) => nested().or(Some(id)),
        None => nested(),
    }
}

fn read_key(path: &Path) -> Option<ApiKey> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return None,
        Err(error) => {
            log::warn!("reading {}: {error}", path.display());
            return None;
        }
    };
    serde_json::from_str(&text)
        .map_err(|error| log::warn!("reading {}: {error}", path.display()))
        .ok()
}

/// Only the user can read it, as the CLIs keep their own logins.
fn write_key(path: &Path, key: &ApiKey) -> Result<()> {
    if let Some(folder) = path.parent() {
        std::fs::create_dir_all(folder).with_context(|| format!("making {}", folder.display()))?;
    }
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(path)
        .with_context(|| format!("writing {}", path.display()))?;
    file.set_permissions(std::fs::Permissions::from_mode(0o600))
        .with_context(|| format!("writing {}", path.display()))?;
    file.write_all(serde_json::to_string(key)?.as_bytes())
        .with_context(|| format!("writing {}", path.display()))
}

/// An error's message, as Instabox's API words it (`{"error": {"code", "message"}}`).
fn error_message(answer: &[u8]) -> Option<String> {
    let value: serde_json::Value = serde_json::from_slice(answer).ok()?;
    let error = value.get("error")?;
    error
        .get("message")
        .or(Some(error))
        .and_then(serde_json::Value::as_str)
        .filter(|message| !message.is_empty())
        .map(str::to_string)
}

#[derive(Deserialize)]
struct Metrics {
    #[serde(default)]
    metrics: Vec<Metric>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Metric {
    timestamp: String,
    #[serde(default)]
    cpu_used_pct: f64,
    #[serde(default)]
    mem_used: f64,
    #[serde(default)]
    mem_total: f64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Usage {
    from: String,
    #[serde(default)]
    running_seconds: f64,
    #[serde(default)]
    vcpu_seconds: Option<f64>,
}

#[derive(Deserialize)]
struct DesktopStatus {
    #[serde(default)]
    installed: bool,
    #[serde(default)]
    running: bool,
    #[serde(default)]
    installing: bool,
    #[serde(default)]
    failed: bool,
    #[serde(default)]
    log: Option<String>,
}

#[derive(Deserialize)]
struct DesktopLink {
    url: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_the_new_keys_secret_and_id() {
        let key = parse_new_key(
            r#"{"id": "key_1", "name": "agentZ", "key": "ibk_secret", "scopes": ["machines:read"]}"#,
        )
        .expect("parses");
        assert_eq!(key.id.as_deref(), Some("key_1"));
        assert_eq!(key.secret, "ibk_secret");
        let key = parse_new_key(
            r#"{"apiKey": {"id": "key_2", "name": "agentZ"}, "secret": "ibk_other"}"#,
        )
        .expect("parses");
        assert_eq!(key.id.as_deref(), Some("key_2"));
        assert!(parse_new_key(r#"{"id": "key_3"}"#).is_err());
    }

    #[test]
    fn says_what_the_api_said() {
        assert_eq!(
            error_message(br#"{"error": {"code": "not_found", "message": "no such machine"}}"#)
                .as_deref(),
            Some("no such machine")
        );
        assert_eq!(error_message(b"<html>"), None);
    }
}
