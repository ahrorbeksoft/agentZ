// A forked thread's view: the window, the toolbar with the source pill, the carried
// conversation, the divider pills and the new thread screen's pieces (agent_view.rs
// render_new_thread, render_fork_dividers, render_fork_source_pill, render_handoff_chips).

ICONS.send = '<path d="M14.54 21.69a.5.5 0 0 0 .94-.03l6.5-19a.5.5 0 0 0-.64-.64l-19 6.5a.5.5 0 0 0-.03.94l7.93 3.18a2 2 0 0 1 1.11 1.11z"/><path d="m21.85 2.15-10.94 10.94"/>';
// Lucide's git-fork, Zed's IconName::GitFork.
ICONS.fork = '<circle cx="12" cy="18" r="3"/><circle cx="6" cy="6" r="3"/><circle cx="18" cy="6" r="3"/><path d="M18 9v2c0 .6-.4 1-1 1H7c-.6 0-1-.4-1-1V9"/><path d="M12 12v3"/>';
ICONS['arrow-up-right'] = '<path d="M7 7h10v10"/><path d="M7 17 17 7"/>';

const FK_W = 1000;
const FK_H = 600;
const SOURCE = 'Add the checkout page';
const FORK = 'Add the checkout page fork';

const caret = '<span style="display:inline-block;width:1.5px;height:16px;background:var(--ac);vertical-align:-3px"></span>';
const fkChev = (name = 'chev-down') => ic(name, 'xs');

// The sidebar: the demo threads with the fork as the open one, beside its source.
const FK_THREADS = [
  { p: 'st', title: FORK, branch: 'checkout-flow', time: 'now', agent: 'claude', on: true, fork: true },
  { p: 'st', title: SOURCE, branch: 'checkout-flow', state: 'done', agent: 'claude' },
  { p: 'st', title: 'Write tests for the rate limiter', branch: 'main', state: 'pending', agent: 'codex' },
  { p: 'ap', title: 'Fix flaky login test', branch: 'main', state: 'awaiting', agent: 'claude' },
  { p: 'ap', title: 'Bump the API version', branch: 'main', time: '2h', agent: 'opencode' },
];
const FK_PROJECTS = { st: { mono: 'ST', color: 'g', name: 'storefront' }, ap: { mono: 'AP', color: 't', name: 'api' } };
function fkCard(thread) {
  const project = FK_PROJECTS[thread.p];
  const status = thread.state ? pill(thread.state) : `<span class="sm mu">${thread.time}</span>`;
  return `<div style="margin:0 4px;padding:8px 10px;border-radius:6px;height:78px;flex:none;${thread.on ? 'background:var(--sel)' : ''}">
    <div class="row g15" style="height:20px">${mono(project.mono, project.color)}<span class="grow trunc sm mu">${project.name}</span>${status}</div>
    <div class="row g15" style="margin-top:4px;min-width:0"><span class="trunc b5">${thread.title}</span></div>
    <div class="row g15 sm faint" style="margin-top:2px"><span class="grow trunc">${thread.branch}</span><span style="opacity:.6;color:var(--mu);display:inline-flex">${ic('laptop', 'sm')}</span><span style="width:14px;height:14px;display:inline-grid;place-items:center;color:rgba(169,175,188,.6);font-size:13px;line-height:1">${GLYPHS[thread.agent]}</span></div></div>`;
}
const fkSidebar = () => sidebar({ list: FK_THREADS.map(fkCard).join('') });

function fkWin(body, { w = FK_W, h = FK_H } = {}) {
  return `<div class="m win" style="width:${w}px;height:${h}px">
    <div class="titlebar">${lights()}${ic('sidebar', 'sm mu')}<span class="row g15" style="font-size:13px">${ic('list', 'xs mu')}All projects${ic('chev-down', 'xs mu')}</span><div class="viewtabs"><span class="on">Agents</span><span>Workspaces</span></div></div>
    <div class="body">${fkSidebar()}<div class="grow col" style="height:100%;background:var(--ed);min-width:0;position:relative">${body}</div></div></div>`;
}

