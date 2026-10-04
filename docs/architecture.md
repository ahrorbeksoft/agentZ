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
| `projects` | `ProjectStore`: projects, threads (and subthread tasks), workspaces, scope, order; `state.json`. |
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
| `agents/custom.json` | user | Agents run from a fixed command (the mock agent for tests) |
| `machine.json` | server | The machine icon chosen in Settings › Machines |
| `worktrees/`, `pastures/` | server | Threads' workspaces, `<repo>/<branch>` |
| `node/` | server | Downloaded Node.js, when the machine has none new enough |
| `server.sock`, `server.pid`, `machine-id`, `logs/server.log` | server | The running server |
| `settings.json` | app | Theme, saved machines, sidebar and terminal preferences |
| `viewed.json` | app | Completions this client has displayed |

## Features

Each entry: what it does, where it lives, and where it comes from.

### Window, sidebar and settings

- **Window** (`shell.rs`): title bar with the sidebar toggle (Cmd-B), project switcher,
  connection status, and Agents | Workspaces tabs; the sidebar; the open thread or settings; the
  diff panel; modals, which close on a press outside them. The connection status
  (`Shell::render_connection_status`, the user's choice of designs) is each machine that can't
  be reached or runs an older server, by its own icon with a dot: accent for an update,
  warning for attention, dim while it reconnects. Its tooltip says which and why, and a click
  opens Settings › Machines.
- **Projects** (`project_store.rs`, `project_switcher.rs`, `project_info.rs`,
  `add_project_modal.rs`): several projects with an "All projects" scope, custom names and icons,
  favicons or monograms, git branches. The switcher is Zed's recent-projects popover.
- **Thread cards** (`sidebar.rs`, t3code): title, agent and machine icons, the thread's own
  branch with a worktree or pasture marker, attention state, details popover (a custom anchored
  element, since GPUI tooltips follow the cursor), rename, delete, archive with an Archived
  shelf, title search, context menu. Automatic titles (the first prompt, the agent, a shell's
  folder, a terminal's agent CLI) keep updating under the user's own (`Thread::automatic_title`),
  which shows while set; clearing it shows the automatic one again, as with workspaces.
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
- **Settings** (`settings_page.rs`, t3code's layout): General (Update Server, Restart Server, start at login,
  combining repositories), Appearance (Zed's theme modes), Agents, Machines, and a page per
  project (with Checkouts).
- **Settings › Agents** (`settings_page.rs`, Zed's settings sub-pages and ACP Registry page): the
  installed agents as rows, each opening the agent's own page. Its heading has the icon, name,
  a login status badge, the version and registry links, Update when there is one, and a "⋯"
  menu with Uninstall. Below it are Account, Defaults (for new threads), Environment and Threads
  tabs (Threads is described under Agent threads).
  Account is a card: the login methods while logged out, or the account the agent reported
  with Change Account and Log Out (both described under Agent threads). Add Agent opens the ACP Registry
  page, which has search, an All / Installed / Not Installed filter, and a card for each agent.
  As in Zed, the cards are a `uniform_list` below a pinned search bar: scrolling re-renders the
  page every frame, and laying out every card held it to about 6 fps. A sub-page has Zed's back button and breadcrumb. With more than one machine, a machine
  picker sits in the header. Registry icons are single-color (`currentColor`), so they are
  drawn in the text color on a neutral tile.
- **Themes** (`app_settings.rs`, `theme_json`): System/Light/Dark with one theme for each, Zed's.

### Agent threads

- **Thread view** (`agent_view.rs`, Zed's `agent_ui` thread view): messages, tool calls, diffs,
  plan, permissions, composer with config selectors, context usage, queued messages, slash
  commands, the "…" menu with Zed's Reauthenticate, Log Out and Reload Agent.
- **Thread header** (`agent_view.rs`, t3code's `ChatHeader`; the user chose its breadcrumb from
  four designs): "project / title ⌄". The project opens New Thread in it. The title opens the
  thread's menu (Rename, Continue with Another Agent ▸, Archive, Delete…), and a double-click
  renames it in place, as you type, as the sidebar does (`TitleButton`: the second click closes
  the menu the first opened; the field takes focus after the menu's delayed focus). Then the
  branch with its worktree or pasture icon, the changes as +added −removed (the Diff icon when
  none), Terminal, and "⋯" with the agent's options. A workspace pane keeps its own header with
  the same buttons.
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
- **Agent registry** (`registry`, `registry_store.rs`): install, update, uninstall from the ACP
  Registry, binary archives or npm.
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

herdr's states, t3code's labels and colors, Zed's notifications.

- The server sends facts: `working_threads`, `blocked_threads` (a permission waiting, its own
  or a subthread's), `awaiting_input_threads` (a request for input waiting), and each thread's
  `completed_at`. Each client decides "done" against the
  completions it has displayed (`viewed.json`), so viewing in one client doesn't clear another.
- Statuses by priority: Pending Approval (warning), Awaiting Input (purple, each theme's fourth
  player color), Working, Completed. Agent control's tools report `waiting_for_input`.
- "Displayed" is Zed's `agent_status_visible`: window active, settings closed, thread open.
- Notifications ("Waiting for tool confirmation", "Waiting for your input", "Finished") for
  threads not displayed. macOS only shows them for an app bundle (`tooling/bundle-mac.sh`).

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
- **Diff panel** (`diff_panel.rs`, Cmd-D): latest turn or all, files and hunks, Viewed (a file
  reopens when it changes). Revert puts files back, only for a thread alone in its worktree or
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
  subscribed clients. Closing a terminal ends its whole session (herdr's pane shutdown).
- **App** (`terminal_entity.rs`, `terminal_view.rs`, `terminal_element.rs`, `terminal_mouse.rs`):
  Zed's element, key mappings (`agentz_protocol::terminal_keys`), IME, mouse, selection. Font
  size with Cmd-+, Cmd-- and Cmd-0.
- **Where they appear**: terminal threads (`terminal_thread_view.rs`, a login shell from New
  Thread); the thread's terminal drawer (`terminal_drawer.rs`, t3code's: Cmd-J, groups of up to
  four split terminals, a dot on its button while something runs with the drawer hidden); ACP
  client terminals, shown live in tool calls; terminal logins, under the login buttons.
- **Agent detection** (`detect.rs`, `detect/`): herdr's manifests read the bottom of the screen;
  the foreground process (via `tcgetpgrp`, since a pane's `/usr/bin/login` runs as root) names
  the agent. Runs in workspace panes and terminal threads.

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
  rename, reorder) with the agents in panes below; each row shows the icon of the project the folder is in (a folder icon outside every
  project) and the folder's name, then the branch (or the path outside git), terminal and agent
  counts and the machine's icon, laid out as the sidebar's shell rows, with the thread cards'
  details popover on hover (`sidebar::ThreadDetails`, no pane list); tab bar; panes holding a shell, an agent CLI,
  or an ACP thread; resize, zoom, swap, close. Which tab shows, focus and zoom are client-only.
  An ACP thread's pane has one header: the thread's title, agent and toolbar buttons
  (`AgentView::render_toolbar_buttons`) beside the pane's own, so its toolbar is hidden there.
  Every header along the top of a view (toolbars, tabs, pane headers, the sidebars' search and
  settings rows) is `agent_view::TOOLBAR_HEIGHT`, so their borders line up.
- **Row menu**: Rename, Close, and in a git repository (a project or not) New Worktree and
  Open Worktree… (herdr's worktree overlays, `worktree_modal.rs`). New Worktree names the branch
  (herdr's generated `agentz/<adjective>-<noun>-<hex>` by default) and makes a worktree or a
  pasture of the repository's main checkout from what the workspace has checked out
  (`Request::CreateWorkspace`, no thread in it; a project's is recorded as its workspace). Open
  Worktree… lists the repository's other checkouts (`Request::RepositoryCheckouts`: `git worktree
  list`, then a project's pastures). Either opens as a new workspace with a shell.
- **Keys** (Mac-style, in the `Workspaces` context, all with Cmd so terminals never get them):
  Cmd-T, Cmd-}/Cmd-{, Cmd-D/Cmd-Shift-D, Cmd-W, Cmd-Shift-Enter, Cmd-Option-arrows, Cmd-Shift-N.
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

- **A multi-line composer**, Zed's message editor: Shift-Enter for a new line, Enter to send,
  pasted text keeps its line breaks, the box grows a few lines then scrolls, Up/Down move between
  lines.
- **@-mentions and adding context** (files, symbols, threads) to a message.
- **Pasting images** into a message.
- **Opening files** from tool calls.
- **Searching message text**, not only titles.
- **Deleting sessions on the agent's side** when a thread is deleted.
- **t3code's pin, snooze and drag-to-reorder** for threads.
- **Automatic workspace cleanup**: t3code's inactive-days and merged rules, cow's `gc`.
- **An app icon** for the bundle.

Open questions for the user:

- Should New Thread suggest a pasture (cow) or a worktree (t3code, herdr) by default? Today the
  current checkout is the default.
- Should merged projects also require the same branch or commit? t3code doesn't.
- Should agents see and manage threads in other projects? Today, the caller's project only.
