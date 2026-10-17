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

### A turn with background work ends too early

With some agents (Claude Agent here, [screenshot](backlog/background-task-turn-ends.png)),
a turn that leaves something running in the background ends, the thread shows "Waiting", and
each message the agent sends as the background work goes on gets its own turn footer (time,
copy, fork). The user likes those footers on real turns but finds them ugly here. Until
everything the agent started stops, background jobs included, the agent is working: drop the
"Waiting", and keep it all one turn.

### The new thread composer looks bad in small panes

The new thread composer doesn't fit small panes: in the Workspaces view with many threads on
one screen (3×3 or more), it looks bad. Make it more responsive.

### Droid's turn ends while its subagents still run

Factory Droid can run a subagent and end its turn while it waits for the subagents
([screenshot](backlog/droid-turn-ends-before-subagents.png): "The four reviewers are still
reading pages, so I'll wait for their reports", then the turn's footer). Claude Agent had this
kind of issue and it was fixed; check whether that fix was only for Claude rather than every
ACP agent, since Droid has the problem now.

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

### Droid still shows Working after it's done

A Factory Droid thread keeps showing Working after its turn is complete
([screenshot](backlog/droid-working-after-done.png)): its final message ("Done — committed as
… and pushed"), the plan All Done, yet the sidebar says Working, the thread says "Working for
23m 15s" and the composer shows Stop.

### A turn often starts without a message

Often a turn's response starts straight with the tool group, with no message from the agent
before it (Factory Droid here, [screenshot](backlog/turn-starts-with-tool-group.png): "Ran 26
commands, changed 1 file, and performed 8 other actions" right under the user's message). The
user sees this a lot and asks whether agentZ is missing a first message in agent responses.

### Catch up with Droid's ACP agent

The Factory Droid team has been updating its ACP agent a lot lately, and the user guesses some
things broke because of it. Check what's new in it and fix what broke. The Droid entries above
may come from this, and so may this one
([screenshot](backlog/droid-waiting-for-you.png)): "Factory Droid is waiting for you ↓" shows
over the conversation while the turn still has Stop; the user isn't sure what's wrong there.

### OpenCode's thread isn't working while its subagents run

While OpenCode runs subagents in the background, its thread doesn't show as working, yet a
background process shows in it ([screenshot](backlog/opencode-subagents-not-working.png): the
turn's footer, then "Running Author Foundation R1+R2 sets: shell" below it, and no Working in
the sidebar).

### Choose which usage window the composer shows

The usage badge beside the composer's agent picker ([screenshot](backlog/composer-usage.png),
"0%" in red beside Claude Agent) shows the lowest of the account's windows. Make what it shows
configurable. The choice the user wants: only the current session (the 5-hour window), except
when a longer window, weekly or monthly, is at 0. Then show that one, even if the current
session has plenty left, since nothing works until it resets.

The composer's account picker needs the session limits too
([screenshot](backlog/account-picker-usage.png), emails covered): each account there shows a
single "N% left". The same setting applies there.

### A read's card shows a summary line in place of the file

A read's card now shows one line, the tool's summary, where the file's lines should be
([screenshot](backlog/read-card-summary-line.png): "Read src/app.html 1 line", then "Read
/Users/…/src/app.html: lines 1-109 (109 lines, 3.4 KiB); the file h…"). The user restarted
recently and wonders whether an old server is running, or whether an ACP agent (Factory Droid)
changed something; see "Catch up with Droid's ACP agent".

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

### An optional Working group in the sidebar

Add an optional Working group to the sidebar that groups the threads whose agents are working,
as Archived groups archived threads, but with the normal thread row style. t3code has it.

### Auto-archive settings

A setting that archives threads after a period of inactivity, going by their last activity:
something like a day, 3 days or a week of inactivity.

### Feature selection in design artifacts

Design artifacts need the feature selection that the repo's design rounds (`design/`) have. The
user thinks it isn't in design artifacts yet.

### Enter in an empty composer steers the queued message

While a turn runs, a message is queued with a command (mostly Enter). The same command again,
while the composer is empty and focused, should make the queued message steer: an empty,
focused composer shows the user means it.

### Zed's smaller binaries

Zed announced ([screenshot](backlog/zed-smaller-binaries.png)) that its executable drops by
about 30% on every platform in the next Stable release (268.6 MB in 1.24.2, 183.7 MB in the
1.25.0 Preview). Check whether whatever did that affects agentZ, and whether agentZ can shrink
the same way.

### An Open in Editor button

Add an Open in Editor button, as t3code has.
