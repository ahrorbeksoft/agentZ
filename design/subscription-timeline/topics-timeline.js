// Subscription timeline: when each account's limits come back, drawn as CLIProxyAPI draws its
// credentials' quota windows. Where it is, what a lane is, how a bar reads, moving in time, and
// the details.

const NOW_NONE = 'Nothing yet: Settings › Usage has a table per agent, a row per account and a cell per window, with what’s left, the time to its reset and a bar colored by pace. Nothing shows when the windows reset next to each other.';
const SOME = (...ids) => ids.map(agentById);

// Today's Account tab (settings_page.rs render_agent_page), for the timeline under it.
const agentPage = (body) => `<div class="tl-pghead" style="height:22px;font-size:12px;color:var(--mu)">${ic('arrow-left', 'xs')}Agents</div>
  <div class="row g3" style="margin:14px 0 16px"><span style="width:40px;height:40px;border-radius:8px;background:var(--sel);display:grid;place-items:center;font-size:18px">${GLYPHS.droid}</span><div class="col grow" style="gap:3px"><span class="row g2" style="font-size:15px">Factory Droid<span class="tl-chip" style="color:var(--ok)">● Logged in</span></span><span style="font-size:11px;color:var(--ph)">v0.235.0 · Factory’s coding agent</span></div></div>
  <div class="row" style="gap:16px;font-size:12px;color:var(--mu);border-bottom:1px solid var(--b);height:28px;margin-bottom:14px"><span style="color:var(--t);border-bottom:1px solid var(--t);height:27px;display:flex;align-items:center">Account</span><span>Defaults</span><span>Environment</span><span>Threads</span></div>
  <div class="row" style="font-size:12px;color:var(--mu);margin-bottom:8px"><span class="grow">Accounts</span>${ic('plus', 'xs')}&nbsp;Add Account</div>${body}`;

// The timeline ------------------------------------------------------------------------------------
TOPICS.push({
  id: 'place', section: 'The timeline', title: 'Where the timeline is', size: 'wide', rec: 'A',
  now: NOW_NONE,
  nowImg: 'img/now-usage-page.png',
  options: [
    { key: 'A', name: 'Under the tables on the Usage page', from: 'CLIProxyAPI’s Quota page',
      desc: 'Settings › Usage keeps its tables; the timeline comes after them, with every agent’s accounts, as CLIProxyAPI puts its chart under its quota cards.',
      good: 'One chart compares every account, across agents; the tables stay as they are.', cost: 'Below the fold once there are several agents.',
      mock: () => settingsPage(`${usageHead()}${usageTable(agentById('codex'))}${usageTable(agentById('devin'))}<div style="height:18px"></div>${timeline({ agents: SOME('claude', 'codex', 'droid') })}`, { h: 930 }) },
    { key: 'B', name: 'In each agent’s card', from: 'new',
      desc: 'Each agent’s table gets its accounts’ lanes under its rows, with the days across the top.',
      good: 'Each agent’s accounts, the ones that take each other’s turns, side by side.', cost: 'A chart per agent, with the days repeated; agents can’t be compared.',
      mock: () => settingsPage(`${usageHead()}${['claude', 'codex'].map((id) => `${usageTable(agentById(id)).replace('class="tl-card"', 'class="tl-card" style="border-bottom-left-radius:0;border-bottom-right-radius:0"')}<div style="margin-top:-1px">${timeline({ agents: SOME(id), header: false, legend: 'none' }).replace('class="tl" style="', 'class="tl" style="border-top-left-radius:0;border-top-right-radius:0;')}</div>`).join('')}`, { h: 700 }) },
    { key: 'C', name: 'A Table and Timeline switch', from: 'new',
      desc: 'The Usage page’s header gets a switch: the tables as today, or the timeline in their place.',
      good: 'The timeline gets the whole page; the tables stay one click away.', cost: 'What’s left and when it comes back are never on screen together.',
      mock: () => settingsPage(`${usageHead(`<span class="tl-seg"><span>${ic('table', 'xs')}Table</span><span class="on">${ic('timeline', 'xs')}Timeline</span></span>${machinePicker()}`)}<div style="height:14px"></div>${timeline({ agents: SOME('claude', 'codex', 'droid') })}`, { h: 720 }) },
    { key: 'D', name: 'On each agent’s Account tab', from: 'agentZ’s Account tab',
      desc: 'Under the accounts on the agent’s Account tab, with that agent’s accounts only.',
      good: 'Beside the accounts you switch between.', cost: 'One agent at a time; not on the Usage page.',
      mock: () => settingsPage(agentPage(`${usageTable(agentById('droid'), { title: false })}<div style="height:16px"></div>${timeline({ agents: SOME('droid') })}`), { section: 'Agents', h: 1110 }) },
  ],
});

