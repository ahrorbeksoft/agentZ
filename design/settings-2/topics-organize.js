// How Settings is organized: the settings sidebar's pages, and where General's sections go.

const s2ThreadsPage = (opts = {}) => s2Page('Threads', [
  s2Section('Threads', [S2_ROWS.order(), S2_ROWS.modifier(), S2_ROWS.thinking(), S2_ROWS.working(), S2_ROWS.archive(opts.archive), S2_ROWS.usage()]),
  s2Section('Chats', [S2_ROWS.chats()]),
]);
const s2TitlesPage = () => s2Page('Thread titles', [
  s2Section('Thread titles', [S2_ROWS.titlesOn(), S2_ROWS.titlesProvider(), S2_ROWS.titlesModel(), S2_ROWS.titlesEffort()], { note: 'Each machine titles its own threads, with the CLIs installed on it.' }),
], { right: s2MachineDd('mac', { all: false }) });
const s2ArtifactsPage = () => s2Page('Artifacts', [
  s2Section('Opening', [S2_ROWS.artifactOpen()]),
  s2Section('Design defaults', [S2_ROWS.variants(), S2_ROWS.includeToday(), S2_ROWS.sizes(), S2_ROWS.variantsAs()], { note: S2_DESIGN_NOTE }),
]);

// A: headings in the sidebar, and General's sections as pages.
const NAV_A = [['h', 'agentZ'], 'General', 'Appearance', 'Notifications', ['h', 'Threads'], 'Threads', 'Subthreads', 'Thread titles', 'Artifacts', ['h', 'Agents'], 'Agents', 'Usage', 'Skills', 'MCP Servers', ['h', 'Machines'], 'Machines', 'Storage'];
// B: today's pages, the open one's sections folded out under it, as Zed's settings sidebar.
const GENERAL_SECTIONS = ['Threads', 'Chats', 'Artifacts', 'Thread titles', 'Projects', 'Server', 'Experimental'];
const navB = (on) => s2NavItem('General', { on: false, end: ic('chev-down', 'xs mu') }) + GENERAL_SECTIONS.map((section) => s2NavItem(section, { on: section === on, icon: '', indent: 24, small: true })).join('') + s2NavList(S2_TODAY_PAGES.slice(1)) + s2NavProjects();
// C: one flat list, as t3code's settings sidebar.
const NAV_C = ['General', 'Appearance', 'Notifications', 'Threads', 'Subthreads', 'Thread titles', 'Artifacts', 'Agents', 'Usage', 'Skills', 'MCP Servers', 'Machines', 'Storage', 'Experimental'];
// D: headings by where a setting applies.
const NAV_D = [['h', 'This app'], 'General', 'Appearance', 'Notifications', 'Threads', 'Machines', ['h', 'Every machine'], 'Subthreads and archiving', 'Artifacts', ['h', 'Each machine'], 'Agents', 'Usage', 'Skills', 'MCP Servers', 'Thread titles', 'Storage'];
// E: t3code's search row atop the list, and its results in the list's place.
const searchRow = (query) => `<div style="padding:6px 8px 2px"><div class="row g2" style="height:30px;padding:0 8px;border-radius:6px;border:1px solid var(--bf);background:var(--ed)">${ic('search', 'sm mu')}<span class="grow">${query}<span class="cursor" style="width:1px;height:14px;margin-left:1px"></span></span>${ic('x', 'xs mu')}</div></div>`;
const result = (title, where, on = false) => `<div class="col" style="margin:0 4px;padding:6px 10px;border-radius:6px;gap:1px;flex:none;${on ? 'background:var(--sel)' : ''}"><span>${title}</span><span class="xs mu">${where}</span></div>`;
const RESULTS = [
  ['Auto-archive threads', 'General › Threads'],
  ['Remove archived threads’ workspaces', 'Storage › Cleaning up'],
];

