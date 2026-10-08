// Usage in its three views: Settings › Usage, the composer's gauge and popover, and an agent's
// Account tab, all built from one shared window row. The helpers are copied from
// ../accounts/topics-accounts.js and topics-limits.js, since decisions.js loads only a round's own
// scripts.

Object.assign(ICONS, {
  updown: '<path d="m7 15 5 5 5-5"/><path d="m7 9 5-5 5 5"/>',
  sparkle: '<path d="M9.937 15.5A2 2 0 0 0 8.5 14.063l-6.135-1.582a.5.5 0 0 1 0-.962L8.5 9.936A2 2 0 0 0 9.937 8.5l1.582-6.135a.5.5 0 0 1 .963 0L14.063 8.5A2 2 0 0 0 15.5 9.937l6.135 1.581a.5.5 0 0 1 0 .964L15.5 14.063a2 2 0 0 0-1.437 1.437l-1.582 6.135a.5.5 0 0 1-.963 0z"/>',
  'arrow-left': '<path d="m12 19-7-7 7-7"/><path d="M19 12H5"/>',
  rotate: '<path d="M21 12a9 9 0 1 1-9-9c2.52 0 4.93 1 6.74 2.74L21 8"/><path d="M21 3v5h-5"/>',
  gauge: '<path d="m12 14 4-4"/><path d="M3.34 19a10 10 0 1 1 17.32 0"/>',
  book: '<path d="M12 7v14"/><path d="M3 18a1 1 0 0 1-1-1V4a1 1 0 0 1 1-1h5a4 4 0 0 1 4 4 4 4 0 0 1 4-4h5a1 1 0 0 1 1 1v13a1 1 0 0 1-1 1h-6a3 3 0 0 0-3 3 3 3 0 0 0-3-3z"/>',
  plug: '<path d="M12 22v-5"/><path d="M9 8V2"/><path d="M15 8V2"/><path d="M18 8v5a4 4 0 0 1-4 4h-4a4 4 0 0 1-4-4V8Z"/>',
  coins: '<circle cx="8" cy="8" r="6"/><path d="M18.09 10.37A6 6 0 1 1 10.34 18"/><path d="M7 6h1v4"/><path d="m16.71 13.88.7.71-2.82 2.82"/>',
  key: '<path d="m15.5 7.5 2.3 2.3a1 1 0 0 0 1.4 0l2.1-2.1a1 1 0 0 0 0-1.4L19 4"/><path d="m21 2-9.6 9.6"/><circle cx="7.5" cy="15.5" r="5.5"/>',
  send: '<path d="M14.54 21.69a.5.5 0 0 0 .94-.03l6.5-19a.5.5 0 0 0-.64-.64l-19 6.5a.5.5 0 0 0-.03.94l7.93 3.18a2 2 0 0 1 1.11 1.11z"/><path d="m21.85 2.15-10.94 10.94"/>',
  flame: '<path d="M8.5 14.5A2.5 2.5 0 0 0 11 12c0-1.38-.5-2-1-3-1.072-2.143-.224-4.054 2-6 .5 2.5 2 4.9 4 6.5 2 1.6 3 3.5 3 5.5a7 7 0 1 1-14 0c0-1.153.433-2.294 1-3a2.5 2.5 0 0 0 2.5 2.5z"/>',
});
GLYPHS.droid = '◆';
GLYPHS.devin = '◐';
GLYPHS.antigravity = '▲';

// The demo world. A window is [label, % left, time to its reset, share of the window still to come].
const acct = (email, plan, hue, windows, extra = {}) => ({ email, plan, hue, windows, ...extra });
const ME = 'alex@hey.com';
const CLAUDE = [
  acct(ME, 'Max 5x', 212, [['Session', 74, '2h 10m', 0.43], ['Weekly', 13, '2d 18h', 0.4]], { external: true }),
  acct('work@acme.dev', 'Team', 150, [['Session', 0, '1h 52m', 0.37], ['Weekly', 44, '4d 6h', 0.62]], { label: 'Work' }),
];
const CODEX = [acct(ME, 'Plus', 212, [['5-hour', 83, '3h 05m', 0.62], ['Weekly', 61, '5d 2h', 0.73]], { external: true })];
const DEVIN = [acct(ME, 'Pro', 212, [['Daily', 100, '', null], ['Weekly', 100, '', null]], { external: true })];
const full3 = [['5-hour', 100, '', null], ['Weekly', 100, '', null], ['Monthly', 100, '', null]];
const DROID = [
  acct(ME, 'Pro', 212, [['5-hour', 97, '4h 18m', 0.86], ['Weekly', 3, '23h 57m', 0.14], ['Monthly', 51, '23d 15h', 0.79]], { external: true, default: true }),
  acct('work@acme.dev', 'Pro', 150, [['5-hour', 68, '2h 40m', 0.53], ['Weekly', 88, '4d 23h', 0.71], ['Monthly', 94, '27d 23h', 0.93]], { label: 'Work' }),
  acct('design@acme.dev', 'Pro', 330, [['5-hour', 42, '1h 05m', 0.22], ['Weekly', 70, '3d 2h', 0.44], ['Monthly', 81, '19d 4h', 0.63]], { label: 'Design' }),
  acct('side@fastmail.com', 'Pro', 28, full3, { label: 'Side' }),
  acct('ci@acme.dev', 'Pro', 260, full3, { label: 'CI' }),
  acct('backup@hey.com', 'Pro', 90, full3, { label: 'Backup' }),
];
const ANTIGRAVITY = [
  acct('alex@gmail.com', 'Pro', 212, [['5-hour', 100, '', null], ['Weekly', 100, '', null]], { external: true }),
  acct('sam.k@gmail.com', 'Pro', 28, [['5-hour', 35, '3h 20m', 0.67], ['Weekly', 72, '5d 1h', 0.73]], { label: 'Sam' }),
  acct('team@acme.dev', 'Ultra', 150, [['5-hour', 12, '58m', 0.19], ['Weekly', 40, '2d 1h', 0.3]], { label: 'Team' }),
  acct('bot@acme.dev', 'Pro', 260, [['5-hour', 100, '', null], ['Weekly', 100, '', null]], { label: 'Bot' }),
];
const USAGE = [
  { name: 'Claude Agent', kind: 'claude', accounts: CLAUDE },
  { name: 'Codex', kind: 'codex', accounts: CODEX },
  { name: 'Devin', kind: 'devin', accounts: DEVIN },
  { name: 'Factory Droid', kind: 'droid', accounts: DROID },
  { name: 'Google Antigravity', kind: 'antigravity', accounts: ANTIGRAVITY },
];
const POPOVER_ACCOUNT = DROID[1];

const accountName = (a) => a.label || a.email;
const accountDetail = (a) => [a.label ? a.email : null, a.plan].filter(Boolean).join(' · ');
const pooled = (accounts, index) => Math.round(accounts.reduce((sum, a) => sum + a.windows[index][1], 0) / accounts.length);
const leftColor = (left) => (left === 0 ? 'var(--del)' : left <= 15 ? 'var(--warn)' : 'var(--t)');
const leftWords = (left) => (left === 0 ? 'Used up' : `${left}% left`);
const resetsIn = (w) => (w[2] ? `resets in ${w[2]}` : '');

