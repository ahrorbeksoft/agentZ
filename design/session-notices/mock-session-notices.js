// Session notices and compaction mocks: an open thread as agent_view.rs draws it (header,
// conversation, what sits above the composer, the composer with its context ring), Zed's
// notice callouts (session_notice.rs) and its compaction entry (thread_view.rs).

Object.assign(ICONS, {
  info: '<circle cx="12" cy="12" r="10"/><path d="M12 16v-4"/><path d="M12 8h.01"/>',
  warning: '<path d="m21.73 18-8-14a2 2 0 0 0-3.48 0l-8 14A2 2 0 0 0 4 21h16a2 2 0 0 0 1.73-3"/><path d="M12 9v4"/><path d="M12 17h.01"/>',
  'x-circle': '<circle cx="12" cy="12" r="10"/><path d="m15 9-6 6"/><path d="m9 9 6 6"/>',
  // Lucide's fold-vertical, standing in for Zed's IconName::Compact.
  compact: '<path d="M12 22v-6"/><path d="M12 8V2"/><path d="M4 12H2"/><path d="M10 12H8"/><path d="M16 12h-2"/><path d="M22 12h-2"/><path d="m15 19-3-3-3 3"/><path d="m15 5-3 3-3-3"/>',
  send: '<path d="M14.54 21.69a.5.5 0 0 0 .94-.03l6.5-19a.5.5 0 0 0-.64-.64l-19 6.5a.5.5 0 0 0-.03.94l7.93 3.18a2 2 0 0 1 1.11 1.11z"/><path d="m21.85 2.15-10.94 10.94"/>',
  'arrow-up-right': '<path d="M7 7h10v10"/><path d="M7 17 17 7"/>',
  think: '<path d="M15 14c.2-1 .7-1.7 1.5-2.5 1-.9 1.5-2.2 1.5-3.5A6 6 0 0 0 6 8c0 1 .2 2.2 1.5 3.5.7.7 1.3 1.5 1.5 2.5"/><path d="M9 18h6"/><path d="M10 22h4"/>',
  slash: '<path d="M22 2 2 22"/>',
});

const SN_W = 720;
const SN_H = 460;
const snChev = (name = 'chev-down') => ic(name, 'xs');

// The header (render_toolbar): project crumb, title, branch, changes, Terminal, ⋯.
const snHeader = ({ title = 'Add the checkout page', extra = '' } = {}) => `<div class="row sn-head">
  <span class="row g15 sm mu none" style="padding:2px 4px">${mono('ST', 'g')}storefront</span><span class="sm mu">/</span>
  <span class="grow row" style="min-width:0"><span class="row g1" style="padding:2px 6px;font-size:13px;min-width:0"><span class="trunc">${title}</span><span class="mu" style="display:inline-flex">${snChev()}</span></span></span>
  <span class="row g1 sm mu none">${ic('branch', 'xs')}checkout-flow</span>
  <span class="row none" style="gap:6px;margin-left:6px">${extra}<span class="oe-out" style="display:inline-flex;align-items:center;gap:6px;height:24px;padding:0 8px;border:1px solid var(--b);border-radius:6px;font-size:12px"><span class="okc">+24</span><span class="delc">−3</span></span>${ibtn('terminal')}${ibtn('more')}</span></div>`;

// Conversation pieces.
const snUser = (text) => `<div class="sn-user"><div>${text}</div></div>`;
const snAgent = (html) => `<div class="sn-agent">${html}</div>`;
const snWork = (summary, { open = false, hov = false, icon = '' } = {}) => `<div class="sn-work"><div class="${hov ? 'hov' : ''}"><span class="chev">${icon || snChev(open ? 'chev-down' : 'chev-right')}</span><span class="trunc">${summary}</span></div></div>`;
const snWorkRow = (icon, text, { shimmer = false } = {}) => `<div class="row g2" style="margin:0 20px 0 50px;height:24px;font-size:13px;color:#8b909d">${ic(icon, 'sm')}<span style="${shimmer ? 'color:var(--t);opacity:.75' : ''}">${text}</span></div>`;
const snGenerating = (text = 'Working for 1m 12s') => `<div class="sn-gen"><span class="dots">•••</span>${text}</div>`;
const snCode = (text) => `<span class="mono-font" style="font-size:12px;background:var(--panel);padding:1px 4px;border-radius:4px">${text}</span>`;

