# Narrow threads: design decisions

Picked on the design board (`design/thread-narrow/`, see `design/README.md`). Each section is the spec for that part: build the picked option as described, with the comments applied. Generated from `choices.json`; don't edit by hand.

## 1. How the picker marks this thread’s account: Not decided yet

*Account picker*

**Today:** In a sent thread, the current account has a blue check at the right and “This thread’s account” in place of its plan under its name. A new thread’s picker marks it with the check alone.

## 2. What gives way when the header is narrow: Not decided yet

*Thread header*

**Today:** The project’s icon and name, “/”, the title, the fork and artifact pills when there are any, the branch (up to 200 px), the changes, Terminal and ⋯. The branch and everything after it never shrink, so the project’s name goes first, then the title: at 440 px the title is “Fix the logi…”, at 360 px it’s gone, and at 320 px the project’s icon, “/” and the title’s chevron draw over each other.

## 3. How the composer’s controls fit when narrow: Not decided yet

*Composer*

**Today:** Zed’s footer: the agent, the account and the gauge on the left; the context ring, Mode, Model, Effort and Send on the right. Under about 560 px the right group wraps to a second line that starts at the left, so Send leaves the corner and the ring sits at the far left.
