# Backlog

Features and bugs to implement, in the order they were given. Each entry is the user's request
as given, not yet researched. Remove an entry once it's built.

## Bugs

### Linux: Ctrl-, doesn't open Settings while a terminal has focus

On Linux, Ctrl-, does nothing while a terminal has focus; it should open Settings as it does
elsewhere.

Not reproduced yet: headless tests (`shell::tests::cmd_comma_opens_settings_while_…`) show the
binding (`secondary-,` in the `Shell` context) reaching `OpenSettings` with a terminal focused,
so the cause is likely in what a real Linux keyboard sends (xkb's key and `key_char` for the
comma while Ctrl is down). Needs a run under a real X or Wayland session.

### The app uses 35–45% CPU while a turn runs

While anything animates (a running turn's shimmering label or a spinner), GPUI redraws the
whole window each frame at 60 Hz, and nothing is cached, so each frame takes about 5–6 ms of
the main thread. macOS flagged agentZ for over 50% CPU for 3 minutes on Oct 9, 11 and 12, all
in this drawing path. A profile showed each frame laying out the whole window again, and
rebuilding every sidebar thread card (`render_threads`, `render_thread_card`,
`thread_details`). Two fixes:

- Cache the sidebar and thread views (`AnyView::cached`, as Zed does), so one spinner doesn't
  rebuild the whole window.
- Move per-frame work out of `render`: `Machines::archived_threads` sorts the threads, and the
  agent view clones each agent's `SessionConfigSelectOption` lists, on every frame.

### A turn often starts without a message

Often a turn's response starts straight with the tool group, with no message from the agent
before it (Factory Droid here, [screenshot](backlog/turn-starts-with-tool-group.png): "Ran 26
commands, changed 1 file, and performed 8 other actions" right under the user's message). The
user sees this a lot and asks whether agentZ is missing a first message in agent responses.

## Features

### Artifacts move with their thread to another machine

From the Artifacts 2 round (`design/artifacts-2/`, decided in its intro and topic 7): an
artifact moves with its thread when the thread goes on on another machine. Nothing moves a
thread to another machine yet (Continue with Another Agent, workspace handoff and the limit
notice stay on its machine), so this waits for that. Until then, a thread can't publish a new
version of another machine's artifact ("there's no artifact … to update").

### Images as mentions everywhere, as in the composer

Rework images. The user likes how they look in the composer (a link/mention with a hover
preview) and not the current implementation elsewhere. Make it the same experience everywhere:

- Copying what was written in the composer and pasting it somewhere else should bring the
  images along; today they're lost when pasted elsewhere.
- A sent message should show its image mentions in place, as written, so text like "in this
  [image], it looks like that and this happened: [image]" reads the same after sending.
- The image previews above user messages aren't wanted.

### A forked thread looks like a new thread

When a thread is forked, show the fork in something like the new thread view's style. The
user doesn't like the current design.

### A live page design doesn't need desktop and mobile views

When a design is a live page, it doesn't need separate desktop and mobile views. Maybe make the
preview resizable instead; the user is open to ideas.

### Feature selection in design artifacts

Design artifacts need the feature selection that the repo's design rounds (`design/`) have. The
user thinks it isn't in design artifacts yet.

### An Open in Editor button

Add an Open in Editor button, as t3code has.
