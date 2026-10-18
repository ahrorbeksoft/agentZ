# Search in a Thread: design decisions

Picked on the design board (`design/thread-search/`, see `design/README.md`). Each section is the spec for that part: build the picked option as described, with the comments applied. Generated from `choices.json`; don't edit by hand.

## 1. The find bar: B. A strip under the header

*Find*

**Today:** There’s no way to search a thread’s conversation. ⌘F (Ctrl-Shift-F on Linux) opens find only in a focused terminal: a strip above it with a text field, ‹ › (“Select Previous Match”, “Select Next Match”), the count (“2/3”, “0/0”, the field turning red when nothing matches) and × (“Close Search Bar”). In a thread ⌘F does nothing. The pictures are recreated from the code.

**B. A strip under the header** (from Zed): Zed’s thread search: a full-width strip between the header and the conversation. “Search this thread…”, with Case Sensitive (Aa), Whole Word (ab) and Regex (.*) inside the box, then ‹ › and the count, and ×.

## 2. What it searches: A. Your messages + B. The agent’s messages + C. Plans + E. Tool calls’ titles

*Find · pick any*

**Today:** Nothing is searched. A conversation holds your messages, the agent’s messages (Markdown), its thoughts and tool calls (folded into one line per run once a message follows them), plans it proposes (cards in the conversation) and ACP plan entries (the plan section above the composer, folded by default).

**A. Your messages** (from t3code): The text of your messages, as shown (attachments’ names aside).

**B. The agent’s messages** (from t3code): The agent’s replies as rendered text, code blocks included (not the Markdown’s symbols).

**C. Plans** (from t3code): Plans the agent proposes, and the steps in the plan section above the composer.

**E. Tool calls’ titles** (from Zed): Each tool call’s line: “Read src/app/checkout/page.tsx”, “Searched for “promo””, the command it ran.

## 3. A match inside something folded: A. Open it and scroll to it

*Find*

**Today:** Runs of tool calls and thoughts fold into one line (“Read 3 files and searched code 1 time”) once a message follows them, and the plan section is folded. The conversation follows its end while the agent works. The picture is recreated from the code.

**A. Open it and scroll to it** (from t3code): t3code: going to a match opens what holds it (a folded run, the plan section, a long message) and scrolls so the match sits about 96 px below the top. While find is open, the conversation stops following the end; new text is searched as it streams in, keeping the current match.

## 4. How matches look: A. The theme’s search colors

*Find*

**Today:** agentZ’s Markdown already paints search matches (Zed’s, used for nothing yet) in the theme’s colors: One Dark’s are blue for matches and orange for the current one, both see-through. The terminal’s find uses the same colors.

**A. The theme’s search colors** (from Zed): Every match in the theme’s search color, the current one in its active color (One Dark: blue, orange), as Zed and agentZ’s terminal do.

## 5. Opening it and moving between matches: A. ⌘F, Enter, Esc + B. ⌘G and ⌘⇧G + C. In the command palette

*Find · pick any*

**Today:** ⌘F and Ctrl-Shift-F are bound only in the terminal. Nothing in the command palette or the thread’s menus finds text.

**A. ⌘F, Enter, Esc** (from t3code): t3code’s keys: ⌘F (Ctrl-F on Linux) opens find unless a terminal is focused, which keeps its own; pressed again it selects the query. Enter and ⇧Enter go to the next and previous match. Esc closes it and puts the cursor back in the composer.

**B. ⌘G and ⌘⇧G** (from Zed): Zed’s ⌘G and ⌘⇧G go to the next and previous match from anywhere in the thread, even with the cursor in the composer.

**C. In the command palette** (from t3code): “Find in Thread” in the command palette, found by “find”, “search”, “messages” or “plans”, as t3code’s “Find in current thread”.

**Note:** ⌘F opens it, and it can be in the command palette too.

## 6. Switching threads: A. It closes

*Find*

**Today:** There is no find in threads. Each pane of the Workspaces view and each tab can show a different thread.

**A. It closes** (from t3code): t3code: find belongs to the thread it was opened in and closes when you go to another.

## 7. The sidebar’s search: A. Titles only, as today

*Every thread*

**Today:** The search field in the sidebar’s head matches thread titles only, ignoring case; ↑ ↓ move, Enter opens, Esc clears. The picture is recreated from the code.

**A. Titles only, as today** (from backlog): The backlog asks for search inside the open thread. The sidebar keeps matching titles.
