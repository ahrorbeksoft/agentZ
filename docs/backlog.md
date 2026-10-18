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

### Port the Markdown, Linux font and macOS blur fixes

Our copied crates still contain the affected code for all three:

- **Tab-indented Markdown code blocks can crash:** port
  [#65398](https://github.com/zed-industries/zed/pull/65398) in `crates/markdown/src/markdown.rs`.
  Zed keys cached highlights by event indices rather than colliding source offsets.
- **Linux fallback fonts can disappear:** port
  [#65269](https://github.com/zed-industries/zed/pull/65269) in
  `crates/gpui_wgpu/src/cosmic_text_system.rs`. Fonts without an `m` glyph must remain in the
  font database for emoji, symbols and non-Latin fallback.
- **Blurred windows are only transparent on macOS 27:** port
  [#65361](https://github.com/zed-industries/zed/pull/65361) in
  `crates/gpui_macos/src/window.rs`, using the material that retains backdrop blur.

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
