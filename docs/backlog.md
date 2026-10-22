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

Waits for "Use reported ACP v2 state with a fallback for inferred activity" below, which waits
until Zed enables ACP v2. Still to decide: what agents without v2 state do meanwhile.

## Features

### Artifacts move with their thread to another machine

From the Artifacts 2 round (`design/artifacts-2/`, decided in its intro and topic 7): an
artifact moves with its thread when the thread goes on on another machine. Nothing moves a
thread to another machine yet (Continue with Another Agent, workspace handoff and the limit
notice stay on its machine), so this waits for that. Until then, a thread can't publish a new
version of another machine's artifact ("there's no artifact … to update").

### Smaller release downloads

The release downloads are too big: 101 MB for macOS and 47 MB for Linux (v0.3.0). Bring them
down.

Findings (October 19, 2026), measured from v0.3.0's assets and release builds on this Mac:

- Most of each download is servers. The macOS zip has the universal app (41 MB compressed),
  the universal server (21 MB), the same two Mac servers again one per chip in `Resources`
  (21 MB, an exact duplicate) and both Linux servers (21 MB). The Linux tarball has the app
  (27 MB), its server (11 MB) and the arm64 Linux server (10 MB).
- One download per platform, each with only its own app and server (zip/tar.gz as now):
  Apple silicon 29.7 MB, Intel 32.1 MB, Linux x86_64 38.0 MB, Linux arm64 about 36 MB (no
  Linux arm64 app is built yet; estimated). An SSH machine of another kind would then need its
  server downloaded, for example from the release, as Zed downloads its remote server.
- xz in place of deflate takes a third off with no code change: Apple silicon 20.0 MB, Linux
  x86_64 about 26 MB. On macOS that would be a DMG with LZMA compression, as Zed ships a DMG
  (not measured); on Linux `.tar.xz`. `site/install.sh` changes with either.
- Build settings, Apple silicon, xz, app + server + icon: fat LTO 19.2 MB; fat LTO with
  `opt-level = "s"` 16.0 MB (the code is slower; likely fine for the server, untried for the
  app's drawing and terminals); plus `panic = "abort"` 13.5 MB, but a panic in any server task
  would then end the whole server and every running turn, so not for the server.
- In the app, Mermaid rendering (`merman`, Zed's feature) is about 8 MB of its 42.6 MB. Unused
  image formats (exr, tiff, dds, …) are about 0.5 MB per binary; the two crypto libraries
  (aws-lc, ring) under 1 MB together.

Suggested: one download per platform, xz/DMG, fat LTO and `opt-level = "s"` for the server
(about 18 MB for Apple silicon), then try `"s"` on the app.

### Annotate an image opened from the composer

Image annotation when an image is opened from the input composer.

### Warn agents that give pictures instead of live pages

Maybe warn agents when they give pictures instead of live pages. A setting, global or per
project (the project's setting overrides the global one), decides whether they're warned that
this is happening.

### Redesign the thread composer

Redesign the composer in threads.

### Show whether an agent or account is in use, and every agent's usage

When picking agents or accounts, show an indicator of whether each is being used (whether
it's working). Also show usage for agents with a single account: the usage percentage is
shown only for accounts, not for agents.

## Zed upstream updates

Requested October 18, 2026: add all five groups from the upstream review. The review compared
our October 1 Zed reference (`57bfce2945`) with upstream through October 17 (`56934ec056`),
245 newer commits. These are source findings; the failures have not been reproduced in agentZ.

### Follow Zed's v2 session lifecycle

agentZ is on SDK 3.4.0, as upstream is
([#65706](https://github.com/zed-industries/zed/pull/65706)); the update needed no code
changes, and agentZ still initializes with protocol v1. Session notices and context compaction
are built.

Waits until Zed enables ACP v2: the user's rule is not to enable it before Zed does. As of
these PRs, Zed's v2 backend is built but inactive ("an unselected v2 backend").

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

Waits until Zed enables ACP v2, as the v2 session lifecycle above does.

Use explicit state updates for foreground activity when available, replacing the matching
inference about autonomous work starting and ending
([state contract](https://github.com/agentclientprotocol/rust-sdk/blob/v3.4.0/md/protocol-v2.md)).
Keep the existing inference for agents without those reports and keep background work tracked
separately: v2 idle describes foreground state and does not mean all work ended. Preserve
steering and AIR functionality where agents still provide them only on v1.
