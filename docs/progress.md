# Progress

Tracks [plan.md](plan.md). When you finish a step:

- tick it;
- add a line to the log with the date and commit;
- update **Next**.

Note anything that changed the plan under **Findings**, and update the plan itself.

**Next:** Phase 1, the app as a client: start the server with `agentz-server start`, connect,
reconnect, and keep client copies of projects, the registry, agent settings and threads.

| Phase | Status |
|---|---|
| 0. Spike | Done |
| 1. Local server split | In progress |
| 2. Attention states and notifications | Not started |
| 3. Agent control (MCP and CLI) | Not started |
| 4. Subthreads | Not started |
| 5. Diffs | Not started |
| 6. Worktrees and pastures | Not started |
| 7. Terminals | Not started |
| 8. Terminal agent detection | Not started |
| 9. Machines over SSH | Not started |
| 10. Polish | Not started |

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
- [ ] App: a client connection that starts the local server detached if it isn't running, and
      reconnects.
- [ ] App: client-side thread copies with `AgentThread`'s API. Port `agent_view`, `sidebar`,
      `settings_page` and `shell` to them.
- [ ] Reattach after an app restart mid-prompt: the turn continues, and the view catches up.
- [ ] Settings: "Stop server".
- [ ] Tests: protocol round-trips, server driven over a socket pair with the mock agent, and
      reattach.
- [ ] Update `AGENTS.md`: layout, how to run and test the server.

## 2. Attention states and notifications

- [ ] Server: thread state (working / blocked / done / idle) and events.
- [ ] Client: track which completions this client has viewed. "Done" until the thread is viewed.
- [ ] Sidebar cards and the project switcher show states, rolled up to the project.
- [ ] macOS notifications for done or blocked threads that aren't on screen.

## 3. Agent control (MCP and CLI)

- [ ] `agentz-server mcp-bridge`: a stdio MCP server that talks to the server socket, with a
      per-session credential in its environment.
- [ ] Inject it into every ACP `session/new` and `session/load`. Revoke the credential when the
      session closes.
- [ ] Tools:
  - [ ] `orchestrator_capabilities`: agents, models, modes, login state, machines;
  - [ ] `agentz_thread_list` and `agentz_thread_read` (messages and activity views,
        incremental);
  - [ ] `agentz_thread_launch` and `create_threads`;
  - [ ] `agentz_thread_send` (auto/queue/restart), `agentz_thread_wait`,
        `agentz_thread_interrupt`;
  - [ ] `agentz_thread_update` (rename) and `agentz_thread_organize` (archive, unarchive).
- [ ] Policy:
  - [ ] project scope;
  - [ ] no permission escalation;
  - [ ] no delete, and no answering permissions;
  - [ ] `clientRequestId` idempotency;
  - [ ] typed failures.
- [ ] Threads and messages created by agents are marked `createdBy: agent`, and the UI shows it.
- [ ] `agentz` CLI with `--json`, covering the same operations. Put `AGENTZ_SOCKET` and
      `AGENTZ_THREAD_ID` in agentZ terminals.
- [ ] Mock agent: scripted MCP tool calls. End-to-end tests.
- [ ] Document the tools for agents, like t3code's orchestration instructions.

## 4. Subthreads

- [ ] Thread lineage in `projects`: parent, kind `subagent`, created by.
- [ ] `delegate_task` (async/wait, agent/model/machine/role/title, timeout), `task_status`,
      `task_cancel`.
- [ ] Finalization from the child's events, idempotent and surviving restarts. The result is the
      last agent message, or the error.
- [ ] The child gets the task prompt only; permissions are inherited and never broader.
- [ ] UI:
  - [ ] the Agents control on the parent (card and thread view);
  - [ ] opening a subthread read-only;
  - [ ] subthreads hidden from the main list.
- [ ] A subthread's permission requests show on the parent, which becomes blocked.
- [ ] Tests: delegation, wait, cancel, nesting, restart during a subthread.

## 5. Diffs

- [ ] Server: checkpoints as hidden git refs before and after each turn. Skip projects that
      aren't git repositories.
- [ ] Turn diff and thread diff queries. `agentz_thread_diff`.
- [ ] Diff panel: This turn / All changes, files and hunks, mark as viewed.
- [ ] Tests with temporary repositories.

## 6. Worktrees and pastures

- [ ] Thread workspace in `projects`: kind (checkout, worktree, pasture), path and branch. The ACP
      session's `cwd` follows it.
- [ ] Server, worktrees:
  - [ ] `git worktree add -b agentz/<id> <data>/worktrees/<repo>/<branch> <base>`;
  - [ ] recursive submodules.
