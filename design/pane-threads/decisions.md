# Pane Threads: design decisions

Picked on the design board (`design/pane-threads/`, see `design/README.md`). Each section is the spec for that part: build the picked option as described, with the comments applied. Generated from `choices.json`; don't edit by hand.

## 1. Workspaces section rows: Not decided yet

*Agents sidebar*

**Today:** The sidebar lists thread cards, then the Shells and Archived shelves. A thread started in a pane is an ordinary project thread among the cards; outside a project, New Thread… first asks you to pick a project. The Workspaces section goes just above Archived and is styled like it (decided); this is what each row shows.

## 2. Workspaces section: open or closed: Not decided yet

*Agents sidebar*

**Today:** Shells starts open each time the app starts. Archived remembers whether you left it open (kept by this Mac's server) and starts closed. A closed shelf shows its count.

## 3. Move to Agents: Not decided yet

*Agents sidebar*

**Today:** Archived rows have an Unarchive button on hover, and Unarchive in their right-click menu, which also has Rename, Project Settings and Delete. Workspaces threads don't exist yet.

## 4. Asking to add the folder as a project: Not decided yet

*Agents sidebar*

**Today:** Decided: a Workspaces thread outside every project moves to Agents only after you agree to add its folder as a project. Closing a pane that runs something asks first with a macOS alert ("Close “claude”?", Close / Cancel). Adding a project today is Open Folder…, a dialog where you type or browse to a path.

## 5. Search results: Not decided yet

*Agents sidebar*

**Today:** Typing in the search swaps the list for t3code's flat results: matching threads, then shells, then archived ones, each with its project's icon, title and time. Decided: it also finds Workspaces threads (under a project, those whose folder is in it). Up and down move the highlight; Enter opens.

## 6. A new thread in a pane: where it works: Not decided yet

*Workspaces view*

**Today:** New Thread… in a pane opens a draft in the workspace's project, with the Agents view's checkout picker under the composer (Local, New worktree, New pasture, existing ones) and the branch on the right. Decided: a pane's thread works in the shell's current folder (else the workspace's) and isn't a project thread, so that picker doesn't fit.

## 7. The thread's title bar: Not decided yet

*Agents view*

**Today:** Opened in the Agents view, a thread's title bar starts with its project (icon and name; a click starts a new thread there), then the title with its menu, then the branch and buttons. A Workspaces thread has no project.
