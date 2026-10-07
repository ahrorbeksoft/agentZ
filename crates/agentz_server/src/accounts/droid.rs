//! Factory Droid's accounts.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use agentz_protocol::accounts::{AccountStatus, LimitWindow};
use agentz_protocol::agents::AgentCommand;
use anyhow::{Context as _, Result};
use serde::Deserialize;
use util::ResultExt as _;

use super::hidden_terminal::HiddenTerminal;
use super::readers::Read;
use super::{AgentDescription, KeyLogin, LoginCheck, Reader};
use crate::terminals::TerminalSize;

/// Droid uploads its sessions to Factory's cloud unless its settings say not to. A new account
/// starts with that off; the user can turn it on in Droid's `/settings`.
const HOME_SETTINGS: &str = "{\n  \"cloudSessionSync\": false\n}\n";
/// Merged over the home's settings for the reader's own runs (`--settings`), so the sessions it
/// opens aren't uploaded on any account, the External one included.
const READER_SETTINGS: &str = "{\"cloudSessionSync\": false}\n";
const READER_SETTINGS_FILE: &str = "droid-settings.json";
/// Wide enough that the trust question's folder fits on one line, and tall enough that
/// `/status` fits on the screen: Droid draws below what it drew before, and what scrolls off
/// can't be drawn again.
const SCREEN: TerminalSize = TerminalSize {
    columns: 200,
    screen_lines: 100,
    cell_width: 8,
    cell_height: 16,
};
const START_TIMEOUT: Duration = Duration::from_secs(60);
const STEP_TIMEOUT: Duration = Duration::from_secs(30);
const END_TIMEOUT: Duration = Duration::from_secs(5);
/// Droid's windows in the order `/limits` always draws them, by their English names: the
/// labels it draws are translated.
pub(super) const WINDOW_LABELS: [&str; 3] = ["5-hour", "Weekly", "Monthly"];
/// How long each of [`WINDOW_LABELS`] runs. Neither `/limits` nor Factory's API says; the
/// monthly one follows the calendar, so 30 days is close enough to place its hairline.
pub(super) const WINDOW_LENGTHS: [Duration; 3] = [
    Duration::from_secs(5 * 60 * 60),
    Duration::from_secs(7 * 24 * 60 * 60),
    Duration::from_secs(30 * 24 * 60 * 60),
];

/// Droid keeps everything in `.factory` under `FACTORY_HOME_OVERRIDE`, or else the user's home.
/// Its login there is encrypted with a key from the keychain that every home shares, so the
/// login itself stays per home.
pub(super) fn description() -> AgentDescription {
    AgentDescription {
        home_variables: BTreeMap::from([("FACTORY_HOME_OVERRIDE".into(), String::new())]),
        shared_folders: BTreeMap::new(),
        file_storage: BTreeMap::new(),
        home_files: BTreeMap::from([(".factory/settings.json".into(), HOME_SETTINGS.into())]),
        // Custom models and session defaults. The login is in other files.
        settings_files: vec![".factory/settings.json".into()],
        login_settings: BTreeMap::new(),
        normal_home: String::new(),
        // A key in the environment overrides the stored login.
        login_variables: vec!["FACTORY_API_KEY".into()],
        // Logged out, `session/new` fails and offers a pairing code.
        login_check: LoginCheck::Session,
        reader: Some(Reader::DroidTerminal),
        // "Authenticate using a Factory API key set in the FACTORY_API_KEY environment
        // variable."
        key_login: Some(KeyLogin {
            method: "factory-api-key".into(),
            variable: "FACTORY_API_KEY".into(),
            reader: Some(Reader::FactoryApi {
                base_url: "https://api.factory.ai".into(),
            }),
        }),
        usage_page: Some("https://app.factory.ai/settings/billing".into()),
    }
}

