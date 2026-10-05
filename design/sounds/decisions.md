# Sounds and notifications: design decisions

Picked on the design board (`design/sounds/`, see `design/README.md`). Each section is the spec for that part: build the picked option as described, with the comments applied. Generated from `choices.json`; don't edit by hand.

## 1. The two sounds: C. Zed’s agent done, and one for input

*Sounds*

**Today:** No sounds. You heard these in the chat and picked t3code’s; this topic records it, and its buttons play them again.

**C. Zed’s agent done, and one for input** (from Zed): Zed’s only agent sound for finishing; Zed has no second one, so input would take t3code’s or herdr’s.

**Comment on A:** Picked in the chat after hearing t3code's, herdr's, Zed's and macOS's sounds.

## 2. When each sound plays: B. A dropdown for each sound

*Settings*

**Today:** Nothing to set: there are no sounds. General has Threads, Projects and Server sections.

**B. A dropdown for each sound** (from Zed, per sound): Zed’s three values on each event’s row. Defaults: finishing plays when hidden, input always (as herdr does, since you need to answer either way).

## 3. Notifications: A. One switch: only while agentZ isn’t focused

*Settings*

**Today:** A macOS notification for every thread that isn’t on screen, even with agentZ in front and another thread open. No setting turns it off (only macOS’s own). Clicking one opens its thread.

**A. One switch: only while agentZ isn’t focused** (from t3code): t3code’s rule: a system notification only when its window isn’t focused. With agentZ in front you get the sidebar’s state and the sound instead. On by default.

## 4. Where the rows go: C. A Notifications page

*Settings*

**Today:** Settings › General has Threads (order, modifier to send, show thinking), Projects and Server. The mocks show the recommended rows; they’ll be the ones you pick above.

**C. A Notifications page** (from new): A page of its own in the settings list, with a bell, after Appearance.

## 5. Hearing a sound from Settings: A. It plays when picked

*Settings*

**Today:** Nothing to preview yet.

**A. It plays when picked** (from macOS Sound settings): Choosing When hidden or Always plays that event’s sound once, as picking an alert sound in macOS does.

## 6. Agents in terminal panes: A. The same as threads

*Workspaces*

**Today:** An agent CLI in a Workspaces pane (Claude Code, Codex…) shows its state on the pane and in the sidebar’s Agents list, but makes no sound and no notification. Threads in panes already count as threads.

**A. The same as threads** (from herdr): As herdr does for its panes: the finished sound when one goes idle after working, the input sound when it gets blocked on a prompt, by the same settings, with the pane counting as on screen when its tab is showing. While agentZ isn’t focused, a notification titled with the agent, “storefront › agents · Finished”; clicking it shows and focuses the pane.
