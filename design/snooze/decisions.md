# Snooze: design decisions

Picked on the design board (`design/snooze/`, see `design/README.md`). Each section is the spec for that part: build the picked option as described, with the comments applied. Generated from `choices.json`; don't edit by hand.

## 1. Where you snooze from: A. A clock on the card + B. Snooze ▸ in the thread menu + C. Snooze Until Reset on the limit notice

*Snoozing · pick any*

**Today:** Nothing snoozes a thread. A thread leaves the list only by being archived: the card’s hover button (“Archive”, beside an unpin button for pinned threads), the thread menu (Unpin, Rename, Regenerate Title, Archive, Project Settings, Delete…), or Auto-archive after a day, 3 days or a week without activity. The pictures are recreated from the code.

**A. A clock on the card** (from t3code): t3code: hovering a card shows a clock button (“Snooze Thread”) before Archive; it opens the snooze menu under it. Not shown while the thread waits for your approval or input.

**B. Snooze ▸ in the thread menu** (from t3code): “Snooze” with the same presets as a submenu, after Archive in the thread menu (t3code puts it after Settle). A snoozed thread’s menu has “Wake Now” there instead.

**C. Snooze Until Reset on the limit notice** (from t3code): t3code’s usage-limit banner: “Snooze Until Reset” beside “Continue at 16:10” (and its own limit topic below). Hidden once the thread is snoozed.

## 2. The snooze menu: A. Presets with their times

*Snoozing*

**Today:** There is no snooze. agentZ writes times on a 24-hour clock (“16:10”, “Mon 16:10”), and knows when an account’s used-up limit resets.

**A. Presets with their times** (from t3code): t3code’s presets, each with its time on the right: In 1 hour, In 3 hours, This evening (18:00, only when that’s more than an hour away), Tomorrow (9:00), Next week (Monday 9:00, left out when that’s tomorrow), then Custom…. When the thread’s account has a used-up limit, “Until the limit resets” and its time comes first. Snoozing shows a toast with Undo for 5 seconds, as t3code does.

## 3. Picking another time: C. A duration only

*Snoozing*

**Today:** There is no date or time picker anywhere in agentZ yet.

**C. A duration only** (from t3code (Duration)): A small dialog with t3code’s Duration mode only: “Snooze for” an amount and minutes, hours or days.

## 4. Where snoozed threads go: A. A Snoozed shelf

*While snoozed*

**Today:** The sidebar lists draft rows, then the thread cards (pinned first), then folded shelves: Working (when that setting is on), Chats, Shells, Workspaces and Archived. A shelf’s header says “Archived (6)” folded and “Archived” open; its rows are slim (icon, title, time).

**A. A Snoozed shelf** (from t3code): t3code’s Snoozed shelf, as a shelf here: after the cards and before Chats, “Snoozed (2)” folded, in the accent color. Slim rows, soonest first, each with the time left (“3h”, “1d”) in the accent color; hovering a row shows “Wake Now” in its place. The open thread stays visible even when the shelf is folded.

## 5. When it comes back: A. In its place, marked Woke

*Waking*

**Today:** Cards are ordered by when they last became active (pinned ones first, then by drag). Attention states (Pending Approval, Awaiting Input, Completed) show in the status slot until you open the thread.

**A. In its place, marked Woke** (from t3code): t3code: the thread returns to the cards where it was and shows “Woke” (an alarm clock, in yellow) in its status slot until you open it or click the mark.

## 6. What wakes it early: A. When it needs you, or finishes

*Waking*

**Today:** A thread that needs you shows it on its card: Pending Approval, Awaiting Input, or an error; one whose turn ended shows Completed. Agents can keep working in the background, and a delegated task or subthread can finish later.

**A. When it needs you, or finishes** (from t3code): t3code’s “raised hand”: a snoozed thread wakes early when it asks for permission or input, fails, or a turn finishes after it was snoozed. Snoozing isn’t offered while it already waits for you.

## 7. The open thread while snoozed: B. A callout, like an archived thread’s

*While snoozed*

**Today:** An archived thread shows an Info callout in place of its composer, “This thread is archived”, with Unarchive. Nothing else marks a thread’s state inside it.

**B. A callout, like an archived thread’s** (from agentZ (archived notice)): An Info callout above the composer: “Snoozed until Wed 9:00”, “It comes back to the list then. Sending a message wakes it now.”, and Wake Now. Unlike an archived thread, the composer stays.

## 8. Snoozing until a limit resets: A. On the limit notice

*Usage limits · pick any*

**Today:** When an account reaches a limit, the thread shows a warning callout: “Your account reached its 5-hour limit”, “Claude Agent stopped. The limit resets at 16:10, in 1h 28m.”, with “Continue at 16:10” (the server sends “Continue.” then), Usage ↗, and others when the account has them. An account can also continue at the reset on its own (“When a limit is reached”). The picture is recreated from the code.

**A. On the limit notice** (from t3code): “Snooze Until Reset” beside “Continue at 16:10”. Both can be used: the thread leaves the list and comes back at 16:10, when the server also sends “Continue.”.