/// Reads the account from `/status` and `/limits` in Droid's terminal UI, run in `folder`.
/// Droid is translated, so only what it draws the same in every language is read: its symbols
/// (`🔐`, `%`, `↻`, the bars), numbers, emails and ids. The only keys sent are the two commands,
/// Enter to run them (and to trust `folder`), and Esc: Enter in `/limits` would change what
/// Droid does at a limit.
///
/// Droid opens a session each time it starts, and keeps it in its own indexes of sessions even
/// once its files are deleted, so reads resume the one session a past read opened in the home.
pub(super) async fn read(agent: AgentCommand, folder: &Path) -> Result<Read> {
    std::fs::create_dir_all(folder).with_context(|| format!("creating {}", folder.display()))?;
    // Droid asks about the folder, and keeps its sessions, by its real path (`/private/tmp`
    // for `/tmp`).
    let folder =
        std::fs::canonicalize(folder).with_context(|| format!("resolving {}", folder.display()))?;
    let settings = folder.join(READER_SETTINGS_FILE);
    // Written only when it's missing or different: another account's read may be starting
    // with it.
    if std::fs::read_to_string(&settings).ok().as_deref() != Some(READER_SETTINGS) {
        std::fs::write(&settings, READER_SETTINGS)
            .with_context(|| format!("writing {}", settings.display()))?;
    }
    // The ACP command is the program and `exec …`; the terminal UI is the program alone.
    let mut args: Vec<String> = agent
        .args
        .iter()
        .take_while(|arg| *arg != "exec")
        .cloned()
        .collect();
    args.extend(["--settings".into(), settings.to_string_lossy().into_owned()]);
    let sessions = sessions_folder(&agent, &folder);
    let resumed = reads_sessions(&sessions, &folder)
        .context("finding the session Droid opened for past reads")
        .log_err()
        .and_then(|sessions| sessions.into_iter().next());
    if let Some(resumed) = &resumed {
        args.extend(["--resume".into(), resumed.clone()]);
    }
    let mut terminal = HiddenTerminal::start(&agent, args, folder.clone(), SCREEN)?;
    let mut session = None;
    let read = read_screens(&mut terminal, &folder, &mut session).await;
    terminal.end(END_TIMEOUT).await;
    match (&read, session, resumed) {
        (Ok(_), Some(session), _) => {
            keep_only_session(&sessions, &session, &folder)
                .context("removing the other sessions Droid opened to read accounts")
                .log_err();
        }
        // The session may be why it failed: the next read opens a new one.
        (Err(_), _, Some(resumed)) => {
            remove_session(&sessions, &resumed)
                .context("removing the session Droid opened to read accounts")
                .log_err();
        }
        _ => {}
    }
    read
}

async fn read_screens(
    terminal: &mut HiddenTerminal,
    folder: &Path,
    session: &mut Option<String>,
) -> Result<Read> {
    let mut trusted = false;
    loop {
        let start = terminal
            .wait_for("its prompt", START_TIMEOUT, |screen| {
                // The question stays on the screen a moment after it's answered.
                start_screen(screen, folder).filter(|start| !(trusted && *start == Start::Trust))
            })
            .await?;
        match start {
            Start::Trust => {
                terminal.write("\r");
                trusted = true;
            }
            Start::LoggedOut => {
                return Ok(Read {
                    logged_in: Some(false),
                    status: AccountStatus::default(),
                });
            }
            Start::Prompt => break,
        }
    }

    run(terminal, "/status").await?;
    let status = terminal
        .wait_for("/status", STEP_TIMEOUT, status_screen)
        .await?;
    *session = status.session;
    terminal.write("\x1b");
    terminal
        .wait_for("its prompt after /status", STEP_TIMEOUT, |screen| {
            prompt(screen)
                .filter(|typed| !typed.starts_with('/'))
                .map(|_| ())
        })
        .await?;

    run(terminal, "/limits").await?;
    let limits = terminal
        .wait_for("/limits", STEP_TIMEOUT, |screen| {
            limits_screen(screen, SystemTime::now())
        })
        .await?;
    terminal.write("\x1b");
    Ok(Read {
        // Droid shows its prompt only once it's logged in, and `/limits` only once it has
        // read them.
        logged_in: Some(true),
        status: AccountStatus {
            email: status.email,
            ..limits
        },
    })
}

