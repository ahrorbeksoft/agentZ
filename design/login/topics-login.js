// Logging an agent in: where a thread shows it, its methods, what it says, each method's step,
// the message that wasn't sent, and the same login in Add Account and Settings. agent_login.rs
// draws all three (LoginLayout Centered, Dialog and Rows); the mocks are JetBrains Dark.

Object.assign(ICONS, {
  key: '<path d="m15.5 7.5 2.3 2.3a1 1 0 0 0 1.4 0l2.1-2.1a1 1 0 0 0 0-1.4L19 4"/><path d="m21 2-9.6 9.6"/><circle cx="7.5" cy="15.5" r="5.5"/>',
  send: '<path d="M14.54 21.69a.5.5 0 0 0 .94-.03l6.5-19a.5.5 0 0 0-.64-.64l-19 6.5a.5.5 0 0 0-.03.94l7.93 3.18a2 2 0 0 1 1.11 1.11z"/><path d="m21.85 2.15-10.94 10.94"/>',
  'arrow-up-right': '<path d="M7 7h10v10"/><path d="M7 17 17 7"/>',
  'x-circle': '<circle cx="12" cy="12" r="10"/><path d="m15 9-6 6"/><path d="m9 9 6 6"/>',
  info: '<circle cx="12" cy="12" r="10"/><path d="M12 16v-4"/><path d="M12 8h.01"/>',
  rotate: '<path d="M21 12a9 9 0 1 1-9-9c2.52 0 4.93 1 6.74 2.74L21 8"/><path d="M21 3v5h-5"/>',
  'eye-off': '<path d="M10.73 5.08A10.43 10.43 0 0 1 12 5c7 0 10 7 10 7a13.16 13.16 0 0 1-1.67 2.68"/><path d="M6.61 6.61A13.53 13.53 0 0 0 2 12s3 7 10 7a9.74 9.74 0 0 0 5.39-1.61"/><line x1="2" x2="22" y1="2" y2="22"/>',
});
GLYPHS.droid = '❋';

const JBT = '--ed:#1e1f22;--panel:#26282b;--title:#26282b;--t:#dfe1e5;--mu:#b0b1b3;--ph:#6f737a;--b:#393b41;--bv:#393b41;--bf:#3574f0;--hov:#3c3e41;--sel:#43454a;--ac:#548af7;--ok:#57965d;--del:#fa6675;--warn:#e0b45c;';
const MONO = "'IBM Plex Mono','SF Mono',Menlo,monospace";
const LW = 780;

// The agents as they describe their logins.
const DROID_NAME = 'Factory Droid';
const CLAUDE_NAME = 'Claude Agent';
const SAYS = 'Click the “Login” button to authenticate, or set a FACTORY_API_KEY environment variable.';
const DROID = [
  { icon: 'globe', name: 'Login', desc: 'Authenticate with Factory using a device pairing code in your browser.' },
  { icon: 'key', name: 'Factory API Key', desc: 'Authenticate using a Factory API key set in the FACTORY_API_KEY environment variable.' },
];
const CLAUDE = [
  { icon: 'terminal', name: 'Claude Subscription', desc: 'Use Claude subscription' },
  { icon: 'terminal', name: 'Anthropic Console', desc: 'Use Anthropic Console (API usage billing)' },
];
const CODE = 'KQWX-7RTP';
const HOST = 'app.factory.ai';
const MESSAGE = 'Add the checkout page with a pay button';
const ME = 'alex@hey.com';

// Buttons in JetBrains Dark: the filled one is the accent with white text.
const pbtn = (text, style = '') => `<span class="btn primary" style="color:#fff;${style}">${text}</span>`;
const obtn = (text, style = '') => `<span class="btn" style="background:none;${style}">${text}</span>`;
const gbtn = (text, style = '') => `<span class="btn" style="border-color:transparent;background:none;${style}">${text}</span>`;
const tbtn = (text) => `<span class="btn sm" style="background:rgba(84,138,247,.16);border-color:rgba(84,138,247,.35);color:#8db3fa">${text}</span>`;
const linkBtn = (text) => `<span class="sm ac" style="flex:none">${text}</span>`;
const spinner = (size = 12) => `<span class="spin" style="width:${size}px;height:${size}px;border-color:rgba(84,138,247,.3);border-top-color:var(--ac)"></span>`;
const caption = (text) => `<div class="xs ph" style="margin:0 0 6px">${text}</div>`;
const tile = (size = 40, kind = 'droid') => `<span style="width:${size}px;height:${size}px;flex:none;border-radius:${Math.round(size / 5)}px;border:1px solid var(--b);background:var(--ed);display:inline-grid;place-items:center;font-size:${Math.round(size * 0.45)}px;color:var(--t)">${GLYPHS[kind]}</span>`;
const iconTile = (icon, size = 24) => `<span class="mu" style="width:${size}px;height:${size}px;flex:none;border-radius:6px;border:1px solid var(--bv);display:inline-grid;place-items:center">${ic(icon, size > 24 ? 'sm' : 'xs')}</span>`;

