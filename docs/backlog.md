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

### Artifacts: pages agents publish, opened in the browser

Pages an agent publishes from a thread, as Claude Code's artifacts: one self-contained `.html`
or `.md` file, opened in the user's browser, with a version for each publish and a way to
export it. Any agent can publish one, for any project. Design boards are one use of them, not a
feature of their own.

- **Stored in the data folder, like threads:** each machine's server keeps its artifacts there,
  not in the project's repo, with a version for each publish (and, for designs, the user's
  picks and the `decisions.md` spec).
- **Not part of any project:** an artifact only records the thread or chat (below) that
  published it, so chats make them too. Whether deleting that thread or chat deletes its
  artifacts isn't decided.
- **Opened in the user's own browser,** served by agentz-server on localhost. No webview.
- **Shown in the app:** after an agent publishes an artifact, the thread shows it with an Open
  button, as Claude Code shows its artifacts. Artifacts are also listed somewhere in the app
  (name and place not decided).
- **Pages use the app's theme,** the same colors and fonts as the main app.
- **Export** from the page: save the HTML or Markdown, print or save as PDF with the browser's
  print, copy the source. Files the agent makes with its own tools (Word, PowerPoint, CSV) are
  offered as downloads from the page.
- **Send back to the thread:** what the user picks or writes on the page goes to the agent, so
  it goes on without the user telling it (the design board's Submit, made general).

What they're for (from the discussion):

- **Decisions sent back:** designs; comparing options (API shapes, data models, plans) side by
  side; reviewing the agent's plan as a checklist before it starts; triage of issues, backlog
  entries or failing tests into Now, Next, Later and Drop; questions bigger than the thread's
  input forms.
- **Reports:** research ("how t3code does it", with code links and screenshots side by side);
  reviews and audits (security findings by severity, dependency upgrades, a pull request
  walkthrough with notes beside the lines); incident reports; how a feature works across
  crates; documents for other people (specs, proposals, release notes), exported.
- **Data and stats:** benchmarks before and after, test results and flaky tests, size
  breakdowns, logs grouped and counted, query results or API responses as a table to sort.
- **Progress on long work:** a migration checklist or an investigation timeline the agent
  republishes as it goes.
- **Small tools:** sliders to tune timing, colors or spacing; theme previews in light and dark;
  clickable prototypes.
- **Visual checks:** screenshot galleries of a UI change across themes, sizes and platforms (the
  Linux checks on devbox1); every icon or every state of a component.
- **Not for:** small diagrams in a reply (the Mermaid entry below), what the app already shows
  (diffs, terminals, threads), files that belong in the project.

Order, by what the uses need: first publish, versions, open and export (every use), with send
back to the thread (decisions, tuning); then pages that update in the open browser when the agent
republishes (progress). Scripts in the page come with HTML.

Notes from the discussion (not decided):

- **Claude Code's artifacts** (`code.claude.com/docs/en/artifacts`) are the model: one `.html`
  or `.md` file with no backend (Markdown renders as a styled document page), scripts only from
  a few CDNs and fonts from Google Fonts, images embedded, 16 MiB at most. Each publish is a new
  version at the same link, and an open page updates in place. A pill under the prompt shows the
  session's artifacts, and `/artifacts` lists them. They need a claude.ai login and are off by
  default in the Agent SDK, so only Claude Code gets them, not the other agents.
- **Left out of Claude's:** public and team sharing, comments, and live data through
  connectors. They need Claude's cloud; agentZ is local.
- **Claude Design** is the model for designs: `/design-sync` copies a repo's components into
  Claude Design and checks each against its Storybook story by screenshot.
- **t3code's visual replies** (`references/t3code`, `html_render`) are close: the server
  stores the page with the thread, copies images given as local paths into it, and adds a
  script giving it the theme as CSS variables and sending clicked links to the browser.
- **Agent side:** a tool to publish (title, file, and the artifact to update), and tools to list
  and read them; a built-in skill on when to make one and how to use the theme.
- Why no webview: Zed has none, and GPUI Kit's `gpui-webview` covers GPUI's overlays,
  embeds only on X11 on Linux, and would make the Linux app need WebKitGTK.
- Artifacts on another machine reach the browser through the app, as cross-machine tools do
  (`tools/relay.rs`). The local server should take only links with a secret token, listen
  only on `127.0.0.1`, and run pages in sandboxed frames: an agent's scripts run in the user's
  browser, and a page from another machine is that machine's code.
- A skill could teach agents how to mock each kind of project (simulator screenshots, Storybook,
  SwiftUI and Compose previews, Flutter goldens) and build a kit in the project's look.
- The UI parts start on the design board, as every UI change does.

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

### Chats: threads for general conversation, outside every project

Chats are for general conversation, not for work in a project. They have their own group in
the sidebar, like the Workspaces and Archived shelves, and never belong to a project.

- **Everything a thread has:** agents, accounts, models, images, files, queued messages,
  subthreads and the rest of a thread's features.
- **Mentions instead of links:** a chat can mention projects, threads and files with @, as a
  thread mentions files and threads today. A chat isn't linked to projects or threads for good
  (considered and left out).
- **Artifacts:** chats publish artifacts as threads do (see Artifacts above).
- **A setting turns chats off**, for users who don't want them. What happens to existing chats
  while it's off isn't decided.

Notes from the research (not decided):

- **t3code's threads without a project** (`references/t3code`, `docs/user/thread-sidebar.md`,
  "Start without a project") are the closest: each works in its own folder under
  `~/.t3/scratch`, named after the date, the first words of its first message and a short id;
  deleting the thread keeps the folder and the agent's files; branch, worktree and diff
  controls are hidden since the folder isn't a Git repository; it starts on the current
  machine and can move to another before its first message. Inside, they belong to a hidden
  Scratch project on each machine, and `t3_thread_launch` starts one with `scratch: true`.