/// Types `command`, and runs it once Droid's menu offers it first: the menu filters a moment
/// after the typing, and Enter runs whatever it offers first.
async fn run(terminal: &mut HiddenTerminal, command: &str) -> Result<()> {
    terminal.write(command);
    terminal
        .wait_for(&format!("{command} in its menu"), STEP_TIMEOUT, |screen| {
            offers_first(screen, command).then_some(())
        })
        .await?;
    terminal.write("\r");
    Ok(())
}

/// What Droid shows when it starts.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Start {
    /// It asks whether to trust `folder`, as it does once in each home.
    Trust,
    /// It offers its ways to log in instead of a prompt.
    LoggedOut,
    Prompt,
}

fn start_screen(screen: &str, folder: &Path) -> Option<Start> {
    if prompt(screen).is_some() {
        return Some(Start::Prompt);
    }
    let lines: Vec<&str> = screen.lines().collect();
    // A menu outside any box, its choice marked `>`.
    let (index, chosen) = lines
        .iter()
        .enumerate()
        .find_map(|(index, line)| Some((index, line.strip_prefix("> ")?)))?;
    // The trust question numbers its choices, the first being to trust.
    if chosen.starts_with("1. ") {
        let folder = folder.to_string_lossy();
        let asks_about_folder = lines.iter().any(|line| box_text(line) == Some(&*folder));
        return asks_about_folder.then_some(Start::Trust);
    }
    let offers_more = lines
        .get(index + 1)
        .is_some_and(|line| line.starts_with("  ") && !line.trim().is_empty());
    offers_more.then_some(Start::LoggedOut)
}

/// What's typed at Droid's prompt, if it shows one: a box's line that starts with `>`.
fn prompt(screen: &str) -> Option<&str> {
    let lines: Vec<&str> = screen.lines().collect();
    lines.iter().enumerate().find_map(|(index, line)| {
        let typed = prompt_text(line)?;
        let in_box = index
            .checked_sub(1)
            .and_then(|above| lines.get(above))
            .is_some_and(|above| above.starts_with('╭'));
        in_box.then_some(typed)
    })
}

fn prompt_text(line: &str) -> Option<&str> {
    Some(line.strip_prefix("│ >")?.trim_end_matches('│').trim())
}

/// Whether `command` is typed at the prompt and is the first command the menu below it offers.
fn offers_first(screen: &str, command: &str) -> bool {
    let lines: Vec<&str> = screen.lines().collect();
    lines.iter().enumerate().any(|(index, line)| {
        prompt_text(line) == Some(command)
            && lines
                .get(index + 1)
                .is_some_and(|below| below.starts_with('╰'))
            && lines
                .get(index + 2)
                .and_then(|first| first.split_whitespace().next())
                == Some(command)
    })
}

/// The text inside a box's line, between its borders.
fn box_text(line: &str) -> Option<&str> {
    Some(line.strip_prefix('│')?.strip_suffix('│')?.trim())
}

/// What `/status` says about the account and the session the read opened.
#[derive(Debug, PartialEq)]
struct Status {
    /// A login with a key has none.
    email: Option<String>,
    session: Option<String>,
}

/// `/status`, once its box is drawn whole: the email in its `🔐` section and the session's
/// id in its `📁` one.
fn status_screen(screen: &str) -> Option<Status> {
    let lines: Vec<&str> = screen.lines().collect();
    let authentication = lines.iter().position(|line| line.contains('🔐'))?;
    lines
        .iter()
        .skip(authentication)
        .any(|line| line.starts_with('╰'))
        .then_some(())?;
    let section = |header: char| -> Vec<&str> {
        let Some(start) = lines.iter().position(|line| line.contains(header)) else {
            return Vec::new();
        };
        lines
            .iter()
            .skip(start + 1)
            .map_while(|line| box_text(line).filter(|text| !text.is_empty()))
            .flat_map(str::split_whitespace)
            .collect()
    };
    let email = section('🔐')
        .into_iter()
        .find(|token| is_email(token))
        .map(str::to_string);
    let session = section('📁')
        .into_iter()
        .find(|token| token.len() == 36 && uuid::Uuid::parse_str(token).is_ok())
        .map(str::to_string);
    Some(Status { email, session })
}

