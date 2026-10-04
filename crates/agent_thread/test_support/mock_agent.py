"""A minimal ACP agent over stdio, used by agent_thread's tests.

It answers initialize and session/new, and replies to every prompt by streaming
"Echo: <prompt>" and a completed tool call, then ending the turn. A prompt of
"permission" first asks the client for permission and reports the chosen option.
A prompt of "mcp" starts the first stdio MCP server given in session/new (or
session/load), calls its first tool, and replies "MCP: <tool result>"; "mcp <tool>
<json arguments>" calls that tool instead. A prompt of "slow" streams
"One two three four five" a word at a time, 200 ms apart. "write <path> <text>"
writes the text and a newline to the file, relative to the session's folder, and
"delete <path>" removes it. "terminal <command>" runs the command in a client
terminal (ACP's terminal/create), shows it in a tool call, waits for it to exit,
and replies "Terminal <exit code>: <output>", then releases it.

With MOCK_LOGIN_FILE set, sessions need that file to exist (otherwise they fail with
"authentication required" and a pairing code, as Factory Droid does); "mock-login" creates
it, and so does the terminal login "mock-terminal-login", offered to clients that support
terminal logins: it runs this script with `--login`, which waits for Enter, then creates the
file and exits. The other logins, as real agents offer them:
- "mock-browser-login" (to clients that take URL elicitations) asks the client to open a
  URL, as Codex's device-code login does, and logs in once the client accepts.
- "mock-api-key" takes `_meta["api-key"]["apiKey"]`, as Codex's does.
- "mock-gateway" (to clients that set `auth._meta.gateway`) takes `_meta["gateway"]`
  with a `baseUrl`, as Claude Agent's does.
Every login and logout is reported with `_auth/status_update`, as Claude Agent and Codex do.

A prompt of "form" asks the client to fill in a form (a session elicitation) and replies
"Form: <action> <content as JSON>".

It supports `session/close`, and with MOCK_CLOSED_FILE set, notes each closed session there.
"""
import json
import os
import subprocess
import sys
import time

LOGIN_FILE = os.environ.get("MOCK_LOGIN_FILE")
# Where `session/close` notes the sessions it closed, a line each.
CLOSED_FILE = os.environ.get("MOCK_CLOSED_FILE")

if sys.argv[-1] == "--login":
    print("Press Enter to log in to the mock agent.", flush=True)
    sys.stdin.readline()
    open(LOGIN_FILE, "w").close()
    print("Logged in.", flush=True)
    sys.exit(0)

# Optional path where conversations are recorded so `session/load` can replay them.
HISTORY_PATH = sys.argv[1] if len(sys.argv) > 1 else None

LONG_BUILD_OUTPUT = "".join(f"   Compiling page {n}/60\n" for n in range(1, 61))

next_request_id = 1000
pending = {}
mcp_servers = []
session_cwd = os.getcwd()
settings = {"model": "sonnet", "effort": "medium", "mode": "default", "fast": False}
# Set by logout: sessions then need a login, until the process restarts.
logged_out = False


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


def client_request(method, params):
    """Sends a request to the client and reads stdin until its answer arrives."""
    global next_request_id
    next_request_id += 1
    request_id = next_request_id
    send({"jsonrpc": "2.0", "id": request_id, "method": method, "params": params})
    while True:
        reply = json.loads(sys.stdin.readline())
        if reply.get("id") == request_id and "method" not in reply:
            if "error" in reply:
                raise RuntimeError(reply["error"].get("message", "error"))
            return reply["result"]


def logged_in():
    if logged_out:
        return False
    return not LOGIN_FILE or os.path.exists(LOGIN_FILE)


def log_in():
    global logged_out
    logged_out = False
    if LOGIN_FILE:
        open(LOGIN_FILE, "w").close()


def send_auth_status():
    if logged_in():
        status = {"kind": "account", "label": "Mock Pro",
                  "account": {"email": "mock@example.com", "plan": "Pro"}}
    else:
        status = {"kind": "none"}
    send({"jsonrpc": "2.0", "method": "_auth/status_update", "params": {"authStatus": status}})


def authenticate(request_id, params):
    method_id = params.get("methodId")
    meta = params.get("_meta") or {}
    error = None
    if method_id == "mock-browser-login":
        answer = client_request("elicitation/create", {
            "mode": "url", "requestId": request_id, "elicitationId": "login-1",
            "url": "https://example.com/device?code=MOCK-1234",
            "message": "Enter code MOCK-1234 to log in to the mock agent."})
        if answer.get("action") == "accept":
            send({"jsonrpc": "2.0", "method": "elicitation/complete",
                  "params": {"elicitationId": "login-1"}})
        else:
            error = f"Login {answer.get('action')}"
    elif method_id == "mock-api-key":
        if not (meta.get("api-key") or {}).get("apiKey"):
            error = "No API key given"
    elif method_id == "mock-gateway":
        if not (meta.get("gateway") or {}).get("baseUrl"):
            error = "No gateway given"
    if error:
        send({"jsonrpc": "2.0", "id": request_id, "error": {"code": -32603, "message": error}})
        return
    log_in()
    send({"jsonrpc": "2.0", "id": request_id, "result": {}})
    send_auth_status()


