// Composer, image markup, pictures and pickers: the demo world and the pieces this round's mocks
// share. Loaded after board/mock.js; names start with c2 so they don't clash with it.

ICONS.send = '<path d="M14.54 21.69a.5.5 0 0 0 .94-.03l6.5-19a.5.5 0 0 0-.64-.64l-19 6.5a.5.5 0 0 0-.03.94l7.93 3.18a2 2 0 0 1 1.11 1.11z"/><path d="m21.85 2.15-10.94 10.94"/>';
ICONS.gauge = '<path d="m12 14 4-4"/><path d="M3.34 19a10 10 0 1 1 17.32 0"/>';
ICONS.image = '<rect width="18" height="18" x="3" y="3" rx="2" ry="2"/><circle cx="9" cy="9" r="2"/><path d="m21 15-3.09-3.09a2 2 0 0 0-2.82 0L6 21"/>';
ICONS.queue = '<path d="M16 5H3"/><path d="M16 12H3"/><path d="M11 19H3"/><path d="m15 18 2 2 4-4"/>';
ICONS['arrow-up'] = '<path d="m5 12 7-7 7 7"/><path d="M12 19V5"/>';
ICONS['arrow-up-right'] = '<path d="M7 7h10v10"/><path d="M7 17 17 7"/>';
ICONS.square = '<rect width="18" height="18" x="3" y="3" rx="2"/>';
ICONS.circle = '<circle cx="12" cy="12" r="10"/>';
ICONS.type = '<polyline points="4 7 4 4 20 4 20 7"/><line x1="9" x2="15" y1="20" y2="20"/><line x1="12" x2="12" y1="4" y2="20"/>';
ICONS.highlighter = '<path d="m9 11-6 6v3h9l3-3"/><path d="m22 12-4.6 4.6a2 2 0 0 1-2.8 0l-5.2-5.2a2 2 0 0 1 0-2.8L14 4"/>';
ICONS.eraser = '<path d="m7 21-4.3-4.3c-1-1-1-2.5 0-3.4l9.6-9.6c1-1 2.5-1 3.4 0l5.6 5.6c1 1 1 2.5 0 3.4L13 21"/><path d="M22 21H7"/><path d="m5 11 9 9"/>';
ICONS.redo = '<path d="m15 14 5-5-5-5"/><path d="M20 9H9.5A5.5 5.5 0 0 0 4 14.5A5.5 5.5 0 0 0 9.5 20H13"/>';
ICONS.sliders = '<line x1="21" x2="14" y1="4" y2="4"/><line x1="10" x2="3" y1="4" y2="4"/><line x1="21" x2="12" y1="12" y2="12"/><line x1="8" x2="3" y1="12" y2="12"/><line x1="21" x2="16" y1="20" y2="20"/><line x1="12" x2="3" y1="20" y2="20"/><line x1="14" x2="14" y1="2" y2="6"/><line x1="8" x2="8" y1="10" y2="14"/><line x1="16" x2="16" y1="18" y2="22"/>';
ICONS['message-square'] = '<path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z"/>';
ICONS.warning = '<path d="m21.73 18-8-14a2 2 0 0 0-3.48 0l-8 14A2 2 0 0 0 4 21h16a2 2 0 0 0 1.73-3"/><path d="M12 9v4"/><path d="M12 17h.01"/>';
ICONS.palette2 = ICONS.palette;
GLYPHS.droid = '❋';
GLYPHS.antigravity = '⋀';

const C2_MONO = "'IBM Plex Mono','SF Mono',Menlo,monospace";
const C2_DIM = '#8d929c';
const c2Code = (text) => `<code style="font:12px ${C2_MONO};background:var(--hov);padding:1px 4px;border-radius:3px">${text}</code>`;
const c2Spin = (size = 12) => `<span class="spin" style="width:${size}px;height:${size}px"></span>`;
const c2Az = (size = 14) => `<span class="mono" style="background:#4b3034;color:#e06c75;width:${size}px;height:${size}px;font-size:7px">AZ</span>`;
const c2Caret = '<span style="display:inline-block;width:1.5px;height:16px;background:var(--ac);vertical-align:-3px;margin-left:1px"></span>';

