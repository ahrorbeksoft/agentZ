# Sounds and notifications: design decisions

Picked on the design board (`design/sounds/`, see `design/README.md`). Each section is the spec for that part: build the picked option as described, with the comments applied. Generated from `choices.json`; don't edit by hand.

## 1. The two sounds: A. t3code’s completion and input

*Sounds*

**Today:** No sounds. You heard these in the chat and picked t3code’s; this topic records it, and its buttons play them again.

**A. t3code’s completion and input** (from t3code): Two short chimes (0.65 s and 0.76 s), bundled as `assets/sounds/` and credited in the architecture doc.

**Comment on A:** Picked in the chat after hearing t3code's, herdr's, Zed's and macOS's sounds.

## 2. When each sound plays: Not decided yet

*Settings*

**Today:** Nothing to set: there are no sounds. General has Threads, Projects and Server sections.

## 3. Notifications: Not decided yet

*Settings*

**Today:** A macOS notification for every thread that isn’t on screen, even with agentZ in front and another thread open. No setting turns it off (only macOS’s own). Clicking one opens its thread.

## 4. Where the rows go: Not decided yet

*Settings*

**Today:** Settings › General has Threads (order, modifier to send, show thinking), Projects and Server. The mocks show the recommended rows; they’ll be the ones you pick above.

## 5. Hearing a sound from Settings: Not decided yet

*Settings*

**Today:** Nothing to preview yet.

## 6. Agents in terminal panes: Not decided yet

*Workspaces*

**Today:** An agent CLI in a Workspaces pane (Claude Code, Codex…) shows its state on the pane and in the sidebar’s Agents list, but makes no sound and no notification. Threads in panes already count as threads.
