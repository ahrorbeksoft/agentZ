// Accounts on the agent's page: the list, its usage limits, adding one, the agent's own login,
// and each account's menu. Also the shared pieces the other topic files use.

Object.assign(ICONS, {
  updown: '<path d="m7 15 5 5 5-5"/><path d="m7 9 5-5 5 5"/>',
  sparkle: '<path d="M9.937 15.5A2 2 0 0 0 8.5 14.063l-6.135-1.582a.5.5 0 0 1 0-.962L8.5 9.936A2 2 0 0 0 9.937 8.5l1.582-6.135a.5.5 0 0 1 .963 0L14.063 8.5A2 2 0 0 0 15.5 9.937l6.135 1.581a.5.5 0 0 1 0 .964L15.5 14.063a2 2 0 0 0-1.437 1.437l-1.582 6.135a.5.5 0 0 1-.963 0z"/>',
  'arrow-left': '<path d="m12 19-7-7 7-7"/><path d="M19 12H5"/>',
  rotate: '<path d="M21 12a9 9 0 1 1-9-9c2.52 0 4.93 1 6.74 2.74L21 8"/><path d="M21 3v5h-5"/>',
  logout: '<path d="m16 17 5-5-5-5"/><path d="M21 12H9"/><path d="M9 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h4"/>',
  'user-plus': '<path d="M16 21v-2a4 4 0 0 0-4-4H6a4 4 0 0 0-4 4v2"/><circle cx="9" cy="7" r="4"/><line x1="19" x2="19" y1="8" y2="14"/><line x1="22" x2="16" y1="11" y2="11"/>',
  key: '<path d="m15.5 7.5 2.3 2.3a1 1 0 0 0 1.4 0l2.1-2.1a1 1 0 0 0 0-1.4L19 4"/><path d="m21 2-9.6 9.6"/><circle cx="7.5" cy="15.5" r="5.5"/>',
  info: '<circle cx="12" cy="12" r="10"/><path d="M12 16v-4"/><path d="M12 8h.01"/>',
  ticket: '<path d="M2 9a3 3 0 0 1 0 6v2a2 2 0 0 0 2 2h16a2 2 0 0 0 2-2v-2a3 3 0 0 1 0-6V7a2 2 0 0 0-2-2H4a2 2 0 0 0-2 2Z"/><path d="M13 5v2"/><path d="M13 17v2"/><path d="M13 11v2"/>',
  gauge: '<path d="m12 14 4-4"/><path d="M3.34 19a10 10 0 1 1 17.32 0"/>',
  book: '<path d="M12 7v14"/><path d="M3 18a1 1 0 0 1-1-1V4a1 1 0 0 1 1-1h5a4 4 0 0 1 4 4 4 4 0 0 1 4-4h5a1 1 0 0 1 1 1v13a1 1 0 0 1-1 1h-6a3 3 0 0 0-3 3 3 3 0 0 0-3-3z"/>',
  plug: '<path d="M12 22v-5"/><path d="M9 8V2"/><path d="M15 8V2"/><path d="M18 8v5a4 4 0 0 1-4 4h-4a4 4 0 0 1-4-4V8Z"/>',
  coins: '<circle cx="8" cy="8" r="6"/><path d="M18.09 10.37A6 6 0 1 1 10.34 18"/><path d="M7 6h1v4"/><path d="m16.71 13.88.7.71-2.82 2.82"/>',
  hourglass: '<path d="M5 22h14"/><path d="M5 2h14"/><path d="M17 22v-4.172a2 2 0 0 0-.586-1.414L12 12l-4.414 4.414A2 2 0 0 0 7 17.828V22"/><path d="M7 2v4.172a2 2 0 0 0 .586 1.414L12 12l4.414-4.414A2 2 0 0 0 17 6.172V2"/>',
  swap: '<path d="M8 3 4 7l4 4"/><path d="M4 7h16"/><path d="m16 21 4-4-4-4"/><path d="M20 17H4"/>',
  download: '<path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/><polyline points="7 10 12 15 17 10"/><line x1="12" x2="12" y1="15" y2="3"/>',
  lock: '<rect width="18" height="11" x="3" y="11" rx="2" ry="2"/><path d="M7 11V7a5 5 0 0 1 10 0v4"/>',
  send: '<path d="M14.54 21.69a.5.5 0 0 0 .94-.03l6.5-19a.5.5 0 0 0-.64-.64l-19 6.5a.5.5 0 0 0-.03.94l7.93 3.18a2 2 0 0 1 1.11 1.11z"/><path d="m21.85 2.15-10.94 10.94"/>',
  archive: '<rect width="20" height="5" x="2" y="3" rx="1"/><path d="M4 8v11a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8"/><path d="M10 12h4"/>',
});
GLYPHS.droid = '◆';
GLYPHS.devin = '◐';