// The agents installed on this Mac, and Claude Agent's accounts. Claude Agent is working in two
// threads (this one on Personal, another on Work), Codex in one.
const C2_AGENTS = [
  { id: 'claude', name: 'Claude Agent', glyph: 'claude', working: 2, left: 69, accounts: 3, via: 'Personal' },
  { id: 'codex', name: 'Codex', glyph: 'codex', working: 1, left: 41, accounts: 1 },
  { id: 'droid', name: 'Factory Droid', glyph: 'droid', working: 0, left: 12, accounts: 1 },
  { id: 'antigravity', name: 'Google Antigravity', glyph: 'antigravity', working: 0, left: null, accounts: 1 },
];
const C2_ACCOUNTS = [
  { name: 'Personal', letter: 'P', color: '#86c391', plan: 'Pro', left: 69, working: 1, current: true },
  { name: 'Work', letter: 'W', color: '#dba06e', plan: 'Max', left: 88, working: 1 },
  { name: 'ahror@example.com', letter: 'A', color: '#86a8dc', plan: 'Pro', left: 46, working: 0, external: true },
];
const c2LeftColor = (left) => (left === 0 ? 'var(--del)' : left <= 15 ? 'var(--warn)' : 'var(--mu)');

const c2Avatar = (account, size = 18) => `<span style="width:${size}px;height:${size}px;border-radius:50%;background:${account.color};color:#1b1f26;font-size:${Math.round(size * 0.5)}px;font-weight:600;display:inline-grid;place-items:center;flex:none">${account.letter}</span>`;
const c2Chip = (lead, label, { chev = true, open = false, color = 'var(--mu)', style = '' } = {}) => `<span class="row none" style="gap:5px;height:22px;padding:0 6px;border-radius:5px;font-size:12.5px;color:${open ? 'var(--t)' : color};${open ? 'background:var(--hov);' : ''}${style}">${lead}${label ? `<span class="trunc">${label}</span>` : ''}${chev ? ic('chev-down', 'xs') : ''}</span>`;
const c2Plus = () => `<span class="ibtn">${ic('plus', 'sm')}</span>`;
const c2AgentChip = (opts = {}) => c2Chip(glyph('claude', 'sm'), 'Claude Agent', opts);
const c2AccountChip = (opts = {}) => c2Chip(c2Avatar(C2_ACCOUNTS[0], 14), 'Personal', opts);
const c2Gauge = (left = 69) => `<span class="row none" style="gap:3px;height:22px;padding:0 4px;font-size:12px;color:${c2LeftColor(left)}">${ic('gauge', 'xs')}${left}%</span>`;
const c2Ring = () => `<span style="width:14px;height:14px;border-radius:50%;border:2px solid var(--sel);border-right-color:var(--mu);border-top-color:var(--mu);flex:none;display:inline-block;margin:0 4px"></span>`;
const c2Send = ({ ready = false } = {}) => `<span class="ibtn" style="background:var(--sel);color:${ready ? 'var(--ac)' : 'var(--ph)'}">${ic('send', 'xs')}</span>`;
const c2Settings = () => c2Chip('', 'Default') + c2Chip('', 'Opus 5.5') + c2Chip('', 'High');
const c2Placeholder = '<span class="ph">Message the agent…</span>';

// Today's footer (render_message_editor): + Add Context, agent, account and usage at the left;
// the context ring, the agent's settings and Send at the right.
const c2FooterLeft = () => `<span class="row" style="gap:2px;min-width:0">${c2Plus()}${c2AgentChip()}${c2AccountChip()}${c2Gauge()}</span>`;
const c2FooterRight = ({ ready = false, send } = {}) => `<span class="row" style="gap:4px;flex:none">${c2Ring()}${c2Settings()}${send ?? c2Send({ ready })}</span>`;
const c2Footer = ({ left = c2FooterLeft(), right = c2FooterRight() } = {}) => `<div class="row" style="justify-content:space-between;gap:8px;min-width:0">${left}${right}</div>`;

