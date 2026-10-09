//! The terminal tools, after herdr's `pane` and `agent` commands: an agent starts terminal
//! threads and the Workspaces view's workspaces, tabs and panes, types text and keys into them
//! and into agent threads' terminal drawers, reads their screens, and waits for output, as a
//! user at the terminal would.
//!
//! The Workspaces view belongs to no project, so its panes are reached on any machine, the
//! project there or not: that's how an agent clones a repository on another machine before
//! adding it as a project with `agentz_project_add`.

use std::path::{Path, PathBuf};
use std::time::Duration;

use agentz_protocol::Response;
use agentz_protocol::layout::{Direction, PaneId};
use agentz_protocol::spaces::{
    PaneAgentState, PaneContent, PaneLocation, PaneTerminal, SpaceId, SpaceRequest,
};
use agentz_protocol::terminal::{TerminalCommand, TerminalInput, TerminalKey};
use agentz_protocol::terminal_keys::{Keystroke, key_bytes};
use projects::ThreadId;
use serde_json::{Value, json};

use super::workspaces::{Folder, Placement, workspace_strategy};
use super::{Arguments, Caller, Failure, MAX_WAIT, Outcome, Server, Step, failure, invalid};
use crate::server::workspace_requests::NewThread;

const MAX_COMMAND_CHARS: usize = 8_000;
const MAX_FOLDER_CHARS: usize = 4_096;
const MAX_TEXT_CHARS: usize = 100_000;
const MAX_KEYS: usize = 100;
const MAX_MATCH_CHARS: usize = 1_000;
const DEFAULT_LINES: u64 = 100;
const MAX_LINES: u64 = 5_000;
const DEFAULT_TERMINAL_WAIT: Duration = Duration::from_secs(60);

/// What the read and wait tools return.
enum Source {
    /// The screen as it's shown.
    Visible,
    /// The last lines of the output, scrollback included.
    Recent(usize),
}

/// Where `agentz_terminal_start` opens a pane of the Workspaces view.
enum PanePlace {
    /// A new workspace at a folder.
    Workspace(PathBuf),
    /// A new tab in a workspace.
    Tab(SpaceId),
    /// Beside a pane, splitting it.
    Split(PaneId, Direction),
}

impl Server {
    pub(super) fn terminal_list(&self, caller: Caller) -> Outcome {
        let mut terminals: Vec<Value> = match caller.project_id {
            Some(project_id) => self
                .projects
                .threads()
                .iter()
                .filter(|thread| thread.project_id == project_id)
                .filter_map(|thread| {
                    let key = if thread.terminal.is_some() {
                        TerminalKey::Thread(thread.id)
                    } else {
                        // Agent threads' drawers count once something runs in them.
                        let key = TerminalKey::Drawer(thread.id);
                        self.terminals.running.contains_key(&key).then_some(key)?
                    };
                    Some(self.terminal_summary(&key))
                })
                .collect(),
            None => Vec::new(),
        };
        // A pane showing a thread is that thread's, listed above when it's this project's.
        let panes: Vec<PaneId> = self
            .spaces
            .panes()
            .filter(|(_, pane)| matches!(pane.content, PaneContent::Terminal(_)))
            .map(|(_, pane)| pane.id)
            .collect();
        terminals.extend(
            panes
                .into_iter()
                .map(|pane| self.terminal_summary(&TerminalKey::Pane(pane))),
        );
        Ok(Step::Done(json!({ "terminals": terminals })))
    }

