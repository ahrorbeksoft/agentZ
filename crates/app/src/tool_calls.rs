//! How a tool call's row reads (`design/tool-calls/decisions.md`): reads and edits by their
//! file's icon, agentZ's own tools as what they did, ToolSearch as the tools it loaded, and other
//! MCP tools by name and server, the same whichever agent called them.

use std::path::{Path, PathBuf};

use agent_client_protocol::schema::v1 as acp;
use agentz_protocol::artifacts::ArtifactId;
use agentz_protocol::mcp_servers::{AGENTZ_SERVER_NAME, AGENTZ_TOOLS};
use agentz_protocol::thread::ToolCall;
use gpui::{App, SharedString};
use projects::ThreadId;
use serde_json::Value;
use theme::{GlobalTheme, IconTheme};
use util::paths::PathExt as _;

/// What a tool call is, for its row.
pub enum ToolCallKind {
    /// One of agentZ's own tools, with what the agent passed it and what it gave back.
    Own {
        tool: OwnTool,
        arguments: Value,
        output: Option<Value>,
    },
    ToolSearch(ToolSearch),
    /// Another MCP server's tool.
    Mcp(McpName),
    /// One of the agent's own subagents.
    Subagent(SubagentCall),
    /// Anything else, shown by its kind and the agent's title.
    Plain,
}

impl ToolCallKind {
    pub fn of(tool_call: &ToolCall) -> Self {
        if let Some(subagent) = SubagentCall::of(tool_call) {
            return Self::Subagent(subagent);
        }
        let title = tool_call.title.trim();
        if is_tool_search(title) {
            return Self::ToolSearch(ToolSearch::of(tool_call));
        }
        // Codex names a call "Tool: <server>/<tool>" and puts both in its input, so only its
        // input is read before the name is known: an edit's can be a whole file.
        let codex_input = title
            .starts_with("Tool: ")
            .then(|| raw_input(tool_call))
            .flatten();
        let tool = match mcp_name(title, codex_input.as_ref()) {
            Some(name) if name.server.eq_ignore_ascii_case(AGENTZ_SERVER_NAME) => {
                match OwnTool::named(&name.tool) {
                    Some(tool) => tool,
                    None => return Self::Mcp(name),
                }
            }
            Some(name) => return Self::Mcp(name),
            None => match own_tool_in(title) {
                Some(tool) => tool,
                None => return Self::Plain,
            },
        };
        let input = codex_input.or_else(|| raw_input(tool_call));
        Self::Own {
            tool,
            arguments: tool_arguments(input),
            output: tool_call.text.iter().find_map(|text| json_in(text)),
        }
    }

    /// The subthread a `delegate_task` call started, once it has. Another machine's thread ids
    /// aren't this one's.
    pub fn delegated_subthread(&self) -> Option<ThreadId> {
        let Self::Own {
            tool: OwnTool::DelegateTask,
            arguments,
            output: Some(output),
        } = self
        else {
            return None;
        };
        if arguments
            .get("machine")
            .is_some_and(|machine| !machine.is_null())
        {
            return None;
        }
        thread_id_in(output.get("childThreadId")).or_else(|| thread_id_in(output.get("taskId")))
    }
}

/// One of the agent's own subagents: Claude Agent's, which work in a subthread of their own and
/// show as the card the server adds, or a call that names a `subagent_type` (Factory Droid's
/// Task, and Claude Agent's own Task without subagent sessions).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SubagentCall {
    /// What it's doing: the call's description, or its title.
    pub description: String,
    /// Its kind ("worker", "Explore"), when the agent says.
    pub kind: Option<String>,
    /// What the agent asked of it.
    pub prompt: Option<String>,
    /// The values of its other options ("heavy" for Droid's complexity).
    pub options: Vec<String>,
    /// The subthread it works in.
    pub subthread: Option<ThreadId>,
    /// Whether it was sent to run in the background, so the call ended as it started and
    /// what came back is only the agent's notice that it did.
    pub in_background: bool,
}

impl SubagentCall {
    pub fn of(tool_call: &ToolCall) -> Option<Self> {
        if let Some(subthread) = tool_call.subthread {
            return Some(Self {
                description: tool_call.title.trim().to_string(),
                kind: None,
                prompt: None,
                options: Vec::new(),
                subthread: Some(subthread),
                in_background: false,
            });
        }
        if !may_be_subagent(tool_call) {
            return None;
        }
        let input = raw_input(tool_call)?;
        let input = input.as_object()?;
        let kind = text_in(input.get("subagent_type"))?;
        let options = input
            .iter()
            .filter(|(key, _)| !matches!(key.as_str(), "subagent_type" | "description" | "prompt"))
            .filter_map(|(_, value)| text_in(Some(value)))
            .collect();
        // Droid's Task answers one it doesn't wait for with "Task launched in background.",
        // its id, and how it reports.
        let in_background = input.get("await").and_then(Value::as_bool) == Some(false)
            || input.get("run_in_background").and_then(Value::as_bool) == Some(true)
            || tool_call
                .text
                .first()
                .is_some_and(|text| text.trim_start().starts_with("Task launched in background"));
        Some(Self {
            description: text_in(input.get("description"))
                .unwrap_or_else(|| tool_call.title.trim().to_string()),
            kind: Some(kind),
            prompt: text_in(input.get("prompt")),
            options,
            subthread: None,
            in_background,
        })
    }
}

/// Whether the call is one of the agent's own subagents ([`SubagentCall`]).
pub fn is_subagent(tool_call: &ToolCall) -> bool {
    tool_call.subthread.is_some()
        || (may_be_subagent(tool_call) && SubagentCall::of(tool_call).is_some())
}

/// The artifact a finished `agentz_artifact_publish` made and the version it was, from its
/// answer. Its row is the artifact's card, which shows apart from the work around it, as a
/// subagent's does.
pub fn published_artifact(tool_call: &ToolCall) -> Option<(ArtifactId, u32)> {
    // Told apart by name before the answer is parsed, as most calls aren't one.
    let title = tool_call.title.as_str();
    if !title.contains("artifact_publish") && !title.contains("Publish an artifact") {
        return None;
    }
    let ToolCallKind::Own {
        tool: OwnTool::ArtifactPublish,
        output: Some(output),
        ..
    } = ToolCallKind::of(tool_call)
    else {
        return None;
    };
    let id = output.get("artifactId")?.as_str()?;
    let version = u32::try_from(output.get("version")?.as_u64()?).ok()?;
    Some((ArtifactId(id.to_string()), version))
}

/// Most calls can't be a subagent, and are told apart without parsing their input: Droid's
/// Task is of kind "other", and Claude Agent's "think".
fn may_be_subagent(tool_call: &ToolCall) -> bool {
    matches!(tool_call.kind, acp::ToolKind::Other | acp::ToolKind::Think)
        && tool_call
            .raw_input
            .as_deref()
            .is_some_and(|input| input.contains("\"subagent_type\""))
}

/// An MCP tool, by its server's name and its own.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct McpName {
    pub server: String,
    pub tool: String,
}

impl McpName {
    /// The tool's name in words: underscores and dashes as spaces, and a capital first letter,
    /// as t3code names MCP tools ("create_issue" is "Create issue").
    pub fn words(&self) -> String {
        tool_words(&self.tool)
    }
}

pub fn tool_words(tool: &str) -> String {
    let spaced = tool.replace(['_', '-'], " ");
    let words = spaced.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut characters = words.chars();
    match characters.next() {
        Some(first) => first.to_uppercase().chain(characters).collect(),
        None => tool.to_string(),
    }
}

/// The server and tool a call's name joins, as each agent spells it: `<server>___<tool>`
/// (Factory Droid), `mcp__<server>__<tool>` (Claude Agent, Qwen Code, Amp), "Tool:
/// <server>/<tool>" with both in its input (Codex), and "<tool> (<server> MCP Server)" (Gemini
/// CLI, and Qwen Code with ": <arguments>" after it). t3code's survey of ACP agents found these.
pub fn mcp_name(title: &str, codex_input: Option<&Value>) -> Option<McpName> {
    let title = title.trim();
    let named = |server: &str, tool: &str| {
        let (server, tool) = (server.trim(), tool.trim());
        (is_name(server) && is_name(tool)).then(|| McpName {
            server: server.to_string(),
            tool: tool.to_string(),
        })
    };
    if let Some(input) = codex_input.and_then(Value::as_object)
        && let (Some(server), Some(tool)) = (
            input.get("server").and_then(Value::as_str),
            input.get("tool").and_then(Value::as_str),
        )
    {
        return named(server, tool);
    }
    if let Some(rest) = title.strip_prefix("Tool: ") {
        let (server, tool) = rest.split_once('/')?;
        return named(server, tool);
    }
    if let Some(rest) = title.strip_prefix("mcp__") {
        let (server, tool) = rest.split_once("__")?;
        return named(server, tool);
    }
    if let Some((server, tool)) = title.split_once("___") {
        return named(server, tool);
    }
    let (tool, rest) = title.split_once(" (")?;
    let (server, _) = rest.split_once(" MCP Server)")?;
    named(server, tool)
}

fn is_name(text: &str) -> bool {
    !text.is_empty()
        && text
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || "_-.".contains(character))
}

/// One of agentZ's own tools: the `agentz` server's, in [`AGENTZ_TOOLS`]'s order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum OwnTool {
    Capabilities,
    ThreadList,
    ThreadRead,
    ThreadLaunch,
    CreateThreads,
    ThreadSend,
    ThreadWait,
    ThreadInterrupt,
    ThreadUpdate,
    ThreadOrganize,
    ThreadDiff,
    DelegateTask,
    WorkspaceStatus,
    WorkspaceList,
    WorkspaceHandoff,
    WorkspaceSync,
    WorkspaceBringBack,
    TaskStatus,
    TaskCancel,
    TerminalList,
    TerminalStart,
    TerminalSend,
    TerminalRead,
    TerminalWait,
    CommandRun,
    ProjectAdd,
    ArtifactPublish,
    ArtifactList,
    ArtifactRead,
}

impl OwnTool {
    const ALL: [Self; AGENTZ_TOOLS.len()] = [
        Self::Capabilities,
        Self::ThreadList,
        Self::ThreadRead,
        Self::ThreadLaunch,
        Self::CreateThreads,
        Self::ThreadSend,
        Self::ThreadWait,
        Self::ThreadInterrupt,
        Self::ThreadUpdate,
        Self::ThreadOrganize,
        Self::ThreadDiff,
        Self::DelegateTask,
        Self::WorkspaceStatus,
        Self::WorkspaceList,
        Self::WorkspaceHandoff,
        Self::WorkspaceSync,
        Self::WorkspaceBringBack,
        Self::TaskStatus,
        Self::TaskCancel,
        Self::TerminalList,
        Self::TerminalStart,
        Self::TerminalSend,
        Self::TerminalRead,
        Self::TerminalWait,
        Self::CommandRun,
        Self::ProjectAdd,
        Self::ArtifactPublish,
        Self::ArtifactList,
        Self::ArtifactRead,
    ];

    pub fn named(name: &str) -> Option<Self> {
        let position = AGENTZ_TOOLS
            .iter()
            .position(|(tool_name, _)| *tool_name == name)?;
        Some(Self::ALL[position])
    }

