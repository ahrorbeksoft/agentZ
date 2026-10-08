//! Google Antigravity's accounts.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use agentz_protocol::accounts::{AccountStatus, LimitPool, LimitWindow};
use agentz_protocol::agents::AgentCommand;
use anyhow::{Context as _, Result, anyhow, bail};
use base64::Engine as _;
use http_client::{
    AsyncBody, HttpClient, HttpRequestExt as _, Method, RedirectPolicy, Request, StatusCode,
};
use serde::Deserialize;

use super::login_checks::{account_command, run_with_account_env};
use super::readers::{Read, account_variable, send};
use super::{AgentDescription, KeyLogin, LoginCheck, Reader};

const HOME_VARIABLE: &str = "GEMINI_HOME";
const FILE_STORAGE_VARIABLE: &str = "AGY_ACP_FORCE_FILE_STORAGE";
/// The ACP server's Google login in its home, with file storage.
const LOGIN_FILE: &str = "antigravity-acp/acp_token.json";
/// Where the ACP server renews its login.
const TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
/// Google's Cloud Code API, which Antigravity asks for its quota and tier.
const CLOUD_CODE: &str = "https://cloudcode-pa.googleapis.com/v1internal";
const QUOTA_SUMMARY: &str = "retrieveUserQuotaSummary";
const LOAD_CODE_ASSIST: &str = "loadCodeAssist";
const USER_INFO_URL: &str = "https://www.googleapis.com/oauth2/v3/userinfo";
/// What OpenUsage names itself when it asks.
const USER_AGENT: &str = "antigravity";
/// The Antigravity CLI's Google login, in the keychain entry its Go keyring library writes
/// with `security`, which may then read it without macOS asking the user.
const KEYCHAIN_SERVICE: &str = "gemini";
const CLI_KEYCHAIN_ACCOUNT: &str = "antigravity";
const SECURITY: &str = "/usr/bin/security";
/// `security`'s exit status when there's no such entry.
const NOT_IN_KEYCHAIN: i32 = 44;
const GO_KEYRING_PREFIX: &str = "go-keyring-base64:";
const CLI: &str = "agy";
/// Print mode's `/usage`, which runs no turn and starts no conversation. The CLI renews its
/// login before it asks, if it's due.
const CLI_USAGE: [&str; 4] = ["-p", "/usage", "--output-format", "json"];
/// A CLI login with less left than this is renewed first.
const RENEW_BEFORE: Duration = Duration::from_secs(60);
const CLI_TIMEOUT: Duration = Duration::from_secs(60);
/// The CLI opens a login page with `open` from `PATH` (and nothing else), which this one, put
/// first, keeps from showing: a login it can't renew starts Google's in the user's browser.
const NO_BROWSER: &str = "#!/bin/sh\n\
    # agentZ: the Antigravity CLI renews its login here, where no login page opens.\n\
    exit 0\n";
const FIVE_HOURS: Duration = Duration::from_secs(5 * 60 * 60);
const WEEK: Duration = Duration::from_secs(7 * 24 * 60 * 60);

/// The ACP server keeps its login, sessions, MCP servers, hooks and skills in `GEMINI_HOME`, or
/// else `~/.gemini`.
pub(super) fn description() -> AgentDescription {
    AgentDescription {
        home_variables: BTreeMap::from([(HOME_VARIABLE.into(), String::new())]),
        shared_folders: BTreeMap::new(),
        external_links: Vec::new(),
        // Without it, the login goes to one keychain entry every home shares, where the
        // user's own is. With it, the login is `antigravity-acp/acp_token.json` in the home.
        file_storage: BTreeMap::from([(FILE_STORAGE_VARIABLE.into(), "1".into())]),
        // Never its `antigravity-acp/settings.json`: with a login method named there and none
        // stored, `session/new` waits up to 5 minutes on a browser login.
        home_files: BTreeMap::new(),
        // Its MCP servers and hooks. Its own `settings.json` holds only the login's method and
        // project.
        settings_files: vec!["config/mcp_config.json".into(), "config/hooks.json".into()],
        login_settings: BTreeMap::new(),
        normal_home: ".gemini".into(),
        // Each logs it in by another method than Google's.
        login_variables: [
            "GEMINI_API_KEY",
            "GOOGLE_API_KEY",
            "GOOGLE_APPLICATION_CREDENTIALS",
            "GOOGLE_CLOUD_PROJECT",
            "GOOGLE_CLOUD_LOCATION",
            "AGY_LLM_GATEWAY_URL",
            "AGY_LLM_GATEWAY_API_KEY",
            "AGY_LLM_GATEWAY_HEADERS",
            "AGY_GATEWAY_URL",
            "AGY_GATEWAY_API_KEY",
            "AGY_GATEWAY_HEADERS",
            "AGY_ACP_CCPA_PROJECT",
        ]
        .map(String::from)
        .to_vec(),
        skills_folders: vec!["config/skills".into(), "antigravity-cli/skills".into()],
        // It reads no skills outside its home but the project's.
        outside_skills_folders: Vec::new(),
        // Logged out, `session/new` fails with "Authentication required".
        login_check: LoginCheck::Session,
        // Nothing over ACP gives the identity or quota: Google's APIs do.
        reader: Some(Reader::GoogleCloudCode),
        // "Gemini API key" reads `GEMINI_API_KEY` when `authenticate` brings none.
        key_login: Some(KeyLogin {
            method: "gemini-api-key".into(),
            variable: "GEMINI_API_KEY".into(),
            reader: None,
        }),
        usage_page: None,
        extra_usage_page: None,
    }
}

