// agentZ mock building blocks. Each returns an HTML string; topics compose them.

const ICONS = {
  search: '<circle cx="11" cy="11" r="8"/><path d="m21 21-4.3-4.3"/>',
  plus: '<path d="M5 12h14"/><path d="M12 5v14"/>',
  x: '<path d="M18 6 6 18"/><path d="m6 6 12 12"/>',
  terminal: '<path d="m7 11 2-2-2-2"/><path d="M11 13h4"/><rect width="18" height="18" x="3" y="3" rx="2" ry="2"/>',
  users: '<path d="M16 21v-2a4 4 0 0 0-4-4H6a4 4 0 0 0-4 4v2"/><circle cx="9" cy="7" r="4"/><path d="M22 21v-2a4 4 0 0 0-3-3.87"/><path d="M16 3.13a4 4 0 0 1 0 7.75"/>',
  laptop: '<path d="M20 16V7a2 2 0 0 0-2-2H6a2 2 0 0 0-2 2v9m16 0H4m16 0 1.28 2.55a1 1 0 0 1-.9 1.45H3.62a1 1 0 0 1-.9-1.45L4 16"/>',
  server: '<rect width="20" height="8" x="2" y="2" rx="2" ry="2"/><rect width="20" height="8" x="2" y="14" rx="2" ry="2"/><line x1="6" x2="6.01" y1="6" y2="6"/><line x1="6" x2="6.01" y1="18" y2="18"/>',
  folder: '<path d="M20 20a2 2 0 0 0 2-2V8a2 2 0 0 0-2-2h-7.9a2 2 0 0 1-1.69-.9L9.6 3.9A2 2 0 0 0 7.93 3H4a2 2 0 0 0-2 2v13a2 2 0 0 0 2 2Z"/>',
  'folder-open': '<path d="m6 14 1.5-2.9A2 2 0 0 1 9.24 10H20a2 2 0 0 1 1.94 2.5l-1.54 6a2 2 0 0 1-1.95 1.5H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h3.9a2 2 0 0 1 1.69.9l.81 1.2a2 2 0 0 0 1.67.9H18a2 2 0 0 1 2 2v2"/>',
  branch: '<line x1="6" x2="6" y1="3" y2="15"/><circle cx="18" cy="6" r="3"/><circle cx="6" cy="18" r="3"/><path d="M18 9a9 9 0 0 1-9 9"/>',
  worktree: '<circle cx="12" cy="18" r="3"/><circle cx="6" cy="6" r="3"/><circle cx="18" cy="6" r="3"/><path d="M18 9v2c0 .6-.4 1-1 1H7c-.6 0-1-.4-1-1V9"/><path d="M12 12v3"/>',
  pasture: '<path d="M7 20h10"/><path d="M10 20c5.5-2.5.8-6.4 3-10"/><path d="M9.5 9.4c1.1.8 1.8 2.2 2.3 3.7-2 .4-3.5.4-4.8-.3-1.2-.6-2.3-1.9-3-4.2 2.8-.5 4.4 0 5.5.8z"/><path d="M14.1 6a7 7 0 0 0-1.1 4c1.9-.1 3.3-.6 4.3-1.4 1-1 1.6-2.3 1.7-4.6-2.7.1-4 1-4.9 2z"/>',
  merge: '<circle cx="18" cy="18" r="3"/><circle cx="6" cy="6" r="3"/><path d="M6 21V9a9 9 0 0 0 9 9"/>',
  split: '<rect width="18" height="18" x="3" y="3" rx="2"/><path d="M12 3v18"/>',
  'split-v': '<rect width="18" height="18" x="3" y="3" rx="2"/><path d="M3 12h18"/>',
  cols3: '<rect width="18" height="18" x="3" y="3" rx="2"/><path d="M9 3v18"/><path d="M15 3v18"/>',
  grid: '<rect width="7" height="7" x="3" y="3" rx="1"/><rect width="7" height="7" x="14" y="3" rx="1"/><rect width="7" height="7" x="14" y="14" rx="1"/><rect width="7" height="7" x="3" y="14" rx="1"/>',
  maximize: '<polyline points="15 3 21 3 21 9"/><polyline points="9 21 3 21 3 15"/><line x1="21" x2="14" y1="3" y2="10"/><line x1="3" x2="10" y1="21" y2="14"/>',
  minimize: '<polyline points="4 14 10 14 10 20"/><polyline points="20 10 14 10 14 4"/><line x1="14" x2="21" y1="10" y2="3"/><line x1="3" x2="10" y1="21" y2="14"/>',
  'chev-down': '<path d="m6 9 6 6 6-6"/>',
  'chev-right': '<path d="m9 18 6-6-6-6"/>',
  'chev-up': '<path d="m18 15-6-6-6 6"/>',
  more: '<circle cx="12" cy="12" r="1"/><circle cx="19" cy="12" r="1"/><circle cx="5" cy="12" r="1"/>',
  settings: '<path d="M12.22 2h-.44a2 2 0 0 0-2 2v.18a2 2 0 0 1-1 1.73l-.43.25a2 2 0 0 1-2 0l-.15-.08a2 2 0 0 0-2.73.73l-.22.38a2 2 0 0 0 .73 2.73l.15.1a2 2 0 0 1 1 1.72v.51a2 2 0 0 1-1 1.74l-.15.09a2 2 0 0 0-.73 2.73l.22.38a2 2 0 0 0 2.73.73l.15-.08a2 2 0 0 1 2 0l.43.25a2 2 0 0 1 1 1.73V20a2 2 0 0 0 2 2h.44a2 2 0 0 0 2-2v-.18a2 2 0 0 1 1-1.73l.43-.25a2 2 0 0 1 2 0l.15.08a2 2 0 0 0 2.73-.73l.22-.39a2 2 0 0 0-.73-2.73l-.15-.08a2 2 0 0 1-1-1.74v-.5a2 2 0 0 1 1-1.74l.15-.09a2 2 0 0 0 .73-2.73l-.22-.38a2 2 0 0 0-2.73-.73l-.15.08a2 2 0 0 1-2 0l-.43-.25a2 2 0 0 1-1-1.73V4a2 2 0 0 0-2-2z"/><circle cx="12" cy="12" r="3"/>',
  pencil: '<path d="M17 3a2.85 2.83 0 1 1 4 4L7.5 20.5 2 22l1.5-5.5Z"/><path d="m15 5 4 4"/>',
  pin: '<path d="M12 17v5"/><path d="M9 10.76a2 2 0 0 1-1.11 1.79l-1.78.9A2 2 0 0 0 5 15.24V16a1 1 0 0 0 1 1h12a1 1 0 0 0 1-1v-.76a2 2 0 0 0-1.11-1.79l-1.78-.9A2 2 0 0 1 15 10.76V7a1 1 0 0 1 1-1 2 2 0 0 0 0-4H8a2 2 0 0 0 0 4 1 1 0 0 1 1 1z"/>',
  bell: '<path d="M6 8a6 6 0 0 1 12 0c0 7 3 9 3 9H3s3-2 3-9"/><path d="M10.3 21a1.94 1.94 0 0 0 3.4 0"/>',
  sidebar: '<rect width="18" height="18" x="3" y="3" rx="2"/><path d="M9 3v18"/>',
  play: '<polygon points="6 3 20 12 6 21 6 3"/>',
  stop: '<rect width="14" height="14" x="5" y="5" rx="2"/>',
  restart: '<path d="M3 12a9 9 0 1 0 9-9 9.75 9.75 0 0 0-6.74 2.74L3 8"/><path d="M3 3v5h5"/>',
  history: '<path d="M3 12a9 9 0 1 0 9-9 9.75 9.75 0 0 0-6.74 2.74L3 8"/><path d="M3 3v5h5"/><path d="M12 7v5l4 2"/>',
  copy: '<rect width="14" height="14" x="8" y="8" rx="2" ry="2"/><path d="M4 16c-1.1 0-2-.9-2-2V4c0-1.1.9-2 2-2h10c1.1 0 2 .9 2 2"/>',
  external: '<path d="M15 3h6v6"/><path d="M10 14 21 3"/><path d="M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6"/>',
  broadcast: '<path d="M4.9 19.1C1 15.2 1 8.8 4.9 4.9"/><path d="M7.8 16.2c-2.3-2.3-2.3-6.1 0-8.5"/><circle cx="12" cy="12" r="2"/><path d="M16.2 7.8c2.3 2.3 2.3 6.1 0 8.5"/><path d="M19.1 4.9C23 8.8 23 15.1 19.1 19"/>',
  globe: '<circle cx="12" cy="12" r="10"/><path d="M12 2a14.5 14.5 0 0 0 0 20 14.5 14.5 0 0 0 0-20"/><path d="M2 12h20"/>',
  check: '<path d="M20 6 9 17l-5-5"/>',
  chat: '<path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z"/>',
  command: '<path d="M15 6v12a3 3 0 1 0 3-3H6a3 3 0 1 0 3 3V6a3 3 0 1 0-3 3h12a3 3 0 1 0-3-3"/>',
  keyboard: '<path d="M10 8h.01"/><path d="M12 12h.01"/><path d="M14 8h.01"/><path d="M16 12h.01"/><path d="M18 8h.01"/><path d="M6 8h.01"/><path d="M7 16h10"/><path d="M8 12h.01"/><rect width="20" height="16" x="2" y="4" rx="2"/>',
  trash: '<path d="M3 6h18"/><path d="M19 6v14c0 1-1 2-2 2H7c-1 0-2-1-2-2V6"/><path d="M8 6V4c0-1 1-2 2-2h4c1 0 2 1 2 2v2"/>',
  file: '<path d="M15 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V7Z"/><path d="M14 2v4a2 2 0 0 0 2 2h4"/><path d="M10 9H8"/><path d="M16 13H8"/><path d="M16 17H8"/>',
  clock: '<circle cx="12" cy="12" r="10"/><polyline points="12 6 12 12 16 14"/>',
  grip: '<circle cx="9" cy="12" r="1"/><circle cx="9" cy="5" r="1"/><circle cx="9" cy="19" r="1"/><circle cx="15" cy="12" r="1"/><circle cx="15" cy="5" r="1"/><circle cx="15" cy="19" r="1"/>',
  eye: '<path d="M2.062 12.348a1 1 0 0 1 0-.696 10.75 10.75 0 0 1 19.876 0 1 1 0 0 1 0 .696 10.75 10.75 0 0 1-19.876 0"/><circle cx="12" cy="12" r="3"/>',
  zap: '<path d="M4 14a1 1 0 0 1-.78-1.63l9.9-10.2a.5.5 0 0 1 .86.46l-1.92 6.02A1 1 0 0 0 13 10h7a1 1 0 0 1 .78 1.63l-9.9 10.2a.5.5 0 0 1-.86-.46l1.92-6.02A1 1 0 0 0 11 14z"/>',
  layers: '<path d="m12.83 2.18a2 2 0 0 0-1.66 0L2.6 6.08a1 1 0 0 0 0 1.83l8.58 3.91a2 2 0 0 0 1.66 0l8.58-3.9a1 1 0 0 0 0-1.83Z"/><path d="m22 17.65-9.17 4.16a2 2 0 0 1-1.66 0L2 17.65"/><path d="m22 12.65-9.17 4.16a2 2 0 0 1-1.66 0L2 12.65"/>',
  bot: '<path d="M12 8V4H8"/><rect width="16" height="12" x="4" y="8" rx="2"/><path d="M2 14h2"/><path d="M20 14h2"/><path d="M15 13v2"/><path d="M9 13v2"/>',
  filter: '<path d="M3 6h18"/><path d="M7 12h10"/><path d="M10 18h4"/>',
  star: '<polygon points="12 2 15.09 8.26 22 9.27 17 14.14 18.18 21.02 12 17.77 5.82 21.02 7 14.14 2 9.27 8.91 8.26 12 2"/>',
  monitor: '<rect width="20" height="14" x="2" y="3" rx="2"/><line x1="8" x2="16" y1="21" y2="21"/><line x1="12" x2="12" y1="17" y2="21"/>',
  corner: '<polyline points="15 10 20 15 15 20"/><path d="M4 4v7a4 4 0 0 0 4 4h12"/>',
  alert: '<circle cx="12" cy="12" r="10"/><line x1="12" x2="12" y1="8" y2="12"/><line x1="12" x2="12.01" y1="16" y2="16"/>',
  'check-circle': '<circle cx="12" cy="12" r="10"/><path d="m9 12 2 2 4-4"/>',
  undo: '<path d="M9 14 4 9l5-5"/><path d="M4 9h10.5a5.5 5.5 0 0 1 5.5 5.5a5.5 5.5 0 0 1-5.5 5.5H11"/>',
  arrow: '<path d="M5 12h14"/><path d="m12 5 7 7-7 7"/>',
  inbox: '<polyline points="22 12 16 12 14 15 10 15 8 12 2 12"/><path d="M5.45 5.11 2 12v6a2 2 0 0 0 2 2h16a2 2 0 0 0 2-2v-6l-3.45-6.89A2 2 0 0 0 16.76 4H7.24a2 2 0 0 0-1.79 1.11z"/>',
  hash: '<line x1="4" x2="20" y1="9" y2="9"/><line x1="4" x2="20" y1="15" y2="15"/><line x1="10" x2="8" y1="3" y2="21"/><line x1="16" x2="14" y1="3" y2="21"/>',
  save: '<path d="M15.2 3a2 2 0 0 1 1.4.6l3.8 3.8a2 2 0 0 1 .6 1.4V19a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2z"/><path d="M17 21v-7a1 1 0 0 0-1-1H8a1 1 0 0 0-1 1v7"/><path d="M7 3v4a1 1 0 0 0 1 1h7"/>',
  note: '<path d="M16 3H5a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2V8Z"/><path d="M15 3v4a2 2 0 0 0 2 2h4"/>',
  activity: '<path d="M22 12h-2.48a2 2 0 0 0-1.93 1.46l-2.35 8.36a.25.25 0 0 1-.48 0L9.24 2.18a.25.25 0 0 0-.48 0l-2.35 8.36A2 2 0 0 1 4.49 12H2"/>',
  palette: '<circle cx="13.5" cy="6.5" r=".5" fill="currentColor"/><circle cx="17.5" cy="10.5" r=".5" fill="currentColor"/><circle cx="8.5" cy="7.5" r=".5" fill="currentColor"/><circle cx="6.5" cy="12.5" r=".5" fill="currentColor"/><path d="M12 2C6.5 2 2 6.5 2 12s4.5 10 10 10c.926 0 1.648-.746 1.648-1.688 0-.437-.18-.835-.437-1.125-.29-.289-.438-.652-.438-1.125a1.64 1.64 0 0 1 1.668-1.668h1.996c3.051 0 5.555-2.503 5.555-5.554C21.965 6.012 17.461 2 12 2z"/>',
  'panel-bottom': '<rect width="18" height="18" x="3" y="3" rx="2"/><path d="M3 15h18"/>',
  move: '<path d="m5 9-3 3 3 3"/><path d="M9 5l3-3 3 3"/><path d="m15 19-3 3-3-3"/><path d="m19 9 3 3-3 3"/><path d="M2 12h20"/><path d="M12 2v20"/>',
  list: '<path d="M3 12h.01"/><path d="M3 18h.01"/><path d="M3 6h.01"/><path d="M8 12h13"/><path d="M8 18h13"/><path d="M8 6h13"/>',
  diff: '<path d="M12 3v14"/><path d="M5 10h14"/><path d="M5 21h14"/>',
};