    pub(super) fn terminal_start(&mut self, caller: Caller, arguments: &Arguments) -> Outcome {
        let command = arguments
            .string("command", MAX_COMMAND_CHARS)?
            .map(String::from);
        let placement = workspace_strategy(arguments)?;
        if let Some(place) = pane_place(arguments)? {
            if !matches!(placement, Placement::Default) {
                return Err(invalid(
                    "workspaceStrategy is for terminal threads; a Workspaces pane works in the \
                     folder it's given.",
                ));
            }
            return self.start_pane_terminal(place, command);
        }
        let project_id = caller.project()?;
        self.with_folders(caller, vec![placement], move |server, folders| {
            let folder = match folders.into_iter().next() {
                Some(Folder::Chosen(folder)) => folder,
                // Where the caller works, so what it runs sees its changes.
                Some(Folder::Default) | None => caller
                    .thread_id
                    .and_then(|thread_id| server.projects.thread(thread_id))
                    .and_then(|thread| thread.workspace.clone()),
            };
            let thread_id = server
                .create_thread_in(
                    project_id,
                    NewThread::Terminal(TerminalCommand { command }),
                    folder,
                )
                .map_err(|error| failure("orchestration_error", format!("{error:#}")))?;
            server
                .projects
                .set_thread_creator(thread_id, caller.creator());
            Ok(Step::Done(
                server.terminal_summary(&TerminalKey::Thread(thread_id)),
            ))
        })
    }

    /// Opens the pane as the user would from the Workspaces view, which shows it without
    /// switching to it.
    fn start_pane_terminal(&mut self, place: PanePlace, command: Option<String>) -> Outcome {
        let content = PaneContent::Terminal(PaneTerminal {
            // The workspace's folder.
            folder: PathBuf::new(),
            command,
        });
        let request = match place {
            PanePlace::Workspace(folder) => SpaceRequest::CreateSpace {
                folder,
                project_id: None,
                content,
            },
            PanePlace::Tab(space) => SpaceRequest::CreateTab { space, content },
            PanePlace::Split(pane, direction) => SpaceRequest::SplitPane {
                pane,
                direction,
                content,
            },
        };
        let location = match self.space_request(request) {
            Ok(Response::SpacePane(location)) => location,
            Ok(response) => {
                return Err(failure(
                    "orchestration_error",
                    format!("unexpected response: {response:?}"),
                ));
            }
            Err(error) => return Err(invalid(format!("{error:#}"))),
        };
        let PaneLocation { pane, .. } = location;
        Ok(Step::Done(self.terminal_summary(&TerminalKey::Pane(pane))))
    }

    pub(super) fn terminal_send(&mut self, caller: Caller, arguments: &Arguments) -> Outcome {
        let key = self.terminal_target(caller, arguments)?;
        let text = arguments
            .0
            .get("text")
            .map(|text| match text {
                Value::String(text) if text.chars().count() <= MAX_TEXT_CHARS => Ok(text.clone()),
                Value::String(_) => Err(invalid(format!(
                    "text is over {MAX_TEXT_CHARS} characters long."
                ))),
                _ => Err(invalid("text must be a string.")),
            })
            .transpose()?
            .filter(|text| !text.is_empty());
        let keys = match arguments.array("keys")? {
            Some(keys) if keys.len() > MAX_KEYS => {
                return Err(invalid(format!("keys has over {MAX_KEYS} entries.")));
            }
            Some(keys) => keys
                .iter()
                .map(|key| key.as_str().ok_or_else(|| invalid("keys must be strings.")))
                .collect::<Result<Vec<_>, _>>()?,
            None => Vec::new(),
        };
        let submit = arguments.bool("submit")?.unwrap_or(false);
        if text.is_none() && keys.is_empty() && !submit {
            return Err(invalid("Send text, keys or submit."));
        }

        let running = self
            .terminals
            .running
            .get_mut(&key)
            .ok_or_else(|| failure("orchestration_error", "The terminal isn't running."))?;
        if let Some(exit) = running.terminal.exit() {
            return Err(failure(
                "terminal_exited",
                format!(
                    "The terminal's process has exited{}, so it can't take input.",
                    exit.code
                        .map(|code| format!(" with code {code}"))
                        .unwrap_or_default()
                ),
            ));
        }
        // All keys are checked before anything is typed.
        let modes = running.terminal.modes();
        let mut key_input = Vec::new();
        for name in keys.iter().copied().chain(submit.then_some("enter")) {
            let bytes = Keystroke::parse(name)
                .and_then(|keystroke| key_bytes(&keystroke, modes, true))
                .ok_or_else(|| {
                    invalid(format!(
                        "Unknown key {name}. Keys look like enter, tab, escape, backspace, up, ctrl-c, alt-b, shift-tab, f5 or a single character."
                    ))
                })?;
            key_input.extend(bytes);
        }
        if let Some(text) = text {
            running.terminal.input(TerminalInput::Paste(text));
        }
        if !key_input.is_empty() {
            running.terminal.input(TerminalInput::Bytes(key_input));
        }
        self.terminal_changed(key.clone());
        Ok(Step::Done(self.terminal_summary(&key)))
    }

