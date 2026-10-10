# Chats: design decisions

Picked on the design board (`design/chats/`, see `design/README.md`). Each section is the spec for that part: build the picked option as described, with the comments applied. Generated from `choices.json`; don't edit by hand.

## 1. Where the Chats group sits: A. The first shelf at the bottom

*Sidebar*

**Today:** The sidebar lists thread cards, every one in a project (the project’s icon and name, its state, the title, its branch and machine), and the Shells, Workspaces and Archived shelves rest at the bottom: a label with its count, a rule and a chevron. Threads started in Workspaces panes are the only ones outside the cards today.

**A. The first shelf at the bottom** (from agentZ’s shelves (t3code’s Settled shelf)): A Chats shelf above Shells, Workspaces and Archived, styled like them, with a + on its header for a new chat. Like them it rests at the bottom while the list is short and scrolls with the cards.

## 2. What a chat’s row shows: B. One line with its state

*Sidebar*

**Today:** Archived and Workspaces rows are one line: the project’s icon, the title and when it last did something, muted. Thread cards have three lines with the state.

**B. One line with its state** (from agentZ’s Archived rows, with the cards’ state dots): Like A, but a chat that’s working or waiting for you reads at full strength with its state’s dot before the time.

## 3. Open or closed: C. Open, the latest few

*Sidebar*

**Today:** Shells starts open each time the app starts. Workspaces and Archived remember whether you left them open (kept by this Mac’s server) and start closed; a closed shelf shows its count. Archived shows its rows a page at a time, with Show more.

**C. Open, the latest few** (from agentZ’s Archived pages): Remembered like A and B, but open it shows the five latest chats and Show 12 more, as Archived pages its rows.

## 4. Chats in search: A. Among the threads, with the chat icon

*Sidebar*

**Today:** Typing in Search lists matching threads as one line each: the project’s icon, the title, Archived for archived ones, and the time, newest first.

**A. Among the threads, with the chat icon** (from agentZ’s search): Chats are results like threads, in the same order, with the chat icon where a project’s icon is.

## 5. How a chat is started: B. Chat first in New thread in… + C. A New Chat command and shortcut + D. The headline’s project is a menu

*Starting · pick any*

**Today:** New Thread (the + at the top of the sidebar, or Ctrl-N) opens New thread in…, a list of projects to pick from, when several are shown; with one it opens a draft there. Only a Workspaces pane starts a thread outside a project.

**B. Chat first in New thread in…** (from t3code (No project)): The project picker New Thread opens lists Chat first, above the projects: Ctrl-N and Enter start a chat.

**C. A New Chat command and shortcut** (from t3code (mod+alt+n)): New Chat in the command palette and on Ctrl-Alt-N (Cmd-Alt-N on the Mac).

**D. The headline’s project is a menu** (from t3code (DraftHeroHeadline)): A new thread’s headline names its project, “What should we build in storefront?”, and the name opens a menu with Chat, the projects and Add Project…. Under it, “or start a chat”.

## 6. The new chat screen: B. Its own headline

*Starting*

**Today:** A new thread’s screen: “What should we work on?”, the composer card with the agent, and a strip under it with the checkout (Local, New worktree, New pasture), the machine and account when there are several, and the branch at the right end.

**B. Its own headline** (from new): Like A, with “What do you want to talk about?” as the headline.

## 7. A chat’s header: A. Chat and the title, More only

*In a chat*

**Today:** A thread’s header has the project’s icon and name, the title with its menu, then the branch, Diff, Terminal and More. The backlog hides worktrees, pastures, diffs, branches and project scripts in a chat.

**A. Chat and the title, More only** (from t3code (hidden branch and diff)): Chat where the project is, then the title. Only More stays at the right.

## 8. The chat’s folder: A. Not shown

*In a chat*

**Today:** A thread works in its project’s folder, worktree or pasture, and a Workspaces thread in its pane’s folder. A chat needs a folder too (ACP’s session/new takes one), so each would get its own in the data folder, named as t3code’s are: the date, the first words of its first message and a short id.

**A. Not shown** (from t3code): The folder is made when the first message is sent and never shown. The agent’s files are there for the agent; Storage settings show its size.

## 9. The @ menu in a chat: A. Projects, Threads and Files

*In a chat*

**Today:** In a thread, @ lists Files (in the thread’s folder) and Threads (the project’s other threads), up to eight and five. A mentioned file goes as a link, and a mentioned thread as its conversation. A chat has no project, so the menu needs projects and every project’s threads.

**A. Projects, Threads and Files** (from agentZ’s @ menu (t3code’s groups)): Three groups: Projects (a project goes as its folder), Threads from every project with the project’s name, and Files in the chat’s own folder. Typing filters them all.

## 10. Projects and threads on other machines: C. Every machine’s

*In a chat*

**Today:** A mentioned thread’s conversation is read by the server the message goes to, so only that machine’s threads can be mentioned. A chat runs on one machine (This Mac here), while api is on Devbox 1 and storefront on both.

**C. Every machine’s** (from new (agentZ’s cross-machine tools)): Everything is offered. A thread on another machine goes as its conversation, fetched by the app; a project there goes as its name, machine and path, which the agent reaches through agentZ’s tools.

## 11. agentZ’s tools in a chat: B. Read anywhere

*In a chat*

**Today:** A thread’s agent gets agentZ’s tools (threads, delegated tasks, workspaces, terminals, commands, add project). The thread tools work only in the caller’s project; a Workspaces thread’s work in the folder it’s in. A chat has no project.

**B. Read anywhere** (from new): Like A, plus listing and reading every project’s threads, diffs and workspaces. Nothing that starts or changes a thread.

## 12. Archiving and deleting a chat: A. Like threads; Delete takes the folder

*In a chat*

**Today:** A thread’s right-click menu has Pin, Rename, Archive, Project Settings and Delete…; Delete asks first: “The thread and its conversation will be removed. This can’t be undone.” Archived threads sit in the Archived shelf.

**A. Like threads; Delete takes the folder** (from agentZ’s threads): Pin, Rename, Archive and Delete… (no Project Settings). Archived chats go to Archived with the chat icon. Delete removes the chat with its folder and attachments.

**Comment on A:** and not archivable

## 13. Where the setting is: A. A Chats group in General

*Turning chats off*

**Today:** Settings › General has Threads (Thread order, Use modifier to send, Show thinking), Thread titles, Projects and Server groups of rows, each with a switch or a menu.

**A. A Chats group in General** (from agentZ’s General page): Its own group after Threads, with one row and a switch, on by default.

## 14. What happens to chats while it’s off: A. Hidden and kept

*Turning chats off*

**Today:** Nothing yet: chats don’t exist. The backlog leaves open what happens to existing chats while the setting is off.

**A. Hidden and kept** (from new): The Chats group, New Chat and search leave them out; their data stays. Turning chats back on brings them back as they were. A chat that’s working finishes its turn.
