# Notification options and volume: design decisions

Picked on the design board (`design/notifications/`, see `design/README.md`). Each section is the spec for that part: build the picked option as described, with the comments applied. Generated from `choices.json`; don't edit by hand.

## 1. When each sound plays: Not decided yet

*Sounds*

**Today:** Each sound has Never, When hidden or Always. When hidden plays whenever the thread isn’t on screen: another app in front, Settings open, another thread open, or its Workspaces tab not showing. So it also plays while you work in agentZ on another thread. Defaults: finished When hidden, input Always.

## 2. What the two new choices are called: Not decided yet

*Sounds*

**Today:** One choice, When hidden, from Zed. The rows’ descriptions say when the sound is for, not where you have to be.

## 3. How you set the volume: Not decided yet

*Volume*

**Today:** No volume: both sounds play at the system’s volume. Zed, t3code and herdr have none either. macOS’s Sound settings have one “Alert volume” slider for all alert sounds. agentZ can set it on both systems: NSSound’s volume on macOS, pw-play’s or paplay’s --volume on Linux.

## 4. Where the volume goes, and hearing it: Not decided yet

*Volume*

**Today:** Picking When hidden or Always plays that sound once. Nothing else plays a sound in Settings. The mocks show the slider (topic 3’s A); the same goes for any control picked there.

## 5. When system notifications show: Not decided yet

*Notifications*

**Today:** One switch, “Notify when agentZ isn’t focused”, on by default: a system notification as an agent finishes or needs input while another app is in front. With agentZ in front you get the sidebar’s state and the sound.

## 6. The notifications section’s name: Not decided yet

*Notifications*

**Today:** The section is called “macOS notifications”, and the switch says “Shows a macOS notification…”, on Linux too, where the desktop shows them.
