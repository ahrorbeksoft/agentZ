// The timeline round's building blocks, over board/mock.js: Settings › Usage in JetBrains Dark
// as it is today (a table per agent), and a timeline of each account's limit windows drawn as
// CLIProxyAPI's management UI draws its credentials' quota windows (QuotaTimeline.tsx), with
// switches for every choice the topics offer.

Object.assign(ICONS, {
  flame: '<path d="M8.5 14.5A2.5 2.5 0 0 0 11 12c0-1.38-.5-2-1-3-1.072-2.143-.224-4.054 2-6 .5 2.5 2 4.9 4 6.5 2 1.6 3 3.5 3 5.5a7 7 0 1 1-14 0c0-1.153.433-2.294 1-3a2.5 2.5 0 0 0 2.5 2.5z"/>',
  'arrow-left': '<path d="m12 19-7-7 7-7"/><path d="M19 12H5"/>',
  info: '<circle cx="12" cy="12" r="10"/><path d="M12 16v-4"/><path d="M12 8h.01"/>',
  table: '<path d="M12 3v18"/><rect width="18" height="18" x="3" y="3" rx="2"/><path d="M3 9h18"/><path d="M3 15h18"/>',
  timeline: '<path d="M3 6h10"/><path d="M8 12h13"/><path d="M5 18h9"/>',
});
GLYPHS.droid = '❋';
GLYPHS.devin = '◐';

// Time: hours from Sunday, October 5, 00:00. Now is Thursday, October 9, 14:20.
const NOW_H = 4 * 24 + 14 + 20 / 60;
const WEEKDAYS = ['Sun', 'Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat'];
const MONTHS = ['Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun', 'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec'];
const pad = (value) => String(value).padStart(2, '0');
function when(hours) {
  const date = new Date(Date.UTC(2025, 9, 5) + Math.round(hours * 60) * 60000);
  return { weekday: WEEKDAYS[date.getUTCDay()], day: date.getUTCDate(), month: date.getUTCMonth() + 1, mon: MONTHS[date.getUTCMonth()], time: `${pad(date.getUTCHours())}:${pad(date.getUTCMinutes())}`, hour: date.getUTCHours(), dayIndex: Math.floor((hours + 1e-6) / 24) };
}
function duration(hours) {
  const minutes = Math.max(0, Math.round(hours * 60));
  if (minutes < 60) return `${minutes}m`;
  if (minutes < 24 * 60) return `${Math.floor(minutes / 60)}h ${pad(minutes % 60)}m`.replace(' 00m', '');
  return `${Math.floor(minutes / 1440)}d ${Math.floor((minutes % 1440) / 60)}h`;
}
// A moment as each label style writes it: CLIProxyAPI's 10/12 08:20, dates as Oct 12, 08:20,
// or how long until it.
function moment(hours, style = 'dates', { short = false } = {}) {
  const at = when(hours);
  if (style === 'relative') return `in ${duration(hours - NOW_H)}`;
  if (style === 'cpa') return short ? at.time : `${pad(at.month)}/${pad(at.day)} ${at.time}`;
  if (short) return at.dayIndex === when(NOW_H).dayIndex ? at.time : `${at.weekday} ${at.time}`;
  return `${at.mon} ${at.day}, ${at.time}`;
}

