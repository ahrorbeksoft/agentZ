# Subscription timeline: design decisions

Picked on the design board (`design/subscription-timeline/`, see `design/README.md`). Each section is the spec for that part: build the picked option as described, with the comments applied. Generated from `choices.json`; don't edit by hand.

## 1. Where the timeline is: Not decided yet

*The timeline*

**Today:** Nothing yet: Settings › Usage has a table per agent, a row per account and a cell per window, with what’s left, the time to its reset and a bar colored by pace. Nothing shows when the windows reset next to each other.

## 2. What a lane is: Not decided yet

*The timeline*

**Today:** Nothing yet. Each account has two or three windows: Claude’s Session and Weekly, Codex’s 5-hour and Weekly, Droid’s 5-hour, Weekly and Monthly, Devin’s Daily and Weekly.

## 3. How lanes are ordered: Not decided yet

*The timeline*

**Today:** The Usage page lists agents in the order they’re installed, and each agent’s accounts in their order there: the External one first.

## 4. The left of a lane: Not decided yet

*The timeline*

**Today:** In the Usage tables an account’s row has its avatar in its color, its name, an Outside tag for the External one, and its plan with its email under it.

## 5. How the current window reads: Not decided yet

*Bars*

**Today:** In the tables a window’s bar is what’s left, from the left, with a hairline where even spending would be; here a bar is the window’s time, from when it opened to when it resets.

## 6. The bars’ color: Not decided yet

*Bars*

**Today:** The tables’ bars are colored by pace (usage round, topic 1): the accent when the window lasts at this rate, yellow when it will be close, red when it runs out first or is used up.

## 7. When a window runs out first: Not decided yet

*Bars*

**Today:** The limit rows say “runs out in 9h 12m” under the reset, with a flame, when what’s used so far, at the same rate, passes the limit before the reset.

## 8. Windows before and after: Not decided yet

*Bars*

**Today:** agentZ knows each window’s length and its next reset only. Windows before the current one and after it can only be worked out from those, as CLIProxyAPI does: back to back, a window’s length apart. A new 5-hour window opens with the first message after a reset, so the ones to come are the earliest they could be.

## 9. Labels and dates: Not decided yet

*Bars*

**Today:** The tables say “68%” and “↻ 2h 40m”, the limit rows “26% left” and “resets in 4h 17m”; times elsewhere in agentZ are relative (“2h”, “3d”).

## 10. Zooming to the short windows: Not decided yet

*Time*

**Today:** Agents name their windows differently: Claude’s Session and Codex’s and Droid’s 5-hour are all five hours long; Devin has a Daily one; Droid a Monthly one.

## 11. Moving in time: Not decided yet

*Time*

**Today:** Nothing yet: Settings › Usage has a table per agent, a row per account and a cell per window, with what’s left, the time to its reset and a bar colored by pace. Nothing shows when the windows reset next to each other.

## 12. Codex’s limit resets: Not decided yet

*Time*

**Today:** A Codex account can have limit resets: use one, and its limits start over. Each expires on a day of its own. The Account tab shows how many and offers Reset Limits.

## 13. Hovering and clicking a bar: Not decided yet

*Details*

**Today:** In the tables a cell’s tooltip has the pace words (“~20% left at reset”, “~12% over the limit at reset”) and the reset’s time; clicking a row opens the account on its agent’s page.

## 14. Accounts with no window counting down: Not decided yet

*Details*

**Today:** A window starts counting down with the first message after a reset. An account that hasn’t been used has all its limits left and no reset time; the tables show 100% with no reset.

## 15. Explaining the bars: Not decided yet

*Details*

**Today:** Nothing yet: Settings › Usage has a table per agent, a row per account and a cell per window, with what’s left, the time to its reset and a bar colored by pace. Nothing shows when the windows reset next to each other.
