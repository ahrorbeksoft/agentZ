# Plan: background servers, terminals and machines

The goal is a herdr-like experience in agentZ:

- Agents keep working after the app quits, and the app reattaches to them.
- The sidebar shows which threads need you.
- Agents' own CLIs run in real terminal panes.
- Other machines are reached over SSH, all in one window.

Progress is tracked in [progress.md](progress.md). Update it as you go.

## Decisions

The user made these on 2026-10-03:

| Question | Decision |
|---|---|
| Which parts of herdr? | Everything: keep the ACP thread UI, *and* add real terminal panes for agents' own CLIs. |
| How to reach machines? | SSH only. No pairing links, no listening ports, no accounts. |
| Remote OS in the first version | Macs and Linux (x86_64 and aarch64). |

The standing rules in `AGENTS.md` still apply:

- Copy Zed, t3code or herdr behavior; don't invent extras.
- Never prompt the user's real agents.
- Never steal focus.
- Commit each finished step.

## References

All three are read-only clones in `references/` (gitignored).

- **herdr** (`references/herdr`, Apache-2.0, Rust) is the model for the experience. Read these:
  - `docs/next/website/src/content/docs/`: `concepts.mdx`, `connecting-machines.mdx`,
    `persistence-remote.mdx`, `session-state.mdx`, `agents.mdx`.
  - `src/server/`, `src/client/`, `src/remote/` and `src/protocol/`: the client/server split,
    attaching, SSH machines and versioned wire codecs.
  - `src/detect/` and `src/detect/manifests/*.toml`: terminal agent state detection.
  - Ported code or manifests must keep herdr's Apache-2.0 notice.
- **Zed** (`references/zed`) is the model for the mechanics:
  - `crates/remote` and `crates/remote_server`:
    - the SSH transport;
    - uploading the server;
    - the `proxy` subcommand that bridges SSH stdio to a daemon over unix sockets;
    - building the Linux server with `cargo zigbuild` (`remote/src/transport.rs`,
      `build_remote_server_from_source`).
  - `crates/terminal`: an `alacritty_terminal`-based terminal model.
  - `crates/terminal_view/src/terminal_element.rs`: the GPUI terminal renderer.
- **t3code** (`references/t3code`) is the model for how machines show up in the UI. Read these:
  - `docs/internals/remote.md`, `overview.md`, `connection-runtime.md`.
  - `docs/user/remote-access.md`, `background-service.md`.
  - `apps/web/src/components/ProjectEnvironmentBadge.tsx` and `EnvironmentMachineIcon.tsx`.

## Architecture

```
agentz (GPUI app, a client)
  ├─ unix socket ─────────────────────────────────────► agentz-server  (this Mac)
  └─ ssh <host> ~/.agentz/server/<ver>/agentz-server proxy
                    └─ stdio ⇄ unix socket ───────────► agentz-server  (remote Mac/Linux)
```

**Each machine runs one `agentz-server`.** It is the only owner of that machine's work:

- agent processes (ACP connections and sessions);
- terminals (PTYs and terminal state);
- projects and threads (`state.json`);
- installed agents (the registry) and per-agent settings.

Clients never substitute their own files, credentials or agents for the server's (t3code's
rule).

**The server is a headless GPUI app**, as Zed's `remote_server` is, using
`gpui_platform::headless()`. `agent_thread`, `projects` and `registry` move into it with few
changes. Headless GPUI builds for Linux without wayland or x11.

**The app is a client.** It connects to every enabled machine at once:

- It keeps a read-only copy of each server's projects, threads and states.
- It only streams the full contents (messages, terminal screens) of what's on screen.
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
  - A thread-detail or terminal-screen subscription is opened only for what a client is
    viewing.
- **Encoding:** serde. JSON first, for debuggability. Switch terminal frames to bincode if
  they're too big.
- **Machine identity:** a stable id stored by each server, independent of the route used to
  reach it (t3code).

### Attention states

Every thread has a state, rolled up to its project (herdr):

| State | ACP thread | Terminal thread |
|---|---|---|
| working | a prompt is running | the manifest matches "working" |
| blocked | a permission request is waiting | the manifest matches an approval prompt |
| done | finished, and this client hasn't viewed it | same |
| idle | finished and viewed | same |
| unknown | — | no manifest matches |

