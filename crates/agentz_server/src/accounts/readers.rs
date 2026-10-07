//! How agentZ reads an account's identity and limits (plan.md › Agent descriptions, the
//! readers' kinds). Each kind comes with the first agent that needs it.

use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use agentz_protocol::accounts::{
    AccountStatus, LimitPool, LimitWindow, Overage, OveragePreference,
};
use agentz_protocol::agents::AgentCommand;
use anyhow::{Context as _, Result, anyhow, bail};
use futures::AsyncReadExt as _;
use http_client::{AsyncBody, HttpClient, Method, Request, StatusCode};
use serde::{Deserialize, Serialize};

use super::droid::{CORE_POOL, STANDARD_POOL, WINDOW_LABELS, WINDOW_LENGTHS};
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
    /// Droid's `/status` and `/limits`, in its terminal UI run where nobody sees it (plan.md,
    /// reader kind 5).
    DroidTerminal,
    /// Claude Code's `auth status` and its `get_usage` control request, through the adapter's
    /// `--cli` (plan.md, reader kinds 1 and 2).
    ClaudeCode,
    /// Codex's `account/read` and `account/rateLimits/read`, from its app-server run through
    /// the adapter's `cli` (plan.md, reader kind 2).
    CodexAppServer,
    /// Devin's `auth status`, then the `GetUserStatus` Devin asks its API server for, sent the
    /// key Devin keeps, as OpenUsage reads it (plan.md, reader kinds 1 and 7).
    DevinApi,
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
    /// Reads the account whose agent command is `agent`. A reader that runs the agent's own UI
    /// runs it in `folder` (the agent's [`super::reader_folder`]).
    pub async fn read(
        &self,
        agent: AgentCommand,
        http: Arc<dyn HttpClient>,
        folder: &Path,
    ) -> Result<Read> {
        match self {
            Reader::DroidTerminal => super::droid::read(agent, folder).await,
            Reader::ClaudeCode => super::claude::read(agent, folder).await,
            Reader::CodexAppServer => super::codex::read(agent, folder).await,
            Reader::DevinApi => super::devin::read(agent, http).await,
            Reader::Command(command) => {
                let output =
                    run_with_account_env(command.program.as_deref(), &command.args, agent).await?;
                if !output.status.success() {
                    bail!("it exited with {}", output.status);
                }
                serde_json::from_slice(&output.stdout).context("its output isn't a read")
            }
            Reader::FactoryApi { base_url } => {
                let key = factory_key(&agent)?;
                let url = format!("{}/api/billing/limits", base_url.trim_end_matches('/'));
                let (status, body) = send_with_key(http, Method::GET, &url, key, None).await?;
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

    /// Droid's "Switch to Droid Core" for the account (decisions.md §8), then a read, which
    /// says whether it took.
    pub async fn switch_to_droid_core(
        &self,
        agent: AgentCommand,
        http: Arc<dyn HttpClient>,
        folder: &Path,
    ) -> Result<Read> {
        match self {
            Reader::DroidTerminal => super::droid::switch_to_droid_core(agent, folder).await,
            // As Droid's `/limits` saves it.
            Reader::FactoryApi { base_url } => {
                let key = factory_key(&agent)?;
                let url = format!(
                    "{}/api/organization/subscription/set-overage-preference",
                    base_url.trim_end_matches('/')
                );
                let body = serde_json::json!({ "overagePreference": "droidCore" }).to_string();
                let (status, answer) =
                    send_with_key(http.clone(), Method::POST, &url, key, Some(body)).await?;
                if !status.is_success() {
                    let message = serde_json::from_slice::<serde_json::Value>(&answer)
                        .ok()
                        .and_then(|answer| Some(answer.get("message")?.as_str()?.to_string()));
                    bail!(
                        "Factory answered {status}{}",
                        message
                            .map(|message| format!(": {message}"))
                            .unwrap_or_default()
                    );
                }
                self.read(agent, http, folder).await
            }
            // The mock agent's `--usage` saves the preference it's given.
            Reader::Command(_) => {
                let mut agent = agent;
                agent
                    .env
                    .insert("AGENTZ_OVERAGE_PREFERENCE".into(), "DroidCore".into());
                self.read(agent, http, folder).await
            }
            Reader::ClaudeCode | Reader::CodexAppServer | Reader::DevinApi => {
                bail!("only Factory Droid has Droid Core")
            }
        }
    }

    /// Uses one of the account's limit resets (decisions.md §9), then reads it. `attempt` is
    /// the same for every try of one use, so a retry can't spend a second reset.
    pub async fn use_limit_reset(
        &self,
        agent: AgentCommand,
        http: Arc<dyn HttpClient>,
        folder: &Path,
        attempt: &str,
    ) -> Result<LimitResetUse> {
        match self {
            Reader::CodexAppServer => super::codex::use_limit_reset(agent, folder, attempt).await,
            // The mock agent's `--usage` uses one when given the attempt, and prints the outcome.
            Reader::Command(command) => {
                let mut using = agent.clone();
                using
                    .env
                    .insert("AGENTZ_LIMIT_RESET_ATTEMPT".into(), attempt.into());
                let output =
                    run_with_account_env(command.program.as_deref(), &command.args, using).await?;
                if !output.status.success() {
                    bail!("it exited with {}", output.status);
                }
                let answer: OutcomeAnswer = serde_json::from_slice(&output.stdout)
                    .context("its output isn't an outcome")?;
                Ok(LimitResetUse {
                    outcome: answer.outcome,
                    read: self.read(agent, http, folder).await,
                })
            }
            Reader::FactoryApi { .. }
            | Reader::DroidTerminal
            | Reader::ClaudeCode
            | Reader::DevinApi => bail!("only Codex has limit resets"),
        }
    }
}

/// What came of using a limit reset, and the read after it.
pub struct LimitResetUse {
    pub outcome: LimitResetOutcome,
    /// Apart from the outcome: the reset is used even if the read after it fails.
    pub read: Result<Read>,
}

/// Codex's `ConsumeAccountRateLimitResetCreditOutcome`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum LimitResetOutcome {
    /// One was used, and the limits it covers cleared.
    Reset,
    /// No limit is used enough to reset.
    NothingToReset,
    /// The account has none left.
    NoCredit,
    /// This attempt already used one.
    AlreadyRedeemed,
}

#[derive(Deserialize)]
struct OutcomeAnswer {
    outcome: LimitResetOutcome,
}

fn factory_key(agent: &AgentCommand) -> Result<&str> {
    agent
        .env
        .get("FACTORY_API_KEY")
        .map(String::as_str)
        .context("the account has no Factory API key")
}

async fn send_with_key(
    http: Arc<dyn HttpClient>,
    method: Method,
    url: &str,
    key: &str,
    json: Option<String>,
) -> Result<(StatusCode, Vec<u8>)> {
    let mut request = Request::builder()
        .method(method)
        .uri(url)
        .header("Authorization", format!("Bearer {key}"))
        .header("Accept", "application/json");
    if json.is_some() {
        request = request.header("Content-Type", "application/json");
    }
    let request = request.body(json.map(AsyncBody::from).unwrap_or_default())?;
    send(http, request).await
}

/// Sends `request` and reads the whole answer.
pub(super) async fn send(
    http: Arc<dyn HttpClient>,
    request: Request<AsyncBody>,
) -> Result<(StatusCode, Vec<u8>)> {
    let url = request.uri().to_string();
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

/// `GET /api/billing/limits`, as Droid 0.235.0's `/limits` reads it: windows for its Standard
/// Usage and Droid Core, its other pool, the Extra Usage balance, and Droid's "When limit is
/// reached" (decisions.md §8).
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct FactoryLimits {
    limits: Option<FactoryPools>,
    extra_usage_balance_cents: Option<f64>,
    overage_preference: Option<String>,
    can_manage_overage: Option<bool>,
    extra_usage_allowed: Option<bool>,
}

#[derive(Deserialize)]
struct FactoryPools {
    standard: Option<FactoryWindows>,
    core: Option<FactoryWindows>,
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
    window_end: Option<String>,
}

/// The Standard Usage windows, by Droid's names for them, with Droid Core's as another pool.
fn factory_limits(body: &[u8], now: SystemTime) -> Result<AccountStatus> {
    let limits: FactoryLimits =
        serde_json::from_slice(body).context("Factory's limits aren't in the expected shape")?;
    let FactoryPools { standard, core } =
        limits.limits.context("Factory's answer has no limits")?;
    let standard = standard.context("Factory's answer has no Standard Usage limits")?;
    let credits = limits
        .extra_usage_balance_cents
        .filter(|cents| *cents > 0.0)
        .map(|cents| format!("${:.2}", cents / 100.0));
    // As Droid reads it: anything else is no choice yet.
    let preference = match limits.overage_preference.as_deref() {
        Some("droidCore") => Some(OveragePreference::DroidCore),
        Some("extraUsage") => Some(OveragePreference::ExtraUsage),
        _ => None,
    };
    Ok(AccountStatus {
        windows: factory_windows(standard, now),
        pool: Some(STANDARD_POOL.into()),
        other_pools: core
            .map(|core| LimitPool {
                label: CORE_POOL.into(),
                windows: factory_windows(core, now),
            })
            .into_iter()
            .collect(),
        credits,
        overage: Some(Overage {
            preference,
            can_change: limits.can_manage_overage == Some(true),
            extra_usage_allowed: limits.extra_usage_allowed == Some(true),
        }),
        ..AccountStatus::default()
    })
}

fn factory_windows(windows: FactoryWindows, now: SystemTime) -> Vec<LimitWindow> {
    WINDOW_LABELS
        .into_iter()
        .zip(WINDOW_LENGTHS)
        .zip([windows.five_hour, windows.weekly, windows.monthly])
        .filter_map(|((label, length), window)| {
            let window = window?;
            let ends_at = window
                .window_end
                .as_deref()
                .and_then(|text| chrono::DateTime::parse_from_rfc3339(text).ok())
                .map(SystemTime::from);
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
                length: Some(length),
            })
        })
        .collect()
}

