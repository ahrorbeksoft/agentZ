# Progress

Tracks [plan.md](plan.md). When you finish a step:

- tick it;
- add a line to the log with the date and commit;
- update **Next**.

Note anything that changed the plan under **Findings**, and update the plan itself.

**Next:** Phase 9: repository identity and merged projects.

| Phase | Status |
|---|---|
| 0. Spike | Done |
| 1. Local server split | Done |
| 2. Attention states and notifications | Done |
| 3. Agent control (MCP and CLI) | Done |
| 4. Subthreads | Done |
| 5. Diffs | Done |
| 6. Worktrees and pastures | Done |
| 7. Terminals | Done |
| 8. Terminal agent detection | Done |
| 9. Machines over SSH | Done |
| 10. Polish | In progress |
| 11. Workspaces view | Not started |

## 0. Spike

- [x] Check `df -h ~`. Install `zig` and `cargo-zigbuild`, and add the
      `x86_64-unknown-linux-musl` target. That's what both of the user's machines need;
      `aarch64-unknown-linux-musl` can wait.
- [x] Cross-build a minimal GPUI-free binary for Linux musl with the server's real dependencies:
      tokio, `agent-client-protocol`, `http_client`/`reqwest_client` (the C code in `ring`,
      `aws-lc-sys` and `zstd-sys`), and `alacritty_terminal`. Note the size and any crates that
      fail.
