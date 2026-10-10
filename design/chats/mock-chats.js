// The chats round's building blocks, over board/mock.js: the app in JetBrains Dark (sidebar,
// thread, new thread screen, composer, settings), with project threads and chats beside them.

Object.assign(ICONS, {
  'chat-dashed': '<path d="M10 17H7l-4 4v-7"/><path d="M14 17h1"/><path d="M14 3h1"/><path d="M19 3a2 2 0 0 1 2 2"/><path d="M21 14v1a2 2 0 0 1-2 2"/><path d="M21 9v1"/><path d="M3 9v1"/><path d="M5 3a2 2 0 0 0-2 2"/><path d="M9 3h1"/>',
  send: '<path d="M14.54 21.69a.5.5 0 0 0 .94-.03l6.5-19a.5.5 0 0 0-.64-.64l-19 6.5a.5.5 0 0 0-.03.94l7.93 3.18a2 2 0 0 1 1.11 1.11z"/><path d="m21.85 2.15-10.94 10.94"/>',
  'folder-plus': '<path d="M12 10v6"/><path d="M9 13h6"/><path d="M20 20a2 2 0 0 0 2-2V8a2 2 0 0 0-2-2h-7.9a2 2 0 0 1-1.69-.9L9.6 3.9A2 2 0 0 0 7.93 3H4a2 2 0 0 0-2 2v13a2 2 0 0 0 2 2Z"/>',
  archive: '<rect width="20" height="5" x="2" y="3" rx="1"/><path d="M4 8v11a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8"/><path d="M10 12h4"/>',
  wrench: '<path d="M14.7 6.3a1 1 0 0 0 0 1.4l1.6 1.6a1 1 0 0 0 1.4 0l3.77-3.77a6 6 0 0 1-7.94 7.94l-6.91 6.91a2.12 2.12 0 0 1-3-3l6.91-6.91a6 6 0 0 1 7.94-7.94l-3.76 3.76z"/>',
  'eye-off': '<path d="M10.73 5.08A10.43 10.43 0 0 1 12 5c7 0 10 7 10 7a13.16 13.16 0 0 1-1.67 2.68"/><path d="M6.61 6.61A13.53 13.53 0 0 0 2 12s3 7 10 7a9.74 9.74 0 0 0 5.39-1.61"/><line x1="2" x2="22" y1="2" y2="22"/>',
  lock: '<rect width="18" height="11" x="3" y="11" rx="2" ry="2"/><path d="M7 11V7a5 5 0 0 1 10 0v4"/>',
});
GLYPHS.droid = '❋';

// The demo: three projects' threads, and chats outside them.
const PROJECTS = {
  st: { name: 'storefront', mono: 'ST', color: 'g' },
  ap: { name: 'api', mono: 'AP', color: 't' },
  dc: { name: 'docs', mono: 'DO', color: 'p' },
};
const THREADS = [
  { id: 'checkout', project: 'st', title: 'Add the checkout page', state: 'working', branch: 'checkout-flow', glyph: 'claude', ago: 'now' },
  { id: 'limiter', project: 'ap', title: 'Speed up the rate limiter', state: 'done', branch: 'limiter-buckets', machine: 'devbox', glyph: 'codex', ago: '12m' },
  { id: 'notes', project: 'st', title: 'Write the 0.9 release notes', state: 'done', branch: 'release-notes', glyph: 'droid', ago: '1h' },
  { id: 'webhooks', project: 'dc', title: 'Document the webhook retries', state: 'done', branch: 'webhook-docs', glyph: 'claude', ago: '3h' },
];
const CHATS = [
  { id: 'webp', title: 'Convert these PNGs to WebP', state: 'working', glyph: 'codex', agent: 'Codex', ago: 'now' },
  { id: 'cache', title: 'Postgres or SQLite for the cache', state: 'pending', glyph: 'claude', agent: 'Claude Agent', ago: '4m' },
  { id: 'async', title: 'Async runtimes compared', state: 'done', glyph: 'claude', agent: 'Claude Agent', ago: '2h' },
  { id: 'rust', title: 'What changed in Rust 1.90', state: 'done', glyph: 'droid', agent: 'Factory Droid', ago: '1d', machine: 'devbox' },
  { id: 'regex', title: 'A regex for the access log', state: 'done', glyph: 'claude', agent: 'Claude Agent', ago: '3d' },
];
const ARCHIVED_COUNT = 6;
const chev = ic('chev-down', 'xs');
const projectMono = (key) => mono(PROJECTS[key].mono, PROJECTS[key].color);
const chatIcon = (cls = 'sm') => `<span class="mu" style="display:inline-flex">${ic('chat', cls)}</span>`;
const machineGlyph = (item) => ic(item.machine === 'devbox' ? 'server' : 'laptop', 'xs');
const sw = (on) => `<span class="sw ${on ? 'on' : ''}"><i></i></span>`;
const jb = (html) => html.replace('class="m"', 'class="m jb"');

