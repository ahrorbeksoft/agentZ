// Cloud machines from providers (Instabox, Proton): where they live in Settings, signing in,
// creating machines, their rows and pages, and what happens when they're stopped or idle. The
// settings helpers are copied from ../usage/topics-usage.js, since decisions.js loads only a
// round's own scripts.

Object.assign(ICONS, {
  'arrow-left': '<path d="m12 19-7-7 7-7"/><path d="M19 12H5"/>',
  sparkle: '<path d="M9.937 15.5A2 2 0 0 0 8.5 14.063l-6.135-1.582a.5.5 0 0 1 0-.962L8.5 9.936A2 2 0 0 0 9.937 8.5l1.582-6.135a.5.5 0 0 1 .963 0L14.063 8.5A2 2 0 0 0 15.5 9.937l6.135 1.581a.5.5 0 0 1 0 .964L15.5 14.063a2 2 0 0 0-1.437 1.437l-1.582 6.135a.5.5 0 0 1-.963 0z"/>',
  gauge: '<path d="m12 14 4-4"/><path d="M3.34 19a10 10 0 1 1 17.32 0"/>',
  book: '<path d="M12 7v14"/><path d="M3 18a1 1 0 0 1-1-1V4a1 1 0 0 1 1-1h5a4 4 0 0 1 4 4 4 4 0 0 1 4-4h5a1 1 0 0 1 1 1v13a1 1 0 0 1-1 1h-6a3 3 0 0 0-3 3 3 3 0 0 0-3-3z"/>',
  plug: '<path d="M12 22v-5"/><path d="M9 8V2"/><path d="M15 8V2"/><path d="M18 8v5a4 4 0 0 1-4 4h-4a4 4 0 0 1-4-4V8Z"/>',
  database: '<ellipse cx="12" cy="5" rx="9" ry="3"/><path d="M3 5V19A9 3 0 0 0 21 19V5"/><path d="M3 12A9 3 0 0 0 21 12"/>',
  key: '<path d="m15.5 7.5 2.3 2.3a1 1 0 0 0 1.4 0l2.1-2.1a1 1 0 0 0 0-1.4L19 4"/><path d="m21 2-9.6 9.6"/><circle cx="7.5" cy="15.5" r="5.5"/>',
  info: '<circle cx="12" cy="12" r="10"/><path d="M12 16v-4"/><path d="M12 8h.01"/>',
  cloud: '<path d="M17.5 19H9a7 7 0 1 1 6.71-9h1.79a4.5 4.5 0 1 1 0 9Z"/>',
  pause: '<rect x="14" y="4" width="4" height="16" rx="1"/><rect x="6" y="4" width="4" height="16" rx="1"/>',
  power: '<path d="M12 2v10"/><path d="M18.4 6.6a9 9 0 1 1-12.77.04"/>',
  camera: '<path d="M14.5 4h-5L7 7H4a2 2 0 0 0-2 2v9a2 2 0 0 0 2 2h16a2 2 0 0 0 2-2V9a2 2 0 0 0-2-2h-3l-2.5-3z"/><circle cx="12" cy="13" r="3"/>',
  coins: '<circle cx="8" cy="8" r="6"/><path d="M18.09 10.37A6 6 0 1 1 10.34 18"/><path d="M7 6h1v4"/><path d="m16.71 13.88.7.71-2.82 2.82"/>',
  lock: '<rect width="18" height="11" x="3" y="11" rx="2" ry="2"/><path d="M7 11V7a5 5 0 0 1 10 0v4"/>',
  link: '<path d="M10 13a5 5 0 0 0 7.54.54l3-3a5 5 0 0 0-7.07-7.07l-1.72 1.71"/><path d="M14 11a5 5 0 0 0-7.54-.54l-3 3a5 5 0 0 0 7.07 7.07l1.71-1.71"/>',
  updown: '<path d="m7 15 5 5 5-5"/><path d="m7 9 5-5 5 5"/>',
  logout: '<path d="m16 17 5-5-5-5"/><path d="M21 12H9"/><path d="M9 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h4"/>',
});

// The demo world: this Mac and Devbox 1 over SSH, an Instabox account with two machines and a
// Proton account with two computers. proton-cloud is the SSH machine already saved today,
// linked to the Proton computer devbox-3.
const PROVIDERS = {
  ib: { name: 'Instabox', account: 'alex@hey.com', via: 'API key', usage: '2 of 5 machines · 1 of 10 snapshots · 14 of 200 GB', price: 'Free during the beta', short: 'Free' },
  pr: { name: 'Proton', account: 'alex@hey.com', via: 'Proton login', usage: '$14.20 left · 31 h running this month', price: '$20 a month while running, billed by the second', short: '$20/mo while running' },
};
const MACHINES = {
  agents: { name: 'agents-1', p: 'ib', state: 'running', what: 'universal · 4 vCPUs · 8 GB · 20 GB' },
  review: { name: 'review-box', p: 'ib', state: 'paused', what: 'agent-browser · paused for being idle 12 min ago' },
  proton: { name: 'proton-cloud', p: 'pr', state: 'running', what: 'devbox-3 · 4 vCPUs · 2–64 GB · 50 GB' },
  scratch: { name: 'scratch', p: 'pr', state: 'stopped', what: 'stopped 2 days ago' },
};
const STATES = {
  running: ['Running', 'ok'], paused: ['Paused', 'warn'], stopped: ['Stopped', 'mu'],
  starting: ['Starting', 'ac'], suspended: ['Suspended', 'del'],
};

// Pieces in the app's style (settings_page.rs and ui's buttons).
const tag = (text, color = 'mu') => `<span class="xs none" style="padding:1px 6px;border-radius:4px;background:var(--hov);color:var(--${color})">${text}</span>`;
const obtn = (text, extra = '') => `<span class="btn" style="${extra}">${text}</span>`;
const gbtn = (text) => `<span class="btn" style="border-color:transparent;background:none">${text}</span>`;
const pbtn = (text) => `<span class="btn primary">${text}</span>`;
const dd = (label) => `<span class="row" style="gap:4px;white-space:nowrap;height:26px;padding:0 8px;border-radius:6px;border:1px solid var(--b);background:var(--ed)">${label}<span class="grow"></span>${ic('updown', 'xs mu')}</span>`;
const link = (text) => `<span class="row g1 sm ac none">${text}${ic('external', 'xs')}</span>`;
const spinner = '<span class="spin"></span>';
const toggle = (on) => `<span style="width:34px;height:20px;border-radius:10px;flex:none;display:inline-block;position:relative;vertical-align:middle;background:${on ? '#5a7099' : '#353a44'};border:1px solid ${on ? '#6f86b0' : 'var(--b)'}"><i style="position:absolute;top:2px;${on ? 'right:2px' : 'left:2px'};width:14px;height:14px;border-radius:50%;background:${on ? '#e6e9ee' : '#6b717d'}"></i></span>`;
const card = (rows, style = '') => `<div class="card" style="overflow:visible;position:relative;${style}">${rows.filter(Boolean).map((row, index) => `<div style="position:relative;${index ? 'border-top:1px solid var(--bv)' : ''}">${row}</div>`).join('')}</div>`;
const sectionHead = (title, action = '') => `<div class="row" style="justify-content:space-between;min-height:20px"><span class="sm mu">${title}</span>${action}</div>`;
const headAction = (text, icon = 'plus') => `<span class="row g1 sm mu">${icon ? ic(icon, 'xs') : ''}${text}</span>`;
const section = (title, rows, action = '') => `<div class="col" style="gap:10px">${sectionHead(title, action)}${card(rows)}</div>`;
const field = (label, input, hint = '') => `<div class="col" style="gap:5px"><span class="sm b5">${label}</span>${input}${hint ? `<span class="xs ph" style="line-height:1.4">${hint}</span>` : ''}</div>`;
const input = (text, { ph = false, focus = false } = {}) => `<span class="field ${focus ? 'focus' : ''}"><span class="${ph ? 'ph' : ''}">${text}</span></span>`;
const seg = (items, on) => `<span class="seg">${items.map((item) => `<span class="${item === on ? 'on' : ''}">${item}</span>`).join('')}</span>`;
const piece = (html, w = 720) => `<div class="m" style="width:${w}px">${html}</div>`;
const caption = (text) => `<div class="xs ph" style="margin:0 0 6px">${text}</div>`;
const infoNote = (html) => `<div class="card row" style="padding:10px 14px;gap:10px;align-items:flex-start;background:none"><span class="ac" style="display:inline-flex;margin-top:1px">${ic('info', 'sm')}</span><span class="sm" style="line-height:1.5">${html}</span></div>`;

