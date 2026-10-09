//! How a tool call's row reads (`design/tool-calls/decisions.md`): reads and edits by their
//! file's icon, agentZ's own tools as what they did, ToolSearch as the tools it loaded, and other
//! MCP tools by name and server, the same whichever agent called them.

use std::path::{Path, PathBuf};

use agent_client_protocol::schema::v1 as acp;
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
    /// Anything else, shown by its kind and the agent's title.
    Plain,
}

impl ToolCallKind {
    pub fn of(tool_call: &ToolCall) -> Self {
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
    ProjectAdd,
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
        Self::ProjectAdd,
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
            Self::TerminalSend => (
                verbs("Type into", "Typing into", "Typed into"),
                terminal(),
                None,
            ),
            Self::TerminalRead => (verbs("Read", "Reading", "Read"), terminal(), None),
            Self::TerminalWait => (
                verbs("Wait for", "Waiting for", "Waited for"),
                terminal(),
                None,
            ),
            Self::ProjectAdd => (
                verbs("Add a project:", "Adding a project:", "Added a project:"),
                argument("path").map(Subject::code),
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
            opens: opens.filter(|_| state == CallState::Done),
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

    /// Making a subthread, a thread or a terminal leads a folded run's summary, as commands and
    /// edits do.
    pub fn leads_summary(self) -> bool {
        matches!(
            self,
            Self::DelegateTask | Self::ThreadLaunch | Self::TerminalStart
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
            Self::ProjectAdd => counted("Added a project", "Added {} projects"),
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
    pub opens: Option<ThreadId>,
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

    /// "Loaded 8 agentZ tools", or "Searched tools for “subthread”".
    pub fn label(&self, state: CallState) -> String {
        match &self.query {
            ToolQuery::Select(_) => {
                let count = self.tools.len();
                let server = self
                    .tools
                    .iter()
                    .map(|tool| ListedTool::named(tool).server())
                    .reduce(|first, other| if first == other { first } else { None })
                    .flatten();
                let tools = match (server, count) {
                    (Some(server), 1) => format!("1 {server} tool"),
                    (Some(server), count) => format!("{count} {server} tools"),
                    (None, 1) => "1 tool".to_string(),
                    (None, count) => format!("{count} tools"),
                };
                let verb = match state {
                    CallState::Running => "Loading",
                    CallState::Done => "Loaded",
                    CallState::Failed => "Load",
                };
                format!("{verb} {tools}")
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
            id: acp::ToolCallId::new("call"),
            title: title.to_string(),
            kind: acp::ToolKind::Other,
            status: acp::ToolCallStatus::Completed,
            text: output.into_iter().map(str::to_string).collect(),
            diffs: Vec::new(),
            locations: Vec::new(),
            raw_input: raw_input.map(|input| {
                format!(
                    "```json\n{}\n```",
                    serde_json::to_string_pretty(&input).unwrap_or_default()
                )
            }),
            terminals: Vec::new(),
            images: Vec::new(),
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
        match &sentence.subject {
            Some(subject) => format!("{} {}", sentence.verb, subject.text),
            None => sentence.verb.clone(),
        }
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
                json!({"text": "q"}),
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
    fn a_tool_search_says_what_it_loaded_or_looked_for() {
        let loaded = tool_call(
            "ToolSearch",
            Some(json!({"query": "select:agentz___delegate_task,agentz___task_status"})),
            Some("Loaded 2 tool(s): agentz___delegate_task, agentz___task_status"),
        );
        let ToolCallKind::ToolSearch(search) = ToolCallKind::of(&loaded) else {
            panic!("a ToolSearch");
        };
        assert_eq!(search.label(CallState::Done), "Loaded 2 agentZ tools");
        assert_eq!(search.label(CallState::Running), "Loading 2 agentZ tools");
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

        // Without output naming them, the query does; tools of several servers are just tools.
        let mixed = tool_call(
            "ToolSearch",
            Some(json!({"query": "select:mcp__github__create_issue,WebFetch"})),
            None,
        );
        let ToolCallKind::ToolSearch(search) = ToolCallKind::of(&mixed) else {
            panic!("a ToolSearch");
        };
        assert_eq!(search.label(CallState::Done), "Loaded 2 tools");
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
}
