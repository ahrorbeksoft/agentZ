# Archived Workspace Cleanup: design decisions

Picked on the design board (`design/archive-cleanup/`, see `design/README.md`). Each section is the spec for that part: build the picked option as described, with the comments applied. Generated from `choices.json`; don't edit by hand.

## 1. What gets cleaned up: A. Archived threads’ workspaces + B. Deleted threads’ workspaces + C. Inactive threads’ workspaces + D. Merged worktrees + E. Workspaces with nothing of their own

*Rules · pick any*

**Today:** agentZ removes nothing on its own; the Storage round decided “by hand only” (`design/storage/`, topic 12), and this backlog entry asks for automatic cleanup now. Archiving a thread leaves its worktree or pasture as it is, and so does deleting it; Settings › Storage lists each one with its size and state (“Its thread was archived 9 days ago”, “Its thread was deleted”) and a trash button, except those whose thread is working or that have changes. The pictures are recreated from the code.

**A. Archived threads’ workspaces** (from t3code (settled retention)): The backlog’s rule, t3code’s “Delete settled worktrees”: a thread’s worktree or pasture is removed some time after the thread is archived (next topics for how long). Its branch and conversation stay.

**B. Deleted threads’ workspaces** (from t3code): t3code’s “Delete worktrees with deleted threads”: deleting a thread removes its workspace too, once no other thread uses it. Changes keep it, as in every rule.

**C. Inactive threads’ workspaces** (from t3code): t3code’s “Delete inactive worktrees”: removed when the thread hasn’t been active for a number of days, archived or not.

**D. Merged worktrees** (from t3code): t3code’s “Delete merged worktrees”: removed once the worktree’s branch is merged into the project’s default branch.

**E. Workspaces with nothing of their own** (from t3code): t3code’s “Delete unchanged worktrees”: removed when they have no commits beyond the default branch and no changes, whatever their thread’s state.

**Note:** Do what t3code does: every rule starts off. Inactive threads’ workspaces has its own number of days, as t3code’s “Delete inactive worktrees”: a switch, 8 days when turned on (1 to 3650), going by the thread’s last activity.

## 2. Where the setting lives: A. On the Storage page

*Rules*

**Today:** Settings › General › Threads has “Auto-archive threads” (Never, After a day, After 3 days, After a week). Settings › Storage shows what each machine’s data takes, with a machine picker in its header, and lists worktrees and pastures. Each project has its own settings page under the sections.

**A. On the Storage page** (from t3code): t3code’s Settings › Storage: a “Cleaning up” group heads the page, above the lists, for the machine picked in its header. The note under it says what’s never removed.

## 3. How long after archiving: B. A menu, like Auto-archive

*Rules*

**Today:** Auto-archive is a menu of four choices. Settings rows put a switch or a menu on the right.

**B. A menu, like Auto-archive** (from new): The same kind of menu as Auto-archive: Never, Right away, After a day, After a week, After a month.

## 4. What keeps a workspace: C. Has changes, or not pushed

*What stays*

**Today:** The Storage page won’t delete a workspace whose thread is working (tag “Working”) or that has changes (tag “Has changes”: “Uncommitted changes in 3 files”, or “Commits the project doesn’t have”); those rows have no trash button. Workspaces’ “Delete Worktree Checkout…” asks again with Delete Anyway when there are changes.

**C. Has changes, or not pushed** (from new): As A, and also kept while its branch has commits that aren’t pushed to its remote (or has no remote branch).

## 5. Workspaces several threads use: B. Never on its own

*What stays*

**Today:** Several threads can work in one folder: a fork works in its source’s folder, subthreads and delegated tasks run in their thread’s, and you can start threads in a project’s checkout. The Storage page lists the folder once.

**B. Never on its own** (from t3code): t3code’s base rule: a workspace two threads use is kept (“shared by 2 threads”); you delete it by hand.

## 6. What it tells you: A. Last run on the Storage page

*Results*

**Today:** Nothing runs, so nothing reports. Deleting by hand on the Storage page says “Couldn’t delete {name}: {reason}” when it fails.

**A. Last run on the Storage page** (from t3code): t3code’s Cleanup group: “Clean Up Now”, with “Last run 2 hours ago. 2.0 GB recovered.” under it, and a fold, “Last run: 2 removed, 2 kept”, listing each folder with its thread and the reason (“Archived for 9 days”, “Has uncommitted changes (3 files)”), and the size. A failure shows in red.

## 7. After its workspace is gone: A. Made again with the next message

*Results*

**Today:** An archived thread shows “This thread is archived” with Unarchive in place of its composer. Its folder is always there, so unarchiving just goes on.

**A. Made again with the next message** (from t3code): t3code: the branch and conversation stay, and the next turn makes the checkout again: a worktree on its branch, a pasture as a new copy of the project. Meanwhile the activity row says so. Nothing else marks it.

## 8. Folders no thread uses: A. Listed, deleted by hand

*Results*

**Today:** The Storage page lists leftover worktrees and pastures of removed projects. A folder in agentZ’s worktrees or pastures that no thread points at (a crash while creating one, a thread from an older build) isn’t listed.

**A. Listed, deleted by hand** (from t3code (#19749)): t3code reports folders no thread uses, with their size, and never removes them. Here they join the Storage list: “No thread uses this worktree”, with the trash button.
