# Subthreads, Again: design decisions

Picked on the design board (`design/subthreads-3/`, see `design/README.md`). Each section is the spec for that part: build the picked option as described, with the comments applied. Generated from `choices.json`; don't edit by hand.

## 1. The parent while its subthreads work: B. Working for…, with the count

*While subthreads work*

**Today:** Once the parent’s turn ends with subthreads still at work, its conversation ends with a spinner and “Waiting for 3 subthreads · the agent continues when they end” (picked in Subthreads, Machines and Shells, topic 8), where a turn has “Working for 4m 12s”. The composer shows Send, not Stop: a message goes to the agent at once and starts a turn, and a result that comes back meanwhile is steered into that turn when the agent steers its own turns (Claude Agent, Codex on ACP v1), or waits for it to end. In the sidebar the parent already shows Working while a subthread works.

**B. Working for…, with the count** (from Zed and t3code “Working for”, with t3code Lineage’s “· 3 running”): As A, with how many are still at work after the time: “Working for 4m 12s · 3 subthreads running” (queued ones count as running). The count goes when the last one ends, while the agent takes in the results.

## 2. Subthread rows in the sidebar: E. As it is

*In the sidebar*

**Today:** Each subthread is a row under its parent’s card, newest first: the agent’s icon, the title and its state (Working, Queued, a status pill, or how long ago it ended), then the agent, model, worktree and the lines it changed. While any runs, all of them show, the finished ones too; once the last one ends they fold away, and the count on the card (“⌄ 👥 8”) folds and unfolds them (“Hide Subthreads”). What you chose holds until another starts or the last one ends.

**E. As it is** (from today): All of them while any runs, newest first; folded once the last one ends.

**Comment on E:** just show the completed ones at bottom, that's it, i guesss we don't need completed grouping

## 3. Hundreds of subthreads: A. Caps, with Show more

*Hundreds of subthreads*

**Today:** Nothing is capped. With 100 tasks (8 running, one of them waiting for approval, 30 ended, 62 queued), the sidebar puts all 100 rows under the parent while any runs, the queued ones saying Queued. The parent’s batch card, “Started 100 subthreads · 8 running”, has a 30px row each, so it’s 3,000px tall, in the order the agent listed them. Each results notice, “12 subthreads finished · 1 failed”, opens to a row per result. The server side holds up already: it sends the app only the threads that changed, `delegate_tasks` takes 100 at once, and Subthreads at once (1–16) keeps the rest queued. The picture is the parent with 100 tasks; the sidebar with them is now-sidebar-hundred.png.

**A. Caps, with Show more** (from t3code (Lineage’s and the sidebar’s Show more)): t3code shows a few related threads, then “Show 12 more”. Here: under the parent, the first 6 subthreads (waiting for you, then running, then ended, then queued), then “Show 12 more”; the batch card the same; a results row opens to 6, then Show more.

## 4. The open subthread’s bars: D. Everything on the title bar

*An open subthread*

**Today:** An open subthread has two bars. Under the parent’s header (“agentZ / Implement the backlog in one go ⌄”, whose menu acts on the parent), Zed’s subagent title bar: ↳, the subthread’s title, a green check once done (a red × and a dashed border once failed or stopped), and at its right Stop, a red square, while it can be stopped, and – (Minimize Subthread, back to the parent, Ctrl-−). In the composer’s place, t3code’s bar: the agent’s icon, model and effort, “Working 3m 40s” or “Done in 6m 12s”, “Runs on its own”, then the buttons “■ Stop” and “↖ Open Parent”. Once it has ended both Stops go, and Minimize and Open Parent stay. Ended, it looks like now-subthread-done.png.

**D. Everything on the title bar** (from Zed subagent title bar with t3code’s words): The bottom bar goes and the conversation runs to the bottom. The title bar has ↳, the title and its state, then muted “Opus 5.5 · Working 3m 40s”, Stop while it runs, and –.

## 5. Shells next to the Working label: C. The count on the Working row

*Shells*

**Today:** While a turn runs, the conversation ends with Zed’s dots and “Working for 13m 58s”. The commands Claude Agent leaves running (JetBrains AIR’s background tasks, which agentZ takes) are in the activity bar on the composer’s top edge: “⌄ 1 Background Task”, then a row each with a spinner, the command, “Shell · 13s” and Stop. A command doesn’t hold the turn, so a dev server left running stays in that panel after the turn ends, with no Working row. The picture is the backlog’s: a Claude Agent thread working for 13m 58s, its test run in the panel.

**C. The count on the Working row** (from new): “Working for 13m 58s · 1 shell”, where “1 shell” opens the panel above the composer, which now starts folded. Once the turn ends with a shell running, the panel stays as today.

**Comment on C:** show in a popover or something not in this
