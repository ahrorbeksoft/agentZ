// What an account can do about its limits: choose what happens when one runs out (Droid's
// Droid Core or extra usage), use a limit reset (Codex, Claude), extra usage and credits, and
// waiting for the reset. Also the thread view the limit topics share.

const DROID_WORK = { email: 'alex@acme.co', label: 'Work', plan: 'Pro', hue: 150,
  windows: [['5-hour', 0, 'resets in 3h 05m', 0.38], ['Weekly', 58, 'resets Sun 00:00', 0.6], ['Monthly', 71, 'resets Nov 1', 0.2]] };
const DROID_CORE = [['5-hour', 100, '', null], ['Weekly', 96, 'resets Sun 00:00', 0.6]];
const CODEX_WORK = { email: 'alex@acme.co', label: 'Work', plan: 'Plus', hue: 150,
  windows: [['5-hour', 0, 'resets in 1h 12m', 0.76], ['Weekly', 35, 'resets Tue 08:00', 0.55]] };

/** A menu whose entries have a second line. */
const richMenu = (items, style, width = 300) => `<div class="menu" style="${style};min-width:${width}px;max-width:${width}px">${items.map(([title, desc, flags = {}]) => `<div class="it ${flags.hl ? 'hl' : ''}" style="height:auto;padding:6px 8px;align-items:flex-start;white-space:normal"><span class="col grow" style="gap:1px"><span>${title}</span><span class="xs mu">${desc}</span></span>${flags.check ? ic('check', 'sm ac') : ''}</div>`).join('')}</div>`;
const groupLabel = (text) => `<div class="xs ph" style="margin:2px 0 7px">${text}</div>`;
const settingLine = (title, desc, control, extra = '') => `<div class="row g3" style="padding:10px 16px 12px 56px;position:relative"><div class="col grow" style="gap:2px"><span>${title}</span>${desc ? `<span class="sm mu">${desc}</span>` : ''}</div><span class="none" style="position:relative">${control}</span>${extra}</div>`;

// The thread view: the end of a conversation, a notice over the composer, and the composer.
const smallToggle = `<span style="width:26px;height:15px;border-radius:8px;background:var(--sel);display:inline-flex;align-items:center;padding:2px"><i style="width:11px;height:11px;border-radius:50%;background:var(--mu);display:block"></i></span>`;
const composerBar = ({ agentLabel = `${glyph('claude')}Claude Agent`, above = '' } = {}) => `<div style="border-top:1px solid var(--b);padding:8px 0 10px;position:relative;background:var(--ed)"><div style="margin:0 auto;width:640px;position:relative">${above}<div class="ph" style="padding:6px 0 12px">Message the agent…</div><div class="row" style="gap:12px;font-size:13px;color:var(--mu)">${ic('plus', 'sm')}<span class="row g15">${agentLabel}</span><span class="grow"></span><span class="row g1">Default ${chev}</span><span class="row g1">Opus ${chev}</span><span class="row g1">High ${chev}</span><span class="row g15">Fast ${smallToggle}</span><span class="ac" style="display:inline-flex">${ic('send', 'sm')}</span></div></div></div>`;
const threadHeader = (title = 'Add the checkout page', extra = '') => `<div class="row g2" style="height:36px;padding:0 12px;border-bottom:1px solid var(--b);flex:none;position:relative">${mono('ST', 'g')}<span class="sm mu">storefront</span><span class="mu">/</span><span class="row g1 sm">${title}${chev}</span><span class="grow"></span><span class="chip">${ic('branch', 'xs')}main</span>${extra}</div>`;
const conversation = () => `<div style="padding:16px 60px 0;display:flex;flex-direction:column;gap:12px"><div style="align-self:flex-end;background:var(--hov);padding:8px 12px;border-radius:8px;max-width:420px">Add a checkout page with a pay button. Use the cart from src/lib/cart.ts.</div><div style="line-height:22px">I added the cart summary and the <code style="font-family:'IBM Plex Mono';font-size:12px">POST /api/orders</code> route. Next is the pay button…</div></div>`;
/** t3code's ThreadErrorBanner as a warning: an icon, what happened, and the ways on. */
const limitBanner = ({ title, body, buttons = '', icon = 'hourglass' }) => `<div class="row g3" style="align-items:flex-start;margin:0 auto 8px;width:640px;padding:10px 12px;border-radius:8px;border:1px solid rgba(222,193,132,.35);background:rgba(222,193,132,.08)"><span class="warnc" style="display:inline-flex;margin-top:2px">${ic(icon, 'sm')}</span><div class="col grow" style="gap:2px"><span class="b5">${title}</span><span class="sm mu">${body}</span>${buttons ? `<div class="row g2" style="margin-top:8px;flex-wrap:wrap">${buttons}</div>` : ''}</div>${ibtn('x', 'sm')}</div>`;
const threadView = (notice, { w = 760, h = 420, composer = {}, header = threadHeader(), overlay = '' } = {}) => frame(`<div class="col" style="height:100%">${header}<div class="grow" style="min-height:0;overflow:hidden">${conversation()}</div>${notice}${composerBar(composer)}</div>${overlay}`, { w, h });
const WORK_OUT = { title: 'Work reached its 5-hour limit', body: 'Claude Agent stopped. The limit resets at 4:10 PM, in 1h 52m.' };

