// The second artifacts round's building blocks, over board/mock.js and the artifacts round's
// mock-artifacts.js (the app window, the thread, the browser, the page's header, documents): the
// Artifacts button and its list, the artifacts page, and the design page agentZ draws from an
// agent's variants, all in JetBrains Dark. Only board/mock.js may be used at the top level here:
// decisions.js reads this file without the artifacts round's.

Object.assign(ICONS, {
  board: '<rect width="7" height="9" x="3" y="3" rx="1"/><rect width="7" height="5" x="14" y="3" rx="1"/><rect width="7" height="9" x="14" y="12" rx="1"/><rect width="7" height="5" x="3" y="16" rx="1"/>',
  phone: '<rect width="14" height="20" x="5" y="2" rx="2" ry="2"/><path d="M12 18h.01"/>',
  archive: '<rect width="20" height="5" x="2" y="3" rx="1"/><path d="M4 8v11a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8"/><path d="M10 12h4"/>',
  'hard-drive': '<line x1="22" x2="2" y1="12" y2="12"/><path d="M5.45 5.11 2 12v6a2 2 0 0 0 2 2h16a2 2 0 0 0 2-2v-6l-3.45-6.89A2 2 0 0 0 16.76 4H7.24a2 2 0 0 0-1.79 1.11z"/><line x1="6" x2="6.01" y1="16" y2="16"/><line x1="10" x2="10.01" y1="16" y2="16"/>',
  minus: '<path d="M5 12h14"/>',
  wifi: '<path d="M12 20h.01"/><path d="M8.5 16.43a5 5 0 0 1 7 0"/><path d="M5 12.86a10 10 0 0 1 5.17-2.69"/><path d="M19 12.86a10 10 0 0 0-2.007-1.523"/><path d="M2 8.82a15 15 0 0 1 4.177-2.643"/><path d="M22 8.82a15 15 0 0 0-11.288-3.764"/><path d="m2 2 20 20"/>',
});
GLYPHS.droid = '❋';
const chev = ic('chev-down', 'xs');
const seg = (items, on) => `<span class="seg">${items.map((item) => `<span class="${item === on ? 'on' : ''}">${item}</span>`).join('')}</span>`;
const sw = (on) => `<span class="sw ${on ? 'on' : ''}"><i></i></span>`;
const labelled = (text) => `<div style="margin:0 0 6px;font:12px 'IBM Plex Sans',sans-serif;color:#8d8e91">${text}</div>`;
const stackOf = (items, gap = 16) => `<div style="display:flex;flex-direction:column;gap:${gap}px;width:max-content">${items.map(([label, html]) => `<div>${labelled(label)}${html}</div>`).join('')}</div>`;

