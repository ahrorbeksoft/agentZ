"""A minimal ACP agent over stdio, used by agent_thread's tests.

It answers initialize and session/new, and replies to every prompt by streaming
"Echo: <prompt>" and a completed tool call, then ending the turn. A prompt of
"permission" first asks the client for permission and reports the chosen option.
A prompt of "mcp" starts the first stdio MCP server given in session/new (or
session/load), calls its first tool, and replies "MCP: <tool result>"; "mcp <tool>
<json arguments>" calls that tool instead. A prompt of "slow" streams
"One two three four five" a word at a time, 200 ms apart. "write <path> <text>"
writes the text and a newline to the file, relative to the session's folder, and
"delete <path>" removes it.
"""
import json
import os
import subprocess
import sys
import time

# Optional path where conversations are recorded so `session/load` can replay them.
HISTORY_PATH = sys.argv[1] if len(sys.argv) > 1 else None

LONG_BUILD_OUTPUT = "".join(f"   Compiling page {n}/60\n" for n in range(1, 61))

next_request_id = 1000
pending = {}
mcp_servers = []
session_cwd = os.getcwd()
settings = {"model": "sonnet", "effort": "medium", "mode": "default", "fast": False}


def config_options():
    return [
        {"id": "mode", "name": "Mode", "category": "mode", "type": "select",
         "currentValue": settings["mode"],
         "options": [{"value": "default", "name": "Default"},
                     {"value": "plan", "name": "Plan", "description": "Plan before editing"}]},
        {"id": "model", "name": "Model", "category": "model", "type": "select",
         "currentValue": settings["model"],
         "options": [{"value": "opus", "name": "Opus"}, {"value": "sonnet", "name": "Sonnet"},
                     {"value": "haiku", "name": "Haiku"}]},
        {"id": "effort", "name": "Effort", "category": "thought_level", "type": "select",
         "currentValue": settings["effort"],
         "options": [{"value": "low", "name": "Low"}, {"value": "medium", "name": "Medium"},
                     {"value": "high", "name": "High"}]},
        {"id": "fast", "name": "Fast", "type": "boolean", "currentValue": settings["fast"]},
    ]


def send(message):
    sys.stdout.write(json.dumps(message) + "\n")
    sys.stdout.flush()


def load_history():
    if HISTORY_PATH and os.path.exists(HISTORY_PATH):
        with open(HISTORY_PATH) as file:
            return json.load(file)
    return []


def record(payload):
    if HISTORY_PATH:
        history = load_history()
        history.append(payload)
        with open(HISTORY_PATH, "w") as file:
            json.dump(history, file)


def update(session_id, payload):
    record(payload)
    send({"jsonrpc": "2.0", "method": "session/update",
          "params": {"sessionId": session_id, "update": payload}})


def text_chunk(kind, text):
    return {"sessionUpdate": kind, "content": {"type": "text", "text": text}}


def finish_prompt(request_id, session_id, prompt_text, chosen=None):
    update(session_id, text_chunk("agent_message_chunk", "Echo: "))
    update(session_id, text_chunk("agent_message_chunk", prompt_text))
    update(session_id, {"sessionUpdate": "tool_call", "toolCallId": "call-1",
                        "title": "Read README.md", "kind": "read", "status": "completed"})
    if chosen is not None:
        update(session_id, text_chunk("agent_message_chunk", f" (chose {chosen})"))
    send({"jsonrpc": "2.0", "id": request_id, "result": {"stopReason": "end_turn"}})