fn is_email(token: &str) -> bool {
    token.split_once('@').is_some_and(|(name, domain)| {
        !name.is_empty()
            && !domain.contains('@')
            && domain.split('.').count() >= 2
            && domain.split('.').all(|part| !part.is_empty())
    })
}

/// `/limits`, once its box is drawn whole. It opens on Standard Usage's tab (`◉`), whose
/// windows are each a line with a percentage and when it resets (`↻`), above a bar of `█` and
/// `░`. A window not started yet shows no reset. The Extra Usage balance is the dollar amount
/// it offers to add to.
fn limits_screen(screen: &str, now: SystemTime) -> Option<AccountStatus> {
    let lines: Vec<&str> = screen.lines().collect();
    let tabs = lines
        .iter()
        .position(|line| line.contains('◉') && line.contains('○'))?;
    let bottom = tabs
        + lines
            .iter()
            .skip(tabs)
            .position(|line| line.starts_with('╰'))?;
    let inside: Vec<&str> = lines
        .iter()
        .take(bottom)
        .skip(tabs + 1)
        .filter_map(|line| box_text(line))
        .collect();
    let mut windows = Vec::new();
    for (line, below) in inside.iter().zip(inside.iter().skip(1)) {
        if !is_bar(below) {
            continue;
        }
        let Some((label, used_percent, rest)) = window_line(line) else {
            continue;
        };
        let resets_at = rest
            .split_once('↻')
            .and_then(|(_, time)| reset_after(time))
            .map(|after| now + after);
        windows.push(LimitWindow {
            label: WINDOW_LABELS
                .get(windows.len())
                .map_or_else(|| label.to_string(), |label| label.to_string()),
            used_percent,
            resets_at,
            length: WINDOW_LENGTHS.get(windows.len()).copied(),
        });
    }
    let credits = lines
        .iter()
        .skip(tabs)
        .find_map(|line| dollars(line))
        .filter(|dollars| *dollars > 0.0)
        .map(|dollars| format!("${dollars:.2}"));
    Some(AccountStatus {
        windows,
        credits,
        ..AccountStatus::default()
    })
}

fn is_bar(text: &str) -> bool {
    !text.is_empty() && text.chars().all(|char| char == '█' || char == '░')
}

/// A window's line: its label, the percentage used, and what follows. A long translated label
/// runs into the number.
fn window_line(text: &str) -> Option<(&str, f64, &str)> {
    let (before, after) = text.split_once('%')?;
    let label = before.trim_end_matches(|char: char| char.is_ascii_digit() || char == '.');
    let number = before[label.len()..].trim_start_matches('.');
    let label = &before[..before.len() - number.len()];
    Some((label.trim(), number.parse().ok()?, after))
}

/// How long until a window resets. Droid writes it in English in every language: "2 days",
/// "1h 5min", "39min".
fn reset_after(text: &str) -> Option<Duration> {
    let text = text.trim();
    if let Some(days) = text
        .strip_suffix("days")
        .or_else(|| text.strip_suffix("day"))
    {
        let days: u64 = days.trim().parse().ok()?;
        return Some(Duration::from_secs(days * 24 * 60 * 60));
    }
    let mut minutes = 0;
    for part in text.split_whitespace() {
        minutes += match (part.strip_suffix('h'), part.strip_suffix("min")) {
            (Some(hours), _) => hours.parse::<u64>().ok()? * 60,
            (_, Some(part)) => part.parse::<u64>().ok()?,
            _ => return None,
        };
    }
    (!text.is_empty()).then(|| Duration::from_secs(minutes * 60))
}