// 8. When a limit is reached ---------------------------------------------------------------
const droidCard = ({ prefLine = '', tabs = false } = {}) => card([
  `${accountHead(DROID_WORK)}<div style="padding:0 16px 12px 56px">${tabs ? `<div class="seg" style="margin-bottom:10px"><span class="on">Standard</span><span>Droid Core</span><span>Extra usage</span></div>` : groupLabel('Standard models')}${windowsGrid(DROID_WORK.windows)}${tabs ? '' : `<div style="height:10px"></div>${groupLabel('Droid Core models')}${windowsGrid(DROID_CORE)}`}</div>`,
  prefLine || null,
]);
const PREF_ITEMS = [['Switch to Droid Core', 'Keep working on Droid Core models, at no extra cost.', { check: true, hl: true }], ['Use extra usage', 'Keep working on the same models, billed from your extra usage balance ($18.20).']];

TOPICS.push({
  id: 'pool', section: 'When limits run out', title: 'Choosing what happens when a limit is reached', size: 'wide', rec: 'A',
  now: 'Only in each agent’s own terminal app. Droid’s <code>/limits</code> has a “When limit is reached” choice: “Switch to Droid Core” (cheaper models, no extra cost) or “Enable Extra Usage” (billed). It’s saved on Factory’s server, so it’s per account and applies to the CLI too; enterprise orgs set it on Factory’s dashboard. Claude’s limit menu offers “Switch to usage credits”, Devin bills “overage” once its org allows it, and Codex spends purchased credits after the plan’s limits. The mocks show a Droid account; agents without such a choice show none.',
  options: [
    { key: 'A', name: 'A setting under the account’s limits', from: 'Droid’s /limits',
      desc: 'The card shows each pool’s windows (Standard and Droid Core for Droid) and a “When a limit is reached” dropdown with Droid’s two choices and their explanations. agentZ saves it on Factory’s server as Droid does (<code>set-overage-preference</code>, read back with the limits). For an enterprise org it shows read-only with “Set by your organization”. Claude’s usage credits would appear the same way once agentZ can reach them (today only Claude’s terminal app can).',
      good: 'Set once per account, in the place you see the limits; the CLI follows the same setting.', cost: 'Writes to the vendor’s account, which agentZ otherwise never does; needs Factory’s endpoint to stay as it is.',
      mock: () => frame(`<div style="padding:16px">${droidCard({ prefLine: settingLine('When a limit is reached', 'Saved to your Factory account; the CLI does the same.', dd('Switch to Droid Core'), richMenu(PREF_ITEMS, 'top:44px;right:16px')) })}</div>`, { w: 720, h: 470 }) },
    { key: 'B', name: 'Pools as tabs, as /limits shows them', from: 'Droid’s /limits tabs',
      desc: 'Tabs over the windows switch between Standard, Droid Core and Extra usage, as in Droid’s panel; the same “When a limit is reached” setting sits under them.',
      good: 'Exactly Droid’s layout.', cost: 'Droid Core’s windows hide behind a tab, and only Droid has pools.',
      mock: () => frame(`<div style="padding:16px">${droidCard({ tabs: true, prefLine: settingLine('When a limit is reached', 'Saved to your Factory account; the CLI does the same.', dd('Switch to Droid Core')) })}</div>`, { w: 720, h: 330 }) },
    { key: 'C', name: 'Asked when it happens', from: 'Claude’s rate-limit options',
      desc: 'No setting on the card. When a thread hits the limit, its notice offers the choices as buttons, as Claude’s terminal app does: “Switch to Droid Core”, “Use Extra Usage”. Picking one saves it on Factory’s server and sends the message again.',
      good: 'Decided with the work in front of you.', cost: 'You find out about the choice only when you’re already stopped.',
      mock: () => threadView(limitBanner({ title: 'Work reached its 5-hour limit on standard models', body: 'Factory Droid stopped. The limit resets at 7:05 PM, in 3h 05m. Droid can keep going:', buttons: `${obtn('Switch to Droid Core')}${obtn('Use Extra Usage · $18.20 left')}` }), { composer: { agentLabel: `${glyph('droid')}Factory Droid` } }) },
    { key: 'D', name: 'Shown, changed on Factory’s site', from: 't3code’s Manage usage',
      desc: 'The card says what’s set (“When a limit is reached: Switch to Droid Core”) with a link to Factory’s billing page to change it. agentZ never writes to the account.',
      good: 'agentZ only reads; nothing can be billed by a click in agentZ.', cost: 'A trip to the browser for a two-way choice.',
      mock: () => frame(`<div style="padding:16px">${droidCard({ prefLine: settingLine('When a limit is reached', 'Switch to Droid Core: keeps working on Droid Core models at no extra cost.', `<span class="row g1 sm ac">Change on factory.ai${ic('external', 'xs')}</span>`) })}</div>`, { w: 720, h: 470 }) },
  ],
});