// The demo world: Claude Agent with three accounts; Codex and Droid for their own actions.
const AGENT = {
  claude: { name: 'Claude Agent', kind: 'claude', sub: 'v0.33.1 · Claude Code over ACP' },
  codex: { name: 'Codex', kind: 'codex', sub: 'v2.1.1 · OpenAI’s coding agent over ACP' },
  droid: { name: 'Factory Droid', kind: 'droid', sub: 'v0.234.0 · Factory’s coding agent' },
};
const ACCTS = {
  ext: { email: 'alex@hey.com', plan: 'Max 5x', hue: 212, external: true,
    windows: [['5-hour', 62, 'resets in 2h 10m', 0.57], ['Weekly', 81, 'resets Thu 09:00', 0.35]] },
  work: { email: 'alex@acme.co', label: 'Work', plan: 'Team', hue: 150,
    windows: [['5-hour', 0, 'resets in 1h 52m', 0.63], ['Weekly', 44, 'resets Mon 06:00', 0.7]] },
  side: { email: 'alex.side@gmail.com', label: 'Side', plan: 'Pro', hue: 28,
    windows: [['5-hour', 100, '', null], ['Weekly', 97, 'resets Sat 11:00', 0.12]] },
};
const CLAUDE_ACCOUNTS = [ACCTS.ext, ACCTS.work, ACCTS.side];
const accountName = (a) => a.label || a.email;
const accountDetail = (a) => [a.label ? a.email : null, a.plan].filter(Boolean).join(' · ');
// The window closest to running out, which pickers show.
const tightest = (a) => a.windows.reduce((low, w) => (w[1] < low[1] ? w : low));
const leftText = (w) => (w[1] === 0 ? `<span class="delc">Used up · ${w[2].replace('resets in ', '')}</span>` : `${w[1]}% left`);

// Pieces in the app's style (settings_page.rs and ui's buttons).
const avatar = (a, size = 28) => `<span style="width:${size}px;height:${size}px;border-radius:50%;flex:none;display:inline-grid;place-items:center;font-size:${Math.round(size * 0.43)}px;font-weight:600;color:#1b1f26;background:linear-gradient(135deg,hsl(${a.hue} 70% 82%),hsl(${a.hue} 42% 60%))">${accountName(a)[0].toUpperCase()}</span>`;
const tag = (text, color = 'mu') => `<span class="xs none" style="padding:1px 6px;border-radius:4px;background:var(--hov);color:var(--${color})">${text}</span>`;
const obtn = (text, extra = '') => `<span class="btn" style="${extra}">${text}</span>`;
const gbtn = (text) => `<span class="btn" style="border-color:transparent;background:none">${text}</span>`;
const pbtn = (text) => `<span class="btn primary">${text}</span>`;
const dd = (label) => `<span class="row" style="gap:4px;white-space:nowrap">${label}${ic('updown', 'xs mu')}</span>`;
const chev = ic('chev-down', 'xs');
const toggle = (on) => `<span style="width:34px;height:20px;border-radius:10px;flex:none;display:inline-block;position:relative;vertical-align:middle;background:${on ? '#5a7099' : '#353a44'};border:1px solid ${on ? '#6f86b0' : 'var(--b)'}"><i style="position:absolute;top:2px;${on ? 'right:2px' : 'left:2px'};width:14px;height:14px;border-radius:50%;background:${on ? '#e6e9ee' : '#6b717d'}"></i></span>`;
const piece = (html, w = 720) => `<div class="m" style="width:${w}px">${html}</div>`;
const card = (rows, style = '') => `<div class="card" style="overflow:visible;position:relative;${style}">${rows.filter(Boolean).map((row, index) => `<div style="position:relative;${index ? 'border-top:1px solid var(--bv)' : ''}">${row}</div>`).join('')}</div>`;
const listHead = (title, action = `<span class="row g1 sm mu">${ic('plus', 'xs')}Add Account</span>`) => `<div class="row" style="justify-content:space-between"><span class="sm mu">${title}</span>${action}</div>`;
const infoNote = (html) => `<div class="card row" style="padding:12px 16px;gap:10px;align-items:flex-start;background:none"><span class="ac" style="display:inline-flex;margin-top:1px">${ic('info', 'sm')}</span><span class="sm" style="line-height:1.5">${html}</span></div>`;

/** Menus: items are [icon, label, flags] with flags { hl, dis, danger, kb, check }, 'hr', or ['lbl', text]. */
function menuList(items, style = 'top:44px;right:12px', width = 230) {
  const body = items.map((item) => {
    if (item === 'hr') return '<div class="hr"></div>';
    if (item[0] === 'lbl') return `<div class="lbl">${item[1]}</div>`;
    const [icon, label, flags = {}] = item;
    return `<div class="it ${flags.hl ? 'hl' : ''} ${flags.dis ? 'dis' : ''} ${flags.danger ? 'danger' : ''}">${icon ? ic(icon, 'sm') : ''}<span class="grow">${label}</span>${flags.kb ? `<span class="kb">${flags.kb}</span>` : ''}${flags.check ? ic('check', 'sm ac') : ''}${flags.sub ? ic('chev-right', 'xs') : ''}</div>`;
  }).join('');
  return `<div class="menu" style="${style};min-width:${width}px">${body}</div>`;
}
const dialog = (title, body, buttons, { w = 420, style = '' } = {}) => `<div class="modal-back"></div><div class="pop" style="z-index:21;left:50%;top:50%;transform:translate(-50%,-50%);width:${w}px;padding:18px 18px 14px;${style}"><div class="b6" style="font-size:15px;margin-bottom:6px">${title}</div><div class="sm mu" style="line-height:1.5">${body}</div><div class="row g2" style="justify-content:flex-end;margin-top:16px">${buttons}</div></div>`;

