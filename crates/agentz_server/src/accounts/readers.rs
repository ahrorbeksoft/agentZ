//! How agentZ reads an account's identity and limits (plan.md › Agent descriptions, the
//! readers' kinds). Each kind comes with the first agent that needs it.

use std::sync::Arc;
use std::time::{Duration, SystemTime};

use agentz_protocol::accounts::{AccountStatus, LimitWindow};
use agentz_protocol::agents::AgentCommand;
use anyhow::{Context as _, Result, anyhow, bail};
use futures::AsyncReadExt as _;
use http_client::{AsyncBody, HttpClient, Method, Request, StatusCode};
use serde::{Deserialize, Serialize};

use super::login_checks::run_with_account_env;

/// How long an HTTP reader waits for its answer.
const HTTP_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Reader {
    /// A command that prints a [`Read`] as JSON, run with the account's environment, as the
    /// mock agent's `--usage` does.
    Command(ReaderCommand),
    /// Factory's billing API, called with the account's `FACTORY_API_KEY` (plan.md, reader
    /// kind 6), as Droid's `/limits` calls it.
    FactoryApi { base_url: String },
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ReaderCommand {
    /// A path, or a name looked up on `PATH`. Without one, the agent's own program runs, with
    /// `args` in place of its arguments.
    pub program: Option<String>,
    pub args: Vec<String>,
}

/// What a reader found.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Read {
    /// Whether the account is logged in, when the reader can tell.
    pub logged_in: Option<bool>,
    #[serde(flatten)]
    pub status: AccountStatus,
}

impl Reader {
    /// Reads the account whose agent command is `agent`.
    pub async fn read(&self, agent: AgentCommand, http: Arc<dyn HttpClient>) -> Result<Read> {
        match self {
            Reader::Command(command) => {
                let output =
                    run_with_account_env(command.program.as_deref(), &command.args, agent).await?;
                if !output.status.success() {
                    bail!("it exited with {}", output.status);
                }
                serde_json::from_slice(&output.stdout).context("its output isn't a read")
            }
            Reader::FactoryApi { base_url } => {
                let key = agent
                    .env
                    .get("FACTORY_API_KEY")
                    .context("the account has no Factory API key")?;
                let url = format!("{}/api/billing/limits", base_url.trim_end_matches('/'));
                let (status, body) = get_with_key(http, &url, key).await?;
                // As for Droid, a key that's refused (or revoked) logs nothing in.
                if status == StatusCode::UNAUTHORIZED {
                    return Ok(Read {
                        logged_in: Some(false),
                        status: AccountStatus::default(),
                    });
                }
                if !status.is_success() {
                    bail!("Factory answered {status}");
                }
                Ok(Read {
                    logged_in: Some(true),
                    status: factory_limits(&body, SystemTime::now())?,
                })
            }
        }
    }
}

async fn get_with_key(
    http: Arc<dyn HttpClient>,
    url: &str,
    key: &str,
) -> Result<(StatusCode, Vec<u8>)> {
    let request = Request::builder()
        .method(Method::GET)
        .uri(url)
        .header("Authorization", format!("Bearer {key}"))
        .header("Accept", "application/json")
        .body(AsyncBody::default())?;
    let read = async {
        let mut response = http
            .send(request)
            .await
            .with_context(|| format!("requesting {url}"))?;
        let mut body = Vec::new();
        response
            .body_mut()
            .read_to_end(&mut body)
            .await
            .with_context(|| format!("reading {url}"))?;
        anyhow::Ok((response.status(), body))
    };
    tokio::time::timeout(HTTP_TIMEOUT, read)
        .await
        .map_err(|_| anyhow!("{url} didn't answer in {}s", HTTP_TIMEOUT.as_secs()))?
}

/// `GET /api/billing/limits`, as Droid 0.234.0's `/limits` reads it: windows for its Standard
/// Usage (and Droid Core, its other pool) and the Extra Usage balance.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct FactoryLimits {
    limits: Option<FactoryPools>,
    extra_usage_balance_cents: Option<f64>,
}