// 9. Limit resets ----------------------------------------------------------------------------
const resetLine = (button = obtn('Use Reset')) => `<div class="row g2 sm mu" style="margin-top:12px">${ic('ticket', 'sm')}<span>1 limit reset available · expires in 27d</span><span class="grow"></span>${button}</div>`;

TOPICS.push({
  id: 'reset', section: 'When limits run out', title: 'Using a limit reset', size: 'wide', rec: 'A',
  now: 'Not shown. Codex grants occasional limit resets: its <code>/usage</code> offers “Use this reset?”, and its app-server reports them (<code>rateLimitResetCredits</code>, one on the test account) and uses one by request. t3code shows “1 reset credit banked · Use reset” with a confirm. Claude has a hidden <code>/limit-reset</code> (once a week, terminal only); agentZ would run it in a hidden terminal, which is untested. The mocks show a Codex account whose 5-hour window is used up.',
  options: [
    { key: 'A', name: 'A line under the limits, with a confirm', from: 't3code ResetCredits',
      desc: 'When the account has resets, a line under its limits says how many and when the next expires, with Use Reset. It always asks first, since a reset can’t be given back. The thread’s limit notice offers the same button (see “When a thread’s account runs out”).',
      good: 'Visible whenever there’s one to use; can’t be clicked by accident.', cost: 'A line most accounts never show.',
      mock: () => frame(`<div style="padding:16px">${card([`${accountHead(CODEX_WORK)}${windowsBlock(CODEX_WORK, resetLine())}`])}</div>${dialog('Use a limit reset?', 'This clears Work’s 5-hour and weekly limits now (alex@acme.co, Codex). It uses your only reset and can’t be undone.', `${obtn('Cancel')}${pbtn('Use Reset')}`, { w: 380, style: 'top:62%' })}`, { w: 720, h: 330 }) },
    { key: 'B', name: 'A ticket on the window', from: 't3code’s usage page',
      desc: 'A small ticket with the count beside the used-up window’s reset time. Clicking it opens a popover with when it expires and Use Reset, then the same confirm.',
      good: 'Takes no room; sits where the problem is.', cost: 'Easy to miss.',
      mock: () => frame(`<div style="padding:16px">${card([`${accountHead(CODEX_WORK)}<div style="padding:0 16px 14px 56px">${windowsGrid([['5-hour', 0, `<span class="row g1" style="justify-content:flex-end">resets in 1h 12m <span class="chip" style="height:18px;padding:0 5px;color:var(--t)">${ic('ticket', 'xs')}1</span></span>`, 0.76], CODEX_WORK.windows[1]], { resetW: 170 })}<div class="pop" style="right:16px;top:96px;width:250px;padding:10px 12px"><div class="sm" style="margin-bottom:8px">1 limit reset available<div class="xs mu">Expires in 27 days. Clears the 5-hour and weekly limits.</div></div>${obtn('Use Reset…')}</div></div>`])}</div>`, { w: 720, h: 300 }) },
    { key: 'C', name: 'Only once a limit is used up', from: 'Codex’s /usage',
      desc: 'Nothing while there’s usage left. Once a window is used up, the card shows it in red with “Use Reset (1 left)”, and so does the thread’s notice.',
      good: 'Offered exactly when it helps; no one spends one early.', cost: 'You can’t see you have one until you need it.',
      mock: () => frame(`<div style="padding:16px">${card([`${accountHead(CODEX_WORK)}${windowsBlock(CODEX_WORK, `<div class="row g2" style="margin-top:12px;padding:8px 10px;border-radius:6px;background:rgba(208,114,119,.12)"><span class="delc" style="display:inline-flex">${ic('hourglass', 'sm')}</span><span class="sm grow">The 5-hour limit is used up. It resets in 1h 12m.</span>${obtn('Use Reset (1 left)…')}</div>`)}`])}</div>`, { w: 720, h: 260 }) },
    { key: 'D', name: 'In the account’s menu', from: 'new',
      desc: '“Use Limit Reset (1 left)…” in the account’s ⋯ menu, with the same confirm.',
      good: 'Out of the way.', cost: 'Hidden; people won’t know they have one.',
      mock: () => frame(`<div style="padding:16px">${accountCard(CODEX_WORK, { head: { right: ibtn('more', 'on'), menu: menuList([['pencil', 'Rename…'], ['star', 'Use for New Threads'], ['rotate', 'Refresh Usage'], ['ticket', 'Use Limit Reset (1 left)…', { hl: true }], 'hr', ['logout', 'Log Out'], ['trash', 'Remove Account…', { danger: true }]], 'top:44px;right:12px', 250) } })}</div>`, { w: 720, h: 330 }) },
  ],
});

