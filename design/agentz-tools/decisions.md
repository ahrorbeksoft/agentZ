# agentZ’s own tools, subthreads and subagents: design decisions

Picked on the design board (`design/agentz-tools/`, see `design/README.md`). Each section is the spec for that part: build the picked option as described, with the comments applied. Generated from `choices.json`; don't edit by hand.

## 1. Icons: Not decided yet

*agentZ’s tools: the row*

**Today:** Every one of agentZ’s 26 tools has the agentZ mark (a Z in a square), whatever it did, as the Tool calls round picked. An agent’s own tools get the icon of their kind or their file.

## 2. What each row says: Not decided yet

*agentZ’s tools: the row*

**Today:** Each tool says what it did, in the past tense, with what it acted on (a thread by its title now, code in the code font), as the Tool calls round picked: “Started a subthread: …”, “Waited for …”, “Listed threads”. What came back shows only when the row is opened. Below, every one of the 26 tools, in its group; the group names are the board’s, not the app’s.

## 3. Running and failed: Not decided yet

*agentZ’s tools: the row*

**Today:** While a tool runs, its row is in the present tense with a spinner (“Starting a subthread: …”, “Waiting for …”). A failed one reads as an order (“Start a subthread: …”) with a red “Failed”; why it failed shows only when the row is opened, as the tool’s text. A wait of minutes looks like a wait of a second.

## 4. What opening a row shows: Not decided yet

*agentZ’s tools: the row*

**Today:** Opening one of agentZ’s rows shows what the tool gave back as it was printed. For agentZ’s tools that’s JSON, with fields like `taskId`, `workState` and `waitTimedOut`, and the input as JSON behind “Input” at the end. Four tools opened, one from each of four groups; the mocks use the result after the sentence (What each row says, B).

## 5. Links to what a tool acted on: Not decided yet

*agentZ’s tools: the row*

**Today:** Rows that made a subthread, a thread or a terminal end in “Open”, which shows it. Rows that name a thread or a terminal they didn’t make (“Waited for …”, “Read …”) link to nothing. The mocks use the result after the sentence (What each row says, B).

## 6. In a folded run: Not decided yet

*agentZ’s tools: the row*

**Today:** A finished run of work folds to one line in t3code’s words: at most two kinds, those that made something first (subthreads, threads, terminals, commands, edits), then “and performed N other actions”. agentZ’s lists, reads, waits and checks count as other actions. Here the run is: listed agents and models, listed threads, started 3 subthreads, checked on 2, waited for 2, ran a command.

## 7. A subthread where it started: Not decided yet

*Subthreads in the parent*

**Today:** A `delegate_task` call is one of agentZ’s rows: “Started a subthread: <title>” and Open. Opened, it shows the tool’s answer, raw (`taskId`, `childThreadId`, `role`, `status`, `workState`, …), above a collapsed Input (your screenshot). How it’s going shows only in the Agents list over the composer; the row stays the same once it ends.

## 8. Opening a subthread’s row: Not decided yet

*Subthreads in the parent*

**Today:** Opened, the row shows the tool’s answer as raw JSON and a collapsed Input (your screenshot, in the topic before). The subthread’s steps and its summary are only in the subthread. The mocks use the row the topic before recommends.

## 9. Several subthreads at once: Not decided yet

*Subthreads in the parent*

**Today:** An agent that starts several subthreads calls `delegate_task` once for each, one row after another, and the run folds to “Started 3 subthreads” once it ends. `create_threads`, which starts several threads in one call, is one row: “Started 3 threads”. The mocks use the row topic 7 recommends.

## 10. A subagent at work: Not decided yet

*Agents’ own subagents*

**Today:** Claude Agent sends its subagents’ steps in sessions of their own, which agentZ makes subthreads. Its row: the bot, the subagent’s description, its type as a tag, Open and a spinner; under it, the one step it’s on. Opening it shows its last 8 steps. Factory Droid sends no steps (topic 12). Here two Claude Agent subagents run at once, from the mock agent.

## 11. A finished subagent: Not decided yet

*Agents’ own subagents*

**Today:** Once a Claude Agent subagent ends, its row shows how long it ran and a check (“Failed”, “Stopped”). Opened: its last 8 steps fading at the top, its report, and a strip that opens its subthread. Closed, nothing says what it did or found.

## 12. Subagents that send no steps: Not decided yet

*Agents’ own subagents*

**Today:** Factory Droid’s Task sends only its input (type, description, options, prompt) and, once done, what came back. Its row: the bot, the description, the type as a tag, the time and a check. Opened: “Task” with the whole prompt, “Report”, and Input. A Task Droid runs in the background ends at once (448 ms in your screenshot), so its report is Droid’s launch notice, task_id and all, and the real report never shows where it started.

## 13. What the parent shows when its tasks end: Not decided yet

*When delegated tasks end*

**Today:** When subthreads end, agentZ tells the parent’s agent with t3code’s message, sent in your place and marked “Sent by the agent in “…””: “Delegated tasks 282, 283 reached terminal states. Use task_status with each taskId to read the results.” (your screenshot). Its next turn starts from that message. Another change is hiding that message from you now; the agent still gets it, as it’s the only thing that starts a turn while the agent is idle. The mocks use topic 7’s recommended row.
