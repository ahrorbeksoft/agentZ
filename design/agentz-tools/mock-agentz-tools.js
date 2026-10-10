// agentZ's own tools, subthreads and subagents in a thread: building blocks for this round's
// mocks, on top of board/mock.js. Thread pieces as agent_view.rs draws them, in JetBrains Dark,
// at the app's sizes: rows 24 px, row text 13 px, details 12 px.

Object.assign(ICONS, {
  agentz: '<rect x="3" y="3" width="18" height="18" rx="4" opacity=".55"/><path d="M8 8h8l-8 8h8"/>',
  'zed-search': '<g transform="scale(1.5)" stroke-width="1.2"><path d="M13 13L11 11"/><circle cx="7.5" cy="7.5" r="4.5"/></g>',
  'arrow-up-right': '<path d="M7 7h10v10"/><path d="M7 17 17 7"/>',
  'arrow-left': '<path d="m12 19-7-7 7-7"/><path d="M19 12H5"/>',
  'x-circle': '<circle cx="12" cy="12" r="10"/><path d="m15 9-6 6"/><path d="m9 9 6 6"/>',
  circle: '<circle cx="12" cy="12" r="9"/>',
  sparkle: '<path d="M12 3l1.9 5.6L19.5 10.5l-5.6 1.9L12 18l-1.9-5.6L4.5 10.5l5.6-1.9z"/>',
});
GLYPHS.droid = '❋';

const ZT = '--ed:#1e1f22;--panel:#26282b;--title:#26282b;--t:#dfe1e5;--mu:#b0b1b3;--ph:#6f737a;--b:#393b41;--bv:#393b41;--bf:#3574f0;--hov:#3c3e41;--sel:#43454a;--ac:#548af7;--ok:#57965d;--del:#fa6675;--warn:#e0b45c;';
// The rows' gray (work_row_color), and the brighter gray of what a row acted on.
const ZDIM = '#8d8e91';
const ZBRIGHT = '#c4c6ca';
const ZMONO = "'IBM Plex Mono','SF Mono',Menlo,monospace";
const ZW = 760;

// The conversation at the app's margins (mx_5).
const zConv = (html, h, w = ZW) => frame(`<div style="position:absolute;inset:0;overflow:hidden;background:var(--panel);padding:14px 20px;display:flex;flex-direction:column;gap:2px">${html}</div>`, { w, h, style: ZT });

// A row (render_tool_call): a 24 px icon cell, the label in the rows' gray, then what trails.
const zRow = (icon, label, { trailing = '', hover = false, chevron = '', indent = 0, iconColor = ZDIM } = {}) => `<div class="row" style="min-height:24px;gap:6px;padding:0 2px;margin-left:${indent}px;border-radius:5px;${hover ? 'background:var(--hov);' : ''}">
  <span style="width:24px;display:inline-flex;justify-content:center;color:${iconColor}">${icon}</span>
  <span class="row grow" style="gap:4px;min-width:0;font-size:13px;color:${ZDIM}">${label}</span>${trailing}${chevron ? `<span style="color:var(--ph);display:inline-flex;margin-right:2px">${ic(chevron, 'xs')}</span>` : ''}</div>`;
const zVerb = (text) => `<span class="none">${text}</span>`;
const zText = (text) => `<span class="trunc">${text}</span>`;
const zTitle = (text) => `<span class="trunc" style="color:${ZBRIGHT}">${text}</span>`;
const zCode = (text) => `<span class="trunc" style="font:12px ${ZMONO}">${text}</span>`;
const zDim = (text, style = '') => `<span class="none" style="font-size:12px;color:var(--ph);${style}">${text}</span>`;
const zOpen = (text = 'Open') => `<span class="none row" style="gap:3px;font-size:12px;color:var(--ac);padding:0 4px">${text}${ic('arrow-up-right', 'xs')}</span>`;
const zSpin = (color = ZDIM) => `<span class="spin" style="border-color:rgba(141,142,145,.3);border-top-color:${color};margin:0 2px"></span>`;
const zBlueSpin = () => '<span class="spin" style="width:12px;height:12px;border-color:rgba(84,138,247,.3);border-top-color:var(--ac);margin:0 2px"></span>';
const zCheck = () => `<span style="display:inline-flex;color:var(--ok);margin:0 2px">${ic('check', 'xs')}</span>`;
const zCross = () => `<span style="display:inline-flex;color:var(--del);margin:0 2px">${ic('x', 'xs')}</span>`;
const zFailed = () => '<span class="none" style="font-size:12px;color:var(--del)">Failed</span>';
const zTag = (text) => `<span class="none" style="font-size:11px;padding:1px 6px;border-radius:4px;background:var(--hov);color:var(--mu)">${text}</span>`;
const zStat = (add, del) => `<span class="none" style="font-size:12px"><span style="color:#57965d">+${add}</span> <span style="color:#fa6675">−${del}</span></span>`;
const zStop = () => `<span style="width:22px;height:22px;border-radius:5px;display:inline-grid;place-items:center;color:var(--del)">${ic('stop', 'xs')}</span>`;

