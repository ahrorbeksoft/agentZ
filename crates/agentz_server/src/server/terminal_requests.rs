//! The server's terminals: starting them for threads, applying clients' input, and streaming
//! screens to the clients watching (herdr's "surface interest"), at most once a frame.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use agent_client_protocol::schema::v1 as acp;
use agent_thread::{AgentThread, TerminalRequest, TurnPoint};
use agentz_protocol::spaces::SpaceFolder;
use agentz_protocol::terminal::{TerminalExit, TerminalKey};
use agentz_protocol::{ConnectionId, Event, Request, Response, ServerMessage};
use alacritty_terminal::event::Event as AlacEvent;
use anyhow::{Context as _, Result, anyhow};
use futures::StreamExt as _;
use futures::channel::mpsc;
use projects::ThreadId;
use util::ResultExt as _;
use util::shell::Shell;
use util::shell_builder::ShellBuilder;

use super::{ClientId, Input, Server, send_to};
use crate::browser;
use crate::checkpoints::Checkpoints;
use crate::detect::process::ForegroundProcess;
use crate::detect::{self, Agent, AgentState, AgentTracker, DetectionInput, ProcessObservation};
use crate::terminal_programs;
use crate::terminals::{Terminal, TerminalSize, TerminalSpawn, frame_changes, without_variables};
use projects::TerminalFolder;

/// Screens are sent at most this often, however fast the output.
const FRAME_INTERVAL: Duration = Duration::from_millis(16);
/// A terminal thread's output counts as activity at most this often: a busy terminal
/// shouldn't save the threads on every frame.
const TERMINAL_ACTIVITY_INTERVAL: Duration = Duration::from_secs(10);
/// How long after a resize or focus change a terminal's output counts as a redraw.
const TERMINAL_REDRAW_GRACE: Duration = Duration::from_secs(1);

pub(super) struct RunningTerminal {
    /// Tells this run's events from an earlier run's, after a restart.
    serial: u64,
    pub(super) terminal: Terminal,
    /// For an agent's terminal: how much output `terminal/output` returns.
    output_byte_limit: Option<usize>,
    /// The agent released it. It stays for the user to look at, as ACP asks, but the agent
    /// can't use it anymore.
    released: bool,
    /// Counts the screen's changes, so agent detection only rereads a changed screen.
    content: u64,
    /// For a pane's or terminal thread's terminal: the agent CLI running in it and its state.
    /// A thread's drawer isn't watched.
    tracker: Option<AgentTracker>,
    /// For a terminal thread: the process group last in front, to look up what runs there
    /// only when it changes.
    foreground_group: Option<u32>,
    /// When the terminal's output last counted as its thread's activity.
    activity_recorded_at: Option<Instant>,
    /// Until when output is the program redrawing for a resize or a focus change, which
    /// isn't activity.
    redraw_until: Option<Instant>,
    /// For a terminal thread or workspace pane: the folder last seen in front. It names a
    /// shell, and a workspace follows the folders of its tabs.
    pub(super) folder: Option<PathBuf>,
}

/// The server's terminal bookkeeping.
#[derive(Default)]
pub(super) struct Terminals {
    pub(super) running: HashMap<TerminalKey, RunningTerminal>,
    /// The method each [`TerminalKey::Login`] terminal runs.
    logins: HashMap<ConnectionId, acp::AuthMethodId>,
    next_serial: u64,
    /// The last theme colors a client sent, for terminals started later.
    pub(super) palette: Option<Vec<[u8; 3]>>,
    /// Terminals whose screens may have changed since they were last sent.
    dirty: Vec<TerminalKey>,
    frames_sent_at: Option<Instant>,
    tick_scheduled: bool,
    detection_scheduled: bool,
    /// Takes terminal threads' checkpoints one after another, so a turn's end follows its
    /// start. Started with the first.
    checkpoint_queue: Option<mpsc::UnboundedSender<CheckpointJob>>,
}

/// A checkpoint of a terminal thread's folder, and what to do on the server once it's taken.
struct CheckpointJob {
    checkpoints: Checkpoints,
    point: TurnPoint,
    then: Option<Box<dyn FnOnce(&mut Server) + Send>>,
}

impl Server {
    pub(super) fn terminal_request(
        &mut self,
        client: ClientId,
        request: Request,
    ) -> Result<Response> {
        match request {
            Request::DrawerTerminals(thread) => {
                let mut numbers: Vec<u32> = self
                    .terminals
                    .running
                    .keys()
                    .filter_map(|key| match key {
                        TerminalKey::Drawer(thread_id) if *thread_id == thread => Some(1),
                        TerminalKey::DrawerTerminal { thread_id, number }
                            if *thread_id == thread =>
                        {
                            Some(*number)
                        }
                        _ => None,
                    })
                    .collect();
                numbers.sort_unstable();
                Ok(Response::DrawerTerminals(numbers))
            }
            Request::SubscribeTerminal(key) => {
                self.client(client)?;
                self.ensure_terminal(&key)?;
                let frame = self.terminal(&key)?.frame();
                self.client(client)?.terminals.insert(key, frame.clone());
                Ok(Response::TerminalFrame(frame))
            }
            Request::UnsubscribeTerminal(key) => {
                self.client(client)?.terminals.remove(&key);
                Ok(Response::Ok)
            }
            Request::TerminalInput { terminal, input } => {
                if let agentz_protocol::terminal::TerminalInput::Palette(palette) = &input {
                    self.terminals.palette = Some(palette.clone());
                }
                let running = self
                    .terminals
                    .running
                    .get_mut(&terminal)
                    .context("the terminal isn't running")?;
                // Opening or focusing a thread resizes its terminal or tells the program, and
                // the program redraws; that isn't the thread doing something.
                if matches!(
                    input,
                    agentz_protocol::terminal::TerminalInput::Resize { .. }
                        | agentz_protocol::terminal::TerminalInput::Focus(_)
                ) {
                    running.redraw_until = Some(Instant::now() + TERMINAL_REDRAW_GRACE);
                }
                let changed = running.terminal.input(input);
                if changed {
                    self.terminal_changed(terminal);
                }
                Ok(Response::Ok)
            }
            Request::TerminalSelectionText(key) => Ok(Response::Message(
                self.terminal(&key)?.selection_text().unwrap_or_default(),
            )),
            Request::FindInTerminal { terminal, query } => Ok(Response::TerminalMatches(
                self.terminal(&terminal)?.find(&query),
            )),
            Request::RestartTerminal(key) => {
                let spawn = match self.terminal_spawn(&key) {
                    Ok(spawn) => spawn,
                    // An agent's terminal runs what the agent asked for.
                    Err(_) => self.terminal(&key)?.spawn().clone(),
                };
                let size = self.terminal(&key).map(Terminal::size).unwrap_or_default();
                self.start_terminal(key, spawn, size)?;
                Ok(Response::Ok)
            }
            Request::CloseTerminal(key) => {
                self.close_terminal(&key);
                Ok(Response::Ok)
            }
            request => Err(anyhow!("not a terminal request: {request:?}")),
        }
    }

