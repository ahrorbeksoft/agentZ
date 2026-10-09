# Agents' own subagents: design decisions

Picked on the design board (`design/agent-subagents/`, see `design/README.md`). Each section is the spec for that part: build the picked option as described, with the comments applied. Generated from `choices.json`; don't edit by hand.

## 1. A subagent’s row: C. Its title and type on a row

*In the conversation*

**Today:** A subagent is a tool call like any other. Factory Droid’s reads “Task” with the hammer every unnamed tool gets, and a spinner while it runs. Claude Agent marks its call as thinking, so it gets the lightbulb, with the subagent’s description as its label (“Find where the login view is drawn”). Nothing says it’s a subagent, what kind, or how long it ran. With the mock agent, both kinds: now-subagent-calls-closed.png.

**C. Its title and type on a row** (from new): A row with a bot icon, the description in the row’s brighter gray, the type as a small tag, and a spinner, or the time and a check once done.

## 2. What opening it shows: B. Zed’s preview of its work

*In the conversation*

**Today:** As picked in the Tool calls round, an opened row shows the tool’s output, with its input as JSON behind “Input” at the end. Factory Droid’s Task has no output while it runs, so it opens to only “Input”: the type, description, await, complexity and prompt as JSON. Once done, its output is the subagent’s report. Claude Agent’s shows its prompt while it runs and its report once done. The mocks use row A from the topic before.

**B. Zed’s preview of its work** (from Zed (render_subagent_expanded_content)): Zed’s: while it runs, the card shows the step the subagent is on. Opened, its last steps (up to 8, fading at the top) and its report; a strip at the bottom opens all of it (as a subthread, if topic 3 picks B). Only agents that report the steps (Claude Agent) have them; Droid’s opens as A.

## 3. Claude Agent’s subagents’ own steps: B. As subthreads

*The subagent’s work*

**Today:** Claude Agent sends its subagents’ tool calls and text into the thread, each marked with the call it belongs to (`parentToolUseId`). agentZ doesn’t read the mark, so they show as the agent’s own rows and words, between its real ones; two subagents at once interleave, as in option D’s mock. Claude Agent can instead give each subagent a session of its own, when the app says it takes them (the `subagents` capability): it then announces the subagent (`subagent_spawned`, with its name, task and prompt), sends its steps to that session, and says when it ends. Devin marks its subagents in its updates too, which t3code reads. Factory Droid sends no steps. From the agents’ code, not seen in agentZ: that takes a prompt to your real Claude.

**B. As subthreads** (from t3code (child threads), with Claude Agent’s subagent sessions): agentZ says it takes subagent sessions, and makes each subagent a subthread of the thread, as agentZ’s own are: in the Agents list (topic 4), and opening with the subthread header and bar the Subthreads round picked, its steps and words in its own conversation. It runs on its own, so its bar has no composer. The card in the thread ends in Open. Devin’s marked subagents could go the same way; Droid’s, which send no steps, stay a card.

## 4. In the Agents list: B. Only while they run

*The subagent’s work*

**Today:** The Agents list over the composer lists only agentZ’s subthreads (started with `delegate_task`), each with its status, title, “· model” and the files it changed, as the Subthreads round picked. It opens while one runs and folds to “N Agents · all done” once the last ends. An agent’s own subagents aren’t in it. t3code’s bar, which the list was based on, lists the agent’s own with its threads. The mocks use row A from topic 1.

**B. Only while they run** (from new): As A, but an agent’s own subagent leaves the list once it ends; its card stays in the conversation.

**Comment on B:** only if they have steps
