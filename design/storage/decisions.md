# Storage: design decisions

Picked on the design board (`design/storage/`, see `design/README.md`). Each section is the spec for that part: build the picked option as described, with the comments applied. Generated from `choices.json`; don't edit by hand.

## 1. Where the page is: Not decided yet

*The page*

**Today:** Nothing yet: no page shows what agentZ keeps on disk or how big it is. Settings has General, Appearance, Notifications, Agents, Usage, Skills, MCP Servers and Machines, then the projects.

## 2. Which machine it shows: Not decided yet

*The page*

**Today:** Each machine’s server keeps its own data folder. With more than one machine, the Agents page has a machine picker in its header (the machine’s icon and name in a menu), since each machine installs and runs its own agents.

## 3. The top of the page: Not decided yet

*The page*

**Today:** Nothing yet: no page shows what agentZ keeps on disk or how big it is. Settings has General, Appearance, Notifications, Agents, Usage, Skills, MCP Servers and Machines, then the projects.

## 4. What the page lists: Not decided yet

*What’s listed · pick any*

**Today:** The data folder holds, for each thread, its conversation (transcripts), its images and uploaded files (attachments) and its handoffs; threads’ worktrees and pastures; installed agents and the registry’s cache; downloaded Node.js; and the server’s log. Agents’ logins in accounts are never changed by agentZ. Chats would add a folder for each.

## 5. A chat’s row: Not decided yet

*What’s listed*

**Today:** Nothing yet. The sidebar’s Archived rows are one line: the icon, the title and when it last did something.

## 6. Project threads: Not decided yet

*What’s listed*

**Today:** Deleting a thread (its right-click menu, Delete…) removes its conversation, images and handoffs; its worktree or pasture stays. Archived threads are in the sidebar’s Archived shelf.

## 7. The order of rows: Not decided yet

*What’s listed*

**Today:** The sidebar sorts threads by Thread order (Settings › General), newest first by default.

## 8. Worktrees and pastures: Not decided yet

*What’s listed*

**Today:** In the Workspaces view, a worktree’s right-click menu has Delete Worktree Checkout…, which deletes its folder (“Its folder … is deleted from disk, and its workspace closes.”) and asks again if it has changes. Deleting a thread keeps its worktree or pasture.

## 9. How you delete: Not decided yet

*Deleting*

**Today:** One at a time: a thread from its right-click menu in the sidebar, a worktree from its menu in the Workspaces view, an agent from the Agents page.

## 10. What Delete does: Not decided yet

*Deleting*

**Today:** Deleting a thread asks first: “Delete ‘hello’? The thread and its conversation will be removed. This can’t be undone.” Delete Worktree Checkout… asks the same way, and again with Delete Anyway when it has changes.

## 11. What can’t be deleted: Not decided yet

*Deleting*

**Today:** The backlog keeps these safe: a working thread’s or chat’s data, worktrees with uncommitted changes, and agents’ logins in account homes, which agentZ never changes. Workspaces’ Delete Worktree Checkout… asks again with Delete Anyway when a worktree has changes.

## 12. Cleaning up on its own: Not decided yet

*Keeping it small*

**Today:** agentZ deletes nothing on its own, except the server’s log, which starts over once it passes its limit. t3code’s Storage page has only rules: where new worktrees go, deleting worktrees with deleted threads, after some days without use, once merged, or with no commits of their own (never with changes or a running session), and how many days to keep browser captures and old logs.

## 13. When sizes are measured: Not decided yet

*Keeping it small*

**Today:** Nothing measures agentZ’s data today. A worktree of a big project can hold hundreds of thousands of files, so measuring it takes a few seconds.
