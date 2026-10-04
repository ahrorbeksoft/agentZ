# Pane Threads: design decisions

Picked on the design board (`design/pane-threads/`, see `design/README.md`). Each section is the spec for that part: build the picked option as described, with the comments applied. Generated from `choices.json`; don't edit by hand.

## 1. Workspaces section rows: A. Like Archived rows

*Agents sidebar*

**Today:** The sidebar lists thread cards, then the Shells and Archived shelves. A thread started in a pane is an ordinary project thread among the cards; outside a project, New Thread… first asks you to pick a project. The Workspaces section goes just above Archived and is styled like it (decided); this is what each row shows.

**A. Like Archived rows** (from agentZ Archived shelf): One line: the icon of the project its folder is in (a folder icon outside every project), the title, and when it last did something. Dimmed icons and muted titles, like Archived, so nothing asks for attention.

## 2. Workspaces section: open or closed: A. Like Archived: remembered, closed at first

*Agents sidebar*

**Today:** Shells starts open each time the app starts. Archived remembers whether you left it open (kept by this Mac's server) and starts closed. A closed shelf shows its count.

**A. Like Archived: remembered, closed at first** (from agentZ Archived shelf): Starts closed with its count beside the name; opening or closing it is remembered across restarts, as Archived is.

## 3. Move to Agents: B. Only in the right-click menu

*Agents sidebar*

**Today:** Archived rows have an Unarchive button on hover, and Unarchive in their right-click menu, which also has Rename, Project Settings and Delete. Workspaces threads don't exist yet.

**B. Only in the right-click menu** (from agentZ row menus): No hover button; right-click a row for Rename, Move to Agents, Archive and Delete.

**Comment on B:** yeah, can be renamed, can be deleted, but not archive

## 4. Asking to add the folder as a project: A. A macOS alert

*Agents sidebar*

**Today:** Decided: a Workspaces thread outside every project moves to Agents only after you agree to add its folder as a project. Closing a pane that runs something asks first with a macOS alert ("Close “claude”?", Close / Cancel). Adding a project today is Open Folder…, a dialog where you type or browse to a path.

**A. A macOS alert** (from agentZ close confirmations (Zed prompts)): "Add “~/docs” as a project?" with "Threads in the Agents list belong to a project." and Add Project / Cancel. Add Project adds it and moves the thread there.

**Comment on A:** idk, make it consistent, if all the alerts use macos alert then yes, if not use the common one

## 5. Search results: B. With a faint list name

*Agents sidebar*

**Today:** Typing in the search swaps the list for t3code's flat results: matching threads, then shells, then archived ones, each with its project's icon, title and time. Decided: it also finds Workspaces threads (under a project, those whose folder is in it). Up and down move the highlight; Enter opens.

**B. With a faint list name** (from new): Like A, with "Workspaces" or "Archived" in faint text before the time on those results.

## 6. A new thread in a pane: where it works: A. The folder, as a plain chip

*Workspaces view*

**Today:** New Thread… in a pane opens a draft in the workspace's project, with the Agents view's checkout picker under the composer (Local, New worktree, New pasture, existing ones) and the branch on the right. Decided: a pane's thread works in the shell's current folder (else the workspace's) and isn't a project thread, so that picker doesn't fit.

**A. The folder, as a plain chip** (from agentZ static chips): A chip with a folder icon and `~/storefront/src` (full path in its tooltip), not a menu; the branch stays on the right when the folder is in git.

**Comment on A:** well, also we wanna be able to choose to create a worktree or pastures too or use the current checkout if it is a git project

## 7. The thread's title bar: A. Its folder

*Agents view*

**Today:** Opened in the Agents view, a thread's title bar starts with its project (icon and name; a click starts a new thread there), then the title with its menu, then the branch and buttons. A Workspaces thread has no project.

**A. Its folder** (from agentZ shell rows): A folder icon and the folder's name (`src`), full path in the tooltip; clicking it does nothing.
