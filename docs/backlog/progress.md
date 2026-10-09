# Backlog progress

State of the backlog implementation session, stopped at the user's request before it
finished. Everything not committed to `main` is on the branch `wip/backlog`, which may not
compile: workers were stopped mid-change. Read this before continuing; remove entries from
`backlog.md` once their work is verified, and delete this file when the backlog is done.

## Built and on main

Each was committed by its worker with tests run; the orchestrator never re-verified them end
to end, so check them before removing their entries from `backlog.md`:

- `91fec52` Open the message queue when a message is queued (bug)
- `2b18572` Show tool output as it was printed, not as markdown (+ `6579db5` architecture)
- `4b29738` List agentZ's own MCP server in Settings › MCP Servers
- `a936e5c` Give every account a color, so threads on different accounts look apart
- `cf82be1` Show machine icons instead of machine names in the project picker

## In progress, on the branch wip/backlog

- **Thread titles from an installed CLI** (`agentz_protocol/title_generation.rs`,
  `agentz_server/title_generation.rs`, `server/title_requests.rs`, tests, protocol wiring):
  most of the server side is written; the Settings UI and end-to-end tests weren't confirmed.
- **Thread view** (`agent_view.rs`, ~340 lines): some of scrollbars in threads, Previous /
  Next turn navigation, and hiding the "Delegated tasks … reached terminal states" message.
  Which of these are complete isn't recorded; diff the branch and test.
- **Sidebar** (`sidebar.rs`, ~50 lines): the thread list flash on account change was being
  worked on; whether it's a fix or an investigation aid isn't recorded.
- **Design rounds** (topics drafted, none reviewed by the user, no `decisions.md` generated
  yet — run `node design/board/decisions.js <round>` after finishing each):
  - `design/tool-calls-2/` — agents' own tool calls by kind: topics complete (667 lines).
  - `design/agentz-tools/` — agentZ's MCP tools, subthreads, subagents, delegated-tasks
    notice: topics complete, screenshots taken.
  - `design/agent-input/` — agents' requests for input: topics complete
    (`topics-permissions.js`), 16 current-state screenshots, but `index.html` missing.
  - `design/project-copies/` — project copies in sync across machines: index, topics,
    screenshots done.
  - `design/new-workspace/` — options for new worktrees/pastures: only the current-state
    screenshots; topics not written.
  - `design/artifacts/` — artifacts UI: topics complete (408 lines).

## Not started

- Mermaid diagrams in threads (port from Zed; research notes are in `backlog.md`).
- Project icon picker as in t3code (needs the Lucide-or-not decision in `backlog.md`).
- Design rounds for Chats and for Storage settings (a worker was launched and stopped
  immediately; nothing exists).
- Design round for the subscription timeline (CLIProxyAPI's management UI must be studied
  first; it's not in `references/`).
- Building everything the design rounds cover, once the user picks.

## How the session worked, for the next agent

- Backlog entries that need no input were built by workers in parallel, each committing and
  pushing on its own; UI changes got design rounds per AGENTS.md. Several workers shared this
  working tree at once with disjoint file areas, `rustfmt <files>` instead of `cargo fmt
  --all`, and never `cargo clean` (target/ was cleaned once at the start).
- Screenshot harness for "today" images: `/tmp/az-shot/` (`start.sh`, `drive.sh`, `shot.sh`,
  `stop.sh`, README.md, verified end to end) with frozen binaries in `/tmp/az-baseline/`.
  `/tmp` is tmpfs: both are gone after a reboot and would need to be rebuilt from AGENTS.md's
  Testing section. Use one Xvfb display per worker (92–96 were used).
- Design boards were served per worker on ports 4481–4485 (`python3 design/server.py <port>`,
  detached) and checked with headless Chromium at
  `/root/.cache/ms-playwright/chromium-1243/chrome-linux64/chrome` (`--headless=new
  --no-sandbox`). The board for the user runs on the default port 4477.
- The safety rules from AGENTS.md apply in full: the user's real server runs from `~/.agentz`
  (never stop it, never run `agentz-server` subcommands without a scratch `AGENTZ_DATA_DIR`,
  never `pkill -f agentz`), and never prompt real agents — the mock agent only.
