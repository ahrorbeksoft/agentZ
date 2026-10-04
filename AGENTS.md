# agentZ

A native macOS app for working with coding agents over the [Agent Client Protocol](https://agentclientprotocol.com)
(ACP): install agents from the ACP Registry, open threads with them across several projects and
machines, in their own worktrees or pastures, alongside real terminals. A background server on
each machine keeps the agents running. It is built on GPUI, Zed's UI framework.

This file is for any agent continuing the work. Read it before changing anything.

## Where the design comes from

- **Zed** (`references/zed`, a read-only clone, gitignored) is the source for GPUI, the `ui`
  components, themes, and the agent thread UI (`crates/agent_ui` there). Agent behavior should
  match Zed's: read Zed's code first and copy its behavior and wording.
- **t3code** (`references/t3code`, a read-only clone, gitignored) is the model for the sidebar,
  thread cards, details popover, settings layout and search. Archiving replaces t3code's
  "settle".
- **herdr** (`references/herdr`, a read-only clone, gitignored, Apache-2.0) is the model for
  background servers, attention states, terminal panes, SSH machines and the Workspaces view.
- **cow** (`references/cow`, a read-only clone, gitignored, MIT) is the model for copy-on-write
  workspaces, which cow calls pastures (instant APFS copies of a project for each thread).
- **Don't invent extras.** Build what was asked, the way Zed or t3code does it. If neither has
  it, keep it minimal and say what you chose.

agentZ is a *lean port*, not a fork. The Zed crates it needs (gpui, ui, theme, markdown, rope, …)
are copied into `crates/` at the same relative paths as in Zed, and Zed's `Cargo.lock` is reused.
Never path-depend on `references/`. To port something from Zed, read it there and rewrite it here
without pulling in its dependency closure (Zed's `agent_ui` alone needs ~155 crates).

## Layout

[`docs/architecture.md`](docs/architecture.md) maps the whole app: the architecture, agentZ's
own crates (everything else in `crates/` is copied from Zed), the data files, and every feature
with the modules it lives in and the reference it was taken from. Read the feature's section
before changing it, and keep the document current when you add or change a feature.

Agents run from a fixed command go in `agents/custom.json` in the data directory and show as
installed: `{"mock": {"name": "Mock", "command": {"path": "/usr/bin/python3", "args":
["…/mock_agent.py"], "env": {}}}}`.

## Build, run, test

- **Build:** `cargo build` builds the app and `agentz-server`. The app looks for the server next
  to its own executable (or at `AGENTZ_SERVER_BIN`) and starts it if nothing listens on
  `server.sock`. This machine has no full Xcode, so Metal shaders compile at runtime (the
  `runtime_shaders` feature, on by default in `crates/app`).
- **Run:**
  ```sh
  pkill -f "target/debug/agentz$"
  # Only after changing the server. This ends turns in progress.
  ./target/debug/agentz-server stop
  python3 -c 'import os, subprocess; env = {k: v for k, v in os.environ.items() if not k.startswith("CLAUDE") and k != "AI_AGENT"}; subprocess.Popen(["./target/debug/agentz"], env=env, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, start_new_session=True)'
  ```
  Relaunch the user's app this way after each change you ship. Your own session's variables
  (`CLAUDECODE`, `CLAUDE_CODE_*`, …) must not reach it: the server it starts passes them to
  every terminal, and a `claude` run there then thinks it's your child session. A plain `nohup … & disown` can be
  killed along with the shell that started it; `start_new_session` avoids that. The server keeps
  running when the app quits, and logs to `logs/server.log` in the data directory.
- **Check:**
  ```sh
  cargo fmt --all
  cargo clippy -p app -p agent_thread -p projects -p registry -p text_input -p agentz_protocol \
    -p agentz_server -p agentz_client --all-targets
  cargo test -p app -p agent_thread -p projects -p registry -p agentz_protocol -p agentz_server \
    -p agentz_client
  ```
  `cargo clippy --workspace` fails on a gpui example that needs an unported crate; that's
  expected.
- **GPUI tests** need `--features gpui_platform/runtime_shaders` (see Testing below).
- **Disk is tight** (a few GB free). `target/debug/incremental` once grew to 14 GB and froze the
  machine, and stale builds piled up to 42 GB in `target/debug`. Check `df -h ~` before long
  build sessions. When space runs low, `cargo clean --profile dev` is safe and a full
  `cargo build` takes about 2 minutes (4 GB).

## Rules from the user

- **Never send prompts to the user's real agents** (Claude, Codex, Devin, OpenCode, …). That
  spends their usage. Test with `crates/agent_thread/test_support/mock_agent.py`. Starting an agent
  or sending `initialize` is fine. Logging the user out of a real agent is not.
- **Never steal focus or the mouse.** Don't bring windows to the front while the user is working.
  See Testing for how to take screenshots.
- **Commit after each finished change** with a clear, imperative message that explains why, and
  push it (`git push origin main`) right after.
- Answer briefly and plainly. Say what you couldn't verify.

## Code conventions

From Zed's guidelines, which this code follows:

- Prefer correctness and clarity. No `unwrap()`. Propagate errors with `?`, log with `.log_err()`,
  and never discard them silently with `let _ =`.
- Comments explain *why*, not what. Full words in names. No `mod.rs` files.
- Use variable shadowing to scope clones into closures and async blocks.
- GPUI: use the inner `cx` inside `update` closures. Never update an entity while it's being
  updated. Store, detach or await every `Task`. Call `cx.notify()` when render output changes.
- Match the surrounding code's style. Add to existing files unless it's a new component.

## Testing

- **Unit and integration:** `agent_thread` tests drive the real mock agent process (login, logout,
  reload, defaults, history replay). Extend the mock when you need a protocol feature; it speaks
  JSON-RPC over stdio in a few lines of Python. Its prompts `permission`, `mcp` (or
  `mcp <tool> <json>`), `slow`, `demo`, `form`, `write <path> <text>` and `delete <path>` script
  different turns (see its docstring). With `MOCK_LOGIN_FILE` in its env it needs a login, and
  offers every kind: plain, terminal, browser (a page to open), API key and gateway; with
  `MOCK_BROWSER_OPEN` too, a browser login that runs `xdg-open` and waits on a `127.0.0.1`
  callback, as Devin's and Codex's do (`tests/browser.rs` drives it as on an SSH machine). It
  names context embedded in a prompt in its echo ("Echo: next [with agentz://handoff]"). With
  `MOCK_SESSIONS_FILE` (a JSON array of ACP session infos, each with an optional `history` to
  replay) it answers `session/list`, two sessions a page, and loads them, for thread import.
- **Server:** `agentz_server` tests run the server in-process over in-memory streams with the
  mock agent as a custom agent; `tests/binary.rs` runs the real binary against a temporary data
  directory. `agentz_client` tests reattach to a turn in progress.
- **Agent control:** `agentz_server` tests call the tools as a thread (`Request::CallTool`) for
  their behavior and policy; `tests/agent_control.rs` has the mock agent call them through the
  real `mcp-bridge`, and runs `call` as a thread's shell would. Delegated tasks run the mock
  agent too, so a task's prompt is a mock script (`hello`, `slow`, `permission`).
- **The app against a scratch server:** set `AGENTZ_DATA_DIR` to a short temporary directory
  (`mktemp -d /tmp/az.XXXX`: `server.sock`'s path must fit in `SUN_LEN`, which the scratchpad's
  doesn't) and put
  the mock agent in its `agents/custom.json`. The app starts a server for that directory. Stop it
  afterwards with `AGENTZ_DATA_DIR=<dir> ./target/debug/agentz-server stop`.
- **UI behavior (hover, layout, focus, timing):** headless GPUI tests in `crates/app`
  (`gpui` with `test-support` is a dev-dependency). `spaces_view::tests` is the example:
  - `crate::init_for_test` sets up themes and key bindings without the user's settings;
    `ServerClient::new_for_test` is a client that never connects, holding given state (add what
    a test needs), and `machines::init_for_test` makes it the machines' only client. Then
    `cx.add_window_view`.
  - Probe with `.debug_selector(..)` (a no-op outside tests) and `cx.debug_bounds("name")`. Drive
    it with `simulate_click`, `simulate_keystrokes` (which also checks key bindings),
    `simulate_mouse_move` and `executor().advance_clock(..)`.
- **Screenshots without bothering the user:**
  - Temporarily remove `cx.activate(true)`, and set `focus: false` and
    `kind: gpui::WindowKind::PopUp` in `main.rs`, with bounds about 1160×740 in a corner of the
    primary display.
  - Drive the app with temporary code (open settings, select a section), using demo data via
    `AGENTZ_DATA_DIR=<scratch>/data`.
  - Find the window id through `CGWindowListCopyWindowInfo` by the scratch app's pid (not the
    user's app, which may be running too). Don't filter on layer 0, because PopUp windows sit
    higher.
  - Capture with `screencapture -x -o -l <id>`, then restore every patched file.
  - A plain background window never draws, so its screenshot is blank or stale.

## Pitfalls already hit

- **Group hover:** an absolutely positioned child with `visible_on_hover` works, but test hover
  headlessly before blaming it.
- **Scrollbars** (`vertical_scrollbar_for`) must be attached to a *non-scrolling* wrapper around
  the element that has `.overflow_y_scroll().track_scroll(..)`, as in Zed. Otherwise they scroll
  away with the content.
- **`flex_1` inside an auto-height popover** collapses to zero height. Give the list a `max_h`
  instead.
- **`Callout` ignores `.icon()`** and always draws its severity's icon.
- **GPUI tooltips always follow the cursor.** The sidebar's details popover is a custom anchored,
  deferred element, so it can sit beside the row.
- **Actions need a focused element under the shell.** When the focused element disappears (for
  example the open thread is deleted), `window.dispatch_action` starts at the window's root, above
  `Shell`'s `on_action` handlers, and New Thread and similar actions do nothing. Move focus back to
  the shell whenever a focused view is removed.
- **The built-in fallback theme is also called "One Dark".** Compare themes with `Arc::ptr_eq`,
  not by name.
- **Some npm agents ship a native binary as their `bin`** (Factory Droid). `registry::runs_with_node`
  decides whether to run it through `node`.
- **ACP reports an agent's settings only inside a session**, and has no login-status request.
  Opening an agent's Settings panel opens an empty session (no prompt). If it works, the agent is
  logged in, and its settings are learned; "authentication required" means it's logged out. The
  method shown ("Logged in with ChatGPT") is the one last used from agentZ.
- **Stay within ACP for agent status.** Don't read agents' own credential files: every agent stores
  its login differently.
- **Threads from older builds may lack `session_id` or `model`.** They fill in the next time the
  thread is opened.
- **`agentz-server proxy` must not wait for stdin when it exits.** Tokio reads stdin on a
  blocking thread and dropping the runtime waits for it, so the SSH session stayed open after
  the server quit. It ends with `shutdown_background`.
- **A running server outlives its binary.** Installing renames the new binary into place, so an
  older server keeps running; it reports the hash beside its executable at start
  (`ServerWelcome::build`), and the app compares it with the installed one.
