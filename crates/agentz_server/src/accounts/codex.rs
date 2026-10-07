//! Codex's accounts.

use std::collections::BTreeMap;
use std::path::Path;
use std::process::Stdio;
use std::time::{Duration, SystemTime};

use agentz_protocol::accounts::{AccountStatus, ExtraUsage, LimitResets, LimitWindow};
use agentz_protocol::agents::AgentCommand;
use anyhow::{Context as _, Result, anyhow, bail};
use serde::Deserialize;
use tokio::io::{AsyncBufRead, AsyncBufReadExt as _, AsyncWriteExt as _, BufReader, Lines};
use tokio::process::{Child, ChildStdin, ChildStdout};
use util::ResultExt as _;

use super::login_checks::account_command;
use super::readers::{LimitResetOutcome, LimitResetUse, Read, format_number, json_rpc_answer};
use super::{AgentDescription, LoginCheck, Reader, SHARED_SKILLS_FOLDER};

/// The adapter runs Codex itself with the arguments after this one.
const CLI: &str = "cli";
/// Codex's own JSON-RPC server, which the adapter runs too. It answers about the account
/// without starting a thread.
const APP_SERVER: &str = "app-server";
const INITIALIZE_REQUEST: u64 = 1;
const ACCOUNT_REQUEST: u64 = 2;
const RATE_LIMITS_REQUEST: u64 = 3;
const CONSUME_REQUEST: u64 = 4;
/// It asks OpenAI for the limits, in 2 to 3 seconds.
const READ_TIMEOUT: Duration = Duration::from_secs(30);
/// t3code gives the reset alone 20 seconds; the read after it comes in the same run.
const RESET_TIMEOUT: Duration = Duration::from_secs(50);
/// The limit every model counts against. Others, such as one model's own, come beside it.
const MAIN_LIMIT: &str = "codex";
const SESSION_MINUTES: u64 = 5 * 60;
const WEEK_MINUTES: u64 = 7 * 24 * 60;
const MONTH_MINUTES: u64 = 30 * 24 * 60;

/// Codex keeps its config, login (`auth.json`) and sessions in `CODEX_HOME`, or else
/// `~/.codex`.
pub(super) fn description() -> AgentDescription {
    AgentDescription {
        home_variables: BTreeMap::from([("CODEX_HOME".into(), String::new())]),
        shared_folders: BTreeMap::new(),
        external_links: Vec::new(),
        file_storage: BTreeMap::new(),
        home_files: BTreeMap::new(),
        // The model, profiles, providers and MCP servers.
        settings_files: vec!["config.toml".into()],
        login_settings: BTreeMap::new(),
        normal_home: ".codex".into(),
        // The adapter's API Key login reads them.
        login_variables: vec!["CODEX_API_KEY".into(), "OPENAI_API_KEY".into()],
        skills_folders: vec!["skills".into()],
        outside_skills_folders: vec![SHARED_SKILLS_FOLDER.into()],
        // The adapter asks Codex for the account before it opens a session.
        login_check: LoginCheck::Session,
        reader: Some(Reader::CodexAppServer),
        // Its API Key login takes the key in `authenticate`, and Codex keeps it in the home.
        key_login: None,
        usage_page: Some("https://chatgpt.com/codex/settings/usage".into()),
        extra_usage_page: None,
    }
}

/// Reads the account from Codex's app-server, run through the adapter in `folder`: who's
/// logged in from `account/read`, and the limits from `account/rateLimits/read`, as Codex's
/// `/status` and t3code's provider probe read them.
pub(super) async fn read(agent: AgentCommand, folder: &Path) -> Result<Read> {
    let mut app_server = AppServer::start(&agent, folder)?;
    let answers = async move {
        app_server
            .send(&[
                initialize(),
                initialized(),
                account_read_request(),
                rate_limits_request(),
            ])
            .await?;
        let answers = collect_answers(&mut app_server.output).await?;
        app_server.quit().await?;
        anyhow::Ok(answers)
    };
    let answers = tokio::time::timeout(READ_TIMEOUT, answers)
        .await
        .map_err(|_| anyhow!("Codex didn't answer in {}s", READ_TIMEOUT.as_secs()))??;
    account_read(answers)
}

