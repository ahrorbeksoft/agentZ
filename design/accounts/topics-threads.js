// Accounts in threads: picking one for a new thread, seeing a thread's account, what a thread
// shows when its account runs out, continuing on another account, and usage at a glance.

const chipText = (inner) => `<span class="row g15" style="color:var(--t)">${inner}${chev}</span>`;
const CLAUDE_CHIP = chipText(`${glyph('claude')}Claude Agent`);

/** The new thread screen (agent_view.rs render_new_thread): headline, composer card, strip. */
function newThread({ agentChip = CLAUDE_CHIP, accountChip = '', menu = '', stripExtra = '', h = 460 } = {}) {
  return frame(`<div class="col" style="height:100%;align-items:center;padding-top:70px;gap:24px">
    <div style="font-size:22px">What should we work on?</div>
    <div class="col" style="width:600px;gap:8px;position:relative">
      <div class="card" style="padding:10px 12px 8px">
        <div class="ph" style="height:44px">Message the agent…</div>
        <div class="row" style="gap:12px;font-size:13px;color:var(--mu)">${ic('plus', 'sm')}${agentChip}${accountChip}<span class="grow"></span><span class="row g1">Default ${chev}</span><span class="row g1">Opus ${chev}</span><span class="row g1">High ${chev}</span><span class="row g15">Fast ${smallToggle}</span><span class="mu" style="display:inline-flex">${ic('send', 'sm')}</span></div>
      </div>
      <div class="row sm mu" style="justify-content:space-between;padding:0 4px"><span class="row g3"><span class="row g1">${ic('folder', 'xs')}Local${chev}</span>${stripExtra}</span><span class="row g1">${ic('branch', 'xs')}main</span></div>
      ${menu}
    </div></div>`, { w: 720, h });
}
/** An account in a menu: avatar, name and plan, and the window closest to running out. */
const accountItem = (a, { hl = false, check = false, dis = false, indent = 0, note = '' } = {}) => `<div class="it ${hl ? 'hl' : ''} ${dis ? 'dis' : ''}" style="height:36px;padding-left:${8 + indent}px">${avatar(a, 18)}<span class="col grow" style="line-height:1.2"><span>${accountName(a)}${a.external ? ' <span class="xs ph">· outside agentZ</span>' : ''}</span><span class="xs mu">${note || a.plan}</span></span><span class="xs" style="padding-left:14px">${leftText(tightest(a))}</span>${check ? ic('check', 'sm ac') : '<span style="width:14px"></span>'}</div>`;
const agentItem = (kind, name, { sub = false, check = false, hl = false } = {}) => `<div class="it ${hl ? 'hl' : ''}">${glyph(kind, 'sm')}<span class="grow">${name}</span>${check ? ic('check', 'sm ac') : ''}${sub ? ic('chev-right', 'xs') : ''}</div>`;
const menuFoot = (items) => `<div class="hr"></div>${items.map(([icon, label]) => `<div class="it">${ic(icon, 'sm')}<span class="grow">${label}</span></div>`).join('')}`;
const accountMenu = (style, { current = ACCTS.ext, foot = true, width = 310 } = {}) => `<div class="menu" style="${style};min-width:${width}px">${CLAUDE_ACCOUNTS.map((a) => accountItem(a, { check: a === current })).join('')}${foot ? menuFoot([['plus', 'Add Account…'], ['settings', 'Manage Accounts…']]) : ''}</div>`;