/// Reads the account from Google's APIs, as OpenUsage reads Antigravity's: the quota and tier
/// from Cloud Code, and who it is from Google's user info, each sent an access token from the
/// account's stored Google login.
pub(super) async fn read(
    agent: AgentCommand,
    http: Arc<dyn HttpClient>,
    folder: &Path,
) -> Result<Read> {
    let cli = Cli::find(&agent);
    read_with(agent, http, folder, &cli).await
}

async fn read_with(
    agent: AgentCommand,
    http: Arc<dyn HttpClient>,
    folder: &Path,
    cli: &Cli,
) -> Result<Read> {
    let (token, logged_in) = if uses_file_storage(&agent) {
        let Some(login) = file_login(&agent)? else {
            // Logged in another way (a key, a gateway), or out, as the login check tells.
            return Ok(Read::default());
        };
        match access_token(http.clone(), &login).await? {
            Some(token) => (token, Some(true)),
            None => {
                return Ok(Read {
                    logged_in: Some(false),
                    status: AccountStatus::default(),
                });
            }
        }
    } else {
        // Without file storage, the ACP server keeps its login in a keychain entry that opens
        // only for it: anything else reading it has macOS ask the user. The user's Antigravity
        // CLI keeps theirs where it may be read, so that stands in for it, with no say on
        // whether the ACP server is logged in.
        match cli.access_token(&agent, folder).await? {
            Some(token) => (token, None),
            None => return Ok(Read::default()),
        }
    };
    let quota = post_cloud_code(http.clone(), &token, QUOTA_SUMMARY).await?;
    let code_assist = post_cloud_code(http.clone(), &token, LOAD_CODE_ASSIST).await?;
    let user = user_info(http, &token).await?;
    Ok(Read {
        logged_in,
        status: AccountStatus {
            email: user.email,
            name: user.name,
            plan: plan(&code_assist)?,
            ..quota_summary(&quota)?
        },
    })
}

fn uses_file_storage(agent: &AgentCommand) -> bool {
    account_variable(agent, FILE_STORAGE_VARIABLE)
        .is_some_and(|value| value == "1" || value.eq_ignore_ascii_case("true"))
}

/// What the ACP server keeps of its Google login: the client it logged in with, and the
/// refresh token.
#[derive(Deserialize)]
struct FileLogin {
    client_id: String,
    client_secret: String,
    refresh_token: String,
}

/// The ACP server's login in the account's home, which is only read.
fn file_login(agent: &AgentCommand) -> Result<Option<FileLogin>> {
    let home = account_variable(agent, HOME_VARIABLE)
        .filter(|home| !home.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| util::paths::home_dir().join(".gemini"));
    let path = home.join(LOGIN_FILE);
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error).with_context(|| format!("reading {}", path.display())),
    };
    // Not serde's error, which may quote the login.
    serde_json::from_str(&text)
        .map(Some)
        .map_err(|_| anyhow!("{} isn't in the expected shape", path.display()))
}

#[derive(Deserialize)]
struct TokenAnswer {
    access_token: String,
}

#[derive(Default, Deserialize)]
#[serde(default)]
struct TokenError {
    error: Option<String>,
}

/// An access token for the login, got as the ACP server gets one each time it starts. Google
/// keeps the refresh token as it is, so the login stays as the agent stored it, and nothing is
/// written back. `None` when Google refuses the login, revoked or expired, which the ACP server
/// couldn't renew either.
async fn access_token(http: Arc<dyn HttpClient>, login: &FileLogin) -> Result<Option<String>> {
    let form = url::form_urlencoded::Serializer::new(String::new())
        .append_pair("client_id", &login.client_id)
        .append_pair("client_secret", &login.client_secret)
        .append_pair("refresh_token", &login.refresh_token)
        .append_pair("grant_type", "refresh_token")
        .finish();
    let request = Request::builder()
        .method(Method::POST)
        .uri(TOKEN_URL)
        .header("Content-Type", "application/x-www-form-urlencoded")
        .header("Accept", "application/json")
        .follow_redirects(RedirectPolicy::NoFollow)
        .body(AsyncBody::from(form))?;
    let (status, answer) = send(http, request).await?;
    if !status.is_success() {
        let error = serde_json::from_slice::<TokenError>(&answer)
            .unwrap_or_default()
            .error;
        if matches!(status, StatusCode::BAD_REQUEST | StatusCode::UNAUTHORIZED)
            && error.as_deref() == Some("invalid_grant")
        {
            return Ok(None);
        }
        bail!(
            "Google answered {status}{}",
            error.map(|error| format!(": {error}")).unwrap_or_default()
        );
    }
    let answer: TokenAnswer = serde_json::from_slice(&answer)
        .map_err(|_| anyhow!("Google's access token isn't in the expected shape"))?;
    Ok(Some(answer.access_token))
}

