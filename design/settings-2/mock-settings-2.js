// Settings, round 2: the settings window and its rows as settings_page.rs draws them, the demo
// world, and the pieces this round's topics share. Loaded after board/mock.js; names start with
// s2 so they don't clash with it.

Object.assign(ICONS, {
  updown: '<path d="m7 15 5 5 5-5"/><path d="m7 9 5-5 5 5"/>',
  sparkle: '<path d="M9.937 15.5A2 2 0 0 0 8.5 14.063l-6.135-1.582a.5.5 0 0 1 0-.962L8.5 9.936A2 2 0 0 0 9.937 8.5l1.582-6.135a.5.5 0 0 1 .963 0L14.063 8.5A2 2 0 0 0 15.5 9.937l6.135 1.581a.5.5 0 0 1 0 .964L15.5 14.063a2 2 0 0 0-1.437 1.437l-1.582 6.135a.5.5 0 0 1-.963 0z"/>',
  gauge: '<path d="m12 14 4-4"/><path d="M3.34 19a10 10 0 1 1 17.32 0"/>',
  book: '<path d="M4 19.5v-15A2.5 2.5 0 0 1 6.5 2H19a1 1 0 0 1 1 1v18a1 1 0 0 1-1 1H6.5a1 1 0 0 1 0-5H20"/>',
  hammer: '<path d="m15 12-8.373 8.373a1 1 0 1 1-3-3L12 9"/><path d="m18 15 4-4"/><path d="m21.5 11.5-1.914-1.914A2 2 0 0 1 19 8.172V7l-2.26-2.26a6 6 0 0 0-4.202-1.756L9 2.96l.92.82A6.18 6.18 0 0 1 12 8.4V10l2 2h1.172a2 2 0 0 1 1.414.586L18.5 14.5"/>',
  database: '<ellipse cx="12" cy="5" rx="9" ry="3"/><path d="M3 5V19A9 3 0 0 0 21 19V5"/><path d="M3 12A9 3 0 0 0 21 12"/>',
  'arrow-left': '<path d="m12 19-7-7 7-7"/><path d="M19 12H5"/>',
  dash: '<path d="M5 12h14"/>',
  cloud: '<path d="M17.5 19H9a7 7 0 1 1 6.71-9h1.79a4.5 4.5 0 1 1 0 9Z"/>',
  info: '<circle cx="12" cy="12" r="10"/><path d="M12 16v-4"/><path d="M12 8h.01"/>',
  'x-circle': '<circle cx="12" cy="12" r="10"/><path d="m15 9-6 6"/><path d="m9 9 6 6"/>',
});
GLYPHS.droid = '❋';
GLYPHS.antigravity = '⋀';

// The demo world. This Mac has all four agents; Devbox 1 (SSH) and the cloud machine agents-1
// have fewer, and their own logins.
const S2_MACHINES = {
  all: { name: 'All machines', icon: 'layers' },
  mac: { name: 'This Mac', icon: 'laptop' },
  devbox: { name: 'Devbox 1', icon: 'server' },
  cloud: { name: 'agents-1', icon: 'cloud' },
};
const S2_AGENTS = {
  claude: { name: 'Claude Agent', kind: 'claude', sub: 'v0.33.1 · Claude Code over ACP' },
  codex: { name: 'Codex', kind: 'codex', sub: 'v2.1.1 · OpenAI’s coding agent over ACP' },
  droid: { name: 'Factory Droid', kind: 'droid', sub: 'v0.234.0 · Factory’s coding agent' },
  antigravity: { name: 'Google Antigravity', kind: 'antigravity', sub: 'v1.4.0 · Google’s coding agent' },
};
// Claude Agent's two accounts on This Mac (Work's 5-hour window is used up), and Codex's one.
const S2_ACCOUNTS = {
  personal: { name: 'Personal', email: 'alex@hey.com', plan: 'Max 5x', hue: 212, left: 62 },
  work: { name: 'Work', email: 'alex@acme.co', plan: 'Team', hue: 150, left: 0, resets: '1h 52m' },
  codex: { name: 'Personal', email: 'alex@hey.com', plan: 'Pro', hue: 28, left: 81 },
};
const S2_PROJECTS = [
  { name: 'agentZ', mono: 'AZ', bg: '#4b3034', fg: '#e06c75' },
  { name: 'ielts-today', mono: 'IT', bg: '#3b3a5c', fg: '#a7a2f0' },
  { name: 'fluency.uz', mono: 'FU', bg: '#4d3238', fg: '#e57887' },
];