/** The providers' marks, stand-ins for their logos on a neutral tile. */
function mark(p, size = 28) {
  const inner = p === 'ib'
    ? `<span style="font:700 ${Math.round(size * 0.36)}px/1 'IBM Plex Sans';color:#6eb4bf">ib</span>`
    : p === 'pr'
      ? `<svg viewBox="2 2 28 28" style="width:${Math.round(size * 0.58)}px;height:${Math.round(size * 0.58)}px"><circle cx="16" cy="16" r="12" fill="none" stroke="#dec184" stroke-width="2.4"/><circle cx="16" cy="16" r="5" fill="#dec184"/></svg>`
      : ic(p === 'mac' ? 'laptop' : 'server', 'sm mu');
  return `<span style="width:${size}px;height:${size}px;border-radius:${Math.round(size / 4)}px;flex:none;display:inline-grid;place-items:center;background:${p === 'ib' ? '#2f4548' : p === 'pr' ? '#4a4433' : 'transparent'}">${inner}</span>`;
}
const stateText = (state) => {
  const [label, color] = STATES[state];
  return `<span class="row g1" style="color:var(--${color})">${state === 'starting' ? spinner : `<i class="dot" style="background:var(--${color})"></i>`}${label}</span>`;
};
const statePill = (state) => {
  const [label, color] = STATES[state];
  return `<span class="row xs none" style="gap:5px;padding:2px 8px;border-radius:4px;background:var(--hov);color:var(--${color})">${state === 'starting' ? spinner : `<i class="dot" style="background:var(--${color})"></i>`}${label}</span>`;
};

/** Today's machine row (render_machine_row): icon, name over "how · status", then its controls. */
function machineRow({ icon = 'server', name, sub, right = '', dim = false, lead } = {}) {
  return `<div class="row g3" style="padding:12px 16px;${dim ? 'opacity:.55' : ''}">${lead ?? `<span style="width:28px;display:inline-grid;place-items:center" class="mu">${ic(icon, 'lg')}</span>`}<div class="col grow" style="gap:2px;min-width:0"><span class="trunc">${name}</span><span class="sm mu trunc">${sub}</span></div><span class="row g2 none">${right}</span></div>`;
}
const todayControls = (on = true) => `${ibtn('pencil')}${ibtn('trash')}${toggle(on)}`;
const macRow = () => machineRow({ name: 'This Mac', sub: 'Local · Connected', right: ibtn('restart') });
const devboxRow = () => machineRow({ name: 'Devbox 1', sub: 'SSH devbox1 · Connected', right: todayControls() });

/** A cloud machine's row in the form each option draws. */
function cloudRow(m, { form = 'pill', menu = '', showProvider = true } = {}) {
  const p = PROVIDERS[m.p];
  const where = showProvider ? `${p.name} · ` : '';
  const main = { running: m.p === 'ib' ? 'Pause' : 'Stop', paused: 'Resume', stopped: 'Start', suspended: 'Add Credit' }[m.state];
  if (form === 'line') {
    const icons = { running: m.p === 'ib' ? ibtn('pause') + ibtn('stop') : ibtn('stop'), paused: ibtn('play'), stopped: ibtn('play') }[m.state];
    return machineRow({ name: m.name, lead: mark(m.p), sub: `${where}${m.what} · ${STATES[m.state][0]}`, right: `${icons}${ibtn('pencil')}${ibtn('trash')}${toggle(true)}`, dim: m.state === 'stopped' });
  }
  if (form === 'metrics') {
    const line = m.state === 'running' ? `<span class="row g2 xs mu" style="margin-top:3px"><span>Running 3h 12m</span><span>CPU 12%</span><span>Memory 2.1 of 8 GB</span><span>${m.p === 'pr' ? '$0.09 today' : ''}</span></span>` : '';
    return `<div class="row g3" style="padding:12px 16px;align-items:flex-start">${mark(m.p)}<div class="col grow" style="gap:2px;min-width:0"><span class="row g2"><span class="trunc">${m.name}</span>${statePill(m.state)}</span><span class="sm mu trunc">${where}${m.what}</span>${line}</div><span class="row g2 none">${obtn(main)}${ibtn('more')}${toggle(true)}</span></div>`;
  }
  return `<div class="row g3" style="padding:12px 16px;position:relative">${mark(m.p)}<div class="col grow" style="gap:2px;min-width:0"><span class="row g2"><span class="trunc">${m.name}</span></span><span class="sm mu trunc">${where}${m.what}</span></div><span class="row g2 none">${statePill(m.state)}${obtn(main)}${ibtn('more', menu ? 'on' : '')}${toggle(true)}</span>${menu}</div>`;
}

/** Menus: items are [icon, label, flags] with flags { hl, dis, danger, kb }, 'hr', or ['lbl', text]. */
function menuList(items, style = 'top:44px;right:12px', width = 230) {
  const body = items.map((item) => {
    if (item === 'hr') return '<div class="hr"></div>';
    if (item[0] === 'lbl') return `<div class="lbl">${item[1]}</div>`;
    const [icon, label, flags = {}] = item;
    return `<div class="it ${flags.hl ? 'hl' : ''} ${flags.dis ? 'dis' : ''} ${flags.danger ? 'danger' : ''}">${icon ? ic(icon, 'sm') : '<span style="width:14px"></span>'}<span class="grow">${label}</span>${flags.kb ? `<span class="kb">${flags.kb}</span>` : ''}${flags.sub ? ic('chev-right', 'xs') : ''}</div>`;
  }).join('');
  return `<div class="menu" style="${style};min-width:${width}px">${body}</div>`;
}
const dialog = (title, body, buttons, { w = 460, style = '', sub = '', foot = '' } = {}) => `<div class="modal-back"></div><div class="pop" style="z-index:21;left:50%;top:50%;transform:translate(-50%,-50%);width:${w}px;${style}"><div style="padding:18px 18px 14px"><div class="b6" style="font-size:15px">${title}</div>${sub ? `<div class="sm mu" style="margin-top:4px;line-height:1.5">${sub}</div>` : ''}<div class="col" style="gap:14px;margin-top:14px">${body}</div></div><div class="row g2" style="border-top:1px solid var(--bv);padding:10px 18px;justify-content:flex-end"><span class="xs ph grow" style="line-height:1.4">${foot}</span>${buttons}</div></div>`;

// Settings: the window and the content column (settings_page.rs render_nav), as in the
// screenshots: General … Machines, Storage.
function settingsWindow(contentHtml, { selected = 'Machines', w = 1100, h = 760, overlay = '', extraNav = [] } = {}) {
  const items = [['General', 'settings'], ['Appearance', 'eye'], ['Notifications', 'bell'], ['Agents', 'sparkle'], ['Usage', 'gauge'], ['Skills', 'book'], ['MCP Servers', 'plug'], ['Machines', 'server'], ...extraNav, ['Storage', 'database']];
  const nav = items.map(([label, icon]) => `<div class="row g2" style="height:28px;padding:0 8px;border-radius:6px;margin:0 4px;${label === selected ? 'background:var(--sel)' : ''}">${ic(icon, 'sm mu')}<span>${label}</span></div>`).join('');
  return `<div class="m win" style="width:${w}px;height:${h}px">
    <div class="titlebar">${lights()}${ic('sidebar', 'sm mu')}<span class="row g15" style="font-size:13px">${ic('list', 'xs mu')}All projects${ic('chev-down', 'xs mu')}</span><div class="viewtabs"><span class="on">Agents</span><span>Workspaces</span></div></div>
    <div class="body"><div class="sidebar"><div class="sb-head" style="color:var(--t)"><span class="grow b5" style="color:var(--t)">Settings</span>${ic('x', 'sm mu')}</div><div class="sb-list">${nav}</div><div class="sb-foot">${ic('arrow-left', 'sm')}<span>Back</span></div></div>
    <div class="grow" style="height:100%;overflow:hidden;background:var(--ed);display:flex;justify-content:center"><div style="width:720px">${contentHtml}</div></div></div>${overlay}</div>`;
}
const page = (title, body, right = '') => `<div class="col" style="padding:24px 0 40px;gap:22px"><div class="row" style="justify-content:space-between;height:28px"><span style="font-size:17px">${title}</span>${right}</div>${body}</div>`;

