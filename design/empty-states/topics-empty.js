// The main area with nothing open: the Agents view with no thread (at launch, after archiving or
// deleting the open one, and before the first project), and the Workspaces view with no
// workspace.

ICONS.send = '<path d="M14.54 21.69a.5.5 0 0 0 .94-.03l6.5-19a.5.5 0 0 0-.64-.64l-19 6.5a.5.5 0 0 0-.03.94l7.93 3.18a2 2 0 0 1 1.11 1.11z"/><path d="m21.85 2.15-10.94 10.94"/>';

const ES_W = 1000;
const ES_H = 620;

const keys = (text) => `<span class="sm ph" style="letter-spacing:1.5px;flex:none">${text}</span>`;
const caret = '<span style="display:inline-block;width:1.5px;height:16px;background:var(--ac);vertical-align:-3px"></span>';

/** The window as the app draws it: the project switcher only in the Agents view. */
function azWin(body, { view = 'agents', w = ES_W, h = ES_H } = {}) {
  const switcher = view === 'agents' ? `<span class="row g15" style="font-size:13px">${ic('list', 'xs mu')}All projects${ic('chev-down', 'xs mu')}</span>` : '';
  return `<div class="m win" style="width:${w}px;height:${h}px">
    <div class="titlebar">${lights()}${ic('sidebar', 'sm mu')}${switcher}<div class="viewtabs"><span class="${view === 'agents' ? 'on' : ''}">Agents</span><span class="${view === 'workspaces' ? 'on' : ''}">Workspaces</span></div></div>
    <div class="body">${body}</div></div>`;
}
const mainArea = (html, { center = true, style = '' } = {}) =>
  `<div class="grow col" style="height:100%;background:var(--ed);min-width:0;position:relative;${center ? 'align-items:center;justify-content:center;' : ''}${style}">${html}</div>`;

// The demo world: storefront on this Mac, api on Devbox 1, and agents the server kept running
// while the app was closed.
const PROJECTS = {
  st: { mono: 'ST', color: 'g', name: 'storefront', path: '~/storefront', machine: 'laptop' },
  ap: { mono: 'AP', color: 't', name: 'api', path: '~/api', machine: 'server' },
};
const THREADS = [
  { id: 1, project: 'st', title: 'Add the checkout page', branch: 'checkout-flow', agent: 'claude', state: 'working', time: 'now' },
  { id: 2, project: 'st', title: 'Write tests for the rate limiter', branch: 'main', agent: 'codex', state: 'pending', time: '2m', asks: 'Allow command? cargo clippy --fix' },
  { id: 3, project: 'ap', title: 'Fix flaky login test', branch: 'main', agent: 'claude', state: 'awaiting', time: '5m', asks: 'Which test runner should I use?' },
  { id: 4, project: 'ap', title: 'Bump the API version', branch: 'main', agent: 'opencode', time: '2h' },
  { id: 5, project: 'st', title: 'Rename the cart store', branch: 'main', agent: 'claude', time: '1d' },
];
const projectIcon = (key) => mono(PROJECTS[key].mono, PROJECTS[key].color);
const stateOrTime = (thread) => (thread.state ? pill(thread.state) : `<span class="sm mu">${thread.time}</span>`);

// t3code's thread card, as sidebar.rs draws it.
const card = (thread, { on = false } = {}) => {
  const project = PROJECTS[thread.project];
  return `<div style="margin:0 4px;padding:8px 10px;border-radius:8px;${on ? 'background:var(--sel)' : ''}">
    <div class="row g2" style="height:20px">${projectIcon(thread.project)}<span class="grow trunc sm mu">${project.name}</span>${stateOrTime(thread)}</div>
    <div class="trunc" style="margin-top:4px">${thread.title}</div>
    <div class="row g2 sm faint" style="margin-top:4px"><span class="grow">${thread.branch}</span>${ic(project.machine, 'xs')}${glyph(thread.agent, 'sm')}</div></div>`;
};
const agentsSidebar = ({ selected = null, list } = {}) =>
  sidebar({ list: list ?? THREADS.map((thread) => card(thread, { on: thread.id === selected })).join('') });
