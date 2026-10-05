# Empty states: design decisions

Picked on the design board (`design/empty-states/`, see `design/README.md`). Each section is the spec for that part: build the picked option as described, with the comments applied. Generated from `choices.json`; don't edit by hand.

## 1. No thread open: Not decided yet

*Agents view*

**Today:** At every launch and after the open thread is archived or deleted, the main area says "Select a thread, or start a new one" over a New Thread button. Nothing opens by itself, so every launch starts here, even while the server has agents working or waiting.

## 2. Before the first project: Not decided yet

*Agents view*

**Today:** Before any project, the sidebar says "No projects yet" with Open Folder…, and the main area still says "Select a thread, or start a new one"; its New Thread button opens the same folder dialog.

## 3. No workspaces: Not decided yet

*Workspaces view*

**Today:** The view always shows a workspace while there is one, so this is only when there are none: the first visit, or after closing the last. The main area says "No workspaces" over a muted New Workspace ⌘⇧N button, which opens the New Workspace picker (each machine’s home and its projects’ checkouts).

## 4. Its sidebar with none: Not decided yet

*Workspaces view*

**Today:** The list says "No workspaces yet" and the Agents section "No agents yet", beside the main area’s "No workspaces": three empty messages at once.