    pub(super) fn terminal_read(&mut self, caller: Caller, arguments: &Arguments) -> Outcome {
        let key = self.terminal_target(caller, arguments)?;
        let source = source(arguments)?;
        let text = self.terminal_text(&key, &source);
        let mut result = self.terminal_summary(&key);
        result["text"] = json!(text);
        Ok(Step::Done(result))
    }

    /// herdr's `wait_for_output`: until the text shows up, the process exits, or time is up.
    pub(super) fn terminal_wait(
        &mut self,
        caller: Caller,
        arguments: &Arguments,
        timed_out: bool,
    ) -> Outcome {
        // A pane closes as its process ends, which is what the wait was for.
        if let Some(pane) = arguments.number("paneId")?
            && self.spaces.pane(PaneId(pane)).is_none()
        {
            let mut result = self.pane_summary(PaneId(pane));
            result["text"] = json!("");
            result["matched"] = json!(false);
            result["timedOut"] = json!(false);
            return Ok(Step::Done(result));
        }
        let key = self.terminal_target(caller, arguments)?;
        let source = source(arguments)?;
        let pattern = arguments.string("match", MAX_MATCH_CHARS)?;
        let timeout = arguments
            .number("timeoutMs")?
            .map_or(DEFAULT_TERMINAL_WAIT, Duration::from_millis)
            .min(MAX_WAIT);
        let text = self.terminal_text(&key, &source);
        let matched = pattern.is_some_and(|pattern| text.contains(pattern));
        let exited = self
            .terminals
            .running
            .get(&key)
            .is_none_or(|running| running.terminal.exit().is_some());
        if !matched && !exited && !timed_out {
            return Ok(Step::Wait(timeout));
        }
        let mut result = self.terminal_summary(&key);
        result["text"] = json!(text);
        result["matched"] = json!(matched);
        result["timedOut"] = json!(!matched && !exited);
        Ok(Step::Done(result))
    }

    /// The terminal the call names, started if it isn't running: a Workspaces pane's, a
    /// terminal thread's own, or an agent thread's drawer. Without `threadId` or `paneId`, the
    /// caller's.
    fn terminal_target(
        &mut self,
        caller: Caller,
        arguments: &Arguments,
    ) -> Result<TerminalKey, Failure> {
        if let Some(pane) = arguments.number("paneId")? {
            if arguments.0.contains_key("threadId") {
                return Err(invalid("Pass threadId or paneId, not both."));
            }
            let pane = PaneId(pane);
            return match self.spaces.pane(pane).map(|pane| &pane.content) {
                Some(PaneContent::Terminal(_)) => Ok(TerminalKey::Pane(pane)),
                Some(_) => Err(invalid(format!(
                    "Pane {} shows a thread; pass its threadId instead.",
                    pane.0
                ))),
                // herdr closes a pane once its process ends.
                None => Err(failure(
                    "terminal_exited",
                    format!(
                        "Pane {} isn't open: its process ended, or it was closed.",
                        pane.0
                    ),
                )),
            };
        }
        let thread_id = arguments
            .thread_id("threadId")?
            .or(caller.thread_id)
            .ok_or_else(|| invalid("threadId is required outside a thread."))?;
        let thread_id = self.target(caller, Some(thread_id))?;
        let key = terminal_key(
            thread_id,
            self.projects
                .thread(thread_id)
                .is_some_and(|thread| thread.terminal.is_some()),
        );
        self.ensure_terminal(&key)
            .map_err(|error| failure("orchestration_error", format!("{error:#}")))?;
        Ok(key)
    }

