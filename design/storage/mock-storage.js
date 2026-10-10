// The storage round's building blocks, over board/mock.js: Settings in JetBrains Dark with a
// Storage page, the groups it could list with their sizes, and the dialogs for deleting.

Object.assign(ICONS, {
  'hard-drive': '<line x1="22" x2="2" y1="12" y2="12"/><path d="M5.45 5.11 2 12v6a2 2 0 0 0 2 2h16a2 2 0 0 0 2-2v-6l-3.45-6.89A2 2 0 0 0 16.76 4H7.24a2 2 0 0 0-1.79 1.11z"/><line x1="6" x2="6.01" y1="16" y2="16"/><line x1="10" x2="10.01" y1="16" y2="16"/>',
  archive: '<rect width="20" height="5" x="2" y="3" rx="1"/><path d="M4 8v11a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8"/><path d="M10 12h4"/>',
  package: '<path d="M11 21.73a2 2 0 0 0 2 0l7-4A2 2 0 0 0 21 16V8a2 2 0 0 0-1-1.73l-7-4a2 2 0 0 0-2 0l-7 4A2 2 0 0 0 3 8v8a2 2 0 0 0 1 1.73z"/><path d="M12 22V12"/><path d="m3.3 7 7.703 4.734a2 2 0 0 0 1.994 0L20.7 7"/>',
});
GLYPHS.droid = '❋';

const chev = ic('chev-down', 'xs');
const sw = (on) => `<span class="sw ${on ? 'on' : ''}"><i></i></span>`;
const jb = (html) => html.replace('class="m"', 'class="m jb"');
const check = (on) => `<span class="check ${on ? 'on' : ''}">${on ? ic('check', 'xs') : ''}</span>`;
const trash = () => `<span class="slot">${ic('trash', 'sm')}</span>`;
const tag = (text, kind = '') => `<span class="stag ${kind}">${text}</span>`;

// The demo, on This Mac: what each kind of data takes, and its rows.
const KINDS = [
  { id: 'chats', name: 'Chats', size: '418 MB', mb: 418, color: '#548af7' },
  { id: 'threads', name: 'Threads', size: '104 MB', mb: 104, color: '#73bd7a' },
  { id: 'checkouts', name: 'Worktrees and pastures', size: '4.6 GB', mb: 4600, color: '#f2c55c' },
  { id: 'agents', name: 'Agents', size: '645 MB', mb: 645, color: '#b48ead' },
  { id: 'other', name: 'Node.js and logs', size: '192 MB', mb: 192, color: '#8d8e91' },
];
const TOTAL = '6.0 GB';
const DEVBOX_KINDS = [
  { name: 'Chats', size: '0.9 MB' }, { name: 'Threads', size: '40 MB' }, { name: 'Worktrees and pastures', size: '2.1 GB' },
  { name: 'Agents', size: '410 MB' }, { name: 'Node.js and logs', size: '190 MB' },
];
const CHATS = [
  { id: 'webp', title: 'Convert these PNGs to WebP', size: '402 MB', ago: 'now', parts: ['Conversation 0.4 MB', 'Images 1.6 MB', 'Folder 400 MB'], working: true },
  { id: 'cache', title: 'Postgres or SQLite for the cache', size: '8.2 MB', ago: '4m', parts: ['Conversation 1.1 MB', 'Images 7.1 MB', 'Folder 0 KB'] },
  { id: 'roadmap', title: 'Notes on the Q3 roadmap', size: '5.8 MB', ago: '2w', parts: ['Conversation 0.6 MB', 'Images 5.2 MB', 'Folder 0 KB'], archived: true },
  { id: 'async', title: 'Async runtimes compared', size: '1.4 MB', ago: '2h', parts: ['Conversation 1.4 MB', 'Images 0 KB', 'Folder 12 KB'] },
  { id: 'regex', title: 'A regex for the access log', size: '0.6 MB', ago: '3d', parts: ['Conversation 0.5 MB', 'Images 0 KB', 'Folder 88 KB'] },
];
const NEWEST_CHATS = ['webp', 'cache', 'async', 'regex', 'roadmap'].map((id) => CHATS.find((chat) => chat.id === id));
const PROJECT_THREADS = [
  { project: 'storefront', mono: 'ST', color: 'g', count: 31, size: '81 MB', archived: 9, archivedSize: '24 MB', threads: [
    { title: 'Add the checkout page', size: '22 MB', ago: 'now', working: true },
    { title: 'Lazy-load the product images', size: '18 MB', ago: '9d', archived: true },
    { title: 'Write the 0.9 release notes', size: '9.4 MB', ago: '1h' },
  ] },
  { project: 'docs', mono: 'DO', color: 'p', count: 9, size: '23 MB', archived: 3, archivedSize: '6.1 MB', threads: [
    { title: 'Document the webhook retries', size: '7.2 MB', ago: '3h' },
  ] },
];
const CHECKOUTS = [
  { branch: 'checkout-flow', project: 'storefront', kind: 'worktree', size: '1.2 GB', thread: 'Add the checkout page', state: 'working' },
  { branch: 'release-notes', project: 'storefront', kind: 'worktree', size: '1.1 GB', thread: 'Write the 0.9 release notes', state: 'changes' },
  { branch: 'lazy-images', project: 'storefront', kind: 'pasture', size: '1.1 GB', thread: 'Lazy-load the product images', state: 'archived' },
  { branch: 'old-search', project: 'storefront', kind: 'worktree', size: '0.9 GB', thread: '', state: 'none' },
  { branch: 'webhook-docs', project: 'docs', kind: 'pasture', size: '300 MB', thread: 'Document the webhook retries', state: 'done' },
];
const CHECKOUT_STATE = {
  working: 'Its thread is working',
  changes: 'Uncommitted changes in 3 files',
  archived: 'Its thread was archived 9 days ago',
  none: 'Its thread was deleted',
  done: 'Its thread finished 3 hours ago',
};