// The demo: each agent's accounts and their windows. A window is its name, % left, hours to its
// reset (null: nothing used, so no window is counting down) and its length in hours.
const COLORS = { blue: '#60a5fa', green: '#4ade80', orange: '#fb923c', purple: '#c084fc' };
const limitWindow = (label, left, resetsIn, length) => ({ label, left, resetsIn, length });
const account = (name, plan, color, windows, extra = {}) => ({ name, plan, color, windows, ...extra });
const ME = 'alex@hey.com';
const DEMO_AGENTS = [
  { id: 'claude', name: 'Claude Agent', accounts: [
    account(ME, 'Max 5x', COLORS.blue, [limitWindow('Session', 74, 2.17, 5), limitWindow('Weekly', 13, 66, 168)], { outside: true }),
    account('Work', 'Team', COLORS.green, [limitWindow('Session', 0, 1.87, 5), limitWindow('Weekly', 64, 102, 168)], { email: 'work@acme.dev' }),
  ] },
  { id: 'codex', name: 'Codex', accounts: [
    account(ME, 'Plus', COLORS.blue, [limitWindow('5-hour', 83, 3.08, 5), limitWindow('Weekly', 81, 122, 168)], { outside: true, limitResets: [NOW_H + 116] }),
  ] },
  { id: 'droid', name: 'Factory Droid', accounts: [
    account(ME, 'Pro', COLORS.blue, [limitWindow('5-hour', 97, 4.3, 5), limitWindow('Weekly', 3, 24, 168), limitWindow('Monthly', 51, 567, 720)], { outside: true }),
    account('Work', 'Pro', COLORS.green, [limitWindow('5-hour', 68, 2.67, 5), limitWindow('Weekly', 88, 119, 168), limitWindow('Monthly', 94, 671, 720)], { email: 'work@acme.dev' }),
    account('Design', 'Pro', COLORS.purple, [limitWindow('5-hour', 25, 1.08, 5), limitWindow('Weekly', 70, 74, 168), limitWindow('Monthly', 81, 460, 720)], { email: 'design@acme.dev' }),
    account('Side', 'Pro', COLORS.orange, [limitWindow('5-hour', 100, null, 5), limitWindow('Weekly', 100, null, 168), limitWindow('Monthly', 100, null, 720)], { email: 'side@fastmail.com' }),
  ] },
  { id: 'devin', name: 'Devin', accounts: [
    account(ME, 'Pro', COLORS.blue, [limitWindow('Daily', 100, null, 24), limitWindow('Weekly', 100, null, 168)], { outside: true }),
  ] },
];
const agentById = (id) => DEMO_AGENTS.find((agent) => agent.id === id);

// A window's pace (usage_limits::pace): what's used so far, projected to the reset at the same rate.
function paceOf(window) {
  if (window.resetsIn === null) return { kind: 'ok' };
  const used = 100 - window.left;
  const start = NOW_H + window.resetsIn - window.length;
  if (window.left === 0) return { kind: 'out', runsOut: NOW_H };
  const elapsed = window.length - window.resetsIn;
  if (used === 0 || elapsed < window.length * 0.01) return { kind: 'ok' };
  const projected = (used * window.length) / elapsed;
  if (projected > 100) return { kind: 'out', runsOut: start + (elapsed * 100) / used, projected };
  if (100 - projected < 10 && used >= 5) return { kind: 'tight', projected };
  return { kind: 'ok', projected };
}
const PACE_COLORS = { ok: 'var(--ac)', tight: 'var(--warn)', out: 'var(--del)' };

// Pieces in the app's style.
const jb = (html) => html.replace('class="m"', 'class="m jb"');
const chev = ic('chev-down', 'xs');
const avatar = (item, size = 18) => `<span style="width:${size}px;height:${size}px;border-radius:50%;flex:none;display:inline-grid;place-items:center;font-size:${Math.round(size * 0.5)}px;font-weight:600;color:#1e1f22;background:${item.color}">${item.name[0].toUpperCase()}</span>`;
const chip = (text) => `<span class="tl-chip">${text}</span>`;
const caption = (text) => `<div style="font-size:11px;color:var(--ph);margin:0 0 6px">${text}</div>`;

// Settings (settings_page.rs): the sections on the left and a page.
const NAV = ['General', 'Appearance', 'Notifications', 'Agents', 'Usage', 'Skills', 'MCP Servers', 'Machines'];
const NAV_ICONS = { General: 'settings', Appearance: 'eye', Notifications: 'bell', Agents: 'bot', Usage: 'activity', Skills: 'note', 'MCP Servers': 'zap', Machines: 'monitor' };
function settingsPage(body, { section = 'Usage', w = 1000, h = 620, over = '', top = 22 } = {}) {
  return jb(frame(`<div class="row" style="height:100%;align-items:stretch;position:relative">
    <div class="tl-asb"><div class="tl-asb-head"><span class="grow">Settings</span>${ic('x', 'sm mu')}</div>
      <div class="col" style="padding:2px 4px;gap:2px">${NAV.map((name) => `<div class="tl-lrow ${name === section ? 'on' : ''}">${ic(NAV_ICONS[name], 'sm mu')}${name}</div>`).join('')}</div></div>
    <div class="grow" style="padding:${top}px 0;overflow:hidden;background:var(--ed);position:relative"><div style="max-width:700px;margin:0 auto;padding:0 20px">${body}</div></div>${over}</div>`, { w, h, style: 'border-radius:8px;border:1px solid #111' }));
}
const machinePicker = () => `<span class="tl-dd">${ic('laptop', 'sm mu')}This Mac${chev}</span>`;
const usageHead = (right = machinePicker()) => `<div class="tl-pghead"><span class="tt grow">Usage</span>${right}</div>`;
// One piece alone on the page's background, for topics about the timeline itself.
const panel = (html, { w = 700, over = '' } = {}) => `<div class="m jb" style="width:${w}px;border-radius:8px;border:1px solid #111"><div style="padding:16px 20px;background:var(--ed);position:relative">${html}${over}</div></div>`;