// The thread around a login: its header, the conversation and the composer, which can't send.
const azBadge = '<span class="mono" style="background:#4b3034;color:#e06c75;width:14px;height:14px;font-size:7px">AZ</span>';
const header = (title) => `<div class="row g1" style="height:36px;flex:none;padding:0 8px;border-bottom:1px solid var(--b);background:var(--panel)"><span class="row g15" style="padding:2px 4px;flex:none">${azBadge}<span class="sm mu">agentZ</span></span><span class="sm mu">/</span><span class="row g1 grow" style="padding:2px 4px;font-size:13px;min-width:0"><span class="trunc">${title}</span>${ic('chev-down', 'xs mu')}</span><span class="row g15" style="flex:none"><span class="btn sm" style="gap:4px;height:22px;background:none">${ic('branch', 'xs')}main</span><span class="ibtn" style="font-size:15px">±</span>${ibtn('terminal')}${ibtn('more')}</span></div>`;
const composer = ({ agent = DROID_NAME, kind = 'droid', placeholder = `Log in to ${agent} to send a message`, canSend = false } = {}) => `<div style="border-top:1px solid var(--b);background:var(--ed);padding:8px 16px;flex:none"><div class="ph" style="padding:4px 4px 12px">${placeholder}</div><div class="row" style="gap:10px;font-size:12px;color:var(--mu)">${ic('plus', 'sm')}<span class="row g1">${glyph(kind, 'sm')}${agent}</span><span class="grow"></span><span class="ibtn" style="background:var(--sel);color:${canSend ? 'var(--ac)' : 'var(--ph)'}">${ic('send', 'xs')}</span></div></div>`;
const bubble = (text = MESSAGE, below = '') => `<div class="col" style="align-items:flex-end;padding:14px 0 6px;gap:4px"><div style="max-width:78%;background:#2f3134;border-radius:12px;padding:8px 12px;line-height:21px">${text}</div>${below}</div>`;
const thread = (convo, { h = 480, bottom = '', empty = false, overlay = '', agent, kind, placeholder, canSend } = {}) => frame(`<div class="col" style="height:100%;background:var(--panel)">${header(empty ? 'New thread' : MESSAGE)}<div class="grow col" style="min-height:0;overflow:hidden;position:relative;padding:0 28px">${convo}</div>${bottom}${composer({ agent, kind, placeholder, canSend })}</div>${overlay}`, { w: LW, h, style: JBT });
const middle = (html) => `<div class="col grow" style="align-items:center;justify-content:center">${html}</div>`;
const pair = (a, b, labels = ['A new thread', 'After your first message']) => `<div class="col" style="gap:12px">${caption(labels[0])}${a}${caption(labels[1])}${b}</div>`;

// The methods, in each look.
const methodRows = (methods, { hl = 0 } = {}) => `<div class="col" style="gap:2px">${methods.map((m, i) => `<div class="row g3" style="padding:9px 10px;border-radius:6px;${i === hl ? 'background:var(--hov)' : ''}">${iconTile(m.icon)}<div class="col grow" style="gap:1px;min-width:0"><span style="font-size:13px">${m.name}</span><span class="xs mu" style="line-height:1.4">${m.desc}</span></div>${ic('chev-right', 'xs mu')}</div>`).join('')}</div>`;
const stackedButtons = (methods, w = 300) => `<div class="col" style="gap:8px;width:${w}px">${methods.map((m, i) => (i === 0 ? pbtn : obtn)(`${ic(m.icon, 'sm')}${m.name}`, 'height:32px;justify-content:center;width:100%')).join('')}</div>`;
const mainButton = (methods, { open = false } = {}) => `<div class="col" style="gap:10px;width:300px;align-items:stretch">${pbtn(`${ic(methods[0].icon, 'sm')}${methods[0].name}`, 'height:32px;justify-content:center')}<span class="xs mu" style="text-align:center;line-height:1.4">${methods[0].desc}</span>${open ? `<div style="border-top:1px solid var(--bv);padding-top:6px">${methodRows(methods.slice(1), { hl: -1 })}</div>` : `<span class="row g1 sm mu" style="justify-content:center">Other ways to log in${ic('chev-down', 'xs')}</span>`}</div>`;
const smallButtons = (methods) => `<div class="row" style="gap:4px;justify-content:flex-end;flex-wrap:wrap">${methods.map((m, i) => (i === 0 ? tbtn(m.name) : `<span class="btn sm" style="background:none">${m.name}</span>`)).join('')}</div>`;
const checkAgain = `<div class="row g1 sm"><span class="mu">Logged in another way?</span>${linkBtn('Check Again')}</div>`;

// The login's head: the agent's icon, what to do, and the line under it.
const head = ({ agent = DROID_NAME, kind = 'droid', line = `Every thread with ${agent} shares the login.`, title = `Log in to ${agent}`, size = 40, align = 'center' } = {}) => `<div class="col" style="align-items:${align === 'center' ? 'center' : 'flex-start'};gap:10px;text-align:${align}">${tile(size, kind)}<div class="col" style="gap:3px;align-items:${align === 'center' ? 'center' : 'flex-start'}"><span style="font-size:16px">${title}</span>${line ? `<span class="sm mu">${line}</span>` : ''}</div></div>`;
const headRow = ({ agent = DROID_NAME, kind = 'droid', title = `Log in to ${agent}`, line = '' } = {}) => `<div class="row g3">${tile(32, kind)}<div class="col grow" style="gap:2px;min-width:0"><span style="font-size:15px">${title}</span>${line ? `<span class="sm mu" style="line-height:1.4">${line}</span>` : ''}</div></div>`;
const says = (text = SAYS, style = '') => `<div class="sm" style="color:#c4c6ca;line-height:1.5;max-width:400px;${style}">${text}</div>`;

