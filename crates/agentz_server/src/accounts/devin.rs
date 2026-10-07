//! Devin's accounts.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use agentz_protocol::accounts::{AccountStatus, ExtraUsage, LimitWindow};
use agentz_protocol::agents::AgentCommand;
use anyhow::{Context as _, Result};
use http_client::{
    AsyncBody, HttpClient, HttpRequestExt as _, Method, RedirectPolicy, Request, StatusCode,
};
use serde::Deserialize;

use super::login_checks::run_with_account_env;
use super::readers::{Read, format_number, send};
use super::{AgentDescription, LoggedIn, LoginCheck, Reader, SHARED_SKILLS_FOLDER, StatusCommand};

/// The ACP server is Devin's program with `acp`; its other commands are the program alone.
const ACP: &str = "acp";
const AUTH_STATUS: [&str; 2] = ["auth", "status"];
/// How `auth status` starts while logged in, whichever way ("Logged in (via Devin).").
const LOGGED_IN: &str = "Logged in";
/// Overrides the stored login.
const KEY_VARIABLE: &str = "WINDSURF_API_KEY";
/// Devin's login in its data folder.
const CREDENTIALS: &str = "devin/credentials.toml";
/// Devin's API server, when its login names none.
const API_SERVER: &str = "https://server.codeium.com";
const USER_STATUS: &str = "exa.seat_management_pb.SeatManagementService/GetUserStatus";
/// The client OpenUsage names itself when it asks.
const CLIENT_NAME: &str = "devin";
const CLIENT_VERSION: &str = "1.108.2";
/// Devin's `/usage` name for it.
const EXTRA_USAGE: &str = "Extra usage balance";
const DAY: Duration = Duration::from_secs(24 * 60 * 60);

/// Devin keeps its config, MCP servers and skills in `XDG_CONFIG_HOME`'s `devin`, and its
/// login and sessions in `XDG_DATA_HOME`'s. Both folders hold other programs' files too, which
/// the account's home links to, since the tools Devin runs see the same variables.
pub(super) fn description() -> AgentDescription {
    AgentDescription {
        home_variables: BTreeMap::from([
            ("XDG_CONFIG_HOME".into(), ".config".into()),
            ("XDG_DATA_HOME".into(), ".local/share".into()),
        ]),
        shared_folders: BTreeMap::from([
            (".config".into(), vec!["devin".into()]),
            (".local/share".into(), vec!["devin".into()]),
        ]),
        file_storage: BTreeMap::new(),
        home_files: BTreeMap::new(),
        // The model, permissions, hooks and keymap. The login is in the data folder.
        settings_files: vec![".config/devin/config.json".into()],
        // The organization `/org` picked, which another login may not belong to.
        login_settings: BTreeMap::from([(
            ".config/devin/config.json".into(),
            vec!["/devin/org_id".into()],
        )]),
        normal_home: String::new(),
        login_variables: vec![KEY_VARIABLE.into()],
        skills_folders: vec![".config/devin/skills".into()],
        outside_skills_folders: vec![SHARED_SKILLS_FOLDER.into(), ".claude/skills".into()],
        // Its sessions open while it's logged out, with no models.
        login_check: LoginCheck::Command(StatusCommand {
            program: None,
            args: AUTH_STATUS.map(String::from).to_vec(),
            after_agent_args: false,
            logged_in: LoggedIn::Prefix(LOGGED_IN.into()),
        }),
        reader: Some(Reader::DevinApi),
        // Its key login takes the key in `authenticate`.
        key_login: None,
        usage_page: Some("https://app.devin.ai/settings/usage".into()),
        extra_usage_page: None,
    }
}

