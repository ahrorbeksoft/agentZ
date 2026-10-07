//! Grok Build's accounts.

use std::collections::{BTreeMap, HashMap};
use std::path::Path;
use std::process::Stdio;
use std::time::{Duration, SystemTime};

use agentz_protocol::accounts::{AccountStatus, ExtraUsage, LimitWindow};
use agentz_protocol::agents::AgentCommand;
use anyhow::{Context as _, Result, anyhow, bail};
use serde::Deserialize;
use tokio::io::{AsyncBufRead, AsyncBufReadExt as _, AsyncWriteExt as _, BufReader, Lines};
use tokio::process::ChildStdin;
use util::ResultExt as _;

use super::login_checks::account_command;
use super::readers::{Read, account_variable, format_number, json_rpc_answer};
use super::{AgentDescription, LoginCheck, Reader, SHARED_SKILLS_FOLDER};

const INITIALIZE_REQUEST: u64 = 1;
const SUBSCRIPTION_REQUEST: u64 = 2;
const BILLING_REQUEST: u64 = 3;
/// Who's logged in, by Grok's own extension.
const CHECK_SUBSCRIPTION: &str = "_x.ai/auth/check_subscription";
/// What Grok's `/usage` shows, which it asks xAI for.
const BILLING: &str = "_x.ai/billing";
/// It starts in up to 1.5 seconds, and answers each in about half a second.
const READ_TIMEOUT: Duration = Duration::from_secs(30);
/// It quits at the end of its input; one still running by then is stopped.
const QUIT_TIMEOUT: Duration = Duration::from_secs(5);
/// How billing fails without a login.
const AUTHENTICATION_REQUIRED: &str = "Authentication required";
/// Keys Grok logs in with ("xai.api_key"), which have no subscription or usage to read.
const KEY_VARIABLES: [&str; 2] = ["XAI_API_KEY", "GROK_CODE_XAI_API_KEY"];
const WEEKLY_PERIOD: &str = "USAGE_PERIOD_TYPE_WEEKLY";
const MONTHLY_PERIOD: &str = "USAGE_PERIOD_TYPE_MONTHLY";

/// Grok keeps its config, login (`auth.json`), sessions and skills in `GROK_HOME`, or else
/// `~/.grok`.
pub(super) fn description() -> AgentDescription {
    AgentDescription {
        home_variables: BTreeMap::from([("GROK_HOME".into(), String::new())]),
        shared_folders: BTreeMap::new(),
        // Its npm launcher runs `$GROK_HOME/bin/grok`, which npm's install of Grok updates in
        // the user's home only: each account's own would stay at the version it first ran,
        // a copy of 150 MB.
        external_links: vec!["bin".into()],
        // `auth.json` is a file in the home.
        file_storage: BTreeMap::new(),
        home_files: BTreeMap::new(),
        // The model, permissions, plugins and MCP servers.
        settings_files: vec!["config.toml".into()],
        login_settings: BTreeMap::new(),
        normal_home: ".grok".into(),
        login_variables: [
            "XAI_API_KEY",
            "GROK_CODE_XAI_API_KEY",
            "GROK_AUTH",
            "GROK_AUTH_PATH",
            "GROK_AUTH_PROVIDER_COMMAND",
            "GROK_AUTH_PROVIDER_ACCESS_TOKEN",
            "GROK_AUTH_PROVIDER_REFRESH_TOKEN",
            "GROK_AUTH_PROVIDER_EXPIRES_AT",
            "GROK_DEPLOYMENT_KEY",
        ]
        .map(String::from)
        .to_vec(),
        skills_folders: vec!["skills".into()],
        // Whatever its home, as Devin reads Claude's.
        outside_skills_folders: vec![
            SHARED_SKILLS_FOLDER.into(),
            ".claude/skills".into(),
            ".cursor/skills".into(),
        ],
        // Logged out, `session/new` fails with "Authentication required".
        login_check: LoginCheck::Session,
        reader: Some(Reader::GrokExtensions),
        // Its key login is offered only once `XAI_API_KEY` is set, so there's nothing to enter
        // a key into.
        key_login: None,
        // OpenUsage's.
        usage_page: Some("https://grok.com/?_s=usage".into()),
        extra_usage_page: None,
    }
}

