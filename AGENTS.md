# agentZ

A native macOS app for working with coding agents over the [Agent Client Protocol](https://agentclientprotocol.com)
(ACP): install agents from the ACP Registry, open threads with them across several projects, and
manage it all from a sidebar. It is built on GPUI, Zed's UI framework.

This file is for any agent continuing the work. Read it before changing anything.

## Where the design comes from

- **Zed** (`references/zed`, a read-only clone, gitignored) is the source for GPUI, the `ui`
  components, themes, and the agent thread UI (`crates/agent_ui` there). Agent behavior should
  match Zed's: read Zed's code first and copy its behavior and wording.
- **t3code** (`references/t3code`, a read-only clone, gitignored) is the model for the sidebar,
  thread cards, details popover, settings layout and search. Archiving replaces t3code's
  "settle".
- **herdr** (`references/herdr`, a read-only clone, gitignored, Apache-2.0) is the model for
  background servers, attention states, terminal panes and SSH machines. See the plan below.
- **cow** (`references/cow`, a read-only clone, gitignored, MIT) is the model for copy-on-write
  workspaces, which cow calls pastures (instant APFS copies of a project for each thread).
- **Don't invent extras.** Build what was asked, the way Zed or t3code does it. If neither has
  it, keep it minimal and say what you chose.

agentZ is a *lean port*, not a fork. The Zed crates it needs (gpui, ui, theme, markdown, rope, …)
are copied into `crates/` at the same relative paths as in Zed, and Zed's `Cargo.lock` is reused.
Never path-depend on `references/`. To port something from Zed, read it there and rewrite it here
without pulling in its dependency closure (Zed's `agent_ui` alone needs ~155 crates).

## Layout

agentZ's own crates (everything else in `crates/` is copied from Zed):

| Crate | What it is |
|---|---|
| `crates/app` | The application (`agentz` binary), a GPUI client of the server. See the modules below. |
| `crates/agentz_server` | The background server (`agentz-server` binary, GPUI-free, tokio). One task (`server.rs`) owns the projects, the registry, agent settings and the running threads, so agents keep working when the app quits. `server/tools.rs` has the agent-control tools (t3code's orchestrator MCP, `agentz_` for `t3_`), including delegation to subthreads and their finalization. `checkpoints.rs` snapshots the working tree as hidden git refs around every turn (t3code's checkpoints) and diffs them. `workspaces.rs` makes and removes worktrees and pastures (cow's copy-on-write clones), syncs pastures and brings their branches back; `server/workspace_requests.rs` handles the requests that need it, and `server/tools/workspaces.rs` has the `agentz_workspace_*` tools and `workspaceStrategy`. `git.rs` runs git with a clean environment. `main.rs` has `run`, `start`, `proxy`, `stop`, and for agents `mcp-bridge` (`mcp_bridge.rs`, the stdio MCP server every session gets), `tools` and `call <tool> [json]`. |
| `crates/agentz_protocol` | The wire format (length-prefixed JSON) and the types the server and clients share: requests, responses, events, thread views and updates, registry and agent settings. `diff.rs` has thread diffs and the patch parser. `workspace.rs` has where a new thread works and what the server reports about a project's repository. |
| `crates/agentz_client` | A connection to the server: requests answered through futures, events in order, and starting a local server. |
| `crates/agent_thread` | One ACP connection and session, run by the server: process, protocol, entries, permissions, config options, login/logout, reload. `test_support/mock_agent.py` is a scripted ACP agent for tests. |
| `crates/projects` | `ProjectStore`: projects (custom name and icon), threads (title, agent, session id, model, archived, created by an agent, and for subthreads the delegated task and its outcome), scope, thread order. Saved to `state.json`. |
| `crates/registry` | `AgentRegistryStore`: fetches the ACP Registry, installs, updates and uninstalls agents (binary archives, or npm via the system `npm`), and builds the command to start one. |
| `crates/paths` | Data locations. `AGENTZ_DATA_DIR` overrides the data directory. |
| `crates/text_input` | The single-line text field (cursor blink, selection, IME). |
| `crates/theme_json` | Loads the bundled JSON themes (One, Ayu, Gruvbox in `assets/themes`). |

`crates/app/src`:

| Module | What it is |
|---|---|
| `main.rs` | Startup, actions, key bindings, menus, theme fonts. |
| `shell.rs` | The window: title bar with the project switcher and the disconnected icon, sidebar, main area (thread or settings), diff panel, New Thread modal. |
| `diff_panel.rs` | The open thread's changes (Cmd-D): latest turn or all, files and hunks, Viewed, and Revert for a thread alone in its worktree or pasture. |
| `server_client.rs` | The global connection to the server: starts it if needed, reconnects with backoff, and feeds events to the copies below. |
| `project_store.rs`, `registry_store.rs`, `thread_entity.rs` | GPUI copies of the server's projects, registry and threads, with the same names and methods as the core types. Changes go to the server as requests and come back as events. |
| `sidebar.rs` | t3code-style thread cards (with the thread's own branch and a worktree or pasture marker), the Archived shelf, search mode, rename, context menu (New Thread Here, and a pasture's Sync and Bring Branch), details popover, Settings footer. |
| `agent_view.rs` | The thread view, after Zed's: messages, tool calls, diffs, plan, permissions, composer with config selectors, context usage, queue, slash commands, the "…" agent menu. |
| `settings_page.rs` | Settings: General (with Restart Server), Appearance, Agents (registry plus each agent's Settings panel), one page per project (with Checkouts, its worktrees and pastures). |
| `app_settings.rs` | `settings.json`: theme mode and themes. Also the server's per-agent settings (env, defaults, known options), changed through requests. |
| `project_info.rs` | Project favicons, monograms and git branches (also of worktrees and pastures), kept current by a global `ProjectInfoStore`. |
| `project_switcher.rs`, `new_thread_modal.rs` | The title bar's project picker, and the New Thread flow (project, agent, then for git repositories the workspace). |

Data lives in `~/Library/Application Support/agentZ/`:

- `state.json` (projects and threads) and `settings.json` (the app's appearance settings);
- `agents/settings.json` (per-agent settings, owned by the server), `agents/registry/` (registry
  cache, icons, installed agents), and optionally `agents/custom.json` (agents run from a fixed
  command, shown as installed: `{"mock": {"name": "Mock", "command": {"path": "/usr/bin/python3",
  "args": ["…/mock_agent.py"], "env": {}}}}`);
- `server.sock`, `server.pid`, `machine-id` and `logs/server.log`, for the server.

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
  python3 -c 'import subprocess; subprocess.Popen(["./target/debug/agentz"], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, start_new_session=True)'
  ```
  Relaunch the user's app this way after each change you ship. A plain `nohup … & disown` can be
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
  machine. Check `df -h ~` before long build sessions; deleting the incremental cache is safe.

## Rules from the user

- **Never send prompts to the user's real agents** (Claude, Codex, Devin, OpenCode, …). That
  spends their usage. Test with `crates/agent_thread/test_support/mock_agent.py`. Starting an agent
  or sending `initialize` is fine. Logging the user out of a real agent is not.
- **Never steal focus or the mouse.** Don't bring windows to the front while the user is working.
  See Testing for how to take screenshots.
- **Commit after each finished change** with a clear, imperative message that explains why. Don't
  push.
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
  `mcp <tool> <json>`), `slow`, `demo`, `write <path> <text>` and `delete <path>` script
  different turns (see its docstring).
- **Server:** `agentz_server` tests run the server in-process over in-memory streams with the
  mock agent as a custom agent; `tests/binary.rs` runs the real binary against a temporary data
  directory. `agentz_client` tests reattach to a turn in progress.
- **Agent control:** `agentz_server` tests call the tools as a thread (`Request::CallTool`) for
  their behavior and policy; `tests/agent_control.rs` has the mock agent call them through the
  real `mcp-bridge`, and runs `call` as a thread's shell would. Delegated tasks run the mock
  agent too, so a task's prompt is a mock script (`hello`, `slow`, `permission`).
- **The app against a scratch server:** set `AGENTZ_DATA_DIR` to a temporary directory and put
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

## State of the work

**Current project:** background servers, agent control (MCP/CLI), subthreads, diffs, worktrees
and pastures, terminals and SSH machines. The design is in [`docs/plan.md`](docs/plan.md). Where
it stands, and what to do next, is in [`docs/progress.md`](docs/progress.md). Keep progress.md up
to date as you work.

Done:
- Multi-project with an "All projects" scope.
- Themes, with System/Light/Dark modes.
- ACP Registry install, update and uninstall.
- The Zed-style thread view.
- Archiving with a read-only archived view.
- Rename, delete, title-only search, and the details popover.
- Settings: General, Appearance, Agents with per-agent login, defaults and environment, and
  per-project pages.
- Zed's Reauthenticate, Log Out and Reload Agent.
- The local background server: agents keep working after the app quits, and the app reattaches.

Not built yet (offered earlier, not scheduled):
- A multi-line composer.
- @-mentions and adding context.
- Pasting images.
- Opening files from tool calls.
- Searching message text.
- Deleting sessions on the agent's side.
- t3code's pin, snooze and drag-to-reorder.
- A git-branch line per thread (only the project's current branch is shown). Planned with
  worktrees in `docs/plan.md`.
