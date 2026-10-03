//! Which agent CLI a workspace pane runs, and whether it's working, waiting for the user or
//! done, read from its screen the way herdr reads its panes (`src/detect/`): the foreground
//! process names the agent, and that agent's manifest reads the bottom of the screen and the
//! terminal's title.
//!
//! Ported from herdr (https://github.com/herdrdev/herdr), licensed under the Apache License,
//! Version 2.0 (see `LICENSE-APACHE`). Changed: only the agents with bundled manifests, no
//! Windows, no hook integrations, and one tracker per terminal instead of a task per pane.

mod manifest;
pub(crate) mod process;

use std::time::{Duration, Instant};

pub(crate) use manifest::DetectionInput;
use process::ForegroundProcess;

/// A terminal agent's state, as its manifest reads it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AgentState {
    /// Finished, its prompt waiting for the next task.
    Idle,
    Working,
    /// Waiting for the user to answer, such as a permission prompt.
    Blocked,
    /// A plain shell, another program, or an agent's screen that says nothing either way.
    Unknown,
}

/// What a manifest made of a screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct AgentDetection {
    pub state: AgentState,
    /// The agent shows something other than its live prompt, such as a transcript viewer, so
    /// the screen says nothing about its state.
    pub skip_state_update: bool,
    /// The agent's idle prompt is on screen, which is stronger than nothing matching.
    pub visible_idle: bool,
    pub visible_blocker: bool,
    pub visible_working: bool,
}

impl AgentDetection {
    /// A known agent whose screen matches no rule (herdr's
    /// `default_known_agent_idle_fallback`).
    fn idle() -> Self {
        Self {
            state: AgentState::Idle,
            skip_state_update: false,
            visible_idle: false,
            visible_blocker: false,
            visible_working: false,
        }
    }
}

/// The agent CLIs herdr has manifests for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Agent {
    Pi,
    Claude,
    Codex,
    Gemini,
    Cursor,
    Devin,
    Antigravity,
    Cline,
    OpenCode,
    GithubCopilot,
    Kimi,
    Kiro,
    Droid,
    Amp,
    Grok,
    Hermes,
    Kilo,
    Qodercli,
    Qwen,
    Letta,
    Maki,
    Muse,
}

