// Archived workspace cleanup mocks: Settings as settings_page.rs draws it (the sections, a
// group of rows with a description and a control), its Storage page's list of worktrees and
// pastures (settings_page/storage.rs), and t3code's cleanup controls and report.

Object.assign(ICONS, {
  archive: '<rect width="20" height="5" x="2" y="3" rx="1"/><path d="M4 8v11a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8"/><path d="M10 12h4"/>',
  'database-zap': '<ellipse cx="12" cy="5" rx="9" ry="3"/><path d="M3 5V19A9 3 0 0 0 15 21.84"/><path d="M21 5V8"/><path d="M21 12L18 17H22L19 22"/><path d="M3 12A9 3 0 0 0 14.59 14.87"/>',
  gauge: '<path d="m12 14 4-4"/><path d="M3.34 19a10 10 0 1 1 17.32 0"/>',
  book: '<path d="M4 19.5v-15A2.5 2.5 0 0 1 6.5 2H20v20H6.5a2.5 2.5 0 0 1 0-5H20"/>',
  hammer: '<path d="m15 12-8.373 8.373a1 1 0 1 1-3-3L12 9"/><path d="m18 15 4-4"/><path d="m21.5 11.5-1.914-1.914A2 2 0 0 1 19 8.172V7l-2.26-1.13a6 6 0 0 0-5.12-.12L9.4 6.7"/>',
  sparkle: '<path d="M9.94 14.06 4 20"/><path d="m14 4 1.5 3.5L19 9l-3.5 1.5L14 14l-1.5-3.5L9 9l3.5-1.5z"/>',
  info: '<circle cx="12" cy="12" r="10"/><path d="M12 16v-4"/><path d="M12 8h.01"/>',
  send: '<path d="M14.54 21.69a.5.5 0 0 0 .94-.03l6.5-19a.5.5 0 0 0-.64-.64l-19 6.5a.5.5 0 0 0-.03.94l7.93 3.18a2 2 0 0 1 1.11 1.11z"/><path d="m21.85 2.15-10.94 10.94"/>',
});

const acChev = (name = 'chev-down') => ic(name, 'xs');
const acSwitch = (on) => `<span class="sw ${on ? 'on' : ''}"><i></i></span>`;
const acDropdown = (label, on = false) => `<span class="ddb ${on ? 'on' : ''}">${label}<span class="mu" style="display:inline-flex">${acChev()}</span></span>`;
const acStepper = (value) => `<span class="row g2"><span class="stepper"><span>−</span><b>${value}</b><span>+</span></span><span class="sm mu">days</span>${acSwitch(true)}</span>`;
const acTag = (text, kind = '') => `<span class="stag ${kind}">${text}</span>`;
const acTrash = (show = true) => `<span class="slot">${show ? ic('trash', 'sm') : ''}</span>`;

