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
| 3. Terminal threads | Not started |
| 4. Terminal agent detection | Not started |
| 5. Machines over SSH | Not started |
| 6. Polish | Not started |

## 0. Spike

- [ ] Check `df -h ~`. Install `zig` and `cargo-zigbuild`, and add the
      `x86_64-unknown-linux-musl` and `aarch64-unknown-linux-musl` targets.
- [ ] Cross-build a minimal headless GPUI binary (`gpui_platform::headless()`) for Linux musl,
      using `agent_thread`, `projects` and `registry`. Note the size and any crates that fail.
- [ ] Build `alacritty_terminal` (Zed's pinned version) in the workspace. Spawn a PTY running
      `sh` and read the screen.
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
- [ ] Session subscription: projects, threads, states, as a snapshot and then events.
- [ ] Thread-detail subscription: entries, plan, permissions, config options, usage, as a
      snapshot and then events.
- [ ] Requests:
  - [ ] threads: create, prompt, cancel, permission answer, config/mode changes, rename,
        archive, delete;
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

## 3. Terminal threads

- [ ] Server: port Zed's `terminal` (PTY plus `alacritty_terminal`) without settings, tasks or
      workspace.
- [ ] Protocol: terminal content snapshot and changes, streamed only for viewed terminals and
      throttled. Input, paste, resize, scroll and selection go back.
- [ ] App: port `terminal_element.rs` and `mappings/` without the editor and workspace
      dependencies.
- [ ] Thread kind "terminal" in `projects`. New Thread › Terminal: a login shell, or an agent CLI
      found on that machine's `PATH`.
- [ ] Terminals survive app restarts, keeping their screen and scrollback.
- [ ] Tests: a scripted `sh` session and its screen snapshots.

## 4. Terminal agent detection

- [ ] Port herdr's manifest format and matcher, reading the bottom of the screen buffer. Keep
      herdr's Apache-2.0 notice.
- [ ] Bundle herdr's manifests for the CLIs we offer.
- [ ] Feed the result into the attention states.

## 5. Machines over SSH

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
- [ ] UI: machine icon on remote cards, machine line in the details popover, projects grouped by
      machine in the switcher.
- [ ] Ask before replacing a running remote server.
- [ ] Test against a real target from the user.

## 6. Polish

- [ ] Optional start at login (launchd agent, systemd user service).
- [ ] Remote server updates, only after the user confirms.
- [ ] Per-machine "Stop server".

## Findings

(None yet.)

## Log

- 2026-10-03: Wrote the plan, after reading herdr, Zed's remote server and t3code's machine
  docs. Cloned herdr into `references/herdr`.
