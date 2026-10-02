# Progress

Tracks [plan.md](plan.md). When you finish a step:

- tick it;
- add a line to the log with the date and commit;
- update **Next**.

Note anything that changed the plan under **Findings**, and update the plan itself.

**Next:** Phase 0. Check that headless GPUI cross-compiles to Linux musl with `cargo zigbuild`.

| Phase | Status |
|---|---|
| 0. Spike | Not started |
| 1. Local server split | Not started |
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

- [ ] Check `df -h ~`. Install `zig` and `cargo-zigbuild`, and add the
      `x86_64-unknown-linux-musl` and `aarch64-unknown-linux-musl` targets.
- [ ] Cross-build a minimal headless GPUI binary (`gpui_platform::headless()`) for Linux musl,
      using `agent_thread`, `projects` and `registry`. Note the size and any crates that fail.
- [ ] Build `alacritty_terminal` (Zed's pinned version) in the workspace. Spawn a PTY running
      `sh` and read the screen.
- [ ] Have the mock agent receive a stdio MCP server in `session/new`, start it, and call a tool.
- [ ] Record the disk cost of the Linux target directories.
- [ ] Decide on anything the spike changes, and update the plan.

## 1. Local server split

- [ ] `crates/agentz_protocol`:
  - [ ] framing and handshake (versions, machine id, OS/arch, capabilities);
  - [ ] request/response and subscription messages;
  - [ ] serde types for projects, threads, entries, tool calls, permissions, config options and
        states.
- [ ] `crates/agentz_server`, a headless GPUI binary:
  - [ ] socket listener in the data directory, with a pid file;
  - [ ] owns `ProjectStore`, `AgentRegistryStore`, `AppSettings` agent settings, and the
        `AgentThread`s;
  - [ ] `proxy` subcommand that starts the server if needed.
- [ ] A thread-management service shared by every transport: the app, MCP and CLI (t3code's
      `ThreadManagementService`).
- [ ] Session subscription: projects, threads, states, as a snapshot and then events.
- [ ] Thread-detail subscription: entries, plan, permissions, config options, usage, as a
      snapshot and then events.
- [ ] Requests:
  - [ ] threads: create, prompt, cancel, permission answer, config/mode changes, rename,
        archive, unarchive, delete;
  - [ ] agents: login, logout, reload;
  - [ ] the registry: install, update, uninstall.
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
  - [ ] `cp --reflink=always` on Linux, or offer only worktrees there;
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
- [ ] Test against a real target from the user.

## 10. Polish

- [ ] Optional start at login (launchd agent, systemd user service).
- [ ] Remote server updates, only after the user confirms.
- [ ] Per-machine "Stop server".

## Findings

- 2026-10-03: The latest t3code (b4d3d51a) has an orchestrator MCP that already does most of
  what was asked for agent control and subagents (delegated tasks, thread list, read, send,
  organize). It also has a stdio bridge injected into ACP `session/new`, and a CLI fallback.
  The plan follows it.

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