// The header's pill with the source's title (render_fork_source_pill).
const sourcePill = ({ open = false, label = SOURCE } = {}) => `<span class="row g1" style="height:22px;padding:0 6px;border-radius:4px;border:1px solid var(--bv);max-width:200px;${open ? 'background:var(--hov)' : ''}"><span class="mu" style="display:inline-flex">${ic('fork', 'xs')}</span><span class="trunc sm mu">${label}</span><span class="mu" style="display:inline-flex">${fkChev()}</span></span>`;
const sourceMenu = (style = 'right:8px;top:34px') => `<div class="menu" style="${style};min-width:190px"><div class="it hl">${ic('arrow-up-right', 'sm')}Open Source Thread</div><div class="it">${ic('merge', 'sm')}Merge Back</div></div>`;
const fkToolbar = ({ title = FORK, pill: pillHtml = sourcePill(), extra = '' } = {}) => `<div class="row g1" style="height:36px;flex:none;padding:0 8px;border-bottom:1px solid var(--b);position:relative">
  <span class="row g15 sm mu" style="padding:0 4px">${mono('ST', 'g')}storefront</span><span class="sm mu">/</span><span class="trunc" style="font-size:13px;padding:0 4px">${title}</span><span class="grow"></span>${pillHtml}${ibtn('diff')}${ibtn('terminal')}${extra}</div>`;

// The conversation the fork carries: three turns of the source, the third being the reply forked from.
const userMsg = (text) => `<div class="row" style="justify-content:flex-end"><div style="max-width:78%;background:var(--panel);border-radius:12px;padding:8px 12px">${text}</div></div>`;
const toolLines = (...lines) => `<div class="col sm ph" style="gap:2px">${lines.map(([icon, text]) => `<span class="row g15">${ic(icon, 'xs')}${text}</span>`).join('')}</div>`;
const TURNS = [
  [userMsg('Add the checkout page with a pay button'),
    `<div>I’ll add the page under <span class="mono-font sm">src/app/checkout</span> and reuse the cart’s totals.</div>${toolLines(['file', 'Read src/app/cart/page.tsx'], ['pencil', 'Edit src/app/checkout/page.tsx'])}`],
  [userMsg('Use the same currency format as the cart'),
    '<div>Done: the page now formats prices with the cart’s <span class="mono-font sm">formatMoney</span>.</div>'],
  [userMsg('Show an error if the payment fails'),
    `<div>Added a failed state: the pay button re-enables and an alert under it says why the card was declined.</div>${toolLines(['pencil', 'Edit src/app/checkout/page.tsx'], ['file', 'Edit src/app/checkout/errors.ts'])}`],
];
const turn = (index) => TURNS[index].join('');
const forkReply = () => TURNS[2][1];
const messages = (indexes, { dim = 1 } = {}) => `<div class="col" style="gap:10px;opacity:${dim}">${indexes.map(turn).join('<div style="height:2px"></div>')}</div>`;
const OWN_TURN = `${userMsg('Now also add a promo code field')}
  <div>I’ll add the field under the order summary and apply the discount to the total.</div>
  ${toolLines(['file', 'Read src/app/cart/totals.ts'], ['pencil', 'Edit src/app/checkout/page.tsx'])}
  <div class="row g15 ac sm"><span class="spin"></span>Working…</div>`;

// t3code's pill divider (render_fork_dividers).
const forkDivider = (label = 'Forked from conversation', { hover = false } = {}) => `<div class="row" style="gap:12px;padding:2px 0"><span class="grow" style="height:1px;background:var(--bv)"></span><span class="row g1" style="padding:2px 8px;border-radius:999px;border:1px solid var(--bv);flex:none;${hover ? 'background:var(--hov)' : ''}"><span class="mu" style="display:inline-flex">${ic('fork', 'xs')}</span><span class="sm mu">${label}</span></span><span class="grow" style="height:1px;background:var(--bv)"></span></div>`;

// The conversation column, as long as the thread's width.
const scroller = (html, { gap = 10, pad = '18px 0 8px', justify = 'flex-start' } = {}) => `<div class="grow col" style="align-items:center;overflow:hidden;padding:${pad};justify-content:${justify}"><div class="col" style="width:620px;gap:${gap}px;line-height:22px">${html}</div></div>`;

const sendButton = (stop = false) => `<span style="color:var(--ac);display:inline-flex">${ic(stop ? 'stop' : 'send')}</span>`;
const composerCard = ({ text = '', chips = '', placeholder = 'Message the agent…' } = {}) => `<div style="border:1px solid var(--b);border-radius:10px;background:var(--panel);padding:12px 14px 10px">
  ${chips}<div style="height:44px;line-height:20px">${text || `${caret}<span class="ph">${placeholder}</span>`}</div>
  <div class="row" style="height:26px;gap:12px;font-size:13px;color:var(--mu)"><span class="row g15">${glyph('claude')}Claude Agent${fkChev()}</span><span class="grow"></span><span class="row g1">Opus 5.5 ${fkChev()}</span>${sendButton()}</div></div>`;
