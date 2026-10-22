// Subthreads, the machine picker and shells: the demo world and the pieces this round's mocks
// share. Loaded after board/mock.js; names are prefixed with r so they don't clash with it.

ICONS['x-circle'] = '<circle cx="12" cy="12" r="10"/><path d="m15 9-6 6"/><path d="m9 9 6 6"/>';
ICONS['arrow-up-right'] = '<path d="M7 7h10v10"/><path d="M7 17 17 7"/>';
ICONS['arrow-up-left'] = '<path d="M7 17V7h10"/><path d="M17 17 7 7"/>';
ICONS.dash = '<path d="M5 12h14"/>';
ICONS.sparkle = '<path d="M9.937 15.5A2 2 0 0 0 8.5 14.063l-6.135-1.582a.5.5 0 0 1 0-.962L8.5 9.936A2 2 0 0 0 9.937 8.5l1.582-6.135a.5.5 0 0 1 .963 0L14.063 8.5A2 2 0 0 0 15.5 9.937l6.135 1.581a.5.5 0 0 1 0 .964L15.5 14.063a2 2 0 0 0-1.437 1.437l-1.582 6.135a.5.5 0 0 1-.963 0z"/>';
ICONS.warning = '<path d="m21.73 18-8-14a2 2 0 0 0-3.48 0l-8 14A2 2 0 0 0 4 21h16a2 2 0 0 0 1.73-3"/><path d="M12 9v4"/><path d="M12 17h.01"/>';
ICONS.send = '<path d="M14.54 21.69a.5.5 0 0 0 .94-.03l6.5-19a.5.5 0 0 0-.64-.64l-19 6.5a.5.5 0 0 0-.03.94l7.93 3.18a2 2 0 0 1 1.11 1.11z"/><path d="m21.85 2.15-10.94 10.94"/>';
ICONS.cloud = '<path d="M17.5 19H9a7 7 0 1 1 6.71-9h1.79a4.5 4.5 0 1 1 0 9Z"/>';
ICONS.download = '<path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/><polyline points="7 10 12 15 17 10"/><line x1="12" x2="12" y1="15" y2="3"/>';
ICONS['rotate-cw'] = '<path d="M21 12a9 9 0 1 1-9-9c2.52 0 4.93 1 6.74 2.74L21 8"/><path d="M21 3v5h-5"/>';
ICONS.forward = '<polyline points="15 10 20 15 15 20"/><path d="M4 4v7a4 4 0 0 0 4 4h12"/>';
ICONS.reply = '<polyline points="9 17 4 12 9 7"/><path d="M20 18v-2a4 4 0 0 0-4-4H4"/>';
GLYPHS.droid = '❋';
GLYPHS.antigravity = '⋀';

// One Dark's text tones as the thread's tool rows draw them (thread-rows round: one dim gray).
const R_DIM = '#8d929c';
const R_MONO = "'IBM Plex Mono','SF Mono',Menlo,monospace";
const rCode = (text) => `<code style="font:12px ${R_MONO};background:var(--hov);padding:1px 4px;border-radius:3px">${text}</code>`;
const rSpin = (size = 12) => `<span class="spin" style="width:${size}px;height:${size}px"></span>`;
const rCheck = (cls = 'xs') => `<span style="display:inline-flex;color:var(--ok)">${ic('check', cls)}</span>`;
const rCross = (cls = 'xs') => `<span style="display:inline-flex;color:var(--del)">${ic('x-circle', cls)}</span>`;
const rStat = ([files, add, del]) => `<span class="none" style="font-size:12px;color:var(--mu)">${files} ${files === 1 ? 'file' : 'files'}</span><span class="none" style="font-size:12px"><span class="okc">+${add}</span> <span class="delc">−${del}</span></span>`;
const rStop = () => `<span class="ibtn sm" style="color:var(--del)">${ic('stop', 'xs')}</span>`;
const rOpen = (label = 'Open') => `<span class="none row" style="gap:3px;font-size:12px;color:var(--ac);padding:0 4px">${label}${ic('arrow-up-right', 'xs')}</span>`;
const rTag = (text) => `<span class="none" style="font-size:11px;padding:1px 6px;border-radius:4px;background:var(--hov);color:var(--mu)">${text}</span>`;
const rTip = (html, style) => `<div class="pop" style="${style};padding:5px 8px;font-size:12px;white-space:nowrap;display:flex;align-items:center;gap:8px">${html}</div>`;
const rAz = (size = 14) => `<span class="mono" style="background:#4b3034;color:#e06c75;width:${size}px;height:${size}px;font-size:${size > 14 ? 8 : 7}px">AZ</span>`;
const rWorktree = (cls = 'xs') => ic('worktree', cls);

