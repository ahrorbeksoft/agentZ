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

Upstream uses SDK 3.4.0
([#65706](https://github.com/zed-industries/zed/pull/65706)); agentZ uses 2.2.0 and initializes
with protocol v1. Port stable session notices and context compaction, which agentZ doesn't
handle yet, following [#65414](https://github.com/zed-industries/zed/pull/65414).

Follow Zed's developing v2 session lifecycle, including resume/replay and metadata handling:
[#65698](https://github.com/zed-industries/zed/pull/65698) and
[#65709](https://github.com/zed-industries/zed/pull/65709). Treat this as a deliberate protocol
update, separate from the smaller copied-crate fixes. Test with the mock agent.

### Port GPUI's focus cleanup performance fix

Follow [#65177](https://github.com/zed-industries/zed/pull/65177): queue final focus-handle drops
instead of scanning the entire registry before every app effect. Upstream fixed hangs when
restoring large conversations with subagents; check its benefit to agentZ's thread views.

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

### Return from a delegated task wait when the child asks a question

Follow t3code's [#19538](https://github.com/pingdotgg/t3code/pull/19538): a blocking
`delegate_task` wait returns when its child needs an answer, with the question identified and
without marking the wait as timed out. The child's later completion must still reach the
parent. Our `wait_for_task` in `crates/agentz_server/src/server/tools.rs` currently checks only
completion and timeout. Verify the behavior with the mock agent.

### Isolate Codex automatic title generation from MCP servers, plugins and apps

Follow t3code's [#17058](https://github.com/pingdotgg/t3code/pull/17058). Our Codex title runner
in `crates/agentz_server/src/title_generation.rs` lacks the overrides that disable the user's
MCP servers, plugins and apps. Titles need only their prompt; starting them in an empty folder
with a read-only sandbox does not disable the user's MCP configuration.

### Clean up archived threads' workspaces automatically

Use t3code's new archived-thread retention rules
([#15208](https://github.com/pingdotgg/t3code/pull/15208), called settled retention there),
shared-subagent-worktree cleanup
([#19748](https://github.com/pingdotgg/t3code/pull/19748)) and orphan reporting
([#19749](https://github.com/pingdotgg/t3code/pull/19749)) as the implementation reference for
our existing automatic workspace cleanup plan in `docs/architecture.md`. Active sessions,
shared worktrees and the chosen local-change protection must be respected. Any new UI starts
on the design board.

### Update herdr's terminal attention detection rules

Port the newer rules into `crates/agentz_server/src/detect/manifests/`: Claude questions in
named sessions ([#5192](https://github.com/herdrdev/herdr/pull/5192)), Codex's startup update
chooser ([#5174](https://github.com/herdrdev/herdr/pull/5174)), and newer Antigravity dialogs
([#4959](https://github.com/herdrdev/herdr/pull/4959)). Our copied manifests lack these rules.
Keep terminal detection distinct from ACP threads' reported work and background tasks.

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