// Pieces in the app's style (settings_page.rs and ui's buttons).
const avatar = (a, size = 28) => `<span style="width:${size}px;height:${size}px;border-radius:50%;flex:none;display:inline-grid;place-items:center;font-size:${Math.round(size * 0.43)}px;font-weight:600;color:#1b1f26;background:linear-gradient(135deg,hsl(${a.hue} 70% 82%),hsl(${a.hue} 42% 60%))">${accountName(a)[0].toUpperCase()}</span>`;
const tag = (text, color = 'mu') => `<span class="xs none" style="padding:1px 6px;border-radius:4px;background:var(--hov);color:var(--${color})">${text}</span>`;
const accountTags = (a) => `${a.external ? tag('Outside agentZ') : ''}${a.default ? tag('Default', 'ac') : ''}`;
const obtn = (text, extra = '') => `<span class="btn" style="${extra}">${text}</span>`;
const gbtn = (text) => `<span class="btn" style="border-color:transparent;background:none">${text}</span>`;
const pbtn = (text) => `<span class="btn primary">${text}</span>`;
const dd = (label) => `<span class="row" style="gap:4px;white-space:nowrap;height:26px;padding:0 8px;border-radius:6px;background:var(--hov)">${label}${ic('updown', 'xs mu')}</span>`;
const chev = ic('chev-down', 'xs');
const link = (text) => `<span class="row g1 sm ac none">${text}${ic('external', 'xs')}</span>`;
const pagePiece = (html) => `<div class="m" style="width:768px;padding:0 24px">${html}</div>`;
const piece = (html, w = 720) => `<div class="m" style="width:${w}px">${html}</div>`;
const card = (rows, style = '') => `<div class="card" style="overflow:visible;position:relative;${style}">${rows.filter(Boolean).map((row, index) => `<div style="position:relative;${index ? 'border-top:1px solid var(--bv)' : ''}">${row}</div>`).join('')}</div>`;
const sectionHead = (title, action = '') => `<div class="row" style="justify-content:space-between"><span class="sm mu">${title}</span>${action}</div>`;
const addAccount = `<span class="row g1 sm mu">${ic('plus', 'xs')}Add Account</span>`;
const spinner = '<span class="spin"></span>';
const caption = (text) => `<div class="xs ph" style="margin:0 0 6px">${text}</div>`;

/** t3code's bar: the fill is what's left, the hairline where even spending would be. */
function bar(left, { mark = null, h = 6, color } = {}) {
  const fill = color || (left === 0 ? 'var(--del)' : left <= 15 ? 'var(--warn)' : 'var(--ac)');
  const track = left === 0 ? 'rgba(208,114,119,.3)' : '#3b414d';
  return `<span style="position:relative;display:block;width:100%;height:${h}px;border-radius:${h}px;background:${track}">${left ? `<i style="position:absolute;left:0;top:0;bottom:0;width:${left}%;border-radius:${h}px;background:${fill}"></i>` : ''}${mark !== null ? `<i style="position:absolute;top:-3px;bottom:-3px;left:${mark * 100}%;width:1px;background:rgba(220,224,229,.55)"></i>` : ''}</span>`;
}

// 1. The shared window row in its four forms -------------------------------------------------
/** A: today's row (t3code LimitWindows): name, % left, the bar, when it resets. */
function rowsA(windows, { labelW = 70, leftW = 70, resetW = 120 } = {}) {
  const rows = windows.map((w) => `<span class="sm mu trunc">${w[0]}</span><span class="sm b5" style="color:${leftColor(w[1])}">${leftWords(w[1])}</span>${bar(w[1], { mark: w[3] })}<span class="sm mu" style="text-align:right;white-space:nowrap">${resetsIn(w)}</span>`).join('');
  return `<div style="display:grid;grid-template-columns:${labelW}px ${leftW}px minmax(0,1fr) ${resetW}px;column-gap:12px;row-gap:9px;align-items:center">${rows}</div>`;
}
/** B: OpenUsage's two lines: the name over a full-width bar, % left and the reset under it. */
function rowsB(windows) {
  return `<div class="col" style="gap:12px">${windows.map((w) => `<div class="col" style="gap:5px"><span class="sm b5">${w[0]}</span>${bar(w[1], { mark: w[3] })}<span class="row xs"><span style="color:${leftColor(w[1])}">${leftWords(w[1])}</span><span class="grow"></span><span class="mu">${w[2] ? `Resets in ${w[2]}` : ''}</span></span></div>`).join('')}</div>`;
}
/** C: a compact cell: % left and the reset over a thin bar. Also the Usage table's cell. */
function wcell(w, { reset = true } = {}) {
  return `<span class="col" style="gap:4px;min-width:0"><span class="row sm" style="gap:6px"><span class="b5" style="color:${leftColor(w[1])}">${w[1] === 0 ? 'Used up' : `${w[1]}%`}</span><span class="grow"></span><span class="xs ph" style="white-space:nowrap">${reset && w[2] ? `↻ ${w[2]}` : ''}</span></span>${bar(w[1], { mark: w[3], h: 4 })}</span>`;
}
function rowsC(windows) {
  return `<div style="display:grid;grid-template-columns:repeat(${windows.length},minmax(0,1fr));gap:16px">${windows.map((w) => `<span class="col" style="gap:4px;min-width:0"><span class="xs mu">${w[0]}</span>${wcell(w)}</span>`).join('')}</div>`;
}
/** D: A, with OpenUsage's pace colors: the fill says whether it lasts until the reset. */
function paceOf(w) {
  if (w[1] === 0) return 'out';
  if (w[3] === null) return 'ok';
  const projected = w[1] - (w[3] * (100 - w[1])) / Math.max(0.01, 1 - w[3]);
  return projected <= 0 ? 'out' : projected < 10 ? 'tight' : 'ok';
}
function rowsD(windows) {
  const rows = windows.map((w) => {
    const pace = paceOf(w);
    const color = pace === 'out' ? 'var(--del)' : pace === 'tight' ? 'var(--warn)' : 'var(--ac)';
    const extra = w[1] === 0 ? '' : pace === 'out' ? `<span class="xs delc row g1">${ic('flame', 'xs')}runs out in ${{ Weekly: '9h', Monthly: '11d' }[w[0]] || '2h'}</span>` : pace === 'tight' ? '<span class="xs warnc">~4% spare</span>' : '';
    return `<span class="sm mu trunc">${w[0]}</span><span class="sm b5" style="color:${w[1] === 0 ? 'var(--del)' : 'var(--t)'}">${leftWords(w[1])}</span><span class="col" style="gap:2px">${bar(w[1], { mark: w[3], color })}</span><span class="col sm mu" style="text-align:right;white-space:nowrap;align-items:flex-end">${resetsIn(w)}${extra}</span>`;
  }).join('');
  return `<div style="display:grid;grid-template-columns:70px 70px minmax(0,1fr) 120px;column-gap:12px;row-gap:9px;align-items:center">${rows}</div>`;
}

/** The top of an account's card or popover: avatar, name and tags, the email and plan, controls. */
function accountHead(a, { right = ibtn('more'), size = 28, pad = '12px 16px', detail } = {}) {
  return `<div class="row g3" style="padding:${pad};position:relative">${avatar(a, size)}<div class="col grow" style="gap:2px;min-width:0"><span class="row g2"><span class="trunc">${accountName(a)}</span>${accountTags(a)}</span><span class="sm mu trunc">${detail ?? accountDetail(a)}</span></div><span class="row g2 none">${right}</span></div>`;
}