// Settings (settings_page.rs): the sections on the left, the page on the right.
const AC_NAV = [['General', 'settings'], ['Appearance', 'eye'], ['Notifications', 'bell'], ['Agents', 'sparkle'], ['Usage', 'gauge'], ['Skills', 'book'], ['MCP Servers', 'hammer'], ['Machines', 'server'], ['Storage', 'database-zap']];
const AC_PROJECT_NAV = [['storefront', 'ST', 'g'], ['api', 'AP', 't']];
function acSettings(body, { section = 'Storage', w = 900, h = 560, width = 600, over = '' } = {}) {
  const nav = AC_NAV.map(([name, icon]) => `<div class="lrow ${name === section ? 'on' : ''}">${ic(icon, 'sm mu')}${name}</div>`).join('');
  const projects = AC_PROJECT_NAV.map(([name, text, color]) => `<div class="lrow ${name === section ? 'on' : ''}">${mono(text, color)}${name}</div>`).join('');
  return frame(`<div class="row" style="height:100%;align-items:stretch;position:relative">
    <div class="asb"><div class="asb-head"><span class="grow">Settings</span>${ic('x', 'sm mu')}</div>
      <div class="col" style="padding:2px 4px;gap:2px">${nav}<div style="height:1px;margin:6px 8px;background:var(--bv)"></div>${projects}</div></div>
    <div class="grow" style="padding:22px 0;overflow:hidden;background:var(--ed);position:relative"><div style="max-width:${width}px;margin:0 auto;padding:0 20px">${body}</div></div>${over}</div>`, { w, h, style: 'border-radius:8px;border:1px solid var(--b)' });
}
const acPageHead = (title, right = '') => `<div class="pghead"><span class="tt grow">${title}</span>${right}</div>`;
const acMachine = () => `<span class="dd">${ic('laptop', 'sm mu')}This Mac${acChev()}</span>`;
const acRow = (name, description, control, { dim = false } = {}) => `<div class="srowx ${dim ? 'dim' : ''}"><div class="grow"><div class="n">${name}</div><div class="d">${description}</div></div>${control}</div>`;
const acGroup = (label, rows, { total = '', right = '', note = '' } = {}) => `<div class="sgroup"><span>${label}</span>${total ? `<span class="tot">${total}</span>` : ''}<span class="grow"></span>${right}</div><div class="scard">${rows.join('')}</div>${note ? `<div class="sm mu" style="margin-top:8px">${note}</div>` : ''}`;
// One group alone, for topics about what it holds.
const acPanel = (html, { w = 640, h = 300, over = '' } = {}) => frame(`<div style="padding:4px 20px 16px;background:var(--ed);height:100%;position:relative">${html}${over}</div>`, { w, h, style: 'border-radius:8px;border:1px solid var(--b)' });

// The Storage page's worktrees and pastures (storage.rs): name over “project · kind · state”,
// a tag for what can’t be deleted, the size, and a trash button on hover.
const AC_CHECKOUTS = {
  checkout: { name: 'checkout-flow', project: 'storefront', kind: 'worktree', size: '1.2 GB', state: 'Its thread is working', tag: ['Working', 'work'] },
  notes: { name: 'release-notes', project: 'storefront', kind: 'worktree', size: '1.1 GB', state: 'Uncommitted changes in 3 files', tag: ['Has changes', 'keep'] },
  images: { name: 'lazy-images', project: 'storefront', kind: 'pasture', size: '1.1 GB', state: 'Its thread was archived 9 days ago' },
  search: { name: 'old-search', project: 'storefront', kind: 'worktree', size: '0.9 GB', state: 'Its thread was deleted' },
  webhooks: { name: 'webhook-docs', project: 'api', kind: 'pasture', size: '300 MB', state: 'Its thread finished 3 hours ago' },
  limits: { name: 'rate-limits', project: 'api', kind: 'worktree', size: '640 MB', state: 'Its thread was archived 2 days ago' },
};
function acCheckout(key, { hov = false, state = null, tag = undefined, trash = null } = {}) {
  const item = AC_CHECKOUTS[key];
  const shownTag = tag === undefined ? item.tag : tag;
  const kind = item.kind === 'pasture' ? 'Pasture' : 'Worktree';
  const canDelete = trash ?? !item.tag;
  return `<div class="strow ${hov ? 'hov' : ''}"><span class="mu" style="display:inline-flex">${ic(item.kind === 'pasture' ? 'pasture' : 'worktree', 'sm')}</span><div class="grow" style="min-width:0"><div class="trunc">${item.name}</div><div class="sub2 trunc">${item.project} · ${kind} · ${state ?? item.state}</div></div>${shownTag ? acTag(shownTag[0], shownTag[1]) : ''}<span class="sz">${item.size}</span>${acTrash(hov && canDelete)}</div>`;
}
const acCheckoutGroup = (keys, options = {}, groupOptions = {}) => acGroup('Worktrees and pastures', keys.map((key) => acCheckout(key, options[key] || {})), { total: '4.6 GB', ...groupOptions });