// The Usage page's table (UsageTable), as today: a row per account, a cell per window.
function limitCell(window) {
  const pace = paceOf(window);
  const color = window.left === 0 ? 'var(--del)' : PACE_COLORS[pace.kind];
  const mark = window.resetsIn === null ? null : (window.resetsIn / window.length) * 100;
  return `<span class="tl-cell" style="min-width:0"><span class="row" style="gap:6px"><span class="pc" style="${window.left === 0 ? 'color:var(--del)' : ''}">${window.left === 0 ? 'Used up' : `${window.left}%`}</span><span class="grow"></span><span class="rs">${window.resetsIn === null ? '' : `↻ ${duration(window.resetsIn)}`}</span></span><span class="tl-thin"><i style="width:${window.left}%;background:${color}"></i>${mark === null ? '' : `<b style="left:${mark}%"></b>`}</span></span>`;
}
function usageTable(agent, { title = true } = {}) {
  const labels = agent.accounts[0].windows.map((window) => window.label);
  const cols = `minmax(0,1fr) repeat(${labels.length},${labels.length > 2 ? 96 : 110}px) 12px`;
  const pooled = labels.map((label, index) => {
    const left = Math.round(agent.accounts.reduce((sum, item) => sum + item.windows[index].left, 0) / agent.accounts.length);
    return `<span class="tl-cell"><span class="pc">${left}%</span><span class="tl-thin"><i style="width:${left}%;background:var(--ac)"></i></span></span>`;
  });
  const rows = [
    `<div class="tl-trow head" style="grid-template-columns:${cols}"><span>Account</span>${labels.map((label) => `<span>${label}</span>`).join('')}<span></span></div>`,
    agent.accounts.length > 1 ? `<div class="tl-trow" style="grid-template-columns:${cols}"><span>All ${agent.accounts.length} accounts</span>${pooled.join('')}<span></span></div>` : '',
    ...agent.accounts.map((item) => `<div class="tl-trow" style="grid-template-columns:${cols}"><span class="row g2" style="min-width:0">${avatar(item, 22)}<span class="col" style="min-width:0"><span class="row g2">${item.name}${item.outside ? chip('Outside') : ''}</span><span class="tl-sub2">${[item.plan, item.email].filter(Boolean).join(' · ')}</span></span></span>${item.windows.map(limitCell).join('')}<span class="mu" style="display:inline-flex">${ic('chev-right', 'xs')}</span></div>`),
  ];
  return `${title ? `<div class="tl-agent">${glyph(agent.id)}<span>${agent.name}</span>${agent.accounts.length > 1 ? `<span class="n">${agent.accounts.length} accounts</span>` : ''}</div>` : ''}<div class="tl-card">${rows.join('')}</div>`;
}

// The spans each zoom shows: where they start and end, in hours, and their cells.
const SPANS = {
  week: { start: 0, end: 336, cell: 24, words: 'two weeks' },
  today: { start: 96, end: 432, cell: 24, words: 'two weeks' },
  session: { start: 96, end: 168, cell: 6, words: 'three days' },
  month: { start: -168, end: 1176, cell: 168, words: 'eight weeks' },
};
// Which of an account's windows a lane draws in each zoom: CLIProxyAPI's longest that fits the
// span, or exactly the 5-hour one, or the one by that name.
const PICKS = {
  week: (windows) => windows.filter((window) => window.length <= 336).sort((a, b) => b.length - a.length)[0],
  session: (windows) => windows.find((window) => window.length === 5),
  month: (windows) => windows.find((window) => window.label === 'Monthly'),
};
const PERIOD = (length) => (length < 24 ? `${length}h` : `${Math.round(length / 24)}d`);

