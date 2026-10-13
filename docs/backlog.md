# Backlog

Features and bugs to implement, in the order they were given. Each entry is the user's request
as given, not yet researched. Remove an entry once it's built.

## Bugs

### Linux: the fallback delete prompt cuts off its detail text

On Linux, GPUI's fallback prompt (the one shown for Delete… confirmations) cuts off its detail
text.

### Linux: Ctrl-, doesn't open Settings while a terminal has focus

On Linux, Ctrl-, does nothing while a terminal has focus; it should open Settings as it does
elsewhere.

### Stop doesn't stop some ACP agents' turns

Stop doesn't work on some ACP agents: Factory Droid's turn, for one, doesn't stop at all. Find a
more reliable way to stop a session's turn.

### The thread header shrinks the project's name too much

With a long thread title, the header's project name shrinks to a few letters ("agentZ" shows
as "age…"). Project names shouldn't shrink this much.

### An edit's card has two headers

An edit's tool call card shows two headers, one above the other
([screenshot](backlog/edit-card-double-header.png)): "Edited docs/backlog.md +11 -0", then
"docs/backlog.md +11 -0 Open". One header is enough.

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
