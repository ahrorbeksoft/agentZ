// Chats as threads, the Working line and the agent picker: the pieces this round's mocks share.
// Loaded after board/mock.js, chats/mock-chats.js (the sidebar, cards and thread in JetBrains
// Dark) and composer-2/mock-composer-2.js (the composer's chips and menus); names start with
// ch2 so they don't clash with theirs.

ICONS.lightbulb = '<path d="M15 14c.2-1 .7-1.7 1.5-2.5 1-.9 1.5-2.2 1.5-3.5A6 6 0 0 0 6 8c0 1 .2 2.2 1.5 3.5.7.7 1.3 1.5 1.5 2.5"/><path d="M9 18h6"/><path d="M10 22h4"/>';

// Sidebar ----------------------------------------------------------------------------------------

const ch2Icons = (item) => `${machineGlyph(item)}${glyph(item.glyph, 'sm')}`;
const ch2Hover = () => `<span class="row" style="gap:2px">${ibtn('clock', 'sm')}${ibtn('archive', 'sm')}</span>`;

// A chat as a card under All projects. `look`: A Chat in the project's place, B with the agent's
// name where the branch is, C two lines, D "No project".
function ch2ChatCard(chat, { look = 'A', on = false, hov = false, end } = {}) {
  const label = look === 'D' ? 'No project' : 'Chat';
  const state = end ?? pill(chat.state);
  const head = `<div class="p">${chatIcon('xs')}<span>${label}</span><span class="grow"></span>${state}</div>`;
  if (look === 'C') {
    return `<div class="tcard ${on ? 'on' : ''} ${hov ? 'hov' : ''}">${head}<div class="tl row" style="gap:6px"><span class="grow trunc">${chat.title}</span><span class="row" style="gap:6px;color:var(--ph)">${ch2Icons(chat)}</span></div></div>`;
  }
  const where = look === 'B' ? `<span class="grow trunc">${chat.agent}</span>` : '<span class="grow"></span>';
  return `<div class="tcard ${on ? 'on' : ''} ${hov ? 'hov' : ''}">${head}<div class="tl trunc">${chat.title}</div><div class="br">${where}${ch2Icons(chat)}</div></div>`;
}

// A chat's card while Chats is picked, as one project's cards are: the title with the state,
// then the machine and agent.
function ch2ScopedCard(chat, { on = false, look = 'A' } = {}) {
  const where = look === 'B' ? `<span class="grow trunc">${chat.agent}</span>` : '<span class="grow"></span>';
  return `<div class="tcard ${on ? 'on' : ''}"><div class="tl row" style="gap:6px"><span class="grow trunc">${chat.title}</span>${pill(chat.state)}</div><div class="br">${where}${ch2Icons(chat)}</div></div>`;
}

// Today's cards and the chats among them, by last activity, as Newest first orders them.
const CH2_MIXED = ['t:checkout', 'c:webp', 'c:cache', 't:limiter', 't:notes', 'c:async', 't:webhooks'];
const ch2Mixed = (chat = (item, opts) => ch2ChatCard(item, opts), { on = 'async', hov = '' } = {}) => CH2_MIXED.map((key) => {
  const [kind, id] = key.split(':');
  return kind === 't'
    ? tcard(THREADS.find((thread) => thread.id === id), { on: id === on })
    : chat(CHATS.find((item) => item.id === id), { on: id === on, hov: id === hov });
}).join('');

// The sidebar under a narrow title bar: the project picker's button and what it picked.
const ch2TitleBar = (label = 'All projects', icon = ic('list', 'sm')) => `<div class="atitle" style="gap:10px">${ic('sidebar', 'sm')}<span class="row g1" style="color:var(--t)">${icon}${label}${chev}</span></div>`;
const ch2Window = (title, sidebarHtml, { h = 600, w = 290, over = '' } = {}) => jb(frame(`<div class="awin">${title}<div class="abody">${sidebarHtml}</div></div>${over}`, { w, h, style: 'border-radius:8px;border:1px solid #111' }));