    fn terminal_text(&self, key: &TerminalKey, source: &Source) -> String {
        let Some(running) = self.terminals.running.get(key) else {
            return String::new();
        };
        match source {
            Source::Visible => running.terminal.screen_text(),
            Source::Recent(lines) => running.terminal.recent_text(*lines),
        }
    }

    fn terminal_summary(&self, key: &TerminalKey) -> Value {
        if let TerminalKey::Pane(pane) = key {
            return self.pane_summary(*pane);
        }
        let thread_id = key.thread_id();
        let thread = thread_id.and_then(|thread_id| self.projects.thread(thread_id));
        let running = self.terminals.running.get(key);
        let exit = running.and_then(|running| running.terminal.exit());
        let status = match (running, exit) {
            (None, _) => "not_started",
            (Some(_), None) => "running",
            (Some(_), Some(_)) => "exited",
        };
        json!({
            "threadId": thread_id.map(|thread_id| thread_id.0),
            "kind": match key {
                TerminalKey::Thread(_) => "terminal_thread",
                TerminalKey::Drawer(_) | TerminalKey::DrawerTerminal { .. } => "drawer",
                TerminalKey::Agent { .. } => "agent_command",
                TerminalKey::Pane(_) => "pane",
                TerminalKey::Login(_) => "login",
            },
            "threadTitle": thread.map(|thread| thread.title.clone()),
            "command": thread
                .and_then(|thread| thread.terminal.as_ref())
                .and_then(|terminal| terminal.command.clone()),
            "folder": thread_id.and_then(|thread_id| self.projects.thread_folder(thread_id)),
            "status": status,
            "exitCode": exit.and_then(|exit| exit.code),
            "signal": exit.and_then(|exit| exit.signal.clone()),
            "windowTitle": running.and_then(|running| running.terminal.title()),
        })
    }

    /// A Workspaces pane's terminal, with where it is: its workspace and tab, by the names
    /// the Workspaces view shows.
    fn pane_summary(&self, pane_id: PaneId) -> Value {
        let key = TerminalKey::Pane(pane_id);
        let running = self.terminals.running.get(&key);
        let exit = running.and_then(|running| running.terminal.exit());
        let status = match (running, exit) {
            (None, _) => "not_started",
            (Some(_), None) => "running",
            (Some(_), Some(_)) => "exited",
        };
        let place = self.spaces.spaces().iter().find_map(|space| {
            space.tabs.iter().enumerate().find_map(|(index, tab)| {
                let pane = tab.pane(pane_id)?;
                let tab_name = tab
                    .name
                    .clone()
                    .unwrap_or_else(|| format!("Tab {}", index + 1));
                Some((space, tab_name, pane))
            })
        });
        let Some((space, tab_name, pane)) = place else {
            return json!({"paneId": pane_id.0, "kind": "pane", "status": "closed"});
        };
        let command = match &pane.content {
            PaneContent::Terminal(terminal) => terminal.command.clone(),
            _ => None,
        };
        let folder = match (&pane.folder, &pane.content) {
            (Some(folder), _) => Some(folder.path.clone()),
            (None, PaneContent::Terminal(terminal)) => Some(terminal.folder.clone()),
            (None, _) => None,
        };
        json!({
            "paneId": pane_id.0,
            "kind": "pane",
            "workspaceId": space.id.0,
            "workspace": space.label(),
            "tab": tab_name,
            "command": command,
            "folder": folder,
            "program": pane.program,
            "agent": pane.agent.as_ref().map(|agent| json!({
                "name": agent.name,
                "state": match agent.state {
                    PaneAgentState::Idle => "idle",
                    PaneAgentState::Working => "working",
                    PaneAgentState::Blocked => "blocked",
                    PaneAgentState::Unknown => "unknown",
                },
            })),
            "status": status,
            "exitCode": exit.and_then(|exit| exit.code),
            "signal": exit.and_then(|exit| exit.signal.clone()),
            "windowTitle": running.and_then(|running| running.terminal.title()),
        })
    }
}