// The demo: threads in two projects on two machines, and chats, that published 15 artifacts.
const T2 = {
  plans: { project: 'storefront', mono: 'ST', color: 'g', title: 'Design the plans page', agent: 'Claude Agent', glyph: 'claude', state: 'done', branch: 'plans-page' },
  checkout: { project: 'storefront', mono: 'ST', color: 'g', title: 'Add the checkout page', agent: 'Claude Agent', glyph: 'claude', state: 'working', branch: 'checkout-flow' },
  pricing: { project: 'storefront', mono: 'ST', color: 'g', title: 'Design the pricing', agent: 'Claude Agent', glyph: 'claude', state: 'done', branch: 'pricing' },
  emails: { project: 'storefront', mono: 'ST', color: 'g', title: 'Write the onboarding emails', agent: 'Claude Agent', glyph: 'claude', state: 'done', branch: 'onboarding' },
  notes: { project: 'storefront', mono: 'ST', color: 'g', title: 'Write the 0.9 release notes', agent: 'Factory Droid', glyph: 'droid', state: 'done', branch: 'release-notes', archived: true },
  cart: { project: 'storefront', mono: 'ST', color: 'g', title: 'Add the cart drawer', agent: 'Claude Agent', glyph: 'claude', state: 'done', branch: 'cart-drawer', archived: true },
  limiter: { project: 'api', mono: 'AP', color: 't', title: 'Speed up the rate limiter', agent: 'Codex', glyph: 'codex', state: 'working', branch: 'limiter-buckets', machine: 'devbox' },
  docsHome: { project: 'api', mono: 'AP', color: 't', title: 'Redesign the docs home', agent: 'Codex', glyph: 'codex', state: 'done', branch: 'docs-home', machine: 'devbox' },
  errors: { project: 'api', mono: 'AP', color: 't', title: 'Document the error codes', agent: 'OpenCode', glyph: 'opencode', state: 'done', branch: 'error-codes', machine: 'devbox' },
  postgres: { project: 'api', mono: 'AP', color: 't', title: 'Move to Postgres 17', agent: 'Codex', glyph: 'codex', state: 'done', branch: 'pg17', machine: 'devbox', archived: true },
  async: { chat: true, title: 'Async runtimes', agent: 'Claude Agent', glyph: 'claude', state: 'done' },
  cli: { chat: true, title: 'Naming the CLI', agent: 'Claude Agent', glyph: 'claude', state: 'done' },
  books: { chat: true, title: 'Rust books', agent: 'Claude Agent', glyph: 'claude', state: 'done' },
};
const A2 = [
  { id: 'plans', title: 'Plans page', kind: 'design', thread: T2.plans, version: 1, ago: '2m ago', day: 'Today', variants: 5, topics: 1, decided: 0 },
  { id: 'research', title: 'Competitors’ plans pages', kind: 'doc', thread: T2.plans, version: 1, ago: '10m ago', day: 'Today' },
  { id: 'spec', title: 'Checkout spec', kind: 'doc', thread: T2.checkout, version: 2, ago: '20m ago', day: 'Today' },
  { id: 'bench', title: 'Limiter benchmarks', kind: 'doc', thread: T2.limiter, version: 5, ago: '35m ago', day: 'Today', machine: 'Devbox 1' },
  { id: 'pricing', title: 'Pricing', kind: 'design', thread: T2.pricing, version: 2, ago: '1h ago', day: 'Today', topics: 3, decided: 2 },
  { id: 'async', title: 'Async runtimes compared', kind: 'doc', thread: T2.async, version: 2, ago: '3h ago', day: 'Today' },
  { id: 'dochome', title: 'Docs home', kind: 'design', thread: T2.docsHome, version: 1, ago: 'yesterday', day: 'Yesterday', variants: 3, topics: 1, decided: 1, submitted: true, pick: 'A', machine: 'Devbox 1' },
  { id: 'notes', title: 'Release notes 0.9', kind: 'doc', thread: T2.notes, version: 1, ago: 'yesterday', day: 'Yesterday' },
  { id: 'errors', title: 'Error codes', kind: 'doc', thread: T2.errors, version: 3, ago: '2d ago', day: 'This week', machine: 'Devbox 1' },
  { id: 'emails', title: 'Onboarding emails', kind: 'design', thread: T2.emails, version: 2, ago: '3d ago', day: 'This week', topics: 2, decided: 2, submitted: true },
  { id: 'cli', title: 'CLI name ideas', kind: 'doc', thread: T2.cli, version: 1, ago: '4d ago', day: 'This week' },
  { id: 'headers', title: 'Rate limit headers', kind: 'doc', thread: T2.limiter, version: 1, ago: '5d ago', day: 'This week', machine: 'Devbox 1' },
  { id: 'cart', title: 'Cart drawer', kind: 'design', thread: T2.cart, version: 3, ago: '1w ago', day: 'Earlier', variants: 4, topics: 1, decided: 1, submitted: true, pick: 'C' },
  { id: 'postgres', title: 'Migration plan', kind: 'doc', thread: T2.postgres, version: 2, ago: '2w ago', day: 'Earlier', machine: 'Devbox 1' },
  { id: 'books', title: 'Reading list', kind: 'doc', thread: T2.books, version: 1, ago: '2w ago', day: 'Earlier' },
];
const art = (id) => A2.find((item) => item.id === id);
const countOf = (test) => A2.filter(test).length;
const COUNTS = {
  all: A2.length,
  waiting: countOf((item) => item.kind === 'design' && !item.submitted),
  storefront: countOf((item) => item.thread.project === 'storefront'),
  api: countOf((item) => item.thread.project === 'api'),
  chats: countOf((item) => item.thread.chat),
  mac: countOf((item) => !item.machine),
  devbox: countOf((item) => item.machine),
};