// 12. Picking the account ------------------------------------------------------------------
TOPICS.push({
  id: 'picker', section: 'Threads', title: 'Picking the account for a new thread', size: 'wide', rec: 'A',
  now: 'The new thread’s composer has an agent chip; its menu lists the installed agents, then Terminal and Manage Agents…. A thread keeps its agent once it starts, and would keep its account too: accounts share no sessions. New threads start on the agent’s default account: the one marked “Use for New Threads”, else the External one, else the first.',
  nowImg: 'img/now-new-thread.png',
  options: [
    { key: 'A', name: 'An account chip beside the agent', from: 'new, beside today’s chip',
      desc: 'When the agent has two or more accounts, a second chip follows the agent’s with the account’s avatar and name. Its menu lists the accounts with their plan and the window closest to running out (a used-up one in red), then Add Account… and Manage Accounts….',
      good: 'Agents and accounts stay separate; the limits show right where you choose.', cost: 'One more chip in a busy row.',
      mock: () => newThread({ accountChip: chipText(`${avatar(ACCTS.ext, 16)}alex@hey.com`), menu: accountMenu('top:84px;left:150px') }) },
    { key: 'B', name: 'Accounts under the agent in its menu', from: 'today’s agent menu',
      desc: 'The agent menu lists each agent’s accounts under it, indented, with the same details. The chip reads “Claude Agent · alex@hey.com”. Agents with one account stay one line.',
      good: 'One chip, one menu.', cost: 'The menu gets long with several agents that have accounts.',
      mock: () => newThread({ agentChip: chipText(`${glyph('claude')}Claude Agent · alex@hey.com`), h: 520, menu: `<div class="menu" style="top:84px;left:30px;min-width:330px">${agentItem('claude', 'Claude Agent')}${CLAUDE_ACCOUNTS.map((a) => accountItem(a, { check: a === ACCTS.ext, indent: 18 })).join('')}${agentItem('codex', 'Codex')}${agentItem('droid', 'Factory Droid')}${menuFoot([['terminal', 'Terminal'], ['settings', 'Manage Agents…']])}</div>` }) },
    { key: 'C', name: 'A submenu per agent', from: 'Zed’s submenus',
      desc: 'Agents with several accounts open a submenu of them; picking the agent itself takes its default account.',
      good: 'The agent menu stays short.', cost: 'The limits are a hover away.',
      mock: () => newThread({ agentChip: chipText(`${glyph('claude')}Claude Agent · alex@hey.com`), menu: `<div class="menu" style="top:84px;left:30px;min-width:220px">${agentItem('claude', 'Claude Agent', { sub: true, check: true, hl: true })}${agentItem('codex', 'Codex')}${agentItem('droid', 'Factory Droid')}${menuFoot([['terminal', 'Terminal'], ['settings', 'Manage Agents…']])}</div>${accountMenu('top:84px;left:254px', { foot: false })}` }) },
    { key: 'D', name: 'Each account its own agent', from: 't3code provider instances',
      desc: 'The agent menu lists “Claude Agent”, “Claude Agent · Work” and “Claude Agent · Side” as separate agents, each icon with the account’s initial, as t3code lists instances.',
      good: 'No new control.', cost: 'Accounts look like different agents; no limits in the menu.',
      mock: () => newThread({ menu: `<div class="menu" style="top:84px;left:30px;min-width:250px">${[['Claude Agent', null, true], ['Claude Agent · Work', ACCTS.work], ['Claude Agent · Side', ACCTS.side]].map(([name, a, check]) => `<div class="it"><span style="position:relative;display:inline-flex">${glyph('claude', 'sm')}${a ? `<span style="position:absolute;right:-5px;bottom:-4px">${avatar(a, 10)}</span>` : ''}</span><span class="grow">${name}</span>${check ? ic('check', 'sm ac') : ''}</div>`).join('')}${agentItem('codex', 'Codex')}${agentItem('droid', 'Factory Droid')}${menuFoot([['terminal', 'Terminal'], ['settings', 'Manage Agents…']])}</div>` }) },
    { key: 'E', name: 'In the strip under the composer', from: 'today’s checkout and machine pickers',
      desc: 'Beside Local and the machine, under the composer: the account’s avatar and name, with the same menu as A.',
      good: 'With the other “where it runs” choices.', cost: 'Far from the agent it belongs to.',
      mock: () => newThread({ stripExtra: `<span class="row g1" style="color:var(--t)">${avatar(ACCTS.ext, 14)}alex@hey.com${chev}</span>`, menu: accountMenu('top:118px;left:70px') }) },
  ],
});