// The project picker (project_switcher.rs): search, All projects, each machine's projects, Add
// Project…. `chats`: where the Chats entry goes (top, bottom, project) or none.
function ch2PickerRow({ icon, name, hint = '', state = '', gear = true, on = false, hl = false }) {
  return `<div class="it ${hl ? 'hl' : ''}" style="height:30px;gap:8px">${icon}<span class="trunc">${name}</span>${hint ? `<span class="xs" style="color:var(--ph)">${hint}</span>` : ''}${dot(state)}${on ? ic('check', 'sm ac') : ''}<span class="grow"></span>${gear ? `<span style="display:inline-flex;color:var(--ph)">${ic('settings', 'sm')}</span>` : ''}</div>`;
}
function ch2Switcher({ chats = 'top', current = 'all', w = 320 } = {}) {
  const all = ch2PickerRow({ icon: ic('list', 'sm'), name: 'All projects', hint: '3 projects', gear: false, on: current === 'all', hl: current === 'all' });
  const chatsRow = ch2PickerRow({ icon: chatIcon(), name: chats === 'project' ? 'Chat' : 'Chats', hint: chats === 'project' ? '' : '5 chats', state: 'pending', gear: chats === 'project', on: current === 'chats', hl: current === 'chats' });
  const project = (key, state) => ch2PickerRow({ icon: projectMono(key), name: PROJECTS[key].name, state });
  const machines = `<div class="lbl">This Mac</div>${chats === 'project' ? chatsRow : ''}${project('st', 'working')}${project('dc', 'done')}<div class="lbl">Devbox 1</div>${project('ap', 'done')}`;
  const search = `<div class="row" style="gap:8px;height:34px;padding:0 8px;color:var(--ph);border-bottom:1px solid var(--bv);margin:-4px -4px 4px">${ic('search', 'sm')}Search projects…</div>`;
  const add = `<div class="hr"></div><div class="it">${ic('folder-open', 'sm')}Add Project…</div>`;
  const body = chats === 'top'
    ? `${all}${chatsRow}<div class="hr"></div>${machines}`
    : chats === 'bottom'
      ? `${all}<div class="hr"></div>${machines}<div class="hr"></div>${chatsRow}`
      : `${all}<div class="hr"></div>${machines}`;
  return `<div class="menu" style="position:relative;width:${w}px">${search}${body}${add}</div>`;
}
// The title bar with the picker open under its button.
const ch2SwitcherFrame = (opts = {}, { h = 330 } = {}) => jb(frame(`<div class="awin">${ch2TitleBar(opts.current === 'chats' ? 'Chats' : 'All projects', opts.current === 'chats' ? chatIcon() : ic('list', 'sm'))}<div style="flex:1;background:var(--panel);padding:4px 10px">${ch2Switcher(opts)}</div></div>`, { w: 350, h, style: 'border-radius:8px;border:1px solid #111' }));

// A card's menu (sidebar.rs), its rows by name.
const ch2CardMenu = (rows, style) => `<div class="menu" style="${style};width:200px">${rows.map((row) => (row === '-' ? '<div class="hr"></div>' : `<div class="it ${row === 'Delete…' ? 'danger' : ''}">${row}${row === 'Snooze' ? `<span class="grow"></span>${ic('chev-right', 'xs')}` : ''}</div>`)).join('')}</div>`;

// The Working line ----------------------------------------------------------------------------

