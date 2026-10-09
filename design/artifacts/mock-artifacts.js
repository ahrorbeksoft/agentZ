// The artifacts round's building blocks, over board/mock.js: the app (sidebar, thread, composer)
// in JetBrains Dark, and published pages in a browser window, in the same theme.

Object.assign(ICONS, {
  'app-window': '<rect x="2" y="4" width="20" height="16" rx="2"/><path d="M10 4v4"/><path d="M2 8h20"/><path d="M6 4v4"/>',
  'file-code': '<path d="M10 12.5 8 15l2 2.5"/><path d="m14 12.5 2 2.5-2 2.5"/><path d="M14 2v4a2 2 0 0 0 2 2h4"/><path d="M15 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V7z"/>',
  download: '<path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/><polyline points="7 10 12 15 17 10"/><line x1="12" x2="12" y1="15" y2="3"/>',
  printer: '<path d="M6 18H4a2 2 0 0 1-2-2v-5a2 2 0 0 1 2-2h16a2 2 0 0 1 2 2v5a2 2 0 0 1-2 2h-2"/><path d="M6 9V3a1 1 0 0 1 1-1h10a1 1 0 0 1 1 1v6"/><rect x="6" y="14" width="12" height="8" rx="1"/>',
  code: '<polyline points="16 18 22 12 16 6"/><polyline points="8 6 2 12 8 18"/>',
  send: '<path d="M14.54 21.69a.5.5 0 0 0 .94-.03l6.5-19a.5.5 0 0 0-.64-.64l-19 6.5a.5.5 0 0 0-.03.94l7.93 3.18a2 2 0 0 1 1.11 1.11z"/><path d="m21.85 2.15-10.94 10.94"/>',
  'arrow-up-right': '<path d="M7 7h10v10"/><path d="M7 17 17 7"/>',
  'arrow-left': '<path d="m12 19-7-7 7-7"/><path d="M19 12H5"/>',
  'arrow-right': '<path d="M5 12h14"/><path d="m12 5 7 7-7 7"/>',
  home: '<path d="M15 21v-8a1 1 0 0 0-1-1h-4a1 1 0 0 0-1 1v8"/><path d="M3 10a2 2 0 0 1 .709-1.528l7-5.999a2 2 0 0 1 2.582 0l7 5.999A2 2 0 0 1 21 10v9a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z"/>',
  refresh: '<path d="M3 12a9 9 0 0 1 9-9 9.75 9.75 0 0 1 6.74 2.74L21 8"/><path d="M21 3v5h-5"/><path d="M21 12a9 9 0 0 1-9 9 9.75 9.75 0 0 1-6.74-2.74L3 16"/><path d="M8 16H3v5"/>',
  lock: '<rect width="18" height="11" x="3" y="11" rx="2" ry="2"/><path d="M7 11V7a5 5 0 0 1 10 0v4"/>',
  moon: '<path d="M12 3a6 6 0 0 0 9 9 9 9 0 1 1-9-9Z"/>',
  sun: '<circle cx="12" cy="12" r="4"/><path d="M12 2v2"/><path d="M12 20v2"/><path d="m4.93 4.93 1.41 1.41"/><path d="m17.66 17.66 1.41 1.41"/><path d="M2 12h2"/><path d="M20 12h2"/><path d="m6.34 17.66-1.41-1.41"/><path d="m19.07 4.93-1.41 1.41"/>',
  chart: '<path d="M3 3v16a2 2 0 0 0 2 2h16"/><path d="M18 17V9"/><path d="M13 17V5"/><path d="M8 17v-3"/>',
  sheet: '<rect width="18" height="18" x="3" y="3" rx="2"/><path d="M3 9h18"/><path d="M3 15h18"/><path d="M9 3v18"/>',
  presentation: '<path d="M2 3h20"/><path d="M21 3v11a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V3"/><path d="m7 21 5-5 5 5"/>',
  'panel-left': '<rect width="18" height="18" x="3" y="3" rx="2"/><path d="M9 3v18"/>',
  'chev-left': '<path d="m15 18-6-6 6-6"/>',
});

const zmark = () => '<span class="zmark">Z</span>';