// A settings page (settings_page.rs): the sections on the left and the page.
const NAV = ['General', 'Appearance', 'Notifications', 'Agents', 'Usage', 'Skills', 'MCP Servers', 'Machines'];
const NAV_ICONS = { General: 'settings', Appearance: 'eye', Notifications: 'bell', Agents: 'bot', Usage: 'activity', Skills: 'note', 'MCP Servers': 'zap', Machines: 'monitor', Storage: 'hard-drive' };
function settingsPage(body, { section = 'Storage', w = 1000, h = 620, nav = [...NAV, 'Storage'], width = 600, over = '' } = {}) {
  return jb(frame(`<div class="row" style="height:100%;align-items:stretch;position:relative">
    <div class="asb"><div class="asb-head"><span class="grow">Settings</span>${ic('x', 'sm mu')}</div>
      <div class="col" style="padding:2px 4px;gap:2px">${nav.map((name) => `<div class="lrow ${name === section ? 'on' : ''}">${ic(NAV_ICONS[name], 'sm mu')}${name}</div>`).join('')}</div></div>
    <div class="grow" style="padding:22px 0;overflow:hidden;background:var(--ed);position:relative"><div style="max-width:${width}px;margin:0 auto;padding:0 20px">${body}</div></div>${over}</div>`, { w, h, style: 'border-radius:8px;border:1px solid #111' }));
}
// One group's card alone, for topics about what a group shows.
const panel = (html, { w = 660, h = 300, over = '' } = {}) => jb(frame(`<div style="padding:4px 20px 16px;background:var(--ed);height:100%;position:relative">${html}${over}</div>`, { w, h, style: 'border-radius:8px;border:1px solid #111' }));
const settingRow = (name, description, control) => `<div class="srowx"><div class="grow"><div class="n">${name}</div><div class="d">${description}</div></div>${control}</div>`;
const settingGroup = (label, rows, { total = '', right = '' } = {}) => `<div class="sgroup"><span>${label}</span>${total ? `<span class="tot">${total}</span>` : ''}<span class="grow"></span>${right}</div><div class="scard">${rows.join('')}</div>`;

// The page's title, with the machine picker the Agents page has in its header.
const machinePicker = (name = 'This Mac', hov = false) => `<span class="dd ${hov ? 'hov' : ''}">${ic(name === 'This Mac' ? 'laptop' : 'server', 'sm mu')}${name}${chev}</span>`;
const pageHead = (right = machinePicker()) => `<div class="pghead"><span class="tt grow">Storage</span>${right}</div>`;

// What takes space: the total and a bar with a color for each kind.
const usageBar = (kinds = KINDS) => `<div class="sbar">${kinds.map((kind) => `<i style="flex:${kind.mb};background:${kind.color}"></i>`).join('')}</div>`;
const legend = (kinds = KINDS) => `<div class="legend">${kinds.map((kind) => `<span><b style="background:${kind.color}"></b>${kind.name} ${kind.size}</span>`).join('')}</div>`;
const summaryBar = (where = 'This Mac') => `<div style="margin:14px 0 4px"><div class="row" style="font-size:13px;margin-bottom:8px"><span class="grow">agentZ uses <b style="color:var(--t)">${TOTAL}</b> on ${where}</span><span class="xs" style="color:var(--ph)">~/.agentz</span></div>${usageBar()}${legend()}</div>`;
const summaryLine = (where = 'This Mac') => `<div style="margin:6px 0 0;font-size:12px;color:var(--mu)">agentZ uses ${TOTAL} on ${where}, in its data folder ~/.agentz.</div>`;