def call_mcp_tool(name=None, arguments=None):
    """Speaks MCP's stdio transport (newline-delimited JSON-RPC) to the first stdio server."""
    server = next((s for s in mcp_servers if "command" in s), None)
    if server is None:
        return "no stdio MCP server"
    env = dict(os.environ)
    env.update({variable["name"]: variable["value"] for variable in server.get("env", [])})
    process = subprocess.Popen([server["command"], *server.get("args", [])], env=env,
                               stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True)

    def request(request_id, method, params):
        process.stdin.write(json.dumps({"jsonrpc": "2.0", "id": request_id,
                                        "method": method, "params": params}) + "\n")
        process.stdin.flush()
        return json.loads(process.stdout.readline())

    try:
        request(1, "initialize", {"protocolVersion": "2025-06-18", "capabilities": {},
                                  "clientInfo": {"name": "mock-agent", "version": "0"}})
        process.stdin.write(json.dumps({"jsonrpc": "2.0",
                                        "method": "notifications/initialized"}) + "\n")
        tools = request(2, "tools/list", {})["result"]["tools"]
        result = request(3, "tools/call", {"name": name or tools[0]["name"],
                                           "arguments": arguments or {}})["result"]
        return "".join(block.get("text", "") for block in result["content"])
    finally:
        process.stdin.close()
        process.wait()


