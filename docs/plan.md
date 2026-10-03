# Plan: background servers, agent control, subthreads, diffs, terminals and machines

The goal is a herdr-like experience in agentZ:

- Agents keep working after the app quits, and the app reattaches to them.
- The sidebar shows which threads need you.
- Agents can manage other agents (threads, subthreads, archive, …) through MCP or a CLI.
- Threads can work in their own git worktree, or an instant copy-on-write pasture (cow).
- Diffs show what each turn changed.
- Real terminals run agents' own CLIs.
- Other machines are reached over SSH, all in one window. The same repository on several machines
  shows as one project.

Progress is tracked in [progress.md](progress.md). Update it as you go.

## Decisions

The user made these on 2026-10-03:

| Question | Decision |
|---|---|
| Which parts of herdr? | Everything: keep the ACP thread UI, *and* add real terminal panes for agents' own CLIs. |
| How to reach machines? | SSH only. No pairing links, no listening ports, no accounts. |
| Remote OS in the first version | Macs and Linux (x86_64 and aarch64). The user's machines today are both x86_64 Linux (see Machines), so that target comes first. |
| Agent control | MCP and a CLI, so agents can see threads, agents and models, start threads, send messages, and archive or unarchive. |
| Subagents | agentZ-owned subthreads that agents start and manage, and that the user can open and watch. |
| Same repository on several machines | Merged into one project in the projects pane, named by its full repository name (`owner/repo`). |
| New thread | The user picks the machine too. |

The standing rules in `AGENTS.md` still apply:

- Copy Zed, t3code or herdr behavior; don't invent extras.
- Never prompt the user's real agents.
- Never steal focus.
- Commit each finished step.

## References

All three are read-only clones in `references/` (gitignored). Pull t3code and herdr before
starting a phase; both move fast.

- **herdr** (`references/herdr`, Apache-2.0, Rust) is the model for background servers,
  attention states, terminal panes and SSH machines. Read these:
  - `docs/next/website/src/content/docs/`: `concepts.mdx`, `connecting-machines.mdx`,
    `persistence-remote.mdx`, `session-state.mdx`, `agents.mdx`, `socket-api.mdx`,
    `agent-automation.mdx`.
  - `src/server/`, `src/client/`, `src/remote/` and `src/protocol/`.
  - `src/detect/` and `src/detect/manifests/*.toml`: terminal agent state detection.
  - Ported code or manifests must keep herdr's Apache-2.0 notice.
- **t3code** (`references/t3code`) is the model for agent control, subthreads, diffs and how
  machines and merged projects show in the UI. Read these:
  - **Agent control and subthreads:**
    - `docs/orchestration-v2/orchestrator-mcp-server.md`: the tool surface, delegated tasks,
      policy.
    - `docs/orchestration-v2/thread-lineage-and-context-transfer.md`.
    - `apps/server/src/mcp/` (toolkits, `AcpMcpStdioBridge.ts`) and
      `apps/server/src/cli/acpMcpBridge.ts`: the stdio bridge injected into ACP `session/new`,
      and the `acp-mcp-call` CLI fallback.
    - `apps/web/src/components/chat/ThreadRelationshipsControl.tsx` and
      `ProviderSubagentBar.tsx`: how subthreads are shown.
    - `docs/user/thread-sidebar.md`: the Agents section and subagent rules.
  - **Diffs:**
    - `apps/server/src/checkpointing/`: per-turn checkpoints as hidden git refs, and diff
      queries.
    - `docs/user/providers-acp.md`: checkpoints with ACP agents.
  - **Worktrees:**
    - `docs/user/thread-sidebar.md`: New worktree, and new thread in this worktree.
    - `docs/user/keybindings.md`: the workspace menu.
    - `docs/user/project-settings.md`: branch naming, submodules, cleanup.
    - `docs/user/composer.md`: file restore only in worktrees.
    - `apps/server/src/vcs/GitVcsDriverCore.ts`: creating them.
    - `apps/server/src/mcp/toolkits/worktree/`: `t3_worktree_status`, `t3_worktree_list`,
      `t3_worktree_handoff`, and `t3_thread_launch`'s `workspaceStrategy`.
    - herdr's `configuration.mdx` § Worktrees: the folder layout, and safe removal.
