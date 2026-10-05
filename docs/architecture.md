# How agentZ is built

What agentZ does, where each feature lives, and which reference it was taken from. The
references are read-only clones in `references/` (gitignored); when changing a feature, read its
source there first and keep agentZ's behavior and wording the same.

| Reference | License | Taken from it |
|---|---|---|
| Zed (`references/zed`) | GPL/Apache | GPUI, `ui`, themes, the agent thread view, terminals, the SSH remote server, Node.js for npm agents |
| t3code (`references/t3code`) | MIT | Sidebar, thread cards, settings, agent control (orchestrator MCP), subthreads, checkpoints and diffs, worktrees, terminal drawer, machines and merged projects |
| herdr (`references/herdr`) | Apache-2.0 | Background server, attention states, terminal agent detection, SSH machines, the Workspaces view (spaces, tabs, split panes) |
| cow (`references/cow`) | MIT | Pastures: copy-on-write project copies, their sync and bring-back |

Code or data ported from herdr keeps its Apache-2.0 notice (`crates/agentz_server/src/detect/`).
The sounds in `assets/sounds/` are Zed's `agent_done.wav` and t3code's
`notification-input.mp3` (`agent_needs_input.mp3`, from "Notification Sound 3" by deadrobotmusic
on freesound.org, CC0).

## Architecture

```
agentz (GPUI app, a client)
  ├─ unix socket ─────────────────────────────────────► agentz-server  (this Mac)
  └─ ssh <host> ~/.agentz/server/<ver>/agentz-server proxy
                    └─ stdio ⇄ unix socket ───────────► agentz-server  (remote Mac/Linux)

agent (ACP) ─stdio MCP─► agentz-server mcp-bridge ─unix socket─► agentz-server  (same machine)
script or agent CLI ─► agentz-server call <tool> [json] ─unix socket─► agentz-server
```

- **Each machine runs one `agentz-server`**, the only owner of that machine's work: agent
  processes and sessions, terminals, projects and threads, checkpoints, workspaces (spaces),
  installed agents and their settings. Clients never substitute their own files, credentials
  or agents for the server's (t3code's rule).
- **The server doesn't link GPUI.** Headless GPUI on Linux pulls in about 450 crates; the core
  crates (`agent_thread`, `projects`, `registry`) are plain Rust on tokio instead. The app wraps
  copies of their state in GPUI entities with the same names and methods.
- **One task owns the server's state** (`server.rs`). Requests and background results arrive on
  one channel; after each batch the server sends what changed. Slow work (git, cloning,
  checkpoints) runs off that task and comes back as an input.
- **The app is a client of every enabled machine** at once. It keeps a copy of each server's
  projects, threads and states, and streams full contents (messages, terminal screens, diffs)
  only for what's on screen. Client-only state stays in the app: theme, layout, which
  completions were seen, saved machines.