    fn terminal(&self, key: &TerminalKey) -> Result<&Terminal> {
        self.terminals
            .running
            .get(key)
            .map(|running| &running.terminal)
            .context("the terminal isn't running")
    }

    /// Starts a thread's terminal or drawer if it isn't running. Agents' terminals only start
    /// when the agent asks.
    pub(super) fn ensure_terminal(&mut self, key: &TerminalKey) -> Result<()> {
        if self.terminals.running.contains_key(key) {
            return Ok(());
        }
        let spawn = self.terminal_spawn(key)?;
        self.start_terminal(key.clone(), spawn, TerminalSize::default())
    }

    /// What a thread's terminal or drawer, or a pane's terminal, runs.
    fn terminal_spawn(&self, key: &TerminalKey) -> Result<TerminalSpawn> {
        let thread_id = match key {
            TerminalKey::Thread(thread_id)
            | TerminalKey::Drawer(thread_id)
            | TerminalKey::DrawerTerminal { thread_id, .. } => *thread_id,
            TerminalKey::Agent { .. } => {
                return Err(anyhow!("an agent's terminal starts when the agent asks"));
            }
            TerminalKey::Login(_) => {
                return Err(anyhow!(
                    "a login terminal starts when a login method is chosen"
                ));
            }
            TerminalKey::Pane(pane) => return self.pane_terminal_spawn(*pane),
        };
        let thread = self.projects.thread(thread_id).context("no such thread")?;
        let command = match key {
            TerminalKey::Thread(_) => thread
                .terminal
                .as_ref()
                .context("this thread runs an agent, not a terminal")?
                .command
                .clone(),
            _ => None,
        };
        Ok(TerminalSpawn {
            program: self.terminal_program(command),
            cwd: self
                .projects
                .thread_folder(thread_id)
                .context("no such project")?,
            env: self.terminal_env(Some(thread_id)),
        })
    }

    /// The shell, or the shell running `command`.
    pub(super) fn terminal_program(
        &self,
        command: Option<String>,
    ) -> Option<(String, Vec<String>)> {
        match (&self.terminal_shell, command) {
            (Some(shell), None) => Some((shell.clone(), Vec::new())),
            (Some(shell), Some(command)) => Some((shell.clone(), vec!["-c".into(), command])),
            (None, None) => None,
            // A login, interactive shell, so the command finds what the user's shell setup puts
            // on `PATH` (Zed loads its environment the same way).
            (None, Some(command)) => Some((
                util::shell::get_system_shell(),
                vec!["-l".into(), "-i".into(), "-c".into(), command],
            )),
        }
    }

    /// herdr's `HERDR_*` variables, so the CLI run in the terminal knows its thread.
    pub(super) fn terminal_env(&self, thread_id: Option<ThreadId>) -> HashMap<String, String> {
        let mut env = HashMap::new();
        if let Some(control) = &self.agent_control {
            env.insert(
                "AGENTZ_BIN_PATH".to_string(),
                control.executable.to_string_lossy().into_owned(),
            );
            env.insert(
                "AGENTZ_SOCKET".to_string(),
                control.socket.to_string_lossy().into_owned(),
            );
            if let Some(thread_id) = thread_id {
                env.insert("AGENTZ_THREAD_ID".to_string(), thread_id.0.to_string());
            }
        }
        env
    }

    /// Starts `spawn` under `key`, replacing whatever ran there.
    pub(super) fn start_terminal(
        &mut self,
        key: TerminalKey,
        spawn: TerminalSpawn,
        size: TerminalSize,
    ) -> Result<()> {
        if self.handing_off {
            return Err(anyhow!("the server is being updated"));
        }
        let serial = self.terminals.next_serial;
        self.terminals.next_serial += 1;
        let terminal = Terminal::start(
            spawn,
            size,
            self.terminals.palette.clone(),
            self.terminal_events(&key, serial),
        )?;
        let output_byte_limit = self
            .terminals
            .running
            .get(&key)
            .and_then(|running| running.output_byte_limit);
        // Whatever ran before is gone with its terminal.
        let tracker = match key {
            TerminalKey::Thread(thread_id) => {
                self.publish_terminal_agent(thread_id, None, AgentState::Unknown);
                Some(AgentTracker::default())
            }
            TerminalKey::Pane(pane) => {
                self.spaces.set_pane_agent(pane, None);
                Some(AgentTracker::default())
            }
            TerminalKey::Drawer(_)
            | TerminalKey::DrawerTerminal { .. }
            | TerminalKey::Agent { .. }
            | TerminalKey::Login(_) => None,
        };
        self.terminals.running.insert(
            key.clone(),
            RunningTerminal {
                serial,
                terminal,
                output_byte_limit,
                released: false,
                content: 0,
                tracker,
                foreground_group: None,
                activity_recorded_at: None,
                redraw_until: None,
                folder: None,
            },
        );
        self.terminal_changed(key);
        self.schedule_agent_detection(detect::TICK_NO_AGENT);
        Ok(())
    }

