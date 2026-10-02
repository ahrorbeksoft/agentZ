# Plan: background servers, agent control, subthreads, diffs, terminals and machines

The goal is a herdr-like experience in agentZ:

- Agents keep working after the app quits, and the app reattaches to them.
- The sidebar shows which threads need you.
- Agents can manage other agents (threads, subthreads, archive, …) through MCP or a CLI.
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
| Remote OS in the first version | Macs and Linux (x86_64 and aarch64). |
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

**The server is a headless GPUI app**, as Zed's `remote_server` is, using
`gpui_platform::headless()`. `agent_thread`, `projects` and `registry` move into it with few
changes. Headless GPUI builds for Linux without wayland or x11.

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
- "Stop server" (Settings) ends it and its agents and terminals.
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

### Terminals

- **Server side:** a port of Zed's `terminal` crate (PTY plus `alacritty_terminal`), without
  settings, tasks or workspace.
  - The server keeps the terminal running, and keeps its screen and scrollback, while nobody is
    watching.
  - t3code keeps 5,000 lines and 8 MiB.
- **Client side:** a port of Zed's `terminal_element.rs`, without the editor and workspace
  dependencies.
  - It renders a terminal content snapshot (cells, cursor, mode, selection) received from the
    server.
  - It sends keystrokes, paste, resize, scroll and selection back.
  - Key-to-bytes mapping uses Zed's `mappings/`.
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
    `cargo zigbuild`, as in Zed's dev path.
  - Replacing a running remote server asks first, because it stops that machine's agents.
- **Offline:** a disconnected machine's projects and threads stay visible, dimmed, as cached
  data. Input to them is disabled until reconnected (herdr).
- **Adding a project on a remote:** a path field with completion served by that machine. A
  native folder dialog can't browse another machine.

**Merged projects** (t3code's repository grouping):

- Each server reports a project's **repository identity**: its normalized `origin` URL (for
  example `github.com/owner/repo`) and the project's path inside the repository.
- Projects with the same identity on different machines become one entry in the projects pane,
  named by the full repository name, `owner/repo`.
  - The entry shows which machines it's on (t3code's `ProjectEnvironmentBadge`).
  - Its thread list combines threads from every machine.
- Projects without an `origin` remote are never merged.
- Each machine's checkout keeps its own branch, which is shown per machine.
- Project settings (name, icon) apply to the merged project.

**New Thread** picks the machine too:

1. **Project.** Merged projects appear once.
2. **Machine.** Only shown when the project is on more than one machine. The default is the
   machine last used for that project. Offline machines are listed but disabled.
3. **Agent.** Agents installed on that machine.

**UI** (t3code):

- With only Local, nothing changes.
- Remote threads get a machine icon on their card. The details popover gets its machine line
  back.
- The project switcher groups projects by machine, except merged ones.
- Settings gains a **Machines** page. Each machine's agents (install, log in, defaults) are
  managed from its own Settings.

## Phases

Each phase ships on its own, keeps the app working, and is committed.

0. **Spike.** Prove the risky parts before building on them:
   - headless GPUI cross-compiles to Linux musl via zigbuild;
   - `alacritty_terminal` builds here;
   - an ACP agent (the mock) accepts a stdio MCP server in `session/new`;
   - measure disk use.
1. **Local server split.** Add the `agentz_protocol` and `agentz_server` crates.
   - Move the stores and threads into the server.
   - In the app, the threads become client copies with the same API as `AgentThread`, so
     `agent_view` changes little.
   - Reattach: a snapshot, then live events.

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
6. **Terminals:** server terminals, the client terminal element, terminal threads, the thread
   terminal drawer, ACP client terminals, `agentz_terminal_*`.
7. **Terminal agent detection** using herdr's manifests.
8. **Machines over SSH:**
   - profiles and Settings › Machines;
   - install/upload and `proxy`;
   - reconnecting and Attention;
   - remote projects and per-machine agents;
   - merged projects;
   - the machine step in New Thread;
   - machine icons.
9. **Polish:**
   - optional start at login (launchd/systemd user service);
   - confirmed remote server updates;
   - per-machine "Stop server".

Everything after phase 1 depends on it. Phase 4 needs 3. Phase 7 needs 6. Phases 3–7 and 8 can
otherwise go in any order.

## Testing

- **Server and protocol:** in-process tests that drive a server over a socket pair with the mock
  agent (`crates/agent_thread/test_support/mock_agent.py`).
- **Agent control:**
  - Extend the mock agent so a scripted prompt makes it call MCP tools (it is an MCP client
    through the bridge).
  - Test delegation, wait, cancel, archive and policy denials end to end, as t3code's
    integration test does.
- **Diffs:** temporary git repositories with scripted edits between turns.
- **Proxy and reconnect:** run `agentz-server proxy` directly as the transport, with no SSH. Kill
  it to simulate a dropped connection.
- **Terminals:** drive a PTY running `sh` with scripted input, and check the screen snapshots.
- **Merged projects:** two servers in one test, with clones of the same repository.
- **Real SSH:** needs a target from the user, for example a Linux VM, another Mac, or
  `ssh localhost` with Remote Login enabled. Ask before relying on one.
- **UI:** headless GPUI tests and PopUp screenshots, as described in `AGENTS.md`.

## Risks

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