/// The Antigravity CLI, whose login stands in for the External account's.
struct Cli {
    security: PathBuf,
    /// `None` while it isn't installed.
    program: Option<PathBuf>,
}

impl Cli {
    /// `agy` on the account's `PATH`, or where its installer puts it.
    fn find(agent: &AgentCommand) -> Self {
        let path = account_variable(agent, "PATH").unwrap_or_default();
        let program = std::env::split_paths(&path)
            .map(|folder| folder.join(CLI))
            .chain([util::paths::home_dir().join(".local/bin").join(CLI)])
            .find(|candidate| candidate.is_file());
        Cli {
            security: SECURITY.into(),
            program,
        }
    }

    /// An access token from the CLI's login, which the CLI renews first when it's about to
    /// expire. `None` without a login.
    async fn access_token(&self, agent: &AgentCommand, folder: &Path) -> Result<Option<String>> {
        let Some(login) = self.login(agent).await? else {
            return Ok(None);
        };
        if login.expires_at > SystemTime::now() + RENEW_BEFORE {
            return Ok(Some(login.access_token));
        }
        self.renew(agent, folder).await?;
        let login = self
            .login(agent)
            .await?
            .context("the Antigravity CLI has no login")?;
        anyhow::ensure!(
            login.expires_at > SystemTime::now(),
            "the Antigravity CLI didn't renew its login"
        );
        Ok(Some(login.access_token))
    }

    async fn login(&self, agent: &AgentCommand) -> Result<Option<CliLogin>> {
        let args = [
            "find-generic-password",
            "-s",
            KEYCHAIN_SERVICE,
            "-a",
            CLI_KEYCHAIN_ACCOUNT,
            "-w",
        ]
        .map(String::from);
        let output =
            run_with_account_env(Some(&self.security.to_string_lossy()), &args, agent.clone())
                .await?;
        if output.status.code() == Some(NOT_IN_KEYCHAIN) {
            return Ok(None);
        }
        anyhow::ensure!(
            output.status.success(),
            "`security` exited with {}",
            output.status
        );
        cli_login(&String::from_utf8_lossy(&output.stdout)).map(Some)
    }

    /// Has the CLI renew its own login, in `folder`, with `open` and `BROWSER` going nowhere.
    async fn renew(&self, agent: &AgentCommand, folder: &Path) -> Result<()> {
        let program = self
            .program
            .as_ref()
            .context("the Antigravity CLI (`agy`), which renews its login, isn't installed")?;
        let bin = folder.join("bin");
        std::fs::create_dir_all(&bin).with_context(|| format!("creating {}", bin.display()))?;
        let open = bin.join("open");
        crate::browser::write_executable(&open, NO_BROWSER)?;
        let mut agent = agent.clone();
        let bin_text = bin.to_string_lossy().into_owned();
        let path = match account_variable(&agent, "PATH").filter(|path| !path.is_empty()) {
            Some(path) => format!("{bin_text}:{path}"),
            None => bin_text,
        };
        agent.env.insert("PATH".into(), path);
        agent
            .env
            .insert("BROWSER".into(), open.to_string_lossy().into_owned());
        let mut command = account_command(program, &CLI_USAGE.map(String::from), &agent);
        command.current_dir(folder).stdin(Stdio::null());
        let output = tokio::time::timeout(CLI_TIMEOUT, command.output())
            .await
            .map_err(|_| {
                anyhow!(
                    "the Antigravity CLI didn't finish in {}s",
                    CLI_TIMEOUT.as_secs()
                )
            })?
            .context("running the Antigravity CLI")?;
        if !output.status.success() {
            let error = serde_json::from_slice::<serde_json::Value>(&output.stdout)
                .ok()
                .and_then(|answer| Some(answer.get("error")?.as_str()?.to_string()));
            bail!(
                "the Antigravity CLI exited with {}{}",
                output.status,
                error.map(|error| format!(": {error}")).unwrap_or_default()
            );
        }
        Ok(())
    }
}

#[derive(Debug, PartialEq)]
struct CliLogin {
    access_token: String,
    expires_at: SystemTime,
}

#[derive(Deserialize)]
struct CliKeychainValue {
    token: CliToken,
}