    /// Passes a terminal's events from its run `serial` on to the server.
    fn terminal_events(
        &self,
        key: &TerminalKey,
        serial: u64,
    ) -> Arc<dyn Fn(AlacEvent) + Send + Sync> {
        let inputs = self.inputs.clone();
        let key = key.clone();
        Arc::new(move |event| {
            inputs
                .unbounded_send(Input::Terminal {
                    key: key.clone(),
                    serial,
                    event,
                })
                .ok();
        })
    }

    /// Stops reading the terminals to hand them to a newer server: each one whose process
    /// runs, with its PTY. An agent's terminals go with it, or end with it when it isn't handed
    /// over.
    #[cfg(unix)]
    pub(super) fn pause_terminals(
        &mut self,
        handed_threads: &collections::HashSet<projects::ThreadId>,
    ) -> Vec<(crate::handoff::HandedOffTerminal, std::os::fd::OwnedFd)> {
        let mut paused = Vec::new();
        for (key, running) in &mut self.terminals.running {
            // A login is quick to start again, and the new server wouldn't know its method.
            let ends_with_agent = matches!(
                key,
                TerminalKey::Agent { thread_id, .. } if !handed_threads.contains(thread_id)
            ) || matches!(key, TerminalKey::Login(_));
            if ends_with_agent || running.terminal.exit().is_some() {
                continue;
            }
            let pty = match running.terminal.pty().try_clone_to_owned() {
                Ok(pty) => pty,
                Err(error) => {
                    log::error!("can't hand off a terminal: {error}");
                    continue;
                }
            };
            match running.terminal.pause() {
                Ok(terminal) => paused.push((
                    crate::handoff::HandedOffTerminal {
                        key: key.clone(),
                        terminal,
                        output_byte_limit: running.output_byte_limit,
                        released: running.released,
                        folder: running.folder.clone(),
                    },
                    pty,
                )),
                Err(error) => log::error!("can't hand off a terminal: {error:#}"),
            }
        }
        paused
    }

    /// The handoff failed: the terminals go on here.
    pub(super) fn resume_terminals(&mut self, keys: &[TerminalKey]) {
        for key in keys {
            if let Some(running) = self.terminals.running.get_mut(key) {
                running.terminal.resume();
            }
        }
    }

    /// The newer server runs the terminals now; their processes are left running.
    pub(super) fn detach_terminals(&mut self, keys: &[TerminalKey]) {
        for key in keys {
            if let Some(running) = self.terminals.running.remove(key) {
                running.terminal.detach();
            }
        }
    }

    /// Takes over the terminals the server before handed over.
    #[cfg(unix)]
    pub(super) fn adopt_terminals(
        &mut self,
        terminals: Vec<crate::handoff::HandedOffTerminal>,
        palette: Option<Vec<[u8; 3]>>,
        ptys: Vec<std::os::fd::OwnedFd>,
    ) {
        self.terminals.palette = palette;
        for (handed, pty) in terminals.into_iter().zip(ptys) {
            let key = handed.key;
            let serial = self.terminals.next_serial;
            self.terminals.next_serial += 1;
            let terminal =
                match Terminal::adopt(handed.terminal, pty, self.terminal_events(&key, serial)) {
                    Ok(terminal) => terminal,
                    Err(error) => {
                        log::error!("failed to take over a terminal: {error:#}");
                        continue;
                    }
                };
            let tracker = match key {
                TerminalKey::Thread(_) | TerminalKey::Pane(_) => Some(AgentTracker::default()),
                TerminalKey::Drawer(_)
                | TerminalKey::DrawerTerminal { .. }
                | TerminalKey::Agent { .. }
                | TerminalKey::Login(_) => None,
            };
            self.terminals.running.insert(
                key.clone(),
                RunningTerminal {
                    serial,
                    terminal,
                    output_byte_limit: handed.output_byte_limit,
                    released: handed.released,
                    content: 0,
                    tracker,
                    foreground_group: None,
                    activity_recorded_at: None,
                    redraw_until: None,
                    folder: handed.folder,
                },
            );
            self.terminal_changed(key);
        }
        self.schedule_agent_detection(detect::TICK_NO_AGENT);
    }

    pub(super) fn close_terminal(&mut self, key: &TerminalKey) {
        if self.terminals.running.remove(key).is_none() {
            return;
        }
        match key {
            TerminalKey::Thread(thread_id) => {
                self.publish_terminal_agent(*thread_id, None, AgentState::Unknown);
                self.projects.set_terminal_command(*thread_id, None);
                self.projects.set_terminal_folder(*thread_id, None);
            }
            TerminalKey::Pane(pane) => self.spaces.set_pane_agent(*pane, None),
            TerminalKey::Drawer(thread_id) => self.projects.set_drawer_command(*thread_id, 1, None),
            TerminalKey::DrawerTerminal { thread_id, number } => {
                self.projects.set_drawer_command(*thread_id, *number, None)
            }
            TerminalKey::Agent { .. } => {}
            TerminalKey::Login(connection) => {
                self.terminals.logins.remove(connection);
            }
        }
        for client in self.clients.values_mut() {
            if client.terminals.remove(key).is_some() {
                send_to(
                    &client.outgoing,
                    ServerMessage::Event(Event::TerminalClosed(key.clone())),
                );
            }
        }
    }

