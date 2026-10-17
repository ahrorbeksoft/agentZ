// Open in Editor mocks: the open thread's header as agent_view.rs render_toolbar draws it
// (project crumb, title, branch, changes, Terminal, ⋯), the editor menus, and the diff panel.

ICONS['folder-closed'] = ICONS.folder;

const PROJECTS = {
  st: { mono: 'ST', color: 'g', name: 'storefront', path: '~/storefront', machine: 'laptop', where: 'This Mac' },
  ap: { mono: 'AP', color: 't', name: 'api', path: '~/api', machine: 'server', where: 'Devbox 1' },
};

// Stand-ins for the editors' logos, as the board's glyphs stand in for the registry's icons.
const OE_EDITORS = {
  cursor: { label: 'Cursor', bg: '#e9e9e9', fg: '#161616', text: '◆' },
  vscode: { label: 'VS Code', bg: '#2d7fd0', fg: '#fff', text: '‹›' },
  zed: { label: 'Zed', bg: '#1f5fe0', fg: '#fff', text: 'Z' },
  webstorm: { label: 'WebStorm', bg: '#17d3d8', fg: '#101214', text: 'WS' },
  idea: { label: 'IntelliJ IDEA', bg: '#e8456b', fg: '#fff', text: 'IJ' },
  vscodium: { label: 'VSCodium', bg: '#2ba3c4', fg: '#fff', text: '‹›' },
};
const edIcon = (id, { dim = false } = {}) => {
  if (id === 'files') return ic('folder-closed', `sm ${dim ? 'ph' : 'mu'}`);
  const e = OE_EDITORS[id];
  return `<span class="oe-ed ${dim ? 'dim' : ''}" style="background:${e.bg};color:${e.fg}">${e.text}</span>`;
};
const edLabel = (id) => (id === 'files' ? 'Finder' : OE_EDITORS[id].label);

const OE_KEY = '⌘⇧O';
const OE_W = 700;
const OE_NARROW = 340;

const oeStat = '<span class="oe-out"><span class="okc">+24</span><span class="delc">−3</span></span>';
const oeTerminal = ibtn('terminal');
const oeMore = (on = false) => ibtn('more', on ? 'on' : '');

const oeSplit = ({ editor = 'vscode', label = true, hl = false, dis = false, chevron = false } = {}) =>
  `<span class="oe-split ${dis ? 'dis' : ''}"><span class="oe-p ${hl ? 'on' : ''}">${editor ? edIcon(editor) : ic('external', 'sm mu')}${label ? '<span>Open</span>' : ''}</span><span class="oe-c ${chevron ? 'on' : ''}">${ic('chev-down', 'xs')}</span></span>`;

// An icon-only button in the header's own button style.
const oeIconButton = (editor, on = false, dis = false) =>
  `<span class="ibtn ${on ? 'on' : ''}" style="${dis ? 'opacity:.4' : ''}">${editor ? edIcon(editor) : ic('external', 'sm')}</span>`;

/**
 * The thread header: project crumb, title, the branch, then the buttons. `before` goes ahead
 * of the changes button and `after` between Terminal and ⋯. Narrow (under 480), the branch,
 * changes and Terminal are in ⋯, and under 360 the project shows only its icon.
 */
function oeHeader({ w = OE_W, project = 'st', title = 'Add the checkout page', before = '', after = '', more = oeMore(), titleOn = false, branch = 'checkout-flow' } = {}) {
  const compact = w < 480;
  const iconOnly = w <= 360;
  const p = PROJECTS[project];
  const crumb = `<span class="row g15 sm mu" style="padding:2px 4px;flex:none">${mono(p.mono, p.color, '')}${iconOnly ? '' : p.name}</span><span class="sm mu">/</span>`;
  const titleEl = `<span class="row g1 none" style="max-width:100%;padding:2px 6px;border-radius:5px;font-size:13px;${titleOn ? 'background:var(--hov)' : ''}"><span class="trunc">${title}</span>${ic('chev-down', 'xs mu')}</span>`;
  const branchEl = `<span class="row g1 sm mu" style="flex:none">${ic('branch', 'xs')}${branch}</span>`;
  const buttons = compact
    ? `${more}`
    : `${before}${oeStat}${oeTerminal}${after}${more}`;
  return `<div class="row" style="height:36px;flex:none;padding:0 8px;gap:4px;border-bottom:1px solid var(--b)">${crumb}<span class="grow row" style="min-width:0">${titleEl}</span>${compact ? '' : branchEl}<span class="row" style="gap:6px;flex:none;margin-left:6px">${buttons}</span></div>`;
}

const oeBody = (lines = ['Add the checkout page with a pay button', 'I’ll add the page under <span class="mono-font sm">src/app/checkout</span> and reuse the cart’s totals.']) =>
  `<div class="oe-body"><div class="row" style="justify-content:flex-end"><div style="background:var(--panel);border-radius:12px;padding:6px 12px;max-width:80%">${lines[0]}</div></div><div class="mu">${lines[1]}</div><div class="row g15 ac sm"><span class="spin"></span>Working…</div></div>`;

