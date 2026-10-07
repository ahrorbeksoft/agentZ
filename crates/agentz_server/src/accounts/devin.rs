//! Devin's accounts.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use agentz_protocol::accounts::{AccountStatus, LimitWindow};
use agentz_protocol::agents::AgentCommand;
use anyhow::{Context as _, Result};
use chrono::{Datelike as _, TimeZone as _};
use util::ResultExt as _;

use super::hidden_terminal::HiddenTerminal;
use super::login_checks::run_with_account_env;
use super::readers::Read;
use super::{AgentDescription, LoggedIn, LoginCheck, Reader, SHARED_SKILLS_FOLDER, StatusCommand};
use crate::terminals::TerminalSize;

/// The ACP server is Devin's program with `acp`; its terminal UI and other commands are the
/// program alone.
const ACP: &str = "acp";
const AUTH_STATUS: [&str; 2] = ["auth", "status"];
/// How `auth status` starts while logged in, whichever way ("Logged in (via Devin).").
const LOGGED_IN: &str = "Logged in";
/// The reader's own config folder (`XDG_CONFIG_HOME`), so its runs start none of the account's
/// MCP servers and run none of its hooks. The login is in the data folder.
const READER_CONFIG_FOLDER: &str = "devin-config";
/// Without `setup_complete` Devin asks its first-run questions, and writes the answers here.
const READER_CONFIG: &str =
    "{\"version\": 1, \"auto_update\": false, \"shell\": {\"setup_complete\": true}}\n";
const READER_ARGS: [&str; 2] = ["--respect-workspace-trust", "false"];
const SCREEN: TerminalSize = TerminalSize {
    columns: 120,
    screen_lines: 50,
    cell_width: 8,
    cell_height: 16,
};
const START_TIMEOUT: Duration = Duration::from_secs(60);
const STEP_TIMEOUT: Duration = Duration::from_secs(30);
const END_TIMEOUT: Duration = Duration::from_secs(5);
const RETRY_DELAY: Duration = Duration::from_secs(1);
const USAGE: &str = "/usage";
const PROMPT: char = '❭';
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
        // It overrides the stored login.
        login_variables: vec!["WINDSURF_API_KEY".into()],
        skills_folders: vec![".config/devin/skills".into()],
        outside_skills_folders: vec![SHARED_SKILLS_FOLDER.into(), ".claude/skills".into()],
        // Its sessions open while it's logged out, with no models.
        login_check: LoginCheck::Command(StatusCommand {
            program: None,
            args: AUTH_STATUS.map(String::from).to_vec(),
            after_agent_args: false,
            logged_in: LoggedIn::Prefix(LOGGED_IN.into()),
        }),
        reader: Some(Reader::DevinTerminal),
        // Its key login takes the key in `authenticate`.
        key_login: None,
        usage_page: Some("https://app.devin.ai/settings/usage".into()),
        extra_usage_page: None,
    }
}

/// Reads the account: who's logged in from `auth status`, then the limits from `/usage` in
/// Devin's terminal UI, run in `folder` with a config of the reader's own. The only keys sent
/// are the command, Enter once the menu offers it, and Ctrl+C twice to quit.
pub(super) async fn read(agent: AgentCommand, folder: &Path) -> Result<Read> {
    let program_args: Vec<String> = agent
        .args
        .iter()
        .take_while(|arg| *arg != ACP)
        .cloned()
        .collect();
    let args: Vec<String> = program_args
        .iter()
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
        return Ok(Read {
            logged_in: Some(false),
            status: AccountStatus::default(),
        });
    };
    let windows = read_usage(agent, program_args, folder).await?;
    Ok(Read {
        logged_in: Some(true),
        status: AccountStatus {
            windows,
            ..identity
        },
    })
}