impl Agent {
    pub(crate) const SCREEN_MANIFEST_AGENTS: [Self; 22] = [
        Self::Pi,
        Self::Claude,
        Self::Codex,
        Self::Gemini,
        Self::Cursor,
        Self::Devin,
        Self::Antigravity,
        Self::Cline,
        Self::OpenCode,
        Self::GithubCopilot,
        Self::Kimi,
        Self::Kiro,
        Self::Droid,
        Self::Amp,
        Self::Grok,
        Self::Hermes,
        Self::Kilo,
        Self::Qodercli,
        Self::Qwen,
        Self::Letta,
        Self::Maki,
        Self::Muse,
    ];

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Pi => "pi",
            Self::Claude => "claude",
            Self::Codex => "codex",
            Self::Gemini => "gemini",
            Self::Cursor => "cursor",
            Self::Devin => "devin",
            Self::Antigravity => "agy",
            Self::Cline => "cline",
            Self::OpenCode => "opencode",
            Self::GithubCopilot => "copilot",
            Self::Kimi => "kimi",
            Self::Kiro => "kiro",
            Self::Droid => "droid",
            Self::Amp => "amp",
            Self::Grok => "grok",
            Self::Hermes => "hermes",
            Self::Kilo => "kilo",
            Self::Qodercli => "qodercli",
            Self::Qwen => "qwen",
            Self::Letta => "letta",
            Self::Maki => "maki",
            Self::Muse => "muse",
        }
    }

    /// The agent a label or executable name stands for.
    pub(crate) fn from_label(name: &str) -> Option<Self> {
        let name = normalized_agent_lookup_name(name);
        match path_basename(&name) {
            "pi" => Some(Self::Pi),
            "claude" | "claude-code" => Some(Self::Claude),
            "codex" => Some(Self::Codex),
            "gemini" => Some(Self::Gemini),
            "cursor" | "cursor-agent" => Some(Self::Cursor),
            "devin" | "devin-cli" | "devin cli" => Some(Self::Devin),
            "agy" | "antigravity" | "antigravity-cli" => Some(Self::Antigravity),
            "cline" | ".cline" => Some(Self::Cline),
            "opencode" | "opencode2" | "open-code" => Some(Self::OpenCode),
            "copilot" | "github-copilot" | "ghcs" => Some(Self::GithubCopilot),
            "kimi" | "kimi-code" | "kimi code" => Some(Self::Kimi),
            "kiro" | "kiro-cli" => Some(Self::Kiro),
            "droid" => Some(Self::Droid),
            "amp" | "amp-local" => Some(Self::Amp),
            "grok" | "grok-build" => Some(Self::Grok),
            "hermes" | "hermes-agent" => Some(Self::Hermes),
            "kilo" | "kilo-code" | "kilo code" => Some(Self::Kilo),
            "qodercli" | "qoderclicn" | "qoder" | "qodercn" => Some(Self::Qodercli),
            "qwen" | "qwen-code" | "qwen code" => Some(Self::Qwen),
            "letta" | "letta-code" | "letta code" => Some(Self::Letta),
            "maki" => Some(Self::Maki),
            "muse" | "muse-code" | "muse-cli" => Some(Self::Muse),
            name if is_muse_versioned_binary(name) => Some(Self::Muse),
            _ => None,
        }
    }

    /// The agent a foreground process runs, seeing through the runtimes and shells agents are
    /// launched with (`node cli.js`, `sh ./codex`).
    pub(crate) fn of_process(process: &ForegroundProcess) -> Option<Self> {
        Self::from_label(&normalized_process_name(process))
    }
}

/// Muse's launcher execs `muse-bin-<version>`; a digit must follow the prefix.
fn is_muse_versioned_binary(name: &str) -> bool {
    path_basename(name)
        .strip_prefix("muse-bin-")
        .is_some_and(|rest| rest.starts_with(|c: char| c.is_ascii_digit()))
}

fn normalized_process_name(process: &ForegroundProcess) -> String {
    let effective = process.argv0.as_deref().unwrap_or(&process.name);
    let lower_effective = effective.to_lowercase();

    if is_generic_runtime_or_shell(&lower_effective)
        && let Some(wrapped_agent) =
            wrapped_agent_name_from_runtime_argv(&lower_effective, process.argv.as_deref())
    {
        return wrapped_agent;
    }

    if Agent::from_label(effective).is_some() {
        return effective.to_string();
    }

    if let Some(runtime) = process.argv.as_deref().and_then(|argv| argv.first()) {
        let runtime_name = normalized_agent_lookup_name(path_basename(runtime));
        if matches!(runtime_name.as_str(), "node" | "bun")
            && let Some(wrapped_agent) =
                wrapped_agent_name_from_runtime_argv(runtime, process.argv.as_deref())
            && matches!(
                Agent::from_label(&wrapped_agent),
                Some(Agent::Qwen | Agent::Cline | Agent::Letta)
            )
        {
            return wrapped_agent;
        }
    }

    if let Some(wrapped_agent) = process
        .argv
        .as_deref()
        .and_then(|argv| argv.first())
        .and_then(|argv0| agent_name_from_path_token(argv0))
    {
        return wrapped_agent;
    }

    effective.to_string()
}

fn wrapped_agent_name_from_runtime_argv(runtime: &str, argv: Option<&[String]>) -> Option<String> {
    let argv = argv?;
    match normalized_agent_lookup_name(path_basename(runtime)).as_str() {
        "node" | "bun" => script_arg_agent_name(argv, &["-e", "--eval", "-p", "--print"], &[]),
        name if is_python_runtime(name) => script_arg_agent_name(argv, &["-c"], &["-m"]),
        "sh" | "bash" | "zsh" | "fish" => script_arg_agent_name(argv, &["-c"], &[]),
        _ => None,
    }
}