    /// The tool's title, as its server describes it.
    pub fn title(self) -> &'static str {
        AGENTZ_TOOLS[self as usize].1
    }

    /// What it did, as its row says it: in the past tense once it's done, the present while it
    /// runs, and as an order when it failed.
    pub fn sentence(
        self,
        arguments: &Value,
        output: Option<&Value>,
        state: CallState,
        title_of: &dyn Fn(ThreadId) -> Option<String>,
    ) -> Sentence {
        // Thread ids are per machine, so another machine's aren't looked up here.
        let is_remote = arguments
            .get("machine")
            .is_some_and(|machine| !machine.is_null());
        let title_of = |thread_id: Option<ThreadId>| {
            thread_id
                .filter(|_| !is_remote)
                .and_then(|thread_id| title_of(thread_id))
        };
        let argument = |key: &str| text_in(arguments.get(key));
        let thread = |key: &str, unnamed: &str, own: &str| {
            let thread_id = thread_id_in(arguments.get(key));
            let text = match thread_id {
                Some(_) => title_of(thread_id).ok_or(unnamed),
                None => Err(own),
            };
            Some(match text {
                Ok(title) => Subject::title(title),
                Err(words) => Subject::plain(words),
            })
        };
        // A Workspaces pane, by `paneId`, is no thread's.
        let terminal = || match arguments.get("paneId").filter(|pane| !pane.is_null()) {
            Some(_) => Some(Subject::plain("a terminal")),
            None => thread("threadId", "a terminal", "this thread's terminal"),
        };
        let opened = |key: &str| {
            output
                .and_then(|output| thread_id_in(output.get(key)))
                .filter(|_| !is_remote)
        };
        let made =
            |thread_id: Option<ThreadId>, spec: &Value, output: Option<&Value>, prompt: &str| {
                title_of(thread_id)
                    .or_else(|| text_in(spec.get("title")))
                    .or_else(|| output.and_then(|output| text_in(output.get("title"))))
                    .or_else(|| text_in(spec.get(prompt)))
                    .map(Subject::title)
            };
        let verbs = |base: &str, present: &str, past: &str| Verbs {
            base: base.to_string(),
            present: present.to_string(),
            past: past.to_string(),
        };
        let mut object = None;
        let (verbs, subject, opens) = match self {
            Self::Capabilities => (
                verbs(
                    "List agents and models",
                    "Listing agents and models",
                    "Listed agents and models",
                ),
                None,
                None,
            ),
            Self::ThreadList => (
                verbs("List threads", "Listing threads", "Listed threads"),
                None,
                None,
            ),
            Self::ThreadRead => (
                verbs("Read", "Reading", "Read"),
                thread("threadId", "a thread", "a thread"),
                None,
            ),
            Self::ThreadLaunch => {
                let thread_id = opened("threadId");
                (
                    verbs("Start a thread:", "Starting a thread:", "Started a thread:"),
                    made(thread_id, arguments, output, "prompt"),
                    thread_id,
                )
            }
            Self::CreateThreads => {
                let specs = arguments
                    .get("threads")
                    .and_then(Value::as_array)
                    .cloned()
                    .unwrap_or_default();
                let made_threads = output
                    .and_then(|output| output.get("threads"))
                    .and_then(Value::as_array);
                match specs.as_slice() {
                    [spec] => {
                        let made_thread = made_threads.and_then(|threads| threads.first());
                        let thread_id = made_thread
                            .and_then(|thread| thread_id_in(thread.get("threadId")))
                            .filter(|_| !is_remote);
                        (
                            verbs("Start a thread:", "Starting a thread:", "Started a thread:"),
                            made(thread_id, spec, made_thread, "prompt"),
                            thread_id,
                        )
                    }
                    specs => {
                        let count = specs.len();
                        (
                            verbs(
                                &format!("Start {count} threads"),
                                &format!("Starting {count} threads"),
                                &format!("Started {count} threads"),
                            ),
                            None,
                            None,
                        )
                    }
                }
            }
            Self::ThreadSend => (
                verbs(
                    "Send a message to",
                    "Sending a message to",
                    "Sent a message to",
                ),
                thread("threadId", "a thread", "a thread"),
                None,
            ),
            Self::ThreadWait => (
                verbs("Wait for", "Waiting for", "Waited for"),
                thread("threadId", "a thread", "a thread"),
                None,
            ),
            Self::ThreadInterrupt => (
                verbs("Interrupt", "Interrupting", "Interrupted"),
                thread("threadId", "a thread", "a thread"),
                None,
            ),
            Self::ThreadUpdate => {
                let target = if thread_id_in(arguments.get("threadId")).is_some() {
                    "a thread"
                } else {
                    "this thread"
                };
                (
                    verbs(
                        &format!("Rename {target} to"),
                        &format!("Renaming {target} to"),
                        &format!("Renamed {target} to"),
                    ),
                    argument("title").map(Subject::title),
                    None,
                )
            }
            Self::ThreadOrganize => (
                match argument("action").as_deref() {
                    Some("pin") => verbs("Pin", "Pinning", "Pinned"),
                    Some("unpin") => verbs("Unpin", "Unpinning", "Unpinned"),
                    Some("archive") => verbs("Archive", "Archiving", "Archived"),
                    Some("unarchive") => verbs("Unarchive", "Unarchiving", "Unarchived"),
                    _ => verbs("Organize", "Organizing", "Organized"),
                },
                thread("threadId", "a thread", "this thread"),
                None,
            ),
            Self::ThreadDiff => (
                verbs(
                    "Read the changes in",
                    "Reading the changes in",
                    "Read the changes in",
                ),
                thread("threadId", "a thread", "this thread"),
                None,
            ),
            Self::DelegateTask => {
                let thread_id = opened("childThreadId").or_else(|| opened("taskId"));
                (
                    verbs(
                        "Start a subthread:",
                        "Starting a subthread:",
                        "Started a subthread:",
                    ),
                    made(thread_id, arguments, output, "task"),
                    thread_id,
                )
            }
            Self::WorkspaceStatus => (
                verbs(
                    "Check this thread's workspace",
                    "Checking this thread's workspace",
                    "Checked this thread's workspace",
                ),
                None,
                None,
            ),
            Self::WorkspaceList => (
                verbs(
                    "List branches and workspaces",
                    "Listing branches and workspaces",
                    "Listed branches and workspaces",
                ),
                None,
                None,
            ),
            Self::WorkspaceHandoff => {
                let kind = match argument("type").as_deref() {
                    Some("pasture") => "pasture",
                    Some("worktree") => "worktree",
                    _ => "workspace",
                };
                (
                    verbs(
                        &format!("Hand off to a new {kind}:"),
                        &format!("Handing off to a new {kind}:"),
                        &format!("Handed off to a new {kind}:"),
                    ),
                    argument("branch").map(Subject::code),
                    None,
                )
            }
            Self::WorkspaceSync => match argument("branch") {
                Some(branch) => (
                    verbs(
                        "Sync this pasture with",
                        "Syncing this pasture with",
                        "Synced this pasture with",
                    ),
                    Some(Subject::code(branch)),
                    None,
                ),
                None => (
                    verbs(
                        "Sync this pasture from the project",
                        "Syncing this pasture from the project",
                        "Synced this pasture from the project",
                    ),
                    None,
                    None,
                ),
            },
            Self::WorkspaceBringBack => (
                verbs(
                    "Bring this pasture's branch to the project",
                    "Bringing this pasture's branch to the project",
                    "Brought this pasture's branch to the project",
                ),
                None,
                None,
            ),
            Self::TaskStatus => (
                verbs("Check on", "Checking on", "Checked on"),
                thread("taskId", "a subthread", "a subthread"),
                None,
            ),
            Self::TaskCancel => (
                verbs("Cancel", "Cancelling", "Cancelled"),
                thread("taskId", "a subthread", "a subthread"),
                None,
            ),
            Self::TerminalList => (
                verbs("List terminals", "Listing terminals", "Listed terminals"),
                None,
                None,
            ),
            Self::TerminalStart => (
                verbs(
                    "Start a terminal:",
                    "Starting a terminal:",
                    "Started a terminal:",
                ),
                argument("command")
                    .or_else(|| argument("folder"))
                    .map(Subject::code),
                opened("threadId"),
            ),
            // What was typed comes first: "Typed `npm test ⏎` into this thread's terminal".
            Self::TerminalSend => match typed(arguments) {
                Some(typed) => {
                    object = terminal().map(|terminal| ("into".to_string(), terminal));
                    (
                        verbs("Type", "Typing", "Typed"),
                        Some(Subject::code(typed)),
                        None,
                    )
                }
                None => (
                    verbs("Type into", "Typing into", "Typed into"),
                    terminal(),
                    None,
                ),
            },
            Self::TerminalRead => (verbs("Read", "Reading", "Read"), terminal(), None),
            Self::TerminalWait => (
                verbs("Wait for", "Waiting for", "Waited for"),
                terminal(),
                None,
            ),
            Self::CommandRun => (
                verbs("Run", "Running", "Ran"),
                argument("command").map(Subject::code),
                None,
            ),
            Self::ProjectAdd => (
                verbs("Add a project:", "Adding a project:", "Added a project:"),
                argument("path").map(Subject::code),
                None,
            ),
            // Its row is the artifact's card, so a finished publish has no result words.
            Self::ArtifactPublish => (
                verbs(
                    "Publish an artifact:",
                    "Publishing an artifact:",
                    "Published an artifact:",
                ),
                argument("title").map(Subject::title),
                None,
            ),
            Self::ArtifactList => (
                verbs("List artifacts", "Listing artifacts…", "Listed artifacts"),
                None,
                None,
            ),
            Self::ArtifactRead => (
                verbs(
                    "Read an artifact",
                    "Reading an artifact…",
                    "Read an artifact",
                ),
                None,
                None,
            ),
        };
        let mut verb = match state {
            CallState::Running => verbs.present,
            CallState::Done => verbs.past,
            CallState::Failed => verbs.base,
        };
        if subject.is_none() {
            if verb.ends_with(':') {
                verb.pop();
            }
            if state == CallState::Running {
                verb.push('…');
            }
        }
        Sentence {
            verb,
            subject,
            object,
            opens: opens.filter(|_| state == CallState::Done),
        }
    }

    /// What came of a finished call, after its sentence and dimmer (`design/agentz-tools`,
    /// topic 2): "8 threads", "Done in 2m 14s", "Exit 0 · 14s", or the lines a thread changed.
    /// Calls that made a thread or a terminal have Open instead, and those that only did
    /// something have nothing to add. `duration` is how long the call took, in words.
    pub fn result(
        self,
        arguments: &Value,
        output: &Value,
        duration: Option<&str>,
    ) -> Option<OwnResult> {
        let words = |text: String| Some(OwnResult::Words(text));
        let count = |key: &str| output.get(key).and_then(Value::as_array).map(Vec::len);
        match self {
            Self::Capabilities => words(counted(count("agents")?, "agent", "agents")),
            Self::ThreadList => {
                let total = output
                    .get("total")
                    .and_then(Value::as_u64)
                    .map(|total| total as usize)
                    .or_else(|| count("threads"))?;
                words(counted(total, "thread", "threads"))
            }
            Self::ThreadRead => {
                let items = count("items")?;
                match text_in(arguments.get("view")).as_deref() {
                    Some("activity") => words(counted(items, "item", "items")),
                    _ => words(counted(items, "message", "messages")),
                }
            }
            Self::CreateThreads => {
                let titles: Vec<String> = output
                    .get("threads")
                    .and_then(Value::as_array)?
                    .iter()
                    .filter_map(|thread| text_in(thread.get("title")))
                    .collect();
                // One thread opens instead.
                if titles.len() < 2 {
                    return None;
                }
                let mut shown = titles[..2].join(", ");
                if titles.len() > 2 {
                    shown.push_str(&format!(", {} more", titles.len() - 2));
                }
                words(shown)
            }
            Self::ThreadWait => {
                if output.get("timedOut").and_then(Value::as_bool) == Some(true) {
                    return words("Still working".to_string());
                }
                if text_in(output.get("turnError")).is_some() {
                    return words("Failed".to_string());
                }
                match text_in(output.get("status")).as_deref() {
                    Some("idle") => words(match duration {
                        Some(duration) => format!("Done in {duration}"),
                        None => "Done".to_string(),
                    }),
                    Some(status) => words(capitalized(thread_status(status).0)),
                    None => None,
                }
            }
            Self::ThreadDiff => {
                let files = output.get("files").and_then(Value::as_array)?;
                if files.is_empty() {
                    return words("No changes".to_string());
                }
                let lines = |key: &str| {
                    files
                        .iter()
                        .filter_map(|file| file.get(key).and_then(Value::as_u64))
                        .sum()
                };
                Some(OwnResult::Changes {
                    added: lines("additions"),
                    removed: lines("deletions"),
                })
            }
            Self::TaskStatus => {
                let status = text_in(output.get("status"))?;
                if status == "running"
                    && text_in(output.get("workState")).as_deref() == Some("waiting_for_children")
                {
                    return words("Waiting for its subthreads".to_string());
                }
                words(capitalized(thread_status(&status).0))
            }
            Self::WorkspaceStatus => {
                let kind = workspace_kind(&text_in(output.get("kind"))?);
                words(match text_in(output.get("branch")) {
                    Some(branch) => format!("{kind} on {branch}"),
                    None => kind,
                })
            }
            Self::WorkspaceList => {
                if output.get("isRepository").and_then(Value::as_bool) == Some(false) {
                    return words("Not a git repository".to_string());
                }
                let more = output.get("hasMoreBranches").and_then(Value::as_bool) == Some(true);
                let branches = count("branches")?;
                let branches = if more {
                    format!("{branches}+ branches")
                } else {
                    counted(branches, "branch", "branches")
                };
                let workspaces = counted(count("workspaces")?, "workspace", "workspaces");
                words(format!("{branches}, {workspaces}"))
            }
            // The server says how it synced: "Rebased the pasture onto …", "Merged …".
            Self::WorkspaceSync => {
                let message = text_in(output.get("message"))?;
                ["Rebased", "Merged"]
                    .into_iter()
                    .find(|word| message.starts_with(word))
                    .and_then(|word| words(word.to_string()))
            }
            Self::TerminalList => words(counted(count("terminals")?, "terminal", "terminals")),
            Self::TerminalRead => {
                let text = output.get("text").and_then(Value::as_str)?;
                words(counted(printed_lines(text), "line", "lines"))
            }
            Self::TerminalWait => {
                if output.get("matched").and_then(Value::as_bool) == Some(true) {
                    return words("Matched".to_string());
                }
                if output.get("timedOut").and_then(Value::as_bool) == Some(true) {
                    return words("Timed out".to_string());
                }
                exit_words(output).and_then(words)
            }
            Self::CommandRun => {
                let ended = if output.get("timedOut").and_then(Value::as_bool) == Some(true) {
                    "Timed out".to_string()
                } else {
                    exit_words(output)?
                };
                words(match duration {
                    Some(duration) => format!("{ended} · {duration}"),
                    None => ended,
                })
            }
            Self::ProjectAdd => text_in(output.get("name")).and_then(words),
            // A publish's card says the version; a list says how many there are.
            Self::ArtifactPublish => None,
            Self::ArtifactList => words(counted(count("artifacts")?, "artifact", "artifacts")),
            Self::ArtifactRead => output
                .get("version")
                .and_then(Value::as_u64)
                .and_then(|version| words(format!("v{version}"))),
            Self::ThreadLaunch
            | Self::ThreadSend
            | Self::ThreadInterrupt
            | Self::ThreadUpdate
            | Self::ThreadOrganize
            | Self::DelegateTask
            | Self::WorkspaceHandoff
            | Self::WorkspaceBringBack
            | Self::TaskCancel
            | Self::TerminalStart
            | Self::TerminalSend => None,
        }
    }

    /// What an opened row shows of what came back, in words (`design/agentz-tools`, topic 4):
    /// threads one a line with their agent, model, status and when; what a command or a
    /// terminal printed, as printed; a workspace in a sentence and its folder; each agent with
    /// its models. Empty when there's nothing to say beyond the input.
    pub fn view(self, arguments: &Value, output: &Value) -> Vec<OwnPart> {
        let threads = |key: &str| -> Vec<OwnPart> {
            output
                .get(key)
                .and_then(Value::as_array)
                .map(|threads| {
                    threads
                        .iter()
                        .filter_map(ThreadLine::of)
                        .map(OwnPart::Thread)
                        .collect()
                })
                .unwrap_or_default()
        };
        let printed = |key: &str| {
            output
                .get(key)
                .and_then(Value::as_str)
                .map(|text| text.trim_end_matches(['\n', ' ']))
                .filter(|text| !text.trim().is_empty())
                .map(|text| OwnPart::Printed(text.to_string()))
        };
        let folder = |key: &str| {
            text_in(output.get(key)).map(|folder| OwnPart::Line(vec![Span::dim(folder)]))
        };
        match self {
            Self::Capabilities => output
                .get("agents")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|agent| {
                    let name = text_in(agent.get("name"))?;
                    let models: Vec<String> = agent
                        .get("models")
                        .and_then(Value::as_array)
                        .into_iter()
                        .flatten()
                        .filter_map(|model| {
                            text_in(model.get("name")).or_else(|| text_in(model.get("id")))
                        })
                        .collect();
                    let mut spans = vec![Span::bright(name)];
                    if !models.is_empty() {
                        spans.push(Span::plain(models.join(", ")));
                    }
                    Some(OwnPart::Line(spans))
                })
                .collect(),
            Self::ThreadList | Self::CreateThreads => {
                let threads = threads("threads");
                if threads.is_empty() && self == Self::ThreadList {
                    return vec![OwnPart::Line(vec![Span::plain("No threads")])];
                }
                threads
            }
            Self::ThreadLaunch => ThreadLine::of(output)
                .map(OwnPart::Thread)
                .into_iter()
                .collect(),
            Self::ThreadRead => {
                let mut parts: Vec<OwnPart> = output
                    .get("thread")
                    .and_then(ThreadLine::of)
                    .map(OwnPart::Thread)
                    .into_iter()
                    .collect();
                for item in output
                    .get("items")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                {
                    let kind = text_in(item.get("type"));
                    let (who, text) = match kind.as_deref() {
                        Some("user_message") => ("User", text_in(item.get("text"))),
                        Some("agent_message") => ("Agent", text_in(item.get("text"))),
                        Some("plan") => ("Plan", text_in(item.get("text"))),
                        Some("thought") => ("Thought", text_in(item.get("text"))),
                        _ => ("Tool call", text_in(item.get("title"))),
                    };
                    if let Some(text) = text {
                        parts.push(OwnPart::Line(vec![Span::dim(who), Span::plain(text)]));
                    }
                }
                parts
            }
            Self::ThreadSend => text_in(arguments.get("message"))
                .map(OwnPart::Prose)
                .into_iter()
                .collect(),
            Self::ThreadWait => {
                let mut parts: Vec<OwnPart> = text_in(output.get("lastAgentMessage"))
                    .map(OwnPart::Prose)
                    .into_iter()
                    .collect();
                parts.extend(
                    text_in(output.get("turnError"))
                        .map(|error| OwnPart::Line(vec![Span::error(error)])),
                );
                parts
            }
            Self::ThreadInterrupt => {
                let mut said = match text_in(output.get("status")).as_deref() {
                    Some("interrupt_requested") => "Its turn was stopped".to_string(),
                    Some("no_active_run") => "It wasn't working".to_string(),
                    _ => return Vec::new(),
                };
                match output.get("droppedQueuedMessages").and_then(Value::as_u64) {
                    Some(1) => said.push_str(", and its queued message was dropped"),
                    Some(dropped) if dropped > 1 => {
                        said.push_str(&format!(", and its {dropped} queued messages were dropped"))
                    }
                    _ => {}
                }
                vec![OwnPart::Line(vec![Span::plain(format!("{said}."))])]
            }
            Self::ThreadDiff => output
                .get("files")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|file| {
                    Some(OwnPart::File {
                        path: text_in(file.get("path"))?,
                        added: file.get("additions").and_then(Value::as_u64).unwrap_or(0),
                        removed: file.get("deletions").and_then(Value::as_u64).unwrap_or(0),
                    })
                })
                .collect(),
            Self::DelegateTask | Self::TaskStatus => {
                let mut parts: Vec<OwnPart> = ThreadLine::of(output)
                    .map(OwnPart::Thread)
                    .into_iter()
                    .collect();
                parts.extend(text_in(output.get("summary")).map(OwnPart::Prose));
                parts
            }
            Self::WorkspaceStatus => {
                let Some(kind) = text_in(output.get("kind")) else {
                    return Vec::new();
                };
                let mut spans = vec![Span::plain(workspace_kind(&kind))];
                if let Some(branch) = text_in(output.get("branch")) {
                    spans.push(Span::plain("on"));
                    spans.push(Span::code(branch));
                }
                if let Some(base) = text_in(output.get("baseRef")) {
                    spans.push(Span::plain("from"));
                    spans.push(Span::code(base));
                }
                let mut parts = vec![OwnPart::Line(spans)];
                parts.extend(folder("folder"));
                parts
            }
            Self::WorkspaceList => {
                if output.get("isRepository").and_then(Value::as_bool) == Some(false) {
                    return vec![OwnPart::Line(vec![Span::plain("Not a git repository")])];
                }
                let mut parts: Vec<OwnPart> = output
                    .get("workspaces")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter_map(|workspace| {
                        let mut spans = vec![Span::plain(workspace_kind(&text_in(
                            workspace.get("kind"),
                        )?))];
                        if let Some(branch) = text_in(workspace.get("branch")) {
                            spans.push(Span::plain("on"));
                            spans.push(Span::code(branch));
                        }
                        spans.extend(text_in(workspace.get("path")).map(Span::dim));
                        Some(OwnPart::Line(spans))
                    })
                    .collect();
                let branches: Vec<String> = output
                    .get("branches")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter_map(|branch| text_in(branch.get("name")))
                    .collect();
                if !branches.is_empty() {
                    parts.push(OwnPart::Line(vec![
                        Span::dim("Branches"),
                        Span::code(branches.join(", ")),
                    ]));
                }
                parts
            }
            Self::WorkspaceHandoff => {
                let Some(kind) = text_in(output.get("kind")) else {
                    return Vec::new();
                };
                let mut spans = vec![Span::plain(format!("A new {kind}"))];
                if let Some(branch) = text_in(output.get("branch")) {
                    spans.push(Span::plain("on"));
                    spans.push(Span::code(branch));
                }
                let mut parts = vec![OwnPart::Line(spans)];
                parts.extend(folder("workspacePath"));
                parts
            }
            Self::WorkspaceSync | Self::WorkspaceBringBack => text_in(output.get("message"))
                .map(|message| OwnPart::Line(code_spans(&message)))
                .into_iter()
                .collect(),
            Self::TerminalList => output
                .get("terminals")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(terminal_line)
                .collect(),
            Self::TerminalStart => terminal_line(output).into_iter().collect(),
            Self::TerminalRead | Self::TerminalWait => printed("text").into_iter().collect(),
            Self::CommandRun => printed("output").into_iter().collect(),
            // The card carries everything a publish has to show.
            Self::ArtifactPublish => Vec::new(),
            Self::ArtifactList => output
                .get("artifacts")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|artifact| {
                    let title = text_in(artifact.get("title"))?;
                    let mut spans = vec![Span::bright(title)];
                    if let Some(version) = artifact.get("versions").and_then(Value::as_u64) {
                        spans.push(Span::dim(format!("v{version}")));
                    }
                    if let Some(place) = text_in(artifact.get("place")) {
                        spans.push(Span::dim(place));
                    }
                    Some(OwnPart::Line(spans))
                })
                .collect(),
            Self::ArtifactRead => text_in(output.get("source"))
                .map(|source| OwnPart::Printed(source.chars().take(2000).collect::<String>()))
                .into_iter()
                .collect(),
            Self::ProjectAdd => {
                let Some(name) = text_in(output.get("name")) else {
                    return Vec::new();
                };
                let mut spans = vec![Span::bright(name)];
                if output.get("alreadyAdded").and_then(Value::as_bool) == Some(true) {
                    spans.push(Span::dim("already added"));
                }
                let mut parts = vec![OwnPart::Line(spans)];
                parts.extend(folder("path"));
                parts
            }
            Self::ThreadUpdate | Self::ThreadOrganize | Self::TaskCancel | Self::TerminalSend => {
                Vec::new()
            }
        }
    }

    /// The tool whose count a folded run of work gives for this one: a thread made by
    /// `create_threads` counts with those `agentz_thread_launch` made.
    pub fn fold_group(self) -> Self {
        match self {
            Self::CreateThreads => Self::ThreadLaunch,
            tool => tool,
        }
    }

    /// Making a subthread, a thread or a terminal, and running a command, lead a folded run's
    /// summary, as commands and edits do.
    pub fn leads_summary(self) -> bool {
        matches!(
            self,
            Self::DelegateTask | Self::ThreadLaunch | Self::TerminalStart | Self::CommandRun
        )
    }

    /// What `count` calls of a [`Self::fold_group`] did, in a folded run's summary ("Started 3
    /// subthreads").
    pub fn fold_label(self, count: usize) -> String {
        let counted = |one: &str, many: &str| {
            if count == 1 {
                one.to_string()
            } else {
                many.replace("{}", &count.to_string())
            }
        };
        match self {
            Self::Capabilities => "Listed agents and models".to_string(),
            Self::ThreadList => "Listed threads".to_string(),
            Self::ThreadRead => counted("Read a thread", "Read {} threads"),
            Self::ThreadLaunch | Self::CreateThreads => {
                counted("Started a thread", "Started {} threads")
            }
            Self::ThreadSend => counted("Sent a message", "Sent {} messages"),
            Self::ThreadWait => counted("Waited for a thread", "Waited for {} threads"),
            Self::ThreadInterrupt => counted("Interrupted a thread", "Interrupted {} threads"),
            Self::ThreadUpdate => counted("Renamed a thread", "Renamed {} threads"),
            Self::ThreadOrganize => counted("Organized a thread", "Organized {} threads"),
            Self::ThreadDiff => counted("Read a thread's changes", "Read {} threads' changes"),
            Self::DelegateTask => counted("Started a subthread", "Started {} subthreads"),
            Self::WorkspaceStatus => "Checked this thread's workspace".to_string(),
            Self::WorkspaceList => "Listed branches and workspaces".to_string(),
            Self::WorkspaceHandoff => "Handed off to a new workspace".to_string(),
            Self::WorkspaceSync => "Synced this pasture".to_string(),
            Self::WorkspaceBringBack => "Brought this pasture's branch to the project".to_string(),
            Self::TaskStatus => counted("Checked on a subthread", "Checked on {} subthreads"),
            Self::TaskCancel => counted("Cancelled a subthread", "Cancelled {} subthreads"),
            Self::TerminalList => "Listed terminals".to_string(),
            Self::TerminalStart => counted("Started a terminal", "Started {} terminals"),
            Self::TerminalSend => counted("Typed into a terminal", "Typed into {} terminals"),
            Self::TerminalRead => counted("Read a terminal", "Read {} terminals"),
            Self::TerminalWait => counted("Waited for a terminal", "Waited for {} terminals"),
            Self::CommandRun => counted("Ran a command", "Ran {} commands"),
            Self::ProjectAdd => counted("Added a project", "Added {} projects"),
            Self::ArtifactPublish => counted("Published an artifact", "Published {} artifacts"),
            Self::ArtifactList => "Listed artifacts".to_string(),
            Self::ArtifactRead => counted("Read an artifact", "Read {} artifacts"),
        }
    }

    /// How many things a call counts for in a folded run: `create_threads` makes several.
    pub fn fold_count(self, arguments: &Value) -> usize {
        match self {
            Self::CreateThreads => arguments
                .get("threads")
                .and_then(Value::as_array)
                .map_or(1, |threads| threads.len().max(1)),
            _ => 1,
        }
    }
}