    pub(super) fn terminal_event(&mut self, key: TerminalKey, serial: u64, event: AlacEvent) {
        let Some(running) = self.terminals.running.get_mut(&key) else {
            return;
        };
        if running.serial != serial {
            return;
        }
        if running.terminal.handle_event(event) {
            let now = Instant::now();
            running.content += 1;
            if let Some(tracker) = &mut running.tracker {
                tracker.content_changed(now);
            }
            // An agent CLI's activity is its turns, as an ACP agent's is: its screen also
            // changes while it only waits, as a focused prompt blinks.
            let runs_agent = running
                .tracker
                .as_ref()
                .is_some_and(|tracker| tracker.agent().is_some());
            let is_redraw = running.redraw_until.is_some_and(|until| now < until);
            let active_thread = match key {
                TerminalKey::Thread(thread_id)
                    if !runs_agent
                        && !is_redraw
                        && running.activity_recorded_at.is_none_or(|recorded| {
                            now.duration_since(recorded) >= TERMINAL_ACTIVITY_INTERVAL
                        }) =>
                {
                    running.activity_recorded_at = Some(now);
                    Some(thread_id)
                }
                _ => None,
            };
            let pane_exited = match key {
                TerminalKey::Pane(pane) => running.terminal.exit().is_some().then_some(pane),
                _ => None,
            };
            let logged_in = match key {
                TerminalKey::Login(connection)
                    if running
                        .terminal
                        .exit()
                        .is_some_and(|exit| exit.code == Some(0)) =>
                {
                    Some(connection)
                }
                _ => None,
            };
            if let Some(thread_id) = active_thread {
                self.projects.record_thread_activity(thread_id);
            }
            self.terminal_changed(key);
            // herdr closes a pane when its process ends.
            if let Some(pane) = pane_exited {
                self.close_space_pane(pane);
            }
            if let Some(connection) = logged_in {
                self.terminal_login_finished(connection);
            }
        }
    }

    fn schedule_agent_detection(&mut self, wait: Duration) {
        if self.terminals.detection_scheduled {
            return;
        }
        self.terminals.detection_scheduled = true;
        let inputs = self.inputs.clone();
        self.runtime.spawn(async move {
            tokio::time::sleep(wait).await;
            inputs
                .unbounded_send(Input::Run(Box::new(Server::detect_terminal_agents)))
                .ok();
        });
    }