fn script_arg_agent_name(
    argv: &[String],
    eval_flags: &[&str],
    module_flags: &[&str],
) -> Option<String> {
    let mut args = argv.iter().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--" {
            return args
                .next()
                .and_then(|token| agent_name_from_path_token(token));
        }
        if flag_matches(arg, eval_flags) || flag_matches(arg, module_flags) {
            return None;
        }
        if arg.starts_with('-') {
            if option_takes_value(arg) {
                args.next();
            }
            continue;
        }
        return agent_name_from_path_token(arg);
    }
    None
}

fn flag_matches(arg: &str, flags: &[&str]) -> bool {
    flags
        .iter()
        .any(|flag| arg == *flag || short_flag_payload(arg, flag) || long_flag_value(arg, flag))
}

fn short_flag_payload(arg: &str, flag: &str) -> bool {
    flag.starts_with('-')
        && !flag.starts_with("--")
        && arg.starts_with(flag)
        && arg.len() > flag.len()
}

fn long_flag_value(arg: &str, flag: &str) -> bool {
    flag.starts_with("--")
        && arg
            .strip_prefix(flag)
            .is_some_and(|rest| rest.starts_with('='))
}

fn option_takes_value(arg: &str) -> bool {
    matches!(
        arg,
        "-r" | "--require"
            | "--loader"
            | "--import"
            | "--experimental-loader"
            | "--inspect-port"
            | "-W"
            | "-X"
            | "-S"
            | "-L"
            | "-o"
    )
}

fn agent_name_from_path_token(token: &str) -> Option<String> {
    let trimmed = token.trim_matches(|c| matches!(c, '"' | '\''));
    if trimmed.is_empty() || trimmed.starts_with('-') {
        return None;
    }
    agent_name_from_basename(path_basename(trimmed))
        .or_else(|| agent_name_from_known_package_path(trimmed))
        .or_else(|| resolved_agent_name_from_path_token(trimmed))
}

fn agent_name_from_known_package_path(path: &str) -> Option<String> {
    let raw_components: Vec<&str> = path
        .split(['/', '\\'])
        .filter(|component| !component.is_empty())
        .collect();
    let ends_with = |suffix: &[&str]| {
        raw_components.len() >= suffix.len()
            && raw_components[raw_components.len() - suffix.len()..]
                .iter()
                .zip(suffix)
                .all(|(actual, expected)| actual.eq_ignore_ascii_case(expected))
    };
    if ends_with(&[
        "node_modules",
        "@earendil-works",
        "pi-coding-agent",
        "dist",
        "cli.js",
    ]) || ends_with(&[
        "node_modules",
        "@earendil-works",
        "pi-coding-agent",
        "dist",
        "bundle",
        "cli.js",
    ]) {
        return Some(Agent::Pi.label().to_string());
    }
    if ends_with(&[
        "node_modules",
        "@moonshot-ai",
        "kimi-code",
        "dist",
        "main.mjs",
    ]) {
        return Some(Agent::Kimi.label().to_string());
    }

    let components: Vec<String> = raw_components
        .into_iter()
        .map(normalized_agent_lookup_name)
        .collect();
    for window in components.windows(5) {
        if window == ["node_modules", "@qwen-code", "qwen-code", "dist", "index"] {
            return Some(Agent::Qwen.label().to_string());
        }
    }
    for window in components.windows(4) {
        if window == ["node_modules", "@letta-ai", "letta-code", "letta"] {
            return Some(Agent::Letta.label().to_string());
        }
    }
    None
}

fn resolved_agent_name_from_path_token(token: &str) -> Option<String> {
    let path = std::path::Path::new(token);
    if path.components().count() < 2 {
        return None;
    }
    let resolved = std::fs::canonicalize(path).ok()?;
    agent_name_from_basename(resolved.file_name()?.to_str()?)
}

