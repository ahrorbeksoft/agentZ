# Chats as Threads, the Working Line and the Agent Picker: design decisions

Picked on the design board (`design/chats-2/`, see `design/README.md`). Each section is the spec for that part: build the picked option as described, with the comments applied. Generated from `choices.json`; don't edit by hand.

## 1. A chat’s card among the threads: Not decided yet

*Chats*

**Today:** Thread cards fill the list, each in a project: the project’s icon and name with the state, the title, then the branch with the machine and agent icons. Chats are their own Chats shelf at the bottom, above Archived, with a + for New Chat: one line each (the chat icon, the title, a dot while it works or waits for you, the time), the five latest, then Show more. In every option the shelf goes and chats are cards among the threads, in the thread order (here Newest first).

## 2. Chats in the project picker: Not decided yet

*Chats*

**Today:** The title bar’s button (“All projects ⌄”) opens the project picker: Search projects…, All projects with the count and a check on the picked one, then each machine’s projects (with other machines, under their names), each with its state’s dot and a gear for Project Settings, then Add Project…. The picked project’s threads are the only cards in the sidebar. Chats aren’t in it. In every option, picking Chats shows only chats, and the button reads “Chats” with the chat icon.

## 3. The sidebar with Chats picked: Not decided yet

*Chats*

**Today:** With one project picked, its cards drop the project’s line: the title with the state first, then the branch and icons. The + (New Thread, Cmd-N) starts a thread in that project, and Archived lists that project’s. Nothing shows only chats today. In every option, the + and Cmd-N start a chat while Chats is picked, and Archived lists only chats. The mocks use A of the first topic.

## 4. Archiving a chat: Not decided yet

*Chats*

**Today:** Chats can’t be archived, as you asked in the Chats round (“and not archivable”): their menu is Pin, Rename and Delete…, and auto-archive skips them. A thread’s card shows Snooze and Archive on hover, its menu has Archive, and Settings › Threads › Auto-archive threads (Never by default) archives the ones nobody used for that long. Archived threads are rows on the Archived shelf. The mocks hover “Async runtimes compared”.

## 5. The Chats setting: Not decided yet

*Chats*

**Today:** Settings › Threads ends with a Chats group: one switch, on by default, “Threads for general conversation, outside every project, in their own group in the sidebar.” While it’s off, the Chats shelf, New Chat, Chat in the pickers and chats in search and Go To are gone; chats are kept.

## 6. A thought without the shimmer: Not decided yet

*The Working line*

**Today:** A thought is a row among the tool calls: a light bulb and “Thinking”, with a band of the text color sweeping across the row’s gray every 2.2 seconds while the agent thinks, then “Thought”. A click opens the text. Settings › Threads › Show thinking (off by default) opens every thought. In every option Show thinking goes, thoughts start closed as they do today with it off, and a click still opens one.

## 7. The shimmer on the Working line: Not decided yet

*The Working line*

**Today:** The line the backlog calls “Running for” is the Working line at the end of a turn: the turn’s dots spinner, “Working for 1m 12s” in small muted text, then what goes on with it (“· 2 subthreads running”, “· 1 shell”, which opens the shells). Only the spinner moves. The mocks use A of the topic before for the thought, and are animated as the app would draw them.

## 8. While the agent waits for you: Not decided yet

*The Working line*

**Today:** While a permission request waits for you, the Working line’s spinner is a sand timer and the line starts with “Awaiting Confirmation”, before “Working for 1m 12s”. The mocks use A of the topic before.

## 9. Accounts inside the agent picker: Not decided yet

*The agent picker*

**Today:** In the composer, “Claude Agent ⌄” opens the agent picker: the installed agents, each with the working dot while one of its threads works, what’s left of the account a switch would use (“69% left”) and a check on this thread’s, then Manage Agents…. Beside it, with two or more accounts, “Personal ⌄” opens the account picker: This thread, then Switch to, each account’s avatar, name, plan, working dot and what’s left, then Manage Accounts…. The new thread screen has the same two. In the mocks only Claude Agent has several accounts: Personal (this thread’s), Work and the External account. In every option the account chip goes from the composer (its look is the last topic), and an agent with one account picks as today. The mocks use B of the next topic for the accounts.

## 10. The account submenu’s rows: Not decided yet

*The agent picker*

**Today:** In the composer, “Claude Agent ⌄” opens the agent picker: the installed agents, each with the working dot while one of its threads works, what’s left of the account a switch would use (“69% left”) and a check on this thread’s, then Manage Agents…. Beside it, with two or more accounts, “Personal ⌄” opens the account picker: This thread, then Switch to, each account’s avatar, name, plan, working dot and what’s left, then Manage Accounts…. The new thread screen has the same two. In the mocks only Claude Agent has several accounts: Personal (this thread’s), Work and the External account. The mocks open the submenu of A in the topic before.

## 11. The agent in the composer: Not decided yet

*The agent picker*

**Today:** The composer shows the agent’s icon as it is in “Claude Agent ⌄”, then the account’s chip (its avatar, “Personal ⌄”) and the usage gauge (“69%”). Thread cards, their details, Go To and the Workspaces view already draw the agent’s icon white on a rounded square in the thread’s account’s color while the agent has more than one account. In every option the new thread screen’s chip does the same, and narrow, the chip is the icon alone.