// The composer as built: a bar across the bottom of the thread (ComposerStyle::Bar).
const c2Bar = ({ body = c2Placeholder, footer = c2Footer(), menu = '' } = {}) => `<div style="border-top:1px solid var(--b);background:var(--ed);padding:8px 0;flex:none;position:relative">
  <div style="max-width:850px;margin:0 auto;padding:0 8px;display:flex;flex-direction:column;gap:8px">
    <div style="padding:4px 10px 0 2px;line-height:20px;position:relative">${body}${menu}</div>${footer}</div></div>`;
// The composer as a card, as the new thread screen has it (ComposerStyle::Card).
const c2Card = ({ body = c2Placeholder, footer = c2Footer(), above = '', below = '', menu = '', radius = 10, pad = '0 16px 14px', minH = 44 } = {}) => `<div style="padding:${pad};flex:none;position:relative">
  ${above}<div style="border:1px solid var(--b);border-radius:${above ? `0 0 ${radius}px ${radius}px` : `${radius}px`};background:var(--ed);box-shadow:0 6px 18px rgba(0,0,0,.28);padding:8px;display:flex;flex-direction:column;gap:8px;position:relative">
    <div style="padding:4px 10px 0 4px;line-height:20px;min-height:${minH - 12}px;position:relative">${body}${menu}</div>${footer}</div>${below}</div>`;

// The thread above the composer: its header and the end of its conversation.
const c2Header = () => `<div class="row g1" style="height:36px;flex:none;padding:0 8px;border-bottom:1px solid var(--b);background:var(--panel)"><span class="row g15" style="padding:2px 4px;flex:none">${c2Az()}<span class="sm mu">agentZ</span></span><span class="sm mu" style="flex:none">/</span><span class="row g1 grow" style="padding:2px 4px;min-width:0;font-size:13px"><span class="trunc">Add usage indicators to the agent picker</span>${ic('chev-down', 'xs mu')}</span><span class="row g15" style="flex:none"><span class="btn sm" style="gap:4px;height:22px">${ic('branch', 'xs')}main</span><span class="ibtn" style="font-size:15px">±</span>${ibtn('terminal')}${ibtn('more')}</span></div>`;
const c2Bubble = (html) => `<div style="display:flex;justify-content:flex-end;padding:4px 0 10px"><div style="max-width:78%;background:#3a3f4b;border-radius:10px;padding:8px 12px;line-height:21px">${html}</div></div>`;
const c2Para = (html, style = '') => `<div style="line-height:22px;color:var(--t);padding:4px 0;${style}">${html}</div>`;
const c2Talk = () => c2Bubble('Show in the agent picker which agents are working, and each one’s usage.') +
  c2Para('The picker now ends each agent’s row with its usage, and a dot while one of its threads is working. I read the usage from the same place the composer’s gauge does, so they always agree.') +
  `<div class="sm ph" style="padding:2px 0 8px">Worked for 2m 14s</div>`;
const c2Thread = (bottom, { w = 780, h = 340, talk = c2Talk(), above = '', overlay = '' } = {}) => frame(`<div class="col" style="height:100%;background:var(--panel)">${c2Header()}
  <div class="grow" style="min-height:0;overflow:hidden;padding:12px 40px 0;display:flex;flex-direction:column;justify-content:flex-end">${talk}</div>${above}${bottom}</div>${overlay}`, { w, h });

// The activity bar over the composer (render_activity_bar): here the plan's current step.
const c2Activity = ({ attached = false } = {}) => `<div style="${attached ? '' : 'padding:0 8px;'}flex:none"><div class="row g2" style="height:28px;padding:0 10px;background:#31353e;border:1px solid var(--b);border-bottom:${attached ? '0' : '0'};border-radius:${attached ? '10px 10px 0 0' : '6px 6px 0 0'};font-size:12px;color:var(--mu)">${ic('chev-right', 'xs')}<span>Current:</span><span style="color:var(--t)">Add the usage column to the agent picker</span><span class="grow"></span><span>2 left</span>${ic('x', 'xs')}</div></div>`;