fn agent_name_from_basename(basename: &str) -> Option<String> {
    Some(Agent::from_label(basename)?.label().to_string())
}

fn normalized_agent_lookup_name(name: &str) -> String {
    let mut name = name.trim().to_lowercase();
    for suffix in [".exe", ".cmd", ".bat", ".ps1", ".js"] {
        if name.ends_with(suffix) {
            name.truncate(name.len() - suffix.len());
            break;
        }
    }
    name
}

fn path_basename(path: &str) -> &str {
    path.rsplit(['/', '\\'])
        .find(|component| !component.is_empty())
        .unwrap_or(path)
}

fn is_generic_runtime_or_shell(name: &str) -> bool {
    let name = normalized_agent_lookup_name(path_basename(name));
    is_python_runtime(&name)
        || matches!(
            name.as_str(),
            "sh" | "bash" | "zsh" | "fish" | "tmux" | "node" | "bun"
        )
}

fn is_python_runtime(name: &str) -> bool {
    name == "python"
        || name.strip_prefix("python").is_some_and(|version| {
            !version.is_empty()
                && version
                    .split('.')
                    .all(|part| !part.is_empty() && part.chars().all(|ch| ch.is_ascii_digit()))
        })
}

/// herdr's timings (`src/pane.rs`).
const PROCESS_RECHECK_IDENTIFIED: Duration = Duration::from_secs(5);
const ACQUISITION_WINDOW: Duration = Duration::from_secs(8);
const ACQUISITION_FAST_WINDOW: Duration = Duration::from_millis(1500);
const ACQUISITION_FAST_RECHECK: Duration = Duration::from_millis(500);
const ACQUISITION_SLOW_RECHECK: Duration = Duration::from_secs(2);
/// An agent that just started draws its first screens; they say nothing yet.
const AGENT_STARTUP_GRACE_WINDOW: Duration = Duration::from_secs(3);
/// Probes in a row that must miss a known agent, while something else is in the foreground,
/// before it counts as gone.
const AGENT_MISS_CONFIRMATION_ATTEMPTS: u32 = 6;
/// Working only turns idle once this many more screens agree, or this long has passed,
/// unless the idle prompt itself is on screen.
const AGENT_PENDING_IDLE_CONFIRMATIONS: u32 = 3;
const AGENT_PENDING_IDLE_CAP: Duration = Duration::from_millis(700);

/// How often to look at a terminal: often while working may be turning idle, less while an
/// agent runs, least while none does (herdr's ticks).
pub(crate) const TICK_PENDING_IDLE: Duration = Duration::from_millis(100);
pub(crate) const TICK_AGENT: Duration = Duration::from_millis(300);
pub(crate) const TICK_NO_AGENT: Duration = Duration::from_millis(500);

/// What one look at the terminal's foreground found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ProcessObservation {
    pub process_group_id: Option<u32>,
    /// The terminal's own shell is in the foreground, so whatever ran in it has ended.
    pub shell_in_foreground: bool,
    pub agent: Option<Agent>,
}

/// Follows one terminal's agent and its state across ticks (herdr's per-pane detection task).
#[derive(Debug, Default)]
pub(crate) struct AgentTracker {
    agent: Option<Agent>,
    state: Option<AgentState>,
    process_group_id: Option<u32>,
    probed_at: Option<Instant>,
    acquisition_started_at: Option<Instant>,
    misses: u32,
    grace_until: Option<Instant>,
    /// The grace window ended; the next tick still skips the screen (herdr).
    grace_ending: bool,
    pending_idle: Option<PendingIdle>,
    scanned_content: Option<u64>,
}

#[derive(Debug, Clone, Copy)]
struct PendingIdle {
    since: Instant,
    confirmations: u32,
}

impl AgentTracker {
    pub(crate) fn agent(&self) -> Option<Agent> {
        self.agent
    }

    pub(crate) fn next_tick(&self) -> Duration {
        if self.pending_idle.is_some() {
            TICK_PENDING_IDLE
        } else if self.agent.is_some() {
            TICK_AGENT
        } else {
            TICK_NO_AGENT
        }
    }