// How an artifact is named in lists: its kind, what it holds, where it came from, its state.
const KIND = { doc: { icon: 'file', name: 'Document' }, design: { icon: 'board', name: 'Design' } };
const kindIc = (item, cls = 'sm') => ic(KIND[item.kind].icon, cls);
const artMeta = (item) => (item.kind === 'doc' ? `Document · v${item.version}` : `Design · ${item.topics > 1 ? `${item.topics} topics` : `${item.variants} variants`} · v${item.version}`);
const fromText = (thread) => (thread.chat ? `Chat · ${thread.title}` : `${thread.project} › ${thread.title}`);
const fromLine = (thread) => (thread.chat ? `${ic('chat', 'xs')}<span class="trunc">Chat · ${thread.title}</span>` : `${mono(thread.mono, thread.color)}<span class="trunc">${thread.project} › ${thread.title}</span>`);
function stateTag(item, { short = false } = {}) {
  if (item.kind !== 'design') return '';
  if (item.submitted) return `<span class="stag2 ok">${ic('check', 'xs')}Decided${item.topics > 1 || short ? '' : ` · ${item.pick}`}</span>`;
  if (item.decided) return `<span class="stag2 ac">${item.decided} of ${item.topics} decided</span>`;
  return '<span class="stag2 ac">Waiting for you</span>';
}
const archivedTag = () => `<span class="stag2">${ic('archive', 'xs')}Archived</span>`;
const machineMark = (item) => (item.machine ? `<span class="row g1 none ph">${ic('server', 'xs')}${item.machine}</span>` : '');

// Pictures for the lists: a design's variants, or a document's first lines.
const VARIANT_IMGS = ['img/design-1.jpg', 'img/design-2.jpg', 'img/design-3.jpg', 'img/design-5.jpg'];
const SKETCH = {
  dochome: `<div class="sk" style="background:#0f1115"><i style="left:8%;top:12%;width:46%;height:10%;background:#e8eaed"></i><i style="left:8%;top:27%;width:62%;height:5%;background:#5b616e"></i><i style="left:8%;top:38%;width:52%;height:10%;background:#22252b;border:1px solid #3a3f48"></i>${[8, 38, 68].map((left) => `<i style="left:${left}%;top:58%;width:25%;height:30%;background:#1a1d22;border:1px solid #2c3038"></i>`).join('')}</div>`,
  emails: `<div class="sk" style="background:#eef0f4"><i style="left:22%;top:0;width:56%;height:100%;background:#fff"></i><i style="left:22%;top:0;width:56%;height:16%;background:#6d5ce8"></i><i style="left:30%;top:24%;width:30%;height:6%;background:#16181d"></i>${[36, 44, 52].map((top) => `<i style="left:30%;top:${top}%;width:40%;height:3.5%;background:#c5c9d2"></i>`).join('')}<i style="left:30%;top:64%;width:18%;height:9%;background:#6d5ce8"></i></div>`,
  cart: `<div class="sk" style="background:#f6f7f9">${[6, 30].map((left) => `<i style="left:${left}%;top:12%;width:20%;height:34%;background:#e3e6eb"></i>`).join('')}<i style="left:0;top:0;width:100%;height:100%;background:rgba(10,12,16,.45);border-radius:0"></i><i style="left:62%;top:0;width:38%;height:100%;background:#fff;border-radius:0"></i>${[10, 30].map((top) => `<i style="left:66%;top:${top}%;width:30%;height:14%;background:#eceef2"></i>`).join('')}<i style="left:66%;top:80%;width:30%;height:10%;background:#16181d"></i></div>`,
};
const DOC_TEXT = {
  research: 'Shopify, Squarespace and Wix each show three or four plans, the yearly price first and the most popular marked.',
  spec: 'The checkout page takes the cart, an address and a payment, and places one order.',
  bench: 'p99 latency per request at 20k requests a second, 8 threads, before and after sharding.',
  async: 'Tokio, smol and Glommio side by side: schedulers, timers, io_uring.',
  notes: 'Checkout is out of beta. Orders go through POST /api/orders, and the cart keeps its items.',
  errors: 'Every error the API returns, with its status, its code and what to do about it.',
  cli: 'Short, easy to type and free on crates.io and npm. Taken: zed, az, agent.',
  headers: 'RateLimit-Limit, RateLimit-Remaining and RateLimit-Reset, as the IETF draft names them.',
  postgres: 'Move the database from Postgres 15 to 17 with logical replication, then switch over.',
  books: 'Rust for Rustaceans, Rust Atomics and Locks, Zero To Production in Rust.',
};
const docThumb = (item) => `<div class="dth"><b>${item.title}</b><p>${DOC_TEXT[item.id] || ''}</p><i style="width:92%"></i><i style="width:78%"></i><i style="width:86%"></i><i style="width:44%"></i></div>`;
function thumbOf(item) {
  if (item.kind === 'doc') return docThumb(item);
  if (item.id === 'plans') return `<div class="mos">${VARIANT_IMGS.map((src) => `<img src="${src}">`).join('')}</div>`;
  if (item.id === 'pricing') return '<img src="img/design-3.jpg">';
  return SKETCH[item.id] || '';
}

