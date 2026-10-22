//! What agentZ's own `agentz` MCP server answers, the same through the `mcp-bridge`'s stdio and
//! on the agent's ACP connection.

use agentz_protocol::ToolResult;
use serde_json::{Value, json};

pub const INSTRUCTIONS: &str = "Tools for managing the threads of this agentZ project: list and read \
threads, start new ones, message, wait for, interrupt, rename and archive them, read the changes \
each thread made (agentz_thread_diff), work in git worktrees and copy-on-write pastures \
(agentz_workspace_*, and workspaceStrategy when launching or delegating), and delegate tasks to \
child agents. A delegated task is \
child work owned by this thread: use delegate_task, or delegate_tasks for several at once (see \
orchestrator_capabilities for agents and models), keep each taskId, and use task_send, task_status \
or task_cancel to manage it. Delegating returns at once, and each task's end is announced to this \
thread with its result, queued until this turn ends, so end the turn instead of polling, waiting \
or spawning watchers; use task_status only when a result is needed mid-turn. agentz_thread_launch and \
create_threads make ordinary top-level threads: use them only when the user asks for separate or \
new threads, never merely because they said subagent. When this project is also on other machines (orchestrator_capabilities' otherMachines), the thread tools take machine to work there; thread ids are per machine. agentz_document_publish publishes a Markdown document for the user to read in their browser, and agentz_design_publish a design of variants to pick from, each with a version per publish; agentz_artifact_list and agentz_artifact_read find them again on any machine, a design with the user's picks and comments.";

pub fn server_info() -> Value {
    json!({"name": "agentz", "title": "agentZ", "version": env!("CARGO_PKG_VERSION")})
}

pub fn capabilities() -> Value {
    json!({"tools": {"listChanged": false}})
}

/// A tool's result as `tools/call` answers it: the value as text, and as structured content too
/// when it's an object.
pub fn call_result(result: ToolResult) -> Value {
    let text =
        serde_json::to_string_pretty(&result.value).unwrap_or_else(|_| result.value.to_string());
    let mut tool_result = json!({
        "content": [{"type": "text", "text": text}],
        "isError": result.is_error,
    });
    if result.value.is_object() {
        tool_result["structuredContent"] = result.value;
    }
    tool_result
}

/// A call that couldn't be run. MCP reports such failures as tool errors, so the agent sees
/// them.
pub fn call_failure(error: impl std::fmt::Display) -> Value {
    json!({
        "content": [{"type": "text", "text": format!("agentZ: {error}")}],
        "isError": true,
    })
}