function ic(name, cls = '') {
  return `<svg class="i ${cls}" viewBox="0 0 24 24">${ICONS[name] || ''}</svg>`;
}

const STATE = {
  working: { label: 'Working', short: 'working' },
  pending: { label: 'Pending Approval', short: 'needs approval' },
  awaiting: { label: 'Awaiting Input', short: 'needs input' },
  done: { label: 'Completed', short: 'done' },
  idle: { label: 'Idle', short: 'idle' },
};
const dot = (state) => (state ? `<span class="dot ${state}"></span>` : '');
const pill = (state, text) => (state ? `<span class="pill ${state}">${dot(state)}${text || STATE[state].label}</span>` : '');

// Agent glyphs stand in for the registry's single-color icons, drawn on a neutral tile.
const GLYPHS = { claude: '✻', codex: '◎', opencode: '▣', gemini: '✦', thread: '✻', shell: '' };
function glyph(kind, cls = '') {
  if (kind === 'shell') return ic('terminal', 'sm mu');
  return `<span class="glyph ${cls}">${GLYPHS[kind] || '•'}</span>`;
}
const mono = (text, color = 'g', cls = '') => `<span class="mono ${color} ${cls}">${text}</span>`;
const kbd = (text) => `<kbd>${text}</kbd>`;
const keycap = (text) => `<span class="kbd">${text}</span>`;
const ibtn = (name, cls = '') => `<span class="ibtn ${cls}">${ic(name, 'sm')}</span>`;
const lights = () => '<span class="lights"><i style="background:#ff5f57"></i><i style="background:#febc2e"></i><i style="background:#28c840"></i></span>';
const note = (text, style) => `<span class="overlay-note" style="${style}">${text}</span>`;

