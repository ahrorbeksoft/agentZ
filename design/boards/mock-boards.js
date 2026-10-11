// The boards round's building blocks, over board/mock.js: a page in the browser with agentZ's
// 44px bar (pages.rs), the board agentZ would draw inside it (the design board's parts, in the
// app's theme), and the thread's card for it, all in JetBrains Dark.

Object.assign(ICONS, {
  'app-window': '<rect x="2" y="4" width="20" height="16" rx="2"/><path d="M10 4v4"/><path d="M2 8h20"/><path d="M6 4v4"/>',
  download: '<path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/><polyline points="7 10 12 15 17 10"/><line x1="12" x2="12" y1="15" y2="3"/>',
  send: '<path d="M14.54 21.69a.5.5 0 0 0 .94-.03l6.5-19a.5.5 0 0 0-.64-.64l-19 6.5a.5.5 0 0 0-.03.94l7.93 3.18a2 2 0 0 1 1.11 1.11z"/><path d="m21.85 2.15-10.94 10.94"/>',
  'arrow-left': '<path d="m12 19-7-7 7-7"/><path d="M19 12H5"/>',
  'arrow-right': '<path d="M5 12h14"/><path d="m12 5 7 7-7 7"/>',
  'arrow-up-right': '<path d="M7 7h10v10"/><path d="M7 17 17 7"/>',
  home: '<path d="M15 21v-8a1 1 0 0 0-1-1h-4a1 1 0 0 0-1 1v8"/><path d="M3 10a2 2 0 0 1 .709-1.528l7-5.999a2 2 0 0 1 2.582 0l7 5.999A2 2 0 0 1 21 10v9a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z"/>',
  refresh: '<path d="M3 12a9 9 0 0 1 9-9 9.75 9.75 0 0 1 6.74 2.74L21 8"/><path d="M21 3v5h-5"/><path d="M21 12a9 9 0 0 1-9 9 9.75 9.75 0 0 1-6.74-2.74L3 16"/><path d="M8 16H3v5"/>',
  lock: '<rect width="18" height="11" x="3" y="11" rx="2" ry="2"/><path d="M7 11V7a5 5 0 0 1 10 0v4"/>',
  'chev-left': '<path d="m15 18-6-6 6-6"/>',
  board: '<rect width="7" height="9" x="3" y="3" rx="1"/><rect width="7" height="5" x="14" y="3" rx="1"/><rect width="7" height="9" x="14" y="12" rx="1"/><rect width="7" height="5" x="3" y="16" rx="1"/>',
  phone: '<rect width="14" height="20" x="5" y="2" rx="2" ry="2"/><path d="M12 18h.01"/>',
});

// The demo: Claude Agent built five designs for storefront's plans page as one board, and a
// checkout board with three topics.
const BOARD = { title: 'Plans page', where: 'storefront › Design the plans page', version: 1, url: '127.0.0.1:47120/a/5d1c9e' };
const DESIGNS = [
  { key: 'A', name: 'Three tiers', img: 'img/design-1.jpg', desc: 'Three plans in cards, Growth marked most popular, each with what it includes.', good: 'Familiar; the middle plan stands out.', cost: 'Comparing one feature means reading three lists.' },
  { key: 'B', name: 'Compare table', img: 'img/design-2.jpg', desc: 'Every feature in one table, a column a plan, with Monthly and Yearly.', good: 'The easiest to compare.', cost: 'Long, and cramped on a phone.' },
  { key: 'C', name: 'Slider by size', img: 'img/design-3.jpg', desc: 'One plan priced by how many products you sell: slide to your size.', good: 'No tiers to weigh.', cost: 'You can’t see what a big shop pays without sliding.' },
  { key: 'D', name: 'One plan', img: 'img/design-4.jpg', desc: 'A single plan, 2% of each sale, with everything in it.', good: 'Nothing to choose.', cost: 'Hides that large shops can ask for a deal.' },
  { key: 'E', name: 'Stories first', img: 'img/design-5.jpg', desc: 'Shops’ quotes on a dark band, then a short list of the plans.', good: 'Sells before it prices.', cost: 'The prices are below the fold.' },
];
const TODAY_TEXT = 'The plans page is a plain table: each plan’s name, price and features in a row, and a Sign up link under it. Built from <code class="c">src/app/plans/page.tsx</code>.';
const CHECKOUT_TOPICS = [
  { n: 1, title: 'The plans page', label: 'B' },
  { n: 2, title: 'The plan picker in checkout', label: 'A' },
  { n: 3, title: 'The yearly discount', label: null, comment: true },
];

// The browser around a page, with its tab and address.
function browser(page, { title = BOARD.title, icon = 'board', url = BOARD.url, w = 1280, h = 800, tabs = '' } = {}) {
  return `<div class="m jb br" style="width:${w}px;height:${h}px">
    <div class="brtabs"><div class="brtab on">${ic(icon, 'xs')}<span class="grow trunc">${title}</span>${ic('x', 'xs')}</div>${tabs}</div>
    <div class="brbar">${ic('arrow-left', 'sm')}${ic('arrow-right', 'sm')}${ic('refresh', 'sm')}<div class="omni">${ic('lock', 'xs')}<span>${url.split('/')[0]}<span class="dimurl">/${url.split('/').slice(1).join('/')}</span></span></div>${ic('more', 'sm')}</div>
    <div class="brpage">${page}</div></div>`;
}