// A row with a size: a lead (icon or checkbox), a name with an optional second line, tags, the
// size, when it was last used, and a slot at the end (a trash button on hover).
function srow({ lead = '', name, sub = '', tags = '', size = '', when = '', end = '', hov = false, dim = false, cls = '', measuring = false }) {
  return `<div class="strow ${hov ? 'hov' : ''} ${dim ? 'dim' : ''} ${cls}">${lead}<div class="grow" style="min-width:0"><div class="trunc">${name}</div>${sub ? `<div class="sub2 trunc">${sub}</div>` : ''}</div>${tags}${measuring ? '<span class="meas">Measuring…</span>' : `<span class="sz">${size}</span>`}${when !== null ? `<span class="wh">${when}</span>` : ''}${end}</div>`;
}
const chatLead = (chat) => `<span class="mu" style="display:inline-flex">${ic(chat.archived ? 'archive' : 'chat', 'sm')}</span>`;
const chatRow = (chat, opts = {}) => srow({ lead: chatLead(chat), name: chat.title, size: chat.size, when: chat.ago, tags: chat.working && opts.tagWorking !== false ? tag('Working', 'work') : '', ...opts });
const chatGroup = (rows, opts = {}) => settingGroup('Chats', rows, { total: '418 MB', ...opts });

function checkoutRow(item, { lead = null, hov = false, end = '', showState = true, keep = false } = {}) {
  const icon = `<span class="mu" style="display:inline-flex">${ic(item.kind === 'pasture' ? 'pasture' : 'worktree', 'sm')}</span>`;
  const sub = showState ? `${item.project} · ${item.kind === 'pasture' ? 'Pasture' : 'Worktree'} · ${CHECKOUT_STATE[item.state]}` : `${item.project} · ${item.kind === 'pasture' ? 'Pasture' : 'Worktree'}`;
  const kept = keep && (item.state === 'working' || item.state === 'changes');
  return srow({ lead: lead ?? icon, name: item.branch, sub, size: item.size, when: null, hov, end, tags: kept ? tag(item.state === 'working' ? 'Working' : 'Has changes', item.state === 'working' ? 'work' : 'keep') : '' });
}

const projectRow = (item, { open = false, end = '', sub = '', size = item.size } = {}) => srow({ lead: `<span class="mu" style="display:inline-flex">${ic(open ? 'chev-down' : 'chev-right', 'xs')}</span>${mono(item.mono, item.color)}`, name: item.project, sub: sub || `${item.count} threads`, size, when: null, end });
const threadRow = (thread, opts = {}) => srow({ cls: 'sub', lead: `<span class="mu" style="display:inline-flex">${ic(thread.archived ? 'archive' : 'chat', 'sm')}</span>`, name: thread.title, size: thread.size, when: thread.ago, tags: thread.working ? tag('Working', 'work') : '', ...opts });

const agentRows = () => [
  srow({ lead: glyph('codex', 'sm'), name: 'Codex', sub: 'Installed agent · uninstall it on the Agents page', size: '248 MB', when: null, end: '<span class="slot"></span>' }),
  srow({ lead: glyph('claude', 'sm'), name: 'Claude Agent', sub: 'Installed agent', size: '212 MB', when: null, end: '<span class="slot"></span>' }),
  srow({ lead: glyph('droid', 'sm'), name: 'Factory Droid', sub: 'Installed agent', size: '182 MB', when: null, end: '<span class="slot"></span>' }),
  srow({ lead: `<span class="mu" style="display:inline-flex">${ic('layers', 'sm')}</span>`, name: 'Registry cache', sub: 'The ACP Registry’s list and icons, fetched again when needed', size: '3 MB', when: null, end: '<span class="btn sm">Clear</span>' }),
];
const otherRows = () => [
  srow({ lead: `<span class="mu" style="display:inline-flex">${ic('package', 'sm')}</span>`, name: 'Node.js 22', sub: 'Downloaded for agents that run on Node; downloaded again when one needs it', size: '180 MB', when: null, end: '<span class="btn sm">Delete</span>' }),
  srow({ lead: `<span class="mu" style="display:inline-flex">${ic('file', 'sm')}</span>`, name: 'Server log', sub: 'Started over when it passes its limit', size: '12 MB', when: null, end: '<span class="btn sm">Clear</span>' }),
];

// The dialog agentZ draws over a page.
const dialog = (title, text, buttons, { left = 330, top = 120, width = 380 } = {}) => `<div class="modal-back"></div><div class="dlg" style="left:${left}px;top:${top}px;width:${width}px"><h4>${title}</h4><p>${text}</p><div class="row g2" style="justify-content:flex-end">${buttons}</div></div>`;

// The whole page as most topics show it: title and machine, the bar, and the first groups.
function storagePage({ head = pageHead(), summary = summaryBar(), groups = defaultGroups(), h = 500, ...opts } = {}) {
  return settingsPage(`${head}${summary}${groups}`, { h, ...opts });
}
const defaultGroups = () => `${chatGroup(CHATS.slice(0, 4).map((chat) => chatRow(chat)))}${settingGroup('Worktrees and pastures', CHECKOUTS.slice(0, 3).map((item) => checkoutRow(item)), { total: '4.6 GB' })}`;
