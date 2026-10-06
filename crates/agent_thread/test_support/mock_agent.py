"""A minimal ACP agent over stdio, used by agent_thread's tests.

It answers initialize and session/new, and replies to every prompt by streaming
"Echo: <prompt>" and a completed tool call, then ending the turn. A prompt of
"permission" first asks the client for permission and reports the chosen option.
A prompt of "mcp" starts the first stdio MCP server given in session/new (or
session/load), calls its first tool, and replies "MCP: <tool result>"; "mcp <tool>
<json arguments>" calls that tool instead. A prompt of "background" ends its turn at
once, then goes on streaming a "." every half second for six seconds, as Claude Agent's
background tasks report after the turn. A prompt of "slow" streams
"One two three four five" a word at a time, 200 ms apart, and "think" streams a thought a
word at a time, 500 ms apart, then replies. "write <path> <text>"
writes the text and a newline to the file, relative to the session's folder, and
"delete <path>" removes it. "terminal <command>" runs the command in a client
terminal (ACP's terminal/create), shows it in a tool call, waits for it to exit,
and replies "Terminal <exit code>: <output>", then releases it.

With MOCK_LOGIN_FILE set, sessions need that file to exist (otherwise they fail with
"authentication required" and a pairing code, as Factory Droid does); "mock-login" creates
it, and so does the terminal login "mock-terminal-login", offered to clients that support
terminal logins: it runs this script with `--login`, which waits for Enter, then creates the
file and exits (with MOCK_BROWSER_OPEN set, it first opens a page with `xdg-open`). The other
logins, as real agents offer them:
- "mock-browser-login" (to clients that take URL elicitations) asks the client to open a
  URL, as Codex's device-code login does, and logs in once the client accepts.
- "mock-browser-open-login" (with MOCK_BROWSER_OPEN set) opens a page with `xdg-open` that
  sends the browser back to a callback on 127.0.0.1, as Devin's and Codex's browser logins do,
  and logs in once a request with a `code` arrives there. It fails if `xdg-open` does.
- "mock-api-key" takes `_meta["api-key"]["apiKey"]`, as Codex's does.
- "mock-gateway" (to clients that set `auth._meta.gateway`) takes `_meta["gateway"]`
  with a `baseUrl`, as Claude Agent's does.
Every login and logout is reported with `_auth/status_update`, as Claude Agent and Codex do.

MOCK_HOME is its home, as FACTORY_HOME_OVERRIDE is Factory Droid's: with it set, the login
MOCK_LOGIN_FILE asks for is the file `login` there instead. MOCK_API_KEY logs it in whatever
the file says, as FACTORY_API_KEY does.

Context embedded in a prompt (an ACP resource, such as the handoff agentZ sends with a continued
thread's first message) is named at the end of the echo: "Echo: next [with agentz://handoff]".
So are resource links (their URIs) and images (their MIME types). With MOCK_IMAGES set, it
takes images. A prompt of "image" shows an image in a tool call's output and in its reply.

A prompt of "form" asks the client to fill in a form (a session elicitation) and replies
"Form: <action> <content as JSON>".

It supports `session/close`, and with MOCK_CLOSED_FILE set, notes each closed session there.

With MOCK_REJECT_MCP set, `session/new` and `session/load` fail when given any MCP server, as
Factory Droid 0.233.0's do.

With MOCK_STEERING set, it takes messages into a running turn (`_session/steering`), as Claude
Agent and Codex do: one sent while a "permission" turn waits for its answer joins that turn,
whose reply ends " (steered: <text>)". With no turn running it answers `promptRequired`, as
Claude Agent does when asked to, and it refuses a message of "refuse".

With MOCK_CHILD_PID_FILE set, it starts a long `sleep` as Factory Droid starts a worker for each
session, and writes its process id to that file.

With MOCK_SESSIONS_FILE set, it lists the sessions in that file (`session/list`, two to a
page): a JSON array of ACP session infos, each with an optional "history" of session updates
that `session/load` replays for it. Listing needs a login, as sessions do.

With MOCK_SCRIPTS set to a JSON object of prompts and the prompts above they stand for, such
as {"Add a checkout page": "demo"}, those prompts run their scripts, so a demo thread shows a
real-looking prompt.
"""
import json
import os
import subprocess
import sys
import threading
import time