// A card on the artifacts page: its picture with its state over it, title, what it is and when,
// and where it came from.
function hcard(item, { archived = 'tag', machine = true, dim = false } = {}) {
  const gone = item.thread.archived;
  const tags = `${machine ? machineMark(item) : ''}${gone && archived === 'tag' ? archivedTag() : ''}${gone && archived === 'dim' ? `<span class="ph row none">${ic('archive', 'xs')}</span>` : ''}`;
  const state = stateTag(item);
  return `<div class="hcard ${dim || (gone && archived === 'dim') ? 'dim' : ''}"><div class="hth">${thumbOf(item)}${state ? `<span class="hstate">${state}</span>` : ''}</div><div class="hcb"><div class="row g15"><span class="mu row none">${kindIc(item)}</span><b class="trunc grow">${item.title}</b></div><div class="hm">${artMeta(item)} · ${item.ago}</div><div class="hw row g15">${fromLine(item.thread)}<span class="grow"></span>${tags}</div></div></div>`;
}
function byDay(items, { cardFn = (item) => hcard(item), columns = 4 } = {}) {
  const days = [...new Set(items.map((item) => item.day))];
  return days.map((day) => `<div class="hday">${day}</div><div class="hgrid" style="grid-template-columns:repeat(${columns},1fr)">${items.filter((item) => item.day === day).map(cardFn).join('')}</div>`).join('');
}

// The artifacts page in the browser: its bar, the places on the left, the cards.
const br2 = (html, opts = {}) => browser(html, { w: 1280, h: 800, ...opts });
const searchField = (text = 'Search artifacts…', { w = 300, typed = false } = {}) => `<div class="field hsearch" style="width:${w}px">${ic('search', 'xs')}<span class="${typed ? '' : 'ph'} sm grow">${text}</span>${typed ? ic('x', 'xs') : '<span class="kbd">/</span>'}</div>`;
const homeBar = ({ search = searchField(), extra = '' } = {}) => `<div class="phd">${zmark()}<span class="ptitle">Artifacts</span><span class="grow"></span>${search}${extra}</div>`;
const homePage = (inner, { h = 800, url = '127.0.0.1:47120/', bar = homeBar(), over = '' } = {}) => br2(`<div class="pg">${bar}<div class="pbody">${inner}</div>${over}</div>`, { title: 'Artifacts', icon: 'layers', url, h });
function homeSide({ on = 'all', machines = false, waiting = true } = {}) {
  const item = (key, icon, name, count) => `<div class="hitem ${key === on ? 'on' : ''}">${icon}<span class="grow trunc">${name}</span><span class="n">${count}</span></div>`;
  return `<div class="hside">${item('all', ic('layers', 'sm'), 'All artifacts', COUNTS.all)}${waiting ? item('waiting', ic('inbox', 'sm'), 'Waiting for you', COUNTS.waiting) : ''}<div class="hsec">Projects</div>${item('storefront', mono('ST', 'g'), 'storefront', COUNTS.storefront)}${item('api', mono('AP', 't'), 'api', COUNTS.api)}${item('chats', ic('chat', 'sm'), 'Chats', COUNTS.chats)}${machines ? `<div class="hsec">Machines</div>${item('mac', ic('laptop', 'sm'), 'This Mac', COUNTS.mac)}${item('devbox', ic('server', 'sm'), 'Devbox 1', COUNTS.devbox)}` : ''}</div>`;
}
const homeHead = (title, count, { tabs = seg(['All', 'Documents', 'Designs'], 'All'), right = '' } = {}) => `<div class="hhead"><span class="ht">${title}</span><span class="ph">${count}</span>${tabs}<span class="grow"></span>${right}<span class="dd">Newest first ${chev}</span></div>`;
function homeA({ on = 'all', machines = false, top = '', items = A2, title = 'All artifacts', count = items.length, cardFn, columns = 4, h, over, bar, head } = {}) {
  return homePage(`<div class="hlay">${homeSide({ on, machines })}<div class="hmain">${top}${head ?? homeHead(title, count)}${byDay(items, { cardFn, columns })}</div></div>`, { h, over, bar });
}