// The parent thread and its subthreads, newest first as ProjectStore::subthreads orders them.
const R_PARENT = 'Add usage indicators to the agent picker';
const R_PARENT_SHORT = 'Add usage indicators to the agent pick…';
const R_SUBS = [
  { id: 284, title: 'Review the server side of usage reads', glyph: 'claude', agent: 'Claude Agent', model: 'Opus 5.5', role: 'review', state: 'working', time: '52s', where: null },
  { id: 283, title: 'Write the picker’s tests', glyph: 'droid', agent: 'Factory Droid', model: 'Opus 5.5', role: 'test', state: 'approval', time: '1m 10s', where: 'agentz/picker-tests', files: [1, 48, 0] },
  { id: 282, title: 'Build the indicator in the picker', glyph: 'codex', agent: 'Codex', model: 'GPT-5.5', role: 'implementation', state: 'working', time: '1m 20s', where: 'agentz/usage-picker', files: [3, 84, 12] },
  { id: 281, title: 'Research how Zed shows usage', glyph: 'claude', agent: 'Claude Agent', model: 'Opus 5.5', role: 'research', state: 'done', time: '2m 14s', where: null },
];
const R_DONE = R_SUBS.map((s, i) => ({ ...s, state: 'done', time: ['3m 02s', '4m 40s', '5m 12s', '2m 14s'][i] }));
const rRunning = (subs) => subs.filter((s) => s.state === 'working' || s.state === 'approval').length;
const rStatusIcon = (s, size = 'sm') => s.state === 'done' ? rCheck(size) : s.state === 'failed' ? rCross(size) : s.state === 'approval' ? `<span style="display:inline-flex;color:var(--warn)">${ic('warning', size)}</span>` : rSpin(size === 'sm' ? 12 : 10);
const R_SUMMARY = 'Zed shows an account’s usage as a thin bar under the model picker’s rows, and the percentage in the row’s tooltip. It reads the limit from the provider’s API every minute.';

// The Agents view's sidebar (sidebar.rs render_thread_card): t3code's thread cards with every
// project shown, then the Shells shelf.
const rStatus = (state, time) => {
  if (state === 'working') return `<span class="row g1 none" style="color:var(--ac);font-size:12px;font-weight:500">${rSpin(11)}Working</span>`;
  if (state === 'approval') return '<span class="pill pending">' + dot('pending') + 'Needs Approval</span>';
  if (state === 'done') return '<span class="pill done">' + dot('done') + 'Completed</span>';
  return `<span class="sm mu none">${time || ''}</span>`;
};
const rPeople = (count, running) => `<span class="row none" style="gap:2px;font-size:11px;color:${running ? 'var(--ac)' : 'var(--faint)'}">${ic('users', 'xs')}${count}</span>`;
function rCard({ title, state, time = '4m', branch = 'main', marker = '', glyphKind = 'claude', people = '', active = false, hover = false, end = '' } = {}) {
  return `<div style="margin:2px 4px;padding:8px 10px;border-radius:6px;position:relative;${active ? 'background:var(--sel)' : hover ? 'background:var(--hov)' : ''}">
    <div class="row g15" style="height:20px">${rAz()}<span class="sm mu grow trunc">agentZ</span>${end || rStatus(state, time)}</div>
    <div class="trunc b5" style="margin-top:4px">${title}</div>
    <div class="row g15 xs faint" style="margin-top:2px;min-width:0">${marker}<span class="grow trunc">${branch}</span>${people}<span style="opacity:.6;display:inline-flex;color:var(--mu)">${ic('laptop', 'sm')}</span>${glyph(glyphKind, 'sm')}</div>
  </div>`;
}
const rShelf = (label, { open = true, count } = {}) => `<div class="row g2" style="height:30px;margin:6px 4px 0;padding:0 10px;font-size:12px;color:var(--mu)"><span>${label}</span>${!open && count ? `<span class="faint">${count}</span>` : ''}<span class="grow" style="height:1px;background:var(--bv)"></span>${ic(open ? 'chev-up' : 'chev-down', 'xs')}</div>`;
// A slim row (render_slim_row): the shelves' rows.
const rSlim = ({ icon = rAz(), title, end, line2 = '', on = false, hover = false, dim = true }) => `<div style="margin:0 4px;padding:5px 10px;border-radius:6px;position:relative;${on ? 'background:var(--sel)' : hover ? 'background:var(--hov)' : ''}">
  <div class="row" style="gap:10px;height:24px"><span style="opacity:${dim && !on && !hover ? 0.4 : 1};display:inline-flex">${icon}</span><span class="grow trunc ${on ? '' : 'mu'}">${title}</span>${end}</div>
  ${line2 ? `<div class="row g1 sm faint" style="padding-left:26px;min-width:0">${line2}</div>` : ''}</div>`;