// The demo: threads that published pages, and a chat that did.
const THREADS = {
  checkout: { project: 'storefront', mono: 'ST', color: 'g', title: 'Add the checkout page', agent: 'Claude Agent', glyph: 'claude', state: 'done', branch: 'checkout-flow', ago: '2m' },
  limiter: { project: 'api', mono: 'AP', color: 't', title: 'Speed up the rate limiter', agent: 'Codex', glyph: 'codex', state: 'working', branch: 'limiter-buckets', machine: 'devbox', ago: 'now' },
  notes: { project: 'storefront', mono: 'ST', color: 'g', title: 'Write the 0.9 release notes', agent: 'Factory Droid', glyph: 'droid', state: 'done', branch: 'release-notes', ago: '1h' },
  chat: { chat: true, title: 'Async runtimes', agent: 'Claude Agent', glyph: 'claude', state: 'done', ago: '3d' },
};
GLYPHS.droid = '❋';
const ARTS = [
  { id: 'layouts', title: 'Checkout layouts', kind: 'html', version: 3, thread: THREADS.checkout, ago: '2m ago', slug: '7f3a9c' },
  { id: 'bench', title: 'Limiter benchmarks', kind: 'html', version: 5, thread: THREADS.limiter, ago: '10m ago', slug: 'b81e04', machine: 'Devbox 1' },
  { id: 'notes', title: 'Release notes 0.9', kind: 'md', version: 1, thread: THREADS.notes, ago: '1h ago', slug: '0c55d2' },
  { id: 'async', title: 'Async runtimes compared', kind: 'md', version: 2, thread: THREADS.chat, ago: '3d ago', slug: 'e2971a' },
];
const kindIcon = (art) => (art.kind === 'md' ? 'file' : art.id === 'bench' ? 'chart' : 'app-window');
const where = (thread) => (thread.chat ? `Chat · ${thread.title}` : `${thread.project} › ${thread.title}`);

// The app's window: title bar, sidebar, thread.
function appWin(sidebarHtml, threadHtml, { w = 1280, h = 760, style = '' } = {}) {
  return frame(`<div class="awin">
    <div class="atitle">${lights()}${ic('sidebar', 'sm')}<span class="row g1">${ic('list', 'sm')}All projects${ic('chev-down', 'xs')}</span><div class="aseg"><span class="on">Agents</span><span>Workspaces</span></div></div>
    <div class="abody">${sidebarHtml}${threadHtml}</div></div>`, { w, h, style: `border-radius:10px;border:1px solid #111;${style}` }).replace('class="m"', 'class="m jb"');
}

// A thread card (sidebar.rs): project and state, title, branch and icons.
function tcard(thread, { on = false } = {}) {
  const state = thread.state === 'working' ? `<span class="pill working">${dot('working')}Working</span>` : `<span class="pill done">${dot('done')}Completed</span>`;
  const proj = thread.chat ? `${ic('chat', 'xs')}<span>Chat</span>` : `${mono(thread.mono, thread.color)}<span>${thread.project}</span>`;
  return `<div class="tcard ${on ? 'on' : ''}"><div class="p">${proj}<span class="grow"></span>${state}</div><div class="tl trunc">${thread.title}</div><div class="br"><span class="grow trunc">${thread.branch || ''}</span>${ic(thread.machine === 'devbox' ? 'server' : 'laptop', 'xs')}${glyph(thread.glyph, 'sm')}</div></div>`;
}
const shelf = (name, count, { open = false, extra = '', hl = false } = {}) => `<div class="shelf" style="${hl ? 'color:var(--t)' : ''}">${extra}<span>${name}${count != null ? ` (${count})` : ''}</span><span class="rule"></span>${ic(open ? 'chev-up' : 'chev-down', 'xs')}</div>`;
function sidebarJB({ cards = defaultCards(), shelves = shelf('Archived', 4), foot = '', list = '' } = {}) {
  return `<div class="asb"><div class="asb-head">${ic('search', 'sm')}<span class="grow">Search…</span>${ic('plus', 'sm mu')}</div>
    <div class="asb-list">${cards}${list}</div>${shelves}<div class="asb-foot">${ic('settings', 'sm')}Settings${foot}</div></div>`;
}
const defaultCards = (on = 'checkout') => ['checkout', 'limiter', 'notes'].map((key) => tcard(THREADS[key], { on: key === on })).join('');