/** One usage window as t3code's bar: the fill is what's left, the hairline where even spending would be. */
function bar(left, { mark = null, h = 6, w = '100%' } = {}) {
  const color = left === 0 ? 'var(--del)' : left <= 15 ? 'var(--warn)' : 'var(--ac)';
  return `<span style="position:relative;display:block;width:${w};height:${h}px;border-radius:${h}px;background:#3b414d">${left ? `<i style="position:absolute;left:0;top:0;bottom:0;width:${left}%;border-radius:${h}px;background:${color}"></i>` : ''}${mark !== null ? `<i style="position:absolute;top:-3px;bottom:-3px;left:${mark * 100}%;width:1px;background:rgba(220,224,229,.55)"></i>` : ''}</span>`;
}
/** t3code's LimitWindows: label and % left, the bar, and when it resets. */
function windowsGrid(windows, { labelW = 140, resetW = 116 } = {}) {
  const rows = windows.map(([label, left, resets, mark]) => `<span class="row sm" style="gap:8px"><span class="mu grow trunc">${label}</span><span class="b5 none">${left === 0 ? '<span class="delc">Used up</span>' : `${left}% left`}</span></span>${bar(left, { mark })}<span class="sm mu" style="text-align:right;white-space:nowrap">${resets}</span>`).join('');
  return `<div style="display:grid;grid-template-columns:${labelW}px minmax(0,1fr) ${resetW}px;column-gap:14px;row-gap:9px;align-items:center">${rows}</div>`;
}
const windowsBlock = (a, extra = '') => `<div style="padding:0 16px 14px 56px">${windowsGrid(a.windows)}${extra}</div>`;

/** The top of an account's card: today's avatar, title and details, then its controls. */
function accountHead(a, { right = ibtn('more'), menu = '', tags = '', name, detail } = {}) {
  const shownTags = tags || (a.external ? tag('Outside agentZ') : '');
  return `<div class="row g3" style="padding:12px 16px;position:relative">${avatar(a)}<div class="col grow" style="gap:2px;min-width:0"><span class="row g2"><span class="trunc">${name ?? accountName(a)}</span>${shownTags}</span><span class="sm mu trunc">${detail ?? accountDetail(a)}</span></div><span class="row g1 none">${right}</span>${menu}</div>`;
}
const accountCard = (a, { head = {}, body, extra = '' } = {}) => card([`${accountHead(a, head)}${body ?? windowsBlock(a)}`, extra || null]);

/** The agent's page: breadcrumb, heading with the status badge, and its tabs (render_agent_page). */
function agentPage(body, { agent = AGENT.claude, badge = ['Logged in', 'ok'], tab = 'Account', tabs = ['Account', 'Defaults', 'Environment', 'Threads'], pad = 28 } = {}) {
  const badgeColor = badge[1] === 'ok' ? 'rgba(161,193,129,.14)' : 'rgba(222,193,132,.14)';
  const tabRow = tabs.map((name) => `<span style="padding:0 0 9px;${name === tab ? 'color:var(--t);border-bottom:2px solid var(--ac);margin-bottom:-1px' : 'color:var(--mu)'}">${name}</span>`).join('');
  return `<div class="col" style="padding:24px 32px ${pad}px;gap:20px">
    <div class="col" style="gap:22px">
      <span class="row g15 sm mu">${ic('arrow-left', 'xs')}Agents</span>
      <div class="row g3"><span style="width:46px;height:46px;border-radius:10px;border:1px solid var(--b);background:var(--panel);display:grid;place-items:center;font-size:20px;flex:none">${GLYPHS[agent.kind]}</span>
        <div class="col grow" style="gap:4px"><span class="row g2"><span style="font-size:17px;font-weight:600">${agent.name}</span><span class="row xs" style="gap:5px;padding:1px 7px;border-radius:4px;background:${badgeColor};color:var(--${badge[1]})"><i class="dot" style="background:var(--${badge[1]})"></i>${badge[0]}</span></span><span class="sm mu">${agent.sub}</span></div>
        <span class="ibtn" style="border:1px solid var(--b);width:28px;height:28px">${ic('more', 'sm')}</span></div>
      <div class="row" style="gap:22px;border-bottom:1px solid var(--b);font-size:14px">${tabRow}</div>
    </div>${body}</div>`;
}
const accountsList = (cards, { head = listHead('Accounts') } = {}) => `<div class="col" style="gap:10px">${head}${cards.join('')}</div>`;