// The app's window with the Artifacts button by the project picker; no Artifacts shelf.
function artButton({ kind = 'label', count = COUNTS.all, open = false } = {}) {
  if (kind === 'none') return '';
  return `<span class="abtn ${open ? 'on' : ''}">${ic('layers', 'sm')}${kind === 'label' ? 'Artifacts' : ''}<span class="n">${count}</span></span>`;
}
function titleBar({ scope = 'All projects', button = 'label', count, open = false } = {}) {
  const picker = `${scope === 'All projects' ? ic('list', 'sm') : mono('ST', 'g')}<span>${scope}</span>${chev}`;
  const counted = count ?? (scope === 'All projects' ? COUNTS.all : COUNTS.storefront);
  const left = button === 'joined'
    ? `<span class="pjoin"><span class="row g1">${picker}</span><span class="row g1 ${open ? 'on' : ''}">${ic('layers', 'sm')}<span class="n">${counted}</span></span></span>`
    : `<span class="row g1 apick">${picker}</span>${artButton({ kind: button, count: counted, open })}`;
  return `<div class="atitle">${lights()}${ic('sidebar', 'sm')}${left}<div class="aseg"><span class="on">Agents</span><span>Workspaces</span></div></div>`;
}
const titleStrip = (opts) => frame(titleBar(opts), { w: 1280, h: 38, style: 'border-radius:8px;border:1px solid #111' }).replace('class="m"', 'class="m jb"');
const storefrontCards = (on = 'plans') => ['plans', 'checkout', 'pricing', 'emails'].map((key) => tcard({ ...T2[key], ago: '' }, { on: key === on })).join('');
const allCards = (on = 'plans') => ['plans', 'limiter', 'checkout', 'pricing'].map((key) => tcard(T2[key], { on: key === on })).join('');
const sidebar2 = (scope = 'storefront') => sidebarJB({ cards: scope === 'storefront' ? storefrontCards() : allCards(), shelves: shelf('Chats', 3) + shelf('Archived', 4) });
function appWin2(threadHtml, { w = 1280, h = 640, scope = 'storefront', button = 'label', open = false, over = '', side } = {}) {
  return frame(`<div class="awin">${titleBar({ scope, button, open })}<div class="abody">${side ?? sidebar2(scope)}${threadHtml}</div></div>${over}`, { w, h, style: 'border-radius:10px;border:1px solid #111' }).replace('class="m"', 'class="m jb"');
}

// The design thread: the ask, the agent's two artifacts and its reply.
function designCard({ meta = 'Design · 5 variants · v1 · published 2m ago', thumbs = false, picked = '', title = 'Plans page' } = {}) {
  return `<div class="acard" style="align-items:flex-start"><span class="kindtile">${ic('board', 'sm')}</span><div class="col grow" style="gap:1px;min-width:0"><span class="ttl trunc">${title}</span><span class="meta trunc">${meta}</span>${thumbs ? `<div class="thumbs">${VARIANTS.map((variant) => `<img src="${variant.img}" class="${variant.key === picked ? 'picked' : ''}">`).join('')}</div>` : ''}</div><span class="btn">${ic('arrow-up-right', 'xs')}Open</span>${ibtn('more')}</div>`;
}
const docCard = (item = art('research'), meta = 'Document · v1 · published 10m ago') => `<div class="acard"><span class="kindtile">${ic('file', 'sm')}</span><div class="col grow" style="gap:1px;min-width:0"><span class="ttl trunc">${item.title}</span><span class="meta trunc">${meta}</span></div><span class="btn">${ic('arrow-up-right', 'xs')}Open</span>${ibtn('more')}</div>`;
const plansAsk = () => bub('Look at how other shops show their plans, then make a few designs for our plans page so I can pick one.');
const plansAfter = () => msg('Five variants of the plans page, built with the site’s components and colors, after what I found on other shops’ pages. Pick one on the design, or take parts of a few, and I’ll build it.');
const plansConvo = ({ card = designCard(), extra = '' } = {}) => plansAsk() + foldRow('Read 6 files and searched the web twice') + docCard() + card + plansAfter() + extra;
const plansThread = (opts = {}) => threadView({ thread: T2.plans, convo: plansConvo(opts), ...opts });
const tpill = (label, { icon = 'layers', open = false } = {}) => `<span class="tpill ${open ? 'on' : ''}">${ic(icon, 'xs')}<span class="trunc">${label}</span>${chev}</span>`;