/// Where the call asks for a Workspaces pane, if it does.
fn pane_place(arguments: &Arguments) -> Result<Option<PanePlace>, Failure> {
    let folder = arguments.string("folder", MAX_FOLDER_CHARS)?;
    let space = arguments.number("workspaceId")?;
    let pane = arguments.number("paneId")?;
    let direction = match arguments.string("direction", 8)? {
        None | Some("right") => Direction::Horizontal,
        Some("down") => Direction::Vertical,
        Some(direction) => {
            return Err(invalid(format!(
                "Unknown direction {direction}. Directions: right, down."
            )));
        }
    };
    if arguments.0.contains_key("direction") && pane.is_none() {
        return Err(invalid("direction is for splitting a pane, with paneId."));
    }
    Ok(match (folder, space, pane) {
        (None, None, None) => None,
        (Some(folder), None, None) => Some(PanePlace::Workspace(Path::new(folder).to_path_buf())),
        (None, Some(space), None) => Some(PanePlace::Tab(SpaceId(space))),
        (None, None, Some(pane)) => Some(PanePlace::Split(PaneId(pane), direction)),
        _ => {
            return Err(invalid(
                "Pass at most one of folder, workspaceId and paneId.",
            ));
        }
    })
}

fn terminal_key(thread_id: ThreadId, is_terminal_thread: bool) -> TerminalKey {
    if is_terminal_thread {
        TerminalKey::Thread(thread_id)
    } else {
        TerminalKey::Drawer(thread_id)
    }
}

fn source(arguments: &Arguments) -> Result<Source, Failure> {
    let lines = arguments
        .number("lines")?
        .unwrap_or(DEFAULT_LINES)
        .clamp(1, MAX_LINES) as usize;
    match arguments.string("source", 16)? {
        None | Some("recent") => Ok(Source::Recent(lines)),
        Some("visible") => Ok(Source::Visible),
        Some(source) => Err(invalid(format!(
            "Unknown source {source}. Sources: visible, recent."
        ))),
    }
}