// The groups of option C in topic 1, which later topics build on.
const providerHead = (p, { action } = {}) => {
  const provider = PROVIDERS[p];
  const actions = action ?? (p === 'pr' ? `${link('Add Credit')}${headAction('New Computer')}` : `${link('Dashboard')}${headAction('New Machine')}`);
  return `<div class="row g2" style="min-height:20px">${mark(p, 18)}<span class="sm b5">${provider.name}</span><span class="sm mu trunc">${provider.account} · ${provider.usage}</span><span class="grow"></span><span class="row g3 none">${actions}${ibtn('more', 'sm')}</span></div>`;
};
const providerGroup = (p, machines, opts = {}) => `<div class="col" style="gap:10px">${providerHead(p, opts)}${card(machines.map((m) => cloudRow(m, { showProvider: false, ...opts.row })))}</div>`;
const groupedMachines = ({ row = {}, menuOn } = {}) => [
  section('This Mac and SSH', [macRow(), devboxRow()], headAction('Add Machine')),
  `<div class="col" style="gap:10px">${providerHead('ib')}${card([MACHINES.agents, MACHINES.review].map((m) => cloudRow(m, { showProvider: false, ...row, menu: menuOn === m ? rowMenu(m) : '' })))}</div>`,
  `<div class="col" style="gap:10px">${providerHead('pr')}${card([MACHINES.proton, MACHINES.scratch].map((m) => cloudRow(m, { showProvider: false, ...row, menu: menuOn === m ? rowMenu(m) : '' })))}</div>`,
].join('');
function rowMenu(m) {
  const instabox = m.p === 'ib';
  const items = instabox
    ? [['pause', 'Pause'], ['stop', 'Stop'], 'hr', ['monitor', 'Open Desktop'], ['copy', 'Copy SSH Command'], ['camera', 'Save Snapshot…'], ['globe', 'Publish Port…'], 'hr', ['settings', 'Machine Settings…'], ['x', 'Remove from agentZ'], ['trash', 'Delete Machine…', { danger: true }]]
    : [['stop', 'Stop'], ['restart', 'Restart'], 'hr', ['copy', 'Copy SSH Command'], ['key', 'SSH Keys…'], 'hr', ['settings', 'Machine Settings…'], ['x', 'Remove from agentZ'], ['trash', 'Delete Computer…', { danger: true }]];
  return menuList(items, 'top:44px;right:40px', 220);
}

// 1. Where providers live -----------------------------------------------------------------------
TOPICS.push({
  id: 'place', section: 'Settings', title: 'Where providers live in Settings', size: 'wide', rec: 'C',
  now: 'Settings › Machines lists <b>This Mac</b> and the machines saved by hand, each reached over SSH: its icon, its name over “SSH devbox1 · Connected”, then Edit, Remove and its switch. Add Machine asks for an SSH target and a name. A cloud computer (proton-cloud is a Proton computer) is just an SSH target: agentZ doesn’t know it’s Proton’s, can’t start or stop it, and doesn’t know what it costs.',
  nowImg: 'img/now-machines.png',
  issues: ['Nothing knows a machine came from a provider', 'Starting, stopping and creating cloud machines happens on the vendors’ sites or CLIs', 'No cost or usage anywhere'],
  options: [
    { key: 'A', name: 'One list, accounts below it', from: 't3code ConnectionsSettings',
      desc: 'Cloud machines join the one Machines list, each with its provider’s mark and “Instabox · universal · 4 vCPUs · 8 GB”, its state and its actions. Below the list, a <b>Cloud Accounts</b> section lists each provider account with its usage, and Add Account.',
      good: 'The list stays one list, as t3code’s environments are.', cost: 'A machine and the account it bills to are far apart; the list mixes free and paid machines.',
      mock: () => settingsWindow(page('Machines', [
        section('Machines', [macRow(), devboxRow(), ...[MACHINES.agents, MACHINES.review, MACHINES.proton, MACHINES.scratch].map((m) => cloudRow(m))], headAction('Add Machine')),
        section('Cloud Accounts', [
          machineRow({ name: 'Instabox', lead: mark('ib'), sub: `${PROVIDERS.ib.account} · ${PROVIDERS.ib.usage}`, right: `${link('Dashboard')}${ibtn('more')}` }),
          machineRow({ name: 'Proton', lead: mark('pr'), sub: `${PROVIDERS.pr.account} · ${PROVIDERS.pr.usage}`, right: `${link('Add Credit')}${ibtn('more')}` }),
        ], headAction('Add Account')),
      ].join(''))) },
    { key: 'B', name: 'A Cloud section of its own', from: 'new',
      desc: 'Settings gets <b>Cloud</b> under Machines: one card per provider account, with its usage, its machines and New Machine. Machines lists them too, as rows that say which provider they’re from, so connecting stays in one place.',
      good: 'Accounts and money get their own page.', cost: 'Two pages show the same machines; a new section in the nav.',
      mock: () => settingsWindow(page('Cloud', [
        `<div class="col" style="gap:10px">${sectionHead('Accounts', headAction('Add Account'))}${['ib', 'pr'].map((p) => card([
          machineRow({ name: PROVIDERS[p].name, lead: mark(p), sub: `${PROVIDERS[p].account} · signed in with ${PROVIDERS[p].via === 'API key' ? 'an API key' : 'Proton'}`, right: `${link(p === 'pr' ? 'Add Credit' : 'Dashboard')}${ibtn('more')}` }),
          `<div class="row sm" style="padding:10px 16px;gap:24px"><span class="mu">${PROVIDERS[p].usage}</span><span class="grow"></span><span class="mu">${PROVIDERS[p].price}</span></div>`,
          ...Object.values(MACHINES).filter((m) => m.p === p).map((m) => `<div class="row g3" style="padding:9px 16px 9px 56px"><span class="grow trunc">${m.name}</span>${stateText(m.state)}<span class="sm mu" style="width:70px;text-align:right">${m.p === 'pr' ? (m.state === 'running' ? '$0.09 today' : '—') : ''}</span></div>`),
          `<div class="row g1 sm mu" style="padding:9px 16px 9px 56px">${ic('plus', 'xs')}${p === 'pr' ? 'New Computer' : 'New Machine'}</div>`,
        ])).join('')}</div>`,
      ].join('')), { selected: 'Cloud', extraNav: [['Cloud', 'cloud']] }) },
    { key: 'C', name: 'Machines grouped by provider', from: 'new',
      desc: 'Machines keeps This Mac and SSH machines as today, then one group per provider account. A group’s header has the provider’s mark and name, the account, its usage (Proton’s balance and hours, Instabox’s counts against its limits), the provider’s page (Add Credit, Dashboard) and New Machine; the account’s menu has Sign Out. Its machines are its rows.',
      good: 'Each machine sits under the account it uses and bills to; nothing new in the nav.', cost: 'A longer page with three accounts or more.',
      mock: () => settingsWindow(page('Machines', groupedMachines())) },
    { key: 'D', name: 'Accounts behind the rows only', from: 'new',
      desc: 'No account list at all. Signing in happens the first time a cloud machine is added, and a row’s menu holds its account: “Instabox account: alex@hey.com”, Sign Out. Usage shows on each machine’s own page.',
      good: 'The least UI.', cost: 'No place to see what an account costs or how close it is to its limits; signing out is hidden in a machine’s menu.',
      mock: () => settingsWindow(page('Machines', section('Machines', [macRow(), devboxRow(), ...[MACHINES.agents, MACHINES.review].map((m) => cloudRow(m)), cloudRow(MACHINES.proton, { menu: menuList([['lbl', 'Proton account: alex@hey.com'], ['external', 'Add Credit'], ['logout', 'Sign Out'], 'hr', ['stop', 'Stop'], ['restart', 'Restart'], ['copy', 'Copy SSH Command'], 'hr', ['trash', 'Delete Computer…', { danger: true }]], 'top:44px;right:40px', 230) }), cloudRow(MACHINES.scratch)], headAction('Add Machine')))) },
  ],
});

// 2. Signing in -----------------------------------------------------------------------------------
const instaboxKey = (key = 'ibk_••••••••••••••••7Q2m') => dialog('Sign In to Instabox', [
  field('API key', input(key, { focus: true }), `Make one on Instabox’s API Keys page, or with <code>instabox keys create</code>. A full-access key, or one with <code>machines:write</code>, <code>machines:delete</code> and the <code>images</code> scopes.`),
  `<div class="row">${link('Open API Keys')}</div>`,
].join(''), `${gbtn('Cancel')}${pbtn('Sign In')}`, { sub: 'agentZ keeps the key on this Mac and uses it for your machines there.', foot: '' });
const protonCode = () => dialog('Sign In to Proton', [
  `<div class="col" style="gap:10px;align-items:center;padding:6px 0 2px"><span class="sm mu">Continue in your browser. Check that it shows this code:</span><span style="font:600 22px/1 'IBM Plex Mono';letter-spacing:3px">KQ7M-4XRD</span><span class="row g2 sm mu">${spinner}Waiting for you to approve it…</span></div>`,
  `<div class="row g2" style="justify-content:center">${obtn(`${ic('copy', 'xs')}Copy Link`)}${obtn(`${ic('external', 'xs')}Open Again`)}</div>`,
].join(''), `${gbtn('Cancel')}`, { sub: 'Proton signs agentZ in as its CLI does, with a code. Your Proton password stays in your browser.', foot: 'The code expires in 10 minutes.' });