// A label with the shimmer: a band of the text color sweeps across it every 2.2 seconds.
const ch2Shim = (text, base = 'var(--mu)') => `<span class="ch2-shim" style="--shim-base:${base}">${text}</span>`;
// A thought's row among the tool calls.
const ch2Thought = (label, { icon = ic('lightbulb', 'sm') } = {}) => `<div class="trow"><span class="ico">${icon}</span><span>${label}</span></div>`;
const CH2_DOTS = '<span class="mono-font" style="font-size:12px;color:var(--mu)">⠹</span>';
const CH2_SAND = '<span class="mono-font" style="font-size:12px;color:var(--mu)">⢎</span>';
// The line at the end of a turn (render_generating): the spinner, then its parts.
const ch2Working = (parts, { spinner = CH2_DOTS } = {}) => `<div class="row" style="gap:8px;padding:8px 0 4px 2px;font-size:12px;color:var(--mu)">${spinner === '' ? '' : `<span style="width:10px;display:inline-flex;justify-content:center">${spinner}</span>`}${parts}</div>`;
const CH2_SHELL = '<span style="color:var(--ph)">· 1 shell</span>';
const CH2_SUBTHREADS = '<span style="color:var(--ph)">· 2 subthreads running</span>';
const ch2Permission = () => `<div style="margin:4px 0 6px;border:1px solid var(--b);border-radius:8px;background:var(--ed);padding:8px 10px;display:flex;flex-direction:column;gap:8px"><div class="row" style="gap:6px;font-size:13px;color:var(--t)">${ic('terminal', 'sm')}Run <code class="c">cargo test -p app</code></div><div class="row" style="gap:6px"><span class="btn sm">Allow once</span><span class="btn sm">Always allow</span><span class="btn sm ghost">Reject</span></div></div>`;
const ch2Convo = ({ thinking = '', working = '', between = '' } = {}) => `${bub('Show in the agent picker which agents are working, and each one’s usage.')}${msg('I’ll start with where the picker reads its agents.')}${trow(ic('file', 'sm'), 'Read <span class="subj">agent_view.rs</span>')}${trow(ic('search', 'sm'), 'Searched for <span class="subj">render_agent_picker</span>')}${thinking}${between}${working}`;

// The composer (render_thread_composer): the card with + and who answers at the left, the
// agent's settings and Send at the right, the strip with the machine under it.
const ch2Send = (working) => (working
  ? '<span style="width:24px;height:24px;border-radius:50%;background:var(--del);display:inline-grid;place-items:center;flex:none"><i style="width:8px;height:8px;background:#fff;border-radius:1.5px;display:block"></i></span>'
  : `<span style="width:24px;height:24px;border-radius:50%;background:var(--sel);color:var(--ph);display:inline-grid;place-items:center;flex:none">${ic('arrow-up', 'xs')}</span>`);
const ch2Composer = ({ chips = ch2TodayChips(), working = false, menu = '' } = {}) => `<div style="padding:0 16px 10px;flex:none;position:relative"><div style="border:1px solid var(--b);border-radius:10px;background:var(--ed);padding:8px;display:flex;flex-direction:column;gap:10px;position:relative"><div style="padding:4px 4px 0;color:var(--ph);min-height:24px">Message the agent…</div><div class="row" style="justify-content:space-between;gap:8px;min-width:0"><span class="row" style="gap:2px;min-width:0">${c2Plus()}${chips}</span><span class="row" style="gap:4px;flex:none">${c2Chip('', 'Default')}${c2Chip('', 'Opus 5.5 · High')}${ch2Send(working)}</span></div>${menu}</div><div class="row sm mu" style="gap:5px;padding:6px 6px 0">${ic('laptop', 'xs')}This Mac</div></div>`;
const ch2Head = () => `${c2Az()}<span>agentZ</span>`;
const ch2Thread = ({ convo, composer = ch2Composer({ working: true }), w = 760, h = 450 } = {}) => jb(frame(threadView({ lead: ch2Head(), title: 'Add usage indicators to the agent picker', right: PROJECT_HEAD_RIGHT, convo, composer }), { w, h, style: 'border-radius:8px;border:1px solid #111' }));

// The agent picker --------------------------------------------------------------------------