// A card in the conversation, where the reply goes.
const loginCard = (body, { w = 440, style = '' } = {}) => `<div style="width:${w}px;border:1px solid var(--b);border-radius:10px;background:#2b2d30;padding:16px;display:flex;flex-direction:column;gap:14px;${style}">${body}</div>`;
const cardA = ({ agent = DROID_NAME, kind = 'droid', methods = DROID, title, line } = {}) => loginCard(`${headRow({ agent, kind, title, line: line ?? `Every thread with ${agent} shares the login.` })}${methodRows(methods, { hl: -1 })}`);

// Today's panel (render_centered): head, the agent's message, a button per method.
const todayPanel = ({ agent = DROID_NAME, kind = 'droid', methods = DROID, message = SAYS } = {}) => `<div class="col" style="align-items:center;gap:12px;padding:24px 0">${head({ agent, kind })}${message ? says(message, 'text-align:left') : ''}<div style="margin-top:4px">${stackedButtons(methods)}</div>${methods.some((m) => m.icon === 'terminal') ? checkAgain : ''}</div>`;

// Zed's callout over the composer (render_auth_required_state).
const callout = (body, { icon = 'info', tone = 'var(--mu)' } = {}) => `<div style="padding:0 16px 8px;flex:none"><div class="row" style="align-items:flex-start;gap:10px;border:1px solid var(--b);border-radius:6px;background:#2b2d30;padding:10px 12px"><span style="color:${tone};display:inline-flex;margin-top:2px">${ic(icon, 'sm')}</span><div class="col grow" style="gap:6px;min-width:0">${body}</div></div></div>`;
const zedCallout = ({ agent = DROID_NAME, methods = DROID, message = SAYS } = {}) => callout(`<span class="b5" style="font-size:13px">Log in to ${agent}</span>${message ? `<span class="sm mu" style="line-height:1.45">${message}</span>` : ''}${smallButtons(methods)}`);

// Add Account's dialog, and a dialog over the thread.
const dialog = (title, body, buttons, { w = 420, style = '' } = {}) => `<div class="pop" style="position:relative;width:${w}px;padding:16px 16px 14px;${style}"><div class="b6" style="font-size:15px;margin-bottom:6px">${title}</div>${body}<div class="row g2" style="justify-content:flex-end;margin-top:14px">${buttons}</div></div>`;
const overDialog = (html) => `<div class="modal-back"></div><div style="position:absolute;inset:0;z-index:21;display:grid;place-items:center">${html}</div>`;

// 1. Where it shows -----------------------------------------------------------------------------
TOPICS.push({
  id: 'place', section: 'In a thread', title: 'Where the login shows', size: 'wide', rec: 'A',
  now: 'A panel takes the middle of an empty thread. After a message, it follows the conversation: your message at the top, the panel under it, and “The message wasn’t sent” with Retry above the composer. The composer says “Log in to Factory Droid to send a message” and can’t send. Topic 5 decides the unsent message; these mocks leave it out, and use topic 2’s rows for the methods where today’s buttons don’t apply.',
  nowImg: 'img/now-login-after-message.png',
  issues: ['A big empty panel floats under your message, far from the composer', 'Nothing frames it: it reads as part of the page, not as the agent’s answer'],
  options: [
    { key: 'A', name: 'A card where the reply would be', from: 'new, in the look of the Add Account dialog',
      desc: 'The login is a card in the conversation, 440 px wide: the agent’s icon and “Log in to Factory Droid” on one line, then the methods. In an empty thread it sits in the middle, as today. After a message, it follows your message on the agent’s side, where its reply would be. Once logged in, the card goes.',
      good: 'It reads as the agent’s answer to your message, and it’s clearly one thing.', cost: 'Still in the conversation, so a long thread scrolls to it.',
      mock: () => pair(thread(middle(cardA()), { h: 400, empty: true }), thread(`${bubble()}${cardA()}`, { h: 400 })) },
    { key: 'B', name: 'A callout over the composer', from: 'Zed (render_auth_required_state)',
      desc: 'Zed’s: the conversation stays as it is, and a callout over the composer says “Log in to Factory Droid”, what the agent said, and a small button per method at its right, the first one tinted. A method’s step shows in the callout. Zed opens a terminal login in a terminal tab; here it would open in the thread’s terminal drawer.',
      good: 'Matches Zed; it sits where you were about to type.', cost: 'Small; a pairing code or an API key makes the callout tall, and an empty thread is all blank above it.',
      mock: () => pair(thread('', { h: 400, empty: true, bottom: zedCallout() }), thread(bubble(), { h: 400, bottom: zedCallout() })) },
    { key: 'C', name: 'The Add Account dialog, opened from the thread', from: 't3code (its banner sends you to setup), with the Add Account dialog',
      desc: 'The thread shows only a line over the composer: “Factory Droid needs a login” with Log In…. That opens the Add Account dialog, titled “Log in to Factory Droid”: the methods, the picked one’s step, then “Logged in as alex@hey.com” with Done. It doesn’t open by itself.',
      good: 'One login view in the whole app; the thread stays clean.', cost: 'A click more, and a dialog over the thread.',
      mock: () => pair(thread(bubble(), { h: 400, bottom: callout(`<div class="row g2"><span class="grow" style="font-size:13px">${DROID_NAME} needs a login</span>${pbtn('Log In…')}</div>`) }), thread(bubble(), { h: 400, bottom: callout(`<div class="row g2"><span class="grow" style="font-size:13px">${DROID_NAME} needs a login</span>${pbtn('Log In…')}</div>`), overlay: overDialog(dialog(`Log in to ${DROID_NAME}`, `<div class="sm mu" style="margin-bottom:10px">Every thread with ${DROID_NAME} shares the login. Choose how to log in.</div>${methodRows(DROID)}`, gbtn('Cancel'))) }), ['Closed', 'After Log In…']) },
    { key: 'D', name: 'As it is', from: 'today',
      desc: 'Today’s panel, in the middle of an empty thread and under your message after one, with no frame.',
      good: 'Nothing to change.', cost: 'The issues stay.',
      mock: () => pair(thread(middle(todayPanel()), { h: 400, empty: true }), thread(`${bubble()}${todayPanel()}`, { h: 460 })) },
  ],
});

