# Usage views: design decisions

Picked on the design board (`design/usage/`, see `design/README.md`). Each section is the spec for that part: build the picked option as described, with the comments applied. Generated from `choices.json`; don't edit by hand.

## 1. How one window reads, in all three views: Not decided yet

*Shared*

**Today:** Two different forms. The Account tab and the thread’s popover use t3code’s row: the window’s name and “26% left”, a bar of what’s left with a hairline where even spending would be, and “resets in 4h 17m” at the right (Accounts round §2: yellow at 15% or less, red when used up). The Usage page instead draws one card per window, with a big “26% left” and the bar split into one chip per account, each with its avatar, email, % and “↻ 4h 19m”. With many accounts the chips truncate and the reset times clip.

## 2. The Usage page with many agents and accounts: Not decided yet

*Settings › Usage*

**Today:** Picked in the Accounts round (§16 A, from t3code’s Usage page): every agent with accounts, and each of its windows as its own card: a big “N% left” (averaged “across N accounts”) and a bar split into one chip per account with its % and reset. Clicking a chip opens the account on its agent’s page. With five agents the page scrolls; with Droid’s 6 accounts and Antigravity’s 4 the chips truncate (“alkimy…”) and the reset times shrink to clipped “↻” icons. Scrolled: 05-usage-screen-scrolled.png; with the mock agent: now-usage-page.png.

## 3. The usage gauge and its popover: Not decided yet

*Threads*

**Today:** Picked in the Accounts round (§16 B, from t3code’s ComposerUsageLimits): beside the agent in the composer, a gauge with the thread’s account’s tightest window (“68%”). Hovering or clicking it opens a 420 px popover: the account’s avatar and email, its plan, “Usage ↗”, then a row per window. In the screenshot the 5-hour row says “resets in 0m” with 68% left, and its hairline sits at the left end. Why: the popover counts down from the last read. With under a minute to go it rounds down to “0m” (format_duration only shows minutes), and once the time passes it says “resets now” until the next read, which comes up to 5 minutes later (usage_reads.rs reads every 5 minutes). Until then it still shows the old 68%. The hairline is right: with no time left in the window, even spending would be at 0. The Account tab in the recording shows the same account a little later, “68% left · resets now”.

## 4. The Account tab with one account: Not decided yet

*Account tab*

**Today:** Picked in the Accounts round (§1 A, §2 A, §10 A, §11 B): one card per account. The head has the avatar, the email, “Outside agentZ” and “Default” tags, the plan and a ⋯ menu. Under it, a row per window, the usage credits line with Manage ↗, and “When a limit is reached / What threads on this account do” with a Stop menu. You said it looks fine but can be improved.

## 5. The Account tab with several accounts: Not decided yet

*Account tab*

**Today:** Every account is a full card: the head, Droid’s pool tabs, a row per window and two setting rows (“When a limit is reached” and “When Factory Droid stops at a limit”). About 220 px per account, so 6 Droid accounts take three screens, and the settings repeat in each card. Scrolled: 07-agent-accounts-multiple-scrolled.png; with the mock agent: now-account-tab.png.

## 6. Adding an account: Not decided yet

*Account tab*

**Today:** Picked in the Accounts round (§3 A, §7 B): Add Account puts a “New account” card at the end of the list, with “Copy settings from” and every login method as a row with Log In. Once logged in, the card becomes the account, named by its email. With a few accounts the card is below the screen, so nothing seems to happen when you click Add Account, and a finished login gives no sign either.
