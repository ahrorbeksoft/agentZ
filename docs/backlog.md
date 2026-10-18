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


### A newly added account shows as "New account"

After an account is added, the agent's Accounts list shows it as "New account", with no name
or usage ([screenshot](backlog/new-account-row.png), Factory Droid on proton-cloud, the
existing account's name covered). At least add a refresh button or something like it.

The Account added dialog shows it the same way
([screenshot](backlog/account-added-dialog.png), email covered): "New account", then "Its
settings were copied from … You can rename it from its ··· menu."

Adding the same account again also shows two accounts, and then one disappears.

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
([#65706](https://github.com/zed-industries/zed/pull/65706)); the update needed no code
changes, and agentZ still initializes with protocol v1. Session notices and context compaction
are built.

Follow Zed's developing v2 session lifecycle, including resume/replay and metadata handling:
[#65698](https://github.com/zed-industries/zed/pull/65698) and
[#65709](https://github.com/zed-industries/zed/pull/65709). Treat this as a deliberate protocol
update, separate from the smaller copied-crate fixes. Test with the mock agent.

For all copied-crate updates, preserve agentZ's Metal drawable handling, nested scrolling,
Markdown image rendering and link chips, and animation frame limits, documented under
Crates in `docs/architecture.md`.

## Approved upstream features and replacements

Requested October 18, 2026: support all of the following, with fallbacks where necessary.
Choose native capabilities when the agent supports them, and retain the existing path for
agents that do not. This explicitly authorizes agent and platform capability fallbacks; the
app and its machine servers still update together. Coordinate the ACP work with the SDK and
protocol update above. Verify behavior with the mock agent; installed adapters' support has
not been verified.

### Use reported ACP v2 state with a fallback for inferred activity

Use explicit state updates for foreground activity when available, replacing the matching
inference about autonomous work starting and ending
([state contract](https://github.com/agentclientprotocol/rust-sdk/blob/v3.4.0/md/protocol-v2.md)).
Keep the existing inference for agents without those reports and keep background work tracked
separately: v2 idle describes foreground state and does not mean all work ended. Preserve
steering and AIR functionality where agents still provide them only on v1.
