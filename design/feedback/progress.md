# Feedback: progress

The entries are in `plan.md` (numbers below are its entries). Agents working on these: take the
first item that isn't done, mark it in progress here in the same commit as your first change,
and mark it done with its commit when it's finished. Each item is committed and pushed on its
own, so `git log` and `git status` show where work stopped.

Notes for whoever continues:
- The entries describe what the user saw and wants, not the cause. Find the cause in the code
  first (start with the feature's section in `docs/architecture.md`), and say what you found
  when you mark the item done.
- Items marked "design round" are UI changes. They start on the design board
  (`design/README.md`), and building waits for the user's picks.
- Some items share one design round, so the views match: 5, 6 and 7 (usage and accounts), and
  12 and 13 (tool calls and images).
- Test with the mock agent against a scratch data directory (AGENTS.md, Testing). Never prompt
  the user's real agents.
- The order is a suggestion: fixes first, then design rounds, then the large feature (14). The
  user may reorder it.

| Order | # | Item | Kind | Status |
|---|---|---|---|---|
| 1 | 9 | Thread title cut short, in the header and when renaming | fix | not started |
| 2 | 3 | A running background task shows as waiting, and the finished sound waits for it | fix | not started |
| 3 | 4 | Subthreads play no sounds | fix | not started |
| 4 | 2 | Subthread count in the details popover, no "N agents" tooltip | fix | not started |
| 5 | 17 | Account color behind the agent icon, as designed; account in the details popover | fix | not started |
| 6 | 8 | Agents / Workspaces switcher marks the active view in every Zed theme | fix | not started |
| 7 | 16 | Project icon scan from t3code | fix | not started |
| 8 | 19 | Scrolling an agent's accounts feels buggy | fix (cause unknown) | not started |
| 9 | 20 | Stop installing the server with install.sh, and drop it from the release | fix | not started |
| 10 | 18 | More notification choices (outside the thread or the app), and a volume setting | design round | not started |
| 11 | 15 | Settings lists a combined project once, with a machine choice | design round | not started |
| 12 | 5, 6, 7 | Usage screen, usage popover in threads, Account tab with several accounts, Add Account as a modal | design round | not started |
| 13 | 11 | Subthreads: Agents list rows, an open subthread looks different, an easier way back | design round | not started |
| 14 | 12, 13 | Tool calls and images in threads, ToolSearch, agentZ's own tools ("Started a subthread") | design round | not started |
| 15 | 10 | Simpler thread terminals | design round | not started |
| 16 | 1 | Thread cards with a draft | design round | not started |
| 17 | 14 | Agents manage workspaces, terminals and projects on any machine, and delegate there | feature | not started |
