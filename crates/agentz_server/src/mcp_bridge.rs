//! `agentz-server mcp-bridge`: the `agentz` MCP server agents get with each session. It speaks
//! MCP's stdio transport (newline-delimited JSON-RPC) to the agent, and passes `tools/list` and
//! `tools/call` on to the server, with the session's credential so the server knows which
//! thread is calling. Calls are answered as they finish, so a waiting tool doesn't hold up the
//! others.

use agentz_protocol::{ClientKind, Request, Response, ToolCaller};
use agentz_server::mcp_tools;
use anyhow::{Context as _, Result};
use futures::StreamExt as _;
use futures::channel::mpsc;
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt as _, AsyncWriteExt as _, BufReader};

/// The newest MCP revision this bridge speaks; older ones a client asks for are accepted, since
/// tools work the same in all of them.
const LATEST_PROTOCOL_VERSION: &str = "2025-06-18";
const SUPPORTED_PROTOCOL_VERSIONS: &[&str] = &["2024-11-05", "2025-03-26", "2025-06-18"];

pub(crate) async fn run() -> Result<()> {
    let token = std::env::var("AGENTZ_MCP_TOKEN").context("AGENTZ_MCP_TOKEN isn't set")?;
    let mut connection = crate::connect_for_tools(ClientKind::Mcp).await?;

    let (outgoing, mut outgoing_messages) = mpsc::unbounded::<Value>();
    let writer = tokio::spawn(async move {
        let mut stdout = tokio::io::stdout();
        while let Some(message) = outgoing_messages.next().await {
            let mut line = message.to_string();
            line.push('\n');
            stdout.write_all(line.as_bytes()).await?;
            stdout.flush().await?;
        }
        anyhow::Ok(())
    });

    let mut lines = BufReader::new(tokio::io::stdin()).lines();
    while let Some(line) = lines.next_line().await.context("reading stdin")? {
        if line.trim().is_empty() {
            continue;
        }
        let message: Value = match serde_json::from_str(&line) {
            Ok(message) => message,
            Err(error) => {
                send(
                    &outgoing,
                    error_response(Value::Null, -32700, &format!("parse error: {error}")),
                );
                continue;
            }
        };
        let Some(method) = message["method"].as_str().map(String::from) else {
            // A response to a request of ours, and the bridge sends none.
            continue;
        };
        let Some(id) = message.get("id").cloned() else {
            // Notifications (`initialized`, `cancelled`) need no answer.
            continue;
        };
        let params = message.get("params").cloned().unwrap_or(Value::Null);
        match method.as_str() {
            "initialize" => {
                let requested = params["protocolVersion"].as_str().unwrap_or_default();
                let protocol_version = if SUPPORTED_PROTOCOL_VERSIONS.contains(&requested) {
                    requested
                } else {
                    LATEST_PROTOCOL_VERSION
                };
                send(
                    &outgoing,
                    result_response(
                        id,
                        json!({
                            "protocolVersion": protocol_version,
                            "capabilities": mcp_tools::capabilities(),
                            "serverInfo": mcp_tools::server_info(),
                            "instructions": mcp_tools::INSTRUCTIONS,
                        }),
                    ),
                );
            }
            "ping" => send(&outgoing, result_response(id, json!({}))),
            "tools/list" | "tools/call" if connection.is_closed() => {
                // The server was updated: the new one listens on the same socket, and knows
                // this bridge's credential.
                match crate::connect_for_tools(ClientKind::Mcp).await {
                    Ok(reconnected) => connection = reconnected,
                    Err(error) => {
                        send(&outgoing, error_response(id, -32603, &format!("{error:#}")));
                        continue;
                    }
                }
                handle(&connection, &outgoing, &token, id, &method, params);
            }
            "tools/list" | "tools/call" => {
                handle(&connection, &outgoing, &token, id, &method, params)
            }
            _ => send(
                &outgoing,
                error_response(id, -32601, &format!("method not found: {method}")),
            ),
        }
    }
    // The agent closed stdin, so it's done with the tools. Calls still waiting end with the
    // process.
    writer.abort();
    Ok(())
}

/// Passes a `tools/list` or `tools/call` on to the server, answering when it does.
fn handle(
    connection: &agentz_client::Connection,
    outgoing: &mpsc::UnboundedSender<Value>,
    token: &str,
    id: Value,
    method: &str,
    params: Value,
) {
    match method {
        "tools/list" => {
            let response = connection.request(Request::ListTools {
                caller: Some(ToolCaller::Session(token.to_string())),
            });
            let outgoing = outgoing.clone();
            tokio::spawn(async move {
                let message = match response.await {
                    Ok(Response::Tools(tools)) => result_response(id, json!({"tools": tools})),
                    Ok(response) => {
                        error_response(id, -32603, &format!("unexpected response: {response:?}"))
                    }
                    Err(error) => error_response(id, -32603, &format!("{error:#}")),
                };
                send(&outgoing, message);
            });
        }
        "tools/call" => {
            let Some(name) = params["name"].as_str() else {
                send(outgoing, error_response(id, -32602, "name is required"));
                return;
            };
            let response = connection.request(Request::CallTool {
                caller: ToolCaller::Session(token.to_string()),
                name: name.to_string(),
                arguments: params.get("arguments").cloned().unwrap_or(Value::Null),
            });
            let outgoing = outgoing.clone();
            tokio::spawn(async move {
                let message = match response.await {
                    Ok(Response::ToolResult(result)) => {
                        result_response(id, mcp_tools::call_result(result))
                    }
                    Ok(response) => {
                        error_response(id, -32603, &format!("unexpected response: {response:?}"))
                    }
                    Err(error) => {
                        result_response(id, mcp_tools::call_failure(format!("{error:#}")))
                    }
                };
                send(&outgoing, message);
            });
        }
        _ => {}
    }
}

fn send(outgoing: &mpsc::UnboundedSender<Value>, message: Value) {
    // Fails only once the writer has stopped, after stdout closed.
    outgoing.unbounded_send(message).ok();
}

fn result_response(id: Value, result: Value) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "result": result})
}

fn error_response(id: Value, code: i64, message: &str) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message}})
}
