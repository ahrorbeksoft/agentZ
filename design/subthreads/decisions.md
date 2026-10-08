# Subthreads: design decisions

Picked on the design board (`design/subthreads/`, see `design/README.md`). Each section is the spec for that part: build the picked option as described, with the comments applied. Generated from `choices.json`; don't edit by hand.

## 1. The rows: Not decided yet

*Agents list*

**Today:** The list above a parent’s composer, under a line like “3 Agents · 1 running”. Each row has a status icon, the title, the agent and the task’s role (“Factory Droid · research”), and a status word: Done, Working, Needs approval, Waiting, Failed, Cancelled or Stopped. The role is what the parent agent passed to `delegate_task`: implementation, research, review, design, test or general. Newest first; six rows show, then it scrolls. A click opens the subthread. In every option the “4 Agents” line above the rows stays as it is.

## 2. Finished subthreads: Not decided yet

*Agents list*

**Today:** Finished subthreads stay in the list for good, and can’t be hidden one by one. You said that’s fine. A click on the “4 Agents” line folds the whole list, which starts open each time the thread opens. The mocks use rows A from the topic before.

## 3. The header: Not decided yet

*An open subthread*

**Today:** A subthread’s header is a main thread’s: “agentZ / Research: UI for child tasks ⌄”, then the branch and buttons. Nothing in it says it’s a subthread, or which thread started it. The sidebar highlights the parent’s card, since subthreads aren’t in the sidebar.

## 4. Anything else that sets it apart: Not decided yet

*An open subthread*

**Today:** Besides its header (the topic before), a subthread looks like a main thread: the same background and the same conversation. The mocks use header A and the bottom bar A from below.

## 5. Above the first message: Not decided yet

*An open subthread*

**Today:** The first message is the task the parent sent, in your own message’s bubble on the right. A small label above it says “✧ Sent by the agent in ‘i wanna test subthreads, spawn some sub threads…’”. Later messages from the parent get the same label. The mocks use header A.

## 6. In place of the composer: Not decided yet

*Where the composer would be*

**Today:** A subthread takes messages only from its parent, so a bar replaces the composer: the agent’s icon, “A subagent of ‘…’. It runs on its own; message its parent instead.”, Stop while it runs, and “↗ Open Parent” at the far right. The mocks use header A; option B fits header C.

## 7. A shortcut to the parent: Not decided yet

*The way back*

**Today:** No shortcut, and no command for it. Open Parent is only a button at the bottom right. In every option the command palette gets “Open Parent Thread”, and the way back you pick above shows the shortcut in its tooltip. The mocks show both.