/// agentZ's tool in a name no other spelling matched: bare, as GLM's and Kimi's agents send
/// it, or after the server's name and any separator, as OpenCode (`agentz_<tool>`) and others
/// join them. Only agentZ's own tool names count, so the match can be loose.
fn own_tool_in(name: &str) -> Option<OwnTool> {
    if let Some(tool) = OwnTool::named(name) {
        return Some(tool);
    }
    let lowered = name.to_ascii_lowercase();
    let rest = ["mcp__", "mcp_", "mcp-"]
        .iter()
        .find_map(|prefix| lowered.strip_prefix(prefix))
        .unwrap_or(&lowered);
    let rest = rest.strip_prefix(AGENTZ_SERVER_NAME)?;
    let tool = rest.trim_start_matches(['_', '-', '.', ':', '/', ' ']);
    let separators = rest.len() - tool.len();
    if !(1..=3).contains(&separators) {
        return None;
    }
    OwnTool::named(tool)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CallState {
    Running,
    Done,
    Failed,
}

impl CallState {
    pub fn of(status: &acp::ToolCallStatus) -> Self {
        match status {
            acp::ToolCallStatus::Pending | acp::ToolCallStatus::InProgress => Self::Running,
            acp::ToolCallStatus::Failed => Self::Failed,
            _ => Self::Done,
        }
    }

    /// [`Self::of`] its status, where a call cut off by a stopped turn or denied by the user
    /// didn't happen either.
    pub fn of_call(tool_call: &ToolCall) -> Self {
        if tool_call.stopped && !matches!(tool_call.status, acp::ToolCallStatus::Completed) {
            return Self::Failed;
        }
        if tool_call
            .answer
            .as_ref()
            .is_some_and(agentz_protocol::thread::ToolAnswer::is_denied)
        {
            return Self::Failed;
        }
        Self::of(&tool_call.status)
    }

    /// The verb for what the call does, in the present while it runs, the past once done, and
    /// as an order when it didn't happen.
    fn verb<'a>(self, running: &'a str, done: &'a str, failed: &'a str) -> &'a str {
        match self {
            Self::Running => running,
            Self::Done => done,
            Self::Failed => failed,
        }
    }
}

struct Verbs {
    base: String,
    present: String,
    past: String,
}

/// What one of agentZ's tools did: "Started a subthread:" and its title, with the thread it
/// made, which the row opens.
#[derive(Debug, PartialEq)]
pub struct Sentence {
    pub verb: String,
    pub subject: Option<Subject>,
    /// A second part after the subject, with the word that joins it: "into" and the terminal
    /// typed into.
    pub object: Option<(String, Subject)>,
    pub opens: Option<ThreadId>,
}

/// What came of one of agentZ's tools, after its sentence ([`OwnTool::result`]).
#[derive(Debug, PartialEq)]
pub enum OwnResult {
    Words(String),
    /// The lines a thread changed.
    Changes {
        added: u64,
        removed: u64,
    },
}

/// A part of what an opened row of agentZ's tools shows ([`OwnTool::view`]).
#[derive(Debug, PartialEq)]
pub enum OwnPart {
    /// A line of words, cut to the row's width.
    Line(Vec<Span>),
    Thread(ThreadLine),
    /// What a command or a terminal printed, in the code font.
    Printed(String),
    /// A message, as written.
    Prose(String),
    /// A file a thread changed, with its lines added and removed.
    File {
        path: String,
        added: u64,
        removed: u64,
    },
}

#[derive(Debug, PartialEq)]
pub struct Span {
    pub text: String,
    pub style: SpanStyle,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpanStyle {
    /// A name, brighter than the line.
    Bright,
    Plain,
    /// Where something is, dimmer than the line.
    Dim,
    /// A branch or a command, in the code font.
    Code,
    Error,
}

impl Span {
    fn styled(text: impl Into<String>, style: SpanStyle) -> Self {
        Self {
            text: text.into(),
            style,
        }
    }

    fn bright(text: impl Into<String>) -> Self {
        Self::styled(text, SpanStyle::Bright)
    }

    fn plain(text: impl Into<String>) -> Self {
        Self::styled(text, SpanStyle::Plain)
    }

    fn dim(text: impl Into<String>) -> Self {
        Self::styled(text, SpanStyle::Dim)
    }

    fn code(text: impl Into<String>) -> Self {
        Self::styled(text, SpanStyle::Code)
    }

    fn error(text: impl Into<String>) -> Self {
        Self::styled(text, SpanStyle::Error)
    }
}

/// A thread or a subthread as agentZ's tools describe it: its title, then its agent, model and
/// status in words, and when it was last active or ended.
#[derive(Debug, PartialEq)]
pub struct ThreadLine {
    pub title: String,
    pub details: Vec<String>,
    pub state: ThreadState,
    /// An RFC 3339 time.
    pub at: Option<String>,
}

/// How a thread is doing, for the dot before its title.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThreadState {
    Working,
    Waiting,
    Failed,
    Idle,
}

impl ThreadLine {
    /// A thread's summary, or a delegated task's result.
    fn of(thread: &Value) -> Option<Self> {
        let title = text_in(thread.get("title"))?;
        let (status, state) = match text_in(thread.get("status")) {
            Some(status) => {
                let (words, state) = thread_status(&status);
                (Some(words.to_string()), state)
            }
            None => (None, ThreadState::Idle),
        };
        let details = [
            text_in(thread.get("agentName")),
            text_in(thread.get("model")),
            status,
        ]
        .into_iter()
        .flatten()
        .collect();
        Some(Self {
            title,
            details,
            state,
            at: text_in(thread.get("lastActivityAt")).or_else(|| text_in(thread.get("endedAt"))),
        })
    }
}

/// A thread's or a delegated task's status, as agentZ's tools give it, in words.
fn thread_status(status: &str) -> (&str, ThreadState) {
    match status {
        "running" => ("working", ThreadState::Working),
        "starting" => ("starting", ThreadState::Working),
        "queued" => ("queued", ThreadState::Working),
        "waiting" => ("waiting", ThreadState::Waiting),
        "waiting_for_approval" => ("waiting for approval", ThreadState::Waiting),
        "waiting_for_input" => ("waiting for input", ThreadState::Waiting),
        "needs_login" => ("needs a login", ThreadState::Failed),
        "failed" => ("failed", ThreadState::Failed),
        "completed" => ("done", ThreadState::Idle),
        "cancelled" | "cancel_requested" => ("cancelled", ThreadState::Idle),
        "interrupted" => ("interrupted", ThreadState::Idle),
        "idle" => ("idle", ThreadState::Idle),
        status => (status, ThreadState::Idle),
    }
}