/// The first dollar amount in `text`.
fn dollars(text: &str) -> Option<f64> {
    let (_, after) = text.split_once('$')?;
    let number: String = after
        .chars()
        .take_while(|char| char.is_ascii_digit() || *char == '.')
        .collect();
    number.parse().ok()
}

/// Where Droid keeps the sessions it opens in `folder` (a real path) in the home `agent` runs
/// in: a folder named by the path, with `-` for each `/`.
fn sessions_folder(agent: &AgentCommand, folder: &Path) -> PathBuf {
    let home = agent
        .env
        .get("FACTORY_HOME_OVERRIDE")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("FACTORY_HOME_OVERRIDE").map(PathBuf::from))
        .unwrap_or_else(|| util::paths::home_dir().clone());
    let name = folder.to_string_lossy().trim_matches('/').replace('/', "-");
    home.join(".factory")
        .join("sessions")
        .join(format!("-{name}"))
}

/// The first line of a session's log.
#[derive(Deserialize)]
struct SessionStart {
    id: String,
    cwd: PathBuf,
}

fn session_start(log: &Path) -> Result<SessionStart> {
    let text =
        std::fs::read_to_string(log).with_context(|| format!("reading {}", log.display()))?;
    serde_json::from_str(text.lines().next().unwrap_or_default())
        .with_context(|| format!("reading {}", log.display()))
}

/// The ids of the sessions in `sessions` whose logs say they were opened in `folder`, which
/// are reads', in order.
fn reads_sessions(sessions: &Path, folder: &Path) -> Result<Vec<String>> {
    let logs = match std::fs::read_dir(sessions) {
        Ok(logs) => logs,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => {
            return Err(error).with_context(|| format!("reading {}", sessions.display()));
        }
    };
    let mut ids = Vec::new();
    for log in logs {
        let log = log
            .with_context(|| format!("reading {}", sessions.display()))?
            .path();
        let Some(id) = log
            .file_name()
            .and_then(|name| name.to_str()?.strip_suffix(".jsonl"))
        else {
            continue;
        };
        let Some(start) = session_start(&log).log_err() else {
            continue;
        };
        if start.id == id && start.cwd == folder {
            ids.push(id.to_string());
        }
    }
    ids.sort();
    Ok(ids)
}

/// Deletes the sessions reads opened other than `session`, the one this read used: those of
/// reads that failed, or that Droid didn't resume.
fn keep_only_session(sessions: &Path, session: &str, folder: &Path) -> Result<()> {
    let ids = reads_sessions(sessions, folder)?;
    anyhow::ensure!(
        ids.iter().any(|id| id == session),
        "{} doesn't keep the reads' sessions",
        sessions.display()
    );
    for id in ids.iter().filter(|id| *id != session) {
        remove_session(sessions, id)?;
    }
    Ok(())
}