#[derive(Deserialize)]
struct CliToken {
    access_token: String,
    expiry: String,
}

/// The CLI's keychain value: its token as JSON, which the Go keyring library may have written
/// in base64.
fn cli_login(value: &str) -> Result<CliLogin> {
    let value = value.trim();
    let json = match value.strip_prefix(GO_KEYRING_PREFIX) {
        Some(encoded) => base64::engine::general_purpose::STANDARD
            .decode(encoded)
            .ok()
            .and_then(|bytes| String::from_utf8(bytes).ok())
            .context("the Antigravity CLI's login isn't base64")?,
        None => value.to_string(),
    };
    // Not serde's error, which may quote the login.
    let value: CliKeychainValue = serde_json::from_str(&json)
        .map_err(|_| anyhow!("the Antigravity CLI's login isn't in the expected shape"))?;
    let expires_at = chrono::DateTime::parse_from_rfc3339(&value.token.expiry)
        .with_context(|| format!("the CLI's login expires at {:?}", value.token.expiry))?;
    Ok(CliLogin {
        access_token: value.token.access_token,
        expires_at: expires_at.into(),
    })
}

async fn post_cloud_code(http: Arc<dyn HttpClient>, token: &str, method: &str) -> Result<Vec<u8>> {
    let request = Request::builder()
        .method(Method::POST)
        .uri(format!("{CLOUD_CODE}:{method}"))
        .header("Authorization", format!("Bearer {token}"))
        .header("Accept", "application/json")
        .header("Content-Type", "application/json")
        .header("User-Agent", USER_AGENT)
        .follow_redirects(RedirectPolicy::NoFollow)
        .body(AsyncBody::from("{}".to_string()))?;
    let (status, answer) = send(http, request).await?;
    anyhow::ensure!(
        status.is_success(),
        "Google's Cloud Code answered {status} to {method}"
    );
    Ok(answer)
}

#[derive(Default, Deserialize)]
#[serde(default)]
struct UserInfo {
    email: Option<String>,
    name: Option<String>,
}

