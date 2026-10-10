# Chats: design decisions

Picked on the design board (`design/chats/`, see `design/README.md`). Each section is the spec for that part: build the picked option as described, with the comments applied. Generated from `choices.json`; don't edit by hand.

## 1. Where the Chats group sits: Not decided yet

*Sidebar*

**Today:** The sidebar lists thread cards, every one in a project (the project’s icon and name, its state, the title, its branch and machine), and the Shells, Workspaces and Archived shelves rest at the bottom: a label with its count, a rule and a chevron. Threads started in Workspaces panes are the only ones outside the cards today.

## 2. What a chat’s row shows: Not decided yet

*Sidebar*

**Today:** Archived and Workspaces rows are one line: the project’s icon, the title and when it last did something, muted. Thread cards have three lines with the state.

## 3. Open or closed: Not decided yet

*Sidebar*

**Today:** Shells starts open each time the app starts. Workspaces and Archived remember whether you left them open (kept by this Mac’s server) and start closed; a closed shelf shows its count. Archived shows its rows a page at a time, with Show more.

## 4. Chats in search: Not decided yet

*Sidebar*

**Today:** Typing in Search lists matching threads as one line each: the project’s icon, the title, Archived for archived ones, and the time, newest first.

## 5. How a chat is started: Not decided yet

*Starting · pick any*

**Today:** New Thread (the + at the top of the sidebar, or Ctrl-N) opens New thread in…, a list of projects to pick from, when several are shown; with one it opens a draft there. Only a Workspaces pane starts a thread outside a project.

## 6. The new chat screen: Not decided yet

*Starting*

**Today:** A new thread’s screen: “What should we work on?”, the composer card with the agent, and a strip under it with the checkout (Local, New worktree, New pasture), the machine and account when there are several, and the branch at the right end.

## 7. A chat’s header: Not decided yet

*In a chat*

**Today:** A thread’s header has the project’s icon and name, the title with its menu, then the branch, Diff, Terminal and More. The backlog hides worktrees, pastures, diffs, branches and project scripts in a chat.

## 8. The chat’s folder: Not decided yet

*In a chat*

**Today:** A thread works in its project’s folder, worktree or pasture, and a Workspaces thread in its pane’s folder. A chat needs a folder too (ACP’s session/new takes one), so each would get its own in the data folder, named as t3code’s are: the date, the first words of its first message and a short id.

## 9. The @ menu in a chat: Not decided yet

*In a chat*

**Today:** In a thread, @ lists Files (in the thread’s folder) and Threads (the project’s other threads), up to eight and five. A mentioned file goes as a link, and a mentioned thread as its conversation. A chat has no project, so the menu needs projects and every project’s threads.

## 10. Projects and threads on other machines: Not decided yet

*In a chat*

**Today:** A mentioned thread’s conversation is read by the server the message goes to, so only that machine’s threads can be mentioned. A chat runs on one machine (This Mac here), while api is on Devbox 1 and storefront on both.

## 11. agentZ’s tools in a chat: Not decided yet

*In a chat*

**Today:** A thread’s agent gets agentZ’s tools (threads, delegated tasks, workspaces, terminals, commands, add project). The thread tools work only in the caller’s project; a Workspaces thread’s work in the folder it’s in. A chat has no project.

## 12. Archiving and deleting a chat: Not decided yet

*In a chat*

**Today:** A thread’s right-click menu has Pin, Rename, Archive, Project Settings and Delete…; Delete asks first: “The thread and its conversation will be removed. This can’t be undone.” Archived threads sit in the Archived shelf.

## 13. Where the setting is: Not decided yet

*Turning chats off*

**Today:** Settings › General has Threads (Thread order, Use modifier to send, Show thinking), Thread titles, Projects and Server groups of rows, each with a switch or a menu.

## 14. What happens to chats while it’s off: Not decided yet

*Turning chats off*

**Today:** Nothing yet: chats don’t exist. The backlog leaves open what happens to existing chats while the setting is off.
