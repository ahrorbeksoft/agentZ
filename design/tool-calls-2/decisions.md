# Every kind of tool call: design decisions

Picked on the design board (`design/tool-calls-2/`, see `design/README.md`). Each section is the spec for that part: build the picked option as described, with the comments applied. Generated from `choices.json`; don't edit by hand.

## 1. Reads: Not decided yet

*Agents’ own tools, by kind*

**Today:** A read is a row with its file’s icon and the agent’s title, the project’s folder taken out: “Read src/cart/total.ts”. Claude Agent adds the lines it asked for (“Read src/cart/total.ts (1 - 120)”), Codex says “Read file 'total.ts'”. Opened, it shows the text the agent got, as printed, in a code block: Claude Agent numbers the lines itself (a number and a tab before each), the others send the bare text. “Input” under it opens the JSON the agent passed.

## 2. Edits and their diffs: Not decided yet

*Agents’ own tools, by kind · pick any*

**Today:** An edit is a row with its file’s icon, “Edited” and the path in the code font, and the lines added and removed at its end (+2 −2). Opened, it shows the diff with 3 lines of context: a line on top, a − or + before each changed line, the removed on red and the added on green, the context dimmer. No line numbers or syntax colors; long lines scroll sideways. An edit of several files says “Edited 3 files” and shows their diffs one after another.

## 3. Created, deleted and moved files: Not decided yet

*Agents’ own tools, by kind*

**Today:** A new file shows as an edit: “Edited src/cart/round.ts” (+3 −0), opening to a diff where every line is added. A delete has Zed’s file-with-a-cross icon and the agent’s title (“Delete src/cart/legacy.ts”); a move has two arrows and the agent’s title (“Move src/cart/util.ts to src/cart/round.ts”). Neither opens to anything but the agent’s text.

## 4. Searches: Not decided yet

*Agents’ own tools, by kind*

**Today:** A search is a row with the magnifying glass and the agent’s title. Claude Agent titles a grep with its command line (`grep -n "roundTotal" src`) and a glob as “Find `src` `**/*.test.ts`”; Codex says “Search for 'roundTotal' in src” and “List files in 'src'”; Factory Droid “Grep roundTotal in src”. Opened, it shows what the tool printed: lines of `path:line:text`, or the paths found.

## 5. Commands: Not decided yet

*Agents’ own tools, by kind*

**Today:** A command is a row with the terminal icon, “Ran” and the command in the code font (“Running” and a spinner while it runs, “Failed” in red at its end if it failed). Opened, a command the agent ran in agentZ’s terminal shows that terminal, live, up to 24 rem; one it ran itself shows its output as printed. Nothing says how long it took or how it ended, other than “Failed”.

## 6. Plans, to-dos and compaction: Not decided yet

*Agents’ own tools, by kind*

**Today:** Tools of the “think” kind get the light-bulb icon and the agent’s title. Claude Agent’s to-do list is “Update TODOs: Find where the cart rounds, Round once, after summing, Run the cart tests”, its task tools “Create task: …”, and both Claude Agent and Codex show “Compact conversation” when they compact. Opened, they show their JSON. The plan they make also shows above the composer, in the plan bar.

## 7. Fetched pages and web searches: Not decided yet

*Agents’ own tools, by kind*

**Today:** A fetch or web search is a row with the globe and the agent’s title: Claude Agent’s “Fetch https://vitest.dev/api/expect” and “Search "vitest toBeCloseTo"”, Codex’s “Web search: vitest toBeCloseTo”. Opened, it shows what came back as printed: a fetch gives the agent’s notes on the page in markdown, so its `##` and `**` show; a search gives Claude Agent’s results as JSON.

## 8. Leaving plan mode: Not decided yet

*Agents’ own tools, by kind*

**Today:** When an agent asks to leave plan mode, its row has two arrows and its title: Claude Agent’s “Approve Plan” (then “Exited Plan Mode”), Codex’s “Implement this plan?”, Factory Droid’s “Approve Spec”. The permission card under it offers the agent’s choices. The plan is the tool’s text, so opened it shows as printed, with its markdown signs.

## 9. Other tools: Not decided yet

*Agents’ own tools, by kind*

**Today:** A tool of the “other” kind (or none) gets the hammer and the agent’s title: “Load skill: review”, “Report 2 findings”, or the tool’s bare name (“NotebookEdit”) when the agent has nothing better. Opened, it shows its output as printed and “Input” for the JSON.

## 10. Failed, cancelled and denied: Not decided yet

*States and runs*

**Today:** A tool call that failed keeps its icon and title and ends in “Failed” in red; opened, it shows what the agent sent back, the error. One that was cancelled when you stopped the turn also says “Failed”, and so does one you denied permission for.

## 11. While a tool call runs: Not decided yet

*States and runs*

**Today:** A tool call that runs says it in the present tense where it can (“Running npm test”) and ends in a spinning circle. While the agent works, its current run is one live line: the row of the step in progress (or the one asking for permission, with its buttons); clicking it opens the run.

## 12. A folded run: Not decided yet

*States and runs*

**Today:** Once the agent writes after them, a run of tool calls folds to one line that says what it did in t3code’s words: “Read 2 files, changed 1 file, ran 2 commands, and performed 1 other action”. A chevron opens it to its rows. A lone call stays a row.

## 13. What holds the output: Not decided yet

*Tool output in a file view*

**Today:** Since the last change, a tool’s output shows as it was printed: one block in the code font on the editor’s background, with a thin border, long lines wrapped. There is nothing over it: no name, no line count, no Copy. Commands run in agentZ’s terminal show that terminal instead. These mocks show a `git diff`’s output, the one from the backlog.

## 14. Line numbers: Not decided yet

*Tool output in a file view*

**Today:** Output has no line numbers of its own. Claude Agent numbers a read’s lines in its text (a number and a tab before each), so those show as text; other agents’ reads and every command have none.

## 15. Colors: Not decided yet

*Tool output in a file view*

**Today:** Output is in the text color only. A terminal agentZ runs for the agent keeps the command’s own colors; output an agent sends as text has none.

## 16. Long output: Not decided yet

*Tool output in a file view*

**Today:** An opened output is at most 24 rem (384 px) tall and scrolls inside, with a scrollbar. A full test run or a big read is mostly out of view, and it opens at its top.

## 17. Long lines: Not decided yet

*Tool output in a file view*

**Today:** Long lines wrap at the block’s edge, breaking anywhere when a word is too long (picked in the last round). Diffs and terminals don’t wrap: they scroll sideways.

## 18. ToolSearch: Not decided yet

*Tools from MCP servers*

**Today:** An agent that loads tools when it needs them does it with ToolSearch. Its row says what it did, with the magnifying glass: “Loaded 3 github tools” for named tools, “Searched tools for “issues”” and “3 found” for words. Opened, it lists the tools one per line by what they do (picked in the last round).

## 19. Other MCP servers’ tools: Not decided yet

*Tools from MCP servers*

**Today:** A tool from an MCP server you added shows its name in words, then its server dimmer, with a plug: “Create issue github” (picked in the last round). Opened, it shows what the server sent back as printed, usually JSON, and “Input” for what the agent passed.

## 20. Images in tool calls: Not decided yet

*Images*

**Today:** A tool call that gives back an image (a read of a PNG, a screenshot) opens by itself to it, at most 384 px wide and tall, in a thin border; a click opens the viewer (picked in the last round).
