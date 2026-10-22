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

### Claude's background dev server keeps its thread Working, on ACP v1

Requested October 21, 2026: "sometimes claude runs a background task and forget about it and
it shows it is still working … like a dev server". Asked whether to follow t3code, the user
said: "t3-code implement their own thing we use acp, we gotta find acp way of doing".

Fixed for agents on ACP v2 (October 22, 2026), the ACP way: the agent's `idle` `state_update`
ends its turn while a background task runs, and its follow-up once the task ends shows as work
of its own. Claude Agent 0.94.0 is on v2 while the experimental ACP v2 switch is on, with
`CLAUDE_AGENT_ACP_EXPERIMENTAL_V2` set.

Still to decide: what agents on v1 do (a Claude Agent without its v2 surface). There a running
background task still counts as reported work (`is_reported` in `AgentThread`'s
`OwnWorkQuiet` handling), waited for with no limit, as built for bug 3 of
`design/feedback/plan.md` (a build Claude said it would wait for). v1 has no state for a
session, so what agentZ does after the prompt's answer is inference. For comparison, t3code
doesn't let background commands hold a thread, but does let subagents and monitors hold it
(`backgroundWorkHoldsCompletion`).

### Changing the agent drops the composer's screenshots and mentions

When the user changes the agent, any screenshots and mentions in the composer are gone.

### A steering prompt to Claude shows up only after a delay

When a prompt is sent to Claude mid-turn and the user presses Steer, it doesn't show for a
little while, then shows (it does work as steering). It seems to wait a little, maybe until
the last turn finishes. Until it shows up, show it somewhere.

## Features

### Artifacts move with their thread to another machine

From the Artifacts 2 round (`design/artifacts-2/`, decided in its intro and topic 7): an
artifact moves with its thread when the thread goes on on another machine. Nothing moves a
thread to another machine yet (Continue with Another Agent, workspace handoff and the limit
notice stay on its machine), so this waits for that. Until then, a thread can't publish a new
version of another machine's artifact ("there's no artifact … to update").

## Zed upstream updates

Requested October 18, 2026: add all five groups from the upstream review. The review compared
our October 1 Zed reference (`57bfce2945`) with upstream through October 17 (`56934ec056`),
245 newer commits. These are source findings; the failures have not been reproduced in agentZ.

### Follow Zed's v2 session lifecycle

agentZ is on SDK 3.4.0, as upstream is
([#65706](https://github.com/zed-industries/zed/pull/65706)). Session notices and context
compaction are built, and so is ACP v2 (October 22, 2026, `agent_thread/src/acp_v2.rs`): a
load is `session/resume` with a replay, sessions close and list as on v1, and a permission
request about a tool call shows in its card.

Left from Zed's v2 lifecycle, not checked against agentZ: keeping updates that arrive before
`session/new` answers ([#65822](https://github.com/zed-industries/zed/pull/65822)), resume
snapshots and unconfirmed state
([#65724](https://github.com/zed-industries/zed/pull/65724)), and shutting connections down
([#65873](https://github.com/zed-industries/zed/pull/65873)). Zed's `session/delete`
([#65921](https://github.com/zed-industries/zed/pull/65921)) deletes sessions from its
history list, which agentZ doesn't have. Read Zed's `agent_servers/src/acp/v2.rs` as it is
when this is built, and test with the mock agent (`MOCK_ACP_V2`).

For all copied-crate updates, preserve agentZ's Metal drawable handling, nested scrolling,
Markdown image rendering and link chips, and animation frame limits, documented under
Crates in `docs/architecture.md`.