LOGIN_FILE = os.environ.get("MOCK_LOGIN_FILE")
if LOGIN_FILE and os.environ.get("MOCK_HOME"):
    LOGIN_FILE = os.path.join(os.environ["MOCK_HOME"], "login")
# Where `session/close` notes the sessions it closed, a line each.
CLOSED_FILE = os.environ.get("MOCK_CLOSED_FILE")
# The sessions `session/list` reports.
SESSIONS_FILE = os.environ.get("MOCK_SESSIONS_FILE")
SESSIONS_PER_PAGE = 2
# Prompts that run another prompt's script.
SCRIPTS = json.loads(os.environ.get("MOCK_SCRIPTS") or "{}")

if sys.argv[-1] == "--login":
    if os.environ.get("MOCK_BROWSER_OPEN"):
        # As `claude /login` does, before it offers a link to open by hand.
        subprocess.run(["xdg-open", "https://example.com/terminal-login"])
    # Says whether the key reached it, which an account's login leaves out.
    key = " with MOCK_API_KEY set" if os.environ.get("MOCK_API_KEY") else ""
    print(f"Press Enter to log in to the mock agent{key}.", flush=True)
    sys.stdin.readline()
    open(LOGIN_FILE, "w").close()
    print("Logged in.", flush=True)
    sys.exit(0)

if os.environ.get("MOCK_CHILD_PID_FILE"):
    worker = subprocess.Popen(["sleep", "600"])
    with open(os.environ["MOCK_CHILD_PID_FILE"], "w") as file:
        file.write(str(worker.pid))

# Optional path where conversations are recorded so `session/load` can replay them.
HISTORY_PATH = sys.argv[1] if len(sys.argv) > 1 else None

LONG_BUILD_OUTPUT = "".join(f"   Compiling page {n}/60\n" for n in range(1, 61))
# A 1×1 PNG, for the "image" prompt.
TINY_PNG = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg=="

next_request_id = 1000
pending = {}
# Messages steered into the turn in progress.
steered = []
mcp_servers = []
session_cwd = os.getcwd()
settings = {"model": "sonnet", "effort": "medium", "mode": "default", "fast": False}
# Set by logout: sessions then need a login, until the process restarts.
logged_out = False
# The resources embedded in the prompt being answered.
prompt_resources = []


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


# Background work writes too.
send_lock = threading.Lock()


def send(message):
    with send_lock:
        sys.stdout.write(json.dumps(message) + "\n")
        sys.stdout.flush()


def work_in_background(session_id):
    for _ in range(12):
        time.sleep(0.5)
        send({"jsonrpc": "2.0", "method": "session/update", "params": {
            "sessionId": session_id, "update": text_chunk("agent_message_chunk", ".")}})


def listed_sessions():
    if not SESSIONS_FILE:
        return []
    with open(SESSIONS_FILE) as file:
        return json.load(file)


def load_history(session_id=None):
    for session in listed_sessions():
        if session["sessionId"] == session_id:
            return session.get("history", [])
    if HISTORY_PATH and os.path.exists(HISTORY_PATH):
        with open(HISTORY_PATH) as file:
            return json.load(file)
    return []