function axisCells(span, labels) {
  const cells = [];
  for (let at = span.start; at < span.end - 1e-6; at += span.cell) {
    const info = when(at);
    const dayStart = info.hour === 0;
    const today = span.cell <= 24 && info.dayIndex === when(NOW_H).dayIndex;
    const weekend = span.cell === 24 && (info.weekday === 'Sat' || info.weekday === 'Sun');
    let top = '';
    let bottom;
    if (span.cell === 168) {
      bottom = `${info.mon} ${info.day}`;
    } else if (span.cell < 24 && !dayStart) {
      bottom = `${pad(info.hour)}:00`;
    } else {
      top = info.weekday;
      bottom = labels === 'cpa' ? `${pad(info.month)}/${pad(info.day)}` : (at === span.start || info.day === 1 || span.cell < 24 ? `${info.mon} ${info.day}` : `${info.day}`);
    }
    cells.push({ at, top, bottom, today, weekend, minor: span.cell < 24 && !dayStart });
  }
  return cells;
}
const cellClass = (cell) => [cell.today ? 'tl-today' : '', cell.weekend ? 'tl-we' : '', cell.minor ? 'tl-minor' : ''].join(' ');

const DEFAULTS = {
  w: 660, headW: 168, mode: 'week', span: null, agents: null, group: 'agent', head: 'avatar', bar: 'used',
  color: 'pace', runout: 'stretch', windows: 'ahead', labels: 'dates', resets: 'tick', idle: 'lane',
  legend: 'full', nav: 'arrows', zoom: 'two', header: true, hover: null, title: 'Limit windows', sort: false,
  lanePer: 'account', pick: 'fit', strip: false, chevron: false,
};
// The window that stops the account first: the gauge's choice, the lowest that's counting down.
const tightest = (windows) => windows.filter((window) => window.resetsIn !== null).sort((a, b) => a.left - b.left)[0] || PICKS.week(windows);