// agentZ's bar over every page (pages.rs): All artifacts, the title, its version, where it came
// from, Export and Send to thread.
function pageBar({ title = BOARD.title, version = BOARD.version, latest = version, where = BOARD.where, send = true, extra = '' } = {}) {
  const tag = version === latest ? `v${version}` : `v${version} of ${latest}`;
  return `<div class="phd">${ibtn('home')}<span class="ph">/</span><span class="ptitle">${title}</span><span class="vtag ${version === latest ? '' : 'ac'}">${tag}</span><span class="sm ph trunc row g1" style="margin-left:4px">${glyph('claude', 'sm')} ${where}</span><span class="grow"></span>${extra}<span class="btn">${ic('download', 'xs')}Export</span>${send ? `<span class="btn primary">${ic('send', 'xs')}Send to thread</span>` : ''}</div>`;
}
const page = (bar, body, over = '') => `<div class="pg">${bar}<div class="pbody">${body}</div>${over}</div>`;

// The board's own bar: its title, how many topics are decided, saving, then its buttons.
function boardTop({ decided = 0, total = 1, saved = 'Saved', buttons = '<span class="bd-tbtn">Only undecided</span><span class="bd-tbtn">Summary</span>', title = BOARD.title } = {}) {
  return `<div class="bd-top"><span class="tt">${title}</span><span class="bd-prog"><span class="bd-bar"><i style="width:${(100 * decided) / total}%"></i></span>${decided} of ${total} decided</span><span class="bd-saved">${saved}</span><span class="grow"></span>${buttons}</div>`;
}
function boardNav(topics, { on = 1, section = 'Plans page' } = {}) {
  const rows = topics.map((t) => `<a class="${t.n === on ? 'on' : ''}"><span class="n">${t.n}</span><span class="t">${t.title}</span>${t.comment ? '<span class="c"></span>' : ''}${t.label ? `<span class="s ${t.label === '—' ? 'none' : ''}">${t.label}</span>` : ''}</a>`).join('');
  return `<div class="bd-nav"><h5>${section}</h5>${rows}<div class="sep"></div><a><span class="n">∑</span><span class="t">Summary</span></a><a><span class="n">?</span><span class="t">How this works</span></a></div>`;
}
const board = ({ top = boardTop(), nav = '', body = '' } = {}) => `<div class="bd">${top}<div class="bd-body">${nav}<div class="bd-page">${body}</div></div></div>`;
function topicHead({ crumb = 'Plans page · 1 of 1', title = 'The plans page', kind = '<b>Pick one</b>, then <b>+ Also</b> on any others to take parts of. Keys: <b>1–5</b> pick.' } = {}) {
  return `<div class="bd-crumb">${crumb}</div><div class="bd-h">${title}</div><div class="bd-kind">${kind}</div>`;
}
const todayBox = (text = TODAY_TEXT, img = 'img/design-0.jpg') => `<div class="bd-today"><div><h6>Today</h6><p>${text}</p></div>${img ? `<img src="${img}">` : ''}</div>`;

// One design's card, as the design board draws an option.
function card(design, { picked = false, also = false, comment = '', rec = false, tags = '', preview, desc = true, pc = true, foot = true } = {}) {
  const pickText = picked ? '✓ Picked' : `Pick ${design.key}`;
  return `<div class="bd-opt ${picked ? 'picked' : ''} ${also ? 'also' : ''}">
    <div class="bd-oh"><span class="bd-letter">${design.key}</span><span class="bd-oname">${design.name}</span>${rec ? '<span class="bd-tag rec">Recommended</span>' : ''}${tags}</div>
    ${desc ? `<p class="bd-desc">${design.desc}</p>` : ''}
    ${pc ? `<div class="bd-pc"><b>Good</b><span>${design.good}</span><b>Cost</b><span>${design.cost}</span></div>` : ''}
    ${preview ?? `<div class="bd-fit"><img src="${design.img}"></div>`}
    ${foot ? `<div class="bd-foot"><span class="bd-pick">${pickText}</span>${picked ? '' : `<span class="bd-also">${also ? '✓ Also' : '+ Also'}</span>`}<span class="bd-cm ${comment ? 'typed' : ''}">${comment || `Comment on ${design.key}…`}</span></div>` : ''}
  </div>`;
}
const grid = (cards, columns = 3) => `<div class="bd-grid" style="grid-template-columns:repeat(${columns},1fr)">${cards}</div>`;
const designCards = (options = {}) => DESIGNS.map((design) => card(design, { rec: design.key === 'B', picked: design.key === options.picked, also: (options.also || []).includes(design.key), comment: (options.comments || {})[design.key] || '', ...(options.each ? options.each(design) : {}) })).join('');

// The plans board as the design board draws a round today, in the browser.
const plansBoard = ({ nav = true, body, top, over = '', bar, h = 800 } = {}) => browser(page(bar ?? pageBar(), board({ top: top ?? boardTop(), nav: nav ? boardNav([{ n: 1, title: 'The plans page', label: null }]) : '', body: body ?? topicHead() + todayBox() + grid(designCards()) }), over), { h });

// The thread, for the card in it.
function threadFrame(convo, { w = 860, h = 460 } = {}) {
  return frame(`<div class="thread"><div class="thead">${mono('ST', 'g')}<span>storefront</span><span class="ph">/</span><span class="tt">Design the plans page</span>${ic('chev-down', 'xs')}<span class="grow"></span>${ibtn('diff', 'sm')}${ibtn('terminal', 'sm')}${ibtn('more', 'sm')}</div><div class="convo"><div class="column">${convo}</div></div></div>`, { w, h, style: 'border-radius:8px;border:1px solid #111' }).replace('class="m"', 'class="m jb"');
}
const bub = (text) => `<div class="bub">${text}</div>`;
const msg = (html) => `<div class="msg">${html}</div>`;
const ASK = bub('Make a few designs for the plans page so I can pick one.');
const AFTER = msg('Five designs for the plans page, built with the site’s components and its colors. Pick one on the board, or take parts of a few, and I’ll build it.');
