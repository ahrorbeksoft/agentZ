# Composer: design decisions

Picked on the design board (`design/composer/`, see `design/README.md`). Each section is the spec for that part: build the picked option as described, with the comments applied. Generated from `choices.json`; don't edit by hand.

## 1. More than one line: D. One line, grows to eight

*Composer*

**Today:** One line. Text past the edge runs off it without scrolling, a pasted text's line breaks turn into spaces, and there's no way to type a new line. In every option Shift-Enter makes a new line, Enter sends, pasted text keeps its line breaks, and Up/Down move between lines (Zed's message editor). This topic is only how tall the box is.

**D. One line, grows to eight** (from new): Like B without the Expand button.

**Comment on D:** the lines grows to eight

## 2. What @ can add: A. Files and folders + B. Threads

*@-mentions · pick any*

**Today:** Nothing: @ is just a character. Zed's @ adds files, folders, symbols, threads, rules and fetched pages; t3code's adds files and folders. agentZ has no language servers, so no symbols. Pick any.

**A. Files and folders** (from Zed, t3code): Files and folders of the folder the thread works in, found by name on its machine (local or SSH). A file goes to the agent as its contents (ACP embedded context) or as a link when the agent doesn't take those; a folder as a link.

**B. Threads** (from Zed): Another thread of the project: its conversation goes along as a transcript, as Continue with another agent sends one.

## 3. The @ menu: B. Matches right away, grouped

*@-mentions*

**Today:** Typing / opens the slash-command menu: above the composer's left edge, 26rem wide, a name and a description per row; Up/Down, Tab or Enter to pick, Escape to close. Nothing opens for @.

**B. Matches right away, grouped** (from t3code composer menu): @ opens a list of matches at once, grouped under Files and Threads, the path dimmed beside each name. No kinds to pick first.

## 4. A mention in the text: A. Outlined chip

*@-mentions*

**Today:** There are no mentions yet. Once picked, a mention stays one piece in the text: Backspace removes it whole, and the agent gets the file or thread beside the message.

**A. Outlined chip** (from Zed MentionCrease): An outlined chip with the kind's icon and the name in the code font, a line tall. Hovering shows the full path.

## 5. Adding without typing @: A. A + button with a menu

*@-mentions*

**Today:** The composer's footer starts with the agent's icon and name.

**A. A + button with a menu** (from Zed Add Context): A + button at the start of the footer (tooltip "Add Context") opens Files & Directories, Threads and Image. The first two type @ and narrow the list to that kind; Image opens a file picker. Image is greyed out when the agent can't take images.

## 6. A pasted image: A. An Image chip in the text

*Images*

**Today:** Pasting an image does nothing. Every option sends images only to agents that say they take them (ACP's prompt capabilities); for the others, pasting an image does nothing, as in Zed. Dropping an image file works like pasting it.

**A. An Image chip in the text** (from Zed message editor): Pasting puts an "Image" chip where the cursor is, like a mention (Zed's crease). Hovering shows the picture; Backspace removes it.

## 7. Right-clicking the composer: A. Cut, Copy, Paste, Paste as Plain Text

*Composer*

**Today:** Nothing opens. Cmd-X, Cmd-C, Cmd-V and Cmd-A are the only way to cut, copy and paste.

**A. Cut, Copy, Paste, Paste as Plain Text** (from Zed message editor): Zed's menu for its message editor. Copy is greyed out with nothing selected. Paste as Plain Text pastes without turning paths into mentions or images into chips.
