//! agentZ's tools on the agent's own ACP connection (MCP-over-ACP), for agents that take MCP
//! servers declared there (`mcpCapabilities.acp`), in place of the `mcp-bridge` process: the
//! agent sends each MCP request as an `mcp/message`. The server id agentZ declares with the
//! session is the thread's tool credential, so a call has the same caller, and so the same
//! project, account and policy, as through the bridge. Each call is answered as it finishes,
//! and one the agent cancels stops waiting, as a call whose bridge went away does.

use std::sync::atomic::Ordering;

use agent_client_protocol::Responder;
use agent_client_protocol::mcp_server::MCP_SERVER_UNAVAILABLE;
use agent_client_protocol::schema::v1 as acp;
use agentz_protocol::{Response, ServerMessage, ToolCaller};
use futures::StreamExt as _;
use futures::channel::mpsc;
use futures::future::{self, Either};
use serde_json::{Map, Value, json};
use util::ResultExt as _;

use super::{Client, Input, Server};
use crate::mcp_tools;

/// The MCP revision ACP's binding speaks: stateless, with no `initialize`.
const MCP_VERSION: &str = "2026-07-28";

impl Server {
    /// Passes the `mcp/message` requests of every thread's agent on to the server.
    pub(super) fn agent_mcp_host(&self) -> agent_thread::McpHost {
        let inputs = self.inputs.clone();
        std::sync::Arc::new(move |request, responder| {
            inputs
                .unbounded_send(Input::AgentMcp(request, responder))
                .ok();
        })
    }

    pub(super) fn answer_mcp_message(
        &mut self,
        request: acp::MessageMcpRequest,
        responder: Responder<acp::MessageMcpResponse>,
    ) {
        let token = request.server_id.0.to_string();
        if !self.tool_sessions.contains_key(&token) {
            responder
                .respond_with_error(agent_client_protocol::Error::new(
                    MCP_SERVER_UNAVAILABLE,
                    "This agent's agentZ credential is no longer valid.",
                ))
                .log_err();
            return;
        }
        if let Some(error) = request_problem(&request.method, request.params.as_ref()) {
            responder
                .respond(acp::MessageMcpResponse::error(error))
                .log_err();
            return;
        }
        let caller = ToolCaller::Session(token);
        let params = request.params.unwrap_or_default();
        let response = match request.method.as_str() {
            "server/discover" => acp::MessageMcpResponse::success(json!({
                "supportedVersions": [MCP_VERSION],
                "capabilities": mcp_tools::capabilities(),
                "serverInfo": mcp_tools::server_info(),
                "instructions": mcp_tools::INSTRUCTIONS,
            })),
            "ping" => acp::MessageMcpResponse::success(json!({})),
            "tools/list" => acp::MessageMcpResponse::success(
                json!({"tools": self.tool_definitions(Some(caller))}),
            ),
            "tools/call" => match params.get("name").and_then(Value::as_str) {
                Some(name) => {
                    let arguments = params.get("arguments").cloned().unwrap_or(Value::Null);
                    return self.call_tool_for_agent(
                        caller,
                        name.to_string(),
                        arguments,
                        responder,
                    );
                }
                None => {
                    acp::MessageMcpResponse::error(acp::McpError::new(-32602, "name is required"))
                }
            },
            method => acp::MessageMcpResponse::error(acp::McpError::new(
                -32601,
                format!("method not found: {method}"),
            )),
        };
        responder.respond(response).log_err();
    }

    /// Runs the call for a client of its own, which ends with the call: the call's answer goes
    /// to it, and once it's gone, a call still waiting is dropped and background work's
    /// answer goes nowhere.
    fn call_tool_for_agent(
        &mut self,
        caller: ToolCaller,
        name: String,
        arguments: Value,
        responder: Responder<acp::MessageMcpResponse>,
    ) {
        let client = self.next_client_id.fetch_add(1, Ordering::Relaxed);
        let (outgoing, mut messages) = mpsc::unbounded();
        self.clients.insert(
            client,
            Client {
                outgoing,
                subscribed_to_session: false,
                threads: Default::default(),
                terminals: Default::default(),
            },
        );
        let cancellation = responder.cancellation();
        let inputs = self.inputs.clone();
        self.runtime.spawn(async move {
            let answer = async {
                while let Some(message) = messages.next().await {
                    if let ServerMessage::Response { result, .. } = message {
                        return Some(result);
                    }
                }
                None
            };
            let response =
                match future::select(Box::pin(answer), Box::pin(cancellation.cancelled())).await {
                    Either::Left((Some(Ok(Response::ToolResult(result))), _)) => Ok(
                        acp::MessageMcpResponse::success(mcp_tools::call_result(result)),
                    ),
                    Either::Left((Some(Ok(response)), _)) => Ok(acp::MessageMcpResponse::error(
                        acp::McpError::new(-32603, format!("unexpected response: {response:?}")),
                    )),
                    Either::Left((Some(Err(error)), _)) => Ok(acp::MessageMcpResponse::success(
                        mcp_tools::call_failure(error.message),
                    )),
                    // The server stopped.
                    Either::Left((None, _)) => Err(agent_client_protocol::Error::new(
                        MCP_SERVER_UNAVAILABLE,
                        "agentZ stopped",
                    )),
                    Either::Right(((), _)) => {
                        Err(agent_client_protocol::Error::request_cancelled())
                    }
                };
            responder.respond_with_result(response).log_err();
            inputs
                .unbounded_send(Input::Run(Box::new(move |server: &mut Server| {
                    server.clients.remove(&client);
                })))
                .ok();
        });
        self.call_tool(client, 0, caller, name, arguments);
    }
}

/// Why ACP's binding turns the request down, if it does: it takes only MCP's stateless
/// revision, which names itself and the client's capabilities in every request's `_meta`, as
/// the SDK's own MCP servers check.
fn request_problem(method: &str, params: Option<&Map<String, Value>>) -> Option<acp::McpError> {
    if method == "initialize" {
        return Some(
            acp::McpError::new(-32601, "Method not found")
                .data(json!("native MCP requests do not use initialize")),
        );
    }
    let invalid =
        |problem: &str| Some(acp::McpError::new(-32602, "Invalid params").data(json!(problem)));
    let Some(meta) = params
        .and_then(|params| params.get("_meta"))
        .and_then(Value::as_object)
    else {
        return invalid("inner params._meta must be an object");
    };
    let Some(version) = meta
        .get("io.modelcontextprotocol/protocolVersion")
        .and_then(Value::as_str)
    else {
        return invalid("inner params._meta requires io.modelcontextprotocol/protocolVersion");
    };
    if version != MCP_VERSION {
        return Some(
            acp::McpError::new(-32022, "Unsupported protocol version")
                .data(json!({"requested": version, "supported": [MCP_VERSION]})),
        );
    }
    if !meta
        .get("io.modelcontextprotocol/clientCapabilities")
        .is_some_and(Value::is_object)
    {
        return invalid(
            "inner params._meta requires io.modelcontextprotocol/clientCapabilities object",
        );
    }
    None
}