/// Reads the account from the agent itself, run in `folder` with no session: who's logged
/// in from `_x.ai/auth/check_subscription`, and the limits from `_x.ai/billing`.
pub(super) async fn read(agent: AgentCommand, folder: &Path) -> Result<Read> {
    std::fs::create_dir_all(folder).with_context(|| format!("creating {}", folder.display()))?;
    let mut command = account_command(&agent.path, &agent.args, &agent);
    command.current_dir(folder).stdin(Stdio::piped());
    let mut child = command.spawn().context("starting Grok")?;
    let mut input = child.stdin.take().context("Grok has no input")?;
    let output = child.stdout.take().context("Grok has no output")?;
    let mut output = BufReader::new(output).lines();
    let answers = async {
        // Asked once it has started: it renews its own login then, if it's due.
        send(&mut input, &initialize()).await?;
        collect_answers(&mut output, &[INITIALIZE_REQUEST])
            .await?
            .remove(&INITIALIZE_REQUEST)
            .context("Grok didn't start")?
            .map_err(|error| anyhow!("Grok didn't start: {error}"))?;
        send(
            &mut input,
            &request(SUBSCRIPTION_REQUEST, CHECK_SUBSCRIPTION),
        )
        .await?;
        send(&mut input, &request(BILLING_REQUEST, BILLING)).await?;
        let mut answers =
            collect_answers(&mut output, &[SUBSCRIPTION_REQUEST, BILLING_REQUEST]).await?;
        match (
            answers.remove(&SUBSCRIPTION_REQUEST),
            answers.remove(&BILLING_REQUEST),
        ) {
            (Some(subscription), Some(billing)) => anyhow::Ok((subscription, billing)),
            _ => bail!("Grok ended without answering"),
        }
    };
    let answers = tokio::time::timeout(READ_TIMEOUT, answers).await;
    drop(input);
    if tokio::time::timeout(QUIT_TIMEOUT, child.wait())
        .await
        .is_err()
    {
        child.kill().await.context("stopping Grok").log_err();
    }
    let (subscription, billing) =
        answers.map_err(|_| anyhow!("Grok didn't answer in {}s", READ_TIMEOUT.as_secs()))??;
    account_read(subscription, billing, logs_in_with_key(&agent))
}

fn initialize() -> serde_json::Value {
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": INITIALIZE_REQUEST,
        "method": "initialize",
        "params": {
            "protocolVersion": 1,
            "clientCapabilities": {
                "fs": {"readTextFile": false, "writeTextFile": false},
                "terminal": false,
            },
            "clientInfo": {
                "name": "agentz",
                "title": "agentZ",
                "version": env!("CARGO_PKG_VERSION"),
            },
        },
    })
}

fn request(id: u64, method: &str) -> serde_json::Value {
    serde_json::json!({"jsonrpc": "2.0", "id": id, "method": method, "params": {}})
}

async fn send(input: &mut ChildStdin, message: &serde_json::Value) -> Result<()> {
    input
        .write_all(format!("{message}\n").as_bytes())
        .await
        .context("asking Grok about the account")?;
    input.flush().await.context("asking Grok about the account")
}

/// Reads Grok's output up to its answers to `ids`, among the notifications it sends, each a
/// result or the message of its error.
async fn collect_answers(
    output: &mut Lines<impl AsyncBufRead + Unpin>,
    ids: &[u64],
) -> Result<HashMap<u64, Result<serde_json::Value, String>>> {
    let mut answers = HashMap::new();
    while answers.len() < ids.len() {
        let Some(line) = output.next_line().await.context("reading Grok's answers")? else {
            break;
        };
        if let Some((id, answer)) = json_rpc_answer(&line)
            && ids.contains(&id)
        {
            answers.insert(id, answer);
        }
    }
    Ok(answers)
}

