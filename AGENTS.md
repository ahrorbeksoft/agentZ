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
| `crates/app` | The application (`agentz` binary). See the modules below. |
| `crates/agent_thread` | One ACP connection and session: process, protocol, entries, permissions, config options, login/logout, reload. `test_support/mock_agent.py` is a scripted ACP agent for tests. |
| `crates/projects` | `ProjectStore`: projects (custom name and icon), threads (title, agent, session id, model, archived), scope, thread order. Saved to `state.json`. |
| `crates/registry` | `AgentRegistryStore`: fetches the ACP Registry, installs, updates and uninstalls agents (binary archives, or npm via the system `npm`), and builds the command to start one. |
| `crates/paths` | Data locations. `AGENTZ_DATA_DIR` overrides the data directory. |
| `crates/text_input` | The single-line text field (cursor blink, selection, IME). |
| `crates/theme_json` | Loads the bundled JSON themes (One, Ayu, Gruvbox in `assets/themes`). |

`crates/app/src`:

| Module | What it is |
|---|---|
| `main.rs` | Startup, actions, key bindings, menus, theme fonts. |
| `shell.rs` | The window: title bar with the project switcher, sidebar, main area (thread or settings), New Thread modal. Starts threads with each agent's environment and defaults. |
| `sidebar.rs` | t3code-style thread cards, the Archived shelf, search mode, rename, context menu, details popover, Settings footer. |
| `agent_view.rs` | The thread view, after Zed's: messages, tool calls, diffs, plan, permissions, composer with config selectors, context usage, queue, slash commands, the "…" agent menu. |
| `settings_page.rs` | Settings: General, Appearance, Agents (registry plus each agent's Settings panel), one page per project. |
| `app_settings.rs` | `settings.json`: theme mode and themes, and per-agent settings (env, defaults, known options). |
| `project_info.rs` | Project favicons, monograms and git branches, kept current by a global `ProjectInfoStore`. |
| `project_switcher.rs`, `new_thread_modal.rs` | The title bar's project picker, and the New Thread flow (project, then agent). |

Data lives in `~/Library/Application Support/agentZ/`: `state.json` (projects and threads),
`settings.json`, and `agents/registry/` (registry cache, icons, installed agents).

## Build, run, test

- **Build:** `cargo build`. This machine has no full Xcode, so Metal shaders compile at runtime
  (the `runtime_shaders` feature, on by default in `crates/app`).
- **Run:**
  ```sh
  pkill -f "target/debug/agentz$"
  nohup ./target/debug/agentz > /dev/null 2>&1 & disown
  ```
  Relaunch the user's app this way after each change you ship.
- **Check:**
  ```sh
  cargo fmt --all
  cargo clippy -p app -p agent_thread -p projects -p registry -p text_input --all-targets
  cargo test -p app -p agent_thread -p projects -p registry
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
  JSON-RPC over stdio in a few lines of Python.
- **UI behavior (hover, layout, focus, timing):** write a temporary headless GPUI test in the
  crate:
  - Add `[dev-dependencies]` for `gpui` and `http_client` with the `test-support` feature.
  - Build the real view: `crate::init_theme`, `projects::init`, and
    `registry::init(FakeHttpClient::with_404_response(), Task::ready(()).shared(), cx)`, then
    `cx.add_window_view`.
  - Probe with `.debug_selector(|| "name".into())` and `cx.debug_bounds("name")`. Drive it with
    `simulate_mouse_move` and `executor().advance_clock(..)`.
  - Restore the files afterwards. These tests were never committed.
- **Screenshots without bothering the user:**
  - Temporarily remove `cx.activate(true)`, and set `focus: false` and
    `kind: gpui::WindowKind::PopUp` in `main.rs`, with bounds about 1160×740 in a corner of the
    primary display.
  - Drive the app with temporary code (open settings, select a section), using demo data via
    `AGENTZ_DATA_DIR=<scratch>/data`.
  - Find the window id through `CGWindowListCopyWindowInfo` by pid. Don't filter on layer 0,
    because PopUp windows sit higher.
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
- **The built-in fallback theme is also called "One Dark".** Compare themes with `Arc::ptr_eq`,
  not by name.
- **Some npm agents ship a native binary as their `bin`** (Factory Droid). `registry::runs_with_node`
  decides whether to run it through `node`.
- **ACP reports an agent's settings only inside a session.** Per-agent defaults are learned from
  threads, or from Settings › Agents › Load Settings, which opens an empty session.
- **ACP can't say which account is logged in.** Don't promise an email.
- **Threads from older builds may lack `session_id` or `model`.** They fill in the next time the
  thread is opened.

## State of the work

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

Not built yet (offered earlier, not scheduled):
- A multi-line composer.
- @-mentions and adding context.
- Pasting images.
- Opening files from tool calls.
- Searching message text.
- Deleting sessions on the agent's side.
- t3code's pin, snooze and drag-to-reorder.
- A git-branch line per thread (only the project's current branch is shown).