// A list row for an artifact, in menus and popovers.
const listRow = (item, { hl = false, end, from = false } = {}) => `<div class="it ${hl ? 'hl' : ''}" style="height:${from ? 40 : 28}px">${kindIc(item)}<span class="col grow" style="min-width:0;gap:1px"><span class="trunc">${item.title}</span>${from ? `<span class="xs ph trunc">${fromText(item.thread)}</span>` : ''}</span>${end ?? (stateTag(item, { short: true }) || `<span class="xs ph">${item.ago.replace(' ago', '')}</span>`)}</div>`;

// The variants: Claude Agent's five plans page designs, and today's page.
const VARIANTS = [
  { key: 'A', name: 'Three tiers', img: 'img/design-1.jpg', desc: 'Three plans in cards, Growth marked most popular, each with what it includes.', good: 'Familiar; the middle plan stands out.', cost: 'Comparing one feature means reading three lists.' },
  { key: 'B', name: 'Compare table', img: 'img/design-2.jpg', desc: 'Every feature in one table, a column a plan, with Monthly and Yearly.', good: 'The easiest to compare.', cost: 'Long, and cramped on a phone.' },
  { key: 'C', name: 'Slider by size', img: 'img/design-3.jpg', desc: 'One plan priced by how many products you sell: slide to your size.', good: 'No tiers to weigh.', cost: 'You can’t see what a big shop pays without sliding.' },
  { key: 'D', name: 'One plan', img: 'img/design-4.jpg', desc: 'A single plan, 2% of each sale, with everything in it.', good: 'Nothing to choose.', cost: 'Hides that large shops can ask for a deal.' },
  { key: 'E', name: 'Stories first', img: 'img/design-5.jpg', desc: 'Shops’ quotes on a dark band, then a short list of the plans.', good: 'Sells before it prices.', cost: 'The prices are below the fold.' },
];
const TODAY_VARIANT = { key: 'T', today: true, name: 'Today’s page', img: 'img/design-0.jpg', desc: 'The plans page as it is: a plain table of each plan’s name, price and features, a Sign up link under it.', good: 'Nothing to build.', cost: 'Hard to compare; no prices in view at the top.' };
const PHONES = { A: 'img/design-1-phone.jpg', D: 'img/design-4-phone.jpg' };
const TODAY_TEXT = 'The plans page is a plain table: each plan’s name, price and features in a row, and a Sign up link under it. Built from <code class="c">src/app/plans/page.tsx</code>.';
const PRICING_TOPICS = [
  { n: 1, title: 'The plans page', label: 'B' },
  { n: 2, title: 'The plan picker in checkout', label: 'A' },
  { n: 3, title: 'The yearly discount', label: null, comment: true },
];

