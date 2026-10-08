# Tool calls and images: design decisions

Picked on the design board (`design/tool-calls/`, see `design/README.md`). Each section is the spec for that part: build the picked option as described, with the comments applied. Generated from `choices.json`; don't edit by hand.

## 1. Icons by kind: B. The file’s own icon for reads and edits

*A tool call*

**Today:** Zed’s icons by the kind the agent gives a tool call: a magnifying glass for reads and searches, a pencil for edits, a terminal for commands, a globe for fetches. Every other tool, agentZ’s own and other MCP tools included, gets a hammer. The labels in these mocks are today’s; other topics decide them.

**B. The file’s own icon for reads and edits** (from Zed (its edit cards), extended to reads): Reads and edits get the icon of their file’s type, as Zed’s edit cards do: an image icon for a PNG, a document for code. Other kinds get A’s icons.

## 2. The input, when a row is opened: B. Output first, the input behind “Input”

*A tool call*

**Today:** Opening a row that isn’t a command or an edit shows the tool’s input first, as pretty-printed JSON in a code block, then its output. Zed does the same, with "Raw Input:" and "Output:" headings, and leaves the input out when the output has an image. Its permission card hides the input behind "View Raw Input".

**B. Output first, the input behind “Input”** (from Zed (its "View Raw Input")): Opening a row shows the output only. At its end, a small "Input" line with a chevron opens the JSON, as Zed’s permission card does with "View Raw Input". A tool call with an image shows only the image.

## 3. Long lines in the output: Not decided yet

*A tool call*

**Today:** Code blocks in a tool call keep each line on one line. They scroll sideways, as Zed’s do, but show no scrollbar, and Copy shows on hover. Zed’s message code blocks also have a Wrap button on hover.

## 4. Images in tool calls: Not decided yet

*Images*

**Today:** A tool call that gives back an image (a read of a PNG, a screenshot) starts closed like any row. Opened, it shows the input, then the image at most 384 px wide and tall, Zed’s size. A click opens it in the image viewer. The input topic decides whether the JSON shows here; these mocks leave it out.

## 5. Images in messages: Not decided yet

*Images*

**Today:** An image you paste into a message, or one an agent puts in its reply, shows as an "@Image" link in the text, as Zed writes a mention. Hovering it shows a 320 × 240 preview; a click opens the image viewer.

## 6. An image opened larger: Not decided yet

*Images*

**Today:** A click on an image opens t3code’s viewer in its simplest form: the image as large as the window allows, over a dark backdrop, with a close button above it. Escape, the button or a click beside the image closes it.

## 7. agentZ’s own tools, as what they did: Not decided yet

*agentZ’s tools and other tools*

**Today:** A call to one of agentZ’s tools shows the name the agent sends, with the MCP server’s prefix: `agentz___delegate_task` from Factory Droid. The hammer icon, nothing about what it did, and opening it shows the JSON input and output. The subthreads it started are only in the Agents list above the composer.

## 8. ToolSearch: Not decided yet

*agentZ’s tools and other tools*

**Today:** Some agents load tools only when they need them, with a tool called ToolSearch. Its row says "ToolSearch" with a magnifying glass; opened, it shows the JSON query ("select:" and the tools’ full names) and the output ("Loaded 8 tool(s): …"). A query can also be words, which finds tools by what they do. Zed and t3code show it like any other tool.

## 9. Other MCP tools’ names: Not decided yet

*agentZ’s tools and other tools*

**Today:** A tool from an MCP server you added shows the name the agent sends, which joins the server’s name and the tool’s, each agent its own way: `github___create_issue` from Factory Droid, `mcp__linear__list_issues` from Claude Agent. agentZ’s own tools are decided in their topic; this is for every other MCP tool.