// The demo world every mock shares.
const SPACES = [
  { id: 'st', name: 'storefront', mono: 'ST', color: 'g', branch: 'checkout-flow', ahead: 2, machine: 'mac', state: 'working', terminals: 3, agents: 1, tabs: [['agents', 'working', 3], ['server', null, 1], ['3', null, 1]] },
  { id: 'bo', name: 'brave-otter', parent: 'st', mono: 'ST', color: 'g', branch: 'agentz/brave-otter-3fa', machine: 'mac', state: 'pending', terminals: 1, agents: 1, kind: 'worktree', tabs: [['1', 'pending', 2]] },
  { id: 'ap', name: 'api', mono: 'AP', color: 't', branch: 'main', behind: 3, machine: 'devbox', state: 'awaiting', terminals: 1, agents: 2, tabs: [['1', 'done', 2], ['review', 'awaiting', 1]] },
  { id: 'rn', name: 'Release notes', path: '~/docs', machine: 'mac', terminals: 1, agents: 0, tabs: [['1', null, 1]] },
  { id: 'hm', name: '~', path: '~', machine: 'mac', terminals: 1, agents: 0, tabs: [['1', null, 1]] },
];
const AGENTS = [
  { title: 'Claude Code', kind: 'claude', state: 'working', space: 'storefront', tab: 'agents', machine: 'This Mac', activity: 'Update(src/app/checkout/page.tsx)', ago: 'now' },
  { title: 'Codex', kind: 'codex', state: 'pending', space: 'brave-otter', tab: '1', machine: 'This Mac', activity: 'Allow command? cargo clippy --fix', ago: '30s' },
  { title: 'Fix flaky login test', kind: 'thread', state: 'awaiting', space: 'api', tab: 'review', machine: 'Devbox 1', activity: 'Which test runner should I use?', ago: '2m' },
  { title: 'OpenCode', kind: 'opencode', state: 'done', space: 'api', tab: '1', machine: 'Devbox 1', activity: 'Added 6 tests for the rate limiter', ago: '5m' },
];
const machineIcon = (machine) => (machine === 'devbox' ? 'server' : 'laptop');

