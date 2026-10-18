// Snooze mocks: the sidebar as sidebar.rs draws it (thread cards with their hover buttons, the
// shelves with slim rows, the thread menu), the open thread, and t3code's snooze pieces.

Object.assign(ICONS, {
  'alarm-clock': '<circle cx="12" cy="13" r="8"/><path d="M12 9v4l2 2"/><path d="M5 3 2 6"/><path d="m22 6-3-3"/><path d="M6.38 18.7 4 21"/><path d="M17.64 18.67 20 21"/>',
  'alarm-clock-off': '<path d="M6.87 6.87a8 8 0 1 0 11.26 11.26"/><path d="M19.9 14.25a8 8 0 0 0-9.15-9.15"/><path d="m22 6-3-3"/><path d="M6.26 18.67 4 21"/><path d="m2 2 20 20"/><path d="M4 4 2 6"/>',
  archive: '<rect width="20" height="5" x="2" y="3" rx="1"/><path d="M4 8v11a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8"/><path d="M10 12h4"/>',
  calendar: '<path d="M8 2v4"/><path d="M16 2v4"/><rect width="18" height="18" x="3" y="4" rx="2"/><path d="M3 10h18"/>',
  refresh: '<path d="M3 12a9 9 0 0 1 9-9 9.75 9.75 0 0 1 6.74 2.74L21 8"/><path d="M21 3v5h-5"/><path d="M21 12a9 9 0 0 1-9 9 9.75 9.75 0 0 1-6.74-2.74L3 16"/><path d="M8 16H3v5"/>',
  'arrow-up-right': '<path d="M7 7h10v10"/><path d="M7 17 17 7"/>',
  warning: '<path d="m21.73 18-8-14a2 2 0 0 0-3.48 0l-8 14A2 2 0 0 0 4 21h16a2 2 0 0 0 1.73-3"/><path d="M12 9v4"/><path d="M12 17h.01"/>',
  info: '<circle cx="12" cy="12" r="10"/><path d="M12 16v-4"/><path d="M12 8h.01"/>',
  send: '<path d="M14.54 21.69a.5.5 0 0 0 .94-.03l6.5-19a.5.5 0 0 0-.64-.64l-19 6.5a.5.5 0 0 0-.03.94l7.93 3.18a2 2 0 0 1 1.11 1.11z"/><path d="m21.85 2.15-10.94 10.94"/>',
});

// The demo's clock: Tuesday 14:42. agentZ writes times on a 24-hour clock (usage_limits.rs).
const SZ_PRESETS = [
  ['In 1 hour', '15:42'],
  ['In 3 hours', '17:42'],
  ['This evening', '18:00'],
  ['Tomorrow', 'Wed 9:00'],
  ['Next week', 'Mon 9:00'],
];
const SZ_RESET = ['Until the limit resets', '16:10'];

const SZ_PROJECTS = { st: { mono: 'ST', color: 'g', name: 'storefront' }, ap: { mono: 'AP', color: 't', name: 'api' } };
const SZ_THREADS = {
  checkout: { p: 'st', title: 'Add the checkout page', branch: 'checkout-flow', state: 'working', pinned: true },
  login: { p: 'ap', title: 'Fix flaky login test', branch: 'main', state: 'awaiting' },
  tests: { p: 'st', title: 'Write tests for the rate limiter', branch: 'main', time: '20m' },
  version: { p: 'ap', title: 'Bump the API version', branch: 'main', time: '2h' },
  grid: { p: 'st', title: 'Product grid on small screens', branch: 'grid', time: '1d' },
  orders: { p: 'ap', title: 'Paginate the orders endpoint', branch: 'orders', time: '3d', snoozed: '1d' },
  docs: { p: 'st', title: 'Document the webhook retries', branch: 'main', time: '5d', snoozed: '3h' },
};
const SZ_ARCHIVED = 6;

const szChev = (name = 'chev-down') => ic(name, 'xs');
const szWorking = () => '<span class="pill working"><span class="spin"></span>Working</span>';
const szAgent = () => `<span style="width:14px;height:14px;display:inline-grid;place-items:center;color:rgba(169,175,188,.6);font-size:13px;line-height:1">${GLYPHS.claude}</span>`;
const szPinMark = () => `<span style="display:inline-flex;color:rgba(169,175,188,.65)">${ic('pin', 'xs')}</span>`;
const szWokeBadge = () => `<span class="sz-woke">${ic('alarm-clock', 'xs')}Woke</span>`;