/// A non-negative `amount` as JavaScript's `toLocaleString("en-US")` writes it with at most
/// `decimals` places, as the agents' own screens do: "1,240", "12.4" with 1, "12.40" with
/// `fixed`.
pub(super) fn format_number(amount: f64, decimals: usize, fixed: bool) -> String {
    let text = format!("{:.*}", decimals, amount.max(0.));
    let (whole, fraction) = text.split_once('.').unwrap_or((&text, ""));
    let fraction = if fixed {
        fraction
    } else {
        fraction.trim_end_matches('0')
    };
    let mut grouped = String::new();
    for (index, digit) in whole.chars().enumerate() {
        if index > 0 && (whole.len() - index).is_multiple_of(3) {
            grouped.push(',');
        }
        grouped.push(digit);
    }
    if fraction.is_empty() {
        grouped
    } else {
        format!("{grouped}.{fraction}")
    }
}

#[cfg(test)]
mod tests {
    use http_client::{BlockedHttpClient, FakeHttpClient, Response};

    use super::*;

    fn no_http() -> Arc<dyn HttpClient> {
        Arc::new(BlockedHttpClient)
    }

    /// Commands and HTTP readers run nothing in the reader's folder.
    fn unused_folder() -> &'static Path {
        Path::new("/nonexistent")
    }

    fn shell(script: &str) -> Reader {
        Reader::Command(ReaderCommand {
            program: Some("/bin/sh".into()),
            args: vec!["-c".into(), script.into()],
        })
    }

    #[test]
    fn numbers_are_written_as_the_agents_write_them() {
        assert_eq!(format_number(0., 2, true), "0.00");
        assert_eq!(format_number(228.6, 2, true), "228.60");
        assert_eq!(format_number(1_234_567.891, 2, true), "1,234,567.89");
        assert_eq!(format_number(1240., 2, false), "1,240");
        assert_eq!(format_number(12.5, 2, false), "12.5");
        assert_eq!(format_number(999.999, 2, false), "1,000");
        assert_eq!(format_number(-3., 0, false), "0");
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
        .read(agent.clone(), no_http(), unused_folder())
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
            .read(agent.clone(), no_http(), unused_folder())
            .await
            .expect("read");
        assert_eq!(logged_out.logged_in, Some(false));
        assert!(logged_out.status.windows.is_empty());

        assert!(
            shell("echo '{}'; exit 1")
                .read(agent.clone(), no_http(), unused_folder())
                .await
                .is_err()
        );
        assert!(
            shell("echo 5-hour: 40%")
                .read(agent, no_http(), unused_folder())
                .await
                .is_err()
        );
    }

    /// A real answer of `GET /api/billing/limits` (October 2026), with other numbers: the
    /// weekly window ended.
    const FACTORY_LIMITS: &str = r#"{
        "usesTokenRateLimitsBilling": true,
        "limits": {
            "standard": {
                "fiveHour": {"usedPercent": 42, "windowEnd": "2026-10-07T12:30:00.000Z",
                    "secondsRemaining": 9000},
                "weekly": {"usedPercent": 100, "windowEnd": "2026-10-01T00:00:00.000Z",
                    "secondsRemaining": 0},
                "monthly": {"usedPercent": 12.5, "windowEnd": "2026-11-01T19:56:38.307Z",
                    "secondsRemaining": 2195798}
            },
            "core": {
                "fiveHour": {"usedPercent": 3, "windowEnd": "2026-10-07T12:30:00.000Z",
                    "secondsRemaining": 9000},
                "weekly": {"usedPercent": 4, "windowEnd": "2026-10-13T20:21:42.847Z",
                    "secondsRemaining": 555702},
                "monthly": {"usedPercent": 3, "windowEnd": "2026-11-05T20:21:42.847Z",
                    "secondsRemaining": 2542902}
            }
        },
        "overagePreference": "droidCore",
        "canManageOverage": true,
        "extraUsageBalanceCents": 1240,
        "extraUsageAllowed": true
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
        assert_eq!(status.pool.as_deref(), Some("Standard"));
        let [core] = &status.other_pools[..] else {
            panic!("expected Droid Core's pool");
        };
        assert_eq!(core.label, "Droid Core");
        let used: Vec<f64> = core
            .windows
            .iter()
            .map(|window| window.used_percent)
            .collect();
        assert_eq!(used, [3.0, 4.0, 3.0]);
        assert_eq!(
            status.overage,
            Some(Overage {
                preference: Some(OveragePreference::DroidCore),
                can_change: true,
                extra_usage_allowed: true,
            })
        );

        // An organization's member, with nothing chosen.
        let member = FACTORY_LIMITS
            .replace(
                r#""overagePreference": "droidCore""#,
                r#""overagePreference": null"#,
            )
            .replace(
                r#""canManageOverage": true"#,
                r#""canManageOverage": false"#,
            );
        let status = factory_limits(member.as_bytes(), now).expect("parse");
        assert_eq!(
            status.overage,
            Some(Overage {
                preference: None,
                can_change: false,
                extra_usage_allowed: true,
            })
        );

        assert!(factory_limits(br#"{"limits": {}}"#, now).is_err());
        assert!(factory_limits(b"<html>", now).is_err());
    }

    #[tokio::test]
    async fn the_factory_api_takes_the_accounts_key() {
        // What the fake Factory keeps: the preference it was last sent.
        let preference = Arc::new(std::sync::Mutex::new(None::<String>));
        let http = FakeHttpClient::create({
            let preference = preference.clone();
            move |mut request| {
                let preference = preference.clone();
                async move {
                    let authorization = request
                        .headers()
                        .get("Authorization")
                        .and_then(|value| value.to_str().ok())
                        .map(str::to_string);
                    let mut sent = String::new();
                    request.body_mut().read_to_string(&mut sent).await?;
                    let (status, body) = match (request.uri().path(), authorization.as_deref()) {
                        (_, Some(key)) if key != "Bearer fk-good" => {
                            (401, r#"{"error": "unauthorized"}"#.to_string())
                        }
                        ("/api/billing/limits", _) => {
                            let chosen = preference.lock().expect("lock").clone();
                            let limits = match chosen {
                                Some(chosen) => FACTORY_LIMITS.replace(
                                    r#""overagePreference": "droidCore""#,
                                    &format!(r#""overagePreference": "{chosen}""#),
                                ),
                                None => FACTORY_LIMITS.to_string(),
                            };
                            (200, limits)
                        }
                        ("/api/organization/subscription/set-overage-preference", _)
                            if request.method() == Method::POST =>
                        {
                            let sent: serde_json::Value = serde_json::from_str(&sent)?;
                            let chosen = sent["overagePreference"].as_str().map(str::to_string);
                            *preference.lock().expect("lock") = chosen;
                            (200, "{}".to_string())
                        }
                        _ => (404, String::new()),
                    };
                    Ok(Response::builder()
                        .status(status)
                        .body(AsyncBody::from(body))?)
                }
            }
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
            .read(with_key("fk-good"), http.clone(), unused_folder())
            .await
            .expect("read");
        assert_eq!(read.logged_in, Some(true));
        assert_eq!(read.status.windows.len(), 3);

        let refused = reader
            .read(with_key("fk-revoked"), http.clone(), unused_folder())
            .await
            .expect("read");
        assert_eq!(refused.logged_in, Some(false));

        // Switching saves the preference as Droid does, and reads it back.
        *preference.lock().expect("lock") = Some("extraUsage".into());
        let switched = reader
            .switch_to_droid_core(with_key("fk-good"), http.clone(), unused_folder())
            .await
            .expect("switch");
        assert_eq!(
            switched
                .status
                .overage
                .and_then(|overage| overage.preference),
            Some(OveragePreference::DroidCore)
        );
        assert!(
            reader
                .switch_to_droid_core(with_key("fk-revoked"), http.clone(), unused_folder())
                .await
                .is_err()
        );

        assert!(
            reader
                .read(AgentCommand::default(), http, unused_folder())
                .await
                .is_err()
        );
    }
}