const noProjectsSidebar = ({ button = false } = {}) => sidebar({
  list: `<div class="col grow" style="align-items:center;justify-content:center;gap:8px"><span class="mu">No projects yet</span>${button ? '<span class="btn">Open Folder…</span>' : ''}</div>`,
});

// Zed's Welcome page pieces (workspace/src/welcome.rs): an uppercase header in the code font
// with a rule, and full-width rows of an icon, a label and a key.
const zedHeader = (title) => `<div class="row g2" style="padding:0 4px;margin-bottom:8px"><span class="mono-font" style="font-size:10.5px;color:var(--mu);letter-spacing:.3px">${title.toUpperCase()}</span><span class="grow" style="height:1px;background:var(--bv)"></span></div>`;
const zedRow = (icon, label, end = '', { on = false } = {}) => `<div class="row g2" style="height:30px;padding:0 8px;border-radius:5px;${on ? 'background:var(--hov)' : ''}"><span class="mu" style="display:inline-flex">${icon}</span><span class="grow trunc">${label}</span>${end}</div>`;
const zedSection = (title, rows) => `<div class="col">${zedHeader(title)}${rows.join('')}</div>`;
// Zed's ProjectEmptyState (ui/src/components/project_empty_state.rs): a line, a button, "or",
// another button, in a narrow column.
const orRule = () => '<div class="row g2" style="margin:2px 0"><span class="grow" style="height:1px;background:var(--b)"></span><span class="xs mu">or</span><span class="grow" style="height:1px;background:var(--b)"></span></div>';
const wideButton = (label, end = '') => `<span class="btn" style="justify-content:center;width:100%;height:28px">${label}${end}</span>`;
const projectEmptyState = (text, first, second, width = 210) => `<div class="col" style="width:${width}px;gap:4px">
  <div class="sm mu" style="text-align:center;margin-bottom:8px">${text}</div>${first}${orRule()}${second}</div>`;

// The new thread screen (agent_view.rs render_new_thread): the headline, the composer as a
// card, and the strip under it.
const chip = (icon, label) => `<span class="row g1 sm mu" style="height:22px;padding:0 6px;border-radius:5px">${icon}${label}${ic('chev-down', 'xs')}</span>`;
const draftToolbar = (project, title) => `<div class="row g1" style="height:36px;flex:none;padding:0 8px;border-bottom:1px solid var(--b)">
  <span class="row g15 sm mu" style="padding:0 4px">${projectIcon(project)}${PROJECTS[project].name}</span><span class="sm mu">/</span><span class="trunc" style="font-size:13px;padding:0 4px">${title}</span><span class="grow"></span>${ibtn('diff')}${ibtn('terminal')}</div>`;
const newThreadScreen = (project) => `${draftToolbar(project, 'New thread')}
  <div class="grow col" style="align-items:center;justify-content:center;padding:0 16px">
    <div class="col" style="width:100%;max-width:600px;gap:24px">
      <div style="text-align:center;font-size:24px">What should we work on?</div>
      <div class="col" style="gap:8px">
        <div style="border:1px solid var(--b);border-radius:10px;background:var(--panel);padding:12px 14px 10px">
          <div style="height:44px;line-height:20px">${caret}<span class="ph">Message the agent…</span></div>
          <div class="row" style="height:26px;gap:12px;font-size:13px;color:var(--mu)"><span class="row g15">${glyph('claude')}Claude Agent${ic('chev-down', 'xs')}</span><span class="grow"></span><span class="row g1">Opus 5.5 ${ic('chev-down', 'xs')}</span><span style="color:var(--ac);display:inline-flex">${ic('send')}</span></div>
        </div>
        <div class="row"><span class="row g1">${chip(ic('folder', 'xs'), 'Local')}${chip(ic('laptop', 'xs'), 'This Mac')}</span><span class="grow"></span><span class="row g1 sm mu">${ic('branch', 'xs')}${PROJECTS[project].name === 'storefront' ? 'checkout-flow' : 'main'}</span></div>
      </div>
    </div>
  </div>`;