// One lane's track: the grid, the now line, its windows' bars, and reset ticks.
function track(item, window, o, span, cells) {
  const width = o.w - o.headW;
  const total = span.end - span.start;
  const x = (hours) => ((Math.min(Math.max(hours, span.start), span.end) - span.start) / total) * 100;
  const px = (hours) => (hours / total) * width;
  const grid = `<div class="tl-grid">${cells.map((c) => `<span class="${cellClass(c)}"></span>`).join('')}</div>`;
  const nowLine = NOW_H >= span.start && NOW_H < span.end ? `<div class="tl-now" style="left:${x(NOW_H)}%"></div>` : '';
  if (!window || window.resetsIn === null) {
    const idle = o.idle === 'lane' ? '<span class="tl-idle">All left · no window counting down</span>' : '';
    return { html: `<div class="tl-track">${grid}${nowLine}${idle}</div>`, tall: false };
  }
  const reset = NOW_H + window.resetsIn;
  const start = reset - window.length;
  const pace = paceOf(window);
  const color = o.color === 'pace' ? PACE_COLORS[pace.kind] : o.color === 'account' ? item.color : (window.left === 0 ? 'var(--del)' : 'var(--ac)');
  const bars = [];
  if (o.windows === 'all') {
    for (let end = start; end > span.start; end -= window.length) bars.push({ from: end - window.length, to: end, state: 'past' });
  }
  bars.push({ from: start, to: reset, state: 'live' });
  if (o.windows !== 'current') {
    for (let from = reset; from < span.end; from += window.length) bars.push({ from, to: from + window.length, state: 'next' });
  }
  const short = span.cell < 24;
  let tall = false;
  let above = '';
  const html = bars.filter((b) => b.to > span.start && b.from < span.end).map((b) => {
    const left = x(b.from);
    const right = x(b.to);
    const visible = Math.min(b.to, span.end) - Math.max(b.from, span.start);
    // Windows other than the current one have nothing used, so only the account's color or
    // the accent says anything about them.
    const style = `left:${left}%;width:${right - left}%;--c:${b.state === 'live' || o.color === 'account' ? color : 'var(--ac)'}`;
    if (b.state !== 'live') {
      const text = px(visible) > 92 && b.state === 'next' ? `<span class="tl-txt">${o.labels === 'cpa' ? '' : 'resets '}${moment(b.to, o.labels === 'relative' ? 'dates' : o.labels, { short })}</span>` : '';
      return `<div class="tl-bar tl-${b.state}" style="${style}">${text}</div>`;
    }
    const visStart = Math.max(b.from, span.start);
    const share = (hours) => Math.max(0, Math.min(100, ((hours - visStart) / visible) * 100));
    let fill = '';
    if (o.bar === 'used') fill = `<span class="tl-fill" style="width:${share(b.from + (window.length * (100 - window.left)) / 100)}%"></span>`;
    if (o.bar === 'left') fill = `<span class="tl-fill" style="width:${share(b.from + (window.length * window.left) / 100)}%"></span>`;
    let out = '';
    if (pace.kind === 'out' && o.runout === 'stretch') out = `<span class="tl-out" style="left:${share(Math.max(pace.runsOut, NOW_H))}%"></span>`;
    let label;
    if (o.bar === 'cell') {
      label = `<span class="row" style="gap:8px;width:100%"><span class="b5" style="font-size:11px;${window.left === 0 ? 'color:var(--del)' : ''}">${window.left === 0 ? 'Used up' : `${window.left}%`}</span><span class="tl-thin" style="width:70px;margin:0"><i style="width:${window.left}%;background:${color}"></i></span><span style="color:var(--ph)">↻ ${duration(window.resetsIn)}</span></span>`;
    } else if (o.labels === 'cpa') {
      label = `${window.left}% · ${moment(reset, 'cpa', { short })}`;
    } else {
      label = `${window.left === 0 ? 'Used up' : `${window.left}% left`} · resets ${moment(reset, o.labels, { short })}`;
    }
    const room = o.bar === 'cell' ? 190 : String(label).length * 6 + 18;
    if (px(visible) < room) {
      tall = true;
      above = `<span class="tl-above" style="left:${left}%">${label}</span>`;
      label = '';
    }
    return `<div class="tl-bar tl-live ${o.bar === 'cell' ? 'tl-cellbar' : ''}" style="${style}">${fill}${out}${label ? `<span class="tl-txt">${label}</span>` : ''}</div>`;
  }).join('');
  const flame = pace.kind === 'out' && o.runout === 'flame' && pace.runsOut < span.end ? `<span class="tl-flame" style="left:${x(Math.max(pace.runsOut, NOW_H))}%;${tall ? 'top:26px' : ''}">${ic('flame', 'xs')}</span>` : '';
  const session = o.strip && item.windows.find((w) => w.length === 5 && w.resetsIn !== null);
  const strip = session ? `<span style="position:absolute;top:38px;height:4px;border-radius:2px;left:${x(NOW_H + session.resetsIn - 5)}%;width:${Math.max(0.6, x(NOW_H + session.resetsIn) - x(NOW_H + session.resetsIn - 5))}%;background:${PACE_COLORS[paceOf(session).kind]};z-index:2"></span>` : '';
  const ticks = o.resets === 'none' ? '' : (item.limitResets || []).filter((at) => at > span.start && at < span.end).map((at) => `<span class="tl-tick" style="left:${x(at)}%"></span>`).join('');
  return { html: `<div class="tl-track ${tall ? 'tl-tall' : ''}">${grid}${nowLine}${html}${above}${flame}${ticks}${strip}</div>`, tall };
}

function laneHead(item, window, agent, o) {
  const pace = window ? paceOf(window) : { kind: 'ok' };
  const icon = o.group === 'flat-icon' || o.group === 'sorted' ? `<span class="glyph" style="width:14px;font-size:12px">${GLYPHS[agent.id]}</span>` : '';
  const resets = (indent) => (o.resets === 'head' && item.limitResets ? `<div class="tl-sub" style="color:var(--warn);${indent ? 'padding-left:23px' : ''}">1 limit reset</div>` : '');
  if (o.head === 'cpa') {
    const limits = item.windows.filter((w) => w.length <= 168).map((w) => `${w.label} <b>${w.left}%</b>`).join('&nbsp;&nbsp;');
    return `<div class="tl-lhead"><div class="tl-top">${icon}<span class="tl-dot" style="background:${item.color}"></span><span class="tl-nm">${item.name}</span>${window ? `<span class="tl-period">${PERIOD(window.length)}</span>` : ''}</div><div class="tl-sub">${limits}</div>${resets(false)}</div>`;
  }
  if (o.head === 'cell' && window) {
    return `<div class="tl-lhead"><div class="tl-top">${icon}${avatar(item, 16)}<span class="tl-nm grow">${item.name}</span></div><div class="tl-sub" style="padding-left:23px">${window.resetsIn === null ? 'All left' : `<b style="${pace.kind !== 'ok' ? `color:${PACE_COLORS[pace.kind]}` : ''}">${window.left === 0 ? 'Used up' : `${window.left}% left`}</b> · ↻ ${duration(window.resetsIn)}`}</div></div>`;
  }
  return `<div class="tl-lhead"><div class="tl-top">${icon}${avatar(item, 16)}<span class="tl-nm">${item.name}</span></div><div class="tl-sub" style="padding-left:23px">${item.plan} · ${window ? window.label : ''}</div>${resets(true)}</div>`;
}