async fn read_usage(
    mut agent: AgentCommand,
    program_args: Vec<String>,
    folder: &Path,
) -> Result<Vec<LimitWindow>> {
    let config = folder.join(READER_CONFIG_FOLDER);
    let config_file = config.join("devin").join("config.json");
    // Written only when it's missing or different: another account's read may be starting
    // with it.
    if std::fs::read_to_string(&config_file).ok().as_deref() != Some(READER_CONFIG) {
        super::descriptions::write_file(&config_file, READER_CONFIG)?;
    }
    let locks = locks_folder(&agent);
    let locks_before = lock_names(&locks)
        .context("listing Devin's session locks")
        .log_err();
    agent.env.insert(
        "XDG_CONFIG_HOME".into(),
        config.to_string_lossy().into_owned(),
    );
    let args = program_args
        .into_iter()
        .chain(READER_ARGS.map(String::from))
        .collect();
    let mut terminal = HiddenTerminal::start(&agent, args, folder.to_path_buf(), SCREEN)?;
    let windows = read_screens(&mut terminal).await;
    // A second Ctrl+C quits. Devin then ends its ACP server, which keeps the session's lock.
    terminal.write("\x03");
    tokio::time::sleep(Duration::from_millis(200)).await;
    terminal.write("\x03");
    terminal.wait_for_exit(END_TIMEOUT).await;
    terminal.end(END_TIMEOUT).await;
    if let Some(locks_before) = locks_before {
        remove_new_locks(&locks, &locks_before)
            .await
            .context("removing the session lock Devin left")
            .log_err();
    }
    windows
}

