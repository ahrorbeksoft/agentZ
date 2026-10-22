# Subthreads, Machines and Shells: design decisions

Picked on the design board (`design/subthreads-machines/`, see `design/README.md`). Each section is the spec for that part: build the picked option as described, with the comments applied. Generated from `choices.json`; don't edit by hand.

## 1. Subthreads in the sidebar: A. Nested under the parent, like worktrees

*Subthreads in the sidebar*

**Today:** Subthreads have no rows of their own. The parent’s card counts them with a people icon (“👥 4”, blue while one runs), and its details popover says “4 subthreads, 2 running”. A subthread opens from its parent’s Agents list or its card in the conversation, and the sidebar then highlights the parent.

**A. Nested under the parent, like worktrees** (from Workspaces worktree rows (herdr)): Each subthread is a row under its parent’s card, indented with tree lines, as a worktree sits under its workspace in the Workspaces view. The first line has the agent’s icon, the title and its status (Working, Needs Approval, or how long ago it ended); the second line has the agent, the model and, when it works in one, its worktree, with the lines it changed. The count on the parent’s card becomes a fold toggle (“Hide Subthreads”). A click opens the subthread.

## 2. Finished subthreads in the sidebar: A. Fold once all are done

*Subthreads in the sidebar*

**Today:** Nothing is in the sidebar today. In the parent’s Agents list, every subthread stays listed, and the list folds to “4 Agents · all done” once the last one ends (the Subthreads round’s pick). The mocks use rows A from the topic before.

**A. Fold once all are done** (from the Agents list (Subthreads round)): While any subthread runs, they’re all shown under the parent. Once the last one ends, they fold away into the count on the parent’s card, as the Agents list folds, and show again when a new one starts. A click on the count opens them.

## 3. The Agents list above the composer: C. No list

*In the parent thread*

**Today:** A parent lists its subthreads above its composer, under “4 Agents · 2 running”: Zed’s subagent headers as rows, with a spinner, check or cross, the title, “· model”, the files it changed and Stop while it runs. It folds to “· all done” once the last one ends. If you pick sidebar rows, the same subthreads show there too.

**C. No list** (from new): The list goes. Subthreads show in the sidebar under the parent and as their cards in the conversation; the bar above the composer keeps the plan and background tasks.

## 4. Where the parent started them: B. One card for the batch

*In the parent thread*

**Today:** Each `delegate_task` call in the parent’s conversation is Zed’s subagent card: a spinner, check or cross, the title, “· model”, the files it changed, and Stop while it runs. A running card shows the step it’s on and a strip that opens it full screen; opened, it shows its last steps and, once it ended, its summary. Calls made together fold into “Started 4 subthreads”, which opens to a card each (the picture shows it opened, above the Agents list).

**B. One card for the batch** (from Zed subagent card, grouped): Subthreads started together share one card: “Started 4 subthreads · 2 running”, then a row each with its status, the agent’s icon, title, model, worktree, the lines it changed, and Stop or how long it ran, and Open. A click on a row opens its preview (its last steps and summary) under it.

## 5. The results: B. One row for results that came together

*In the parent thread*

**Today:** When subthreads end, the parent gets a message it doesn’t show: “Delegated tasks 281, 282 reached terminal states. Use task_status with each taskId to read the results.” Its agent then calls `task_status` for each, shown as ordinary tool calls, and only then answers. Each result also shows in its start card once you open it. The mocks use the batch card B from the topic before.

**B. One row for results that came together** (from agentZ tool rows, folded): Results that reach the parent in one message share a row, “4 subthreads finished”, which opens to the rows of A.

## 6. The task and its instructions: A. Folded after a few lines

*An open subthread*

**Today:** A subthread’s first message is the task its parent wrote, in a card: “Task from ‘…’” with the role, and the whole prompt under it. Parent agents fill it with the same instructions every time (you are a subagent, work only in your worktree, run the checks, report like this), so the task itself is a line or two lost above a screen of rules. Later messages from the parent show whole too, with “Sent by the agent in …” above them.

**A. Folded after a few lines** (from new): The card shows the task’s first four lines, fading out, and “Show more” opens the rest. Later messages from the parent fold the same way past four lines.

## 7. More control, with good defaults: A. Several subthreads in one call + C. Send a subthread more work + D. Results in the message + F. Limits with defaults

*What agents can do · pick any*

