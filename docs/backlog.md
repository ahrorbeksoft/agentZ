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

### An Open in Editor button

Add an Open in Editor button, as t3code has.
