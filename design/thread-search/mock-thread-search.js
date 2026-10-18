// Search in a thread mocks: an open thread as agent_view.rs draws it (header, conversation with
// folded work, a plan card, the composer) and find bars from t3code, Zed and agentZ's terminal.

Object.assign(ICONS, {
  send: '<path d="M14.54 21.69a.5.5 0 0 0 .94-.03l6.5-19a.5.5 0 0 0-.64-.64l-19 6.5a.5.5 0 0 0-.03.94l7.93 3.18a2 2 0 0 1 1.11 1.11z"/><path d="m21.85 2.15-10.94 10.94"/>',
  'chev-left': '<path d="m15 18-6-6 6-6"/>',
  think: '<path d="M15 14c.2-1 .7-1.7 1.5-2.5 1-.9 1.5-2.2 1.5-3.5A6 6 0 0 0 6 8c0 1 .2 2.2 1.5 3.5.7.7 1.3 1.5 1.5 2.5"/><path d="M9 18h6"/><path d="M10 22h4"/>',
  regex: '<path d="M17 3v10"/><path d="m12.67 5.5 8.66 5"/><path d="m12.67 10.5 8.66-5"/><path d="M9 17a2 2 0 0 0-2-2H5a2 2 0 0 0-2 2v2a2 2 0 0 0 2 2h2a2 2 0 0 0 2-2v-2z"/>',
});

const TS_W = 720;
const TS_H = 470;
const TS_QUERY = 'promo';
const tsChev = (name = 'chev-down') => ic(name, 'xs');

/**
 * The conversation, as pieces of a kind (user, agent, plan, thought, tool, output). ⟦promo⟧
 * marks a match; a scene highlights the matches in the kinds it searches, counting them in
 * order, and draws the `active`-th (from 1) as the current one.
 */
const TS_PIECES = [
  { kind: 'user', html: 'Add a ⟦promo⟧ code field to the checkout page' },
  { kind: 'work', summary: 'Thought, read 3 files and searched code 1 time', rows: [
    { kind: 'thought', icon: 'think', html: 'Thought: the cart has no ⟦promo⟧ support, so the field needs an endpoint' },
    { kind: 'tool', icon: 'file', html: 'Read src/app/checkout/page.tsx' },
    { kind: 'tool', icon: 'search', html: 'Searched for “⟦promo⟧”' },
  ] },
  { kind: 'agent', html: 'The cart has no ⟦promo⟧ support yet. Here’s my plan:' },
  { kind: 'plan', items: ['Add a ⟦promo⟧ code field under the order summary', 'Check the code with <span class="mono-font sm">/api/⟦promo⟧s</span>', 'Take the discount off the total before tax'] },
  { kind: 'user', html: 'Go ahead' },
  { kind: 'work', summary: 'Changed 2 files and ran 1 command', rows: [
    { kind: 'tool', icon: 'pencil', html: 'Edit src/app/checkout/⟦promo⟧-field.tsx' },
    { kind: 'output', icon: 'terminal', html: 'npm test: ⟦promo⟧-field.test.tsx passed' },
  ] },
  { kind: 'agent', html: 'Done: the field sits under the order summary, and the ⟦promo⟧ discount comes off the total before tax.' },
];
const TS_DEFAULT_KINDS = ['user', 'agent', 'plan'];

function tsMarker(kinds, active, { flash = false } = {}) {
  let count = 0;
  const mark = (html, kind) => html.replace(/⟦(.*?)⟧/g, (_, text) => {
    if (!kinds.includes(kind)) return text;
    count += 1;
    return `<mark class="ts-m ${count === active ? 'on' : ''} ${count === active && flash ? 'flash' : ''}">${text}</mark>`;
  });
  return { mark, total: () => count };
}
// Counts the matches a scene would find, for the bar's “2/5”.
function tsTotal(kinds) {
  const marker = tsMarker(kinds, 0);
  for (const piece of TS_PIECES) {
    if (piece.html) marker.mark(piece.html, piece.kind);
    (piece.items || []).forEach((item) => marker.mark(item, 'plan'));
    (piece.rows || []).forEach((row) => marker.mark(row.html, row.kind));
  }
  return marker.total();
}