const s2MachineIc = (machine, cls = 'sm mu') => ic(S2_MACHINES[machine].icon, cls);
const s2MachineName = (machine) => S2_MACHINES[machine].name;
const s2Avatar = (account, size = 16) => `<span style="width:${size}px;height:${size}px;border-radius:50%;flex:none;display:inline-grid;place-items:center;font-size:${Math.round(size * 0.5)}px;font-weight:600;color:#1b1f26;background:linear-gradient(135deg,hsl(${account.hue} 70% 82%),hsl(${account.hue} 42% 60%))">${account.name[0]}</span>`;
const s2ProjectIcon = (project, size = 14) => `<span style="width:${size}px;height:${size}px;border-radius:4px;flex:none;display:inline-grid;place-items:center;background:${project.bg};color:${project.fg};font-size:${Math.round(size * 0.45)}px;font-weight:700;letter-spacing:-.2px">${project.mono}</span>`;
const s2Tag = (text, color = 'mu') => `<span class="xs none" style="padding:1px 6px;border-radius:4px;background:var(--hov);color:var(--${color})">${text}</span>`;

// Controls as settings_page.rs draws them: ui's DropdownMenu, Switch, the steppers, and Zed's
// ToggleButtonGroup.
const s2Dd = (label, { icon = '', open = false, menu = '' } = {}) => `<span class="row" style="gap:6px;height:24px;padding:0 6px 0 8px;border-radius:6px;border:1px solid ${open ? 'var(--bf)' : 'var(--b)'};background:var(--hov);white-space:nowrap;position:relative;font-size:13px">${icon}${label}${ic('updown', 'xs mu')}${menu}</span>`;
const s2Switch = (on, { mixed = false } = {}) => `<span style="width:34px;height:20px;border-radius:10px;flex:none;display:inline-block;position:relative;vertical-align:middle;background:${on ? '#5a7099' : '#353a44'};border:1px solid ${on ? '#6f86b0' : 'var(--b)'}">${mixed ? '<i style="position:absolute;top:8px;left:10px;width:12px;height:2px;border-radius:1px;background:#a9afbc"></i>' : `<i style="position:absolute;top:2px;${on ? 'right:2px' : 'left:2px'};width:14px;height:14px;border-radius:50%;background:${on ? '#e6e9ee' : '#6b717d'}"></i>`}</span>`;
const s2Stepper = (value) => `<span class="row" style="height:28px;padding:0 2px;gap:4px;border-radius:6px;border:1px solid var(--b);background:var(--ed)"><span class="ibtn sm">${ic('dash', 'xs')}</span><span style="width:16px;text-align:center">${value}</span><span class="ibtn sm">${ic('plus', 'xs')}</span></span>`;
const s2Seg = (labels, selected) => `<span class="row none" style="border:1px solid var(--b);border-radius:6px;overflow:hidden;font-size:12px;height:24px">${labels.map((label, index) => `<span class="row" style="height:100%;padding:0 9px;${index ? 'border-left:1px solid var(--b);' : ''}${index === selected ? 'background:var(--sel);color:var(--t)' : 'color:var(--mu)'}">${label}</span>`).join('')}</span>`;
// Zed's Reset to Default beside a row's title (IconName::Undo).
const s2Reset = (hover = false) => `<span class="ibtn sm ${hover ? 'hov' : ''}" style="color:var(--mu)">${ic('undo', 'xs')}</span>`;
const s2Tip = (text, style) => `<div class="pop" style="${style};padding:4px 8px;font-size:12px;white-space:nowrap;z-index:25">${text}</div>`;

