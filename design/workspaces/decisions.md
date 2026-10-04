# Workspaces: design decisions

Picked on the design board (`design/workspaces/`, see `design/README.md`). Each section is the spec for that part: build the picked option as described, with the comments applied. Generated from `choices.json`; don't edit by hand.

## 1. Workspace rows: A. Tidied two lines

*Sidebar*

**Today:** Two lines, laid out like the Agents sidebar's shell rows: the project icon (dimmed when not selected), the name and the status; then the branch (or the path outside git), ↑↓, terminal and agent counts, and the machine.

**A. Tidied two lines** (from herdr rows): Same layout, cleaned up: icons at full color, agents shown by their own icons instead of a count, `~` paths, and the machine only when you have more than one.

## 2. Organizing the list: A. Flat, worktrees nested

*Sidebar*

**Today:** One flat list in the order you drag it to. A worktree opened from a workspace's menu becomes an unrelated workspace at the end. Workspaces on other machines are mixed in.

**A. Flat, worktrees nested** (from herdr): Your order stays, but a worktree or pasture made from a workspace sits under it, indented with a connector. Closing the parent closes the group (checkouts and branches stay). herdr's grouped worktrees.

**Comment on A:** but if it is opened inside the this worktree we should hide from here (like herdr)

## 3. Agents list: A. Two lines, herdr's default + also E. "Needs you" strip on top

*Sidebar*

**Today:** An "Agents" section under the workspaces: one slim row per agent in a pane (agent CLIs and ACP threads), with its state dot, name and "machine · workspace › tab". Clicking focuses its pane.

**A. Two lines, herdr's default** (from herdr rows): herdr's default agent layout: the state and where it is on top (machine, workspace, tab), the agent's icon and name below.

**Also take from E. "Needs you" strip on top** (from new): No list at the bottom. Agents waiting on you show in a tinted strip above the workspaces, with a Go button; working and done ones show in their workspace's row (pairs with rows E).

**Comment on E:** Keep A's list at the bottom and add E's "Needs you" strip on top, as an attention signal (the user's pick on Getting your attention)

## 4. Collapsed sidebar: A. Hidden (today)

*Sidebar*

**Today:** Cmd-B hides the sidebar completely. With it hidden there's no way to see other workspaces or their states, or to switch, except by showing it again.

**A. Hidden (today)** (from Zed): Keep it as is: hidden is hidden.

## 5. Row actions and menu: A. A fuller right-click menu

*Sidebar*

**Today:** Right-click a row: Rename, Close, and in a git repository New Worktree and Open Worktree…. Double-click renames. No buttons on hover.

**A. A fuller right-click menu** (from herdr + Zed): The same menu with what's missing: New Tab, Copy Path, Reveal in Finder (this Mac only), New Thread Here (opens the Agents view in that folder), and on a worktree, Delete Worktree Checkout… (herdr's safe remove).

## 6. Hover details: C. Git at a glance

*Sidebar*

**Today:** Half a second on a row shows the thread cards' details popover: name, project, machine, branch, path and what's inside ("3 terminals, 1 agent"). It hides while the counts show their own tooltip.

**C. Git at a glance** (from Zed git panel): Focused on the checkout: branch and upstream, ahead/behind, uncommitted changes as +/−, the last commit, and the worktree's path.

## 7. Workspace header and tabs: A. Tabs named by what runs

*Main area*

**Today:** Zed's tab bar: each tab with its state dot and name (numbered when unnamed, as herdr does), × on hover, + at the end. Nothing above the panes says which workspace you're in.

**A. Tabs named by what runs** (from Zed + iTerm2): Today's bar, but an unnamed tab takes its focused pane's title ("Claude Code", "npm run dev") in muted text instead of a number; a name you give it wins. Cmd-1…9 shown in the tooltip.

## 8. Pane header: A. Fixed and quieter

*Main area*

**Today:** Each pane has a 36px header: icon, title, a detail (a shell's window title, a thread's agent), the state dot, then Split, Zoom and Close. An ACP thread's pane folds its toolbar into the same header.

**A. Fixed and quieter** (from Zed pane tabs): The title always fits first; the detail is the folder relative to the workspace and the program, not the raw window title. Unfocused panes show their buttons on hover only.

## 9. Focus and dividers: A. Clear accent outline

*Main area*