// Settings: the window and the content column (settings_page.rs render_nav).
function settingsWindow(contentHtml, { selected = 'Usage', w = 1100, h = 760, overlay = '' } = {}) {
  const nav = [['General', 'settings'], ['Appearance', 'eye'], ['Notifications', 'bell'], ['Agents', 'sparkle'], ['Usage', 'gauge'], ['Skills', 'book'], ['MCP Servers', 'plug'], ['Machines', 'server']]
    .map(([label, icon]) => `<div class="row g2" style="height:28px;padding:0 8px;border-radius:6px;margin:0 4px;${label === selected ? 'background:var(--sel)' : ''}">${ic(icon, 'sm mu')}<span>${label}</span></div>`).join('');
  return `<div class="m win" style="width:${w}px;height:${h}px">
    <div class="titlebar">${lights()}${ic('sidebar', 'sm mu')}<span class="row g15" style="font-size:13px">${ic('list', 'xs mu')}All projects${ic('chev-down', 'xs mu')}</span><div class="viewtabs"><span class="on">Agents</span><span>Workspaces</span></div></div>
    <div class="body"><div class="sidebar"><div class="sb-head" style="color:var(--t)"><span class="grow b5" style="color:var(--t)">Settings</span>${ic('x', 'sm mu')}</div><div class="sb-list">${nav}<div class="sm mu" style="padding:14px 12px 4px">Projects</div><div class="row g2" style="height:28px;padding:0 12px">${mono('ST', 'g')}<span>storefront</span></div><div class="row g2" style="height:28px;padding:0 12px">${mono('AP', 't')}<span>api</span></div></div><div class="sb-foot">${ic('arrow-left', 'sm')}<span>Back</span></div></div>
    <div class="grow" style="height:100%;overflow:hidden;background:var(--ed);display:flex;justify-content:center"><div style="width:720px">${contentHtml}</div></div></div>${overlay}</div>`;
}
const machinePicker = `<span class="row g15 sm" style="height:24px;padding:0 8px;border:1px solid var(--b);border-radius:6px">${ic('laptop', 'xs mu')}This Mac${ic('updown', 'xs mu')}</span>`;
const usagePage = (body, { right = machinePicker } = {}) => `<div class="col" style="padding:24px 0 40px;gap:22px"><div class="row" style="justify-content:space-between;height:28px"><span style="font-size:17px">Usage</span>${right}</div>${body}</div>`;
const agentTitle = (agent, extra = '') => `<div class="row g2">${glyph(agent.kind)}<span class="b5">${agent.name}</span>${extra}</div>`;

/** The agent's page: breadcrumb, heading with the status badge, and its tabs (render_agent_page). */
function agentPage(body, { agent = { name: 'Factory Droid', kind: 'droid', sub: 'v0.235.0 · Factory’s coding agent' } } = {}) {
  const tabRow = ['Account', 'Defaults', 'Environment', 'Threads'].map((name, index) => `<span style="padding:0 0 9px;${index === 0 ? 'color:var(--t);border-bottom:2px solid var(--ac);margin-bottom:-1px' : 'color:var(--mu)'}">${name}</span>`).join('');
  return `<div class="col" style="padding:24px 0 28px;gap:20px">
    <div class="col" style="gap:22px">
      <span class="row sm mu" style="justify-content:space-between"><span class="row g15">${ic('arrow-left', 'xs')}Agents</span><span class="row g15">${ic('laptop', 'xs')}This Mac</span></span>
      <div class="row g3"><span style="width:46px;height:46px;border-radius:10px;border:1px solid var(--b);background:var(--panel);display:grid;place-items:center;font-size:20px;flex:none">${GLYPHS[agent.kind]}</span>
        <div class="col grow" style="gap:4px"><span class="row g2"><span style="font-size:17px;font-weight:600">${agent.name}</span><span class="row xs" style="gap:5px;padding:1px 7px;border-radius:4px;background:rgba(161,193,129,.14);color:var(--ok)"><i class="dot" style="background:var(--ok)"></i>Logged in</span></span><span class="sm mu">${agent.sub}</span></div>
        <span class="ibtn" style="border:1px solid var(--b);width:28px;height:28px">${ic('more', 'sm')}</span></div>
      <div class="row" style="gap:22px;border-bottom:1px solid var(--b);font-size:14px">${tabRow}</div>
    </div>${body}</div>`;
}
const CLAUDE_AGENT = { name: 'Claude Agent', kind: 'claude', sub: 'v0.87.0 · ACP wrapper for Anthropic’s Claude' };

// The thread view: the end of a conversation and the composer (agent_view.rs).
const conversation = () => `<div style="padding:16px 60px 0;display:flex;flex-direction:column;gap:12px"><div style="align-self:flex-end;background:var(--hov);padding:8px 12px;border-radius:8px;max-width:420px">Add a checkout page with a pay button. Use the cart from src/lib/cart.ts.</div><div style="line-height:22px">I added the cart summary and the <code style="font-family:'IBM Plex Mono';font-size:12px">POST /api/orders</code> route. Next is the pay button, then a test for the order total.</div><div class="row g1 sm ph">${ic('chev-right', 'xs')}Ran 3 commands</div></div>`;
const gaugeChip = (text, color = 'var(--mu)', on = false) => `<span class="row g1 sm" style="height:20px;padding:0 6px;border-radius:5px;background:${on ? 'var(--sel)' : 'var(--hov)'};color:${color}">${ic('gauge', 'xs')}${text}</span>`;
const composer = ({ gauge = gaugeChip('68%', 'var(--mu)', true) } = {}) => `<div style="border-top:1px solid var(--b);padding:8px 0 10px;background:var(--ed)"><div style="margin:0 auto;width:680px"><div class="ph" style="padding:6px 0 12px">Message the agent…</div><div class="row" style="gap:12px;font-size:13px;color:var(--mu)">${ic('plus', 'sm')}<span class="row g15">${glyph('droid')}Factory Droid</span>${gauge}<span class="grow"></span><span class="row g1">Auto (High) ${chev}</span><span class="row g1">Opus 5.5 ${chev}</span><span class="row g1">Extra High ${chev}</span><span class="ac" style="display:inline-flex">${ic('send', 'sm')}</span></div></div></div>`;
const threadView = (popover, { w = 800, h = 470, gauge } = {}) => frame(`<div class="col" style="height:100%"><div class="grow" style="min-height:0;overflow:hidden">${conversation()}</div>${composer({ gauge })}</div><div style="position:absolute;left:60px;bottom:84px;z-index:20">${popover}</div>`, { w, h });
const popover = (html, w = 420) => `<div class="pop" style="position:relative;width:${w}px;padding:12px 14px">${html}</div>`;
const readLine = (text = 'Read 2 min ago') => `<div class="row g2 xs ph" style="margin-top:10px">${text}<span class="grow"></span><span class="row g1 mu">${ic('rotate', 'xs')}Refresh</span></div>`;

// 1. How one window reads ---------------------------------------------------------------------
const SAMPLE = DROID[0];
const sampleCard = (rows) => card([`${accountHead(SAMPLE)}<div style="padding:0 16px 14px 56px">${rows(SAMPLE.windows)}</div>`]);
const samplePopover = (rows) => popover(`<div class="row g2" style="margin-bottom:10px">${avatar(SAMPLE, 18)}<span>${accountName(SAMPLE)}</span><span class="sm mu">${SAMPLE.plan}</span><span class="grow"></span><span class="row g1 sm mu">Usage${ic('external', 'xs')}</span></div>${rows(SAMPLE.windows)}`);
const sampleTableRow = `<div class="card" style="display:grid;grid-template-columns:minmax(0,1fr) repeat(3,130px);gap:18px;padding:10px 16px;align-items:center"><span class="row g2" style="min-width:0">${avatar(SAMPLE, 22)}<span class="trunc">${accountName(SAMPLE)}</span></span>${SAMPLE.windows.map((w) => wcell(w)).join('')}</div>`;
const threeViews = (rows, table = sampleTableRow) => piece(`<div class="col" style="padding:16px;gap:14px">${caption('On the Account tab')}${sampleCard(rows)}<div class="row g3" style="align-items:flex-end"><div class="col grow">${caption('On the Usage page')}${table}</div></div>${caption('In the thread’s popover')}${samplePopover(rows)}</div>`, 720);