// An open thread, reduced: its toolbar, a turn, and the composer along the bottom.
const openThread = (thread) => `${draftToolbar(thread.project, thread.title)}
  <div class="grow col" style="padding:18px 0 0;align-items:center;overflow:hidden">
    <div class="col" style="width:620px;gap:10px;line-height:22px">
      <div class="row" style="justify-content:flex-end"><div style="max-width:78%;background:var(--panel);border-radius:12px;padding:8px 12px">Add the checkout page with a pay button</div></div>
      <div>I’ll add the page under <span class="mono-font sm">src/app/checkout</span> and reuse the cart’s totals.</div>
      <div class="col sm ph" style="gap:2px"><span class="row g15">${ic('file', 'xs')}Read src/app/cart/page.tsx</span><span class="row g15">${ic('pencil', 'xs')}Edit src/app/checkout/page.tsx</span></div>
      <div class="row g15 ac sm"><span class="spin"></span>Working…</div>
    </div>
  </div>
  <div style="border-top:1px solid var(--b);padding:8px 0 10px"><div style="width:620px;margin:0 auto"><div class="ph" style="height:22px">Message the agent…</div>
    <div class="row" style="height:26px;gap:12px;font-size:13px;color:var(--mu)"><span class="row g15">${glyph('claude')}Claude Agent</span><span class="grow"></span><span class="row g1">Opus 5.5 ${ic('chev-down', 'xs')}</span><span style="display:inline-flex">${ic('stop')}</span></div></div></div>`;

// The Workspaces view's sidebar today, with nothing in it.
const emptyAgentsSection = () => `<div class="col none" style="padding-top:4px"><div class="section-h">Agents<span class="rule"></span></div><div class="sm mu" style="padding:0 12px 10px">No agents yet</div></div>`;
const workspacesSidebar = ({ list = '<div class="sm mu" style="padding:6px 12px">No workspaces yet</div>', agents = emptyAgentsSection() } = {}) =>
  sidebar({ list, agents });

// What the New Workspace picker offers with no workspace open (new_space_picker.rs): each
// machine's home and its projects' checkouts.
const PLACES = [
  { section: 'This Mac', icon: ic('folder', 'sm mu'), label: 'Home Folder', detail: '~' },
  { section: 'This Mac', icon: projectIcon('st'), label: 'storefront', detail: '~/storefront' },
  { section: 'Devbox 1', icon: ic('folder', 'sm mu'), label: 'Home Folder', detail: '~' },
  { section: 'Devbox 1', icon: projectIcon('ap'), label: 'api', detail: '~/api' },
];
const placeRow = (place, { on = false } = {}) => `<div class="row g2" style="height:30px;padding:0 8px;border-radius:5px;${on ? 'background:var(--sel)' : ''}">${place.icon}<span class="none">${place.label}</span><span class="grow trunc sm mu">${place.detail}</span></div>`;
const placesBySection = (render) => {
  const sections = [...new Set(PLACES.map((place) => place.section))];
  return sections.map((section) => render(section, PLACES.filter((place) => place.section === section))).join('');
};