// Terminal screens.
const C = (color, text) => `<span style="color:var(--${color})">${text}</span>`;
const B = (text) => `<b style="color:#e6e9ee">${text}</b>`;
const prompt = (folder = 'storefront', branch = 'checkout-flow') =>
  `${C('tg', '➜')}  ${C('tc', B(folder))} ${C('tb', 'git:(')}${C('tr', branch)}${C('tb', ')')} `;
const SCREENS = {
  claude: [
    `${C('ty', '✻')} Welcome to ${B('Claude Code')}`, '',
    '> Add the checkout page with a pay button', '',
    `${C('ty', '⏺')} Read(src/app/cart/page.tsx)`,
    `${C('ty', '⏺')} Update(src/app/checkout/page.tsx)`,
    '  ⎿  Updated with 24 additions and 3 removals', '',
    `${C('ty', '✶')} Implementing… ${C('faint', '(esc to interrupt)')}`, '',
    C('faint', '╭────────────────────────────────────╮'),
    C('faint', '│') + ' >                                  ' + C('faint', '│'),
    C('faint', '╰────────────────────────────────────╯'),
  ],
  codex: [
    `${B('>_ OpenAI Codex')} (v0.46)`, '',
    '▌ Write tests for the rate limiter', '',
    '• Ran cargo test -p limiter', '  └ 14 passed', '',
    `${C('ty', '• Allow command?')} cargo clippy --fix`,
    `  ${C('tb', '›')} 1. Yes   2. No, tell Codex what to do`,
  ],
  shell: [`Last login: Sun Oct  4 16:58:16 on ttys003`, `${prompt()}<span class="cursor hollow"></span>`],
  shellIdle: [`${prompt()}git status -sb`, `## checkout-flow...origin/main ${C('tg', '[ahead 2]')}`, ` M src/app/checkout/page.tsx`, `${prompt()}<span class="cursor"></span>`],
  dev: [`${prompt()}npm run dev`, '', `  ${C('tm', '▲ Next.js 15.0.3')}`, `  - Local:   ${C('tc', 'http://localhost:3000')}`, '', ` ${C('tg', '✓')} Ready in 1.2s`, ` ${C('tg', '○')} Compiling /checkout ...`, ` ${C('tg', '✓')} Compiled /checkout in 812ms`],
  opencode: [`${B('opencode')} ${C('faint', 'v0.15')}`, '', '> add rate limiter tests', '', `${C('tg', '✓')} Added 6 tests for the rate limiter`, `${C('tg', '✓')} cargo test: 20 passed`, '', C('faint', 'Ready for the next task')],
  logs: [`${prompt('api', 'main')}tail -f logs/api.log`, `${C('faint', '12:01:04')} GET /health 200 2ms`, `${C('faint', '12:01:09')} POST /login 200 41ms`, `${C('faint', '12:01:12')} ${C('ty', 'WARN')} rate limit 429 /login`, `${C('faint', '12:01:15')} GET /health 200 1ms`],
};
const term = (lines, style = '') => `<div class="term" style="${style}">${lines.join('\n')}</div>`;