TOPICS.push({
  id: 'lanes', section: 'The timeline', title: 'What a lane is', size: 'wide', rec: 'A',
  now: 'Nothing yet. Each account has two or three windows: Claude’s Session and Weekly, Codex’s 5-hour and Weekly, Droid’s 5-hour, Weekly and Monthly, Devin’s Daily and Weekly.',
  nowImg: 'img/now-account-tab.png',
  options: [
    { key: 'A', name: 'An account, with its longest window that fits', from: 'CLIProxyAPI’s pickLaneWindow',
      desc: 'A lane for each account, drawing its longest window that fits the view: the weekly one across two weeks, the 5-hour one when zoomed to it. Droid’s monthly one is longer than two weeks, so it’s left to the table.',
      good: 'Two or three readable bars a lane.', cost: 'The 5-hour windows only when zoomed in.',
      mock: () => panel(timeline({ agents: SOME('claude', 'droid') })) },
    { key: 'B', name: 'A lane for each window', from: 'new',
      desc: 'Every window of every account in the view gets its own lane: Session and Weekly for each Claude account.',
      good: 'Every window there is, at once.', cost: 'Twice the lanes; across two weeks the 5-hour ones are a row of slivers.',
      mock: () => panel(timeline({ agents: SOME('claude'), lanePer: 'window' })) },
    { key: 'C', name: 'An account, with its tightest window', from: 'agentZ’s usage gauge',
      desc: 'A lane for each account, drawing the window that stops it first, the one the composer’s gauge shows.',
      good: 'Shows what’s holding each account back now.', cost: 'Lanes mix 5-hour slivers and week-long bars.',
      mock: () => panel(timeline({ agents: SOME('claude', 'droid'), pick: 'tightest' })) },
  ],
});

TOPICS.push({
  id: 'group', section: 'The timeline', title: 'How lanes are ordered', size: 'wide', rec: 'A',
  now: 'The Usage page lists agents in the order they’re installed, and each agent’s accounts in their order there: the External one first.',
  options: [
    { key: 'A', name: 'Under each agent', from: 'agentZ’s Usage tables',
      desc: 'A row with the agent’s icon and name, then its accounts in the tables’ order.',
      good: 'Reads like the tables above it.', cost: 'A row for each agent.',
      mock: () => panel(timeline({ agents: SOME('claude', 'codex', 'droid') })) },
    { key: 'B', name: 'Soonest reset first', from: 'CLIProxyAPI’s Soonest recovery sort',
      desc: 'One list, the account whose window resets first at the top, each with its agent’s icon.',
      good: 'The next account to come back is first.', cost: 'The order changes as windows reset.',
      mock: () => panel(timeline({ agents: SOME('claude', 'codex', 'droid'), group: 'sorted', sort: true })) },
    { key: 'C', name: 'One list in the tables’ order', from: 'CLIProxyAPI’s chart',
      desc: 'One list with no agent rows, each lane with its agent’s icon, as CLIProxyAPI’s lanes have their provider’s color.',
      good: 'The shortest chart.', cost: 'Agents are told apart by small icons only.',
      mock: () => panel(timeline({ agents: SOME('claude', 'codex', 'droid'), group: 'flat-icon' })) },
  ],
});

TOPICS.push({
  id: 'head', section: 'The timeline', title: 'The left of a lane', size: 'wide', rec: 'B',
  now: 'In the Usage tables an account’s row has its avatar in its color, its name, an Outside tag for the External one, and its plan with its email under it.',
  options: [
    { key: 'A', name: 'Name, period and every window', from: 'CLIProxyAPI’s lane head',
      desc: 'A dot in the account’s color, its name and the drawn window’s length (7d), and what’s left of each window under it.',
      good: 'Every window’s number without looking up.', cost: 'Repeats the table above it.',
      mock: () => panel(timeline({ agents: SOME('claude', 'codex'), head: 'cpa' })) },
    { key: 'B', name: 'Avatar, name, plan and window', from: 'agentZ’s Usage tables',
      desc: 'The account’s avatar and name as in the tables, and its plan and the drawn window’s name under it.',
      good: 'Matches the tables; the bar says the rest.', cost: 'The other windows’ numbers are in the table only.',
      mock: () => panel(timeline({ agents: SOME('claude', 'codex') })) },
    { key: 'C', name: 'Avatar, name and what’s left', from: 'agentZ’s table cells',
      desc: 'The avatar and name, and the drawn window’s % left and time to its reset under it, colored by pace.',
      good: 'The number and the bar side by side.', cost: 'The same number twice in a lane.',
      mock: () => panel(timeline({ agents: SOME('claude', 'codex'), head: 'cell' })) },
  ],
});

