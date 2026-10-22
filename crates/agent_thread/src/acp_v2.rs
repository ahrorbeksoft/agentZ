//! ACP v2 under the SDK's v1 connection. agentZ asks every agent for v2 in `initialize` and
//! speaks v1 with the SDK: with an agent that answers v2, each message is rewritten between
//! the two as it passes the wire, both ways. One that answers v1 goes on unchanged.
//!
//! v2 ends a turn with an `idle` `state_update`, while the prompt's answer only says the prompt
//! was taken in. So the SDK gets v1's answer to each prompt at that `idle`, and the states go
//! on to the thread as the agent sent them, for it to read before the typed updates.

use std::collections::{HashMap, VecDeque};

use base64::Engine as _;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

/// Marks a prompt sent into a running turn. It's taken out of the prompt, and
/// [`PROMPT_TAKEN_NOTIFICATION`] reports when the agent takes the prompt in.
pub(crate) const STEERED_PROMPT_META: &str = "agentz/steered";

/// A notification no agent sends: the agent took in a prompt marked [`STEERED_PROMPT_META`]
/// (v2's `user_message` for it), in the session given.
pub(crate) const PROMPT_TAKEN_NOTIFICATION: &str = "_agentz/prompt_taken";

/// The error by which an agent refuses the protocol version asked for.
pub(crate) const UNSUPPORTED_VERSION: i32 = -32022;

/// How many tool calls' content is kept, which v2's content chunks and terminal output add to.
const TOOL_CALLS_KEPT: usize = 64;
/// How many terminals' output is kept, and how much of each, from its end.
const TERMINALS_KEPT: usize = 32;
const TERMINAL_OUTPUT_KEPT: usize = 64 * 1024;

/// What a request's answer is, once v1's answer to its prompt was given at the turn's end.
const ANSWERED_PROMPT: &str = "agentz/answered";

const TOOL_KINDS: [&str; 10] = [
    "read",
    "edit",
    "delete",
    "move",
    "search",
    "execute",
    "think",
    "fetch",
    "switch_mode",
    "other",
];

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Version {
    /// `initialize` asked for v2 and wasn't answered yet.
    #[default]
    Asking,
    V1,
    V2,
}