async fn read_screens(terminal: &mut HiddenTerminal) -> Result<Vec<LimitWindow>> {
    terminal
        .wait_for("its prompt", START_TIMEOUT, |screen| {
            prompt(screen).map(|_| ())
        })
        .await?;
    let deadline = tokio::time::Instant::now() + STEP_TIMEOUT;
    loop {
        terminal.write(USAGE);
        terminal
            .wait_for(&format!("{USAGE} in its menu"), STEP_TIMEOUT, |screen| {
                offers_first(screen, USAGE).then_some(())
            })
            .await?;
        terminal.write("\r");
        let answer = terminal
            .wait_for(USAGE, STEP_TIMEOUT, |screen| {
                usage_answer(screen, SystemTime::now())
            })
            .await?;
        match answer {
            Answer::Windows(windows) => return Ok(windows),
            Answer::Failed(message) => anyhow::bail!("{USAGE} said \"{message}\""),
            // Billed some other way, if it still doesn't know.
            Answer::TooEarly if tokio::time::Instant::now() >= deadline => return Ok(Vec::new()),
            Answer::TooEarly => tokio::time::sleep(RETRY_DELAY).await,
        }
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

fn is_rule(line: &str) -> bool {
    !line.is_empty() && line.chars().all(|char| char == '─')
}

fn prompt_text(line: &str) -> Option<&str> {
    Some(line.strip_prefix(PROMPT)?.trim())
}

/// The line of Devin's prompt, between two rules, and what it shows: what's typed, or its
/// placeholder. The lowest one, since what was typed before stays above it.
fn prompt_line(lines: &[&str]) -> Option<usize> {
    (1..lines.len().saturating_sub(1)).rev().find(|index| {
        prompt_text(lines[*index]).is_some()
            && is_rule(lines[index - 1])
            && is_rule(lines[index + 1])
    })
}

fn prompt(screen: &str) -> Option<&str> {
    let lines: Vec<&str> = screen.lines().collect();
    prompt_text(lines[prompt_line(&lines)?])
}

/// Whether `command` is typed at the prompt and is the first command its menu offers, under
/// the status line below the prompt: Enter runs the one it offers first.
fn offers_first(screen: &str, command: &str) -> bool {
    let lines: Vec<&str> = screen.lines().collect();
    let Some(index) = prompt_line(&lines) else {
        return false;
    };
    prompt_text(lines[index]) == Some(command)
        && lines
            .get(index + 3)
            .and_then(|first| first.split_whitespace().find(|word| word.starts_with('/')))
            == Some(command)
}

#[derive(Debug, PartialEq)]
enum Answer {
    /// The account's limits: none for a login without a quota.
    Windows(Vec<LimitWindow>),
    /// It answered before learning how the account is billed, with no quota.
    TooEarly,
    /// Fetching the quota failed, as it says.
    Failed(String),
}

/// `/usage`'s answer, once it's drawn whole: the lines between the command, echoed above the
/// prompt, and the prompt drawn again below them (not still showing the command, whose last
/// answer may be above it). For a login billed by quota it fetches it ("Fetching quota…"),
/// then draws a line for each window, with what's used ("0% used") and when it resets.
fn usage_answer(screen: &str, now: SystemTime) -> Option<Answer> {
    let lines: Vec<&str> = screen.lines().collect();
    let prompt = prompt_line(&lines)?;
    if prompt_text(lines[prompt]) == Some(USAGE) {
        return None;
    }
    let echo = (0..prompt - 1)
        .rev()
        .find(|index| prompt_text(lines[*index]) == Some(USAGE))?;
    let answer: Vec<&str> = lines[echo + 1..prompt - 1]
        .iter()
        .map(|line| line.trim())
        .filter(|line| !line.is_empty())
        .collect();
    let windows: Vec<LimitWindow> = answer
        .iter()
        .filter_map(|line| window_line(line, now))
        .collect();
    if !windows.is_empty() || answer.contains(&"No quota data available.") {
        return Some(Answer::Windows(windows));
    }
    if let Some(failed) = answer.iter().find(|line| {
        line.starts_with("Failed to fetch quota") || line.starts_with("Timed out fetching quota")
    }) {
        return Some(Answer::Failed(failed.to_string()));
    }
    if answer.iter().any(|line| line.starts_with("Fetching quota")) {
        return None;
    }
    // "No credits or ACUs consumed yet in this session.", where a login billed by credits or
    // ACUs names the one it is.
    if answer.iter().any(|line| line.contains("credits or ACUs")) {
        return Some(Answer::TooEarly);
    }
    (!answer.is_empty()).then(|| Answer::Windows(Vec::new()))
}

/// " Daily   ■■■■  12% used  · resets in 16h 24m".
fn window_line(line: &str, now: SystemTime) -> Option<LimitWindow> {
    let (before, after) = line.split_once("% used")?;
    let label = before
        .split(|char: char| !(char.is_alphanumeric() || char == ' ' || char == '-'))
        .next()?
        .trim();
    let before = before.trim_end();
    let number = before
        .rsplit(|char: char| !(char.is_ascii_digit() || char == '.'))
        .next()?;
    if label.is_empty() || number.is_empty() {
        return None;
    }
    let resets_at = after
        .split_once("resets ")
        .and_then(|(_, when)| reset_time(when.trim(), now));
    Some(LimitWindow {
        label: label.to_string(),
        used_percent: number.parse().ok()?,
        resets_at,
        length: match label {
            "Daily" => Some(DAY),
            "Weekly" => Some(7 * DAY),
            _ => None,
        },
    })
}

/// When a window resets, as Devin writes it: in a while ("in 16h 24m", "in 2d 3h") or at a
/// time with its offset ("Oct 11, 1:00 PM (UTC+5)"), whose year is that of its next
/// occurrence.
fn reset_time(text: &str, now: SystemTime) -> Option<SystemTime> {
    if let Some(after) = text.strip_prefix("in ") {
        let mut seconds = 0;
        for part in after.split_whitespace() {
            let unit = match part.chars().last()? {
                'd' => 24 * 60 * 60,
                'h' => 60 * 60,
                'm' => 60,
                's' => 1,
                _ => return None,
            };
            let count: u64 = part[..part.len() - 1].parse().ok()?;
            seconds += count * unit;
        }
        return Some(now + Duration::from_secs(seconds));
    }
    let (date, offset) = text.split_once(" (UTC")?;
    let offset = offset.strip_suffix(')')?;
    let offset_seconds = if offset.is_empty() {
        0
    } else {
        let sign = match offset.chars().next()? {
            '+' => 1,
            '-' => -1,
            _ => return None,
        };
        let (hours, minutes) = offset[1..].split_once(':').unwrap_or((&offset[1..], "0"));
        sign * (hours.parse::<i32>().ok()? * 60 * 60 + minutes.parse::<i32>().ok()? * 60)
    };
    let offset = chrono::FixedOffset::east_opt(offset_seconds)?;
    let now_there = chrono::DateTime::<chrono::Utc>::from(now).with_timezone(&offset);
    // A moment ago still counts, for a read that lands as it resets.
    let earliest = now - Duration::from_secs(60 * 60);
    [now_there.year(), now_there.year() + 1]
        .into_iter()
        .filter_map(|year| {
            let time = chrono::NaiveDateTime::parse_from_str(
                &format!("{year} {date}"),
                "%Y %b %d, %I:%M %p",
            )
            .ok()?;
            Some(SystemTime::from(
                offset.from_local_datetime(&time).single()?,
            ))
        })
        .find(|time| *time >= earliest)
}

/// Where Devin keeps its sessions' locks in the account's data folder.
fn locks_folder(agent: &AgentCommand) -> PathBuf {
    agent
        .env
        .get("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("XDG_DATA_HOME").map(PathBuf::from))
        .unwrap_or_else(|| util::paths::home_dir().join(".local/share"))
        .join("devin/cli/session_locks")
}

fn lock_names(locks: &Path) -> Result<Vec<std::ffi::OsString>> {
    match std::fs::read_dir(locks) {
        Ok(entries) => entries
            .map(|entry| Ok(entry?.file_name()))
            .collect::<std::io::Result<_>>()
            .with_context(|| format!("reading {}", locks.display())),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(error) => Err(error).with_context(|| format!("reading {}", locks.display())),
    }
}

/// Removes the locks of sessions that ended, opened since `before`. Devin keeps a lock for each
/// session its ACP server opens, named after the session and holding the server's process
/// id, and leaves it there when it ends. The read's session keeps nothing else.
async fn remove_new_locks(locks: &Path, before: &[std::ffi::OsString]) -> Result<()> {
    let after = lock_names(locks)?;
    let deadline = tokio::time::Instant::now() + END_TIMEOUT;
    for name in after.iter().filter(|name| !before.contains(name)) {
        let lock = locks.join(name);
        let Some(pid) = std::fs::read_to_string(&lock)
            .ok()
            .and_then(|text| text.trim().parse::<u32>().ok())
        else {
            continue;
        };
        #[cfg(unix)]
        {
            while crate::terminals::process_exists(pid) && tokio::time::Instant::now() < deadline {
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
            if crate::terminals::process_exists(pid) {
                continue;
            }
        }
        super::descriptions::remove_file(&lock)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const AUTH_STATUS_LOGGED_IN: &str = include_str!("devin_reads/auth-status.txt");
    const AUTH_STATUS_LOGGED_OUT: &str = include_str!("devin_reads/auth-status-logged-out.txt");
    const READY: &str = include_str!("devin_screens/ready.txt");
    const MENU_USAGE: &str = include_str!("devin_screens/menu-usage.txt");
    const FETCHING: &str = include_str!("devin_screens/fetching.txt");
    const USAGE_SCREEN: &str = include_str!("devin_screens/usage.txt");
    const QUIT: &str = include_str!("devin_screens/quit.txt");
    const TOO_EARLY: &str = include_str!("devin_screens/too-early.txt");

    fn at(text: &str) -> SystemTime {
        SystemTime::from(chrono::DateTime::parse_from_rfc3339(text).expect("time"))
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
    fn the_prompt_and_its_menu() {
        assert_eq!(
            prompt(READY),
            Some("Ask Devin to build features, fix bugs, or work on your code")
        );
        assert_eq!(prompt(MENU_USAGE), Some(USAGE));
        assert!(offers_first(MENU_USAGE, USAGE));
        assert!(!offers_first(READY, USAGE));
        // Partly typed, with another command first.
        let partly = MENU_USAGE
            .replace("❭ /usage", "❭ /u")
            .replace("○ /usage  ", "○ /undo   ");
        assert!(!offers_first(&partly, USAGE));
        let other_first = MENU_USAGE.replace("○ /usage  ", "○ /undo   ");
        assert!(!offers_first(&other_first, USAGE));
        assert_eq!(prompt("Starting…"), None);
    }

    fn answered_windows(screen: &str, now: SystemTime) -> Option<Vec<LimitWindow>> {
        match usage_answer(screen, now)? {
            Answer::Windows(windows) => Some(windows),
            answer => panic!("{answer:?}"),
        }
    }

    #[test]
    fn usage_gives_the_daily_and_weekly_windows() {
        let now = at("2026-10-07T15:36:00Z");
        assert_eq!(usage_answer(READY, now), None);
        assert_eq!(usage_answer(MENU_USAGE, now), None);
        assert_eq!(usage_answer(FETCHING, now), None);
        let windows = answered_windows(USAGE_SCREEN, now).expect("usage");
        assert_eq!(
            windows,
            [
                LimitWindow {
                    label: "Daily".into(),
                    used_percent: 0.0,
                    resets_at: Some(now + Duration::from_secs((16 * 60 + 21) * 60)),
                    length: Some(DAY),
                },
                LimitWindow {
                    label: "Weekly".into(),
                    used_percent: 0.0,
                    resets_at: Some(at("2026-10-11T08:00:00Z")),
                    length: Some(7 * DAY),
                },
            ]
        );
        assert_eq!(
            answered_windows(QUIT, now).map(|windows| windows.len()),
            Some(2)
        );

        let used = USAGE_SCREEN
            .replace(
                "  0% used  · resets in 16h 21m",
                "  12.5% used  · resets in 45m",
            )
            .replace(
                "Weekly  ■■■■■■■■■■■■■■■■■■■■  0%",
                "Weekly  ■■■■■■■■■■■■■■■■■■■■100%",
            );
        let used = answered_windows(&used, now).expect("usage");
        assert_eq!(used[0].used_percent, 12.5);
        assert_eq!(used[0].resets_at, Some(now + Duration::from_secs(45 * 60)));
        assert_eq!(used[1].used_percent, 100.0);

        // A login with no quota says so in place of the windows.
        let none = FETCHING.replace("Fetching quota…", "No quota data available.");
        assert_eq!(answered_windows(&none, now), Some(Vec::new()));
        let failed = FETCHING.replace(
            "Fetching quota…",
            "Fetching quota…\n\n Timed out fetching quota data.",
        );
        assert_eq!(
            usage_answer(&failed, now),
            Some(Answer::Failed("Timed out fetching quota data.".into()))
        );
    }

    #[test]
    fn usage_before_devin_knows_the_billing() {
        let now = at("2026-10-07T15:36:00Z");
        assert_eq!(usage_answer(TOO_EARLY, now), Some(Answer::TooEarly));
        // Typed again, with that answer still above the prompt.
        let again = TOO_EARLY.replace(
            "❭ Ask Devin to build features, fix bugs, or work on your code",
            "❭ /usage",
        );
        assert_eq!(usage_answer(&again, now), None);
        // A login billed by credits has no quota.
        let credits =
            TOO_EARLY.replace("No credits or ACUs consumed yet", "No credits consumed yet");
        assert_eq!(answered_windows(&credits, now), Some(Vec::new()));
    }

    #[test]
    fn resets_are_read_in_devins_words() {
        let now = at("2026-12-30T10:00:00Z");
        let after = |seconds: u64| Some(now + Duration::from_secs(seconds));
        assert_eq!(reset_time("in 16h 24m", now), after((16 * 60 + 24) * 60));
        assert_eq!(reset_time("in 2d 3h", now), after((2 * 24 + 3) * 60 * 60));
        assert_eq!(reset_time("in 5m", now), after(5 * 60));
        assert_eq!(
            reset_time("Dec 31, 9:05 AM (UTC+5:30)", now),
            Some(at("2026-12-31T03:35:00Z"))
        );
        // Next year's.
        assert_eq!(
            reset_time("Jan 2, 12:00 PM (UTC-3)", now),
            Some(at("2027-01-02T15:00:00Z"))
        );
        assert_eq!(
            reset_time("Dec 30, 10:30 AM (UTC)", now),
            Some(at("2026-12-30T10:30:00Z"))
        );
        assert_eq!(reset_time("soon", now), None);
        assert_eq!(reset_time("in a while", now), None);
    }

    /// Devin as the reader sees it: `auth status`, and its terminal UI showing the captured
    /// screens in turn, each after the keys that lead to it, answering `/usage` too early the
    /// first time. Other keys are written to `unexpected`. Its session's lock holds its own
    /// process id.
    const FAKE_DEVIN: &str = r#"
import json, os, sys, time, tty

reads = os.environ["DEVIN_READS"]
screens = os.environ["DEVIN_SCREENS"]
if sys.argv[1:] == ["auth", "status"]:
    name = "auth-status-logged-out" if os.environ.get("DEVIN_LOGGED_OUT") else "auth-status"
    with open(os.path.join(reads, name + ".txt")) as file:
        sys.stdout.write(file.read())
    sys.exit(0)
with open("run.json", "w") as file:
    json.dump({"args": sys.argv[1:], "config": os.environ.get("XDG_CONFIG_HOME")}, file)
if os.environ.get("DEVIN_BROKEN"):
    print("It broke")
    sys.exit(1)
locks = os.path.join(os.environ["XDG_DATA_HOME"], "devin", "cli", "session_locks")
os.makedirs(locks, exist_ok=True)
with open(os.path.join(locks, "quiet-otter.lock"), "w") as file:
    file.write(str(os.getpid()) + "\n")

def show(name):
    with open(os.path.join(screens, name + ".txt")) as file:
        text = file.read()
    sys.stdout.write("\x1b[2J\x1b[H" + text.replace("\n", "\r\n"))
    sys.stdout.flush()

def expect(keys):
    got = b""
    while len(got) < len(keys):
        got += os.read(0, len(keys) - len(got))
    if got != keys.encode():
        with open("unexpected", "w") as file:
            file.write(repr(got))
        sys.exit(1)

tty.setraw(0)
show("ready")
expect("/usage")
show("ready")
time.sleep(0.3)
show("menu-usage")
expect("\r")
show("too-early")
expect("/usage")
show("menu-usage")
expect("\r")
show("fetching")
time.sleep(0.3)
show("usage")
expect("\x03")
show("quit")
expect("\x03")
"#;

    fn fake_devin(dir: &Path, data: &Path) -> Option<AgentCommand> {
        let path = std::env::var_os("PATH")?;
        let python = std::env::split_paths(&path)
            .map(|dir| dir.join("python3"))
            .find(|candidate| candidate.is_file())?;
        let script = dir.join("fake_devin.py");
        std::fs::write(&script, FAKE_DEVIN).expect("write the fake");
        let accounts = concat!(env!("CARGO_MANIFEST_DIR"), "/src/accounts");
        Some(AgentCommand {
            path: python,
            args: vec![script.to_string_lossy().into_owned(), ACP.into()],
            env: [
                ("DEVIN_READS", format!("{accounts}/devin_reads")),
                ("DEVIN_SCREENS", format!("{accounts}/devin_screens")),
                ("XDG_DATA_HOME", data.to_string_lossy().into_owned()),
                ("XDG_CONFIG_HOME", "/the/accounts/config".into()),
            ]
            .into_iter()
            .map(|(variable, value)| (variable.to_string(), value))
            .collect(),
            ..AgentCommand::default()
        })
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn reads_devins_terminal_ui() {
        let dir = tempfile::tempdir().expect("temp dir");
        let data = dir.path().join("data");
        let folder = dir.path().join("reader");
        std::fs::create_dir_all(&folder).expect("folder");
        let Some(agent) = fake_devin(dir.path(), &data) else {
            return;
        };
        let locks = data.join("devin/cli/session_locks");
        std::fs::create_dir_all(&locks).expect("locks");
        // The user's own sessions' locks, one of a Devin still running.
        std::fs::write(locks.join("old-heron.lock"), "999999999\n").expect("write");
        std::fs::write(
            locks.join("busy-lark.lock"),
            format!("{}\n", std::process::id()),
        )
        .expect("write");

        let found = read(agent.clone(), &folder).await.expect("read");
        assert!(!folder.join("unexpected").exists());
        assert_eq!(found.logged_in, Some(true));
        assert_eq!(found.status.email.as_deref(), Some("work@example.com"));
        assert_eq!(found.status.name.as_deref(), Some("Example"));
        assert_eq!(found.status.plan.as_deref(), Some("Pro"));
        let labels: Vec<&str> = found
            .status
            .windows
            .iter()
            .map(|window| window.label.as_str())
            .collect();
        assert_eq!(labels, ["Daily", "Weekly"]);

        let run: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(folder.join("run.json")).expect("run"))
                .expect("json");
        let config = folder.join(READER_CONFIG_FOLDER);
        assert_eq!(
            run,
            serde_json::json!({"args": READER_ARGS, "config": config.to_string_lossy()})
        );
        assert_eq!(
            std::fs::read_to_string(config.join("devin/config.json")).expect("config"),
            READER_CONFIG
        );
        let mut left: Vec<String> = std::fs::read_dir(&locks)
            .expect("locks")
            .map(|entry| {
                entry
                    .expect("entry")
                    .file_name()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();
        left.sort();
        assert_eq!(left, ["busy-lark.lock", "old-heron.lock"]);

        // A failed read still ends Devin.
        let mut broken = agent.clone();
        broken.env.insert("DEVIN_BROKEN".into(), "1".into());
        read(broken, &folder).await.expect_err("it broke");

        // Logged out, the terminal UI doesn't start.
        std::fs::remove_file(folder.join("run.json")).expect("remove");
        let mut logged_out = agent;
        logged_out.env.insert("DEVIN_LOGGED_OUT".into(), "1".into());
        let found = read(logged_out, &folder).await.expect("read");
        assert_eq!(found.logged_in, Some(false));
        assert!(!folder.join("run.json").exists());
    }
}
