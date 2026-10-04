# Workspaces: build progress

The spec is `decisions.md` (numbers below are its sections). Agents building this: take the
first item that isn't done, mark it in progress here in the same commit as your first change,
and mark it done with its commit when it's finished. Each item is committed and pushed on its
own, so `git log` and `git status` show where work stopped.

Notes for whoever continues:
- §2's comment ("hide it like herdr") means herdr's grouping by git identity: any workspace
  whose folder is a linked worktree of a repository sits under the workspace on that
  repository's main checkout, however it was opened; the parent row's ▾/▸ hides the children
  (collapsed, only the focused child stays, and the parent shows the group's most urgent
  state); a child is labeled by its branch unless renamed. herdr: `src/client/shell/sidebar.rs`
  `workspace_entries`, `displayed_workspace_status`, `workspace_rows`.
- §3 is A plus E's "Needs you" strip (the user's "E too" on §17 meant §3's E).
- §15 is A plus C's dialog, without C's "Open with".
- §19E: the sheet shows the shortcuts for what's focused (terminal, thread, sidebar) too.

| Order | § | Item | Status |
|---|---|---|---|
| 1 | 8 | Pane header: title first, folder as detail, buttons on hover | done |
| 2 | 9 | Focus: accent outline | done |
| 3 | 7 | Unnamed tabs named by what runs, Cmd-1…9 in tooltips | done |
| 4 | 12 | Empty state: minimal | done |
| 5 | 1 | Rows: full-color icons, agent icons, `~` paths, machine only with remotes | done |
| 6 | 3A | Agents list: herdr's two-line rows | done |
| 7 | 5 | Row menu: New Tab, Copy Path, Reveal in Finder, New Thread Here | not started |
| 8 | 2 | Worktree groups (herdr), collapse toggle | not started |
| 9 | 15 | Delete Worktree Checkout…; fuller New Worktree dialog | not started |
| 10 | 6 | Hover details: git at a glance | not started |
| 11 | 3E | "Needs you" strip on top of the sidebar | not started |
| 12 | 17 | Badge on the Agents / Workspaces switch | not started |
| 13 | 13 | New workspace picker: Recent first, Open marks | not started |
| 14 | 11 | Split menu: Shell, agent CLIs on the machine, New Thread | not started |
| 15 | 10 | Drag a pane: edges split, middle swaps | not started |
| 16 | 18 | Find in terminal (Cmd-F) | not started |
| 17 | 19E | Shortcut sheet (Cmd-/), by focus | not started |
| 18 | 16 | Command palette (Cmd-K) | not started |
| 19 | 14 | Save a tab as a layout | not started |
| 20 | 19D | Notes pane (needs a multi-line editor) | not started |