const R_OTHERS = [
  { title: 'Links in terminals aren’t clickable', state: 'done', time: '18m', glyphKind: 'droid' },
  { title: 'Turns split without user messages', time: '1h', glyphKind: 'claude' },
  { title: 'Ctrl-, with a terminal focused', time: '3h', glyphKind: 'codex' },
];
const rOthers = () => R_OTHERS.map((t) => rCard(t)).join('');
const rShells = ({ hover = -1, close = false, running = true } = {}) => rShelf('Shells') + [
  { title: 'npm run dev', line2: `<span class="grow trunc">main</span>${ic('laptop', 'xs')}`, running },
  { title: 'zsh', line2: `<span class="grow trunc">~/projects/agentZ</span>${ic('laptop', 'xs')}`, time: '10m' },
].map((row, index) => {
  const closeButton = `<span class="ibtn sm hov" style="color:var(--t)">${ic('x', 'xs')}</span>`;
  const end = close && hover === index ? closeButton : row.running ? `<span class="row g1 none" style="font-size:12px;font-weight:500;color:var(--ac)">${dot('working')}Running</span>` : `<span class="sm mu none">${row.time}</span>`;
  return rSlim({ icon: ic('terminal', 'sm mu'), title: row.title, end, line2: row.line2, hover: hover === index });
}).join('');
const rSidebar = (list, { h = 640, w = 290 } = {}) => frame(sidebar({ list }), { w, h });

// The thread's header (render_toolbar) and conversation.
const rCrumb = `<span class="row g15" style="padding:2px 4px;flex:none">${rAz()}<span class="sm mu">agentZ</span></span><span class="sm mu" style="flex:none">/</span>`;
const rTitleMenu = (title) => `<span class="row g1" style="padding:2px 4px;min-width:0;font-size:13px"><span class="trunc">${title}</span>${ic('chev-down', 'xs mu')}</span>`;
const rHeaderRight = () => `<span class="row g15" style="flex:none"><span class="btn sm" style="gap:4px;height:22px">${ic('branch', 'xs')}main</span><span class="ibtn" style="font-size:15px">±</span>${ibtn('terminal')}${ibtn('more')}</span>`;
const rHeader = (title = R_PARENT) => `<div class="row g1" style="height:36px;flex:none;padding:0 8px;border-bottom:1px solid var(--b);background:var(--panel)">${rCrumb}<div class="row grow" style="min-width:0">${rTitleMenu(title)}</div>${rHeaderRight()}</div>`;
const rPara = (html, style = '') => `<div style="line-height:22px;color:var(--t);padding:4px 0;${style}">${html}</div>`;
const rBubble = (html) => `<div style="display:flex;justify-content:flex-end;padding:4px 0 10px"><div style="max-width:78%;background:#3a3f4b;border-radius:10px;padding:8px 12px;line-height:21px">${html}</div></div>`;
// A tool call's row (thread-rows round B: one dim gray, commands in the code font).
const rRow = (icon, html, { trailing = '', chevron = '', indent = 0, iconHtml } = {}) => `<div class="row" style="min-height:24px;gap:6px;padding:0 2px;margin-left:${indent}px;border-radius:5px;color:${R_DIM};font-size:13px">
  <span style="width:24px;display:inline-flex;justify-content:center;color:${R_DIM}">${iconHtml || ic(icon, 'sm')}</span><span class="row grow" style="gap:4px;min-width:0">${html}</span>${trailing}${chevron}</div>`;