/** Menus: items are [icon html, label, flags] with flags { hl, dis, check, end }, 'hr', or ['lbl', text]. */
function s2Menu(items, style = 'top:30px;right:0', width = 210) {
  const body = items.map((item) => {
    if (item === 'hr') return '<div class="hr"></div>';
    if (item[0] === 'lbl') return `<div class="lbl">${item[1]}</div>`;
    const [icon, label, flags = {}] = item;
    return `<div class="it ${flags.hl ? 'hl' : ''} ${flags.dis ? 'dis' : ''}">${icon || ''}<span class="grow">${label}</span>${flags.end || ''}${flags.check ? ic('check', 'sm') : ''}</div>`;
  }).join('');
  return `<div class="menu" style="${style};min-width:${width}px">${body}</div>`;
}
const s2MachineMenu = (chosen, { all = true, style } = {}) => s2Menu([
  ...(all ? [[s2MachineIc('all'), 'All machines', { check: chosen === 'all', hl: chosen === 'all' }], 'hr'] : []),
  ...['mac', 'devbox', 'cloud'].map((machine) => [s2MachineIc(machine), s2MachineName(machine), { check: chosen === machine, hl: !all && chosen === machine }]),
], style);
const s2MachineDd = (machine, { open = false, all = true } = {}) => s2Dd(s2MachineName(machine), { icon: s2MachineIc(machine), open, menu: open ? s2MachineMenu(machine, { all }) : '' });

// render_row and render_section: a title and description beside the control, in a bordered
// group under a small muted heading. `after` is a line under the description.
const s2Row = (title, desc, control = '', { titleExtra = '', after = '', hl = false, dim = false } = {}) => `<div class="row" style="padding:12px 16px;gap:24px;position:relative;${hl ? 'background:rgba(116,173,232,.12);' : ''}${dim ? 'opacity:.5;' : ''}"><div class="col grow" style="gap:2px"><span class="row g1">${title}${titleExtra}</span>${desc ? `<span class="sm mu" style="overflow-wrap:anywhere">${desc}</span>` : ''}${after}</div>${control ? `<div class="none" style="position:relative">${control}</div>` : ''}</div>`;
const s2Section = (title, rows, { right = '', note = '' } = {}) => `<div class="col" style="gap:8px"><div class="row" style="justify-content:space-between;min-height:16px"><span class="sm mu row g15">${title}</span>${right}</div><div class="col" style="border:1px solid var(--b);border-radius:8px;background:var(--panel)">${rows.map((row, index) => `<div style="${index < rows.length - 1 ? 'border-bottom:1px solid var(--bv)' : ''}">${row}</div>`).join('')}</div>${note ? `<span class="sm mu">${note}</span>` : ''}</div>`;
const s2Page = (heading, sections, { right = '', w = 720, top = 24 } = {}) => `<div class="col" style="width:${w}px;padding:${top}px 32px 32px;gap:24px">${heading ? `<div class="row" style="justify-content:space-between;gap:12px;position:relative;min-height:28px;z-index:5"><span style="font-size:17px">${heading}</span>${right}</div>` : ''}${sections.join('')}</div>`;
const s2Piece = (html, w = 720) => `<div class="m" style="width:${w}px">${html}</div>`;
const s2Stack = (frames) => `<div style="display:flex;flex-direction:column;gap:12px;align-items:flex-start">${frames.join('')}</div>`;
// A muted line under a row's description that names the machine or account setting its own.
const s2Differs = (icon, who, value) => `<span class="row g1 sm" style="margin-top:3px;color:var(--warn)">${icon}<span>${who} sets its own: ${value}</span></span>`;