// The design page agentZ draws: one bar (the page's and the design's), the topics on the left
// when there are several, then the topic: its title, how to answer, and the variants' cards.
function designBar({ title = 'Plans page', version = 1, latest = version, where = 'storefront › Design the plans page', decided = 0, total = 1, saved = 'Saved', buttons } = {}) {
  const tag = version === latest ? `v${version}` : `v${version} of ${latest}`;
  const progress = `<span class="bd-prog"><span class="bd-bar"><i style="width:${(100 * decided) / total}%"></i></span>${decided} of ${total} decided</span>`;
  const submit = `<span class="btn ${decided === total ? 'primary' : ''}">${ic('check', 'xs')}Submit</span>`;
  return `<div class="phd">${ibtn('home')}<span class="ph">/</span><span class="ptitle">${title}</span><span class="vtag ${version === latest ? '' : 'ac'}">${tag}</span>${chev}<span class="sm ph trunc row g1" style="margin-left:4px">${glyph('claude', 'sm')} ${where}</span><span class="grow"></span>${progress}${saved ? `<span class="bd-saved">${saved}</span>` : ''}${buttons ?? submit}</div>`;
}
const page = (bar, body, over = '') => `<div class="pg">${bar}<div class="pbody">${body}</div>${over}</div>`;
const board = ({ nav = '', body = '' } = {}) => `<div class="bd"><div class="bd-body">${nav}<div class="bd-page">${body}</div></div></div>`;
function boardNav(topics, { on = 1, section = 'Pricing' } = {}) {
  const rows = topics.map((topic) => `<a class="${topic.n === on ? 'on' : ''}"><span class="n">${topic.n}</span><span class="t">${topic.title}</span>${topic.comment ? '<span class="c"></span>' : ''}${topic.label ? `<span class="s">${topic.label}</span>` : ''}</a>`).join('');
  return `<div class="bd-nav"><h5>${section}</h5>${rows}<div class="sep"></div><a class="${on === 'summary' ? 'on' : ''}"><span class="n">∑</span><span class="t">Summary</span></a></div>`;
}
const topicHead = ({ crumb = '', title = 'The plans page', kind = '<b>Pick one</b>, then <b>+ Also</b> on any others to take parts of. Keys: <b>1–5</b> pick.' } = {}) => `${crumb ? `<div class="bd-crumb">${crumb}</div>` : ''}<div class="bd-h">${title}</div><div class="bd-kind">${kind}</div>`;
const todayBox = (text = TODAY_TEXT, img = 'img/design-0.jpg') => `<div class="bd-today"><div><h6>Today</h6><p>${text}</p></div>${img ? `<img src="${img}">` : ''}</div>`;
function card(variant, { picked = false, also = false, comment = '', rec = false, tags = '', preview, desc = true, pc = true, foot = true } = {}) {
  const pickText = picked ? '✓ Picked' : variant.today ? 'Keep today’s' : `Pick ${variant.key}`;
  const letter = variant.today ? `<span class="bd-letter">${ic('history', 'xs')}</span>` : `<span class="bd-letter">${variant.key}</span>`;
  return `<div class="bd-opt ${picked ? 'picked' : ''} ${also ? 'also' : ''}">
    <div class="bd-oh">${letter}<span class="bd-oname">${variant.name}</span>${rec ? '<span class="bd-tag rec">Recommended</span>' : ''}${variant.today ? '<span class="bd-tag">Today</span>' : ''}${tags}</div>
    ${desc ? `<p class="bd-desc">${variant.desc}</p>` : ''}
    ${pc ? `<div class="bd-pc"><b>Good</b><span>${variant.good}</span><b>Cost</b><span>${variant.cost}</span></div>` : ''}
    ${preview ?? `<div class="bd-fit"><img src="${variant.img}"></div>`}
    ${foot ? `<div class="bd-foot"><span class="bd-pick">${pickText}</span>${picked || variant.today ? '' : `<span class="bd-also">${also ? '✓ Also' : '+ Also'}</span>`}<span class="bd-cm ${comment ? 'typed' : ''}">${comment || `Comment on ${variant.today ? 'today’s' : variant.key}…`}</span></div>` : ''}
  </div>`;
}
const grid = (cards, columns = 3) => `<div class="bd-grid" style="grid-template-columns:repeat(${columns},1fr)">${cards}</div>`;
const variantCards = (options = {}, list = VARIANTS) => list.map((variant) => card(variant, { rec: variant.key === 'B', picked: variant.key === options.picked, also: (options.also || []).includes(variant.key), comment: (options.comments || {})[variant.key] || '', ...(options.each ? options.each(variant) : {}) })).join('');
const PLANS_URL = '127.0.0.1:47120/a/5d1c9e';
const designPage = ({ bar = designBar(), nav = '', body, over = '', h = 800, title = 'Plans page', inner, url = PLANS_URL } = {}) => br2(page(bar, inner ?? board({ nav, body }), over), { title, icon: 'board', url, h });
const plansDesign = ({ body, bar, over, h, inner } = {}) => designPage({ bar, over, h, inner, body: body ?? topicHead() + grid(variantCards()) });
const pricingBar = (decided = 2, opts = {}) => designBar({ title: 'Pricing', where: 'storefront › Design the pricing', version: 2, decided, total: 3, ...opts });
const pricingDesign = ({ on = 1, body, topics = PRICING_TOPICS, decided = 2, h, nav = true, bar, over } = {}) => designPage({ title: 'Pricing', url: '127.0.0.1:47120/a/c41f08', bar: bar ?? pricingBar(decided), nav: nav ? boardNav(topics, { on }) : '', body, h, over });