/// Reads the account: who's logged in from `auth status`, then what Devin's `/usage` shows
/// from `GetUserStatus`, which Devin asks its API server for, sent the key Devin keeps.
pub(super) async fn read(agent: AgentCommand, http: Arc<dyn HttpClient>) -> Result<Read> {
    let args: Vec<String> = agent
        .args
        .iter()
        .take_while(|arg| *arg != ACP)
        .cloned()
        .chain(AUTH_STATUS.map(String::from))
        .collect();
    let output = run_with_account_env(None, &args, agent.clone()).await?;
    anyhow::ensure!(
        output.status.success(),
        "`devin auth status` exited with {}",
        output.status
    );
    let Some(identity) = auth_status(&String::from_utf8_lossy(&output.stdout)) else {
        return Ok(logged_out());
    };
    // Logged in some way that keeps no key, with no quota to read.
    let Some(login) = stored_login(&agent)? else {
        return Ok(Read {
            logged_in: Some(true),
            status: identity,
        });
    };
    let url = format!("{}/{USER_STATUS}", login.api_server);
    let body = serde_json::json!({
        "metadata": {
            "apiKey": login.key,
            "ideName": CLIENT_NAME,
            "ideVersion": CLIENT_VERSION,
            "extensionName": CLIENT_NAME,
            "extensionVersion": CLIENT_VERSION,
            "locale": "en",
        }
    });
    // Not redirected: the key is in the body, for the server the login names.
    let request = Request::builder()
        .method(Method::POST)
        .uri(&url)
        .header("Content-Type", "application/json")
        .header("Connect-Protocol-Version", "1")
        .follow_redirects(RedirectPolicy::NoFollow)
        .body(AsyncBody::from(body.to_string()))?;
    let (status, answer) = send(http, request).await?;
    // A key that's refused (or revoked) logs nothing in.
    if status == StatusCode::UNAUTHORIZED || status == StatusCode::FORBIDDEN {
        return Ok(logged_out());
    }
    anyhow::ensure!(status.is_success(), "Devin answered {status}");
    let quota = user_status(&answer)?;
    Ok(Read {
        logged_in: Some(true),
        status: AccountStatus {
            plan: identity.plan.clone().or(quota.plan),
            windows: quota.windows,
            extra_usage: quota.extra_usage,
            ..identity
        },
    })
}

fn logged_out() -> Read {
    Read {
        logged_in: Some(false),
        status: AccountStatus::default(),
    }
}

/// The name, email and plan `auth status` gives, if it's logged in.
fn auth_status(output: &str) -> Option<AccountStatus> {
    if !output.trim_start().starts_with(LOGGED_IN) {
        return None;
    }
    let field = |name: &str| {
        output.lines().find_map(|line| {
            let (key, value) = line.trim().split_once(':')?;
            let value = value.trim();
            (key == name && !value.is_empty()).then(|| value.to_string())
        })
    };
    Some(AccountStatus {
        email: field("Email"),
        name: field("Name"),
        plan: field("Plan"),
        ..AccountStatus::default()
    })
}

/// The key Devin sends its API server, and that server.
#[derive(Debug, PartialEq)]
struct DevinLogin {
    key: String,
    api_server: String,
}

#[derive(Default, Deserialize)]
#[serde(default)]
struct Credentials {
    windsurf_api_key: Option<String>,
    api_server_url: Option<String>,
}

/// Devin's login as it would use it: the key in `WINDSURF_API_KEY`, or else in its
/// `credentials.toml`, which is only read.
fn stored_login(agent: &AgentCommand) -> Result<Option<DevinLogin>> {
    let data = variable(agent, "XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| util::paths::home_dir().join(".local/share"));
    let path = data.join(CREDENTIALS);
    let credentials = match std::fs::read_to_string(&path) {
        Ok(text) => toml::from_str(&text).with_context(|| format!("reading {}", path.display()))?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Credentials::default(),
        Err(error) => return Err(error).with_context(|| format!("reading {}", path.display())),
    };
    let api_server = credentials
        .api_server_url
        .as_deref()
        .map(|url| url.trim().trim_end_matches('/'))
        .filter(|url| url.starts_with("https://"))
        .unwrap_or(API_SERVER)
        .to_string();
    let key = variable(agent, KEY_VARIABLE)
        .or(credentials.windsurf_api_key)
        .map(|key| key.trim().to_string())
        .filter(|key| !key.is_empty());
    Ok(key.map(|key| DevinLogin { key, api_server }))
}

/// A variable as the agent's run of the account sees it.
fn variable(agent: &AgentCommand, name: &str) -> Option<String> {
    if let Some(value) = agent.env.get(name) {
        return Some(value.clone());
    }
    if agent.env_remove.iter().any(|removed| removed == name) {
        return None;
    }
    std::env::var(name).ok()
}

/// `GetUserStatus`'s answer, as proto3 JSON writes it: zeros and falses are left out, and
/// 64-bit numbers are strings.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct UserStatusAnswer {
    user_status: UserStatus,
}

