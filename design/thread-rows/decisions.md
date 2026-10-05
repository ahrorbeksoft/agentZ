# Thread rows: design decisions

Picked on the design board (`design/thread-rows/`, see `design/README.md`). Each section is the spec for that part: build the picked option as described, with the comments applied. Generated from `choices.json`; don't edit by hand.

## 1. A row’s text: B. One dim gray, the command in the code font

*Rows*

**Today:** A row says what the tool call did: "Ran" in the theme’s muted gray, then the command in the code font in the text color, the same color as the agent’s messages. In JetBrains Dark the muted gray (#b0b1b3) is close to the text color (#dfe1e5), so the whole row is nearly as bright as a message.

**B. One dim gray, the command in the code font** (from t3code’s gray, today’s code font): The same dim gray for the whole row, but the command keeps the code font, so it still reads as code.

## 2. Between messages: D. Folded once the turn ends

*Rows*

**Today:** Rows sit right under and above the agent’s messages, with no space and nothing around them, so where a message ends and the work starts is unclear.

**D. Folded once the turn ends** (from t3code work groups): While the agent works, rows show as they come. When the turn ends, each run folds into one line that says what it did ("Ran 5 commands"), opening to its rows on click. The thread round picked rows always open; this is t3code’s default.

**Note:** Changed after the build (2026-10-05): a run folds as soon as the agent writes a message after it, also while the turn goes on; the turn's last run folds when it ends.