// 1. No thread open -------------------------------------------------------------------------
TOPICS.push({
  id: 'no-thread', section: 'Agents view', title: 'No thread open', size: 'wide', rec: 'C',
  now: 'At every launch and after the open thread is archived or deleted, the main area says "Select a thread, or start a new one" over a New Thread button. Nothing opens by itself, so every launch starts here, even while the server has agents working or waiting.',
  nowImg: 'img/now-agents.png',
  issues: ['The button doesn’t show its key (⌘N).', 'The empty area says nothing about the agents that need you; only the cards’ pills do.'],
  options: [
    {
      key: 'A', name: 'Pick a thread to continue', from: 't3code NoActiveThreadState',
      desc: 'As today, in t3code’s words: “Pick a thread to continue” over “Select an existing thread or create a new one to get started.”, then New Thread with its key.',
      good: 'Smallest change; says what to do.', cost: 'Still a stop at every launch.',
      mock: () => azWin(agentsSidebar() + mainArea(`<div class="col" style="align-items:center;gap:6px;text-align:center;max-width:420px">
        <div style="font-size:16px;font-weight:600">Pick a thread to continue</div>
        <div class="mu" style="font-size:13px">Select an existing thread or create a new one to get started.</div>
        <div style="margin-top:12px"><span class="btn">New Thread${keys('⌘N')}</span></div></div>`)),
    },
    {
      key: 'B', name: 'Get started and recent threads', from: 'Zed Welcome page',
      desc: 'Zed’s Welcome page, for threads: “Welcome back to agentZ”, then Get Started (New Thread, Open Folder…, Go To…, Command Palette, each with its key) and Recent Threads, the five worked in last, with their state. Arrow keys and Enter, or a click, open one.',
      good: 'Every way to start in one place, keyboard first.', cost: 'Repeats the sidebar’s list right beside it.',
      mock: () => azWin(agentsSidebar() + mainArea(`<div class="col" style="width:400px;gap:24px">
        <div style="text-align:center;font-size:20px">Welcome back to agentZ</div>
        ${zedSection('Get Started', [
          zedRow(ic('plus', 'sm'), 'New Thread', keys('⌘N'), { on: true }),
          zedRow(ic('folder-open', 'sm'), 'Open Folder…', keys('⌘O')),
          zedRow(ic('search', 'sm'), 'Go To…', keys('⌘P')),
          zedRow(ic('command', 'sm'), 'Command Palette', keys('⌘⇧P')),
        ])}
        ${zedSection('Recent Threads', THREADS.map((thread) => zedRow(projectIcon(thread.project), thread.title, stateOrTime(thread))))}
      </div>`)),
    },
    {
      key: 'C', name: 'A draft right away', from: 't3code index route',
      desc: 'What t3code does now: the main area is never empty. At launch the new thread screen opens in the project of the latest thread, ready to type. Archiving the open thread opens a draft in its project; deleting it opens that project’s next thread. A draft left with nothing typed goes away, as now, and isn’t in the list.',
      good: 'Every launch lands on a prompt; nothing to click to start.', cost: 'A draft starts its agent, so every launch starts one. Older threads are still a click in the sidebar.',
      mock: () => azWin(agentsSidebar() + mainArea(newThreadScreen('st'), { center: false }) + note('At launch: a draft in storefront, the latest thread’s project', 'right:16px;bottom:16px')),
    },
    {
      key: 'D', name: 'What’s waiting on you', from: 'agentZ Needs you strip, herdr attention',
      desc: 'The threads waiting for an approval or an answer, as the Workspaces view’s Needs you strip lists them: the state, the title and project, what it asks, and Go to. Under them, the latest threads. With nothing waiting, A’s words.',
      good: 'The server keeps agents working while the app is closed, so at launch this is what needs you.', cost: 'The cards already show the same states; another list to keep in step.',
      mock: () => {
        const waiting = THREADS.filter((thread) => thread.asks);
        const tint = (state) => (state === 'pending' ? 'rgba(222,193,132,.08)' : 'rgba(180,119,207,.09)');
        const rows = waiting.map((thread) => `<div class="row g3" style="padding:10px 12px;border-radius:8px;background:${tint(thread.state)};border:1px solid var(--bv)">
          <div class="col grow" style="gap:3px;min-width:0"><div class="row g2">${pill(thread.state)}<span class="sm mu row g15">${projectIcon(thread.project)}${PROJECTS[thread.project].name}</span></div>
            <div class="trunc">${thread.title}</div><div class="trunc sm mu">${thread.asks}</div></div>
          <span class="btn sm">Go to</span></div>`).join('');
        const recent = THREADS.filter((thread) => !thread.asks).map((thread) => zedRow(projectIcon(thread.project), thread.title, stateOrTime(thread))).join('');
        return azWin(agentsSidebar() + mainArea(`<div class="col" style="width:460px;gap:22px">
          <div class="col" style="gap:8px"><div class="sm mu b5">Waiting on you</div>${rows}</div>
          <div class="col">${zedHeader('Recent')}${recent}</div>
          <div class="row" style="justify-content:center"><span class="btn">New Thread${keys('⌘N')}</span></div></div>`));
      },
    },
    {
      key: 'E', name: 'Reopen the last thread', from: 'new (macOS apps reopen what was open); t3code after deleting',
      desc: 'The app remembers the open thread and opens it again at launch. Archiving or deleting the open thread opens the next one in the list, as t3code does after deleting. A’s words show only when there are no threads at all.',
      good: 'Picks up where you left off; almost never empty.', cost: 'After archiving, you land in a thread you didn’t pick.',
      mock: () => azWin(agentsSidebar({ selected: 1 }) + mainArea(openThread(THREADS[0]), { center: false }) + note('At launch: the thread open when you quit', 'right:16px;bottom:76px')),
    },
  ],
});