// A menu over the composer, and its rows.
const c2Menu = (rows, style, w = 260) => `<div class="menu" style="${style};width:${w}px;padding:4px">${rows}</div>`;
const c2Check = () => `<span style="width:14px;display:inline-flex;flex:none">${ic('check', 'sm ac')}</span>`;
const c2NoCheck = () => '<span style="width:14px;display:inline-flex;flex:none"></span>';
const c2Manage = (label) => `<div class="hr"></div><div class="it">${ic('settings', 'sm')}${label}</div>`;
// Today's agent picker (render_agent_picker): icon, name, a check on the current one.
const c2AgentRow = (agent, { hl = false, end = '', line2 = '', icon } = {}) => `<div class="it ${hl ? 'hl' : ''}" style="${line2 ? 'height:auto;padding:4px 8px;align-items:flex-start' : ''}"><span style="display:inline-flex;${line2 ? 'margin-top:3px' : ''}">${icon ?? glyph(agent.glyph, 'sm')}</span><span class="col grow" style="min-width:0"><span class="trunc">${agent.name}</span>${line2 ? `<span class="xs mu">${line2}</span>` : ''}</span>${end}${agent.id === 'claude' ? c2Check() : c2NoCheck()}</div>`;
const c2AgentMenu = (row = (agent) => c2AgentRow(agent), { style = 'bottom:62px;left:40px', w = 250, extra = '' } = {}) => c2Menu(C2_AGENTS.map((agent) => row(agent)).join('') + extra + c2Manage('Manage Agents…'), style, w);
// Today's account picker (render_account_picker): "This thread", then "Switch to".
const c2AccountRow = (account, { end, line2, hl = false, lead } = {}) => `<div class="it ${hl ? 'hl' : ''}" style="height:auto;padding:5px 8px;gap:10px">${lead ?? c2Avatar(account)}<span class="col grow" style="min-width:0;gap:1px"><span class="row g1"><span class="trunc">${account.name}</span>${account.external ? '<span class="xs ph none">· External</span>' : ''}</span><span class="xs mu">${line2 ?? account.plan}</span></span>${end ?? `<span class="xs none" style="color:${c2LeftColor(account.left)}">${account.left}% left</span>`}${account.current ? c2Check() : c2NoCheck()}</div>`;
const c2AccountMenu = (row = (account) => c2AccountRow(account), { style = 'bottom:62px;left:150px', w = 290 } = {}) => c2Menu(`<div class="lbl">This thread</div>${row(C2_ACCOUNTS[0])}<div class="hr"></div><div class="lbl">Switch to</div>${C2_ACCOUNTS.slice(1).map((account) => row(account)).join('')}${c2Manage('Manage Accounts…')}`, style, w);

// A fake screenshot of a checkout page: the image pasted into the composer.
const C2_SHOT_W = 520;
const C2_SHOT_H = 320;
const c2Shot = (marks = '', { w = C2_SHOT_W, h = C2_SHOT_H } = {}) => `<div style="width:${w}px;height:${h}px;background:#f5f6f8;border-radius:4px;overflow:hidden;position:relative;font:12px/1.4 'IBM Plex Sans',sans-serif;color:#23262d;flex:none">
  <div style="height:34px;background:#1f2430;display:flex;align-items:center;gap:14px;padding:0 14px;color:#e5e7eb;font-weight:600">storefront<span style="font-weight:400;color:#9ca3af">Shop</span><span style="font-weight:400;color:#9ca3af">Orders</span></div>
  <div style="display:flex;gap:16px;padding:16px">
    <div style="flex:1.4;display:flex;flex-direction:column;gap:9px">
      <div style="font-size:17px;font-weight:600">Checkout</div>
      <div style="height:28px;border:1px solid #d6d9df;border-radius:5px;background:#fff;padding:6px 8px;color:#9aa0aa">Email</div>
      <div style="height:28px;border:1px solid #d6d9df;border-radius:5px;background:#fff;padding:6px 8px;color:#9aa0aa">Card number</div>
      <div style="display:flex;gap:8px"><div style="flex:1;height:28px;border:1px solid #d6d9df;border-radius:5px;background:#fff;padding:6px 8px;color:#9aa0aa">MM / YY</div><div style="flex:1;height:28px;border:1px solid #d6d9df;border-radius:5px;background:#fff;padding:6px 8px;color:#9aa0aa">CVC</div></div>
      <div style="width:120px;height:32px;border-radius:6px;background:#e8833a;color:#fff;font-weight:600;display:grid;place-items:center;margin-top:4px">Pay $42.00</div>
    </div>
    <div style="flex:1;background:#fff;border:1px solid #e1e4e9;border-radius:8px;padding:12px;display:flex;flex-direction:column;gap:7px;align-self:flex-start">
      <div style="font-weight:600">Order summary</div>
      <div style="display:flex;justify-content:space-between;color:#555b66"><span>2 × Mug</span><span>$24.00</span></div>
      <div style="display:flex;justify-content:space-between;color:#555b66"><span>Shipping</span><span>$18.00</span></div>
      <div style="height:1px;background:#e1e4e9"></div>
      <div style="display:flex;justify-content:space-between;font-weight:600"><span>Total</span><span>$42.00</span></div>
    </div>
  </div>${marks}</div>`;