// The app's window: title bar, sidebar, and the thread or screen beside it.
function appWin(sidebarHtml, mainHtml, { w = 1060, h = 640, filter = 'All projects' } = {}) {
  return jb(frame(`<div class="awin">
    <div class="atitle">${ic('sidebar', 'sm')}<span class="row g1">${ic('list', 'sm')}${filter}${chev}</span><div class="aseg"><span class="on">Agents</span><span>Workspaces</span></div></div>
    <div class="abody">${sidebarHtml}${mainHtml}</div></div>`, { w, h, style: 'border-radius:10px;border:1px solid #111' }));
}

// A project thread's card (sidebar.rs): project and state, title, branch and icons.
function tcard(thread, { on = false, hov = false } = {}) {
  const project = PROJECTS[thread.project];
  return `<div class="tcard ${on ? 'on' : ''} ${hov ? 'hov' : ''}"><div class="p">${projectMono(thread.project)}<span>${project.name}</span><span class="grow"></span>${pill(thread.state)}</div><div class="tl trunc">${thread.title}</div><div class="br"><span class="grow trunc">${thread.branch}</span>${machineGlyph(thread)}${glyph(thread.glyph, 'sm')}</div></div>`;
}
const cards = (on = 'checkout', list = THREADS) => list.map((thread) => tcard(thread, { on: thread.id === on })).join('');

// A chat as a card: Chat in place of the project, and no branch.
function chatCard(chat, { on = false, hov = false } = {}) {
  return `<div class="tcard ${on ? 'on' : ''} ${hov ? 'hov' : ''}"><div class="p">${ic('chat', 'xs')}<span>Chat</span><span class="grow"></span>${pill(chat.state)}</div><div class="tl trunc">${chat.title}</div><div class="br"><span class="grow trunc">${chat.agent}</span>${machineGlyph(chat)}${glyph(chat.glyph, 'sm')}</div></div>`;
}

// A chat as a slim row, like an Archived one: an icon, the title and when it last did something.
// `state` adds a dot while it's working or waiting for you; `lead` picks the icon.
function chatRow(chat, { on = false, hov = false, state = false, lead = 'chat', end = '' } = {}) {
  const icon = lead === 'agent' ? glyph(chat.glyph, 'sm') : chatIcon();
  const busy = state && chat.state !== 'done';
  const trailing = end || (busy ? `${dot(chat.state)}<span class="when">${chat.ago}</span>` : `<span class="when">${chat.ago}</span>`);
  return `<div class="lrow ${on ? 'on' : ''} ${hov ? 'hov' : ''} ${!busy && !on ? 'quiet' : ''}">${icon}<span class="grow trunc">${chat.title}</span>${trailing}</div>`;
}
const chatRows = (opts = {}, list = CHATS) => list.map((chat) => chatRow(chat, { ...opts, on: opts.onId === chat.id })).join('');

// t3code's shelf header: a label with its count, a rule and a chevron; `plus` adds a New Chat
// button on it.
function shelf(name, count, { open = false, plus = false, hov = false, tone = '', extra = '' } = {}) {
  return `<div class="shelf" style="${tone ? `color:${tone}` : ''}"><span>${name}${count != null ? ` (${count})` : ''}</span>${extra}<span class="rule"></span>${plus ? `<span class="ibtn sm ${hov ? 'hov' : ''}" style="color:var(--mu)">${ic('plus', 'xs')}</span>` : ''}${ic(open ? 'chev-up' : 'chev-down', 'xs')}</div>`;
}
const archivedShelf = () => shelf('Archived', ARCHIVED_COUNT);