**Today:** In a split tab the focused pane has a thin blue-gray border (the theme's border.focused); others have none. The focused header uses the active tab background. Dividers are 1px lines you can drag.

**A. Clear accent outline** (from Zed active pane): The focused pane gets a 1px accent outline and the bright header; everything else as today.

## 10. Rearranging panes: B. Edges split, middle swaps

*Main area*

**Today:** Dragging a pane's header onto another pane swaps the two. Tabs and workspaces reorder by dragging. Panes can't move to another tab or workspace, or into a new split.

**B. Edges split, middle swaps** (from Zed pane drag targets): Over another pane, the half you're nearest lights up: dropping there splits it that way; the middle swaps. Zed does this for its pane items.

## 11. What a new pane opens: A. Choices in the split menu

*Main area*

**Today:** New Tab, Split Right and Split Down open a shell in the workspace's folder. To get an agent you start it in the shell, or use the pane menu's New Thread… / Show Thread ▸.

**A. Choices in the split menu** (from Zed split menu): Split Right / Down stay a shell, but each gets a submenu: Shell, the agent CLIs found on that machine (Claude Code, Codex…), and New Thread with an installed ACP agent.

## 12. Empty state: E. Minimal

*Main area*

**Today:** With no workspaces: "Workspaces put terminals and threads side by side" and a New Workspace button that opens the + picker.

**E. Minimal** (from Zed empty pane): Just "No workspaces" and the shortcut, like Zed's empty editor.

## 13. New workspace: A. Today's picker, refined

*Creating*

**Today:** The sidebar's + opens a popover: a search field, then each machine's home folder and each project's checkout, worktrees and pastures, under their machine. Picking one opens a workspace with a shell there.

**A. Today's picker, refined** (from Zed recent projects): Recent folders first, then projects and homes. A folder that already has a workspace is marked Open, and picking it goes there instead of making another.

## 14. Layouts and templates: B. Save a tab as a layout

*Creating*

**Today:** Every new tab is one shell; you build splits by hand each time. herdr and t3code leave this to plugins and project scripts; zellij and tmuxinator have layout files.

**B. Save a tab as a layout** (from iTerm2 arrangements): A tab's menu gets Save Layout…: name it, and choose which panes keep their command (npm run dev, claude). Saved layouts join the + menu.

## 15. Worktrees: A. Grouped, with Delete Checkout + also C. A fuller dialog

*Creating*

**Today:** A git workspace's menu has New Worktree (a dialog: branch name, Worktree or Pasture, "the branch starts from … · Enter makes it and opens a terminal there") and Open Worktree… (the repository's other checkouts). Either opens a separate workspace.

**A. Grouped, with Delete Checkout** (from herdr worktrees): herdr's model: a worktree opens under its source workspace; its menu adds Delete Worktree Checkout…, which runs `git worktree remove`, asks again before forcing if files changed, and keeps the branch.

**Also take from C. A fuller dialog** (from t3code new worktree): Today's dialog plus: the base branch as a picker, what each kind means in one line, what to open in it (Shell, Claude Code, Codex), and where it will live.

**Comment on A:** p picked c too but from that remove the open with thing

## 16. Jump to anything: B. Command palette

*Navigation and attention*

**Today:** You get around with the sidebar and the Agents list, Cmd-} / Cmd-{ for tabs and Cmd-Option-arrows between panes. There's no search across panes, and no way to jump to tab 3 or workspace 2 directly.

**B. Command palette** (from Zed / t3code command palette): Cmd-K mixes places (workspaces, tabs, panes) with actions (Split Right, New Worktree…, Rename Tab), each with its shortcut.

## 17. Getting your attention: C. Badges on the view switch

*Navigation and attention*

**Today:** States show as the row's status, a dot on the tab and the pane header, and in the Agents list. ACP threads also send macOS notifications when you aren't looking; agent CLIs in panes don't.

**C. Badges on the view switch** (from macOS dock badges): The Agents | Workspaces switch shows a count on the view you're not in when something there needs you, colored by the most urgent state.

**Comment on C:** i picked E too [clarified: E from the Agents list topic, the "Needs you" strip on top, not this page's inbox]

## 18. Terminal pane features: B. Find in the terminal

*Navigation and attention · pick any*

**Today:** Panes run Zed's terminal element: selection, mouse, IME, font size. Zed's terminal also has find; herdr has copy mode and search; t3code finds local servers. None of these are in agentZ's panes yet.

**B. Find in the terminal** (from Zed terminal search): Cmd-F opens a find bar in the pane: matches highlighted in the screen and scrollback, a count, Enter for next.

## 19. Workspace features: D. A notes pane + E. Shortcut sheet

*Navigation and attention · pick any*

**Today:** A workspace holds tabs and panes; closing is final; there's no place for scripts, notes or a list of shortcuts.

**D. A notes pane** (from new): A pane type for a workspace's own markdown note: a checklist, context to paste to agents, commands to remember. Kept by the server with the workspace.

**E. Shortcut sheet** (from herdr prefix+? help): Cmd-/ shows every Workspaces shortcut in an overlay, filterable as you type, as herdr's keybind help does.

**Comment on E:** based on the focus, too