/// Uses one of the account's limit resets, as Codex's `/usage` and t3code's Use reset do
/// (`account/rateLimitResetCredit/consume`), then reads the account in the same run.
/// `attempt` is the request's idempotency key.
pub(super) async fn use_limit_reset(
    agent: AgentCommand,
    folder: &Path,
    attempt: &str,
) -> Result<LimitResetUse> {
    let mut app_server = AppServer::start(&agent, folder)?;
    let consume = serde_json::json!({
        "id": CONSUME_REQUEST,
        "method": "account/rateLimitResetCredit/consume",
        "params": {"idempotencyKey": attempt},
    });
    let used = async move {
        app_server
            .send(&[initialize(), initialized(), consume])
            .await?;
        let answer = answer_to(&mut app_server.output, CONSUME_REQUEST)
            .await?
            .map_err(|error| anyhow!("Codex couldn't use the reset: {error}"))?;
        let ConsumeResponse { outcome } = serde_json::from_value(answer)
            .context("Codex's answer to the reset isn't in the expected shape")?;
        // Asked only now, so the read sees the reset.
        let read = async {
            app_server
                .send(&[account_read_request(), rate_limits_request()])
                .await?;
            account_read(collect_answers(&mut app_server.output).await?)
        }
        .await;
        app_server.quit().await.log_err();
        anyhow::Ok(LimitResetUse { outcome, read })
    };
    tokio::time::timeout(RESET_TIMEOUT, used)
        .await
        .map_err(|_| anyhow!("Codex didn't answer in {}s", RESET_TIMEOUT.as_secs()))?
}

/// Codex's app-server, run through the adapter.
struct AppServer {
    child: Child,
    input: ChildStdin,
    output: Lines<BufReader<ChildStdout>>,
}

impl AppServer {
    fn start(agent: &AgentCommand, folder: &Path) -> Result<Self> {
        std::fs::create_dir_all(folder)
            .with_context(|| format!("creating {}", folder.display()))?;
        let args: Vec<String> = agent
            .args
            .iter()
            .cloned()
            .chain([CLI.to_string(), APP_SERVER.to_string()])
            .collect();
        let mut command = account_command(&agent.path, &args, agent);
        command.current_dir(folder).stdin(Stdio::piped());
        let mut child = command.spawn().context("starting Codex")?;
        let input = child.stdin.take().context("Codex has no input")?;
        let output = child.stdout.take().context("Codex has no output")?;
        Ok(Self {
            child,
            input,
            output: BufReader::new(output).lines(),
        })
    }

    async fn send(&mut self, messages: &[serde_json::Value]) -> Result<()> {
        for message in messages {
            self.input
                .write_all(format!("{message}\n").as_bytes())
                .await
                .context("asking Codex about the account")?;
        }
        self.input
            .flush()
            .await
            .context("asking Codex about the account")
    }

    /// At the end of its input it quits.
    async fn quit(self) -> Result<()> {
        let Self {
            mut child, input, ..
        } = self;
        drop(input);
        child.wait().await.context("waiting for Codex to quit")?;
        Ok(())
    }
}

// Its messages leave out `"jsonrpc"`.
fn initialize() -> serde_json::Value {
    serde_json::json!({
        "id": INITIALIZE_REQUEST,
        "method": "initialize",
        "params": {
            "clientInfo": {
                "name": "agentz",
                "title": "agentZ",
                "version": env!("CARGO_PKG_VERSION"),
            },
            "capabilities": null,
        },
    })
}

fn initialized() -> serde_json::Value {
    serde_json::json!({"method": "initialized"})
}

/// Never a refresh: reading the account mustn't change its login.
fn account_read_request() -> serde_json::Value {
    serde_json::json!({
        "id": ACCOUNT_REQUEST,
        "method": "account/read",
        "params": {"refreshToken": false},
    })
}