function timelineHead(o, span) {
  const first = when(span.start);
  const last = when(span.end - 1);
  const range = o.labels === 'cpa' ? `${pad(first.month)}/${pad(first.day)} – ${pad(last.month)}/${pad(last.day)}` : `${first.mon} ${first.day} – ${last.mon} ${last.day}`;
  const current = o.offsetLabel ? '' : ' · current';
  const nav = o.nav === 'arrows' ? `<span class="row" style="gap:4px"><span class="tl-btn">‹</span><span class="tl-btn ${o.offsetLabel ? '' : 'tl-off'}">${o.offsetLabel || 'Today'}</span><span class="tl-btn">›</span></span>`
    : o.nav === 'forward' ? `<span class="row" style="gap:4px"><span class="tl-btn tl-off">Today</span><span class="tl-btn">›</span></span>` : '';
  const mode = o.mode;
  const zoom = o.zoom === 'two' ? `<span class="tl-seg"><span class="${mode === 'week' ? 'on' : ''}">Weekly</span><span class="${mode === 'session' ? 'on' : ''}">5-hour</span></span>`
    : o.zoom === 'names' ? `<span class="tl-seg"><span class="${mode === 'session' ? 'on' : ''}">5-hour</span><span>Daily</span><span class="${mode === 'week' ? 'on' : ''}">Weekly</span><span class="${mode === 'month' ? 'on' : ''}">Monthly</span></span>` : '';
  const info = o.legend === 'button' ? `<span class="tl-btn" style="border-color:transparent">${ic('info', 'sm mu')}</span>` : '';
  const words = o.nav === 'none' ? `from today · ${span.words}` : `${span.words}${current}`;
  return `<div class="tl-head"><div class="grow"><div class="tl-ht">${o.title}</div><div class="tl-hr">${range} · ${words}</div></div>${info}${nav}${zoom}</div>`;
}

function legend(o, span) {
  if (o.legend !== 'full') return '';
  const swatch = (style) => `<span class="tl-sw" style="${style}"></span>`;
  const items = [
    `${swatch('background:color-mix(in srgb,var(--ac) 30%,transparent);border:1px solid color-mix(in srgb,var(--ac) 55%,transparent)')}current window, filled with what’s used`,
    o.windows !== 'current' ? `${swatch('border:1px dashed color-mix(in srgb,var(--ac) 45%,transparent)')}windows to come` : '',
    o.windows === 'all' ? `${swatch('background:color-mix(in srgb,var(--ac) 10%,transparent)')}past` : '',
    o.runout === 'stretch' ? `${swatch('background:repeating-linear-gradient(135deg,rgba(247,84,100,.6) 0 3px,rgba(247,84,100,.15) 3px 7px)')}stopped, at this pace` : '',
    o.runout === 'flame' ? `<span class="row" style="gap:5px"><span style="color:var(--del);display:inline-flex">${ic('flame', 'xs')}</span>runs out, at this pace</span>` : '',
    o.resets !== 'none' ? `<span class="row" style="gap:5px"><span style="width:3px;height:10px;border-radius:2px;background:var(--warn);display:inline-block"></span>a limit reset expires</span>` : '',
  ].filter(Boolean);
  const note = span.cell < 24
    ? 'Each bar is one 5-hour window. One opens with the first message after a reset, so those to come are the earliest they could be.'
    : 'Each bar is one whole window, from when it opened to when it resets. Accounts whose bars end together come back together.';
  return `<div class="tl-legend">${items.map((item) => `<span class="row" style="gap:0">${item}</span>`).join('')}<span class="tl-note">${note}</span></div>`;
}