TOPICS.push({
  id: 'row', section: 'Shared', title: 'How one window reads, in all three views', size: 'wide', rec: 'A',
  now: 'Two different forms. The Account tab and the thread’s popover use t3code’s row: the window’s name and “26% left”, a bar of what’s left with a hairline where even spending would be, and “resets in 4h 17m” at the right (Accounts round §2: yellow at 15% or less, red when used up). The Usage page instead draws one card per window, with a big “26% left” and the bar split into one chip per account, each with its avatar, email, % and “↻ 4h 19m”. With many accounts the chips truncate and the reset times clip.',
  nowImg: '../feedback/evidence/private/07-agent-account-single.png',
  issues: ['The Usage page and the other two views read differently', 'Chips on the Usage page truncate names and clip reset times', 'Under a minute before a reset the row says “resets in 0m”'],
  options: [
    { key: 'A', name: 'Today’s row everywhere, a cell in tables', from: 't3code LimitWindows',
      desc: 'The Account tab and the popover keep today’s one-line row. Where accounts sit in a table (the Usage page, a compact account list), each window is a small cell made of the same parts: % left with the reset (“↻ 2h 40m”) over a thin bar with its hairline. The % turns yellow at 15% or less and says “Used up” in red at 0. Under a minute it says “resets in under 1m”; past the reset and before the next read it says “resetting”, not an old percentage.',
      good: 'One vocabulary, two sizes; the Account tab barely changes.', cost: 'Two forms to keep in step.',
      mock: () => threeViews(rowsA) },
    { key: 'B', name: 'Two lines, as OpenUsage', from: 'OpenUsage dashboard rows',
      desc: 'The window’s name over a full-width bar; under it “68% left” at the left and “Resets in 2h 40m” at the right. The same two lines in every view.',
      good: 'The bar gets the whole width; nothing is squeezed into columns.', cost: 'Twice as tall: three windows take about 130 px per account, so many accounts mean a long page.',
      mock: () => threeViews(rowsB, `<div class="card" style="padding:12px 16px">${rowsB(SAMPLE.windows.slice(0, 1))}</div>`) },
    { key: 'C', name: 'Compact cells everywhere', from: 'new',
      desc: 'Every view shows each window as the cell from A, side by side: name, % with the reset, and a thin bar. No full-width rows.',
      good: 'The most compact; three views look the same.', cost: 'Bars are short, so the hairline is hard to read; “left” is implied, not written.',
      mock: () => threeViews(rowsC) },
    { key: 'D', name: 'A, colored by pace', from: 'OpenUsage pace colors',
      desc: 'A’s row, but the fill’s color says whether the window will last until its reset at the current rate: blue on course, yellow when it will be close (“~4% spare”), red when it will run out first (“runs out in 9h”). Used up is still red.',
      good: 'Warns before you hit the limit, not after.', cost: 'Changes the color rule picked in the Accounts round; a guess from the hairline, which can be wrong early in a window.',
      mock: () => threeViews(rowsD) },
  ],
});

// 2. Settings › Usage ---------------------------------------------------------------------------
const tableCols = (n) => `minmax(0,1fr) repeat(${n},128px) 16px`;
function accountCell(a, { size = 22 } = {}) {
  return `<span class="row g2" style="min-width:0">${avatar(a, size)}<span class="col" style="min-width:0;gap:1px"><span class="row g2" style="min-width:0"><span class="trunc">${accountName(a)}</span>${a.external ? tag('Outside') : ''}</span><span class="xs mu trunc">${a.plan}${a.label ? ` · ${a.email}` : ''}</span></span></span>`;
}
function usageTable(agent, { pooledRow = true } = {}) {
  const n = agent.accounts[0].windows.length;
  const head = `<div class="xs mu" style="display:grid;grid-template-columns:${tableCols(n)};gap:18px;padding:8px 16px">${['Account', ...agent.accounts[0].windows.map((w) => w[0]), ''].map((t) => `<span>${t}</span>`).join('')}</div>`;
  const all = pooledRow && agent.accounts.length > 1 ? `<div style="display:grid;grid-template-columns:${tableCols(n)};gap:18px;padding:9px 16px;align-items:center;background:rgba(255,255,255,.02)"><span class="sm b5">All ${agent.accounts.length} accounts</span>${agent.accounts[0].windows.map((_, i) => wcell(['', pooled(agent.accounts, i), '', null], { reset: false })).join('')}<span></span></div>` : '';
  const rows = agent.accounts.map((a) => `<div style="display:grid;grid-template-columns:${tableCols(n)};gap:18px;padding:8px 16px;align-items:center">${accountCell(a)}${a.windows.map((w) => wcell(w)).join('')}${ic('chev-right', 'xs mu')}</div>`);
  return `<div class="col" style="gap:10px">${agentTitle(agent, agent.accounts.length > 1 ? `<span class="sm ph">${agent.accounts.length} accounts</span>` : '')}${card([head, all || null, ...rows])}</div>`;
}
const pageA = () => usagePage(USAGE.map((agent) => usageTable(agent)).join(''));

function accountTile(a) {
  return `<div class="card" style="padding:12px 14px">${`<div class="row g2" style="margin-bottom:12px">${avatar(a, 22)}<span class="col grow" style="min-width:0"><span class="trunc">${accountName(a)}</span><span class="xs mu trunc">${a.plan}</span></span></div>`}${rowsB(a.windows)}</div>`;
}
const pageB = () => usagePage(USAGE.map((agent) => `<div class="col" style="gap:10px">${agentTitle(agent)}<div style="display:grid;grid-template-columns:1fr 1fr;gap:10px">${agent.accounts.map(accountTile).join('')}</div></div>`).join(''));

function segmentBar(accounts, index) {
  return `<span style="display:grid;grid-template-columns:repeat(${accounts.length},minmax(0,1fr));gap:3px">${accounts.map((a, n) => { const w = a.windows[index]; return `<span style="position:relative;height:22px;border-radius:4px;background:#3b414d;overflow:hidden">${w[1] ? `<i style="position:absolute;left:0;top:0;bottom:0;width:${w[1]}%;background:${w[1] <= 15 ? 'rgba(222,193,132,.45)' : 'rgba(116,173,232,.4)'}"></i>` : '<i style="position:absolute;inset:0;background:rgba(208,114,119,.3)"></i>'}<span class="xs b5" style="position:relative;display:flex;align-items:center;gap:4px;height:100%;padding:0 6px;color:${w[1] === 0 ? 'var(--del)' : 'var(--t)'}"><span class="ph">${n + 1}</span>${w[1]}%</span></span>`; }).join('')}</span>`;
}
function pooledCard(agent) {
  const accounts = agent.accounts;
  const many = accounts.length > 1;
  const rows = accounts[0].windows.map((w0, i) => {
    const soonest = accounts.map((a) => a.windows[i]).filter((w) => w[2] && w[1] < 100).map((w) => w[2])[0];
    return `<div style="display:grid;grid-template-columns:80px 76px minmax(0,1fr) 92px;gap:12px;align-items:center;padding:8px 16px"><span class="sm mu">${w0[0]}</span><span class="b5">${pooled(accounts, i)}% <span class="xs mu" style="font-weight:400">left</span></span>${many ? segmentBar(accounts, i) : bar(accounts[0].windows[i][1], { mark: accounts[0].windows[i][3] })}<span class="xs mu" style="text-align:right">${soonest ? `↻ ${soonest}` : ''}</span></div>`;
  }).join('');
  const legend = many ? `<div class="row xs" style="flex-wrap:wrap;gap:6px 14px;padding:10px 16px">${accounts.map((a, n) => `<span class="row g1"><span class="ph">${n + 1}</span>${avatar(a, 14)}<span class="mu">${accountName(a)}</span></span>`).join('')}</div>` : `<div class="row g2 xs mu" style="padding:8px 16px">${avatar(accounts[0], 14)}${accountName(accounts[0])} · ${accounts[0].plan}</div>`;
  return `<div class="col" style="gap:10px">${agentTitle(agent)}${card([`<div style="padding:4px 0">${rows}</div>`, legend])}</div>`;
}
const pageC = () => usagePage(USAGE.map(pooledCard).join(''));