// 10. Extra usage and credits -----------------------------------------------------------------
const extraLine = (right) => `<div class="row g2 sm" style="margin-top:12px"><span class="mu" style="display:inline-flex">${ic('coins', 'sm')}</span><span class="mu">Extra usage</span><span>$12.40 of $50.00 left this month</span><span class="grow"></span>${right}</div>`;

TOPICS.push({
  id: 'extra', section: 'When limits run out', title: 'Extra usage and credits', size: 'wide', rec: 'A',
  now: 'Not shown. Every subscription agent can bill past its plan: Claude’s usage credits (<code>extra_usage</code> in its usage data, turned on in <code>/usage-credits</code>), Codex’s purchased credits (<code>credits.balance</code>), Droid’s extra usage balance, Devin’s overage, Grok’s prepaid balance. Turning paid usage on happens on the vendor’s site or in its terminal app.',
  options: [
    { key: 'A', name: 'The balance, with a link to manage it', from: 't3code’s Manage usage',
      desc: 'For accounts that report one, a line under the limits: what’s left of the extra usage or credits, and “Manage” opening the vendor’s billing page. agentZ never turns paid usage on itself.',
      good: 'You see the money you can still spend; billing stays where it’s safe.', cost: 'Turning it on means the browser.',
      mock: () => piece(`<div style="padding:16px">${card([`${accountHead(ACCTS.work)}${windowsBlock(ACCTS.work, extraLine(`<span class="row g1 sm ac">Manage${ic('external', 'xs')}</span>`))}`])}</div>`) },
    { key: 'B', name: 'A switch to turn it on', from: 'Claude’s /usage-credits, Droid’s preference',
      desc: 'A switch per account: “Use extra usage when limits run out”, asking first because it bills the card on file. agentZ changes it through the agent (Droid’s preference; Claude’s only through its terminal app, untested).',
      good: 'No trip to the browser.', cost: 'A click in agentZ can cost money; Claude has no supported way to do it.',
      mock: () => piece(`<div style="padding:16px">${card([`${accountHead(ACCTS.work)}${windowsBlock(ACCTS.work)}`, settingLine('Use extra usage when limits run out', 'Billed to the card on alex@acme.co’s Claude account. $12.40 of $50.00 left this month.', toggle(true))])}</div>`) },
    { key: 'C', name: 'Only the balance', from: 'new',
      desc: 'The line from A without a link.', good: 'The least to build.', cost: 'Nowhere to go when it runs low.',
      mock: () => piece(`<div style="padding:16px">${card([`${accountHead(ACCTS.work)}${windowsBlock(ACCTS.work, extraLine(''))}`])}</div>`) },
    { key: 'D', name: 'Not shown', from: 'today',
      desc: 'Only the plan’s windows.', good: 'Simple.', cost: 'Threads can keep going after the limits and you won’t see why, or what it costs.',
      mock: () => piece(`<div style="padding:16px">${accountCard(ACCTS.work)}</div>`) },
  ],
});