/**
 * A thread card (render_thread_card): project and status, title, branch with machine and
 * agent. Hovered, the status gives way to the hover buttons over a fade.
 */
function szCard(key, { on = false, hov = false, buttons = null, status = null, dim = false } = {}) {
  const thread = SZ_THREADS[key];
  const project = SZ_PROJECTS[thread.p];
  const slot = status ?? (thread.state === 'working' ? szWorking() : thread.state ? pill(thread.state) : `<span class="sm mu">${thread.time}</span>`);
  const hover = hov ? `<div class="sz-hover" style="background:linear-gradient(90deg,transparent,${on ? 'var(--sel)' : 'var(--hov)'} 24px)">${buttons ?? szHoverButtons(thread)}</div>` : '';
  return `<div class="sz-card ${on ? 'on' : ''} ${hov ? 'hov' : ''}" style="${dim ? 'opacity:.55' : ''}">
    <div class="row g15" style="height:20px">${mono(project.mono, project.color)}<span class="grow trunc sm mu">${project.name}</span>${hov ? '' : `${thread.pinned ? szPinMark() : ''}${slot}`}</div>
    <div class="row g15" style="margin-top:4px;min-width:0"><span class="trunc b5">${thread.title}</span></div>
    <div class="row g15 sm faint" style="margin-top:2px"><span class="grow trunc">${thread.branch}</span><span style="opacity:.6;color:var(--mu);display:inline-flex">${ic('laptop', 'sm')}</span>${szAgent()}</div>${hover}</div>`;
}
const szHoverButtons = (thread, extra = '') => `${thread.pinned ? `<span class="sz-hbtn">${ic('pin', 'xs')}</span>` : ''}${extra}<span class="sz-hbtn">${ic('archive', 'xs')}Archive</span>`;
const szClockButton = (hl = false) => `<span class="sz-hbtn ${hl ? 'hl' : ''}">${ic('clock', 'xs')}</span>`;

// A shelf (render_shelf_header) and its slim rows (render_slim_row).
const szShelf = (label, { count = 0, open = false, tone = '' } = {}) => `<div class="sz-shelf ${tone}">${open || !count ? label : `${label} (${count})`}<span class="rule"></span>${szChev(open ? 'chev-up' : 'chev-down')}</div>`;
function szSlim(key, { hov = false, on = false, end = null } = {}) {
  const thread = SZ_THREADS[key];
  const project = SZ_PROJECTS[thread.p];
  const right = end ?? (hov ? `<span class="sz-hbtn">${ic('alarm-clock-off', 'xs')}</span>` : `<span class="sz-left">${thread.snoozed}</span>`);
  return `<div class="sz-slim ${hov ? 'hov' : ''} ${on ? 'on' : ''}"><span class="ico">${mono(project.mono, project.color)}</span><span class="grow trunc">${thread.title}</span>${right}</div>`;
}
const szRule = () => '<div style="margin:6px 10px;height:1px;flex:none;background:var(--bv);opacity:.6"></div>';
const szRestShelves = () => szShelf('Chats', { count: 3 }) + szShelf('Archived', { count: SZ_ARCHIVED });

// A wider frame shows the editor's background beside the sidebar, for menus that open past its edge.
function szSidebar(rows, { h = 640, over = '', w = 290 } = {}) {
  const list = sidebar({ list: rows.join('') });
  const body = w > 290 ? `<div class="row" style="height:100%;align-items:stretch"><div class="none" style="width:290px;border-right:1px solid var(--b)">${list}</div><div class="grow" style="background:var(--ed)"></div></div>` : list;
  return frame(`${body}${over}`, { w, h, style: 'border:1px solid var(--b);border-radius:8px' });
}
const szActiveCards = (options = {}, keys = ['checkout', 'login', 'tests', 'version', 'grid']) => keys.map((key) => szCard(key, { on: key === 'checkout', ...(options[key] || {}) }));