- **A chat still needs a folder:** ACP's `session/new` takes a `cwd`, so each chat would get one
  of its own in the data folder, as in t3code. It runs on one machine.
- **What changes in agentZ:** `Thread::project_id` is required today, so either it becomes
  optional or chats sit under a hidden project per machine, as t3code's do. The @ menu lists
  only the thread folder's files and the project's other threads; a chat needs projects and
  every project's threads in it. A mentioned thread's conversation is already sent
  (`thread::mentioned_thread`). A mentioned project would go as its folder.
- **Agent tools:** the thread tools work only in the caller's project (agent control's
  policy). Which ones a chat's agent gets isn't decided.
- **Hidden in a chat:** worktrees, pastures, diffs, branches and project scripts.
- The UI parts start on the design board, as every UI change does.

### Storage settings: what agentZ keeps on disk, and deleting it

A settings page (name not decided: Storage or similar) that lists what agentZ produces on disk,
with sizes, so the user can see what takes space and delete it.

- **Chats,** each with its size counting everything it keeps (its conversation, attachments
  and folder), and a way to delete them.
- **Maybe more:** other caches and logs agentZ makes, and anything else it produces (not
  decided which).

Notes from the research (not decided):

- **t3code's Settings › Storage** (`references/t3code`, `docs/user/project-settings.md`,
  `apps/web/src/components/settings/StorageSettings.tsx`) has no sizes, only rules: where new
  worktrees go; automatic worktree cleanup per machine or project (after some inactive days,
  once merged, or with no commits of their own; never with uncommitted changes or a running
  session); deleting worktrees with deleted threads; and how many days to keep browser
  captures and rotated logs. Current logs and message attachments are always kept.
- **What agentZ keeps** (`docs/architecture.md`, Data): for each thread, `transcripts/<id>.json`,
  `attachments/<id>/` and `handoffs/<id>.json`; threads' `worktrees/` and `pastures/`;
  `logs/server.log`; caches such as `agents/registry/` (with installed agents) and
  downloaded `node/`. Chats and artifacts would add their own folders.
- **Each machine has its own:** every server keeps its own data folder, so the page would show
  one machine at a time, as the Agents page's machine picker does.
- **Kept safe:** a running thread's or chat's data, worktrees with uncommitted changes, and
  agents' logins in account homes (`accounts/`), which agentZ never changes.
- The UI parts start on the design board, as every UI change does.

### A better view of subagents in threads

The current view of a subagent in a thread ([screenshot](backlog/subagent-view.png)) has no
steps: it shows the subagent's title and type, the task it was given and its report, but not
what it did along the way. The user doesn't like it and wants it replaced.

- What the new view should be: not given yet, beyond not being stepless.

### A better view of subthreads in threads

When an agent starts a subthread, the thread shows "Started a subthread: <title>" with an Open
link, and under it the tool's raw JSON (`taskId`, `childThreadId`, `title`, `role`, `status`,
`workState`, …) above a collapsed Input ([screenshot](backlog/subthread-view.png)). The user
hates how it looks and wants it replaced.

- What the new view should be: not given yet.

### The message that tells an agent its delegated tasks ended

When delegated tasks end, the parent thread shows a message in the user's place, marked "Sent
by the agent in "Summarize agent control docs"": "Delegated tasks 282, 283 reached terminal
states. Use task_status with each taskId to read the results."
([screenshot](backlog/delegated-tasks-message.png)). The user doesn't want to see it.

- Find a better way to tell the agent its tasks ended. If there isn't one, hide the message
  from the user.

## Bugs

### Open the message queue when a message is queued

When the user sends a message while the agent is working, the message is queued, and the queue
list should open so it's visible.

### The thread list flashes when the account changes

The thread list flashes (or something like it) when the user changes an account
([recording](backlog/private/thread-list-flash-on-account-change.mov), kept out of git because it
shows personal information; it's only on the user's Mac). It may also happen when changing the
agent; not confirmed.