// A pane in the app's current style; options override pieces.
function pane({ icon = 'terminal', glyphKind, title = 'zsh', detail = '', state, focus = false, body = SCREENS.shell, buttons = true, head, style = '' } = {}) {
  const lead = glyphKind ? glyph(glyphKind, 'sm') : ic(icon, 'sm');
  const header = head !== undefined ? head : `<div class="phead">${lead}<span class="trunc" style="flex:none;max-width:60%">${title}</span>${detail ? `<span class="trunc mu">${detail}</span>` : ''}${dot(state)}<span class="grow"></span>${buttons ? ibtn('split') + ibtn('maximize') + ibtn('x') : ''}</div>`;
  return `<div class="pane ${focus ? 'focus' : ''}" style="${style}">${header}${typeof body === 'string' ? body : term(body)}</div>`;
}
// A split: `dir` h (side by side) or v (stacked), `ratio` of the first.
function split(dir, ratio, a, b) {
  return `<div class="tile ${dir}" style="flex:1"><div class="tile" style="flex:${ratio};min-width:0;min-height:0">${a}</div><div class="divider"></div><div class="tile" style="flex:${1 - ratio};min-width:0;min-height:0">${b}</div></div>`;
}
const cell = (html) => `<div class="tile" style="flex:1">${html}</div>`;