// 2. The methods ----------------------------------------------------------------------------------
const methodsMock = (render) => frame(`<div class="row" style="gap:24px;padding:20px;align-items:flex-start;background:var(--panel);height:100%">${[[DROID_NAME, 'droid', DROID], [CLAUDE_NAME, 'claude', CLAUDE]].map(([agent, kind, methods]) => `<div class="col">${caption(agent)}${render(agent, kind, methods)}</div>`).join('')}</div>`, { w: 1000, h: 260, style: JBT });
const inCard = (agent, kind, body) => loginCard(`${headRow({ agent, kind, line: `Every thread with ${agent} shares the login.` })}${body}`);
TOPICS.push({
  id: 'methods', section: 'In a thread', title: 'The login methods', size: 'wide', rec: 'A',
  now: 'A full-width button per method, 300 px wide and 32 tall, with its icon and name: the agent’s first method filled, the rest outlined. Its description is a tooltip. The Add Account dialog lists the same methods as rows instead: an icon in a tile, the name, the description under it and a chevron. These mocks put the methods in topic 1 A’s card.',
  nowImg: 'img/now-login.png',
  issues: ['A stack of big buttons, the first loudly filled though it’s just the agent’s first', 'What a method does is hidden in a tooltip (“Use Anthropic Console (API usage billing)”)', 'The dialog shows the same methods another way'],
  options: [
    { key: 'A', name: 'Rows, as in the Add Account dialog', from: 'the Add Account dialog (Usage round, topic 6 A)',
      desc: 'Each method is a row: its icon in a small tile, its name, the agent’s description under it, and a chevron. A row lights up under the mouse; a click starts it. None is filled.',
      good: 'You read what each method does before picking; the same as the dialog.', cost: 'No method is put forward.',
      mock: () => methodsMock((agent, kind, methods) => inCard(agent, kind, methodRows(methods, { hl: -1 }))) },
    { key: 'B', name: 'One main button, the rest folded', from: 'new',
      desc: 'The agent’s first method is the one filled button, with its description under it. The others are under “Other ways to log in”, which opens them as A’s rows.',
      good: 'One obvious action for most people.', cost: 'The agent’s first method isn’t always yours: Claude Agent lists the subscription first, Droid its pairing code.',
      mock: () => methodsMock((agent, kind, methods) => inCard(agent, kind, `<div class="col" style="align-items:center">${mainButton(methods)}</div>`)) },
    { key: 'C', name: 'Stacked buttons', from: 'today (Zed’s buttons, stacked)',
      desc: 'Today’s full-width buttons, the first filled, the description as a tooltip.',
      good: 'Big targets; no change.', cost: 'The issues stay.',
      mock: () => methodsMock((agent, kind, methods) => inCard(agent, kind, `<div class="col" style="align-items:center">${stackedButtons(methods)}</div>`)) },
    { key: 'D', name: 'Small buttons on one line', from: 'Zed',
      desc: 'Zed’s: a small button per method on one line at the right, the first tinted, the rest outlined, the description as a tooltip.',
      good: 'Compact; fits a callout (topic 1 B).', cost: 'Easy to miss, and the descriptions are hidden.',
      mock: () => methodsMock((agent, kind, methods) => inCard(agent, kind, smallButtons(methods))) },
  ],
});