- [x] Build `alacritty_terminal` (Zed's pinned version) in the workspace. Spawn a PTY running
      `sh` and read the screen.
- [x] Have the mock agent receive a stdio MCP server in `session/new`, start it, and call a tool.
- [x] Copy the Linux binary to `devbox1` under `~/.agentz/spike/`, check that it runs (for
      example, an HTTPS request and spawning a process), then remove it.
- [x] Record the disk cost of the Linux target directories.
- [x] Decide on anything the spike changes, and update the plan.

The spike's code is in commit 8430863 (`crates/spike_server`, removed in the next commit). Read
it with `git show 8430863:crates/spike_server/src/main.rs`. It has a tokio ACP client, a PTY
read through `alacritty_terminal` without GPUI, and a minimal stdio MCP server.

## 1. Local server split

GPUI-free core first, with the app working throughout:

- [x] `projects`: plain structs, no `Global`/`Context`.
- [x] `registry`: tokio tasks and `tokio::process`, no GPUI.
- [x] `agent_thread`: tokio tasks, background results on a channel, events queued for the owner.
- [x] App: thin GPUI entities wrapping the core in-process, so the UI works as before.
- [x] Tests: plain async tests against the mock agent. All current tests still pass.

Then the server:

- [x] `crates/agentz_protocol`:
  - [x] framing and handshake (versions, machine id, OS/arch, capabilities);
  - [x] request/response and subscription messages;
  - [x] serde types for projects, threads, entries, tool calls, permissions, config options and
        states.
- [x] `crates/agentz_server`, a GPUI-free tokio binary:
  - [x] socket listener in the data directory, with a pid file;
  - [x] owns `ProjectStore`, `AgentRegistryStore`, `AppSettings` agent settings, and the
        `AgentThread`s;
  - [x] `proxy` subcommand that starts the server if needed.
- [x] A thread-management service shared by every transport: the app, MCP and CLI (t3code's
      `ThreadManagementService`).
- [x] Session subscription: projects, threads, states, as a snapshot and then events.
- [x] Thread-detail subscription: entries, plan, permissions, config options, usage, as a
      snapshot and then events.
- [x] Requests:
  - [x] threads: create, prompt, cancel, permission answer, config/mode changes, rename,
        archive, unarchive, delete;
  - [x] agents: login, logout, reload;
  - [x] the registry: install, update, uninstall.
- [x] App: a client connection that starts the local server detached if it isn't running, and
      reconnects.
- [x] App: client-side thread copies with `AgentThread`'s API. Port `agent_view`, `sidebar`,
      `settings_page` and `shell` to them.
- [x] Reattach after an app restart mid-prompt: the turn continues, and the view catches up.
- [x] Settings: "Stop server" (built as "Restart Server"; see Findings).
- [x] Tests: protocol round-trips, server driven over a socket pair with the mock agent, and
      reattach.
- [x] Update `AGENTS.md`: layout, how to run and test the server.

## 2. Attention states and notifications

- [x] Server: thread state (working / blocked / done / idle) and events.
- [x] Client: track which completions this client has viewed. "Done" until the thread is viewed.
- [x] Sidebar cards and the project switcher show states, rolled up to the project.
- [x] macOS notifications for done or blocked threads that aren't on screen.

## 3. Agent control (MCP and CLI)

- [x] `agentz-server mcp-bridge`: a stdio MCP server that talks to the server socket, with a
      per-session credential in its environment.
- [x] Inject it into every ACP `session/new` and `session/load`. Revoke the credential when the
      session closes.
- [x] Tools:
  - [x] `orchestrator_capabilities`: agents, models, modes, machines. Login state is left out:
        ACP can't report it without starting the agent;
  - [x] `agentz_thread_list` and `agentz_thread_read` (messages and activity views,
        incremental);
  - [x] `agentz_thread_launch` and `create_threads`;
  - [x] `agentz_thread_send` (auto/queue/restart), `agentz_thread_wait`,
        `agentz_thread_interrupt`;
  - [x] `agentz_thread_update` (rename) and `agentz_thread_organize` (archive, unarchive).
- [x] Policy:
  - [x] project scope;
  - [x] no permission escalation: agentZ has no permission modes yet, so launched threads get
        the agent's own defaults;
  - [x] no delete, and no answering permissions;
  - [x] `clientRequestId` idempotency;
  - [x] typed failures.
- [x] Threads and messages created by agents are marked `createdBy: agent`, and the UI shows it.
- [x] CLI: `agentz-server call <tool> [json]` (JSON output, exit 1 on a failure) and
      `agentz-server tools`. Agents get `AGENTZ_SOCKET`, `AGENTZ_THREAD_ID` and
      `AGENTZ_BIN_PATH` in their environment. agentZ terminals get them in phase 7.
- [x] Mock agent: scripted MCP tool calls. End-to-end tests.
- [x] Document the tools for agents, like t3code's orchestration instructions (the tool
      descriptions and the bridge's MCP instructions).

## 4. Subthreads

- [x] Thread lineage in `projects`: parent, kind `subagent`, created by. (A subthread is a
      thread with a `task`; its parent is the creator.)
- [x] `delegate_task` (async/wait, agent/model/role/title, timeout), `task_status`,
      `task_cancel`. The machine argument waits for phase 9.
- [x] Finalization from the child's events, idempotent and surviving restarts. The result is the
      last agent message, or the error.
- [x] The child gets the task prompt only; permissions are inherited and never broader.
- [x] UI:
  - [x] the Agents control on the parent (card and thread view);
  - [x] opening a subthread read-only;
  - [x] subthreads hidden from the main list.
- [x] A subthread's permission requests show on the parent, which becomes blocked.
- [x] Tests: delegation, wait, cancel, nesting, restart during a subthread.

## 5. Diffs

- [x] Server: checkpoints as hidden git refs before and after each turn. Skip projects that
      aren't git repositories.
- [x] Turn diff and thread diff queries. `agentz_thread_diff`.
- [x] Diff panel: This turn / All changes, files and hunks, mark as viewed.
- [x] Tests with temporary repositories.

## 6. Worktrees and pastures

- [x] Thread workspace in `projects`: kind (checkout, worktree, pasture), path and branch. The ACP
      session's `cwd` follows it.
- [x] Server, worktrees:
  - [x] `git worktree add -b agentz/<id> <data>/worktrees/<repo>/<branch> <base>`;
  - [x] recursive submodules.
- [x] Server, pastures (port of cow's `create`):
  - [x] `clonefile(2)` on macOS, skipping `target`, `.build`, `DerivedData`, `.turbo`;
  - [x] on Linux, `cp --reflink=always -R`, falling back to a full `cp -R` with a warning
        (cow);
  - [x] git fixes: drop `.git/worktrees`, `checkout.guess false`, branch;
  - [x] cleanup: `*.pid`, `*.sock`, `*.socket`, plus `.cow.json` `post_clone`;
  - [x] roll back on failure.
- [x] Pastures: sync from the project (temporary remote, rebase or merge, abort on conflict) and
      bring the branch back to the project.
- [x] List the project's worktrees and pastures.
- [x] New Thread › Workspace: Current checkout / New pasture / New worktree (base branch) /
      existing. Thread menu: New Thread Here, Sync from Project, Bring Branch to Project.
- [x] Cards show the thread's own branch. The details popover shows the workspace kind and
      folder.
- [x] Project Settings › Checkouts:
  - [x] list worktrees and pastures;
  - [x] remove a worktree with `git worktree remove`, asking again before forcing;
  - [x] remove a pasture, warning about uncommitted or unpushed work;
  - [x] keep branches;
  - [x] refuse while the workspace is in use.
- [x] Tools:
  - [x] `agentz_workspace_status`, `agentz_workspace_list`;
  - [x] `agentz_workspace_handoff`, reopening the session with `session/load` in the new
        `cwd` or a new session;
  - [x] `agentz_workspace_sync`, `agentz_workspace_bring_back`;
  - [x] `workspaceStrategy` (root, worktree, pasture, existing) on `agentz_thread_launch` and
        `delegate_task`.
- [x] Diffs: restoring files only for a thread in its own, unshared workspace (the diff panel's
      Revert, `Request::RestoreCheckpoint`).
- [x] Tests with temporary repositories (APFS for pastures).

## 7. Terminals

- [x] Server: port Zed's `terminal` (PTY plus `alacritty_terminal`) without settings, tasks or
      workspace. Keep 5,000 lines of scrollback. Programs get `TERM=xterm-256color`,
      `TERM_PROGRAM=agentZ` and `AGENTZ_*`; color queries are answered from the client's theme.
- [x] Protocol: terminal frames (lines of styled runs, cursor, modes, selection), only the
      changed lines after the first, streamed only to subscribed clients and at most every 16 ms.
      Input, paste, resize, scroll, selection, focus and the palette go back. Terminals are keyed
      by what owns them (terminal thread, drawer, agent command) and start on demand.
- [x] Zed's key mappings (`mappings/keys.rs`) in `agentz_protocol::terminal_keys`, shared by the
      app and the terminal tools.
- [x] App: port `terminal_element.rs` and `mappings/mouse.rs` without the editor and workspace
      dependencies. The app keeps a copy of each subscribed terminal's frame (`terminal_entity`),
      and the view takes Zed's macOS terminal keymap, IME, mouse reporting, selection, scrolling,
      Copy, Paste, Select All and Clear.
- [x] Terminal threads: thread kind "terminal". New Thread › Terminal: a login shell, or an
      agent CLI found on that machine's `PATH`. The thread view has the terminal's title, its
      exit status and Restart.
- [x] Thread terminal drawer under ACP threads: View › Terminal, `cmd-j` or the toolbar button.
- [x] ACP client `terminal` capability backed by server terminals (Zed's non-interactive shell,
      no pagers, output cut from the start). Tool calls name their terminals.
- [x] Tool calls show their live terminal inline (up to 16 lines), open by default.
- [x] `agentz_terminal_list`, `_start`, `_send` (text, keys, submit), `_read` (recent or
      visible) and `_wait` (for text or exit), after herdr's `pane` commands. The CLI reaches
      them through `agentz-server call`.
- [x] Terminals survive app restarts, keeping their screen and scrollback: the server owns them.
- [x] Tests: scripted `sh` sessions and their screens, terminal threads streamed to watchers, an
      agent's `terminal/*` requests, and the terminal tools.

## 8. Terminal agent detection

- [x] Port herdr's manifest format and matcher, reading the bottom of the screen buffer. Keep
      herdr's Apache-2.0 notice. Remote manifest updates and local overrides are left out.
- [x] Bundle herdr's manifests for the CLIs we offer (all 22, unchanged, with a `NOTICE`).
- [x] Feed the result into the attention states. The agent is the terminal's foreground
      process group leader (herdr's process probe, macOS and Linux), and the state follows
      herdr's loop: startup grace, a working-to-idle hold, six misses before an agent counts as
      gone, and only changed screens reread. Working and blocked show as the thread's own;
      returning to idle, or the agent exiting, completes it. OSC 9 progress isn't captured,
      since `alacritty_terminal` drops it; the title is.

## 9. Machines over SSH

- [x] Client: saved machine profiles and Settings › Machines (add, rename, disable, remove).
- [x] SSH transport (`agentz_client::ssh`, herdr's options and error classification):
  - [x] `BatchMode` with a shared ControlMaster;
  - [x] `uname -sm` detection;
  - [x] upload to `~/.agentz/server/<version>/`, skipped when the SHA-256 matches;
  - [x] run `proxy` (the server loads the login shell's environment itself).
- [x] Build the Linux servers with `tooling/build-remote-servers.sh` (`cargo zigbuild`, musl;
      x86_64 by default, aarch64 on request). The app looks for them next to itself, in
      `../Resources`, `../remote-servers`, or `$AGENTZ_REMOTE_SERVERS`.
- [x] Connection states: Online / Reconnecting (backoff up to 2 minutes) / Attention, with the
      error and the command to run.
- [x] Offline machines stay visible but dimmed, with input disabled.
- [x] Remote projects: Open Folder asks for the machine when there are others (This Mac uses
      the folder picker), then a path field completed from that machine's folders (t3code's
      `filesystem.browse`). Settings › Machines has Add Project… per connected machine.
- [x] Per-machine agents: install, log in, defaults (Settings › Agents has a machine picker).
- [x] Managed Node for npm agents when the machine has none (Zed's `node_runtime`):
      `registry::node_runtime` uses the system Node.js when it's 22 or newer, otherwise
      downloads Node.js v24.11.0 from nodejs.org (checked against `SHASUMS256.txt`) into
      `<data>/node`, with its own npm cache and blank npm configs, as Zed does. Agents run on
      that Node get its `bin` folder first on `PATH`. The official Linux builds need glibc.
      `cargo test -p registry managed_node_downloads_and_runs -- --ignored` downloads it and
      installs a package (16 s here).
- [x] Repository identity from each server, as t3code does (`agentz_server::repositories`,
      sent as `Project::repository`):
  - [x] repository root, then the primary remote (`upstream`, `origin`, first by name);
  - [x] canonical key via `normalizeGitRemoteUrl` (its tests ported);
  - [x] display name `owner/repo`;
  - [x] cached for 15 minutes, or 1 minute when there's no repository or remote (swept every
        minute, and right after a project is added).
- [x] Merged projects (`machines::build_project_groups`, t3code's tests ported):
  - [x] grouping modes `repository` (default), `repository_path`, `separate`;
  - [x] Settings › General switch (back to the last combining mode) with a "Combine by"
        choice, and a per-project override in the project's Repository section;
  - [x] t3code's label rule;
  - [x] machines badge (title bar and switcher, only when a group spans machines), combined
        threads, and a machine tag on cards when one project is shown;
  - [x] branch shown per checkout (each card's checkout line);
  - [x] project name and icon shared by the group (images only by this Mac's checkouts);
  - [x] the chosen project stays chosen when grouping changes.
- [x] New Thread: Project → Machine (only when the project is combined from several
      checkouts, on any machines; defaults to the one with the newest thread; offline ones
      disabled) → Agent on that machine. With one project shown, it opens on the machine step.
- [x] UI: machine icon on remote cards, machine line in the details popover, projects grouped by
      machine in the switcher (combined ones under "On several machines", first).
- [x] Agent control across machines: the thread and task tools take `machine`, and the
      server relays the call through the app, which sends it to that machine's server as the
      calling project (so it works only while the app is open). The app tells each server which
      machines its projects are on (`SetPeers`). `agentz_thread_list` covers every machine of a
      combined project. A task delegated elsewhere is an ordinary thread there, since the
      child's server can't report back to the parent's.
- [x] Ask before replacing a running remote server: the server reports the hash recorded
      beside its binary when it started, the SSH client compares it with the one installed
      now, and an older server shows an icon in the title bar and Restart Server… in
      Settings › Machines.
- [x] Test against `t3-home` and `devbox1`, leaving their t3code and herdr installs alone.
  - [x] Install and reconnect: `AGENTZ_SSH_TEST_TARGET=<host> cargo test -p agentz_server --test
        ssh -- --ignored` (1–4 s; the second run reuses the server and uploads nothing).
        `AGENTZ_SSH_TEST_RESTART=1` replaces an older running server (done on both).
  - [x] npm agents: `AGENTZ_SSH_TEST_INSTALL_AGENT=claude-acp` installed it on `devbox1` in
        5 s, with the Node.js 24 that nvm has put on both machines since the first check
        (found through the login shell). The download path is tested on this Mac.
  - [x] The app: both connect and show in Settings › Machines with their OS and server
        version; projects added there list and complete their folders.

## 10. Polish

- [x] A macOS app bundle (Info.plist, bundle id, ad-hoc signature) so system notifications show,
      and launching with `open -g` doesn't take focus: `tooling/bundle-mac.sh [--debug]` makes
      `target/bundle/agentZ.app` (`dev.agentz.agentZ`), with the server beside the app and the
      Linux servers in `Resources`. A bundled app leaves activation to Launch Services. Checked
      with `open -g`: the front app stayed in front, and notifications were enabled. No app
      icon yet.
- [x] Optional start at login: Settings › General › Start at login writes a launch agent
      (`~/Library/LaunchAgents/dev.agentz.server.plist`) that runs `agentz-server start` at
      load, and nothing more, so a stopped server stays stopped. The app points it at its
      current server when it opens. Checked with `launchctl bootstrap` in a scratch data
      directory. Not on Linux: remote servers start when the app connects, and a systemd user
      service would end at logout unless lingering is on, while today's server outlives it.
- [x] Remote server updates, only after the user confirms: done in phase 9 (an older server
      keeps running, with Restart Server… in Settings › Machines).
- [x] Per-machine "Stop Server…" in Settings › Machines, This Mac included. The machine shows
      "Server stopped" (and the title bar's disconnected icon) and isn't reconnected, since
      that would start the server again, until Start Server, or the app's next launch.

## 11. Workspaces view

- [ ] Title bar tabs: **Agents** (the current app, a thread full screen) | **Workspaces**.
- [ ] Server: workspaces (machine plus folder), tabs, and pane trees (herdr's `TileLayout`):
  - [ ] saved;
  - [ ] restored after a restart, terminals as new shells in their folders, threads
        reattached;
  - [ ] streamed to clients.
- [ ] Workspaces sidebar:
  - [ ] search;
  - [ ] **+** picker: a project checkout, worktree or pasture, or a machine's home folder;
  - [ ] rows with rolled-up state, name, branch and ahead/behind.
- [ ] Agents list at the bottom of the sidebar:
  - [ ] every agent on every machine;
  - [ ] slim rows like the Archived shelf;
  - [ ] state, machine, workspace, tab, agent;
  - [ ] clicking a row focuses its pane, or opens it in Agents when it isn't in one.
- [ ] Tabs: new, rename, close, reorder, next/previous.
- [ ] Panes:
  - [ ] split right/down;
  - [ ] drag to resize;
  - [ ] zoom, swap, close;
  - [ ] focus by click and keyboard.
- [ ] A pane can hold a shell, an agent CLI, or an ACP thread (new or existing). The thread is
      the same one shown in Agents.
- [ ] A terminal shown in two places follows the size of the view last interacted with.
- [ ] Mac-style shortcuts for herdr's actions, for example:
  - [ ] Cmd-T new tab;
  - [ ] Cmd-D / Cmd-Shift-D split right/down;
  - [ ] Cmd-W close pane;
  - [ ] Cmd-Option-arrows move between panes;
  - [ ] Cmd-Shift-N new workspace.

  None may clash with keys terminal panes need.
- [ ] Tests: the pane tree (split, close, resize, swap), save and restore, and a headless UI
      test of the layout.

## Findings

- 2026-10-03: The latest t3code (b4d3d51a) has an orchestrator MCP that already does most of
  what was asked for agent control and subagents (delegated tasks, thread list, read, send,
  organize). It also has a stdio bridge injected into ACP `session/new`, and a CLI fallback.
  The plan follows it.

- 2026-10-03: The user's remote machines, checked read-only over SSH:

  | | `t3-home` | `devbox1` |
  |---|---|---|
  | Address | Tailscale (`192.0.2.10`), user `ahrorbek` | `203.0.113.10`, user `root` |
  | OS | Linux Mint 22.3, kernel 6.8, x86_64, glibc 2.39 | Ubuntu 26.04, kernel 7.0, x86_64, glibc 2.43 |
  | Hardware | 2 CPUs, 3.7 GB RAM, 363 GB free | 4 CPUs, 7.7 GB RAM, 82 GB free |
  | Filesystem | ext4 | ext4 |
  | Tools | git; no node, npm, uv or cargo | git, tmux; no node, npm, uv or cargo |
  | Agents on `PATH` | `claude` | none |
  | Already installed | t3code (`~/.t3`), herdr (`~/.herdr`) | t3code (`~/.t3`) |

  What follows from that:
  - **Linux x86_64 musl is the first remote build.** A static musl binary avoids glibc version
    differences.
  - **Pastures are full copies on these machines.** ext4 has no reflinks, so cow falls back to a
    plain `cp -R` there: it works, but costs the full size and copy time. Worktrees stay the
    cheap option.
  - **No Node.** npm registry agents can't install until the server can manage its own Node.
    That's added to phase 9. (Later, both machines got Node.js 24 through nvm.)
  - **No Rust.** Building on the remote isn't an option, so we cross-build here.
  - **`t3-home` is small** (2 CPUs, 3.7 GB RAM). Keep the server light.

- 2026-10-03: The server won't use GPUI. Headless GPUI on Linux pulls in about 450 crates
  (fonts, portals, layout). `http_client`, `reqwest_client` and `agent-client-protocol` are
  about 110 each, and `util`, `paths` and `collections` are already GPUI-free. Phase 1 starts by
  rewriting `agent_thread`, `projects` and `registry` as plain Rust on tokio. `reqwest_client`
  already runs tokio.

- 2026-10-03: Phase 0 spike results (commit 8430863):
  - **Cross-building works unchanged.** A GPUI-free binary with tokio, `agent-client-protocol`,
    `http_client` (with `github-download`, as `registry` uses it), `reqwest_client` and
    `alacritty_terminal` builds for `x86_64-unknown-linux-musl` with:

    ```sh
    CARGO_PROFILE_RELEASE_STRIP=symbols RUSTFLAGS="-C target-feature=+crt-static" \
      cargo zigbuild --release --target x86_64-unknown-linux-musl
    ```

    - It's 264 crates, with C code in `aws-lc-sys`, `ring` and `zstd-sys`. Nothing needed
      patching.
    - zig's linker warns "ignoring deprecated linker optimization setting '1'". That's harmless.
    - Tools: Homebrew's `zig` 0.16.0 and `cargo-zigbuild` 0.23.4.
  - **Size:** 7.2 MB stripped, 3.1 MB gzipped. The release profile keeps debug info, which makes
    48 MB unstripped, so Linux servers must be built with `strip = "symbols"`. Neither
    `zig objcopy` (unimplemented) nor `rust-objcopy` (needs `llvm-tools`) could strip it
    afterwards.
  - **It runs on `devbox1`.**
    - An HTTPS request to the ACP Registry, using the system's certificates.
    - `sh` in an `alacritty_terminal` PTY. The screen read back correctly.
    - The mock agent (system Python), started over ACP. It received the binary as a stdio MCP
      server in `session/new`, started it, and called its tool. The credential passed in the
      MCP server's `env` arrived.
    - Peak memory was about 3.7 MB. `~/.agentz` was removed afterwards.
  - **Disk:** a clean cross-build takes about 2.5 minutes and about 1 GB: 644 MB in
    `target/x86_64-unknown-linux-musl`, and 323 MB of host build scripts and proc macros in
    `target/release`.
  - **Headless GPUI also cross-compiled**, tried before the switch. It was 93 MB with debug info
    (against 48 MB), and took about 4.5 minutes and 1.3 GB. It never ran on Linux. That supports
    the GPUI-free decision.
  - **The mock agent can now call MCP tools.** A prompt of `mcp` starts the first stdio MCP
    server from `session/new`, calls its first tool, and replies `MCP: <result>`. Phase 3's
    end-to-end tests can build on it.
  - **`alacritty_terminal` needs no GPUI.** `tty::new`, `Term` and `EventLoop`, with a
    channel-backed `EventListener`, are enough. The event loop runs on its own thread.

- 2026-10-03: How the core and the app fit together (phase 1, step 1):
  - **`SharedString` stays.** `gpui_shared_string` is its own crate with no GPUI dependency,
    so the core uses it as is. No `Arc<str>` conversion is needed.
  - **`projects` needs no event channel.** Every change is a synchronous `&mut self` call, so
    the store keeps a `revision()` counter instead. The app's `project_store::ProjectStore`
    notifies when a call changes it. Saving is debounced on a plain thread.
  - **`registry` reports background work as messages.** `AgentRegistryStore::new` takes a
    tokio runtime handle and returns an inbox. The owner passes each message to `handle()`.
    The app's `registry_store::AgentRegistryStore` pumps the inbox and notifies. The server
    will do the same from its own loop. For now the app uses `reqwest_client`'s runtime.
  - **`agent_thread` works the same way.** `AgentThread::start` takes a runtime handle and
    returns an inbox. The agent's process, the SDK's handlers and requests in flight all report
    through it. Events (`WorkingChanged`, `SessionStarted`, …) queue up until the owner calls
    `take_events()`. The app's `thread_entity::AgentThread` emits them as GPUI events.
  - **Reloads drop stale messages.** Each connection has a generation, and messages from an
    older one are ignored (stale permission requests are cancelled). Before, a session opened
    by the old connection could land after a reload.
  - **One tokio worker for now.** `reqwest_client`'s runtime has a single worker thread. That's
    enough for the app in-process. The server will build its own runtime.
- 2026-10-03: The protocol and server library (phase 1, step 2):
  - **Shared types live in `agentz_protocol`.** Thread, registry and agent settings types moved
    there. `AgentThread` keeps what clients see in a `ThreadView` (read through `Deref`), and
    its permission responders beside it. A client applies `ThreadUpdate`s to its own copy, so
    the server and the app read threads through the same API.
  - **Unknown variants are untagged.** `#[serde(other)]` can't hold data or work with external
    tagging, so each enum ends in `#[serde(untagged)] Unknown(serde_json::Value)`. A malformed
    known variant also lands there.
  - **One task owns the server's state** (`agentz_server::server::Server`). Requests and the
    stores' and threads' messages arrive on one channel. After each batch it sends what changed:
    a projects snapshot when the revision moved, the registry when its snapshot differs, agent
    settings by revision, and a `ThreadUpdate` per subscribed thread from `changes_since`.
    `changes_since` compares every entry, which is fine for now; entry revisions would make it
    cheaper.
  - **Agent settings moved to the server**, in `agents/settings.json`. The first start copies
    `agents` from the app's `settings.json`.
  - **Accounts close with their client.** An agent opened from settings to log in or out
    belongs to the client that opened it.
  - **Custom agents** (`ServerConfig::custom_agents`) run a fixed command instead of a registry
    agent. Tests use them for the mock agent.
  - **The binary** is `agentz-server` with `run` (the default), `start`, `proxy` and `stop`.
    It listens on `server.sock` in the data directory (mode 0600) and writes `server.pid`.
    `start` launches `run` in its own process group with output in `logs/server.log` and
    returns once the socket accepts. A socket left by a killed server is removed, as herdr does.
    SIGTERM and Ctrl-C shut down cleanly. Like the app, it loads the login shell's environment
    when stdout isn't a terminal, and answers `--printenv`.
  - **herdr's macOS bootstrap switch is skipped.** herdr moves its daemon to the per-user Mach
    bootstrap so it outlives logout. agentZ's server stays in the login session, which keeps
    the Keychain working for agents. Revisit if that matters.
  - **It still cross-builds**: 10.8 MB static and stripped for `x86_64-unknown-linux-musl`, in
    1m 38s with a warm cache.
- 2026-10-03: The app as a client (phase 1, step 2):
  - **`crates/agentz_client`** connects to the socket, sends requests (each future resolves with
    its response) and reads events. `Events::next` also routes responses, so a response never
    overtakes the events sent before it. The server sends a request's changes before its
    response, so after `CreateThread` returns, the client already has the thread.
  - **The app keeps the same entity names and methods** (`ProjectStore`, `AgentRegistryStore`,
    `AgentThread`), now as copies of the server's state. Changes are requests; the copy updates
    when the server's event arrives. `server_client::ServerClient` owns the connection, starts
    the server with `agentz-server start` when nothing listens, and reconnects with backoff
    (250 ms up to 5 s). After a reconnect, open threads subscribe again and take the new
    snapshot.
  - **The title bar shows a "disconnected" icon**, as Zed's does, with the error in its tooltip.
  - **Agent settings** are the server's; the app sends each change as an `AgentSettingsChange`.
  - **Custom agents** load from `agents/custom.json` in the data directory and show as
    installed. That's how the mock agent runs in a scratch data directory for screenshots and
    manual tests.
  - **Checked by hand** in a scratch data directory: the app started the server, a throwaway
    protocol client created a project and a mock thread and disconnected, the turn finished on
    the server, and the app showed the thread and its messages.
  - **"Stop server" became "Restart Server".** The app reconnects and starts a server whenever
    none is running, so stopping it from the app is a restart anyway. The button asks first,
    because agents that are working stop. `agentz-server stop` still stops it for good.
  - **Reattaching mid-turn is tested** through `agentz_client`: one connection starts the mock's
    `slow` turn and drops after the first words, and a new connection's copy ends with the
    whole reply, nothing lost or repeated.

- 2026-10-03: Attention states (phase 2):
  - **The server sends facts; each client decides "done".** The session's projects snapshot
    has `working_threads`, `blocked_threads` (a permission request is waiting, recomputed after
    every batch) and each thread's `completed_at` (when a turn last ended, saved in
    `state.json`). The app keeps the completions it has displayed in `viewed.json`, so viewing
    a thread in one client doesn't clear another's (herdr).
  - **"Displayed" is Zed's `agent_status_visible`:** the window is active, settings are closed,
    and the thread is the open one.
  - **Labels and colors are t3code's:** Pending Approval (amber), Working (spinner, as before),
    Completed (green). The project switcher shows each project's most pressing status as a dot.
  - **Notifications are Zed's:** "Waiting for tool confirmation" or "Finished", for threads that
    aren't displayed, with the dock icon bouncing when the window isn't active. Clicking one
    opens the thread. macOS only shows notifications for an app bundle, so the plain
    `target/debug/agentz` build logs that they're disabled. Phase 10 adds a bundle.

- 2026-10-03: Subthreads (phase 4):
  - **A subthread is a thread with a `task`.** `projects::Task` keeps the parent, the prompt, the
    role, the `clientRequestId`, the outcome (completed, failed, cancelled, interrupted, with the
    summary) and whether the parent has heard. Lists skip subthreads; deleting a thread deletes
    its subthreads.
  - **Finalization runs after every batch** on the server: a task ends when its child is idle
    with nothing queued and no unannounced tasks of its own. The summary is the last agent
    message, or the error. Tasks unfinished when the server stops end as Interrupted at the next
    start, and the parent is told then.
  - **The parent hears with t3code's message**, "Delegated task N reached a terminal state…",
    sent as a follow-up from the task, batched when several end together. `task_status` and
    waiting count as hearing, so the message is dropped if it hasn't gone out yet. Cancelled
    tasks aren't announced.
  - **No permission modes yet**, so a child inherits the caller's ACP mode and Mode-category
    config option, which is never broader than the parent's.
  - **The app keeps one entity per thread** (`AgentThread::shared`), so the parent's Agents
    section and the subthread's own view share one subscription. A subthread opens in the main
    view with Stop and Open Parent in place of the composer. Its permission requests show as
    cards on the parent, and the parent's card shows Pending Approval.

- 2026-10-03: Diffs (phase 5):
  - **Checkpoints are t3code's:** a commit of the whole working tree (untracked files included)
    made with a private index, under `refs/agentz/checkpoints/<machine>/<thread>/<turn>`. Turn 0
    is taken when the first turn starts; turn N when turn N ends. The index lives in the git
    common directory and is reused between captures, so only changed files are hashed again.
    Sparse checkouts and folders outside git are skipped; deleting a thread or removing its
    project deletes its refs.
  - **A turn hook in `agent_thread`** runs before and after each prompt, so the checkpoint
    finishes before the turn counts as ended. A cancel during the "before" checkpoint skips the
    prompt.
  - **Diffs are parsed on the server** (`agentz_protocol::diff`), capped at 10 MB of patch, and
    sent as files and hunks. `Request::ThreadDiff` asks for the latest turn or all changes;
    the `thread_diff` capability says the server has it. `agentz_thread_diff` gives agents the
    same diff as a patch or a file list (default 50,000 characters).
  - **The diff panel** sits right of the thread (Cmd-D, or the toolbar button) and follows the
    open thread. It reloads when a turn ends. Files collapse; Viewed collapses a file until its
    contents change (t3code). No restore yet: phase 6 adds it for threads in their own
    workspace.

- 2026-10-03: Worktrees and pastures (phase 6):
  - **A thread's workspace is a path** into its project's `workspaces`, which record kind,
    branch and base. `thread_folder` is the agent's `cwd` and where checkpoints are taken.
    Subthreads and delegated tasks work where their parent does unless given a
    `workspaceStrategy`; launched threads default to the project's folder (`root`).
  - **Making a workspace is slow work off the server's task:** `spawn_then` runs git or the
    clone and comes back as `Input::Run`. Tools return `Step::Then` so a tool call can wait for a
    workspace without blocking the server.
  - **Workspace paths are canonicalized** (`/private/tmp`, not `/tmp`), as project paths are, so
    an agent's reported folder matches.
  - **Handoff** records the thread in `moving_threads`. When its turn ends, the server closes the
    agent and starts it again in the new folder (with `session/load` when it can), then sends
    the continuation prompt.
  - **Sync's dirty check ignores untracked files** (`--untracked-files=no`): a pasture copies
    untracked files like `.cow.json` from the project.
  - **Restoring** follows t3code's `restoreCheckpoint` (`git restore --source`, `git clean -fd`,
    then unstage), then drops the later checkpoints so the reverted turns stop counting. Only a
    thread alone in its worktree or pasture (its own subthreads aside) may restore, and not while
    a turn runs there. ACP can't rewind a conversation, so only files go back.
  - **The app:** New Thread's Workspace step loads the repository's branches and pasture support
    when the agent step opens and skips itself for folders outside git. Cards show the thread's
    own branch with a worktree or pasture icon; the details popover shows the folder. Project
    Settings › Checkouts lists workspaces with their threads and removes them, asking again
    when the server reports work that would be lost.
- **Machines (phase 9):**
  - **A running server outlives its binary.** The installer renames the new binary into place,
    so an older server keeps running and `proxy` (the new binary) connects to it. The server
    reads `agentz-server.sha256` beside its executable at start and sends it as
    `ServerWelcome::build`; a mismatch with the hash the client installed means it's older.
  - **`proxy` must not wait for stdin when it exits.** Tokio reads stdin on a blocking thread,
    and dropping the runtime waits for it, so the SSH session stayed open after the server
    quit and the app never saw the disconnect. It now ends with `shutdown_background`.

## Open questions

- **Default for new workspaces.** Should New Thread's workspace step suggest a pasture (cow's
  argument: instant, dependencies ready) or a worktree (t3code and herdr) when both are
  possible? The plan offers both, with the current checkout as the default.
- **"Same state" for merged projects.** t3code merges by repository only, whatever branch or
  commit each checkout is on. The plan copies that. Should agentZ also require the same branch
  or commit?
- **Agent control scope.** The plan uses t3code's rule: the caller's project only. Should agents
  also see and manage threads in other projects?

## Log

- 2026-10-03: Wrote the plan, after reading herdr, Zed's remote server and t3code's machine
  docs. Cloned herdr into `references/herdr`.
- 2026-10-03: Updated t3code to b4d3d51a. Added agent control (MCP and CLI), subthreads, diffs,
  terminal drawer and ACP client terminals, merged projects and the machine step in New Thread.
- 2026-10-03: Read t3code's project grouping in full and corrected the plan:
  - the primary remote is `upstream` before `origin`;
  - the default mode merges the whole repository, with `repository_path` and `separate` as
    options;
  - checkouts on the same machine merge too;
  - branches aren't compared.
- 2026-10-03: Added worktrees (phase 6), from t3code's workspace model and herdr's layout and
  safe removal. Later phases are renumbered.
- 2026-10-03: Read cow (cloned into `references/cow`, MIT). Added copy-on-write pastures next to
  worktrees in phase 6, with cow's sync and bring-back. Renamed the worktree tools to workspace
  tools.
- 2026-10-03: Renamed copies to pastures, cow's name for them.
- 2026-10-03: Checked `t3-home` and `devbox1` (read-only) and recorded them under Findings.
- 2026-10-03: Corrected the plan: cow supports Linux (reflink, else a full copy with a warning).
  Pastures are offered there too, as in cow.
- 2026-10-03: Switched the server from headless GPUI to a GPUI-free core on tokio, at the
  user's request.
- 2026-10-03: Finished phase 0 (8430863). The GPUI-free spike cross-builds for x86_64 musl and
  runs on `devbox1`, and the mock agent calls MCP tools. See Findings.
- 2026-10-03: Phase 1: `projects` (86606b5), `registry` (8b46efe) and `agent_thread` are plain
  Rust on tokio, wrapped by GPUI entities in the app. That finishes the GPUI-free core.
- 2026-10-03: Phase 1: `agentz_protocol` (9cf8256) and the `agentz_server` library, tested over
  in-memory streams with the mock agent, including a turn that outlives its client (687d42b).
- 2026-10-03: Phase 1: the `agentz-server` binary, with `start`, `proxy` and `stop`, tested as
  processes against a scratch data directory.
- 2026-10-03: Phase 1: the app is a client of the server (`agentz_client`, client-side copies
  of projects, the registry, agent settings and threads) (1f20235).
- 2026-10-03: Finished phase 1: the reattach test, Settings › Restart Server, and `AGENTS.md`.
- 2026-10-03: Finished phase 2: attention states, viewed completions, notifications.
- 2026-10-03: Finished phase 3: the MCP bridge in every session, the thread tools with t3code's
  policy, `agentz-server call`, and "Started by" / "Sent by" marks in the sidebar and thread.
- 2026-10-03: Finished phase 4: `delegate_task`, `task_status` and `task_cancel`, finalization
  that survives restarts, the Agents section, read-only subthreads and their permissions on the
  parent.
- 2026-10-03: Finished phase 5: checkpoints as hidden git refs around every turn (a932899),
  `agentz_thread_diff`, and the diff panel.
- 2026-10-03: Phase 6: thread workspaces, worktrees and pastures on the server, and the
  workspace tools (13fbcc2).
- 2026-10-03: Finished phase 6: New Thread's Workspace step, branches and markers on cards,
  the thread menu's pasture actions, Project Settings › Checkouts, and Revert in the diff panel.
- 2026-10-03: Phase 7: terminals in the server (488876a) and in the app (dfe27ed).
- 2026-10-03: Phase 8: terminal agent detection with herdr's manifests (f514af9).
- 2026-10-03: Phase 9: the SSH transport, Linux server builds, and the app working with
  several machines at once: a client per machine, Settings › Machines, per-machine agents,
  offline dimming. Connected to `devbox1` and `t3-home`.
- 2026-10-03: Phase 9: adding projects on other machines, with folder completion.
- 2026-10-03: Phase 9: an older server left running on a machine is detected and replaced
  only after the user confirms. Fixed `agentz-server proxy` lingering after its server quit.
- 2026-10-03: Phase 9: each server resolves its projects' repository identity.
- 2026-10-03: Phase 9: agents can start, message and list threads on the project's other
  machines, through the app.
- 2026-10-03: Phase 9: npm agents install on machines without Node.js.
- 2026-10-03: Finished phase 9.
- 2026-10-03: Added phase 11, the Workspaces view (herdr's workspaces, tabs and panes) next to
  the Agents view, at the user's request.
- 2026-10-03: The user settled phase 11's questions:
  - Mac-style direct shortcuts;
  - the agents list shows every agent on every machine;
  - workspaces live on the server and are restored after restarts.