TOPICS.push({
  id: 'sign-in', section: 'Settings', title: 'Signing in to a provider', size: 'wide', rec: 'A',
  now: 'There’s nothing to sign in to: agentZ reaches a cloud computer as an SSH target, with the key in <code>~/.ssh</code>. Instabox signs programs in with an API key (<code>ibk_…</code>); its CLI signs people in through the browser. Proton has no API keys: its CLI signs in with a code it shows, approved in the browser (<code>proton login</code>), and keeps the token in <code>~/.config/proton/credentials.json</code>. You’re signed in there already.',
  options: [
    { key: 'A', name: 'agentZ’s own sign-in, each the provider’s way', from: 'Instabox docs, Proton CLI',
      desc: 'Add Account (or the first New Machine) asks which provider. <b>Instabox</b> asks for an API key, with a link to its API Keys page and the scopes it needs. <b>Proton</b> opens its sign-in page in the browser and shows the code to compare, with Copy Link and Open Again, as <code>proton login</code> does; it closes by itself once approved. agentZ keeps each credential in its data directory (mode 0600), as API-key accounts keep theirs.',
      good: 'Works without the vendors’ CLIs; signing out in agentZ doesn’t sign the CLI out.', cost: 'You sign in to Proton a second time, beside the CLI.',
      mock: () => frame(`<div class="row" style="height:100%;gap:0">${['ib', 'pr'].map((p) => `<div style="position:relative;flex:1;height:100%;border-right:1px solid var(--b)">${p === 'ib' ? instaboxKey() : protonCode()}</div>`).join('')}</div>`, { w: 1100, h: 460 }) },
    { key: 'B', name: 'A, but use the CLI’s login when there is one', from: 'new',
      desc: 'As A, but when the provider’s CLI is signed in on this Mac (<code>~/.config/proton/credentials.json</code>), agentZ offers it first: “Your Proton CLI is signed in as alex@hey.com. Use It, or Sign In Separately.” Using it means agentZ reads the CLI’s token and stays signed in as long as the CLI is; <code>proton logout</code> signs agentZ out too.',
      good: 'One click when the CLI is set up already.', cost: 'agentZ reads another program’s login, which it doesn’t do for agents; the CLI and agentZ come and go together.',
      mock: () => frame(dialog('Sign In to Proton', [
        `<div class="card row g3" style="padding:12px 14px;background:none">${mark('pr', 28)}<div class="col grow" style="gap:2px"><span>Your Proton CLI is signed in</span><span class="sm mu">alex@hey.com · ~/.config/proton/credentials.json</span></div></div>`,
        `<span class="sm mu" style="line-height:1.5">agentZ would use the CLI’s login, and sign out when it does.</span>`,
      ].join(''), `${gbtn('Cancel')}${obtn('Sign In Separately')}${pbtn('Use It')}`), { w: 620, h: 360 }) },
    { key: 'C', name: 'Inline on the page', from: 'new',
      desc: 'With no account yet, Machines shows a tile per provider (its mark, one line on what it is and costs) with Connect. Connect opens the tile in place: Instabox’s key field, or Proton’s code with “Waiting…”. No dialog.',
      good: 'The providers are visible before you sign in to any.', cost: 'Tiles for providers you may never use stay on the page until you dismiss them.',
      mock: () => settingsWindow(page('Machines', [
        section('Machines', [macRow(), devboxRow()], headAction('Add Machine')),
        `<div class="col" style="gap:10px">${sectionHead('Cloud')}<div class="row g3" style="align-items:stretch">${[
          `<div class="card col" style="flex:1;padding:14px;gap:10px"><div class="row g2">${mark('ib', 22)}<span class="b5">Instabox</span><span class="grow"></span>${tag('Free')}</div><span class="sm mu" style="line-height:1.5">Linux machines for agents: 4 vCPUs, 8 GB, 20 GB. Pause and resume, snapshots, a desktop.</span>${field('API key', input('ibk_…', { ph: true, focus: true }))}<div class="row g2">${link('Make a key')}<span class="grow"></span>${gbtn('Cancel')}${pbtn('Connect')}</div></div>`,
          `<div class="card col" style="flex:1;padding:14px;gap:10px"><div class="row g2">${mark('pr', 22)}<span class="b5">Proton</span><span class="grow"></span>${tag('$20/mo while running')}</div><span class="sm mu" style="line-height:1.5">One persistent Linux computer: 4 vCPUs, 2–64 GB, 50 GB, Ubuntu 24.04 with Claude Code and Codex.</span><span class="grow"></span><div class="row g2"><span class="grow"></span>${obtn('Connect')}</div></div>`,
        ].join('')}</div></div>`,
      ].join(''))) },
  ],
});

// 3. Adding a machine -------------------------------------------------------------------------------
const instaboxForm = () => [
  field('Account', dd(`${mark('ib', 16)}&nbsp;Instabox · alex@hey.com`)),
  `<div class="row g3"><div style="flex:1">${field('Start from', `<div class="row g2">${seg(['Template', 'Snapshot'], 'Template')}<span class="grow">${dd('universal — Docker, Python, Go, Rust')}</span></div>`)}</div></div>`,
  field('Name', input('agents-2'), 'Lowercase letters, digits and -.'),
  `<div class="row g3"><div style="flex:1">${field('Pause or delete it after', dd('Never'))}</div><div style="flex:1">${field('When idle', dd('Pause after 30 min'), 'Instabox’s default; changing it needs manual lifecycle on the account.')}</div></div>`,
].join('');
const protonForm = () => [
  field('Account', dd(`${mark('pr', 16)}&nbsp;Proton · alex@hey.com`)),
  field('Name', input('scratch-2')),
  `<div class="card col" style="padding:10px 14px;gap:4px;background:none"><span class="row g2 sm"><span class="b5">4 shared vCPUs · 2–64 GB memory · 50 GB · Ubuntu 24.04</span></span><span class="sm mu">$20 a month while it runs, billed by the second from your Cube balance ($14.20 left). Stopping it stops the charge.</span></div>`,
].join('');

TOPICS.push({
  id: 'add', section: 'Machines', title: 'Adding a cloud machine', size: 'wide', rec: 'B',
  now: 'Add Machine is one dialog: an SSH target (“user@host, or a Host from ~/.ssh/config”) and a name, with “agentZ installs its server in ~/.agentz there, and nothing else.” A cloud machine has to be made on the vendor’s site or CLI first, then added by its SSH host.',
  nowImg: 'img/now-add-machine.png',
  options: [
    { key: 'A', name: 'One dialog, a switch at the top', from: 'today’s dialog',
      desc: 'Add Machine gets <b>SSH · Instabox · Proton</b> at the top; the form below changes. SSH is today’s form. A provider’s form: the account, what to start from (Instabox: a template or one of your snapshots), the name, and Instabox’s timeout and idle pause; the footer says what it costs or how many machines are left. Create Machine makes it, then the row shows it starting, installing agentZ’s server and connecting.',
      good: 'Today’s dialog, extended.', cost: 'A busy dialog; the switch hides what each provider is until you click it.',
      mock: () => settingsWindow(page('Machines', groupedMachines()), { overlay: dialog('Add Machine', `${seg(['SSH', 'Instabox', 'Proton'], 'Instabox')}${instaboxForm()}`, `${gbtn('Cancel')}${pbtn('Create Machine')}`, { w: 540, foot: 'Free: 2 of 5 machines used.' }) }) },
    { key: 'B', name: 'Pick the kind, then its form', from: 'new',
      desc: 'Add Machine first lists the kinds, each with one line and its price: <b>SSH machine</b> (“A machine you already reach with ssh”), <b>Instabox</b> (“Free during the beta · 4 vCPUs · 8 GB · pauses when idle”), <b>Proton</b> (“$20 a month while running · 4 vCPUs · 2–64 GB · 50 GB”), and <b>Add Existing…</b> for machines an account already has. Picking one shows its form, as in A, with Back.',
      good: 'What each provider is and costs shows before choosing; each form stays short.', cost: 'One more click.',
      mock: () => frame(`<div class="row" style="height:100%">${[
        `<div style="position:relative;flex:1;height:100%;border-right:1px solid var(--b)">${dialog('Add Machine', [
          ['server', null, 'SSH machine', 'A machine you already reach with ssh.', ''],
          [null, 'ib', 'Instabox', 'Linux machines for agents. Pause and resume, snapshots, a desktop.', 'Free · 2 of 5 used'],
          [null, 'pr', 'Proton', 'A persistent Linux computer with Claude Code and Codex installed.', '$20/mo while running'],
          ['link', null, 'Add Existing…', 'A machine an account already has.', ''],
        ].map(([icon, p, name, line, price], index) => `<div class="card row g3" style="padding:10px 12px;${index === 1 ? 'border-color:var(--bf);background:var(--hov)' : 'background:none'}">${p ? mark(p, 26) : `<span style="width:26px;display:inline-grid;place-items:center" class="mu">${ic(icon, 'lg')}</span>`}<div class="col grow" style="gap:2px;min-width:0"><span>${name}</span><span class="sm mu">${line}</span></div><span class="xs mu none">${price}</span></div>`).join(''), gbtn('Cancel'), { w: 470 })}</div>`,
        `<div style="position:relative;flex:1;height:100%">${dialog(`<span class="row g2"><span class="mu">${ic('arrow-left', 'sm')}</span>${mark('ib', 18)}New Instabox Machine</span>`, instaboxForm(), `${gbtn('Cancel')}${pbtn('Create Machine')}`, { w: 500, foot: 'Free: 2 of 5 machines used.' })}</div>`,
      ].join('')}</div>`, { w: 1120, h: 560 }) },
    { key: 'C', name: 'A menu on Add Machine', from: 'new',
      desc: 'Add Machine opens a menu: SSH Machine…, New Instabox Machine…, New Proton Computer…, then Add Existing… with each account’s machines not in agentZ yet. Each item opens a dialog of its own. A provider group’s New Machine opens its dialog directly.',
      good: 'Each dialog is about one thing; existing machines are a click away.', cost: 'The menu says less than B’s list about what each provider is.',
      mock: () => settingsWindow(page('Machines', groupedMachines()), { overlay: `<div style="position:absolute;inset:0;z-index:20">${menuList([['server', 'SSH Machine…'], 'hr', ['lbl', 'New'], ['cloud', 'Instabox Machine…'], ['cloud', 'Proton Computer…'], 'hr', ['lbl', 'Add existing'], ['link', 'Instabox: old-box'], ['link', 'Proton: devbox-2']], 'top:128px;right:200px', 230)}</div>` }) },
  ],
});

