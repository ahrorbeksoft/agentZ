//! Claude Agent's accounts.

use std::collections::BTreeMap;
use std::path::Path;
use std::process::Stdio;
use std::time::{Duration, SystemTime};

use agentz_protocol::accounts::{AccountStatus, LimitWindow};
use agentz_protocol::agents::AgentCommand;
use anyhow::{Context as _, Result, anyhow, bail};
use serde::Deserialize;
use tokio::io::AsyncWriteExt as _;

use super::login_checks::{account_command, run_with_account_env};
use super::readers::Read;
use super::{AgentDescription, LoginCheck, Reader, StatusCommand};

/// The adapter runs Claude Code itself with the arguments around this one, as its terminal
/// logins do.
const CLI: &str = "--cli";
const AUTH_STATUS: [&str; 3] = ["auth", "status", "--json"];
/// Claude Code reading control requests from its input with no prompt, as the Agent SDK
/// starts it: nothing reaches the model. It keeps no session, starts no MCP servers, and runs
/// none of the user's hooks, which would take each read for a session starting (t3code's
/// capabilities probe).
const USAGE: [&str; 10] = [
    "-p",
    "--input-format",
    "stream-json",
    "--output-format",
    "stream-json",
    "--verbose",
    "--no-session-persistence",
    "--strict-mcp-config",
    "--settings",
    r#"{"disableAllHooks":true}"#,
];
/// What t3code's probe sets beside those: no claude.ai MCP servers, which aren't in any
/// settings file, and no looking for an IDE.
const USAGE_ENV: [(&str, &str); 3] = [
    ("ENABLE_CLAUDEAI_MCP_SERVERS", "false"),
    ("CLAUDE_CODE_AUTO_CONNECT_IDE", "0"),
    ("CLAUDE_CODE_IDE_SKIP_AUTO_INSTALL", "1"),
];
const USAGE_REQUEST: &str = "usage";
/// It asks Anthropic for the limits, in 1 to 4 seconds.
const USAGE_TIMEOUT: Duration = Duration::from_secs(30);
const SESSION_LENGTH: Duration = Duration::from_secs(5 * 60 * 60);
const WEEK_LENGTH: Duration = Duration::from_secs(7 * 24 * 60 * 60);

/// Claude Code keeps everything in `CLAUDE_CONFIG_DIR`, or else `~/.claude` (and
/// `~/.claude.json`). Its keychain entry is named after that folder, so each home has its own
/// login.
pub(super) fn description() -> AgentDescription {
    AgentDescription {
        home_variables: BTreeMap::from([("CLAUDE_CONFIG_DIR".into(), String::new())]),
        file_storage: BTreeMap::new(),
        home_files: BTreeMap::new(),
        // The model, permissions, hooks and environment. The login is in the keychain.
        settings_files: vec!["settings.json".into()],
        normal_home: ".claude".into(),
        // Each overrides the stored login, as the adapter's own list of them says.
        login_variables: vec![
            "ANTHROPIC_API_KEY".into(),
            "ANTHROPIC_AUTH_TOKEN".into(),
            "CLAUDE_CODE_OAUTH_TOKEN".into(),
        ],
        // Its sessions open while it's logged out. `loggedIn` is also true on an API key or
        // another cloud's credentials.
        login_check: LoginCheck::Command(StatusCommand {
            program: None,
            args: cli_args(&AUTH_STATUS),
            after_agent_args: true,
            logged_in: Some("/loggedIn".into()),
        }),
        reader: Some(Reader::ClaudeCode),
        // Its logins are Claude's own: in a terminal, or a gateway.
        key_login: None,
        usage_page: Some("https://claude.ai/settings/usage".into()),
    }
}

fn cli_args(args: &[&str]) -> Vec<String> {
    std::iter::once(CLI)
        .chain(args.iter().copied())
        .map(String::from)
        .collect()
}

/// Reads the account: who's logged in from `auth status`, then the limits from `get_usage`,
/// which is what Claude Code's `/usage` shows, run in `folder`.
pub(super) async fn read(agent: AgentCommand, folder: &Path) -> Result<Read> {
    let args: Vec<String> = agent
        .args
        .iter()
        .cloned()
        .chain(cli_args(&AUTH_STATUS))
        .collect();
    // Logged out, it exits with 1 and still prints its status.
    let output = run_with_account_env(None, &args, agent.clone()).await?;
    let status: AuthStatus = serde_json::from_slice(&output.stdout)
        .context("`claude auth status` didn't print its status")?;
    if !status.logged_in {
        return Ok(Read {
            logged_in: Some(false),
            status: AccountStatus::default(),
        });
    }
    let usage = read_usage(&agent, folder).await?;
    Ok(Read {
        logged_in: Some(true),
        status: account_status(status, usage),
    })
}