    /// Reads which agent each pane's or terminal thread's terminal runs and what its screen
    /// says, as herdr's detection loop does.
    fn detect_terminal_agents(&mut self) {
        self.terminals.detection_scheduled = false;
        let now = Instant::now();
        let mut published = Vec::new();
        // Terminal threads' and drawer terminals' whose foreground changed, and the program
        // now there unless it's the shell.
        let mut foregrounds: Vec<(TerminalKey, Option<ForegroundProcess>)> = Vec::new();
        // Terminal threads whose foreground moved to another folder.
        let mut folders = Vec::new();
        // Workspace panes whose foreground moved to another folder.
        let mut moved_panes = Vec::new();
        let mut next_tick: Option<Duration> = None;
        for (key, running) in &mut self.terminals.running {
            // A drawer's terminals aren't watched for agents, only for what runs in them.
            if matches!(
                key,
                TerminalKey::Drawer(_) | TerminalKey::DrawerTerminal { .. }
            ) {
                let group = if running.terminal.exit().is_some() {
                    None
                } else {
                    running.terminal.foreground_process_group_id()
                };
                if group != running.foreground_group {
                    running.foreground_group = group;
                    foregrounds.push((key.clone(), foreground_program(group)));
                }
                if running.terminal.exit().is_none() {
                    let tick = detect::TICK_NO_AGENT;
                    next_tick = Some(next_tick.map_or(tick, |next| next.min(tick)));
                }
                continue;
            }
            let Some(tracker) = &mut running.tracker else {
                continue;
            };
            if running.terminal.exit().is_some() {
                if let Some(state) = tracker.exited() {
                    published.push((key.clone(), None, state));
                }
                if let TerminalKey::Thread(_) | TerminalKey::Pane(_) = key
                    && running.foreground_group.take().is_some()
                {
                    foregrounds.push((key.clone(), None));
                }
                continue;
            }
            let mut update = None;
            let process_group_id = running.terminal.foreground_process_group_id();
            if let TerminalKey::Thread(_) | TerminalKey::Pane(_) = key
                && process_group_id != running.foreground_group
            {
                running.foreground_group = process_group_id;
                foregrounds.push((key.clone(), foreground_program(process_group_id)));
            }
            // The group's leader is the shell, or a program it started, which works where
            // the shell is.
            if let TerminalKey::Thread(thread_id) = key
                && let Some(folder) = process_group_id.and_then(detect::process::process_cwd)
                && running.folder.as_ref() != Some(&folder)
            {
                running.folder = Some(folder.clone());
                folders.push((*thread_id, folder));
            }
            if let TerminalKey::Pane(pane) = key
                && let Some(folder) = process_group_id.and_then(detect::process::process_cwd)
                && running.folder.as_ref() != Some(&folder)
            {
                running.folder = Some(folder.clone());
                moved_panes.push((*pane, folder));
            }
            if tracker.should_probe(now, process_group_id) {
                let leader = process_group_id.and_then(detect::process::group_leader);
                let agent = leader.as_ref().and_then(Agent::of_process);
                // The shell may not be the terminal's own process (`login` starts it on
                // macOS), so it's known by name.
                let shell_in_foreground =
                    agent.is_none() && leader.as_ref().is_some_and(detect::is_shell);
                update = tracker.observe_process(
                    now,
                    ProcessObservation {
                        process_group_id,
                        shell_in_foreground,
                        agent,
                    },
                );
            }
            if let Some(agent) = tracker.agent()
                && tracker.should_scan(now, running.content)
            {
                let screen = running.terminal.detection_text();
                let detection = detect::detect(
                    agent,
                    DetectionInput {
                        screen: &screen,
                        osc_title: running.terminal.title().unwrap_or_default(),
                        osc_progress: "",
                    },
                );
                if let Some(state) = tracker.observe_screen(now, running.content, detection) {
                    update = Some(state);
                }
            }
            if let Some(state) = update {
                published.push((key.clone(), tracker.agent(), state));
            }
            let tick = tracker.next_tick();
            next_tick = Some(next_tick.map_or(tick, |next| next.min(tick)));
        }
        for (key, leader) in foregrounds {
            let program = leader
                .as_ref()
                .map(|leader| leader.argv0.clone().unwrap_or_else(|| leader.name.clone()));
            match key {
                TerminalKey::Thread(thread_id) => {
                    // A thread started with a command runs it until it exits, whatever its
                    // shell shows in front.
                    let command = self
                        .projects
                        .thread(thread_id)
                        .and_then(|thread| thread.terminal.as_ref()?.command.clone());
                    self.projects
                        .set_terminal_command(thread_id, command.or(program));
                }
                TerminalKey::Drawer(thread_id) => {
                    self.projects.set_drawer_command(thread_id, 1, program)
                }
                TerminalKey::DrawerTerminal { thread_id, number } => {
                    self.projects.set_drawer_command(thread_id, number, program)
                }
                TerminalKey::Pane(pane) => self.spaces.set_pane_program(
                    pane,
                    program,
                    leader.as_ref().and_then(command_line),
                ),
                TerminalKey::Agent { .. } | TerminalKey::Login(_) => {}
            }
        }
        for (thread_id, folder) in folders {
            self.refresh_terminal_folder(thread_id, folder.clone());
            // A shell is named after where it is; one started with a command keeps its name.
            let is_shell = self
                .projects
                .thread(thread_id)
                .and_then(|thread| thread.terminal.as_ref())
                .is_some_and(|terminal| terminal.command.is_none());
            if is_shell {
                self.projects
                    .rename_thread(thread_id, folder_title(&folder));
            }
        }
        for (pane, folder) in moved_panes {
            self.spaces.set_pane_folder(
                pane,
                Some(SpaceFolder {
                    display_path: home_relative(&folder),
                    path: folder,
                }),
            );
            self.refresh_space_of_pane(pane);
        }
        for (key, agent, state) in published {
            match key {
                TerminalKey::Thread(thread_id) => {
                    self.publish_terminal_agent(thread_id, agent, state)
                }
                TerminalKey::Pane(pane) => self.publish_pane_agent(pane, agent, state),
                TerminalKey::Drawer(_)
                | TerminalKey::DrawerTerminal { .. }
                | TerminalKey::Agent { .. }
                | TerminalKey::Login(_) => {}
            }
        }
        if let Some(next_tick) = next_tick {
            self.schedule_agent_detection(next_tick);
        }
    }

    /// A terminal thread's agent CLI, which makes it a thread rather than a shell, and its
    /// state as the thread's own: working, waiting for an answer as a permission request
    /// waits, or done once working ends.
    ///
    /// Its turns are checkpointed as an ACP thread's are, with the turns read from the screen:
    /// the baseline once the agent starts (or starts working), then each turn as working ends.
    fn publish_terminal_agent(
        &mut self,
        thread_id: ThreadId,
        agent: Option<Agent>,
        state: AgentState,
    ) {
        let name = agent.map(|agent| {
            terminal_programs::label(agent.label())
                .unwrap_or(agent.label())
                .to_string()
        });
        let started = name.is_some() && self.projects.terminal_agent(thread_id).is_none();
        self.projects.set_terminal_agent(thread_id, name);
        self.projects
            .set_thread_blocked(thread_id, state == AgentState::Blocked);
        let working = matches!(state, AgentState::Working | AgentState::Blocked);
        let was_working = self.projects.is_thread_working(thread_id);
        if started || (working && !was_working) {
            self.take_terminal_checkpoint(thread_id, TurnPoint::Starting, None);
        }
        if working {
            self.projects.set_thread_working(thread_id, true);
        } else if was_working {
            // Done once the turn's checkpoint is there, since the changes panel reloads then.
            self.take_terminal_checkpoint(
                thread_id,
                TurnPoint::Ended,
                Some(Box::new(move |server: &mut Server| {
                    if !server.is_terminal_agent_working(thread_id) {
                        server.projects.set_thread_working(thread_id, false);
                    }
                })),
            );
        }
    }

    /// Whether a terminal thread's agent CLI is working, or waiting for an answer, now.
    fn is_terminal_agent_working(&self, thread_id: ThreadId) -> bool {
        self.terminals
            .running
            .get(&TerminalKey::Thread(thread_id))
            .and_then(|running| running.tracker.as_ref()?.state())
            .is_some_and(|state| matches!(state, AgentState::Working | AgentState::Blocked))
    }

    /// The folder a terminal thread's shell is in, as last seen.
    pub(super) fn terminal_folder(&self, thread_id: ThreadId) -> Option<PathBuf> {
        self.terminals
            .running
            .get(&TerminalKey::Thread(thread_id))
            .and_then(|running| running.folder.clone())
            .or_else(|| {
                let folder = self.projects.terminal_folder(thread_id)?;
                Some(folder.path.clone())
            })
    }

