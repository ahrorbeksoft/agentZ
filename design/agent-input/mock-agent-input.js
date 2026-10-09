// Agents' requests for input in a thread: building blocks for this round's mocks, on top of
// board/mock.js. Thread pieces as agent_view.rs and elicitation_card.rs draw them, in
// JetBrains Dark, at the app's sizes: rows 24 px, row text 13 px, details 12 px.

Object.assign(ICONS, {
  'zed-hammer': '<g transform="scale(1.5)" stroke-width="1.2"><path d="M9 8.5L4.95 12.62a1.25 1.25 0 0 1-1.75-1.75L7.5 6.5"/><path d="M10.84 9.98l3-3"/><path d="M12.84 7.42l-1.07-1a1 1 0 0 1-.33-.73v-.61L10.17 3.9A3.4 3.4 0 0 0 7.82 3l-1.98-.01.52.43a2.9 2.9 0 0 1 1.15 2.4L7.5 6.5 9 8.5l.5-.5s.37-.2.58-.01l1.07 1"/></g>',
  'zed-search': '<g transform="scale(1.5)" stroke-width="1.2"><path d="M13 13L11 11"/><circle cx="7.5" cy="7.5" r="4.5"/></g>',
  'check-double': '<path d="M18 6 7 17l-5-5"/><path d="m22 10-7.5 7.5L13 16"/>',
  'arrow-up-right': '<path d="M7 7h10v10"/><path d="M7 17 17 7"/>',
  'arrow-up': '<path d="m5 12 7-7 7 7"/><path d="M12 19V5"/>',
  'arrow-left': '<path d="m12 19-7-7 7-7"/><path d="M19 12H5"/>',
  swap: '<path d="m16 3 4 4-4 4"/><path d="M20 7H4"/><path d="m8 21-4-4 4-4"/><path d="M4 17h16"/>',
  shield: '<path d="M20 13c0 5-3.5 7.5-7.66 8.95a1 1 0 0 1-.67-.01C7.5 20.5 4 18 4 13V6a1 1 0 0 1 1-1c2 0 4.5-1.2 6.24-2.72a1.17 1.17 0 0 1 1.52 0C14.51 3.81 17 5 19 5a1 1 0 0 1 1 1z"/>',
  'shield-check': '<path d="M20 13c0 5-3.5 7.5-7.66 8.95a1 1 0 0 1-.67-.01C7.5 20.5 4 18 4 13V6a1 1 0 0 1 1-1c2 0 4.5-1.2 6.24-2.72a1.17 1.17 0 0 1 1.52 0C14.51 3.81 17 5 19 5a1 1 0 0 1 1 1z"/><path d="m9 12 2 2 4-4"/>',
  'shield-x': '<path d="M20 13c0 5-3.5 7.5-7.66 8.95a1 1 0 0 1-.67-.01C7.5 20.5 4 18 4 13V6a1 1 0 0 1 1-1c2 0 4.5-1.2 6.24-2.72a1.17 1.17 0 0 1 1.52 0C14.51 3.81 17 5 19 5a1 1 0 0 1 1 1z"/><path d="m14.5 9.5-5 5"/><path d="m9.5 9.5 5 5"/>',
  'help-circle': '<circle cx="12" cy="12" r="10"/><path d="M9.09 9a3 3 0 0 1 5.83 1c0 2-3 3-3 3"/><path d="M12 17h.01"/>',
  messages: '<path d="M14 9a2 2 0 0 1-2 2H6l-4 4V4a2 2 0 0 1 2-2h8a2 2 0 0 1 2 2z"/><path d="M18 9h2a2 2 0 0 1 2 2v11l-4-4h-6a2 2 0 0 1-2-2v-1"/>',
  map: '<path d="M14.1 4.6a2 2 0 0 0 1.8 0l3.7-1.9A1 1 0 0 1 21 3.6v12.8a1 1 0 0 1-.6.9l-4.5 2.2a2 2 0 0 1-1.8 0l-4.2-2.1a2 2 0 0 0-1.8 0l-3.7 1.9A1 1 0 0 1 3 18.4V5.6a1 1 0 0 1 .6-.9l4.5-2.2a2 2 0 0 1 1.8 0z"/><path d="M15 5.8v15"/><path d="M9 3.2v15"/>',
  lock: '<rect width="18" height="11" x="3" y="11" rx="2" ry="2"/><path d="M7 11V7a5 5 0 0 1 10 0v4"/>',
  'eye-off': '<path d="M10.73 5.08A10.43 10.43 0 0 1 12 5c7 0 10 7 10 7a13.16 13.16 0 0 1-1.67 2.68"/><path d="M6.61 6.61A13.53 13.53 0 0 0 2 12s3 7 10 7a9.74 9.74 0 0 0 5.39-1.61"/><line x1="2" x2="22" y1="2" y2="22"/>',
  plug: '<path d="M12 22v-5"/><path d="M9 8V2"/><path d="M15 8V2"/><path d="M18 8v5a4 4 0 0 1-4 4h-4a4 4 0 0 1-4-4V8Z"/>',
  'square-pen': '<path d="M12 3H5a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7"/><path d="M18.375 2.625a1 1 0 0 1 3 3l-9.013 9.014a2 2 0 0 1-.853.505l-2.873.84a.5.5 0 0 1-.62-.62l.84-2.873a2 2 0 0 1 .506-.852z"/>',
  dots: '<circle cx="9" cy="6" r="1"/><circle cx="15" cy="6" r="1"/><circle cx="9" cy="12" r="1"/><circle cx="15" cy="12" r="1"/><circle cx="9" cy="18" r="1"/><circle cx="15" cy="18" r="1"/>',
  'chev-left': '<path d="m15 18-6-6 6-6"/>',
  return: '<polyline points="9 10 4 15 9 20"/><path d="M20 4v7a4 4 0 0 1-4 4H4"/>',
  download: '<path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/><polyline points="7 10 12 15 17 10"/><line x1="12" x2="12" y1="15" y2="3"/>',
});