// Bars -------------------------------------------------------------------------------------------
TOPICS.push({
  id: 'bar', section: 'Bars', title: 'How the current window reads', size: 'wide', rec: 'A',
  now: 'In the tables a window’s bar is what’s left, from the left, with a hairline where even spending would be; here a bar is the window’s time, from when it opened to when it resets.',
  options: [
    { key: 'A', name: 'Filled with what’s used', from: 'CLIProxyAPI’s windowFill',
      desc: 'The bar runs from when the window opened to its reset, filled from the left with what’s used. A fill past the now line means it’s going faster than even.',
      good: 'The fill and the now line say the pace without words.', cost: 'Filled with what’s used, while the tables fill with what’s left.',
      mock: () => panel(timeline({ agents: SOME('claude', 'codex') })) },
    { key: 'B', name: 'Filled with what’s left', from: 'agentZ’s table bars',
      desc: 'The same bar, filled from the left with what’s left, as the tables’ bars are.',
      good: 'Fills mean the same as in the tables.', cost: 'The fill’s end is a percentage on a time axis, so it means nothing against the days.',
      mock: () => panel(timeline({ agents: SOME('claude', 'codex'), bar: 'left' })) },
    { key: 'C', name: 'The table’s cell in an outline', from: 'agentZ’s table cells',
      desc: 'An outline for the window’s time, with the table’s cell inside it: what’s left over a small bar, and the time to its reset.',
      good: 'Reads exactly as the tables.', cost: 'Needs room; short windows push it above the bar.',
      mock: () => panel(timeline({ agents: SOME('claude', 'codex'), bar: 'cell' })) },
  ],
});

TOPICS.push({
  id: 'color', section: 'Bars', title: 'The bars’ color', size: 'wide', rec: 'A',
  now: 'The tables’ bars are colored by pace (usage round, topic 1): the accent when the window lasts at this rate, yellow when it will be close, red when it runs out first or is used up.',
  options: [
    { key: 'A', name: 'By pace', from: 'agentZ’s limit rows (OpenUsage’s pace)',
      desc: 'The current window in the tables’ pace colors; the ones to come in the accent.',
      good: 'Red means the same everywhere.', cost: 'Lanes aren’t told apart by color.',
      mock: () => panel(timeline({ agents: SOME('claude', 'droid') })) },
    { key: 'B', name: 'The account’s color', from: 'CLIProxyAPI’s provider colors',
      desc: 'Every bar in the account’s color, as CLIProxyAPI colors its lanes by provider and agentZ gives each account a color.',
      good: 'Matches the account’s avatar and its threads.', cost: 'The pace is in the fill, the now line and the hatching only.',
      mock: () => panel(timeline({ agents: SOME('claude', 'droid'), color: 'account' })) },
    { key: 'C', name: 'One color, red when used up', from: 'new',
      desc: 'All bars in the accent, a used up window in red, and no hatching.',
      good: 'The calmest chart.', cost: 'Says nothing about pace.',
      mock: () => panel(timeline({ agents: SOME('claude', 'droid'), color: 'accent', runout: 'none' })) },
  ],
});

TOPICS.push({
  id: 'runout', section: 'Bars', title: 'When a window runs out first', size: 'wide', rec: 'B',
  now: 'The limit rows say “runs out in 9h 12m” under the reset, with a flame, when what’s used so far, at the same rate, passes the limit before the reset.',
  options: [
    { key: 'A', name: 'Nothing more', from: 'CLIProxyAPI’s chart',
      desc: 'Only the bar’s color and its fill say it; the time is in the tooltip.',
      good: 'Simplest.', cost: 'Doesn’t show for how long the account stops.',
      mock: () => panel(timeline({ agents: SOME('claude', 'droid'), runout: 'none' })) },
    { key: 'B', name: 'The stopped time hatched', from: 'new, from agentZ’s pace',
      desc: 'From when it runs out at this pace to its reset, the bar is hatched in red: the time the account would be stopped.',
      good: 'Shows when, and for how long, so you see which account to move to.', cost: 'One more thing in a bar.',
      mock: () => panel(timeline({ agents: SOME('claude', 'droid') })) },
    { key: 'C', name: 'A flame where it runs out', from: 'agentZ’s limit rows',
      desc: 'The limit rows’ flame, on the bar where it runs out at this pace.',
      good: 'Small, and the same mark as the rows.', cost: 'A point, not how long it stops.',
      mock: () => panel(timeline({ agents: SOME('claude', 'droid'), runout: 'flame' })) },
  ],
});

