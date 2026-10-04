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
- §11: after it was built, the user dropped the agent CLIs: Split Right and Split Down offer a
  shell or a new thread, and nothing scans the machine for agent CLIs. Later the user asked for
  two header buttons in place of the one Split menu: Split Right and Split Down, each a menu of
  Shell and New Thread….
- Threads started in a pane (the user's decisions after §11):
  - Every thread started in a pane (New Thread…, a split button's New Thread…) is a Workspaces thread,
    even in a project's folder. It works in the shell's current folder, else the workspace's.
  - The Agents sidebar lists them in a "Workspaces" section just above Archived, styled like
    it. Under All projects it lists all of them; under a project, those whose folder is in it.
  - In the Agents sidebar they don't ask for attention: no state marks, not counted on the
    switch. In the Workspaces view they show their state like any other agent.
  - Closing the pane leaves the thread in that section.
  - Move to Agents on its row (like Unarchive) makes it a thread of the project its folder is
    in. Outside every project it first asks to add the folder as a project.
  - Search covers the scope's threads, its Workspaces threads and its archived threads; under
    All projects, every thread.

| Order | § | Item | Status |
|---|---|---|---|
| 1 | 8 | Pane header: title first, folder as detail, buttons on hover | done |
| 2 | 9 | Focus: accent outline | done |
| 3 | 7 | Unnamed tabs named by what runs, Cmd-1…9 in tooltips | done (later the user asked for "Tab 1", "Tab 2"… by position instead) |
| 4 | 12 | Empty state: minimal | done |
| 5 | 1 | Rows: full-color icons, agent icons, `~` paths, machine only with remotes | done |
| 6 | 3A | Agents list: herdr's two-line rows | done |
| 7 | 5 | Row menu: New Tab, Copy Path, Reveal in Finder, New Thread Here | done (Delete Worktree Checkout… comes with item 9) |
| 8 | 2 | Worktree groups (herdr), collapse toggle | done |
| 9 | 15 | Delete Worktree Checkout…; fuller New Worktree dialog | done |
| 10 | 6 | Hover details: git at a glance | done |
| 11 | 3E | "Needs you" strip on top of the sidebar | done |
| 12 | 17 | Badge on the Agents / Workspaces switch | done |
| 13 | 13 | New workspace picker: Recent first, Open marks | done |
| 14 | 11 | Split menu: Shell, New Thread | done (a15a4bd, 07b7f86; agent CLIs dropped after) |
| — | — | Threads started in panes (above; designs in `design/pane-threads/`) | done |
| 15 | 10 | Drag a pane: edges split, middle swaps | done (6da7f7a) |
| 16 | 18 | Find in terminal (Cmd-F) | done (defd471) |
| 17 | 19E | Shortcut sheet (Cmd-/), by focus | done (df4e01b) |
| 18 | 16 | Command palette (Cmd-K) | done (the user split it in two, Zed's keys: Cmd-Shift-P for actions, Cmd-P to go to workspaces, tabs, panes and threads, in both views) |
| 19 | 14 | Save a tab as a layout | not started |
| 20 | 19D | Notes pane (needs a multi-line editor) | not started |