const labeled = (w, html) => `<span class="col" style="gap:2px;min-width:0"><span class="xs ph">${w[0]}</span>${html}</span>`;
function agentSummary(agent, open) {
  const n = 3;
  const pad = (cells) => cells.concat(Array(n - cells.length).fill('<span></span>')).join('');
  const many = agent.accounts.length > 1;
  const head = `<div style="display:grid;grid-template-columns:${tableCols(n)};gap:18px;padding:10px 16px;align-items:center"><span class="row g2" style="min-width:0">${glyph(agent.kind)}<span class="b5">${agent.name}</span><span class="sm ph">${many ? `${agent.accounts.length} accounts` : agent.accounts[0].plan}</span></span>${pad(agent.accounts[0].windows.map((w, i) => labeled(w, many ? wcell([w[0], pooled(agent.accounts, i), '', null], { reset: false }) : wcell(w))))}${many ? ic(open ? 'chev-down' : 'chev-right', 'xs mu') : '<span></span>'}</div>`;
  const rows = open ? agent.accounts.map((a) => `<div style="display:grid;grid-template-columns:${tableCols(n)};gap:18px;padding:7px 16px 7px 40px;align-items:center">${accountCell(a, { size: 18 })}${pad(a.windows.map((w) => wcell(w)))}<span></span></div>`).join('') : '';
  return `<div>${head}${rows ? `<div style="padding-bottom:6px">${rows}</div>` : ''}</div>`;
}
const pageD = () => usagePage(`<div class="col" style="gap:8px">${sectionHead('Agents', '<span class="xs ph">Each agent shows what’s left across its accounts</span>')}${card(USAGE.map((agent) => agentSummary(agent, agent.kind === 'droid')))}</div>`);

TOPICS.push({
  id: 'page', section: 'Settings › Usage', title: 'The Usage page with many agents and accounts', size: 'wide', rec: 'A',
  now: 'Picked in the Accounts round (§16 A, from t3code’s Usage page): every agent with accounts, and each of its windows as its own card: a big “N% left” (averaged “across N accounts”) and a bar split into one chip per account with its % and reset. Clicking a chip opens the account on its agent’s page. With five agents the page scrolls; with Droid’s 6 accounts and Antigravity’s 4 the chips truncate (“alkimy…”) and the reset times shrink to clipped “↻” icons. Scrolled: <a href="../feedback/evidence/private/05-usage-screen-scrolled.png">05-usage-screen-scrolled.png</a>; with the mock agent: <a href="img/now-usage-page.png">now-usage-page.png</a>.',
  nowImg: '../feedback/evidence/private/05-usage-screen-top.png',
  issues: ['One card per window makes the page long', 'One account’s chip stretches over the whole bar', 'Names truncate and reset times clip with several accounts', 'Some fills are hard to tell from the empty bar'],
  options: [
    { key: 'A', name: 'A table per agent', from: 'new, with t3code’s pooled number',
      desc: 'Each agent is one card: a row per account, a column per window. Each cell is topic 1’s cell: % left and the reset over a thin bar. Agents with several accounts start with an “All N accounts” row, the average t3code shows today. Clicking a row opens the account on the agent’s page. The External account is first, tagged “Outside”.',
      good: 'A whole agent fits in a few lines; accounts compare down a column; nothing truncates.', cost: 'Agents with different windows get different columns, so the tables don’t line up across agents.',
      mock: () => settingsWindow(pageA(), { h: 1000 }) },
    { key: 'B', name: 'A card per account', from: 'OpenUsage provider cards',
      desc: 'Each agent heads a grid of account cards, two across. A card has the account’s avatar, name and plan, then its windows in OpenUsage’s two lines.',
      good: 'Every account gets room; reads like OpenUsage’s popover.', cost: 'The longest option: Droid alone takes three rows of cards.',
      mock: () => settingsWindow(pageB(), { h: 1000 }) },
    { key: 'C', name: 'One card per agent, pooled bars', from: 't3code UsageLimitsPooled',
      desc: 'Today’s pooled view, made compact: one card per agent with a row per window. Each row has the average left, a bar with one equal segment per account showing its number and %, and the soonest reset. A legend under the rows names the accounts by number, as t3code does on narrow screens.',
      good: 'Closest to today; you see at once how much is left across all accounts.', cost: 'You match numbers to names to know which account is low; only the soonest reset shows.',
      mock: () => settingsWindow(pageC(), { h: 1000 }) },
    { key: 'D', name: 'A line per agent that opens', from: 'new',
      desc: 'One list: a line per agent with its windows averaged across its accounts. Clicking an agent with several accounts opens its accounts under it, each with its own cells, as in A.',
      good: 'Every agent fits on one screen; details when you want them.', cost: 'A click to see which account is low.',
      mock: () => settingsWindow(pageD(), { h: 1000 }) },
  ],
});

// 3. The thread's popover -----------------------------------------------------------------------
const popoverHead = (a, right = `<span class="row g1 sm mu">Usage${ic('external', 'xs')}</span>`) => `<div class="row g2" style="margin-bottom:10px">${avatar(a, 18)}<span class="trunc">${accountName(a)}</span><span class="sm mu">${a.plan}</span><span class="grow"></span>${right}</div>`;
const popoverA = (a = POPOVER_ACCOUNT, extra = '') => popover(`${popoverHead(a)}${rowsA(a.windows, { labelW: 62, leftW: 64, resetW: 108 })}${extra}${readLine()}`);
const otherRow = (a) => { const low = a.windows.reduce((x, w) => (w[1] < x[1] ? w : x)); return `<div class="row g2 sm" style="height:26px">${avatar(a, 16)}<span class="grow trunc">${accountName(a)}</span><span class="xs mu">${low[0]}</span><span style="width:60px">${bar(low[1], { h: 4 })}</span><span class="b5" style="width:56px;text-align:right;color:${leftColor(low[1])}">${leftWords(low[1])}</span></div>`; };