/** Settings with the content column (settings_page.rs render_nav). */
function settingsWindow(contentHtml, { extraNav = [], selected = 'Agents', w = 1010, h = 760 } = {}) {
  const nav = [['General', 'settings'], ['Appearance', 'eye'], ['Notifications', 'bell'], ['Agents', 'sparkle'], ...extraNav, ['Machines', 'server']]
    .map(([label, icon]) => `<div class="row g2" style="height:28px;padding:0 8px;border-radius:6px;margin:0 4px;${label === selected ? 'background:var(--sel)' : ''}">${ic(icon, 'sm mu')}<span>${label}</span></div>`).join('');
  return `<div class="m win" style="width:${w}px;height:${h}px">
    <div class="titlebar">${lights()}${ic('sidebar', 'sm mu')}<span class="row g15" style="font-size:13px">${ic('list', 'xs mu')}All projects${ic('chev-down', 'xs mu')}</span><div class="viewtabs"><span class="on">Agents</span><span>Workspaces</span></div></div>
    <div class="body"><div class="sidebar"><div class="sb-head" style="color:var(--t)"><span class="grow b5" style="color:var(--t)">Settings</span>${ic('x', 'sm mu')}</div><div class="sb-list">${nav}<div class="sm mu" style="padding:14px 12px 4px">Projects</div><div class="row g2" style="height:28px;padding:0 12px">${mono('ST', 'g')}<span>storefront</span></div></div><div class="sb-foot">${ic('arrow-left', 'sm')}<span>Back</span></div></div>
    <div class="grow" style="height:100%;overflow:hidden;background:var(--ed);display:flex;justify-content:center"><div style="width:720px">${contentHtml}</div></div></div></div>`;
}

// 1. The list ------------------------------------------------------------------------------
const miniWindows = (a) => a.windows.map(([label, left]) => `<span class="col" style="gap:3px;width:64px"><span class="xs mu row"><span class="grow">${label === 'Weekly' ? 'Week' : '5h'}</span>${left === 0 ? '<span class="delc">0%</span>' : `${left}%`}</span>${bar(left, { h: 4 })}</span>`).join('');

