# Backlog progress

State of the backlog session. Every entry in `backlog.md` that needed no picks is built and
on `main`, and its entry is removed. Every entry left there is a UI change with a round on the
design board, all of them decided (`56cfa31`), and they're being built one round at a time.
Delete this file once those are built.

## Built and on main

Checked together on `main` after the last of them: `cargo fmt --all --check`, the clippy
command from AGENTS.md with no warnings, and the tests from AGENTS.md (with `--no-fail-fast`).
The only failures are four terminal tests in `agentz_server` that fail on Linux on this
machine and also failed before this session: `paused_terminals_are_adopted_with_their_process_and_screen`,
`terminal_agents_turns_are_checkpointed`, `terminal_panes_show_their_agents` and
`terminal_threads_show_their_agents_state`. A fifth, `runs_commands_and_reports_their_output_and_exit`,
failed once under load ("$ one" for "one") and passed five times alone. The built app was
also run against a scratch server with the mock agent: a message sent during a permission
request opened the queue, a command's output with `---`, `- **…**` and `#` lines showed
verbatim, MCP Servers listed agentz as Built-in and on, accounts seeded without colors got
three different ones, and a thread card's agent icon sat on its account's color. Machine
icons in the project picker need a second machine and were checked only by their test.

- `91fec52` Open the message queue when a message is queued (bug)
- `2b18572` Show tool output as it was printed, not as markdown
- `4b29738` List agentZ's own MCP server in Settings › MCP Servers. The entry's "maybe a
  skill that explains it" isn't built: agentZ ships no skills of its own today.
- `a936e5c` Give every account a color, so threads on different accounts look apart
- `cf82be1` Show machine icons instead of machine names in the project picker
- `5a48c3d` Scrollbars in threads and every area in them that scrolls
- `8466218` Thread titles from an installed CLI (Codex, Claude or agy), Settings › General
- `b9c2da5` t3code's turn rail (Previous / Next turn), and the delegated tasks' notice hidden
- `7bfd6a5` The thread list no longer flashes when a draft's account or agent changes (bug)
- `b96194a` Mermaid diagrams in threads, as Zed draws them
- `dd4a22b` The project icon picker, as t3code's, with Zed's icons only (the user's choice:
  no Lucide)

## Design rounds, decided and built in this order

Each round's `decisions.md` is its spec. A built round's backlog entry is removed.

| Backlog entry | Round | Built |
|---|---|---|
| Subscription timeline | `design/subscription-timeline/` (15 topics) | yes, Settings › Usage |
| Merge projects across machines | `design/project-copies/` (5) | yes, one name and New Thread's machine picker |
| Agents' requests for user input | `design/agent-input/` (13) | |
| Every kind of tool call | `design/tool-calls-2/` (20) and `design/agentz-tools/` | |
| Subagents and subthreads in threads | `design/agentz-tools/` (13) | |
| More options for a new thread's worktree or pasture | `design/new-workspace/` (11) | |
| Chats | `design/chats/` (14) | |
| Storage settings | `design/storage/` (13) | |
| Artifacts | `design/artifacts/` (14) | |

## Notes for the next agent

- The branch `wip/backlog` (also on origin) holds the interrupted first session's work. All of
  it was redone or finished on `main`, so it's only history now; it hasn't been deleted.
- Known and not fixed: on Linux, GPUI's fallback delete prompt cuts off its detail text, and
  Ctrl-, doesn't open Settings while a terminal has focus.
- Screenshot harness for "today" images: `/tmp/az-shot/` (README.md there), with
  `AZ_MOCK_ACCOUNTS`, `AZ_PRE_LAUNCH` and `AZ_MORE_PROJECTS` for accounts and projects. `/tmp`
  is tmpfs, so it's gone after a reboot and would need rebuilding from AGENTS.md's Testing
  section.
- Rounds are checked with headless Chromium at
  `/root/.cache/ms-playwright/chromium-1243/chrome-linux64/chrome` (`--headless=new
  --no-sandbox`) against `python3 design/server.py <port>`; the board for the user runs on the
  default port 4477.
- The safety rules from AGENTS.md apply in full: the user's real server runs from `~/.agentz`
  (never stop it, never run `agentz-server` subcommands without a scratch `AGENTZ_DATA_DIR`,
  never `pkill -f agentz`), and never prompt real agents — the mock agent only.