    /// Whether the foreground process should be identified again, given its process group
    /// now (herdr's `should_probe_foreground_job`).
    pub(crate) fn should_probe(&self, now: Instant, process_group_id: Option<u32>) -> bool {
        let Some(probed_at) = self.probed_at else {
            return true;
        };
        if process_group_id != self.process_group_id {
            return true;
        }
        let since = now.saturating_duration_since(probed_at);
        if self.agent.is_some() {
            return since >= PROCESS_RECHECK_IDENTIFIED;
        }
        match self.acquisition_started_at {
            Some(started) if now.saturating_duration_since(started) <= ACQUISITION_WINDOW => {
                let recheck = if now.saturating_duration_since(started) <= ACQUISITION_FAST_WINDOW {
                    ACQUISITION_FAST_RECHECK
                } else {
                    ACQUISITION_SLOW_RECHECK
                };
                since >= recheck
            }
            _ => false,
        }
    }

    /// The screen changed while no agent is known: an agent may be starting, so look harder
    /// for a while (herdr's `sync_content_change_acquisition`).
    pub(crate) fn content_changed(&mut self, now: Instant) {
        if self.agent.is_none()
            && self
                .acquisition_started_at
                .is_none_or(|started| now.saturating_duration_since(started) > ACQUISITION_WINDOW)
        {
            self.acquisition_started_at = Some(now);
        }
    }

    /// Takes a probe's result. Returns a state to publish when the agent came or went.
    pub(crate) fn observe_process(
        &mut self,
        now: Instant,
        observation: ProcessObservation,
    ) -> Option<AgentState> {
        if observation.process_group_id != self.process_group_id {
            self.process_group_id = observation.process_group_id;
            if observation.agent.is_none() {
                self.acquisition_started_at = Some(now);
            }
        }
        self.probed_at = Some(now);
        match observation.agent {
            Some(agent) => {
                self.misses = 0;
                if self.agent == Some(agent) {
                    return None;
                }
                self.agent = Some(agent);
                self.acquisition_started_at = None;
                self.grace_until = Some(now + AGENT_STARTUP_GRACE_WINDOW);
                self.grace_ending = false;
                self.pending_idle = None;
                self.scanned_content = None;
                self.publish(AgentState::Unknown)
            }
            None if self.agent.is_none() => None,
            None => {
                self.misses += 1;
                if !observation.shell_in_foreground
                    && self.misses < AGENT_MISS_CONFIRMATION_ATTEMPTS
                {
                    return None;
                }
                // Back at the shell: the agent finished, which counts as done.
                self.agent_gone()
            }
        }
    }

    /// The terminal's process exited.
    pub(crate) fn exited(&mut self) -> Option<AgentState> {
        if self.agent.is_none() && self.state.is_none() {
            return None;
        }
        self.agent_gone()
    }

    fn agent_gone(&mut self) -> Option<AgentState> {
        self.agent = None;
        self.misses = 0;
        self.grace_until = None;
        self.grace_ending = false;
        self.pending_idle = None;
        self.scanned_content = None;
        self.publish(AgentState::Idle)
    }

    /// Whether the screen needs reading on this tick. `content` counts the terminal's screen
    /// changes, so an idle agent's unchanged screen isn't read again.
    pub(crate) fn should_scan(&mut self, now: Instant, content: u64) -> bool {
        if self.agent.is_none() {
            return false;
        }
        if let Some(grace_until) = self.grace_until {
            if now < grace_until {
                return false;
            }
            self.grace_until = None;
            self.grace_ending = true;
            return false;
        }
        if self.grace_ending {
            self.grace_ending = false;
            return false;
        }
        !(self.state == Some(AgentState::Idle)
            && self.pending_idle.is_none()
            && self.scanned_content == Some(content))
    }