TOPICS.push({
  id: 'organize', section: 'Layout', title: 'How Settings is organized', size: 'wide', rec: 'AE',
  now: 'The settings sidebar lists nine pages (General, Appearance, Notifications, Agents, Usage, Skills, MCP Servers, Machines, Storage), then “Projects” with a row per project. General is one long page of seven sections and 23 rows: Threads (Thread order, Use modifier to send, Show thinking, Group working threads, Auto-archive threads, Subthreads at once, Usage shown in the composer), Chats, Artifacts (Open in the browser and four design defaults), Thread titles (its own machine picker, then Generate thread titles, Provider, Model, Reasoning effort), Projects (three rows on combining), Server (Start at login) and Experimental (ACP v2). Agents, Usage, Skills, MCP Servers and Storage each have the machine picker in their header. Nothing searches. “How this works” lists every setting with where it applies.',
  nowImg: 'img/now-settings-general.png',
  issues: [
    'General mixes the app’s own preferences (Show thinking), settings sent to every machine (Auto-archive, Subthreads at once, ACP v2), one machine’s (Thread titles, with a second machine picker mid-page) and This Mac’s alone (Start at login).',
    'Threads, artifacts and titles are sections of General, while Notifications, with four rows, has a page.',
    'Agents, Usage, Skills and MCP Servers go together but sit as peers of Appearance and Storage.',
    'Finding a setting means knowing its page: “Usage shown in the composer” is under General › Threads, not Usage.',
  ],
  options: [
    { key: 'A', name: 'Headings in the sidebar, General split up', from: 'new, with t3code’s one-topic pages',
      desc: 'The sidebar gets small muted headings, as it has for Projects: <b>agentZ</b> (General, Appearance, Notifications), <b>Threads</b> (Threads, Subthreads, Thread titles, Artifacts), <b>Agents</b> (Agents, Usage, Skills, MCP Servers), <b>Machines</b> (Machines, Storage), then Projects. General’s big sections become pages: Threads holds its seven rows less Subthreads at once, plus Chats; Subthreads holds Subthreads at once and the roles (topic 4); Thread titles takes the machine picker into its header, like Agents; Artifacts holds its five rows. General keeps Projects, Server and Experimental.',
      good: 'Each page is about one thing, and the headings say what goes with what.', cost: 'Thirteen pages instead of nine, and the headings make the sidebar longer.',
      mock: () => s2Window(s2ThreadsPage(), { nav: s2NavList(NAV_A, 'Threads') + s2NavProjects(), h: 760 }) },
    { key: 'B', name: 'Today’s pages, their sections under them', from: 'Zed’s settings sidebar',
      desc: 'The pages stay. As in Zed’s settings sidebar, the open page lists its sections under it, indented: General › Threads, Chats, Artifacts, Thread titles, Projects, Server, Experimental. A click scrolls to the section, and the one in view is highlighted as you scroll. Opening another page folds them away; the chevron folds them by hand.',
      good: 'Nothing moves, and the long General page gets a table of contents.', cost: 'General is still one page that mixes the app’s, every machine’s, one machine’s and This Mac’s settings.',
      mock: () => s2Window(s2Page('', [s2Section('Thread titles', [S2_ROWS.titlesMachine(), S2_ROWS.titlesOn(), S2_ROWS.titlesProvider(), S2_ROWS.titlesModel(), S2_ROWS.titlesEffort()]), s2Section('Projects', [S2_ROWS.combine(), S2_ROWS.combineBy(), S2_ROWS.owner()]), s2Section('Server', [S2_ROWS.login()])], { top: 12 }), { nav: navB('Thread titles') }) },
    { key: 'C', name: 'One flat list of short pages', from: 't3code’s settings sidebar',
      desc: 'As t3code’s settings sidebar: one list of pages with icons and no headings. General, Appearance, Notifications, Threads, Subthreads, Thread titles, Artifacts, Agents, Usage, Skills, MCP Servers, Machines, Storage and Experimental, then Projects. The pages hold what A’s do, and Experimental gets a page of its own at the end.',
      good: 'Plain, every page one click away, nothing to read but names.', cost: 'Fourteen rows in a row, and nothing says Usage goes with Agents.',
      mock: () => s2Window(s2ArtifactsPage(), { nav: s2NavList(NAV_C, 'Artifacts') + s2NavProjects() }) },
    { key: 'D', name: 'Grouped by where a setting applies', from: 'new',
      desc: 'Headings by scope. <b>This app</b>: General, Appearance, Notifications, Threads (order, sending, thinking, the Working group, the composer’s usage, Chats) and Machines. <b>Every machine</b>, for what the app sends to each server: Subthreads and archiving (Auto-archive, Subthreads at once, the roles, ACP v2) and Artifacts. <b>Each machine</b>, the pages with the machine picker: Agents, Usage, Skills, MCP Servers, Thread titles, Storage.',
      good: 'You know before opening a page whether a change reaches other machines.', cost: 'Groups by a fact about storage, not by what you look for: Auto-archive leaves Threads. With topic 2’s All machines, the line between the last two groups blurs.',
      mock: () => s2Window(s2TitlesPage(), { nav: s2NavList(NAV_D, 'Thread titles') + s2NavProjects(), h: 760 }) },
    { key: 'E', name: 'Search', from: 't3code and Zed',
      desc: 'A search field atop the settings sidebar, as t3code’s (Cmd-F or / focuses it) and Zed’s settings window have. Typing replaces the list with matching settings, by title, description and a few extra words (“archive”), each with its page and section; ↑↓ and Enter, or a click, open the page scrolled to the row, which is highlighted for a moment. Escape clears it. It goes with any of the others: pick it with + Also.',
      good: 'Finds a setting by what it’s called, wherever it lives.', cost: 'Every row needs its search words kept beside it. The agents’ own options on their Defaults tab come from each agent, so only the ones last seen are found.',
      mock: () => s2Window(s2ThreadsPage({ archive: { hl: true } }), { search: searchRow('archive'), nav: RESULTS.map(([title, where], index) => result(title, where, index === 0)).join(''), scrollTop: 300 }) },
  ],
});