function sidebarJB({ head = '', top = '', list = cards(), shelves = archivedShelf(), foot = '' } = {}) {
  return `<div class="asb"><div class="asb-head">${head || `${ic('search', 'sm')}<span class="grow">Search…</span>${ic('plus', 'sm mu')}`}</div>
    ${top}<div class="asb-list">${list}</div><div style="flex:none;padding:0 4px 6px">${shelves}</div><div class="asb-foot">${ic('settings', 'sm')}Settings${foot}</div></div>`;
}
// The sidebar alone, at its real width.
const sbFrame = (html, { h = 600, w = 290, over = '' } = {}) => jb(frame(`${html}${over}`, { w, h, style: 'border-radius:8px;border:1px solid #111' }));

// The thread or chat: header, conversation, composer.
function threadView({ lead, title = 'Async runtimes compared', right = '', convo = '', composer = composerBox(), over = '' } = {}) {
  return `<div class="thread">
    <div class="thead">${lead ?? `${chatIcon()}<span>Chat</span>`}<span class="ph">/</span><span class="tt">${title}</span>${chev}<span class="grow"></span>${right}</div>
    <div class="convo"><div class="column">${convo}</div></div>
    ${over}${composer}
  </div>`;
}
const PROJECT_HEAD_RIGHT = `<span class="bbtn">${ic('branch', 'xs')}main</span>${ibtn('diff')}${ibtn('terminal')}${ibtn('more')}`;
const bub = (text) => `<div class="bub">${text}</div>`;
const msg = (html) => `<div class="msg">${html}</div>`;
const worked = (text = 'Worked for 41s') => `<div class="worked">${text}</div>`;
const trow = (icon, label) => `<div class="trow"><span class="ico">${icon}</span><span class="row grow" style="gap:4px;min-width:0">${label}</span></div>`;
const ASYNC_CONVO = `${bub('Compare Tokio, smol and Glommio for a small HTTP proxy.')}${msg('For a small proxy, <b>Tokio</b> is the safe pick: work stealing, a wheel for timers, and the widest set of libraries. <b>smol</b> is smaller and easy to read; <b>Glommio</b> needs Linux and a thread per core.')}${worked()}`;

// The composer at a thread's foot, with optional text, an overlay above it, and its agent.
function composerBox({ text = '', overlay = '', agent = 'Claude Agent', kind = 'claude', placeholder = 'Message the agent…' } = {}) {
  return `<div class="composer">${overlay}<div class="in"><div style="padding:2px 0 12px;${text ? '' : 'color:var(--ph)'}">${text || placeholder}</div>
    <div class="row" style="gap:12px;font-size:12px;color:var(--mu)">${ic('plus', 'sm')}<span class="row g1">${glyph(kind, 'sm')}${agent}</span><span class="grow"></span><span>Default ${chev}</span><span>Sonnet ${chev}</span><span>Medium ${chev}</span><span class="ibtn" style="background:var(--sel);color:var(--t)">${ic('send', 'xs')}</span></div></div></div>`;
}
const mchip = (icon, label) => `<span class="mchip">${icon}${label}</span>`;
const projectChip = (key) => mchip(projectMono(key), PROJECTS[key].name);
const threadChip = (title) => mchip(ic('chat', 'xs'), title);
const fileChip = (name) => mchip(ic('file', 'xs'), name);

// The @ menu (mention_menu.rs): labeled groups of rows, each with an icon, a name and a hint.
const mrow = (icon, name, hint = '', { hl = false, dim = false } = {}) => `<div class="it ${hl ? 'hl' : ''}" style="${dim ? 'color:var(--ph)' : ''}">${icon}<span class="grow trunc">${name}</span>${hint ? `<span class="xs" style="color:var(--ph);padding-left:12px">${hint}</span>` : ''}</div>`;
const mgroup = (label, rows) => `<div class="lbl">${label}</div>${rows.join('')}`;
const mentionMenu = (groups, { style = 'left:24px;bottom:100%;margin-bottom:-6px', width = 360 } = {}) => `<div class="menu" style="${style};width:${width}px">${groups.join('')}</div>`;