TOPICS.push({
  id: 'windows', section: 'Bars', title: 'Windows before and after', size: 'wide', rec: 'B',
  now: 'agentZ knows each window’s length and its next reset only. Windows before the current one and after it can only be worked out from those, as CLIProxyAPI does: back to back, a window’s length apart. A new 5-hour window opens with the first message after a reset, so the ones to come are the earliest they could be.',
  options: [
    { key: 'A', name: 'Past, current and to come', from: 'CLIProxyAPI’s past, live and next',
      desc: 'Windows before the current one faded, the current one, and those to come dashed, across the whole view.',
      good: 'Shows the rhythm of every account.', cost: 'The faded past ones are guesses with nothing in them.',
      mock: () => panel(timeline({ agents: SOME('claude', 'codex'), windows: 'all' })) },
    { key: 'B', name: 'The current one and those to come', from: 'CLIProxyAPI, without the past',
      desc: 'The current window, then the ones after it, dashed, to the end of the view.',
      good: 'Everything drawn is something you can still use.', cost: 'The view’s start is empty in lanes that reset soon.',
      mock: () => panel(timeline({ agents: SOME('claude', 'codex') })) },
    { key: 'C', name: 'The current one only', from: 'new',
      desc: 'Only the current window of each account.',
      good: 'Nothing guessed.', cost: 'You don’t see when the next one after it would end.',
      mock: () => panel(timeline({ agents: SOME('claude', 'codex'), windows: 'current' })) },
  ],
});

TOPICS.push({
  id: 'labels', section: 'Bars', title: 'Labels and dates', size: 'wide', rec: 'B',
  now: 'The tables say “68%” and “↻ 2h 40m”, the limit rows “26% left” and “resets in 4h 17m”; times elsewhere in agentZ are relative (“2h”, “3d”).',
  options: [
    { key: 'A', name: 'Month and day numbers', from: 'CLIProxyAPI’s labels',
      desc: 'The current bar says “13% · 10/12 08:20”, the ones to come their end; the days are 10/05, 10/06, and so on.',
      good: 'Short.', cost: 'The bare percentage doesn’t say left or used; month first reads wrong outside the US.',
      mock: () => panel(timeline({ agents: SOME('claude', 'codex'), labels: 'cpa' })) },
    { key: 'B', name: 'Left, and the day it resets', from: 'agentZ’s limit rows',
      desc: '“13% left · resets Oct 12, 08:20”, the ones to come “resets Oct 19, 08:20” at their end; on the axis the view’s first day and a month’s first day have the month’s name, the others their number.',
      good: 'Says what the number is and when, in the rows’ words.', cost: 'Longer labels move above short bars.',
      mock: () => panel(timeline({ agents: SOME('claude', 'codex') })) },
    { key: 'C', name: 'Left, and how long until it resets', from: 'agentZ’s table cells',
      desc: '“13% left · resets in 2d 18h”, as the tables count down.',
      good: 'Matches the tables.', cost: 'The bars already show when; a countdown repeats it in other words.',
      mock: () => panel(timeline({ agents: SOME('claude', 'codex'), labels: 'relative' })) },
  ],
});