const QT = '--ed:#1e1f22;--panel:#26282b;--title:#26282b;--t:#dfe1e5;--mu:#b0b1b3;--ph:#6f737a;--b:#393b41;--bv:#393b41;--bf:#3574f0;--hov:#3c3e41;--sel:#43454a;--ac:#548af7;--ok:#57965d;--del:#fa6675;--warn:#e0b45c;--pur:#b189f5;';
// The rows' gray (work_row_color), and the brighter gray of what a row acted on.
const QDIM = '#8d8e91';
const QBRIGHT = '#c4c6ca';
const QMONO = "'IBM Plex Mono','SF Mono',Menlo,monospace";
const QW = 700;
// The card's background (elicitation_card.rs: the editor's surface on the panel).
const QCARD = '#2b2d30';

// The conversation at the app's margins (mx_5), scrolled to its end.
const qConv = (html, h, w = QW, style = '') => frame(`<div style="position:absolute;inset:0;overflow:hidden;background:var(--panel);padding:14px 20px;display:flex;flex-direction:column;justify-content:flex-end;gap:2px;${style}">${html}</div>`, { w, h, style: QT });

// A whole thread pane: the conversation, then what's docked over the composer, then the composer.
function qThread({ conv = '', dock = '', composer = qComposer(), h = 420, w = QW, head = '' } = {}) {
  return frame(`<div style="position:absolute;inset:0;display:flex;flex-direction:column;background:var(--panel)">${head}
    <div style="flex:1;min-height:0;overflow:hidden;padding:14px 20px 6px;display:flex;flex-direction:column;justify-content:flex-end;gap:2px">${conv}</div>
    ${dock ? `<div style="margin:0 20px">${dock}</div>` : ''}${composer}</div>`, { w, h, style: QT });
}
const qHead = (title = 'Checkout total') => `<div class="row" style="height:36px;flex:none;padding:0 12px;gap:8px;border-bottom:1px solid var(--b);font-size:13px"><span class="mono t" style="background:#2f4548;color:#6eb4bf">SF</span><span style="color:var(--mu)">storefront</span><span style="color:var(--ph)">/</span><span class="b5">${title}</span>${ic('chev-down', 'xs')}<span class="grow"></span><span class="sm" style="color:var(--mu)">${ic('branch', 'xs')} checkout-flow</span></div>`;

