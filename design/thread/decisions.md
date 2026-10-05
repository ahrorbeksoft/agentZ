# Thread: design decisions

Picked on the design board (`design/thread/`, see `design/README.md`). Each section is the spec for that part: build the picked option as described, with the comments applied. Generated from `choices.json`; don't edit by hand.

## 1. Tool calls between messages: Not decided yet

*The agent's work*

**Today:** Each tool call is its own row, and edits and commands are cards with their diff or command, so a long turn is mostly tool calls and the answer is far down.

## 2. A tool call, opened: Not decided yet

*The agent's work*

**Today:** A command is a card with its command; its output shows in the card. Reads and searches are rows that open to their text.

## 3. Edits: Not decided yet

*The agent's work*

**Today:** An edit is a card that always shows its diff.

## 4. Thinking: Not decided yet

*The agent's work*

**Today:** A "Thinking" row that opens to the agent's thoughts, shown while they stream.

## 5. Your messages: Not decided yet

*Messages*

**Today:** A full-width box with a border, like the composer.

## 6. While it works, and after: Not decided yet

*Messages*

**Today:** While it works, a spinner and the elapsed time at the end. After, copy, scroll-to-message and scroll-to-top buttons under the answer.

## 7. Steering: Not decided yet

*Queued messages*

**Today:** A message typed while the agent works waits in the queue above the composer, sent when the turn ends. Send Now cancels the turn and sends it; you can also edit or remove it. Over ACP an agent takes one message at a time, so a message can't join a running turn: Zed steers only its own agent, and t3code steers ACP agents by cancelling and resending.
