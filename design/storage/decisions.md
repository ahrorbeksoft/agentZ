# Storage: design decisions

Picked on the design board (`design/storage/`, see `design/README.md`). Each section is the spec for that part: build the picked option as described, with the comments applied. Generated from `choices.json`; don't edit by hand.

## 1. Where the page is: A. Storage, after Machines

*The page*

**Today:** Nothing yet: no page shows what agentZ keeps on disk or how big it is. Settings has General, Appearance, Notifications, Agents, Usage, Skills, MCP Servers and Machines, then the projects.

**A. Storage, after Machines** (from t3code’s Settings › Storage): A Storage section of its own, the last before the projects, named as t3code names it.

## 2. Which machine it shows: A. The machine picker in the header

*The page*

**Today:** Each machine’s server keeps its own data folder. With more than one machine, the Agents page has a machine picker in its header (the machine’s icon and name in a menu), since each machine installs and runs its own agents.

**A. The machine picker in the header** (from agentZ’s Agents page): This Mac first; the picker in the header switches to another machine. Shown only with more than one machine.

## 3. The top of the page: A. The total and a bar

*The page*

**Today:** Nothing yet: no page shows what agentZ keeps on disk or how big it is. Settings has General, Appearance, Notifications, Agents, Usage, Skills, MCP Servers and Machines, then the projects.

**A. The total and a bar** (from macOS’s Storage settings): How much agentZ uses on the machine and where its data folder is, over a bar with a color for each kind, and the kinds with their sizes under it.

## 4. What the page lists: A. Chats + B. Project threads + E. Node.js and logs + D. Agents + C. Worktrees and pastures

*What’s listed · pick any*

**Today:** The data folder holds, for each thread, its conversation (transcripts), its images and uploaded files (attachments) and its handoffs; threads’ worktrees and pastures; installed agents and the registry’s cache; downloaded Node.js; and the server’s log. Agents’ logins in accounts are never changed by agentZ. Chats would add a folder for each.

**A. Chats** (from the backlog): Each chat with its size: its conversation, images and folder together.

**B. Project threads** (from agentZ’s data folder): A row for each project with its threads’ conversations and images; it opens to the threads, biggest first.

**E. Node.js and logs** (from agentZ’s data folder): Downloaded Node.js, deleted until an agent needs it again, and the server’s log, which Clear empties.

**D. Agents** (from agentZ’s data folder): Installed agents with their sizes (uninstalled on the Agents page, as today) and the registry’s cache, which Clear empties.

**C. Worktrees and pastures** (from agentZ’s data folder): Each worktree and pasture agentZ made, with its project, kind and what its thread is doing.

## 5. A chat’s row: A. One line

*What’s listed*

**Today:** Nothing yet. The sidebar’s Archived rows are one line: the icon, the title and when it last did something.

**A. One line** (from agentZ’s Archived rows): The chat icon (archived ones have the archive icon), the title, the size and when it was last used.

## 6. Project threads: A. By project, opening to threads

*What’s listed*

**Today:** Deleting a thread (its right-click menu, Delete…) removes its conversation, images and handoffs; its worktree or pasture stays. Archived threads are in the sidebar’s Archived shelf.

**A. By project, opening to threads** (from new): A row for each project with its threads’ total; opening one lists its biggest threads first, then Show more.

## 7. The order of rows: A. Biggest first

*What’s listed*

**Today:** The sidebar sorts threads by Thread order (Settings › General), newest first by default.

**A. Biggest first** (from macOS’s Storage settings): Each group’s rows by size, biggest first.

## 8. Worktrees and pastures: A. Each with its thread

*What’s listed*

**Today:** In the Workspaces view, a worktree’s right-click menu has Delete Worktree Checkout…, which deletes its folder (“Its folder … is deleted from disk, and its workspace closes.”) and asks again if it has changes. Deleting a thread keeps its worktree or pasture.

**A. Each with its thread** (from agentZ’s Workspaces view): Each one with its branch, project and kind, and what its thread is doing: working, finished, archived, or deleted.

## 9. How you delete: A. A trash button on each row

*Deleting*

**Today:** One at a time: a thread from its right-click menu in the sidebar, a worktree from its menu in the Workspaces view, an agent from the Agents page.

**A. A trash button on each row** (from agentZ’s Skills and MCP Servers rows): Hovering a row shows a trash button; it asks first.

## 10. What Delete does: A. Asks, then it’s gone

*Deleting*

**Today:** Deleting a thread asks first: “Delete ‘hello’? The thread and its conversation will be removed. This can’t be undone.” Delete Worktree Checkout… asks the same way, and again with Delete Anyway when it has changes.

**A. Asks, then it’s gone** (from agentZ’s Delete dialogs): A dialog with how many, their size and what goes with them; Delete removes them for good.

## 11. What can’t be deleted: A. Shown, with the reason

*Deleting*

**Today:** The backlog keeps these safe: a working thread’s or chat’s data, worktrees with uncommitted changes, and agents’ logins in account homes, which agentZ never changes. Workspaces’ Delete Worktree Checkout… asks again with Delete Anyway when a worktree has changes.

**A. Shown, with the reason** (from new): Listed with their size, without a checkbox, and a tag saying why: Working, or Has changes. Account homes aren’t listed.

## 12. Cleaning up on its own: A. By hand only

*Keeping it small*

**Today:** agentZ deletes nothing on its own, except the server’s log, which starts over once it passes its limit. t3code’s Storage page has only rules: where new worktrees go, deleting worktrees with deleted threads, after some days without use, once merged, or with no commits of their own (never with changes or a running session), and how many days to keep browser captures and old logs.

**A. By hand only** (from the backlog): No rules: the page shows sizes and you delete.

## 13. When sizes are measured: C. Kept current by the server

*Keeping it small*

**Today:** Nothing measures agentZ’s data today. A worktree of a big project can hold hundreds of thousands of files, so measuring it takes a few seconds.

**C. Kept current by the server** (from new): Each server measures in the background as threads change, so the page opens with current sizes.