- [ ] Server, pastures (port of cow's `create`):
  - [ ] `clonefile(2)` on macOS, skipping `target`, `.build`, `DerivedData`, `.turbo`;
  - [ ] on Linux, `cp --reflink=always -R`, falling back to a full `cp -R` with a warning
        (cow);
  - [ ] git fixes: drop `.git/worktrees`, `checkout.guess false`, branch;
  - [ ] cleanup: `*.pid`, `*.sock`, `*.socket`, plus `.cow.json` `post_clone`;
  - [ ] roll back on failure.
- [ ] Pastures: sync from the project (temporary remote, rebase or merge, abort on conflict) and
      bring the branch back to the project.
- [ ] List the project's worktrees and pastures.
- [ ] New Thread › Workspace: Current checkout / New pasture / New worktree (base branch) /
      existing. Thread menu: New thread in this workspace, Sync, Bring branch to project.
- [ ] Cards show the thread's own branch. The details popover shows the workspace kind and
      folder.
- [ ] Project Settings › Checkouts:
  - [ ] list worktrees and pastures;
  - [ ] remove a worktree with `git worktree remove`, asking again before forcing;
  - [ ] remove a pasture, warning about uncommitted or unpushed work;
  - [ ] keep branches;
  - [ ] refuse while the workspace is in use.
- [ ] Tools:
  - [ ] `agentz_workspace_status`, `agentz_workspace_list`;
  - [ ] `agentz_workspace_handoff`, reopening the session with `session/load` in the new
        `cwd` or a new session;
  - [ ] `agentz_workspace_sync`, `agentz_workspace_bring_back`;
  - [ ] `workspaceStrategy` (root, worktree, pasture, existing) on `agentz_thread_launch` and
        `delegate_task`.
- [ ] Diffs: restoring files only for a thread in its own, unshared workspace.
- [ ] Tests with temporary repositories (APFS for pastures).

## 7. Terminals

- [ ] Server: port Zed's `terminal` (PTY plus `alacritty_terminal`) without settings, tasks or
      workspace. Keep 5,000 lines and 8 MiB of scrollback.
- [ ] Protocol: terminal content snapshot and changes, streamed only for viewed terminals and
      throttled. Input, paste, resize, scroll and selection go back.
- [ ] App: port `terminal_element.rs` and `mappings/` without the editor and workspace
      dependencies.
- [ ] Terminal threads: thread kind "terminal". New Thread › Terminal: a login shell, or an
      agent CLI found on that machine's `PATH`.
- [ ] Thread terminal drawer under ACP threads.
- [ ] ACP client `terminal` capability backed by server terminals. Tool calls link to their live
      terminal.
- [ ] `agentz_terminal_*` tools and CLI commands.
- [ ] Terminals survive app restarts, keeping their screen and scrollback.
- [ ] Tests: a scripted `sh` session and its screen snapshots.

## 8. Terminal agent detection

- [ ] Port herdr's manifest format and matcher, reading the bottom of the screen buffer. Keep
      herdr's Apache-2.0 notice.
- [ ] Bundle herdr's manifests for the CLIs we offer.
- [ ] Feed the result into the attention states.

## 9. Machines over SSH

- [ ] Client: saved machine profiles and Settings › Machines (add, rename, disable, remove).
- [ ] SSH transport:
  - [ ] `BatchMode` with a shared ControlMaster;
  - [ ] `uname -sm` detection;
  - [ ] upload to `~/.agentz/server/<version>/`;
  - [ ] run `proxy` under a login shell.
- [ ] Build the Linux servers (x86_64/aarch64 musl) with `cargo zigbuild`.
- [ ] Connection states: Online / Reconnecting (backoff up to 2 minutes) / Attention, with the
      error and the command to run.
- [ ] Offline machines stay visible but dimmed, with input disabled.
- [ ] Remote projects: path field with completion from that machine.
- [ ] Per-machine agents: install, log in, defaults.
- [ ] Managed Node for npm agents when the machine has none (Zed's `node_runtime`).
- [ ] Repository identity from each server, as t3code does:
  - [ ] repository root, then the primary remote (`upstream`, `origin`, first by name);
  - [ ] canonical key via `normalizeGitRemoteUrl`;
  - [ ] display name `owner/repo`;
  - [ ] cached for 15 minutes, or 1 minute when there's no repository or remote.
- [ ] Merged projects:
  - [ ] grouping modes `repository` (default), `repository_path`, `separate`;
  - [ ] Settings › General switch and a per-project override;
  - [ ] t3code's label rule;
  - [ ] machines badge and combined threads;
  - [ ] branch shown per checkout;
  - [ ] project name and icon shared by the group.
- [ ] New Thread: Project → Machine (only when the project is on several machines; defaults to
      the last used one) → Agent on that machine.
- [ ] UI: machine icon on remote cards, machine line in the details popover, projects grouped by
      machine in the switcher.
- [ ] Agent control across machines: `delegate_task` and thread launch with a machine; listing
      covers every machine of a merged project.
- [ ] Ask before replacing a running remote server.
- [ ] Test against `t3-home` and `devbox1`, leaving their t3code and herdr installs alone.

## 10. Polish

- [ ] Optional start at login (launchd agent, systemd user service).
- [ ] Remote server updates, only after the user confirms.
- [ ] Per-machine "Stop server".

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
    That's added to phase 9.
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