// The fill behind an agent's icon on an account's threads (controls::account_fill_color).
const CH2_FILL = { Personal: '#3f9b5d', Work: '#c46a2c', 'ahror@example.com': '#4f7fd0' };
const ch2OnColor = (kind, color, size = 14) => `<span style="width:${size}px;height:${size}px;border-radius:${Math.round(size / 3.5)}px;background:${color};color:#fff;display:inline-grid;place-items:center;font-size:${Math.round(size * 0.62)}px;line-height:1;flex:none">${GLYPHS[kind]}</span>`;
const ch2Tinted = (kind, color) => `<span class="glyph sm" style="background:none;color:${color};font-size:12px">${GLYPHS[kind]}</span>`;
const ch2Left = (left) => (left === null ? '' : `<span class="xs none" style="color:${c2LeftColor(left)}">${left}% left</span>`);
const ch2Dot = (count) => (count ? '<span class="dot working"></span>' : '');
const ch2External = (account) => (account.external ? '<span class="xs none" style="color:var(--ph)">· External</span>' : '');
// Today's chips: the agent, the account and its usage.
const ch2TodayChips = () => `${c2Chip(glyph('claude', 'sm'), 'Claude Agent')}${c2AccountChip()}${c2Gauge(69)}`;

// A menu row: its icon, the name (and the working dot), a second line, its end, the check and,
// for a submenu, a chevron.
function ch2Row({ lead, name, after = '', line2 = '', end = '', check = false, chevron = null, hl = false, indent = 0 }) {
  return `<div class="it ${hl ? 'hl' : ''}" style="gap:9px;${line2 ? 'height:auto;padding-top:5px;padding-bottom:5px;' : ''}${indent ? `padding-left:${indent}px;` : ''}">${lead}<span class="col grow" style="min-width:0;gap:1px"><span class="row" style="gap:6px;min-width:0"><span class="trunc">${name}</span>${after}</span>${line2 ? `<span class="xs mu trunc">${line2}</span>` : ''}</span>${end}<span style="width:14px;display:inline-flex;flex:none">${check ? ic('check', 'sm ac') : ''}</span>${chevron === null ? '' : `<span style="width:12px;display:inline-flex;flex:none;color:var(--mu)">${chevron ? ic('chev-right', 'xs') : ''}</span>`}</div>`;
}
// The agent picker's rows: `chevrons` marks the agents with accounts; `ownEnd` keeps what's
// left on those rows.
const ch2AgentRows = ({ hl = 'claude', chevrons = true, ownEnd = true, only } = {}) => (only ?? C2_AGENTS).map((agent) => ch2Row({
  lead: glyph(agent.glyph, 'sm'), name: agent.name, after: ch2Dot(agent.working),
  end: agent.accounts > 1 && !ownEnd ? '' : ch2Left(agent.left), check: agent.id === 'claude',
  chevron: chevrons ? agent.accounts > 1 : null, hl: agent.id === hl,
})).join('');
// Claude Agent's accounts: `look` B is one list, C adds the plan, A is the account picker's.
function ch2AccountRows(look = 'B', { indent = 0, size = 16 } = {}) {
  const row = (account, { plan = false } = {}) => ch2Row({ lead: c2Avatar(account, size), name: account.name, after: `${ch2External(account)}${ch2Dot(account.working)}`, line2: plan ? account.plan : '', end: ch2Left(account.left), check: account.current, indent });
  if (look === 'A') return `<div class="lbl">This thread</div>${row(C2_ACCOUNTS[0], { plan: true })}<div class="hr"></div><div class="lbl">Switch to</div>${C2_ACCOUNTS.slice(1).map((account) => row(account, { plan: true })).join('')}`;
  return C2_ACCOUNTS.map((account) => row(account, { plan: look === 'C' })).join('');
}
// The picker with Claude Agent's accounts open beside it, as a submenu opens.
const ch2WithSubmenu = (menu, submenu, { top = 4, w = 640, h = 250 } = {}) => jb(frame(`<div style="position:absolute;inset:0;background:var(--panel);padding:14px;display:flex;align-items:flex-start">${menu}<div style="margin-top:${top}px;margin-left:-3px">${submenu}</div></div>`, { w, h }));
const ch2PickerAlone = (menu, { w = 360, h = 250 } = {}) => jb(frame(`<div style="position:absolute;inset:0;background:var(--panel);padding:14px">${menu}</div>`, { w, h }));