// The new thread screen (agent_view.rs render_new_thread): a headline, the composer card and
// the strip under it.
function newScreen({ headline = 'What should we work on?', left = '', right = '', under = '', overlay = '', agent = 'Claude Agent', kind = 'claude', h = 340 } = {}) {
  return `<div class="thread" style="background:var(--ed)">
    <div class="thead">${chatIcon()}<span>Chat</span><span class="ph">/</span><span class="tt">New chat</span><span class="grow"></span>${ibtn('more')}</div>
    <div class="col" style="flex:1;align-items:center;justify-content:center;gap:22px;padding:0 24px 40px;position:relative">
      <div style="font-size:22px;color:var(--t)">${headline}</div>
      <div class="col" style="width:100%;max-width:600px;gap:8px;position:relative">
        <div class="card" style="padding:10px 12px 8px;background:var(--panel)">
          <div style="height:44px;color:var(--ph)">Message the agent…</div>
          <div class="row" style="gap:12px;font-size:12px;color:var(--mu)">${ic('plus', 'sm')}<span class="row g1" style="color:var(--t)">${glyph(kind, 'sm')}${agent}${chev}</span><span class="grow"></span><span>Default ${chev}</span><span>Sonnet ${chev}</span><span>Medium ${chev}</span><span class="ibtn" style="background:var(--sel);color:var(--t)">${ic('send', 'xs')}</span></div>
        </div>
        <div class="row sm" style="justify-content:space-between;padding:0 4px;gap:8px;color:var(--mu);min-height:22px"><span class="row g2">${left}</span><span class="row g1" style="min-width:0">${right}</span></div>
        ${under}${overlay}
      </div>
    </div>
  </div>`;
}
const stripChip = (icon, label, { on = false, chevron = true } = {}) => `<span class="row g1" style="height:22px;padding:0 6px;border-radius:5px;${on ? 'background:var(--sel);color:var(--t);' : ''}">${icon}${label}${chevron ? chev : ''}</span>`;
const machineChip = (name = 'This Mac', on = false) => stripChip(ic(name === 'This Mac' ? 'laptop' : 'server', 'xs'), name, { on });
const accountChip = () => stripChip(`<span class="dot" style="background:#d38b5d"></span>`, 'work@acme.dev');

// A settings page (settings_page.rs): the sections on the left and the page's groups.
function settingsPage(body, { section = 'General', w = 1000, h = 600 } = {}) {
  const nav = ['General', 'Appearance', 'Notifications', 'Agents', 'Usage', 'Skills', 'MCP Servers', 'Machines'];
  const icons = { General: 'settings', Appearance: 'eye', Notifications: 'bell', Agents: 'bot', Usage: 'activity', Skills: 'note', 'MCP Servers': 'zap', Machines: 'monitor' };
  return jb(frame(`<div class="row" style="height:100%;align-items:stretch">
    <div class="asb" style="width:240px"><div class="asb-head" style="color:var(--t)"><span class="grow">Settings</span>${ic('x', 'sm mu')}</div>
      <div class="col" style="padding:2px 4px;gap:2px">${nav.map((name) => `<div class="lrow ${name === section ? 'on' : ''}" style="height:28px">${ic(icons[name], 'sm mu')}${name}</div>`).join('')}</div></div>
    <div class="grow" style="padding:22px 0;overflow:hidden;background:var(--ed)"><div style="max-width:560px;margin:0 auto">${body}</div></div></div>`, { w, h, style: 'border-radius:8px;border:1px solid #111' }));
}
const settingRow = (name, description, control) => `<div class="srowx"><div class="grow"><div class="n">${name}</div><div class="d">${description}</div></div>${control}</div>`;
const settingGroup = (label, rows) => `<div class="sgroup">${label}</div><div class="scard">${rows.join('')}</div>`;