// Marks on the screenshot: a box around the Pay button, an arrow to it, a scribble and a note.
const C2_RED = '#ff453a';
const c2Marks = ({ rect = true, arrow = true, pen = true, text = true, highlight = false, numbers = false, color = C2_RED } = {}) => `<svg style="position:absolute;inset:0;pointer-events:none" width="${C2_SHOT_W}" height="${C2_SHOT_H}" viewBox="0 0 ${C2_SHOT_W} ${C2_SHOT_H}" fill="none" stroke="${color}" stroke-width="3" stroke-linecap="round" stroke-linejoin="round">
  ${rect ? '<rect x="10" y="208" width="134" height="44" rx="4"/>' : ''}
  ${arrow ? '<path d="M228 268 L152 238"/><path d="M152 238 l13 -1 M152 238 l8 10"/>' : ''}
  ${pen ? '<path d="M352 196 c 30 -14 90 -12 116 2 c 18 10 4 26 -30 28 c -40 3 -96 -2 -98 -18 c -1 -8 8 -12 16 -14"/>' : ''}
  ${highlight ? '<path d="M28 98 L 200 98" stroke="#ffd60a" stroke-opacity=".45" stroke-width="16"/>' : ''}
  </svg>${text ? `<span style="position:absolute;left:234px;top:258px;font:600 14px 'IBM Plex Sans',sans-serif;color:${color}">Make this full width</span>` : ''}${numbers ? `<span style="position:absolute;left:2px;top:198px;width:20px;height:20px;border-radius:50%;background:${color};color:#fff;font:600 11px sans-serif;display:grid;place-items:center">1</span><span style="position:absolute;left:340px;top:184px;width:20px;height:20px;border-radius:50%;background:${color};color:#fff;font:600 11px sans-serif;display:grid;place-items:center">2</span>` : ''}`;

// The image viewer (attachment_image.rs ImageViewer): a dark backdrop over the window, the
// image fitted in the middle, Close above its corner and the caption under it.
const c2Viewer = ({ image = c2Shot(), caption = 'Image', top = '', bottom = '', side = '', w = 780, h = 520, closeExtra = '' } = {}) => frame(`<div style="position:absolute;inset:0;background:var(--panel)"></div>
  <div style="position:absolute;inset:0;background:rgba(0,0,0,.8);display:flex;flex-direction:column;align-items:center;justify-content:center;gap:8px">
    ${top}<div class="row" style="gap:6px;align-self:center;width:${C2_SHOT_W}px;justify-content:flex-end">${closeExtra}<span class="ibtn" style="color:#e6e9ee">${ic('x', 'sm')}</span></div>
    <div class="row" style="gap:10px;align-items:flex-start">${image}${side}</div>
    <div class="sm" style="color:#c9ced8">${caption}</div>${bottom}</div>`, { w, h });
// The composer's outlined Image chip (Zed's crease).
const c2ImageChip = ({ marked = false, label = 'Image', hl = false } = {}) => `<span class="row none" style="display:inline-flex;gap:4px;height:20px;padding:0 6px;border:1px solid var(--b);border-radius:5px;font:12px ${C2_MONO};color:var(--t);vertical-align:-4px;${hl ? 'background:var(--hov);' : ''}">${ic(marked ? 'pencil' : 'image', 'xs')}${label}</span>`;