async fn read_usage(agent: &AgentCommand, folder: &Path) -> Result<Usage> {
    std::fs::create_dir_all(folder).with_context(|| format!("creating {}", folder.display()))?;
    let args: Vec<String> = agent.args.iter().cloned().chain(cli_args(&USAGE)).collect();
    let mut command = account_command(&agent.path, &args, agent);
    command
        .envs(USAGE_ENV)
        .current_dir(folder)
        .stdin(Stdio::piped());
    let mut child = command.spawn().context("starting Claude Code")?;
    let mut input = child.stdin.take().context("Claude Code has no input")?;
    let request = serde_json::json!({
        "type": "control_request",
        "request_id": USAGE_REQUEST,
        "request": {"subtype": "get_usage", "skip_behaviors": true},
    });
    input
        .write_all(format!("{request}\n").as_bytes())
        .await
        .context("asking Claude Code for its usage")?;
    // At the end of its input it answers what it was asked, then quits.
    drop(input);
    let output = tokio::time::timeout(USAGE_TIMEOUT, child.wait_with_output())
        .await
        .map_err(|_| anyhow!("Claude Code didn't answer in {}s", USAGE_TIMEOUT.as_secs()))?
        .context("reading Claude Code's usage")?;
    usage_response(&output.stdout)
}

/// `claude auth status --json`.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AuthStatus {
    logged_in: bool,
    email: Option<String>,
    /// "pro", "max", "team", … for a Claude login.
    subscription_type: Option<String>,
}

/// The `get_usage` answer, as the adapter's and t3code's `/usage` read it.
#[derive(Deserialize)]
struct Usage {
    subscription_type: Option<String>,
    #[serde(default)]
    rate_limits_available: bool,
    rate_limits: Option<RateLimits>,
}

#[derive(Deserialize)]
struct RateLimits {
    five_hour: Option<UsageWindow>,
    seven_day: Option<UsageWindow>,
    /// Weekly limits of single models, beside the one for all of them.
    #[serde(default)]
    model_scoped: Vec<ModelWindow>,
}

#[derive(Deserialize)]
struct UsageWindow {
    /// Percent used.
    utilization: Option<f64>,
    resets_at: Option<String>,
}

#[derive(Deserialize)]
struct ModelWindow {
    display_name: String,
    utilization: Option<f64>,
    resets_at: Option<String>,
}

/// The answer to agentZ's `get_usage` among the lines Claude Code printed.
fn usage_response(stdout: &[u8]) -> Result<Usage> {
    for line in stdout.split(|byte| *byte == b'\n') {
        let Ok(message) = serde_json::from_slice::<serde_json::Value>(line) else {
            continue;
        };
        let response = &message["response"];
        if message["type"] != "control_response" || response["request_id"] != USAGE_REQUEST {
            continue;
        }
        if response["subtype"] != "success" {
            bail!("Claude Code couldn't read the usage: {}", response["error"]);
        }
        return serde_json::from_value(response["response"].clone())
            .context("Claude Code's usage isn't in the expected shape");
    }
    bail!("Claude Code ended without its usage")
}

/// t3code's windows for Claude: the session and the week for all models, then each model's
/// week. A login with no limits (an API key) has none.
fn account_status(status: AuthStatus, usage: Usage) -> AccountStatus {
    let plan = status
        .subscription_type
        .or(usage.subscription_type)
        .map(|plan| plan_label(&plan));
    let windows = match usage.rate_limits.filter(|_| usage.rate_limits_available) {
        Some(limits) => {
            let windows = [
                ("Session".to_string(), SESSION_LENGTH, limits.five_hour),
                ("Weekly".to_string(), WEEK_LENGTH, limits.seven_day),
            ]
            .into_iter()
            .filter_map(|(label, length, window)| {
                let window = window?;
                Some((label, length, window.utilization?, window.resets_at))
            });
            let model_windows = limits.model_scoped.into_iter().filter_map(|window| {
                Some((
                    format!("Weekly · {}", window.display_name),
                    WEEK_LENGTH,
                    window.utilization?,
                    window.resets_at,
                ))
            });
            windows
                .chain(model_windows)
                .map(|(label, length, used_percent, resets_at)| LimitWindow {
                    label,
                    used_percent,
                    // None until the window starts.
                    resets_at: resets_at
                        .as_deref()
                        .and_then(|text| chrono::DateTime::parse_from_rfc3339(text).ok())
                        .map(SystemTime::from),
                    length: Some(length),
                })
                .collect()
        }
        None => Vec::new(),
    };
    AccountStatus {
        email: status.email,
        plan,
        windows,
        ..AccountStatus::default()
    }
}