TOPICS.push({
  id: 'list', section: 'Agent page', title: 'Accounts on the agent’s page', size: 'wide', rec: 'A',
  now: 'One Account card on the agent’s Account tab: avatar, email, plan, Change Account and Log Out, with a note when the login came from outside agentZ. Change Account replaces the login; there is no second one.',
  nowImg: 'img/now-account.png',
  issues: ['One login per agent', 'No usage limits anywhere'],
  options: [
    { key: 'A', name: 'A card per account, limits inside', from: 'today’s Account card, t3code’s limit rows',
      desc: 'The Account tab lists every account as today’s card, with its limits under the email: each window’s bar, % left and reset. “Add Account” sits over the list, as “Add Agent” does over the agents. Each card has a ⋯ menu (topic 5). The External account comes first, then agentZ’s in the order they were added.',
      good: 'Everything about an account in one place; reads like today’s page.', cost: 'Long with four or more accounts.',
      mock: () => settingsWindow(agentPage(accountsList(CLAUDE_ACCOUNTS.map((a) => accountCard(a)))), { h: 800 }) },
    { key: 'B', name: 'A row per account, opened for details', from: 'Settings rows, Agents list',
      desc: 'One group with a row per account: avatar, name, plan, and two small bars for the windows. Clicking a row opens it in place, with the full limits and its buttons.',
      good: 'Compact; many accounts fit.', cost: 'Reset times and actions are a click away.',
      mock: () => piece(agentPage(accountsList([card([
        `<div class="row g3" style="padding:10px 16px">${avatar(ACCTS.ext)}<div class="col grow" style="gap:2px"><span class="row g2">${accountName(ACCTS.ext)}${tag('Outside agentZ')}</span><span class="sm mu">${accountDetail(ACCTS.ext)}</span></div><span class="row g3">${miniWindows(ACCTS.ext)}</span>${ic('chev-right', 'sm mu')}</div>`,
        `<div class="row g3" style="padding:10px 16px">${avatar(ACCTS.work)}<div class="col grow" style="gap:2px"><span>${accountName(ACCTS.work)}</span><span class="sm mu">${accountDetail(ACCTS.work)}</span></div><span class="row g3">${miniWindows(ACCTS.work)}</span>${ic('chev-down', 'sm mu')}</div><div style="padding:0 16px 14px 56px">${windowsGrid(ACCTS.work.windows)}<div class="row g2" style="margin-top:12px">${obtn('Rename')}${gbtn('Log Out')}<span class="grow"></span>${ibtn('more')}</div></div>`,
        `<div class="row g3" style="padding:10px 16px">${avatar(ACCTS.side)}<div class="col grow" style="gap:2px"><span>${accountName(ACCTS.side)}</span><span class="sm mu">${accountDetail(ACCTS.side)}</span></div><span class="row g3">${miniWindows(ACCTS.side)}</span>${ic('chev-right', 'sm mu')}</div>`,
      ])]))) },
    { key: 'C', name: 'Today’s card with an account switcher', from: 'new',
      desc: 'The tab keeps one card, today’s, with the limits added. A dropdown over it picks which account the card shows, and ends with “Add Account…”.',
      good: 'The page barely changes.', cost: 'Accounts can’t be compared; one shows at a time.',
      mock: () => piece(agentPage(`<div class="col" style="gap:10px;position:relative"><div class="row g2"><span class="sm mu">Account</span><span class="chip" style="color:var(--t)">${avatar(ACCTS.ext, 16)}alex@hey.com${ic('chev-down', 'xs')}</span><span class="sm ph">3 accounts</span></div>
        ${menuList([['', `${avatar(ACCTS.ext, 16)}&nbsp; alex@hey.com`, { check: true }], ['', `${avatar(ACCTS.work, 16)}&nbsp; Work`], ['', `${avatar(ACCTS.side, 16)}&nbsp; Side`], 'hr', ['plus', 'Add Account…']], 'top:26px;left:58px', 220)}
        ${accountCard(ACCTS.ext, { head: { right: `${obtn('Log Out')}` } })}</div>`, { pad: 150 })) },
    { key: 'D', name: 'A table', from: 'new',
      desc: 'One row per account with a column per window: account, plan, 5-hour, weekly, then ⋯.',
      good: 'Comparing accounts at a glance.', cost: 'Agents have different windows (Droid has monthly, Codex’s free plan one 30-day window), so the columns change per agent and get cramped.',
      mock: () => piece(agentPage(accountsList([card([
        `<div style="display:grid;grid-template-columns:1fr 70px 130px 130px 24px;gap:12px;padding:8px 16px" class="xs mu"><span>Account</span><span>Plan</span><span>5-hour</span><span>Weekly</span><span></span></div>`,
        ...CLAUDE_ACCOUNTS.map((a) => `<div style="display:grid;grid-template-columns:1fr 70px 130px 130px 24px;gap:12px;padding:10px 16px;align-items:center"><span class="row g2" style="min-width:0">${avatar(a, 22)}<span class="trunc">${accountName(a)}</span>${a.external ? tag('Outside') : ''}</span><span class="sm mu">${a.plan}</span>${a.windows.map(([, left, resets]) => `<span class="col" style="gap:3px"><span class="xs">${left === 0 ? '<span class="delc">Used up</span>' : `${left}% left`} <span class="ph">${resets.replace('resets ', '')}</span></span>${bar(left, { h: 4 })}</span>`).join('')}${ibtn('more', 'sm')}</div>`),
      ])]))) },
    { key: 'E', name: 'Each account its own agent', from: 't3code provider instances',
      desc: 'As t3code adds a second Codex: every account is an entry in the Agents list (“Claude Agent · Work”), with an initials badge on the icon and its own page with its own defaults and environment.',
      good: 'Per-account defaults and environment for free.', cost: 'Agents multiply in the list and in every picker, and each account’s settings must be set again.',
      mock: () => piece(`<div class="col" style="padding:24px 32px 28px;gap:12px"><div style="font-size:17px">Agents</div><div class="row" style="justify-content:space-between"><span class="sm mu">Installed</span><span class="row g1 sm mu">${ic('plus', 'xs')}Add Agent</span></div>${card([
        ['Claude Agent', 'registry · v0.33.1', null], ['Claude Agent · Work', 'alex@acme.co · Team', ACCTS.work], ['Claude Agent · Side', 'alex.side@gmail.com · Pro', ACCTS.side], ['Codex', 'registry · v2.1.1', null, 'codex'],
      ].map(([name, sub, a, kind = 'claude']) => `<div class="row g3" style="padding:10px 16px"><span style="position:relative;width:30px;height:30px;border-radius:7px;border:1px solid var(--b);display:grid;place-items:center;flex:none">${GLYPHS[kind]}${a ? `<span style="position:absolute;right:-5px;bottom:-5px">${avatar(a, 15)}</span>` : ''}</span><div class="col grow" style="gap:2px"><span>${name}</span><span class="sm mu">${sub}</span></div>${ic('chev-right', 'sm mu')}</div>`))}</div>`) },
  ],
});