// The storefront › agents tab: Claude Code beside two shells.
function agentsTab(overrides = {}) {
  const o = { claude: {}, shell: {}, dev: {}, ...overrides };
  return split('h', 0.56,
    pane({ glyphKind: 'claude', title: 'Claude Code', detail: '~/storefront', state: 'working', focus: true, body: SCREENS.claude, ...o.claude }),
    split('v', 0.5,
      pane({ title: 'zsh', detail: '~/storefront', body: SCREENS.shellIdle, ...o.shell }),
      pane({ title: 'npm run dev', detail: '~/storefront', body: SCREENS.dev, ...o.dev })));
}

function tabBar(tabs = [['agents', 'working'], ['server'], ['3']], { active = 0, lead = '', end = ibtn('plus'), style = '' } = {}) {
  const items = tabs.map(([name, state], i) => `<div class="tab ${i === active ? 'on' : ''}">${dot(state)}${name}</div>`).join('');
  return `<div class="tabbar" style="${style}">${lead}${items}<span class="grow"></span><div class="end">${end}</div></div>`;
}

function sbHead(extra = '') {
  return `<div class="sb-head">${ic('search', 'sm')}<span class="grow sm">Search…</span>${extra}${ibtn('plus')}</div>`;
}
const sbFoot = () => `<div class="sb-foot">${ic('settings', 'sm')}<span class="sm">Settings</span></div>`;