// The composer: its text, then its toolbar (mode, model, send).
const qComposer = ({ text = '', placeholder = 'Message Claude Agent — @ to add context, / for commands', top = '', bottom = '', send = 'send', style = '' } = {}) => `<div style="margin:6px 20px 14px;border:1px solid var(--b);border-radius:8px;background:var(--ed);flex:none;${style}">${top}
  <div style="padding:9px 12px;min-height:40px;font-size:13px;line-height:20px;color:${text ? 'var(--t)' : 'var(--ph)'}">${text || placeholder}</div>
  <div class="row" style="height:32px;padding:0 6px 0 10px;gap:10px;font-size:12px;color:var(--mu)">${bottom || `<span class="row g1">Default ${ic('chev-down', 'xs')}</span><span class="row g1">Opus 4.1 ${ic('chev-down', 'xs')}</span><span class="grow"></span>`}${send === 'send' ? `<span style="width:24px;height:24px;border-radius:6px;background:var(--hov);display:grid;place-items:center;color:var(--mu)">${ic('arrow-up', 'sm')}</span>` : send}</div></div>`;

// A tool call's row (render_tool_call).
const qRow = (icon, label, { trailing = '', hover = false, indent = 0, iconColor = QDIM, color = QDIM } = {}) => `<div class="row" style="min-height:24px;gap:6px;padding:0 2px;margin-left:${indent}px;border-radius:5px;${hover ? 'background:var(--hov);' : ''}">
  <span style="width:24px;display:inline-flex;justify-content:center;color:${iconColor}">${typeof icon === 'string' && ICONS[icon] ? ic(icon, 'sm') : icon}</span>
  <span class="row grow" style="gap:4px;min-width:0;font-size:13px;color:${color}">${label}</span>${trailing}</div>`;
const qVerb = (text) => `<span class="none">${text}</span>`;
const qText = (text) => `<span class="trunc">${text}</span>`;
const qBright = (text) => `<span class="trunc" style="color:${QBRIGHT}">${text}</span>`;
const qCode = (text, color = 'inherit') => `<span class="trunc" style="font:12px ${QMONO};color:${color}">${text}</span>`;
const qDim = (text, style = '') => `<span class="none" style="font-size:12px;color:var(--ph);${style}">${text}</span>`;
const qSpin = (color = QDIM) => `<span class="spin" style="border-color:rgba(141,142,145,.3);border-top-color:${color};margin:0 4px"></span>`;
const qStat = (add, del) => `<span class="none" style="font-size:12px"><span style="color:#57965d">+${add}</span> <span style="color:#fa6675">−${del}</span></span>`;
const qOpen = (text = 'Open') => `<span class="none row" style="gap:3px;font-size:12px;color:var(--ac);padding:0 4px">${text}${ic('arrow-up-right', 'xs')}</span>`;
const qInline = (text) => `<code style="font:12px ${QMONO};background:var(--hov);padding:1px 4px;border-radius:3px">${text}</code>`;

const qPara = (html, style = '') => `<div style="line-height:22px;color:var(--t);padding:4px 0;${style}">${html}</div>`;
const qBubble = (html) => `<div style="display:flex;justify-content:flex-end;padding:4px 0 10px"><div style="max-width:78%;background:#2f3134;border-radius:12px;padding:8px 12px;line-height:21px">${html}</div></div>`;
// The line under the turn (render_generating): its dots, the state, how long it's run.
const qGen = (label = '', time = 'Working for 23s', color = 'var(--ph)') => `<div class="row" style="height:28px;gap:8px;font-size:12px;color:var(--ph);padding-left:2px;margin-top:4px"><span style="display:inline-flex;color:var(--ph)">${ic('dots', 'xs')}</span>${label ? `<span style="color:${color}">${label}</span>` : ''}<span>${time}</span></div>`;

// Board captions inside a mock, to say what each part shows. Not part of the app.
const qCaption = (text) => `<div style="font:11px -apple-system,sans-serif;color:#e0b45c;margin:8px 0 4px;letter-spacing:.2px">${text}</div>`;

