# Chats as Threads, the Working Line and the Agent Picker: design decisions

Picked on the design board (`design/chats-2/`, see `design/README.md`). Each section is the spec for that part: build the picked option as described, with the comments applied. Generated from `choices.json`; don't edit by hand.

## 1. A chat’s card among the threads: A. Chat in the project’s place

*Chats*

**Today:** Thread cards fill the list, each in a project: the project’s icon and name with the state, the title, then the branch with the machine and agent icons. Chats are their own Chats shelf at the bottom, above Archived, with a + for New Chat: one line each (the chat icon, the title, a dot while it works or waits for you, the time), the five latest, then Show more. In every option the shelf goes and chats are cards among the threads, in the thread order (here Newest first).

**A. Chat in the project’s place** (from new (the Chats round’s chat card)): The first line has the chat icon and “Chat” where a thread has its project, with the state. The last line has no branch, only the machine and agent icons, as a thread in a folder without git.

## 2. Chats in the project picker: A. Under All projects

*Chats*

**Today:** The title bar’s button (“All projects ⌄”) opens the project picker: Search projects…, All projects with the count and a check on the picked one, then each machine’s projects (with other machines, under their names), each with its state’s dot and a gear for Project Settings, then Add Project…. The picked project’s threads are the only cards in the sidebar. Chats aren’t in it. In every option, picking Chats shows only chats, and the button reads “Chats” with the chat icon.

**A. Under All projects** (from new (Zed’s recent projects list)): Chats comes second, under All projects and above every machine’s projects: the chat icon, “Chats”, how many (“5 chats”) and the most urgent chat’s dot. No gear. Search finds it as it finds a project.

## 3. The sidebar with Chats picked: A. Like one project’s cards

*Chats*

**Today:** With one project picked, its cards drop the project’s line: the title with the state first, then the branch and icons. The + (New Thread, Cmd-N) starts a thread in that project, and Archived lists that project’s. Nothing shows only chats today. In every option, the + and Cmd-N start a chat while Chats is picked, and Archived lists only chats. The mocks use A of the first topic.

**A. Like one project’s cards** (from agentZ (a picked project’s cards)): Chat’s line goes, as a project’s does: the title with the state, then the machine and agent icons.

## 4. Archiving a chat: A. Like threads

*Chats*

**Today:** Chats can’t be archived, as you asked in the Chats round (“and not archivable”): their menu is Pin, Rename and Delete…, and auto-archive skips them. A thread’s card shows Snooze and Archive on hover, its menu has Archive, and Settings › Threads › Auto-archive threads (Never by default) archives the ones nobody used for that long. Archived threads are rows on the Archived shelf. The mocks hover “Async runtimes compared”.

**A. Like threads** (from agentZ’s threads): Archive on hover and in the menu; auto-archive takes chats too. Archived chats are rows on the Archived shelf with the chat icon, and unarchive as threads do.

## 5. The Chats setting: A. Kept, with new words

*Chats*

**Today:** Settings › Threads ends with a Chats group: one switch, on by default, “Threads for general conversation, outside every project, in their own group in the sidebar.” While it’s off, the Chats shelf, New Chat, Chat in the pickers and chats in search and Go To are gone; chats are kept.

**A. Kept, with new words** (from agentZ’s Chats setting): The switch stays and does what it does, with the picker’s Chats entry hidden too. Its words lose the shelf: “Threads for general conversation, outside every project. They show among the threads, with Chat in place of a project.”

## 6. A thought without the shimmer: A. Thinking, held still

*The Working line*

**Today:** A thought is a row among the tool calls: a light bulb and “Thinking”, with a band of the text color sweeping across the row’s gray every 2.2 seconds while the agent thinks, then “Thought”. A click opens the text. Settings › Threads › Show thinking (off by default) opens every thought. In every option Show thinking goes, thoughts start closed as they do today with it off, and a click still opens one.

**A. Thinking, held still** (from Zed (its thinking block’s still “Thinking”)): “Thinking” in the row’s gray while the agent thinks, then “Thought”, as today without the band.

## 7. The shimmer on the Working line: A. “Working for 1m 12s” shimmers

*The Working line*

**Today:** The line the backlog calls “Running for” is the Working line at the end of a turn: the turn’s dots spinner, “Working for 1m 12s” in small muted text, then what goes on with it (“· 2 subthreads running”, “· 1 shell”, which opens the shells). Only the spinner moves. The mocks use A of the topic before for the thought, and are animated as the app would draw them.

**A. “Working for 1m 12s” shimmers** (from agentZ’s Thinking shimmer (t3code’s live-tool-shine)): Thinking’s band sweeps across “Working for 1m 12s” every 2.2 seconds. The spinner stays; the subthreads and shells after it hold still.

## 8. While the agent waits for you: B. It keeps shimmering

*The Working line*

**Today:** While a permission request waits for you, the Working line’s spinner is a sand timer and the line starts with “Awaiting Confirmation”, before “Working for 1m 12s”. The mocks use A of the topic before.

**B. It keeps shimmering** (from new): “Working for 1m 12s” shimmers as before; “Awaiting Confirmation” holds still before it.

## 9. Accounts inside the agent picker: A. A submenu on hover

*The agent picker*

**Today:** In the composer, “Claude Agent ⌄” opens the agent picker: the installed agents, each with the working dot while one of its threads works, what’s left of the account a switch would use (“69% left”) and a check on this thread’s, then Manage Agents…. Beside it, with two or more accounts, “Personal ⌄” opens the account picker: This thread, then Switch to, each account’s avatar, name, plan, working dot and what’s left, then Manage Accounts…. The new thread screen has the same two. In the mocks only Claude Agent has several accounts: Personal (this thread’s), Work and the External account. In every option the account chip goes from the composer (its look is the last topic), and an agent with one account picks as today. The mocks use B of the next topic for the accounts.

**A. A submenu on hover** (from Zed’s context menu submenus): An agent with two or more accounts ends with ›, and hovering its row (or → on it) opens its accounts beside the menu. Clicking the row itself picks the agent as today: this thread’s account for its own agent, the default for another.

## 10. The account submenu’s rows: B. One list

*The agent picker*

**Today:** In the composer, “Claude Agent ⌄” opens the agent picker: the installed agents, each with the working dot while one of its threads works, what’s left of the account a switch would use (“69% left”) and a check on this thread’s, then Manage Agents…. Beside it, with two or more accounts, “Personal ⌄” opens the account picker: This thread, then Switch to, each account’s avatar, name, plan, working dot and what’s left, then Manage Accounts…. The new thread screen has the same two. In the mocks only Claude Agent has several accounts: Personal (this thread’s), Work and the External account. The mocks open the submenu of A in the topic before.

**B. One list** (from Zed’s context menus): The accounts in their order, one line each: the avatar, the name (External marked), the working dot, what’s left and a check on the current one. Manage Accounts… at the end.

## 11. The agent in the composer: A. On the account’s color, as the cards draw it

*The agent picker*

**Today:** The composer shows the agent’s icon as it is in “Claude Agent ⌄”, then the account’s chip (its avatar, “Personal ⌄”) and the usage gauge (“69%”). Thread cards, their details, Go To and the Workspaces view already draw the agent’s icon white on a rounded square in the thread’s account’s color while the agent has more than one account. In every option the new thread screen’s chip does the same, and narrow, the chip is the icon alone.

**A. On the account’s color, as the cards draw it** (from agentZ’s thread cards (AgentIcon)): The agent’s icon white on a rounded square in the account’s color, then “Claude Agent ⌄” and the gauge. The account’s name is in the chip’s tooltip (“Claude Agent · Personal”).