- **Lifetimes.** The app starts this Mac's server on demand, detached; quitting the app leaves it
  running. Settings › General › Restart Server ends it (and its agents), and Update Server hands
  it over (below); `agentz-server stop` stops it for good. `proxy` starts a remote server detached, so a dropped SSH connection never
  stops agents (Zed's design).

### Protocol (`crates/agentz_protocol`)

- Length-prefixed JSON over any byte stream. The handshake carries protocol and server versions,
  machine id, OS/arch, the build hash, and a capability list. Clients disable only the feature a
  capability is missing for, never the connection: remote servers outlive client releases.
- Requests have ids; the server sends a request's changes before its response, so after
  `CreateThread` returns, the client already has the thread (`agentz_client`).
- The session subscription (projects, threads, states, spaces) is always on; thread details and
  terminal screens are subscribed to only while viewed.
- Every enum ends in `#[serde(untagged)] Unknown(serde_json::Value)`, so newer variants don't
  break older clients.

## Crates

agentZ's own crates. Everything else in `crates/` is copied from Zed at the same relative path.

| Crate | What it is |
|---|---|
| `app` | The `agentz` binary: the window and every view. Modules are listed under each feature below. |
| `agentz_server` | The `agentz-server` binary (`main.rs`: `run`, `start`, `proxy`, `stop`, `mcp-bridge`, `tools`, `call`, and the hidden `open-url`). |
| `agentz_protocol` | Wire format and shared types: threads (`thread.rs`), agents (`agents.rs`), diffs (`diff.rs`), worktrees and pastures (`workspace.rs`), terminals (`terminal.rs`, `terminal_keys.rs`), spaces and their pane trees (`spaces.rs`, `layout.rs`). |
| `agentz_client` | A connection to a server, and starting a local one; `ssh.rs` reaches remote ones. |
| `agent_thread` | One ACP connection and session: process, protocol, entries, permissions, requests for input (elicitations), config options, login (with an API key, a gateway, a browser or a terminal), the reported account, logout, reload, the per-turn hook. `test_support/mock_agent.py` is the scripted test agent. |
| `projects` | `ProjectStore`: projects, threads (and subthread tasks), workspaces, scope, order, pins; `order_key.rs` is t3code's fractional order keys; `state.json`. |
| `registry` | `AgentRegistryStore`: the ACP Registry, installs (binary archives, or npm), launch commands; `node_runtime.rs` finds or downloads Node.js. |
| `paths` | Data locations (`AGENTZ_DATA_DIR` overrides). |
| `text_input` | The single-line text field. |
| `theme_json` | The bundled JSON themes in `assets/themes`. |

`tooling/bundle-mac.sh` makes `target/bundle/agentZ.app`; `tooling/build-remote-servers.sh`
cross-builds the Linux servers.

### Data

In `~/Library/Application Support/agentZ/` (`~/.agentz/` on Linux):

| File | Owner | What |
|---|---|---|
| `state.json` | server | Projects, threads, workspaces |
| `spaces.json` | server | Workspaces view: spaces, tabs, pane trees |
| `agents/settings.json` | server | Per-agent env, defaults and known options |
| `agents/registry/` | server | Registry cache, icons, installed agents |
| `agents/custom.json` | server | Custom agents, run from a command (Settings › Agents › Add Custom Agent; the mock agent for tests) |
| `machine.json` | server | The machine icon chosen in Settings › Machines |
| `worktrees/`, `pastures/` | server | Threads' workspaces, `<repo>/<branch>` |
| `node/` | server | Downloaded Node.js, when the machine has none new enough |
| `server.sock`, `server.pid`, `machine-id`, `logs/server.log` | server | The running server |
| `settings.json` | app | Theme, saved machines, sidebar and terminal preferences, saved layouts |
| `viewed.json` | app | Completions this client has displayed |

## Features

Each entry: what it does, where it lives, and where it comes from.

### Window, sidebar and settings

- **Window** (`shell.rs`): title bar with the sidebar toggle (Cmd-B), project switcher,
  connection status, and Agents | Workspaces tabs, the view not shown counting its agents
  waiting for an approval or an answer beside its side, in the most urgent one's color; the
  sidebar; the open thread or settings; the
  diff panel; modals, which close on a press outside them. The connection status
  (`Shell::render_connection_status`, the user's choice of designs) is each machine that can't
  be reached or runs an older server, by its own icon with a dot: accent for an update,
  warning for attention, dim while it reconnects. Its tooltip says which and why, and a click
  opens Settings › Machines.
- **Shortcuts** (`shortcut_sheet.rs`, herdr's keybind help): Cmd-/ shows the shortcuts of
  what's focused first (a terminal, its find bar, a thread's message editor, a sidebar's
  search), then the Workspaces view's and the app's, filtered by command or key as you type.
  Each key is the one that runs from where the sheet was opened (`binding_in`: GPUI's
  `bindings_for_input` with the focused element's context stack), so a key taken there by
  another binding isn't listed. Escape closes it and gives focus back.
- **Command palette** (`command_palette.rs`, Zed's): Cmd-Shift-P lists the actions that apply
  where it was opened (`Window::available_actions`, so only what's handled there: the
  Workspaces view registers Rename Tab, Close Workspace, New Worktree… only when they apply,
  and the shell the Agents view's switcher, Changes and Terminal only there), named as Zed
  names them ("workspaces: split right") with their keys. A list's and a text field's own
  keys (`menu`, `text_input`) are left out. The chosen one runs where the palette was opened.
- **Go To** (`go_to_picker.rs`; `SpacesView::places`, `go_to_picker::thread_places`): Cmd-P
  lists the workspaces, tabs and panes, and the Agents view's threads, the view on screen's
  first, filtered by name or where they are (a pane's workspace and tab). Choosing one shows
  it in its view and focuses it. The palette, Go To, the shortcut sheet and Save Layout are the
  shell's overlays (`Shell::overlay`): each one's key closes it, one replaces another, and
  focus goes back to what had it.
- **Projects** (`project_store.rs`, `project_switcher.rs`, `project_info.rs`,
  `add_project_modal.rs`): several projects with an "All projects" scope, custom names and icons,
  favicons or monograms, git branches. The switcher is Zed's recent-projects popover.
- **Thread cards** (`sidebar.rs`, t3code): title, agent and machine icons, the thread's own
  branch with a worktree or pasture marker, attention state, details popover (a custom anchored
  element, since GPUI tooltips follow the cursor), rename, delete, archive with an Archived
  shelf, title search, context menu. Automatic titles (the first prompt, the agent, a shell's
  folder, a terminal's agent CLI) keep updating under the user's own (`Thread::automatic_title`),
  which shows while set; clearing it shows the automatic one again, as with workspaces.
- **Pinned threads** (`sidebar.rs`, `Machines::active_threads`, `projects::order_key`; t3code's
  `pinnedAt`, order keys and `planSidebarThreadDrop`, the user's picks in `design/pins/`):
  pinned cards come first, with nothing between them and the rest, each with a muted pin
  before its time or state, which unpins it ("Unpin thread"). Pin (Unpin on a pinned thread)
  is first in a card's menu and the title menu, and puts the thread above every pinned one on
  any machine (`Machines::pin_thread`). Shells, drafts, Workspaces threads and agent CLI cards
  can't be pinned (`Thread::can_pin`). Order keys (`Thread::{pin_order_key,
  active_order_key}`, fractional keys as in t3code) keep where the user put each pinned card
  and, in Newest first order, each other card; new threads lead the rest. A dragged card
  (`ThreadDrag`, sliding with `SlideDrag` like workspace rows) is raised where it was picked
  up, and the Pinned and Active labels open above the pinned cards and the rest, the section
  it would land in in the accent. Over another section its time gives way to Pin, Unpin,
  Archive or Unarchive. The Archived header shows while a card is held, at full strength, and
  in the accent with the card over it; an archived row dragged up among the cards is
  unarchived there, and pinned among the pinned ones. Letting go writes one key between its
  neighbors', or new keys for the whole section beside a card without one
  (`order_key::plan_reorder`; `Request::{PinThread, UnpinThread, ReorderThreads}`), and shows
  the new order at once (`ProjectStore` applies it before the server answers). Latest
  activity first, the rest keep that order, so a card dragged among them only unpins or
  unarchives. An agent CLI card moves only among the rest. Archiving unpins and drops a
  thread's keys. Agents pin and unpin with `agentz_thread_organize`.
- **Draft rows** (`sidebar.rs`, `Machines::typed_drafts`; t3code's `SidebarDraftBlock`): drafts
  (below, under Agent threads) aren't cards. One with text typed in it is a row above the cards,
  newest first, with t3code's pen, its project, and the first line of the text on a warning
  tint, and × (Discard draft) on hover. The open draft's row is the one it had when opened
  (`Sidebar::frozen_draft`), so it doesn't repaint as you type, and a draft never left has
  none. A thread with unsent text that isn't open gets the pen before its project ("Unsent
  draft") and a Discard draft × beside Archive. Discarding clears the text
  (`Request::SetUnsentText` with none), and the server then removes a draft no one has open.
- **Shells shelf** (`sidebar.rs`): terminal threads, named after their current folder, under the
  project that folder is in; one becomes a thread card while an agent CLI runs in it. Under a
  title that isn't the repository's name (renamed, in a subfolder, an agent CLI), the branch
  reads `repository/branch` (`sidebar::repository_branch`); workspace rows do the same. The
  repository is its main checkout's folder, so worktrees keep its name.
- **Workspaces shelf** (`sidebar.rs`, `Machines::workspaces_threads`; the user's picks in
  `design/pane-threads/`): threads started in workspace panes, above Archived and like its rows
  (the icon of the project the folder is in, or a folder's; title; last activity), closed at
  first with its count and remembered by this Mac's server
  (`Request::ToggleWorkspacesExpanded`). Under a project, those whose folder is in it. They get
  no card, mark or waiting count in the Agents view; their panes show what they do. The row
  menu is Rename, Move to Threads and Delete…: Move to Threads (`Request::MoveToAgents`) makes it
  a thread of the project its folder is in, still working there, and outside every project
  first asks "Add “~/docs” as a project?". Search finds them, and labels them and archived
  threads in faint text.
- **Settings** (`settings_page.rs`, t3code's layout): General (Update Server, Restart Server, start at login,
  combining repositories), Appearance (Zed's theme modes), Notifications (sounds and macOS
  notifications, see Attention states), Agents, Machines, and a page per project (with
  Checkouts).
- **Settings › Agents** (`settings_page.rs`, Zed's settings sub-pages and ACP Registry page): the
  installed agents as rows, each opening the agent's own page. Its heading has the icon, name,
  a login status badge, the version and registry links, Update when there is one, and a "⋯"
  menu with Uninstall. Below it are Account, Defaults (for new threads), Environment and Threads
  tabs (Threads is described under Agent threads).
  Account is a card: the login methods while logged out, or the account the agent reported
  with Change Account and Log Out (both described under Agent threads). Add Agent is Zed's
  menu: Install from Registry, Add Custom Agent, and the ACP docs. The ACP Registry
  page has search, an All / Installed / Not Installed filter, and a card for each registry agent.
  As in Zed, the cards are a `uniform_list` below a pinned search bar: scrolling re-renders the
  page every frame, and laying out every card held it to about 6 fps. A sub-page has Zed's back button and breadcrumb. With more than one machine, a machine
  picker sits in the header. Registry icons are single-color (`currentColor`), so they are
  drawn in the text color on a neutral tile.
- **Themes** (`app_settings.rs`, `theme_json`): System/Light/Dark with one theme for each, Zed's.

### Agent threads

- **Thread view** (`agent_view.rs`, Zed's `agent_ui` thread view): messages, tool calls, diffs,
  plan, permissions, composer with config selectors, context usage, queued messages, slash
  commands, the "…" menu with Zed's Reauthenticate, Log Out and Reload Agent. The config
  selectors come with the agent's session, so while the agent starts "Loading options…" with a
  spinner stands in for them.
- **Long threads** (`agent_view.rs`, `thread_entity.rs`; Zed's thread list): the conversation is
  GPUI's `list` (a head row, the entries, a tail row) in Zed's tail-follow mode, so only the rows
  in view are drawn, also while typing or as the cursor blinks. The app keeps a revision per
  entry, and the view redoes only the changed entries' markdown and row heights. The server
  diffs a thread from the first entry that changed (`AgentThread::take_entries_changed_from`)
  and sends streamed text as what was appended (`ThreadUpdate::appended`), not the whole message
  each chunk.
- **Composer** (`agent_view.rs`, `text_input`'s several-line mode; picked in `design/composer/`):
  Zed's message editor. One line, growing with the text to eight, then scrolling. Shift-Enter
  makes a new line and Enter sends; with Settings › General's "Use modifier to send" (Zed's
  `use_modifier_to_send`), Cmd-Enter sends and Enter makes a new line. Pasted text keeps its
  line breaks, and Up/Down move between rows, unless a menu is open for the composer
  (`TextInput && menu`). Right-click gives Zed's Cut, Copy, Paste and Paste as Plain Text.
  Clicks select as in Zed's editor: a double-click selects a word (or a run of punctuation or
  spaces, or a chip whole), a triple-click its line, a fourth all of it, and dragging on from a
  double- or triple-click extends by whole words or lines.
- **Mentions** (`mention_menu.rs`, `agent_view.rs`, `server/prompt_requests.rs`; Zed's mentions
  with t3code's menu, picked in `design/composer/`): @ lists matches at once, under Files and
  Threads: the thread folder's files and folders (`Request::ListFiles`, gitignored ones left
  out, fuzzy-matched in the app) and the project's other threads. A pick becomes a chip, an
  outlined box with the kind's icon and the name in the code font, which the cursor steps over
  and Backspace removes whole; hovering shows its path. Zed's + button (Add Context) types @
  narrowed to Files & Directories or Threads, or picks images. Pasting an image, or pasting or
  dropping an image file, makes an Image chip whose hover shows the picture, for agents that
  take images (ACP's prompt capabilities); other copied files become mentions, except on
  another machine's thread, which gets the path as text. Paste as Plain Text pastes only text.
- **Sending mentions** (`PromptPart`, `agent_thread::MessagePart`): a message goes as its parts
  in order. The server reads a mentioned file (up to 1 MB of text) and takes a mentioned
  thread's conversation (`thread::mentioned_thread`, the handoff's summary), starting its agent
  and waiting up to 30 seconds for it to load. As Zed sends them, a file is embedded for agents
  that take embedded context and a link otherwise, a folder is a link, a thread is embedded or
  plain text, and an image goes only to agents that take images. The user's message shows each
  mention as Zed writes one, `[@name](uri)`, also when an agent replays it.
- **Timeline** (`agent_view.rs`; t3code's `MessagesTimeline`, picked in `design/thread/`): a
  tool call is one compact row: its kind's icon, then "Ran"/"Running" and the command in the
  code font, "Edited" and the path (or "Edited N files") with +added −removed, or the agent's
  title with the thread folder stripped, always on one line (`one_line`: newlines and runs of
  spaces become one space, as t3code's truncated rows show a multi-line command, and a
  command's `\`-newline continuations too); a spinner while it runs, "Failed" when it fails, a
  chevron on hover. Every row starts closed and a click opens its output beside it (input,
  diffs, terminals, text), up to 24 rems tall; a call awaiting permission stays open. A read's
  text is the file, so it shows as one code block (`as_code_block`): Claude fences it, but Droid
  sends it bare, and as markdown it would lose its lines and indentation. Rows are
  one dim gray (`work_row_color`, t3code's secondary label: muted, a quarter of the way to the background) so they read apart
  from messages (`design/thread-rows/`). A thought is a row too (t3code's reasoning row):
  "Thinking" with t3code's shine (`shimmering_label`) while the agent thinks, then "Thought",
  opening to the text; Settings › General's "Show thinking" (`AppSettings::show_thinking`, Zed's
  `thinking_display` as expanded or collapsed) opens them all. Each run of two or more tool
  calls and thoughts folds into one line that opens to them once the agent writes a message
  after it, also mid-turn (t3code's work groups, `AgentView::folded_run`), saying what it did in
  t3code's words (`summarize_work`: "Ran 5 commands", "Read 2 files, changed 2 files, and
  performed 2 other actions"); the running turn's last run shows its rows as they come and folds
  when the turn ends, and a lone call stays a row. A run opens with its rows as they first
  showed: rows opened in it before it folded are closed again. The
  user's message is a bubble on the right with its time ("09:07", "yesterday at
  23:30") and Copy on hover. The bubble is the thread's background a tenth of the way toward its
  text (`user_message_background`), so it shows in every theme: most themes give
  `element_background` the panel's own color. While a turn runs the footer says "Working for
  12s"; under a finished answer, "Worked for 8.0s" (t3code's durations) and Copy. Both come from the server
  (`ThreadState::sent_times` by entry, `finished_turns` by each turn's end entry), so they
  survive reopening the thread; messages and turns an agent replays from history have none.
- **Steer** (`agent_view.rs`, picked in `design/thread/`): a queued message has Steer beside
  Send Now. ACP takes one prompt at a time, so Steer moves the message to the front and waits
  for the step the agent is on: once no tool call since the last message is running (or it
  asks for permission), agentZ cancels the turn and the queue sends the message. Editing or
  removing the front message disarms it.
- **Thread header** (`agent_view.rs`, t3code's `ChatHeader`; the user chose its breadcrumb from
  four designs): "project / title ⌄". The project opens New Thread in it. The title opens the
  thread's menu (Pin or Unpin where it can be pinned, Rename, Continue with Another Agent ▸
  except on a draft, Archive, Delete…), and a double-click
  renames it in place, as you type, as the sidebar does (`TitleButton`: the second click closes
  the menu the first opened; the field takes focus after the menu's delayed focus). Then the
  branch with its worktree or pasture icon, the changes as +added −removed (the Diff icon when
  none), Terminal, and "⋯" with the agent's options. A workspace pane keeps its own header with
  the same buttons. A Workspaces thread has its folder (icon and name, the path in its
  tooltip) where the project goes, and no Archive.
- **Continue with another agent** (`agentz_protocol::thread::handoff`, `Request::ContinueThread`,
  `server/workspace_requests.rs`, `continuations.rs`; t3code's context handoff, Zed's New Thread
  from Summary): a thread keeps its agent, since each agent replays only its own sessions.
  Instead the menu starts a thread with another installed agent in the same workspace, and
  agentZ writes the old conversation for it: the user's messages, the agent's replies with the
  tools it used, and the plan, in tags, keeping the first message and the latest ones within
  40,000 characters. It waits in `handoffs/<thread id>.json` and shows as a chip in the composer (the user's choice of two
  designs): a click previews exactly what goes, × drops it (`Request::DropHandoff`). It goes
  with the first message as an embedded resource (`agentz://handoff`), or as text to agents
  without `embeddedContext`, and a replayed first message shows without it
  (`without_handoff`). Sent, it links the threads (`Thread::continued_from`): the new thread
  opens with a "Continued from" divider, and the old one ends with a "Continued in" card (also
  the user's choice).
- **New Thread** (`Shell::new_thread`, `Shell::start_draft`, `new_thread_modal.rs`; t3code's
  `useHandleNewThread`, the user's choice): opens a draft right away in the shown project, or
  asks which project first when several are shown (the modal is only that picker). It reuses
  the open draft of that project and workspace while nothing is typed in it, and otherwise
  starts one with the agent of the machine's newest thread (else the first installed; with
  none, Settings › Agents opens). The new thread screen (`AgentView::render_new_thread`, the
  user's choice of designs) is "What should we work on?" over the composer, with the agent
  picker in it (installed agents, then Terminal, which replaces the draft with a shell, and
  Manage Agents…), and under it the checkout picker (Local, a new worktree or pasture, or an
  existing one), the machine picker and the branch. Changing any of them replaces the draft
  with a new one.
- **Drafts** (`Thread::is_draft`, `Thread::unsent_text`, `Server::sweep_drafts`,
  `Shell::open_thread`; t3code's draft threads and composer drafts): every new agent thread is
  a draft until its first message, so its agent starts at once, but it isn't in the thread
  list. What's typed in any thread's composer is kept on its machine (`Request::SetUnsentText`,
  half a second after typing pauses, and as the view closes) and comes back when it's opened,
  as t3code keeps composer drafts; a discard from elsewhere empties the composer
  (`AgentView::follow_discarded_unsent_text`). The shell closes a draft it moves away from
  (other views keep threads open), and the server deletes a draft with nothing typed once no
  client has had it open for 3 seconds (60 after it's made, or found at start, for a far
  client to open it). Quitting counts as leaving; Settings and Workspaces don't. A draft with
  text stays, as a draft row in the sidebar. The first message makes it a thread
  (`ThreadEvent::FirstPrompt`). A continuation is a draft too: a message queued for a login
  keeps it, and dropping the context makes it an ordinary draft.
- **No thread open** (`Shell::open_pending_draft`, `Shell::open_draft_after_archiving`,
  `Shell::render_no_thread`; t3code's index route, picked in `design/empty-states/`): the
  Agents view drops into a draft rather than stay empty. At launch, once this Mac's session
  arrives, it's in the project of the sidebar's first thread (else the first project shown).
  Archiving the open thread from this app (`ProjectStoreEvent::Archiving`) opens one in its
  project; an agent or another app archiving it leaves it on screen, read-only. Deleting it
  opens the project's first thread in the sidebar, or else a draft there. A draft waits while
  Settings or Workspaces is shown, and for an agent to be installed; until then the main area
  says "Select a thread, or start a new one".
- **Welcome** (`welcome.rs`, `Shell::render_welcome`; Zed's Welcome page, picked in
  `design/empty-states/`): before the first project, the Agents view shows "Welcome to agentZ"
  and Get Started: Open Folder…, Install an Agent… (Settings › Agents, until an agent is
  installed on a machine), Add Machine… and Settings, with their keys. The sidebar only says
  "No projects yet". The first project opens a draft in it.
- **Agent registry** (`registry`, `registry_store.rs`): install, update, uninstall from the ACP
  Registry, binary archives or npm. An npm agent installs the registry's exact version, and
  Zed's range (`0.0.0 - <version>`) only when npm refuses it, as under a min-release-age
  policy: npm resolves a range to the `latest` tag when that fits, and Grok's registry version
  is tagged `alpha`. An agent's stderr loses its terminal colors before it's logged or shown in
  an error.
- **Custom agents** (`server/custom_agents.rs`, `Request::SaveCustomAgent`, the custom agent
  form in `settings_page.rs`; Zed's Add Custom Agent form): agents run from a command, such as
  one installed outside the registry, kept per machine in `agents/custom.json` and live at
  once. The form has an optional name, the command, space-separated arguments (as in Zed) and
  variables, which become the agent's Environment tab. Saving starts the agent once, only to
  initialize it: one that doesn't start isn't kept, and the form shows why with its stderr. A
  blank name takes the one in ACP's `agentInfo` (its title, else its name), which is also
  kept to show its version, and its icon: the registry agent whose name or id matches, since
  ACP has no icons. A name an installed agent already has is refused, as pickers would show
  two alike. The id (`custom-<name>`) is fixed when it's added, since threads keep it. A custom
  agent's "⋯" menu has Configure… (the form, filled in) and Remove; the ACP Registry page
  leaves them out.
- **Agent icons** (`agent_icons.rs`, `registry`): each server downloads the registry's icons,
  and its listings name each one by a SHA-256 of its SVG. Every machine reads the same
  registry, so the app keeps one cache for all of them. It fetches an icon once
  (`Request::AgentIcons`) from the first machine to list it, usually this Mac, and shows it
  for that agent on every machine, even one that couldn't download it. Icons travel as markup
  rather than server paths, so any client (a browser too) can draw them
  (`Icon::from_svg_markup`).
- **Login state** comes from ACP only, through an empty session (see Pitfalls in `AGENTS.md`).
- **Logging in** (`agent_thread`, `server/terminal_requests.rs`, `agent_view.rs`,
  `settings_page.rs`; Zed's `terminal_auth_task`, t3code's `AcpRegistryAuth`): `agent` methods
  are ACP's `authenticate`, and the agent opens the browser itself on its machine. `terminal`
  methods, and older ones naming a command in `_meta["terminal-auth"]`
  (`agentz_protocol::thread::terminal_login_command`), run on the agent's machine in a
  `TerminalKey::Login` terminal (`Request::TerminalLogin`) that the thread's login callout or
  the agent's settings page shows. That's where the agent keeps its login, so this works for
  remote machines too. When it exits with 0 the server closes it, records the method, and
  restarts the agent, which opens its session logged in. `initialize` advertises
  `auth.terminal` and Zed's `_meta["terminal-auth"]`: without them Claude Agent offers no login
  method at all, and Codex and Devin leave out their terminal ones.
- **Logins that take something** (`agentz_protocol::thread::login_input`, `agent_login.rs`):
  a method's `_meta` asks for an API key (`api-key`, Codex's) or an LLM gateway (`gateway`,
  Claude Agent's and Codex's; `initialize` sets `auth._meta.gateway`). The form's answer goes
  back in `authenticate`'s `_meta` under the same key. Keys are masked (`TextInput::set_masked`).
- **Browser and device logins**: an agent may ask the client to open a page (a URL
  elicitation, Codex's device login), or print a link and a one-time code
  (`agentz_protocol::thread::login_code`). The login panel shows the code in boxes, Copy Code
  and "Open <host>", then waits. Cancel restarts the agent (`Request::CancelAuthentication`),
  as t3code does, since browser logins only return when the user finishes. The login's own
  page request shows in the login panel, not as a card.
- **Browser logins on SSH machines** (`agentz_server::browser`, `agentz-server open-url`,
  `Request::OpenLoginPage`, `ThreadState::login_page`, `agentz_client::ssh::HeldForward`,
  `agent_login::loopback_forwards`; VS Code Remote's `BROWSER` helper and port forwarding): a
  browser there isn't one the user sees, and the page sends it back to `localhost` there, where
  the agent waits. A server on Linux, or one SSH started, writes its own `xdg-open`,
  `x-www-browser`, `www-browser`, `sensible-browser` (and `open` on a Mac) into `browser/` in
  its data directory, and puts them first on agents' and login terminals' `PATH`, with
  `BROWSER` and `AGENTZ_CONNECTION`. While the connection logs in (`authenticate`, or its login
  terminal), they hand the page to the server, which the clients show as "Continue in your
  browser" with Copy Link and "Open <host>" (the user's choice over opening it by itself), or a
  row under the login terminal. Otherwise they run the real program. Open forwards the ports of
  the `localhost` addresses the page names (`redirect_uri`) through the machine's shared SSH
  connection (`ssh -O forward`), then opens it; the forward is cancelled 30 seconds after the
  login ends, so a fixed port (Codex's 1455) isn't left taken on the Mac.
- **The login panel** (`agent_login.rs`, agentZ's own design, since Zed only has a callout):
  `LoginLayout::Rows` on the agent's page, a row for each method; `LoginLayout::Centered` in the
  middle of a thread that needs a login, a full-width button for each method. While logged out,
  the thread's composer is dimmed, says "Log in to <agent> to send a message", and doesn't send.
- **The account** (`agentz_protocol::thread::AuthStatus`): Claude Agent and Codex report their
  login, unasked, with `_auth/status_update` (the account's email, plan and how it's logged
  in). The Account card shows it, or "Logged in" with the method agentZ logged in with.
- **Where the login came from** (`AgentSettings::{login_method, login_identity}`,
  `settings_page::render_login_source`; the user chose the note under the card from three
  designs): agents often find a login their own CLI made, which they share. The server records
  each login agentZ makes, per agent and machine, with the first account reported after it
  (`AuthStatus::identity`). It forgets it when the agent is logged out (by agentZ, or asking for
  a login) or reports another account. While logged in with nothing recorded, the note under
  the Account card is a callout: "Logged in outside agentZ", found on that machine, and that
  logging out here logs out the CLI too. Otherwise it says "Logged in from agentZ with <method>".
- **Logging out** (`confirm_dialog.rs`, t3code's dialogs): Log Out on the agent's page or in a
  thread's "…" menu first asks in a dialog in the shell's modal layer, since it stops every
  thread that shares the login. Uninstall asks in the same dialog.
- **Closing sessions** (`AgentThread::{stop_agent, close_session}`, as Zed closes a thread's
  session): an agent that advertises `sessionCapabilities.close` gets `session/close` before it
  stops (Reload Agent, the thread going away) or its session is dropped (logged out), waiting
  at most 3 seconds. A reload opens the new session only after the old one closed. A thread
  handed off to a new server leaves its session open.
- **Importing threads** (`agent_thread::list_sessions`, `server/session_requests.rs`, the agent
  page's Threads tab in `settings_page.rs`; t3code's "Native sessions", Zed's thread import):
  opening the tab starts the agent in a scratch folder and reads every page of ACP's
  `session/list` (`Request::ListAgentSessions`). The server matches each session's folder,
  made canonical, to a project's folder or one of its worktrees and pastures. The tab shows one
  project's sessions at a time, newest first, with Import and Import All
  (`Request::ImportAgentSessions`). Imported sessions become archived threads, as in Zed, so
  a bulk import doesn't flood the thread list. A session that already has a thread (by agent
  and session id) shows "In agentZ" with Open. Sessions outside every project are only
  counted, since a thread belongs to a project. Opening an imported thread loads its session,
  which replays the conversation.
- **Requests for input** (`elicitation_card.rs`, Zed's checks for ACP's `elicitation/create`): a
  card in the thread for a form (text, numbers, a choice, checkboxes) with Decline and Submit,
  or a page to open, named by its host, with a warning for non-ASCII hosts. An opened page
  stays as "Waiting for you to finish in your browser" until the agent sends
  `elicitation/complete`. × cancels.
- **Controls** (`controls.rs`): the login, account and input-request surfaces' buttons with a
  solid accent or red fill, fields with a focus ring, avatars, icon tiles and code boxes,
  where `ui`'s styles fall short.
- **Background turns** (`agentz_server`, herdr): agents keep working when the app quits; the app
  reattaches with a snapshot, then live events.

### Attention states and notifications

herdr's states, t3code's labels and colors, Zed's notifications and sound, herdr's two sounds.
Designed in `design/sounds/`.

- The server sends facts: `working_threads`, `blocked_threads` (a permission waiting, its own
  or a subthread's), `awaiting_input_threads` (a request for input waiting), and each thread's
  `completed_at`. Each client decides "done" against the
  completions it has displayed (`viewed.json`), so viewing in one client doesn't clear another.
- Statuses by priority: Pending Approval (warning), Awaiting Input (purple, each theme's fourth
  player color), Working, Completed. Agent control's tools report `waiting_for_input`.
- "Displayed" is Zed's `agent_status_visible`: window active, settings closed, thread open.
- **Sounds** (`sound.rs`, played with `NSSound`): Zed's agent-done sound as a thread finishes,
  t3code's input sound as it waits for a permission or an answer. Settings › Notifications
  sets each to Zed's Never, When hidden (not displayed) or Always; finishing defaults to When
  hidden, input to Always (as herdr always plays its request sound). Picking When hidden or
  Always plays the sound once, as macOS's Sound settings do.
- **Notifications** (`Shell::notify_attention`): "Waiting for tool confirmation", "Waiting for
  your input" or "Finished", only while agentZ isn't the active app (t3code's rule; Settings ›
  Notifications turns them off). macOS only shows them for an app bundle
  (`tooling/bundle-mac.sh`).
- **Agents in Workspaces panes** (`ServerClient::set_spaces`, `Shell::notify_pane_attention`,
  herdr's pane notifications): an agent CLI going idle after working plays the finished sound,
  and one becoming blocked the input sound, by the same settings, displayed while its tab is on
  screen. The notification is titled with the agent ("Codex", "storefront › agents ·
  Finished", or "Needs attention"), and clicking it shows and focuses the pane.

### Agent control (MCP and CLI)

t3code's orchestrator MCP (`docs/orchestration-v2/`, `apps/server/src/mcp/`), with `agentz_`
for `t3_`.

- **Delivery** (`mcp_bridge.rs`): every ACP session gets `agentz-server mcp-bridge` as a stdio MCP
  server, with a per-session credential in its environment, so the server knows the calling
  thread. Agents and terminals also get `AGENTZ_SOCKET`, `AGENTZ_THREAD_ID` and `AGENTZ_BIN_PATH`,
  for `agentz-server call <tool> [json]` (t3code's `acp-mcp-call`).
- **Tools** (`server/tools.rs`): `orchestrator_capabilities`, `agentz_thread_list`/`_read`/
  `_launch`/`_send`/`_wait`/`_interrupt`/`_update`/`_organize`/`_diff`, `create_threads`,
  `delegate_task`, `task_status`, `task_cancel`; workspace tools (`tools/workspaces.rs`); terminal
  tools after herdr's `pane` commands (`tools/terminals.rs`).
- **Policy**: the caller's project only; no broader permissions than the caller; agents can't
  delete threads or answer permissions; `clientRequestId` idempotency; agent-created threads and
  messages are marked `createdBy: agent` and shown as such.
- **Across machines** (`tools/relay.rs`): calls naming another machine go through the app, which
  reaches every machine, so they work only while the app is open.

### Subthreads

t3code's delegated tasks (`thread-lineage-and-context-transfer.md`, `ProviderSubagentBar.tsx`).

- A subthread is a thread with a `task` (`projects::Task`): parent, prompt, role, outcome. It
  gets the task prompt only, and the parent's mode, never broader.
- Finalization runs after every batch: a task ends when its child is idle with nothing queued.
  The parent hears with t3code's message; tasks unfinished at shutdown end as Interrupted.
- In the app: the parent's Agents control lists them; a subthread opens read-only; its
  permission requests show on the parent, which becomes blocked.

### Diffs

t3code's checkpoints (`apps/server/src/checkpointing/`).

- **Checkpoints** (`checkpoints.rs`): a commit of the whole tree with a private index, under
  `refs/agentz/checkpoints/<machine>/<thread>/<turn>`. Turn 0 when the first turn starts, turn N
  when it ends, through `agent_thread`'s turn hook. Never touches branches or the user's index.
  Folders outside git get none.
- **Terminal agents' turns** (`terminal_requests.rs`): an agent CLI in a terminal thread is
  checkpointed in the folder its shell is in, with turns read by agent detection: the baseline
  once the agent is seen or starts working, then a turn each time working ends. The thread
  counts as done only once that checkpoint is taken, so the diff panel reloads with it. One
  queue takes them in order. Its header's changes button shows the lines changed, as an agent
  thread's does (`agent_view::render_changes_button`).
- **Terminal threads' activity** (`terminal_requests.rs`): a shell's output counts, at most every
  10 seconds, but not a redraw within a second of a resize or focus change (opening a thread
  does both). An agent CLI's activity is its turns, since its screen also changes while it only
  waits, as a focused prompt blinks.
- **Diff panel** (`diff_panel.rs`, Cmd-D; t3code's scope menu, as the user picked): Working
  tree (everything uncommitted, untracked files too, through a private index), Branch changes
  (`base...HEAD`, the base being the branch's `gh-merge-base`, the remote's default branch, or
  `main`/`master`, as t3code finds it), Latest turn, and Turn ▸ any finished turn with when it
  finished. The working tree and branch changes are fetched again every 5 seconds while shown,
  t3code's 5-second staleness. Files and hunks, Viewed (a file reopens when it changes). The
  panel narrows when the window can't fit it beside the conversation. Revert puts files back, only for a thread alone in its worktree or
  pasture; ACP can't rewind a conversation, so only files go back.
- The diff button shows a dot while the panel is hidden and the thread has changed files.
- Diffs are parsed on the server (`agentz_protocol::diff`), capped at 10 MB of patch.

### Worktrees and pastures

t3code's workspace model, herdr's folder layout and safe removal, cow's pastures.

- A thread works in its project's checkout, a **worktree** (`git worktree add -b agentz/<id>`,
  submodules recursive), or a **pasture**: cow's `create` (`clonefile(2)` on macOS skipping build
  folders, `cp --reflink` or a full copy on Linux, git fixes, runtime-file cleanup, `.cow.json`
  `post_clone`, rollback on failure).
- `workspaces.rs` makes, removes and syncs them; `server/workspace_requests.rs` handles the
  requests. Paths are canonicalized (`/private/tmp`).
- **Pastures** sync from the project (temporary remote, rebase or merge, abort on conflict) and
  bring their branch back (cow's `sync` and `extract`).
- **Handoff** (`agentz_workspace_handoff`) moves a thread after its turn: the agent restarts in the
  new folder, with `session/load` when it can.
- **UI**: the new thread screen's checkout picker (`agent_view.rs`), card markers, thread menu (Sync,
  Bring Branch), a workspace row's New Worktree and Open Worktree… (`worktree_modal.rs`, below),
  Project Settings › Checkouts. Removing asks again when work
  would be lost; branches are kept; a workspace in use can't be removed.
- Left out of cow: symlinked dependency folders, jj, orientation files.

### Terminals

Zed's `terminal` and `terminal_view`, t3code's drawer, herdr's surface interest.

- **Server** (`terminals.rs`, `server/terminal_requests.rs`): a PTY with `alacritty_terminal`, as
  Zed's `alacritty.rs` without GPUI. 5,000 lines of scrollback, kept while nobody watches.
  Screens stream as lines of styled runs, only changed lines, at most every 16 ms, only to
  subscribed clients. Closing a terminal ends its whole session (herdr's pane shutdown), off
  the server's request loop, and keeps reading the PTY until its process is gone: a program
  writing as it ends would otherwise fill the PTY and never end.
- **App** (`terminal_entity.rs`, `terminal_view.rs`, `terminal_element.rs`, `terminal_mouse.rs`):
  Zed's element, key mappings (`agentz_protocol::terminal_keys`), IME, mouse, selection. Font
  size with Cmd-+, Cmd-- and Cmd-0.
- **Find** (Zed's terminal search): Cmd-F opens a find bar above a scrolling terminal, seeded
  with the selection. The server searches the screen and history as plain text (smart case:
  lowercase ignores case), returning the 1,024 matches nearest the prompt and the total
  (`Request::FindInTerminal`); the element highlights them in `search_match_background`, and
  the one shown is the selection (`TerminalInput::ShowMatch`, which scrolls to it). The count
  reads "n/total"; Enter and Shift-Enter step through, wrapping; Escape closes. It searches
  again as the output moves, one search at a time, and keeps the selected match. The bar sits
  outside the terminal's key context so its keys don't reach the shell.
- **Where they appear**: terminal threads (`terminal_thread_view.rs`, a login shell from New
  Thread; Cmd-W closes one as it closes a Workspaces pane, deleting the thread and asking first
  while a program runs in front of its shell); the thread's terminal drawer (`terminal_drawer.rs`, t3code's: Cmd-J, groups of up to
  four split terminals, a dot on its button while something runs with the drawer hidden); ACP
  client terminals, shown live in tool calls; terminal logins, under the login buttons.
- **Agent detection** (`detect.rs`, `detect/`): herdr's manifests read the bottom of the screen;
  the foreground process (via `tcgetpgrp`, since a pane's `/usr/bin/login` runs as root) names
  the agent. Each agent CLI maps to the ACP Registry agent whose icon stands for it
  (`terminal_programs::registry_agent`, `PaneAgent::registry_agent`). Runs in workspace panes and terminal threads.

### Machines over SSH

herdr's connection model, Zed's remote server mechanics, t3code's UI.

- **Connecting** (`agentz_client/src/ssh.rs`, `machines.rs`, `server_client.rs`): OpenSSH with
  `BatchMode` and a shared ControlMaster; `uname -sm`; the server uploaded to
  `~/.agentz/server/<version>/` (skipped when the SHA-256 matches; streamed in chunks with
  its progress shown on the machine's row, and given up only when it stops moving for a minute); `proxy` over stdio. States:
  Online, Reconnecting (backoff to 2 minutes), Attention (the error and the command to run).
- **Linux servers**: static musl from `cargo zigbuild`, stripped (about 7 MB; 48 MB unstripped).
- **Updates**: an older server keeps running beside the new binary (over SSH told by the
  installed SHA-256, on this Mac by the binary's modification time); the title bar shows it and
  Settings › Machines offers Update Server (Restart Server… for servers without the
  `hand_off` capability). When handing off fails, the app restarts the server instead and
  says so: what ran there stops, but the new server still takes over.
- **Machines settings** (`settings_page.rs`, `machine_modal.rs`, `machine_icon_picker.rs`,
  t3code's `ConnectionsSettings.tsx` and `EnvironmentRow`): one row per machine with its icon,
  name, and one line of transport, status and (when an update is installed) the server's
  version; then icon buttons for Update Server, Retry Now (while offline), Restart Server… (this
  Mac), Edit… and Remove…, and another machine's switch (connect or not). The icon opens a grid
  of the machine kinds' icons, named in tooltips (t3code's `EnvironmentIconMenu`). A switched-off row dims. The
  section header has Update All and Add Machine, which opens the Add/Edit dialog. Servers aren't stopped from the app.
- **Server handoff** (`handoff.rs`, `server/hand_off.rs`, `terminals.rs`, herdr's live
  handoff): `Request::HandOff` starts the installed binary as `run --handoff` with a socket pair
  as its stdin, flushes `state.json` and `spaces.json`, pauses every terminal's event loop and
  sends a JSON manifest (each terminal's key, spawn, size, title and its screen and scrollback
  as escape sequences, `screen_replay`), then the listening socket and the PTYs over
  `SCM_RIGHTS`. The new server adopts them (`Terminal::adopt`: the process isn't its child, so
  its end is seen by the PTY hanging up or the pid disappearing), sends SIGWINCH so full-screen
  programs redraw, and says it's ready; the old one commits, stops saving and exits without
  hanging up on the terminals. Before the commit either side gives up and the old server
  resumes its terminals.
- **Agent handoff** (`agent_thread/src/wire.rs`, `AgentThread::{pause, hand_off, adopt}`): an
  agent's pipes run through a wire under the ACP SDK that notes what's unanswered either way
  (the SDK's request ids are UUIDs, so connections can't clash). Handing off pauses each
  thread's wire between lines, then sends a marker through the SDK; once the thread sees it,
  everything read before is applied. A thread whose session is open, waiting on nothing but
  its turn, goes in the manifest (its `ThreadView`, session, MCP token, the turn's request id,
  the agent's unanswered requests and partial lines) with its stdin, stdout and stderr after
  the PTYs, its ACP terminals too. The new server opens a new SDK connection on the pipes
  without `initialize` or loading the session, receives the unanswered requests again (a
  waiting permission shows again), and takes the turn's answer out of the wire, ending the
  turn after what came before it (a second marker). It answers `A` for ready-with-agents; an
  older server answers `R` and the agents end with the old one. Agents are killed by
  `ProcessGuard` rather than `kill_on_drop`, so a released one outlives the old server; an
  adopted one's end is seen by its stdout closing. Others (starting, logging in, or with
  another request in flight, or not paused within 5 seconds) end with the old server and load
  their sessions in the new one; `TurnsRunning` lists only their running turns. MCP bridges
  reconnect when their connection closes; follow-ups go in the manifest. While handing off,
  changes to a paused thread are refused.
- **Offline machines** stay visible, dimmed, with input disabled.
- **Remote projects**: a path field completed from that machine (`directories.rs`, t3code's
  `filesystem.browse`).
- **Node.js** (`registry/src/node_runtime.rs`, Zed's `node_runtime`): the system's when 22+,
  else v24 downloaded into `node/`.
- **Merged projects** (`repositories.rs`, `machines::build_project_groups`, t3code's
  `projectGrouping.ts` and `normalizeGitRemoteUrl` with their tests): checkouts with the same
  primary remote are one project; modes `repository`, `repository_path`, `separate`. New Thread
  then asks which checkout.
- **Machine icons** (`machine_kind.rs`, t3code's `ServerEnvironmentMachine.ts`): detected from
  the hardware, or chosen by clicking the machine's icon in Settings › Machines.
- **Start at login** (`login_item.rs`): a launch agent that runs `agentz-server start` once.

### Workspaces view

herdr's model (`concepts.mdx`, `src/layout.rs`, `src/workspace/`). Called *spaces* in code,
since a thread's workspace is its checkout.

- **Server** (`spaces.rs`, `server/space_requests.rs`): spaces rooted at a folder on one machine,
  with tabs of split-pane trees (`agentz_protocol::layout`, herdr's `TileLayout` with its tests).
  Saved in `spaces.json` and restored after a restart: terminals as new shells in their folders
  (after a handoff, the same ones), threads reattached. A workspace is where most of its tabs are: each tab by its top-left pane's
  folder (a shell's foreground process, as terminal threads are followed; a thread pane's
  terminal folder or checkout), a tie going to the earliest tab. That folder, not the one it was
  opened in, names it unless renamed, and its branch and ahead/behind are looked up every
  5 seconds and whenever a pane `cd`s or tabs change (`Space::current`, not saved).
- **App** (`spaces_view.rs`, `new_space_picker.rs`): the sidebar of spaces (search, **+**,
  rename, reorder) with the agents in panes below, as herdr's two-line agent rows (state and
  machine · workspace › tab, then the agent's icon and name); each row shows the icon of the
  project the folder is in (a folder icon outside every project) and the folder's name, then
  the branch (or the path outside git), its agents by their icons (up to three, then +N), the
  terminal count, and the machine's icon once there are remote machines, laid out as the
  sidebar's shell rows, with, on hover, the
  checkout at a glance in git (branch → upstream with ↑↓, uncommitted files and lines, the last
  commit, the path, its worktrees; `SpaceGit::{upstream, changes, last_commit}`) or the thread
  cards' details popover outside it (`sidebar::ThreadDetails`, no pane list); tab bar; panes holding a shell, an agent CLI,
  or an ACP thread; resize, zoom, close. Which tab shows, focus and zoom are client-only.
  A dragged tab slides along its bar, raised (a lighter background, full-strength text, a
  shadow) and held in the bar however far the pointer strays (`TabDrag`): once its edge passes
  the middle of the next tab, that tab slides over into its place, so the order changes while
  dragging, and letting go anywhere, even past the window's edge, leaves it there
  (`SpaceRequest::MoveTab`; the bar keeps that order until the server's arrives). Held near an
  end of a bar with more tabs than fit, it scrolls the bar.
  A dragged workspace row does the same up and down the list (`SpaceDrag`; both slide with
  `slide_drag::SlideDrag`, as the Agents sidebar's cards do): raised (opaque in the selected row's color, with a shadow), held in the list
  however far left or right the pointer goes, and among its machine's rows. A group's parent
  takes its worktrees and pastures along, folded or not, and they don't drag themselves
  (herdr's block move). Letting go sends a `SpaceRequest::MoveSpace` for each workspace that
  moves (`move_block`), and the list keeps that order until the server's arrives.
  Dragging a pane's header (in a tab of several panes, not zoomed) carries a small card of it,
  192×128 with its icon and title over an empty body, the pointer in its middle
  (`PaneDrag`, `render_carried_pane`). The tab holds still while the pointer moves,
  so the places under it stay put; a quarter of a second after it stops
  (`PANE_DROP_PREVIEW_DELAY`; moves within 3px don't count, as the platform repeats the last
  move while the button is held) the tab is laid out as dropping it there would leave it, the
  pane itself dimmed to 45% at the size it would get. In another pane, a 3×3 grid decides (`pane_drop_edge`): its top row puts the pane above it, its
  bottom row below, the middle row's sides beside it (`SpaceRequest::MovePane`,
  `TileLayout::move_pane`), and the middle swaps the two. They're aimed at the panes as shown,
  but the tab only ever shows one move from its own tree, the one letting go sends
  (`find_pane_drop`): swapping with the pane now in its old place puts it back, and a place no
  one move makes changes nothing. Letting go drops it where it's aimed, shown yet or not, or as
  shown if the pointer hasn't moved since. Aimed at the pane itself nothing changes, and with
  the pointer out of the tab it goes back where it was at once, where letting go leaves it. The tab keeps the dropped layout until the server's arrives. Terminals keep their
  size while a pane is dragged, cut to the place they're shown in, and take their new size once
  it's dropped (`terminal_entity::hold_sizes`).
  An ACP thread's pane has one header: the thread's title and agent, with Agent Options
  (`AgentView::render_agent_options`) first among the pane's buttons, so its toolbar is hidden
  there (`AgentView::show_in_pane`). It has no changes or terminal drawer in a pane: the
  workspace has shells beside it.
  A terminal pane's header names what runs there (its agent, the command it was opened with,
  or the program in front of its shell) and where it is, within the workspace as
  "storefront/src" (`Pane::{program, folder}`, which the server looks up with agent detection
  and doesn't save). The title fits before the folder. The header's buttons are Split Right
  and Split Down (Zed's split icon, shading the half the new pane takes, and it turned down;
  each a menu of Shell and New Thread…), Zoom and
  Close; an unfocused pane's show on hover.
  Every header along the top of a view (toolbars, tabs, pane headers, the sidebars' search and
  settings rows) is `agent_view::TOOLBAR_HEIGHT`, so their borders line up.
- **New Workspace** (`new_space_picker.rs`, Zed's recent projects): the workspaces used here
  most recently first, then each machine's home and projects' checkouts. A folder with a
  workspace open is marked Open, and Enter goes there; Cmd-Enter opens another.
- **Needs you**: while an agent in a pane waits for an approval or an answer, a tinted strip
  above the workspaces lists each, with Go to focus its pane.
- **No workspaces** (`SpacesView::render_empty_state`, `welcome.rs`; Zed's Welcome page, picked
  in `design/empty-states/`): the main area says "No workspaces" over Get Started: New
  Workspace… (the picker), Go To…, Command Palette and Shortcuts, with their keys. The sidebar's
  list says "No workspaces yet" over a New Workspace… button, and the Agents section is left
  out.
- **Worktree groups** (herdr's `workspace_entries`): a workspace in a linked worktree of a
  repository (`SpaceGit::{checkout, main_checkout}`), or in a project's pasture, sits under the
  workspace on that repository's main checkout, however it was opened, with tree lines and its
  branch as its name (without `agentz/`). A group shows once it has both. The parent's ▾/▸
  folds it to the active worktree (client-only) and then shows the group's most urgent state.
  Closing a group's only parent closes its worktrees' workspaces too; checkouts and branches
  stay. A worktree's or pasture's row has Delete Worktree Checkout… (herdr's): it asks, removes
  it safely (`Request::RemoveWorkspace`, which takes any linked worktree, not only a project's),
  asks again before forcing when work would be lost, and closes the workspace; the branch
  stays.
- **Row menu**: New Tab, Rename, Copy Path, Reveal in Finder (this Mac), New Thread Here (in a
  project: a draft in the checkout the workspace is in, in the Agents view), Close Workspace,
  and in a git repository (a project or not) New Worktree… and Open Worktree… (herdr's worktree overlays, `worktree_modal.rs`). New Worktree names the branch
  (herdr's generated `agentz/<adjective>-<noun>-<hex>` by default) and makes a worktree or a
  pasture of the repository's main checkout from what the workspace has checked out, or a
  branch picked in the dialog, which also says where it will be made (the server's data
  folder, `RepositoryCheckouts::data_dir`) (`Request::CreateWorkspace`, no thread in it; a
  project's is recorded as its workspace). Open
  Worktree… lists the repository's other checkouts (`Request::RepositoryCheckouts`: `git worktree
  list`, then a project's pastures). Either opens as a new workspace with a shell.
- **Threads in panes**: a pane's New Thread… and a split button's New Thread… start a draft of a
  Workspaces thread (`ProjectId::WORKSPACES`, `Request::CreateWorkspacesThread`), working where
  the pane is (its shell's current folder, else the workspace's), in no project. Under its
  composer the folder is a chip (`AgentView::render_folder_picker`); in git it's a menu of
  Current checkout, New worktree and New pasture, made from the folder's repository
  (`workspaces::create_from`; a project's is recorded as its workspace), and the thread
  remembers the folder it was started in (`Thread::started_in`). It has no Terminal starter or
  machine picker. Closing the pane keeps the thread, and the pane menu's Show Thread lists it.
  The server's tools treat its folder as a project's path (`orchestrator_capabilities` has no
  project).
- **Tabs**: a tab the user hasn't named is "Tab 1", "Tab 2"… by its position, in muted text,
  so moving it renumbers it (`tab_label`); a name the user gives stays wherever it moves, and a
  rename left unchanged keeps it automatic.
- **Layouts** (`save_layout_modal.rs`, iTerm2's arrangements): a tab's menu has Save Layout…
  (also `workspaces: save layout` in the palette), which names the tab's splits and lists its
  panes, each pane with a command ticked to run it again: the command it was opened with, or
  the command line of what runs in front of its shell (`Pane::command_line`, the foreground
  process's arguments, quoted for the shell). An unticked pane or a thread pane opens a plain
  shell. Layouts are the app's (`AppSettings::saved_layouts`, as `LayoutNode` trees), so one
  opens in any workspace on any machine; saving under a taken name replaces it. Once there
  are some, the tab bar's **+** is a menu of New Tab and the layouts, each deletable from its
  row. Opening one sends `SpaceRequest::CreateTabFromLayout`: the server makes the tab with
  its splits in the workspace's folder, running each command in a shell there.
- **Closing** a pane, tab or workspace asks first while a terminal there runs something in
  front of its shell (`Pane::program`: a server, an agent CLI even at its prompt), naming it,
  as Ghostty does; idle shells and thread panes close at once (`SpacesView::confirm_close`).
- **Keys** (Mac-style, in the `Workspaces` context, all with Cmd so terminals never get them):
  Cmd-T, Cmd-}/Cmd-{, Cmd-1…9 (a tab by position), Cmd-D/Cmd-Shift-D, Cmd-W, Cmd-Shift-Enter,
  Cmd-Option-arrows, Cmd-Shift-N.
- A terminal shown in two places takes the size of the view last interacted with (herdr).

## Testing against real machines

- **SSH**: `AGENTZ_SSH_TEST_TARGET=<host> cargo test -p agentz_server --test ssh -- --ignored`.
  `AGENTZ_SSH_TEST_RESTART=1` replaces an older running server;
  `AGENTZ_SSH_TEST_INSTALL_AGENT=<id>` installs an agent there. The user's machines are `t3-home`
  and `devbox1` (both x86_64 Linux, ext4, so pastures there are full copies). Install only under
  `~/.agentz`, and leave their t3code and herdr installs alone.
- **Node.js download**: `cargo test -p registry managed_node_downloads_and_runs -- --ignored`.
- Cross-builds need about 1 GB more disk; check `df -h ~` first.

## Future plans

Wanted, not scheduled. Each should follow Zed's agent panel or t3code.

- **@-mentions of symbols** (Zed's), which needs language servers.
- **Opening files** from tool calls.
- **Searching message text**, not only titles.
- **Deleting sessions on the agent's side** when a thread is deleted.
- **t3code's snooze** for threads.
- **Automatic workspace cleanup**: t3code's inactive-days and merged rules, cow's `gc`.
- **An app icon** for the bundle.

Open questions for the user:

- Should New Thread suggest a pasture (cow) or a worktree (t3code, herdr) by default? Today the
  current checkout is the default.
- Should merged projects also require the same branch or commit? t3code doesn't.
- Should agents see and manage threads in other projects? Today, the caller's project only.