    /// Queues a checkpoint of a terminal thread's folder, then runs `then` on the server. A
    /// deleted thread gets none.
    fn take_terminal_checkpoint(
        &mut self,
        thread_id: ThreadId,
        point: TurnPoint,
        then: Option<Box<dyn FnOnce(&mut Server) + Send>>,
    ) {
        let checkpoints = self
            .projects
            .thread(thread_id)
            .and_then(|_| self.checkpoints(thread_id));
        let Some(checkpoints) = checkpoints else {
            if let Some(then) = then {
                then(self);
            }
            return;
        };
        let queue = self.terminals.checkpoint_queue.get_or_insert_with(|| {
            let (queue, mut jobs) = mpsc::unbounded::<CheckpointJob>();
            let inputs = self.inputs.clone();
            self.runtime.spawn(async move {
                while let Some(job) = jobs.next().await {
                    job.checkpoints.on_turn(job.point).await;
                    if let Some(then) = job.then {
                        inputs.unbounded_send(Input::Run(then)).ok();
                    }
                }
            });
            queue
        });
        let job = CheckpointJob {
            checkpoints,
            point,
            then,
        };
        if let Err(error) = queue.unbounded_send(job)
            && let Some(then) = error.into_inner().then
        {
            then(self);
        }
    }

    /// Looks up the branch of the folder a terminal thread is in, and shows the folder with it
    /// once known. A folder that's been left by then is dropped.
    pub(super) fn refresh_terminal_folder(&mut self, thread_id: ThreadId, path: PathBuf) {
        let lookup = path.clone();
        self.spawn_then(
            async move { crate::spaces::space_git(&lookup).await },
            move |server, git| {
                let is_current = server
                    .terminals
                    .running
                    .get(&TerminalKey::Thread(thread_id))
                    .is_some_and(|running| running.folder.as_ref() == Some(&path));
                if is_current {
                    server.projects.set_terminal_folder(
                        thread_id,
                        Some(TerminalFolder {
                            display_path: Some(home_relative(&path)),
                            path,
                            is_repository: git.is_some(),
                            repository: git.as_ref().and_then(|git| git.repository.clone()),
                            branch: git.and_then(|git| git.branch),
                        }),
                    );
                }
            },
        );
    }

    pub(super) fn terminal_changed(&mut self, key: TerminalKey) {
        if !self.terminals.dirty.contains(&key) {
            self.terminals.dirty.push(key);
        }
    }

    /// Closes the terminals of threads and panes that are gone.
    /// Whether a command the thread's agent started in a terminal still runs.
    pub(super) fn has_running_agent_terminal(&self, thread_id: ThreadId) -> bool {
        self.terminals.running.iter().any(|(key, running)| {
            matches!(key, TerminalKey::Agent { thread_id: id, .. } if *id == thread_id)
                && !running.released
                && running.terminal.exit().is_none()
        })
    }

    pub(super) fn close_orphaned_terminals(&mut self) {
        let orphaned: Vec<TerminalKey> = self
            .terminals
            .running
            .keys()
            .filter(|key| match key {
                TerminalKey::Pane(pane) => self.spaces.pane(*pane).is_none(),
                TerminalKey::Login(ConnectionId::LoginSession(login_session_id)) => {
                    !self.login_sessions.contains_key(login_session_id)
                }
                key => key
                    .thread_id()
                    .is_none_or(|thread_id| self.projects.thread(thread_id).is_none()),
            })
            .cloned()
            .collect();
        for key in orphaned {
            self.close_terminal(&key);
        }
    }

    /// Sends watchers what changed on their terminals, at most once per [`FRAME_INTERVAL`].
    pub(super) fn send_terminal_frames(&mut self) {
        if self.terminals.dirty.is_empty() {
            return;
        }
        let now = Instant::now();
        if let Some(sent_at) = self.terminals.frames_sent_at {
            let since = now.duration_since(sent_at);
            if since < FRAME_INTERVAL {
                if !self.terminals.tick_scheduled {
                    self.terminals.tick_scheduled = true;
                    let inputs = self.inputs.clone();
                    let wait = FRAME_INTERVAL - since;
                    self.runtime.spawn(async move {
                        tokio::time::sleep(wait).await;
                        inputs
                            .unbounded_send(Input::Run(Box::new(|server| {
                                server.terminals.tick_scheduled = false;
                            })))
                            .ok();
                    });
                }
                return;
            }
        }
        self.terminals.frames_sent_at = Some(now);
        for key in std::mem::take(&mut self.terminals.dirty) {
            let Some(running) = self.terminals.running.get(&key) else {
                continue;
            };
            running.terminal.take_wakeup();
            if !self
                .clients
                .values()
                .any(|client| client.terminals.contains_key(&key))
            {
                continue;
            }
            let frame = running.terminal.frame();
            for client in self.clients.values_mut() {
                let Some(sent) = client.terminals.get_mut(&key) else {
                    continue;
                };
                if let Some(changes) = frame_changes(sent, &frame) {
                    *sent = frame.clone();
                    send_to(
                        &client.outgoing,
                        ServerMessage::Event(Event::TerminalFrame {
                            terminal: key.clone(),
                            frame: changes,
                        }),
                    );
                }
            }
        }
    }
}