const rVerb = (text) => `<span class="none">${text}</span>`;
const rTitle = (text) => `<span class="trunc" style="color:var(--t)">${text}</span>`;
const rFold = (text, open = false) => rRow(open ? 'chev-down' : 'chev-right', `<span class="trunc">${text}</span>`);
const rConversation = (html, { h = 420, w = 780, header = rHeader(), bottom = '' } = {}) => frame(`<div class="col" style="height:100%;background:var(--panel)">${header}<div class="grow" style="min-height:0;overflow:hidden;padding:12px 24px 0;display:flex;flex-direction:column;gap:2px">${html}</div>${bottom}</div>`, { w, h });
const rAsk = rBubble('Add usage indicators to the agent picker. Split it up: research how Zed does it, build it in a worktree, write tests, and have someone review the server side.');
const rSaid = rPara('I started four subthreads: one researches how Zed shows usage, one builds the indicator in a worktree, one writes the tests, and one reviews the server side. I’ll put it together once they’re done.');

// The activity bar over the composer (render_activity_bar) and the composer.
const rActivity = (sections) => `<div style="padding:0 8px;flex:none;position:relative"><div style="background:#31353e;border:1px solid var(--b);border-bottom:0;border-radius:6px 6px 0 0">${sections.join('<div style="height:1px;background:var(--b)"></div>')}</div></div>`;
const rComposer = () => `<div style="border-top:1px solid var(--b);background:var(--ed);padding:8px 16px;flex:none">
  <div class="ph" style="padding:4px 4px 12px">Message the agent…</div>
  <div class="row" style="gap:10px;font-size:12px;color:var(--mu)">${ic('plus', 'sm')}<span class="row g1">${glyph('claude', 'sm')}Claude Agent</span><span class="grow"></span><span class="row g1">Opus 5.5 ${ic('chev-down', 'xs')}</span><span class="row g1">High ${ic('chev-down', 'xs')}</span><span class="ibtn" style="background:var(--sel);color:var(--t)">${ic('send', 'xs')}</span></div></div>`;

// The Agents list as built (render_agents_section): Zed's subagent headers as rows.
const rAgentsHead = (subs, { open = true, done = false } = {}) => {
  const running = rRunning(subs);
  const extra = done ? '<span class="sm mu">· all done</span>' : running ? `<span class="sm mu">· ${running} running</span>` : '';
  return `<div class="row g1" style="padding:4px;${open ? 'border-bottom:1px solid var(--b)' : ''}"><span class="ibtn sm">${ic(open ? 'chev-down' : 'chev-right', 'xs')}</span><span class="sm mu">${subs.length} Agents</span>${extra}</div>`;
};
const rListRow = (s, last) => `<div class="row" style="height:32px;padding:0 8px;gap:6px;background:var(--ed);${last ? '' : 'border-bottom:1px solid var(--bv)'}"><span style="width:16px;display:inline-flex;justify-content:center">${rStatusIcon(s)}</span><span class="trunc" style="font-size:13px;flex:0 1 auto">${s.title}</span><span class="sm mu none">· ${s.model}</span><span class="grow"></span>${s.files ? `<span class="sm mu none">—</span>${rStat(s.files)}` : ''}${s.state === 'working' || s.state === 'approval' ? rStop() : ''}</div>`;
const rAgentsList = (subs = R_SUBS, row = rListRow) => rAgentsHead(subs) + subs.map((s, i) => row(s, i === subs.length - 1)).join('');

// Zed's subagent card (render_subthread_card): the parent's delegate_task call.
const rZedCard = (s, { body = '', dashed = false } = {}) => `<div style="border:1px ${dashed ? 'dashed' : 'solid'} var(--b);border-radius:6px;overflow:hidden;margin:3px 0;background:var(--ed)"><div class="row" style="height:32px;padding:0 8px;gap:6px;background:#2c313a"><span style="width:16px;display:inline-flex;justify-content:center">${rStatusIcon(s)}</span><span class="trunc" style="font-size:13px;flex:0 1 auto">${s.title}</span><span class="sm mu none">· ${s.model}</span><span class="grow"></span>${s.state === 'approval' ? '<span class="xs warnc none" style="padding:1px 6px;border-radius:4px;background:rgba(222,193,132,.12)">Waiting for you</span>' : ''}${s.files ? rStat(s.files) : ''}${s.state === 'working' || s.state === 'approval' ? rStop() : ''}</div>${body}</div>`;
const rZedStrip = () => `<div class="row" style="justify-content:center;height:24px;border-top:1px solid var(--b);color:var(--ph)">${ic('maximize', 'xs')}</div>`;
const rZedStep = (text) => `<div class="row g2" style="padding:6px 12px;font-size:12px;color:${R_DIM}">${ic('search', 'xs')}<span class="trunc">${text}</span></div>`;