#[derive(Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct UserStatus {
    plan_status: PlanStatus,
}

#[derive(Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct PlanStatus {
    plan_info: PlanInfo,
    daily_quota_remaining_percent: Option<f64>,
    weekly_quota_remaining_percent: Option<f64>,
    daily_quota_reset_at_unix: Option<ProtoInt>,
    weekly_quota_reset_at_unix: Option<ProtoInt>,
    overage_balance_micros: Option<ProtoInt>,
}

#[derive(Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct PlanInfo {
    plan_name: Option<String>,
    hide_daily_quota: bool,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum ProtoInt {
    Number(i64),
    Text(String),
}

impl ProtoInt {
    fn value(&self) -> Result<i64> {
        match self {
            ProtoInt::Number(number) => Ok(*number),
            ProtoInt::Text(text) => text
                .parse()
                .with_context(|| format!("{text:?} isn't a number")),
        }
    }
}

/// What Devin's `/usage` shows: the Daily and Weekly quota, and the extra usage balance
/// once there's any.
fn user_status(body: &[u8]) -> Result<AccountStatus> {
    let answer: UserStatusAnswer =
        serde_json::from_slice(body).context("Devin's user status isn't in the expected shape")?;
    let plan = answer.user_status.plan_status;
    let daily = (!plan.plan_info.hide_daily_quota)
        .then(|| {
            quota_window(
                "Daily",
                plan.daily_quota_remaining_percent,
                plan.daily_quota_reset_at_unix.as_ref(),
                DAY,
            )
        })
        .transpose()?
        .flatten();
    let weekly = quota_window(
        "Weekly",
        plan.weekly_quota_remaining_percent,
        plan.weekly_quota_reset_at_unix.as_ref(),
        7 * DAY,
    )?;
    let balance = plan
        .overage_balance_micros
        .as_ref()
        .map(ProtoInt::value)
        .transpose()?
        .filter(|micros| *micros > 0);
    Ok(AccountStatus {
        plan: plan
            .plan_info
            .plan_name
            .map(|name| name.trim().to_string())
            .filter(|name| !name.is_empty()),
        windows: daily.into_iter().chain(weekly).collect(),
        extra_usage: balance.map(|micros| ExtraUsage {
            label: EXTRA_USAGE.into(),
            summary: format!("${}", format_number(micros as f64 / 1e6, 2, true)),
        }),
        ..AccountStatus::default()
    })
}

/// A window Devin gives as the percentage left, which it leaves out once none is: a window
/// that resets with no percentage is used up.
fn quota_window(
    label: &str,
    remaining_percent: Option<f64>,
    resets_at: Option<&ProtoInt>,
    length: Duration,
) -> Result<Option<LimitWindow>> {
    let resets_at = resets_at
        .map(ProtoInt::value)
        .transpose()?
        .and_then(|seconds| u64::try_from(seconds).ok())
        .filter(|seconds| *seconds > 0)
        .map(|seconds| SystemTime::UNIX_EPOCH + Duration::from_secs(seconds));
    let Some(remaining_percent) = remaining_percent.or(resets_at.map(|_| 0.0)) else {
        return Ok(None);
    };
    Ok(Some(LimitWindow {
        label: label.into(),
        used_percent: (100.0 - remaining_percent).clamp(0.0, 100.0),
        resets_at,
        length: Some(length),
    }))
}

#[cfg(test)]
mod tests {
    use futures::AsyncReadExt as _;
    use http_client::{FakeHttpClient, Response};

    use super::*;

    const AUTH_STATUS_LOGGED_IN: &str = include_str!("devin_reads/auth-status.txt");
    const AUTH_STATUS_LOGGED_OUT: &str = include_str!("devin_reads/auth-status-logged-out.txt");
    const USER_STATUS_ANSWER: &str = include_str!("devin_reads/user-status.json");

    fn at_unix(seconds: u64) -> Option<SystemTime> {
        Some(SystemTime::UNIX_EPOCH + Duration::from_secs(seconds))
    }

    #[test]
    fn auth_status_gives_who_is_logged_in() {
        assert_eq!(
            auth_status(AUTH_STATUS_LOGGED_IN),
            Some(AccountStatus {
                email: Some("work@example.com".into()),
                name: Some("Example".into()),
                plan: Some("Pro".into()),
                ..AccountStatus::default()
            })
        );
        assert_eq!(auth_status(AUTH_STATUS_LOGGED_OUT), None);
        // A login without a user, as a key's may be.
        let key_login = "Logged in (via API key).\n\nCredentials:\n  File: /x\n";
        assert_eq!(auth_status(key_login), Some(AccountStatus::default()));
    }