// 4. What starting costs ----------------------------------------------------------------------------
TOPICS.push({
  id: 'cost', section: 'Machines', title: 'Starting something that costs money', size: 'wide', rec: 'B',
  now: 'Nothing in agentZ costs money by itself. Proton bills $20 per 730 running hours from the moment a computer is created or started; its CLI asks first: “This computer costs $20 per 730 running hours (about one month), billed per running second from your shared Cube balance. Stopping it stops the charge. Continue?” (skipped with <code>--yes</code>). Instabox is free during its beta.',
  options: [
    { key: 'A', name: 'The price on the form and the button', from: 'Proton dashboard',
      desc: 'The create form says the price and the balance, and the buttons say it: “Create · $20/mo while running”, and Start shows the price in its tooltip. Nothing asks again.',
      good: 'No extra step.', cost: 'A Start in a row menu or a thread’s banner starts billing with one click.',
      mock: () => frame(dialog(`<span class="row g2">${mark('pr', 18)}New Proton Computer</span>`, protonForm(), `${gbtn('Cancel')}${pbtn('Create · $20/mo while running')}`, { w: 520 }), { w: 640, h: 420 }) },
    { key: 'B', name: 'Ask each time, in Proton’s words', from: 'Proton CLI',
      desc: 'Creating, starting or restarting a Proton computer asks, as its CLI does: “proton-cloud costs $20 per 730 running hours, billed by the second from your Cube balance ($14.20 left). Stopping it stops the charge.” with Cancel and Start. A provider that’s free never asks.',
      good: 'What the vendor itself does; no surprise charges.', cost: 'A dialog every time, even for a computer you start daily.',
      mock: () => frame(dialog('Start proton-cloud?', '<span class="sm" style="line-height:1.55">It costs <b>$20 per 730 running hours</b>, billed by the second from your Cube balance ($14.20 left). Stopping it stops the charge.</span>', `${gbtn('Cancel')}${pbtn('Start')}`, { w: 440 }), { w: 600, h: 300 }) },
    { key: 'C', name: 'Ask the first time per account', from: 'new',
      desc: 'As B, with “Don’t ask again for this account”. Once ticked, the price shows on the buttons as in A. Settings › Machines › the account’s menu has Ask Before Charging to turn it back on.',
      good: 'One confirmation, then out of the way.', cost: 'A setting to remember.',
      mock: () => frame(dialog('Start proton-cloud?', `<span class="sm" style="line-height:1.55">It costs <b>$20 per 730 running hours</b>, billed by the second from your Cube balance ($14.20 left). Stopping it stops the charge.</span><span class="row g2 sm"><span style="width:14px;height:14px;border:1px solid var(--b);border-radius:3px;display:inline-block"></span>Don’t ask again for this account</span>`, `${gbtn('Cancel')}${pbtn('Start')}`, { w: 440 }), { w: 600, h: 320 }) },
  ],
});

// 5. A cloud machine's row ---------------------------------------------------------------------------
TOPICS.push({
  id: 'row', section: 'Machines', title: 'A cloud machine’s row', size: 'wide', rec: 'B',
  now: 'A row is the machine’s icon, its name over “SSH proton · Connected” (or Connecting, Uploading agentz-server · 40%, Reconnecting: the error), then Update Server when there’s one, Retry Now while it’s offline, Edit, Remove and its switch. A stopped Proton computer reads “Reconnecting” with an SSH error.',
  nowImg: 'img/now-machines.png',
  options: [
    { key: 'A', name: 'Today’s row, state in its line', from: 'today’s row',
      desc: 'The provider’s mark for the icon; the line says “Instabox · universal · 4 vCPUs · 8 GB · Running”; icon buttons by state (Pause and Stop while running, Start while stopped, Resume while paused) beside Edit, Remove and the switch. The connection’s own status (Connecting, Uploading…) replaces the state while it’s in progress.',
      good: 'Closest to today.', cost: 'Icon buttons need tooltips to tell Pause from Stop; the rest of a provider’s actions have nowhere to go.',
      mock: () => piece(`<div style="padding:16px">${card([cloudRow(MACHINES.agents, { form: 'line' }), cloudRow(MACHINES.review, { form: 'line' }), cloudRow(MACHINES.proton, { form: 'line' }), cloudRow(MACHINES.scratch, { form: 'line' })])}</div>`, 720) },
    { key: 'B', name: 'A state badge, one action, a menu', from: 't3code EnvironmentRow',
      desc: 'The provider’s mark, the name over what it is (template and size), then a badge with the provider’s state (Running, Paused, Stopped, Starting, Suspended), one button for the likely action (Pause, Resume, Start, Stop; Add Credit when Proton suspended it), and a menu with the rest: Stop or Restart, Open Desktop, Copy SSH Command, Save Snapshot…, Publish Port…, Machine Settings…, Remove from agentZ, Delete Machine…. Connecting shows under the name as today.',
      good: 'Every provider action has a place, and the state reads at a glance.', cost: 'A wider row than today’s.',
      mock: () => piece(`<div style="padding:16px 16px 230px">${card([cloudRow(MACHINES.agents, { menu: rowMenu(MACHINES.agents) }), cloudRow(MACHINES.review), cloudRow(MACHINES.proton), cloudRow(MACHINES.scratch)])}</div>`, 720) },
    { key: 'C', name: 'B, with live numbers', from: 'Instabox dashboard',
      desc: 'B’s row with a third line while it runs: how long it’s been running, CPU and memory (Instabox’s samples every 30 s), and for Proton what it has cost today.',
      good: 'You see what each machine is doing and costing without opening it.', cost: 'Taller rows; a read every 30 s per machine while Settings is open (Proton has no metrics, so only time and cost).',
      mock: () => piece(`<div style="padding:16px">${card([cloudRow(MACHINES.agents, { form: 'metrics' }), cloudRow(MACHINES.review, { form: 'metrics' }), cloudRow(MACHINES.proton, { form: 'metrics' }), cloudRow(MACHINES.scratch, { form: 'metrics' })])}</div>`, 720) },
  ],
});

// 6. A machine's page --------------------------------------------------------------------------------
const kv = (label, value, right = '') => `<div class="row g3" style="padding:11px 16px"><span class="sm mu" style="width:150px;flex:none">${label}</span><span class="grow sm">${value}</span>${right}</div>`;
const spark = (points, color = 'var(--ac)') => `<svg viewBox="0 0 120 28" style="width:120px;height:28px;flex:none"><polyline points="${points}" fill="none" stroke="${color}" stroke-width="1.5"/></svg>`;
function machinePage(tab, body, { m = MACHINES.agents, tabs = ['Overview', 'Snapshots', 'Ports', 'Desktop', 'Vault', 'SSH'] } = {}) {
  const tabRow = tabs.map((name) => `<span style="padding:0 0 9px;${name === tab ? 'color:var(--t);border-bottom:2px solid var(--ac);margin-bottom:-1px' : 'color:var(--mu)'}">${name}</span>`).join('');
  const p = PROVIDERS[m.p];
  return `<div class="col" style="padding:24px 0 28px;gap:20px">
    <div class="col" style="gap:22px">
      <span class="row g15 sm mu">${ic('arrow-left', 'xs')}Machines</span>
      <div class="row g3">${mark(m.p, 46)}
        <div class="col grow" style="gap:4px"><span class="row g2"><span style="font-size:17px;font-weight:600">${m.name}</span>${statePill(m.state)}</span><span class="sm mu">${p.name} · ${m.p === 'ib' ? 'm_q4mz7tk2bx5rn3wd6vp2hj4sye · universal 2026-10-13' : 'devbox-3 · ssh.proton.computer:22021'}</span></div>
        <span class="row g2 none">${m.p === 'ib' ? obtn(`${ic('pause', 'xs')}Pause`) : obtn(`${ic('stop', 'xs')}Stop`)}${m.p === 'ib' ? obtn(`${ic('monitor', 'xs')}Open Desktop`) : ''}<span class="ibtn" style="border:1px solid var(--b);width:28px;height:28px">${ic('more', 'sm')}</span></span></div>
      <div class="row" style="gap:22px;border-bottom:1px solid var(--b);font-size:14px">${tabRow}</div>
    </div>${body}</div>`;
}
const overviewIb = () => [
  card([
    kv('Size', '4 vCPUs · 8 GB memory · 20 GB storage'),
    kv('CPU', '12% now', spark('0,22 10,20 20,24 30,14 40,18 50,9 60,15 70,19 80,12 90,16 100,20 110,17 120,18')),
    kv('Memory', '2.1 of 8 GB', spark('0,20 20,19 40,18 60,16 80,15 100,15 120,14', 'var(--ok)')),
    kv('Running this month', '31 h 12 min · 125 vCPU-hours'),
  ]),
  section('Lifecycle', [
    kv('Pause or delete after', 'Never', dd('Never')),
    kv('When idle', 'Pause after 30 minutes', dd('30 min')),
    kv('Vault', 'Gets your Vault items when it boots', toggle(true)),
    kv('Labels', `${tag('project=storefront')} ${tag('owner=alex')}`, ibtn('pencil')),
  ]),
].join('');

