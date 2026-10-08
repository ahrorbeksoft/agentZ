# Subthreads: design decisions

Picked on the design board (`design/subthreads/`, see `design/README.md`). Each section is the spec for that part: build the picked option as described, with the comments applied. Generated from `choices.json`; don't edit by hand.

## 1. The rows: B. Status, title and model, with changed files

*Agents list*

**Today:** The list above a parent’s composer, under a line like “3 Agents · 1 running”. Each row has a status icon, the title, the agent and the task’s role (“Factory Droid · research”), and a status word: Done, Working, Needs approval, Waiting, Failed, Cancelled or Stopped. The role is what the parent agent passed to `delegate_task`: implementation, research, review, design, test or general. Newest first; six rows show, then it scrolls. A click opens the subthread. In every option the “4 Agents” line above the rows stays as it is.

**B. Status, title and model, with changed files** (from Zed subagent cards): Zed’s subagent header as a row: a spinner, green check or red cross, the title, then “· Opus 5.5” muted. When the subthread changed files, “— 1 file changed +12 −0” follows. A running row has a red Stop button at its end. No status word and no role.

## 2. Finished subthreads: C. The list folds once all are done + also A. All stay in the list

*Agents list*

**Today:** Finished subthreads stay in the list for good, and can’t be hidden one by one. You said that’s fine. A click on the “4 Agents” line folds the whole list, which starts open each time the thread opens. The mocks use rows A from the topic before.

**C. The list folds once all are done** (from new): While any subthread runs, the list is open. Once the last one finishes, it folds to its “4 Agents · all done” line, and opens again if a new one starts. A click on that line opens it.

**Also take from A. All stay in the list** (from today): Every subthread stays listed, running or finished, newest first. The “4 Agents” line folds the whole list.

## 3. The header: C. A subthread bar under the parent’s header

*An open subthread*

**Today:** A subthread’s header is a main thread’s: “agentZ / Research: UI for child tasks ⌄”, then the branch and buttons. Nothing in it says it’s a subthread, or which thread started it. The sidebar highlights the parent’s card, since subthreads aren’t in the sidebar.

**C. A subthread bar under the parent’s header** (from Zed subagent title bar): The header shows the parent: “agentZ / i wanna test subthreads, spawn some sub threads… ⌄”. Under it, Zed’s subagent bar: an arrow into the subthread, its title, a green check once done, Stop while it runs, and a Minimize button (–) that opens the parent.

## 4. Anything else that sets it apart: A. Nothing else

*An open subthread*

**Today:** Besides its header (the topic before), a subthread looks like a main thread: the same background and the same conversation. The mocks use header A and the bottom bar A from below.

**A. Nothing else** (from Zed, t3code): Only the header, the start of the conversation and the bottom bar change. Neither Zed nor t3code tints a subthread.

## 5. Above the first message: B. The task as a card

*An open subthread*

**Today:** The first message is the task the parent sent, in your own message’s bubble on the right. A small label above it says “✧ Sent by the agent in ‘i wanna test subthreads, spawn some sub threads…’”. Later messages from the parent get the same label. The mocks use header A.

**B. The task as a card** (from new): The first message is a full-width card instead of a bubble: “Task from ‘i wanna test subthreads…’” with the role (“Research”) in its header, and the task under it.

## 6. In place of the composer: A. What runs, for how long, and Open Parent

*Where the composer would be*

**Today:** A subthread takes messages only from its parent, so a bar replaces the composer: the agent’s icon, “A subagent of ‘…’. It runs on its own; message its parent instead.”, Stop while it runs, and “↗ Open Parent” at the far right. The mocks use header A; option B fits header C.

**A. What runs, for how long, and Open Parent** (from t3code ProviderSubagentBar): The bar t3code puts there: the agent’s icon, the model and effort, “Working 1m 20s” or “Done in 2m 14s”, then “Runs on its own” in muted text, Stop while it runs, and an Open Parent button with an arrow pointing back.

## 7. A shortcut to the parent: A. Ctrl-minus

*The way back*

**Today:** No shortcut, and no command for it. Open Parent is only a button at the bottom right. In every option the command palette gets “Open Parent Thread”, and the way back you pick above shows the shortcut in its tooltip. The mocks show both.

**A. Ctrl-minus** (from Zed): Ctrl-− opens the parent, on macOS and Linux. Zed binds Ctrl-− in its agent thread to Go Back. It works anywhere in a subthread.