// Buttons and controls, as ui's Button styles draw them.
function qBtn(text, { kind = 'outline', icon = '', iconColor = '', key = '', chev = false, small = false, style = '' } = {}) {
  const look = {
    primary: 'background:var(--ac);border-color:var(--ac);color:#fff;font-weight:500',
    outline: 'background:transparent;border-color:var(--b);color:var(--t)',
    ghost: 'background:transparent;border-color:transparent;color:var(--t)',
    filled: 'background:var(--hov);border-color:transparent;color:var(--t)',
    danger: 'background:transparent;border-color:var(--b);color:var(--del)',
  }[kind];
  return `<span class="row" style="height:${small ? 22 : 26}px;padding:0 ${small ? 7 : 10}px;gap:6px;border:1px solid;border-radius:6px;font-size:${small ? 12 : 13}px;white-space:nowrap;flex:none;${look};${style}">${icon ? `<span style="display:inline-flex;color:${iconColor || 'inherit'}">${ic(icon, 'xs')}</span>` : ''}${text}${key ? `<span style="font-size:11px;color:${kind === 'primary' ? 'rgba(255,255,255,.75)' : 'var(--ph)'};margin-left:2px">${key}</span>` : ''}${chev ? ic('chev-down', 'xs') : ''}</span>`;
}
const qKey = (text) => `<span style="display:inline-flex;align-items:center;justify-content:center;min-width:18px;height:18px;padding:0 4px;border-radius:4px;border:1px solid var(--b);font-size:11px;color:var(--mu);background:var(--ed);flex:none">${text}</span>`;
const qRadio = (on) => `<span style="width:14px;height:14px;border-radius:50%;flex:none;border:1px solid ${on ? 'var(--ac)' : '#5a5d63'};display:inline-grid;place-items:center;background:var(--ed)">${on ? '<i style="width:6px;height:6px;border-radius:50%;background:var(--ac)"></i>' : ''}</span>`;
const qCheck = (on) => `<span style="width:14px;height:14px;border-radius:3px;flex:none;border:1px solid ${on ? 'var(--ac)' : '#5a5d63'};display:inline-grid;place-items:center;background:${on ? 'var(--ac)' : 'var(--ed)'};color:#fff">${on ? ic('check', 'xs') : ''}</span>`;
const qInput = (text = '', { placeholder = '', focus = false, h = 30, mono = false, error = false, trailing = '' } = {}) => `<div class="row" style="height:${h}px;border:1px solid ${error ? 'var(--del)' : focus ? 'var(--bf)' : 'var(--b)'};border-radius:6px;background:var(--ed);padding:0 10px;gap:6px;font:13px ${mono ? QMONO : 'inherit'};color:${text ? 'var(--t)' : 'var(--ph)'}"><span class="grow trunc">${text || placeholder}${focus ? '<span style="display:inline-block;width:1px;height:14px;background:var(--t);vertical-align:-2px;margin-left:1px"></span>' : ''}</span>${trailing}</div>`;
const qLabel = (text, extra = '') => `<div class="row g1" style="font-size:12px;color:var(--mu);margin-top:2px">${text}${extra}</div>`;
const qHelp = (text) => `<div style="font-size:12px;color:var(--ph);line-height:17px">${text}</div>`;
const qErr = (text) => `<div class="row g1" style="font-size:12px;color:var(--del)">${ic('alert', 'xs')}${text}</div>`;

// One choice: a radio or a checkbox, its label, and the description under it.
const qChoice = ({ kind = 'radio', on = false, label, desc = '', hover = false, key = '', rec = false }) => `<div class="row" style="align-items:flex-start;gap:9px;padding:4px 6px;border-radius:5px;${hover ? 'background:var(--hov);' : ''}">
  ${key ? qKey(key) : `<span style="padding-top:3px">${kind === 'radio' ? qRadio(on) : qCheck(on)}</span>`}
  <span class="col grow" style="gap:1px;min-width:0"><span class="row g2" style="font-size:13px;color:var(--t);line-height:20px">${label}${rec ? '<span style="font-size:11px;padding:0 6px;border-radius:4px;background:rgba(84,138,247,.15);color:var(--ac)">Recommended</span>' : ''}</span>${desc ? `<span style="font-size:12px;color:var(--ph);line-height:17px">${desc}</span>` : ''}</span></div>`;