// Code: the agent's calls and what agentZ answers, highlighted as the app's code blocks are.
function highlight(text) {
  return text.split('\n').map((line) => {
    const escaped = line.replace(/[&<>]/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;' })[c]);
    if (/^#/.test(line)) return `<span class="hd">${escaped}</span>`;
    if (/^\s*\/\//.test(line)) return `<span class="cm2">${escaped}</span>`;
    // Strings first: the spans added after them have quoted attributes of their own.
    return escaped
      .replace(/"([^"\n]*)"(\s*:)?/g, (match, inner, colon) => (colon ? `<span class="k">"${inner}"</span>${colon}` : `<span class="s">"${inner}"</span>`))
      .replace(/^(\s*)(agentz_\w+)/, '$1<span class="fn">$2</span>')
      .replace(/(→ .*)$/, '<span class="cm2">$1</span>');
  }).join('\n');
}
const codeFrame = (blocks, { w = 780, h = 30 + blocks.reduce((sum, [, text]) => sum + 56 + 18.6 * text.split('\n').length, 0) } = {}) => frame(`<div class="code">${blocks.map(([label, text]) => `<div class="lbl">${label}</div><pre>${highlight(text)}</pre>`).join('')}</div>`, { w, h, style: 'border-radius:8px;border:1px solid #111' }).replace('class="m"', 'class="m jb"');
const agentCalls = (tool, who = 'Claude Agent') => `${glyph(who === 'Codex' ? 'codex' : 'claude', 'sm')} ${who} calls ${tool}`;

// The @ menu (mention_menu.rs): labeled groups of rows, each with an icon, a name and a hint.
const mchip = (icon, label) => `<span class="mchip">${icon}${label}</span>`;
const mrow = (icon, name, hint = '', { hl = false } = {}) => `<div class="it ${hl ? 'hl' : ''}">${icon}<span class="grow trunc">${name}</span>${hint ? `<span class="xs" style="color:var(--ph);padding-left:12px">${hint}</span>` : ''}</div>`;
const mgroup = (label, rows) => `<div class="lbl">${label}</div>${rows.join('')}`;
const mentionMenu = (groups, { style = 'left:calc(50% - 356px);bottom:96px', width = 420 } = {}) => `<div class="menu" style="${style};width:${width}px">${groups.join('')}</div>`;

// Settings (settings_page.rs): the sections on the left, the page's groups of rows.
const SETTINGS_NAV = [['General', 'settings'], ['Appearance', 'eye'], ['Notifications', 'bell'], ['Agents', 'bot'], ['Usage', 'activity'], ['Skills', 'note'], ['MCP Servers', 'zap'], ['Machines', 'monitor'], ['Projects', 'folder'], ['Storage', 'hard-drive']];
function settings2(body, { section = 'General', w = 1000, h = 620, width = 620 } = {}) {
  return frame(`<div class="row" style="height:100%;align-items:stretch;position:relative">
    <div class="asb"><div class="asb-head"><span class="grow">Settings</span>${ic('x', 'sm mu')}</div>
      <div class="col" style="padding:2px 4px;gap:2px">${SETTINGS_NAV.map(([name, icon]) => `<div class="lrow ${name === section ? 'on' : ''}">${ic(icon, 'sm mu')}${name}</div>`).join('')}</div></div>
    <div class="grow" style="padding:22px 0;overflow:hidden;background:var(--ed);position:relative"><div style="max-width:${width}px;margin:0 auto;padding:0 20px">${body}</div></div></div>`, { w, h, style: 'border-radius:8px;border:1px solid #111' }).replace('class="m"', 'class="m jb"');
}
const settingRow = (name, description, control) => `<div class="srowx"><div class="grow"><div class="n">${name}</div><div class="d">${description}</div></div>${control}</div>`;
const settingGroup = (label, rows, { right = '' } = {}) => `<div class="sgroup"><span>${label}</span><span class="grow"></span>${right}</div><div class="scard">${rows.join('')}</div>`;
const dropdown = (text) => `<span class="dd" style="border:1px solid var(--b)">${text}${chev}</span>`;
const stepper = (value) => `<span class="stepper">${ic('minus', 'xs')}<b>${value}</b>${ic('plus', 'xs')}</span>`;