// Time -------------------------------------------------------------------------------------------
TOPICS.push({
  id: 'zoom', section: 'Time', title: 'Zooming to the short windows', size: 'wide', rec: 'A',
  now: 'Agents name their windows differently: Claude’s Session and Codex’s and Droid’s 5-hour are all five hours long; Devin has a Daily one; Droid a Monthly one.',
  options: [
    { key: 'A', name: 'Weekly and 5-hour', from: 'CLIProxyAPI’s two zooms',
      desc: 'Weekly shows two weeks, a cell a day; 5-hour shows three days, a cell per 6 hours, and only the windows five hours long, whatever the agent names them. Shown here on 5-hour.',
      good: 'Both kinds of limit everyone has, and nothing else to learn.', cost: 'Devin’s daily and Droid’s monthly windows aren’t drawn.',
      mock: () => panel(timeline({ agents: SOME('claude', 'codex', 'droid'), mode: 'session' })) },
    { key: 'B', name: 'A zoom for each window name', from: 'new',
      desc: 'The switch has a choice for each kind of window the accounts report: 5-hour, Daily, Weekly and Monthly, each with a span to fit it. Shown here on Monthly, eight weeks a cell a week.',
      good: 'Every window can be drawn.', cost: 'A longer switch; most choices show one agent.',
      mock: () => panel(timeline({ agents: SOME('claude', 'codex', 'droid'), mode: 'month', zoom: 'names', group: 'agent' })) },
    { key: 'C', name: 'One view, 5-hour windows as a strip', from: 'new',
      desc: 'No switch: two weeks only, with each account’s current 5-hour window as a short strip under its weekly bar.',
      good: 'Nothing to switch.', cost: 'A 5-hour window is a few pixels wide in two weeks.',
      mock: () => panel(timeline({ agents: SOME('claude', 'codex'), zoom: 'one', strip: true })) },
  ],
});

TOPICS.push({
  id: 'nav', section: 'Time', title: 'Moving in time', size: 'wide', rec: 'A',
  now: NOW_NONE,
  options: [
    { key: 'A', name: '‹ Today ›', from: 'CLIProxyAPI’s navigation',
      desc: 'The view starts on Sunday; the arrows move it a week (a day on 5-hour), and the middle button shows the view’s first day and goes back to today. Shown here one week ahead.',
      good: 'See next week’s resets; always one click back.', cost: 'Today sits anywhere in the view, at the left on Sundays.',
      mock: () => panel(timeline({ agents: SOME('claude', 'codex'), span: { start: 168, end: 504, cell: 24, words: 'two weeks' }, offsetLabel: 'Oct 12' })) },
    { key: 'B', name: 'Always from today', from: 'new',
      desc: 'No buttons: the view starts at today and shows the two weeks ahead.',
      good: 'Nothing to press; what’s ahead is always in view.', cost: 'No way to look further.',
      mock: () => panel(timeline({ agents: SOME('claude', 'codex'), span: SPANS.today, nav: 'none' })) },
    { key: 'C', name: 'Forward only', from: 'CLIProxyAPI, without going back',
      desc: 'Today and ›: the view moves ahead and back to today, never before it.',
      good: 'No past weeks of guessed windows.', cost: 'An odd button row.',
      mock: () => panel(timeline({ agents: SOME('claude', 'codex'), nav: 'forward' })) },
  ],
});

TOPICS.push({
  id: 'resets', section: 'Time', title: 'Codex’s limit resets', size: 'wide', rec: 'A',
  now: 'A Codex account can have limit resets: use one, and its limits start over. Each expires on a day of its own. The Account tab shows how many and offers Reset Limits.',
  options: [
    { key: 'A', name: 'A tick where it expires', from: 'CLIProxyAPI’s manual reset expiry',
      desc: 'A yellow tick in the account’s lane on the day the limit reset expires; its tooltip says when.',
      good: 'You see whether you’ll need it before it expires.', cost: 'A small mark, easy to miss.',
      mock: () => panel(timeline({ agents: SOME('claude', 'codex') }), { over: tip(['Limit reset', `<span class="mu2">Expires ${moment(NOW_H + 116)}, in ${duration(116)}</span>`, '<span class="mu2">before this window resets</span>'], 'left:400px;top:314px') }) },
    { key: 'B', name: 'A tick, and the count by the name', from: 'CLIProxyAPI, with the Account tab’s count',
      desc: 'The tick, and “1 limit reset” in yellow under the account’s plan.',
      good: 'Seen at a glance.', cost: 'A taller lane for the account.',
      mock: () => panel(timeline({ agents: SOME('claude', 'codex'), resets: 'head' })) },
    { key: 'C', name: 'Not on the timeline', from: 'new',
      desc: 'Limit resets stay on the Account tab.',
      good: 'Nothing Codex-only on the chart.', cost: 'A reset can expire unused.',
      mock: () => panel(timeline({ agents: SOME('claude', 'codex'), resets: 'none' })) },
  ],
});