// 3. What it says ----------------------------------------------------------------------------------
const wordsMock = (body) => frame(`<div style="padding:20px;background:var(--panel);height:100%">${loginCard(body, { w: 460 })}</div>`, { w: 500, h: 330, style: JBT });
const quote = (text = SAYS) => `<div class="col" style="gap:3px;border-left:2px solid var(--b);padding:2px 0 2px 10px"><span class="xs ph">${DROID_NAME} says</span>${says(text)}</div>`;
TOPICS.push({
  id: 'words', section: 'In a thread', title: 'What it says', size: 'medium', rec: 'A',
  now: 'Under the icon: “Log in to Factory Droid”, then “Every thread with Factory Droid shares the login.” in muted text. Under that, what the agent said when it asked for the login, as markdown, as Zed shows it; Droid says “Click the “Login” button to authenticate, or set a FACTORY_API_KEY environment variable.” Its lines are left-aligned under a centered head. With several accounts, it doesn’t say which account logs in. The mocks use topic 1 A’s card and topic 2 A’s rows.',
  nowImg: 'img/now-login.png',
  issues: ['The agent’s message floats under the head, aligned differently, and reads like agentZ talking', '“Every thread shares the login” isn’t true with several accounts: each account has its own'],
  options: [
    { key: 'A', name: 'The account, and the agent’s words quoted', from: 'new',
      desc: 'Under the title: the account the thread uses (“Work · work@acme.dev”) when the agent has more than one, else “Every thread with Factory Droid shares the login.” The agent’s message follows in a quote, under “Factory Droid says”.',
      good: 'You know which account you’re logging in, and whose words those are.', cost: 'A little longer.',
      mock: () => wordsMock(`${headRow({ title: `Log in to ${DROID_NAME}`, line: 'Work · work@acme.dev' })}${quote()}${methodRows(DROID, { hl: -1 })}`) },
    { key: 'B', name: 'The agent’s words in place of agentZ’s line', from: 'Zed',
      desc: 'Zed’s: under the title, the agent’s message as the description. Only when the agent says nothing, agentZ’s line (“Every thread with Factory Droid shares the login.”, or the account with several).',
      good: 'Short; the agent explains its own login.', cost: 'Some agents say things that don’t fit agentZ (“set a FACTORY_API_KEY environment variable”).',
      mock: () => wordsMock(`${headRow({ title: `Log in to ${DROID_NAME}`, line: SAYS })}${methodRows(DROID, { hl: -1 })}`) },
    { key: 'C', name: 'The title only', from: 'new',
      desc: 'Just “Log in to Factory Droid” and, with several accounts, the account. The agent’s message is left out: each method’s description says what it does.',
      good: 'The calmest.', cost: 'An agent’s hint (a command to run, a page to visit) is lost.',
      mock: () => wordsMock(`${headRow({ title: `Log in to ${DROID_NAME}`, line: 'Work · work@acme.dev' })}${methodRows(DROID, { hl: -1 })}`) },
    { key: 'D', name: 'As it is', from: 'today',
      desc: 'The title, “Every thread with Factory Droid shares the login.”, then the agent’s message as markdown.',
      good: 'No change.', cost: 'The issues stay.',
      mock: () => wordsMock(`${headRow({ title: `Log in to ${DROID_NAME}`, line: `Every thread with ${DROID_NAME} shares the login.` })}${says()}${methodRows(DROID, { hl: -1 })}`) },
  ],
});