- "Viewed" is per client (herdr).
- Notifications: a macOS notification when a thread that isn't on screen becomes done or
  blocked. Nothing for the visible thread.

### Terminals

- **Server side:** a port of Zed's `terminal` crate (PTY plus `alacritty_terminal`), without
  settings, tasks or workspace. The server keeps the terminal running, and keeps its screen and
  scrollback, while nobody is watching.
- **Client side:** a port of Zed's `terminal_element.rs`, without the editor and workspace
  dependencies.
  - It renders a terminal content snapshot (cells, cursor, mode, selection) received from the
    server.
  - It sends keystrokes, paste, resize, scroll and selection back.
  - Key-to-bytes mapping uses Zed's `mappings/`.
- **Streaming:** only for terminals a client is viewing (herdr's "surface interest"), throttled
  to frame rate. Background terminals still parse output, for detection.
- **Terminal threads:** New Thread offers **Terminal** next to the ACP agents:
  - a login shell, or
  - an agent CLI found on that machine's `PATH` (claude, codex, opencode, gemini, …, from
    herdr's manifest list).

  It's one terminal per thread first. Splits and tabs come later, if wanted.
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
- **UI** (t3code):
  - With only Local, nothing changes.
  - Remote threads get a machine icon on their card. The details popover gets its machine line
    back.
  - The project switcher groups projects by machine.
  - Settings gains a **Machines** page. Each machine's agents (install, log in, defaults) are
    managed from its own Settings.
- **Adding a project on a remote:** a path field with completion served by that machine. A
  native folder dialog can't browse another machine.

## Phases

Each phase ships on its own, keeps the app working, and is committed.

0. **Spike.** Prove the risky parts before building on them:
   - headless GPUI cross-compiles to Linux musl via zigbuild;
   - `alacritty_terminal` builds here;
   - measure disk use.
1. **Local server split.** Add the `agentz_protocol` and `agentz_server` crates.
   - Move the stores and threads into the server.
   - In the app, the threads become client copies with the same API as `AgentThread`, so
     `agent_view` changes little.
   - Reattach: a snapshot, then live events.

   Result: the same app, but threads survive quitting it.
2. **Attention states and notifications** for ACP threads.
3. **Terminal threads.** The server-side terminal, the client terminal element, streaming,
   input, and New Thread › Terminal.
4. **Terminal agent detection** using herdr's manifests.
5. **Machines over SSH:**
   - profiles and Settings › Machines;
   - install/upload and `proxy`;
   - reconnecting and Attention;
   - remote projects and per-machine agent settings;
   - machine icons and grouping.
6. **Polish:**
   - optional start at login (launchd/systemd user service);
   - confirmed remote server updates;
   - per-machine "Stop server".

Phases 3–4 and 5 don't depend on each other. Both need phase 1.

## Testing

- **Server and protocol:** in-process tests that drive a server over a socket pair with the mock
  agent (`crates/agent_thread/test_support/mock_agent.py`).
- **Proxy and reconnect:** run `agentz-server proxy` directly as the transport, with no SSH. Kill
  it to simulate a dropped connection.
- **Terminals:** drive a PTY running `sh` with scripted input, and check the screen snapshots.
- **Real SSH:** needs a target from the user, for example a Linux VM, another Mac, or
  `ssh localhost` with Remote Login enabled. Ask before relying on one.
- **UI:** headless GPUI tests and PopUp screenshots, as described in `AGENTS.md`.

## Risks

- **Phase 1 touches almost everything.** Thread state crossing a socket, reattach snapshots, and
  keeping `agent_view` unchanged. Keep tests green at every step.
- **Disk:** Linux cross-builds add another target directory (several GB). Check `df -h ~` first.
- **SSH authentication can't prompt in the background.** Show the error and the command. Later,
  agentZ could run that `ssh` in a terminal pane for the user.
- **macOS privacy prompts** (Desktop, Documents) now attach to `agentz-server`, not the app.
- **PATH:** agents must see the same `PATH` as now. Locally, the server inherits the app's
  environment. Remotely, it runs under a login shell (`sh -lc`).