**Today:** An agent starts one subthread per `delegate_task` call. It gets the task prompt only, works in the parent’s folder unless the agent asks for a worktree or pasture, and runs once: there’s no way to send it more. Its end is announced with “Delegated task 282 reached a terminal state. Use task_status with taskId 282 to read the result.”, so the agent then calls `task_status` to read each result. The agent can also wait, blocking its turn (`mode: "wait"`, up to an hour). Pick any of these.

**A. Several subthreads in one call** (from t3code create_threads): `delegate_tasks` takes a list of tasks, each with its own title, agent, model and worktree, plus instructions given once for all of them. They start together and show as one batch (“Started 4 subthreads”). `delegate_task` stays for one.

**C. Send a subthread more work** (from t3code thread_send): `task_send` sends a message to a subthread it started, running or ended, which goes on in the same conversation with what it already knows: “Also cover the logged-out state”. It shows in the subthread as a message from the parent, and the parent hears of its end again.

**D. Results in the message** (from new): The message that tells the parent its subthreads ended carries each one’s outcome, summary and changed files, so the agent reads them without calling `task_status`. Pairs with the result rows in “The results”.

**F. Limits with defaults** (from new): At most 4 subthreads run at once for a thread; more wait in a queue (“Queued” in their row) and start as others end. A subthread can’t start subthreads of its own unless its parent allows it (`canDelegate`).

**Comment on F:** idk, should be configurable i guess

## 8. How the parent waits: A. Always end the turn

*What agents can do*

**Today:** Agents are told to end their turn and get a message when a subthread ends, but they may also call `delegate_task` with `mode: "wait"` (or `agentz_thread_wait`), which holds their turn open for up to 10 minutes by default and an hour at most, ending early only if the subthread asks a question. Meanwhile the parent’s turn looks stuck on one tool call, and its agent can do nothing else.

**A. Always end the turn** (from t3code delegated tasks): `mode: "wait"` goes. `delegate_task` answers at once and tells the agent to end its turn; the results come back in a message that starts its next turn. While it waits, the end of the parent’s conversation says “Waiting for 3 subthreads” with a spinner, and the sidebar keeps showing it as Working.

## 9. The machine menu: A. One line each, the details beside it

*Machine picker*

**Today:** Under a new thread’s composer, “This Mac ⌄” opens a menu of the project’s copies: each machine’s name with its folder in small gray type, then a line like “main · → origin/main up to date · no changes”, and a check on the current one. A machine that’s offline or lacks the selected agent is grayed with the reason. A footer says “As of each machine’s last fetch: This Mac 9m ago, agents-1 never”. The mocks use Claude Agent, which both machines have; Devbox 1 is offline.

**A. One line each, the details beside it** (from Zed context menu asides): A row per machine: its icon, its name, and at the right only what differs from the others, in yellow (“↓2 · 3 files”), or “Offline”. Hovering a row shows Zed’s aside beside the menu: the folder, the branch and its upstream, the changes, and when it was last fetched. No footer.

## 10. A machine without the selected agent: A. Pick it, and take an agent it has

*Machine picker*

**Today:** The menu checks only the draft’s agent: a machine where it isn’t installed is grayed (“Google Antigravity isn’t installed”) and can’t be picked. The agent menu lists only the current machine’s agents. So with Antigravity on This Mac and Claude Agent on agents-1, you must first switch to an agent both have, then switch machines. The mocks use the menu A from the topic before.

**A. Pick it, and take an agent it has** (from agentZ’s default agent for a machine): Every online machine can be picked. One without the draft’s agent says which agent it would use instead (“Uses Claude Agent”): the agent of that machine’s newest thread, else its first installed one, as a new thread there would start with. Picking it moves the draft there with that agent, keeping what you typed; the agent menu then lists that machine’s agents.

## 11. A close button for terminals in the Agents view: A. In the terminal’s header

*Shells*

**Today:** A terminal opened from the Agents view (New Thread › Terminal) is listed under Shells in the sidebar and fills the main area. Its header has the icon, its title, the folder, Restart and the changes button, but nothing to close it. It closes only with Ctrl-Shift-W (Cmd-W on macOS), which asks first while a program runs, or with Delete… in its row’s right-click menu. The terminals in a thread’s drawer already have ×, and stay as they are.

**A. In the terminal’s header** (from Zed and Workspaces pane headers): A × at the right end of the terminal’s header, after Restart and the changes button, with the tooltip “Close Terminal” and its shortcut. It does what Ctrl-Shift-W does: closes at once at a prompt, and asks first while a program runs.