fn rate_limits_request() -> serde_json::Value {
    serde_json::json!({"id": RATE_LIMITS_REQUEST, "method": "account/rateLimits/read"})
}

/// What Codex answered to `account/read` and `account/rateLimits/read`: each a result, or the
/// message of its error.
struct Answers {
    account: Result<serde_json::Value, String>,
    rate_limits: Result<serde_json::Value, String>,
}

/// Reads Codex's output up to its answers, among the notifications it sends.
async fn collect_answers(output: &mut Lines<impl AsyncBufRead + Unpin>) -> Result<Answers> {
    let mut account = None;
    let mut rate_limits = None;
    while let Some(line) = output
        .next_line()
        .await
        .context("reading Codex's answers")?
    {
        match json_rpc_answer(&line) {
            Some((INITIALIZE_REQUEST, Err(error))) => bail!("Codex didn't start: {error}"),
            Some((ACCOUNT_REQUEST, answer)) => account = Some(answer),
            Some((RATE_LIMITS_REQUEST, answer)) => rate_limits = Some(answer),
            _ => {}
        }
        if account.is_some() && rate_limits.is_some() {
            break;
        }
    }
    match (account, rate_limits) {
        (Some(account), Some(rate_limits)) => Ok(Answers {
            account,
            rate_limits,
        }),
        _ => bail!("Codex ended without answering"),
    }
}

/// Reads Codex's output up to its answer to `id`.
async fn answer_to(
    output: &mut Lines<impl AsyncBufRead + Unpin>,
    id: u64,
) -> Result<Result<serde_json::Value, String>> {
    while let Some(line) = output
        .next_line()
        .await
        .context("reading Codex's answers")?
    {
        match json_rpc_answer(&line) {
            Some((INITIALIZE_REQUEST, Err(error))) => bail!("Codex didn't start: {error}"),
            Some((answered, answer)) if answered == id => return Ok(answer),
            _ => {}
        }
    }
    bail!("Codex ended without answering")
}

/// `account/rateLimitResetCredit/consume`.
#[derive(Deserialize)]
struct ConsumeResponse {
    outcome: LimitResetOutcome,
}

/// `account/read`.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AccountResponse {
    account: Option<Account>,
    /// False where Codex runs on another provider, which needs no login.
    #[serde(default)]
    requires_openai_auth: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Account {
    /// "chatgpt", "apiKey" or "amazonBedrock".
    #[serde(rename = "type")]
    kind: String,
    email: Option<String>,
    plan_type: Option<String>,
}

