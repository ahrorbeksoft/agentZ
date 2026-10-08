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

## 3. Long lines in the output: A. Wrapped

*A tool call*

**Today:** Code blocks in a tool call keep each line on one line. They scroll sideways, as Zed’s do, but show no scrollbar, and Copy shows on hover. Zed’s message code blocks also have a Wrap button on hover.

**A. Wrapped** (from t3code): Long lines wrap at the block’s edge, breaking anywhere when a word is too long, as t3code’s tool output does. Commands’ terminals stay as they are.

## 4. Images in tool calls: B. Shown at once, 384 px, zoom on click

*Images*

**Today:** A tool call that gives back an image (a read of a PNG, a screenshot) starts closed like any row. Opened, it shows the input, then the image at most 384 px wide and tall, Zed’s size. A click opens it in the image viewer. The input topic decides whether the JSON shows here; these mocks leave it out.

**B. Shown at once, 384 px, zoom on click** (from new, at Zed’s size): A tool call with an image starts open, so the image shows under its row without a click, at most 384 px wide and tall (today’s size), in a thin border, with a zoom-in cursor; a click opens the viewer. Clicking the row closes it as any row.

## 5. Images in messages: A. Thumbnails in the bubble

*Images*

**Today:** An image you paste into a message, or one an agent puts in its reply, shows as an "@Image" link in the text, as Zed writes a mention. Hovering it shows a 320 × 240 preview; a click opens the image viewer.

**A. Thumbnails in the bubble** (from t3code): Your message’s images show as thumbnails above its text, two to a row, each 100 × 75 px, cropped to fill. An image in an agent’s reply shows the same way where it is. A click opens the viewer, with arrows to the message’s other images.

## 6. An image opened larger: A. t3code’s viewer, whole

*Images*

**Today:** A click on an image opens t3code’s viewer in its simplest form: the image as large as the window allows, over a dark backdrop, with a close button above it. Escape, the button or a click beside the image closes it.

**A. t3code’s viewer, whole** (from t3code): Arrows (and the ← → keys) step through the other images in the same message or tool call. The name shows under the image ("az-linux-rel.png · 1 of 2"). A click zooms to 200% where you clicked and back to fit; scrolling zooms, dragging moves the zoomed image, and "200% zoom" shows while zoomed.

## 7. agentZ’s own tools, as what they did: A. A line that says what it did, with Open

*agentZ’s tools and other tools*

**Today:** A call to one of agentZ’s tools shows the name the agent sends, with the MCP server’s prefix: `agentz___delegate_task` from Factory Droid. The hammer icon, nothing about what it did, and opening it shows the JSON input and output. The subthreads it started are only in the Agents list above the composer.

**A. A line that says what it did, with Open** (from t3code (its own tools’ labels and "Open chat")): Each of agentZ’s tools gets a sentence in the past tense with what it acted on, and the agentZ mark: "Started a subthread: <title>", "Started a terminal: npm run dev", "Waited for <title>", "Listed agents and models". While it runs, the present tense and a spinner ("Starting a subthread…"). Rows that made a subthread, a thread or a terminal end in "Open", which shows it. A folded run counts them too ("Started 3 subthreads").

## 8. ToolSearch: A. "Loaded 8 tools", opening to their names in words

*agentZ’s tools and other tools*

**Today:** Some agents load tools only when they need them, with a tool called ToolSearch. Its row says "ToolSearch" with a magnifying glass; opened, it shows the JSON query ("select:" and the tools’ full names) and the output ("Loaded 8 tool(s): …"). A query can also be words, which finds tools by what they do. Zed and t3code show it like any other tool.

**A. "Loaded 8 tools", opening to their names in words** (from new): The row says what it did: "Loaded 8 agentZ tools" for a "select:" query, or "Searched tools for “subthread”" for words. Opened, it lists the tools one per line by what they do (agentZ’s tools by their own titles, others as the MCP names topic picks), with no JSON.

## 9. Other MCP tools’ names: A. The tool in words, then its server

*agentZ’s tools and other tools*

**Today:** A tool from an MCP server you added shows the name the agent sends, which joins the server’s name and the tool’s, each agent its own way: `github___create_issue` from Factory Droid, `mcp__linear__list_issues` from Claude Agent. agentZ’s own tools are decided in their topic; this is for every other MCP tool.

**A. The tool in words, then its server** (from t3code): The tool’s name with its underscores and dashes as spaces and a capital first letter ("Create issue"), then the server’s name, dimmer ("github"), with a plug icon. Every agent’s spelling reads the same.