TOPICS.push({
  id: 'page', section: 'Machines', title: 'Everything else a provider offers', size: 'wide', rec: 'A',
  now: 'A saved machine has only Edit (its target and name) and Remove. Instabox also has snapshots, public ports, a desktop with share links, a Vault of secrets, SSH tokens for other terminals, CPU and memory samples, running time, timeouts, idle pause and labels. Proton has its SSH keys and billing. Today they’re all on the vendors’ dashboards and CLIs.',
  options: [
    { key: 'A', name: 'A page per machine, with tabs', from: 'the agent page (render_agent_page)',
      desc: 'Clicking a cloud machine opens its page, as an agent’s does: back to Machines, its mark, name and state badge, its id, Pause and Open Desktop, and tabs for what its provider has. <b>Overview</b>: size, CPU and memory, running time, timeout, idle pause, Vault and labels. <b>Snapshots</b>: those saved from it, Save Snapshot. <b>Ports</b>: published URLs, Publish Port. <b>Desktop</b>: Open Desktop (a control link) and Copy View Link (view only, for an hour). <b>Vault</b>: what it gets, Push Now. <b>SSH</b>: Copy SSH Command (a 7-day token for any terminal), Revoke All. A Proton computer’s page has Overview (size, running time, cost) and SSH Keys.',
      good: 'Room for everything, and only what the provider has.', cost: 'The most to build; another level in Settings.',
      mock: () => settingsWindow(machinePage('Overview', overviewIb()), { h: 820 }) },
    { key: 'B', name: 'All in the row’s menu, as small dialogs', from: 'new',
      desc: 'No page: the row’s menu has every action, and each opens a small dialog: Save Snapshot… (a name), Publish Port… (a port and a name, then its URL to copy), Machine Settings… (timeout, idle pause, Vault, labels), Copy SSH Command, Open Desktop. Lists (snapshots, ports) show inside their dialogs.',
      good: 'Nothing new to navigate.', cost: 'No place to see a machine’s ports, snapshots and numbers together; long menus.',
      mock: () => settingsWindow(page('Machines', groupedMachines()), { overlay: dialog('Publish a Port of agents-1', [
        `<div class="row g3"><div style="width:110px">${field('Port', input('3000', { focus: true }))}</div><div class="grow">${field('Name', input('Web app'))}</div></div>`,
        `<div class="col" style="gap:6px"><span class="sm b5">Published</span>${card([`<div class="row g2 sm" style="padding:8px 12px"><span class="grow trunc mono-font">https://8080-q4mz7tk2bx5rn3wd6vp2hj4sye.instabox.run</span><span class="mu">Report</span>${ibtn('copy', 'sm')}${ibtn('x', 'sm')}</div>`])}</div>`,
      ].join(''), `${gbtn('Cancel')}${pbtn('Publish')}`, { w: 520, sub: 'Anyone with the URL reaches the whole service until you unpublish it. A visit wakes the machine if it paused for being idle.' }) }) },
    { key: 'C', name: 'The row opens in place', from: 'new',
      desc: 'Clicking a cloud machine’s row opens it inside the list: under its row, its numbers on one line, then short lists for snapshots and ports with their buttons, and Desktop, SSH and Settings as buttons. Other rows move down; one row is open at a time.',
      good: 'Everything stays on the Machines page.', cost: 'A long page while open; less room than A for the Vault and labels.',
      mock: () => settingsWindow(page('Machines', [
        section('This Mac and SSH', [macRow(), devboxRow()], headAction('Add Machine')),
        `<div class="col" style="gap:10px">${providerHead('ib')}${card([
          cloudRow(MACHINES.agents, { showProvider: false }),
          `<div class="col" style="padding:4px 16px 14px 56px;gap:12px"><span class="row g3 sm mu"><span>CPU 12%</span><span>Memory 2.1 of 8 GB</span><span>Running 31 h this month</span><span>Never times out · pauses after 30 min idle</span></span>
            <div class="row g3" style="align-items:flex-start"><div class="col grow" style="gap:6px"><span class="row sm"><span class="b5 grow">Snapshots</span><span class="mu row g1">${ic('plus', 'xs')}Save</span></span><span class="row g2 sm"><span class="grow">dev-base</span><span class="mu">3.1 GB · Oct 12</span></span></div><div class="col grow" style="gap:6px"><span class="row sm"><span class="b5 grow">Ports</span><span class="mu row g1">${ic('plus', 'xs')}Publish</span></span><span class="row g2 sm"><span class="grow">3000 · Web app</span>${ibtn('copy', 'sm')}</span></div></div>
            <div class="row g2">${obtn(`${ic('monitor', 'xs')}Open Desktop`)}${obtn(`${ic('copy', 'xs')}Copy SSH Command`)}${obtn(`${ic('settings', 'xs')}Settings…`)}</div></div>`,
          cloudRow(MACHINES.review, { showProvider: false }),
        ])}</div>`,
      ].join('')), { h: 820 }) },
  ],
});

// 7. Snapshots ----------------------------------------------------------------------------------------
const snapshotsList = () => card([
  `<div class="row g3" style="padding:11px 16px">${ic('camera', 'sm mu')}<div class="col grow" style="gap:2px"><span>dev-base</span><span class="sm mu">From agents-1 · software, home and workspace · 3.1 GB · Oct 12</span></div>${obtn('New Machine')}${ibtn('more')}</div>`,
  `<div class="row g3" style="padding:11px 16px">${ic('camera', 'sm mu')}<div class="col grow" style="gap:2px"><span class="row g2">claude-logged-in ${tag('Saving…', 'ac')}</span><span class="sm mu">From review-box · software only</span></div>${obtn('New Machine')}${ibtn('more')}</div>`,
]);
TOPICS.push({
  id: 'snapshots', section: 'Machines', title: 'Instabox snapshots', size: 'wide', rec: 'B',
  now: 'Not in agentZ. An Instabox snapshot saves a machine’s software and its <code>/data</code> (home, workspace, saved sign-ins), or its software only; new machines start from it. Snapshots belong to the account (10 of them), not to the machine they came from, and stay when it’s deleted. Saving stops a running machine first, unless the server can save it while it runs.',
  options: [
    { key: 'A', name: 'On the machine they came from', from: 'Instabox’s machine menu',
      desc: 'The machine’s page (topic 6 A) has a Snapshots tab: those saved from it, Save Snapshot… (name, software only or with home and workspace), and per snapshot New Machine and Delete. Add Machine’s Start from › Snapshot lists all of them.',
      good: 'Saving one is next to the machine it saves.', cost: 'A snapshot whose machine was deleted has no page to show on.',
      mock: () => settingsWindow(machinePage('Snapshots', `<div class="col" style="gap:10px">${sectionHead('Saved from agents-1', headAction('Save Snapshot…'))}${snapshotsList()}</div>`)) },
    { key: 'B', name: 'Under the account', from: 'Instabox’s Snapshots page',
      desc: 'The Instabox group in Machines lists the account’s snapshots under its machines (collapsed to a “2 snapshots” line until opened), with New Machine and Delete; a machine’s menu or page has Save Snapshot…. Add Machine’s Start from › Snapshot lists them too.',
      good: 'Where they belong: the account, with its limit (1 of 10) in the header.', cost: 'Saving starts from the machine; finding the snapshot means looking under the account.',
      mock: () => settingsWindow(page('Machines', [
        section('This Mac and SSH', [macRow(), devboxRow()], headAction('Add Machine')),
        `<div class="col" style="gap:10px">${providerHead('ib')}${card([cloudRow(MACHINES.agents, { showProvider: false }), cloudRow(MACHINES.review, { showProvider: false })])}<div class="sm mu row g1" style="margin-top:4px">${ic('chev-down', 'xs')}2 snapshots</div>${snapshotsList()}</div>`,
      ].join(''))) },
    { key: 'C', name: 'Only where they’re used', from: 'new',
      desc: 'No list: Save Snapshot… is in a machine’s menu, and snapshots show only in Add Machine’s Start from › Snapshot, each with Delete on hover.',
      good: 'The least UI.', cost: 'No way to see or clean up snapshots without starting to create a machine.',
      mock: () => frame(dialog(`<span class="row g2">${mark('ib', 18)}New Instabox Machine</span>`, [
        field('Start from', `<div class="row g2">${seg(['Template', 'Snapshot'], 'Snapshot')}<span class="grow">${dd('dev-base · 3.1 GB · Oct 12')}</span></div>`),
        `<div class="card" style="padding:4px;background:var(--panel)">${['dev-base · from agents-1 · 3.1 GB', 'claude-logged-in · from review-box · saving…'].map((s, i) => `<div class="row g2 sm" style="height:28px;padding:0 8px;border-radius:5px;${i === 0 ? 'background:var(--hov)' : ''}">${ic('camera', 'xs mu')}<span class="grow">${s}</span>${i === 0 ? ibtn('trash', 'sm') : ''}</div>`).join('')}</div>`,
        field('Name', input('agents-2')),
      ].join(''), `${gbtn('Cancel')}${pbtn('Create Machine')}`, { w: 520 }), { w: 660, h: 440 }) },
  ],
});