// 2. No projects yet --------------------------------------------------------------------------
TOPICS.push({
  id: 'no-projects', section: 'Agents view', title: 'Before the first project', size: 'wide', rec: 'A',
  now: 'Before any project, the sidebar says "No projects yet" with Open Folder…, and the main area still says "Select a thread, or start a new one"; its New Thread button opens the same folder dialog.',
  nowImg: 'img/now-first-run.png',
  issues: ['Two buttons for one step.', 'It asks you to select a thread that can’t exist yet.'],
  options: [
    {
      key: 'A', name: 'Open a folder to start', from: 't3code NoProjectsHero',
      desc: 't3code’s first screen: “What should we work on?”, the new thread screen’s headline, over “Open a folder to start your first thread.” and Open Folder… with its key. The sidebar only says “No projects yet”, so there’s one button.',
      good: 'One clear step, in the words the next screen uses.', cost: 'Says nothing about installing an agent; New Thread opens Settings › Agents for that, as now.',
      mock: () => azWin(noProjectsSidebar() + mainArea(`<div class="col" style="align-items:center;gap:8px;text-align:center">
        <div style="font-size:24px">What should we work on?</div>
        <div class="mu" style="font-size:13px">Open a folder to start your first thread.</div>
        <div style="margin-top:16px"><span class="btn primary">${ic('plus', 'sm')}Open Folder…</span></div></div>`)),
    },
    {
      key: 'B', name: 'A folder, or another machine', from: 'Zed ProjectEmptyState',
      desc: 'Zed’s empty sidebar, in the main area: a line saying what it’s for, Open Folder… with its key, “or”, and Add Machine… for projects on another machine over SSH. The sidebar only says “No projects yet”.',
      good: 'Shows that projects can live on other machines.', cost: 'A second choice most people don’t need on day one.',
      mock: () => azWin(noProjectsSidebar() + mainArea(projectEmptyState('Open a folder to start a thread with an agent in it.', wideButton('Open Folder…', keys('⌘O')), wideButton(`${ic('server', 'xs')}Add Machine…`), 230))),
    },
    {
      key: 'C', name: 'Welcome to agentZ', from: 'Zed Welcome page',
      desc: 'Zed’s first-run Welcome page: “Welcome to agentZ”, then Get Started: Open Folder…, Install an Agent… (Settings › Agents, listed until one is installed), Add Machine… and Settings. It shows until the first project.',
      good: 'Covers both first steps, a folder and an agent.', cost: 'A page you see once; more to build.',
      mock: () => azWin(noProjectsSidebar() + mainArea(`<div class="col" style="width:400px;gap:24px">
        <div style="text-align:center;font-size:20px">Welcome to agentZ</div>
        ${zedSection('Get Started', [
          zedRow(ic('folder-open', 'sm'), 'Open Folder…', keys('⌘O'), { on: true }),
          zedRow(ic('bot', 'sm'), 'Install an Agent…'),
          zedRow(ic('server', 'sm'), 'Add Machine…'),
          zedRow(ic('settings', 'sm'), 'Settings', keys('⌘,')),
        ])}</div>`)),
    },
  ],
});