def ask_form(session_id):
    answer = client_request("elicitation/create", {
        "mode": "form", "sessionId": session_id,
        "message": "How should the mock agent greet you?",
        "requestedSchema": {"type": "object", "required": ["name"], "properties": {
            "name": {"type": "string", "title": "Name", "minLength": 1},
            "tone": {"type": "string", "title": "Tone", "oneOf": [
                {"const": "warm", "title": "Warm"}, {"const": "dry", "title": "Dry"}]},
            "times": {"type": "integer", "title": "Times", "minimum": 1, "maximum": 3},
            "loud": {"type": "boolean", "title": "Loud", "default": False}}}})
    return f"Form: {answer.get('action')} {json.dumps(answer.get('content'), sort_keys=True)}"


def run_in_terminal(session_id, command):
    created = client_request("terminal/create", {"sessionId": session_id, "command": command,
                                                 "outputByteLimit": 10000})
    terminal_id = created["terminalId"]
    update(session_id, {"sessionUpdate": "tool_call", "toolCallId": "terminal-1",
                        "title": command, "kind": "execute", "status": "in_progress",
                        "content": [{"type": "terminal", "terminalId": terminal_id}]})
    exited = client_request("terminal/wait_for_exit",
                            {"sessionId": session_id, "terminalId": terminal_id})
    output = client_request("terminal/output", {"sessionId": session_id, "terminalId": terminal_id})
    client_request("terminal/release", {"sessionId": session_id, "terminalId": terminal_id})
    update(session_id, {"sessionUpdate": "tool_call_update", "toolCallId": "terminal-1",
                        "status": "completed"})
    return f"Terminal {exited.get('exitCode')}: {output['output'].strip()}"


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
        auth_methods = [{"id": "mock-login", "name": "Log In",
                         "description": "Log in to the mock agent"}]
        capabilities = message["params"].get("clientCapabilities", {})
        if capabilities.get("auth", {}).get("terminal"):
            auth_methods.append({"id": "mock-terminal-login", "name": "Log in in a terminal",
                                 "type": "terminal", "args": ["--login"]})
        if "url" in (capabilities.get("elicitation") or {}):
            auth_methods.append({"id": "mock-browser-login", "name": "Log in with a browser"})
        auth_methods.append({"id": "mock-api-key", "name": "Use an API key",
                             "_meta": {"api-key": {"provider": "mock"}}})
        if ((capabilities.get("auth") or {}).get("_meta") or {}).get("gateway"):
            auth_methods.append({"id": "mock-gateway", "name": "Use a gateway",
                                 "_meta": {"gateway": {"protocol": "anthropic"}}})
        send({"jsonrpc": "2.0", "id": message["id"],
              "result": {"protocolVersion": 1,
                         "agentCapabilities": {"loadSession": HISTORY_PATH is not None,
                                               "sessionCapabilities": {"close": {}},
                                               "auth": {"logout": {}}},
                         "authMethods": auth_methods}})
        send_auth_status()
    elif method == "authenticate":
        authenticate(message["id"], message["params"])
    elif method == "logout":
        logged_out = True
        if LOGIN_FILE and os.path.exists(LOGIN_FILE):
            os.remove(LOGIN_FILE)
        send({"jsonrpc": "2.0", "id": message["id"], "result": {}})
        send_auth_status()
    elif method in ("session/new", "session/load") and not logged_in():
        send({"jsonrpc": "2.0", "id": message["id"],
              "error": {"code": -32000, "message": "\n\nYour code: MOCK-1234\n\nClick Log In."}})
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
    elif method == "session/close":
        if CLOSED_FILE:
            with open(CLOSED_FILE, "a") as file:
                file.write(message["params"]["sessionId"] + "\n")
        send({"jsonrpc": "2.0", "id": message["id"], "result": {}})
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
        elif prompt_text.startswith("terminal "):
            try:
                reply = run_in_terminal(params["sessionId"], prompt_text[len("terminal "):])
            except RuntimeError as error:
                reply = f"Terminal failed: {error}"
            update(params["sessionId"], text_chunk("agent_message_chunk", reply))
            send({"jsonrpc": "2.0", "id": message["id"], "result": {"stopReason": "end_turn"}})
        elif prompt_text == "form":
            reply = ask_form(params["sessionId"])
            update(params["sessionId"], text_chunk("agent_message_chunk", reply))
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