#[derive(Deserialize)]
struct FactoryPools {
    standard: Option<FactoryWindows>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct FactoryWindows {
    five_hour: Option<FactoryWindow>,
    weekly: Option<FactoryWindow>,
    monthly: Option<FactoryWindow>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct FactoryWindow {
    used_percent: f64,
    window_end: Option<FactoryTime>,
}

/// Droid reads it with `new Date(…)`, which takes either.
#[derive(Deserialize)]
#[serde(untagged)]
enum FactoryTime {
    Text(String),
    Milliseconds(f64),
}

impl FactoryTime {
    fn time(&self) -> Option<SystemTime> {
        match self {
            FactoryTime::Text(text) => chrono::DateTime::parse_from_rfc3339(text)
                .ok()
                .map(SystemTime::from),
            FactoryTime::Milliseconds(milliseconds) if *milliseconds >= 0.0 => {
                Some(SystemTime::UNIX_EPOCH + Duration::from_secs_f64(milliseconds / 1000.0))
            }
            FactoryTime::Milliseconds(_) => None,
        }
    }
}

/// The Standard Usage windows, by Droid's names for them. Droid Core's pool waits for its tabs
/// (decisions.md §8).
fn factory_limits(body: &[u8], now: SystemTime) -> Result<AccountStatus> {
    let limits: FactoryLimits =
        serde_json::from_slice(body).context("Factory's limits aren't in the expected shape")?;
    let standard = limits
        .limits
        .and_then(|pools| pools.standard)
        .context("Factory's answer has no Standard Usage limits")?;
    let windows = [
        ("5-hour", standard.five_hour),
        ("Weekly", standard.weekly),
        ("Monthly", standard.monthly),
    ]
    .into_iter()
    .filter_map(|(label, window)| {
        let window = window?;
        let ends_at = window.window_end.as_ref().and_then(FactoryTime::time);
        // A window that ended hasn't started again: Droid shows "Use Droid to start".
        let active = ends_at.filter(|ends_at| *ends_at > now);
        Some(LimitWindow {
            label: label.to_string(),
            used_percent: if active.is_some() {
                window.used_percent
            } else {
                0.0
            },
            resets_at: active,
        })
    })
    .collect();
    let credits = limits
        .extra_usage_balance_cents
        .filter(|cents| *cents > 0.0)
        .map(|cents| format!("${:.2}", cents / 100.0));
    Ok(AccountStatus {
        windows,
        credits,
        ..AccountStatus::default()
    })
}

#[cfg(test)]
mod tests {
    use http_client::{BlockedHttpClient, FakeHttpClient, Response};

    use super::*;

    fn no_http() -> Arc<dyn HttpClient> {
        Arc::new(BlockedHttpClient)
    }

    fn shell(script: &str) -> Reader {
        Reader::Command(ReaderCommand {
            program: Some("/bin/sh".into()),
            args: vec!["-c".into(), script.into()],
        })
    }

    #[tokio::test]
    async fn commands_print_what_they_read() {
        let agent = AgentCommand {
            env: [("PLAN".to_string(), "Pro".to_string())]
                .into_iter()
                .collect(),
            ..AgentCommand::default()
        };
        let read = shell(
            r#"echo "{\"logged_in\": true, \"email\": \"work@example.com\", \"plan\": \"$PLAN\",
                \"windows\": [{\"label\": \"5-hour\", \"used_percent\": 40,
                \"resets_at\": {\"secs_since_epoch\": 60, \"nanos_since_epoch\": 0}}]}""#,
        )
        .read(agent.clone(), no_http())
        .await
        .expect("read");
        assert_eq!(read.logged_in, Some(true));
        assert_eq!(read.status.email.as_deref(), Some("work@example.com"));
        assert_eq!(read.status.plan.as_deref(), Some("Pro"));
        let [window] = &read.status.windows[..] else {
            panic!("expected a window");
        };
        assert_eq!(window.label, "5-hour");
        assert_eq!(window.used_percent, 40.0);
        assert_eq!(
            window.resets_at,
            Some(SystemTime::UNIX_EPOCH + Duration::from_secs(60))
        );

        let logged_out = shell(r#"echo '{"logged_in": false}'"#)
            .read(agent.clone(), no_http())
            .await
            .expect("read");
        assert_eq!(logged_out.logged_in, Some(false));
        assert!(logged_out.status.windows.is_empty());

        assert!(
            shell("echo '{}'; exit 1")
                .read(agent.clone(), no_http())
                .await
                .is_err()
        );
        assert!(
            shell("echo 5-hour: 40%")
                .read(agent, no_http())
                .await
                .is_err()
        );
    }

    /// Shaped as Droid 0.234.0's code reads `GET /api/billing/limits`: no real answer was seen
    /// yet. The weekly window ended, and the monthly one counts in milliseconds.
    const FACTORY_LIMITS: &str = r#"{
        "limits": {
            "standard": {
                "fiveHour": {"usedPercent": 42, "windowEnd": "2026-10-07T12:30:00Z"},
                "weekly": {"usedPercent": 100, "windowEnd": "2026-10-01T00:00:00Z"},
                "monthly": {"usedPercent": 12.5, "windowEnd": 1793491200000}
            },
            "core": {
                "fiveHour": {"usedPercent": 3, "windowEnd": "2026-10-07T12:30:00Z"}
            }
        },
        "extraUsageBalanceCents": 1240
    }"#;

    #[test]
    fn factory_limits_name_droids_windows() {
        let now = SystemTime::from(
            chrono::DateTime::parse_from_rfc3339("2026-10-07T10:00:00Z").expect("time"),
        );
        let status = factory_limits(FACTORY_LIMITS.as_bytes(), now).expect("parse");
        let windows: Vec<(&str, f64, bool)> = status
            .windows
            .iter()
            .map(|window| {
                (
                    window.label.as_str(),
                    window.used_percent,
                    window.resets_at.is_some(),
                )
            })
            .collect();
        assert_eq!(
            windows,
            [
                ("5-hour", 42.0, true),
                ("Weekly", 0.0, false),
                ("Monthly", 12.5, true)
            ]
        );
        assert_eq!(
            status.windows[0].resets_at,
            Some(now + Duration::from_secs(150 * 60))
        );
        assert_eq!(status.credits.as_deref(), Some("$12.40"));
        assert_eq!(status.email, None);

        assert!(factory_limits(br#"{"limits": {}}"#, now).is_err());
        assert!(factory_limits(b"<html>", now).is_err());
    }

    #[tokio::test]
    async fn the_factory_api_takes_the_accounts_key() {
        let http = FakeHttpClient::create(|request| async move {
            let authorization = request
                .headers()
                .get("Authorization")
                .and_then(|value| value.to_str().ok())
                .map(str::to_string);
            let (status, body) = match authorization.as_deref() {
                _ if request.uri().path() != "/api/billing/limits" => (404, String::new()),
                Some("Bearer fk-good") => (200, FACTORY_LIMITS.to_string()),
                _ => (401, r#"{"error": "unauthorized"}"#.to_string()),
            };
            Ok(Response::builder()
                .status(status)
                .body(AsyncBody::from(body))?)
        });
        let reader = Reader::FactoryApi {
            base_url: "https://api.factory.test/".into(),
        };
        let with_key = |key: &str| AgentCommand {
            env: [("FACTORY_API_KEY".to_string(), key.to_string())]
                .into_iter()
                .collect(),
            ..AgentCommand::default()
        };

        let read = reader
            .read(with_key("fk-good"), http.clone())
            .await
            .expect("read");
        assert_eq!(read.logged_in, Some(true));
        assert_eq!(read.status.windows.len(), 3);

        let refused = reader
            .read(with_key("fk-revoked"), http.clone())
            .await
            .expect("read");
        assert_eq!(refused.logged_in, Some(false));

        assert!(reader.read(AgentCommand::default(), http).await.is_err());
    }
}