pub(super) fn definitions() -> Vec<Value> {
    let thread_id = json!({
        "type": "integer",
        "description": "A terminal thread from agentz_terminal_list or agentz_terminal_start, or an agent thread, whose terminal drawer is used. Defaults to this thread: a terminal thread's own terminal, or an agent thread's drawer.",
    });
    let source = json!({
        "type": "string",
        "enum": ["recent", "visible"],
        "description": "recent (the default) is the last lines of output, scrollback included; visible is the screen as the user sees it.",
    });
    let lines = json!({"type": "integer", "minimum": 1, "maximum": MAX_LINES, "description": "How many lines recent returns. Defaults to 100."});
    let pane_id = json!({
        "type": "integer",
        "description": "A Workspaces pane from agentz_terminal_list or agentz_terminal_start, in place of threadId.",
    });
    vec![
        json!({
            "name": "agentz_terminal_list",
            "title": "List agentZ terminals",
            "description": "List the terminals in the calling thread's project: terminal threads, which run a shell or a command such as an agent CLI, and the terminal drawers of agent threads that have one running. Then the terminal panes of the machine's Workspaces view (kind pane, with paneId, workspaceId, the workspace's and tab's names, and the agent CLI detected in it), which belong to no project, so they're listed on a machine without this project too. status is not_started, running or exited, with the exit code or signal once exited.",
            "inputSchema": {"type": "object", "properties": {}, "additionalProperties": false},
            "annotations": {"readOnlyHint": true, "destructiveHint": false, "idempotentHint": true},
        }),
        json!({
            "name": "agentz_terminal_start",
            "title": "Start an agentZ terminal",
            "description": "Start a terminal thread in this project that the user can see and type into, for long-running work such as dev servers, watchers or another agent's CLI. command runs in the user's login shell and the terminal ends with it; without it the terminal runs an interactive shell. It works in this thread's folder unless workspaceStrategy says otherwise. With folder, workspaceId or paneId it opens a pane of the Workspaces view instead, which needs no project: a new workspace at folder, a new tab in a workspace, or a split beside a pane. Use that for work outside this project, such as cloning a repository on another machine before adding it with agentz_project_add. A pane closes when its process ends, so run one-off commands in its shell rather than as command. Use agentz_terminal_send to type into it and agentz_terminal_read or agentz_terminal_wait to see its output. For a one-off command whose output you need, use your own command tool on this machine and agentz_command_run on another.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "command": {"type": "string", "maxLength": MAX_COMMAND_CHARS},
                    "workspaceStrategy": {
                        "type": "object",
                        "description": "Where the terminal works: type=root for the project's checkout, existing with a path from agentz_workspace_list, or worktree or pasture for a new one.",
                        "properties": {
                            "type": {"type": "string", "enum": ["root", "worktree", "pasture", "existing"]},
                            "baseRef": {"type": "string", "maxLength": 256},
                            "branch": {"type": "string", "maxLength": 256},
                            "path": {"type": "string", "maxLength": 4096},
                        },
                        "required": ["type"],
                        "additionalProperties": false,
                    },
                    "folder": {"type": "string", "maxLength": MAX_FOLDER_CHARS, "description": "Open a new workspace in the Workspaces view at this folder, an absolute path or one starting with ~ on the machine."},
                    "workspaceId": {"type": "integer", "description": "Open a new tab in this workspace, from agentz_terminal_list."},
                    "paneId": {"type": "integer", "description": "Split this Workspaces pane, from agentz_terminal_list, and open beside it."},
                    "direction": {"type": "string", "enum": ["right", "down"], "description": "Where the split opens, with paneId. Defaults to right."},
                    "clientRequestId": {"type": "string", "maxLength": 256, "description": "Stable idempotency key to reuse when retrying this mutation."},
                },
                "additionalProperties": false,
            },
            "annotations": {"readOnlyHint": false, "destructiveHint": false, "openWorldHint": true},
        }),
        json!({
            "name": "agentz_terminal_send",
            "title": "Type into an agentZ terminal",
            "description": "Type into a terminal: text first, pasted as a terminal paste (bracketed when the program asks for it), then keys in order, then enter when submit is true. Keys are names such as enter, tab, escape, backspace, up, down, pageup, ctrl-c, ctrl-d, alt-b, shift-tab, f5 or a single character. To run a shell command, send it as text with submit=true. A terminal whose process has exited takes no input. Returns the terminal's status; read it afterwards for the output.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "threadId": thread_id,
                    "paneId": pane_id,
                    "text": {"type": "string", "maxLength": MAX_TEXT_CHARS},
                    "keys": {"type": "array", "items": {"type": "string"}, "maxItems": MAX_KEYS},
                    "submit": {"type": "boolean", "description": "Press enter after the text and keys."},
                },
                "additionalProperties": false,
            },
            "annotations": {"readOnlyHint": false, "destructiveHint": true, "openWorldHint": true},
        }),
        json!({
            "name": "agentz_terminal_read",
            "title": "Read an agentZ terminal",
            "description": "Read a terminal's text, with its status, command and folder. Starts a drawer or terminal thread that isn't running yet.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "threadId": thread_id,
                    "paneId": pane_id,
                    "source": source,
                    "lines": lines,
                },
                "additionalProperties": false,
            },
            "annotations": {"readOnlyHint": true, "destructiveHint": false, "idempotentHint": true},
        }),
        json!({
            "name": "agentz_terminal_wait",
            "title": "Wait for an agentZ terminal",
            "description": "Wait until a terminal's text contains match, or its process exits, then return the text as agentz_terminal_read does, with matched. Without match it waits for the process to exit; a pane closes as it does. Text already there counts, including a command line you typed, so match on output the command line doesn't contain. timeoutMs defaults to 60 seconds; a timeout returns timedOut=true and leaves the terminal running.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "threadId": thread_id,
                    "paneId": pane_id,
                    "match": {"type": "string", "maxLength": MAX_MATCH_CHARS, "description": "Text to wait for, matched exactly."},
                    "source": source,
                    "lines": lines,
                    "timeoutMs": {"type": "integer", "minimum": 0, "maximum": MAX_WAIT.as_millis() as u64},
                },
                "additionalProperties": false,
            },
            "annotations": {"readOnlyHint": true, "destructiveHint": false, "idempotentHint": true},
        }),
    ]
}