// What opens under a row, past its icon (ml 30 px).
const zOut = (html, style = '') => `<div style="margin-left:30px;padding:4px 0 6px;display:flex;flex-direction:column;gap:6px;${style}">${html}</div>`;
const zBlock = (text, style = '') => `<div style="border:1px solid var(--b);border-radius:6px;background:var(--ed);padding:8px;overflow:hidden;${style}"><div style="font:12px/17px ${ZMONO};color:#c9ccd1;white-space:pre-wrap;overflow-wrap:anywhere">${text}</div></div>`;
const zInput = (open = false) => `<div class="row g1" style="font:11px ${ZMONO};color:var(--ph)">Input${ic(open ? 'chev-up' : 'chev-down', 'xs')}</div>`;
const zHeading = (text, extra = '') => `<div class="row g2" style="font-size:12px;color:var(--ph)">${text}${extra}</div>`;

const zPara = (html, style = '') => `<div style="line-height:22px;color:var(--t);padding:4px 0;${style}">${html}</div>`;
const zBubble = (html) => `<div style="display:flex;justify-content:flex-end;padding:4px 0 10px"><div style="max-width:78%;background:#2f3134;border-radius:12px;padding:8px 12px;line-height:21px">${html}</div></div>`;
const zInline = (text) => `<code style="font:12px ${ZMONO};background:var(--hov);padding:1px 4px;border-radius:3px">${text}</code>`;
// Board captions inside a mock, to say what each part shows. Not part of the app.
const zCaption = (text) => `<div style="font:11px -apple-system,sans-serif;color:#e0b45c;margin:8px 0 4px;letter-spacing:.2px">${text}</div>`;
const zStack = (items) => items.map(([label, html]) => `${zCaption(label)}${html}`).join('');

// t3code's subagent avatar: the agent's icon on a round tile, with a status dot.
const zAvatar = (glyphText, dotColor) => `<span style="position:relative;width:24px;height:24px;border-radius:50%;border:1px solid #4a4d54;background:#2f3134;display:grid;place-items:center;flex:none;font-size:11px;color:#dfe1e5">${glyphText}${dotColor ? `<i style="position:absolute;right:-1px;bottom:-1px;width:8px;height:8px;border-radius:50%;background:${dotColor};box-shadow:0 0 0 2px var(--panel)"></i>` : ''}</span>`;
// t3code's SubagentTimelineLink: avatar, title over its status or detail, the time, a chevron.
const zT3Card = ({ glyphText = '✻', dotColor, title, status = '', statusColor = 'var(--ph)', detail = '', time = '', hover = false }) => `<div class="row" style="gap:10px;padding:6px 8px;border-radius:6px;margin-left:24px;${hover ? 'background:var(--hov);' : ''}">
  ${zAvatar(glyphText, dotColor)}
  <span class="col grow" style="min-width:0;gap:1px"><span class="row" style="gap:8px;min-width:0"><span class="trunc" style="font-size:12px;font-weight:500;color:var(--t)">${title}</span>${status && detail ? `<span class="none" style="font-size:10px;color:${statusColor}">${status}</span>` : ''}</span><span class="trunc" style="font-size:11px;color:${detail ? 'var(--ph)' : statusColor}">${detail || status}</span></span>
  <span style="font-size:12px;color:var(--ph)">${time}</span><span style="color:var(--ph);display:inline-flex">${ic('chev-right', 'xs')}</span></div>`;
