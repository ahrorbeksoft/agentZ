# Draft thread cards: design decisions

Picked on the design board (`design/draft-cards/`, see `design/README.md`). Each section is the spec for that part: build the picked option as described, with the comments applied. Generated from `choices.json`; don't edit by hand.

## 1. What marks a card with unsent text: Not decided yet

*Cards*

**Today:** A card whose thread has unsent text gets a small yellow pen before its project icon (tooltip “Unsent draft”), and a Discard draft × beside Archive on hover. The open thread’s card has none. Here Write tests for the rate limiter and Speed up the product grid have unsent text.

## 2. Where the mark goes: Not decided yet

*Cards*

**Today:** The pen comes first on the project line, before the project icon. With one project selected, which drops the project line, it comes first on the title line. This topic applies if the first topic’s pick is a pen or the Draft label; the mocks draw the first topic’s B (a gray pen).

## 3. New threads with nothing sent: Not decided yet

*Draft rows*

**Today:** A new thread with text typed and nothing sent is a draft row above the cards: the yellow pen, its project, and the first line of the text in place of a title, on a faint yellow tint, with a faint line under the rows. Discard draft shows on hover. Here Add a dark mode toggle to the header is one; the cards below it have no unsent text.