// 3. No workspaces ----------------------------------------------------------------------------
TOPICS.push({
  id: 'no-workspace', section: 'Workspaces view', title: 'No workspaces', size: 'wide', rec: 'B',
  now: 'The view always shows a workspace while there is one, so this is only when there are none: the first visit, or after closing the last. The main area says "No workspaces" over a muted New Workspace ⌘⇧N button, which opens the New Workspace picker (each machine’s home and its projects’ checkouts).',
  nowImg: 'img/now-workspaces.png',
  issues: ['The button is as faint as the text around it.', 'The places to open are a click away, in the picker.'],
  options: [
    {
      key: 'A', name: 'New Workspace, or Home', from: 'Zed ProjectEmptyState',
      desc: 'Zed’s empty state: a line saying what a workspace is, New Workspace… with its key, “or”, and Open Home Folder, which opens a workspace in ~ on this Mac with a shell.',
      good: 'Explains the view to someone new; the common case is one click.', cost: 'Projects are still in the picker.',
      mock: () => azWin(workspacesSidebar() + mainArea(projectEmptyState('A workspace is a folder with tabs of shells and agents, kept running by the server.', wideButton('New Workspace…', keys('⌘⇧N')), wideButton(`${ic('folder', 'xs')}Open Home Folder`), 240)), { view: 'workspaces' }),
    },
    {
      key: 'B', name: 'The places to open, listed', from: 'Zed Welcome page Recent Projects, agentZ New Workspace picker',
      desc: 'What the picker offers, laid out like Zed’s Recent Projects: under each machine, its Home Folder and its projects’ checkouts, with the path. A click opens a workspace there with a shell. New Workspace… under them opens the picker for the rest.',
      good: 'The usual places are one click, and the view teaches what it holds.', cost: 'A long list with many projects (it would show the first few per machine).',
      mock: () => azWin(workspacesSidebar() + mainArea(`<div class="col" style="width:420px;gap:22px">
        <div style="text-align:center;font-size:20px">Open a workspace</div>
        ${placesBySection((section, places) => zedSection(section, places.map((place, index) => placeRow(place, { on: section === 'This Mac' && index === 0 }))))}
        <div class="row" style="justify-content:center"><span class="btn">New Workspace…${keys('⌘⇧N')}</span></div></div>`), { view: 'workspaces' }),
    },
    {
      key: 'C', name: 'The picker, already open', from: 'agentZ New Workspace picker (Zed’s recent projects)',
      desc: 'The New Workspace picker itself sits in the middle, focused: type to filter, Enter to open. The same list and keys as ⌘⇧N.',
      good: 'Keyboard first: type “api”, press Enter.', cost: 'A search field waiting for input looks busier than an empty view.',
      mock: () => azWin(workspacesSidebar() + mainArea(`<div class="col" style="gap:10px;align-items:center">
        <div class="sm mu">No workspaces. Open one:</div>
        <div style="width:416px;border:1px solid var(--b);border-radius:8px;background:var(--panel);box-shadow:0 12px 32px rgba(0,0,0,.35);overflow:hidden">
          <div class="row g2" style="padding:8px 12px;border-bottom:1px solid var(--bv)">${ic('search', 'sm mu')}${caret}<span class="ph" style="font-size:13px">Search folders…</span></div>
          <div style="padding:4px">${placesBySection((section, places) => `<div class="xs ph" style="padding:6px 8px 2px">${section}</div>${places.map((place, index) => placeRow(place, { on: section === 'This Mac' && index === 0 })).join('')}`)}</div>
          <div class="row g3 sm mu" style="padding:6px 12px;border-top:1px solid var(--bv)"><span>↩ Open</span><span>⌘↩ Open Another</span></div>
        </div></div>`), { view: 'workspaces' }),
    },
    {
      key: 'D', name: 'Start one in Home', from: 'herdr (starts in the folder it’s run from)',
      desc: 'The first time the view opens with no workspace, one opens in ~ with a shell, as herdr starts in the folder it’s run from and a terminal app opens a window. After you close the last one, A’s text shows.',
      good: 'A shell at once, like opening a terminal.', cost: 'A workspace you may not have wanted.',
      mock: () => {
        const home = { name: '~', path: '~', machine: 'mac', terminals: 1, agents: 0 };
        const shell = pane({ title: 'zsh', detail: '~', focus: true, buttons: true, body: [`Last login: Mon Oct  5 18:50:02 on ttys004`, `${C('tg', '➜')}  ${C('tc', B('~'))} <span class="cursor"></span>`] });
        return azWin(workspacesSidebar({ list: currentRow(home, { active: true }) }) + main(tabBar([['Tab 1']]), shell, note('Opened the first time you visit the view', 'right:16px;bottom:16px')), { view: 'workspaces' });
      },
    },
    {
      key: 'E', name: 'Keys to get going', from: 'Zed Welcome page Get Started',
      desc: 'Zed’s Get Started list, for this view: New Workspace…, Go To…, Command Palette and Shortcuts, each with its key.',
      good: 'Teaches the keys you’ll use every day here.', cost: 'Two clicks to a workspace (the row, then the picker).',
      mock: () => azWin(workspacesSidebar() + mainArea(`<div class="col" style="width:400px;gap:24px">
        <div style="text-align:center;font-size:20px">No workspaces</div>
        ${zedSection('Get Started', [
          zedRow(ic('plus', 'sm'), 'New Workspace…', keys('⌘⇧N'), { on: true }),
          zedRow(ic('search', 'sm'), 'Go To…', keys('⌘P')),
          zedRow(ic('command', 'sm'), 'Command Palette', keys('⌘⇧P')),
          zedRow(ic('keyboard', 'sm'), 'Shortcuts', keys('⌘/')),
        ])}</div>`), { view: 'workspaces' }),
    },
  ],
});