/// What's in flight between the SDK and an agent that may speak v2.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Translator {
    version: Version,
    /// Requests sent to the agent, by id (as JSON text), for their answers.
    requests: HashMap<String, SentRequest>,
    /// Prompts whose v1 answer is still to come, oldest first.
    prompts: Vec<Prompt>,
    /// Sessions whose history replays.
    replaying: Vec<String>,
    /// The latest tool calls' content as the agent gave it, oldest first.
    tool_calls: VecDeque<ToolCallContent>,
    /// The latest terminals' output, oldest first.
    terminals: VecDeque<TerminalOutput>,
    /// Counts the tool calls made up for permission requests that have none.
    next_tool_call: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct SentRequest {
    /// v1's method.
    method: String,
    session: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Prompt {
    id: Value,
    session: String,
    /// The agent took it in: its turn's end answers it.
    taken: bool,
    steered: bool,
    /// Its `user_message` came, which the thread doesn't show again.
    #[serde(default)]
    echoed: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct ToolCallContent {
    session: String,
    id: String,
    content: Vec<Value>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct TerminalOutput {
    id: String,
    text: String,
}

/// Where the lines for a line the SDK wrote go.
#[derive(Default)]
pub(crate) struct Translated {
    pub(crate) to_agent: Vec<String>,
    /// Answers the agent can't give, as if it gave them.
    pub(crate) to_sdk: Vec<String>,
}

impl Translated {
    fn to_agent(line: String) -> Self {
        Self {
            to_agent: vec![line],
            to_sdk: Vec::new(),
        }
    }
}

impl Translator {
    /// Asks the agent for v2.
    pub(crate) fn asking() -> Self {
        Self::default()
    }

    /// Speaks v1 only, to an agent that refused v2.
    pub(crate) fn v1() -> Self {
        Self {
            version: Version::V1,
            ..Self::default()
        }
    }

    pub(crate) fn is_v2(&self) -> bool {
        self.version == Version::V2
    }

    /// The lines for the agent for one the SDK wrote.
    pub(crate) fn outgoing(&mut self, line: String) -> Translated {
        if self.version == Version::V1 {
            return Translated::to_agent(line);
        }
        let Ok(Value::Object(mut message)) = serde_json::from_str::<Value>(&line) else {
            return Translated::to_agent(line);
        };
        let Some(method) = message
            .get("method")
            .and_then(Value::as_str)
            .map(str::to_string)
        else {
            return Translated::to_agent(line);
        };
        let id = message.get("id").cloned();
        let params = message.entry("params").or_insert_with(|| json!({}));
        let session = string(params.get("sessionId"));
        let v2_method = if self.version == Version::Asking {
            if method != "initialize" {
                return Translated::to_agent(line);
            }
            if let Some(params) = params.as_object_mut() {
                ask_for_v2(params);
            }
            method.clone()
        } else {
            match self.v2_request(&method, id.as_ref(), session.as_deref(), params) {
                Ok(v2_method) => v2_method,
                Err(answer) => {
                    return Translated {
                        to_agent: Vec::new(),
                        to_sdk: answer.into_iter().collect(),
                    };
                }
            }
        };
        if let Some(id) = &id {
            self.requests
                .insert(id.to_string(), SentRequest { method, session });
        }
        message.insert("method".into(), v2_method.into());
        Translated::to_agent(Value::Object(message).to_string())
    }

    /// v2's method for a v1 request or notification, with its params rewritten; or the answer
    /// the SDK gets in place of the agent's, for what v2 doesn't have.
    fn v2_request(
        &mut self,
        method: &str,
        id: Option<&Value>,
        session: Option<&str>,
        params: &mut Value,
    ) -> Result<String, Option<String>> {
        let v2_method = match method {
            "authenticate" => "auth/login",
            "logout" => "auth/logout",
            // v2 resumes a session, and replays it when asked to.
            "session/load" => {
                if let Some(params) = params.as_object_mut() {
                    params.insert("replayFrom".into(), json!({"type": "start"}));
                    v2_mcp_servers(params);
                }
                if let Some(session) = session {
                    self.replaying.push(session.to_string());
                }
                "session/resume"
            }
            "session/new" | "session/resume" | "session/fork" => {
                if let Some(params) = params.as_object_mut() {
                    v2_mcp_servers(params);
                }
                method
            }
            "session/set_config_option" => {
                if let Some(params) = params.as_object_mut()
                    && !params.contains_key("type")
                {
                    params.insert("type".into(), "id".into());
                }
                method
            }
            "session/prompt" => {
                let steered = params
                    .as_object_mut()
                    .is_some_and(|params| take_meta_flag(params, STEERED_PROMPT_META));
                if let (Some(id), Some(session)) = (id, session) {
                    self.prompts.push(Prompt {
                        id: id.clone(),
                        session: session.to_string(),
                        taken: false,
                        steered,
                        echoed: false,
                    });
                }
                method
            }
            // The mode is a config option in v2.
            "session/set_mode" => {
                return Err(id.map(|id| {
                    error_line(id, json!({"code": -32601, "message": "Method not found"}))
                }));
            }
            _ => method,
        };
        Ok(v2_method.to_string())
    }

    /// The lines for the SDK for one the agent wrote.
    pub(crate) fn incoming(&mut self, line: String) -> Vec<String> {
        if self.version == Version::V1 {
            return vec![line];
        }
        let Ok(Value::Object(mut message)) = serde_json::from_str::<Value>(&line) else {
            return vec![line];
        };
        let method = string(message.get("method"));
        match (method.as_deref(), message.get("id").cloned()) {
            (None, Some(id)) => self.answer(id, message, line),
            _ if self.version == Version::Asking => vec![line],
            (Some("session/update"), None) => self.session_update(message),
            (Some("session/request_permission"), Some(_)) => {
                if let Some(params) = message.get_mut("params").and_then(Value::as_object_mut) {
                    self.v1_permission_request(params);
                }
                vec![Value::Object(message).to_string()]
            }
            _ => vec![line],
        }
    }

    fn answer(&mut self, id: Value, mut message: Map<String, Value>, line: String) -> Vec<String> {
        let Some(request) = self.requests.remove(&id.to_string()) else {
            return vec![line];
        };
        if self.version == Version::Asking {
            return self.initialized(message, line);
        }
        if message.contains_key("error") {
            match request.method.as_str() {
                "session/prompt" => self.prompts.retain(|prompt| prompt.id != id),
                "session/load" => self.end_replay(request.session.as_deref()),
                _ => {}
            }
            return vec![line];
        }
        let Some(result) = message.get_mut("result").and_then(Value::as_object_mut) else {
            return vec![line];
        };
        let mut commands = None;
        match request.method.as_str() {
            // Only says the prompt was taken in: v1's answer waits for the turn's end.
            "session/prompt" => {
                return match self.prompts.iter().position(|prompt| prompt.id == id) {
                    Some(index) => self.take_prompt(index),
                    None => Vec::new(),
                };
            }
            ANSWERED_PROMPT => return Vec::new(),
            "session/new" | "session/fork" | "session/load" | "session/resume" => {
                commands = result.remove("availableCommands");
                if let Some(options) = result.get_mut("configOptions") {
                    v1_config_options(options);
                }
                if request.method == "session/load" {
                    self.end_replay(request.session.as_deref());
                }
            }
            "session/set_config_option" => {
                if let Some(options) = result.get_mut("configOptions") {
                    v1_config_options(options);
                }
            }
            _ => {}
        }
        let session = string(result.get("sessionId")).or(request.session);
        let mut lines = vec![Value::Object(message).to_string()];
        // v2 gives a session's commands with it, where v1 agents send them next.
        if let (Some(Value::Array(commands)), Some(session)) = (commands, session)
            && !commands.is_empty()
        {
            lines.push(update_line(
                &session,
                json!({
                    "sessionUpdate": "available_commands_update",
                    "availableCommands": v1_commands(commands),
                }),
                None,
            ));
        }
        lines
    }

    /// The agent answered `initialize`: v2 goes on translated, anything else as it is.
    fn initialized(&mut self, mut message: Map<String, Value>, line: String) -> Vec<String> {
        let version = message
            .get("result")
            .and_then(|result| result.get("protocolVersion"))
            .and_then(Value::as_u64);
        if version != Some(2) {
            self.version = Version::V1;
            return vec![line];
        }
        self.version = Version::V2;
        if let Some(result) = message.get_mut("result") {
            *result = v1_initialize(result.take());
        }
        vec![Value::Object(message).to_string()]
    }

    fn end_replay(&mut self, session: Option<&str>) {
        if let Some(session) = session
            && let Some(index) = self.replaying.iter().position(|known| known == session)
        {
            self.replaying.remove(index);
        }
    }

    fn session_update(&mut self, mut message: Map<String, Value>) -> Vec<String> {
        let Some(params) = message.get_mut("params").and_then(Value::as_object_mut) else {
            return vec![Value::Object(message).to_string()];
        };
        let session = string(params.get("sessionId")).unwrap_or_default();
        let meta = params.get("_meta").cloned();
        let Some(Value::Object(mut update)) = params.remove("update") else {
            return vec![Value::Object(message).to_string()];
        };
        let kind = string(update.get("sessionUpdate")).unwrap_or_default();
        let updates = match kind.as_str() {
            "user_message_chunk" | "agent_message_chunk" | "agent_thought_chunk" => {
                if update.get("content").is_some_and(is_v1_content_block) {
                    vec![Value::Object(update)]
                } else {
                    Vec::new()
                }
            }
            "user_message" => {
                let replaying = self.replaying.contains(&session);
                // The thread shows its own prompts already.
                if !replaying
                    && let Some(index) = self
                        .prompts
                        .iter()
                        .position(|prompt| prompt.session == session && !prompt.echoed)
                {
                    self.prompts[index].echoed = true;
                    return self.take_prompt(index);
                }
                message_chunks(&update, "user_message_chunk")
            }
            "agent_message" => message_chunks(&update, "agent_message_chunk"),
            "agent_thought" => message_chunks(&update, "agent_thought_chunk"),
            // As it is, for the thread to read: v1 has no state for a session.
            "state_update" => {
                let answers = if update.get("state").and_then(Value::as_str) == Some("idle") {
                    self.end_turn(&session, &update)
                } else {
                    Vec::new()
                };
                let mut lines = vec![update_line(&session, Value::Object(update), meta)];
                lines.extend(answers);
                return lines;
            }
            "tool_call_update" => vec![self.v1_tool_call_update(&session, update)],
            "tool_call_content_chunk" => self
                .tool_call_content_chunk(&session, &update)
                .into_iter()
                .collect(),
            "terminal_update" | "terminal_output_chunk" => {
                self.terminal_update(&session, &kind, &update)
            }
            "plan_update" => vec![v1_plan(update)],
            "plan_removed" => vec![json!({"sessionUpdate": "plan", "entries": []})],
            "available_commands_update" => {
                if let Some(Value::Array(commands)) = update.remove("availableCommands") {
                    update.insert("availableCommands".into(), v1_commands(commands).into());
                }
                vec![Value::Object(update)]
            }
            "config_option_update" => {
                if let Some(options) = update.get_mut("configOptions") {
                    v1_config_options(options);
                }
                vec![Value::Object(update)]
            }
            _ => vec![Value::Object(update)],
        };
        updates
            .into_iter()
            .map(|update| update_line(&session, update, meta.clone()))
            .collect()
    }

    /// Notes that the agent took in a prompt, by its `user_message` or its answer, whichever
    /// comes first. One sent into a running turn shows in the thread then.
    fn take_prompt(&mut self, index: usize) -> Vec<String> {
        let prompt = &mut self.prompts[index];
        if prompt.taken {
            return Vec::new();
        }
        prompt.taken = true;
        if !prompt.steered {
            return Vec::new();
        }
        vec![
            json!({
                "jsonrpc": "2.0",
                "method": PROMPT_TAKEN_NOTIFICATION,
                "params": {"sessionId": prompt.session},
            })
            .to_string(),
        ]
    }

    /// v1's answers to the session's prompts that the agent took in, at the end of its turn.
    fn end_turn(&mut self, session: &str, idle: &Map<String, Value>) -> Vec<String> {
        let (ended, going): (Vec<Prompt>, Vec<Prompt>) = std::mem::take(&mut self.prompts)
            .into_iter()
            .partition(|prompt| prompt.session == session && prompt.taken);
        self.prompts = going;
        let answer = turn_answer(idle);
        ended
            .into_iter()
            .map(|prompt| {
                // Its v2 answer may still be on its way.
                if let Some(request) = self.requests.get_mut(&prompt.id.to_string()) {
                    request.method = ANSWERED_PROMPT.to_string();
                }
                match &answer {
                    Ok(result) => response_line(&prompt.id, result.clone()),
                    Err(error) => error_line(&prompt.id, error.clone()),
                }
            })
            .collect()
    }

    fn v1_tool_call_update(&mut self, session: &str, mut update: Map<String, Value>) -> Value {
        let id = string(update.get("toolCallId")).unwrap_or_default();
        let content = update.remove("content");
        let mut fields = v1_tool_call_fields(update);
        match content {
            Some(Value::Array(content)) => {
                let content = self.remember_content(session, &id, content);
                fields.insert("content".into(), content);
            }
            Some(Value::Null) => {
                let content = self.remember_content(session, &id, Vec::new());
                fields.insert("content".into(), content);
            }
            _ => {}
        }
        fields.insert("sessionUpdate".into(), "tool_call_update".into());
        Value::Object(fields)
    }

    fn tool_call_content_chunk(
        &mut self,
        session: &str,
        chunk: &Map<String, Value>,
    ) -> Option<Value> {
        let id = string(chunk.get("toolCallId"))?;
        let item = chunk.get("content")?.clone();
        let mut content = self
            .tool_calls
            .iter()
            .find(|call| call.session == session && call.id == id)
            .map(|call| call.content.clone())
            .unwrap_or_default();
        content.push(item);
        let content = self.remember_content(session, &id, content);
        Some(json!({"sessionUpdate": "tool_call_update", "toolCallId": id, "content": content}))
    }

    /// Keeps the tool call's content, and gives it as v1 has it.
    fn remember_content(&mut self, session: &str, id: &str, content: Vec<Value>) -> Value {
        self.tool_calls
            .retain(|call| !(call.session == session && call.id == id));
        let v1_content = self.v1_content(&content);
        self.tool_calls.push_back(ToolCallContent {
            session: session.to_string(),
            id: id.to_string(),
            content,
        });
        while self.tool_calls.len() > TOOL_CALLS_KEPT {
            self.tool_calls.pop_front();
        }
        v1_content
    }

    fn v1_content(&self, content: &[Value]) -> Value {
        Value::Array(
            content
                .iter()
                .flat_map(|item| self.v1_content_item(item))
                .collect(),
        )
    }

    fn v1_content_item(&self, item: &Value) -> Vec<Value> {
        match item.get("type").and_then(Value::as_str) {
            Some("content") => item
                .get("content")
                .filter(|block| is_v1_content_block(block))
                .map(|_| item.clone())
                .into_iter()
                .collect(),
            Some("diff") if item.get("changes").is_some() => v1_diffs(item),
            Some("diff") => vec![item.clone()],
            // v2's terminals are the agent's own, shown by their output: v1's are the client's.
            Some("terminal") => {
                let id = item.get("terminalId").and_then(Value::as_str);
                self.terminals
                    .iter()
                    .find(|terminal| Some(terminal.id.as_str()) == id)
                    .filter(|terminal| !terminal.text.trim().is_empty())
                    .map(|terminal| {
                        json!({"type": "content", "content": {
                            "type": "text",
                            "text": format!("```console\n{}\n```", terminal.text.trim_end()),
                        }})
                    })
                    .into_iter()
                    .collect()
            }
            _ => Vec::new(),
        }
    }

    /// Keeps a terminal's output, and gives the updates of the session's tool calls that
    /// show it.
    fn terminal_update(
        &mut self,
        session: &str,
        kind: &str,
        update: &Map<String, Value>,
    ) -> Vec<Value> {
        let Some(id) = string(update.get("terminalId")) else {
            return Vec::new();
        };
        let mut terminal = match self.terminals.iter().position(|terminal| terminal.id == id) {
            Some(index) => self.terminals.remove(index),
            None => None,
        }
        .unwrap_or_else(|| TerminalOutput {
            id: id.clone(),
            text: String::new(),
        });
        let appends = kind == "terminal_output_chunk";
        let data = if appends {
            update.get("data")
        } else {
            update.get("output").and_then(|output| output.get("data"))
        };
        if let Some(text) = data.and_then(Value::as_str).and_then(decode_output) {
            if appends {
                terminal.text.push_str(&text);
            } else {
                terminal.text = text;
            }
            keep_end(&mut terminal.text, TERMINAL_OUTPUT_KEPT);
        }
        self.terminals.push_back(terminal);
        while self.terminals.len() > TERMINALS_KEPT {
            self.terminals.pop_front();
        }
        self.tool_calls
            .iter()
            .filter(|call| {
                call.session == session
                    && call.content.iter().any(|item| {
                        item.get("type").and_then(Value::as_str) == Some("terminal")
                            && item.get("terminalId").and_then(Value::as_str) == Some(&id)
                    })
            })
            .map(|call| {
                json!({
                    "sessionUpdate": "tool_call_update",
                    "toolCallId": call.id,
                    "content": self.v1_content(&call.content),
                })
            })
            .collect()
    }

    /// v1 asks about a tool call, which v2 makes the request's subject, if it has one.
    fn v1_permission_request(&mut self, params: &mut Map<String, Value>) {
        let session = string(params.get("sessionId")).unwrap_or_default();
        let title = params.remove("title");
        params.remove("description");
        let mut tool_call = match params.remove("subject") {
            Some(Value::Object(mut subject)) => match subject.get("type").and_then(Value::as_str) {
                Some("tool_call") => match subject.remove("toolCall") {
                    Some(Value::Object(mut tool_call)) => {
                        let id = string(tool_call.get("toolCallId")).unwrap_or_default();
                        let content = tool_call.remove("content");
                        let mut fields = v1_tool_call_fields(tool_call);
                        if let Some(Value::Array(content)) = content {
                            let content = self.remember_content(&session, &id, content);
                            fields.insert("content".into(), content);
                        }
                        fields
                    }
                    _ => Map::new(),
                },
                Some("command") => match subject.remove("toolCallId") {
                    Some(id) => Map::from_iter([("toolCallId".to_string(), id)]),
                    None => Map::from_iter([
                        ("kind".to_string(), "execute".into()),
                        (
                            "rawInput".to_string(),
                            json!({
                                "command": subject.get("command"),
                                "cwd": subject.get("cwd"),
                            }),
                        ),
                    ]),
                },
                _ => Map::new(),
            },
            _ => Map::new(),
        };
        if !tool_call.contains_key("toolCallId") {
            self.next_tool_call += 1;
            tool_call.insert(
                "toolCallId".into(),
                format!("agentz-permission-{}", self.next_tool_call).into(),
            );
            if let Some(title) = title {
                tool_call.insert("title".into(), title);
            }
        }
        params.insert("toolCall".into(), Value::Object(tool_call));
    }
}

fn string(value: Option<&Value>) -> Option<String> {
    value.and_then(Value::as_str).map(str::to_string)
}

fn response_line(id: &Value, result: Value) -> String {
    json!({"jsonrpc": "2.0", "id": id, "result": result}).to_string()
}

fn error_line(id: &Value, error: Value) -> String {
    json!({"jsonrpc": "2.0", "id": id, "error": error}).to_string()
}

fn update_line(session: &str, update: Value, meta: Option<Value>) -> String {
    let mut params = json!({"sessionId": session, "update": update});
    if let Some(meta) = meta {
        params["_meta"] = meta;
    }
    json!({"jsonrpc": "2.0", "method": "session/update", "params": params}).to_string()
}

/// Takes a flag out of the params' `_meta`, and `_meta` with it once it's empty.
fn take_meta_flag(params: &mut Map<String, Value>, key: &str) -> bool {
    let Some(Value::Object(meta)) = params.get_mut("_meta") else {
        return false;
    };
    let flag = meta.remove(key).and_then(|flag| flag.as_bool()) == Some(true);
    if meta.is_empty() {
        params.remove("_meta");
    }
    flag
}

/// The v1 `initialize` request, with v2's fields beside its own: an agent that speaks v2
/// reads those, and one that doesn't answers v1, as ACP asks.
fn ask_for_v2(params: &mut Map<String, Value>) {
    params.insert("protocolVersion".into(), 2.into());
    if let Some(info) = params.get("clientInfo").cloned() {
        params.insert("info".into(), info);
    }
    let v1 = params
        .get("clientCapabilities")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let mut capabilities = Map::new();
    let mut auth = Map::new();
    if v1.pointer("/auth/terminal").and_then(Value::as_bool) == Some(true) {
        auth.insert("terminal".into(), json!({}));
    }
    if let Some(meta) = v1.pointer("/auth/_meta") {
        auth.insert("_meta".into(), meta.clone());
    }
    capabilities.insert("auth".into(), Value::Object(auth));
    for key in ["elicitation", "_meta"] {
        if let Some(value) = v1.get(key).filter(|value| !value.is_null()) {
            capabilities.insert(key.into(), value.clone());
        }
    }
    params.insert("capabilities".into(), Value::Object(capabilities));
}

/// v1's answer to `initialize` for v2's. What v2 has for every agent (listing, resuming with
/// a replay, closing) v1 has as capabilities.
fn v1_initialize(result: Value) -> Value {
    let present = |value: Option<&Value>| value.is_some_and(|value| !value.is_null());
    let session = result
        .pointer("/capabilities/session")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let auth_methods: Vec<Value> = match result.get("authMethods") {
        Some(Value::Array(methods)) => methods.iter().cloned().map(v1_auth_method).collect(),
        _ => Vec::new(),
    };
    let mut session_capabilities = json!({"list": {}, "resume": {}, "close": {}});
    for key in [
        "delete",
        "additionalDirectories",
        "systemPrompt",
        "fork",
        "_meta",
    ] {
        if let Some(value) = session.get(key).filter(|value| !value.is_null()) {
            session_capabilities[key] = value.clone();
        }
    }
    let mut agent_capabilities = json!({
        "loadSession": true,
        "promptCapabilities": {
            "image": present(session.pointer("/prompt/image")),
            "audio": present(session.pointer("/prompt/audio")),
            "embeddedContext": present(session.pointer("/prompt/embeddedContext")),
        },
        "mcpCapabilities": {
            "http": present(session.pointer("/mcp/http")),
            "sse": false,
            "acp": present(session.pointer("/mcp/acp")),
        },
        "sessionCapabilities": session_capabilities,
        // A v2 agent with logins logs out with `auth/logout`.
        "auth": if auth_methods.is_empty() { json!({}) } else { json!({"logout": {}}) },
    });
    for (key, pointer) in [
        ("providers", "/capabilities/providers"),
        ("_meta", "/capabilities/_meta"),
    ] {
        if let Some(value) = result.pointer(pointer).filter(|value| !value.is_null()) {
            agent_capabilities[key] = value.clone();
        }
    }
    let mut v1 = json!({
        "protocolVersion": 1,
        "agentCapabilities": agent_capabilities,
        "authMethods": auth_methods,
    });
    for (key, v2_key) in [("agentInfo", "info"), ("_meta", "_meta")] {
        if let Some(value) = result.get(v2_key).filter(|value| !value.is_null()) {
            v1[key] = value.clone();
        }
    }
    v1
}

/// v2 names a method's id `methodId`, tags the agent's own with `type`, and lists a terminal
/// login's variables as names and values.
fn v1_auth_method(method: Value) -> Value {
    let Value::Object(mut method) = method else {
        return method;
    };
    if let Some(id) = method.remove("methodId") {
        method.insert("id".into(), id);
    }
    if method.get("type").and_then(Value::as_str) == Some("agent") {
        method.remove("type");
    }
    if let Some(Value::Array(variables)) = method.get("env") {
        let env: Map<String, Value> = variables
            .iter()
            .filter_map(|variable| {
                Some((
                    variable.get("name")?.as_str()?.to_string(),
                    variable.get("value")?.clone(),
                ))
            })
            .collect();
        method.insert("env".into(), Value::Object(env));
    }
    Value::Object(method)
}

/// v2 names an option's id `configId` and a select group's `groupId`.
fn v1_config_options(options: &mut Value) {
    let Some(options) = options.as_array_mut() else {
        return;
    };
    for option in options.iter_mut().filter_map(Value::as_object_mut) {
        if let Some(id) = option.remove("configId") {
            option.insert("id".into(), id);
        }
        if let Some(Value::Array(choices)) = option.get_mut("options") {
            for group in choices.iter_mut().filter_map(Value::as_object_mut) {
                if let Some(id) = group.remove("groupId") {
                    group.insert("group".into(), id);
                }
            }
        }
    }
}

/// v2 tags a command's free-text input with `type: "text"`, the only kind v1 has.
fn v1_commands(commands: Vec<Value>) -> Vec<Value> {
    commands
        .into_iter()
        .map(|mut command| {
            if let Some(command) = command.as_object_mut()
                && let Some(Value::Object(mut input)) = command.remove("input")
                && input
                    .remove("type")
                    .and_then(|kind| kind.as_str().map(str::to_string))
                    == Some("text".to_string())
            {
                command.insert("input".into(), Value::Object(input));
            }
            command
        })
        .collect()
}

/// v2 tags every MCP server, stdio ones too, and has no SSE servers.
fn v2_mcp_servers(params: &mut Map<String, Value>) {
    let Some(Value::Array(servers)) = params.get_mut("mcpServers") else {
        return;
    };
    servers.retain(|server| server.get("type").and_then(Value::as_str) != Some("sse"));
    for server in servers.iter_mut().filter_map(Value::as_object_mut) {
        server
            .entry("type")
            .or_insert_with(|| Value::String("stdio".into()));
    }
}

/// v1's plan is a list of items, which v2's plans of items replace whole.
fn v1_plan(update: Map<String, Value>) -> Value {
    let plan = update.get("plan");
    if plan
        .and_then(|plan| plan.get("type"))
        .and_then(Value::as_str)
        != Some("items")
    {
        return Value::Object(update);
    }
    let mut v1 = json!({
        "sessionUpdate": "plan",
        "entries": plan.and_then(|plan| plan.get("entries")).cloned().unwrap_or_else(|| json!([])),
    });
    if let Some(meta) = update.get("_meta").filter(|meta| !meta.is_null()) {
        v1["_meta"] = meta.clone();
    }
    v1
}

/// v1's chunks for a whole message, which v2 sends as it replays one (empty first, to clear
/// it). v1 can't clear a message, so an empty one has none.
fn message_chunks(update: &Map<String, Value>, kind: &str) -> Vec<Value> {
    let message_id = update.get("messageId").filter(|id| !id.is_null());
    let Some(Value::Array(content)) = update.get("content") else {
        return Vec::new();
    };
    content
        .iter()
        .filter(|block| is_v1_content_block(block))
        .map(|block| {
            let mut chunk = json!({"sessionUpdate": kind, "content": block});
            if let Some(id) = message_id {
                chunk["messageId"] = id.clone();
            }
            chunk
        })
        .collect()
}

fn is_v1_content_block(block: &Value) -> bool {
    matches!(
        block.get("type").and_then(Value::as_str),
        Some("text" | "image" | "audio" | "resource_link" | "resource")
    )
}

/// v1's answer to a prompt, from the `idle` that ended its turn.
fn turn_answer(idle: &Map<String, Value>) -> Result<Value, Value> {
    let stop_reason = idle
        .get("stopReason")
        .and_then(Value::as_str)
        .unwrap_or("end_turn");
    if stop_reason == "error" {
        return Err(idle
            .get("error")
            .filter(|error| error.is_object())
            .cloned()
            .unwrap_or_else(|| json!({"code": -32603, "message": "Internal error"})));
    }
    let stop_reason = match stop_reason {
        "end_turn" | "max_tokens" | "max_turn_requests" | "refusal" | "cancelled" => stop_reason,
        _ => "end_turn",
    };
    let mut result = json!({"stopReason": stop_reason});
    for key in ["usage", "_meta"] {
        if let Some(value) = idle.get(key).filter(|value| !value.is_null()) {
            result[key] = value.clone();
        }
    }
    Ok(result)
}

/// v1's fields for v2's patch of a tool call. v2's `null` clears a field, which v1 can do only
/// for its lists; v2's statuses and kinds that v1 lacks become the nearest it has. v2's tool
/// name goes where Claude Agent sends it on v1, which the thread reads.
fn v1_tool_call_fields(fields: Map<String, Value>) -> Map<String, Value> {
    let mut fields: Map<String, Value> = fields
        .into_iter()
        .filter_map(|(key, value)| {
            let value = match (key.as_str(), value) {
                ("sessionUpdate", _) => return None,
                ("locations", Value::Null) => json!([]),
                (_, Value::Null) => return None,
                ("status", Value::String(status)) => match status.as_str() {
                    "pending" | "in_progress" | "completed" | "failed" => Value::String(status),
                    "cancelled" => "failed".into(),
                    _ => return None,
                },
                ("kind", Value::String(kind)) if !TOOL_KINDS.contains(&kind.as_str()) => {
                    "other".into()
                }
                (_, value) => value,
            };
            Some((key, value))
        })
        .collect();
    if let Some(Value::String(name)) = fields.remove("name") {
        let meta = fields.entry("_meta").or_insert_with(|| json!({}));
        if let Some(meta) = meta.as_object_mut()
            && let Some(claude) = meta
                .entry("claudeCode")
                .or_insert_with(|| json!({}))
                .as_object_mut()
        {
            claude.entry("toolName").or_insert(Value::String(name));
        }
    }
    fields
}

fn decode_output(data: &str) -> Option<String> {
    base64::engine::general_purpose::STANDARD
        .decode(data)
        .ok()
        .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
}

fn keep_end(text: &mut String, limit: usize) {
    if text.len() <= limit {
        return;
    }
    let mut start = text.len() - limit;
    while !text.is_char_boundary(start) {
        start += 1;
    }
    text.drain(..start);
}

/// v1's diffs for a v2 diff, from its git patch: one for each hunk, with the lines it has
/// before and after the change. v1 can't say what a diff without a patch changed.
fn v1_diffs(diff: &Value) -> Vec<Value> {
    let Some(text) = diff.pointer("/patch/text").and_then(Value::as_str) else {
        return Vec::new();
    };
    let paths: Vec<&str> = diff
        .get("changes")
        .and_then(Value::as_array)
        .map(|changes| {
            changes
                .iter()
                .filter_map(|change| change.get("path").and_then(Value::as_str))
                .collect()
        })
        .unwrap_or_default();
    let files = parse_patch(text);
    let named_by_changes = files.len() == paths.len();
    let mut diffs = Vec::new();
    for (index, file) in files.into_iter().enumerate() {
        let path = if named_by_changes {
            paths[index].to_string()
        } else {
            file.path
        };
        for hunk in file.hunks {
            diffs.push(json!({
                "type": "diff",
                "path": path,
                "oldText": (!file.created).then_some(hunk.old),
                "newText": hunk.new,
            }));
        }
    }
    diffs
}

struct PatchFile {
    path: String,
    created: bool,
    hunks: Vec<Hunk>,
}

#[derive(Default)]
struct Hunk {
    old: String,
    new: String,
}

#[derive(Clone, Copy)]
enum Side {
    Old,
    New,
    Both,
}

/// The files of a git patch, with their hunks. A hunk's line counts say where it ends, so a
/// removed line that reads like a header ("-- x") stays in it.
fn parse_patch(text: &str) -> Vec<PatchFile> {
    let mut files: Vec<PatchFile> = Vec::new();
    let mut old_path: Option<String> = None;
    let mut remaining = (0usize, 0usize);
    let mut last_side = None;
    for line in text.lines() {
        // The line before it has no newline at its end.
        if line.starts_with('\\') {
            if let (Some(hunk), Some(side)) = (
                files.last_mut().and_then(|file| file.hunks.last_mut()),
                last_side,
            ) {
                if matches!(side, Side::Old | Side::Both) && hunk.old.ends_with('\n') {
                    hunk.old.pop();
                }
                if matches!(side, Side::New | Side::Both) && hunk.new.ends_with('\n') {
                    hunk.new.pop();
                }
            }
            continue;
        }
        if remaining != (0, 0) {
            let Some(hunk) = files.last_mut().and_then(|file| file.hunks.last_mut()) else {
                remaining = (0, 0);
                continue;
            };
            let (side, text) = match line.as_bytes().first() {
                Some(b'+') => (Side::New, &line[1..]),
                Some(b'-') => (Side::Old, &line[1..]),
                Some(b' ') => (Side::Both, &line[1..]),
                _ => (Side::Both, line),
            };
            if matches!(side, Side::Old | Side::Both) {
                hunk.old.push_str(text);
                hunk.old.push('\n');
                remaining.0 = remaining.0.saturating_sub(1);
            }
            if matches!(side, Side::New | Side::Both) {
                hunk.new.push_str(text);
                hunk.new.push('\n');
                remaining.1 = remaining.1.saturating_sub(1);
            }
            last_side = Some(side);
            continue;
        }
        if let Some(path) = line.strip_prefix("--- ") {
            old_path = Some(header_path(path));
        } else if let Some(path) = line.strip_prefix("+++ ") {
            let new_path = header_path(path);
            let created = old_path.as_deref() == Some("/dev/null");
            let path = if new_path == "/dev/null" {
                old_path.take().unwrap_or_default()
            } else {
                new_path
            };
            old_path = None;
            files.push(PatchFile {
                path,
                created,
                hunks: Vec::new(),
            });
        } else if let Some(header) = line.strip_prefix("@@ ")
            && let Some(file) = files.last_mut()
        {
            remaining = hunk_counts(header);
            file.hunks.push(Hunk::default());
            last_side = None;
        }
    }
    files
}

/// A `---` or `+++` line's path, without git's `a/` or `b/` and what follows a tab.
fn header_path(path: &str) -> String {
    let path = path.split('\t').next().unwrap_or(path);
    if path.starts_with('/') {
        return path.to_string();
    }
    path.strip_prefix("a/")
        .or_else(|| path.strip_prefix("b/"))
        .unwrap_or(path)
        .to_string()
}

/// The line counts before and after the change of a hunk header's `-a,b +c,d`.
fn hunk_counts(header: &str) -> (usize, usize) {
    let mut ranges = header.split_whitespace();
    let count = |range: Option<&str>, sign: char| {
        range
            .and_then(|range| range.strip_prefix(sign))
            .map(|range| match range.split_once(',') {
                Some((_, count)) => count.parse().unwrap_or(0),
                None => 1,
            })
            .unwrap_or(0)
    };
    let old = count(ranges.next(), '-');
    let new = count(ranges.next(), '+');
    (old, new)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(value: Value) -> String {
        value.to_string()
    }

    fn parse(lines: &[String]) -> Vec<Value> {
        lines
            .iter()
            .map(|line| serde_json::from_str(line).expect("a JSON line"))
            .collect()
    }

    /// A translator that asked for v2 and got it.
    fn on_v2() -> Translator {
        let mut translator = Translator::asking();
        translator.outgoing(line(json!({
            "jsonrpc": "2.0", "id": 0, "method": "initialize",
            "params": {"protocolVersion": 1, "clientCapabilities": {}},
        })));
        translator.incoming(line(json!({
            "jsonrpc": "2.0", "id": 0,
            "result": {"protocolVersion": 2, "info": {"name": "agent", "version": "1"},
                       "capabilities": {}, "authMethods": []},
        })));
        assert!(translator.is_v2());
        translator
    }

    fn update(session: &str, update: Value) -> String {
        line(json!({"jsonrpc": "2.0", "method": "session/update",
                    "params": {"sessionId": session, "update": update}}))
    }

    #[test]
    fn asks_for_v2_and_goes_on_with_v1_unchanged() {
        let mut translator = Translator::asking();
        let sent = translator.outgoing(line(json!({
            "jsonrpc": "2.0", "id": 0, "method": "initialize",
            "params": {
                "protocolVersion": 1,
                "clientInfo": {"name": "agentZ", "version": "0.1.0"},
                "clientCapabilities": {
                    "terminal": true,
                    "auth": {"terminal": true, "_meta": {"gateway": true}},
                    "elicitation": {"form": {}},
                    "_meta": {"terminal-auth": true},
                },
            },
        })));
        let sent = parse(&sent.to_agent);
        let params = &sent[0]["params"];
        assert_eq!(params["protocolVersion"], 2);
        assert_eq!(params["info"]["name"], "agentZ");
        assert_eq!(params["clientInfo"]["name"], "agentZ");
        assert_eq!(params["clientCapabilities"]["terminal"], true);
        assert_eq!(
            params["capabilities"],
            json!({"auth": {"terminal": {}, "_meta": {"gateway": true}},
                   "elicitation": {"form": {}}, "_meta": {"terminal-auth": true}})
        );
        let answer = line(json!({"jsonrpc": "2.0", "id": 0, "result": {
            "protocolVersion": 1, "agentCapabilities": {"loadSession": true}, "authMethods": []}}));
        assert_eq!(translator.incoming(answer.clone()), vec![answer]);
        assert!(!translator.is_v2());
        let prompt = line(
            json!({"jsonrpc": "2.0", "id": 1, "method": "session/prompt",
                                 "params": {"sessionId": "s", "prompt": []}}),
        );
        assert_eq!(translator.outgoing(prompt.clone()).to_agent, vec![prompt]);
    }

    #[test]
    fn reads_v2_initialize_as_v1() {
        let mut translator = Translator::asking();
        translator.outgoing(line(
            json!({"jsonrpc": "2.0", "id": 0, "method": "initialize",
                                        "params": {"protocolVersion": 1}}),
        ));
        let answer = translator.incoming(line(json!({"jsonrpc": "2.0", "id": 0, "result": {
            "protocolVersion": 2,
            "info": {"name": "claude-agent-acp", "version": "0.94.0"},
            "capabilities": {
                "session": {"prompt": {"image": {}, "embeddedContext": {}},
                            "mcp": {"stdio": {}, "http": {}}, "fork": {}, "delete": {}},
                "_meta": {"claudeCode": {}},
            },
            "authMethods": [
                {"type": "agent", "methodId": "claude-login", "name": "Log in"},
                {"type": "terminal", "methodId": "claude-terminal", "name": "Terminal",
                 "args": ["--login"], "env": [{"name": "A", "value": "1"}]},
            ],
        }})));
        let answer = &parse(&answer)[0]["result"];
        assert_eq!(answer["protocolVersion"], 1);
        assert_eq!(answer["agentInfo"]["name"], "claude-agent-acp");
        let capabilities = &answer["agentCapabilities"];
        assert_eq!(capabilities["loadSession"], true);
        assert_eq!(
            capabilities["promptCapabilities"],
            json!({"image": true, "audio": false, "embeddedContext": true})
        );
        assert_eq!(capabilities["mcpCapabilities"]["http"], true);
        assert_eq!(capabilities["mcpCapabilities"]["acp"], false);
        assert_eq!(capabilities["sessionCapabilities"]["fork"], json!({}));
        assert_eq!(capabilities["sessionCapabilities"]["resume"], json!({}));
        assert_eq!(capabilities["auth"]["logout"], json!({}));
        assert_eq!(capabilities["_meta"], json!({"claudeCode": {}}));
        assert_eq!(
            answer["authMethods"],
            json!([
                {"id": "claude-login", "name": "Log in"},
                {"type": "terminal", "id": "claude-terminal", "name": "Terminal",
                 "args": ["--login"], "env": {"A": "1"}},
            ])
        );
        let parsed: agent_client_protocol::schema::v1::InitializeResponse =
            serde_json::from_value(answer.clone()).expect("v1's InitializeResponse");
        assert!(parsed.agent_capabilities.load_session);
    }

    #[test]
    fn answers_a_prompt_when_its_turn_goes_idle() {
        let mut translator = on_v2();
        translator.outgoing(line(
            json!({"jsonrpc": "2.0", "id": 5, "method": "session/prompt",
                                        "params": {"sessionId": "s", "prompt": []}}),
        ));
        // Its own prompt shows in the thread already.
        let echo = update(
            "s",
            json!({"sessionUpdate": "user_message", "messageId": "u1",
                                      "content": [{"type": "text", "text": "hi"}]}),
        );
        assert!(translator.incoming(echo).is_empty());
        let receipt = line(json!({"jsonrpc": "2.0", "id": 5, "result": {"messageId": "u1"}}));
        assert!(translator.incoming(receipt).is_empty());
        let running = update(
            "s",
            json!({"sessionUpdate": "state_update", "state": "running"}),
        );
        assert_eq!(
            parse(&translator.incoming(running))[0]["params"]["update"]["state"],
            "running"
        );
        let idle = translator.incoming(update(
            "s",
            json!({
            "sessionUpdate": "state_update", "state": "idle", "stopReason": "cancelled",
            "usage": {"totalTokens": 3, "inputTokens": 2, "outputTokens": 1}}),
        ));
        let idle = parse(&idle);
        assert_eq!(idle.len(), 2);
        assert_eq!(idle[0]["params"]["update"]["state"], "idle");
        assert_eq!(idle[1]["id"], 5);
        assert_eq!(idle[1]["result"]["stopReason"], "cancelled");
        assert_eq!(idle[1]["result"]["usage"]["totalTokens"], 3);
        // The agent's own work after it answers nothing.
        let idle = translator.incoming(update(
            "s",
            json!({"sessionUpdate": "state_update",
                                                          "state": "idle"}),
        ));
        assert_eq!(idle.len(), 1);
    }

    #[test]
    fn fails_a_prompt_whose_turn_failed() {
        let mut translator = on_v2();
        translator.outgoing(line(
            json!({"jsonrpc": "2.0", "id": 5, "method": "session/prompt",
                                        "params": {"sessionId": "s", "prompt": []}}),
        ));
        translator.incoming(line(
            json!({"jsonrpc": "2.0", "id": 5, "result": {"messageId": "u"}}),
        ));
        let idle = parse(&translator.incoming(update(
            "s",
            json!({
            "sessionUpdate": "state_update", "state": "idle", "stopReason": "error",
            "error": {"code": -32000, "message": "Authentication required"}}),
        )));
        assert_eq!(idle[1]["error"]["code"], -32000);
    }

    #[test]
    fn reports_a_steered_prompt_taken_in() {
        let mut translator = on_v2();
        translator.outgoing(line(
            json!({"jsonrpc": "2.0", "id": 5, "method": "session/prompt",
                                        "params": {"sessionId": "s", "prompt": []}}),
        ));
        translator.incoming(update(
            "s",
            json!({"sessionUpdate": "user_message", "messageId": "u1"}),
        ));
        let steered = translator.outgoing(line(json!({
            "jsonrpc": "2.0", "id": 6, "method": "session/prompt",
            "params": {"sessionId": "s", "prompt": [], "_meta": {STEERED_PROMPT_META: true}}})));
        assert!(parse(&steered.to_agent)[0]["params"].get("_meta").is_none());
        // Claude Agent takes it in once the first turn ends.
        let idle = parse(&translator.incoming(update(
            "s",
            json!({
            "sessionUpdate": "state_update", "state": "idle", "stopReason": "end_turn"}),
        )));
        assert_eq!(idle.len(), 2);
        assert_eq!(idle[1]["id"], 5);
        let taken = parse(&translator.incoming(update(
            "s",
            json!({
            "sessionUpdate": "user_message", "messageId": "u2"}),
        )));
        assert_eq!(taken[0]["method"], PROMPT_TAKEN_NOTIFICATION);
        assert_eq!(taken[0]["params"]["sessionId"], "s");
        // Its answer comes after: v1's was given at the idle already.
        let idle = parse(&translator.incoming(update(
            "s",
            json!({
            "sessionUpdate": "state_update", "state": "idle", "stopReason": "end_turn"}),
        )));
        assert_eq!(idle[1]["id"], 6);
        assert!(
            translator
                .incoming(line(
                    json!({"jsonrpc": "2.0", "id": 6, "result": {"messageId": "u2"}})
                ))
                .is_empty()
        );
    }

    #[test]
    fn takes_a_prompt_in_by_its_answer_before_its_echo() {
        let mut translator = on_v2();
        translator.outgoing(line(json!({
            "jsonrpc": "2.0", "id": 6, "method": "session/prompt",
            "params": {"sessionId": "s", "prompt": [], "_meta": {STEERED_PROMPT_META: true}}})));
        let taken = parse(&translator.incoming(line(json!({
            "jsonrpc": "2.0", "id": 6, "result": {"messageId": "u2"}}))));
        assert_eq!(taken[0]["method"], PROMPT_TAKEN_NOTIFICATION);
        let echo = update(
            "s",
            json!({"sessionUpdate": "user_message", "messageId": "u2",
                                      "content": [{"type": "text", "text": "more"}]}),
        );
        assert!(translator.incoming(echo).is_empty());
    }

    #[test]
    fn passes_a_prompt_ended_before_it_was_taken_in() {
        let mut translator = on_v2();
        translator.outgoing(line(
            json!({"jsonrpc": "2.0", "id": 5, "method": "session/prompt",
                                        "params": {"sessionId": "s", "prompt": []}}),
        ));
        let cancelled = line(json!({"jsonrpc": "2.0", "id": 5,
                                    "error": {"code": -32800, "message": "Request cancelled"}}));
        assert_eq!(translator.incoming(cancelled.clone()), vec![cancelled]);
        let idle = translator.incoming(update(
            "s",
            json!({"sessionUpdate": "state_update",
                                                          "state": "idle"}),
        ));
        assert_eq!(idle.len(), 1);
    }

    #[test]
    fn loads_a_session_by_resuming_it_with_its_replay() {
        let mut translator = on_v2();
        let sent = translator.outgoing(line(json!({
            "jsonrpc": "2.0", "id": 2, "method": "session/load",
            "params": {"sessionId": "s", "cwd": "/p", "mcpServers": [
                {"name": "a", "command": "/bin/a", "args": [], "env": []},
                {"type": "sse", "name": "b", "url": "http://b", "headers": []}]}})));
        let sent = &parse(&sent.to_agent)[0];
        assert_eq!(sent["method"], "session/resume");
        assert_eq!(sent["params"]["replayFrom"], json!({"type": "start"}));
        assert_eq!(
            sent["params"]["mcpServers"],
            json!([
            {"type": "stdio", "name": "a", "command": "/bin/a", "args": [], "env": []}])
        );
        // v2 clears each message before replaying it.
        let cleared = update(
            "s",
            json!({"sessionUpdate": "user_message", "messageId": "u1",
                                         "content": []}),
        );
        assert!(translator.incoming(cleared).is_empty());
        let replayed = parse(&translator.incoming(update(
            "s",
            json!({
            "sessionUpdate": "user_message", "messageId": "u1",
            "content": [{"type": "text", "text": "hello"}]}),
        )));
        assert_eq!(
            replayed[0]["params"]["update"]["sessionUpdate"],
            "user_message_chunk"
        );
        assert_eq!(replayed[0]["params"]["update"]["content"]["text"], "hello");
        let answer = parse(&translator.incoming(line(
            json!({"jsonrpc": "2.0", "id": 2, "result": {
                "configOptions": [{"configId": "mode", "name": "Mode", "type": "select",
                                   "currentValue": "default",
                                   "options": [{"groupId": "g", "name": "G", "options": []}]}],
                "availableCommands": [{"name": "compact", "description": "Compact",
                                       "input": {"type": "text", "hint": "focus"}}],
            }}),
        )));
        assert_eq!(answer.len(), 2);
        let option = &answer[0]["result"]["configOptions"][0];
        assert_eq!(option["id"], "mode");
        assert_eq!(option["options"][0]["group"], "g");
        assert!(answer[0]["result"].get("availableCommands").is_none());
        assert_eq!(
            answer[1]["params"]["update"],
            json!({"sessionUpdate": "available_commands_update", "availableCommands": [
                {"name": "compact", "description": "Compact", "input": {"hint": "focus"}}]})
        );
        assert!(translator.replaying.is_empty());
    }

    #[test]
    fn renames_requests_v2_names_otherwise() {
        let mut translator = on_v2();
        let sent = translator.outgoing(line(json!({"jsonrpc": "2.0", "id": 3,
            "method": "authenticate", "params": {"methodId": "login"}})));
        assert_eq!(parse(&sent.to_agent)[0]["method"], "auth/login");
        let sent = translator.outgoing(line(json!({"jsonrpc": "2.0", "id": 4,
            "method": "session/set_config_option",
            "params": {"sessionId": "s", "configId": "model", "value": "opus"}})));
        assert_eq!(parse(&sent.to_agent)[0]["params"]["type"], "id");
        let refused = translator.outgoing(line(json!({"jsonrpc": "2.0", "id": 7,
            "method": "session/set_mode", "params": {"sessionId": "s", "modeId": "plan"}})));
        assert!(refused.to_agent.is_empty());
        assert_eq!(parse(&refused.to_sdk)[0]["error"]["code"], -32601);
    }

    #[test]
    fn asks_permission_about_the_subject_tool_call() {
        let mut translator = on_v2();
        let request = parse(&translator.incoming(line(json!({
            "jsonrpc": "2.0", "id": 9, "method": "session/request_permission",
            "params": {"sessionId": "s", "title": "Allow?", "options": [],
                       "subject": {"type": "tool_call", "toolCall": {
                           "toolCallId": "t", "title": "rm a", "rawOutput": null,
                           "status": "pending"}}}}))));
        let params = &request[0]["params"];
        assert_eq!(
            params["toolCall"],
            json!({"toolCallId": "t", "title": "rm a",
                                              "status": "pending"})
        );
        assert!(params.get("title").is_none());
        let request = parse(&translator.incoming(line(json!({
            "jsonrpc": "2.0", "id": 10, "method": "session/request_permission",
            "params": {"sessionId": "s", "title": "Run ls?", "options": [],
                       "subject": {"type": "command", "command": "ls", "cwd": "/p"}}}))));
        let tool_call = &request[0]["params"]["toolCall"];
        assert_eq!(tool_call["title"], "Run ls?");
        assert_eq!(tool_call["kind"], "execute");
        assert_eq!(tool_call["rawInput"]["command"], "ls");
        let parsed: agent_client_protocol::schema::v1::RequestPermissionRequest =
            serde_json::from_value(request[0]["params"].clone()).expect("v1's request");
        assert_eq!(
            parsed.tool_call.tool_call_id.0.as_ref(),
            "agentz-permission-1"
        );
    }

    #[test]
    fn reports_tool_calls_as_v1_does() {
        let mut translator = on_v2();
        let reported = parse(&translator.incoming(update(
            "s",
            json!({
            "sessionUpdate": "tool_call_update", "toolCallId": "t", "title": "Run",
            "kind": "execute", "status": "cancelled", "rawInput": null, "name": "Bash",
            "content": [{"type": "terminal", "terminalId": "t1"}]}),
        )));
        let reported = &reported[0]["params"]["update"];
        assert_eq!(reported["status"], "failed");
        assert_eq!(reported["_meta"]["claudeCode"]["toolName"], "Bash");
        assert!(reported.get("rawInput").is_none());
        assert_eq!(reported["content"], json!([]));
        let output = base64::engine::general_purpose::STANDARD.encode("one\n");
        let shown = parse(&translator.incoming(update(
            "s",
            json!({
            "sessionUpdate": "terminal_update", "terminalId": "t1",
            "output": {"data": output}}),
        )));
        let more = base64::engine::general_purpose::STANDARD.encode("two\n");
        assert_eq!(
            shown[0]["params"]["update"]["content"][0]["content"]["text"],
            "```console\none\n```"
        );
        let shown = parse(&translator.incoming(update(
            "s",
            json!({
            "sessionUpdate": "terminal_output_chunk", "terminalId": "t1", "data": more}),
        )));
        assert_eq!(
            shown[0]["params"]["update"]["content"][0]["content"]["text"],
            "```console\none\ntwo\n```"
        );
        let chunk = parse(&translator.incoming(update(
            "s",
            json!({
            "sessionUpdate": "tool_call_content_chunk", "toolCallId": "t",
            "content": {"type": "content", "content": {"type": "text", "text": "done"}}}),
        )));
        assert_eq!(
            chunk[0]["params"]["update"]["content"]
                .as_array()
                .map(Vec::len),
            Some(2)
        );
        let plan = parse(&translator.incoming(update(
            "s",
            json!({
            "sessionUpdate": "plan_update",
            "plan": {"type": "items", "planId": "plan", "entries": [
                {"content": "a", "priority": "high", "status": "pending"}]}}),
        )));
        assert_eq!(plan[0]["params"]["update"]["sessionUpdate"], "plan");
        assert_eq!(plan[0]["params"]["update"]["entries"][0]["content"], "a");
    }

    #[test]
    fn reads_diffs_from_their_patches() {
        let patch = "diff --git /p/a.rs /p/a.rs\n--- /p/a.rs\n+++ /p/a.rs\n\
                     @@ -1,3 +1,3 @@\n one\n-two\n+2\n three\n\
                     @@ -10,1 +10,2 @@\n--- not a header\n+ten\n+eleven\n\\ No newline at end of file\n\
                     diff --git /p/b.rs /p/b.rs\nnew file mode 100644\n--- /dev/null\n+++ /p/b.rs\n\
                     @@ -0,0 +1 @@\n+new\n";
        let diffs = v1_diffs(&json!({
            "type": "diff",
            "changes": [{"operation": "modify", "path": "/p/a.rs"},
                        {"operation": "add", "path": "/p/b.rs"}],
            "patch": {"format": "git_patch", "text": patch},
        }));
        assert_eq!(
            diffs,
            vec![
                json!({"type": "diff", "path": "/p/a.rs", "oldText": "one\ntwo\nthree\n",
                       "newText": "one\n2\nthree\n"}),
                json!({"type": "diff", "path": "/p/a.rs", "oldText": "-- not a header\n",
                       "newText": "ten\neleven"}),
                json!({"type": "diff", "path": "/p/b.rs", "oldText": null, "newText": "new\n"}),
            ]
        );
        assert!(
            v1_diffs(&json!({"type": "diff", "changes": [
            {"operation": "modify", "path": "/p/a.rs"}]}))
            .is_empty()
        );
    }

    #[test]
    fn carries_its_state_to_another_connection() {
        let mut translator = on_v2();
        translator.outgoing(line(
            json!({"jsonrpc": "2.0", "id": 5, "method": "session/prompt",
                                        "params": {"sessionId": "s", "prompt": []}}),
        ));
        let carried: Translator =
            serde_json::from_str(&serde_json::to_string(&translator).expect("serialized"))
                .expect("deserialized");
        let mut translator = carried;
        assert!(translator.is_v2());
        translator.incoming(line(
            json!({"jsonrpc": "2.0", "id": 5, "result": {"messageId": "u"}}),
        ));
        let idle = parse(&translator.incoming(update(
            "s",
            json!({
            "sessionUpdate": "state_update", "state": "idle", "stopReason": "end_turn"}),
        )));
        assert_eq!(idle[1]["id"], 5);
    }
}