// The new thread screen's composer and the row under it (render_new_thread_strip).
const rChip = (icon, label, { open = false, chevron = true } = {}) => `<span class="row g15 none" style="height:24px;padding:0 6px;border-radius:5px;font-size:13px;color:var(--mu);${open ? 'background:var(--hov);color:var(--t)' : ''}">${icon}${label}${chevron ? ic('chev-down', 'xs') : ''}</span>`;
const rNewComposer = ({ agent = 'Google Antigravity', agentGlyph = 'antigravity', model = 'Gemini 3.8 Flash' } = {}) => `<div style="border:1px solid var(--b);border-radius:10px;background:var(--ed);padding:12px 12px 8px">
  <div class="ph" style="padding:2px 2px 26px;font-size:15px">Message the agent…</div>
  <div class="row" style="gap:12px;font-size:13px;color:var(--mu)">${ic('plus', 'sm')}<span class="row g15">${glyph(agentGlyph, 'sm')}${agent}${ic('chev-down', 'xs')}</span><span class="grow"></span><span class="row g1">${model} ${ic('chev-down', 'xs')}</span><span class="row g1">High ${ic('chev-down', 'xs')}</span><span class="row g1">YOLO ${ic('chev-down', 'xs')}</span><span class="ibtn" style="background:var(--sel);color:var(--t)">${ic('send', 'xs')}</span></div></div>`;
const rStrip = ({ machine = 'This Mac', machineIcon = 'laptop', open = true, middle } = {}) => `<div class="row" style="gap:6px;padding:8px 2px 0">${middle ?? `${rChip(ic('folder', 'sm'), 'Local')}${rChip(ic(machineIcon, 'sm'), machine, { open })}`}<span class="grow"></span><span class="row g1 sm mu">${ic('branch', 'xs')}main</span></div>`;
const rNewThread = (menu, { h = 380, w = 760, composer = rNewComposer(), strip = rStrip() } = {}) => frame(`<div style="position:absolute;inset:0;background:var(--panel);padding:18px 24px">
  <div style="text-align:center;font-size:26px;padding:4px 0 18px;color:var(--t)">What should we build in <span style="border-bottom:1px dashed var(--mu)">agentZ</span> ${ic('chev-down', 'sm mu')} ?</div>
  ${composer}${strip}${menu}</div>`, { w, h });

// The machines: this Mac, the cloud machine agents-1, and Devbox 1.
const R_COPIES = [
  { name: 'This Mac', icon: 'laptop', path: '~/projects/agentZ', branch: 'main', upstream: 'origin/main', ahead: 0, behind: 0, changes: null, fetched: '9m', agents: ['Google Antigravity', 'Claude Agent', 'Codex'] },
  { name: 'agents-1', icon: 'cloud', path: '/home/ubuntu/projects/agentZ', branch: 'main', upstream: 'origin/main', ahead: 0, behind: 2, changes: [3, 42, 7], fetched: null, agents: ['Claude Agent', 'Codex', 'Factory Droid'] },
  { name: 'Devbox 1', icon: 'server', path: '/root/projects/agentZ', branch: 'main', upstream: 'origin/main', ahead: 0, behind: 0, changes: null, fetched: '2h', offline: true, agents: ['Claude Agent'] },
];
const rUpstream = (copy) => {
  const marks = `${copy.ahead ? ` <span class="okc">↑${copy.ahead}</span>` : ''}${copy.behind ? ` <span class="delc">↓${copy.behind}</span>` : ''}`;
  return `<span class="mu">→ ${copy.upstream}</span>${marks || ' <span class="mu">up to date</span>'}`;
};
const rChanges = (copy) => (copy.changes ? `<span>${copy.changes[0]} files</span> <span class="okc">+${copy.changes[1]}</span> <span class="delc">−${copy.changes[2]}</span>` : '<span class="mu">no changes</span>');
const rMenu = (rows, style, { w } = {}) => `<div class="menu" style="${style};${w ? `width:${w}px;` : ''}padding:4px">${rows}</div>`;
