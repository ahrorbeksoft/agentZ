# agentZ’s own tools, subthreads and subagents: design decisions

Picked on the design board (`design/agentz-tools/`, see `design/README.md`). Each section is the spec for that part: build the picked option as described, with the comments applied. Generated from `choices.json`; don't edit by hand.

## 1. Icons: A. The agentZ mark for all

*agentZ’s tools: the row*

**Today:** Every one of agentZ’s 26 tools has the agentZ mark (a Z in a square), whatever it did, as the Tool calls round picked. An agent’s own tools get the icon of their kind or their file.

**A. The agentZ mark for all** (from Tool calls round (t3code marks its own tools with its wordmark)): As today: the mark on every one of agentZ’s tools, so they read apart from the agent’s own.

## 2. What each row says: B. The sentence, then what came back

*agentZ’s tools: the row*

**Today:** Each tool says what it did, in the past tense, with what it acted on (a thread by its title now, code in the code font), as the Tool calls round picked: “Started a subthread: …”, “Waited for …”, “Listed threads”. What came back shows only when the row is opened. Below, every one of the 26 tools, in its group; the group names are the board’s, not the app’s.

**B. The sentence, then what came back** (from t3code (a work row’s detail after its label), Zed (an edit’s +/− at its end)): After the sentence, dimmer, what came of it: “8 threads”, “Done in 2m 14s”, “Working”, “Exit 0 · 14s”, “+12 −3”. A terminal’s row says what was typed. Tools that only made something keep Open.

## 3. Running and failed: A. As today

*agentZ’s tools: the row*

**Today:** While a tool runs, its row is in the present tense with a spinner (“Starting a subthread: …”, “Waiting for …”). A failed one reads as an order (“Start a subthread: …”) with a red “Failed”; why it failed shows only when the row is opened, as the tool’s text. A wait of minutes looks like a wait of a second.

**A. As today** (from Tool calls round): Present tense and a spinner; an order and “Failed”, the reason inside.

## 4. What opening a row shows: A. What came back, in words

*agentZ’s tools: the row*

**Today:** Opening one of agentZ’s rows shows what the tool gave back as it was printed. For agentZ’s tools that’s JSON, with fields like `taskId`, `workState` and `waitTimedOut`, and the input as JSON behind “Input” at the end. Four tools opened, one from each of four groups; the mocks use the result after the sentence (What each row says, B).

**A. What came back, in words** (from new (as the Tool calls round showed ToolSearch’s tools in words)): Each tool opens to a few lines in words: threads one a line with their agent, model, status and when; a command’s output as it was printed, in the code font; a workspace in a sentence and its folder; each agent with its models. The JSON stays behind “Input”.

## 5. Links to what a tool acted on: A. Open for what it made

*agentZ’s tools: the row*

**Today:** Rows that made a subthread, a thread or a terminal end in “Open”, which shows it. Rows that name a thread or a terminal they didn’t make (“Waited for …”, “Read …”) link to nothing. The mocks use the result after the sentence (What each row says, B).

**A. Open for what it made** (from t3code (“Open chat” on a thread it created)): As today: Open at the end of rows that made a subthread, a thread or a terminal.

## 6. In a folded run: A. As today

*agentZ’s tools: the row*

**Today:** A finished run of work folds to one line in t3code’s words: at most two kinds, those that made something first (subthreads, threads, terminals, commands, edits), then “and performed N other actions”. agentZ’s lists, reads, waits and checks count as other actions. Here the run is: listed agents and models, listed threads, started 3 subthreads, checked on 2, waited for 2, ran a command.

**A. As today** (from t3code (summarizeToolGroup)): One line for the run, the subthreads in it.

## 7. A subthread where it started: C. Zed’s subagent card

*Subthreads in the parent*

