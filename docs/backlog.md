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

## Features

### Artifacts move with their thread to another machine

From the Artifacts 2 round (`design/artifacts-2/`, decided in its intro and topic 7): an
artifact moves with its thread when the thread goes on on another machine. Nothing moves a
thread to another machine yet (Continue with Another Agent, workspace handoff and the limit
notice stay on its machine), so this waits for that. Until then, a thread can't publish a new
version of another machine's artifact ("there's no artifact … to update").

### Improve subthreads again

Requested October 23, 2026, after the subthreads redesign (09f27b1), which made it clear to
agents that they can just end their turn once they delegate:

- While subthreads are working, show the agent as still working, as if it weren't the end of
  its turn.
- When a subthread's result comes back, steer the agent with it if the agent has steering of
  its own (not agentZ's custom implementation, and only if it really exists); otherwise just
  queue it.
- On the left side, show only the running agents; done ones go inside a completed tasks button
  or something like it. If none are working, all of them can be shown.
- Users should be able to work with hundreds of subthreads, if they want to finish one whole
  project in one go.

Built: a result steers into the parent's running turn when its agent has steering of its own
(Claude Agent's and Codex's `_session/steering` on ACP v1; de71e44), and the server sends the
app only the threads that changed, with `delegate_tasks` taking up to 100 tasks (fb98ec2).
Left, on the design board (`design/subthreads-3/`): the parent shown as working, the sidebar's
running-only rows, and how the sidebar and the parent's cards hold up with hundreds of
subthreads. Measured before fb98ec2, in a debug build's headless test, a sidebar with 1,000
threads and 300 subthreads took about 0.9 s per redraw, as did 1,300 plain threads, so the
sidebar's cost is the number of rows, not subthreads as such.

### Improve the subagent view

Improve the subagent view: its Cancel and Open Parent buttons look bad. Maybe there shouldn't
be buttons like that at all, maybe there should; to be decided.

On the design board: `design/subthreads-3/`.

### Subthread templates per role, on the account with the most to spare

Requested October 23, 2026: "we have subthreads, but now agents choose the models themselves,
but i am thinking why not let users choose subthread templates like idk, they choose workers,
researchers etc per agent provider and we also introduce clever selction that (idk some kinda
system that chooses the agent/acount with the most usage for the selected subthread type or
something idk". Agreed in the conversation that followed:

Then, on the roles: "we have research, review, worker, and other. users should be able choose
for these. no agent overrides, agents see these availabe ones and use them if not or asked other
agents to do things they use those", and "call them better, researcher, reviewer, worker and
other".

- **Four roles: researcher, reviewer, worker and other.** They replace today's six
  (implementation, research, review, design, test, general: `TASK_ROLES` in
  `server/tools.rs`), which are only a label on the task's message. A task the parent gives no
  role is other.
- **The user picks what each role runs on**, per role and not per agent: an ordered list of
  agent and model (and effort or mode), e.g. Researcher: Codex · gpt-5 · high, then Claude ·
  Sonnet.
- **No agent overrides.** Agents see the four roles and the user's picks for them
  (`orchestrator_capabilities`), and delegate by role. They name an agent or model themselves
  only when the role has no pick, or when the user asked for other agents to do the work; then
  they use those. (That's my reading of "use them if not or asked other agents to do things
  they use those".)
- **The account is picked for the user, the agent isn't.** Vendors' limits can't be compared
  (Claude's 40% left isn't Codex's), so the role's list orders the agents. Within an agent,
  skip accounts logged out or used up, and prefer the one that would waste the most at its
  reset (60% left resetting in an hour before 80% left resetting in 6 days), from the
  projection the limit rows already make (`usage_limits::pace`). When all of an agent's
  accounts are used up, go to the role's next entry.
- **Left out for now:** moving a running subthread to another account when it reaches its
  limit mid-turn, and roles beyond the four.
- Neither Zed nor t3code has this. It's a settings change (likely beside Subthreads at once in
  Settings › General › Threads), so it starts on the design board.

Built (3b4bf5a): the four roles, the picks the app sends every server
(`Request::SetSubthreadRoles`, `subthread-roles.json`), `orchestrator_capabilities` listing
them, and the account choice by spare usage. Left: the Settings screen for the picks, on the
design board (`design/settings-2/`).

### Reorganize Settings

The whole Settings page is a mess; organize it better now that there are so many settings.
Also:

- For machine-specific settings, a general setting that applies to all machines when set.
- Likewise for profile-specific settings: a general one for all profiles.

On the design board: `design/settings-2/`. "Profile" isn't a term in agentZ; the board reads it
as an agent's accounts and asks.

### Show shells next to the Working label

Show shells next to the Working label. In the screenshot, a Claude Agent thread shows "Working
for 13m 58s" at the end of its turn, while its running shell is in a separate "1 Background
Task" panel above the composer.

![A Claude Agent thread's Working label, with its background shell in a panel above the composer](backlog/shells-beside-working-label.png)

On the design board: `design/subthreads-3/`.

### Redesign chats

Requested October 23, 2026: redesign chats. They don't need their own grouping any more: a chat
should be just like any other thread, but show "Chat" or something like it in place of a
project name. In the project selector, it should be possible to select Chat to show only chats.

### Move the shimmer from Thinking to "Running for"

Requested October 23, 2026:

- Remove the shimmering effect from thinking.
- Remove the Show Thinking option.
- Use that shimmer effect on "Running for" instead.