/// A workspace's kind, as `agentz_workspace_status` and `agentz_workspace_list` name it.
fn workspace_kind(kind: &str) -> String {
    match kind {
        "pasture" => "Pasture".to_string(),
        "worktree" => "Worktree".to_string(),
        "checkout" => "The project's checkout".to_string(),
        kind => capitalized(kind),
    }
}

/// A terminal as `agentz_terminal_list` and `agentz_terminal_start` describe it: what runs in
/// it, or its thread or pane, then how it's doing and its folder.
fn terminal_line(terminal: &Value) -> Option<OwnPart> {
    let name = match text_in(terminal.get("command")) {
        Some(command) => Span::code(command),
        None => match text_in(terminal.get("threadTitle")) {
            Some(title) => Span::bright(title),
            None => {
                let workspace = text_in(terminal.get("workspace"))?;
                let tab = text_in(terminal.get("tab"));
                Span::bright(match tab {
                    Some(tab) => format!("{workspace}, {tab}"),
                    None => workspace,
                })
            }
        },
    };
    let status = match text_in(terminal.get("status")).as_deref() {
        Some("running") => Some("running".to_string()),
        Some("exited") => exit_words(terminal).map(|words| words.to_lowercase()),
        Some("not_started") => Some("not started".to_string()),
        Some("closed") => Some("closed".to_string()),
        _ => None,
    };
    let mut spans = vec![name];
    spans.extend(status.map(Span::plain));
    spans.extend(text_in(terminal.get("folder")).map(Span::dim));
    Some(OwnPart::Line(spans))
}

/// How a process ended, from its `exitCode` or `signal`: "Exit 0", "SIGKILL".
fn exit_words(output: &Value) -> Option<String> {
    match output.get("exitCode").and_then(Value::as_i64) {
        Some(code) => Some(format!("Exit {code}")),
        None => text_in(output.get("signal")),
    }
}

/// A message with code in backticks, as the server writes them, in spans. Its last period
/// goes, as a line's spans are set apart.
fn code_spans(message: &str) -> Vec<Span> {
    message
        .trim_end()
        .trim_end_matches('.')
        .split('`')
        .enumerate()
        .filter(|(_, text)| !text.trim().is_empty())
        .map(|(index, text)| {
            if index % 2 == 1 {
                Span::code(text)
            } else {
                Span::plain(text.trim())
            }
        })
        .collect()
}

/// How many lines a terminal's text has, without the empty ones at its end.
fn printed_lines(text: &str) -> usize {
    text.trim_end().lines().count()
}

/// What `agentz_terminal_send` typed: its text, then its keys, with Enter as ⏎.
fn typed(arguments: &Value) -> Option<String> {
    let key = |name: &str| match name.trim().to_ascii_lowercase().as_str() {
        "enter" | "return" => "⏎".to_string(),
        _ => name.trim().to_string(),
    };
    let mut parts: Vec<String> = text_in(arguments.get("text")).into_iter().collect();
    parts.extend(
        arguments
            .get("keys")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .filter(|name| !name.trim().is_empty())
            .map(key),
    );
    if arguments.get("submit").and_then(Value::as_bool) == Some(true) {
        parts.push("⏎".to_string());
    }
    (!parts.is_empty()).then(|| parts.join(" "))
}

/// "1 thread", "8 threads".
fn counted(count: usize, one: &str, many: &str) -> String {
    if count == 1 {
        format!("1 {one}")
    } else {
        format!("{count} {many}")
    }
}

fn capitalized(text: &str) -> String {
    let mut characters = text.chars();
    match characters.next() {
        Some(first) => first.to_uppercase().chain(characters).collect(),
        None => String::new(),
    }
}

#[derive(Debug, PartialEq)]
pub struct Subject {
    pub text: String,
    pub style: SubjectStyle,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SubjectStyle {
    /// A thread's title, brighter than the row.
    Title,
    /// A command or a branch, in the code font.
    Code,
    /// Words that stand in for a name the call didn't give, in the row's own gray.
    Plain,
}

impl Subject {
    fn title(text: String) -> Self {
        Self {
            text,
            style: SubjectStyle::Title,
        }
    }

    fn code(text: String) -> Self {
        Self {
            text,
            style: SubjectStyle::Code,
        }
    }

    fn plain(text: &str) -> Self {
        Self {
            text: text.to_string(),
            style: SubjectStyle::Plain,
        }
    }
}

/// Whether the call is an agent's ToolSearch, which loads deferred tools by name or finds them
/// by words.
fn is_tool_search(title: &str) -> bool {
    let name: String = title
        .chars()
        .filter(|character| !matches!(character, ' ' | '_' | '-'))
        .collect();
    name.eq_ignore_ascii_case("toolsearch")
}

/// A ToolSearch call: what it asked for, and the tools it named.
#[derive(Debug, PartialEq)]
pub struct ToolSearch {
    pub query: ToolQuery,
    /// The tools it loaded or found, by the names agents call them.
    pub tools: Vec<String>,
    /// Whether its output named them, so it's known what a search found.
    pub listed: bool,
}

#[derive(Debug, PartialEq)]
pub enum ToolQuery {
    /// "select:a,b": these tools, by name.
    Select(Vec<String>),
    /// Words that find tools by what they do.
    Words(String),
}

impl ToolSearch {
    pub fn of(tool_call: &ToolCall) -> Self {
        let query = raw_input(tool_call)
            .and_then(|input| text_in(input.get("query")))
            .unwrap_or_default();
        let query = match query.strip_prefix("select:") {
            Some(names) => ToolQuery::Select(
                names
                    .split(',')
                    .map(str::trim)
                    .filter(|name| !name.is_empty())
                    .map(str::to_string)
                    .collect(),
            ),
            None => ToolQuery::Words(query),
        };
        let listed = tool_call
            .text
            .iter()
            .map(|text| listed_tools(text))
            .find(|tools| !tools.is_empty())
            .unwrap_or_default();
        let is_listed = !listed.is_empty();
        let tools = match (&query, is_listed) {
            (ToolQuery::Select(names), false) => names.clone(),
            _ => listed,
        };
        Self {
            query,
            tools,
            listed: is_listed,
        }
    }

    /// "Loaded Create issue, List issues and Add comment", or "Searched tools for “subthread”".
    pub fn label(&self, state: CallState) -> String {
        match &self.query {
            ToolQuery::Select(_) => {
                let verb = state.verb("Loading", "Loaded", "Load");
                if self.tools.is_empty() {
                    return format!("{verb} tools");
                }
                let names: Vec<String> = self
                    .tools
                    .iter()
                    .map(|tool| ListedTool::named(tool).words())
                    .collect();
                format!("{verb} {}", join_words(&names))
            }
            ToolQuery::Words(words) => {
                let verb = match state {
                    CallState::Running => "Searching tools",
                    CallState::Done => "Searched tools",
                    CallState::Failed => "Search tools",
                };
                if words.is_empty() {
                    verb.to_string()
                } else {
                    format!("{verb} for “{words}”")
                }
            }
        }
    }

    /// How many tools a search by words found, once its output says.
    pub fn found(&self) -> Option<usize> {
        match self.query {
            ToolQuery::Words(_) if self.listed => Some(self.tools.len()),
            _ => None,
        }
    }