// 4. Each method's step --------------------------------------------------------------------------------
const codeBoxes = (code) => `<span class="row" style="gap:5px">${[...code].map((ch) => (ch === '-' ? '<span class="mu">–</span>' : `<span style="width:26px;height:32px;border:1px solid var(--b);border-radius:5px;background:var(--ed);display:inline-grid;place-items:center;font:15px ${MONO}">${ch}</span>`)).join('')}</span>`;
const loginTerm = (h = 150) => `<div style="height:${h}px;border:1px solid var(--b);border-radius:6px;background:var(--ed);padding:6px 8px;font:12px/17px ${MONO};color:#c9ccd1;white-space:pre;overflow:hidden">Opening browser to sign in…\nIf the browser didn’t open, visit:\n<span style="color:#8db3fa">https://claude.ai/oauth/authorize?code=tr…</span>\n\nPaste code here if prompted &gt; <span class="cursor" style="background:var(--ac)"></span></div>`;
const keyField = `<div class="col" style="gap:6px"><span class="xs mu">API key</span><div class="field focus" style="background:var(--ed);border-color:var(--bf);box-shadow:0 0 0 1px var(--bf)"><span class="ph grow">fk-…</span>${ic('eye', 'xs mu')}</div><span class="xs mu">${DROID_NAME} keeps the key; agentZ doesn’t store it.</span></div>`;
const methodHeading = (m) => `<div class="row g2">${iconTile(m.icon)}<span style="font-size:13px">${m.name}</span></div>`;
const footer = (buttons) => `<div class="row g2" style="justify-content:flex-end">${buttons}</div>`;
const stepsMock = (panels, h = 440) => frame(`<div class="row" style="gap:20px;padding:20px;align-items:flex-start;background:var(--panel);height:100%">${panels.map(([label, html]) => `<div class="col">${caption(label)}${html}</div>`).join('')}</div>`, { w: 1340, h, style: JBT });
const LABELS = ['Droid’s Login (a pairing code)', 'Claude Agent’s Claude Subscription (a terminal)', 'Droid’s Factory API Key'];
const stepCard = (agent, kind, body) => loginCard(`${headRow({ agent, kind, line: '' })}${body}`, { w: 400 });
const codeStep = `<div class="col" style="align-items:center;gap:12px;padding:4px 0"><span class="sm"><span class="mu">Enter this code at</span> ${HOST}</span>${codeBoxes(CODE)}<span class="row g2">${obtn(`${ic('copy', 'xs')}Copy Code`)}${pbtn(`Open ${HOST}${ic('arrow-up-right', 'xs')}`)}</span><span class="row g15 sm mu">${spinner(11)}Waiting for you to finish</span></div>`;
TOPICS.push({
  id: 'steps', section: 'In a thread', title: 'Each method’s step', size: 'wide', rec: 'A',
  now: 'Picking a method shows its step in the panel. A pairing code replaces the head and buttons: “Enter this code at app.factory.ai”, the code in boxes, Copy Code and Open app.factory.ai, then “Waiting for you to finish · Cancel”. A terminal login opens a 240 px terminal under the buttons, which stay, with “Finish in the terminal. Claude Agent starts again logged in once it’s done.” An API key opens a card under the head with the field, “Factory Droid keeps the key, agentZ doesn’t store it.”, Cancel and Log In. The Add Account dialog shows the same steps in its body, with Back and Cancel at its foot. Pairing code: <a href="img/now-login-browser.png">now-login-browser.png</a>; terminal: <a href="img/now-login-terminal.png">now-login-terminal.png</a>; API key: <a href="img/now-login-key.png">now-login-key.png</a>; the dialog: <a href="img/now-add-account-browser.png">now-add-account-browser.png</a>.',
  nowImg: 'img/now-login-terminal.png',
  issues: ['Each step is laid out its own way: the head goes for a code, stays for a key, and the buttons stay over a terminal', 'Cancel is a small link for a code and a button for a key', 'No Back to pick another method, except in the dialog'],
  options: [
    { key: 'A', name: 'Each step in the card, with Back', from: 'the Add Account dialog',
      desc: 'The step replaces the methods in the same card, under the same head, as the dialog does: the code with Copy Code and Open; the method’s name over its terminal at the card’s width; the key field. Each ends in Back (to the methods) and Cancel, or Log In for a key.',
      good: 'Every step looks alike, and you can go back.', cost: 'A terminal in a 400 px card is narrow.',
      mock: () => stepsMock([
        [LABELS[0], stepCard(DROID_NAME, 'droid', `${codeStep}${footer(`${gbtn('Back')}${obtn('Cancel')}`)}`)],
        [LABELS[1], stepCard(CLAUDE_NAME, 'claude', `${methodHeading(CLAUDE[0])}${loginTerm()}<span class="xs mu">Finish in the terminal. ${CLAUDE_NAME} starts again logged in once it’s done.</span>${footer(`${gbtn('Back')}${obtn('Cancel')}`)}`)],
        [LABELS[2], stepCard(DROID_NAME, 'droid', `${methodHeading(DROID[1])}${keyField}${footer(`${gbtn('Back')}${pbtn('Log In', 'opacity:.55')}`)}`)],
      ]) },
    { key: 'B', name: 'As it is', from: 'today',
      desc: 'Today’s steps, each laid out its own way.',
      good: 'No change.', cost: 'The issues stay.',
      mock: () => stepsMock([
        [LABELS[0], `<div style="width:400px;padding-top:20px" class="col">${`<div class="col" style="align-items:center;gap:12px">${tile(40)}<span class="sm"><span class="mu">Enter this code at</span> ${HOST}</span>${codeBoxes(CODE)}<span class="row g2">${obtn(`${ic('copy', 'xs')}Copy Code`)}${pbtn(`Open ${HOST}${ic('arrow-up-right', 'xs')}`)}</span><span class="row g15 sm mu">${ic('rotate', 'xs')}Waiting for you to finish · ${linkBtn('Cancel')}</span></div>`}</div>`],
        [LABELS[1], `<div class="col" style="width:440px;align-items:center;gap:10px">${head({ agent: CLAUDE_NAME, kind: 'claude' })}${stackedButtons(CLAUDE)}<div style="width:100%">${loginTerm(130)}</div><span class="xs mu" style="align-self:flex-start">Finish in the terminal. ${CLAUDE_NAME} starts again logged in once it’s done.</span></div>`],
        [LABELS[2], `<div class="col" style="width:400px;align-items:center;gap:10px">${head()}${says()}<div style="width:100%;border:1px solid var(--b);border-radius:8px;background:#2b2d30;padding:12px;display:flex;flex-direction:column;gap:10px">${methodHeading(DROID[1])}${keyField}${footer(`${gbtn('Cancel')}${pbtn('Log In', 'opacity:.55')}`)}</div></div>`],
      ]) },
    { key: 'C', name: 'One line, the terminal in the drawer', from: 'Zed',
      desc: 'Zed’s: while it logs in, the card is one line, “Logging in to Factory Droid…” with a spinner and Cancel; a pairing code shows on that line with Copy and Open. A terminal login opens as a tab in the thread’s terminal drawer, as Zed opens it in a terminal tab. An API key is a field on the line.',
      good: 'Small; a terminal gets the drawer’s full width.', cost: 'The terminal is away from the login, and the drawer covers part of the thread.',
      mock: () => stepsMock([
        [LABELS[0], loginCard(`<div class="row g2">${spinner()}<span class="grow" style="font-size:13px">Logging in to ${DROID_NAME}…</span>${linkBtn('Cancel')}</div><div class="row g2 sm"><span class="mu">Enter</span><span style="font:13px ${MONO}">${CODE}</span><span class="mu">at ${HOST}</span><span class="grow"></span>${obtn(`${ic('copy', 'xs')}Copy`, 'height:22px;font-size:12px')}${pbtn(`Open${ic('arrow-up-right', 'xs')}`, 'height:22px;font-size:12px')}</div>`, { w: 400 })],
        [LABELS[1], `<div class="col" style="gap:10px;width:440px">${loginCard(`<div class="row g2">${spinner()}<span class="grow" style="font-size:13px">Logging in to ${CLAUDE_NAME} in the terminal below</span>${linkBtn('Cancel')}</div>`, { w: 440 })}<div style="border:1px solid var(--b);border-radius:6px;overflow:hidden"><div class="row" style="height:30px;background:var(--panel);border-bottom:1px solid var(--b);font-size:12px"><span class="row g1" style="height:100%;padding:0 10px;background:var(--ed)">${ic('terminal', 'xs')}Claude Login</span><span class="row g1 mu" style="padding:0 10px">zsh</span><span class="grow"></span>${ibtn('plus')}${ibtn('maximize')}</div>${loginTerm(140).replace('border:1px solid var(--b);border-radius:6px;', '')}</div></div>`],
        [LABELS[2], loginCard(`<div class="row g2"><span class="mu" style="font-size:13px">${DROID[1].name}</span><div class="field grow" style="height:28px;background:var(--ed)"><span class="ph grow">fk-…</span></div>${pbtn('Log In', 'opacity:.55')}${linkBtn('Cancel')}</div>`, { w: 420 })],
      ], 380) },
  ],
});

