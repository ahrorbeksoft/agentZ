# Session Notices and Compaction: design decisions

Picked on the design board (`design/session-notices/`, see `design/README.md`). Each section is the spec for that part: build the picked option as described, with the comments applied. Generated from `choices.json`; don't edit by hand.

## 1. Where a notice shows: A. Callouts above the composer

*Notices*

**Today:** agentZ doesn’t tell agents it takes notices (its session capabilities list only config options) and drops any session update it doesn’t know, without logging it. Above the composer it already stacks its own callouts: request elicitations, the usage-limit notice, the login notice, “Conversation not loaded”, errors, then the activity bar. Each is a Callout: a rule along its top, the severity’s icon, a small title, a muted description and ×. The picture is recreated from the code.

**A. Callouts above the composer** (from Zed): Zed’s SessionNotice: each notice is a Callout above the composer, after agentZ’s other notices and before errors. Info has the muted ⓘ, Warning ⚠, Error ⊗; another severity (such as “maintenance”) looks like Info without an icon. Title and description are plain text. × (“Dismiss Notice”) removes it and puts focus back in the thread; nothing is sent to the agent.

## 2. Several notices, and how long they stay: A. Each one, until dismissed

*Notices*

**Today:** There are no notices from agents. agentZ’s own notices each show once (one limit notice, one error), so stacking never comes up.

**A. Each one, until dismissed** (from Zed): Zed: every notice is its own callout, oldest first, repeats included. The list is at most 192 px tall and scrolls beyond that. Each stays until ×. A thread in the background keeps its notices until you open it.

## 3. A notice in a thread you aren’t looking at: A. Nothing outside the thread

*Notices*

**Today:** A thread card shows its project, title, branch, machine and agent, and its status: Working, Pending Approval, Awaiting Input, Completed, or the time. Only those states ask for attention (and their sounds and notifications).

**A. Nothing outside the thread** (from Zed): Zed: a thread in the background keeps its notices and shows them when you open it. The card doesn’t change.

## 4. Compaction in the conversation: A. A divider with a button

*Compaction*

**Today:** Nothing. An agent compacts on its own when its context fills, or when you send its slash command (Claude’s `/compact`); agentZ drops the compaction updates, so the conversation goes on with no mark, and only the context ring under the composer drops (its tooltip: “Context 16% • 31k / 200k”). The picture is recreated from the code.

**A. A divider with a button** (from Zed): Zed’s entry, where the first update arrived: a rule on each side of a small button with the Compact icon. It reads “Compacting Context…” with a spinner while it runs (the “Working for…” row hides meanwhile), then “Context Compacted”, “Compaction Failed” or “Compaction Canceled”. With a summary or an error it has a chevron and opens (next topic); without, it is a plain label.

## 5. Reading the summary: A. Opens in place

*Compaction*

**Today:** agentZ never shows a compaction’s summary. The agent can send one (rich content like a tool’s output, streamed in pieces and replaced by later updates) or an error saying why it failed.

**A. Opens in place** (from Zed): Zed: a click on the divider opens the summary right under it, in a bordered box rendered like a tool’s output, with a bar at its foot that closes it. A failure shows its error in red there. Opening it stops the conversation following the end, so it doesn’t scroll away.

## 6. Asking for a compaction: C. In the context ring

*Compaction*

**Today:** The agent decides when to compact. You can ask for one only with the agent’s own slash command, if it has one (Claude Agent’s `/compact`), which agentZ lists in the composer’s / menu with the agent’s other commands. The context ring turns yellow at 85%.

**C. In the context ring** (from new): A click on the context ring opens a menu with its numbers and Compact Context, which sends the agent’s compact command. Without the command, the ring only has its tooltip.
