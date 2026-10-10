# Every kind of tool call: design decisions

Picked on the design board (`design/tool-calls-2/`, see `design/README.md`). Each section is the spec for that part: build the picked option as described, with the comments applied. Generated from `choices.json`; don't edit by hand.

## 1. Reads: D. Reads don’t open

*Agents’ own tools, by kind*

**Today:** A read is a row with its file’s icon and the agent’s title, the project’s folder taken out: “Read src/cart/total.ts”. Claude Agent adds the lines it asked for (“Read src/cart/total.ts (1 - 120)”), Codex says “Read file 'total.ts'”. Opened, it shows the text the agent got, as printed, in a code block: Claude Agent numbers the lines itself (a number and a tab before each), the others send the bare text. “Input” under it opens the JSON the agent passed.

**D. Reads don’t open** (from Zed (its read tool)): A read is a row only, as in Zed, where reading a file shows its path and nothing more: what it read is the file. The lines it got show dimmer after the path, and there is no chevron.

## 2. Edits and their diffs: A. Line numbers + D. A header per file, with Open

*Agents’ own tools, by kind · pick any*

**Today:** An edit is a row with its file’s icon, “Edited” and the path in the code font, and the lines added and removed at its end (+2 −2). Opened, it shows the diff with 3 lines of context: a line on top, a − or + before each changed line, the removed on red and the added on green, the context dimmer. No line numbers or syntax colors; long lines scroll sideways. An edit of several files says “Edited 3 files” and shows their diffs one after another.

**A. Line numbers** (from Zed (its editor’s diff)): The new file’s line numbers in a gutter left of the − and +, as Zed’s editor shows a diff. Removed lines have none.

**D. A header per file, with Open** (from Zed (its edit card), agentZ’s diff panel): Each file’s diff gets a header: its icon, its path, +2 −2, and “Open”, which shows the file in agentZ’s diff panel. An edit of several files is then one header per file.

## 3. Created, deleted and moved files: A. Says what happened, a new file opens as a file

*Agents’ own tools, by kind*

**Today:** A new file shows as an edit: “Edited src/cart/round.ts” (+3 −0), opening to a diff where every line is added. A delete has Zed’s file-with-a-cross icon and the agent’s title (“Delete src/cart/legacy.ts”); a move has two arrows and the agent’s title (“Move src/cart/util.ts to src/cart/round.ts”). Neither opens to anything but the agent’s text.

**A. Says what happened, a new file opens as a file** (from new): “Created src/cart/round.ts” (+3), “Deleted src/cart/legacy.ts” (−42) and “Moved src/cart/util.ts → src/cart/round.ts”. A created file opens to its lines in a file view with numbers and colors, not a diff; a deleted one to its lines, dimmer.

## 4. Searches: A. One wording, matches by file

*Agents’ own tools, by kind*

**Today:** A search is a row with the magnifying glass and the agent’s title. Claude Agent titles a grep with its command line (`grep -n "roundTotal" src`) and a glob as “Find `src` `**/*.test.ts`”; Codex says “Search for 'roundTotal' in src” and “List files in 'src'”; Factory Droid “Grep roundTotal in src”. Opened, it shows what the tool printed: lines of `path:line:text`, or the paths found.