// Settings rows (settings_page.rs render_row) in a section card.
const c2Switch = (on) => `<span style="width:28px;height:16px;border-radius:8px;background:${on ? '#3b6ea8' : 'var(--sel)'};display:inline-flex;align-items:center;padding:2px;justify-content:${on ? 'flex-end' : 'flex-start'};flex:none"><i style="width:12px;height:12px;border-radius:50%;background:${on ? '#dce0e5' : 'var(--mu)'};display:block"></i></span>`;
const c2Segments = (labels, on) => `<span class="seg none">${labels.map((label, i) => `<span class="${i === on ? 'on' : ''}">${label}</span>`).join('')}</span>`;
const c2Dropdown = (label) => `<span class="btn sm none" style="gap:6px">${label}${ic('chev-down', 'xs')}</span>`;
const c2Stepper = (value) => `<span class="row none" style="border:1px solid var(--b);border-radius:6px;height:24px;font-size:12px"><span style="padding:0 8px;color:var(--mu)">−</span><span style="padding:0 8px;border-left:1px solid var(--b);border-right:1px solid var(--b)">${value}</span><span style="padding:0 8px;color:var(--mu)">+</span></span>`;
const c2Row = (title, desc, control, { hl = false } = {}) => `<div class="row" style="padding:10px 12px;gap:16px;border-top:1px solid var(--bv);${hl ? 'background:rgba(116,173,232,.07);box-shadow:inset 2px 0 0 var(--ac);' : ''}"><div class="col grow" style="gap:2px;min-width:0"><span style="font-size:13px">${title}</span><span class="xs mu">${desc}</span></div>${control}</div>`;
const c2Section = (title, rows, note = '') => `<div style="padding:4px 0 14px"><div class="sm mu" style="padding:0 2px 6px">${title}</div><div style="border:1px solid var(--b);border-radius:8px;background:var(--panel);overflow:hidden;margin-top:-1px">${rows.replace('border-top:1px solid var(--bv);', '')}</div>${note ? `<div class="xs ph" style="padding:6px 2px 0">${note}</div>` : ''}</div>`;
const c2SettingsPage = (heading, sections, { w = 620, h = 420, overlay = '' } = {}) => frame(`<div style="position:absolute;inset:0;background:var(--ed);padding:18px 26px;overflow:hidden"><div style="font-size:16px;padding-bottom:12px">${heading}</div>${sections}</div>${overlay}`, { w, h });
const c2DesignRows = ({ variantsAs = 0, extra = '' } = {}) => c2Row('Variants', 'How many variants a topic has.', c2Stepper(5)) + c2Row('Include today’s', 'Today’s version as a variant to keep.', c2Switch(true)) + c2Row('Sizes', 'The sizes each picture is shown at.', c2Segments(['Desktop', 'Phone', 'Both'], 0)) + c2Row('Variants as', 'Pictures, live pages, or both.', c2Segments(['Pictures', 'Live pages', 'Both'], variantsAs)) + extra;

// A tool call in the thread (thread-rows round B) with its answer, as the agent got it.
const c2ToolRow = (icon, html, trailing = '') => `<div class="row" style="min-height:24px;gap:6px;padding:0 2px;color:${C2_DIM};font-size:13px"><span style="width:24px;display:inline-flex;justify-content:center">${ic(icon, 'sm')}</span><span class="row grow" style="gap:4px;min-width:0">${html}</span>${trailing}</div>`;
const c2Answer = (lines) => `<div style="margin:2px 0 6px 30px;border:1px solid var(--bv);border-radius:6px;background:var(--ed);padding:8px 10px;font:12px/1.5 ${C2_MONO};color:#c8ccd4;white-space:pre-wrap">${lines}</div>`;
// The design's card in the thread (artifacts-2 round): its name, its state and Open.
const c2DesignCard = ({ name = 'Checkout page', detail = '3 topics · 5 variants each', note = '' } = {}) => `<div style="margin:4px 0 8px 30px;border:1px solid var(--b);border-radius:8px;background:var(--ed);padding:9px 12px;display:flex;flex-direction:column;gap:6px;max-width:470px">
  <div class="row g2">${ic('palette', 'sm mu')}<span class="b5">${name}</span><span class="xs mu">Design · v1</span><span class="grow"></span><span class="row none" style="gap:3px;font-size:12px;color:var(--ac)">Open${ic('arrow-up-right', 'xs')}</span></div>
  <div class="xs mu">${detail}</div>${note}</div>`;