impl Server {
    /// Runs one of a connection's terminal login methods, replacing a login already running.
    /// It runs here rather than on the client's machine because the agent keeps its login
    /// where it runs.
    pub(super) fn start_terminal_login(
        &mut self,
        connection: ConnectionId,
        method_id: acp::AuthMethodId,
    ) -> Result<()> {
        let thread = match connection {
            ConnectionId::Thread(thread_id) => self.threads.get(&thread_id),
            ConnectionId::LoginSession(login_session_id) => self
                .login_sessions
                .get(&login_session_id)
                .map(|login_session| &login_session.thread),
        }
        .context("the agent isn't running")?;
        let command = thread
            .terminal_auth_command(&method_id)
            .context("the agent has no such terminal login")?;
        let mut env: HashMap<String, String> = command.env.into_iter().collect();
        let path = env
            .get("PATH")
            .cloned()
            .or_else(|| std::env::var("PATH").ok());
        if let Some(directory) = &self.browser_programs {
            env.extend(browser::agent_env(directory, connection, path.as_deref()));
        }
        let removed = command
            .env_remove
            .into_iter()
            .filter(|variable| !env.contains_key(variable))
            .collect();
        let program = without_variables(
            command.path.to_string_lossy().into_owned(),
            command.args,
            removed,
        );
        let spawn = TerminalSpawn {
            program: Some(program),
            cwd: thread.state.cwd.clone(),
            env,
        };
        self.update_thread(connection, AgentThread::terminal_login_started)?;
        self.terminals.logins.insert(connection, method_id);
        self.start_terminal(
            TerminalKey::Login(connection),
            spawn,
            TerminalSize::default(),
        )
    }

    /// Whether the connection's login terminal still runs its login.
    pub(super) fn terminal_login_runs(&self, connection: ConnectionId) -> bool {
        self.terminals
            .running
            .get(&TerminalKey::Login(connection))
            .is_some_and(|running| running.terminal.exit().is_none())
    }

    /// A login terminal exited successfully: it closes, and its agent restarts logged in.
    fn terminal_login_finished(&mut self, connection: ConnectionId) {
        let Some(method_id) = self.terminals.logins.get(&connection).cloned() else {
            return;
        };
        self.close_terminal(&TerminalKey::Login(connection));
        self.update_thread(connection, |thread| {
            thread.terminal_login_finished(&method_id)
        })
        .log_err();
    }

    /// Answers an agent's `terminal/*` request (ACP's client terminals, as Zed runs them).
    pub(super) fn agent_terminal_request(&mut self, thread_id: ThreadId, request: TerminalRequest) {
        match request {
            TerminalRequest::Create(request, responder) => {
                let result = self.create_agent_terminal(thread_id, request);
                match result {
                    Ok(terminal_id) => {
                        responder.respond(acp::CreateTerminalResponse::new(terminal_id))
                    }
                    Err(error) => responder.respond_with_internal_error(format!("{error:#}")),
                }
                .log_err();
            }
            TerminalRequest::Output(request, responder) => {
                match self.agent_terminal(thread_id, &request.terminal_id) {
                    Ok(running) => {
                        let (output, truncated) =
                            truncate_start(running.terminal.text(), running.output_byte_limit);
                        responder.respond(
                            acp::TerminalOutputResponse::new(output, truncated)
                                .exit_status(running.terminal.exit().map(acp_exit_status)),
                        )
                    }
                    Err(error) => responder.respond_with_internal_error(format!("{error:#}")),
                }
                .log_err();
            }
            TerminalRequest::WaitForExit(request, responder) => {
                let key = agent_terminal_key(thread_id, &request.terminal_id);
                let exit = match self.agent_terminal(thread_id, &request.terminal_id) {
                    Ok(_) => self
                        .terminals
                        .running
                        .get_mut(&key)
                        .map(|running| running.terminal.wait_for_exit()),
                    Err(error) => {
                        responder
                            .respond_with_internal_error(format!("{error:#}"))
                            .log_err();
                        return;
                    }
                };
                let Some(exit) = exit else {
                    return;
                };
                self.runtime.spawn(async move {
                    match exit.await {
                        Ok(exit) => responder.respond(acp::WaitForTerminalExitResponse::new(
                            acp_exit_status(&exit),
                        )),
                        Err(_) => responder.respond_with_internal_error("the terminal was closed"),
                    }
                    .log_err();
                });
            }
            TerminalRequest::Kill(request, responder) => {
                let key = agent_terminal_key(thread_id, &request.terminal_id);
                match self.terminals.running.get_mut(&key) {
                    Some(running) if !running.released => {
                        running.terminal.kill();
                        self.terminal_changed(key);
                        responder.respond(acp::KillTerminalResponse::new())
                    }
                    _ => responder.respond_with_internal_error("no such terminal"),
                }
                .log_err();
            }
            TerminalRequest::Release(request, responder) => {
                let key = agent_terminal_key(thread_id, &request.terminal_id);
                match self.terminals.running.get_mut(&key) {
                    Some(running) if !running.released => {
                        running.terminal.kill();
                        running.released = true;
                        self.terminal_changed(key);
                        responder.respond(acp::ReleaseTerminalResponse::new())
                    }
                    _ => responder.respond_with_internal_error("no such terminal"),
                }
                .log_err();
            }
        }
    }

    fn agent_terminal(
        &self,
        thread_id: ThreadId,
        terminal_id: &acp::TerminalId,
    ) -> Result<&RunningTerminal> {
        self.terminals
            .running
            .get(&agent_terminal_key(thread_id, terminal_id))
            .filter(|running| !running.released)
            .context("no such terminal")
    }