function tsConversation({ kinds = TS_DEFAULT_KINDS, active = 0, open = [], flash = false, from = 0, highlight = true } = {}) {
  const marker = tsMarker(highlight ? kinds : [], active, { flash });
  const parts = TS_PIECES.map((piece, index) => {
    if (piece.kind === 'user') return `<div class="ts-user"><div>${marker.mark(piece.html, 'user')}</div></div>`;
    if (piece.kind === 'agent') return `<div class="ts-agent">${marker.mark(piece.html, 'agent')}</div>`;
    if (piece.kind === 'plan') return `<div class="ts-plan"><div class="ph2">${ic('list', 'xs')}Plan</div><div class="pb"><ol>${piece.items.map((item) => `<li>${marker.mark(item, 'plan')}</li>`).join('')}</ol></div></div>`;
    const isOpen = open.includes(index);
    const rows = piece.rows.map((row) => marker.mark(row.html, row.kind));
    return `<div class="ts-work"><div><span class="chev">${tsChev(isOpen ? 'chev-down' : 'chev-right')}</span><span class="trunc">${piece.summary}</span></div></div>${isOpen ? piece.rows.map((row, i) => `<div class="ts-row">${ic(row.icon, 'sm')}<span class="trunc">${rows[i]}</span></div>`).join('') : ''}`;
  });
  return `<div class="ts-col" style="margin-top:${-from}px">${parts.join('')}</div>`;
}

// The header (render_toolbar): project crumb, title, branch, changes, Terminal, ⋯.
const tsHeader = ({ title = 'Add a promo code field', extra = '', titleSlot = null } = {}) => `<div class="row ts-head">
  <span class="row g15 sm mu none" style="padding:2px 4px">${mono('ST', 'g')}storefront</span><span class="sm mu">/</span>
  <span class="grow row" style="min-width:0">${titleSlot ?? `<span class="row g1" style="padding:2px 6px;font-size:13px;min-width:0"><span class="trunc">${title}</span><span class="mu" style="display:inline-flex">${tsChev()}</span></span>`}</span>
  <span class="row g1 sm mu none">${ic('branch', 'xs')}promo-code</span>
  <span class="row none" style="gap:6px;margin-left:6px">${extra}<span style="display:inline-flex;align-items:center;gap:6px;height:24px;padding:0 8px;border:1px solid var(--b);border-radius:6px;font-size:12px"><span class="okc">+41</span><span class="delc">−2</span></span>${ibtn('terminal')}${ibtn('more')}</span></div>`;

const tsComposer = ({ text = '' } = {}) => `<div class="ts-comp"><div class="ts-comp-in">
  <div style="height:22px;font-size:13px">${text || '<span class="ph">Message the agent…</span>'}</div>
  <div class="row" style="height:26px;gap:10px;font-size:12px;color:var(--mu)"><span class="row g15">${glyph('claude')}Claude Agent</span><span class="grow"></span><span class="row g1">Opus 4.5 ${tsChev()}</span><span style="display:inline-flex;color:var(--ac)">${ic('send', 'sm')}</span></div></div></div>`;

// A whole thread: header, an optional strip under it, the conversation, the composer.
const tsScene = ({ w = TS_W, h = TS_H, header = tsHeader(), strip = '', conv = {}, above = '', composer = tsComposer(), overlays = '', cls = '', sb = '' } = {}) =>
  frame(`<div class="col" style="height:100%">${header}${strip}<div class="ts-conv">${tsConversation(conv)}${sb}</div>${above}${composer}</div>${overlays}`, { w, h, style: 'border:1px solid var(--b);border-radius:8px' }).replace('class="m"', `class="m ${cls}"`);
const tsCaption = (text) => `<div class="ts-cap">${text}</div>`;
const tsPair = (...items) => `<div class="ts-pair">${items.join('')}</div>`;

