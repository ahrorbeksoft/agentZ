# Composer: design decisions

Picked on the design board (`design/composer/`, see `design/README.md`). Each section is the spec for that part: build the picked option as described, with the comments applied. Generated from `choices.json`; don't edit by hand.

## 1. More than one line: Not decided yet

*Composer*

**Today:** One line. Text past the edge runs off it without scrolling, a pasted text's line breaks turn into spaces, and there's no way to type a new line. In every option Shift-Enter makes a new line, Enter sends, pasted text keeps its line breaks, and Up/Down move between lines (Zed's message editor). This topic is only how tall the box is.

## 2. What @ can add: Not decided yet

*@-mentions · pick any*

**Today:** Nothing: @ is just a character. Zed's @ adds files, folders, symbols, threads, rules and fetched pages; t3code's adds files and folders. agentZ has no language servers, so no symbols. Pick any.

## 3. The @ menu: Not decided yet

*@-mentions*

**Today:** Typing / opens the slash-command menu: above the composer's left edge, 26rem wide, a name and a description per row; Up/Down, Tab or Enter to pick, Escape to close. Nothing opens for @.

## 4. A mention in the text: Not decided yet

*@-mentions*

**Today:** There are no mentions yet. Once picked, a mention stays one piece in the text: Backspace removes it whole, and the agent gets the file or thread beside the message.

## 5. Adding without typing @: Not decided yet

*@-mentions*

**Today:** The composer's footer starts with the agent's icon and name.

## 6. A pasted image: Not decided yet

*Images*

**Today:** Pasting an image does nothing. Every option sends images only to agents that say they take them (ACP's prompt capabilities); for the others, pasting an image does nothing, as in Zed. Dropping an image file works like pasting it.

## 7. Right-clicking the composer: Not decided yet

*Composer*

**Today:** Nothing opens. Cmd-X, Cmd-C, Cmd-V and Cmd-A are the only way to cut, copy and paste.