// The page's summary (render_summary): the total and the data folder, over a bar per kind.
const AC_KINDS = [['Chats', '418 MB', 418, '#548af7'], ['Threads', '104 MB', 104, '#73bd7a'], ['Worktrees and pastures', '4.6 GB', 4600, '#f2c55c'], ['Agents', '645 MB', 645, '#b48ead'], ['Node.js and logs', '192 MB', 192, '#8d8e91']];
const acSummary = () => `<div style="margin:14px 0 4px"><div class="row" style="font-size:13px;margin-bottom:8px;color:var(--mu)"><span class="grow">agentZ uses <b style="color:var(--t);font-weight:600">6.0 GB</b> on This Mac</span><span class="xs" style="color:var(--ph)">~/.agentz</span></div><div class="sbar">${AC_KINDS.map(([, , mb, color]) => `<i style="flex:${mb};background:${color}"></i>`).join('')}</div><div class="legend">${AC_KINDS.map(([name, size, , color]) => `<span><b style="background:${color}"></b>${name} ${size}</span>`).join('')}</div></div>`;

// The open thread, for an archived thread whose workspace was removed.
const acThread = ({ above = '', composer = true, over = '', w = 640, h = 330 } = {}) => frame(`<div class="col" style="height:100%"><div class="row ac-head"><span class="row g15 sm mu none" style="padding:2px 4px">${mono('ST', 'g')}storefront</span><span class="sm mu">/</span><span class="grow trunc" style="font-size:13px;padding:0 6px">Lazy-load the product images</span><span class="row g1 sm mu none">${ic('pasture', 'xs')}lazy-images</span>${ibtn('more')}</div>
  <div class="grow col" style="justify-content:flex-end;align-items:center;padding-bottom:6px;font-size:13px;line-height:21px"><div style="width:100%;max-width:600px"><div style="display:flex;justify-content:flex-end;padding:12px 20px 4px"><div style="max-width:80%;padding:8px 12px;border-radius:12px;background:#3d424c">Lazy-load the product images below the fold</div></div><div style="padding:6px 20px">Done: images below the fold load as they scroll into view, with a blurred placeholder. I left the hero image eager.</div></div></div>
  ${above}${composer ? `<div style="flex:none;border-top:1px solid var(--b);padding:8px 20px 10px;background:var(--panel)"><div class="ph" style="height:22px;font-size:13px">Message the agent…</div><div class="row" style="height:26px;gap:10px;font-size:12px;color:var(--mu)"><span class="row g15">${glyph('claude')}Claude Agent</span><span class="grow"></span><span style="display:inline-flex;color:var(--ac)">${ic('send', 'sm')}</span></div></div>` : ''}</div>${over}`, { w, h, style: 'border:1px solid var(--b);border-radius:8px' });
const acArchivedNotice = () => `<div style="padding:0 8px 8px"><div class="ac-callout"><span class="mu" style="height:18px;display:inline-flex;align-items:center">${ic('info', 'sm')}</span><div class="grow">This thread is archived</div><span class="btn sm primary">Unarchive</span></div></div>`;

// The sidebar's Archived shelf, open, with a hovered thread and its details popover beside it.
const acSlim = ([title, monoText, color, ago], hov = false) => `<div class="ac-slim ${hov ? 'hov' : ''}"><span class="ico">${mono(monoText, color)}</span><span class="grow trunc">${title}</span><span class="xs ph">${ago}</span></div>`;
const AC_ARCHIVED_THREADS = [['Lazy-load the product images', 'ST', 'g', '9d'], ['Replace the old search page', 'ST', 'g', '12d'], ['Rate-limit the public API', 'AP', 't', '2d'], ['Document the webhooks', 'AP', 't', '3w']];
const acArchivedSidebar = ({ over = '', w = 640, h = 330 } = {}) => frame(`<div class="row" style="height:100%;align-items:stretch"><div class="none" style="width:290px;border-right:1px solid var(--b)">${sidebar({ list: `<div class="ac-shelf">Archived<span class="rule"></span>${acChev('chev-up')}</div>${AC_ARCHIVED_THREADS.map((thread, index) => acSlim(thread, index === 0)).join('')}` })}</div><div class="grow" style="background:var(--ed)"></div></div>${over}`, { w, h, style: 'border:1px solid var(--b);border-radius:8px' });

const acCaption = (text) => `<div class="ac-cap">${text}</div>`;
const acPair = (...items) => `<div class="ac-pair">${items.join('')}</div>`;