for line in sys.stdin:
    message = json.loads(line)
    method = message.get("method")
    if method is None and "id" in message:
        request_id, session_id, prompt_text = pending.pop(message["id"])
        outcome = message.get("result", {}).get("outcome", {})
        finish_prompt(request_id, session_id, prompt_text, outcome.get("optionId", "cancelled"))
    elif method == "initialize":
        send({"jsonrpc": "2.0", "id": message["id"],
              "result": {"protocolVersion": 1,
                         "agentCapabilities": {"loadSession": HISTORY_PATH is not None,
                                               "auth": {"logout": {}}},
                         "authMethods": [{"id": "mock-login", "name": "Log In",
                                          "description": "Log in to the mock agent"}]}})
    elif method in ("authenticate", "logout"):
        send({"jsonrpc": "2.0", "id": message["id"], "result": {}})
    elif method == "session/new":
        mcp_servers = message["params"].get("mcpServers", [])
        session_cwd = message["params"].get("cwd", session_cwd)
        send({"jsonrpc": "2.0", "id": message["id"],
              "result": {"sessionId": "session-1", "configOptions": config_options()}})
        send({"jsonrpc": "2.0", "method": "session/update", "params": {"sessionId": "session-1", "update": {
            "sessionUpdate": "available_commands_update", "availableCommands": [
                {"name": "review", "description": "Review the current changes"},
                {"name": "init", "description": "Create an AGENTS.md for this project"},
                {"name": "compact", "description": "Summarize the conversation to free up context",
                 "input": {"hint": "optional focus"}}]}}})
    elif method == "session/load":
        mcp_servers = message["params"].get("mcpServers", [])
        session_cwd = message["params"].get("cwd", session_cwd)
        session_id = message["params"]["sessionId"]
        for payload in load_history():
            send({"jsonrpc": "2.0", "method": "session/update",
                  "params": {"sessionId": session_id, "update": payload}})
        send({"jsonrpc": "2.0", "id": message["id"], "result": {"configOptions": config_options()}})
    elif method == "session/set_config_option":
        params = message["params"]
        settings[params["configId"]] = params["value"]
        send({"jsonrpc": "2.0", "id": message["id"], "result": {"configOptions": config_options()}})
    elif method == "session/prompt":
        params = message["params"]
        prompt_text = "".join(block.get("text", "") for block in params["prompt"])
        record(text_chunk("user_message_chunk", prompt_text))
        if prompt_text == "permission":
            next_request_id += 1
            pending[next_request_id] = (message["id"], params["sessionId"], prompt_text)
            send({"jsonrpc": "2.0", "id": next_request_id, "method": "session/request_permission",
                  "params": {"sessionId": params["sessionId"],
                             "toolCall": {"toolCallId": "call-2", "title": "Edit .env"},
                             "options": [
                                 {"optionId": "allow", "name": "Allow once", "kind": "allow_once"},
                                 {"optionId": "deny", "name": "Deny", "kind": "reject_once"}]}})
        elif prompt_text == "mcp" or prompt_text.startswith("mcp "):
            parts = prompt_text.split(" ", 2)
            name = parts[1] if len(parts) > 1 else None
            arguments = json.loads(parts[2]) if len(parts) > 2 else None
            update(params["sessionId"], text_chunk("agent_message_chunk",
                                                   "MCP: " + call_mcp_tool(name, arguments)))
            send({"jsonrpc": "2.0", "id": message["id"], "result": {"stopReason": "end_turn"}})
        elif prompt_text.startswith("write ") or prompt_text.startswith("delete "):
            parts = prompt_text.split(" ", 2)
            path = os.path.join(session_cwd, parts[1])
            if parts[0] == "write":
                os.makedirs(os.path.dirname(path), exist_ok=True)
                with open(path, "w") as file:
                    file.write((parts[2] if len(parts) > 2 else "") + "\n")
            else:
                os.remove(path)
            update(params["sessionId"], text_chunk("agent_message_chunk", f"Done: {parts[0]} {parts[1]}"))
            send({"jsonrpc": "2.0", "id": message["id"], "result": {"stopReason": "end_turn"}})
        elif prompt_text == "slow":
            for word in ["One", " two", " three", " four", " five"]:
                update(params["sessionId"], text_chunk("agent_message_chunk", word))
                time.sleep(0.2)
            send({"jsonrpc": "2.0", "id": message["id"], "result": {"stopReason": "end_turn"}})
        elif prompt_text == "demo":
            session_id = params["sessionId"]
            update(session_id, text_chunk("agent_thought_chunk",
                                          "The user wants a checkout page. I'll look at the cart first."))
            update(session_id, {"sessionUpdate": "plan", "entries": [
                {"content": "Cart summary component", "priority": "high", "status": "completed"},
                {"content": "Orders API route", "priority": "high", "status": "completed"},
                {"content": "Wire the pay button", "priority": "medium", "status": "in_progress"},
                {"content": "End-to-end tests", "priority": "low", "status": "pending"}]})
            update(session_id, {"sessionUpdate": "tool_call", "toolCallId": "read-1",
                                "title": "Read src/app/cart/page.tsx", "kind": "read", "status": "completed",
                                "rawInput": {"path": "src/app/cart/page.tsx", "limit": 200},
                                "content": [{"type": "content", "content": {"type": "text", "text": "export default function Cart() {}"}}]})
            update(session_id, {"sessionUpdate": "tool_call", "toolCallId": "edit-1",
                                "title": "Edit src/app/checkout/page.tsx", "kind": "edit", "status": "completed",
                                "content": [{"type": "diff", "path": "src/app/checkout/page.tsx",
                                             "oldText": "export default function Checkout() {\n  return <div>TODO</div>\n}\n",
                                             "newText": "export default function Checkout() {\n  const cart = useCart()\n  return <CheckoutLayout cart={cart} />\n}\n"}]})
            update(session_id, {"sessionUpdate": "tool_call", "toolCallId": "run-1",
                                "title": "npm run build", "kind": "execute", "status": "completed",
                                "content": [{"type": "content", "content": {"type": "text", "text": "```\n> shop-landing@0.1.0 build\n> next build\n\n" + LONG_BUILD_OUTPUT + "✓ Compiled successfully\n```"}}]})
            update(session_id, {"sessionUpdate": "usage_update", "used": 91_200, "size": 200_000,
                                "cost": {"amount": 0.42, "currency": "USD"}})
            update(session_id, {"sessionUpdate": "session_info_update", "title": "Checkout page with pay button"})
            update(session_id, text_chunk("agent_message_chunk",
                "## Checkout page\n\nThe page builds. I split the work into:\n\n"
                "- a **cart summary** component\n- the `POST /api/orders` route\n- the pay button\n\n"
                "```tsx\nexport default function Checkout() {\n  const cart = useCart()\n  return <CheckoutLayout cart={cart} />\n}\n```\n\n"
                "Next I'll wire the pay button and add end-to-end tests."))
            send({"jsonrpc": "2.0", "id": message["id"], "result": {"stopReason": "end_turn"}})
        else:
            finish_prompt(message["id"], params["sessionId"], prompt_text)