**A. One wording, matches by file** (from Zed (its grep tool’s output)): “Searched for roundTotal in src”, with “4 matches in 3 files” dimmer at its end. Opened, the matches under each file’s name and icon, with their line numbers and the word marked, as Zed’s grep tool shows them. A glob says “Found 3 files for **/*.test.ts” and opens to the paths.

## 5. Commands: A. Zed’s terminal card

*Agents’ own tools, by kind*

**Today:** A command is a row with the terminal icon, “Ran” and the command in the code font (“Running” and a spinner while it runs, “Failed” in red at its end if it failed). Opened, a command the agent ran in agentZ’s terminal shows that terminal, live, up to 24 rem; one it ran itself shows its output as printed. Nothing says how long it took or how it ended, other than “Failed”.

**A. Zed’s terminal card** (from Zed (its terminal card)): Opened, a command is a card: a header with the command, the folder it ran in, how long it took, its exit code if it failed and Copy (Stop while it runs), over its output in the terminal’s colors. A failed one gets a dashed red border, as in Zed.

## 6. Plans, to-dos and compaction: B. Not in the thread

*Agents’ own tools, by kind*

**Today:** Tools of the “think” kind get the light-bulb icon and the agent’s title. Claude Agent’s to-do list is “Update TODOs: Find where the cart rounds, Round once, after summing, Run the cart tests”, its task tools “Create task: …”, and both Claude Agent and Codex show “Compact conversation” when they compact. Opened, they show their JSON. The plan they make also shows above the composer, in the plan bar.

**B. Not in the thread** (from Zed (its plan entries)): To-do updates and task tools don’t show as rows: the plan bar above the composer already shows the list, as Zed shows a plan only there.

## 7. Fetched pages and web searches: A. Pages as text, searches as links

*Agents’ own tools, by kind*

**Today:** A fetch or web search is a row with the globe and the agent’s title: Claude Agent’s “Fetch https://vitest.dev/api/expect” and “Search "vitest toBeCloseTo"”, Codex’s “Web search: vitest toBeCloseTo”. Opened, it shows what came back as printed: a fetch gives the agent’s notes on the page in markdown, so its `##` and `**` show; a search gives Claude Agent’s results as JSON.

**A. Pages as text, searches as links** (from t3code (its web tools), new): “Fetched vitest.dev/api/expect”, the address a link that opens in the browser. Opened, the notes drawn as markdown, set off by a line on their left, since they are prose and not printed output. “Searched the web for “vitest toBeCloseTo”” opens to the results as links, a title and an address each.

## 8. Leaving plan mode: A. The plan as a card, then the switch

*Agents’ own tools, by kind*

**Today:** When an agent asks to leave plan mode, its row has two arrows and its title: Claude Agent’s “Approve Plan” (then “Exited Plan Mode”), Codex’s “Implement this plan?”, Factory Droid’s “Approve Spec”. The permission card under it offers the agent’s choices. The plan is the tool’s text, so opened it shows as printed, with its markdown signs.

**A. The plan as a card, then the switch** (from Zed (its permission card), new): The plan opens by itself, drawn as markdown in a card with “Plan” at its top, and the agent’s choices under it. Once answered, it folds to “Approved the plan” and a line “Plan → Default” marks the switch.

## 9. Other tools: B. Known tools in words, with their icons

*Agents’ own tools, by kind*

**Today:** A tool of the “other” kind (or none) gets the hammer and the agent’s title: “Load skill: review”, “Report 2 findings”, or the tool’s bare name (“NotebookEdit”) when the agent has nothing better. Opened, it shows its output as printed and “Input” for the JSON.

**B. Known tools in words, with their icons** (from t3code (its labels for each tool)): Tools many agents share get a sentence and an icon: a skill is “Loaded the review skill” with a book, findings are “Reported 2 findings” opening to them drawn as markdown, a question to you is “Asked you” with a speech bubble. Tools nothing knows keep the hammer and their title.

## 10. Failed, cancelled and denied: B. A word for each

*States and runs*

**Today:** A tool call that failed keeps its icon and title and ends in “Failed” in red; opened, it shows what the agent sent back, the error. One that was cancelled when you stopped the turn also says “Failed”, and so does one you denied permission for.

**B. A word for each** (from Zed (its wording)): Icons stay. The row ends in “Failed” in red, “Stopped” dim when you stopped the turn, or “Denied” dim when you said no.

## 11. While a tool call runs: A. Today’s spinner

*States and runs*

**Today:** A tool call that runs says it in the present tense where it can (“Running npm test”) and ends in a spinning circle. While the agent works, its current run is one live line: the row of the step in progress (or the one asking for permission, with its buttons); clicking it opens the run.

**A. Today’s spinner** (from Zed): As today: the present tense and a spinner.

## 12. A folded run: B. The words, then what changed and what failed

*States and runs*

**Today:** Once the agent writes after them, a run of tool calls folds to one line that says what it did in t3code’s words: “Read 2 files, changed 1 file, ran 2 commands, and performed 1 other action”. A chevron opens it to its rows. A lone call stays a row.

**B. The words, then what changed and what failed** (from t3code, new): Today’s words, then at the line’s end the lines it changed (+2 −2) and, if a command or tool failed, “1 failed” in red.

## 13. What holds the output: E. Row and output in one card

*Tool output in a file view*

**Today:** Since the last change, a tool’s output shows as it was printed: one block in the code font on the editor’s background, with a thin border, long lines wrapped. There is nothing over it: no name, no line count, no Copy. Commands run in agentZ’s terminal show that terminal instead. These mocks show a `git diff`’s output, the one from the backlog.

**E. Row and output in one card** (from Zed (its tool call card)): An opened row becomes a card: the row is its header, the output its body, as Zed draws an opened tool call.

## 14. Line numbers: B. In a gutter, for files

*Tool output in a file view*

**Today:** Output has no line numbers of its own. Claude Agent numbers a read’s lines in its text (a number and a tab before each), so those show as text; other agents’ reads and every command have none.

**B. In a gutter, for files** (from Zed (its editor)): File contents (reads, created files) get numbers in a dim gutter: Claude Agent’s moved there from its text, the read’s first line for the others. Commands and other output get none.

## 15. Colors: A. None

*Tool output in a file view*

**Today:** Output is in the text color only. A terminal agentZ runs for the agent keeps the command’s own colors; output an agent sends as text has none.

**A. None** (from agentZ): As today: the text color.

## 16. Long output: A. 24 rem, scrolling

*Tool output in a file view*

**Today:** An opened output is at most 24 rem (384 px) tall and scrolls inside, with a scrollbar. A full test run or a big read is mostly out of view, and it opens at its top.

**A. 24 rem, scrolling** (from agentZ (Zed’s size)): As today, in the file view.

## 17. Long lines: B. One line each, scrolling sideways

*Tool output in a file view*

**Today:** Long lines wrap at the block’s edge, breaking anywhere when a word is too long (picked in the last round). Diffs and terminals don’t wrap: they scroll sideways.

**B. One line each, scrolling sideways** (from Zed (its code blocks)): Each line stays on one line; the block scrolls sideways, with a scrollbar under it.

## 18. ToolSearch: C. The tools’ names in the row

*Tools from MCP servers*

**Today:** An agent that loads tools when it needs them does it with ToolSearch. Its row says what it did, with the magnifying glass: “Loaded 3 github tools” for named tools, “Searched tools for “issues”” and “3 found” for words. Opened, it lists the tools one per line by what they do (picked in the last round).

**C. The tools’ names in the row** (from new): “Loaded Create issue, List issues and Add comment” with “github” dimmer, cut off with “…” when long; nothing to open.

## 19. Other MCP servers’ tools: B. What it acted on, from its input

*Tools from MCP servers*

**Today:** A tool from an MCP server you added shows its name in words, then its server dimmer, with a plug: “Create issue github” (picked in the last round). Opened, it shows what the server sent back as printed, usually JSON, and “Input” for what the agent passed.

**B. What it acted on, from its input** (from new): After the tool’s name, its input’s first short text value, in the muted color: “Create issue Cart total off by a cent github”. Opened, A’s output.

## 20. Images in tool calls: C. A’s, with what it is under it

*Images*

**Today:** A tool call that gives back an image (a read of a PNG, a screenshot) opens by itself to it, at most 384 px wide and tall, in a thin border; a click opens the viewer (picked in the last round).

**C. A’s, with what it is under it** (from new): A, with the image’s name, its size in pixels and on disk under it, dim: “cart.png · 1280 × 800 · 212 KB”.