/// Whether the run has a key Grok logs in with, which overrides no stored login but gives it
/// one where there's none.
fn logs_in_with_key(agent: &AgentCommand) -> bool {
    KEY_VARIABLES
        .iter()
        .any(|variable| account_variable(agent, variable).is_some_and(|key| !key.trim().is_empty()))
}

/// `_x.ai/auth/check_subscription`.
#[derive(Deserialize)]
struct Subscription {
    authenticated: bool,
    meta: Option<SubscriptionMeta>,
}

#[derive(Default, Deserialize)]
#[serde(default)]
struct SubscriptionMeta {
    email: Option<String>,
    subscription_tier: Option<String>,
    /// A team's own login, whose usage limits its team manages.
    is_team_principal: bool,
}

/// `_x.ai/billing`, as proto3 JSON writes it: zeros are left out.
#[derive(Deserialize)]
struct Billing {
    config: BillingConfig,
    subscription_tier: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct BillingConfig {
    #[serde(default)]
    credit_usage_percent: f64,
    current_period: Option<Period>,
    on_demand_cap: Option<Amount>,
    on_demand_used: Option<Amount>,
}

#[derive(Deserialize)]
struct Period {
    #[serde(rename = "type")]
    kind: String,
    start: String,
    end: String,
}

/// An amount of cents, as xAI's billing writes money.
#[derive(Deserialize)]
struct Amount {
    #[serde(default)]
    val: Option<Number>,
}

/// A number, or one written as text, as proto3 JSON writes 64-bit ones.
#[derive(Deserialize)]
#[serde(untagged)]
enum Number {
    Number(f64),
    Text(String),
}

impl Amount {
    fn cents(amount: Option<&Amount>) -> Result<f64> {
        match amount.and_then(|amount| amount.val.as_ref()) {
            None => Ok(0.),
            Some(Number::Number(number)) => Ok(*number),
            Some(Number::Text(text)) => text
                .parse()
                .with_context(|| format!("{text:?} isn't an amount")),
        }
    }
}

fn logged_out() -> Read {
    Read {
        logged_in: Some(false),
        status: AccountStatus::default(),
    }
}

fn account_read(
    subscription: Result<serde_json::Value, String>,
    billing: Result<serde_json::Value, String>,
    with_key: bool,
) -> Result<Read> {
    let subscription =
        subscription.map_err(|error| anyhow!("Grok couldn't check the login: {error}"))?;
    let subscription: Subscription = serde_json::from_value(subscription)
        .context("Grok's login check isn't in the expected shape")?;
    if !subscription.authenticated {
        // A key logs Grok in with no subscription: the session check tells.
        return Ok(if with_key {
            Read::default()
        } else {
            logged_out()
        });
    }
    let meta = subscription.meta.unwrap_or_default();
    let mut identity = AccountStatus {
        email: non_empty(meta.email),
        plan: non_empty(meta.subscription_tier),
        ..AccountStatus::default()
    };
    let billing = match billing {
        Ok(billing) => billing,
        Err(error) if error.contains(AUTHENTICATION_REQUIRED) => return Ok(logged_out()),
        // "Usage limits are managed by your team."
        Err(_) if meta.is_team_principal => {
            return Ok(Read {
                logged_in: Some(true),
                status: identity,
            });
        }
        Err(error) => bail!("Grok couldn't read the usage: {error}"),
    };
    let billing: Billing =
        serde_json::from_value(billing).context("Grok's usage isn't in the expected shape")?;
    let config = billing.config;
    let window = config
        .current_period
        .as_ref()
        .map(|period| period_window(period, config.credit_usage_percent))
        .transpose()?
        .flatten();
    let extra_usage = pay_as_you_go(&config)?;
    Ok(Read {
        logged_in: Some(true),
        status: AccountStatus {
            plan: non_empty(billing.subscription_tier).or(identity.plan.take()),
            windows: window.into_iter().collect(),
            extra_usage: Some(extra_usage),
            ..identity
        },
    })
}

fn non_empty(text: Option<String>) -> Option<String> {
    text.map(|text| text.trim().to_string())
        .filter(|text| !text.is_empty())
}

/// The usage pool's window, as Grok's `/usage` names it ("Weekly limit").
fn period_window(period: &Period, used_percent: f64) -> Result<Option<LimitWindow>> {
    let label = match period.kind.as_str() {
        WEEKLY_PERIOD => "Weekly",
        MONTHLY_PERIOD => "Monthly",
        _ => return Ok(None),
    };
    let start = time(&period.start)?;
    let end = time(&period.end)?;
    Ok(Some(LimitWindow {
        label: label.into(),
        used_percent: used_percent.clamp(0., 100.),
        resets_at: Some(end),
        length: end.duration_since(start).ok(),
    }))
}

fn time(text: &str) -> Result<SystemTime> {
    chrono::DateTime::parse_from_rfc3339(text)
        .map(SystemTime::from)
        .with_context(|| format!("{text:?} isn't a time"))
}

/// Grok's "Pay as you go": what's left under the cap it's on with, or off without one.
fn pay_as_you_go(config: &BillingConfig) -> Result<ExtraUsage> {
    let cap = Amount::cents(config.on_demand_cap.as_ref())?;
    let used = Amount::cents(config.on_demand_used.as_ref())?;
    let summary = if cap > 0. {
        format!("{} of {} left", dollars(cap - used), dollars(cap))
    } else {
        "Off".to_string()
    };
    Ok(ExtraUsage {
        label: "Pay as you go".into(),
        summary,
    })
}

fn dollars(cents: f64) -> String {
    format!("${}", format_number(cents / 100., 2, true))
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    /// What Grok 1.0.49 answered for the user's Free login, the email replaced.
    const SUBSCRIPTION: &str = include_str!("grok_reads/check-subscription.json");
    const SUBSCRIPTION_LOGGED_OUT: &str =
        include_str!("grok_reads/check-subscription-logged-out.json");
    const BILLING_ANSWER: &str = include_str!("grok_reads/billing.json");
    /// `billing.json` with made-up usage, a cap and a plan.
    const BILLING_PAID: &str = include_str!("grok_reads/billing-edited-paid.json");
    const BILLING_LOGGED_OUT: &str = include_str!("grok_reads/billing-logged-out.json");

    fn json(text: &str) -> serde_json::Value {
        serde_json::from_str(text).expect("json")
    }

    fn error(text: &str) -> Result<serde_json::Value, String> {
        Err(json(text)["message"].as_str().expect("message").to_string())
    }

    fn at(text: &str) -> SystemTime {
        time(text).expect("time")
    }

    #[test]
    fn billing_gives_the_week_and_pay_as_you_go() {
        let read =
            account_read(Ok(json(SUBSCRIPTION)), Ok(json(BILLING_ANSWER)), false).expect("read");
        assert_eq!(read.logged_in, Some(true));
        assert_eq!(
            read.status,
            AccountStatus {
                email: Some("user@example.com".into()),
                plan: Some("Free".into()),
                windows: vec![LimitWindow {
                    label: "Weekly".into(),
                    used_percent: 0.,
                    resets_at: Some(at("2026-10-13T00:00:00+00:00")),
                    length: Some(Duration::from_secs(7 * 24 * 60 * 60)),
                }],
                extra_usage: Some(ExtraUsage {
                    label: "Pay as you go".into(),
                    summary: "Off".into(),
                }),
                ..AccountStatus::default()
            }
        );

        let read =
            account_read(Ok(json(SUBSCRIPTION)), Ok(json(BILLING_PAID)), false).expect("read");
        assert_eq!(read.status.plan.as_deref(), Some("SuperGrok"));
        assert_eq!(read.status.windows[0].used_percent, 42.5);
        assert_eq!(
            read.status
                .extra_usage
                .map(|extra| extra.summary)
                .as_deref(),
            Some("$22.00 of $25.00 left")
        );

        // A monthly period, its amounts written as text.
        let mut monthly = json(BILLING_PAID);
        monthly["config"]["currentPeriod"]["type"] = MONTHLY_PERIOD.into();
        monthly["config"]["onDemandUsed"]["val"] = "2500".into();
        let read = account_read(Ok(json(SUBSCRIPTION)), Ok(monthly), false).expect("read");
        assert_eq!(read.status.windows[0].label, "Monthly");
        assert_eq!(
            read.status
                .extra_usage
                .map(|extra| extra.summary)
                .as_deref(),
            Some("$0.00 of $25.00 left")
        );
    }

    #[test]
    fn a_login_check_that_fails_reads_logged_out() {
        let read = account_read(
            Ok(json(SUBSCRIPTION_LOGGED_OUT)),
            error(BILLING_LOGGED_OUT),
            false,
        )
        .expect("read");
        assert_eq!(read, logged_out());
        // Billing refused even so.
        let read =
            account_read(Ok(json(SUBSCRIPTION)), error(BILLING_LOGGED_OUT), false).expect("read");
        assert_eq!(read, logged_out());
        // With a key, it's logged in as far as the session check goes, with nothing to read.
        let read = account_read(
            Ok(json(SUBSCRIPTION_LOGGED_OUT)),
            error(BILLING_LOGGED_OUT),
            true,
        )
        .expect("read");
        assert_eq!(read, Read::default());
    }

    #[test]
    fn a_team_login_has_no_usage_of_its_own() {
        let mut team = json(SUBSCRIPTION);
        team["meta"]["is_team_principal"] = true.into();
        let read =
            account_read(Ok(team), Err("Billing service error".into()), false).expect("read");
        assert_eq!(read.logged_in, Some(true));
        assert_eq!(read.status.email.as_deref(), Some("user@example.com"));
        assert!(read.status.windows.is_empty());
        // Anyone else's failed read keeps the last numbers.
        assert!(
            account_read(
                Ok(json(SUBSCRIPTION)),
                Err("Billing service error".into()),
                false
            )
            .is_err()
        );
    }

    #[test]
    fn keys_in_the_run_log_in() {
        let mut agent = AgentCommand::default();
        agent.env.insert("XAI_API_KEY".into(), "xai-key".into());
        assert!(logs_in_with_key(&agent));
        let removed = AgentCommand {
            env_remove: KEY_VARIABLES.map(String::from).to_vec(),
            ..AgentCommand::default()
        };
        assert!(!logs_in_with_key(&removed));
    }

    /// Grok as the reader sees it: it answers `initialize`, then billing before the login
    /// check, after a notification, from the captured answers, and quits at the end of its
    /// input. It writes down how it was run and what it was sent.
    const FAKE_GROK: &str = r#"
import json, os, sys

reads = os.environ["GROK_READS"]
assert sys.argv[1:] == ["agent", "stdio"], sys.argv
logged_out = bool(os.environ.get("GROK_LOGGED_OUT"))

def fixture(name):
    with open(os.path.join(reads, name)) as file:
        return json.load(file)

def send(message):
    message["jsonrpc"] = "2.0"
    sys.stdout.write(json.dumps(message) + "\n")
    sys.stdout.flush()

received = []
for line in sys.stdin:
    message = json.loads(line)
    received.append(message)
    method = message["method"]
    if method == "initialize":
        name = "initialize-logged-out.json" if logged_out else "initialize-logged-in.json"
        send({"id": message["id"], "result": fixture(name)})
    elif method == "_x.ai/auth/check_subscription":
        check = message
    elif method == "_x.ai/billing":
        send({"method": "_x.ai/mcp/init_progress", "params": {}})
        if logged_out:
            send({"id": message["id"], "error": fixture("billing-logged-out.json")})
            send({"id": check["id"], "result": fixture("check-subscription-logged-out.json")})
        else:
            send({"id": message["id"], "result": fixture("billing.json")})
            send({"id": check["id"], "result": fixture("check-subscription.json")})
with open("run.json", "w") as file:
    json.dump({"received": received, "grok_home": os.environ.get("GROK_HOME")}, file)
"#;

    fn fake_grok(dir: &Path) -> Option<AgentCommand> {
        let path = std::env::var_os("PATH")?;
        let python = std::env::split_paths(&path)
            .map(|dir| dir.join("python3"))
            .find(|candidate| candidate.is_file())?;
        let script = dir.join("fake_grok.py");
        std::fs::write(&script, FAKE_GROK).expect("write the fake");
        Some(AgentCommand {
            path: python,
            args: vec![
                script.to_string_lossy().into_owned(),
                "agent".into(),
                "stdio".into(),
            ],
            env: [
                (
                    "GROK_READS".to_string(),
                    concat!(env!("CARGO_MANIFEST_DIR"), "/src/accounts/grok_reads").into(),
                ),
                ("GROK_HOME".to_string(), "/accounts/grok-build/2".into()),
            ]
            .into_iter()
            .collect(),
            env_remove: KEY_VARIABLES.map(String::from).to_vec(),
        })
    }

    #[tokio::test]
    async fn reads_grok_through_its_extensions() {
        let dir = tempfile::tempdir().expect("temp dir");
        let folder = dir.path().join("reader");
        let Some(agent) = fake_grok(dir.path()) else {
            return;
        };
        let found = read(agent.clone(), &folder).await.expect("read");
        assert_eq!(found.logged_in, Some(true));
        assert_eq!(found.status.email.as_deref(), Some("user@example.com"));
        assert_eq!(found.status.plan.as_deref(), Some("Free"));
        assert_eq!(found.status.windows.len(), 1);
        let run: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(folder.join("run.json")).expect("run"))
                .expect("json");
        let received = run["received"].as_array().expect("received");
        let methods: Vec<&str> = received
            .iter()
            .filter_map(|message| message["method"].as_str())
            .collect();
        assert_eq!(methods, ["initialize", CHECK_SUBSCRIPTION, BILLING]);
        assert!(received.iter().all(|message| message["jsonrpc"] == "2.0"));
        assert_eq!(received[1]["params"], serde_json::json!({}));
        assert_eq!(run["grok_home"], "/accounts/grok-build/2");

        let mut logged_out = agent;
        logged_out.env.insert("GROK_LOGGED_OUT".into(), "1".into());
        let found = read(logged_out, &folder).await.expect("read");
        assert_eq!(found, super::logged_out());
    }

