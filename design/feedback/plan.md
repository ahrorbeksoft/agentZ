# Feedback: plan

The user's bugs and requests from going through the app on 2026-10-08, one entry each: what they
reported, the evidence, and what they want. The entries weren't researched: whoever picks one up
finds the cause and fixes it. Progress is tracked in `progress.md`.

Screenshots are in `evidence/`. The ones that show the user's account emails are in
`evidence/private/`, which git ignores, so they exist only on the user's Mac.

## 1. Thread cards with draft text look wrong

**Reported:** "I don't like how threads with draft texts look."

**Evidence:** `evidence/01-draft-thread-card.png`, a sidebar thread card for a thread with an unsent draft:
- First row: a yellow pencil-in-square icon (the draft marker), then the project badge "AZ" and the project name "agentZ". "● Completed" is on the right, in green.
- Second row: the title "Current state of the app".
- Third row: the branch "main", with a laptop icon (machine) and the agent's icon on the right.

**Notes for whoever picks this up:**
- The user hasn't said what bothers them. Find out with the design board before changing anything.
- Per AGENTS.md, UI changes start on the design board (`design/README.md`). Make a round with a few options and a screenshot of the current state beside them, and build what the user picks.
- t3code (`references/t3code`) is the model for thread cards. Check how it marks a draft.

## 2. Show the subthread count in the details popover, not in a tooltip

**Reported:** "Gotta put subthreads count in to popover and remove the tooltip."

**Evidence:** `evidence/02-subthread-count-tooltip.png`. The mouse is over the sidebar card for "i wanna test subthreads, spawn some s…", which has 3 subthreads:
- The card's last row has a people icon with "3", then the machine icon and the agent icon.
- Hovering the card shows two things at once: a "3 agents" tooltip near the card, and the details popover beside it.
- The popover has no subthread count. Its rows are: the title, the project ("agentZ"), the machine ("This Mac"), the branch ("main"), and the model and agent ("Opus 5.5 · Factory Droid").

**Wanted:**
- Remove the "3 agents" tooltip.
- Add the subthread count as a row in the details popover, styled like the existing rows.
- Keep the people icon with "3" on the card. Only the tooltip goes.

## 3. A thread with a background task still running shows as done and plays the sound too early

**Reported:** "While there is some task show waiting instead of done or completed, and don't make any sounds until the task is also done and it is actually finished."

**Evidence:** `evidence/03-background-task-running.png`, the thread "Current state of the app" (Claude Agent, Opus 5.5):
- The agent's turn has ended ("Worked for 24s"). Its last message says it will wait for the devbox1 bundle build and carry on once it finishes.
- Above the composer, the "1 Background Task" panel shows "Run the Linux bundle script on devbox1 with a memory cap" (Shell · 4m 26s) still running, with a spinner and a stop button.
- In the sidebar, the card for this thread shows "now" in its top-right corner.
- Possibly related: in `evidence/01-draft-thread-card.png`, the card for the same thread shows "● Completed". It isn't confirmed that a background task was running when that was taken.

**Wanted:**
- While a thread has a background task running, show it as waiting, not done or completed, everywhere its status appears.
- Don't play the finished sound when the turn ends while a task is still running. Play it only once the task has ended and the agent has actually finished the work that follows.

**Testing:** the mock agent's `background-task [seconds]` prompt leaves a command running after its turn, then goes on by itself once it ends (see AGENTS.md, Testing).

## 4. Subthreads play sounds

**Reported:** "There is sound on every subthread finish. Subthreads are handled by main threads, even input required should be silent and handled by the main threads."

**Evidence:** none attached. The user hears a sound each time a subthread finishes.

**Wanted:**
- Subthreads never play a sound: not when they finish, and not when they need input (input required).
- The main thread that spawned them handles them, so only the main thread's own sounds play.
- Related to bug 3, which also changes when the finished sound plays.

## 5. The Usage screen needs a better view

**Reported:** "I kinda dislike this usage screen, we need better view."