// Menus --------------------------------------------------------------------------------------
const szItem = (icon, label, { hl = false, time = '', arrow = false, dis = false } = {}) => `<div class="it ${hl ? 'hl' : ''} ${dis ? 'dis' : ''}">${icon ? ic(icon, 'sm') : '<span style="width:14px"></span>'}<span>${label}</span>${time ? `<span class="tm">${time}</span>` : ''}${arrow ? `<span class="kb">${ic('chev-right', 'xs')}</span>` : ''}</div>`;
const szHr = '<div class="hr"></div>';
const szMenu = (items, style) => `<div class="menu" style="${style}">${items.join('')}</div>`;
// The thread menu today (sidebar.rs thread_context_menu); `snooze` goes after Archive.
const szThreadMenu = ({ snooze = '', hl = '' } = {}) => [
  szItem('pin', 'Unpin', { hl: hl === 'pin' }),
  szItem('pencil', 'Rename'),
  szItem('refresh', 'Regenerate Title'),
  szItem('archive', 'Archive'),
  snooze,
  szHr,
  szItem('settings', 'Project Settings'),
  szHr,
  szItem('trash', 'Delete…'),
];
const szPresetItems = ({ reset = false, hl = -1, icons = false } = {}) => [
  ...(reset ? [szItem(icons ? 'refresh' : '', SZ_RESET[0], { time: SZ_RESET[1], hl: hl === 'reset' }), szHr] : []),
  ...SZ_PRESETS.map(([label, time], i) => szItem(icons ? 'clock' : '', label, { time, hl: hl === i })),
  szHr,
  szItem(icons ? 'calendar' : '', 'Custom…', { hl: hl === 'custom' }),
];

// The open thread ----------------------------------------------------------------------------
const szHeader = (extra = '', title = 'Paginate the orders endpoint') => `<div class="row sz-head">
  <span class="row g15 sm mu none" style="padding:2px 4px">${mono('AP', 't')}api</span><span class="sm mu">/</span>
  <span class="grow row" style="min-width:0"><span class="row g1" style="padding:2px 6px;font-size:13px;min-width:0"><span class="trunc">${title}</span><span class="mu" style="display:inline-flex">${szChev()}</span></span></span>
  ${extra}<span class="row g1 sm mu none">${ic('branch', 'xs')}orders</span>
  <span class="row none" style="gap:6px;margin-left:6px">${ibtn('terminal')}${ibtn('more')}</span></div>`;
const szConversation = () => `<div class="sz-col">
  <div style="display:flex;justify-content:flex-end;padding:12px 20px 4px"><div style="max-width:80%;padding:8px 12px;border-radius:12px;background:#3d424c">Paginate the orders endpoint with cursors</div></div>
  <div style="margin:0 20px;padding:4px 0;color:#8b909d;display:flex;gap:6px;align-items:center"><span style="width:24px;display:inline-grid;place-items:center">${szChev('chev-right')}</span>Read 4 files, changed 3 files, and ran 2 commands</div>
  <div style="padding:6px 20px">Done: <span class="mono-font sm">GET /orders</span> takes <span class="mono-font sm">cursor</span> and <span class="mono-font sm">limit</span>, and returns <span class="mono-font sm">next_cursor</span>. The mobile client still pages by number; it needs updating before this ships.</div></div>`;
const szComposer = () => `<div class="sz-comp"><div class="sz-comp-in"><div style="height:22px;font-size:13px"><span class="ph">Message the agent…</span></div>
  <div class="row" style="height:26px;gap:10px;font-size:12px;color:var(--mu)"><span class="row g15">${glyph('claude')}Claude Agent</span><span class="grow"></span><span class="row g1">Opus 4.5 ${szChev()}</span><span style="display:inline-flex;color:var(--ac)">${ic('send', 'sm')}</span></div></div></div>`;
const szThread = ({ header = szHeader(), above = '', overlays = '', w = 640, h = 330, body = szConversation() } = {}) =>
  frame(`<div class="col" style="height:100%">${header}<div class="sz-conv">${body}</div>${above}${szComposer()}</div>${overlays}`, { w, h, style: 'border:1px solid var(--b);border-radius:8px' });

// The usage-limit notice (render_limit_notice), with optional extra buttons.
const szLimitNotice = (buttons = '') => `<div style="padding:0 8px 8px"><div class="sz-callout" style="background:rgba(222,193,132,.05)"><span style="height:18px;display:inline-flex;align-items:center;color:var(--warn)">${ic('warning', 'sm')}</span><div class="grow col" style="gap:8px;min-width:0"><div class="row"><span class="grow">Your account reached its 5-hour limit</span><span class="mu" style="display:inline-flex">${ic('x', 'sm')}</span></div><div class="mu">Claude Agent stopped. The limit resets at 16:10, in 1h 28m.</div><div class="row g1"><span class="btn sm">Continue at 16:10</span>${buttons}<span class="btn ghost sm">Usage ${ic('arrow-up-right', 'xs')}</span></div></div></div></div>`;

const szCaption = (text) => `<div class="sz-cap">${text}</div>`;
const szPair = (...items) => `<div class="sz-pair">${items.join('')}</div>`;