// Today's rows, in their words (settings_page.rs).
const S2_ROWS = {
  order: () => s2Row('Thread order', 'How threads are sorted in the sidebar.', s2Dd('Latest activity')),
  modifier: () => s2Row('Use modifier to send', 'Whether to always use cmd-enter (or ctrl-enter on Linux or Windows) to send messages.', s2Switch(false)),
  thinking: () => s2Row('Show thinking', 'Whether the agent’s thinking shows open in threads. Otherwise it’s a Thinking row that opens on click.', s2Switch(false)),
  working: () => s2Row('Group working threads', 'Whether the sidebar groups the threads whose agents are working under Working. Threads waiting for you, and pinned ones, stay where they are.', s2Switch(false)),
  archive: (opts) => s2Row('Auto-archive threads', 'Archives a thread nobody has used for this long, going by its last activity. Pinned threads, ones that are working or waiting for you, and ones that are open stay.', s2Dd('Never'), opts),
  limit: () => s2Row('Subthreads at once', 'How many subthreads a thread runs at once; more wait their turn.', s2Stepper(4)),
  usage: () => s2Row('Usage shown in the composer', 'The window of an account’s limits the composer’s gauge and its account picker show. The current session is the shortest window, the 5-hour one, unless a longer window is used up.', s2Dd('Current session')),
  chats: () => s2Row('Chats', 'Threads for general conversation, outside every project, in their own group in the sidebar.', s2Switch(true)),
  artifactOpen: () => s2Row('Open in the browser', 'When a page an agent publishes opens by itself. Otherwise its card’s Open does it.', s2Dd('When I’m in the thread')),
  variants: () => s2Row('Variants', 'How many variants a topic has.', s2Stepper(3)),
  includeToday: () => s2Row('Include today’s', 'Today’s version as a variant to keep.', s2Switch(true)),
  sizes: () => s2Row('Sizes', 'The sizes each picture is shown at.', s2Seg(['Desktop', 'Phone', 'Both'], 0)),
  variantsAs: () => s2Row('Variants as', 'Pictures, live pages, or both.', s2Seg(['Pictures', 'Live pages', 'Both'], 0)),
  titlesMachine: () => s2Row('Machine', 'Each machine titles its own threads, with the CLIs installed on it.', s2MachineDd('mac', { all: false })),
  titlesOn: (opts) => s2Row('Generate thread titles', 'Titles a thread from its first message when its agent doesn’t name it, with a coding agent’s CLI installed on the machine.', s2Switch(true), opts),
  titlesProvider: (opts) => s2Row('Provider', 'The CLI that writes the titles.', s2Dd('Claude'), opts),
  titlesModel: (opts, value = 'Sonnet 5.5') => s2Row('Model', 'The model the CLI writes titles with.', s2Dd(value), opts),
  titlesEffort: (opts) => s2Row('Reasoning effort', 'How much the model thinks before it writes a title.', s2Dd('Low'), opts),
  combine: () => s2Row('Combine matching repositories across machines', 'Checkouts of one repository, on this machine or others, share one entry in the projects list.', s2Switch(true)),
  combineBy: () => s2Row('Combine by', 'Projects from the same repository share one row.', s2Dd('Group by repository')),
  owner: () => s2Row('Show repository owner', 'Combined projects named by their repository show it as owner/repo.', s2Switch(true)),
  login: () => s2Row('Start at login', 'Starts the server when you log in, before agentZ opens, so scripts using agentz-server call can reach it.', s2Switch(false)),
  acp: () => s2Row('ACP v2', 'Asks agents for the draft ACP v2 before v1, on every machine. Most agents’ v2 isn’t stable yet. Agents started after it changes use it.', s2Switch(false)),
};
const S2_DESIGN_NOTE = 'Given to agents as defaults for their designs; they may change them for a project.';