**Evidence:** two screenshots of Settings → Usage on "This Mac" (they show the user's account emails, so they're kept out of git):
- `evidence/private/05-usage-screen-top.png`: one section per agent (Claude Agent, Codex, Devin, Factory Droid, …). Each limit window (Session, Weekly, Monthly, Daily, 5-hour) is its own card: a big "N% left" on the left, and a bar on the right with an account chip (avatar letter, email, percent) and the reset time ("↻ 4h 19m").
- `evidence/private/05-usage-screen-scrolled.png`: agents with several accounts (Factory Droid with 6, Google Antigravity with 4). The card says "N% left across N accounts", and the bar splits into one small chip per account.

**Visible in the screenshots (not confirmed as what the user dislikes):**
- One card per window makes the page long. Five agents already need scrolling.
- With one account, a single chip stretches across the whole bar.
- With several accounts, the chips are narrow: names truncate ("alkimy…", "ahrorb…"), and reset times shrink to clipped "↻" icons between chips.
- Some fills are hard to tell apart from the empty part of the bar.

**Notes for whoever picks this up:**
- The user hasn't said what a better view looks like. This is a UI change, so it starts on the design board (`design/README.md`): a round with a few layouts and the current screen beside them, then build what's picked.
- Check earlier decisions about this screen in `design/accounts/` before proposing designs.

## 6. The usage popover in threads needs a better design

**Reported:** "We gotta improve this design (the usage in the threads)."