async fn user_info(http: Arc<dyn HttpClient>, token: &str) -> Result<UserInfo> {
    let request = Request::builder()
        .method(Method::GET)
        .uri(USER_INFO_URL)
        .header("Authorization", format!("Bearer {token}"))
        .header("Accept", "application/json")
        .follow_redirects(RedirectPolicy::NoFollow)
        .body(AsyncBody::default())?;
    let (status, answer) = send(http, request).await?;
    anyhow::ensure!(status.is_success(), "Google's user info answered {status}");
    serde_json::from_slice(&answer).context("Google's user info isn't in the expected shape")
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CodeAssist {
    paid_tier: Option<Tier>,
    current_tier: Option<Tier>,
}

#[derive(Deserialize)]
struct Tier {
    name: Option<String>,
}

/// The tier `loadCodeAssist` names, the paid one first ("Google AI Pro" over the free
/// "Antigravity"), as OpenUsage reads it.
fn plan(body: &[u8]) -> Result<Option<String>> {
    let answer: CodeAssist =
        serde_json::from_slice(body).context("Google's tier isn't in the expected shape")?;
    Ok([answer.paid_tier, answer.current_tier]
        .into_iter()
        .flatten()
        .filter_map(|tier| tier.name)
        .map(|name| name.trim().to_string())
        .find(|name| !name.is_empty()))
}

/// `retrieveUserQuotaSummary`'s answer: groups of models ("Gemini Models", "Claude and GPT
/// models"), each with its own 5-hour and weekly window.
#[derive(Deserialize)]
struct QuotaSummary {
    #[serde(default)]
    groups: Vec<QuotaGroup>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct QuotaGroup {
    display_name: String,
    #[serde(default)]
    buckets: Vec<QuotaBucket>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct QuotaBucket {
    display_name: Option<String>,
    window: Option<String>,
    reset_time: Option<String>,
    remaining_fraction: Option<f64>,
}

/// The first group's windows, as the account's pool, and the other groups as its other pools.
fn quota_summary(body: &[u8]) -> Result<AccountStatus> {
    let summary: QuotaSummary = serde_json::from_slice(body)
        .context("Google's quota summary isn't in the expected shape")?;
    let mut pools = summary.groups.into_iter().map(|group| {
        let mut windows: Vec<LimitWindow> =
            group.buckets.into_iter().filter_map(quota_window).collect();
        windows.sort_by_key(|window| window.length.unwrap_or(Duration::MAX));
        LimitPool {
            label: group.display_name,
            windows,
        }
    });
    let first = pools
        .next()
        .context("Google's quota summary has no groups")?;
    Ok(AccountStatus {
        windows: first.windows,
        pool: Some(first.label),
        other_pools: pools.collect(),
        ..AccountStatus::default()
    })
}

fn quota_window(bucket: QuotaBucket) -> Option<LimitWindow> {
    let (label, length) = match bucket.window.as_deref() {
        Some("5h") => ("5-hour".to_string(), Some(FIVE_HOURS)),
        Some("weekly") => ("Weekly".to_string(), Some(WEEK)),
        _ => (bucket.display_name.or(bucket.window)?, None),
    };
    let resets_at = bucket
        .reset_time
        .as_deref()
        .and_then(|text| chrono::DateTime::parse_from_rfc3339(text).ok())
        .map(SystemTime::from);
    // Proto3's JSON leaves zeros out: a window that resets with no fraction left is used up.
    let remaining = bucket
        .remaining_fraction
        .or(resets_at.map(|_| 0.0))?
        .clamp(0.0, 1.0);
    Some(LimitWindow {
        label,
        used_percent: (1.0 - remaining) * 100.0,
        // A window nothing was used in starts at the first use: its reset is only now plus its
        // length.
        resets_at: resets_at.filter(|_| remaining < 1.0),
        length,
    })
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::Mutex;

    use futures::AsyncReadExt as _;
    use http_client::{FakeHttpClient, Response};

    use super::*;

    /// What the ACP server 1.3.0 answered in a new home.
    const INITIALIZE: &str = include_str!("antigravity_reads/initialize.json");
    /// What Google answered a login with Google AI Pro, some of the Claude and GPT models'
    /// weekly window used.
    const QUOTA_SUMMARY_ANSWER: &str = include_str!("antigravity_reads/quota-summary.json");
    const LOAD_CODE_ASSIST_ANSWER: &str = include_str!("antigravity_reads/load-code-assist.json");
    const USER_INFO_ANSWER: &str = include_str!("antigravity_reads/userinfo.json");

    fn at(text: &str) -> Option<SystemTime> {
        Some(
            chrono::DateTime::parse_from_rfc3339(text)
                .expect("time")
                .into(),
        )
    }

    fn json(text: &str) -> serde_json::Value {
        serde_json::from_str(text).expect("json")
    }

    #[test]
    fn the_quota_summary_gives_each_group_its_windows() {
        let status = quota_summary(QUOTA_SUMMARY_ANSWER.as_bytes()).expect("parse");
        assert_eq!(status.pool.as_deref(), Some("Gemini Models"));
        // Nothing used yet, so no reset until the first use.
        assert_eq!(
            status.windows,
            [
                LimitWindow {
                    label: "5-hour".into(),
                    used_percent: 0.0,
                    resets_at: None,
                    length: Some(FIVE_HOURS),
                },
                LimitWindow {
                    label: "Weekly".into(),
                    used_percent: 0.0,
                    resets_at: None,
                    length: Some(WEEK),
                },
            ]
        );
        let [other] = status.other_pools.as_slice() else {
            panic!("expected one other pool: {:?}", status.other_pools);
        };
        assert_eq!(other.label, "Claude and GPT models");
        let labels: Vec<&str> = other
            .windows
            .iter()
            .map(|window| window.label.as_str())
            .collect();
        assert_eq!(labels, ["5-hour", "Weekly"]);
        let weekly = &other.windows[1];
        assert!((weekly.used_percent - 5.50232).abs() < 1e-3);
        assert_eq!(weekly.left_percent(), 94);
        assert_eq!(weekly.resets_at, at("2026-10-10T10:27:11Z"));

        // Used up, the fraction is left out.
        let mut used_up = json(QUOTA_SUMMARY_ANSWER);
        used_up["groups"][1]["buckets"][0]
            .as_object_mut()
            .expect("bucket")
            .remove("remainingFraction");
        let status = quota_summary(used_up.to_string().as_bytes()).expect("parse");
        let weekly = &status.other_pools[0].windows[1];
        assert_eq!(weekly.used_percent, 100.0);
        assert_eq!(weekly.resets_at, at("2026-10-10T10:27:11Z"));

        // A window it doesn't know goes by Google's name for it, after those it knows.
        let mut unknown = json(QUOTA_SUMMARY_ANSWER);
        unknown["groups"][0]["buckets"][0]["window"] = "monthly".into();
        let status = quota_summary(unknown.to_string().as_bytes()).expect("parse");
        let labels: Vec<&str> = status
            .windows
            .iter()
            .map(|window| window.label.as_str())
            .collect();
        assert_eq!(labels, ["5-hour", "Weekly Limit Remaining"]);
        assert_eq!(status.windows[1].length, None);

        assert!(quota_summary(b"<html>").is_err());
        assert!(quota_summary(br#"{"groups": []}"#).is_err());
    }

    #[test]
    fn the_plan_is_the_paid_tier_first() {
        assert_eq!(
            plan(LOAD_CODE_ASSIST_ANSWER.as_bytes())
                .expect("parse")
                .as_deref(),
            Some("Google AI Pro")
        );
        let mut free = json(LOAD_CODE_ASSIST_ANSWER);
        free.as_object_mut().expect("answer").remove("paidTier");
        assert_eq!(
            plan(free.to_string().as_bytes()).expect("parse").as_deref(),
            Some("Antigravity")
        );
        assert_eq!(plan(b"{}").expect("parse"), None);
    }

    /// The CLI's keychain value, as its Go keyring library writes it.
    fn keychain_value(access_token: &str, expires_at: SystemTime) -> String {
        let expiry = chrono::DateTime::<chrono::Utc>::from(expires_at).to_rfc3339();
        let token = serde_json::json!({
            "token": {
                "access_token": access_token,
                "token_type": "Bearer",
                "refresh_token": "1//cli",
                "expiry": expiry,
            },
            "auth_method": "oauth",
            "id_token": "header.payload.signature",
        });
        format!(
            "{GO_KEYRING_PREFIX}{}\n",
            base64::engine::general_purpose::STANDARD.encode(token.to_string())
        )
    }

    #[test]
    fn the_clis_login_is_read_as_its_keyring_writes_it() {
        let expires_at = at("2026-10-08T06:16:34.681125+05:00").expect("time");
        assert_eq!(
            cli_login(&keychain_value("ya29.cli", expires_at)).expect("login"),
            CliLogin {
                access_token: "ya29.cli".into(),
                expires_at,
            }
        );
        let plain = r#"{"token": {"access_token": "ya29.cli", "expiry": "2026-10-08T01:16:34Z"}}"#;
        assert_eq!(cli_login(plain).expect("login").access_token, "ya29.cli");
        assert!(cli_login("go-keyring-base64:not base64").is_err());
        let error = cli_login(r#"{"token": {"access_token": 2970, "expiry": ""}}"#)
            .expect_err("a token that isn't text");
        assert!(!error.to_string().contains("2970"), "{error}");
    }

    /// Google, as far as the reader asks it: what it was sent is kept, the refresh token for
    /// an access token, and each request's access token.
    fn fake_google(sent: Arc<Mutex<Vec<String>>>) -> Arc<dyn HttpClient> {
        FakeHttpClient::create(move |mut request| {
            let sent = sent.clone();
            async move {
                let url = request.uri().to_string();
                let authorization = request
                    .headers()
                    .get("Authorization")
                    .and_then(|value| value.to_str().ok())
                    .unwrap_or_default()
                    .to_string();
                let mut body = String::new();
                request.body_mut().read_to_string(&mut body).await?;
                let (status, answer) = if url == TOKEN_URL {
                    let form: HashMap<String, String> =
                        url::form_urlencoded::parse(body.as_bytes())
                            .into_owned()
                            .collect();
                    assert_eq!(form["grant_type"], "refresh_token");
                    assert_eq!(form["client_id"], "client.apps.googleusercontent.com");
                    assert_eq!(form["client_secret"], "client-secret");
                    let refresh_token = &form["refresh_token"];
                    sent.lock()
                        .expect("lock")
                        .push(format!("token for {refresh_token}"));
                    match refresh_token.as_str() {
                        "1//revoked" => (
                            400,
                            r#"{"error": "invalid_grant", "error_description": "Token has been expired or revoked."}"#,
                        ),
                        _ => (
                            200,
                            r#"{"access_token": "ya29.minted", "expires_in": 3599, "token_type": "Bearer"}"#,
                        ),
                    }
                } else {
                    let method = url
                        .strip_prefix(&format!("{CLOUD_CODE}:"))
                        .unwrap_or(&url)
                        .to_string();
                    if method != USER_INFO_URL {
                        assert_eq!(request.method(), Method::POST);
                        assert_eq!(body, "{}");
                        assert_eq!(
                            request
                                .headers()
                                .get("User-Agent")
                                .and_then(|value| value.to_str().ok()),
                            Some(USER_AGENT)
                        );
                    }
                    sent.lock()
                        .expect("lock")
                        .push(format!("{method} with {authorization}"));
                    match method.as_str() {
                        QUOTA_SUMMARY => (200, QUOTA_SUMMARY_ANSWER),
                        LOAD_CODE_ASSIST => (200, LOAD_CODE_ASSIST_ANSWER),
                        USER_INFO_URL => (200, USER_INFO_ANSWER),
                        _ => (404, ""),
                    }
                };
                Ok(Response::builder()
                    .status(status)
                    .body(AsyncBody::from(answer.to_string()))?)
            }
        })
    }

    fn requests_with(token: &str) -> Vec<String> {
        [QUOTA_SUMMARY, LOAD_CODE_ASSIST, USER_INFO_URL]
            .map(|method| format!("{method} with Bearer {token}"))
            .to_vec()
    }

    fn assert_read_the_fixtures(found: &Read) {
        assert_eq!(found.status.email.as_deref(), Some("work@example.com"));
        assert_eq!(found.status.name.as_deref(), Some("Example Person"));
        assert_eq!(found.status.plan.as_deref(), Some("Google AI Pro"));
        assert_eq!(found.status.pool.as_deref(), Some("Gemini Models"));
        assert_eq!(found.status.windows.len(), 2);
        assert_eq!(found.status.other_pools.len(), 1);
    }

    #[tokio::test]
    async fn an_account_gets_an_access_token_from_the_login_in_its_home() {
        let dir = tempfile::tempdir().expect("temp dir");
        let home = dir.path().join("home");
        std::fs::create_dir_all(home.join("antigravity-acp")).expect("home");
        let agent = AgentCommand {
            env: [
                (HOME_VARIABLE, home.to_string_lossy().into_owned()),
                (FILE_STORAGE_VARIABLE, "1".into()),
            ]
            .into_iter()
            .map(|(variable, value)| (variable.to_string(), value))
            .collect(),
            ..AgentCommand::default()
        };
        // The CLI is never asked about an account's login.
        let cli = Cli {
            security: "/nonexistent/security".into(),
            program: None,
        };
        let folder = dir.path().join("reader");
        let sent = Arc::new(Mutex::new(Vec::new()));
        let http = fake_google(sent.clone());
        let take = || std::mem::take(&mut *sent.lock().expect("lock"));

        // Logged out, or in with a key: nothing to read, and nothing is asked.
        let found = read_with(agent.clone(), http.clone(), &folder, &cli)
            .await
            .expect("read");
        assert_eq!(found, Read::default());
        assert!(take().is_empty());

        let login_file = home.join(LOGIN_FILE);
        let write_login = |refresh_token: &str| {
            let login = serde_json::json!({
                "client_id": "client.apps.googleusercontent.com",
                "client_secret": "client-secret",
                "refresh_token": refresh_token,
                "token_uri": "https://oauth2.googleapis.com/token",
                "scopes": ["https://www.googleapis.com/auth/cloud-platform"],
            });
            let text = login.to_string();
            std::fs::write(&login_file, &text).expect("write the login");
            text
        };
        let stored = write_login("1//stored");
        let found = read_with(agent.clone(), http.clone(), &folder, &cli)
            .await
            .expect("read");
        assert_eq!(found.logged_in, Some(true));
        assert_read_the_fixtures(&found);
        let mut expected = vec!["token for 1//stored".to_string()];
        expected.extend(requests_with("ya29.minted"));
        assert_eq!(take(), expected);
        // The login is left as the agent stored it.
        assert_eq!(
            std::fs::read_to_string(&login_file).expect("read the login"),
            stored
        );

        // A login Google refuses is logged out.
        write_login("1//revoked");
        let found = read_with(agent.clone(), http.clone(), &folder, &cli)
            .await
            .expect("read");
        assert_eq!(found.logged_in, Some(false));
        assert_eq!(take(), ["token for 1//revoked"]);

        std::fs::write(&login_file, r#"{"refresh_token": 2970}"#).expect("write");
        let error = read_with(agent, http, &folder, &cli)
            .await
            .expect_err("a login it can't make out");
        assert!(!format!("{error:#}").contains("2970"), "{error:#}");
    }

    /// `security`, as the reader runs it: the CLI's entry is a file, missing while there's
    /// none.
    const FAKE_SECURITY: &str = r#"#!/bin/sh
[ "$*" = "find-generic-password -s gemini -a antigravity -w" ] || exit 2
[ -f "$FAKE_KEYCHAIN" ] || exit 44
cat "$FAKE_KEYCHAIN"
"#;

    /// The CLI's print mode `/usage`, renewing its login if a renewed one is there, and noting
    /// the `open` and `BROWSER` it would open a login page with.
    const FAKE_CLI: &str = r#"#!/bin/sh
[ "$*" = "-p /usage --output-format json" ] || exit 2
{ command -v open; echo "$BROWSER"; } > "$FAKE_CLI_LOG"
if [ -f "$FAKE_RENEWED" ]; then
    cp "$FAKE_RENEWED" "$FAKE_KEYCHAIN"
    echo '{"status": "SUCCESS"}'
else
    echo '{"status": "ERROR", "error": "authentication failed or timed out"}'
    exit 1
fi
"#;

    #[tokio::test]
    async fn the_external_account_reads_the_clis_login_which_the_cli_renews() {
        let dir = tempfile::tempdir().expect("temp dir");
        let security = dir.path().join("security");
        crate::browser::write_executable(&security, FAKE_SECURITY).expect("write the fake");
        let program = dir.path().join("agy");
        crate::browser::write_executable(&program, FAKE_CLI).expect("write the fake");
        let keychain = dir.path().join("keychain");
        let renewed = dir.path().join("renewed");
        let log = dir.path().join("cli.log");
        let agent = AgentCommand {
            env: [
                ("FAKE_KEYCHAIN", &keychain),
                ("FAKE_RENEWED", &renewed),
                ("FAKE_CLI_LOG", &log),
            ]
            .into_iter()
            .map(|(variable, path)| (variable.to_string(), path.to_string_lossy().into_owned()))
            .chain([("PATH".to_string(), "/usr/bin:/bin".to_string())])
            .collect(),
            env_remove: vec![HOME_VARIABLE.into(), FILE_STORAGE_VARIABLE.into()],
            ..AgentCommand::default()
        };
        let cli = Cli {
            security,
            program: Some(program),
        };
        let folder = dir.path().join("reader");
        let sent = Arc::new(Mutex::new(Vec::new()));
        let http = fake_google(sent.clone());
        let take = || std::mem::take(&mut *sent.lock().expect("lock"));
        let hour = Duration::from_secs(60 * 60);

        // Without a login, nothing is asked.
        let found = read_with(agent.clone(), http.clone(), &folder, &cli)
            .await
            .expect("read");
        assert_eq!(found, Read::default());
        assert!(take().is_empty());

        // A login good for a while yet is used as it is. It says nothing of whether the ACP
        // server is logged in.
        std::fs::write(
            &keychain,
            keychain_value("ya29.cli", SystemTime::now() + hour),
        )
        .expect("write");
        let found = read_with(agent.clone(), http.clone(), &folder, &cli)
            .await
            .expect("read");
        assert_eq!(found.logged_in, None);
        assert_read_the_fixtures(&found);
        assert_eq!(take(), requests_with("ya29.cli"));
        assert!(!log.exists());

        // One about to expire, the CLI renews first, where a login page goes nowhere.
        std::fs::write(
            &keychain,
            keychain_value("ya29.old", SystemTime::now() + Duration::from_secs(30)),
        )
        .expect("write");
        std::fs::write(
            &renewed,
            keychain_value("ya29.renewed", SystemTime::now() + hour),
        )
        .expect("write");
        let found = read_with(agent.clone(), http.clone(), &folder, &cli)
            .await
            .expect("read");
        assert_read_the_fixtures(&found);
        assert_eq!(take(), requests_with("ya29.renewed"));
        let no_browser = folder.join("bin/open").to_string_lossy().into_owned();
        assert_eq!(
            std::fs::read_to_string(&log).expect("the CLI's log"),
            format!("{no_browser}\n{no_browser}\n")
        );

        // A CLI that can't renew it fails the read, which keeps the last one.
        std::fs::write(
            &keychain,
            keychain_value("ya29.old", SystemTime::now() - hour),
        )
        .expect("write");
        std::fs::remove_file(&renewed).expect("remove");
        let error = read_with(agent.clone(), http.clone(), &folder, &cli)
            .await
            .expect_err("no renewed login");
        assert!(
            format!("{error:#}").contains("authentication failed or timed out"),
            "{error:#}"
        );
        let uninstalled = Cli {
            program: None,
            ..cli
        };
        assert!(read_with(agent, http, &folder, &uninstalled).await.is_err());
        assert!(take().is_empty());
    }

    #[test]
    fn its_key_login_is_one_of_its_methods() {
        let initialize: serde_json::Value = serde_json::from_str(INITIALIZE).expect("json");
        let methods: Vec<&str> = initialize["authMethods"]
            .as_array()
            .expect("methods")
            .iter()
            .filter_map(|method| method["id"].as_str())
            .collect();
        let key_login = description().key_login.expect("key login");
        assert!(methods.contains(&key_login.method.as_str()), "{methods:?}");
    }

    #[test]
    fn accounts_keep_their_login_in_their_home() {
        let mut command = AgentCommand {
            env: [
                ("GEMINI_API_KEY".to_string(), "outside".to_string()),
                ("GOOGLE_CLOUD_PROJECT".to_string(), "outside".to_string()),
            ]
            .into_iter()
            .collect(),
            ..AgentCommand::default()
        };
        let home = Path::new("/data/accounts/antigravity-acp/2");
        description().apply(&mut command, BTreeMap::new(), home, Some("key".into()));
        assert_eq!(
            command.env.get("GEMINI_HOME").map(String::as_str),
            Some("/data/accounts/antigravity-acp/2")
        );
        assert_eq!(
            command
                .env
                .get("AGY_ACP_FORCE_FILE_STORAGE")
                .map(String::as_str),
            Some("1")
        );
        assert_eq!(
            command.env.get("GEMINI_API_KEY").map(String::as_str),
            Some("key")
        );
        assert!(!command.env.contains_key("GOOGLE_CLOUD_PROJECT"));
        assert!(
            command
                .env_remove
                .contains(&"GOOGLE_CLOUD_PROJECT".to_string())
        );
        // Nothing names a login method before one is used.
        let description = description();
        assert!(description.home_files.is_empty());
        assert!(
            !description
                .settings_files
                .iter()
                .any(|path| path.starts_with("antigravity-acp"))
        );
    }
}