// 13. A thread's account -------------------------------------------------------------------
const THREADS = [
  { project: 'storefront', title: 'Add the checkout page', state: 'working', a: ACCTS.ext, active: true },
  { project: 'storefront', title: 'Rate-limit the checkout API', state: 'awaiting', a: ACCTS.work },
  { project: 'api', title: 'Cursor pagination', state: 'done', a: ACCTS.side },
  { project: 'api', title: 'Speed up product search', kind: 'codex' },
];
function threadRow(thread, { badge = 'none', hover = false } = {}) {
  const kind = thread.kind || 'claude';
  let agentMark = glyph(kind, 'sm');
  if (thread.a && badge === 'initial') agentMark = `<span style="position:relative;display:inline-flex">${glyph(kind, 'sm')}<span style="position:absolute;right:-5px;bottom:-4px">${avatar(thread.a, 10)}</span></span>`;
  if (thread.a && badge === 'color') agentMark = `<span class="glyph sm" style="background:hsl(${thread.a.hue} 35% 40%);color:#fff">${GLYPHS[kind]}</span>`;
  const label = thread.a && badge === 'name' ? `<span class="xs mu trunc" style="max-width:90px">${accountName(thread.a)}</span>` : '';
  return `<div class="srow ${thread.active ? 'on' : ''} ${hover ? 'hov' : ''}" style="padding:6px 10px"><div class="row g2" style="height:20px">${ic('folder', 'xs mu')}<span class="grow sm mu">${thread.project}</span>${pill(thread.state)}</div><div style="padding:2px 0">${thread.title}</div><div class="row g2 xs faint" style="height:18px"><span class="grow">main</span>${label}${ic('laptop', 'xs')}${agentMark}</div></div>`;
}
const threadSidebar = (opts = {}, extra = '') => frame(`<div class="sidebar" style="height:100%">${sbHead()}<div class="sb-list" style="gap:4px">${THREADS.map((thread, index) => threadRow(thread, { ...opts, hover: opts.hoverIndex === index })).join('')}</div>${sbFoot()}</div>${extra}`, { w: 290, h: 420 });
const detailsPopover = (thread, style) => `<div class="pop" style="${style};width:260px;padding:10px 12px;font-size:12px"><div class="b5" style="font-size:13px;margin-bottom:8px">${thread.title}</div>${[['Agent', `${glyph('claude', 'sm')} Claude Agent`], ['Account', `${avatar(thread.a, 14)} ${accountName(thread.a)} <span class="mu">${thread.a.label ? thread.a.email : ''}</span>`], ['Machine', 'This Mac'], ['Branch', 'main']].map(([k, v]) => `<div class="row g2" style="height:22px"><span class="mu" style="width:60px">${k}</span><span class="row g15 trunc">${v}</span></div>`).join('')}</div>`;