// 8. Removing and deleting -----------------------------------------------------------------------------
TOPICS.push({
  id: 'delete', section: 'Machines', title: 'Removing or deleting a cloud machine', size: 'wide', rec: 'A',
  now: 'Remove… takes a machine out of agentZ; the machine itself and agentZ’s server there stay. Proton’s own Delete asks you to type the computer’s name (“This permanently deletes this computer and its disk. Its files cannot be recovered.”).',
  options: [
    { key: 'A', name: 'Remove and Delete, apart', from: 'Proton dashboard',
      desc: 'The menu has <b>Remove from agentZ</b> (it keeps running at the provider, and can be added back) and <b>Delete Machine…</b>: “This deletes agents-1 at Instabox with its disk. Its threads, terminals and files there can’t be recovered.” with its name to type, as Proton asks. Snapshots saved from it stay.',
      good: 'Nobody deletes a machine by removing it.', cost: 'Two items that sound alike.',
      mock: () => settingsWindow(page('Machines', groupedMachines()), { overlay: dialog('Delete agents-1?', `<span class="sm" style="line-height:1.55">This deletes it at Instabox, with its disk. Its threads, terminals and files there can’t be recovered. Snapshots saved from it stay.</span>${field('Type its name to delete it', input('agents-1', { focus: true }))}`, `${gbtn('Cancel')}<span class="btn" style="background:var(--del);border-color:var(--del);color:#1b1f26">Delete Machine</span>`, { w: 440 }) }) },
    { key: 'B', name: 'One Remove, with a box to delete', from: 'new',
      desc: 'One Remove…: “Remove agents-1 from agentZ?” with “Also delete it at Instabox, with its disk” unticked. Ticking it asks for the name.',
      good: 'One item in the menu.', cost: 'Deleting hides behind a checkbox.',
      mock: () => settingsWindow(page('Machines', groupedMachines()), { overlay: dialog('Remove agents-1?', `<span class="sm" style="line-height:1.55">It keeps running at Instabox, and you can add it back.</span><span class="row g2 sm"><span style="width:14px;height:14px;border:1px solid var(--b);border-radius:3px;display:inline-block"></span>Also delete it at Instabox, with its disk</span>`, `${gbtn('Cancel')}${pbtn('Remove')}`, { w: 440 }) }) },
  ],
});

// 9. Machines an account already has ---------------------------------------------------------------------
TOPICS.push({
  id: 'existing', section: 'Machines', title: 'Machines an account already has', size: 'wide', rec: 'B',
  now: 'proton-cloud is saved as the SSH target <code>proton</code>, an alias in <code>~/.ssh/config</code> for <code>ssh.proton.computer:22021</code>: that’s the Proton computer devbox-3. Machines made on a vendor’s site or CLI aren’t in agentZ until they’re added by hand.',
  options: [
    { key: 'A', name: 'Pick them after signing in', from: 'new',
      desc: 'Signing in lists the account’s machines with a box each, all ticked; one already saved over SSH at the same host and port says “Linked to proton-cloud” and is linked rather than added twice. Later ones are in Add Machine › Add Existing….',
      good: 'You choose what agentZ connects to.', cost: 'One more step after signing in.',
      mock: () => frame(dialog(`<span class="row g2">${mark('pr', 18)}Signed in to Proton</span>`, `<span class="sm mu">alex@hey.com has 2 computers. Add them to agentZ?</span>${card([
        `<div class="row g3" style="padding:10px 14px">${ic('check', 'sm ac')}<div class="col grow" style="gap:2px"><span>devbox-3</span><span class="sm mu">Running · already saved as proton-cloud: linked</span></div>${tag('Linked', 'ok')}</div>`,
        `<div class="row g3" style="padding:10px 14px">${ic('check', 'sm ac')}<div class="col grow" style="gap:2px"><span>scratch</span><span class="sm mu">Stopped</span></div></div>`,
      ])}`, `${gbtn('Skip')}${pbtn('Add 1 Computer')}`, { w: 480 }), { w: 620, h: 380 }) },
    { key: 'B', name: 'All of them, always', from: 'new',
      desc: 'An account’s group lists all its machines, as the provider has them, refreshed every minute while Settings is open; new ones appear by themselves, and the one saved over SSH is linked and moves into the group. A machine’s switch (off for new ones) decides whether agentZ connects to it.',
      good: 'agentZ always matches the provider.', cost: 'Machines you never use with agentZ show anyway (switched off).',
      mock: () => settingsWindow(page('Machines', groupedMachines())) },
    { key: 'C', name: 'Only by hand', from: 'today',
      desc: 'Nothing is listed by itself: Add Machine › Add Existing… lists the account’s machines not in agentZ. proton-cloud stays a plain SSH machine until it’s added again from Proton.',
      good: 'Nothing changes by itself.', cost: 'proton-cloud stays outside the Proton group, without start, stop or cost.',
      mock: () => settingsWindow(page('Machines', [section('This Mac and SSH', [macRow(), devboxRow(), machineRow({ name: 'proton-cloud', sub: 'SSH proton · Connected', right: todayControls() })], headAction('Add Machine')), `<div class="col" style="gap:10px">${providerHead('pr')}${card([cloudRow(MACHINES.scratch, { showProvider: false })])}</div>`].join(''))) },
  ],
});