TOPICS.push({
  id: 'popover', section: 'Threads', title: 'The usage gauge and its popover', size: 'wide', rec: 'A',
  now: 'Picked in the Accounts round (§16 B, from t3code’s ComposerUsageLimits): beside the agent in the composer, a gauge with the thread’s account’s tightest window (“68%”). Hovering or clicking it opens a 420 px popover: the account’s avatar and email, its plan, “Usage ↗”, then a row per window. In the screenshot the 5-hour row says “resets in 0m” with 68% left, and its hairline sits at the left end. Why: the popover counts down from the last read. With under a minute to go it rounds down to “0m” (format_duration only shows minutes), and once the time passes it says “resets now” until the next read, which comes up to 5 minutes later (usage_reads.rs reads every 5 minutes). Until then it still shows the old 68%. The hairline is right: with no time left in the window, even spending would be at 0. The Account tab in the recording shows the same account a little later, “68% left · resets now”.',
  nowImg: '../feedback/evidence/private/06-thread-usage-popover.jpeg',
  issues: ['“resets in 0m” and then “resets now” with an old percentage for up to 5 minutes', 'Nothing says how old the numbers are', 'The gauge’s % doesn’t say which window it is'],
  options: [
    { key: 'A', name: 'Today’s popover, tidied', from: 't3code ComposerUsageLimits',
      desc: 'Same layout, with topic 1’s row. The header has the account’s name and plan and “Usage ↗”. A footer says when it was read, with Refresh. Under a minute it says “resets in under 1m”; past the reset it says “resetting” and agentZ reads the account again at the reset time, so an old % never stays. The gauge’s tooltip names the window (“Weekly: 3% left”).',
      good: 'Small change; fixes the stale reading.', cost: 'Still only the thread’s own account.',
      mock: () => threadView(popoverA()) },
    { key: 'B', name: 'With the agent’s other accounts', from: 't3code’s banner, which lists every account',
      desc: 'A’s popover, then “Other accounts” with one line each: the account and its tightest window. It answers whether you can go on elsewhere; continuing on another account stays in the thread’s menu (Accounts round §15).',
      good: 'You see where there’s room before you run out.', cost: 'Taller; with 6 accounts it is a second list to read.',
      mock: () => threadView(popoverA(POPOVER_ACCOUNT, `<div class="xs mu" style="margin:14px 0 4px;padding-top:10px;border-top:1px solid var(--bv)">Other accounts</div>${DROID.filter((a) => a !== POPOVER_ACCOUNT).slice(0, 4).map(otherRow).join('')}<div class="xs ph" style="padding-top:2px">and 1 more</div>`), { h: 600 }) },
    { key: 'C', name: 'Two lines per window', from: 'OpenUsage dashboard',
      desc: 'Topic 1’s option B in the popover: each window as its name over a full-width bar, % left and the reset under it.',
      good: 'Matches OpenUsage; the bars are wide.', cost: 'Taller than the composer’s other menus.',
      mock: () => threadView(popover(`${popoverHead(POPOVER_ACCOUNT)}${rowsB(POPOVER_ACCOUNT.windows)}${readLine()}`, 340), { h: 520 }) },
    { key: 'D', name: 'The window named in the gauge', from: 'new',
      desc: 'A’s popover, and the gauge says which window it shows: “5h 68%”, “Week 3%”. It turns yellow and red as the bar does.',
      good: 'You know what the number means without hovering.', cost: 'A wider chip in a crowded composer row.',
      mock: () => threadView(popoverA(), { gauge: gaugeChip('5h 68%', 'var(--mu)', true) }) },
  ],
});

// 4. The Account tab with one account -----------------------------------------------------------
const SOLO = acct(ME, 'Pro', 212, [['Session', 26, '4h 17m', 0.86], ['Weekly', 13, '2d 18h', 0.4]], { external: true });
const settingRow = (title, desc, control, pad = '10px 16px 10px 56px') => `<div class="row g3" style="padding:${pad}"><div class="col grow" style="gap:2px"><span>${title}</span>${desc ? `<span class="sm mu">${desc}</span>` : ''}</div><span class="none">${control}</span></div>`;
const creditsLine = (pad = '0 16px 0 56px') => `<div class="row g2 sm" style="padding:${pad}"><span class="mu" style="display:inline-flex">${ic('coins', 'sm')}</span><span class="mu">Usage credits</span><span>Off</span><span class="grow"></span>${link('Manage')}</div>`;
const readMeta = '<span class="xs ph">read 2 min ago</span>';

TOPICS.push({
  id: 'single', section: 'Account tab', title: 'The Account tab with one account', size: 'wide', rec: 'A',
  now: 'Picked in the Accounts round (§1 A, §2 A, §10 A, §11 B): one card per account. The head has the avatar, the email, “Outside agentZ” and “Default” tags, the plan and a ⋯ menu. Under it, a row per window, the usage credits line with Manage ↗, and “When a limit is reached / What threads on this account do” with a Stop menu. You said it looks fine but can be improved.',
  nowImg: '../feedback/evidence/private/07-agent-account-single.png',
  issues: ['The setting row is as large as the account’s name, so it reads as a second heading', 'Nothing says how old the limits are'],
  options: [
    { key: 'A', name: 'Tidied card', from: 'today’s card',
      desc: 'The same card, quieter. The limits and the usage credits line stay. Under a hairline, “When a limit is reached” becomes a normal setting row in the body’s size, with its menu at the right. The head says when the limits were read; Refresh Usage stays in the ⋯ menu.',
      good: 'The same layout people know, easier to scan.', cost: 'Small change.',
      mock: () => pagePiece(agentPage(`<div class="col" style="gap:10px">${sectionHead('Accounts', addAccount)}${card([`${accountHead(SOLO, { right: `${readMeta}${ibtn('more')}` })}<div style="padding:0 16px 12px 56px">${rowsA(SOLO.windows)}</div><div style="padding-bottom:12px">${creditsLine()}</div>`, `<div class="row g3 sm" style="padding:10px 16px 10px 56px"><span class="grow">When a limit is reached</span>${dd('Stop')}</div>`])}</div>`, { agent: CLAUDE_AGENT })) },
    { key: 'B', name: 'Limits and settings apart', from: 'Zed settings rows',
      desc: 'The card shows only the account and its limits. Under it, a “When a limit is reached” group in Settings’ usual row style, with the account’s settings: what threads do, and the usage credits with Manage ↗.',
      good: 'The card is only about usage; settings look like settings.', cost: 'With several accounts, which group belongs to which card is less clear.',
      mock: () => pagePiece(agentPage(`<div class="col" style="gap:10px">${sectionHead('Accounts', addAccount)}${card([`${accountHead(SOLO, { right: `${readMeta}${ibtn('more')}` })}<div style="padding:0 16px 14px 56px">${rowsA(SOLO.windows)}</div>`])}<div style="height:8px"></div>${sectionHead('When a limit is reached')}${card([settingRow('What threads on this account do', '', dd('Stop'), '10px 16px'), settingRow('Usage credits', 'Off. Billed by Anthropic once your limits run out.', link('Manage'), '10px 16px')])}</div>`, { agent: CLAUDE_AGENT })) },
    { key: 'C', name: 'Limits beside the settings', from: 'new',
      desc: 'Two columns inside the card: the windows on the left, the account’s settings on the right (what threads do, usage credits).',
      good: 'A shorter card.', cost: 'The bars get half the width; the right column wraps on a narrow window.',
      mock: () => pagePiece(agentPage(`<div class="col" style="gap:10px">${sectionHead('Accounts', addAccount)}${card([`${accountHead(SOLO, { right: `${readMeta}${ibtn('more')}` })}<div style="display:grid;grid-template-columns:minmax(0,1fr) 210px;gap:24px;padding:0 16px 14px 56px"><div>${rowsA(SOLO.windows, { labelW: 56, leftW: 62, resetW: 92 })}</div><div class="col sm" style="gap:10px;border-left:1px solid var(--bv);padding-left:16px"><span class="mu">When a limit is reached</span><span class="row">${dd('Stop')}</span><span class="row g2"><span class="mu">Usage credits</span><span>Off</span><span class="grow"></span>${link('Manage')}</span></div></div>`])}</div>`, { agent: CLAUDE_AGENT })) },
    { key: 'D', name: 'Keep today’s', from: 'today',
      desc: 'No change for one account; only the several-accounts list changes.', good: 'Nothing to build.', cost: 'The two-line setting row stays as large as the name.',
      mock: () => pagePiece(agentPage(`<div class="col" style="gap:10px">${sectionHead('Accounts', addAccount)}${card([`${accountHead(SOLO)}<div style="padding:0 16px 12px 56px">${rowsA(SOLO.windows)}</div><div style="padding-bottom:4px">${creditsLine()}</div>${settingRow('When a limit is reached', 'What threads on this account do.', dd('Stop'), '10px 16px 14px 56px')}`])}</div>`, { agent: CLAUDE_AGENT })) },
  ],
});