// The current workspace row (spaces_view.rs render_space_row).
function currentRow(space, { active = false, hover = false } = {}) {
  const icon = space.mono ? mono(space.mono, space.color) : ic('folder', 'sm mu');
  const where = space.branch ? (space.name === (space.parent ? '' : space.name) ? space.branch : space.branch) : space.path;
  const ab = `${space.ahead ? `<span class="okc">↑${space.ahead}</span>` : ''}${space.behind ? `<span class="delc">↓${space.behind}</span>` : ''}`;
  const counts = `${space.terminals ? `<span class="cnt">${ic('terminal', 'xs')}${space.terminals}</span>` : ''}${space.agents ? `<span class="cnt">${ic('users', 'xs')}${space.agents}</span>` : ''}`;
  return `<div class="srow ${active ? 'on' : ''} ${hover ? 'hov' : ''}">
    <div class="l1"><span style="opacity:${active ? 1 : 0.4}">${icon}</span><span class="grow trunc mu">${space.name}</span>${pill(space.state)}</div>
    <div class="l2"><span class="trunc">${where}</span>${ab}<span class="grow"></span><span class="row g15">${counts}</span><span style="opacity:.6;color:var(--mu);display:inline-flex">${ic(machineIcon(space.machine), 'sm')}</span></div>
  </div>`;
}

function currentAgentRow(agent, { active = false, remotes = true } = {}) {
  const where = `${remotes ? agent.machine + ' · ' : ''}${agent.space} › ${agent.tab}`;
  return `<div class="arow ${active ? 'on' : ''}"><span style="width:6px">${dot(agent.state)}</span>${ic('terminal', 'sm')}<span class="grow trunc">${agent.title}</span><span class="xs trunc" style="max-width:120px">${where}</span></div>`;
}

function agentsSection(rows) {
  return `<div class="col none" style="padding-top:4px"><div class="section-h">Agents<span class="rule"></span></div><div class="col" style="padding:0 0 4px">${rows}</div></div>`;
}

function sidebar({ head = sbHead(), list = '', agents = '', foot = sbFoot(), style = '' } = {}) {
  return `<div class="sidebar" style="${style}">${head}<div class="sb-list">${list}</div>${agents}${foot}</div>`;
}

// A whole window: title bar, then the view.
function win(content, { w = 1100, h = 620, titleExtra = '', viewBadge = '' } = {}) {
  return `<div class="m win" style="width:${w}px;height:${h}px">
    <div class="titlebar">${lights()}${ic('sidebar', 'sm mu')}${titleExtra}<div class="viewtabs"><span>Agents${viewBadge}</span><span class="on">Workspaces</span></div></div>
    <div class="body">${content}</div></div>`;
}
// The main area: tab bar over the panes.
const main = (tabbar, panes, extra = '') => `<div class="col grow" style="height:100%;position:relative">${tabbar}<div class="tile" style="flex:1;min-height:0">${panes}</div>${extra}</div>`;
// A frame of just a piece of the UI.
const frame = (html, { w = 290, h = 420, style = '' } = {}) => `<div class="m" style="width:${w}px;height:${h}px;${style}">${html}</div>`;

const currentList = (activeId = 'st') => SPACES.map((s) => currentRow(s, { active: s.id === activeId })).join('');
const currentAgents = () => agentsSection(AGENTS.map((a, i) => currentAgentRow(a, { active: i === 0 })).join(''));