// 4. The Workspaces sidebar with none -----------------------------------------------------------
const sidebarFrame = (html) => frame(html, { w: 290, h: 520 });
TOPICS.push({
  id: 'sidebar-empty', section: 'Workspaces view', title: 'Its sidebar with none', size: 'narrow', rec: 'A',
  now: 'The list says "No workspaces yet" and the Agents section "No agents yet", beside the main area’s "No workspaces": three empty messages at once.',
  nowImg: 'img/now-workspaces.png',
  options: [
    {
      key: 'A', name: 'Quiet', from: 'new',
      desc: 'Nothing in the list, and no Agents section while there’s no workspace. The main area alone says what to do.',
      good: 'One message on screen.', cost: 'With the main area’s message gone too (topic 3 D), nothing says it’s empty.',
      mock: () => sidebarFrame(workspacesSidebar({ list: '', agents: '' })),
    },
    {
      key: 'B', name: 'Hide only the Agents section', from: 'new',
      desc: 'The list keeps "No workspaces yet"; the Agents section shows once an agent runs in a pane, with workspaces or without.',
      good: 'Agents is only there when it has something.', cost: 'Still two messages.',
      mock: () => sidebarFrame(workspacesSidebar({ agents: '' })),
    },
    {
      key: 'C', name: 'The button in the list', from: 'Zed threads sidebar empty state',
      desc: 'Like the Agents view’s "No projects yet": the list says "No workspaces yet" over a New Workspace… button, as Zed’s threads sidebar does. No Agents section.',
      good: 'Matches the Agents view’s sidebar.', cost: 'Two buttons on screen when the main area has one too.',
      mock: () => sidebarFrame(workspacesSidebar({ list: '<div class="col grow" style="align-items:center;justify-content:center;gap:8px"><span class="mu">No workspaces yet</span><span class="btn">New Workspace…</span></div>', agents: '' })),
    },
    {
      key: 'D', name: 'As it is', from: 'today',
      desc: '"No workspaces yet" in the list, and the Agents section with "No agents yet".',
      good: 'Nothing to change.', cost: 'Three empty messages with the main area’s.',
      mock: () => sidebarFrame(workspacesSidebar()),
    },
  ],
});
