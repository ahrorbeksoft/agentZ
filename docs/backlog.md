# Backlog

Features and bugs to implement, in the order they were given. Each entry is the user's request
as given, not yet researched. Remove an entry once it's built.

## Features

### Generate thread titles with an installed CLI (optional)

Some ACP agents never update their thread's title. Add an optional setting that generates titles
for those threads with a CLI the user already has: Codex, Claude or Antigravity (`agy`).

- Off by default. The user turns it on in Settings.
- Once it's on, the user chooses the provider (Codex, Claude or agy), the model, and the
  reasoning effort if that provider offers one.
- Use it only for threads whose agent doesn't set a title itself.

### Merge projects across machines

When the same project exists on several machines, show it as one project, not one per machine.

- Learn how t3code does it first (`references/t3code`) and follow that.
- Add some way to see whether the copies on each machine are in sync (and how they differ if
  they aren't).

### Show agentZ's own MCP server in MCP server settings

List agentZ's MCP server in the existing MCP servers settings next to the user's servers, so it
can be turned on or off there.

- Enabled by default.
- Maybe add a skill that explains how to use it, because agents may not understand everything
  it can do from the tool descriptions alone.

### Navigate between the user's messages in a thread

Add a control for jumping between the user's messages in a thread, like the one in
[this screenshot](backlog/turn-navigation.png): a small control at the left edge of the
conversation, with an icon and a down chevron whose tooltip says "Next turn".

### Scrollbars in threads

Add scrollbars to the whole thread and to every scrollable area inside it, such as a tool call's
output.

### Improve the design of agents' requests for user input

Redesign every kind of input an agent can ask the user for, not just one. The example is the
question form ([screenshot](backlog/agent-question-form.png)): the "Claude Agent is asking" card
with the question, options with descriptions, an "Other" text field, and Decline / Submit.
Agents can show other kinds of input too, and all of them need the same improvement.

- What to improve: not given yet.

### Project icon picker, as in t3code (Settings › a project › Project)

Today: an Icon row (Choose File…, Reset) and a Monogram row. You can't pick an icon or an emoji.

Build: one "Project icon" row with the current icon, Choose icon, Choose file and Reset. The
choice applies to every copy of a combined project.

- **Choose icon:** a dialog with three tabs and Cancel / Save icon at the bottom.
  - **Icons:** a searchable grid of Lucide icons, in one of 18 colors.
  - **Emoji:** a grid, plus a field to paste any emoji.
  - **Monogram:** letters and a color, with a preview.
- **Choose file:** a searchable list of the image files inside the project, read by the project's
  machine, so it works for remote projects too. On this Mac, "Open in Finder" picks a file
  outside the project.
- **Shown everywhere a project's icon appears:** the chosen icon, emoji or monogram first, then
  the chosen file, then the found favicon, then the automatic monogram.

To decide: agentZ has only Zed's few hundred icons, not Lucide's ~1,600, so either add Lucide's
or limit the Icons tab to what agentZ has.

### Subscription timeline, as in CLIProxyAPI's management UI

Add a timeline for subscriptions like the one in CLIProxyAPI's management UI. That UI isn't in
`references/`, so study it in CLIProxyAPI itself before building.

### Machine icons in place of machine names in the project picker

In the project picker ([screenshot](backlog/project-picker-machines.png)), show each machine's
icon in place of its name: both in the picker's button ("agentZ This Mac, Devbox 1") and in each
project's row ("This Mac, Devbox 1", "This Mac, Ahrorbek…").

### More options when creating a worktree or pasture for a new thread

When a new thread gets its own worktree or pasture, offer more choices about how it's created,
such as picking a branch and checking out from an existing branch.

- Research how t3code does it first (`references/t3code`), then decide what to build.

### Mermaid diagrams in threads, as in Zed

Draw ` ```mermaid ` code blocks in agents' messages as diagrams, the way Zed's agent threads do,
for every agent. Only mermaid: not the full set of agent-described components (charts, tables,
cards in json-render's format), which was considered and left out.

Notes from the research (Zed in `references/zed`):

- Zed turns the diagram into an SVG with the `merman` crate (`crates/mermaid_render`, merman
  `=0.8.0-alpha.5`, features `layout-cytoscape` and `svg`), colors it with the theme and accent
  colors from `player_colors`, and draws it through gpui's `svg_renderer`.
- The markdown side is `crates/markdown/src/mermaid.rs`, turned on by
  `MarkdownOptions::render_mermaid_diagrams` (set in `acp_thread.rs`'s `create_markdown`). It
  keeps the last drawn diagram while a block streams in, and the diagrams can be zoomed
  (`on_mermaid_zoom` in `thread_view.rs`). A theme change redraws them
  (`invalidate_mermaid_caches` in `conversation_view.rs`).
- agentZ already has gpui's `svg_renderer` (`parse_svg`, `render_parsed`), `usvg`, `resvg` and
  `quick-xml`. `merman` isn't in `Cargo.lock` yet. Our copy of the markdown crate has no
  `mermaid.rs`, only the parser's tests for mermaid fences.
- Supported diagram types, from Zed's system prompt: flowchart, sequence, class, state, ER,
  gantt, pie, gitgraph, mindmap, timeline, quadrant chart, xy chart and journey.

## Bugs

### Open the message queue when a message is queued

When the user sends a message while the agent is working, the message is queued, and the queue
list should open so it's visible.

### The thread list flashes when the account changes

The thread list flashes (or something like it) when the user changes an account
([recording](backlog/private/thread-list-flash-on-account-change.mov), kept out of git because it
shows personal information; it's only on the user's Mac). It may also happen when changing the
agent; not confirmed.