// The elicitation card: a bordered card with a header (icon, title, ×), its body and a footer.
function qCard({ icon = 'messages', iconColor = 'var(--ac)', title, close = true, body = '', foot = '', border = 'var(--b)', head = '', style = '' }) {
  return `<div style="border:1px solid ${border};border-radius:8px;background:${QCARD};margin:4px 0;overflow:hidden;${style}">
    ${head || `<div class="row" style="height:36px;padding:0 10px 0 12px;gap:8px"><span style="display:inline-flex;color:${iconColor}">${ic(icon, 'sm')}</span><span class="grow trunc" style="font-size:13px;color:var(--t)">${title}</span>${close ? `<span style="color:var(--ph);display:inline-flex">${ic('x', 'sm')}</span>` : ''}</div>`}
    ${body ? `<div class="col" style="padding:2px 12px 12px;gap:8px">${body}</div>` : ''}
    ${foot ? `<div class="row" style="min-height:42px;padding:6px 10px 6px 12px;gap:8px;border-top:1px solid var(--b)">${foot}</div>` : ''}</div>`;
}
const qFoot = (left = `${qKey('⏎')}<span class="sm" style="color:var(--ph)">to submit</span>`, buttons = [qBtn('Decline', { kind: 'ghost' }), qBtn('Submit', { kind: 'primary' })]) => `<span class="row g15">${left}</span><span class="grow"></span>${buttons.join('')}`;

// Today's permission buttons (render_permission_buttons): ghost rows under the tool's row.
const OPT_ICON = { once: ['check', 'var(--mu)'], always: ['check-double', 'var(--ok)'], reject: ['x', 'var(--del)'] };
const qTodayButtons = (options) => `<div style="border-top:1px solid var(--b);margin:2px 0 0;padding:4px 0 2px">${options.map(([kind, text]) => `<div class="row" style="height:24px;gap:6px;padding:0 8px;font-size:13px;color:var(--t)"><span style="display:inline-flex;color:${OPT_ICON[kind][1]}">${ic(OPT_ICON[kind][0], 'xs')}</span>${text}</div>`).join('')}</div>`;

// A small tag, as Zed's Chip.
const qTag = (text, color = 'var(--mu)', bg = 'var(--hov)') => `<span class="none" style="font-size:11px;line-height:16px;padding:0 6px;border-radius:4px;background:${bg};color:${color}">${text}</span>`;

// A terminal box as a command's row opens to.
const qTerm = (text, style = '') => `<div style="border:1px solid var(--b);border-radius:6px;background:var(--ed);padding:6px 8px;font:12px/17px ${QMONO};color:#c9ccd1;white-space:pre-wrap;${style}">${text}</div>`;
// A diff as an edit's row opens to.
const qDiff = (lines, style = '') => `<div style="border:1px solid var(--b);border-radius:6px;background:var(--ed);overflow:hidden;font:12px/18px ${QMONO};${style}"><div class="row" style="height:26px;padding:0 8px;gap:6px;border-bottom:1px solid var(--b);font:12px 'IBM Plex Sans',sans-serif;color:var(--mu)">${ic('file', 'xs')}src/cart/total.ts<span class="grow"></span>${qStat(3, 2)}</div>${lines.map(([sign, text]) => `<div style="padding:0 8px;white-space:pre;${sign === '+' ? 'background:rgba(87,150,93,.16);color:#c9ccd1' : sign === '-' ? 'background:rgba(250,102,117,.13);color:#c9ccd1' : 'color:#9a9da3'}"><span style="display:inline-block;width:14px;color:var(--ph)">${sign === ' ' ? '' : sign}</span>${text}</div>`).join('')}</div>`;

// A thread card in the sidebar (sidebar.rs): project, state, title, then its branch.
const PILL = { pending: ['var(--warn)', 'Pending Approval'], awaiting: ['var(--pur)', 'Awaiting Input'], working: ['var(--ac)', 'Working'], done: ['var(--ok)', 'Completed'], needs: ['var(--pur)', 'Needs You'] };
function qSideRow({ title, state = '', label = '', line2 = 'checkout-flow', line2Color = 'var(--ph)', active = false, project = 'storefront', extra = '' }) {
  const [color, text] = PILL[state] || ['var(--ph)', ''];
  return `<div style="margin:0 4px;padding:7px 10px;border-radius:6px;${active ? 'background:var(--sel);' : ''}">
    <div class="row g2" style="font-size:12px;color:var(--mu)"><span class="mono t" style="background:#2f4548;color:#6eb4bf">SF</span><span class="grow trunc">${project}</span>${state ? `<span class="row g1" style="color:${color};font-weight:500"><i style="width:6px;height:6px;border-radius:50%;background:${color};display:inline-block"></i>${label || text}</span>` : ''}</div>
    <div style="font-size:14px;color:var(--t);margin:4px 0 2px" class="trunc">${title}</div>
    <div class="row g1 trunc" style="font-size:12px;color:${line2Color}">${line2}</div>${extra}</div>`;
}
const qSidebar = (rows, { w = 300, h = 300, head = true } = {}) => frame(`<div style="position:absolute;inset:0;background:var(--panel);display:flex;flex-direction:column;gap:2px">${head ? `<div class="row" style="height:36px;padding:0 10px 0 12px;gap:6px;color:var(--ph);border-bottom:1px solid var(--b);font-size:13px;flex:none">${ic('search', 'sm')}<span class="grow">Search…</span>${ic('plus', 'sm')}</div>` : ''}<div class="col" style="gap:2px;padding:4px 0">${rows}</div></div>`, { w, h, style: QT });