/// `account/rateLimits/read`.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RateLimitsResponse {
    rate_limits: RateLimitSnapshot,
    #[serde(default)]
    rate_limits_by_limit_id: Option<BTreeMap<String, RateLimitSnapshot>>,
    #[serde(default)]
    rate_limit_reset_credits: Option<ResetCredits>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ResetCredits {
    available_count: i64,
    #[serde(default)]
    credits: Option<Vec<ResetCredit>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ResetCredit {
    status: String,
    /// Seconds since the epoch.
    expires_at: Option<i64>,
}

/// t3code's summary of them: how many, and when the first available one expires.
fn limit_resets(credits: Option<ResetCredits>) -> Option<LimitResets> {
    let credits = credits?;
    let available = u32::try_from(credits.available_count)
        .ok()
        .filter(|count| *count > 0)?;
    let next_expires_at = credits
        .credits
        .unwrap_or_default()
        .into_iter()
        .filter(|credit| credit.status == "available")
        .filter_map(|credit| u64::try_from(credit.expires_at?).ok())
        .min()
        .map(|seconds| SystemTime::UNIX_EPOCH + Duration::from_secs(seconds));
    Some(LimitResets {
        available,
        next_expires_at,
    })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RateLimitSnapshot {
    limit_id: Option<String>,
    plan_type: Option<String>,
    primary: Option<RateLimitWindow>,
    secondary: Option<RateLimitWindow>,
    #[serde(default)]
    credits: Option<Credits>,
}

/// Credits bought for ChatGPT, which Codex spends once the plan's limits run out.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Credits {
    #[serde(default)]
    has_credits: bool,
    #[serde(default)]
    unlimited: bool,
    /// A number of credits, as text.
    balance: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RateLimitWindow {
    used_percent: f64,
    window_duration_mins: Option<u64>,
    /// Seconds since the epoch.
    resets_at: Option<i64>,
}

fn account_read(answers: Answers) -> Result<Read> {
    let account = answers
        .account
        .map_err(|error| anyhow!("Codex couldn't read the account: {error}"))?;
    let response: AccountResponse =
        serde_json::from_value(account).context("Codex's account isn't in the expected shape")?;
    let Some(account) = response.account else {
        return Ok(Read {
            logged_in: Some(!response.requires_openai_auth),
            status: AccountStatus::default(),
        });
    };
    // An API key or another cloud's credentials have no limits to read.
    if account.kind != "chatgpt" {
        return Ok(Read {
            logged_in: Some(true),
            status: AccountStatus::default(),
        });
    }
    let rate_limits = answers
        .rate_limits
        .map_err(|error| anyhow!("Codex couldn't read the limits: {error}"))?;
    let mut rate_limits: RateLimitsResponse = serde_json::from_value(rate_limits)
        .context("Codex's limits aren't in the expected shape")?;
    let limit_resets = limit_resets(rate_limits.rate_limit_reset_credits.take());
    let snapshot = main_snapshot(rate_limits);
    Ok(Read {
        logged_in: Some(true),
        status: AccountStatus {
            email: account.email,
            plan: account.plan_type.as_deref().and_then(plan_label),
            extra_usage: snapshot.credits.as_ref().and_then(credits),
            windows: windows(snapshot),
            limit_resets,
            ..AccountStatus::default()
        },
    })
}

fn main_snapshot(response: RateLimitsResponse) -> RateLimitSnapshot {
    response
        .rate_limits_by_limit_id
        .and_then(|mut by_limit| by_limit.remove(MAIN_LIMIT))
        .unwrap_or(response.rate_limits)
}

/// What's left of the account's credits, while it has any.
fn credits(credits: &Credits) -> Option<ExtraUsage> {
    let summary = if credits.unlimited {
        "Unlimited".to_string()
    } else {
        let balance = credits
            .balance
            .as_deref()?
            .trim()
            .parse::<f64>()
            .ok()
            .filter(|balance| credits.has_credits && *balance > 0.)?;
        format!("{} left", format_number(balance, 2, false))
    };
    Some(ExtraUsage {
        label: "Credits".into(),
        summary,
    })
}

/// t3code's windows for Codex: the main limit's two, named by their length. `primary` and
/// `secondary` are places, not lengths; without a length, a paid plan's are the 5-hour and
/// weekly ones, and Free and Go have one monthly allowance.
fn windows(snapshot: RateLimitSnapshot) -> Vec<LimitWindow> {
    if snapshot
        .limit_id
        .as_deref()
        .is_some_and(|limit_id| limit_id != MAIN_LIMIT)
    {
        return Vec::new();
    }
    let monthly_plan = matches!(snapshot.plan_type.as_deref(), Some("free" | "go"));
    let primary_minutes = if monthly_plan {
        MONTH_MINUTES
    } else {
        SESSION_MINUTES
    };
    [
        (snapshot.primary, primary_minutes),
        (snapshot.secondary, WEEK_MINUTES),
    ]
    .into_iter()
    .filter_map(|(window, default_minutes)| {
        let window = window?;
        let minutes = window.window_duration_mins.unwrap_or(default_minutes);
        let label = if minutes >= MONTH_MINUTES {
            "Monthly"
        } else if minutes >= WEEK_MINUTES {
            "Weekly"
        } else {
            "Session"
        };
        Some(LimitWindow {
            label: label.into(),
            used_percent: window.used_percent,
            resets_at: window
                .resets_at
                .and_then(|seconds| u64::try_from(seconds).ok())
                .filter(|seconds| *seconds > 0)
                .map(|seconds| SystemTime::UNIX_EPOCH + Duration::from_secs(seconds)),
            length: Some(Duration::from_secs(minutes * 60)),
        })
    })
    .collect()
}

/// t3code's names for ChatGPT's plans, without "ChatGPT … Subscription" around them.
fn plan_label(plan: &str) -> Option<String> {
    let label = match plan {
        "free" => "Free",
        "go" => "Go",
        "plus" => "Plus",
        "pro" => "Pro 20x",
        "prolite" => "Pro 5x",
        "promax" => "Pro Max",
        "team" => "Team",
        "self_serve_business_prolite" | "self_serve_business_usage_based" | "business" => {
            "Business"
        }
        "ent26" | "enterprise_cbp_automation" | "enterprise_cbp_usage_based" | "enterprise" => {
            "Enterprise"
        }
        "edu" | "edu_plus" | "edu_pro" => "Edu",
        _ => return None,
    };
    Some(label.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// What Codex 0.160.0 answered for a Free account.
    const ACCOUNT: &str = include_str!("codex_reads/account.json");
    const LOGGED_OUT: &str = include_str!("codex_reads/account-logged-out.json");
    const RATE_LIMITS: &str = include_str!("codex_reads/rate-limits.json");
    /// Codex's answer to `account/rateLimits/read` without a ChatGPT login.
    const NO_CHATGPT: &str = "chatgpt authentication required to read rate limits";

    fn fixture(json: &str) -> serde_json::Value {
        serde_json::from_str(json).expect("fixture")
    }

    fn read_answers(
        account: serde_json::Value,
        rate_limits: Result<serde_json::Value, &str>,
    ) -> Result<Read> {
        account_read(Answers {
            account: Ok(account),
            rate_limits: rate_limits.map_err(str::to_string),
        })
    }

    fn read_limits(change: impl FnOnce(&mut serde_json::Value)) -> Vec<LimitWindow> {
        let mut rate_limits = fixture(RATE_LIMITS);
        change(&mut rate_limits);
        read_answers(fixture(ACCOUNT), Ok(rate_limits))
            .expect("read")
            .status
            .windows
    }

    #[test]
    fn account_and_rate_limits_give_the_account() {
        let read = read_answers(fixture(ACCOUNT), Ok(fixture(RATE_LIMITS))).expect("read");
        assert_eq!(read.logged_in, Some(true));
        assert_eq!(read.status.email.as_deref(), Some("work@example.com"));
        assert_eq!(read.status.plan.as_deref(), Some("Free"));
        assert_eq!(
            read.status.windows,
            [LimitWindow {
                label: "Monthly".into(),
                used_percent: 3.,
                resets_at: Some(SystemTime::UNIX_EPOCH + Duration::from_secs(1793476030)),
                length: Some(Duration::from_secs(30 * 24 * 60 * 60)),
            }]
        );

        // A paid plan's 5-hour and weekly windows, the main limit's over another's.
        let plus = serde_json::json!({
            "limitId": "codex",
            "planType": "plus",
            "primary": {"usedPercent": 12, "windowDurationMins": 300, "resetsAt": 1784000000},
            "secondary": {"usedPercent": 47, "windowDurationMins": 10080, "resetsAt": null},
        });
        let windows = read_limits(|rate_limits| {
            rate_limits["rateLimits"]["limitId"] = "codex_bengalfox".into();
            rate_limits["rateLimitsByLimitId"]["codex"] = plus.clone();
        });
        let found: Vec<(&str, f64, Option<Duration>, bool)> = windows
            .iter()
            .map(|window| {
                (
                    window.label.as_str(),
                    window.used_percent,
                    window.length,
                    window.resets_at.is_some(),
                )
            })
            .collect();
        assert_eq!(
            found,
            [
                ("Session", 12., Some(Duration::from_secs(5 * 60 * 60)), true),
                (
                    "Weekly",
                    47.,
                    Some(Duration::from_secs(7 * 24 * 60 * 60)),
                    false
                ),
            ]
        );

        // Without lengths, a paid plan's are the 5-hour and weekly ones.
        let windows = read_limits(|rate_limits| {
            let mut snapshot = plus.clone();
            snapshot["primary"]["windowDurationMins"] = serde_json::Value::Null;
            snapshot["secondary"]["windowDurationMins"] = serde_json::Value::Null;
            rate_limits["rateLimitsByLimitId"] = serde_json::Value::Null;
            rate_limits["rateLimits"] = snapshot;
        });
        let labels: Vec<&str> = windows.iter().map(|window| window.label.as_str()).collect();
        assert_eq!(labels, ["Session", "Weekly"]);

        // Only another limit's windows: none of the main one.
        let windows = read_limits(|rate_limits| {
            rate_limits["rateLimitsByLimitId"] = serde_json::Value::Null;
            rate_limits["rateLimits"]["limitId"] = "codex_bengalfox".into();
        });
        assert_eq!(windows, []);
    }

    #[test]
    fn credits_are_shown_while_there_are_some() {
        let summary = |credits: serde_json::Value| {
            let mut rate_limits = fixture(RATE_LIMITS);
            rate_limits["rateLimitsByLimitId"]["codex"]["credits"] = credits;
            read_answers(fixture(ACCOUNT), Ok(rate_limits))
                .expect("read")
                .status
                .extra_usage
                .map(|credits| {
                    assert_eq!(credits.label, "Credits");
                    credits.summary
                })
        };
        // The captured Free account has none.
        let read = read_answers(fixture(ACCOUNT), Ok(fixture(RATE_LIMITS))).expect("read");
        assert_eq!(read.status.extra_usage, None);
        assert_eq!(
            summary(serde_json::json!({"hasCredits": true, "unlimited": false, "balance": "1240"}))
                .as_deref(),
            Some("1,240 left")
        );
        assert_eq!(
            summary(serde_json::json!({"hasCredits": true, "unlimited": false, "balance": "5.25"}))
                .as_deref(),
            Some("5.25 left")
        );
        assert_eq!(
            summary(serde_json::json!({"hasCredits": true, "unlimited": true, "balance": null}))
                .as_deref(),
            Some("Unlimited")
        );
        for none in [
            serde_json::json!({"hasCredits": false, "unlimited": false, "balance": "0"}),
            serde_json::json!({"hasCredits": true, "unlimited": false, "balance": "0"}),
            serde_json::Value::Null,
        ] {
            assert_eq!(summary(none), None);
        }
    }

    #[test]
    fn limit_resets_are_counted_as_t3code_does() {
        let read = read_answers(fixture(ACCOUNT), Ok(fixture(RATE_LIMITS))).expect("read");
        assert_eq!(
            read.status.limit_resets,
            Some(LimitResets {
                available: 1,
                next_expires_at: Some(SystemTime::UNIX_EPOCH + Duration::from_secs(1793293254)),
            })
        );

        // The first to expire of those still available.
        let mut rate_limits = fixture(RATE_LIMITS);
        rate_limits["rateLimitResetCredits"] = serde_json::json!({
            "availableCount": 2,
            "credits": [
                {"status": "available", "expiresAt": 1793300000},
                {"status": "redeemed", "expiresAt": 1793000000},
                {"status": "available", "expiresAt": 1793200000},
            ],
        });
        let read = read_answers(fixture(ACCOUNT), Ok(rate_limits.clone())).expect("read");
        assert_eq!(
            read.status.limit_resets,
            Some(LimitResets {
                available: 2,
                next_expires_at: Some(SystemTime::UNIX_EPOCH + Duration::from_secs(1793200000)),
            })
        );

        for none in [
            serde_json::json!({"availableCount": 0, "credits": []}),
            serde_json::Value::Null,
        ] {
            rate_limits["rateLimitResetCredits"] = none;
            let read = read_answers(fixture(ACCOUNT), Ok(rate_limits.clone())).expect("read");
            assert_eq!(read.status.limit_resets, None);
        }
    }

    #[test]
    fn logins_without_chatgpt_have_no_limits() {
        let read = read_answers(
            fixture(LOGGED_OUT),
            Err("codex account authentication required to read rate limits"),
        )
        .expect("read");
        assert_eq!(read.logged_in, Some(false));
        assert_eq!(read.status, AccountStatus::default());

        let api_key =
            serde_json::json!({"account": {"type": "apiKey"}, "requiresOpenaiAuth": true});
        let read = read_answers(api_key, Err(NO_CHATGPT)).expect("read");
        assert_eq!(read.logged_in, Some(true));
        assert_eq!(read.status, AccountStatus::default());

        let other_provider = serde_json::json!({"account": null, "requiresOpenaiAuth": false});
        let read = read_answers(other_provider, Err(NO_CHATGPT)).expect("read");
        assert_eq!(read.logged_in, Some(true));

        // A ChatGPT login whose limits can't be read fails the read.
        assert!(read_answers(fixture(ACCOUNT), Err("offline")).is_err());
    }

    #[test]
    fn plans_are_named_as_t3code_does() {
        assert_eq!(plan_label("plus").as_deref(), Some("Plus"));
        assert_eq!(plan_label("prolite").as_deref(), Some("Pro 5x"));
        assert_eq!(
            plan_label("self_serve_business_usage_based").as_deref(),
            Some("Business")
        );
        assert_eq!(plan_label("unknown"), None);
    }

    /// The adapter as the reader sees it: with `cli app-server`, Codex's app-server answers
    /// the captured account and limits, after a notification and the other way around, and
    /// uses a limit reset with the outcome in CODEX_RESET_OUTCOME. It writes down how it was
    /// run and what it was sent.
    const FAKE_ADAPTER: &str = r#"
import json, os, sys

reads = os.environ["CODEX_READS"]
assert sys.argv[1:] == ["cli", "app-server"], sys.argv
logged_out = bool(os.environ.get("CODEX_LOGGED_OUT"))

def fixture(name):
    with open(os.path.join(reads, name)) as file:
        return json.load(file)

def send(message):
    sys.stdout.write(json.dumps(message) + "\n")
    sys.stdout.flush()

received = []
for line in sys.stdin:
    message = json.loads(line)
    received.append(message)
    method = message["method"]
    if method == "initialize":
        send({"id": message["id"], "result": {"userAgent": "agentz/0.160.0", "codexHome": os.environ.get("CODEX_HOME")}})
        send({"method": "remoteControl/status/changed", "params": {"status": "disabled"}})
    elif method == "account/rateLimitResetCredit/consume":
        send({"id": message["id"], "result": {"outcome": os.environ.get("CODEX_RESET_OUTCOME", "reset")}})
    elif method == "account/read":
        account_read = message
    elif method == "account/rateLimits/read":
        if logged_out:
            send({"id": message["id"], "error": {"code": -32600, "message": "codex account authentication required to read rate limits"}})
        else:
            send({"id": message["id"], "result": fixture("rate-limits.json")})
        name = "account-logged-out.json" if logged_out else "account.json"
        send({"id": account_read["id"], "result": fixture(name)})
with open("run.json", "w") as file:
    json.dump({"received": received, "codex_home": os.environ.get("CODEX_HOME")}, file)
"#;

    fn fake_adapter(dir: &Path) -> Option<AgentCommand> {
        let path = std::env::var_os("PATH")?;
        let python = std::env::split_paths(&path)
            .map(|dir| dir.join("python3"))
            .find(|candidate| candidate.is_file())?;
        let script = dir.join("fake_adapter.py");
        std::fs::write(&script, FAKE_ADAPTER).expect("write the fake");
        Some(AgentCommand {
            path: python,
            args: vec![script.to_string_lossy().into_owned()],
            env: [
                (
                    "CODEX_READS".to_string(),
                    concat!(env!("CARGO_MANIFEST_DIR"), "/src/accounts/codex_reads").into(),
                ),
                ("CODEX_HOME".to_string(), "/accounts/codex-acp/2".into()),
            ]
            .into_iter()
            .collect(),
            ..AgentCommand::default()
        })
    }

    #[tokio::test]
    async fn reads_codex_through_the_adapter() {
        let dir = tempfile::tempdir().expect("temp dir");
        let folder = dir.path().join("reader");
        let Some(agent) = fake_adapter(dir.path()) else {
            return;
        };
        let found = read(agent.clone(), &folder).await.expect("read");
        assert_eq!(found.logged_in, Some(true));
        assert_eq!(found.status.email.as_deref(), Some("work@example.com"));
        assert_eq!(found.status.windows.len(), 1);
        // It quit at the end of its input, having been told no more.
        let run: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(folder.join("run.json")).expect("run"))
                .expect("json");
        let methods: Vec<&str> = run["received"]
            .as_array()
            .expect("received")
            .iter()
            .filter_map(|message| message["method"].as_str())
            .collect();
        assert_eq!(
            methods,
            [
                "initialize",
                "initialized",
                "account/read",
                "account/rateLimits/read"
            ]
        );
        assert_eq!(
            run["received"][2]["params"],
            serde_json::json!({"refreshToken": false})
        );
        assert_eq!(run["codex_home"], "/accounts/codex-acp/2");

        let mut logged_out = agent;
        logged_out.env.insert("CODEX_LOGGED_OUT".into(), "1".into());
        let found = read(logged_out, &folder).await.expect("read");
        assert_eq!(found.logged_in, Some(false));
        assert_eq!(found.status, AccountStatus::default());
    }

    #[tokio::test]
    async fn uses_a_limit_reset_through_the_adapter() {
        let dir = tempfile::tempdir().expect("temp dir");
        let folder = dir.path().join("reader");
        let Some(agent) = fake_adapter(dir.path()) else {
            return;
        };
        let used = use_limit_reset(agent.clone(), &folder, "attempt-1")
            .await
            .expect("used");
        assert_eq!(used.outcome, LimitResetOutcome::Reset);
        let read = used.read.expect("the read after it");
        assert_eq!(read.status.email.as_deref(), Some("work@example.com"));
        // The account is read once Codex has answered the reset.
        let run: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(folder.join("run.json")).expect("run"))
                .expect("json");
        let methods: Vec<&str> = run["received"]
            .as_array()
            .expect("received")
            .iter()
            .filter_map(|message| message["method"].as_str())
            .collect();
        assert_eq!(
            methods,
            [
                "initialize",
                "initialized",
                "account/rateLimitResetCredit/consume",
                "account/read",
                "account/rateLimits/read"
            ]
        );
        assert_eq!(
            run["received"][2]["params"],
            serde_json::json!({"idempotencyKey": "attempt-1"})
        );

        let mut nothing_to_reset = agent;
        nothing_to_reset
            .env
            .insert("CODEX_RESET_OUTCOME".into(), "nothingToReset".into());
        let used = use_limit_reset(nothing_to_reset, &folder, "attempt-2")
            .await
            .expect("used");
        assert_eq!(used.outcome, LimitResetOutcome::NothingToReset);
    }

    #[test]
    fn accounts_get_their_own_codex_home() {
        let mut command = AgentCommand {
            env: [
                ("OPENAI_API_KEY".to_string(), "sk-outside".to_string()),
                ("CODEX_API_KEY".to_string(), "sk-outside".to_string()),
            ]
            .into_iter()
            .collect(),
            ..AgentCommand::default()
        };
        let home = Path::new("/data/accounts/codex-acp/2");
        description().apply(&mut command, BTreeMap::new(), home, None);
        assert_eq!(
            command.env.get("CODEX_HOME").map(String::as_str),
            Some("/data/accounts/codex-acp/2")
        );
        assert!(!command.env.contains_key("OPENAI_API_KEY"));
        assert!(!command.env.contains_key("CODEX_API_KEY"));
        assert!(command.env_remove.contains(&"OPENAI_API_KEY".to_string()));
    }
}