**Evidence:** `evidence/private/06-thread-usage-popover.jpeg` (shows an account email, so it's kept out of git). In a Factory Droid thread, the composer's bottom-left shows "Factory Droid" with a usage chip ("68%"). Hovering it opens a popover above the composer:
- Header: the account's avatar letter and email, and a "Usage ↗" link on the right.
- One row per window: "5-hour 68% left", "Weekly 88% left", "Monthly 94% left". Each row has a bar, and the right column says when it resets ("resets in 0m", "resets in 4d 23h", "resets in 27d 23h").
- The Weekly and Monthly bars have a thin vertical tick on them. The 5-hour bar has one at its left end.

**Visible in the screenshot (not confirmed as what the user dislikes):**
- The 5-hour row says "resets in 0m" while showing 68% left. That may be stale data or a separate bug. Check it while working on this.

**Notes for whoever picks this up:**
- The user hasn't said what they want instead. This is a UI change, so it starts on the design board (`design/README.md`).
- Design it together with bug 5 (the Usage screen in Settings), so the two views show usage the same way.

## 7. An agent's Account tab gets messy with several accounts, and Add Account gives no feedback

**Reported:** "In per agent view if a single agent is there looks fine, not bad, but can be improved. But when we have multiple accounts it gets messy. And Add Account adds to the end, we don't even know if it is there or not, so Add Account should be a modal or something."

**Evidence** (both show account emails, so they're kept out of git):
- `evidence/private/07-agent-account-single.png`: Agents → Claude Agent ("Logged in", v0.87.0), Account tab, on "This Mac". Under "Accounts", with "+ Add Account" on the right, there is one account card:
  - The avatar letter, the email, an "Outside agentZ" badge, the plan ("Pro") and a "···" menu.
  - "Session 26% left" and "Weekly 13% left", each with a bar, a thin tick on the bar, and when it resets ("resets in 4h 17m", "resets in 2d 18h").
  - "Usage credits Off" with a "Manage ↗" link.
  - "When a limit is reached / What threads on this account do", with a "Stop" dropdown.
- `evidence/private/07-agent-accounts-multiple.mov`: the user's screen recording of the same tab with several accounts (copied from the Desktop). Not reviewed while writing this entry. Watch it for what "messy" means.

**Wanted:**
- With one account: it's acceptable, but can be improved.
- With several accounts: make the list clear instead of messy.
- Add Account: a new account is now added at the end of the list, so the user can't tell whether it was added. Make adding an account a modal (or something similar) that shows its progress and result.

**Notes for whoever picks this up:**
- This is a UI change, so it starts on the design board (`design/README.md`).
- Design it together with bugs 5 and 6, which also show account usage, so all three views match.
- Check earlier decisions in `design/accounts/` first.

## 8. In some themes, the Agents / Workspaces switcher doesn't show which view is active

**Reported:** "In some accounts [themes], this is just flat, can't know which is now active. We gotta find something that's common in all the themes. We mostly import themes from Zed, I want it to be compatible with any Zed theme."

**Evidence:**
- `evidence/08-view-switcher-flat.png`: the "Agents | Workspaces" switcher in the title bar, in a purple-toned theme. Both buttons look the same, so nothing shows which view is active. Which theme it is wasn't recorded.
- For comparison, `evidence/private/05-usage-screen-top.png` (top right) shows the same switcher in another theme, where the active "Agents" button has a visibly different background.

**Wanted:**
- The active view is clearly marked in every theme.
- Build the selected style only from theme colors that every Zed theme defines, so any theme imported from Zed works. Don't rely on colors some themes leave out or set to nearly the same value as the background.
- Check the fix against several of the bundled themes, including ones where it looks flat today.

## 9. The thread title is trimmed even on a wide screen, and editing shows the trimmed title

**Reported:** "The thread title is trimmed, even in a wide screen, and edits also show the trimmed one."

**Evidence:** the thread header of a Factory Droid thread in agentZ. The thread's first message, which the title comes from, is "i want you to draft a plan to improve a couple of bugs and things, for now just use temp dir, i'll give you bugs one by one".
- `evidence/09-thread-title-trimmed.png`: the header reads "agentZ / i want you to draft a plan to improve a couple… ⌄". Right after the chevron there's a lot of empty space before "main", "+33 −17" and the buttons on the right.
- `evidence/09-thread-title-editing.png`: while renaming, the text field holds "i want you to draft a plan to improve a couple…", with the "…" inside the text being edited.

**Visible in the screenshots (not confirmed):** the title is cut even though there's room for it, and the "…" is part of the edited text. So the cut likely happens in the title itself, not in the layout.

**Wanted:**
- Show the full title in the header, and trim it only when it doesn't fit the available width.
- When renaming, the field holds the full title, not the trimmed one.

## 10. Simplify thread terminals

**Reported:** "We gotta simplify thread terminals."

**Evidence:** `evidence/10-thread-terminal.png`, a thread's terminal pane:
- A zsh shell in agentZ ("Last login: …", then the prompt "➜ agentZ git:(main) ✗").
- In the top-right corner, a bordered toolbar drawn over the terminal with five buttons: split side by side, split top and bottom, new terminal (+), maximize, and close (×).

**Notes for whoever picks this up:**
- The user hasn't said what to simplify. This is a UI change, so it starts on the design board (`design/README.md`), with the current pane screenshotted beside the options.
- herdr (`references/herdr`) is the model for terminal panes, and Zed (`references/zed`) for its terminal panel. Compare how they do it.

## 11. Subthreads: the Agents list looks off, an open subthread looks like a main thread, and the way back is hard to find

**Reported:** "Subthreads are not hidable from there, that's not bad, but looks not good, maybe the labels. And also when we open any subthread it should be kinda a little different than the main thread, and also the back button was hard to find."

**Evidence:**
- `evidence/11-agents-list.png`: the Agents list above the composer in a main thread, open, with the header "⌄ 3 Agents". Three rows, each: a green check, the title ("Research: UI for child tasks", "Research: how agents get agentZ's tools", "Research: server side of delegated tasks"), then on the right "Factory Droid · research" and a green "Done".
- `evidence/11-subthread-open.jpeg`: the subthread "Research: UI for child tasks" open.
  - The header reads "agentZ / Research: UI for child tasks ⌄", the same as a main thread's.
  - Above the first message, a small label says "✧ Sent by the agent in 'i wanna test subthreads, spawn some sub threads…'".
  - The conversation looks the same as a main thread's. Above the bottom bar is a "Plan" row with "All Done ×".
  - In place of the composer, a bar reads "A subagent of 'i wanna test subthreads, spawn some sub threads…'. It runs on its own; message its parent instead." The only way back is "↗ Open Parent" at the far right of that bar.

**Wanted:**
- Agents list: finished subthreads can't be hidden from it. That's acceptable, but the rows don't look good, possibly because of their labels ("Factory Droid · research", "Done"). Improve how the rows look.
- An open subthread looks a little different from a main thread, so it's clear you're inside a subthread.
- Make the way back to the parent thread easy to find. Today it's only "Open Parent" in the bottom-right corner.

**Notes for whoever picks this up:**
- This is a UI change, so it starts on the design board (`design/README.md`).
- Related: bug 2 (subthread count in the details popover) and bug 4 (subthread sounds).
- t3code's `ProviderSubagentBar` (`references/t3code`) is what the Agents list was based on.

## 12. Improve how threads show images and tool calls

**Reported:** "Improve photo views, and also tool calls."

**Evidence:** `evidence/12-read-image-tool-call.png`, a tool call in a thread where the agent read an image:
- The title row reads "Read /tmp/az-linux-rel.png", with a magnifying-glass icon.
- Under it, a code block shows the tool's raw input as JSON: `{ "file_path": "/tmp/az-linux-rel.png" }`, which repeats the path already in the title.
- Under that, the image (a screenshot of agentZ's welcome screen on Linux) is drawn small, inside a wide gray frame. Most of the frame is empty, and the image's text is too small to read.

**Visible in the screenshot (not confirmed as what the user dislikes):**
- The raw JSON input repeats the title.
- The image is small and can't be read at this size. Nothing shows a way to enlarge it.
- A file read gets a search icon.

**Notes for whoever picks this up:**
- Zed (`references/zed`, `crates/agent_ui`) is the model for the thread UI. Compare how it shows tool calls and images, and match it.
- This is a UI change, so it starts on the design board (`design/README.md`), covering both images and tool calls.

## 13. ToolSearch and agentZ's own tool calls look raw

**Reported:** "ToolSearch, and also we gotta do something like 'started a subthread' or something, idk, we gotta make these look good you know."

**Evidence:**
- `evidence/13-toolsearch-call.png`: a "ToolSearch" tool call with a magnifying-glass icon.
  - A code block shows its raw JSON input: `{ "query": "select:agentz___orchestrator_capabilities,agentz___agentz_thread_launch,agentz___create_thread…`.
  - A second block shows its output: "Loaded 8 tool(s): agentz___orchestrator_capabilities, agentz___agentz_thread_launch, agentz___create_threa…".
  - Both lines run off the right edge, cut mid-word instead of wrapping.
- `evidence/13-delegate-task-calls.png`: three tool calls in a row, each just a hammer icon and the raw tool name "agentz___delegate_task", with the MCP server prefix and triple underscore. Nothing says what each one did.

**Wanted:**
- Show ToolSearch calls in a readable way, instead of raw JSON and cut-off lines.
- Show calls to agentZ's own tools as what they did, for example "Started a subthread" for `delegate_task` (perhaps naming the subthread), instead of the raw tool name.
- Overall, these tool calls should look good.

**Notes for whoever picks this up:**
- Related to bug 12 (tool calls in general), so design them in the same round on the design board (`design/README.md`).
- Related to bug 11: a "started a subthread" row could link to the subthread.

## 14. Let agents manage workspaces, terminals and projects on any machine

**Reported:** "We gotta expand workspaces support for agents. Agents should be able to manage workspaces' terminals. For example, an agent should be able to open a terminal on a remote machine, clone a repository and create a project on that machine. When that happens, if there are two projects that match across machines, they should match (they do). And they should be able to delegate tasks to another machine, but below this thread. You get the idea."

**Evidence:** none attached. This is a feature request for agent control (agentZ's tools for agents).

**Starting point:** an agent in an agentZ thread sees these tools today. Their names are listed here; what each one supports, including which machines it can reach, wasn't checked.
- Threads: `orchestrator_capabilities`, `agentz_thread_list`, `agentz_thread_read`, `agentz_thread_launch`, `create_threads`, `agentz_thread_send`, `agentz_thread_wait`, `agentz_thread_interrupt`, `agentz_thread_update`, `agentz_thread_organize`, `agentz_thread_diff`.
- Tasks: `delegate_task`, `task_status`, `task_cancel`.
- Workspaces: `agentz_workspace_status`, `agentz_workspace_list`, `agentz_workspace_handoff`, `agentz_workspace_sync`, `agentz_workspace_bring_back`.
- Terminals: `agentz_terminal_list`, `agentz_terminal_start`, `agentz_terminal_send`, `agentz_terminal_read`, `agentz_terminal_wait`.

**Wanted:**
- Agents can manage the terminals in Workspaces, on any of the user's machines, not only their own.
- Example flow an agent should be able to do on its own:
  1. Open a terminal on a remote machine.
  2. Clone a repository there.
  3. Create a project on that machine from the clone.
- When a project created this way matches a project on another machine, the two are matched as one project. This already works for projects the user adds; keep it working for projects an agent creates.
- Agents can delegate a task to another machine. The task's subthread still sits under the thread that delegated it, like a local subthread.

**Notes for whoever picks this up:**
- Agent control's policy decides what a thread may do. Decide what these new powers need (for example, asking the user before acting on another machine) and say what you chose.
- Related: bug 11 (how subthreads look) and bug 13 (how agentZ's tool calls look). A subthread on another machine should say which machine it runs on.

## 15. Settings lists a combined project once per machine

**Reported:** "We gotta merge projects in the settings too, with a dropdown or select machines."

**Evidence:** `evidence/15-settings-projects-per-machine.png`, Settings with the project "ielts-today" open:
- Under "Projects", the settings sidebar lists each machine's copy as its own entry: "agentZ", "ielts-today", "ielts-today · Ahrorbek's Laptop", "fluency.uz", "fluency.uz · Devbox 1", "fluency.uz · Ahrorbek's Laptop".
- The "ielts-today" page already knows they're one project. Under Repository, "Combined with" says "Ahrorbek's Laptop: /home/ahrorbek/projects/ielts-today. Name and icon changes apply to all of them."
- The page's sections are Project (Name, Icon, Monogram, Folder), Repository (Repository, Grouping, Combined with), Checkouts and Danger. Folder and Checkouts belong to one machine's copy.

**Wanted:**
- In the settings sidebar, list a combined project once, as the sidebar elsewhere already does.
- On its page, choose the machine with a dropdown or a machine selector. Settings shared by all copies (name, icon, monogram) are shown once. Settings that belong to one copy (folder, checkouts, removing it) follow the chosen machine.

**Notes for whoever picks this up:**
- This is a UI change, so it starts on the design board (`design/README.md`), where the user can pick between a dropdown and a machine selector.
- The Usage page already has a machine picker in its top-right corner ("This Mac", see `evidence/private/05-usage-screen-top.png`). Consider matching it.

## 16. Find project icons the way t3code does

**Reported:** "We gotta adopt a project icon scan from t3code."

**Evidence:** `evidence/15-settings-projects-per-machine.png`. The project settings say the icon is "Automatic: the project's favicon, or a monogram". For "ielts-today" (`/Users/ahrorbek/projects/svelte5/ielts-today`, a project in a `svelte5` folder), it shows the "IT" monogram, so the automatic scan found no favicon there. Whether the project has one that t3code would find wasn't checked.

**Wanted:**
- Replace agentZ's favicon scan with t3code's: the same places to look and the same order (`references/t3code`).
- Keep the monogram as the fallback when nothing is found.

**Notes for whoever picks this up:**
- Test it on the user's projects that show a monogram today, such as ielts-today and fluency.uz, and check each machine's copy.

## 17. The account color fills the agent icon instead of sitting behind it, and the details popover doesn't name the account

**Reported:** "Account color in agents should not fill the icon but should be the background. It was like that in the selected design, but the agent implemented it incorrectly. And also, in the details show the account too (if there are more accounts of that agent)."

**Evidence:**
- `evidence/17-account-color-on-icon.png`: two sidebar cards, both "agentZ / what's this project about? / main", both "Working", both Google Antigravity threads. At the bottom right of each, the Antigravity logo itself is drawn in its account's color: teal on the first card, green on the second. There's no colored background behind the icon.
- `evidence/17-details-without-account.png`: the details popover for one of those threads. Its rows are the title, the project ("agentZ"), the machine ("This Mac"), the branch ("main"), and "Gemini 3.8 Flash (High) · Google Antigravity". Nothing says which account the thread uses.

**Wanted:**
- Draw the agent icon in its usual colors, on a background in the account's color, as in the design the user picked. The icon itself isn't recolored.
- In the details popover, show which account the thread uses, when that agent has more than one account. With a single account, leave it out.

**Notes for whoever picks this up:**
- Find the picked design in the design board's decisions (likely `design/accounts/`) and match it exactly. This is a fix to what was already decided, so no new round is needed for the icon.
- Related to bug 2, which also adds a row to the details popover. Style the two new rows the same way.

## 18. More notification options, and a volume control

**Reported:** "We need more options for notifications, because 'when hidden' here means when you are outside of the thread, not [outside] the app. And also we gotta be able to change the volume."

**Evidence:** none for this one. The screenshot sent with it was the same details popover as `evidence/17-details-without-account.png`, so it wasn't saved again.

**Wanted:**
- Today the notification setting's "when hidden" choice means the thread isn't the one in view, even while the user is in the app. Add more choices, so the user can also pick "only when agentZ isn't the active app", in addition to the current one.
- Make the choice names say clearly which one they mean (outside the thread, or outside the app).
- Add a volume setting for agentZ's sounds.

**Notes for whoever picks this up:**
- Check Settings → Notifications for the current choices before naming the new ones, and keep the names short.
- Related to bugs 3 and 4, which change when sounds play (background tasks, subthreads). The new choices apply on top of those rules.
- This changes the settings UI, so per AGENTS.md it starts on the design board (`design/README.md`), even if it's a small round.

## 19. Scrolling an agent's accounts feels buggy

**Reported:** "Are lists virtualized? Agent accounts scroll kinda feels buggy."

**Evidence:** none attached, and the user hasn't said how it misbehaves (jumps, stutters, lags, resets). `evidence/private/07-agent-accounts-multiple.mov` (bug 7) shows that tab with several accounts and may show it.

**What's known (a quick look at `crates/app/src`, not a diagnosis):**
- Virtualized (only visible rows are drawn): the thread conversation (`agent_view.rs`, `ListState` + `list`), the diff panel (`diff_panel.rs`), and the agent registry's cards (`settings_page.rs`, `uniform_list("registry-cards", …)`).
- Not virtualized (every row drawn in a plain `overflow_y_scroll` div): the sidebar, every other settings page, the agent's Account tab included (`settings_page.rs`, "settings-content"), and the modals and pickers.
- An agent has a handful of accounts, so drawing them all shouldn't be slow by itself. The cause is likely something else, for example the page re-laying out while usage refreshes, or the scroll position being reset. Unconfirmed.

**Wanted:**
- Scrolling the Account tab with several accounts feels smooth and stays where the user puts it.
- Find the actual cause before changing anything. Reproduce it with a mock agent with several accounts against a scratch data directory (AGENTS.md, Testing).
- Virtualize a list only if the cause turns out to be its size.

## 20. Stop installing the server with install.sh

**Reported:** "We gotta remove installing the server via the install.sh script. The client installs it automatically, why bother. Uploading an install.sh in the release? Just stupid."

**Evidence:** none attached.

**Wanted:**
- `install.sh` no longer installs `agentz-server`. The app installs the server on each machine itself, so the script doesn't need to.
- The release no longer uploads `install.sh` as an asset.
- Remove whatever only existed for that path (docs, site links, release steps), and keep what the app's own server install needs.

**Notes for whoever picks this up:**
- Find out what else `install.sh` does before deleting it. If installing the server is all it does, remove the script entirely. If it also installs the app, ask the user whether that part stays.
- `site/install.sh` and `.github/workflows/release.yml` changed last in 9e31cba ("Run the app on Linux, packaged as Zed packages Zed"), along with `tooling/bundle-linux.sh`. Read that commit first, so the Linux packaging keeps working.
- Update `docs/architecture.md` and the README if they describe installing with `install.sh`.