TOPICS.push({
  id: 'indicator', section: 'Threads', title: 'Seeing a thread’s account', size: 'medium', rec: 'A',
  now: 'A thread row shows its project, title, branch, machine and the agent’s icon; the details popover and the composer name the agent. Nothing would say which account. In every option nothing changes for agents with one account.', nowImg: 'img/now-thread.png',
  options: [
    { key: 'A', name: 'The account’s initial on the agent icon', from: 't3code ProviderInstanceIcon',
      desc: 'A small avatar with the account’s initial on the agent’s icon, in the thread row and the composer chip. The details popover names the account in full.',
      good: 'Tells threads apart without more text.', cost: 'Small; initials can repeat.',
      mock: () => threadSidebar({ badge: 'initial', hoverIndex: 1 }, detailsPopover(THREADS[1], 'left:200px;top:96px')) },
    { key: 'B', name: 'The account’s name in the row', from: 'new',
      desc: 'The account’s short name before the machine icon on the row’s last line, and in the composer chip.',
      good: 'Readable without hovering.', cost: 'Crowds the row’s last line.',
      mock: () => threadSidebar({ badge: 'name' }) },
    { key: 'C', name: 'Only in the details popover', from: 't3code details popover',
      desc: 'Rows stay as they are; the popover and the composer chip name the account.',
      good: 'The sidebar doesn’t change.', cost: 'You have to hover to know.',
      mock: () => threadSidebar({ hoverIndex: 1 }, detailsPopover(THREADS[1], 'left:200px;top:96px')) },
    { key: 'D', name: 'A color per account', from: 't3code accent colors',
      desc: 'Each account gets a color, picked on its card, that tints the agent’s icon wherever the thread shows.',
      good: 'Visible at a glance across the sidebar.', cost: 'Colors mean nothing until learned; another setting.',
      mock: () => threadSidebar({ badge: 'color' }) },
  ],
});

// 14. When the account runs out ------------------------------------------------------------
const OUT_BUTTONS = `${obtn(`Continue on Side · 97% left`)}${obtn('Continue at 4:10 PM')}<span class="row g1 sm mu" style="margin-left:4px">Usage${ic('external', 'xs')}</span>`;

TOPICS.push({
  id: 'exhausted', section: 'Threads', title: 'When a thread’s account runs out', size: 'wide', rec: 'A',
  now: 'The turn ends with the agent’s error text in the thread (“You’ve hit your limit · resets 4:10pm”, “Usage limit reached”), worded differently by each agent. agentZ would recognize it from the limits it reads, and the buttons depend on what the agent offers: another account with room, waiting for the reset, a limit reset (Codex), Droid Core or extra usage (Droid).',
  options: [
    { key: 'A', name: 'A notice over the composer', from: 't3code ThreadErrorBanner',
      desc: 'A yellow notice above the composer: which account ran out, when it resets, and the ways on. “Continue on Side” offers the account with the most left; its arrow lists the others.',
      good: 'Stays in view while you decide; one place for every agent’s options.', cost: 'Covers a bit of the conversation.',
      mock: () => threadView(limitBanner({ ...WORK_OUT, buttons: OUT_BUTTONS })) },
    { key: 'B', name: 'In the conversation, where it stopped', from: 'Zed’s thread error callout',
      desc: 'The same text and buttons as a callout at the end of the conversation, as Zed shows a thread’s errors.',
      good: 'Part of the thread’s history: you see later why it stopped.', cost: 'Scrolls away; the old buttons stay after they no longer apply.',
      mock: () => frame(`<div class="col" style="height:100%">${threadHeader()}<div class="grow" style="min-height:0;overflow:hidden">${conversation()}<div style="padding:12px 60px 0">${limitBanner({ ...WORK_OUT, buttons: OUT_BUTTONS }).replace('margin:0 auto 8px;width:640px', 'margin:0')}</div></div>${composerBar()}</div>`, { w: 760, h: 420 }) },
    { key: 'C', name: 'The composer becomes the notice', from: 'new',
      desc: 'Until the reset, the composer is replaced by the notice and its buttons, so nothing can be sent on the used-up account.',
      good: 'No failed sends.', cost: 'Can’t queue a message for later from the composer.',
      mock: () => frame(`<div class="col" style="height:100%">${threadHeader()}<div class="grow" style="min-height:0;overflow:hidden">${conversation()}</div><div style="border-top:1px solid var(--b);padding:14px 0 16px">${limitBanner({ ...WORK_OUT, buttons: OUT_BUTTONS }).replace('rgba(222,193,132,.08)', 'var(--panel)')}</div></div>`, { w: 760, h: 420 }) },
  ],
});