// 5. The Account tab with several accounts --------------------------------------------------------
const poolTabs = '<div class="seg" style="margin-bottom:10px"><span class="on">Standard</span><span>Droid Core</span><span>Extra usage</span></div>';
const droidSettings = (left = 56) => `<div class="row g3 sm" style="padding:12px 16px 8px ${left}px"><span class="grow">When a limit is reached</span>${dd('Switch to Droid Core')}</div><div class="row g3 sm" style="padding:0 16px 12px ${left}px"><span class="grow">When Droid stops at a limit</span>${dd('Stop')}</div>`;
const compactCols = 'minmax(0,1fr) repeat(3,104px) 16px';
function compactRow(a, { open = false, selected = false } = {}) {
  const head = `<div style="display:grid;grid-template-columns:${compactCols};gap:16px;padding:10px 16px;align-items:center;${selected ? 'background:var(--hov)' : ''}"><span class="row g3" style="min-width:0">${avatar(a, 26)}<span class="col" style="min-width:0;gap:1px"><span class="row g2" style="min-width:0"><span class="trunc">${accountName(a)}</span>${accountTags(a)}</span><span class="xs mu trunc">${accountDetail(a)}</span></span></span>${a.windows.map((w) => wcell(w)).join('')}${ic(open ? 'chev-down' : 'chev-right', 'xs mu')}</div>`;
  if (!open) return head;
  return `${head}<div style="padding:2px 16px 0 58px"><div class="row" style="justify-content:space-between">${poolTabs}<span class="row g2">${readMeta}${ibtn('more')}</span></div>${rowsA(a.windows)}</div>${droidSettings(58)}`;
}
const compactHead = `<div class="xs mu" style="display:grid;grid-template-columns:${compactCols};gap:16px;padding:8px 16px"><span>Account</span><span>5-hour</span><span>Weekly</span><span>Monthly</span><span></span></div>`;
const listA = () => card([compactHead, ...DROID.map((a, i) => compactRow(a, { open: i === 1 }))]);

TOPICS.push({
  id: 'several', section: 'Account tab', title: 'The Account tab with several accounts', size: 'wide', rec: 'A',
  now: 'Every account is a full card: the head, Droid’s pool tabs, a row per window and two setting rows (“When a limit is reached” and “When Factory Droid stops at a limit”). About 220 px per account, so 6 Droid accounts take three screens, and the settings repeat in each card. Scrolled: <a href="../feedback/evidence/private/07-agent-accounts-multiple-scrolled.png">07-agent-accounts-multiple-scrolled.png</a>; with the mock agent: <a href="img/now-account-tab.png">now-account-tab.png</a>.',
  nowImg: '../feedback/evidence/private/07-agent-accounts-multiple-top.png',
  issues: ['Long: three screens for 6 accounts', 'The same two settings repeat in every card', 'Hard to compare accounts; full accounts look like used ones at a glance'],
  options: [
    { key: 'A', name: 'A line per account that opens', from: 'the Usage table (topic 2 A)',
      desc: 'Each account is one line: avatar, name, tags, email and plan, and a cell per window, the same as on the Usage page. Clicking a line opens it in place to today’s card body: the pool tabs, the full rows, the settings and the ⋯ menu. One opens at a time; with one account it’s always open, as in topic 4.',
      good: '6 accounts fit on one screen and read like the Usage page.', cost: 'Settings and the menu are a click away.',
      mock: () => settingsWindow(agentPage(`<div class="col" style="gap:10px">${sectionHead('Accounts', addAccount)}${listA()}</div>`), { selected: 'Agents', h: 900 }) },
    { key: 'B', name: 'Lines, details in a dialog', from: 'new',
      desc: 'The same lines as A, which never open. Clicking one opens a dialog with the full card: limits, settings and actions.',
      good: 'The list never moves.', cost: 'A dialog for every change; you can’t see two accounts’ details together.',
      mock: () => settingsWindow(agentPage(`<div class="col" style="gap:10px">${sectionHead('Accounts', addAccount)}${card([compactHead, ...DROID.map((a) => compactRow(a))])}</div>`), { selected: 'Agents', h: 900, overlay: `<div class="modal-back"></div><div class="pop" style="z-index:21;left:50%;top:50%;transform:translate(-30%,-50%);width:600px">${accountHead(DROID[1], { right: `${readMeta}${ibtn('more')}${ibtn('x')}` })}<div style="padding:0 16px 14px 56px">${poolTabs}${rowsA(DROID[1].windows)}</div><div style="border-top:1px solid var(--bv)">${droidSettings()}</div></div>` }) },
    { key: 'C', name: 'A list beside the selected account', from: 'macOS Settings’ accounts',
      desc: 'Two columns: a narrow list of accounts on the left, each with its tightest window, and the selected account’s full card on the right.',
      good: 'Details always in view; the list stays short.', cost: 'Only one account’s windows show at a time; the content column is narrow for two columns.',
      mock: () => settingsWindow(agentPage(`<div class="col" style="gap:10px">${sectionHead('Accounts', addAccount)}<div style="display:grid;grid-template-columns:220px minmax(0,1fr);gap:12px;align-items:start">${card(DROID.map((a, i) => { const low = a.windows.reduce((x, w) => (w[1] < x[1] ? w : x)); return `<div class="row g2" style="padding:8px 10px;${i === 1 ? 'background:var(--sel)' : ''}">${avatar(a, 22)}<span class="col grow" style="min-width:0;gap:3px"><span class="sm trunc">${accountName(a)}</span>${bar(low[1], { h: 3 })}</span><span class="xs b5" style="color:${leftColor(low[1])}">${low[1]}%</span></div>`; }))}${card([accountHead(DROID[1], { right: ibtn('more') }), `<div style="padding:12px 16px 14px">${poolTabs}${rowsA(DROID[1].windows, { labelW: 58, leftW: 62, resetW: 96 })}</div>`, droidSettings(16)])}</div></div>`), { selected: 'Agents', h: 900 }) },
    { key: 'D', name: 'Cards with limits only', from: 'today’s cards',
      desc: 'Today’s cards keep the head and the windows, without the tabs and the settings. Each card has “Settings” at the bottom, which opens its pool tabs and settings in place.',
      good: 'Every account’s limits in full, without the repeated settings.', cost: 'Still about 130 px per account: 6 accounts take two screens.',
      mock: () => settingsWindow(agentPage(`<div class="col" style="gap:10px">${sectionHead('Accounts', addAccount)}${DROID.map((a) => card([`${accountHead(a)}<div style="padding:0 16px 10px 56px">${rowsA(a.windows)}</div><div class="row g1 xs mu" style="padding:0 16px 10px 56px">${ic('chev-right', 'xs')}Settings</div>`])).join('')}</div>`), { selected: 'Agents', h: 900 }) },
  ],
});