// Zed's subagent card: a bordered card with a 32 px header and what's under it.
const zZedCard = (header, body = '', { dashed = false } = {}) => `<div style="border:1px ${dashed ? 'dashed' : 'solid'} var(--b);border-radius:6px;overflow:hidden;margin:3px 0">${header}${body}</div>`;
const zZedHead = ({ state, title, meta = '', files = '', stop = false, hover = false }) => `<div class="row" style="height:32px;padding:0 8px;gap:6px;background:${hover ? 'var(--hov)' : '#2b2d30'}"><span style="width:16px;display:inline-flex;justify-content:center">${state === 'working' ? zBlueSpin() : state === 'failed' ? zCross() : zCheck()}</span><span class="trunc" style="font-size:13px;flex:0 1 auto;color:var(--t)">${title}</span>${meta ? `<span class="sm" style="flex:none;color:var(--ph)">· ${meta}</span>` : ''}${files ? `<span class="sm" style="flex:none;color:var(--ph)">— ${files}</span>` : ''}<span class="grow"></span>${stop ? zStop() : ''}</div>`;
const zStrip = () => `<div class="row" style="justify-content:center;height:26px;border-top:1px solid var(--b);color:var(--ph)">${ic('maximize', 'sm')}</div>`;
// A box of steps that fades at the top when there are more above it.
const zFade = (html, h) => `<div style="position:relative;max-height:${h}px;overflow:hidden;display:flex;flex-direction:column;justify-content:flex-end;padding:4px">${html}<div style="position:absolute;inset:0;background:linear-gradient(180deg,var(--ed) 0%,rgba(30,31,34,0) 35%)"></div></div>`;

// The demo world: the thread from the Subthreads round, its subthreads, and other threads.
const SUBS = [
  { title: 'Research: UI for child tasks', role: 'research', model: 'Opus 4.1', state: 'done', time: '2m 14s', files: '',
    step: ['zed-search', 'Searched “SubagentTimelineLink”'],
    summary: 't3code draws each child as a row with the agent’s icon and a status dot, its title over its last step or its result, and how long it ran; the whole row opens the child thread.' },
  { title: 'Research: how agents get agentZ’s tools', role: 'research', model: 'Opus 4.1', state: 'working', time: '1m 02s', files: '',
    step: ['file', 'Read crates/agentz_server/src/mcp_bridge.rs'], summary: '' },
  { title: 'Research: server side of delegated tasks', role: 'research', model: 'Sonnet 4.5', state: 'working', time: '48s', files: '',
    step: ['zed-search', 'Searched “finish_tasks”'], summary: '' },
];
const SUB_STEPS = [
  ['file', 'Read docs/architecture.md'],
  ['zed-search', 'Searched “subagent”', '14 results'],
  ['file', 'Read apps/web/src/components/chat/V2LifecycleRow.tsx'],
  ['file', 'Read apps/web/src/components/chat/ProviderSubagentBar.tsx'],
  ['zed-search', 'Searched “SubagentTimelineLink”', '3 results'],
  ['file', 'Read apps/web/src/components/chat/MessagesTimeline.tsx'],
  ['terminal', 'Ran', 'rg -n "formatSubagentDisplayTitle" apps/web'],
  ['file', 'Read crates/app/src/agent_view.rs'],
];
const OTHER_THREAD = 'Fix flaky login test';
const userAsk = zBubble('i wanna test subthreads, spawn some sub threads to research how child tasks should look');
const agentSaid = zPara('I started three research subthreads; I’ll put their findings together once they’re done.');

// A step as the subthread's own row shows it.
const zStep = ([icon, text, extra], { indent = 0, spins = false } = {}) => {
  const label = icon === 'terminal' ? `${zVerb(text)}${zCode(extra)}` : zText(text);
  const trailing = icon === 'terminal' ? '' : extra ? zDim(extra) : '';
  return zRow(ic(icon, 'sm'), label, { trailing: `${trailing}${spins ? zSpin() : ''}`, indent });
};
