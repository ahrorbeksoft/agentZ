# Usage views: design decisions

Picked on the design board (`design/usage/`, see `design/README.md`). Each section is the spec for that part: build the picked option as described, with the comments applied. Generated from `choices.json`; don't edit by hand.

## 1. How one window reads, in all three views: D. A, colored by pace

*Shared*

**Today:** Two different forms. The Account tab and the thread’s popover use t3code’s row: the window’s name and “26% left”, a bar of what’s left with a hairline where even spending would be, and “resets in 4h 17m” at the right (Accounts round §2: yellow at 15% or less, red when used up). The Usage page instead draws one card per window, with a big “26% left” and the bar split into one chip per account, each with its avatar, email, % and “↻ 4h 19m”. With many accounts the chips truncate and the reset times clip.

**D. A, colored by pace** (from OpenUsage pace colors): A’s row, but the fill’s color says whether the window will last until its reset at the current rate: blue on course, yellow when it will be close (“~4% spare”), red when it will run out first (“runs out in 9h”). Used up is still red.

## 2. The Usage page with many agents and accounts: A. A table per agent

*Settings › Usage*

**Today:** Picked in the Accounts round (§16 A, from t3code’s Usage page): every agent with accounts, and each of its windows as its own card: a big “N% left” (averaged “across N accounts”) and a bar split into one chip per account with its % and reset. Clicking a chip opens the account on its agent’s page. With five agents the page scrolls; with Droid’s 6 accounts and Antigravity’s 4 the chips truncate (“alkimy…”) and the reset times shrink to clipped “↻” icons. Scrolled: 05-usage-screen-scrolled.png; with the mock agent: now-usage-page.png.

**A. A table per agent** (from new, with t3code’s pooled number): Each agent is one card: a row per account, a column per window. Each cell is topic 1’s cell: % left and the reset over a thin bar. Agents with several accounts start with an “All N accounts” row, the average t3code shows today. Clicking a row opens the account on the agent’s page. The External account is first, tagged “Outside”.

## 3. The usage gauge and its popover: A. Today’s popover, tidied

*Threads*

**Today:** Picked in the Accounts round (§16 B, from t3code’s ComposerUsageLimits): beside the agent in the composer, a gauge with the thread’s account’s tightest window (“68%”). Hovering or clicking it opens a 420 px popover: the account’s avatar and email, its plan, “Usage ↗”, then a row per window. In the screenshot the 5-hour row says “resets in 0m” with 68% left, and its hairline sits at the left end. Why: the popover counts down from the last read. With under a minute to go it rounds down to “0m” (format_duration only shows minutes), and once the time passes it says “resets now” until the next read, which comes up to 5 minutes later (usage_reads.rs reads every 5 minutes). Until then it still shows the old 68%. The hairline is right: with no time left in the window, even spending would be at 0. The Account tab in the recording shows the same account a little later, “68% left · resets now”.

**A. Today’s popover, tidied** (from t3code ComposerUsageLimits): Same layout, with topic 1’s row. The header has the account’s name and plan and “Usage ↗”. A footer says when it was read, with Refresh. Under a minute it says “resets in under 1m”; past the reset it says “resetting” and agentZ reads the account again at the reset time, so an old % never stays. The gauge’s tooltip names the window (“Weekly: 3% left”).

## 4. The Account tab with one account: A. Tidied card

*Account tab*

**Today:** Picked in the Accounts round (§1 A, §2 A, §10 A, §11 B): one card per account. The head has the avatar, the email, “Outside agentZ” and “Default” tags, the plan and a ⋯ menu. Under it, a row per window, the usage credits line with Manage ↗, and “When a limit is reached / What threads on this account do” with a Stop menu. You said it looks fine but can be improved.

**A. Tidied card** (from today’s card): The same card, quieter. The limits and the usage credits line stay. Under a hairline, “When a limit is reached” becomes a normal setting row in the body’s size, with its menu at the right. The head says when the limits were read; Refresh Usage stays in the ⋯ menu.

## 5. The Account tab with several accounts: B. Lines, details in a dialog

*Account tab*

**Today:** Every account is a full card: the head, Droid’s pool tabs, a row per window and two setting rows (“When a limit is reached” and “When Factory Droid stops at a limit”). About 220 px per account, so 6 Droid accounts take three screens, and the settings repeat in each card. Scrolled: 07-agent-accounts-multiple-scrolled.png; with the mock agent: now-account-tab.png.

**B. Lines, details in a dialog** (from new): The same lines as A, which never open. Clicking one opens a dialog with the full card: limits, settings and actions.

## 6. Adding an account: A. A dialog from start to result

*Account tab*

**Today:** Picked in the Accounts round (§3 A, §7 B): Add Account puts a “New account” card at the end of the list, with “Copy settings from” and every login method as a row with Log In. Once logged in, the card becomes the account, named by its email. With a few accounts the card is below the screen, so nothing seems to happen when you click Add Account, and a finished login gives no sign either.

**A. A dialog from start to result** (from t3code’s Add account dialog): Add Account opens a dialog. 1: the agent’s login methods as a list, and “Copy settings from” at the bottom. 2: picking one shows its progress in the dialog: a browser login waits with Open the Page Again and Copy Link, a terminal login shows its terminal there, an API key asks for the key. 3: the result: the new account named by its email, its plan and limits, and Done. A failed login says why, with Try Again. If the login is an account that’s already there, it says so and adds nothing. The new account then sits in the list where it belongs.