// The conversation every scene shows, ending with the turn in progress.
const SN_TURNS = {
  first: snUser('Add the checkout page with a pay button'),
  firstWork: snWork('Read 3 files, changed 2 files, and ran 2 commands'),
  firstReply: snAgent(`Added ${snCode('src/app/checkout/page.tsx')} with a pay button. It reuses the cart’s totals and formats prices with ${snCode('formatMoney')}.`),
  second: snUser('Now add a promo code field under the order summary'),
  secondWork: snWork('Read 2 files and searched code 1 time'),
};
const snConversation = (pieces) => `<div class="sn-col">${pieces.join('')}</div>`;
const snDefaultBody = (extra = []) => snConversation([SN_TURNS.first, SN_TURNS.firstWork, SN_TURNS.firstReply, SN_TURNS.second, SN_TURNS.secondWork, ...extra, snGenerating()]);

// The composer, with the context ring (render_context_usage) before the model.
function snRing(ratio = 0.42, { color } = {}) {
  const r = 6;
  const length = 2 * Math.PI * r;
  const stroke = color || (ratio >= 0.85 ? 'var(--warn)' : 'var(--mu)');
  return `<span class="sn-ring"><svg width="16" height="16" viewBox="0 0 16 16"><circle cx="8" cy="8" r="${r}" fill="none" stroke="rgba(169,175,188,.25)" stroke-width="2"/><circle cx="8" cy="8" r="${r}" fill="none" stroke="${stroke}" stroke-width="2" stroke-dasharray="${(ratio * length).toFixed(2)} ${length.toFixed(2)}" transform="rotate(-90 8 8)" stroke-linecap="round"/></svg></span>`;
}
const snComposer = ({ ring = 0.42, text = '', working = true, ringExtra = '' } = {}) => `<div class="sn-comp"><div class="sn-comp-in">
  <div style="height:22px;font-size:13px">${text || '<span class="ph">Message the agent…</span>'}</div>
  <div class="row" style="height:26px;gap:10px;font-size:12px;color:var(--mu)"><span class="row g15">${glyph('claude')}Claude Agent</span><span class="grow"></span>${ringExtra}${ring === null ? '' : snRing(ring)}<span class="row g1">Opus 4.5 ${snChev()}</span><span style="display:inline-flex;color:${working ? 'var(--mu)' : 'var(--ac)'}">${ic(working ? 'stop' : 'send', 'sm')}</span></div></div></div>`;

// A whole thread: header, conversation, what sits above the composer, the composer.
const snScene = ({ w = SN_W, h = SN_H, header = snHeader(), body = snDefaultBody(), above = '', composer = snComposer(), overlays = '' } = {}) =>
  frame(`<div class="col" style="height:100%">${header}<div class="sn-conv">${body}</div>${above ? `<div class="sn-above">${above}</div>` : ''}${composer}</div>${overlays}`, { w, h, style: 'border:1px solid var(--b);border-radius:8px' });
const snCaption = (text) => `<div class="sn-cap">${text}</div>`;
const snPair = (...items) => `<div class="sn-pair">${items.join('')}</div>`;
const snTip = (html, style) => `<div class="sn-tip" style="${style}">${html}</div>`;