// The whole timeline. Every option is a switch; DEFAULTS are the recommended picks.
function timeline(options = {}) {
  const o = { ...DEFAULTS, ...options };
  const span = o.span || SPANS[o.mode];
  const pick = PICKS[o.mode];
  const cells = axisCells(span, o.labels);
  const cols = `grid-template-columns:${o.headW}px minmax(0,1fr)`;
  const agents = o.agents || DEMO_AGENTS;
  let lanes = [];
  const idle = [];
  for (const agent of agents) {
    for (const item of agent.accounts) {
      const windows = o.lanePer === 'window' ? item.windows.filter((window) => window.length <= span.end - span.start) : [o.pick === 'tightest' ? tightest(item.windows) : pick(item.windows)];
      for (const window of windows) {
        if (!window) continue;
        if (window.resetsIn === null && o.idle !== 'lane') {
          idle.push(`${item.name} (${agent.name})`);
          continue;
        }
        lanes.push({ agent, item, window });
      }
    }
  }
  if (o.sort) lanes = lanes.slice().sort((a, b) => (a.window.resetsIn ?? 1e9) - (b.window.resetsIn ?? 1e9));
  const rows = [];
  let lastAgent = null;
  for (const lane of lanes) {
    if (o.group === 'agent' && lane.agent !== lastAgent && agents.length > 1) {
      rows.push(`<div class="tl-group"><span><span class="glyph" style="font-size:12px">${GLYPHS[lane.agent.id]}</span>${lane.agent.name}</span></div>`);
    }
    lastAgent = lane.agent;
    const hovered = o.hover && o.hover.agent === lane.agent.id && o.hover.name === lane.item.name;
    const drawn = track(lane.item, lane.window, o, span, cells);
    const head = laneHead(lane.item, lane.window, lane.agent, o);
    rows.push(`<div class="tl-lane ${hovered ? 'tl-hov' : ''}" style="${cols}">${o.chevron && hovered ? head.replace('<div class="tl-top">', '<div class="tl-top" style="position:relative">').replace('</div><div class="tl-sub"', `<span class="mu" style="position:absolute;right:-4px;display:inline-flex">${ic('chev-right', 'xs')}</span></div><div class="tl-sub"`) : head}${drawn.html}</div>`);
  }
  const axis = `<div class="tl-axis" style="${cols}"><span class="tl-lab">Account</span><div class="tl-cells">${cells.map((c) => `<span class="${cellClass(c)}"><span class="tl-wd">${c.top}</span>${c.bottom}</span>`).join('')}</div></div>`;
  const empty = lanes.length ? '' : `<div class="tl-foot" style="border-top:1px solid var(--b)">No account reports a ${o.mode === 'session' ? '5-hour' : 'monthly'} window.</div>`;
  const idleLine = idle.length && o.idle === 'line' ? `<div class="tl-foot">No window counting down, all left: ${idle.join(', ')}.</div>` : '';
  return `<div class="tl" style="width:${o.w}px">${o.header ? timelineHead(o, span) : ''}${axis}${rows.join('')}${empty}${idleLine}${legend(o, span)}${o.over || ''}</div>`;
}

// A tooltip over a mock, placed by the caller.
const tip = (lines, style) => `<div class="tl-tip" style="${style}">${lines.join('<br>')}</div>`;

function windowTip(agentId, name, style, label = 'Weekly') {
  const item = agentById(agentId).accounts.find((candidate) => candidate.name === name);
  const window = item.windows.find((candidate) => candidate.label === label);
  const reset = NOW_H + window.resetsIn;
  const start = reset - window.length;
  if (style === 'cpa') return [item.name, `${moment(start, 'cpa')} → ${moment(reset, 'cpa')}`, `${window.left}% remaining`];
  const pace = paceOf(window);
  const lines = [`<b>${window.label} · ${window.left === 0 ? 'Used up' : `${window.left}% left`}</b>`, `<span class="mu2">Opened ${moment(start)} · resets ${moment(reset)}</span>`];
  if (pace.kind === 'out' && window.left > 0) lines.push(`<span class="tl-red">Runs out ${moment(pace.runsOut)}, at this pace</span>`, `<span class="mu2">~${Math.round(pace.projected - 100)}% over the limit at reset</span>`);
  else if (pace.projected) lines.push(`<span class="mu2">~${Math.round(100 - pace.projected)}% left at reset</span>`);
  return lines;
}