// 2. How the limits read -------------------------------------------------------------------
TOPICS.push({
  id: 'quota', section: 'Agent page', title: 'How an account’s limits read', size: 'wide', rec: 'A',
  now: 'agentZ shows no usage. The agents say it differently: Claude’s <code>/usage</code> says “38% used · resets 4:10pm”, Codex says “62% left (resets 16:10)”, Droid draws bars per window, t3code shows “62% left” with a bar and “resets in 2h 10m”. The mocks show the External account; the windows come from each agent (5-hour and weekly here; Droid adds monthly).',
  options: [
    { key: 'A', name: 'Bars with % left and the reset', from: 't3code LimitWindows',
      desc: 'A row per window: its name and “62% left”, a bar of what’s left, and “resets in 2h 10m”. A hairline on the bar marks where even spending would be; the tooltip gives the exact time. The bar turns yellow under 15% and red when used up.',
      good: 'Reads at a glance; the hairline tells you whether you’ll run out before the reset.', cost: 'Says “left” where Claude says “used”.',
      mock: () => piece(`<div style="padding:16px">${accountCard(ACCTS.ext)}</div>`) },
    { key: 'B', name: 'Bars with % used, as Claude says it', from: 'Claude’s /usage',
      desc: 'The same rows counting up: “38% used”, a bar that fills as you spend, “resets 4:10 PM”.',
      good: 'Matches Claude’s and Droid’s own screens.', cost: 'Codex and t3code count down; a full bar means trouble, which reads backwards next to a battery-like icon.',
      mock: () => piece(`<div style="padding:16px">${accountCard(ACCTS.ext, { body: `<div style="padding:0 16px 14px 56px"><div style="display:grid;grid-template-columns:140px 1fr 116px;column-gap:14px;row-gap:9px;align-items:center">${[['5-hour', 38, 'resets 4:10 PM'], ['Weekly', 19, 'resets Thu 09:00']].map(([label, used, at]) => `<span class="row sm"><span class="mu grow">${label}</span><span class="b5">${used}% used</span></span>${bar(used)}<span class="sm mu" style="text-align:right">${at}</span>`).join('')}</div></div>` })}</div>`) },
    { key: 'C', name: 'One line of text', from: 'Codex’s /status',
      desc: 'The windows as text under the email, no bars: “5-hour 62% left · resets in 2h 10m · Weekly 81% left · resets Thu”.',
      good: 'The smallest; no new drawing.', cost: 'Harder to compare; used-up only shows in color.',
      mock: () => piece(`<div style="padding:16px">${accountCard(ACCTS.ext, { body: '', head: { detail: `Max 5x · 5-hour <span style="color:var(--t)">62% left</span>, resets in 2h 10m · Weekly <span style="color:var(--t)">81% left</span>, resets Thu` } })}</div>`) },
    { key: 'D', name: 'Rings beside the email', from: 'new',
      desc: 'A small ring per window at the right of the account’s row, with the reset in its tooltip.',
      good: 'Compact, and the row stays one line.', cost: 'Tiny; the reset time needs a hover.',
      mock: () => piece(`<div style="padding:16px">${accountCard(ACCTS.ext, { body: '', head: { right: ACCTS.ext.windows.map(([label, left]) => `<span class="col" style="align-items:center;gap:2px;margin-right:6px"><span style="width:28px;height:28px;border-radius:50%;background:conic-gradient(var(--ac) ${left * 3.6}deg,#3b414d 0);display:grid;place-items:center"><i style="width:20px;height:20px;border-radius:50%;background:var(--panel);display:grid;place-items:center;font-style:normal;font-size:9px">${left}</i></span><span class="xs mu">${label === 'Weekly' ? 'Week' : '5h'}</span></span>`).join('') + ibtn('more') } })}</div>`) },
    { key: 'E', name: 'Only the window closest to running out', from: 'new',
      desc: 'One bar for whichever window has the least left, named (“5-hour: 62% left”); the others in its tooltip.',
      good: 'The number that decides whether you can keep working.', cost: 'The weekly window hides until it’s the tighter one.',
      mock: () => piece(`<div style="padding:16px">${accountCard(ACCTS.ext, { body: `<div style="padding:0 16px 14px 56px">${windowsGrid([ACCTS.ext.windows[0]])}<span class="xs ph" style="display:block;margin-top:6px">Weekly: 81% left, in the tooltip</span></div>` })}</div>`) },
  ],
});

// 3. Adding an account ---------------------------------------------------------------------
const LOGIN_ROWS = [['terminal', 'Log in with Claude.ai', 'Runs Claude’s login in a terminal where it runs.', true], ['terminal', 'Log in with the Anthropic Console', 'For an API plan billed by the Console.', false]];
const loginRows = (rows = LOGIN_ROWS) => rows.map(([icon, title, desc, primary]) => `<div class="row g3" style="padding:10px 16px"><span style="width:24px;height:24px;border-radius:6px;border:1px solid var(--bv);display:grid;place-items:center" class="mu">${ic(icon, 'xs')}</span><div class="col grow" style="gap:2px"><span>${title}</span><span class="sm mu">${desc}</span></div>${primary ? pbtn('Log In') : obtn('Log In')}</div>`).join('');
const newAccountCard = () => card([
  `<div class="row g3" style="padding:12px 16px"><span style="width:28px;height:28px;border-radius:50%;border:1.5px dashed var(--b);display:grid;place-items:center" class="mu">${ic('plus', 'xs')}</span><div class="col grow" style="gap:2px"><span>New account</span><span class="sm mu">Choose how Claude Agent logs in. The account takes its email once it’s logged in.</span></div>${gbtn('Cancel')}</div>`,
  `<div style="border-top:0">${loginRows()}</div>`,
]);