// Notices ------------------------------------------------------------------------------------
// Severity → Zed's callout: Info (muted ⓘ), Warning (⚠), Error (⊗), anything else Info with no icon.
const SN_SEVERITY = {
  info: { icon: 'info', color: 'var(--mu)', bg: 'rgba(116,173,232,.04)' },
  warning: { icon: 'warning', color: 'var(--warn)', bg: 'rgba(222,193,132,.05)' },
  error: { icon: 'x-circle', color: 'var(--del)', bg: 'rgba(208,114,119,.08)' },
  other: { icon: null, color: 'var(--mu)', bg: 'rgba(116,173,232,.04)' },
};
const SN_NOTICES = {
  mcp: { sev: 'warning', title: 'MCP server unavailable', desc: 'Continuing without it. Other integrations are still available.' },
  indexing: { sev: 'info', title: 'Indexing resumed', desc: '' },
  failed: { sev: 'error', title: 'Optional integration failed', desc: 'Work will continue without this integration.' },
  network: { sev: 'warning', title: 'Network performance is degraded', desc: 'Responses may take longer than usual.' },
  maintenance: { sev: 'other', title: 'Scheduled maintenance', desc: 'The service restarts at 02:00 UTC for about ten minutes.' },
};
const snSevIcon = (sev, cls = 'sm') => (SN_SEVERITY[sev].icon ? `<span style="display:inline-flex;color:${SN_SEVERITY[sev].color}">${ic(SN_SEVERITY[sev].icon, cls)}</span>` : '');
const snX = (hov = false) => `<span class="sn-x ${hov ? 'hov' : ''}">${ic('x', 'sm')}</span>`;
function snCallout(notice, { dismiss = true, hovX = false, actions = '', count = '', first = false } = {}) {
  const sev = SN_SEVERITY[notice.sev];
  const icon = sev.icon ? `<span class="ic">${snSevIcon(notice.sev)}</span>` : '';
  return `<div class="sn-callout" style="background:${sev.bg};${first ? '' : ''}">${icon}<div class="grow col" style="min-width:0"><div class="row g1" style="min-height:18px"><span class="tt">${notice.title}${count}</span><span class="acts">${actions}${dismiss ? snX(hovX) : ''}</span></div>${notice.desc ? `<div class="ds">${notice.desc}</div>` : ''}</div></div>`;
}
const snStack = (keys, options = {}) => keys.map((key) => snCallout(SN_NOTICES[key], options)).join('');

// The usage-limit notice agentZ already draws above the composer (render_limit_notice).
const snLimitNotice = () => `<div class="sn-callout" style="background:rgba(222,193,132,.05)"><span class="ic">${snSevIcon('warning')}</span><div class="grow col" style="min-width:0;gap:8px"><div class="row g1" style="min-height:18px"><span class="tt">Your account reached its 5-hour limit</span><span class="acts">${snX()}</span></div><div class="ds">Claude Agent stopped. The limit resets at 16:10, in 2h 13m.</div><div class="row g1"><span class="btn sm">Continue at 16:10</span><span class="btn ghost sm">Usage ${ic('arrow-up-right', 'xs')}</span></div></div></div>`;

// Compaction ---------------------------------------------------------------------------------
const SN_STATUS = {
  progress: 'Compacting Context…',
  done: 'Context Compacted',
  failed: 'Compaction Failed',
  cancelled: 'Compaction Canceled',
};
const snSpin = () => '<span class="spin"></span>';
// Zed's entry: a rule, a small button with the Compact icon and a chevron, a rule.
function snCompaction({ status = 'done', open = false, hov = false, summary = true } = {}) {
  const lead = status === 'progress' ? snSpin() : `<span class="mu" style="display:inline-flex">${ic('compact', 'xs')}</span>`;
  const chev = status === 'progress' || !summary ? '' : `<span class="mu" style="display:inline-flex">${snChev(open ? 'chev-up' : 'chev-down')}</span>`;
  const label = `<span style="color:${status === 'failed' ? 'var(--del)' : 'inherit'}">${SN_STATUS[status]}</span>`;
  return `<div style="padding:6px 20px"><div class="row" style="gap:8px"><span class="sn-rule"></span><span class="sn-cbtn ${hov ? 'hov' : ''} ${status === 'progress' ? 'dis' : ''}">${lead}${label}${chev}</span><span class="sn-rule"></span></div>${open ? `<div style="margin-top:6px">${status === 'failed' ? snFailure() : snSummary()}</div>` : ''}</div>`;
}
const SN_SUMMARY_HTML = `<div><b style="font-weight:600">Summary of the conversation so far</b></div>
  <ul><li>The user asked for a checkout page with a pay button. Done in ${snCode('src/app/checkout/page.tsx')}, reusing the cart’s totals and ${snCode('formatMoney')}.</li>
  <li>A failed payment re-enables the button and shows why the card was declined (${snCode('errors.ts')}).</li>
  <li>Now: a promo code field under the order summary that applies its discount to the total.</li></ul>`;
