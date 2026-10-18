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

### A turn often starts without a message

Often a turn's response starts straight with the tool group, with no message from the agent
before it (Factory Droid here, [screenshot](backlog/turn-starts-with-tool-group.png): "Ran 26
commands, changed 1 file, and performed 8 other actions" right under the user's message). The
user sees this a lot and asks whether agentZ is missing a first message in agent responses.

Looked into: agentZ shows every message chunk it gets. In a Factory Droid thread's transcript
here, Droid opens its turns with thoughts and tool calls and no message: its first message came
after 1, 4 and 157 entries in the thread's three turns, with 334 thoughts among 709 tool calls
in the last. So Droid sends no first message, and its thoughts are what sit folded in the "Ran
N commands…" header. Open question: show something in that place (say the latest thought's
first line) or leave it as it is.

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

### Update ACP and support session notices, compaction and v2 sessions

agentZ is on SDK 3.4.0, as upstream is
([#65706](https://github.com/zed-industries/zed/pull/65706)); the update needed no code
changes, and agentZ still initializes with protocol v1. Port stable session notices and
context compaction, which agentZ doesn't handle yet, following
[#65414](https://github.com/zed-industries/zed/pull/65414).

Follow Zed's developing v2 session lifecycle, including resume/replay and metadata handling:
[#65698](https://github.com/zed-industries/zed/pull/65698) and
[#65709](https://github.com/zed-industries/zed/pull/65709). Treat this as a deliberate protocol
update, separate from the smaller copied-crate fixes. Test with the mock agent.

### Add GPUI hot reloading and headless windowing support

Bring in optional debug hot reloading of view and component render functions through
Subsecond ([#65738](https://github.com/zed-industries/zed/pull/65738)), and evaluate the
cross-platform headless windowing API and Linux implementation
([#64969](https://github.com/zed-industries/zed/pull/64969)) for UI iteration and testing.

For all copied-crate updates, preserve agentZ's Metal drawable handling, nested scrolling,
Markdown image rendering and link chips, and animation frame limits, documented under
Crates in `docs/architecture.md`.

## Other upstream updates

Requested October 18, 2026: add the findings from the other references. Compared with our
saved snapshots, t3code had 715 newer commits, herdr 68 and OpenUsage 17. Cow had no changes
since our v0.1.10 snapshot. These are source findings; the failures have not been reproduced
in agentZ.

### Clean up archived threads' workspaces automatically

Use t3code's new archived-thread retention rules
([#15208](https://github.com/pingdotgg/t3code/pull/15208), called settled retention there),
shared-subagent-worktree cleanup
([#19748](https://github.com/pingdotgg/t3code/pull/19748)) and orphan reporting
([#19749](https://github.com/pingdotgg/t3code/pull/19749)) as the implementation reference for
our existing automatic workspace cleanup plan in `docs/architecture.md`. Active sessions,
shared worktrees and the chosen local-change protection must be respected. Any new UI starts
on the design board.

### Bound server handshakes and recover SSH connections after wake

Follow herdr's handshake deadlines
([#5242](https://github.com/herdrdev/herdr/pull/5242)), recovery of stuck shared SSH connections
([#5252](https://github.com/herdrdev/herdr/pull/5252)), and prompt recovery after wake
([#5249](https://github.com/herdrdev/herdr/pull/5249)). Our initial server welcome read in
`crates/agentz_client/src/agentz_client.rs` has no deadline, despite timeouts elsewhere in the
SSH startup path. Review the connection and reconnection paths without ending remote agents.

### Make state and transcript saves durable before replacing files

Follow herdr's [#5152](https://github.com/herdrdev/herdr/pull/5152): sync temporary file contents
to disk before replacing the saved file, then sync its parent directory. Our transcript saves
flush their buffers but do not sync to disk. Review the same pattern in project state and
other persisted files so a hard crash cannot leave a successful rename with unwritten contents.

### Check Antigravity's Argon quota and correct the Codex usage link

OpenUsage added Antigravity's separate Argon weekly quota
([#1395](https://github.com/robinebers/openusage/pull/1395)). Our generic group parser already
handles additional quota groups, and we already use the production endpoint it now prefers;
add coverage for the reported Argon payload and check disabled or invalid buckets rather than
copying its fixed pool list.

Update the Codex usage page in `crates/agentz_server/src/accounts/codex.rs` from
`https://chatgpt.com/codex/settings/usage` to `https://chatgpt.com/settings/usage`, following
OpenUsage's [#1364](https://github.com/robinebers/openusage/pull/1364).

Within this group, prioritize delegated-task questions, title isolation, terminal detection
and handshake deadlines.

## Approved upstream features and replacements

Requested October 18, 2026: support all of the following, with fallbacks where necessary.
Choose native capabilities when the agent supports them, and retain the existing path for
agents that do not. This explicitly authorizes agent and platform capability fallbacks; the
app and its machine servers still update together. Coordinate the ACP work with the SDK and
protocol update above. Verify behavior with the mock agent; installed adapters' support has
not been verified.

### Support standard ACP subagent sessions

Use typed subagent announcements, state and cancellation in place of AIR-specific
`subagent_spawned` and `subagent_state_update` parsing where supported. Standard subagents
can be adopted on v1 as well as v2; the current Claude adapter implements them
([subagent sessions](https://github.com/agentclientprotocol/claude-agent-acp#subagent-sessions)).
Keep the AIR path for agents that need it, preserve its async-task support, and avoid showing
the same child twice. Keep agentZ-owned delegated tasks and their parent relationships.

### Fork native sessions when supported

Use `session/fork` for a fork of the current session with the same agent, preserving the
agent's context instead of sending a text handoff
([SDK lifecycle](https://github.com/agentclientprotocol/rust-sdk/blob/v3.4.0/md/protocol-v2.md)).
Verify that the returned session is usable. Keep the existing handoff for unsupported agents,
cross-agent forks and earlier-turn forks the native operation cannot express. Preserve the
fork's local conversation and its link to the parent.

### Use MCP-over-ACP with the stdio bridge as a fallback

Attach agentZ's tools to the existing ACP connection where supported, replacing the
per-thread `mcp-bridge` subprocess and its extra socket connection
([native MCP-over-ACP](https://github.com/agentclientprotocol/rust-sdk/blob/v3.4.0/md/mcp-over-acp.md)).
Keep the stdio bridge for agents without that transport, and keep the standalone CLI tool
path. Preserve caller identity, account scope, cancellation and tool policy across transports.

### Use SDK session builders for session setup and restoration

Replace the corresponding handwritten new/load/resume and routing plumbing with the SDK's
session builders. Stable v1 load/resume helpers are already available
([API](https://docs.rs/agent-client-protocol/3.4.0/agent_client_protocol/struct.ConnectionTo.html#method.load_session)).
Keep agentZ's application policy for login, defaults, retained transcripts and server handoff.
Select load/resume operations according to the agent's capabilities and preserve replay order.

### Use reported ACP v2 state with a fallback for inferred activity

Use explicit state updates for foreground activity when available, replacing the matching
inference about autonomous work starting and ending
([state contract](https://github.com/agentclientprotocol/rust-sdk/blob/v3.4.0/md/protocol-v2.md)).
Keep the existing inference for agents without those reports and keep background work tracked
separately: v2 idle describes foreground state and does not mean all work ended. Preserve
steering and AIR functionality where agents still provide them only on v1.

### Replace custom question controls with Zed's ChoiceCard

Port Zed's reusable `ChoiceCard` for radio and checkbox choices
([#63885](https://github.com/zed-industries/zed/pull/63885)), replacing the hand-drawn controls
in `crates/app/src/elicitation_card.rs` with its focus and accessibility handling. Preserve
validation, Recommended tags, Other answers and multi-question navigation.

The user explicitly approved implementing Zed's ChoiceCard without a design round. Implement
the upstream component directly; no additional design-board approval is needed for this item.

### Keep each machine awake while agents work

Follow Zed's idle-sleep prevention
([#53130](https://github.com/zed-industries/zed/pull/53130)). Hold the machine's sleep inhibition
while its agents have active work, including background work, and release it when that work
ends. Own it in the machine's background server so closing the app does not end it, and a
remote machine inhibits its own sleep. Support macOS and Linux, with a graceful fallback
where sleep inhibition is unavailable.

### Step reasoning effort with keyboard shortcuts

Follow t3code's [#19928](https://github.com/pingdotgg/t3code/pull/19928): shortcuts step through
the current agent/model's advertised reasoning-effort choices. Use the existing configuration
path and keep the picker in sync; do not assume every model has the same effort levels.

### Search the conversation in a thread

Add in-thread message and plan search, beyond title search, following
[t3code's chat find](https://github.com/pingdotgg/t3code/blob/main/docs/user/keybindings.md).
Search the retained conversation, navigate between matches and reveal the matching message
in the virtualized conversation with its match highlighted.

### Snooze threads until later

Follow [t3code's thread snooze](https://github.com/pingdotgg/t3code/blob/main/docs/user/thread-sidebar.md):
hide a thread until a chosen time, including its quota reset when known, then return it to
the list. Keep snooze independent of archiving and automatic resume; waking a snoozed thread
does not itself send a prompt. Persist its wake time so snooze survives app and server restarts.

## Approved GPUI features

Requested October 18, 2026, after reviewing the
[GPUI release tracker](https://github.com/gpui-archipelago/gpui-release-notes/issues/3): add
container queries, optional reduced motion and diagnostics. These APIs are already in our
copied GPUI crates but are not wired into the app. The review was of source; runtime behavior
and performance gains have not been verified.

### Use container queries for responsive layouts

Use GPUI's `container_query`
([#60774](https://github.com/zed-industries/zed/pull/60774)) to replace the corresponding
custom width measurement and deferred rerenders in `crates/app/src/agent_view.rs`, including
compact headers and composers where suitable. The parent must supply the container's size;
its contents cannot determine that size. Preserve existing responsive behavior and retain
measurement needed for conversation scrolling and other behavior beyond layout breakpoints.

### Offer optional reduced motion

Expose an optional reduced-motion preference using GPUI's `reduce_motion` support
([#60614](https://github.com/zed-industries/zed/pull/60614)). Keep it off by default, and use
the upstream behavior for animations and animated images when enabled. Review app-owned
animations so they honor the preference too. Preserve the existing animation frame limits.

### Enable optional local performance diagnostics

Wire up GPUI's profiler feature and Zed's controls for the frame-time overlay
([#62749](https://github.com/zed-industries/zed/pull/62749)), frame and input-latency statistics,
and hang monitoring ([#64944](https://github.com/zed-industries/zed/pull/64944)). Keep these
developer diagnostics opt-in and local. Follow the upstream controls and verify that enabling
diagnostics does not itself cause continuous redraws.