/// "pro" as "Pro", "max" as "Max".
fn plan_label(plan: &str) -> String {
    plan.split(['_', '-', ' '])
        .filter(|word| !word.is_empty())
        .map(|word| {
            let mut letters = word.chars();
            letters
                .next()
                .map(|first| first.to_uppercase().chain(letters).collect())
                .unwrap_or_default()
        })
        .collect::<Vec<String>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    const AUTH_STATUS_FIXTURE: &str = include_str!("claude_reads/auth-status.json");
    const LOGGED_OUT: &str = include_str!("claude_reads/auth-status-logged-out.json");
    /// What Claude Code 2.1.287 printed for `get_usage` on a Pro account.
    const USAGE_FIXTURE: &str = include_str!("claude_reads/usage.jsonl");

    fn fixture_usage(change: impl FnOnce(&mut serde_json::Value)) -> String {
        let mut message: serde_json::Value =
            serde_json::from_str(USAGE_FIXTURE.trim()).expect("fixture");
        change(&mut message["response"]["response"]);
        message.to_string()
    }

    #[test]
    fn auth_status_and_usage_give_the_account() {
        let status = || serde_json::from_str::<AuthStatus>(AUTH_STATUS_FIXTURE).expect("status");
        assert!(status().logged_in);
        let logged_out: AuthStatus = serde_json::from_str(LOGGED_OUT).expect("status");
        assert!(!logged_out.logged_in);
        let usage = usage_response(USAGE_FIXTURE.as_bytes()).expect("usage");
        let account = account_status(status(), usage);
        assert_eq!(account.email.as_deref(), Some("work@example.com"));
        assert_eq!(account.plan.as_deref(), Some("Pro"));
        let week_resets = chrono::DateTime::parse_from_rfc3339("2026-10-10T22:00:00.194008Z")
            .map(SystemTime::from)
            .expect("time");
        assert_eq!(
            account.windows,
            [
                // Not started: nothing used, and no reset.
                LimitWindow {
                    label: "Session".into(),
                    used_percent: 0.,
                    resets_at: None,
                    length: Some(SESSION_LENGTH),
                },
                LimitWindow {
                    label: "Weekly".into(),
                    used_percent: 67.,
                    resets_at: Some(week_resets),
                    length: Some(WEEK_LENGTH),
                },
            ]
        );

        let with_model = fixture_usage(|usage| {
            usage["rate_limits"]["five_hour"]["utilization"] = 100.into();
            usage["rate_limits"]["five_hour"]["resets_at"] = "2026-10-07T17:00:00+00:00".into();
            usage["rate_limits"]["model_scoped"] = serde_json::json!([
                {"display_name": "Fable", "utilization": 12, "resets_at": "2026-10-10T22:00:00+00:00"},
                {"display_name": "Not read", "utilization": null, "resets_at": null},
            ]);
        });
        let usage = usage_response(with_model.as_bytes()).expect("usage");
        let account = account_status(status(), usage);
        let windows: Vec<(&str, f64, bool)> = account
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
                ("Session", 100., true),
                ("Weekly", 67., true),
                ("Weekly · Fable", 12., true),
            ]
        );

        // An API key has no limits.
        let key =
            r#"{"loggedIn": true, "authMethod": "api_key", "apiKeySource": "ANTHROPIC_API_KEY"}"#;
        let no_limits = fixture_usage(|usage| {
            usage["subscription_type"] = serde_json::Value::Null;
            usage["rate_limits_available"] = false.into();
            usage["rate_limits"] = serde_json::Value::Null;
        });
        let account = account_status(
            serde_json::from_str(key).expect("status"),
            usage_response(no_limits.as_bytes()).expect("usage"),
        );
        assert_eq!(account, AccountStatus::default());

        let failed = r#"{"type":"control_response","response":{"subtype":"error","request_id":"usage","error":"offline"}}"#;
        assert!(usage_response(failed.as_bytes()).is_err());
        assert!(usage_response(b"").is_err());
    }

    #[test]
    fn plans_are_named_as_claude_does() {
        assert_eq!(plan_label("pro"), "Pro");
        assert_eq!(plan_label("max"), "Max");
        assert_eq!(plan_label("team_premium"), "Team Premium");
        assert_eq!(plan_label(""), "");
    }

    /// The adapter as the reader sees it: with `--cli`, Claude Code's `auth status` prints the
    /// captured status, and `-p` answers the captured usage after a line of its own. It writes
    /// down how it was run.
    const FAKE_ADAPTER: &str = r#"
