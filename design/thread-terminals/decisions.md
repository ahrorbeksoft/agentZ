# Thread terminals: design decisions

Picked on the design board (`design/thread-terminals/`, see `design/README.md`). Each section is the spec for that part: build the picked option as described, with the comments applied. Generated from `choices.json`; don't edit by hand.

## 1. One shell, tabs or splits: B. Several, as tabs

*Shells*

**Today:** A thread’s terminal starts with one shell. + adds a shell in a group of its own, and each split button adds one beside or under the shell in front, up to four in a group. One group shows at a time. With two shells or more, a list 144 pixels wide on the right names the groups (Single, Side by side, Stacked) and the shells (Terminal 1, Terminal 2), and the toolbar moves into it. The other topics depend on this pick.

**B. Several, as tabs** (from Zed, without splits): + adds a shell as a tab in a strip above the terminal. One shows at a time, at full width. Each tab is named by what runs in it (zsh, npm run dev), as Workspaces panes are, and has its own ×.

## 2. Which buttons stay: B. New, Full Screen and Close

*Buttons*

**Today:** Five buttons, in this order: Split Terminal Horizontally (side by side), Split Terminal Vertically (top and bottom), New Terminal, Full Screen, and Close Terminal. The split buttons turn gray once a group has four shells. The terminal button in the thread’s header shows and hides the terminal; it isn’t part of this toolbar. These mocks keep the buttons where they are today, so only the set changes; where they go is the next topic. With one shell (A in the first topic), New goes too.

**B. New, Full Screen and Close** (from t3code, without splits): The two split buttons go. With tabs (B in the first topic), × sits on each tab instead, leaving + and Full Screen here.

## 3. Where the buttons go: B. A strip above the shell

*Buttons*

**Today:** The toolbar floats over the shell’s top right corner in a bordered box, whatever the shell prints there. With two shells or more, it moves into a thin strip at the top of the list on the right. The mocks show one shell and the three buttons of the previous topic’s recommendation; they’ll be the ones you pick.

**B. A strip above the shell** (from Zed): A strip 30 pixels tall across the top of the terminal, as Zed’s terminal panel has: the shell’s name on the left (its tabs, if there are several), the buttons on the right.

## 4. Hiding and closing: B. Ask first while something runs

*Closing*

**Today:** The header’s terminal button and Cmd-J hide the terminal and keep its shells running; a dot on the button says something still runs. × ends the shell in front at once, even while a program runs in it. Typing exit ends a shell too. When the last shell ends, the terminal hides, and the next Cmd-J starts a new shell. Closing a Workspaces pane or a terminal thread asks first while something runs in it. The mocks show two tabs, as the first topic recommends.

**B. Ask first while something runs** (from agentZ Workspaces panes): An idle shell’s × ends it at once. While a program runs in it, × first asks the question closing a Workspaces pane asks, with Close and Cancel.

## 5. Where the terminal sits: A. Under the composer, as today

*The terminal*

**Today:** At the bottom of the thread, under the composer, as wide as the conversation (an open Changes panel stays to its right). It opens 280 pixels tall. The mocks show the recommended strip with one shell.

**A. Under the composer, as today** (from t3code): The conversation, then the composer, then the terminal along the bottom.

## 6. Resizing and full screen: A. Drag, and a Full Screen button

*The terminal*

**Today:** The top edge drags, from 100 pixels up to where the conversation keeps 160. Each open thread has its own height, back to 280 when the app restarts. Full Screen hides the conversation and the composer until you press it again. Workspaces panes fill their tab with Zoom In (Cmd-Shift-Enter). This goes with the buttons topic: its B keeps the Full Screen button, and C and E drop it.

**A. Drag, and a Full Screen button** (from agentZ today): As today: drag the top edge, or press Full Screen to fill the thread and press it again to come back.