// 6. Adding an account ------------------------------------------------------------------------------
const methodRow = (icon, title, desc, hl = false) => `<div class="row g3" style="padding:9px 10px;border-radius:6px;${hl ? 'background:var(--hov)' : ''}"><span style="width:24px;height:24px;border-radius:6px;border:1px solid var(--bv);display:grid;place-items:center" class="mu">${ic(icon, 'xs')}</span><div class="col grow" style="gap:1px"><span>${title}</span>${desc ? `<span class="xs mu">${desc}</span>` : ''}</div>${ic('chev-right', 'xs mu')}</div>`;
const METHODS = () => [methodRow('globe', 'Log in with a browser', 'Opens Factory’s login page.', true), methodRow('terminal', 'Log in in a terminal', 'Runs Droid’s login where it runs.'), methodRow('key', 'Use an API key', 'A Factory API key.')].join('');
const copyFrom = `<div class="row g2 sm" style="padding:10px 0 4px;border-top:1px solid var(--bv);margin-top:8px"><span class="col grow" style="gap:1px"><span>Copy settings from</span><span class="xs mu">Its defaults and environment variables.</span></span>${dd(`${avatar(DROID[0], 14)}&nbsp;${ME}`)}</div>`;
const NEW = acct('qa@acme.dev', 'Pro', 190, full3);
const modal = (title, body, buttons, { w = 400, style = '' } = {}) => `<div class="pop" style="position:relative;width:${w}px;padding:16px 16px 14px;${style}"><div class="b6" style="font-size:15px;margin-bottom:4px">${title}</div>${body}<div class="row g2" style="justify-content:flex-end;margin-top:14px">${buttons}</div></div>`;
const stepChoose = () => modal('Add a Factory Droid account', `<div class="sm mu" style="margin-bottom:10px;line-height:1.45">Each account has its own login, sessions and history. Choose how to log in.</div>${METHODS()}${copyFrom}`, gbtn('Cancel'));
const stepWaiting = () => modal('Add a Factory Droid account', `<div class="col" style="align-items:center;gap:10px;padding:18px 0 8px;text-align:center"><span class="spin" style="width:18px;height:18px;border-width:2px"></span><span>Waiting for the browser…</span><span class="sm mu" style="line-height:1.45">Finish logging in on Factory’s page. Choose the account you want to add there.</span><span class="row g2" style="margin-top:4px">${obtn('Open the Page Again')}${obtn(`${ic('copy', 'xs')}Copy Link`)}</span></div>`, `${gbtn('Back')}${obtn('Cancel')}`);
const stepDone = () => modal('Account added', `<div class="col" style="gap:12px;padding-top:6px">${card([accountHead(NEW, { right: '', pad: '10px 12px', size: 24 }), `<div style="padding:10px 12px">${rowsA(NEW.windows, { labelW: 58, leftW: 64, resetW: 50 })}</div>`])}<span class="sm mu">Its settings were copied from ${ME}. You can rename it from its ⋯ menu.</span></div>`, pbtn('Done'));
const stepsMock = () => frame(`<div class="row g3" style="padding:20px;align-items:flex-start;background:rgba(0,0,0,.25);height:100%">${[['1. Choose how to log in', stepChoose()], ['2. While it logs in', stepWaiting()], ['3. The result', stepDone()]].map(([t, m]) => `<div class="col">${caption(t)}${m}</div>`).join('')}</div>`, { w: 1290, h: 470 });

const pendingRow = `<div style="display:grid;grid-template-columns:${compactCols};gap:16px;padding:10px 16px;align-items:center;background:rgba(116,173,232,.08);box-shadow:inset 2px 0 0 var(--ac)"><span class="row g3" style="min-width:0"><span style="width:26px;height:26px;border-radius:50%;border:1.5px dashed var(--b);display:grid;place-items:center">${spinner}</span><span class="col" style="gap:1px"><span>New account</span><span class="xs mu">Logging in with a browser…</span></span></span><span></span><span></span><span class="row" style="justify-content:flex-end">${gbtn('Cancel')}</span><span></span></div>`;
const toast = (html, style) => `<div class="toast row g2" style="${style}"><span class="okc" style="display:inline-flex">${ic('check-circle', 'sm')}</span><span class="grow">${html}</span></div>`;

TOPICS.push({
  id: 'add', section: 'Account tab', title: 'Adding an account', size: 'wide', rec: 'A',
  now: 'Picked in the Accounts round (§3 A, §7 B): Add Account puts a “New account” card at the end of the list, with “Copy settings from” and every login method as a row with Log In. Once logged in, the card becomes the account, named by its email. With a few accounts the card is below the screen, so nothing seems to happen when you click Add Account, and a finished login gives no sign either.',
  nowImg: 'img/now-add-account.png',
  issues: ['The new card is added out of view', 'No clear progress or result', 'Every login method is a full row with its own Log In'],
  options: [
    { key: 'A', name: 'A dialog from start to result', from: 't3code’s Add account dialog',
      desc: 'Add Account opens a dialog. 1: the agent’s login methods as a list, and “Copy settings from” at the bottom. 2: picking one shows its progress in the dialog: a browser login waits with Open the Page Again and Copy Link, a terminal login shows its terminal there, an API key asks for the key. 3: the result: the new account named by its email, its plan and limits, and Done. A failed login says why, with Try Again. If the login is an account that’s already there, it says so and adds nothing. The new account then sits in the list where it belongs.',
      good: 'You see each step and the result; nothing happens off screen.', cost: 'A dialog to build, with a step for each kind of login.',
      mock: () => stepsMock() },
    { key: 'B', name: 'Pick in a dialog, progress in the list', from: 'new',
      desc: 'The dialog only asks how to log in. It then closes and a “New account · Logging in…” line appears at the top of the list, highlighted, with Cancel. When the login ends, the line becomes the account and a toast says “Added qa@acme.dev”.',
      good: 'A short dialog; you can keep working on the page while it logs in.', cost: 'Progress for a terminal login has nowhere to show but a separate terminal.',
      mock: () => settingsWindow(agentPage(`<div class="col" style="gap:10px">${sectionHead('Accounts', addAccount)}${card([compactHead, pendingRow, ...DROID.slice(0, 5).map((a) => compactRow(a))])}</div>`), { selected: 'Agents', h: 760, overlay: toast('Added <b>qa@acme.dev</b> to Factory Droid', 'right:24px;bottom:24px') }) },
    { key: 'C', name: 'Today’s card, at the top', from: 'today',
      desc: 'Today’s “New account” card, placed first in the list, scrolled into view and outlined while it’s open. When the login ends, it becomes the account and keeps a short “Added” tag.',
      good: 'The least change.', cost: 'The page still grows by a long card of login rows.',
      mock: () => settingsWindow(agentPage(`<div class="col" style="gap:10px">${sectionHead('Accounts', addAccount)}${card([`<div class="row g3" style="padding:12px 16px"><span style="width:28px;height:28px;border-radius:50%;border:1.5px dashed var(--b);display:grid;place-items:center" class="mu">${ic('plus', 'xs')}</span><div class="col grow" style="gap:2px"><span>New account</span><span class="sm mu">Choose how Factory Droid logs in.</span></div>${gbtn('Cancel')}</div>`, `<div style="padding:4px 6px">${METHODS()}</div>`], 'border-color:var(--bf);box-shadow:0 0 0 1px var(--bf)')}${card([compactHead, ...DROID.slice(0, 3).map((a) => compactRow(a))])}</div>`), { selected: 'Agents', h: 760 }) },
    { key: 'D', name: 'A menu, then a toast', from: 'Accounts round §3 C',
      desc: 'Add Account opens a menu of the login methods. Picking one starts it at once; a toast shows the progress (“Logging in with a browser… Cancel”) and then the result (“Added qa@acme.dev”).',
      good: 'One click less; no dialog.', cost: '“Copy settings from” has no place; a terminal login opens elsewhere.',
      mock: () => settingsWindow(agentPage(`<div class="col" style="gap:10px;position:relative">${sectionHead('Accounts', `<span class="row g1 sm" style="padding:2px 6px;border-radius:5px;background:var(--hov)">${ic('plus', 'xs')}Add Account${chev}</span>`)}<div class="menu" style="top:24px;right:0;min-width:240px"><div class="it hl">${ic('globe', 'sm')}Log in with a browser</div><div class="it">${ic('terminal', 'sm')}Log in in a terminal</div><div class="it">${ic('key', 'sm')}Use an API key…</div></div>${card([compactHead, ...DROID.slice(0, 5).map((a) => compactRow(a))])}</div>`), { selected: 'Agents', h: 760, overlay: `<div class="toast row g2" style="right:24px;bottom:24px">${spinner}<span class="grow">Logging in with a browser…</span><span class="sm ac">Cancel</span></div>` }) },
  ],
});