// 10. A stopped or paused machine elsewhere ------------------------------------------------------------
const threadFrame = (banner, { toast = '', input = 'proton-cloud is stopped' } = {}) => frame(`<div class="col" style="height:100%"><div class="row g2 sm mu" style="height:36px;padding:0 16px;border-bottom:1px solid var(--b)"><span class="glyph">✻</span><span style="color:var(--t)">Fix the flaky login test</span><span class="grow"></span>${mark('pr', 16)}<span>proton-cloud</span></div>${banner}<div style="padding:16px 60px;display:flex;flex-direction:column;gap:12px;opacity:.6"><div style="align-self:flex-end;background:var(--hov);padding:8px 12px;border-radius:8px">Find why login.spec.ts fails one run in five.</div><div style="line-height:22px">The test waits on a fixed 200 ms timeout before the session cookie is set…</div></div><div class="grow"></div><div style="border-top:1px solid var(--b);padding:12px 60px" class="ph">${input}</div></div>${toast}`, { w: 760, h: 420 });
const bannerRow = (html, buttons) => `<div class="row g3" style="margin:12px 16px 0;padding:10px 14px;border:1px solid var(--b);border-radius:8px;background:var(--panel)">${ic('power', 'sm mu')}<span class="grow sm" style="line-height:1.45">${html}</span>${buttons}</div>`;
TOPICS.push({
  id: 'stopped', section: 'Everywhere', title: 'Opening something on a stopped machine', size: 'wide', rec: 'C',
  now: 'A machine that’s unreachable shows as offline everywhere: dimmed, its input disabled, “Reconnecting” with an SSH error, and agentZ keeps retrying. A Proton computer that’s stopped stays that way until someone starts it on Proton’s site. An Instabox machine that paused for being idle wakes by itself when agentZ reconnects over SSH; one paused by hand or by its timeout waits for Resume.',
  options: [
    { key: 'A', name: 'Always ask', from: 'new',
      desc: 'The thread, a workspace or New Thread on a stopped or paused machine says so where the input would be, with its button: “proton-cloud is stopped. Start · $20/mo while running” or “review-box is paused. Resume”. Nothing starts by itself; agentZ doesn’t dial a machine its provider says is stopped.',
      good: 'Nothing happens you didn’t ask for.', cost: 'A click before every free resume too.',
      mock: () => threadFrame(bannerRow('<b>proton-cloud is stopped.</b> Its threads, terminals and agents come back when it starts.', `${obtn('Start')}<span class="xs mu">$20/mo while running</span>`)) },
    { key: 'B', name: 'Start it when it’s needed', from: 'new',
      desc: 'Opening a thread or workspace on a stopped or paused machine starts it, with “Starting proton-cloud…” where the input would be until it’s connected, and a toast with Cancel. Sending a message or starting a task there does the same.',
      good: 'No extra click; the machine feels always there.', cost: 'Opening an old thread can start a paid computer you didn’t mean to.',
      mock: () => threadFrame(bannerRow(`<span class="row g2">${spinner}<b>Starting proton-cloud…</b> then connecting.</span>`, gbtn('Cancel')), { input: 'Starting proton-cloud…' }) },
    { key: 'C', name: 'Start free ones, ask for paid ones', from: 'new',
      desc: 'B where it costs nothing (resuming or starting an Instabox machine), A where it costs money (starting a Proton computer, with the price, and topic 4’s confirmation if picked).',
      good: 'Free things just work; paid things ask.', cost: 'Two behaviors to learn.',
      mock: () => frame(`<div class="row" style="height:100%;gap:12px;padding:12px;align-items:flex-start">${[
        `<div class="col" style="flex:1;gap:6px">${caption('Instabox: resumes by itself')}<div class="card" style="overflow:hidden">${bannerRow(`<span class="row g2">${spinner}<b>Resuming review-box…</b></span>`, gbtn('Cancel'))}<div style="height:12px"></div></div></div>`,
        `<div class="col" style="flex:1;gap:6px">${caption('Proton: asks')}<div class="card" style="overflow:hidden">${bannerRow('<b>proton-cloud is stopped.</b>', `${obtn('Start')}<span class="xs mu">$20/mo</span>`)}<div style="height:12px"></div></div></div>`,
      ].join('')}</div>`, { w: 860, h: 140 }) },
  ],
});

// 11. Idle machines ------------------------------------------------------------------------------------
const toastBox = (title, body, buttons = '') => `<div class="toast" style="position:relative;width:360px"><div class="row g2"><span class="b5 grow">${title}</span>${ic('x', 'xs mu')}</div><div class="sm mu" style="margin-top:4px;line-height:1.45">${body}</div>${buttons ? `<div class="row g2" style="margin-top:8px;justify-content:flex-end">${buttons}</div>` : ''}</div>`;
TOPICS.push({
  id: 'idle', section: 'Everywhere', title: 'Idle machines and running costs', size: 'medium', type: 'multi', rec: 'ACD',
  now: 'agentZ keeps a machine awake while its agents work (<code>caffeinate</code>, <code>systemd-inhibit</code>), and stops idle agents after 30 minutes. Cloud providers have their own rules: an Instabox machine pauses after 30 idle minutes (what counts as idle isn’t documented), and changing that needs “manual lifecycle” on the account; a Proton computer runs, and bills, until it’s stopped, and is suspended when the balance runs out.',
  options: [
    { key: 'A', name: 'Keep Instabox awake while agents work', from: 'agentZ’s keep-awake',
      desc: 'While an agent on an Instabox machine works, its server holds off the idle pause, as it holds off sleep: through the provider, by turning the idle pause off and back on when the work ends. On an account without manual lifecycle that’s refused; agentZ then says on the machine’s page that turns may stop when Instabox pauses it.',
      good: 'A long turn isn’t frozen halfway.', cost: 'Only works where the account allows it, and Instabox’s idle rule isn’t documented.',
      mock: () => piece(`<div style="padding:16px">${card([kv('When idle', 'Pause after 30 minutes, <span class="okc">but not while agents work</span>', dd('30 min'))])}</div>`, 560) },
    { key: 'B', name: 'Stop or pause machines left idle', from: 'new',
      desc: 'Per machine, Stop when idle (Proton) or Pause when idle (Instabox): after no agent has worked and no thread, terminal or workspace on it has been open for the time picked (Never, 30 min, 1 hour, 4 hours). Never is the default.',
      good: 'Saves Proton money when you forget it running.', cost: 'Stopping ends its terminals’ programs; anything left running outside agentZ stops too.',
      mock: () => piece(`<div style="padding:16px">${card([kv('Stop when idle', 'No agent working and nothing open on it for', dd('1 hour'))])}</div>`, 560) },
    { key: 'C', name: 'Warn before a balance runs out', from: 'Proton billing',
      desc: 'A notification when Proton’s balance would run out within a day at the current rate, and when Proton suspends a computer for it, with Add Credit.',
      good: 'No surprise suspensions mid-task.', cost: 'Reads the balance every few minutes while a computer runs.',
      mock: () => piece(`<div style="padding:16px">${toastBox(`<span class="row g2">${mark('pr', 16)}Proton balance is low</span>`, '$1.80 left: about 16 hours at the current rate. proton-cloud is suspended when it runs out.', link('Add Credit'))}</div>`, 400) },
    { key: 'D', name: 'Say when a provider stops a machine', from: 'new',
      desc: 'A notification when a machine pauses or stops without agentZ asking (Instabox’s idle pause or timeout, Proton’s suspension), naming the threads that were working.',
      good: 'You know why a thread went quiet.', cost: 'One more kind of notification.',
      mock: () => piece(`<div style="padding:16px">${toastBox(`<span class="row g2">${mark('ib', 16)}review-box paused</span>`, 'Instabox paused it after 30 idle minutes. “Fix the flaky login test” was working.', obtn('Resume'))}</div>`, 400) },
  ],
});

// 12. Where cost and limits show -----------------------------------------------------------------------
TOPICS.push({
  id: 'usage', section: 'Everywhere', title: 'Cost, usage and limits', size: 'wide', rec: 'A',
  now: 'Settings › Usage shows each agent account’s limits (5-hour, weekly…). Nothing shows a cloud account: Proton’s balance, hours and charges, or how many of Instabox’s 5 machines, 10 snapshots and 200 GB are used.',
  options: [
    { key: 'A', name: 'In each account’s header', from: 'topic 1 C',
      desc: 'Each provider group in Machines says it in its header: “$14.20 left · 31 h running this month” with Add Credit, or “2 of 5 machines · 1 of 10 snapshots · 14 of 200 GB” in yellow from 80%. A machine’s page has its own hours and cost.',
      good: 'Where the machines are.', cost: 'Not beside the agents’ usage.',
      mock: () => settingsWindow(page('Machines', groupedMachines())) },
    { key: 'B', name: 'On the Usage page', from: 'Settings › Usage',
      desc: 'Settings › Usage gets a Cloud section under the agents: one card per account with its balance or limits as bars (Instabox’s machines, snapshots and storage against their limits; Proton’s hours this month and what’s left).',
      good: 'All the spending in one place.', cost: 'Machines doesn’t say what an account costs.',
      mock: () => settingsWindow(page('Usage', `<div class="col" style="gap:10px">${sectionHead('Cloud')}${card([
        `<div class="col" style="padding:12px 16px;gap:10px"><div class="row g2">${mark('ib', 20)}<span class="b5">Instabox</span><span class="sm mu">alex@hey.com · Free</span></div>${[['Machines', '2 of 5', 40], ['Snapshots', '1 of 10', 10], ['Storage', '14 of 200 GB', 7]].map(([l, t, v]) => `<div class="row g3 sm"><span class="mu" style="width:90px">${l}</span><span style="width:110px">${t}</span><span class="grow" style="height:6px;border-radius:6px;background:#3b414d;position:relative"><i style="position:absolute;left:0;top:0;bottom:0;width:${v}%;border-radius:6px;background:var(--ac)"></i></span></div>`).join('')}</div>`,
        `<div class="col" style="padding:12px 16px;gap:10px"><div class="row g2">${mark('pr', 20)}<span class="b5">Proton</span><span class="sm mu">alex@hey.com · $20 per 730 running hours</span><span class="grow"></span>${link('Add Credit')}</div><div class="row g3 sm"><span class="mu" style="width:90px">Balance</span><span>$14.20 · about 18 days of one computer</span></div><div class="row g3 sm"><span class="mu" style="width:90px">This month</span><span>31 h running · $0.86</span></div></div>`,
      ])}</div>`), { selected: 'Usage' }) },
    { key: 'C', name: 'Both', from: 'A and B',
      desc: 'A’s header line in Machines, and B’s cards on the Usage page.',
      good: 'Seen from either place.', cost: 'Two views to keep in step.',
      mock: () => settingsWindow(page('Machines', groupedMachines())) },
  ],
});