    #[test]
    fn user_status_gives_devins_usage() {
        let status = user_status(USER_STATUS_ANSWER.as_bytes()).expect("parse");
        assert_eq!(status.plan.as_deref(), Some("Pro"));
        assert_eq!(
            status.windows,
            [
                LimitWindow {
                    label: "Daily".into(),
                    used_percent: 12.0,
                    resets_at: at_unix(1_791_446_400),
                    length: Some(DAY),
                },
                LimitWindow {
                    label: "Weekly".into(),
                    used_percent: 39.0,
                    resets_at: at_unix(1_791_705_600),
                    length: Some(7 * DAY),
                },
            ]
        );
        assert_eq!(
            status.extra_usage,
            Some(ExtraUsage {
                label: "Extra usage balance".into(),
                summary: "$12.40".into(),
            })
        );

        // Used up, the weekly percentage is left out; a balance that's spent isn't shown.
        let used_up = USER_STATUS_ANSWER
            .replace(r#""weeklyQuotaRemainingPercent": 61,"#, "")
            .replace(r#""12400000""#, r#""-90354""#);
        let status = user_status(used_up.as_bytes()).expect("parse");
        assert_eq!(status.windows[1].used_percent, 100.0);
        assert_eq!(status.windows[1].resets_at, at_unix(1_791_705_600));
        assert_eq!(status.extra_usage, None);

        // A plan that hides the daily quota.
        let hidden = USER_STATUS_ANSWER.replace(
            r#""planName": "Pro","#,
            r#""planName": "Pro", "hideDailyQuota": true,"#,
        );
        let status = user_status(hidden.as_bytes()).expect("parse");
        let labels: Vec<&str> = status
            .windows
            .iter()
            .map(|window| window.label.as_str())
            .collect();
        assert_eq!(labels, ["Weekly"]);

        // Billed by credits, with no quota at all.
        let status =
            user_status(br#"{"userStatus": {"planStatus": {"planInfo": {}}}}"#).expect("parse");
        assert!(status.windows.is_empty());
        assert_eq!(status.plan, None);

        assert!(user_status(b"<html>").is_err());
        let changed = USER_STATUS_ANSWER.replace(
            r#""weeklyQuotaRemainingPercent": 61"#,
            r#""weeklyQuotaRemainingPercent": "61%""#,
        );
        assert!(user_status(changed.as_bytes()).is_err());
    }

    /// Devin's `auth status`, as the reader runs it, printing a captured output.
    const FAKE_DEVIN: &str = r#"
if [ "$1 $2" != "auth status" ]; then exit 2; fi
if [ -n "$DEVIN_LOGGED_OUT" ]; then
    cat "$DEVIN_READS/auth-status-logged-out.txt"
else
    cat "$DEVIN_READS/auth-status.txt"
fi
"#;

    #[tokio::test]
    async fn reads_devins_user_status_with_its_key() {
        let dir = tempfile::tempdir().expect("temp dir");
        let script = dir.path().join("fake_devin.sh");
        std::fs::write(&script, FAKE_DEVIN).expect("write the fake");
        let data = dir.path().join("data");
        std::fs::create_dir_all(data.join("devin")).expect("data");
        std::fs::write(
            data.join(CREDENTIALS),
            "windsurf_api_key = \"devin-stored\"\napi_server_url = \"https://devin.test/\"\n\
             devin_webapp_host = \"https://app.devin.ai\"\n",
        )
        .expect("write the login");
        let agent = AgentCommand {
            path: "/bin/sh".into(),
            args: vec![script.to_string_lossy().into_owned(), ACP.into()],
            env: [
                (
                    "DEVIN_READS",
                    concat!(env!("CARGO_MANIFEST_DIR"), "/src/accounts/devin_reads").to_string(),
                ),
                ("XDG_DATA_HOME", data.to_string_lossy().into_owned()),
            ]
            .into_iter()
            .map(|(variable, value)| (variable.to_string(), value))
            .collect(),
            env_remove: vec![KEY_VARIABLE.into()],
        };

        // Devin's server, as far as the reader asks it: what it was sent is kept.
        let sent = Arc::new(std::sync::Mutex::new(
            Vec::<(String, serde_json::Value)>::new(),
        ));
        let http = FakeHttpClient::create({
            let sent = sent.clone();
            move |mut request| {
                let sent = sent.clone();
                async move {
                    assert_eq!(request.method(), Method::POST);
                    assert_eq!(
                        request
                            .headers()
                            .get("Connect-Protocol-Version")
                            .and_then(|value| value.to_str().ok()),
                        Some("1")
                    );
                    let mut body = String::new();
                    request.body_mut().read_to_string(&mut body).await?;
                    let body: serde_json::Value = serde_json::from_str(&body)?;
                    let key = body["metadata"]["apiKey"].as_str().map(str::to_string);
                    sent.lock()
                        .expect("lock")
                        .push((request.uri().to_string(), body));
                    let (status, answer) = match key.as_deref() {
                        Some("devin-revoked") => (401, r#"{"code": "unauthenticated"}"#),
                        _ => (200, USER_STATUS_ANSWER),
                    };
                    Ok(Response::builder()
                        .status(status)
                        .body(AsyncBody::from(answer.to_string()))?)
                }
            }
        });

        let found = read(agent.clone(), http.clone()).await.expect("read");
        assert_eq!(found.logged_in, Some(true));
        assert_eq!(found.status.email.as_deref(), Some("work@example.com"));
        assert_eq!(found.status.name.as_deref(), Some("Example"));
        assert_eq!(found.status.plan.as_deref(), Some("Pro"));
        assert_eq!(found.status.windows.len(), 2);
        assert!(found.status.extra_usage.is_some());
        let (url, body) = sent.lock().expect("lock").remove(0);
        assert_eq!(url, format!("https://devin.test/{USER_STATUS}"));
        assert_eq!(
            body,
            serde_json::json!({"metadata": {
                "apiKey": "devin-stored",
                "ideName": "devin",
                "ideVersion": "1.108.2",
                "extensionName": "devin",
                "extensionVersion": "1.108.2",
                "locale": "en",
            }})
        );

        // The key variable overrides the stored login.
        let mut with_key = agent.clone();
        with_key
            .env
            .insert(KEY_VARIABLE.into(), "devin-revoked".into());
        let refused = read(with_key, http.clone()).await.expect("read");
        assert_eq!(refused.logged_in, Some(false));
        assert_eq!(
            sent.lock().expect("lock").remove(0).1["metadata"]["apiKey"],
            "devin-revoked"
        );

        // Logged out, nothing is asked.
        let mut logged_out = agent.clone();
        logged_out.env.insert("DEVIN_LOGGED_OUT".into(), "1".into());
        let found = read(logged_out, http.clone()).await.expect("read");
        assert_eq!(found.logged_in, Some(false));

        // Logged in with no key kept, there's only who it is.
        std::fs::remove_file(data.join(CREDENTIALS)).expect("remove");
        let found = read(agent, http).await.expect("read");
        assert_eq!(found.logged_in, Some(true));
        assert_eq!(found.status.email.as_deref(), Some("work@example.com"));
        assert!(found.status.windows.is_empty());
        assert!(sent.lock().expect("lock").is_empty());
    }

    #[test]
    fn the_login_names_only_https_servers() {
        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::create_dir_all(dir.path().join("devin")).expect("data");
        let agent = AgentCommand {
            env: [(
                "XDG_DATA_HOME".to_string(),
                dir.path().to_string_lossy().into_owned(),
            )]
            .into_iter()
            .collect(),
            env_remove: vec![KEY_VARIABLE.into()],
            ..AgentCommand::default()
        };
        let write = |text: &str| {
            std::fs::write(dir.path().join(CREDENTIALS), text).expect("write the login");
        };
        write("windsurf_api_key = \"devin-stored\"\napi_server_url = \"http://devin.test\"\n");
        assert_eq!(
            stored_login(&agent).expect("login"),
            Some(DevinLogin {
                key: "devin-stored".into(),
                api_server: API_SERVER.into(),
            })
        );
        write("windsurf_api_key = \"\"\n");
        assert_eq!(stored_login(&agent).expect("login"), None);
        write("windsurf_api_key = [");
        assert!(stored_login(&agent).is_err());
    }
}