const oeScene = ({ w = OE_W, h = 210, header, overlays = '', body = oeBody() } = {}) =>
  frame(`${header}${body}${overlays}`, { w, h, style: 'border:1px solid var(--b);border-radius:8px' });
const oeCaption = (text) => `<div class="oe-cap">${text}</div>`;
const oePair = (a, b) => `<div class="oe-pair">${a}${b}</div>`;

// Menus -------------------------------------------------------------------------------------
const oeItem = ({ icon = '', label, key = '', hl = false, dis = false, sub = '', danger = false, arrow = false }) =>
  `<div class="it ${hl ? 'hl' : ''} ${dis ? 'dis' : ''} ${danger ? 'danger' : ''}">${icon}<span>${label}</span>${sub ? `<span class="oe-sub">${sub}</span>` : ''}${key ? `<span class="kb">${key}</span>` : ''}${arrow ? `<span class="kb">${ic('chev-right', 'xs')}</span>` : ''}</div>`;
const oeHr = '<div class="hr"></div>';
const oeMenu = (items, style = '') => `<div class="menu" style="${style}">${[].concat(items).join('')}</div>`;
const oeEditorItems = (ids, { first = 'vscode', key = true, dis = false } = {}) =>
  ids.map((id) => oeItem({ icon: edIcon(id), label: edLabel(id), key: key && id === first ? OE_KEY : '', dis })).join('');
const oeTip = (text, style) => `<span class="oe-tip" style="${style}">${text}</span>`;

const OE_DETECTED = ['cursor', 'vscode', 'zed', 'webstorm'];

// The narrow header's ⋯ today: the branch line, Terminal, Changes, then the agent's options.
const oeCompactItems = (extra = '') => [
  `<div class="it" style="height:auto;padding:6px 8px">${ic('branch', 'sm')}<span class="sm mu">checkout-flow</span></div>`,
  oeHr,
  oeItem({ icon: ic('terminal', 'sm'), label: 'Toggle Terminal', key: '⌘J' }),
  oeItem({ icon: ic('diff', 'sm'), label: 'Show Changes', sub: '+24 −3', key: '⌘D' }),
  extra,
  oeHr,
  oeItem({ label: 'Reload Agent' }),
].join('');
const oeAgentItems = (extra = '') => [extra, extra ? oeHr : '', oeItem({ label: 'Reauthenticate' }), oeItem({ label: 'Log Out…' }), oeHr, oeItem({ label: 'Reload Agent' })].join('');

// The diff panel (diff_panel.rs): file headers over hunks, the changed files listed.
const oeDiffLine = (no, kind, text) => {
  const bg = kind === '+' ? 'rgba(161,193,129,.13)' : kind === '-' ? 'rgba(208,114,119,.13)' : 'none';
  const color = kind === '+' ? 'var(--ok)' : kind === '-' ? 'var(--del)' : 'var(--mu)';
  return `<div class="row mono-font" style="height:19px;font-size:11.5px;background:${bg};white-space:pre"><span class="ph" style="width:44px;text-align:right;padding-right:10px;flex:none">${no}</span><span style="width:12px;flex:none;color:${color}">${kind === ' ' ? '' : kind}</span><span>${text}</span></div>`;
};
const oeFileHead = ({ name = 'src/app/checkout/page.tsx', stat = true, end = '', hl = false } = {}) =>
  `<div class="row g2" style="height:32px;padding:0 10px;background:${hl ? 'var(--hov)' : 'var(--panel)'};border-bottom:1px solid var(--bv);font-size:12px">${ic('chev-down', 'xs mu')}${ic('file', 'xs mu')}<span class="mono-font trunc">${name}</span>${stat ? '<span class="okc xs">+24</span><span class="delc xs">−3</span>' : ''}<span class="grow"></span>${end}<span class="btn ghost sm" style="padding:0 4px">Viewed</span></div>`;
const oeDiffBody = (hot = null) => `<div style="padding:4px 0">${oeDiffLine(12, ' ', 'export default function Checkout() {')}${oeDiffLine(13, '-', '  const total = cart.total')}${oeDiffLine(14, '+', hot === 14 ? '  const total = totals(cart)' : '  const total = totals(cart)')}${oeDiffLine(15, '+', '  const [paying, setPaying] = useState(false)')}${oeDiffLine(16, ' ', '  return (')}</div>`;
const oeDiffPanel = ({ w = 520, h = 260, header, body = oeDiffBody(), overlays = '' } = {}) =>
  frame(`<div class="row" style="height:36px;padding:0 10px;gap:8px;border-bottom:1px solid var(--b);font-size:13px">${ic('diff', 'sm mu')}<span>Working tree</span>${ic('chev-down', 'xs mu')}<span class="grow"></span><span class="xs mu">3 files</span>${ibtn('x')}</div>${header}${body}${overlays}`, { w, h, style: 'border:1px solid var(--b);border-radius:8px' });