    fn create_agent_terminal(
        &mut self,
        thread_id: ThreadId,
        request: acp::CreateTerminalRequest,
    ) -> Result<String> {
        let folder = self
            .projects
            .thread_folder(thread_id)
            .context("no such thread")?;
        let cwd = match request.cwd {
            Some(cwd) if cwd.is_absolute() => cwd,
            Some(cwd) => folder.join(cwd),
            None => folder,
        };
        let mut env = self.terminal_env(Some(thread_id));
        // Nothing waits at a pager for a key the agent can't press (Zed).
        env.insert("PAGER".into(), String::new());
        env.insert("GIT_PAGER".into(), "cat".into());
        env.extend(
            request
                .env
                .into_iter()
                .map(|variable| (variable.name, variable.value)),
        );
        let shell = self
            .terminal_shell
            .clone()
            .unwrap_or_else(util::shell::get_default_system_shell_preferring_bash);
        let (program, args) = ShellBuilder::new(&Shell::Program(shell), false)
            .non_interactive()
            .redirect_stdin_to_dev_null()
            .build(Some(request.command), &request.args);
        let terminal_id = uuid::Uuid::new_v4().to_string();
        let key = agent_terminal_key(thread_id, &acp::TerminalId::new(terminal_id.clone()));
        self.start_terminal(
            key.clone(),
            TerminalSpawn {
                program: Some((program, args)),
                cwd,
                env,
            },
            TerminalSize::default(),
        )?;
        if let Some(running) = self.terminals.running.get_mut(&key) {
            running.output_byte_limit = request
                .output_byte_limit
                .map(|limit| usize::try_from(limit).unwrap_or(usize::MAX));
        }
        Ok(terminal_id)
    }
}

fn agent_terminal_key(thread_id: ThreadId, terminal_id: &acp::TerminalId) -> TerminalKey {
    TerminalKey::Agent {
        thread_id,
        terminal_id: terminal_id.0.to_string(),
    }
}

fn acp_exit_status(exit: &TerminalExit) -> acp::TerminalExitStatus {
    acp::TerminalExitStatus::new()
        .exit_code(exit.code)
        .signal(exit.signal.clone())
}

/// ACP keeps the end of the output: past the limit, the start is dropped, at a line break
/// when there's one.
fn truncate_start(text: String, limit: Option<usize>) -> (String, bool) {
    let Some(limit) = limit.filter(|limit| text.len() > *limit) else {
        return (text, false);
    };
    let mut start = text.len() - limit;
    while !text.is_char_boundary(start) {
        start += 1;
    }
    let at_line_start = text.as_bytes().get(start.wrapping_sub(1)) == Some(&b'\n');
    if !at_line_start && let Some(line_end) = text[start..].find('\n') {
        start += line_end + 1;
    }
    (text[start..].to_string(), true)
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{ForegroundProcess, command_line, home_relative, truncate_start};

    #[test]
    fn a_command_line_runs_what_the_arguments_say() {
        let line = |argv: &[&str]| {
            command_line(&ForegroundProcess::new(
                "node".into(),
                argv.iter().map(|word| word.to_string()).collect(),
            ))
        };
        assert_eq!(
            line(&["python3", "-m", "http.server", "a b", "it's"]).as_deref(),
            Some("python3 -m http.server 'a b' \"it's\"")
        );
        // Node's `process.title` overwrites the arguments' memory.
        assert_eq!(
            line(&["npm run dev", "", ""]).as_deref(),
            Some("npm run dev")
        );
        assert_eq!(line(&[]), None);
    }

    #[test]
    fn paths_under_home_start_with_a_tilde() {
        let home = util::paths::home_dir();
        assert_eq!(home_relative(home.as_path()), "~");
        assert_eq!(home_relative(&home.join("projects/api")), "~/projects/api");
        assert_eq!(home_relative(Path::new("/tmp")), "/tmp");
    }

    #[test]
    fn output_is_cut_from_the_start() {
        assert_eq!(truncate_start("abc".into(), None), ("abc".into(), false));
        assert_eq!(truncate_start("abc".into(), Some(3)), ("abc".into(), false));
        assert_eq!(
            truncate_start("one\ntwo\nthree".into(), Some(8)),
            ("three".into(), true)
        );
        assert_eq!(
            truncate_start("one\ntwo\nthree".into(), Some(9)),
            ("two\nthree".into(), true)
        );
        assert_eq!(
            truncate_start("héllo".into(), Some(4)),
            ("llo".into(), true)
        );
    }
}

/// A shell's name for its folder: `~` for home, else the folder's own name.
fn folder_title(folder: &Path) -> String {
    if folder == util::paths::home_dir().as_path() {
        return "~".to_string();
    }
    folder.file_name().map_or_else(
        || folder.display().to_string(),
        |name| name.to_string_lossy().into_owned(),
    )
}

/// The path with home written as `~`.
pub(super) fn home_relative(path: &Path) -> String {
    let home = util::paths::home_dir();
    match path.strip_prefix(home.as_path()) {
        Ok(rest) if rest.as_os_str().is_empty() => "~".to_string(),
        Ok(rest) => format!("~/{}", rest.display()),
        Err(_) => path.display().to_string(),
    }
}

/// The program leading a terminal's foreground, unless that's the shell itself.
fn foreground_program(group: Option<u32>) -> Option<ForegroundProcess> {
    group
        .and_then(detect::process::group_leader)
        .filter(|leader| !detect::is_shell(leader))
}

/// A program's command line, to run it again: its arguments, quoted for the shell, after
/// the first as it is. That one may be a title the program gave itself, as npm's Node sets
/// `process.title` to "npm run dev", which clears the arguments after it.
fn command_line(leader: &ForegroundProcess) -> Option<String> {
    let mut words = leader.argv.iter().flatten().filter(|word| !word.is_empty());
    let mut line = words.next()?.clone();
    for word in words {
        line.push(' ');
        match util::shell::ShellKind::Posix.try_quote(word) {
            Some(quoted) => line.push_str(&quoted),
            None => line.push_str(word),
        }
    }
    Some(line)
}