// 11. Waiting for the reset -------------------------------------------------------------------
TOPICS.push({
  id: 'resume', section: 'When limits run out', title: 'Waiting for the reset', size: 'wide', rec: 'A',
  now: 'A thread that hits a limit stops with the agent’s error, and nothing happens at the reset. Claude’s terminal app offers “Continue automatically at reset”, and t3code’s limit banner has “Resume at reset”; no ACP agent offers it. agentZ already queues messages (sent when the turn ends), so it can send one at the reset time for any agent.',
  options: [
    { key: 'A', name: '“Continue at 4:10 PM” in the thread', from: 't3code’s Resume at reset, Claude’s auto-resume',
      desc: 'The thread’s limit notice has “Continue at 4:10 PM”. It queues “Continue.” (or what’s typed) and agentZ sends it when the account’s window resets, even with the app closed, since the server sends it. A strip over the composer shows it waiting, with Send Now and Cancel.',
      good: 'Works for every agent; you choose per thread.', cost: 'The reset time is the agent’s; if it’s wrong the message fails and waits again.',
      mock: () => threadView(`<div class="row g2 sm" style="margin:0 auto 8px;width:640px;padding:7px 10px;border:1px solid var(--b);border-radius:8px;background:var(--panel)"><span class="mu" style="display:inline-flex">${ic('clock', 'sm')}</span><span class="grow">Sends “Continue.” at 4:10 PM, when Work’s 5-hour limit resets</span><span class="sm ac">Send Now</span><span class="sm mu">Cancel</span></div>`) },
    { key: 'B', name: 'A setting per account', from: 'Claude’s rate-limit options',
      desc: 'A “When a limit is reached” setting on the account: Stop (today) or Continue at reset, for every thread on it. agentZ never moves a thread to another account by itself.',
      good: 'Set once; long tasks finish overnight.', cost: 'Threads resume without you choosing, perhaps hours later.',
      mock: () => frame(`<div style="padding:16px">${card([`${accountHead(ACCTS.work)}${windowsBlock(ACCTS.work)}`, settingLine('When a limit is reached', 'What threads on this account do.', dd('Continue at reset'), richMenu([['Stop', 'The thread waits for you.'], ['Continue at reset', 'agentZ sends “Continue.” when the limit resets.', { check: true, hl: true }]], 'top:44px;right:16px', 280))])}</div>`, { w: 720, h: 360 }) },
    { key: 'C', name: 'Only the reset time', from: 'today, with the time',
      desc: 'The notice says when it resets; you come back and send.', good: 'Nothing new.', cost: 'Easy to forget a stopped thread overnight.',
      mock: () => threadView(limitBanner(WORK_OUT)) },
  ],
});