// 15. Continuing on another account --------------------------------------------------------
const titleMenu = (items) => `<div class="menu" style="top:32px;left:110px;min-width:250px">${items}</div>`;
const titleItems = (extra = '', continueHl = false) => `${['pin', 'Pin'] && `<div class="it">${ic('pin', 'sm')}<span class="grow">Pin</span></div>`}<div class="it">${ic('pencil', 'sm')}<span class="grow">Rename</span></div><div class="it ${continueHl ? 'hl' : ''}">${ic('arrow', 'sm')}<span class="grow">Continue with Another Agent</span>${ic('chev-right', 'xs')}</div>${extra}<div class="hr"></div><div class="it">${ic('archive', 'sm')}<span class="grow">Archive</span></div><div class="it">${ic('trash', 'sm')}<span class="grow">Delete…</span></div>`;
const threadWithMenu = (menus) => frame(`<div class="col" style="height:100%">${threadHeader('Rate-limit the checkout API')}<div class="grow" style="min-height:0;overflow:hidden;opacity:.6">${conversation()}</div>${composerBar()}</div>${menus}`, { w: 760, h: 400 });
const otherAccounts = (style) => `<div class="menu" style="${style};min-width:300px">${accountItem(ACCTS.ext)}${accountItem(ACCTS.work, { dis: true, note: 'This thread’s account' })}${accountItem(ACCTS.side, { hl: true })}</div>`;

TOPICS.push({
  id: 'continue', section: 'Threads', title: 'Continuing on another account', size: 'wide', rec: 'B',
  now: 'A thread’s title menu has Continue with Another Agent ▸, which starts a new thread with the conversation handed over as a transcript. Accounts share no sessions, so moving to another account works the same way: a new thread on that account, carrying the conversation, linked to the old one.',
  options: [
    { key: 'A', name: 'Accounts in Continue with Another Agent', from: 'today’s submenu',
      desc: 'The submenu lists the agent’s other accounts under it (the thread’s own one greyed), then the other agents.',
      good: 'One place for every kind of continuing.', cost: '“Another Agent” for the same agent reads oddly.',
      mock: () => threadWithMenu(titleMenu(titleItems('', true)) + `<div class="menu" style="top:96px;left:364px;min-width:300px">${agentItem('claude', 'Claude Agent')}${accountItem(ACCTS.ext, { indent: 18 })}${accountItem(ACCTS.work, { indent: 18, dis: true, note: 'This thread’s account' })}${accountItem(ACCTS.side, { indent: 18, hl: true })}${agentItem('codex', 'Codex')}${agentItem('droid', 'Factory Droid')}</div>`) },
    { key: 'B', name: 'Its own menu item', from: 'today’s submenu, repeated',
      desc: '“Continue on Another Account ▸” under Continue with Another Agent, only when the agent has two or more accounts. It lists them with their limits; the thread’s own one is greyed. The limit notice’s “Continue on Side” does the same.',
      good: 'Says exactly what it does; the limits help choose.', cost: 'One more menu item.',
      mock: () => threadWithMenu(titleMenu(titleItems(`<div class="it hl">${ic('swap', 'sm')}<span class="grow">Continue on Another Account</span>${ic('chev-right', 'xs')}</div>`)) + otherAccounts('top:122px;left:364px')) },
    { key: 'C', name: 'Only from the limit notice', from: 'new',
      desc: 'No menu item; the limit notice’s “Continue on Side” is the only way.',
      good: 'Nothing new in the menu.', cost: 'Can’t move a thread to another account before the limit hits.',
      mock: () => threadView(limitBanner({ ...WORK_OUT, buttons: OUT_BUTTONS }), { header: threadHeader('Rate-limit the checkout API') }) },
  ],
});