import json, os, sys

reads = os.environ["CLAUDE_READS"]
args = sys.argv[1:]
assert args[0] == "--cli", args
if args[1:] == ["auth", "status", "--json"]:
    name = "auth-status-logged-out.json" if os.environ.get("CLAUDE_LOGGED_OUT") else "auth-status.json"
    with open(os.path.join(reads, name)) as file:
        status = json.load(file)
    print(json.dumps(status))
    sys.exit(0 if status["loggedIn"] else 1)
with open("run.json", "w") as file:
    json.dump({"args": args[1:], "env": {k: os.environ.get(k) for k in ["ENABLE_CLAUDEAI_MCP_SERVERS", "CLAUDE_CONFIG_DIR"]}}, file)
request = json.loads(sys.stdin.readline())
assert request["request"] == {"subtype": "get_usage", "skip_behaviors": True}, request
# Ends at the end of its input.
assert sys.stdin.read() == ""
print(json.dumps({"type": "system", "subtype": "status"}))
assert request["request_id"] == "usage", request
with open(os.path.join(reads, "usage.jsonl")) as file:
    sys.stdout.write(file.read())
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
                    "CLAUDE_READS".to_string(),
                    concat!(env!("CARGO_MANIFEST_DIR"), "/src/accounts/claude_reads").into(),
                ),
                (
                    "CLAUDE_CONFIG_DIR".to_string(),
                    "/accounts/claude-acp/2".into(),
                ),
            ]
            .into_iter()
            .collect(),
            ..AgentCommand::default()
        })
    }

    #[tokio::test]
    async fn reads_claude_code_through_the_adapter() {
        let dir = tempfile::tempdir().expect("temp dir");
        let folder = dir.path().join("reader");
        let Some(agent) = fake_adapter(dir.path()) else {
            return;
        };
        let LoginCheck::Command(login_check) = description().login_check else {
            panic!("Claude's sessions open logged out, so its status command tells");
        };
        assert!(login_check.run(agent.clone()).await.expect("check"));

        let found = read(agent.clone(), &folder).await.expect("read");
        assert_eq!(found.logged_in, Some(true));
        assert_eq!(found.status.email.as_deref(), Some("work@example.com"));
        assert_eq!(found.status.windows.len(), 2);
        let run: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(folder.join("run.json")).expect("run"))
                .expect("json");
        assert_eq!(run["args"], serde_json::json!(USAGE));
        assert_eq!(run["env"]["ENABLE_CLAUDEAI_MCP_SERVERS"], "false");
        assert_eq!(run["env"]["CLAUDE_CONFIG_DIR"], "/accounts/claude-acp/2");

        // Logged out, it doesn't ask for the usage.
        std::fs::remove_file(folder.join("run.json")).expect("remove");
        let mut logged_out = agent;
        logged_out
            .env
            .insert("CLAUDE_LOGGED_OUT".into(), "1".into());
        assert!(!login_check.run(logged_out.clone()).await.expect("check"));
        let found = read(logged_out, &folder).await.expect("read");
        assert_eq!(found.logged_in, Some(false));
        assert_eq!(found.status, AccountStatus::default());
        assert!(!folder.join("run.json").exists());
    }

    #[test]
    fn accounts_get_their_own_config_folder() {
        let mut command = AgentCommand {
            env: [("ANTHROPIC_API_KEY".to_string(), "sk-outside".to_string())]
                .into_iter()
                .collect(),
            ..AgentCommand::default()
        };
        let home = Path::new("/data/accounts/claude-acp/2");
        description().apply(&mut command, BTreeMap::new(), home, None);
        assert_eq!(
            command.env.get("CLAUDE_CONFIG_DIR").map(String::as_str),
            Some("/data/accounts/claude-acp/2")
        );
        assert!(!command.env.contains_key("ANTHROPIC_API_KEY"));
        assert!(
            command
                .env_remove
                .contains(&"CLAUDE_CODE_OAUTH_TOKEN".to_string())
        );
    }
}