    #[test]
    fn accounts_get_their_own_grok_home_and_its_binaries() {
        let mut command = AgentCommand {
            env: [("XAI_API_KEY".to_string(), "xai-outside".to_string())]
                .into_iter()
                .collect(),
            ..AgentCommand::default()
        };
        let dir = tempfile::tempdir().expect("temp dir");
        let home = dir.path().join("accounts/grok-build/2");
        description().apply(&mut command, BTreeMap::new(), &home, None);
        assert_eq!(
            command.env.get("GROK_HOME").map(PathBuf::from),
            Some(home.clone())
        );
        assert!(!command.env.contains_key("XAI_API_KEY"));
        assert!(command.env_remove.contains(&"XAI_API_KEY".to_string()));

        // The home runs the binaries npm keeps in the External account's.
        let external = dir.path().join("grok");
        std::fs::create_dir_all(external.join("bin")).expect("bin");
        let description = AgentDescription {
            home_variables: BTreeMap::new(),
            normal_home: external.to_string_lossy().into_owned(),
            ..description()
        };
        description.link_shared_folders(&home).expect("link");
        assert_eq!(
            std::fs::read_link(home.join("bin")).expect("linked"),
            external.join("bin")
        );
        // Linked once; a home's own folder stays.
        description.link_shared_folders(&home).expect("link again");
        let other = dir.path().join("accounts/grok-build/3");
        std::fs::create_dir_all(other.join("bin")).expect("its own");
        description.link_shared_folders(&other).expect("link");
        assert!(std::fs::read_link(other.join("bin")).is_err());
    }
}