// 5. The message that wasn't sent ----------------------------------------------------------------------
const notSent = `<span class="row g1 xs" style="color:var(--del)">${ic('alert', 'xs')}Not sent<span class="mu">·</span>${linkBtn('Retry')}</span>`;
const failedCallout = () => callout(`<div class="row g2" style="align-items:flex-start"><div class="col grow" style="gap:3px"><span class="b5" style="font-size:13px">The message wasn’t sent</span><span class="sm mu">${DROID_NAME} asked for a login before taking it. Once you’ve logged in, retry.</span></div>${obtn('Retry', 'height:22px;font-size:12px')}</div>`, { icon: 'x-circle', tone: 'var(--del)' });
const loggedInCard = loginCard(`<div class="row g2"><span class="okc" style="display:inline-flex">${ic('check-circle', 'sm')}</span><div class="col grow" style="gap:2px"><span style="font-size:13px">Logged in to ${DROID_NAME} as ${ME}</span><span class="sm mu">Your message wasn’t sent.</span></div>${pbtn('Retry')}</div>`, { w: 440 });
TOPICS.push({
  id: 'unsent', section: 'In a thread', title: 'The message that wasn’t sent', size: 'wide', rec: 'A',
  now: 'Built from your report: a message the agent asks a login for fails. Your message stays at the top, the login shows under it, and over the composer a red callout says “The message wasn’t sent — Factory Droid asked for a login before taking it. Once you’ve logged in, retry.” with Retry, which works once the agent is ready. After the login, the callout stays until Retry or a new message. A message that fails for another reason (a lost connection) has Retry in its error callout. The mocks use topic 1 A’s card.',
  nowImg: 'img/now-login-after-message.png',
  issues: ['Three separate pieces: the message at the top, the login in the middle and the failure at the bottom', 'Retry shows before you can use it'],
  options: [
    { key: 'A', name: 'In the login card', from: 'new',
      desc: 'The card’s title says “Log in to send your message” and there’s no separate callout. Once logged in, the card shrinks to one line, “Logged in to Factory Droid as alex@hey.com. Your message wasn’t sent.”, with Retry filled. A new message instead removes it.',
      good: 'One place, and Retry shows only once it can work.', cost: 'Only for logins; a network error keeps its callout.',
      mock: () => pair(thread(`${bubble()}${cardA({ title: 'Log in to send your message', line: `${DROID_NAME} needs a login before it takes messages.` })}`, { h: 400 }), thread(`${bubble()}${loggedInCard}`, { h: 300, placeholder: 'Message the agent…', canSend: true }), ['Before logging in', 'After']) },
    { key: 'B', name: 'Under your message', from: 'chat apps (Messages’ “Not Delivered”)',
      desc: 'Your message gets a red “Not sent · Retry” under its bubble, as a chat app marks a message that didn’t go. The login card follows. After the login only the mark stays. A message that fails for another reason gets the same mark, in place of its callout’s Retry.',
      good: 'The failure is on the message it’s about, for every kind of failure.', cost: 'Small; it’s easy to miss under a long message.',
      mock: () => pair(thread(`${bubble(MESSAGE, notSent)}${cardA()}`, { h: 400 }), thread(bubble(MESSAGE, notSent), { h: 300, placeholder: 'Message the agent…', canSend: true }), ['Before logging in', 'After']) },
    { key: 'C', name: 'As it is', from: 'today (Zed’s error callouts)',
      desc: 'The callout over the composer, as built.',
      good: 'No change; the same place as other errors.', cost: 'The issues stay.',
      mock: () => pair(thread(`${bubble()}${cardA()}`, { h: 400, bottom: failedCallout() }), thread(bubble(), { h: 300, bottom: failedCallout(), placeholder: 'Message the agent…', canSend: true }), ['Before logging in', 'After']) },
  ],
});

