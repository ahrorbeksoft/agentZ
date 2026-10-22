# Backlog

Features and bugs to implement, in the order they were given. Keep the user's request and any
findings with each entry. Remove an entry once it's built.

## Bugs

### Linux: Ctrl-, doesn't open Settings while a terminal has focus

On Linux, Ctrl-, does nothing while a terminal has focus; it should open Settings as it does
elsewhere.

Not reproduced yet: headless tests (`shell::tests::cmd_comma_opens_settings_while_…`) show the
binding (`secondary-,` in the `Shell` context) reaching `OpenSettings` with a terminal focused,
so the cause is likely in what a real Linux keyboard sends (xkb's key and `key_char` for the
comma while Ctrl is down). Needs a run under a real X or Wayland session.

Skipped for now (October 18, 2026). Not reproduced under X11 either: on Xvfb with no input
method, `xdotool key ctrl+comma` opened Settings with focus in a Workspaces pane, a thread's
terminal drawer and a terminal thread. Left to try: Wayland, an input method (IBus or Fcitx,
which get every key on X11 through XIM), a desktop shortcut on Ctrl-, and non-US layouts.

### Claude's background dev server keeps its thread Working

Requested October 21, 2026: "sometimes claude runs a background task and forget about it and
it shows it is still working … like a dev server". Asked whether to follow t3code, the user
said: "t3-code implement their own thing we use acp, we gotta find acp way of doing".

Findings, from "Mock and practice result pages" (thread 535), local time:

- Claude started `bun run dev --port 5188` in the background at 00:00 on October 20 and
  finished its turn at 00:24:33. The thread stayed Working until the user stopped the server
  from the background-task panel at 00:46:08 (Claude's record: "stopped by the user").
- Any running background task counts as reported work (`is_reported` in `AgentThread`'s
  `OwnWorkQuiet` handling), which is waited for with no limit. While it runs, the machine is
  also kept awake, the agent isn't stopped when idle, and the finished sound waits. This was
  built for bug 3 of `design/feedback/plan.md`: a build Claude said it would wait for.
- The ACP way is v2's `state_update` session update. The agent reports its foreground work as
  `running`, `requires_action` and `idle`, including work it starts by itself, "for example
  to … follow up on a completed background task". Background activity goes on while `idle`
  and doesn't change it. So a dev server leaves the thread idle, and Claude's follow-up once a
  task ends shows as running. Claude Agent 0.94.0 sends these on v2 (`v2Foreground` in its
  `dist/v2/prompt.js`), with its own model cycles after a background subagent as work of their
  own. Codex ACP 2.2.2 has `state_update` too (not read in detail).
- v1 has no state for a session, only for subagent sessions (`StateUpdate`, unstable). There
  the prompt's answer is the only end of foreground work the agent reports, and what agentZ
  does after it is inference.
- For comparison, t3code doesn't let background commands hold a thread, but does let
  subagents and monitors hold it (`backgroundWorkHoldsCompletion`).

Waits for "Use reported ACP v2 state with a fallback for inferred activity" below, part of "Use
ACP v2 with agents that support it". Still to decide: what agents on v1 do.

## Features

### Artifacts move with their thread to another machine

From the Artifacts 2 round (`design/artifacts-2/`, decided in its intro and topic 7): an
artifact moves with its thread when the thread goes on on another machine. Nothing moves a
thread to another machine yet (Continue with Another Agent, workspace handoff and the limit
notice stay on its machine), so this waits for that. Until then, a thread can't publish a new
version of another machine's artifact ("there's no artifact … to update").

### Try a size-optimized build of the app

Left from "Smaller release downloads" (built October 22, 2026: one download per platform, a DMG
per kind of Mac and `.tar.xz` on Linux, each with only its own server, the servers built with
fat LTO and `opt-level = "s"`, and the app downloading other machines' servers from its
release). The app still has the release profile (thin LTO).

Findings (October 19, 2026), Apple silicon, xz, app + server + icon: fat LTO 19.2 MB against
20.0 MB; fat LTO with `opt-level = "s"` 16.0 MB, but the code is slower, untried for the app's
drawing and terminals. In the app, Mermaid rendering (`merman`) is about 8 MB of its 42.6 MB.
Needs a check of drawing and terminal speed on a Mac before it ships.

### Use ACP v2 with agents that support it

Requested October 22, 2026: use ACP v2 with the agents that support it rather than wait for
Zed to enable it for everyone. Zed asks for v2 only behind its `acp-beta` flag (on for staff
and beta users) since [#65892](https://github.com/zed-industries/zed/pull/65892) (October 20),
and falls back to v1. The user's answers: v2 is automatic, with no setting; on v2 the
background tasks' Stop isn't shown, and Steer sends the prompt mid-turn.

- **Negotiation:** ask every agent for v2 in `initialize` and fall back to v1 as Zed's #65892
  does. An agent that answers with v1 (Droid 0.245.0) goes on with v1 on the same connection.
  One that rejects v2 with `-32022` (its data `requested: 2` and a `supported` list with 1 but
  not 2) gets a fresh v1 process. Other errors don't fall back. The SDK's
  `Client.protocol_connector()` (`unstable_protocol_v2`, already enabled) handles the first
  case but doesn't retry a rejected v2 `initialize`.
- **Who switches:** of the installed agents, only Claude Agent 0.94.0 has a v2 surface
  (`dist/v2/`, an adapter around its v1 agent). Codex ACP 2.2.2 has no v2 code in its bundle,
  Droid 0.245.0 answers v1, and Antigravity wasn't checked. The rest stay on v1, unchanged.
- **Steer on v2:** Steer shows for every agent on v2 and sends the queued message as an
  ordinary `session/prompt` at once, taking it out of agentZ's queue. For Claude that isn't
  steering: 0.94.0 and 0.94.1-preview.4 queue a prompt behind the running turn
  (`startTurn`'s `turnQueue`) and take it in once the turn ends. The prompt's answer (its
  message id) and the `user_message` update for it come then, and a cancel ends one not yet
  taken in with `-32800`. Its real steering (`_session/steering`) isn't served on v2
  (`UNSERVED_EXTENSION_KEYS` in `dist/v2/initialize.js`, which also drops `steering` from the
  v2 `initialize` answer).
- **Background tasks on v2:** don't show their Stop. Claude's v2 surface doesn't serve
  `_session/async_task/stop`. Its `jetbrains` `_meta` still reaches the agent and extension
  notifications are passed on, so the tasks should still be reported (not verified).
- **Reported state:** "Use reported ACP v2 state with a fallback for inferred activity" below,
  which fixes "Claude's background dev server keeps its thread Working" for agents on v2.
- **Session lifecycle:** "Follow Zed's v2 session lifecycle" below.
- **Different on v2, nothing lost:** agentZ's tools go through `mcp-bridge`, since Claude's v2
  surface takes only stdio and HTTP MCP servers, not MCP-over-ACP. Modes come as config options
  (v2 has no `session/set_mode`). v2 has no client terminals or file system, but Claude runs its
  commands itself on v1 too, and agentZ offers no file system. `authenticate` and `logout`
  become `auth/login` and `auth/logout`; terminal and gateway logins still pass (`auth._meta`).
- **Work it needs:** a v2 path in `agent_thread`, since the SDK keeps v1 and v2 apart
  (`schema::v2`, `Client.v2()`) and converts nothing. On v2 a turn ends with the `idle`
  `state_update`, while the prompt's answer only means it was taken in, so the turn hook
  (checkpoints), the queue, attention states and the server's agent handoff (`wire.rs` takes
  the turn's answer out of the wire) need a v2 form. `mock_agent.py` needs a v2 mode. v2 is
  still a draft, so SDK updates will break this path more often than v1's.

## Zed upstream updates

Requested October 18, 2026: add all five groups from the upstream review. The review compared
our October 1 Zed reference (`57bfce2945`) with upstream through October 17 (`56934ec056`),
245 newer commits. These are source findings; the failures have not been reproduced in agentZ.

### Follow Zed's v2 session lifecycle

agentZ is on SDK 3.4.0, as upstream is
([#65706](https://github.com/zed-industries/zed/pull/65706)); the update needed no code
changes, and agentZ still initializes with protocol v1. Session notices and context compaction
are built.

Part of "Use ACP v2 with agents that support it" (Features), which no longer waits for Zed.

Follow Zed's developing v2 session lifecycle, including resume/replay and metadata handling:
[#65698](https://github.com/zed-industries/zed/pull/65698) and
[#65709](https://github.com/zed-industries/zed/pull/65709). Treat this as a deliberate protocol
update, separate from the smaller copied-crate fixes. Test with the mock agent.

Zed added more through October 22: explicit `session/close`
([#65815](https://github.com/zed-industries/zed/pull/65815)) and `session/delete`
([#65921](https://github.com/zed-industries/zed/pull/65921)), shutting connections down
([#65873](https://github.com/zed-industries/zed/pull/65873)), keeping updates that arrive
before `session/new` answers
([#65822](https://github.com/zed-industries/zed/pull/65822)), resume snapshots and unconfirmed
state ([#65724](https://github.com/zed-industries/zed/pull/65724)), and a permission request
shown in its tool call's card
([#65909](https://github.com/zed-industries/zed/pull/65909)). Read Zed's
`agent_servers/src/acp/v2.rs` as it is when this is built.

For all copied-crate updates, preserve agentZ's Metal drawable handling, nested scrolling,
Markdown image rendering and link chips, and animation frame limits, documented under
Crates in `docs/architecture.md`.

## Approved upstream features and replacements

Requested October 18, 2026: support all of the following, with fallbacks where necessary.
Choose native capabilities when the agent supports them, and retain the existing path for
agents that do not. This explicitly authorizes agent and platform capability fallbacks; the
app and its machine servers still update together. Coordinate the ACP work with "Use ACP v2
with agents that support it" (Features). Verify behavior with the mock agent; installed adapters' support has
not been verified.

### Use reported ACP v2 state with a fallback for inferred activity

Part of "Use ACP v2 with agents that support it" (Features), which no longer waits for Zed.

Use explicit state updates for foreground activity when available, replacing the matching
inference about autonomous work starting and ending
([state contract](https://github.com/agentclientprotocol/rust-sdk/blob/v3.4.0/md/protocol-v2.md)).
Keep the existing inference for agents without those reports and keep background work tracked
separately: v2 idle describes foreground state and does not mean all work ended. Agents on v1
keep steering and AIR's functionality; on v2, Steer and background tasks' Stop work as "Use
ACP v2 with agents that support it" says.