// Details ----------------------------------------------------------------------------------------
TOPICS.push({
  id: 'hover', section: 'Details', title: 'Hovering and clicking a bar', size: 'wide', rec: 'C',
  now: 'In the tables a cell’s tooltip has the pace words (“~20% left at reset”, “~12% over the limit at reset”) and the reset’s time; clicking a row opens the account on its agent’s page.',
  options: [
    { key: 'A', name: 'When it opens and resets', from: 'CLIProxyAPI’s bar tooltip',
      desc: 'The account, when the window opened and when it resets, and what’s left.',
      good: 'Exact times.', cost: 'Nothing about pace.',
      mock: () => panel(timeline({ agents: SOME('claude', 'codex'), hover: { agent: 'claude', name: ME } }), { over: tip(windowTip('claude', ME, 'cpa'), 'left:300px;top:170px') }) },
    { key: 'B', name: 'The limit rows’ words', from: 'agentZ’s limit rows',
      desc: 'The window and what’s left, when it opened and resets, and the pace: when it runs out and how far over it ends.',
      good: 'Every number behind the bar, in the rows’ words.', cost: 'A taller tooltip.',
      mock: () => panel(timeline({ agents: SOME('claude', 'codex'), hover: { agent: 'claude', name: ME } }), { over: tip(windowTip('claude', ME, 'rows'), 'left:300px;top:170px') }) },
    { key: 'C', name: 'B, and a click opens the account', from: 'agentZ’s Usage tables',
      desc: 'B’s tooltip, and clicking a lane opens the account on its agent’s page, as the tables’ rows do.',
      good: 'From the chart straight to the account.', cost: 'None.',
      mock: () => panel(timeline({ agents: SOME('claude', 'codex'), hover: { agent: 'claude', name: ME }, chevron: true }), { over: tip(windowTip('claude', ME, 'rows'), 'left:300px;top:170px') }) },
  ],
});

TOPICS.push({
  id: 'idle', section: 'Details', title: 'Accounts with no window counting down', size: 'wide', rec: 'A',
  now: 'A window starts counting down with the first message after a reset. An account that hasn’t been used has all its limits left and no reset time; the tables show 100% with no reset.',
  options: [
    { key: 'A', name: 'A lane that says so', from: 'CLIProxyAPI’s idle lane',
      desc: 'The account keeps its lane, saying “All left · no window counting down”.',
      good: 'The account to move to is in the chart.', cost: 'Empty lanes.',
      mock: () => panel(timeline({ agents: SOME('droid', 'devin') })) },
    { key: 'B', name: 'Left out', from: 'CLIProxyAPI’s chart',
      desc: 'Only accounts with a window counting down get a lane, as CLIProxyAPI leaves out credentials it can’t draw.',
      good: 'A shorter chart.', cost: 'The unused accounts are missing from it.',
      mock: () => panel(timeline({ agents: SOME('droid', 'devin'), idle: 'hide' })) },
    { key: 'C', name: 'One line under the chart', from: 'new',
      desc: 'Left out of the lanes, and named in a line under them.',
      good: 'Short, and still says which are unused.', cost: 'Easy to skip.',
      mock: () => panel(timeline({ agents: SOME('droid', 'devin'), idle: 'line' })) },
  ],
});

TOPICS.push({
  id: 'legend', section: 'Details', title: 'Explaining the bars', size: 'wide', rec: 'A',
  now: NOW_NONE,
  options: [
    { key: 'A', name: 'A legend and a sentence', from: 'CLIProxyAPI’s legend',
      desc: 'Under the chart: a swatch for each kind of mark, and a sentence on what a bar is.',
      good: 'Explains itself the first time.', cost: 'Two lines always shown.',
      mock: () => panel(timeline({ agents: SOME('claude', 'codex') })) },
    { key: 'B', name: 'Nothing', from: 'agentZ’s tables',
      desc: 'No legend; the tooltips explain each bar.',
      good: 'Nothing extra.', cost: 'The hatching and dashed bars are guesses until you hover.',
      mock: () => panel(timeline({ agents: SOME('claude', 'codex'), legend: 'none' })) },
    { key: 'C', name: 'Behind an info button', from: 'new',
      desc: 'An info button by the title shows the legend in a tooltip.',
      good: 'There when wanted.', cost: 'Hidden.',
      mock: () => panel(timeline({ agents: SOME('claude', 'codex'), legend: 'button' }), { over: tip(['<b>Limit windows</b>', '<span class="mu2">The current window, filled with what’s used, then those to come, dashed.</span>', '<span class="mu2">Hatched red: stopped, at this pace. Yellow tick: a limit reset expires.</span>'], 'left:250px;top:58px;white-space:normal;width:330px') }) },
  ],
});
