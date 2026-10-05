# Thread: design decisions

Picked on the design board (`design/thread/`, see `design/README.md`). Each section is the spec for that part: build the picked option as described, with the comments applied. Generated from `choices.json`; don't edit by hand.

## 1. Tool calls between messages: B. Compact rows, always open

*The agent's work*

**Today:** Each tool call is its own row, and edits and commands are cards with their diff or command, so a long turn is mostly tool calls and the answer is far down.

**B. Compact rows, always open** (from t3code rows, ungrouped): Every tool call is t3code's compact one-line row, always shown, with no folding line. Edits and commands are rows too, not cards.

## 2. A tool call, opened: A. The row opens to its output

*The agent's work*

**Today:** A command is a card with its command; its output shows in the card. Reads and searches are rows that open to their text.

**A. The row opens to its output** (from t3code WorkLogDetails): A click on the row opens its output under it, indented past the icon, in the code font, at most 24rem tall then scrolling. The row says what ran; the output is only there when asked for.

## 3. Edits: A. A row with its line counts, opening to the diff

*The agent's work*

**Today:** An edit is a card that always shows its diff.

**A. A row with its line counts, opening to the diff** (from t3code): "Edited cart/total.ts +4 −2" as a compact row; a click opens the diff under it, the same diff as the card has today.

## 4. Thinking: A. "Thinking" while it thinks, then a Thought row

*The agent's work*

**Today:** A "Thinking" row that opens to the agent's thoughts, shown while they stream.

**A. "Thinking" while it thinks, then a Thought row** (from t3code): While the agent thinks, one "Thinking" line with a shimmer. After, a "Thought" row among the work rows (with how long), opening to the text.

**Note:** Changed after the build (2026-10-05): "Thinking" shimmers by default, and a setting, Show thinking, shows the thinking block open.

## 5. Your messages: A. A bubble on the right

*Messages*

**Today:** A full-width box with a border, like the composer.

**A. A bubble on the right** (from t3code): Your message in a rounded bubble with a soft background, on the right, at most about four fifths wide; its time and copy show on hover under it.

## 6. While it works, and after: A. "Working for 1m 12s", then the turn's time

*Messages*

**Today:** While it works, a spinner and the elapsed time at the end. After, copy, scroll-to-message and scroll-to-top buttons under the answer.

**A. "Working for 1m 12s", then the turn's time** (from t3code): While it works, "Working for 1m 12s" at the end of the thread. After, a quiet line under the answer with how long the turn took and Copy.

**Comment on A:** worked for not working for

## 7. Steering: C. Both: Steer, and Send Now

*Queued messages*

**Today:** A message typed while the agent works waits in the queue above the composer, sent when the turn ends. Send Now cancels the turn and sends it; you can also edit or remove it. Over ACP an agent takes one message at a time, so a message can't join a running turn: Zed steers only its own agent, and t3code steers ACP agents by cancelling and resending.

**C. Both: Steer, and Send Now** (from Zed): Steer waits for the current step (A); Send Now cuts in at once, as today.
