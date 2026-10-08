# Notification options and volume: design decisions

Picked on the design board (`design/notifications/`, see `design/README.md`). Each section is the spec for that part: build the picked option as described, with the comments applied. Generated from `choices.json`; don't edit by hand.

## 1. When each sound plays: A. Four choices in each dropdown

*Sounds*

**Today:** Each sound has Never, When hidden or Always. When hidden plays whenever the thread isn’t on screen: another app in front, Settings open, another thread open, or its Workspaces tab not showing. So it also plays while you work in agentZ on another thread. Defaults: finished When hidden, input Always.

**A. Four choices in each dropdown** (from Zed, one value added): Never, When away from the thread (today’s When hidden), When away from agentZ (only while another app is in front), Always. Away from the thread includes away from agentZ. Settings open counts as away from the thread, not from agentZ. Defaults stay: finished away from the thread, input always.

## 2. What the two new choices are called: B. In another thread, in another app

*Sounds*

**Today:** One choice, When hidden, from Zed. The rows’ descriptions say when the sound is for, not where you have to be.

**B. In another thread, in another app** (from new): “When in another thread” and “When in another app”. Descriptions unchanged.

## 3. How you set the volume: A. One slider for both sounds

*Volume*

**Today:** No volume: both sounds play at the system’s volume. Zed, t3code and herdr have none either. macOS’s Sound settings have one “Alert volume” slider for all alert sounds. agentZ can set it on both systems: NSSound’s volume on macOS, pw-play’s or paplay’s --volume on Linux.

**A. One slider for both sounds** (from macOS Alert volume): A Volume row with a slider from silent to full, applied to both sounds, on top of the system volume. Default full, so nothing changes until you move it.

## 4. Where the volume goes, and hearing it: A. First in Sounds, plays as you let go

*Volume*

**Today:** Picking When hidden or Always plays that sound once. Nothing else plays a sound in Settings. The mocks show the slider (topic 3’s A); the same goes for any control picked there.

**A. First in Sounds, plays as you let go** (from macOS Alert volume): The Volume row heads the Sounds section, since it applies to both rows under it. Letting go of the slider plays the finished sound at the new level, as macOS does.

## 5. When system notifications show: A. Keep the switch

*Notifications*

**Today:** One switch, “Notify when agentZ isn’t focused”, on by default: a system notification as an agent finishes or needs input while another app is in front. With agentZ in front you get the sidebar’s state and the sound.

**A. Keep the switch** (from t3code): As today. It already means away from agentZ, the case you asked for with sounds.

## 6. The notifications section’s name: A. System notifications

*Notifications*

**Today:** The section is called “macOS notifications”, and the switch says “Shows a macOS notification…”, on Linux too, where the desktop shows them.

**A. System notifications** (from new): One name on both systems; the description says “a system notification”.