fn remove_session(sessions: &Path, id: &str) -> Result<()> {
    for path in [
        sessions.join(format!("{id}.jsonl")),
        sessions.join(format!("{id}.settings.json")),
    ] {
        match std::fs::remove_file(&path) {
            Err(error) if error.kind() != std::io::ErrorKind::NotFound => {
                return Err(error).with_context(|| format!("removing {}", path.display()));
            }
            _ => {}
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const TRUST: &str = include_str!("droid_screens/trust.txt");
    const LOGGED_OUT: &str = include_str!("droid_screens/logged-out.txt");
    const READY: &str = include_str!("droid_screens/ready.txt");
    const MENU_UNFILTERED: &str = include_str!("droid_screens/menu-status-unfiltered.txt");
    const MENU_LIMITS: &str = include_str!("droid_screens/menu-limits.txt");
    const STATUS: &str = include_str!("droid_screens/status.txt");
    const LIMITS: &str = include_str!("droid_screens/limits.txt");
    /// The folder the screens were captured in.
    const FOLDER: &str = "/private/tmp/reader";
    const SESSION: &str = "00000000-0000-4000-8000-000000000001";

    #[test]
    fn droid_starts_by_trust_login_or_prompt() {
        let folder = Path::new(FOLDER);
        assert_eq!(start_screen(TRUST, folder), Some(Start::Trust));
        assert_eq!(start_screen(TRUST, Path::new("/elsewhere")), None);
        assert_eq!(start_screen(LOGGED_OUT, folder), Some(Start::LoggedOut));
        assert_eq!(start_screen(READY, folder), Some(Start::Prompt));
        assert_eq!(prompt(MENU_LIMITS), Some("/limits"));
        assert_eq!(start_screen(STATUS, folder), None);
    }

    #[test]
    fn commands_run_once_the_menu_offers_them() {
        assert!(!offers_first(MENU_UNFILTERED, "/status"));
        assert!(offers_first(MENU_LIMITS, "/limits"));
        assert!(!offers_first(MENU_LIMITS, "/status"));
        assert!(!offers_first(READY, "/limits"));
    }

    #[test]
    fn status_gives_the_email_and_session() {
        assert_eq!(
            status_screen(STATUS),
            Some(Status {
                email: Some("work@example.com".into()),
                session: Some(SESSION.into()),
            })
        );
        // Half drawn.
        let top: String = STATUS
            .lines()
            .take_while(|line| !line.contains("🧠"))
            .collect::<Vec<_>>()
            .join("\n");
        assert_eq!(status_screen(&top), None);
        // A login with a key, in another language.
        let key_login = STATUS
            .replace("work@example.com", "")
            .replace("Email:", "邮箱:")
            .replace("Authenticated", "已认证");
        assert_eq!(
            status_screen(&key_login),
            Some(Status {
                email: None,
                session: Some(SESSION.into()),
            })
        );
        assert_eq!(status_screen(READY), None);
    }

    #[test]
    fn limits_give_the_standard_windows() {
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(1_000_000);
        let status = limits_screen(LIMITS, now).expect("limits");
        let windows: Vec<(&str, f64, Option<SystemTime>)> = status
            .windows
            .iter()
            .map(|window| (window.label.as_str(), window.used_percent, window.resets_at))
            .collect();
        let day = 24 * 60 * 60;
        assert_eq!(
            windows,
            [
                ("5-hour", 48.0, Some(now + Duration::from_secs(39 * 60))),
                ("Weekly", 80.0, Some(now + Duration::from_secs(2 * day))),
                ("Monthly", 40.0, Some(now + Duration::from_secs(25 * day))),
            ]
        );
        assert_eq!(status.credits, None);
        assert_eq!(status.email, None);

        // In another language, with a label that runs into its number, a window not started
        // yet, and a balance.
        let translated = LIMITS
            .replace("Weekly   80%", "Settimanale12.5%")
            .replace("↻ 25 days", "Usa Droid per iniziare")
            .replace("$0.00", "$12.40");
        let status = limits_screen(&translated, now).expect("limits");
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
                ("5-hour", 48.0, true),
                ("Weekly", 12.5, true),
                ("Monthly", 40.0, false)
            ]
        );
        assert_eq!(status.credits.as_deref(), Some("$12.40"));

        assert_eq!(limits_screen(MENU_LIMITS, now), None);
        let half_drawn: String = LIMITS
            .lines()
            .take_while(|line| !line.contains("Monthly"))
            .collect::<Vec<_>>()
            .join("\n");
        assert_eq!(limits_screen(&half_drawn, now), None);
    }

    #[test]
    fn resets_are_read_in_droids_words() {
        let minutes = |minutes: u64| Some(Duration::from_secs(minutes * 60));
        assert_eq!(reset_after("39min"), minutes(39));
        assert_eq!(reset_after(" 1h 5min "), minutes(65));
        assert_eq!(reset_after("1 day"), minutes(24 * 60));
        assert_eq!(reset_after("3 days"), minutes(3 * 24 * 60));
        assert_eq!(reset_after("Use Droid to start"), None);
        assert_eq!(reset_after(""), None);
    }

    fn write_session(sessions: &Path, id: &str, cwd: &Path) {
        std::fs::create_dir_all(sessions).expect("create");
        let start = serde_json::json!({"type": "session_start", "id": id, "cwd": cwd});
        std::fs::write(sessions.join(format!("{id}.jsonl")), format!("{start}\n")).expect("write");
        std::fs::write(sessions.join(format!("{id}.settings.json")), "{}").expect("write");
    }

    #[test]
    fn reads_keep_one_session() {
        let agent = AgentCommand {
            env: [("FACTORY_HOME_OVERRIDE".to_string(), "/home".to_string())]
                .into_iter()
                .collect(),
            ..AgentCommand::default()
        };
        assert_eq!(
            sessions_folder(
                &agent,
                Path::new("/Users/me/Application Support/agentZ/reader")
            ),
            Path::new("/home/.factory/sessions/-Users-me-Application Support-agentZ-reader")
        );

        let dir = tempfile::tempdir().expect("temp dir");
        let sessions = dir.path().join("-reader");
        let folder = Path::new("/data/accounts/factory-droid/reader");
        let exists = |id: &str| sessions.join(format!("{id}.jsonl")).exists();
        let failed_read = "00000000-0000-4000-8000-000000000002";
        let elsewhere = "00000000-0000-4000-8000-000000000003";
        write_session(&sessions, failed_read, folder);
        write_session(&sessions, SESSION, folder);
        write_session(&sessions, elsewhere, Path::new("/Users/me/elsewhere"));
        std::fs::write(sessions.join("broken.jsonl"), "").expect("write");
        assert_eq!(
            reads_sessions(&sessions, folder).expect("sessions"),
            [SESSION, failed_read]
        );

        keep_only_session(&sessions, elsewhere, folder).expect_err("not a read's session");
        assert!(exists(failed_read));
        keep_only_session(&sessions, SESSION, folder).expect("keep one");
        assert!(exists(SESSION));
        assert!(!exists(failed_read));
        assert!(
            !sessions
                .join(format!("{failed_read}.settings.json"))
                .exists()
        );
        assert!(exists(elsewhere));

        remove_session(&sessions, SESSION).expect("remove");
        assert!(!exists(SESSION));
        assert!(!sessions.join(format!("{SESSION}.settings.json")).exists());
        remove_session(&sessions, SESSION).expect("already gone");
        assert!(
            reads_sessions(&dir.path().join("none"), folder)
                .expect("no sessions")
                .is_empty()
        );
    }

    /// Droid's terminal UI as the reader sees it: the captured screens, in turn, each after
    /// the keys that lead to it. Other keys are written to `unexpected`.
    const FAKE_DROID: &str = r#"
import json, os, sys, time, tty

screens = os.environ["DROID_SCREENS"]
cwd = os.getcwd()
session = "00000000-0000-4000-8000-000000000001"
resumes = "--resume" in sys.argv
with open("args.json", "w") as file:
    json.dump(sys.argv[1:], file)
if os.environ.get("DROID_BROKEN"):
    print("It broke")
    sys.exit(1)

def show(name, command=None):
    with open(os.path.join(screens, name + ".txt")) as file:
        text = file.read().replace("/private/tmp/reader", cwd)
    if command:
        text = text.replace("/limits", command)
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
if os.environ.get("DROID_LOGGED_OUT"):
    show("logged-out")
    time.sleep(60)
# A folder it opened a session in is trusted.
if not resumes:
    show("trust")
    expect("\r")
    project = os.path.join(
        os.environ["FACTORY_HOME_OVERRIDE"], ".factory", "sessions", "-" + cwd.strip("/").replace("/", "-")
    )
    os.makedirs(project, exist_ok=True)
    with open(os.path.join(project, session + ".jsonl"), "w") as file:
        file.write(json.dumps({"type": "session_start", "id": session, "cwd": cwd}) + "\n")
    with open(os.path.join(project, session + ".settings.json"), "w") as file:
        file.write("{}")
show("ready")
expect("/status")
show("menu-status-unfiltered")
time.sleep(0.3)
show("menu-limits", "/status")
expect("\r")
show("status")
expect("\x1b")
show("ready")
expect("/limits")
show("menu-limits")
expect("\r")
show("limits")
expect("\x1b")
show("ready")
time.sleep(60)
"#;

    fn fake_droid(dir: &Path, home: &Path) -> Option<AgentCommand> {
        let path = std::env::var_os("PATH")?;
        let python = std::env::split_paths(&path)
            .map(|dir| dir.join("python3"))
            .find(|candidate| candidate.is_file())?;
        let script = dir.join("fake_droid.py");
        std::fs::write(&script, FAKE_DROID).expect("write the fake");
        Some(AgentCommand {
            path: python,
            args: vec![
                script.to_string_lossy().into_owned(),
                "exec".into(),
                "--output-format".into(),
                "acp-daemon".into(),
            ],
            env: [
                (
                    "DROID_SCREENS".to_string(),
                    concat!(env!("CARGO_MANIFEST_DIR"), "/src/accounts/droid_screens").into(),
                ),
                (
                    "FACTORY_HOME_OVERRIDE".to_string(),
                    home.to_string_lossy().into_owned(),
                ),
            ]
            .into_iter()
            .collect(),
            ..AgentCommand::default()
        })
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn reads_droids_terminal_ui() {
        let dir = tempfile::tempdir().expect("temp dir");
        let home = dir.path().join("home");
        let folder = dir.path().join("reader");
        let Some(agent) = fake_droid(dir.path(), &home) else {
            return;
        };
        let args = || -> Vec<String> {
            serde_json::from_str(&std::fs::read_to_string(folder.join("args.json")).expect("args"))
                .expect("json")
        };
        let found = read(agent.clone(), &folder).await.expect("read");
        assert!(!folder.join("unexpected").exists());
        assert_eq!(found.logged_in, Some(true));
        assert_eq!(found.status.email.as_deref(), Some("work@example.com"));
        assert_eq!(found.status.windows.len(), 3);

        let real_folder = std::fs::canonicalize(&folder).expect("folder");
        let settings = real_folder.join(READER_SETTINGS_FILE);
        let settings = settings.to_string_lossy();
        assert_eq!(args(), ["--settings", &*settings]);
        assert_eq!(
            std::fs::read_to_string(&*settings).expect("settings"),
            READER_SETTINGS
        );
        let sessions = sessions_folder(&agent, &real_folder);
        assert_eq!(
            reads_sessions(&sessions, &real_folder).expect("sessions"),
            [SESSION]
        );

        // The next read resumes its session, and removes one a failed read left.
        let failed_read = "00000000-0000-4000-8000-000000000002";
        write_session(&sessions, failed_read, &real_folder);
        let found = read(agent.clone(), &folder).await.expect("read");
        assert!(!folder.join("unexpected").exists());
        assert_eq!(found.logged_in, Some(true));
        assert_eq!(args(), ["--settings", &*settings, "--resume", SESSION]);
        assert_eq!(
            reads_sessions(&sessions, &real_folder).expect("sessions"),
            [SESSION]
        );

        // A read that fails doesn't resume that session again.
        let mut broken = agent.clone();
        broken.env.insert("DROID_BROKEN".into(), "1".into());
        read(broken, &folder).await.expect_err("it broke");
        assert!(
            reads_sessions(&sessions, &real_folder)
                .expect("sessions")
                .is_empty()
        );

        let mut logged_out = agent;
        logged_out.env.insert("DROID_LOGGED_OUT".into(), "1".into());
        let found = read(logged_out, &folder).await.expect("read");
        assert_eq!(found.logged_in, Some(false));
    }
}