// The thread: header, conversation, whatever sits over the composer, the composer.
function threadView({ thread = THREADS.checkout, convo = '', over = '', headExtra = '', composerTop = '', style = '' } = {}) {
  const lead = thread.chat ? `${ic('chat', 'sm')}<span>Chat</span>` : `${mono(thread.mono, thread.color)}<span>${thread.project}</span>`;
  return `<div class="thread" style="${style}">
    <div class="thead">${lead}<span class="ph">/</span><span class="tt">${thread.title}</span>${ic('chev-down', 'xs')}<span class="grow"></span>${headExtra}${thread.branch ? `<span class="row g1 sm">${ic('branch', 'xs')}${thread.branch}</span>` : ''}${ibtn('diff')}${ibtn('terminal')}${ibtn('more')}</div>
    <div class="convo"><div class="column">${convo}</div></div>
    ${over}
    <div class="composer"><div class="in">${composerTop}<div class="ph" style="padding:2px 0 12px">Message the agent…</div>
      <div class="row" style="gap:12px;font-size:12px;color:var(--mu)">${ic('plus', 'sm')}<span class="row g1">${glyph(thread.glyph, 'sm')}${thread.agent}</span><span class="grow"></span><span>Default ⌄</span><span>Opus 5.5 ⌄</span><span>High ⌄</span><span class="ibtn" style="background:var(--sel);color:var(--t)">${ic('send', 'xs')}</span></div></div></div>
  </div>`;
}
// A thread on its own, without the window.
const threadFrame = (opts, { w = 900, h = 560 } = {}) => frame(threadView(opts), { w, h, style: 'border-radius:8px;border:1px solid #111' }).replace('class="m"', 'class="m jb"');

const bub = (text) => `<div class="bub">${text}</div>`;
const msg = (html) => `<div class="msg">${html}</div>`;
const worked = (text = 'Worked for 1m 12s') => `<div class="worked">${text}</div>`;
const trow = (icon, label, { trailing = '', style = '' } = {}) => `<div class="trow" style="${style}"><span class="ico">${icon}</span><span class="row grow" style="gap:4px;min-width:0">${label}</span>${trailing}</div>`;
const subj = (text) => `<span class="subj trunc">${text}</span>`;
const openLink = (text = 'Open') => `<span class="lnk">${text}${ic('arrow-up-right', 'xs')}</span>`;
const foldRow = (text) => trow(ic('chev-right', 'xs'), `<span class="trunc">${text}</span>`);
const ownRow = (verb, subject, trailing = '') => trow(zmark(), `<span class="none">${verb}</span>${subject ? subj(subject) : ''}`, { trailing });
const spinner = () => '<span class="spin" style="border-color:rgba(84,138,247,.3);border-top-color:var(--ac)"></span>';

// The checkout thread up to the publish, and after it.
const CHECKOUT_ASK = bub('Show me three layouts for the checkout page side by side, so I can pick one.');
const CHECKOUT_WORK = foldRow('Read 4 files and searched once');
const CHECKOUT_AFTER = msg('Three layouts, each with the cart summary and the pay button. Pick one on the page and send it back; I’ll build that one.');

// The browser around a page, with its tab and address.
function browser(page, { title = 'Checkout layouts', icon = 'app-window', url = '127.0.0.1:47120/a/7f3a9c', w = 1100, h = 700, tabs = '', style = '' } = {}) {
  return `<div class="m jb br" style="width:${w}px;height:${h}px;${style}">
    <div class="brtabs"><div class="brtab on">${ic(icon, 'xs')}<span class="grow trunc">${title}</span>${ic('x', 'xs')}</div>${tabs}</div>
    <div class="brbar">${ic('arrow-left', 'sm')}${ic('arrow-right', 'sm')}${ic('refresh', 'sm')}<div class="omni">${ic('lock', 'xs')}<span>${url.split('/')[0]}<span class="dimurl">/${url.split('/').slice(1).join('/')}</span></span></div>${ic('more', 'sm')}</div>
    <div class="brpage">${page}</div></div>`;
}

// The page's header, as the header topic's A: all pages, the title with its version and
// where it came from, then Export and Send to thread.
function pageHead(art = ARTS[0], { send = true, exportOpen = false, version = art.version, latest = art.version, extra = '' } = {}) {
  return `<div class="phd">${ibtn('home')}<span class="ph">/</span><span class="ptitle">${art.title}</span><span class="vtag ${version === latest ? '' : 'ac'}">v${version}${version === latest ? '' : ` of ${latest}`}</span>${ic('chev-down', 'xs')}<span class="sm ph trunc" style="margin-left:6px">${glyph(art.thread.glyph, 'sm')} ${where(art.thread)}</span><span class="grow"></span>${extra}<span class="btn ${exportOpen ? 'on' : ''}" style="background:var(--panel)">${ic('download', 'xs')}Export${ic('chev-down', 'xs')}</span>${send ? `<span class="btn primary">${ic('send', 'xs')}Send to thread</span>` : ''}</div>`;
}
const pageShell = (head, body, { banner = '', over = '' } = {}) => `<div class="pg">${head}${banner}<div class="pbody">${body}</div>${over}</div>`;

