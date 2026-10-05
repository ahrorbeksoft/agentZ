# Empty states: design decisions

Picked on the design board (`design/empty-states/`, see `design/README.md`). Each section is the spec for that part: build the picked option as described, with the comments applied. Generated from `choices.json`; don't edit by hand.

## 1. No thread open: C. A draft right away

*Agents view*

**Today:** At every launch and after the open thread is archived or deleted, the main area says "Select a thread, or start a new one" over a New Thread button. Nothing opens by itself, so every launch starts here, even while the server has agents working or waiting.

**C. A draft right away** (from t3code index route): What t3code does now: the main area is never empty. At launch the new thread screen opens in the project of the latest thread, ready to type. Archiving the open thread opens a draft in its project; deleting it opens that project’s next thread. A draft left with nothing typed goes away, as now, and isn’t in the list.

## 2. Before the first project: C. Welcome to agentZ

*Agents view*

**Today:** Before any project, the sidebar says "No projects yet" with Open Folder…, and the main area still says "Select a thread, or start a new one"; its New Thread button opens the same folder dialog.

**C. Welcome to agentZ** (from Zed Welcome page): Zed’s first-run Welcome page: “Welcome to agentZ”, then Get Started: Open Folder…, Install an Agent… (Settings › Agents, listed until one is installed), Add Machine… and Settings. It shows until the first project.

## 3. No workspaces: E. Keys to get going

*Workspaces view*

**Today:** The view always shows a workspace while there is one, so this is only when there are none: the first visit, or after closing the last. The main area says "No workspaces" over a muted New Workspace ⌘⇧N button, which opens the New Workspace picker (each machine’s home and its projects’ checkouts).

**E. Keys to get going** (from Zed Welcome page Get Started): Zed’s Get Started list, for this view: New Workspace…, Go To…, Command Palette and Shortcuts, each with its key.

## 4. Its sidebar with none: C. The button in the list

*Workspaces view*

**Today:** The list says "No workspaces yet" and the Agents section "No agents yet", beside the main area’s "No workspaces": three empty messages at once.

**C. The button in the list** (from Zed threads sidebar empty state): Like the Agents view’s "No projects yet": the list says "No workspaces yet" over a New Workspace… button, as Zed’s threads sidebar does. No Agents section.