// The settings window: its sidebar (render_nav) beside the open page.
const S2_NAV_ICONS = { General: 'settings', Appearance: 'eye', Notifications: 'bell', Agents: 'sparkle', Usage: 'gauge', Skills: 'book', 'MCP Servers': 'hammer', Machines: 'server', Storage: 'database', Threads: 'chat', Subthreads: 'users', 'Thread titles': 'pencil', Artifacts: 'file', Experimental: 'zap', 'Subthreads and archiving': 'users' };
const S2_TODAY_PAGES = ['General', 'Appearance', 'Notifications', 'Agents', 'Usage', 'Skills', 'MCP Servers', 'Machines', 'Storage'];
const s2NavItem = (label, { on = false, icon, indent = 0, small = false, end = '' } = {}) => `<div class="row g2" style="height:${small ? 24 : 28}px;padding:0 8px 0 ${8 + indent}px;border-radius:6px;margin:0 4px;flex:none;${on ? 'background:var(--sel)' : ''}">${icon === undefined ? ic(S2_NAV_ICONS[label] || 'settings', 'sm mu') : icon}<span class="grow trunc" style="${small ? `font-size:13px;color:${on ? 'var(--t)' : 'var(--mu)'}` : ''}">${label}</span>${end}</div>`;
const s2NavHeading = (text) => `<div class="sm mu" style="padding:12px 12px 4px;flex:none">${text}</div>`;
const s2NavProjects = () => s2NavHeading('Projects') + S2_PROJECTS.map((project) => s2NavItem(project.name, { icon: s2ProjectIcon(project) })).join('');
/** Entries: a page's name, ['h', heading], or [name, options for s2NavItem]. */
const s2NavList = (entries, on) => entries.map((entry) => {
  if (Array.isArray(entry) && entry[0] === 'h') return s2NavHeading(entry[1]);
  if (Array.isArray(entry)) return s2NavItem(entry[0], entry[1]);
  return s2NavItem(entry, { on: entry === on });
}).join('');
function s2Window(content, { nav = s2NavList(S2_TODAY_PAGES, 'General') + s2NavProjects(), search = '', w = 1010, h = 720, scrollTop = 0 } = {}) {
  return `<div class="m win" style="width:${w}px;height:${h}px">
    <div class="titlebar">${lights()}${ic('sidebar', 'sm mu')}<span class="row g15" style="font-size:13px">${ic('list', 'xs mu')}All projects${ic('chev-down', 'xs mu')}</span><div class="viewtabs"><span class="on">Agents</span><span>Workspaces</span></div></div>
    <div class="body"><div class="sidebar"><div class="sb-head" style="color:var(--t)"><span class="grow b5" style="color:var(--t)">Settings</span>${ic('x', 'sm mu')}</div>${search}<div class="sb-list" style="gap:1px">${nav}</div><div class="sb-foot">${ic('arrow-left', 'sm')}<span>Back</span></div></div>
    <div class="grow" style="height:100%;overflow:hidden;background:var(--ed);position:relative"><div style="margin-top:-${scrollTop}px">${content}</div></div></div></div>`;
}

// An agent's page in Settings › Agents: its heading, then the Account, Defaults, Environment
// and Threads tabs.
const s2AgentHeading = (agent = S2_AGENTS.claude) => `<div class="row g4" style="gap:16px"><span style="width:52px;height:52px;border-radius:10px;background:#3f4552;display:grid;place-items:center;font-size:24px;flex:none">${GLYPHS[agent.kind]}</span><div class="col grow" style="gap:4px"><span class="row g2"><span style="font-size:18px;font-weight:600">${agent.name}</span><span class="row g1 xs" style="padding:1px 7px;border-radius:9px;background:rgba(161,193,129,.14);color:var(--ok)"><span class="dot done"></span>Logged in</span></span><span class="sm mu">${agent.sub}</span></div><span class="ibtn" style="border:1px solid var(--b);width:28px;height:28px">${ic('more', 'sm')}</span></div>`;
const s2AgentTabs = (current) => `<div class="row" style="gap:20px;border-bottom:1px solid var(--bv);font-size:13px">${['Account', 'Defaults', 'Environment', 'Threads'].map((tab) => `<span style="padding-bottom:10px;margin-bottom:-1px;border-bottom:2px solid ${tab === current ? 'var(--ac)' : 'transparent'};color:${tab === current ? 'var(--t)' : 'var(--mu)'}">${tab}</span>`).join('')}</div>`;
const s2AgentPage = (tab, sections, { right = s2MachineLabel('mac') } = {}) => `<div class="col" style="width:720px;padding:24px 32px 32px;gap:22px"><div class="row" style="justify-content:space-between;height:28px"><span class="row g15 sm mu">${ic('arrow-left', 'xs')}Agents</span>${right}</div>${s2AgentHeading()}${s2AgentTabs(tab)}${sections.join('')}</div>`;
const s2MachineLabel = (machine) => `<span class="row g15 mu">${s2MachineIc(machine)}${s2MachineName(machine)}</span>`;
const s2AccountEntry = (account, tag = '') => `<span class="row g15">${s2Avatar(account)}${account.name}${tag}</span>`;
// render_account_picker: "Account" and a menu of the accounts over the tabs.
const s2AccountPicker = (label, { open = false, menu = '' } = {}) => `<div class="row g3" style="position:relative;z-index:6"><span class="sm mu">Account</span>${s2Dd(label, { open, menu })}</div>`;
