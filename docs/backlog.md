# Backlog

Features and bugs to implement, in the order they were given. Each entry is the user's request
as given, not yet researched. Remove an entry once it's built.

## Features

### Merge projects across machines

When the same project exists on several machines, show it as one project, not one per machine.

- Learn how t3code does it first (`references/t3code`) and follow that.
- Add some way to see whether the copies on each machine are in sync (and how they differ if
  they aren't).

### Improve the design of agents' requests for user input

Redesign every kind of input an agent can ask the user for, not just one. The example is the
question form ([screenshot](backlog/agent-question-form.png)): the "Claude Agent is asking" card
with the question, options with descriptions, an "Other" text field, and Decline / Submit.
Agents can show other kinds of input too, and all of them need the same improvement.

- What to improve: not given yet.

### Subscription timeline, as in CLIProxyAPI's management UI

Add a timeline for subscriptions like the one in CLIProxyAPI's management UI. That UI isn't in
`references/`, so study it in CLIProxyAPI itself before building.

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
- **Not for:** small diagrams in a reply (threads draw mermaid blocks), what the app already
  shows (diffs, terminals, threads), files that belong in the project.

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

### Every kind of tool call through design rounds again

Take every kind of tool call through design rounds again, agentZ's own MCP tools included. How
they look today was picked in earlier rounds (`design/thread/`, `design/tool-calls/`,
`design/agent-subagents/`); each kind now gets designs to pick from again.

- **Every kind:** agents' own tools by ACP's kinds (read, edit, delete, move, search, execute,
  think, fetch, switch mode, other); every one of agentZ's MCP tools (`AGENTZ_TOOLS`: threads,
  subthreads, delegated tasks, workspaces, terminals, commands, projects); ToolSearch; other
  MCP servers' tools; and agents' own subagents.
- **Overlaps with** A better view of subagents and A better view of subthreads above: those can
  be topics of these rounds.
- What to change: not given yet.