- **cow** (`references/cow`, MIT, Rust) is the model for copy-on-write workspaces. Read these:
  - `README.md`;
  - `src/commands/create.rs`: clonefile, exclusions, git fixes, cleanup, `.cow.json`;
  - `src/commands/sync.rs`, `extract.rs`, `remove.rs`, `gc.rs`;
  - `src/commands/mcp.rs`: its tools.
  - **Terminals:** `docs/user/terminal.md` (server-side scrollback limits) and
    `docs/user/providers-acp.md` (agents running commands in app terminals).
  - **Machines and merged projects:**
    - `docs/internals/remote.md`, `overview.md`, `connection-runtime.md`.
    - `docs/user/remote-access.md`, `background-service.md`.
    - `packages/client-runtime/src/state/projectGrouping.ts`: repository-identity grouping.
    - `packages/contracts/src/environment.ts`: `RepositoryIdentity`.
    - `apps/web/src/components/ProjectEnvironmentBadge.tsx`, `EnvironmentMachineIcon.tsx`,
      `BranchToolbarEnvironmentSelector.tsx`.
- **Zed** (`references/zed`) is the model for the mechanics:
  - `crates/remote` and `crates/remote_server`:
    - the SSH transport;
    - uploading the server;
    - the `proxy` subcommand that bridges SSH stdio to a daemon over unix sockets;
    - Linux builds with `cargo zigbuild` (`remote/src/transport.rs`,
      `build_remote_server_from_source`).
  - `crates/terminal` (built on `alacritty_terminal`) and
    `crates/terminal_view/src/terminal_element.rs` (the GPUI renderer).
  - `crates/agent_ui/src/agent_diff.rs`: how Zed reviews an agent's changes.

## Architecture

```
agentz (GPUI app, a client)
  ├─ unix socket ─────────────────────────────────────► agentz-server  (this Mac)
  └─ ssh <host> ~/.agentz/server/<ver>/agentz-server proxy
                    └─ stdio ⇄ unix socket ───────────► agentz-server  (remote Mac/Linux)

agent (ACP) ─stdio MCP─► agentz-server mcp-bridge ─unix socket─► agentz-server  (same machine)
agent CLI in a terminal ─► agentz <command> --json ─unix socket─► agentz-server
```

**Each machine runs one `agentz-server`.** It is the only owner of that machine's work:

- agent processes (ACP connections and sessions);
- terminals (PTYs and terminal state);
- projects and threads (`state.json`), including the subthread lineage;
- checkpoints and diffs;
- installed agents (the registry) and per-agent settings.

Clients never substitute their own files, credentials or agents for the server's (t3code's
rule).

**The server doesn't use GPUI.** Only the app draws, so only the app links GPUI. (Zed's
`remote_server` runs GPUI headless instead. That was the first plan, rejected on 2026-10-03.)

- **Why:** headless GPUI on Linux pulls in about 450 crates: fonts (`fontdb`,
  `fontconfig-parser`), desktop portals (`ashpd`), text layout, images. That's a few hundred
  more than the server needs. The cost is a bigger binary, slower cross-builds, and more C code
  to cross-compile, on machines as small as `t3-home`.
- **The core crates become plain Rust.** Today `agent_thread`, `projects` and `registry` (about
  4,000 lines) are GPUI entities:
  - `Context<Self>`, `cx.notify()` and `cx.emit()` become plain structs that publish events
    on a channel subscribers listen to.
  - `cx.spawn` and `Task` become tasks on **tokio**:
    - `reqwest_client` already runs tokio, and herdr uses it.
    - The current `smol::process` calls become `tokio::process`.
  - `Global` stores become fields owned by the server.
  - `SharedString` becomes `Arc<str>` or `String`.
  - ACP handling, the registry logic, persistence and the mock agent stay as they are.
- **Shared crates stay as they are.** `http_client`, `reqwest_client`, `util`, `paths` and
  `collections` already work without GPUI. They only use the small `gpui_util`.
- **In the app**, the client copies of projects and threads are GPUI entities, filled from the
  protocol. `agent_view` and the rest of the UI keep reading them through `cx`.

**The app is a client.** It connects to every enabled machine at once:

- It keeps a read-only copy of each server's projects, threads and states.
- It only streams the full contents (messages, terminal screens, diffs) of what's on screen.
- It keeps client-only state locally: theme, window layout, which completions you've seen,
  saved machines.

**`proxy`** is a subcommand of the server binary. It connects its stdin/stdout to the server's
socket, and starts the server detached if it isn't running. Because the server isn't a child of
the SSH session, a dropped connection never stops agents. That's Zed's design.

**Lifetimes:**

- The app starts the local server on demand, detached. Quitting the app leaves it running.
- "Restart Server" (Settings › General) ends it and its agents and terminals. The app then
  starts a new one, since a running app always needs it. `agentz-server stop` stops it for good.