// Find bars -----------------------------------------------------------------------------------
const tsQuery = (query = TS_QUERY, caret = true) => (query ? `<span class="q">${query}${caret ? '<span class="caret"></span>' : ''}</span>` : `<span class="q ph">${caret ? '<span class="caret" style="margin:0 1px 0 0"></span>' : ''}Find in thread</span>`);
const tsCount = (text, none = false) => `<span class="ts-count ${none ? 'none' : ''}">${text}</span>`;
// t3code: a rounded bar floating at the thread's top right, “Find in thread”, n/total, ↑ ↓ ×.
const tsFloat = ({ query = TS_QUERY, count = '2/5', none = false, style = '' } = {}) => `<div class="ts-float" style="${style}">${tsQuery(query)}${tsCount(count, none)}<span class="ts-ib">${ic('chev-up', 'sm')}</span><span class="ts-ib">${ic('chev-down', 'sm')}</span><span class="ts-ib">${ic('x', 'sm')}</span></div>`;
// Zed: a strip under the header, the query box with its toggles, ‹ ›, the count and ×.
const tsZedStrip = ({ query = TS_QUERY, count = '2/5' } = {}) => `<div class="ts-strip"><div class="ts-input">${query ? `<span class="q">${query}<span class="caret"></span></span>` : '<span class="q ph">Search this thread…</span>'}<span class="ts-tog">Aa</span><span class="ts-tog" style="text-decoration:underline">ab</span><span class="ts-tog">.*</span></div><span class="ts-ib sq">${ic('chev-left', 'sm')}</span><span class="ts-ib sq">${ic('chev-right', 'sm')}</span><span class="ts-count" style="font-size:12px;text-align:left;margin-left:4px">${count}</span><span class="ts-ib sq">${ic('x', 'sm')}</span></div>`;
// agentZ's terminal find bar: a text field, ‹ ›, the count and ×.
const tsTermStrip = ({ query = TS_QUERY, count = '2/5', placeholder = 'Find' } = {}) => `<div class="ts-strip" style="padding:4px 8px;gap:8px;border-bottom-color:var(--bv)"><div class="ts-input" style="height:26px">${query ? `<span class="q">${query}<span class="caret"></span></span>` : `<span class="q ph">${placeholder}</span>`}</div><span class="ts-ib sq">${ic('chev-left', 'sm')}</span><span class="ts-ib sq">${ic('chev-right', 'sm')}</span><span class="ts-count" style="font-size:12px;color:var(--t)">${count}</span><span class="ts-ib sq">${ic('x', 'sm')}</span></div>`;

// The sidebar, for the search across threads.
const TS_PROJECTS = { st: { mono: 'ST', color: 'g', name: 'storefront' }, ap: { mono: 'AP', color: 't', name: 'api' } };
const TS_THREADS = [
  { p: 'st', title: 'Add a promo code field', branch: 'promo-code', time: 'now', on: true },
  { p: 'st', title: 'Add the checkout page', branch: 'checkout-flow', time: '2h', snippet: '…apply a ⟦promo⟧ code later, once the cart…' },
  { p: 'ap', title: 'Rate limit the coupon endpoint', branch: 'main', time: '1d', snippet: '…the /⟦promo⟧s route gets 20 requests a minute…' },
  { p: 'st', title: 'Write tests for the rate limiter', branch: 'main', time: '3d' },
];
function tsCard(thread, { snippet = false, query = TS_QUERY, titleMatch = false } = {}) {
  const project = TS_PROJECTS[thread.p];
  const title = titleMatch ? thread.title.replace(new RegExp(query, 'i'), (m) => `<mark class="ts-m">${m}</mark>`) : thread.title;
  return `<div style="margin:0 4px;padding:8px 10px;border-radius:6px;flex:none;${thread.on ? 'background:var(--sel)' : ''}">
    <div class="row g15" style="height:20px">${mono(project.mono, project.color)}<span class="grow trunc sm mu">${project.name}</span><span class="sm mu">${thread.time}</span></div>
    <div class="row g15" style="margin-top:4px;min-width:0"><span class="trunc b5">${title}</span></div>
    ${snippet && thread.snippet ? `<div class="sm mu trunc" style="margin-top:2px">${thread.snippet.replace(/⟦(.*?)⟧/g, '<mark class="ts-m">$1</mark>')}</div>` : `<div class="row g15 sm faint" style="margin-top:2px"><span class="grow trunc">${thread.branch}</span><span style="opacity:.6;color:var(--mu);display:inline-flex">${ic('laptop', 'sm')}</span></div>`}</div>`;
}
const tsSearchHead = (query = TS_QUERY) => `<div class="sb-head" style="color:var(--t)">${ic('search', 'sm mu')}<span class="grow sm">${query}<span class="caret"></span></span>${ibtn('x')}${ibtn('plus')}</div>`;
const tsSidebar = (cards, { h = 420, head = tsSearchHead(), after = '' } = {}) => frame(sidebar({ head, list: cards.join('') + after }), { w: 290, h, style: 'border:1px solid var(--b);border-radius:8px' });