// 6. The same login everywhere ---------------------------------------------------------------------
const settingsCard = (rows) => `<div style="width:420px;border:1px solid var(--b);border-radius:8px;background:#2b2d30"><div class="row g3" style="padding:12px 16px"><span style="width:28px;height:28px;border-radius:50%;flex:none;display:inline-grid;place-items:center;font-size:12px;font-weight:600;color:#1b1f26;background:linear-gradient(135deg,hsl(212 70% 82%),hsl(212 42% 60%))">A</span><div class="col grow" style="gap:2px"><span style="font-size:13px">${ME}</span><span class="row g1 xs" style="color:var(--warn)"><span class="dot" style="background:var(--warn)"></span>Not logged in</span></div>${ibtn('more')}</div>${rows}</div>`;
const settingsRowsToday = DROID.map((m, i) => `<div class="row g3" style="padding:12px 16px;border-top:1px solid var(--bv)">${iconTile(m.icon, 28)}<div class="col grow" style="gap:2px;min-width:0"><span style="font-size:13px">${m.name}</span><span class="xs mu" style="line-height:1.4">${m.desc}</span></div>${i === 0 ? pbtn('Log In') : obtn('Log In')}</div>`).join('');
const settingsRowsA = `<div style="padding:4px 6px 6px;border-top:1px solid var(--bv)">${methodRows(DROID, { hl: -1 })}</div>`;
const placesMock = (threadPiece, dialogPiece, settingsPiece) => frame(`<div class="row" style="gap:20px;padding:20px;align-items:flex-start;background:var(--panel);height:100%">${[['In a thread', threadPiece], ['Add Account’s dialog', dialogPiece], ['A logged-out account on the Account tab', settingsPiece]].map(([label, html]) => `<div class="col">${caption(label)}${html}</div>`).join('')}</div>`, { w: 1380, h: 340, style: JBT });
const dialogA = dialog(`Add a ${DROID_NAME} account`, `<div class="sm mu" style="margin-bottom:10px">Each account has its own login, sessions and history. Choose how to log in.</div>${methodRows(DROID, { hl: -1 })}`, gbtn('Cancel'));
TOPICS.push({
  id: 'everywhere', section: 'Everywhere', title: 'The same login in Add Account and Settings', size: 'wide', rec: 'A',
  now: 'The login shows in three places, from one piece of code laid out three ways. In a thread, today’s panel. In the Add Account dialog, the methods as rows with chevrons, then the picked one’s step with Back and Cancel. On the Account tab, a logged-out account’s card has a row per method with its own Log In (the first filled), and a step opens in place of its row. The dialog: <a href="img/now-add-account.png">now-add-account.png</a>.',
  nowImg: 'img/now-add-account.png',
  issues: ['Three looks for one thing', 'The Account tab’s card has a Log In per method'],
  options: [
    { key: 'A', name: 'One look in all three', from: 'the Add Account dialog',
      desc: 'Topic 2’s methods and topic 4’s steps are the same in the thread, the dialog and the account’s card; only the frame around them differs. In the card, a step replaces the rows, with Back.',
      good: 'Learn it once; what you pick in this round applies everywhere.', cost: 'The Account tab changes too.',
      mock: () => placesMock(cardA(), dialogA, settingsCard(settingsRowsA)) },
    { key: 'B', name: 'The thread and the dialog alike, Settings as it is', from: 'the Add Account dialog, and today',
      desc: 'The thread and the dialog share topic 2’s methods and topic 4’s steps (the dialog changes too if topic 2 picks another look than its rows). The Account tab’s card keeps a row per method with Log In.',
      good: 'Settings keeps a Log In button per method, which suits a page of settings.', cost: 'Two looks.',
      mock: () => placesMock(cardA(), dialogA, settingsCard(settingsRowsToday)) },
    { key: 'C', name: 'Settings logs in through the dialog', from: 'new, with the Add Account dialog',
      desc: 'A logged-out account’s card has one Log In… button, which opens the Add Account dialog for that account (“Log in to alex@hey.com”): its methods, the step and the result. The thread and the dialog share topic 2’s and topic 4’s look.',
      good: 'Two places instead of three, and the card stays short.', cost: 'A dialog for something the card did in place.',
      mock: () => placesMock(cardA(), dialog(`Log in to ${ME}`, `<div class="sm mu" style="margin-bottom:10px">${DROID_NAME} · Choose how to log in.</div>${methodRows(DROID, { hl: -1 })}`, gbtn('Cancel')), settingsCard(`<div class="row g2" style="padding:10px 16px;border-top:1px solid var(--bv)"><span class="sm mu grow">Log in to use this account.</span>${pbtn('Log In…')}</div>`)) },
  ],
});