TOPICS.push({
  id: 'add', section: 'Agent page', title: 'Adding an account', size: 'wide', rec: 'A',
  now: 'Not possible. Logging in from the Account card (the rows below, as today for a logged-out agent) or Change Account always uses the agent’s one login.',
  nowImg: 'img/now-logged-out.png',
  options: [
    { key: 'A', name: 'A new card with today’s login rows', from: 'today’s login rows',
      desc: 'Add Account puts a “New account” card at the end of the list with the agent’s login methods, as a logged-out agent shows them today. agentZ makes the account’s folder and runs the login there. Once logged in, the card becomes the account, named by its email; Rename can give it a shorter name later. Cancel removes the empty folder.',
      good: 'The login flow everyone has seen already; no naming before you know which account it is.', cost: 'The name is the email until you rename it.',
      mock: () => piece(agentPage(accountsList([accountCard(ACCTS.ext), accountCard(ACCTS.work), newAccountCard()]))) },
    { key: 'B', name: 'Name it, then log in', from: 't3code’s Add ChatGPT account',
      desc: 'Add Account opens a dialog asking for the account’s name (“Personal”, “Work”); Continue shows the login methods in the same dialog, and it closes once logged in.',
      good: 'Every account has a short name from the start.', cost: 'A dialog and a name to think of before logging in.',
      mock: () => frame(agentPage(accountsList([accountCard(ACCTS.ext)])) + dialog('Add Claude Agent account', `Each account has its own login, sessions and history. Choose the other account on the login page.<div class="col" style="gap:6px;margin-top:14px"><span style="color:var(--t)">Name</span><div class="field focus">Work<span style="width:1.5px;height:15px;background:var(--ac)"></span></div><span class="xs ph">Shown in the account list and the new thread’s picker.</span></div>`, `${obtn('Cancel')}${pbtn('Continue')}`, { w: 400 }), { w: 720, h: 560 }) },
    { key: 'C', name: 'Pick the login method from the button', from: 'new',
      desc: 'Add Account opens a menu of the agent’s login methods. Picking one starts that login right away (a terminal or the browser, as today); the account shows up once the login finishes.',
      good: 'One click less.', cost: 'Nothing on the page while a terminal login is open elsewhere; a long menu for agents with many methods.',
      mock: () => frame(agentPage(`<div class="col" style="gap:10px;position:relative"><div class="row" style="justify-content:space-between"><span class="sm mu">Accounts</span><span class="row g1 sm" style="padding:2px 6px;border-radius:5px;background:var(--hov)">${ic('plus', 'xs')}Add Account${chev}</span></div>${menuList([['terminal', 'Log in with Claude.ai', { hl: true }], ['terminal', 'Log in with the Anthropic Console']], 'top:24px;right:0', 250)}${accountCard(ACCTS.ext)}${accountCard(ACCTS.work)}</div>`), { w: 720, h: 520 }) },
  ],
});

// 4. The agent's own login -----------------------------------------------------------------
TOPICS.push({
  id: 'external', section: 'Agent page', title: 'The agent’s own login', size: 'wide', rec: 'A',
  now: 'The card shows the agent’s own login, with a note under it: “Logged in outside agentZ. … Every thread with it uses this login, and logging out here logs out the CLI too.” In the new design this is the External account: listed only while the agent is logged in outside agentZ, never removed by agentZ, and the one existing threads stay on.',
  nowImg: 'img/now-account.png',
  options: [
    { key: 'A', name: 'First in the list, tagged', from: 'today’s note, as a tag',
      desc: 'The External account comes first with an “Outside agentZ” tag. Today’s note moves into the tag’s tooltip. Its menu has no Remove; Log Out asks first and says it logs out the CLI too.',
      good: 'One list, and the difference is still visible.', cost: 'The explanation is behind a hover.',
      mock: () => piece(agentPage(accountsList([accountCard(ACCTS.ext, { head: { tags: `<span style="position:relative">${tag('Outside agentZ')}<span class="pop xs" style="left:-20px;top:22px;width:330px;padding:8px 10px;line-height:1.45;color:var(--t)">Claude Agent’s own login on This Mac, from its CLI. Threads on it go into the CLI’s history, and logging out here logs out the CLI too.</span></span>` } }), accountCard(ACCTS.work), accountCard(ACCTS.side)]), { pad: 30 })) },
    { key: 'B', name: 'Its own section', from: 'new',
      desc: 'Two groups: “Logged in outside agentZ” with the External account, then “agentZ accounts” with Add Account.',
      good: 'The difference is spelled out.', cost: 'Two headings for what is usually two or three accounts.',
      mock: () => piece(agentPage(`<div class="col" style="gap:18px">${accountsList([accountCard(ACCTS.ext)], { head: listHead('Logged in outside agentZ', '') })}${accountsList([accountCard(ACCTS.work), accountCard(ACCTS.side)], { head: listHead('agentZ accounts') })}</div>`)) },
    { key: 'C', name: 'Today’s note under its card', from: 'today',
      desc: 'Today’s info note stays, right under the External account’s card.',
      good: 'Nothing new to learn.', cost: 'A paragraph between cards.',
      mock: () => piece(agentPage(accountsList([accountCard(ACCTS.ext, { head: { tags: ' ' } }), infoNote('<b>Logged in outside agentZ.</b> Claude Agent found the login it already had on This Mac (from its CLI). Threads on it use this login, and logging out here logs out the CLI too.'), accountCard(ACCTS.work), accountCard(ACCTS.side)]))) },
    { key: 'D', name: 'Named after where it lives', from: 'new',
      desc: 'Its title is “Claude Code CLI” and the email moves to the second line, so it reads as the CLI’s login rather than one of agentZ’s.',
      good: 'Says what it is without a tag.', cost: 'Its email is less prominent than the others’.',
      mock: () => piece(agentPage(accountsList([accountCard(ACCTS.ext, { head: { name: 'Claude Code CLI', tags: ' ', detail: 'alex@hey.com · Max 5x' } }), accountCard(ACCTS.work), accountCard(ACCTS.side)]))) },
  ],
});