def list_sessions(params):
    sessions = [{key: value for key, value in session.items() if key != "history"}
                for session in listed_sessions()
                if params.get("cwd") in (None, session["cwd"])]
    start = int(params.get("cursor") or 0)
    result = {"sessions": sessions[start:start + SESSIONS_PER_PAGE]}
    if start + SESSIONS_PER_PAGE < len(sessions):
        result["nextCursor"] = str(start + SESSIONS_PER_PAGE)
    return result


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
    if prompt_resources:
        update(session_id, text_chunk("agent_message_chunk",
                                      f" [with {', '.join(prompt_resources)}]"))
    update(session_id, {"sessionUpdate": "tool_call", "toolCallId": "call-1",
                        "title": "Read README.md", "kind": "read", "status": "completed"})
    if chosen is not None:
        update(session_id, text_chunk("agent_message_chunk", f" (chose {chosen})"))
    for text in steered:
        update(session_id, text_chunk("agent_message_chunk", f" (steered: {text})"))
    steered.clear()
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
    if os.environ.get("MOCK_API_KEY"):
        return True
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


def browser_open_login():
    """Opens the login page, as Devin and Codex do, and waits for the browser to come back to
    the callback it names. Returns why it failed, if it did."""
    import http.server
    import urllib.parse
    codes = []

    class Callback(http.server.BaseHTTPRequestHandler):
        def do_GET(self):
            query = urllib.parse.parse_qs(urllib.parse.urlparse(self.path).query)
            codes.append(query.get("code", [""])[0])
            self.send_response(200)
            self.end_headers()
            self.wfile.write(b"Logged in to the mock agent.")

        def log_message(self, *args):
            pass

    server = http.server.HTTPServer(("127.0.0.1", 0), Callback)
    callback = urllib.parse.quote(f"http://127.0.0.1:{server.server_port}/callback", safe="")
    url = f"https://example.com/login?redirect_uri={callback}&state=mock"
    try:
        # Its output mustn't reach stdout, which is the client's.
        opened = subprocess.run(["xdg-open", url], stdout=subprocess.DEVNULL).returncode == 0
    except OSError:
        opened = False
    if not opened:
        server.server_close()
        return "Could not open browser for authentication"
    server.timeout = 120
    server.handle_request()
    server.server_close()
    return None if codes and codes[0] else "The login didn't finish"


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
    elif method_id == "mock-browser-open-login":
        error = browser_open_login()
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
        if os.environ.get("MOCK_BROWSER_OPEN"):
            auth_methods.append({"id": "mock-browser-open-login", "name": "Log in with your browser"})
        auth_methods.append({"id": "mock-api-key", "name": "Use an API key",
                             "_meta": {"api-key": {"provider": "mock"}}})
        if ((capabilities.get("auth") or {}).get("_meta") or {}).get("gateway"):
            auth_methods.append({"id": "mock-gateway", "name": "Use a gateway",
                                 "_meta": {"gateway": {"protocol": "anthropic"}}})
        session_capabilities = {"close": {}}
        if SESSIONS_FILE:
            session_capabilities["list"] = {}
        send({"jsonrpc": "2.0", "id": message["id"],
              "result": {"protocolVersion": 1,
                         "_meta": ({"steering": {"supported": True}}
                                   if os.environ.get("MOCK_STEERING") else {}),
                         "agentInfo": {"name": "mock-agent", "title": "Mock Agent",
                                       "version": "1.2.3"},
                         "agentCapabilities": {
                             "loadSession": HISTORY_PATH is not None or SESSIONS_FILE is not None,
                             "sessionCapabilities": session_capabilities,
                             "promptCapabilities": {"embeddedContext": True,
                                                    "image": bool(os.environ.get("MOCK_IMAGES"))},
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
    elif method in ("session/new", "session/load", "session/list") and not logged_in():
        send({"jsonrpc": "2.0", "id": message["id"],
              "error": {"code": -32000, "message": "\n\nYour code: MOCK-1234\n\nClick Log In."}})
    elif (method in ("session/new", "session/load") and os.environ.get("MOCK_REJECT_MCP")
          and message["params"].get("mcpServers")):
        send({"jsonrpc": "2.0", "id": message["id"],
              "error": {"code": -32603, "message": "Internal error", "data": {
                  "details": "Droid process exited unexpectedly (exit code 1)"}}})
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
        for payload in load_history(session_id):
            send({"jsonrpc": "2.0", "method": "session/update",
                  "params": {"sessionId": session_id, "update": payload}})
        send({"jsonrpc": "2.0", "id": message["id"], "result": {"configOptions": config_options()}})
    elif method == "session/list":
        send({"jsonrpc": "2.0", "id": message["id"], "result": list_sessions(message["params"])})
    elif method == "session/close":
        if CLOSED_FILE:
            with open(CLOSED_FILE, "a") as file:
                file.write(message["params"]["sessionId"] + "\n")
        send({"jsonrpc": "2.0", "id": message["id"], "result": {}})
    elif method == "_session/steering":
        text = "".join(block.get("text", "") for block in message["params"]["prompt"]
                       if block.get("type", "text") == "text")
        if text == "refuse":
            send({"jsonrpc": "2.0", "id": message["id"],
                  "error": {"code": -32603, "message": "Internal error"}})
        elif pending:
            steered.append(text)
            record(text_chunk("user_message_chunk", text))
            send({"jsonrpc": "2.0", "id": message["id"], "result": {"outcome": "injected"}})
        else:
            send({"jsonrpc": "2.0", "id": message["id"],
                  "result": {"outcome": "promptRequired", "reason": "noRunningTurn"}})
    elif method == "session/set_config_option":
        params = message["params"]
        settings[params["configId"]] = params["value"]
        send({"jsonrpc": "2.0", "id": message["id"], "result": {"configOptions": config_options()}})
    elif method == "session/prompt":
        params = message["params"]
        prompt_text = "".join(block.get("text", "") for block in params["prompt"]
                              if block.get("type", "text") == "text")
        # Context in the prompt (a handoff from another thread, mentions), named in the reply.
        prompt_resources = []
        for block in params["prompt"]:
            if block.get("type") == "resource":
                prompt_resources.append(block["resource"]["uri"])
            elif block.get("type") == "resource_link":
                prompt_resources.append(block["uri"])
            elif block.get("type") == "image":
                prompt_resources.append(block["mimeType"])
        record(text_chunk("user_message_chunk", prompt_text))
        prompt_text = SCRIPTS.get(prompt_text, prompt_text)
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
        elif prompt_text == "background":
            update(params["sessionId"], text_chunk("agent_message_chunk", "Working in the background"))
            send({"jsonrpc": "2.0", "id": message["id"], "result": {"stopReason": "end_turn"}})
            threading.Thread(target=work_in_background, args=(params["sessionId"],),
                             daemon=True).start()
        elif prompt_text == "slow":
            for word in ["One", " two", " three", " four", " five"]:
                update(params["sessionId"], text_chunk("agent_message_chunk", word))
                time.sleep(0.2)
            send({"jsonrpc": "2.0", "id": message["id"], "result": {"stopReason": "end_turn"}})
        elif prompt_text == "think":
            for word in ["The", " receipt", " rounds", " once,", " so", " sum", " the", " items",
                         " first", " and", " round", " at", " the", " end."]:
                update(params["sessionId"], text_chunk("agent_thought_chunk", word))
                time.sleep(0.5)
            update(params["sessionId"], text_chunk("agent_message_chunk", "It sums first, then rounds once."))
            send({"jsonrpc": "2.0", "id": message["id"], "result": {"stopReason": "end_turn"}})
        elif prompt_text == "image":
            session_id = params["sessionId"]
            image = {"type": "image", "mimeType": "image/png", "data": TINY_PNG}
            update(session_id, {"sessionUpdate": "tool_call", "toolCallId": "screenshot-1",
                                "title": "Take a screenshot", "kind": "other", "status": "completed",
                                "content": [{"type": "content", "content": image}]})
            update(session_id, {"sessionUpdate": "agent_message_chunk", "content": image})
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
