# Subscription timeline: design decisions

Picked on the design board (`design/subscription-timeline/`, see `design/README.md`). Each section is the spec for that part: build the picked option as described, with the comments applied. Generated from `choices.json`; don't edit by hand.

## 1. Where the timeline is: A. Under the tables on the Usage page

*The timeline*

**Today:** Nothing yet: Settings › Usage has a table per agent, a row per account and a cell per window, with what’s left, the time to its reset and a bar colored by pace. Nothing shows when the windows reset next to each other.

**A. Under the tables on the Usage page** (from CLIProxyAPI’s Quota page): Settings › Usage keeps its tables; the timeline comes after them, with every agent’s accounts, as CLIProxyAPI puts its chart under its quota cards.

## 2. What a lane is: A. An account, with its longest window that fits

*The timeline*

**Today:** Nothing yet. Each account has two or three windows: Claude’s Session and Weekly, Codex’s 5-hour and Weekly, Droid’s 5-hour, Weekly and Monthly, Devin’s Daily and Weekly.

**A. An account, with its longest window that fits** (from CLIProxyAPI’s pickLaneWindow): A lane for each account, drawing its longest window that fits the view: the weekly one across two weeks, the 5-hour one when zoomed to it. Droid’s monthly one is longer than two weeks, so it’s left to the table.

## 3. How lanes are ordered: A. Under each agent

*The timeline*

**Today:** The Usage page lists agents in the order they’re installed, and each agent’s accounts in their order there: the External one first.

**A. Under each agent** (from agentZ’s Usage tables): A row with the agent’s icon and name, then its accounts in the tables’ order.

## 4. The left of a lane: B. Avatar, name, plan and window

*The timeline*

**Today:** In the Usage tables an account’s row has its avatar in its color, its name, an Outside tag for the External one, and its plan with its email under it.

**B. Avatar, name, plan and window** (from agentZ’s Usage tables): The account’s avatar and name as in the tables, and its plan and the drawn window’s name under it.

## 5. How the current window reads: A. Filled with what’s used

*Bars*

**Today:** In the tables a window’s bar is what’s left, from the left, with a hairline where even spending would be; here a bar is the window’s time, from when it opened to when it resets.

**A. Filled with what’s used** (from CLIProxyAPI’s windowFill): The bar runs from when the window opened to its reset, filled from the left with what’s used. A fill past the now line means it’s going faster than even.

## 6. The bars’ color: A. By pace

*Bars*

**Today:** The tables’ bars are colored by pace (usage round, topic 1): the accent when the window lasts at this rate, yellow when it will be close, red when it runs out first or is used up.

**A. By pace** (from agentZ’s limit rows (OpenUsage’s pace)): The current window in the tables’ pace colors; the ones to come in the accent.

## 7. When a window runs out first: A. Nothing more

*Bars*

**Today:** The limit rows say “runs out in 9h 12m” under the reset, with a flame, when what’s used so far, at the same rate, passes the limit before the reset.

**A. Nothing more** (from CLIProxyAPI’s chart): Only the bar’s color and its fill say it; the time is in the tooltip.

## 8. Windows before and after: B. The current one and those to come

*Bars*

**Today:** agentZ knows each window’s length and its next reset only. Windows before the current one and after it can only be worked out from those, as CLIProxyAPI does: back to back, a window’s length apart. A new 5-hour window opens with the first message after a reset, so the ones to come are the earliest they could be.

**B. The current one and those to come** (from CLIProxyAPI, without the past): The current window, then the ones after it, dashed, to the end of the view.

## 9. Labels and dates: C. Left, and how long until it resets

*Bars*

**Today:** The tables say “68%” and “↻ 2h 40m”, the limit rows “26% left” and “resets in 4h 17m”; times elsewhere in agentZ are relative (“2h”, “3d”).

**C. Left, and how long until it resets** (from agentZ’s table cells): “13% left · resets in 2d 18h”, as the tables count down.

## 10. Zooming to the short windows: A. Weekly and 5-hour

*Time*

**Today:** Agents name their windows differently: Claude’s Session and Codex’s and Droid’s 5-hour are all five hours long; Devin has a Daily one; Droid a Monthly one.

**A. Weekly and 5-hour** (from CLIProxyAPI’s two zooms): Weekly shows two weeks, a cell a day; 5-hour shows three days, a cell per 6 hours, and only the windows five hours long, whatever the agent names them. Shown here on 5-hour.

## 11. Moving in time: B. Always from today

*Time*

**Today:** Nothing yet: Settings › Usage has a table per agent, a row per account and a cell per window, with what’s left, the time to its reset and a bar colored by pace. Nothing shows when the windows reset next to each other.

**B. Always from today** (from new): No buttons: the view starts at today and shows the two weeks ahead.

## 12. Codex’s limit resets: A. A tick where it expires

*Time*

**Today:** A Codex account can have limit resets: use one, and its limits start over. Each expires on a day of its own. The Account tab shows how many and offers Reset Limits.

**A. A tick where it expires** (from CLIProxyAPI’s manual reset expiry): A yellow tick in the account’s lane on the day the limit reset expires; its tooltip says when.

## 13. Hovering and clicking a bar: C. B, and a click opens the account

*Details*

**Today:** In the tables a cell’s tooltip has the pace words (“~20% left at reset”, “~12% over the limit at reset”) and the reset’s time; clicking a row opens the account on its agent’s page.

**C. B, and a click opens the account** (from agentZ’s Usage tables): B’s tooltip, and clicking a lane opens the account on its agent’s page, as the tables’ rows do.

## 14. Accounts with no window counting down: A. A lane that says so

*Details*

**Today:** A window starts counting down with the first message after a reset. An account that hasn’t been used has all its limits left and no reset time; the tables show 100% with no reset.

**A. A lane that says so** (from CLIProxyAPI’s idle lane): The account keeps its lane, saying “All left · no window counting down”.

## 15. Explaining the bars: A. A legend and a sentence

*Details*

**Today:** Nothing yet: Settings › Usage has a table per agent, a row per account and a cell per window, with what’s left, the time to its reset and a bar colored by pace. Nothing shows when the windows reset next to each other.

**A. A legend and a sentence** (from CLIProxyAPI’s legend): Under the chart: a swatch for each kind of mark, and a sentence on what a bar is.