// 5. The account's menu --------------------------------------------------------------------
const MENU_ITEMS = {
  A: ['pencil', 'Rename…'], B: ['star', 'Use for New Threads'], C: ['rotate', 'Refresh Usage', { kb: 'read 3 min ago' }],
  D: ['external', 'Open Usage Page'], E: ['folder', 'Show in Finder'], F: ['logout', 'Log Out'], G: ['trash', 'Remove Account…', { danger: true }],
};
function menuMock(key, { note: noteText = '', tags = '' } = {}) {
  const items = ['A', 'B', 'C', 'D', 'E', 'hr', 'F', 'G'].map((k) => (k === 'hr' ? 'hr' : [MENU_ITEMS[k][0], MENU_ITEMS[k][1], { ...(MENU_ITEMS[k][2] || {}), hl: k === key }]));
  return frame(`<div class="col" style="padding:16px;gap:10px">${accountCard(ACCTS.work, { head: { tags, right: ibtn('more', 'on'), menu: menuList(items, 'top:44px;right:12px', 250) } })}${accountCard(ACCTS.side)}</div>${noteText ? note(noteText, 'bottom:14px;left:16px') : ''}`, { w: 560, h: 330 });
}

TOPICS.push({
  id: 'menu', section: 'Agent page', title: 'What each account’s menu offers', size: 'medium', type: 'multi', rec: ['A', 'B', 'C', 'D', 'F', 'G'],
  now: 'The agent’s ⋯ menu has Uninstall; the card has Change Account and Log Out. Pick any for the account’s ⋯ menu. The limit actions have their own topics under “When limits run out”.',
  options: [
    { key: 'A', name: 'Rename', from: 't3code’s instance name',
      desc: 'A short name shown instead of the email, here and in pickers (“Work”). Edited in place on the card.',
      good: 'Emails are long and alike.', cost: '—',
      mock: () => frame(`<div class="col" style="padding:16px;gap:10px">${card([`<div class="row g3" style="padding:12px 16px">${avatar(ACCTS.work)}<div class="col grow" style="gap:4px"><div class="field focus" style="height:24px;width:220px;font-size:14px">Work<span style="width:1.5px;height:14px;background:var(--ac)"></span></div><span class="sm mu">alex@acme.co · Team</span></div>${ibtn('more')}</div>${windowsBlock(ACCTS.work)}`])}</div>`, { w: 560, h: 200 }) },
    { key: 'B', name: 'Use for New Threads', from: 'new',
      desc: 'Marks the account new threads start on, with a “Default” tag. Without it, new threads take the External account, or else the first one.',
      good: 'Keeps the personal plan out of work threads.', cost: 'One more thing to explain.',
      mock: () => menuMock('B', { tags: tag('Default', 'ac') }) },
    { key: 'C', name: 'Refresh Usage', from: 't3code’s Refresh',
      desc: 'Reads the limits now; the item says when they were last read. They’re also read every 5 minutes and after each turn.',
      good: 'Fresh numbers before a long task.', cost: 'Some agents take a few seconds (a hidden terminal for Droid and Devin).',
      mock: () => menuMock('C') },
    { key: 'D', name: 'Open Usage Page', from: 't3code’s Manage usage',
      desc: 'Opens the vendor’s usage or billing page in the browser: claude.ai/settings/usage, chatgpt.com/codex/settings/usage, app.factory.ai/settings/billing, app.devin.ai/settings/usage.',
      good: 'Upgrades, payments and invoices stay on the vendor’s site.', cost: 'Opens whatever account the browser is logged in to.',
      mock: () => menuMock('D') },
    { key: 'E', name: 'Show in Finder', from: 'new',
      desc: 'Opens the account’s folder (<code>accounts/claude-acp/work</code> in the data folder).',
      good: 'Handy for debugging.', cost: 'Exposes files people shouldn’t edit.',
      mock: () => menuMock('E') },
    { key: 'F', name: 'Log Out', from: 'today',
      desc: 'Logs the account out and keeps it in the list as logged out, with Log In on its card. Its threads stay and ask to log in again.',
      good: 'Today’s behavior, per account.', cost: '—',
      mock: () => menuMock('F') },
    { key: 'G', name: 'Remove Account', from: 'new',
      desc: 'Asks first, then deletes the account’s folder: its login, sessions and history. Its threads stay in the sidebar but can’t continue (Continue on another account still works). Not offered for the External account.',
      good: 'Accounts don’t pile up.', cost: 'Removing is final; the confirm says so.',
      mock: () => menuMock('G') },
  ],
});