**Today:** A `delegate_task` call is one of agentZ’s rows: “Started a subthread: <title>” and Open. Opened, it shows the tool’s answer, raw (`taskId`, `childThreadId`, `role`, `status`, `workState`, …), above a collapsed Input (your screenshot). How it’s going shows only in the Agents list over the composer; the row stays the same once it ends.

**C. Zed’s subagent card** (from Zed (render_subagent_card)): A bordered card: a spinner or check, the title, “· model”, the files it changed, and Stop while it runs. A running one shows the step it’s on inside, and a strip at the bottom opens it full screen.

## 8. Opening a subthread’s row: A. Zed’s preview of its work

*Subthreads in the parent*

**Today:** Opened, the row shows the tool’s answer as raw JSON and a collapsed Input (your screenshot, in the topic before). The subthread’s steps and its summary are only in the subthread. The mocks use the row the topic before recommends.

**A. Zed’s preview of its work** (from Zed (render_subagent_expanded_content), as agentZ’s subagents open): Its last 8 steps, fading at the top when there are more, then its summary once it ends, and a strip that opens the subthread (“Make Subagent Full Screen”). The same as a Claude Agent subagent opens.

## 9. Several subthreads at once: A. A row each

*Subthreads in the parent*

**Today:** An agent that starts several subthreads calls `delegate_task` once for each, one row after another, and the run folds to “Started 3 subthreads” once it ends. `create_threads`, which starts several threads in one call, is one row: “Started 3 threads”. The mocks use the row topic 7 recommends.

**A. A row each** (from Zed (a card for each subagent)): Each subthread its own row, one under another, as they were started.

## 10. A subagent at work: B. Zed’s card while it runs

*Agents’ own subagents*

**Today:** Claude Agent sends its subagents’ steps in sessions of their own, which agentZ makes subthreads. Its row: the bot, the subagent’s description, its type as a tag, Open and a spinner; under it, the one step it’s on. Opening it shows its last 8 steps. Factory Droid sends no steps (topic 12). Here two Claude Agent subagents run at once, from the mock agent.

**B. Zed’s card while it runs** (from Zed (render_subagent_card)): A bordered card: a spinner, the description, “· Explore”, Stop. Inside, the step it’s on, as its own row, with any output or question of its own; a strip at the bottom opens it full screen.

## 11. A finished subagent: C. As today

*Agents’ own subagents*

**Today:** Once a Claude Agent subagent ends, its row shows how long it ran and a check (“Failed”, “Stopped”). Opened: its last 8 steps fading at the top, its report, and a strip that opens its subthread. Closed, nothing says what it did or found.

**C. As today** (from Zed (render_subagent_expanded_content)): Closed, the row with its time and check. Opened, its last 8 steps fading at the top, its report, and the strip that opens it.

## 12. Subagents that send no steps: B. The report first

*Agents’ own subagents*

**Today:** Factory Droid’s Task sends only its input (type, description, options, prompt) and, once done, what came back. Its row: the bot, the description, the type as a tag, the time and a check. Opened: “Task” with the whole prompt, “Report”, and Input. A Task Droid runs in the background ends at once (448 ms in your screenshot), so its report is Droid’s launch notice, task_id and all, and the real report never shows where it started.

**B. The report first** (from Tool calls round (output first, the input behind “Input”)): Opened: its report alone, with “Task” at the end, a line that opens the task as Input opens JSON. A background one opens to its task.

## 13. What the parent shows when its tasks end: A. Nothing

*When delegated tasks end*

**Today:** When subthreads end, agentZ tells the parent’s agent with t3code’s message, sent in your place and marked “Sent by the agent in “…””: “Delegated tasks 282, 283 reached terminal states. Use task_status with each taskId to read the results.” (your screenshot). Its next turn starts from that message. Another change is hiding that message from you now; the agent still gets it, as it’s the only thing that starts a turn while the agent is idle. The mocks use topic 7’s recommended row.

**A. Nothing** (from new (the change being made now)): The message stays hidden. The subthreads’ rows and the Agents list already show that they ended, and the agent’s next turn just follows.