// 16. Usage at a glance --------------------------------------------------------------------
const poolSegment = (a, left, reset) => `<div style="flex:1;position:relative;height:30px;border-radius:6px;background:#3b414d;overflow:hidden"><i style="position:absolute;left:0;top:0;bottom:0;width:${left}%;background:rgba(116,173,232,.35)"></i><span class="row g15 xs" style="position:relative;height:100%;padding:0 8px">${avatar(a, 16)}<b class="b5">${accountName(a)}</b>${left}%<span class="grow"></span>${reset ? `↻ ${reset}` : ''}</span></div>`;
const poolCard = (label, total, segments) => `<div class="card" style="padding:14px 16px;display:grid;grid-template-columns:150px 1fr;gap:16px;align-items:center;background:none"><div class="col" style="gap:2px"><span class="sm">${label}</span><span><span style="font-size:24px;font-weight:600">${total}%</span> <span class="sm mu">left</span></span><span class="xs mu">across ${segments.length} accounts</span></div><div class="row" style="gap:4px">${segments.join('')}</div></div>`;

TOPICS.push({
  id: 'overview', section: 'Threads', title: 'Usage at a glance', size: 'wide', type: 'multi', rec: ['A', 'C'],
  now: 'Nothing. Each agent’s page will show its accounts’ limits (topic 1). Pick any other places.',
  options: [
    { key: 'A', name: 'A Usage page in Settings', from: 't3code’s Usage page',
      desc: 'Settings › Usage lists every agent with accounts. Each window is one card: how much is left across all accounts, and a bar split into one segment per account with its % and reset. Clicking a segment opens the account on its agent’s page.',
      good: 'Answers “can I keep going” before “on which account”.', cost: 'A new page that repeats the agents’ pages.',
      mock: () => settingsWindow(`<div class="col" style="padding:24px 32px;gap:14px"><div style="font-size:17px">Usage</div><span class="row g2 b5">${glyph('claude')}Claude Agent</span>${poolCard('5-hour', 54, [poolSegment(ACCTS.ext, 62, '2h 10m'), poolSegment(ACCTS.work, 0, '1h 52m'), poolSegment(ACCTS.side, 100, '')])}${poolCard('Weekly', 74, [poolSegment(ACCTS.ext, 81, 'Thu'), poolSegment(ACCTS.work, 44, 'Mon'), poolSegment(ACCTS.side, 97, 'Sat')])}<span class="row g2 b5" style="margin-top:8px">${glyph('codex')}Codex</span>${poolCard('5-hour', 0, [poolSegment(CODEX_WORK, 0, '1h 12m')])}</div>`, { extraNav: [['Usage', 'gauge']], selected: 'Usage', h: 700 }) },
    { key: 'B', name: 'A gauge in the composer', from: 't3code ComposerUsageLimits',
      desc: 'Beside the agent chip, a small gauge with the thread’s account’s tightest window (“62%”). Clicking it shows that account’s windows, with Use Reset and Usage when they apply.',
      good: 'You see what’s left while you work.', cost: 'Another item in the composer row.',
      mock: () => threadView('', { composer: { agentLabel: `${glyph('claude')}Claude Agent</span><span class="row g1 sm" style="padding:1px 6px;border-radius:5px;background:var(--hov);color:var(--t)">${ic('gauge', 'xs')}62%` }, overlay: `<div class="pop" style="left:120px;bottom:64px;width:400px;padding:12px 14px"><div class="row g2" style="margin-bottom:10px">${avatar(ACCTS.ext, 18)}<span>alex@hey.com</span><span class="sm mu">Max 5x</span></div>${windowsGrid(ACCTS.ext.windows, { labelW: 120, resetW: 110 })}</div>` }) },
    { key: 'C', name: 'In the account picker', from: 'the picker topic',
      desc: 'Already in the account picker (“Picking the account for a new thread”, options A–C): each account with its tightest window. Pick this to keep that and nothing more.',
      good: 'No new place.', cost: 'Only seen when starting a thread.',
      mock: () => newThread({ accountChip: chipText(`${avatar(ACCTS.ext, 16)}alex@hey.com`), menu: accountMenu('top:84px;left:150px', { foot: false }) }) },
  ],
});