    /// Takes what the agent's manifest made of the screen. Returns a state to publish.
    pub(crate) fn observe_screen(
        &mut self,
        now: Instant,
        content: u64,
        detection: AgentDetection,
    ) -> Option<AgentState> {
        self.scanned_content = Some(content);
        if detection.skip_state_update {
            self.pending_idle = None;
            return None;
        }
        let state = detection.state;
        if self.state == Some(AgentState::Working)
            && state == AgentState::Idle
            && !detection.visible_idle
            && !detection.visible_blocker
        {
            match &mut self.pending_idle {
                None => {
                    self.pending_idle = Some(PendingIdle {
                        since: now,
                        confirmations: 0,
                    });
                    return None;
                }
                Some(pending) => {
                    pending.confirmations += 1;
                    if pending.confirmations < AGENT_PENDING_IDLE_CONFIRMATIONS
                        && now.saturating_duration_since(pending.since) < AGENT_PENDING_IDLE_CAP
                    {
                        return None;
                    }
                }
            }
        }
        self.pending_idle = None;
        self.publish(state)
    }

    fn publish(&mut self, state: AgentState) -> Option<AgentState> {
        if self.state == Some(state) {
            return None;
        }
        self.state = Some(state);
        Some(state)
    }
}

/// What the agent's manifest makes of `screen` and the terminal's title.
pub(crate) fn detect(agent: Agent, input: DetectionInput<'_>) -> AgentDetection {
    manifest::detect(agent, input)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn process(name: &str, argv: &[&str]) -> ForegroundProcess {
        ForegroundProcess::new(
            name.to_string(),
            argv.iter().map(|arg| arg.to_string()).collect(),
        )
    }

    #[test]
    fn identifies_agents_through_runtimes_and_shells() {
        assert_eq!(
            Agent::of_process(&process("codex", &["codex", "--yolo"])),
            Some(Agent::Codex)
        );
        assert_eq!(
            Agent::of_process(&process(
                "node",
                &["node", "--require", "x", "/usr/lib/claude"]
            )),
            Some(Agent::Claude)
        );
        assert_eq!(
            Agent::of_process(&process("sh", &["/bin/sh", "/tmp/bin/codex"])),
            Some(Agent::Codex)
        );
        assert_eq!(
            Agent::of_process(&process("node", &["node", "-e", "codex"])),
            None
        );
        assert_eq!(
            Agent::of_process(&process("python3.12", &["python3.12", "-m", "droid"])),
            None
        );
        assert_eq!(
            Agent::of_process(&process(
                "node",
                &["node", "/x/node_modules/@qwen-code/qwen-code/dist/index.js"]
            )),
            Some(Agent::Qwen)
        );
        assert_eq!(Agent::of_process(&process("zsh", &["-zsh"])), None);
        assert_eq!(Agent::of_process(&process("vim", &["vim", "codex"])), None);
        assert_eq!(
            Agent::from_label("muse-bin-0.1.0-R708.1"),
            Some(Agent::Muse)
        );
        assert_eq!(Agent::from_label("muse-binary"), None);
        assert_eq!(Agent::from_label("Claude-Code"), Some(Agent::Claude));
    }

    fn observation(agent: Option<Agent>, shell_in_foreground: bool) -> ProcessObservation {
        ProcessObservation {
            process_group_id: Some(if agent.is_some() { 20 } else { 10 }),
            shell_in_foreground,
            agent,
        }
    }

    fn detection(state: AgentState, visible: bool) -> AgentDetection {
        AgentDetection {
            state,
            skip_state_update: false,
            visible_idle: visible && state == AgentState::Idle,
            visible_blocker: visible && state == AgentState::Blocked,
            visible_working: visible && state == AgentState::Working,
        }
    }

    #[test]
    fn follows_an_agent_from_start_to_exit() {
        let start = Instant::now();
        let mut tracker = AgentTracker::default();
        assert!(tracker.should_probe(start, Some(10)));
        assert_eq!(
            tracker.observe_process(start, observation(None, true)),
            None
        );
        assert!(!tracker.should_scan(start, 0));

        // The agent starts: unknown until its startup grace passes.
        assert!(tracker.should_probe(start, Some(20)));
        assert_eq!(
            tracker.observe_process(start, observation(Some(Agent::Codex), false)),
            Some(AgentState::Unknown)
        );
        assert!(!tracker.should_scan(start + Duration::from_secs(1), 1));
        let after_grace = start + AGENT_STARTUP_GRACE_WINDOW;
        assert!(!tracker.should_scan(after_grace, 1));
        assert!(!tracker.should_scan(after_grace, 1));
        assert!(tracker.should_scan(after_grace, 1));
        assert_eq!(
            tracker.observe_screen(after_grace, 1, detection(AgentState::Working, true)),
            Some(AgentState::Working)
        );

        // Working only turns idle once the screen agrees a few times.
        let t = after_grace + Duration::from_millis(100);
        assert_eq!(
            tracker.observe_screen(t, 2, detection(AgentState::Idle, false)),
            None
        );
        assert_eq!(tracker.next_tick(), TICK_PENDING_IDLE);
        assert_eq!(
            tracker.observe_screen(t, 2, detection(AgentState::Idle, false)),
            None
        );
        assert_eq!(
            tracker.observe_screen(t, 2, detection(AgentState::Working, true)),
            None
        );
        assert_eq!(
            tracker.observe_screen(t, 2, detection(AgentState::Idle, false)),
            None
        );
        assert_eq!(
            tracker.observe_screen(
                t + AGENT_PENDING_IDLE_CAP,
                2,
                detection(AgentState::Idle, false)
            ),
            Some(AgentState::Idle)
        );
        // An unchanged idle screen isn't read again.
        assert!(!tracker.should_scan(t, 2));
        assert!(tracker.should_scan(t, 3));
        assert_eq!(
            tracker.observe_screen(t, 3, detection(AgentState::Blocked, true)),
            Some(AgentState::Blocked)
        );
        assert_eq!(
            tracker.observe_screen(t, 4, detection(AgentState::Working, true)),
            Some(AgentState::Working)
        );
        // The idle prompt on screen is enough at once.
        assert_eq!(
            tracker.observe_screen(t, 5, detection(AgentState::Idle, true)),
            Some(AgentState::Idle)
        );

        // Another program in the foreground has to miss the agent several times.
        for _ in 1..AGENT_MISS_CONFIRMATION_ATTEMPTS {
            assert_eq!(tracker.observe_process(t, observation(None, false)), None);
        }
        assert_eq!(tracker.agent(), Some(Agent::Codex));
        assert_eq!(
            tracker.observe_process(t, observation(Some(Agent::Codex), false)),
            None
        );
        // Back at the shell, it's gone at once.
        assert_eq!(
            tracker.observe_process(t, observation(Some(Agent::Codex), false)),
            None
        );
        assert_eq!(
            tracker.observe_screen(t, 6, detection(AgentState::Working, true)),
            Some(AgentState::Working)
        );
        assert_eq!(
            tracker.observe_process(t, observation(None, true)),
            Some(AgentState::Idle)
        );
        assert_eq!(tracker.agent(), None);
    }

    #[test]
    fn probes_harder_while_an_agent_may_be_starting() {
        let start = Instant::now();
        let mut tracker = AgentTracker::default();
        tracker.observe_process(start, observation(None, true));
        tracker.content_changed(start);
        assert!(!tracker.should_probe(start + Duration::from_millis(400), Some(10)));
        assert!(tracker.should_probe(start + Duration::from_millis(500), Some(10)));
        tracker.observe_process(start + Duration::from_secs(2), observation(None, true));
        assert!(!tracker.should_probe(start + Duration::from_secs(3), Some(10)));
        assert!(tracker.should_probe(start + Duration::from_secs(4), Some(10)));
        tracker.observe_process(start + Duration::from_secs(4), observation(None, true));
        assert!(!tracker.should_probe(start + Duration::from_secs(20), Some(10)));
    }
}