- Starting at login (launchd/systemd) is optional and comes later.

### Protocol (`agentz_protocol`)

- **Framing:** length-prefixed messages over any byte stream (unix socket or SSH stdio).
- **Handshake:** protocol version, server version, machine id, OS/arch, and a capability list.
- **Capabilities:** clients use the server's advertised capabilities and disable only the missing
  feature, never the whole connection. Remote servers outlive client releases (herdr's
  stable-endpoint rules, t3code's capability rule).
- **Messages:**
  - Requests with ids and responses: create a thread, send a prompt, answer a permission,
    resize a terminal, …
  - Subscriptions: first a snapshot, then events.
  - The session subscription (projects, threads, states) is cheap and always on.
  - Thread-detail, terminal-screen and diff subscriptions are opened only for what a client is
    viewing.
- **Encoding:** serde. JSON first, for debuggability. Switch terminal frames to bincode if
  they're too big.
- **Machine identity:** a stable id stored by each server, independent of the route used to
  reach it (t3code).
- **One service, several transports.** The app, the MCP bridge and the CLI all call the same
  server operations. t3code's `ThreadManagementService` is the shared boundary; transports only
  authenticate and shape responses.

### Attention states

Every thread has a state, rolled up to its project (herdr):

| State | ACP thread | Terminal thread |
|---|---|---|
| working | a prompt is running | the manifest matches "working" |
| blocked | a permission request is waiting (its own, or a subthread's) | the manifest matches an approval prompt |
| done | finished, and this client hasn't viewed it | same |
| idle | finished and viewed | same |
| unknown | — | no manifest matches |

- "Viewed" is per client (herdr).
- Notifications: a macOS notification when a thread that isn't on screen becomes done or
  blocked. Nothing for the visible thread.

### Agent control: MCP and CLI

Agents manage threads and other agents through the server, following t3code's orchestrator MCP.

**MCP delivery:**

- Every ACP session agentZ opens gets an `agentz` stdio MCP server in `session/new` /
  `session/load` `mcpServers`: `agentz-server mcp-bridge`. ACP agents must support stdio MCP.
- The bridge connects to the server's socket. A per-session credential is passed in its
  environment, never on the command line. It is scoped to the machine, the calling thread and
  the session, and revoked when the session closes.
- So the server always knows *which thread* is calling.

**CLI** (`agentz`, also the app binary's subcommands or a small separate binary):

- herdr- and t3code-style commands with `--json` output, for agents running in terminals,
  scripts and agents without MCP.
- t3code's equivalent is `acp-mcp-call <tool> <json>`.
- Terminals started by agentZ get `AGENTZ_SOCKET` and `AGENTZ_THREAD_ID` in their environment,
  as herdr does with `HERDR_*`.

**Tools.** The names and behavior follow t3code; the `agentz_` prefix replaces `t3_`.

| Tool | What it does |
|---|---|
| `orchestrator_capabilities` | The caller's agent and model; installed agents on each machine with their models, modes and options, login state, and whether each can run subthreads; machines and their status. |
| `delegate_task` | Start a subthread (see below). `mode: async` or `wait`, optional agent, model, title, role, machine, timeout. |
| `task_status`, `task_cancel` | Poll or cancel a subthread the caller started. |
| `agentz_thread_list` | Threads in the caller's project, newest first, with filters (title, state, archived, include subthreads). Paginated. |
| `agentz_thread_read` | A thread's state and timeline: `messages` view or `activity` view (tool calls summarized), incremental with `afterPosition`, bounded text. |
| `agentz_thread_launch` / `create_threads` | Start one or several ordinary threads (agent, model, machine, prompt). |
| `agentz_thread_send` | Message an ordinary thread: `auto` / `queue` / `restart`. ACP has no steering, so `restart` is cancel-and-resend, as in t3code. |
| `agentz_thread_wait`, `agentz_thread_interrupt` | Wait for a thread's current turn to finish, or cancel it. |
| `agentz_thread_update` | Rename. |
| `agentz_thread_organize` | Archive or unarchive. Pin, snooze and the rest come only if those features exist. |
| `agentz_thread_diff` | A thread's changes (see Diffs). |
| `agentz_terminal_*` | Start a terminal, send input, read the screen (herdr's `agent read` / `send`). |

**Policy** (t3code):

- Management is limited to the caller's project. Merged projects span machines.
- A subthread or target may not get broader permissions than the caller.
- Agents can't delete threads or answer other threads' permission requests. Those stay with the
  user.
- Requests carry an optional `clientRequestId`, so retries don't create duplicate work.
- Threads and messages created through MCP are marked `createdBy: agent`, so the UI can tell them
  from the user's.

### Subthreads

t3code's delegated tasks, owned by agentZ rather than any one agent:

- `delegate_task` creates a child thread with lineage `subagent` → parent, and sends it the task
  prompt only. The parent's history is not copied.
- The agent, model and permissions are inherited unless given, and never broader.
- Subthreads can be nested. The result returned to the parent is the child's last agent message,
  or its error.
- Finalizing is driven by the child's events, so it also completes after a server restart.

**What the user sees** (t3code):

- Subthreads don't crowd the sidebar. The parent's card and thread view show them in an **Agents**
  control, with each one's state, title and agent.
- Any subthread can be opened and watched like a normal thread. It's read-only for messages;
  message the parent instead.
- When a subthread needs a permission, the parent becomes blocked and shows the request.
  Answering it there answers the child.

### Diffs

t3code's checkpoints:

- Before each turn, the server snapshots the working tree as a hidden git ref
  (`refs/agentz/checkpoints/…`). Snapshots never add commits to the user's branch.
- The diff of a turn is the difference between its start and end checkpoints. The thread's diff
  is from its first checkpoint to now.
- **Diff panel** next to the thread: **This turn** or **All changes**, files with hunks,
  and marking files as viewed (t3code).
- The rendering reuses the existing tool-call diff styling.
- Rolling back files to a checkpoint comes later, if wanted. ACP agents can't rewind their own
  conversation, so t3code starts a fresh session after a rollback.
- Projects that aren't git repositories get no checkpoints, and the panel says so.

### Worktrees and pastures

t3code's workspace model, with herdr's folder layout and removal, plus cow's copy-on-write
pastures.

**A thread runs in a workspace**, one of:

- the project's own checkout;
- a **worktree**: a `git worktree` on its own branch, sharing the project's `.git`;
- a **pasture** (cow's name): an instant copy-on-write clone of the whole project folder,
  `.git` included, on its own branch.

Details:

- The thread stores its workspace kind, path and branch. Its ACP session's `cwd` is the
  workspace.
- Workspaces stay part of their project and are never separate projects. Pastures keep the same
  remotes, so t3code's grouping treats them as the same repository.
- Projects that aren't git repositories only have their own checkout.

**Why pastures** (cow's README):

- A worktree checks out tracked files only. It has no `node_modules`, `.env` or build caches, so
  every new one needs an install and a build.
- A pasture is made with APFS `clonefile(2)` in one syscall. A 2 GB repository copies in about
  130 ms, and only modified blocks use disk. Dependencies, `.env` and caches are there
  immediately.

**Choosing it.** New Thread gets a last step, **Workspace** (t3code's workspace menu):

- **Current checkout**, the default;
- **New pasture** or **New worktree**, from a base branch (the checkout's current branch unless
  changed);
- an existing worktree or pasture of this project.

A thread's menu also offers **New thread in this workspace**.

**Creating a worktree** happens on the thread's machine, by its server:

- `git worktree add -b <branch> <folder> <base>`.
- The folder is `<data dir>/worktrees/<repo>/<branch>` (t3code and herdr).
- The branch is `agentz/<short id>`, following t3code's default static prefix (`t3code/`).
  t3code can also have a model name the branch; agentZ has no app-owned text generation, so
  that's left out.
- Submodules are initialized recursively, as in t3code.
- t3code's per-project setup scripts (`t3.json`) are not planned.

**Creating a pasture** ports cow's `create` (MIT, `references/cow/src/commands/create.rs`):

1. **Clone the folder** to `<data dir>/pastures/<repo>/<branch>`.
   - On macOS, `clonefile(2)` on the whole folder.
   - cow skips build-output folders (`target`, `.build`, `DerivedData`, `.turbo`) by cloning
     around them. Their copies would go stale as soon as the source rebuilt.
   - On Linux, as cow does:
     - `cp --reflink=always -R`, a real copy-on-write clone on btrfs or xfs;
     - otherwise (ext4, for example) a full `cp -R` with a warning. The pasture works the same,
       with dependencies and `.env` ready, but it costs the full disk space and copy time.
       New Thread says so next to **New pasture** on such machines.
     - cow skips the build-output folders and large-folder symlinks only on macOS. On Linux it
       copies everything.
2. **Fix git** in the pasture:
   - delete the `.git/worktrees` entries inherited from the source;
   - set `checkout.guess false`;
   - check out the branch, or create it from the base.
3. **Clean up runtime files:**
   - remove `*.pid`, `*.sock` and `*.socket`;
   - honor a repository's `.cow.json` `post_clone` (`remove` patterns, then `run` commands), so
     repositories set up for cow work the same.
4. **Undo on failure:** if any step fails, remove the partial pasture.

Left out of cow, at least at first:

- **Symlinking large dependency folders** (`node_modules`, `vendor`, …) instead of cloning them,
  and its `materialise` undo. cow does it to save time on huge trees, and its own docs note it
  breaks some bundlers, like Turbopack.
- **jj support.**
- **The AGENTS.md / CLAUDE.md orientation files** cow writes into each pasture. agentZ's
  `agentz_workspace_status` tool gives agents that context instead.

**Bringing work back from a pasture.** A pasture has its own `.git`, so its commits must be moved
explicitly. These are cow's `sync` and `extract`, offered in the thread's menu and as tools:

- **Sync from project:** fetch a branch from the project's checkout through a temporary remote,
  then rebase onto it (or merge). On conflicts, abort the rebase and report the conflicted
  files.
- **Bring branch to project:** create the pasture's branch in the project's checkout at the
  pasture's `HEAD`, ready to review and push from there.

Worktrees share `.git` with the project, so their branches are already there.

**Showing it:**

- Thread cards show the thread's own branch: the workspace's branch, or the checkout's. This
  replaces "only the project's current branch" from the backlog.
- The details popover shows the workspace kind and folder.
- **Project Settings › Checkouts** lists the project's worktrees and pastures.

**Removing.** Deleting or archiving a thread never deletes its workspace.

- Remove a workspace from Project Settings › Checkouts.
  - A worktree uses `git worktree remove`. If git refuses because of changed or untracked files,
    agentZ asks again before forcing it.
  - A pasture warns about uncommitted changes and unpushed commits, then deletes the folder (cow's
    `remove`).
- Branches are kept (herdr).
- A workspace in use by a running thread or terminal can't be removed.
- Automatic cleanup policies come later, if wanted. Examples: t3code's inactive-days and
  merged rules, and cow's `gc` for branches already pushed or merged.

**Agent control** (t3code's tools, renamed from worktree to workspace because of pastures):

- `agentz_workspace_status`: the thread's workspace kind, folder, branch, and the project root.
- `agentz_workspace_list`: the repository's branches and their worktrees and pastures.
- `agentz_workspace_handoff`: move the calling thread into a new worktree or pasture. With an
  optional `continuationPrompt`, the next turn starts there.
  - The ACP session has to be reopened in the new `cwd`. agentZ uses `session/load` with the new
    `cwd` when the agent supports it, otherwise a new session.
- `agentz_workspace_sync` and `agentz_workspace_bring_back`: cow's `sync` and `extract
  --branch`, for pastures.
- `agentz_thread_launch` and `delegate_task` take a `workspaceStrategy`, as in t3code:
  - `root`, the default;
  - `worktree` or `pasture`, with `baseRef`;
  - `existing`, with a path.

  So an agent can fan work out to subthreads, each in its own pasture or worktree. cow's MCP has
  the same tools for one agent at a time.

**Elsewhere:**

- **Diffs:** checkpoints work the same in a worktree or pasture. Restoring files is only offered
  for a thread in its own workspace, and refused when another thread or terminal uses that
  folder (t3code).
- **Terminals:** the thread terminal drawer opens in the thread's workspace.

### Terminals

- **Server side:** a GPUI-free port of the model in Zed's `terminal` crate:
  - the PTY and `alacritty_terminal`'s `Term` and event loop;
  - no settings, tasks, workspace or GPUI entity.

  Zed's crate wraps these in a GPUI entity; the server uses them directly.
  - The server keeps the terminal running, and keeps its screen and scrollback, while nobody is
    watching.
  - t3code keeps 5,000 lines and 8 MiB.
- **Client side:** a port of Zed's `terminal_element.rs`, without the editor and workspace
  dependencies.
  - It renders a terminal content snapshot (cells, cursor, mode, selection) received from the
    server.
  - It sends keystrokes, paste, resize, scroll and selection back.
  - Key-to-bytes mapping uses Zed's `mappings/`. It runs in the app, since it needs GPUI
    keystrokes and the terminal mode from the snapshot.
- **Streaming:** only for terminals a client is viewing (herdr's "surface interest"), throttled
  to frame rate. Background terminals still parse output, for detection.
- **Where terminals appear:**
  - **Terminal threads** (herdr panes): New Thread offers **Terminal** next to the ACP agents.
    It runs a login shell, or an agent CLI found on that machine's `PATH` (claude, codex,
    opencode, gemini, …, from herdr's manifest list). One terminal per thread first; splits and
    tabs come later, if wanted.
  - **Thread terminal drawer** (t3code): a terminal under an ACP thread, in its project folder.
  - **ACP client terminals:** advertise ACP's `terminal` client capability, so agents that run
    commands through the client (`terminal/create`, `terminal/output`, …) use agentZ
    terminals. Their tool calls then show live terminals the user can open, as t3code does for
    Devin.
- **Detection:** herdr's manifests, applied to the bottom of the screen buffer, never the
  scrolled viewport. They are evidence-based AND/OR rules per agent.

### Machines

- **Saved profiles (client side):** id, label, SSH target (`host`, alias, or
  `ssh://user@host:port`), enabled. No secrets are stored; authentication stays with OpenSSH
  (herdr).
- **Connecting:**
  - `ssh -o BatchMode=yes` with a shared `ControlMaster`/`ControlPersist 600` (herdr).
  - States: **Online**, **Reconnecting** (backoff up to 2 minutes), **Attention** (auth or host
    key, showing the error and the command to run in a terminal).
  - One machine failing never affects another.
- **Install and update:**
  - Detect the remote OS and CPU with `uname -sm`. Upload the matching server to
    `~/.agentz/server/<version>/agentz-server`.
  - The macOS server comes from the normal build. Linux servers (x86_64/aarch64 musl) come from
    `cargo zigbuild`, as in Zed's dev path, with `-C target-feature=+crt-static` and
    `strip = "symbols"`. That makes about 7 MB, or 3 MB gzipped, which is the form to upload.
    Without stripping, the release profile's debug info makes it about 48 MB.
  - Replacing a running remote server asks first, because it stops that machine's agents.
- **Node for npm agents.** Registry agents distributed through npm need Node, and remote
  machines may not have it (neither of the user's does).
  - The server downloads an official Node.js build into its data directory when an npm agent is
    installed and no suitable Node is on `PATH`. That's Zed's `node_runtime`
    (`references/zed/crates/node_runtime`).
  - `uvx` agents would need the same for `uv`, if any are wanted.
- **Offline:** a disconnected machine's projects and threads stay visible, dimmed, as cached
  data. Input to them is disabled until reconnected (herdr).
- **Adding a project on a remote:** a path field with completion served by that machine. A
  native folder dialog can't browse another machine.

**Merged projects**, exactly as t3code groups them
(`packages/client-runtime/src/state/projectGrouping.ts`,
`apps/server/src/project/RepositoryIdentityResolver.ts`, `normalizeGitRemoteUrl` in
`packages/shared/src/git.ts`):

- **Repository identity** is resolved by each server for every project:
  1. `git -C <path> rev-parse --show-toplevel` gives the repository root.
  2. `git remote -v` lists the fetch remotes. The primary remote is `upstream`, then `origin`,
     then the first by name.
  3. Its URL is normalized into a **canonical key**: lowercased, without a trailing `/` or
     `.git`, and turned into `host/owner/repo` for both `https://` and `git@host:owner/repo`
     forms. Azure DevOps URLs get a special case.
  4. The **display name** is the key without the host (`owner/repo`).

  Results are cached for 15 minutes, and for 1 minute when there's no repository or remote.
- **Grouping key.** Projects with the same key are one project in the projects pane. This works
  across machines *and* across several checkouts on one machine (clones, worktrees). The mode
  decides the key:
  - **`repository`** (the default): the canonical key alone. Every checkout of the repository
    merges, whichever subfolder was added.
  - **`repository_path`**: the canonical key plus the project's path inside the repository, so
    different subfolders of a monorepo stay apart.
  - **`separate`**: never merge. The key is machine plus path.
  - Projects with no remote are never merged.
- **No branch or commit check.** t3code merges checkouts on different branches. Each checkout
  keeps its own branch, shown per machine.
- **Label.** If every member has the same name and it isn't just the repository's name, that
  name is used. Otherwise it's the display name (`owner/repo`), then the repository name.
- **The members stay real.** Threads are still created in one physical project (a machine plus a
  path). The merged entry lists them and combines their threads. It shows which machines it's
  on (`ProjectEnvironmentBadge`, only when projects span machines).
- **Settings:**
  - **Settings › General** has a switch, "Combine matching repositories across environments".
    It toggles between the last-used mode and `separate`.
  - The project's sidebar menu can override the mode for that project.
  - Project settings (name, icon) apply to every checkout in the group.

**New Thread** picks the machine too:

1. **Project.** Merged projects appear once.
2. **Machine.** Only shown when the project is on more than one machine. The default is the
   machine last used for that project. Offline machines are listed but disabled.
3. **Agent.** Agents installed on that machine.
4. **Workspace.** Current checkout, new pasture, new worktree, or an existing one (see Worktrees and
   pastures).

**UI** (t3code):

- With only Local, nothing changes.
- Remote threads get a machine icon on their card. The details popover gets its machine line
  back.
- The project switcher groups projects by machine, except merged ones.
- Settings gains a **Machines** page. Each machine's agents (install, log in, defaults) are
  managed from its own Settings.

### Workspaces view (herdr's layout)

The user asked for this on 2026-10-03. The app gets two views, chosen by tabs in the title bar:
**Agents | Workspaces**.

**Agents** is the current app: the t3code sidebar, with one thread **full screen** in the main
area. Every agent shows up here:

- ACP threads;
- terminal threads, including terminal agents like `claude`;
- agents that are open in a workspace pane.

**Workspaces** is herdr's model (`references/herdr`: `concepts.mdx`, `keyboard.mdx`,
`configuration.mdx` § Sidebar row layouts, `session-state.mdx`, `src/layout.rs`,
`src/workspace/`):

- **Sidebar, top: workspaces.** Search, and a **+** button. Rows follow herdr's space rows:
  rolled-up state icon and name, then branch and ahead/behind.
- **Sidebar, bottom: agents.** Every agent, in the slim one-line style of the Archived shelf.
  Rows follow herdr's agent rows: state icon, machine (only with several machines), workspace,
  tab, agent. Clicking one focuses its pane.
- **The + picker** creates a workspace rooted at one of:
  - a project (one of its checkouts, worktrees or pastures, on any machine);
  - a machine's home folder (`~` on Local, `t3-home`, `devbox1`, …).
  - It opens with one terminal pane there.
- **Each workspace has tabs**, shown in a tab bar above the panes.
- **Each tab is a tree of split panes**, herdr's `TileLayout`:
  - split right or down;
  - drag borders to resize;
  - zoom, swap, close.
- **A pane holds a terminal or an agent:**
  - a shell;
  - an agent CLI like `claude` (a terminal thread);
  - an ACP agent thread, new or existing.

  An ACP thread in a pane is the same thread as in Agents, shown at the pane's size there and
  full screen there. Any size or shape works in a pane.
- **Mouse first**, as herdr is: click to focus, drag borders, right-click menus. Keyboard
  bindings cover herdr's actions (new tab, split right/down, move between panes, zoom, close,
  next/previous tab, new workspace, goto picker). As a GUI app, agentZ uses direct chords
  instead of herdr's terminal prefix. See the open question on which chords.

**Where the state lives** follows herdr's runtime/client rule: workspaces, tabs and the pane
tree are shared session state.

- They live in the server of the workspace's machine, are saved, and come back after a server
  restart. herdr's "snapshot restore": terminal panes come back as new shells in their saved
  folders, and agent threads reattach.
- A workspace belongs to one machine, the one its folder is on.
- Which tab is shown, sidebar sizes and the focused pane are client-only presentation state.

**Sizes.** A terminal open both in a pane and full screen in Agents follows the view the user
last interacted with. That's herdr's rule for one terminal viewed by several clients.

**Building blocks:**

- The split tree, resizing and borders are a port of herdr's `TileLayout`, or Zed's
  `workspace::pane_group` (GPL, so it's fine in the app).
- Panes reuse phase 7's terminal view and the thread view.
- A pane thread view is the existing `agent_view` with a compact header.

## Phases

Each phase ships on its own, keeps the app working, and is committed.

0. **Spike.** Prove the risky parts before building on them:
   - a GPUI-free binary with the server's real dependencies (tokio, `agent-client-protocol`,
     `http_client`/`reqwest_client`, `alacritty_terminal`) cross-compiles to Linux musl via
     zigbuild, and runs on `devbox1`;
   - `alacritty_terminal` builds here;
   - an ACP agent (the mock) accepts a stdio MCP server in `session/new`;
   - measure disk use.
1. **Local server split.** Two steps, with the app working after each:
   1. **GPUI-free core.** Rewrite `agent_thread`, `projects` and `registry` as plain Rust on
      tokio. In the meantime, the app wraps them in thin GPUI entities in-process. Their tests
      become plain async tests against the mock agent.
   2. **Server.** Add the `agentz_protocol` and `agentz_server` crates, and move the stores and
      threads into the server. In the app, the threads become client copies with the same API
      as `AgentThread`, so `agent_view` changes little. Reattach: a snapshot, then live events.

   Result: the same app, but threads survive quitting it.
2. **Attention states and notifications** for ACP threads.
3. **Agent control:** the MCP bridge injected into sessions, the CLI, and the tools above except
   subthreads, diffs and terminals.
4. **Subthreads:**
   - `delegate_task`, `task_status`, `task_cancel`;
   - lineage and finalization;
   - the Agents control;
   - permissions forwarded to the parent.
5. **Diffs:** checkpoints, the diff panel, `agentz_thread_diff`.
6. **Worktrees and pastures:**
   - the thread workspace;
   - the Workspace step in New Thread;
   - creating and removing worktrees and pastures;
   - syncing a pasture, and bringing its branch back;
   - branches on cards;
   - Checkouts in Project Settings;
   - the workspace tools.
7. **Terminals:** server terminals, the client terminal element, terminal threads, the thread
   terminal drawer, ACP client terminals, `agentz_terminal_*`.
8. **Terminal agent detection** using herdr's manifests.
9. **Machines over SSH:**
   - profiles and Settings › Machines;
   - install/upload and `proxy`;
   - reconnecting and Attention;
   - remote projects and per-machine agents;
   - merged projects;
   - the machine step in New Thread;
   - machine icons.
10. **Polish:**
   - optional start at login (launchd/systemd user service);
   - confirmed remote server updates;
   - per-machine "Stop server".

11. **Workspaces view:**
    - the Agents | Workspaces tabs;
    - server-side workspaces, tabs and pane trees, saved and restored;
    - the workspaces sidebar with search, **+** and the agents list;
    - split panes holding terminals and agent threads;
    - mouse and keyboard actions.

Everything after phase 1 depends on it. Phase 4 needs 3. Phase 8 needs 7. The workspace tools
need phase 3. Otherwise phases 3–8 and 9 can go in any order.

## Testing

- **Server and protocol:** in-process tests that drive a server over a socket pair with the mock
  agent (`crates/agent_thread/test_support/mock_agent.py`).
- **Agent control:**
  - Extend the mock agent so a scripted prompt makes it call MCP tools (it is an MCP client
    through the bridge).
  - Test delegation, wait, cancel, archive and policy denials end to end, as t3code's
    integration test does.
- **Diffs:** temporary git repositories with scripted edits between turns.
- **Worktrees and pastures:** temporary repositories:
  - create, reuse and remove worktrees and pastures;
  - excluded build folders;
  - `.cow.json`;
  - sync with and without conflicts;
  - bringing a branch back;
  - refused removals;
  - a handoff with the mock agent.
- Pasture tests need APFS, which the temp directory on this Mac is. cow's tests are macOS-only for
  the same reason.
- **Proxy and reconnect:** run `agentz-server proxy` directly as the transport, with no SSH. Kill
  it to simulate a dropped connection.
- **Terminals:** drive a PTY running `sh` with scripted input, and check the screen snapshots.
- **Merged projects:** two servers in one test, with clones of the same repository.
- **Real SSH:** the user's machines `t3-home` and `devbox1`, from `~/.ssh/config`. They're both
  reachable with keys, so `BatchMode` works.
  - Install only under `~/.agentz`.
  - Don't touch the t3code and herdr installs already on them (`~/.t3`, `~/.herdr`).
  - Never prompt the user's real agents there either.
- **UI:** headless GPUI tests and PopUp screenshots, as described in `AGENTS.md`.

## Risks

- **The GPUI-free rewrite of the core** is the first part of phase 1. It's plumbing, not logic,
  but it touches every event and async call in about 4,000 lines. Keep the mock-agent tests
  passing throughout.
- **Phase 1 touches almost everything.** Thread state crossing a socket, reattach snapshots, and
  keeping `agent_view` unchanged. Keep tests green at every step.
- **MCP support varies by agent.** Agents that ignore `mcpServers` get the CLI fallback through
  their shell tool. `orchestrator_capabilities` should say which agents can run subthreads
  (t3code).
- **Agents managing agents can spend the user's usage quickly.** Subthreads only inherit
  narrower permissions, and every agent-created thread is visible and marked.
- **Disk:** Linux cross-builds add another target directory (several GB). Check `df -h ~` first.
- **SSH authentication can't prompt in the background.** Show the error and the command. Later,
  agentZ could run that `ssh` in a terminal pane for the user.
- **macOS privacy prompts** (Desktop, Documents) now attach to `agentz-server`, not the app.
- **PATH:** agents must see the same `PATH` as now. Locally, the server inherits the app's
  environment. Remotely, it runs under a login shell (`sh -lc`).
