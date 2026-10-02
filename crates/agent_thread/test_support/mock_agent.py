"""A minimal ACP agent over stdio, used by agent_thread's tests.

It answers initialize and session/new, and replies to every prompt by streaming
"Echo: <prompt>" and a completed tool call, then ending the turn. A prompt of
"permission" first asks the client for permission and reports the chosen option.
"""
import json
import os
import sys

# Optional path where conversations are recorded so `session/load` can replay them.
HISTORY_PATH = sys.argv[1] if len(sys.argv) > 1 else None

next_request_id = 1000
pending = {}
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
                         "agentCapabilities": {"loadSession": HISTORY_PATH is not None},
                         "authMethods": []}})
    elif method == "session/new":
        send({"jsonrpc": "2.0", "id": message["id"],
              "result": {"sessionId": "session-1", "configOptions": config_options()}})
        send({"jsonrpc": "2.0", "method": "session/update", "params": {"sessionId": "session-1", "update": {
            "sessionUpdate": "available_commands_update", "availableCommands": [
                {"name": "review", "description": "Review the current changes"},
                {"name": "init", "description": "Create an AGENTS.md for this project"},
                {"name": "compact", "description": "Summarize the conversation to free up context",
                 "input": {"hint": "optional focus"}}]}}})
    elif method == "session/load":
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
                                "content": [{"type": "content", "content": {"type": "text", "text": "export default function Cart() {}"}}]})
            update(session_id, {"sessionUpdate": "tool_call", "toolCallId": "edit-1",
                                "title": "Edit src/app/checkout/page.tsx", "kind": "edit", "status": "completed",
                                "content": [{"type": "diff", "path": "src/app/checkout/page.tsx",
                                             "oldText": "export default function Checkout() {\n  return <div>TODO</div>\n}\n",
                                             "newText": "export default function Checkout() {\n  const cart = useCart()\n  return <CheckoutLayout cart={cart} />\n}\n"}]})
            update(session_id, {"sessionUpdate": "tool_call", "toolCallId": "run-1",
                                "title": "npm run build", "kind": "execute", "status": "completed",
                                "content": [{"type": "content", "content": {"type": "text", "text": "```\n> shop-landing@0.1.0 build\n> next build\n\n✓ Compiled successfully\n```"}}]})
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