const strip = () => `<div class="row"><span class="row g1">${chipBtn(ic('folder', 'xs'), 'Local')}${chipBtn(ic('laptop', 'xs'), 'This Mac')}</span><span class="grow"></span><span class="row g1 sm mu">${ic('branch', 'xs')}checkout-flow</span></div>`;
const chipBtn = (icon, label) => `<span class="row g1 sm mu" style="height:22px;padding:0 6px;border-radius:5px">${icon}${label}${fkChev()}</span>`;

// The new thread screen's body: `above` over the headline, `between` under it.
const newScreen = ({ headline = 'What should we work on?', sub = '', above = '', between = '', composer = composerCard() } = {}) => `<div class="grow col" style="align-items:center;justify-content:center;padding:0 16px">
  <div class="col" style="width:100%;max-width:600px;gap:20px">
    ${above}
    <div style="text-align:center"><div style="font-size:24px">${headline}</div>${sub ? `<div class="mu" style="margin-top:6px;font-size:13px">${sub}</div>` : ''}</div>
    <div class="col" style="gap:8px">${between}${composer}${strip()}</div>
  </div></div>`;

// An open thread's composer, along the bottom.
const bottomComposer = ({ working = false, chips = '' } = {}) => `<div style="border-top:1px solid var(--b);padding:8px 0 10px"><div style="width:620px;margin:0 auto">${chips}<div class="ph" style="height:22px">${working ? '' : caret}<span>Message the agent…</span></div>
  <div class="row" style="height:26px;gap:12px;font-size:13px;color:var(--mu)"><span class="row g15">${glyph('claude')}Claude Agent</span><span class="grow"></span><span class="row g1">Opus 5.5 ${fkChev()}</span><span style="display:inline-flex">${ic(working ? 'stop' : 'send')}</span></div></div></div>`;

// The merge-back chip's look (render_handoff_chips): an attachment chip over the composer.
const contextChip = (label = SOURCE, count = '6 messages', { menu = false } = {}) => `<div class="row g1" style="margin-bottom:8px"><span class="row g1 sm" style="height:24px;padding:0 4px 0 7px;border-radius:5px;border:1px solid var(--b);background:var(--ed);color:var(--mu)">${ic('fork', 'xs')}<span class="trunc" style="max-width:260px">${label} · ${count}</span>${menu ? `<span class="mu" style="display:inline-flex;margin-left:2px">${fkChev()}</span>` : ''}<span style="display:inline-grid;place-items:center;width:16px;height:16px;border-radius:4px">${ic('x', 'xs')}</span></span></div>`;

// A card for the source: what it is, where it parted, and how much comes along.
const sourceCard = ({ actions = '' } = {}) => `<div class="row g2" style="border:1px solid var(--b);border-radius:8px;background:var(--panel);padding:10px 12px">
  <span class="mu" style="display:inline-flex">${ic('fork')}</span>
  <div class="grow col" style="min-width:0"><div class="trunc sm mu">Forked from</div><div class="trunc b5">${SOURCE}</div><div class="xs ph" style="margin-top:2px">At its reply 3 of 5 · 6 messages come along as context</div></div>${actions || '<span class="btn sm">Open</span>'}</div>`;

// One line that stands for the carried messages.
const foldLine = ({ open = false, label = `6 earlier messages from ${SOURCE}`, actions = '' } = {}) => `<div class="row g15" style="height:30px;padding:0 10px;border-radius:6px;border:1px solid var(--bv);color:var(--mu);font-size:12px"><span style="display:inline-flex">${ic('fork', 'xs')}</span><span class="grow trunc">${label}</span>${actions}<span style="display:inline-flex">${fkChev(open ? 'chev-up' : 'chev-down')}</span></div>`;

// A quoted reply, cut off with a fade, as the new thread screen would show where it left off.
const quotedReply = ({ lines = 3 } = {}) => `<div style="border:1px solid var(--bv);border-radius:8px;padding:10px 12px;position:relative;overflow:hidden">
  <div class="row g15 xs ph" style="margin-bottom:6px">${ic('fork', 'xs')}Forked from ${SOURCE} · its reply 3 of 5</div>
  <div style="line-height:20px;font-size:13px;max-height:${lines * 20}px;overflow:hidden;color:var(--mu)">Added a failed state: the pay button re-enables and an alert under it says why the card was declined. I also edited <span class="mono-font sm">src/app/checkout/errors.ts</span> for the messages.</div>
  <div style="position:absolute;left:0;right:0;bottom:0;height:20px;background:linear-gradient(transparent,var(--ed))"></div></div>`;

// Overlay labels in the mocks.
const fkNote = (text, style) => note(text, style);
