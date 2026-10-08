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
| 1 | 9 | Thread title cut short, in the header and when renaming | fix | done: the server cut the first prompt to 48 characters with "…"; it keeps up to 256 now, and the views cut it to their width. Titles made before stay cut |
| 2 | 3 | A running background task shows as waiting, and the finished sound waits for it | fix | done: the server completed a thread as its turn ended, whatever it left running. It now keeps it in `waiting_threads` (shown as Waiting) while a background task runs and for 15 seconds after one ends, while the agent is due to go on, and completes it once that's over |
| 3 | 4 | Subthreads play no sounds | fix | done: subthreads' own completions were already silent, but each one's end started a turn in the parent, which completed it and played the sound; a subthread's permission request played the input sound on the parent too. The parent now waits while it has subthreads it hasn't heard the end of, and subthreads' requests are silent |
| 4 | 2 | Subthread count in the details popover, no "N agents" tooltip | fix | done: the card's count had its own tooltip; it's gone, and the details popover has a row after the agent, "3 subthreads" (", 1 running" while some run), with the card's people icon |
| 5 | 17 | Account color behind the agent icon, as designed; account in the details popover | fix | done: the build followed the decision's words ("tints the agent's icon") rather than its mock, which draws the icon white on a rounded square in the account's color. `controls::AgentIcon` draws it so everywhere a thread's agent icon shows, the square in the bare icon's place, in the color as kept (the darker shade) in every theme. The details popover names the account, with its avatar, after the agent while the agent has more than one |
| 6 | 8 | Agents / Workspaces switcher marks the active view in every Zed theme | fix | not started |
| 7 | 16 | Project icon scan from t3code | fix | not started |
| 8 | 19 | Scrolling an agent's accounts feels buggy | fix (cause unknown) | not started |
| 9 | 21 | Antigravity starts an OAuth login at random times | fix (cause unknown) | not started |
| 10 | 20 | Stop installing the server with install.sh, and drop it from the release | fix | not started |
| 11 | 18 | More notification choices (outside the thread or the app), and a volume setting | design round | not started |
| 12 | 15 | Settings lists a combined project once, with a machine choice | design round | not started |
| 13 | 5, 6, 7 | Usage screen, usage popover in threads, Account tab with several accounts, Add Account as a modal | design round | not started |
| 14 | 11 | Subthreads: Agents list rows, an open subthread looks different, an easier way back | design round | not started |
| 15 | 12, 13 | Tool calls and images in threads, ToolSearch, agentZ's own tools ("Started a subthread") | design round | not started |
| 16 | 10 | Simpler thread terminals | design round | not started |
| 17 | 1 | Thread cards with a draft | design round | not started |
| 18 | 14 | Agents manage workspaces, terminals and projects on any machine, and delegate there | feature | not started |
