# Narrow threads: design decisions

Picked on the design board (`design/thread-narrow/`, see `design/README.md`). Each section is the spec for that part: build the picked option as described, with the comments applied. Generated from `choices.json`; don't edit by hand.

## 1. How the picker marks this thread’s account: D. Under its own heading

*Account picker*

**Today:** In a sent thread, the current account has a blue check at the right and “This thread’s account” in place of its plan under its name. A new thread’s picker marks it with the check alone.

**D. Under its own heading** (from new): “This thread” over the current account, then “Switch to” over the others. No check.

## 2. What gives way when the header is narrow: C. The branch and Terminal into ⋯

*Thread header*

**Today:** The project’s icon and name, “/”, the title, the fork and artifact pills when there are any, the branch (up to 200 px), the changes, Terminal and ⋯. The branch and everything after it never shrink, so the project’s name goes first, then the title: at 440 px the title is “Fix the logi…”, at 360 px it’s gone, and at 320 px the project’s icon, “/” and the title’s chevron draw over each other.

**C. The branch and Terminal into ⋯** (from t3code (compact controls menu)): Below 480 px the branch and Terminal leave the header and head the ⋯ menu: the branch as a line at its top, then Toggle Terminal. Above 480 px, as A.

**Comment on C:** influde changes, and artifacts inside the the ... on small screens

## 3. How the composer’s controls fit when narrow: B. Icons for the agent and account

*Composer*

**Today:** Zed’s footer: the agent, the account and the gauge on the left; the context ring, Mode, Model, Effort and Send on the right. Under about 560 px the right group wraps to a second line that starts at the left, so Send leaves the corner and the ring sits at the far left.

**B. Icons for the agent and account** (from t3code (icon-only controls)): Below 560 px the agent picker shows only its icon and the account only its avatar (their names in the tooltips). One line down to about 400 px; under that, A’s wrap.