// The demo world: Claude Agent in storefront, working on the checkout total.
const AGENT = 'Claude Agent';
const PLAN_MD = `<div style="font-size:15px;font-weight:600;margin:2px 0 6px">Add a checkout total</div>
  <div style="font-size:13px;font-weight:600;margin:6px 0 2px">Steps</div>
  <ol style="margin:0;padding-left:20px;line-height:21px;font-size:13px"><li>Add ${qInline('cartTotal()')} in ${qInline('src/cart/total.ts')}: sum the line items, then apply the discount.</li><li>Round to cents once, at the end, so totals match the receipt.</li><li>Show the total under the items in ${qInline('src/app/checkout/page.tsx')}.</li><li>Add tests for an empty cart, a discount, and rounding.</li></ol>
  <div style="font-size:13px;font-weight:600;margin:8px 0 2px">Files</div>
  <ul style="margin:0;padding-left:20px;line-height:21px;font-size:13px"><li>${qInline('src/cart/total.ts')} (new)</li><li>${qInline('src/app/checkout/page.tsx')}</li><li>${qInline('src/cart/total.test.ts')} (new)</li></ul>`;
const PLAN_SHORT = `<div style="font-size:15px;font-weight:600;margin:2px 0 6px">Add a checkout total</div>
  <div style="font-size:13px;font-weight:600;margin:6px 0 2px">Steps</div>
  <ol style="margin:0;padding-left:20px;line-height:21px;font-size:13px"><li>Add ${qInline('cartTotal()')} in ${qInline('src/cart/total.ts')}: sum the line items, then apply the discount.</li><li>Round to cents once, at the end, so totals match the receipt.</li></ol>`;
const PLAN_OPTIONS = [
  ['always', 'Yes, clear context (34% used) and use auto mode'],
  ['always', 'Yes, and use auto mode'],
  ['always', 'Yes, and bypass permissions'],
  ['once', 'Yes, manually approve edits'],
  ['reject', 'No, keep planning'],
];
const CLAUDE_BASH = [['once', 'Yes'], ['always', 'Yes, and don’t ask again for npm test commands'], ['reject', 'No']];
const CODEX_BASH = [['once', 'Yes, proceed'], ['always', 'Yes, and don’t ask again for this command in this session'], ['reject', 'No, continue without running it'], ['reject', 'No, and tell Codex what to do differently']];
const DROID_BASH = [['once', 'Allow'], ['always', 'Allow always'], ['always', 'Allow & auto-run (low risk)'], ['always', 'Allow & auto-run (medium risk)'], ['always', 'Allow & auto-run (high risk)'], ['reject', 'Cancel']];
const QUESTION = 'Your rule is that UI changes start on the design board. How should I go about t3code’s icon picker?';
const Q_OPTIONS = [
  ['Design round first', 'A short round on the board: today’s screenshot, t3code’s dialog mocked in agentZ’s look, and options where agentZ differs. Build what you pick.', true],
  ['Port t3code’s as is', 'Skip the board and build t3code’s exactly: one Project icon row with Choose icon and Choose file.'],
];
const userRun = qBubble('Run the tests');
const userPort = qBubble('Port the icon picker');
const agentRun = qPara('The total is in. I’ll run the tests to check it.');
const cmdRow = (verb = 'Run', trailing = qSpin()) => qRow('terminal', `${qVerb(verb)}${qCode('npm test')}`, { trailing });