// The sample pages.
function phoneMock(kind) {
  const rows = { a: ['78%', '62%', '70%', '40%'], b: ['50%', '90%', '90%', '60%'], c: ['86%', '30%', '66%', '52%'] }[kind];
  const side = kind === 'b' ? '<div class="row g1"><div class="blk" style="flex:1;height:52px"></div><div class="blk" style="flex:1;height:52px"></div></div>' : '';
  return `<div class="phone">${side}${rows.map((w) => `<div class="blk" style="width:${w}"></div>`).join('')}<div class="blk" style="height:34px;margin-top:6px"></div><div class="blk ac"></div></div>`;
}
const LAYOUTS = [
  { key: 'a', name: 'One column', note: 'Cart, address and pay button in one scroll.' },
  { key: 'b', name: 'Summary beside', note: 'Totals stay beside the form on wide screens.' },
  { key: 'c', name: 'Steps', note: 'Address, then payment, then review.' },
];
function layoutsBody({ picked = 'b', comment = 'Keep the totals sticky on phones too.' } = {}) {
  return `<div class="pin"><div class="h1">Three checkout layouts</div><div class="lead">Built from <code class="c">src/app/checkout/</code> and the cart’s components. Pick one and say what to change.</div>
    <div class="opts3">${LAYOUTS.map((l) => `<div class="ocard ${l.key === picked ? 'on' : ''}"><div class="row g2"><span class="radio ${l.key === picked ? 'on' : ''}"></span><b style="font-weight:600">${l.name}</b></div>${phoneMock(l.key)}<div class="sm mu">${l.note}</div><div class="ta" style="${l.key === picked && comment ? 'color:var(--t)' : ''}">${l.key === picked && comment ? comment : 'Comment…'}</div></div>`).join('')}</div></div>`;
}
const BENCH = [['Token bucket (before)', 41.2, 210], ['Token bucket, sharded', 12.8, 66], ['Sliding window', 18.5, 95], ['Fixed window', 9.6, 49]];
function benchBody({ rows = BENCH, note = 'p99 latency per request at 20k requests a second, 8 threads.' } = {}) {
  return `<div class="pin"><div class="h1">Limiter benchmarks</div><div class="lead">${note}</div>
    <table class="ptable"><tr><th>Limiter</th><th>p99 (µs)</th><th style="width:45%"></th></tr>${rows.map(([n, v, w]) => `<tr><td>${n}</td><td>${v}</td><td><span class="hbar" style="width:${w}px;${n.includes('before') ? 'background:var(--ph)' : ''}"></span></td></tr>`).join('')}</table></div>`;
}
const NOTES_MD = `<h1>Release notes 0.9</h1>
<p>Checkout is out of beta. Orders go through <code>POST /api/orders</code>, and the cart keeps its items across devices.</p>
<h2>New</h2>
<ul><li><b>One-page checkout</b> with the totals beside the form.</li><li>Saved addresses, picked with the arrow keys.</li><li>Apple Pay and Google Pay where the browser has them.</li></ul>
<h2>Changed</h2>
<pre><code><span class="kw">export default function</span> <span class="fn">Checkout</span>() {
  <span class="kw">const</span> cart = <span class="fn">useCart</span>()  <span class="cm">// now synced</span>
  <span class="kw">return</span> &lt;CheckoutLayout cart={cart} /&gt;
}</code></pre>
<table><tr><th>Page</th><th>Before</th><th>After</th></tr><tr><td>/cart</td><td>1.9 s</td><td>0.8 s</td></tr><tr><td>/checkout</td><td>2.4 s</td><td>1.1 s</td></tr></table>`;
const ASYNC_MD = `<h1>Async runtimes compared</h1>
<table><tr><th></th><th>Tokio</th><th>smol</th><th>Glommio</th></tr><tr><td>Scheduler</td><td>Work stealing</td><td>Work stealing</td><td>Thread per core</td></tr><tr><td>Timers</td><td>Wheel</td><td>Heap</td><td>Wheel</td></tr><tr><td>io_uring</td><td>Optional</td><td>No</td><td>Yes</td></tr></table>
<h2>When to pick which</h2><ul><li><b>Tokio</b> for servers with many short tasks.</li><li><b>smol</b> for small tools and libraries.</li></ul>`;
const SPEC_MD = `<h1>Checkout spec</h1><p>The checkout page takes the cart, an address and a payment, and places one order.</p>
<h2>Flow</h2><ul><li>The cart summary is shown beside the form.</li><li>The pay button is enabled once the address is valid.</li></ul>
<h2>API</h2><pre><code>POST /api/orders { cart_id, address, payment }</code></pre>`;
const docBody = (art) => `<div class="pin doc">${art.id === 'async' ? ASYNC_MD : art.id === 'spec' ? SPEC_MD : NOTES_MD}</div>`;