const snSummary = ({ maxH = 0, fade = false } = {}) => `<div class="sn-summary"><div class="body" style="${maxH ? `max-height:${maxH}px;overflow:hidden;position:relative` : ''}">${SN_SUMMARY_HTML}${fade ? '<div style="position:absolute;left:0;right:0;bottom:0;height:26px;background:linear-gradient(transparent,#282c33)"></div>' : ''}</div><div class="foot">${snChev('chev-up')}</div></div>`;
const snFailure = () => `<div class="sn-summary"><div class="body" style="color:var(--del)">The summary request timed out. The conversation is unchanged.</div><div class="foot">${snChev('chev-up')}</div></div>`;

// The sidebar, for notices in threads you aren't looking at.
const SN_THREADS = [
  { p: 'st', title: 'Add the checkout page', branch: 'checkout-flow', state: 'working', on: true },
  { p: 'st', title: 'Write tests for the rate limiter', branch: 'main', state: 'working', notice: 'error' },
  { p: 'ap', title: 'Fix flaky login test', branch: 'main', state: 'awaiting' },
  { p: 'ap', title: 'Bump the API version', branch: 'main', time: '2h', notice: 'warning' },
];
const SN_PROJECTS = { st: { mono: 'ST', color: 'g', name: 'storefront' }, ap: { mono: 'AP', color: 't', name: 'api' } };
const snWorking = () => '<span class="pill working"><span class="spin"></span>Working</span>';
function snCard(thread, { status = null, mark = '' } = {}) {
  const project = SN_PROJECTS[thread.p];
  const slot = status ?? (thread.state === 'working' ? snWorking() : thread.state ? pill(thread.state) : `<span class="sm mu">${thread.time}</span>`);
  return `<div class="sn-card ${thread.on ? 'on' : ''}">
    <div class="row g15" style="height:20px">${mono(project.mono, project.color)}<span class="grow trunc sm mu">${project.name}</span>${slot}</div>
    <div class="row g15" style="margin-top:4px;min-width:0"><span class="trunc b5">${thread.title}</span></div>
    <div class="row g15 sm faint" style="margin-top:2px"><span class="grow trunc">${thread.branch}</span>${mark}<span style="opacity:.6;color:var(--mu);display:inline-flex">${ic('laptop', 'sm')}</span><span style="width:14px;height:14px;display:inline-grid;place-items:center;color:rgba(169,175,188,.6);font-size:13px;line-height:1">${GLYPHS.claude}</span></div></div>`;
}
// A wider frame shows the editor's background beside the sidebar, for what sits outside it.
const snSidebar = (cards, { h = 380, over = '', w = 290 } = {}) => {
  const list = sidebar({ list: cards.join('') });
  const body = w > 290 ? `<div class="row" style="height:100%;align-items:stretch"><div class="none" style="width:290px;border-right:1px solid var(--b)">${list}</div><div class="grow" style="background:var(--ed)"></div></div>` : list;
  return frame(`${body}${over}`, { w, h, style: 'border:1px solid var(--b);border-radius:8px' });
};