    /// The server of every tool it loaded by name, when they share one: "github", or "agentZ"
    /// for agentZ's own.
    pub fn server(&self) -> Option<String> {
        match self.query {
            ToolQuery::Select(_) => self
                .tools
                .iter()
                .map(|tool| ListedTool::named(tool).server())
                .reduce(|first, other| if first == other { first } else { None })
                .flatten(),
            ToolQuery::Words(_) => None,
        }
    }
}

/// Words joined as a sentence lists them: "A", "A and B", "A, B and C".
pub fn join_words(words: &[String]) -> String {
    match words {
        [] => String::new(),
        [only] => only.clone(),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
    }
}

/// The tool names in a ToolSearch's output, which lists them after a colon: Factory Droid's
/// "Loaded 8 tool(s): agentz___delegate_task, …".
fn listed_tools(text: &str) -> Vec<String> {
    text.lines()
        .filter_map(|line| line.split_once(": "))
        .map(|(_, list)| {
            list.split(',')
                .map(str::trim)
                .filter(|name| is_name(name))
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .find(|tools| !tools.is_empty())
        .unwrap_or_default()
}

/// A tool as an opened ToolSearch lists it, by what it does.
#[derive(Debug, PartialEq)]
pub enum ListedTool {
    Own(OwnTool),
    Mcp(McpName),
    /// The agent's own tool, by its name.
    Other(String),
}

impl ListedTool {
    pub fn named(name: &str) -> Self {
        match mcp_name(name, None) {
            Some(mcp) if mcp.server.eq_ignore_ascii_case(AGENTZ_SERVER_NAME) => {
                match OwnTool::named(&mcp.tool) {
                    Some(tool) => Self::Own(tool),
                    None => Self::Mcp(mcp),
                }
            }
            Some(mcp) => Self::Mcp(mcp),
            None => match own_tool_in(name) {
                Some(tool) => Self::Own(tool),
                None => Self::Other(name.to_string()),
            },
        }
    }

    fn server(&self) -> Option<String> {
        match self {
            Self::Own(_) => Some("agentZ".to_string()),
            Self::Mcp(mcp) => Some(mcp.server.clone()),
            Self::Other(_) => None,
        }
    }

    /// What the tool does: agentZ's by its title, other MCP tools in words, and the agent's
    /// own by name.
    pub fn words(&self) -> String {
        match self {
            Self::Own(tool) => tool.title().to_string(),
            Self::Mcp(mcp) => mcp.words(),
            Self::Other(name) => name.clone(),
        }
    }
}

/// What one of the agent's own tools did, by what most agents' tools share
/// (`design/tool-calls-2/decisions.md`), so its row says it in the same words for any agent.
/// Edits and commands keep their own rows.
#[derive(Clone, Debug, PartialEq)]
pub enum AgentAction {
    /// A to-do list or task update: the plan bar shows the list, so it has no row.
    Todo,
    /// A read, with the lines it got when known ("lines 1–120", "18 lines").
    Read {
        path: Option<PathBuf>,
        lines: Option<String>,
    },
    /// A new file: the one diff it made has no old text.
    Created {
        path: PathBuf,
        lines: usize,
    },
    Deleted {
        path: Option<PathBuf>,
        lines: Option<usize>,
    },
    Moved {
        from: PathBuf,
        to: PathBuf,
    },
    Grep(Grep),
    Glob(Glob),
    Fetch {
        url: String,
    },
    WebSearch {
        query: Option<String>,
    },
    Skill {
        name: String,
    },
    Findings {
        count: Option<usize>,
    },
    /// A question the agent asked the user.
    Question {
        question: Option<String>,
    },
    /// Anything else, by its kind and the agent's title.
    Other,
}

/// A search of files' contents.
#[derive(Clone, Debug, PartialEq)]
pub struct Grep {
    pub pattern: String,
    pub path: Option<String>,
    pub ignore_case: bool,
}

/// A search of files' names.
#[derive(Clone, Debug, PartialEq)]
pub struct Glob {
    pub pattern: String,
    pub path: Option<String>,
}

/// To-do and task tools, by their names in words run together: Claude Agent's TodoWrite and
/// Task*, Codex's update_plan, Gemini CLI's write_todos.
const TODO_TOOLS: [&str; 8] = [
    "todowrite",
    "todoread",
    "taskcreate",
    "taskupdate",
    "tasklist",
    "taskget",
    "updateplan",
    "writetodos",
];

/// Claude Agent's titles for its to-do and task tools.
const TODO_TITLES: [&str; 6] = [
    "Update TODOs",
    "Update Todos",
    "Create task:",
    "Update task:",
    "List tasks",
    "Get task",
];

impl AgentAction {
    pub fn of(tool_call: &ToolCall) -> Self {
        let title = tool_call.title.trim();
        let input = raw_input(tool_call);
        let input_text = |keys: &[&str]| {
            keys.iter()
                .find_map(|key| text_in(input.as_ref().and_then(|input| input.get(*key))))
        };
        if is_hidden(tool_call) {
            return Self::Todo;
        }
        let name = tool_name(tool_call);
        let is_think = matches!(tool_call.kind, acp::ToolKind::Think | acp::ToolKind::Other);
        if name.as_deref() == Some("skill")
            || title.starts_with("Load skill:")
            || title.starts_with("Skill:")
        {
            let skill = input_text(&["skill", "name", "command"]).or_else(|| {
                title
                    .split_once(':')
                    .map(|(_, name)| name.trim().to_string())
                    .filter(|name| !name.is_empty())
            });
            if let Some(name) = skill {
                return Self::Skill { name };
            }
        }
        if name.as_deref() == Some("reportfindings")
            || (title.starts_with("Report ") && title.contains("finding"))
        {
            let count = input
                .as_ref()
                .and_then(|input| input.get("findings"))
                .and_then(Value::as_array)
                .map(Vec::len)
                .or_else(|| title.contains("none found").then_some(0))
                .or_else(|| first_number(title));
            return Self::Findings { count };
        }
        let questions = input
            .as_ref()
            .and_then(|input| input.get("questions"))
            .and_then(Value::as_array);
        if matches!(
            name.as_deref(),
            Some("askuserquestion" | "askuser" | "askfollowupquestion")
        ) || (is_think && questions.is_some())
        {
            let question = questions
                .and_then(|questions| questions.first())
                .and_then(|question| text_in(question.get("question")))
                .or_else(|| input_text(&["question"]))
                .or_else(|| {
                    (title != "Asking for your input" && name.is_none() && !title.is_empty())
                        .then(|| title.to_string())
                });
            return Self::Question { question };
        }

        match tool_call.kind {
            acp::ToolKind::Read => Self::Read {
                path: file_path(tool_call),
                lines: read_lines(tool_call),
            },
            acp::ToolKind::Delete => Self::Deleted {
                path: tool_call
                    .locations
                    .first()
                    .cloned()
                    .or_else(|| tool_call.diffs.first().map(|diff| diff.path.clone()))
                    .or_else(|| input_text(&["file_path", "path"]).map(PathBuf::from)),
                lines: tool_call
                    .diffs
                    .first()
                    .and_then(|diff| diff.old_text.as_ref())
                    .map(|text| text.lines().count()),
            },
            acp::ToolKind::Move => match moved_paths(tool_call, input.as_ref()) {
                Some((from, to)) => Self::Moved { from, to },
                None => Self::Other,
            },
            acp::ToolKind::Search => search_of(tool_call, input.as_ref(), name.as_deref()),
            acp::ToolKind::Fetch => {
                let is_web_search = matches!(
                    name.as_deref(),
                    Some("websearch" | "googlewebsearch" | "searchweb")
                ) || title.starts_with("Search \"")
                    || title.starts_with("Web search")
                    || title.starts_with("Searching the web")
                    || title.starts_with("Searching the Web")
                    || (input_text(&["query"]).is_some() && input_text(&["url"]).is_none());
                if is_web_search {
                    let query = input_text(&["query"]).or_else(|| {
                        let quoted = title
                            .strip_prefix("Search \"")
                            .and_then(|rest| rest.strip_suffix('"'));
                        let named = title
                            .strip_prefix("Web search:")
                            .map(str::trim)
                            .filter(|query| !query.is_empty());
                        quoted.or(named).map(str::to_string)
                    });
                    return Self::WebSearch { query };
                }
                match input_text(&["url"]).or_else(|| url_in(title)) {
                    Some(url) => Self::Fetch { url },
                    None => Self::Other,
                }
            }
            _ => match tool_call.diffs.as_slice() {
                [diff] if diff.old_text.is_none() => Self::Created {
                    path: diff.path.clone(),
                    lines: diff.new_text.lines().count(),
                },
                _ => Self::Other,
            },
        }
    }

    /// What its row says, with what it acted on, and dim words after them; `None` for those
    /// whose row is drawn as before: edits, commands and tools nothing knows.
    pub fn label(
        &self,
        tool_call: &ToolCall,
        state: CallState,
        folder: Option<&Path>,
    ) -> Option<ActionLabel> {
        let shown = |path: &Path| display_path(&path.to_string_lossy(), folder);
        let label = match self {
            Self::Todo | Self::Other => return None,
            Self::Read { path, lines } => ActionLabel {
                parts: vec![
                    LabelPart::Words(state.verb("Reading", "Read", "Read").into()),
                    LabelPart::Name(match path {
                        Some(path) => shown(path),
                        None => without_read_range(tool_call.title.trim())
                            .trim_start_matches("Read ")
                            .to_string(),
                    }),
                ],
                detail: lines.clone(),
            },
            Self::Created { path, .. } => ActionLabel {
                parts: vec![
                    LabelPart::Words(state.verb("Creating", "Created", "Create").into()),
                    LabelPart::Code(shown(path)),
                ],
                detail: None,
            },
            Self::Deleted { path, .. } => ActionLabel {
                parts: vec![
                    LabelPart::Words(state.verb("Deleting", "Deleted", "Delete").into()),
                    match path {
                        Some(path) => LabelPart::Code(shown(path)),
                        None => LabelPart::Name(tool_call.title.trim().to_string()),
                    },
                ],
                detail: None,
            },
            Self::Moved { from, to } => ActionLabel {
                parts: vec![
                    LabelPart::Words(state.verb("Moving", "Moved", "Move").into()),
                    LabelPart::Code(shown(from)),
                    LabelPart::Words("→".into()),
                    LabelPart::Code(shown(to)),
                ],
                detail: None,
            },
            Self::Grep(grep) => {
                let mut parts = vec![
                    LabelPart::Words(
                        state
                            .verb("Searching for", "Searched for", "Search for")
                            .into(),
                    ),
                    LabelPart::Code(grep.pattern.clone()),
                ];
                if let Some(path) = &grep.path {
                    parts.push(LabelPart::Words("in".into()));
                    parts.push(LabelPart::Name(display_path(path, folder)));
                }
                let detail = (state == CallState::Done)
                    .then(|| output_text(tool_call))
                    .flatten()
                    .and_then(|text| grep_files(&text, grep.path.as_deref()))
                    .map(|files| grep_count(&files));
                ActionLabel { parts, detail }
            }
            Self::Glob(glob) => {
                let found = (state == CallState::Done)
                    .then(|| output_text(tool_call))
                    .flatten()
                    .map(|text| glob_paths(&text).len());
                let verb = match (state, found) {
                    (CallState::Running, _) => "Finding files for".to_string(),
                    (CallState::Failed, _) => "Find files for".to_string(),
                    (CallState::Done, Some(0)) => "Found no files for".to_string(),
                    (CallState::Done, Some(1)) => "Found 1 file for".to_string(),
                    (CallState::Done, Some(count)) => format!("Found {count} files for"),
                    (CallState::Done, None) => "Found files for".to_string(),
                };
                ActionLabel {
                    parts: vec![
                        LabelPart::Words(verb),
                        LabelPart::Code(glob.pattern.clone()),
                    ],
                    detail: None,
                }
            }
            Self::Fetch { url } => ActionLabel {
                parts: vec![
                    LabelPart::Words(state.verb("Fetching", "Fetched", "Fetch").into()),
                    LabelPart::Link {
                        text: without_scheme(url),
                        url: url.clone(),
                    },
                ],
                detail: None,
            },
            Self::WebSearch { query } => {
                let verb = state.verb("Searching the web", "Searched the web", "Search the web");
                ActionLabel {
                    parts: match query {
                        Some(query) => vec![
                            LabelPart::Words(format!("{verb} for")),
                            LabelPart::Name(format!("“{query}”")),
                        ],
                        None => vec![LabelPart::Words(verb.into())],
                    },
                    detail: None,
                }
            }
            Self::Skill { name } => ActionLabel {
                parts: vec![
                    LabelPart::Words(state.verb("Loading the", "Loaded the", "Load the").into()),
                    LabelPart::Title(name.clone()),
                    LabelPart::Words("skill".into()),
                ],
                detail: None,
            },
            Self::Findings { count } => ActionLabel {
                parts: vec![LabelPart::Words(match (state, count) {
                    (CallState::Running, _) => "Reporting findings".to_string(),
                    (CallState::Failed, _) => "Report findings".to_string(),
                    (CallState::Done, Some(0)) => "Reported no findings".to_string(),
                    (CallState::Done, Some(1)) => "Reported 1 finding".to_string(),
                    (CallState::Done, Some(count)) => format!("Reported {count} findings"),
                    (CallState::Done, None) => "Reported findings".to_string(),
                })],
                detail: None,
            },
            Self::Question { question } => {
                let mut parts = vec![LabelPart::Words(
                    state.verb("Asking you", "Asked you", "Ask you").into(),
                )];
                parts.extend(question.clone().map(LabelPart::Name));
                ActionLabel {
                    parts,
                    detail: None,
                }
            }
        };
        Some(label)
    }
}

/// A row's words, in pieces drawn each its own way.
#[derive(Clone, Debug, PartialEq)]
pub struct ActionLabel {
    pub parts: Vec<LabelPart>,
    /// Dim words after them: "lines 1–120", "4 matches in 3 files".
    pub detail: Option<String>,
}

impl ActionLabel {
    /// The label as plain text, for tests.
    #[cfg(test)]
    fn text(&self) -> String {
        self.parts
            .iter()
            .map(|part| match part {
                LabelPart::Words(text)
                | LabelPart::Name(text)
                | LabelPart::Code(text)
                | LabelPart::Title(text) => text.as_str(),
                LabelPart::Link { text, .. } => text.as_str(),
            })
            .collect::<Vec<_>>()
            .join(" ")
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum LabelPart {
    /// Words in the row's gray, never cut short.
    Words(String),
    /// A name in the row's gray, cut short with "…" when it doesn't fit.
    Name(String),
    /// A path or a pattern, in the code font.
    Code(String),
    /// What it acted on, in the brighter gray.
    Title(String),
    /// An address that opens in the browser.
    Link { text: String, url: String },
}

/// The agent's own name for the tool, in lowercase words run together ("todowrite"): Claude
/// Agent's from its metadata, or a title that is one name.
fn tool_name(tool_call: &ToolCall) -> Option<String> {
    let name = match tool_call.tool_name.as_deref() {
        Some(name) => name,
        None => {
            let title = tool_call.title.trim();
            if title.is_empty() || !is_name(title) {
                return None;
            }
            title
        }
    };
    Some(
        name.chars()
            .filter(|character| !matches!(character, '_' | '-' | ' ' | '.'))
            .flat_map(char::to_lowercase)
            .collect(),
    )
}

/// Whether the call is a to-do or task update, which has no row: told apart by its name or
/// title alone, since every run's rows ask.
pub fn is_hidden(tool_call: &ToolCall) -> bool {
    if !matches!(tool_call.kind, acp::ToolKind::Think | acp::ToolKind::Other) {
        return false;
    }
    let title = tool_call.title.trim();
    TODO_TITLES.iter().any(|prefix| title.starts_with(prefix))
        || tool_name(tool_call).is_some_and(|name| TODO_TOOLS.contains(&name.as_str()))
}

/// A path as the row shows it: inside the thread's folder, from there.
pub fn display_path(path: &str, folder: Option<&Path>) -> String {
    let Some(folder) = folder else {
        return path.to_string();
    };
    match Path::new(path).strip_prefix(folder) {
        Ok(relative) if !relative.as_os_str().is_empty() => relative.to_string_lossy().into_owned(),
        _ => path.to_string(),
    }
}

/// The lines a read got: the range Claude Agent puts in its title ("(1 - 120)", "(from line
/// 40)"), or else how many lines came back.
fn read_lines(tool_call: &ToolCall) -> Option<String> {
    let title = tool_call.title.trim();
    if let Some(range) = title
        .strip_suffix(')')
        .and_then(|rest| rest.rsplit_once(" ("))
        .map(|(_, range)| range)
    {
        if let Some((start, end)) = range.split_once(" - ")
            && let (Ok(start), Ok(end)) = (start.trim().parse::<u32>(), end.trim().parse::<u32>())
        {
            return Some(format!("lines {start}–{end}"));
        }
        if let Some(start) = range
            .strip_prefix("from line ")
            .and_then(|start| start.trim().parse::<u32>().ok())
        {
            return Some(format!("from line {start}"));
        }
    }
    if !tool_call.images.is_empty() || !matches!(tool_call.status, acp::ToolCallStatus::Completed) {
        return None;
    }
    let text = output_text(tool_call)?;
    let count = text
        .lines()
        .filter(|line| !line.starts_with("[File truncated"))
        .count();
    Some(match count {
        1 => "1 line".to_string(),
        count => format!("{count} lines"),
    })
}

/// A read's title without the range Claude Agent adds after its path.
fn without_read_range(title: &str) -> &str {
    match title.rsplit_once(" (") {
        Some((rest, range))
            if range.ends_with(')')
                && range.chars().any(|character| character.is_ascii_digit()) =>
        {
            rest
        }
        _ => title,
    }
}

/// Where a move went from and to: its input's two paths, its two places, or its title's.
fn moved_paths(tool_call: &ToolCall, input: Option<&Value>) -> Option<(PathBuf, PathBuf)> {
    let pairs = [
        ("source", "destination"),
        ("from", "to"),
        ("old_path", "new_path"),
        ("source_path", "destination_path"),
        ("src", "dst"),
    ];
    if let Some(input) = input {
        for (from, to) in pairs {
            if let (Some(from), Some(to)) = (text_in(input.get(from)), text_in(input.get(to))) {
                return Some((from.into(), to.into()));
            }
        }
    }
    if let [from, to] = tool_call.locations.as_slice() {
        return Some((from.clone(), to.clone()));
    }
    let title = tool_call.title.trim();
    let rest = title
        .strip_prefix("Move ")
        .or_else(|| title.strip_prefix("Rename "))?;
    let (from, to) = rest
        .split_once(" → ")
        .or_else(|| rest.split_once(" -> "))
        .or_else(|| rest.split_once(" to "))?;
    let unquote = |path: &str| path.trim().trim_matches(['`', '\'', '"']).to_string();
    Some((unquote(from).into(), unquote(to).into()))
}

/// A search, as a grep or a glob, from its input or each agent's title: Claude Agent's `grep
/// -i "pattern" path` and "Find `path` `pattern`", Codex's "Search for 'pattern' in path",
/// Factory Droid's "Grep pattern in path".
fn search_of(tool_call: &ToolCall, input: Option<&Value>, name: Option<&str>) -> AgentAction {
    let title = tool_call.title.trim();
    let input_text = |keys: &[&str]| {
        keys.iter()
            .find_map(|key| text_in(input.and_then(|input| input.get(*key))))
    };
    let backticked: Vec<&str> = title.split('`').skip(1).step_by(2).collect();
    let is_glob = matches!(name, Some("glob" | "globfiles" | "findfiles"))
        || title.starts_with("Find `")
        || title.starts_with("Glob ");
    if is_glob {
        let pattern = input_text(&["pattern", "glob", "glob_pattern"])
            .or_else(|| backticked.last().map(|pattern| pattern.to_string()))
            .or_else(|| title.strip_prefix("Glob ").map(str::to_string));
        let path = input_text(&["path", "dir_path", "directory", "folder"])
            .or_else(|| (backticked.len() >= 2).then(|| backticked[0].to_string()));
        return match pattern {
            Some(pattern) => AgentAction::Glob(Glob { pattern, path }),
            None => AgentAction::Other,
        };
    }
    let ignore_case = input.is_some_and(|input| {
        ["-i", "case_insensitive", "ignore_case", "ignoreCase"]
            .iter()
            .any(|key| input.get(*key).and_then(Value::as_bool) == Some(true))
    }) || title.starts_with("grep -i")
        || title.contains(" -i ");
    let pattern = input_text(&["pattern", "regex", "query"]);
    let path = input_text(&["path", "file_path", "dir_path", "directory"]);
    let (pattern, path) = match pattern {
        Some(pattern) => (Some(pattern), path),
        None => match grep_title(title) {
            Some((pattern, title_path)) => (Some(pattern), path.or(title_path)),
            None => (None, path),
        },
    };
    match pattern {
        Some(pattern) => AgentAction::Grep(Grep {
            pattern,
            path,
            ignore_case,
        }),
        None => AgentAction::Other,
    }
}

/// The pattern and path in a search's title.
fn grep_title(title: &str) -> Option<(String, Option<String>)> {
    let some_path = |path: &str| {
        let path = path.trim().trim_matches(['\'', '"', '`']);
        (!path.is_empty()).then(|| path.to_string())
    };
    if title.starts_with("grep ") {
        let (_, quoted) = title.split_once('"')?;
        let (pattern, rest) = quoted.rsplit_once('"')?;
        return Some((pattern.replace("\\\"", "\""), some_path(rest)));
    }
    let rest = title
        .strip_prefix("Search for ")
        .or_else(|| title.strip_prefix("Search "))
        .or_else(|| title.strip_prefix("Grep "))?;
    let (pattern, path) = match rest.rsplit_once(" in ") {
        Some((pattern, path)) => (pattern, some_path(path)),
        None => (rest, None),
    };
    let pattern = pattern.trim().trim_matches(['\'', '"', '`']);
    (!pattern.is_empty()).then(|| (pattern.to_string(), path))
}

/// The tool's output, as one text, outside any code fence it came in.
pub fn output_text(tool_call: &ToolCall) -> Option<String> {
    let texts: Vec<&str> = tool_call
        .text
        .iter()
        .map(|text| unfenced(text))
        .filter(|text| !text.trim().is_empty())
        .collect();
    (!texts.is_empty()).then(|| texts.join("\n"))
}

/// Text outside the code fence around it, when it's all one fenced block.
pub fn unfenced(text: &str) -> &str {
    let trimmed = text.trim();
    let fence_length = trimmed.len() - trimmed.trim_start_matches('`').len();
    if fence_length < 3 {
        return text;
    }
    let Some((_, body)) = trimmed.split_once('\n') else {
        return text;
    };
    let body = body.trim_end();
    let Some(body) = body.strip_suffix(&trimmed[..fence_length]) else {
        return text;
    };
    body.strip_suffix('\n').unwrap_or(body)
}

/// One file's matches in a grep's output.
#[derive(Clone, Debug, PartialEq)]
pub struct GrepFile {
    pub path: String,
    pub lines: Vec<GrepLine>,
    /// How many matches the output counted, when it only counted them.
    pub count: Option<usize>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct GrepLine {
    pub number: u32,
    pub text: String,
}

impl GrepFile {
    pub fn matches(&self) -> usize {
        self.count.unwrap_or(self.lines.len())
    }
}

/// The files and lines a grep's output names, as grep and ripgrep print them (`path:line:text`,
/// `path:count`, or paths alone), or `None` when the output is something else, which then shows
/// as printed. `searched` is the file a search of one file printed bare `line:text` lines for.
pub fn grep_files(text: &str, searched: Option<&str>) -> Option<Vec<GrepFile>> {
    fn file(files: &mut Vec<GrepFile>, path: &str) -> usize {
        match files.iter().position(|file| file.path == path) {
            Some(index) => index,
            None => {
                files.push(GrepFile {
                    path: path.to_string(),
                    lines: Vec::new(),
                    count: None,
                });
                files.len() - 1
            }
        }
    }
    let mut files: Vec<GrepFile> = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty()
            || trimmed == "--"
            || trimmed.starts_with('[')
            || trimmed.starts_with("Found ")
            || trimmed.starts_with("No matches")
            || trimmed.starts_with("No files")
        {
            continue;
        }
        if let Some((path, rest)) = line.split_once(':') {
            if let Some((number, text)) = rest.split_once(':')
                && let Ok(number) = number.parse::<u32>()
                && !path.is_empty()
            {
                let index = file(&mut files, path);
                files[index].lines.push(GrepLine {
                    number,
                    text: text.to_string(),
                });
                continue;
            }
            if let Ok(number) = path.parse::<u32>()
                && let Some(searched) = searched
            {
                let index = file(&mut files, searched);
                files[index].lines.push(GrepLine {
                    number,
                    text: rest.to_string(),
                });
                continue;
            }
            if let Ok(count) = rest.trim().parse::<usize>() {
                let index = file(&mut files, path);
                files[index].count = Some(count);
                continue;
            }
        }
        // A context line (`path-line-text`) of a search with -A, -B or -C.
        if files.iter().any(|file| {
            line.strip_prefix(file.path.as_str())
                .and_then(|rest| rest.strip_prefix('-'))
                .and_then(|rest| rest.split_once('-'))
                .is_some_and(|(number, _)| number.parse::<u32>().is_ok())
        }) {
            continue;
        }
        if line.contains('\t') || line.starts_with(' ') || line.contains(": ") {
            return None;
        }
        file(&mut files, trimmed);
    }
    Some(files)
}

/// "4 matches in 3 files", "3 files" when only files were listed, or "No matches".
pub fn grep_count(files: &[GrepFile]) -> String {
    let plural = |count: usize, one: &str, many: &str| {
        format!("{count} {}", if count == 1 { one } else { many })
    };
    if files.is_empty() {
        return "No matches".to_string();
    }
    let file_count = plural(files.len(), "file", "files");
    if files
        .iter()
        .all(|file| file.lines.is_empty() && file.count.is_none())
    {
        return file_count;
    }
    let matches: usize = files.iter().map(GrepFile::matches).sum();
    format!("{} in {file_count}", plural(matches, "match", "matches"))
}

/// The paths a glob's output lists, one a line.
pub fn glob_paths(text: &str) -> Vec<String> {
    text.lines()
        .map(str::trim)
        .filter(|line| {
            !line.is_empty()
                && !line.starts_with('[')
                && !line.starts_with('(')
                && !line.starts_with("No files")
                && !line.starts_with("Found ")
        })
        .map(str::to_string)
        .collect()
}

/// A web search's result: a page's title and address.
#[derive(Clone, Debug, PartialEq)]
pub struct WebHit {
    pub title: String,
    pub url: String,
}

/// The pages a web search's output names: Claude Agent's "Title (url)" lines, markdown links,
/// or JSON with titles and addresses ("Links: [{"title": …, "url": …}]").
pub fn web_hits(text: &str) -> Vec<WebHit> {
    fn push(hits: &mut Vec<WebHit>, title: &str, url: &str) {
        let url = url.trim();
        if (url.starts_with("http://") || url.starts_with("https://"))
            && !hits.iter().any(|hit| hit.url == url)
        {
            let title = title.trim();
            hits.push(WebHit {
                title: if title.is_empty() {
                    without_scheme(url)
                } else {
                    title.to_string()
                },
                url: url.to_string(),
            });
        }
    }
    fn json_hits(value: &Value, hits: &mut Vec<WebHit>) {
        match value {
            Value::Array(values) => values.iter().for_each(|value| json_hits(value, hits)),
            Value::Object(object) => {
                if let Some(url) = object.get("url").and_then(Value::as_str) {
                    let title = object.get("title").and_then(Value::as_str).unwrap_or("");
                    push(hits, title, url);
                } else {
                    object.values().for_each(|value| json_hits(value, hits));
                }
            }
            _ => {}
        }
    }
    let mut hits = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        let line = line
            .strip_prefix("- ")
            .or_else(|| line.strip_prefix("* "))
            .unwrap_or(line);
        if let Some(json) = line.strip_prefix("Links: ")
            && let Ok(value) = serde_json::from_str::<Value>(json)
        {
            json_hits(&value, &mut hits);
            continue;
        }
        if let Some(rest) = line.strip_prefix('[')
            && let Some((title, rest)) = rest.split_once("](")
            && let Some((url, _)) = rest.split_once(')')
        {
            push(&mut hits, title, url);
            continue;
        }
        if let Some(rest) = line.strip_suffix(')')
            && let Some((title, url)) = rest.rsplit_once(" (")
        {
            push(&mut hits, title, url);
        }
    }
    if hits.is_empty()
        && let Some(value) = json_in(text)
    {
        json_hits(&value, &mut hits);
    }
    hits
}

/// An address without its scheme or its last slash, as a row shows it.
pub fn without_scheme(url: &str) -> String {
    let rest = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))
        .unwrap_or(url);
    rest.trim_end_matches('/').to_string()
}

/// The first web address in a text, as in Claude Agent's "Fetch https://… (from char 40)".
fn url_in(text: &str) -> Option<String> {
    let start = text.find("https://").or_else(|| text.find("http://"))?;
    let url = text[start..].split_whitespace().next()?;
    Some(url.to_string())
}

fn first_number(text: &str) -> Option<usize> {
    text.split(|character: char| !character.is_ascii_digit())
        .find(|digits| !digits.is_empty())
        .and_then(|digits| digits.parse().ok())
}

/// How a command ended, when its output says: Claude Agent's "Exit code 1", or "Process exited
/// with code 1" as agentZ's and Factory Droid's commands print it.
pub fn exit_code(tool_call: &ToolCall) -> Option<i32> {
    tool_call.text.iter().rev().find_map(|text| {
        text.lines().rev().find_map(|line| {
            let line = line.trim().trim_start_matches('[').trim_end_matches(']');
            let lowered = line.to_ascii_lowercase();
            let rest = lowered
                .strip_prefix("exit code")
                .or_else(|| lowered.strip_prefix("process exited with code"))?;
            rest.trim_start_matches([':', ' ']).trim().parse().ok()
        })
    })
}

/// Where a command ran, when its input says.
pub fn command_folder(tool_call: &ToolCall) -> Option<String> {
    let input = raw_input(tool_call)?;
    ["cwd", "workdir", "working_directory", "dir"]
        .iter()
        .find_map(|key| text_in(input.get(*key)))
}

/// The first short text an MCP tool was given, which is usually what it acted on: an issue's
/// title, a query.
pub fn mcp_subject(tool_call: &ToolCall) -> Option<String> {
    const SHORT: usize = 60;
    let codex_input = tool_call
        .title
        .trim()
        .starts_with("Tool: ")
        .then(|| raw_input(tool_call))
        .flatten();
    let arguments = tool_arguments(codex_input.or_else(|| raw_input(tool_call)));
    arguments.as_object()?.values().find_map(|value| {
        let text = value.as_str()?.trim();
        (!text.is_empty() && !text.contains('\n') && text.chars().count() <= SHORT)
            .then(|| text.to_string())
    })
}

/// A size on disk as people read it: "212 KB", "1.4 MB".
pub fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * 1024;
    if bytes < KB {
        format!("{bytes} B")
    } else if bytes < MB {
        format!("{} KB", (bytes + KB / 2) / KB)
    } else {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    }
}

/// The file a read or an edit is of, for its icon: the one place it names, or the one file it
/// changed, or for a read, the path in its input.
pub fn file_path(tool_call: &ToolCall) -> Option<PathBuf> {
    let is_read = matches!(tool_call.kind, acp::ToolKind::Read);
    let is_edit = matches!(tool_call.kind, acp::ToolKind::Edit) || !tool_call.diffs.is_empty();
    if !is_read && !is_edit {
        return None;
    }
    if let [location] = tool_call.locations.as_slice() {
        return Some(location.clone());
    }
    if let [diff] = tool_call.diffs.as_slice() {
        return Some(diff.path.clone());
    }
    if !is_read || tool_call.diffs.len() > 1 || tool_call.locations.len() > 1 {
        return None;
    }
    let input = raw_input(tool_call)?;
    ["file_path", "path", "filePath", "absolute_path"]
        .iter()
        .find_map(|key| text_in(input.get(*key)))
        .map(PathBuf::from)
}

/// The icon Zed's file icons give the file's type (`file_icons::FileIcons::get_icon`): by its
/// whole name, then each suffix after a dot, then its extensions, then its extension or hidden
/// name, else the default file's.
pub fn file_icon(path: &Path, cx: &App) -> Option<SharedString> {
    let icon_theme = GlobalTheme::icon_theme(cx);
    let default_theme = theme::default_icon_theme();
    let icon_for_suffix = |suffix: &str| -> Option<SharedString> {
        let kind = icon_theme
            .file_stems
            .get(suffix)
            .or_else(|| icon_theme.file_suffixes.get(suffix))?;
        icon_for_kind(kind, icon_theme, &default_theme)
    };
    if let Some(mut name) = path.file_name().and_then(|name| name.to_str()) {
        if let Some(icon) = icon_for_suffix(name) {
            return Some(icon);
        }
        while let Some((_, suffix)) = name.split_once('.') {
            if let Some(icon) = icon_for_suffix(suffix) {
                return Some(icon);
            }
            name = suffix;
        }
    }
    if let Some(icon) = path
        .multiple_extensions()
        .and_then(|extensions| icon_for_suffix(&extensions))
    {
        return Some(icon);
    }
    if let Some(icon) = path
        .extension_or_hidden_file_name()
        .and_then(icon_for_suffix)
    {
        return Some(icon);
    }
    if let Some(icon) = path
        .extension()
        .and_then(|extension| extension.to_str())
        .and_then(icon_for_suffix)
    {
        return Some(icon);
    }
    icon_for_kind("default", icon_theme, &default_theme)
}

fn icon_for_kind(kind: &str, icon_theme: &IconTheme, default: &IconTheme) -> Option<SharedString> {
    icon_theme
        .file_icons
        .get(kind)
        .or_else(|| default.file_icons.get(kind))
        .map(|icon| icon.path.clone())
}

/// The tool call's input as JSON: agent_thread keeps it as markdown, a fenced JSON block.
fn raw_input(tool_call: &ToolCall) -> Option<Value> {
    tool_call.raw_input.as_deref().and_then(json_in)
}

/// The JSON in a tool's input or output text, which agents may fence as code.
fn json_in(text: &str) -> Option<Value> {
    let text = text.trim();
    let text = match text.strip_prefix("```") {
        Some(fenced) => {
            let (_, body) = fenced.split_once('\n')?;
            body.trim_end().trim_end_matches('`')
        }
        None => text,
    };
    serde_json::from_str::<Value>(text)
        .ok()
        .filter(|value| value.is_object() || value.is_array())
}

/// What the agent passed the tool: Codex's input holds it under `arguments`, beside the server
/// and tool.
fn tool_arguments(input: Option<Value>) -> Value {
    match input {
        Some(Value::Object(mut input))
            if input.contains_key("server") && input.contains_key("tool") =>
        {
            match input.remove("arguments") {
                Some(Value::String(text)) => json_in(&text).unwrap_or(Value::Null),
                Some(arguments) => arguments,
                None => Value::Null,
            }
        }
        Some(input) => input,
        None => Value::Null,
    }
}

fn text_in(value: Option<&Value>) -> Option<String> {
    value
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(str::to_string)
}

/// Thread ids are numbers, but agents often quote them.
fn thread_id_in(value: Option<&Value>) -> Option<ThreadId> {
    match value? {
        Value::Number(number) => number.as_u64().map(ThreadId),
        Value::String(text) => text.trim().parse().ok().map(ThreadId),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn tool_call(title: &str, raw_input: Option<Value>, output: Option<&str>) -> ToolCall {
        ToolCall {
            text: output.into_iter().map(str::to_string).collect(),
            raw_input: raw_input.map(|input| {
                format!(
                    "```json\n{}\n```",
                    serde_json::to_string_pretty(&input).unwrap_or_default()
                )
            }),
            ..ToolCall::new(
                acp::ToolCallId::new("call"),
                title.to_string(),
                acp::ToolKind::Other,
                acp::ToolCallStatus::Completed,
            )
        }
    }

    #[test]
    fn the_tools_are_the_protocols_in_order() {
        for (position, tool) in OwnTool::ALL.iter().enumerate() {
            assert_eq!(*tool as usize, position);
            assert_eq!(OwnTool::named(AGENTZ_TOOLS[position].0), Some(*tool));
        }
    }

    #[test]
    fn every_agents_spelling_of_an_mcp_tool_reads_the_same() {
        let github = Some(McpName {
            server: "github".into(),
            tool: "create_issue".into(),
        });
        // Factory Droid, Claude Agent, Gemini CLI, Qwen Code.
        assert_eq!(mcp_name("github___create_issue", None), github);
        assert_eq!(mcp_name("mcp__github__create_issue", None), github);
        assert_eq!(mcp_name("create_issue (github MCP Server)", None), github);
        assert_eq!(
            mcp_name(r#"create_issue (github MCP Server): {"title": "x"}"#, None),
            github
        );
        // Codex, by its title or its input.
        assert_eq!(mcp_name("Tool: github/create_issue", None), github);
        let codex_input = json!({"server": "github", "tool": "create_issue", "arguments": {}});
        assert_eq!(
            mcp_name("Tool: github/create_issue", Some(&codex_input)),
            github
        );
        assert_eq!(
            mcp_name("mcp__linear__list_issues", None),
            Some(McpName {
                server: "linear".into(),
                tool: "list_issues".into(),
            })
        );
        // An agent's own tools and titles aren't MCP tools.
        assert_eq!(mcp_name("Read src/main.rs", None), None);
        assert_eq!(mcp_name("ToolSearch", None), None);
        assert_eq!(mcp_name("npm run dev (in the background)", None), None);

        assert_eq!(tool_words("create_issue"), "Create issue");
        assert_eq!(tool_words("get-library-docs"), "Get library docs");
        assert_eq!(tool_words("list__issues"), "List issues");
    }

    #[test]
    fn agentzs_tools_are_known_in_every_spelling() {
        for title in [
            "agentz___delegate_task",
            "mcp__agentz__delegate_task",
            "delegate_task (agentz MCP Server)",
            "agentz_delegate_task",
            "agentz-delegate_task",
            "delegate_task",
        ] {
            assert!(
                matches!(
                    ToolCallKind::of(&tool_call(title, None, None)),
                    ToolCallKind::Own {
                        tool: OwnTool::DelegateTask,
                        ..
                    }
                ),
                "{title}"
            );
        }
        // OpenCode joins the server and a tool named agentz_… with one more underscore.
        assert!(matches!(
            ToolCallKind::of(&tool_call("agentz_agentz_thread_list", None, None)),
            ToolCallKind::Own {
                tool: OwnTool::ThreadList,
                ..
            }
        ));
        // Codex passes the arguments inside its input.
        let codex = tool_call(
            "Tool: agentz/agentz_terminal_start",
            Some(json!({"server": "agentz", "tool": "agentz_terminal_start",
                        "arguments": {"command": "npm run dev"}})),
            None,
        );
        match ToolCallKind::of(&codex) {
            ToolCallKind::Own {
                tool, arguments, ..
            } => {
                assert_eq!(tool, OwnTool::TerminalStart);
                assert_eq!(arguments, json!({"command": "npm run dev"}));
            }
            _ => panic!("Codex's call to agentZ's tool"),
        }
        assert!(matches!(
            ToolCallKind::of(&tool_call("github___create_issue", None, None)),
            ToolCallKind::Mcp(_)
        ));
        assert!(matches!(
            ToolCallKind::of(&tool_call("Read README.md", None, None)),
            ToolCallKind::Plain
        ));
    }

    fn sentence_text(sentence: &Sentence) -> String {
        let mut text = match &sentence.subject {
            Some(subject) => format!("{} {}", sentence.verb, subject.text),
            None => sentence.verb.clone(),
        };
        if let Some((joint, object)) = &sentence.object {
            text.push_str(&format!(" {joint} {}", object.text));
        }
        text
    }

    #[test]
    fn agentzs_tools_say_what_they_did() {
        let titles = |thread_id: ThreadId| match thread_id.0 {
            12 => Some("Research: UI for child tasks".to_string()),
            20 => Some("npm run dev".to_string()),
            _ => None,
        };
        let say = |tool: OwnTool, arguments: Value, output: Option<Value>, state| {
            tool.sentence(&arguments, output.as_ref(), state, &titles)
        };
        let started = say(
            OwnTool::DelegateTask,
            json!({"task": "Find how child tasks should look", "title": "Research"}),
            Some(json!({"taskId": 12, "childThreadId": 12, "title": "Research"})),
            CallState::Done,
        );
        assert_eq!(
            sentence_text(&started),
            "Started a subthread: Research: UI for child tasks"
        );
        assert_eq!(started.opens, Some(ThreadId(12)));
        // While it runs, there's nothing to open yet.
        let starting = say(
            OwnTool::DelegateTask,
            json!({"task": "Find how child tasks should look"}),
            None,
            CallState::Running,
        );
        assert_eq!(
            sentence_text(&starting),
            "Starting a subthread: Find how child tasks should look"
        );
        assert_eq!(starting.opens, None);
        assert_eq!(
            sentence_text(&say(
                OwnTool::DelegateTask,
                json!({}),
                None,
                CallState::Running
            )),
            "Starting a subthread…"
        );
        assert_eq!(
            sentence_text(&say(
                OwnTool::DelegateTask,
                json!({"task": "x", "title": "Research"}),
                None,
                CallState::Failed
            )),
            "Start a subthread: Research"
        );

        let terminal = say(
            OwnTool::TerminalStart,
            json!({"command": "npm run dev"}),
            Some(json!({"threadId": 20, "kind": "terminal_thread"})),
            CallState::Done,
        );
        assert_eq!(sentence_text(&terminal), "Started a terminal: npm run dev");
        assert_eq!(
            terminal.subject.map(|subject| subject.style),
            Some(SubjectStyle::Code)
        );
        assert_eq!(terminal.opens, Some(ThreadId(20)));
        assert_eq!(
            sentence_text(&say(
                OwnTool::TerminalStart,
                json!({}),
                None,
                CallState::Done
            )),
            "Started a terminal"
        );

        let cases = [
            (OwnTool::Capabilities, json!({}), "Listed agents and models"),
            (
                OwnTool::ThreadWait,
                json!({"threadId": "12"}),
                "Waited for Research: UI for child tasks",
            ),
            (
                OwnTool::ThreadWait,
                json!({"threadId": 99}),
                "Waited for a thread",
            ),
            (
                OwnTool::ThreadUpdate,
                json!({"title": "Checkout"}),
                "Renamed this thread to Checkout",
            ),
            (
                OwnTool::ThreadOrganize,
                json!({"action": "archive"}),
                "Archived this thread",
            ),
            (
                OwnTool::TaskStatus,
                json!({"taskId": 12}),
                "Checked on Research: UI for child tasks",
            ),
            (
                OwnTool::TerminalSend,
                json!({"text": "npm test", "submit": true}),
                "Typed npm test ⏎ into this thread's terminal",
            ),
            (
                OwnTool::TerminalSend,
                json!({"keys": ["ctrl-c"], "threadId": 20}),
                "Typed ctrl-c into npm run dev",
            ),
            (
                OwnTool::TerminalSend,
                json!({}),
                "Typed into this thread's terminal",
            ),
            (
                OwnTool::TerminalRead,
                json!({"paneId": 3, "machine": "devbox"}),
                "Read a terminal",
            ),
            (
                OwnTool::TerminalStart,
                json!({"folder": "~/src", "machine": "devbox"}),
                "Started a terminal: ~/src",
            ),
            (
                OwnTool::ProjectAdd,
                json!({"path": "~/src/agentZ", "machine": "devbox"}),
                "Added a project: ~/src/agentZ",
            ),
            (
                OwnTool::CommandRun,
                json!({"command": "cargo test", "machine": "devbox"}),
                "Ran cargo test",
            ),
            (
                OwnTool::WorkspaceHandoff,
                json!({"type": "pasture", "branch": "agentz/fix"}),
                "Handed off to a new pasture: agentz/fix",
            ),
            (
                OwnTool::CreateThreads,
                json!({"threads": [{"prompt": "a"}, {"prompt": "b"}, {"prompt": "c"}]}),
                "Started 3 threads",
            ),
            // Another machine's thread ids aren't this one's.
            (
                OwnTool::ThreadRead,
                json!({"threadId": 12, "machine": "devbox"}),
                "Read a thread",
            ),
        ];
        for (tool, arguments, expected) in cases {
            assert_eq!(
                sentence_text(&say(tool, arguments, None, CallState::Done)),
                expected
            );
        }

        assert_eq!(OwnTool::DelegateTask.fold_label(3), "Started 3 subthreads");
        assert_eq!(OwnTool::TerminalStart.fold_label(1), "Started a terminal");
        assert_eq!(OwnTool::CreateThreads.fold_group(), OwnTool::ThreadLaunch);
    }

    #[test]
    fn agentzs_tools_say_what_came_of_them() {
        let words = |text: &str| Some(OwnResult::Words(text.to_string()));
        let result = |tool: OwnTool, arguments: Value, output: Value| {
            tool.result(&arguments, &output, Some("2m 14s"))
        };
        let cases = [
            (
                OwnTool::ThreadList,
                json!({}),
                json!({"threads": [{"threadId": 1}], "total": 8}),
                words("8 threads"),
            ),
            (
                OwnTool::Capabilities,
                json!({}),
                json!({"agents": [{"name": "Claude Agent"}]}),
                words("1 agent"),
            ),
            (
                OwnTool::ThreadRead,
                json!({"threadId": 4}),
                json!({"thread": {}, "items": [{}, {}, {}]}),
                words("3 messages"),
            ),
            (
                OwnTool::CreateThreads,
                json!({}),
                json!({"threads": [{"title": "Fix flaky login test"},
                                   {"title": "Checkout flow review"}, {"title": "Docs"}]}),
                words("Fix flaky login test, Checkout flow review, 1 more"),
            ),
            (
                OwnTool::ThreadWait,
                json!({}),
                json!({"status": "idle", "timedOut": false}),
                words("Done in 2m 14s"),
            ),
            (
                OwnTool::ThreadWait,
                json!({}),
                json!({"status": "running", "timedOut": true}),
                words("Still working"),
            ),
            (
                OwnTool::ThreadWait,
                json!({}),
                json!({"status": "waiting_for_approval", "timedOut": false}),
                words("Waiting for approval"),
            ),
            (
                OwnTool::ThreadDiff,
                json!({}),
                json!({"files": [{"additions": 10, "deletions": 1},
                                 {"additions": 2, "deletions": 2}]}),
                Some(OwnResult::Changes {
                    added: 12,
                    removed: 3,
                }),
            ),
            (
                OwnTool::TaskStatus,
                json!({}),
                json!({"status": "running", "workState": "working"}),
                words("Working"),
            ),
            (
                OwnTool::TaskStatus,
                json!({}),
                json!({"status": "completed", "workState": "result_available"}),
                words("Done"),
            ),
            (
                OwnTool::WorkspaceStatus,
                json!({}),
                json!({"kind": "pasture", "branch": "agentz/brave-otter"}),
                words("Pasture on agentz/brave-otter"),
            ),
            (
                OwnTool::WorkspaceList,
                json!({}),
                json!({"isRepository": true, "branches": [{}, {}, {}, {}, {}, {}],
                       "workspaces": [{}, {}]}),
                words("6 branches, 2 workspaces"),
            ),
            (
                OwnTool::WorkspaceSync,
                json!({}),
                json!({"message": "Rebased the pasture onto the project's `main`."}),
                words("Rebased"),
            ),
            (
                OwnTool::TerminalList,
                json!({}),
                json!({"terminals": [{}, {}, {}]}),
                words("3 terminals"),
            ),
            (
                OwnTool::TerminalRead,
                json!({}),
                json!({"text": "one\ntwo\n\n"}),
                words("2 lines"),
            ),
            (
                OwnTool::TerminalWait,
                json!({}),
                json!({"status": "exited", "exitCode": 0, "matched": false, "timedOut": false}),
                words("Exit 0"),
            ),
            (
                OwnTool::CommandRun,
                json!({}),
                json!({"exitCode": 0, "timedOut": false, "output": ""}),
                words("Exit 0 · 2m 14s"),
            ),
            (
                OwnTool::ProjectAdd,
                json!({}),
                json!({"name": "storefront", "path": "/src/storefront"}),
                words("storefront"),
            ),
            (
                OwnTool::ArtifactList,
                json!({}),
                json!({"artifacts": [{}, {}]}),
                words("2 artifacts"),
            ),
            (
                OwnTool::ArtifactRead,
                json!({"artifactId": "a1"}),
                json!({"artifactId": "a1", "version": 3, "source": "<h1>Plan</h1>"}),
                words("v3"),
            ),
            // A publish shows as the artifact's card.
            (
                OwnTool::ArtifactPublish,
                json!({}),
                json!({"artifactId": "a1", "version": 2}),
                None,
            ),
            // Tools that made something open it instead, and others only did something.
            (
                OwnTool::ThreadLaunch,
                json!({}),
                json!({"threadId": 4, "title": "Fix"}),
                None,
            ),
            (
                OwnTool::ThreadSend,
                json!({}),
                json!({"delivery": "queued"}),
                None,
            ),
        ];
        for (tool, arguments, output, expected) in cases {
            assert_eq!(result(tool, arguments, output), expected, "{tool:?}");
        }
    }

    #[test]
    fn a_finished_publish_names_its_artifact_and_version() {
        let published = tool_call(
            "mcp__agentz__agentz_artifact_publish",
            Some(json!({"title": "Checkout redesign", "kind": "page"})),
            Some(r#"{"artifactId": "k3x9", "version": 2, "versions": 2}"#),
        );
        assert_eq!(
            published_artifact(&published),
            Some((ArtifactId("k3x9".to_string()), 2))
        );
        let failed = tool_call(
            "mcp__agentz__agentz_artifact_publish",
            Some(json!({"title": "Checkout redesign"})),
            Some("A page needs an index.html."),
        );
        assert_eq!(published_artifact(&failed), None);
        let listed = tool_call(
            "mcp__agentz__agentz_artifact_list",
            None,
            Some(r#"{"artifactId": "k3x9", "version": 2}"#),
        );
        assert_eq!(published_artifact(&listed), None);
    }

    #[test]
    fn agentzs_tools_open_to_what_came_back_in_words() {
        let listed = OwnTool::ThreadList.view(
            &json!({}),
            &json!({"threads": [
                {"threadId": 412, "title": "Fix flaky login test", "status": "running",
                 "agentName": "Claude Agent", "model": "Opus 4.1",
                 "lastActivityAt": "2025-10-09T18:02:11Z"},
                {"threadId": 398, "title": "Checkout flow review", "status": "idle",
                 "agentName": "Factory Droid", "model": null},
            ]}),
        );
        assert_eq!(
            listed,
            vec![
                OwnPart::Thread(ThreadLine {
                    title: "Fix flaky login test".into(),
                    details: vec!["Claude Agent".into(), "Opus 4.1".into(), "working".into()],
                    state: ThreadState::Working,
                    at: Some("2025-10-09T18:02:11Z".into()),
                }),
                OwnPart::Thread(ThreadLine {
                    title: "Checkout flow review".into(),
                    details: vec!["Factory Droid".into(), "idle".into()],
                    state: ThreadState::Idle,
                    at: None,
                }),
            ]
        );
        assert_eq!(
            OwnTool::CommandRun.view(
                &json!({"command": "cargo test"}),
                &json!({"exitCode": 0, "output": "running 2 tests\ntest result: ok\n"}),
            ),
            vec![OwnPart::Printed("running 2 tests\ntest result: ok".into())]
        );
        assert_eq!(
            OwnTool::WorkspaceStatus.view(
                &json!({}),
                &json!({"kind": "pasture", "branch": "agentz/brave-otter", "baseRef": "main",
                        "folder": "/data/pastures/storefront/brave-otter"}),
            ),
            vec![
                OwnPart::Line(vec![
                    Span::plain("Pasture"),
                    Span::plain("on"),
                    Span::code("agentz/brave-otter"),
                    Span::plain("from"),
                    Span::code("main"),
                ]),
                OwnPart::Line(vec![Span::dim("/data/pastures/storefront/brave-otter")]),
            ]
        );
        assert_eq!(
            OwnTool::Capabilities.view(
                &json!({}),
                &json!({"agents": [{"name": "Claude Agent", "models": [
                    {"id": "opus", "name": "Opus 4.1"}, {"id": "sonnet", "name": "Sonnet 4.5"}]}]}),
            ),
            vec![OwnPart::Line(vec![
                Span::bright("Claude Agent"),
                Span::plain("Opus 4.1, Sonnet 4.5"),
            ])]
        );
        assert_eq!(
            OwnTool::WorkspaceSync.view(
                &json!({}),
                &json!({"message": "Rebased the pasture onto the project's `main`."}),
            ),
            vec![OwnPart::Line(vec![
                Span::plain("Rebased the pasture onto the project's"),
                Span::code("main"),
            ])]
        );
        // Renaming says it all in its row: only its input is left.
        assert!(
            OwnTool::ThreadUpdate
                .view(
                    &json!({"title": "x"}),
                    &json!({"threadId": 1, "title": "x"})
                )
                .is_empty()
        );
    }

    #[test]
    fn a_task_sent_to_the_background_is_known() {
        let task = |input: Value, report: &str| {
            let mut call = tool_call("Task", Some(input), Some(report));
            call.kind = acp::ToolKind::Other;
            SubagentCall::of(&call).map(|subagent| subagent.in_background)
        };
        assert_eq!(
            task(
                json!({"subagent_type": "worker", "description": "Build", "prompt": "x"}),
                "Built it."
            ),
            Some(false)
        );
        assert_eq!(
            task(
                json!({"subagent_type": "explorer", "description": "Trace", "prompt": "x"}),
                "Task launched in background.\ntask_id: 2a17"
            ),
            Some(true)
        );
        assert_eq!(
            task(
                json!({"subagent_type": "explorer", "description": "Trace", "await": false}),
                ""
            ),
            Some(true)
        );
    }

    #[test]
    fn a_tool_search_says_what_it_loaded_or_looked_for() {
        let loaded = tool_call(
            "ToolSearch",
            Some(json!({"query": "select:agentz___delegate_task,agentz___task_status"})),
            Some("Loaded 2 tool(s): agentz___delegate_task, agentz___task_status"),
        );
        let ToolCallKind::ToolSearch(search) = ToolCallKind::of(&loaded) else {
            panic!("a ToolSearch");
        };
        assert_eq!(
            search.label(CallState::Done),
            "Loaded Delegate a child task and Get delegated task status"
        );
        assert_eq!(
            search.label(CallState::Running),
            "Loading Delegate a child task and Get delegated task status"
        );
        assert_eq!(search.server().as_deref(), Some("agentZ"));
        assert_eq!(search.found(), None);
        assert_eq!(
            search
                .tools
                .iter()
                .map(|tool| ListedTool::named(tool))
                .collect::<Vec<_>>(),
            vec![
                ListedTool::Own(OwnTool::DelegateTask),
                ListedTool::Own(OwnTool::TaskStatus)
            ]
        );

        // Without output naming them, the query does; tools of several servers name none.
        let mixed = tool_call(
            "ToolSearch",
            Some(json!({"query": "select:mcp__github__create_issue,WebFetch"})),
            None,
        );
        let ToolCallKind::ToolSearch(search) = ToolCallKind::of(&mixed) else {
            panic!("a ToolSearch");
        };
        assert_eq!(
            search.label(CallState::Done),
            "Loaded Create issue and WebFetch"
        );
        assert_eq!(search.server(), None);
        assert_eq!(
            ListedTool::named("mcp__github__create_issue"),
            ListedTool::Mcp(McpName {
                server: "github".into(),
                tool: "create_issue".into()
            })
        );
        assert_eq!(
            ListedTool::named("WebFetch"),
            ListedTool::Other("WebFetch".into())
        );

        let words = tool_call(
            "ToolSearch",
            Some(json!({"query": "subthread"})),
            Some("```\nFound 2 tool(s): agentz___delegate_task, agentz___task_status\n```"),
        );
        let ToolCallKind::ToolSearch(search) = ToolCallKind::of(&words) else {
            panic!("a ToolSearch");
        };
        assert_eq!(
            search.label(CallState::Done),
            "Searched tools for “subthread”"
        );
        assert_eq!(search.found(), Some(2));
    }

    #[test]
    fn reads_and_edits_name_their_file() {
        let mut read = tool_call(
            "Read /tmp/az-linux-rel.png",
            Some(json!({"file_path": "/tmp/az-linux-rel.png"})),
            None,
        );
        read.kind = acp::ToolKind::Read;
        assert_eq!(
            file_path(&read),
            Some(PathBuf::from("/tmp/az-linux-rel.png"))
        );
        read.locations = vec![PathBuf::from("/tmp/other.rs")];
        assert_eq!(file_path(&read), Some(PathBuf::from("/tmp/other.rs")));
        let search = tool_call("Search", Some(json!({"path": "src"})), None);
        assert_eq!(file_path(&search), None);
    }

    #[test]
    fn subagents_read_as_what_they_do() {
        let droid = tool_call(
            "Task",
            Some(json!({
                "subagent_type": "worker",
                "description": "Build subthreads round picks",
                "await": true,
                "complexity": "heavy",
                "prompt": "Build what the user picked.",
            })),
            None,
        );
        let ToolCallKind::Subagent(subagent) = ToolCallKind::of(&droid) else {
            panic!("a subagent");
        };
        assert_eq!(
            subagent,
            SubagentCall {
                description: "Build subthreads round picks".into(),
                kind: Some("worker".into()),
                prompt: Some("Build what the user picked.".into()),
                options: vec!["heavy".into()],
                subthread: None,
                in_background: false,
            }
        );

        let mut claude = tool_call("Find where the login view is drawn", None, None);
        claude.subthread = Some(ThreadId(7));
        let ToolCallKind::Subagent(subagent) = ToolCallKind::of(&claude) else {
            panic!("a subagent");
        };
        assert_eq!(subagent.description, "Find where the login view is drawn");
        assert_eq!(subagent.kind, None);
        assert_eq!(subagent.subthread, Some(ThreadId(7)));

        let task = tool_call("Task", Some(json!({"description": "Not a subagent"})), None);
        assert!(matches!(ToolCallKind::of(&task), ToolCallKind::Plain));
    }

    fn label_text(tool_call: &ToolCall, folder: Option<&Path>) -> Option<(String, Option<String>)> {
        let label =
            AgentAction::of(tool_call).label(tool_call, CallState::of_call(tool_call), folder)?;
        Some((label.text(), label.detail))
    }

    #[test]
    fn every_agents_reads_and_files_read_the_same() {
        let folder = Path::new("/work/storefront");
        let mut claude = tool_call(
            "Read /work/storefront/src/cart/total.ts (1 - 120)",
            Some(json!({"file_path": "/work/storefront/src/cart/total.ts"})),
            Some("1\tconst a = 1;"),
        );
        claude.kind = acp::ToolKind::Read;
        assert_eq!(
            label_text(&claude, Some(folder)),
            Some((
                "Read src/cart/total.ts".to_string(),
                Some("lines 1–120".to_string())
            ))
        );
        let mut codex = tool_call(
            "Read total.ts",
            Some(json!({"path": "/work/storefront/src/cart/total.ts"})),
            Some("a\nb\nc"),
        );
        codex.kind = acp::ToolKind::Read;
        assert_eq!(
            label_text(&codex, Some(folder)),
            Some((
                "Read src/cart/total.ts".to_string(),
                Some("3 lines".to_string())
            ))
        );

        let mut created = tool_call("Write /work/storefront/src/cart/round.ts", None, None);
        created.kind = acp::ToolKind::Edit;
        created.diffs = vec![agentz_protocol::thread::FileDiff {
            path: "/work/storefront/src/cart/round.ts".into(),
            old_text: None,
            new_text: "a\nb\nc\n".into(),
            start_line: Some(1),
        }];
        assert_eq!(
            AgentAction::of(&created),
            AgentAction::Created {
                path: "/work/storefront/src/cart/round.ts".into(),
                lines: 3
            }
        );
        assert_eq!(
            label_text(&created, Some(folder)),
            Some(("Created src/cart/round.ts".to_string(), None))
        );

        let mut deleted = tool_call("Delete src/cart/legacy.ts", None, None);
        deleted.kind = acp::ToolKind::Delete;
        deleted.locations = vec!["/work/storefront/src/cart/legacy.ts".into()];
        assert_eq!(
            label_text(&deleted, Some(folder)),
            Some(("Deleted src/cart/legacy.ts".to_string(), None))
        );

        let mut moved = tool_call("Move src/cart/util.ts to src/cart/round.ts", None, None);
        moved.kind = acp::ToolKind::Move;
        assert_eq!(
            label_text(&moved, Some(folder)),
            Some((
                "Moved src/cart/util.ts → src/cart/round.ts".to_string(),
                None
            ))
        );

        // Edits and commands keep their own rows.
        let mut edit = tool_call("Edit src/cart/total.ts", None, None);
        edit.kind = acp::ToolKind::Edit;
        assert_eq!(AgentAction::of(&edit), AgentAction::Other);
        assert_eq!(label_text(&edit, None), None);
    }

    #[test]
    fn every_agents_searches_read_the_same() {
        let output = "src/cart/total.ts:3:  return roundTotal(sum);\nsrc/cart/total.ts:9:export function roundTotal(\nsrc/cart/round.ts:1:import { roundTotal }";
        for title in [
            "grep -n \"roundTotal\" src",
            "Search for 'roundTotal' in src",
            "Grep roundTotal in src",
        ] {
            let mut search = tool_call(title, None, Some(output));
            search.kind = acp::ToolKind::Search;
            assert_eq!(
                AgentAction::of(&search),
                AgentAction::Grep(Grep {
                    pattern: "roundTotal".into(),
                    path: Some("src".into()),
                    ignore_case: false,
                }),
                "{title}"
            );
            assert_eq!(
                label_text(&search, None),
                Some((
                    "Searched for roundTotal in src".to_string(),
                    Some("3 matches in 2 files".to_string())
                )),
                "{title}"
            );
        }
        let files = grep_files(output, None).unwrap_or_default();
        assert_eq!(files.len(), 2);
        assert_eq!(files[0].lines[1].number, 9);
        assert_eq!(
            grep_files("src/a.ts:2\nsrc/b.ts:5", None).map(|files| grep_count(&files)),
            Some("7 matches in 2 files".to_string())
        );
        assert_eq!(
            grep_files("src/a.ts\nsrc/b.ts", None).map(|files| grep_count(&files)),
            Some("2 files".to_string())
        );
        let mut ignoring_case = tool_call(
            "grep -i \"total\" src",
            Some(json!({"pattern": "total", "-i": true})),
            None,
        );
        ignoring_case.kind = acp::ToolKind::Search;
        assert!(matches!(
            AgentAction::of(&ignoring_case),
            AgentAction::Grep(Grep {
                ignore_case: true,
                ..
            })
        ));

        let mut glob = tool_call(
            "Find `src` `**/*.test.ts`",
            None,
            Some("src/a.test.ts\nsrc/b.test.ts\nsrc/c.test.ts"),
        );
        glob.kind = acp::ToolKind::Search;
        assert_eq!(
            AgentAction::of(&glob),
            AgentAction::Glob(Glob {
                pattern: "**/*.test.ts".into(),
                path: Some("src".into()),
            })
        );
        assert_eq!(
            label_text(&glob, None),
            Some(("Found 3 files for **/*.test.ts".to_string(), None))
        );
    }

    #[test]
    fn pages_and_web_searches_say_what_they_fetched() {
        let mut fetch = tool_call("Fetch https://vitest.dev/api/expect", None, None);
        fetch.kind = acp::ToolKind::Fetch;
        assert_eq!(
            AgentAction::of(&fetch),
            AgentAction::Fetch {
                url: "https://vitest.dev/api/expect".into()
            }
        );
        assert_eq!(
            label_text(&fetch, None),
            Some(("Fetched vitest.dev/api/expect".to_string(), None))
        );

        for title in [
            "Search \"vitest toBeCloseTo\"",
            "Web search: vitest toBeCloseTo",
        ] {
            let mut search = tool_call(title, None, None);
            search.kind = acp::ToolKind::Fetch;
            assert_eq!(
                label_text(&search, None),
                Some((
                    "Searched the web for “vitest toBeCloseTo”".to_string(),
                    None
                )),
                "{title}"
            );
        }
        let output = "Web search results for query: \"vitest toBeCloseTo\"\n\nLinks: [{\"title\":\"expect | Vitest\",\"url\":\"https://vitest.dev/api/expect\"},{\"title\":\"Expect\",\"url\":\"https://jestjs.io/docs/expect\"}]";
        assert_eq!(
            web_hits(output),
            vec![
                WebHit {
                    title: "expect | Vitest".into(),
                    url: "https://vitest.dev/api/expect".into()
                },
                WebHit {
                    title: "Expect".into(),
                    url: "https://jestjs.io/docs/expect".into()
                },
            ]
        );
        assert_eq!(
            web_hits("- [Vitest](https://vitest.dev/)"),
            vec![WebHit {
                title: "Vitest".into(),
                url: "https://vitest.dev/".into()
            }]
        );
    }

    #[test]
    fn tools_many_agents_share_read_in_words() {
        let skill = tool_call("Load skill: review", None, None);
        assert_eq!(
            label_text(&skill, None),
            Some(("Loaded the review skill".to_string(), None))
        );
        let findings = tool_call(
            "Report 2 findings",
            Some(json!({"findings": [{"title": "a"}, {"title": "b"}]})),
            None,
        );
        assert_eq!(
            label_text(&findings, None),
            Some(("Reported 2 findings".to_string(), None))
        );
        let mut question = tool_call(
            "AskUserQuestion",
            Some(json!({"questions": [{"question": "Which approach?", "header": "Approach"}]})),
            None,
        );
        question.tool_name = Some("AskUserQuestion".into());
        assert_eq!(
            label_text(&question, None),
            Some(("Asked you Which approach?".to_string(), None))
        );

        // To-do lists show in the plan bar, so they have no row.
        let mut todos = tool_call("Update TODOs: Round once", None, None);
        todos.kind = acp::ToolKind::Think;
        assert!(is_hidden(&todos));
        assert_eq!(AgentAction::of(&todos), AgentAction::Todo);
        let mut codex_plan = tool_call("update_plan", None, None);
        codex_plan.kind = acp::ToolKind::Other;
        assert!(is_hidden(&codex_plan));
        let mut read = tool_call("TodoWrite", None, None);
        read.kind = acp::ToolKind::Read;
        assert!(!is_hidden(&read));

        // A tool nothing knows keeps its title.
        let notebook = tool_call("NotebookEdit", None, None);
        assert_eq!(AgentAction::of(&notebook), AgentAction::Other);
    }

    #[test]
    fn commands_and_mcp_tools_say_how_they_ended_and_what_they_acted_on() {
        let mut claude = tool_call("npm test", None, Some("FAIL src/cart\nExit code 1"));
        claude.kind = acp::ToolKind::Execute;
        assert_eq!(exit_code(&claude), Some(1));
        let mut droid = tool_call("npm test", None, Some("ok\n[Process exited with code 0]"));
        droid.kind = acp::ToolKind::Execute;
        assert_eq!(exit_code(&droid), Some(0));
        let mut cwd = tool_call("ls", Some(json!({"command": "ls", "cwd": "/work"})), None);
        cwd.kind = acp::ToolKind::Execute;
        assert_eq!(command_folder(&cwd).as_deref(), Some("/work"));

        let issue = tool_call(
            "mcp__github__create_issue",
            Some(
                json!({"body": "Long\ntext", "title": "Cart total off by a cent", "labels": ["bug"]}),
            ),
            None,
        );
        assert_eq!(
            mcp_subject(&issue).as_deref(),
            Some("Cart total off by a cent")
        );
        assert_eq!(format_bytes(212 * 1024), "212 KB");
        assert_eq!(format_bytes(1_468_006), "1.4 MB");
        assert_eq!(format_bytes(900), "900 B");
    }
}
