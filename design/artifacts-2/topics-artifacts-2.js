// The second artifacts round: the Artifacts button and a thread's list, the artifacts page,
// documents, designs agentZ draws from an agent's variants, picks and Submit, mentions, and the
// design defaults and the agent's tools. Built on mock-artifacts-2.js; only board/mock.js may be
// used at the top level (decisions.js reads this file without the artifacts round's).

// A piece of the app on its own, in JetBrains Dark.
const jbFrame = (html, { w = 900, h = 560 } = {}) => frame(html, { w, h, style: 'border-radius:8px;border:1px solid #111' }).replace('class="m"', 'class="m jb"');
const STOREFRONT_ITEMS = A2.filter((item) => item.thread.project === 'storefront');
const PLANS_ITEMS = A2.filter((item) => item.thread === T2.plans);
const WAITING_ITEMS = A2.filter((item) => item.kind === 'design' && !item.submitted);
const threadGroups = (items) => [...new Set(items.map((item) => item.thread))].map((thread) => [thread, items.filter((item) => item.thread === thread)]);
const groupLabel = (thread) => `<div class="lbl row g1"><span class="trunc">${thread.chat ? `Chat · ${thread.title}` : thread.title}</span>${thread.archived ? `<span class="row g1 none">· ${ic('archive', 'xs')}Archived</span>` : ''}</div>`;

// 1. The button by the project picker ------------------------------------------------------------
const buttonMock = (button) => stackOf([
  ['All projects: every artifact', titleStrip({ scope: 'All projects', button })],
  ['storefront picked: its threads’ artifacts', appWin2(plansThread(), { button, h: 560 })],
]);
TOPICS.push({
  id: 'button', section: 'The Artifacts button', title: 'The button by the project picker', size: 'wide', rec: 'B',
  now: 'Artifacts are listed in an <b>Artifacts</b> shelf in the sidebar, above Archived, closed at first with its count: the picked project’s artifacts and every chat’s, or every one under All projects (screenshot). Open, it has a row per artifact (its kind, title, version, where it came from and when), then All artifacts…. The title bar has the sidebar button, the project picker, and Agents · Workspaces. Decided: the shelf goes, and a button by the picker takes its place, counting every artifact under All projects and the picked project’s otherwise (chats’ only under All projects).',
  nowImg: 'img/now-app.png',
  issues: ['A shelf of artifacts sits among the threads and grows with them', 'The shelf counts chats’ artifacts under every project'],
  options: [
    { key: 'A', name: 'An icon and the count', from: 'Claude Code (the ⧉ pill under the prompt)',
      desc: 'Beside the picker, the artifacts icon and how many there are: 15 under All projects, 7 with storefront picked. Its tooltip says Artifacts.',
      good: 'Small; the title bar stays quiet.', cost: 'An icon and a number to learn, and easy to miss.',
      mock: () => buttonMock('icon') },
    { key: 'B', name: '“Artifacts” and the count', from: 'new, a button like the picker beside it',
      desc: 'The icon, the word Artifacts and the count, drawn as the picker is (no border, a hover), right after it.',
      good: 'Says what it is.', cost: 'Wider; with a long project name, the title bar gets crowded.',
      mock: () => buttonMock('label') },
    { key: 'C', name: 'Joined to the picker', from: 'new',
      desc: 'The picker and the button as one bordered control: the project on the left, the artifacts icon and count on the right, so the count reads as that project’s.',
      good: 'Shows the count belongs to the picked project.', cost: 'Two controls in one shape, so a click can land on the wrong half.',
      mock: () => buttonMock('joined') },
  ],
});

// 2. What the button opens -----------------------------------------------------------------------
function buttonList({ items = STOREFRONT_ITEMS, pictures = false, tab = 'All', style = 'left:206px;top:34px', width = pictures ? 470 : 390 } = {}) {
  const list = pictures
    ? `<div class="hgrid apics">${items.map((item) => hcard(item, { machine: false })).join('')}</div>`
    : threadGroups(items).map(([thread, rows]) => groupLabel(thread) + rows.map((item) => listRow(item, { hl: item.id === 'plans' })).join('')).join('');
  return `<div class="menu apop" style="${style};width:${width}px;${pictures ? 'height:620px' : ''}">${searchField('Search storefront’s artifacts…', { w: width - 12 })}<div class="row g2" style="padding:0 4px 4px">${seg(['All', 'Documents', 'Designs'], tab)}<span class="grow"></span><span class="xs ph">${items.length} artifacts</span></div><div class="alist">${list}</div><div class="foot">${ic('arrow-up-right', 'xs')}Open the artifacts page</div></div>`;
}
TOPICS.push({
  id: 'opens', section: 'The Artifacts button', title: 'What the button opens', size: 'wide', rec: 'A',
  now: 'The shelf opens in place in the sidebar, as a list (screenshot: under All projects); a click on a row opens it in the browser, and All artifacts… at the end opens the All artifacts page. The mocks have storefront picked: 7 artifacts from its threads, two of them archived.',
  nowImg: 'img/now-app-all.png',
  issues: [],
  options: [
    { key: 'A', name: 'A list under it', from: 'agentZ’s menus (the project picker’s)',
      desc: 'A popover under the button: a search, All · Documents · Designs, then rows grouped by the thread they came from (an archived thread says so), each with its kind, title and a design’s state (Waiting for you, 2 of 3 decided, Decided), or when. A click opens it in the browser; “Open the artifacts page” ends the list. The arrow keys and Enter work, as in the picker.',
      good: 'Quick to scan; what still needs you shows on its row.', cost: 'No pictures: designs differ from documents only by their icon and state.',
      mock: () => appWin2(plansThread(), { open: true, over: buttonList() }) },
    { key: 'B', name: 'A list with pictures', from: 'claude.ai (its artifact cards)',
      desc: 'As A, but each artifact is a small card, two across, newest first: its picture with its state over it, the title, and the thread it came from.',
      good: 'Designs are easy to tell apart at a glance.', cost: 'A big popover, and four artifacts in view instead of a dozen.',
      mock: () => appWin2(plansThread(), { open: true, over: buttonList({ pictures: true }), h: 700 }) },
    { key: 'C', name: 'The artifacts page', from: 'the first round’s All artifacts',
      desc: 'The button opens the artifacts page in the browser, at the picked project (topic 4); nothing opens in the app.',
      good: 'One place for them, with room for pictures, search and filters.', cost: 'Leaves the app for a glance at what’s there.',
      mock: () => homeA({ on: 'storefront', items: STOREFRONT_ITEMS, title: 'storefront', h: 760 }) },
  ],
});

// 3. A thread's artifacts ------------------------------------------------------------------------
const pillMenu = (style = 'right:186px;top:34px') => `<div class="menu" style="${style};width:300px">${PLANS_ITEMS.map((item, index) => listRow(item, { hl: index === 0 })).join('')}</div>`;
const threadChips = () => `<div class="composer-strip">${PLANS_ITEMS.map((item) => `<span class="chip">${kindIc(item, 'xs')}<span style="color:var(--t)">${item.title}</span>${item.kind === 'design' ? stateTag(item, { short: true }) : ''}</span>`).join('')}</div>`;
TOPICS.push({
  id: 'thread', section: 'The Artifacts button', title: 'A thread’s artifacts', size: 'wide', rec: 'A',
  now: 'Once a thread has published, its header shows a pill before the branch: the artifact’s title, or with several their count, “2 artifacts”, which opens a menu of the thread’s artifacts and All artifacts… (screenshot). Each publish is also a card in the conversation. Decided: a thread lists only its own artifacts.',
  nowImg: 'img/now-pill-menu.png',
  issues: [],
  options: [
    { key: 'A', name: 'The header’s pill, only this thread’s', from: 'the first round (topic 5 B)',
      desc: 'As today, but the menu lists only this thread’s artifacts, each with a design’s state, and no All artifacts… (the Artifacts button has the rest). With one artifact, the pill is its title and opens it.',
      good: 'Nothing new to learn; out of the way.', cost: 'A design waiting for you shows only once the menu is open.',
      mock: () => jbFrame(plansThread({ headExtra: tpill('2 artifacts', { open: true }), over: pillMenu() })) },
    { key: 'B', name: 'Chips over the composer', from: 'Claude Code (the pill under the prompt)',
      desc: 'A row of chips over the composer, one per artifact of the thread: its icon, title and a design’s state. A click opens it; the header has no pill.',
      good: 'In view while you write about them, and a design waiting for you says so.', cost: 'A row above the composer in every thread that has published, which grows with each artifact.',
      mock: () => jbFrame(plansThread({ composerTop: threadChips() })) },
    { key: 'C', name: 'Only the cards', from: 'new',
      desc: 'Nothing in the header: the cards in the conversation are the thread’s list, and the Artifacts button has every one.',
      good: 'Nothing added.', cost: 'Scroll back to find an early artifact in a long thread.',
      mock: () => jbFrame(plansThread()) },
  ],
});

// 4. The page's layout ---------------------------------------------------------------------------
function homeTable() {
  const groups = [['storefront', mono('ST', 'g'), (item) => item.thread.project === 'storefront'], ['api', mono('AP', 't'), (item) => item.thread.project === 'api'], ['Chats', ic('chat', 'sm'), (item) => item.thread.chat]];
  const rows = groups.map(([name, icon, test]) => {
    const items = A2.filter(test);
    return `<tr class="grp"><td colspan="5"><span class="row g2">${icon}${name}<span class="ph" style="font-weight:400">${items.length}</span></span></td></tr>${items.map((item) => `<tr><td class="t"><span class="row g2"><span class="hrowpic">${thumbOf(item)}</span><span class="trunc" style="max-width:250px">${item.title}</span>${stateTag(item, { short: true })}${item.thread.archived ? archivedTag() : ''}</span></td><td>${KIND[item.kind].name} · v${item.version}</td><td><span class="trunc" style="display:block;max-width:250px">${item.thread.title}</span></td><td>${item.machine || 'This Mac'}</td><td>${item.ago}</td></tr>`).join('')}`;
  }).join('');
  return `<table class="htable"><tr><th>Title</th><th>Kind</th><th>From</th><th>Machine</th><th>Updated ${ic('chev-down', 'xs')}</th></tr>${rows}</table>`;
}
function homeByThread() {
  return threadGroups(A2).map(([thread, items]) => `<div class="hthread"><div class="row g2 sm">${fromLine(thread)}<span class="ph row g1 none">· ${glyph(thread.glyph, 'sm')} ${thread.agent}</span>${thread.machine ? `<span class="ph row g1 none">· ${ic('server', 'xs')} Devbox 1</span>` : ''}${thread.archived ? archivedTag() : ''}<span class="grow"></span><span class="ph xs">${items.length === 1 ? '1 artifact' : `${items.length} artifacts`}</span></div><div class="hgrid" style="grid-template-columns:repeat(5,1fr)">${items.map((item) => hcard(item, { machine: false, archived: 'none' })).join('')}</div></div>`).join('');
}
TOPICS.push({
  id: 'home', section: 'The artifacts page', title: 'The page’s layout', size: 'wide', rec: 'A',
  now: 'All artifacts is a page the server serves at <code>/</code>: a card per artifact with its kind’s icon, title, “Page · v1 · 5m ago” and where it came from, newest first, with All · Threads · Chats and a search (screenshot). No pictures, no places, every machine’s artifacts in one grid. Decided: a new page, the homepage, in the app’s theme.',
  nowImg: 'img/now-home.png',
  issues: ['Hard to find one among many, across projects and machines', 'Nothing says which designs still need you'],
  options: [
    { key: 'A', name: 'Places on the left, cards by day', from: 'claude.ai’s artifacts page, with the app’s sidebar',
      desc: 'The page’s bar (agentZ’s mark, Artifacts, the search), then places on the left with their counts: All artifacts, Waiting for you (designs not submitted), each project and Chats. On the right, a card per artifact with its picture (a design’s variants, a document’s first lines), its state over it, grouped by day: Today, Yesterday, This week, Earlier.',
      good: 'Every project a click away; pictures make designs easy to spot.', cost: 'A left list to keep up as projects come and go.',
      mock: () => homeA() },
    { key: 'B', name: 'Waiting first, then everything', from: 'claude.ai’s artifacts page',
      desc: 'No left list: the designs waiting for you in a row at the top, then everything by day. A project is picked from a menu by the search.',
      good: 'What needs you is the first thing you see.', cost: 'One project at a time only through a menu.',
      mock: () => homePage(`<div class="hmain" style="height:100%">${homeHead('Waiting for you', WAITING_ITEMS.length, { tabs: '' })}<div class="hgrid" style="grid-template-columns:repeat(5,1fr);margin:8px 0 14px">${WAITING_ITEMS.map((item) => hcard(item)).join('')}</div>${homeHead('Everything', A2.length, { right: `<span class="dd">${ic('list', 'sm')}All projects ${chev}</span>` })}${byDay(A2, { columns: 5 })}</div>`) },
    { key: 'C', name: 'A table by project', from: 'new, like Finder’s list view',
      desc: 'A row per artifact, grouped by project and Chats: a small picture, the title and state, kind and version, the thread it came from, the machine and when. A click on a column’s name sorts by it.',
      good: 'Many artifacts in view, and easy to sort.', cost: 'The pictures are too small to tell designs apart.',
      mock: () => homePage(`<div class="hlay">${homeSide()}<div class="hmain">${homeHead('All artifacts', A2.length)}${homeTable()}</div></div>`) },
    { key: 'D', name: 'By thread', from: 'new, like the sidebar’s thread cards',
      desc: 'Each thread with its artifacts in a row under it, the newest thread first, so a thread’s document and its design sit together.',
      good: 'You see what each piece of work produced.', cost: 'A thread with one artifact takes a whole row.',
      mock: () => homePage(`<div class="hlay">${homeSide()}<div class="hmain">${homeHead('All artifacts', A2.length)}${homeByThread()}</div></div>`) },
  ],
});

// 5. Finding one among many ----------------------------------------------------------------------
const FOUND = [
  { item: 'plans', line: 'B. Compare table: Every feature in one table, a column a plan, with Monthly and <span class="hmark">Yearly</span>.' },
  { item: 'pricing', line: '3. The <span class="hmark">yearly</span> discount: how much off when a shop pays for a year.' },
  { item: 'research', line: 'Shopify, Squarespace and Wix each show three or four plans, the <span class="hmark">yearly</span> price first and the most popular marked.' },
  { item: 'emails', line: 'Email 3, a week in: switching to <span class="hmark">yearly</span> saves 20%.' },
];
const foundRow = ({ item: id, line }) => {
  const item = art(id);
  return `<div class="hres"><span class="hrowpic" style="width:96px;height:54px">${thumbOf(item)}</span><div class="col grow" style="gap:3px;min-width:0"><div class="row g2">${kindIc(item)}<b class="trunc">${item.title}</b>${stateTag(item, { short: true })}<span class="grow"></span><span class="xs ph">${item.ago}</span></div><div class="sm mu">${line}</div><div class="xs ph row g1">${fromLine(item.thread)}</div></div></div>`;
};
const focusedSearch = (text, w = 300) => searchField(text, { w, typed: true }).replace('class="field hsearch"', 'class="field hsearch focus"');
const palette = () => `<div class="menu" style="left:calc(50% - 290px);top:110px;width:580px;padding:6px">${focusedSearch('chec', 566)}${['spec', 'research', 'plans'].map((id, index) => listRow(art(id), { from: true, hl: index === 0, end: `<span class="xs ph">${art(id).ago}</span>` })).join('')}<div class="row g2 xs ph" style="padding:6px 8px 2px">${keycap('↑')}${keycap('↓')} to move ${keycap('↵')} to open ${keycap('esc')} to close</div></div><div class="modal-back" style="z-index:15"></div>`;
TOPICS.push({
  id: 'find', section: 'The artifacts page', title: 'Finding one among many', size: 'wide', type: 'multi', rec: 'ABC',
  now: 'All artifacts’ search matches titles, and All · Threads · Chats narrows by where they came from. The shelf has no search. The mocks use topic 4’s A.',
  nowImg: 'img/now-home.png',
  issues: [],
  options: [
    { key: 'A', name: 'Search their words too', from: 'new',
      desc: 'The search finds words inside documents and designs (their topics’ and variants’ words, and your comments), showing the matching line under each title with the words marked.',
      good: 'Find a design by what was in it, not only its title.', cost: 'The server reads every artifact to search them.',
      mock: () => homePage(`<div class="hlay">${homeSide()}<div class="hmain">${homeHead('Results for “yearly”', FOUND.length, { tabs: '' })}<div style="margin-top:10px">${FOUND.map(foundRow).join('')}</div></div></div>`, { bar: homeBar({ search: focusedSearch('yearly') }) }) },
    { key: 'B', name: 'All · Documents · Designs', from: 'new',
      desc: 'Tabs over the cards, by kind; the count follows.',
      good: 'Every design in one view.', cost: 'Two kinds today, so the tabs do little until there are more.',
      mock: () => homeA({ items: A2.filter((item) => item.kind === 'design'), head: homeHead('All artifacts', 5, { tabs: seg(['All', 'Documents', 'Designs'], 'Designs') }) }) },
    { key: 'C', name: 'Waiting for you', from: 'new, like an inbox',
      desc: 'A place on the left for designs not yet submitted, with their count, newest first; Submit on a design takes it off.',
      good: 'What still needs a pick, from every project, in one place.', cost: 'A design you mean to leave undecided stays there.',
      mock: () => homeA({ on: 'waiting', items: WAITING_ITEMS, title: 'Waiting for you', top: '', head: `${homeHead('Waiting for you', WAITING_ITEMS.length, { tabs: '' })}<div class="sm ph" style="margin-bottom:4px">Designs you haven’t submitted, from every project and machine.</div>` }) },
    { key: 'D', name: 'Machine and agent filters', from: 'new',
      desc: 'Menus beside the tabs: Any machine, This Mac, Devbox 1; Any agent, Claude Agent, Codex, Factory Droid, OpenCode.',
      good: 'Narrow a long list fast.', cost: 'Two more controls for what the places mostly do.',
      mock: () => homeA({ items: A2.filter((item) => item.machine), head: homeHead('All artifacts', 5, { right: `${dropdown(`${ic('server', 'xs')}Devbox 1`)}${dropdown('Any agent')}` }) }) },
    { key: 'E', name: '⌘K to jump to one', from: 'agentZ’s command palette',
      desc: 'On the page, ⌘K opens a box: type part of a title, and ↵ opens the artifact.',
      good: 'The quickest way to one you know.', cost: 'Only for those who know it’s there.',
      mock: () => homeA({ over: palette() }) },
  ],
});

// 6. From an archived thread ---------------------------------------------------------------------
const storefrontHome = (cardFn, items = STOREFRONT_ITEMS, extra = '') => homePage(`<div class="hlay">${homeSide({ on: 'storefront' })}<div class="hmain">${homeHead('storefront', items.length)}${byDay(items, { cardFn, columns: 5 })}${extra}</div></div>`, { h: 1000 });
TOPICS.push({
  id: 'archived', section: 'The artifacts page', title: 'From an archived thread', size: 'wide', rec: 'A',
  now: 'Archiving a thread keeps its artifacts, and nothing marks them: the shelf and All artifacts list them like the rest. Deleting a thread keeps them too, “from a deleted thread” (the first round’s topic 14). Decided: an archived thread’s artifacts stay in the lists, marked as from an archived thread. The mocks have storefront picked: <i>Write the 0.9 release notes</i> and <i>Add the cart drawer</i> are archived.',
  issues: [],
  options: [
    { key: 'A', name: 'An Archived tag', from: 'agentZ’s Archived shelf',
      desc: 'The card says Archived beside where it came from, with the archive icon; otherwise it’s like the rest.',
      good: 'Plain, and the card is as easy to read.', cost: 'Easy to overlook among many.',
      mock: () => storefrontHome((item) => hcard(item, { archived: 'tag' })) },
    { key: 'B', name: 'Dimmed', from: 'new',
      desc: 'The card is dimmed, with the archive icon where the tag would be; it brightens on hover.',
      good: 'Current work stands out.', cost: 'A dimmed card looks unavailable, though it opens.',
      mock: () => storefrontHome((item) => hcard(item, { archived: 'dim' })) },
    { key: 'C', name: 'Gathered at the end', from: 'agentZ’s sidebar (Archived after the threads)',
      desc: 'Archived threads’ artifacts leave the days and gather at the end, under “From archived threads (2)”, closed at first as the sidebar’s Archived shelf is.',
      good: 'The days show only current work.', cost: 'One step more to reach them, and they lose their place in time.',
      mock: () => storefrontHome((item) => hcard(item, { archived: 'none' }), STOREFRONT_ITEMS.filter((item) => !item.thread.archived), `<div class="hfold">${ic('chev-right', 'xs')}<span>From archived threads</span><span class="ph">2</span><span class="rule"></span></div>`) },
  ],
});

// 7. Other machines' artifacts -------------------------------------------------------------------
const offlineBand = (text = 'Devbox 1 isn’t connected; its artifacts show once it is.') => `<div class="hband">${ic('wifi', 'sm')}<span class="grow">${text}</span><span class="btn sm">Reconnect</span></div>`;
TOPICS.push({
  id: 'machines', section: 'The artifacts page', title: 'Other machines’ artifacts', size: 'wide', rec: 'A',
  now: 'Each machine’s server keeps its own artifacts; the app gathers every connected machine’s into the shelf and All artifacts, and opens another machine’s through this Mac (<code>/m/devbox1/a/…</code>). Neither says which machine one is on, and an offline machine’s are simply missing. Decided, so not shown here: an artifact moves with its thread when the thread goes on on another machine, and the same artifact opens from any machine. In the mocks, <i>api</i> is on Devbox 1.',
  issues: [],
  options: [
    { key: 'A', name: 'The machine on its cards', from: 'agentZ’s thread cards (the machine icon)',
      desc: 'A card from another machine names it beside where it came from, as thread cards show theirs. When a machine isn’t connected, a line over the cards says so, and its artifacts aren’t listed.',
      good: 'Says where it is without another control.', cost: 'An offline machine’s artifacts disappear until it’s back.',
      mock: () => stackOf([['Devbox 1 connected', homeA()], ['Devbox 1 offline', homeA({ top: offlineBand(), items: A2.filter((item) => !item.machine), count: 10, h: 560 })]]) },
    { key: 'B', name: 'A, and machines in the places', from: 'agentZ’s Machines settings',
      desc: 'As A, and a Machines group on the left: This Mac and Devbox 1, with their counts.',
      good: 'One machine’s artifacts in a click.', cost: 'A longer left list for something rarely asked.',
      mock: () => homeA({ machines: true, on: 'devbox', items: A2.filter((item) => item.machine), title: 'Devbox 1' }) },
    { key: 'C', name: 'Kept while offline', from: 'new',
      desc: 'As A, but an offline machine’s artifacts stay, from the last list the app had, greyed; a click says it opens once the machine is back.',
      good: 'The list doesn’t change shape when a machine drops.', cost: 'Cards you can’t open, and a list that may be out of date.',
      mock: () => homeA({ top: offlineBand('Devbox 1 is offline. Its artifacts are from the last list, 2h ago, and open once it’s back.'), cardFn: (item) => hcard(item, { dim: !!item.machine }) }) },
  ],
});

// 8. A document's page ---------------------------------------------------------------------------
const RESEARCH_MD = `<h1>Competitors’ plans pages</h1>
<p>How three site builders show their plans. Shopify, Squarespace and Wix each show three or four plans, the yearly price first and the most popular marked.</p>
<h2>Shopify</h2>
<p>Four plans in cards. The yearly price is the default, with what it saves beside the switch, and one plan is marked most popular.</p>
<ul><li>A compare table under the cards, a feature a row.</li><li>Questions at the end: fees, trials, changing plans.</li></ul>
<h2>Squarespace</h2>
<p>Four plans in one row, the price large and “billed yearly” under it. No compare table: each card lists its features.</p>
<h2>Wix</h2>
<p>Plans by what you sell, in tabs, then three cards in each.</p>
<h2>What they share</h2>
<table><tr><th>Site</th><th>Plans</th><th>Shown first</th><th>Most popular marked</th></tr><tr><td>Shopify</td><td>4</td><td>Yearly</td><td>Yes</td></tr><tr><td>Squarespace</td><td>4</td><td>Yearly</td><td>No</td></tr><tr><td>Wix</td><td>3</td><td>Yearly</td><td>Yes</td></tr></table>
<h2>For storefront</h2>
<ul><li>Three plans, Growth marked most popular.</li><li>Yearly first, with what it saves.</li></ul>`;
const RESEARCH_TOC = ['Shopify', 'Squarespace', 'Wix', 'What they share', 'For storefront'];
const docBar = (item = art('research')) => `<div class="phd">${ibtn('home')}<span class="ph">/</span><span class="ptitle">${item.title}</span><span class="vtag">v${item.version}</span>${chev}<span class="sm ph trunc row g1" style="margin-left:4px">${glyph(item.thread.glyph, 'sm')} ${fromText(item.thread)}</span><span class="grow"></span><span class="btn">${ic('download', 'xs')}Export${chev}</span></div>`;
const docPage = (body) => br2(page(docBar(), body), { title: 'Competitors’ plans pages', icon: 'file', url: '127.0.0.1:47120/a/91c2e7' });
const detail = (label, rows) => `<div><h6>${label}</h6>${rows.map((row) => `<div class="v">${row}</div>`).join('')}</div>`;
TOPICS.push({
  id: 'doc', section: 'Documents', title: 'A document’s page', size: 'wide', rec: 'B',
  now: 'A Markdown artifact opens as a document page (the first round’s topic 13 B): 15 px text with room between lines, a larger title, ruled tables, in a 720 px column, under the page’s bar with home, the title, the version menu, where it came from, Files, Export and Send to thread (screenshot). Decided: a document is Markdown only, and read-only: no comments.',
  nowImg: 'img/now-doc.png',
  issues: ['Send to thread has nothing to send from a document'],
  options: [
    { key: 'A', name: 'Today’s column, without Send to thread', from: 'the first round (topic 13 B)',
      desc: 'As today, but the bar has only the version menu, where it came from, and Export (Markdown, Print or Save as PDF, Copy Source). To talk about it, mention it in the thread (topic 20).',
      good: 'A page for reading and nothing else.', cost: 'A long document is a long scroll.',
      mock: () => docPage(`<div class="pin doc" style="max-width:784px">${RESEARCH_MD}</div>`) },
    { key: 'B', name: 'With an outline', from: 'Zed’s outline panel, and documentation sites',
      desc: 'As A, with its headings on the left, the one in view marked; a click scrolls to it. A document with fewer than three headings has none.',
      good: 'See a long document’s shape, and jump in it.', cost: 'A column of its own on a wide window.',
      mock: () => docPage(`<div class="doclay"><div class="toc"><span class="xs ph">On this page</span>${RESEARCH_TOC.map((heading, index) => `<span class="${index === 0 ? 'on' : ''}">${heading}</span>`).join('')}</div><div class="pin doc">${RESEARCH_MD}</div><div style="width:200px;flex:none"></div></div>`) },
    { key: 'C', name: 'With its details', from: 'new, like a file’s Get Info',
      desc: 'As A, with what it is on the right: where it came from (opening the thread), the agent, the machine, its versions, and the threads that mention it.',
      good: 'Everything about it in view, and a way back to where it came up.', cost: 'Details you rarely need take the right side.',
      mock: () => docPage(`<div class="doclay"><div style="width:240px;flex:none"></div><div class="pin doc">${RESEARCH_MD}</div><div class="drail">${detail('From', [`${mono('ST', 'g')}<span class="ac trunc">storefront › Design the plans page</span>`])}${detail('Agent', [`${glyph('claude', 'sm')}Claude Agent · Opus 5.5`])}${detail('Machine', [`${ic('laptop', 'xs')}This Mac`])}${detail('Versions', ['<span class="vtag">v1</span>10m ago'])}${detail('Mentioned in', [`${ic('board', 'xs')}<span class="trunc">Plans page (its design)</span>`, `${mono('ST', 'g')}<span class="trunc">Design the pricing</span>`])}</div></div>`) },
  ],
});

// 9. A design with several topics ----------------------------------------------------------------
const [VA, VB, VC, VD] = VARIANTS;
const fitImg = (src, extra = '') => `<div class="bd-fit"><img src="${src}">${extra}</div>`;
const lightMock = (inner) => `<div style="background:#fff;color:#16181d;font:13px/1.4 -apple-system,BlinkMacSystemFont,sans-serif;padding:16px 18px;height:178px;display:flex;flex-direction:column;gap:8px">${inner}</div>`;
const pickerRow = (name, price, on) => `<div style="display:flex;align-items:center;gap:10px;border:1px solid ${on ? '#4f46e5' : '#e6e8ec'};border-radius:9px;padding:8px 10px"><span style="width:14px;height:14px;border-radius:50%;border:${on ? '4px solid #4f46e5' : '1.5px solid #b8bcc6'}"></span><b style="font-weight:600">${name}</b><span style="flex:1"></span>${price}</div>`;
const PICKERS = [
  { key: 'A', name: 'Radio list', desc: 'The three plans as rows, the price at the end of each.', good: 'All three in view.', cost: 'Tall on a phone.', html: lightMock(`<b>Your plan</b>${pickerRow('Starter', '$0', false)}${pickerRow('Growth', '$29 / mo', true)}${pickerRow('Scale', '$99 / mo', false)}`) },
  { key: 'B', name: 'Segmented', desc: 'Starter · Growth · Scale in one control, what you get under it.', good: 'Short.', cost: 'The others’ prices are a click away.', html: lightMock(`<b>Your plan</b><div style="display:flex;background:#f0f1f4;border-radius:9px;padding:3px">${['Starter', 'Growth', 'Scale'].map((name) => `<span style="flex:1;text-align:center;padding:6px;border-radius:7px;${name === 'Growth' ? 'background:#fff;box-shadow:0 1px 3px rgba(0,0,0,.15);font-weight:600' : 'color:#5b616e'}">${name}</span>`).join('')}</div><div style="font-size:22px;font-weight:700">$29<small style="font-size:13px;color:#7a808c;font-weight:500"> / month</small></div><div style="color:#5b616e">3 stores, unlimited products, discount codes</div>`) },
  { key: 'C', name: 'Dropdown', desc: 'A menu like the address form’s fields.', good: 'The shortest.', cost: 'Hides that there are other plans.', html: lightMock(`<b>Your plan</b><div style="display:flex;align-items:center;border:1px solid #d9dce2;border-radius:9px;padding:9px 12px"><span>Growth · $29 / month</span><span style="flex:1"></span>▾</div><div style="color:#7a808c">Change it any time from your shop’s settings.</div>`) },
];
const plansTopic = (crumb = 'Pricing · 1 of 3') => topicHead({ crumb }) + grid(variantCards({ picked: 'B', comments: { B: 'Yes, and make Yearly the default.' } }));
TOPICS.push({
  id: 'topics', section: 'Designs', title: 'A design with several topics', size: 'wide', rec: 'D',
  now: 'There are no designs yet: an agent that wants you to pick writes a whole page itself, or publishes each variant as its own artifact. The design board, the model here, lists a round’s topics on the left by section, each with what you picked (a green letter) and a dot when there’s a comment; ← → move between them, and Summary and How this works end the list. Most designs will have one topic: a few variants of one thing. The mocks’ bigger design, Pricing, has three: the plans page, the plan picker in checkout, and the yearly discount.',
  issues: [],
  options: [
    { key: 'A', name: 'A list on the left', from: 'the design board',
      desc: 'Topics on the left with what you picked, Summary at the end, on every design, even one with a single topic.',
      good: 'As the design board: every topic and your picks in view.', cost: 'A list of one on most designs.',
      mock: () => pricingDesign({ body: plansTopic() }) },
    { key: 'B', name: 'Steps across the top', from: 'new, like a checkout’s steps',
      desc: 'The topics as steps over the page, each ticked with your pick once decided; Back and Next at the bottom, and Summary as the last step.',
      good: 'Walks you through in order; reads well for two or three topics.', cost: 'Long titles get cut; ten topics don’t fit.',
      mock: () => pricingDesign({ nav: false, body: `<div class="bd-steps">${PRICING_TOPICS.map((topic) => `<span class="bd-step ${topic.n === 1 ? 'on' : ''} ${topic.label ? 'done' : ''}"><span class="dotn">${topic.label ? '✓' : topic.n}</span>${topic.title}${topic.label ? ` · ${topic.label}` : ''}</span><span class="ph">${ic('chev-right', 'xs')}</span>`).join('')}<span class="bd-step"><span class="dotn">∑</span>Summary</span></div>${plansTopic()}` }) },
    { key: 'C', name: 'All on one page', from: 'new',
      desc: 'Every topic one after another on one long page, each under its heading; the bar counts what’s decided.',
      good: 'Nothing to navigate: scroll and pick.', cost: 'A long page, and harder to see what’s left.',
      mock: () => pricingDesign({ nav: false, body: `<div style="margin-top:-500px;display:flex;flex-direction:column;gap:12px">${grid(variantCards({ picked: 'B' }))}<div class="bd-section">2 · The plan picker in checkout</div><div class="bd-kind" style="margin-top:0"><b>Pick one</b>, then <b>+ Also</b> on any others.</div>${grid(PICKERS.map((picker) => card(picker, { picked: picker.key === 'A', rec: picker.key === 'A', preview: `<div class="bd-fit">${picker.html}</div>` })).join(''))}</div>` }) },
    { key: 'D', name: 'A list only with two or more', from: 'the design board, without the list for one topic',
      desc: 'A design with one topic is just that topic; with two or more, the list on the left as A, Summary at its end.',
      good: 'Nothing extra on the usual design; the list when it helps.', cost: 'Designs look different by their size.',
      mock: () => stackOf([['One topic', plansDesign({ h: 520 })], ['Three topics', pricingDesign({ body: plansTopic(), h: 520 })]]) },
  ],
});

// 10. How the variants are laid out --------------------------------------------------------------
const panelFor = (variant, { picked = false, comment = '' } = {}) => `<div class="col g2" style="min-width:0">
  <div class="bd-oh"><span class="bd-letter" style="${picked ? 'background:var(--ok);color:var(--ed)' : ''}">${variant.key}</span><span class="bd-oname">${variant.name}</span>${variant.key === 'B' ? '<span class="bd-tag rec">Recommended</span>' : ''}</div>
  <p class="bd-desc">${variant.desc}</p><div class="bd-pc"><b>Good</b><span>${variant.good}</span><b>Cost</b><span>${variant.cost}</span></div>
  <div class="row g1"><span class="bd-pick" style="${picked ? 'background:var(--ok);border-color:var(--ok);color:var(--ed)' : ''}">${picked ? '✓ Picked' : `Pick ${variant.key}`}</span>${picked ? '' : '<span class="bd-also">+ Also</span>'}</div>
  <div class="bd-cm ${comment ? 'typed' : ''}" style="min-height:56px">${comment || `Comment on ${variant.key}…`}</div></div>`;
const strip = (on, picked = '') => `<div class="bd-strip">${VARIANTS.map((variant) => `<div class="th ${variant.key === on ? 'on' : ''} ${variant.key === picked ? 'picked' : ''}"><img src="${variant.img}"><span>${variant.key}</span></div>`).join('')}</div>`;
const arrows = () => `<span class="bd-arrow" style="left:10px">${ic('chev-left', 'sm')}</span><span class="bd-arrow" style="right:10px">${ic('chev-right', 'sm')}</span>`;
const PICKED_B = { picked: 'B', comments: { B: 'Yes, and make Yearly the default.' } };
const oneDecided = () => designBar({ decided: 1 });
TOPICS.push({
  id: 'variants', section: 'Designs', title: 'How the variants are laid out', size: 'wide', rec: 'A',
  now: 'The design board puts a topic’s options in cards, in columns as wide as the topic asks: each with its letter, name, description, Good and Cost, the mock, Pick, + Also and a comment (screenshot: the first artifacts round). The mocks have five variants at 1280×800 in a 1280 px browser.',
  nowImg: 'img/now-local-board.png',
  issues: ['A whole page shrunk to a third of the screen is hard to read'],
  options: [
    { key: 'A', name: 'Cards in a grid', from: 'the design board',
      desc: 'Three cards across (two in a narrow window), each with everything the design board’s cards have. A click on a picture shows it up close (topic 12).',
      good: 'Every variant in view at once, and the board you know.', cost: 'Pictures a third of the width; reading one means opening it.',
      mock: () => plansDesign({ bar: oneDecided(), body: topicHead() + grid(variantCards(PICKED_B)) }) },
    { key: 'B', name: 'One at a time, with a strip', from: 'new, like a photo viewer',
      desc: 'One variant large with its words, Pick and comment beside it; a strip of the others under it, and ← → to move. The strip marks the picked one.',
      good: 'Each variant big enough to read.', cost: 'One at a time, so comparing means flipping.',
      mock: () => plansDesign({ bar: oneDecided(), body: `${topicHead({ kind: '<b>Pick one</b>, then <b>+ Also</b> on any others. Keys: <b>← →</b> variants, <b>1–5</b> pick.' })}<div style="display:grid;grid-template-columns:720px 300px;gap:18px;align-items:start;justify-content:center;margin-bottom:12px"><div style="position:relative">${fitImg(VB.img)}${arrows()}</div>${panelFor(VB, { picked: true, comment: 'Yes, and make Yearly the default.' })}</div>${strip('B', 'B')}` }) },
    { key: 'C', name: 'Two side by side', from: 'new',
      desc: 'Two variants at half the width each, with a menu over each to choose which; they scroll together. Pick under either.',
      good: 'The best for weighing two finalists.', cost: 'Two at a time; you choose the pair first.',
      mock: () => plansDesign({ bar: oneDecided(), body: `${topicHead()}<div class="row g2 sm mu">Compare<span class="bd-tbtn">B. Compare table ${chev}</span>with<span class="bd-tbtn">C. Slider by size ${chev}</span></div>${grid(card(VB, { rec: true, picked: true }) + card(VC, { also: true, comment: 'The slider, for shops over 1,000 products.' }), 2)}` }) },
    { key: 'D', name: 'A row each', from: 'new',
      desc: 'One variant a row: its picture at half the width on the left, its words, Pick and comment on the right. Scroll down for the next.',
      good: 'Big pictures, with each variant’s words beside it.', cost: 'Two variants a screen; a long scroll for five.',
      mock: () => plansDesign({ bar: oneDecided(), body: topicHead() + VARIANTS.slice(0, 2).map((variant) => `<div class="bd-opt ${variant.key === 'B' ? 'picked' : ''}" style="display:grid;grid-template-columns:560px 1fr;gap:16px;align-items:start">${fitImg(variant.img)}${panelFor(variant, { picked: variant.key === 'B' })}</div>`).join('') }) },
  ],
});

// 11. Today's version ----------------------------------------------------------------------------
const closeUpDesign = (stage, panel, bar = oneDecided()) => designPage({ bar, inner: `<div class="bd"><div class="bd-body"><div style="flex:1;min-width:0;position:relative;background:#111214;display:grid;place-items:center;padding:18px">${stage}</div>${panel}</div></div>` });
TOPICS.push({
  id: 'original', section: 'Designs', title: 'Today’s version', size: 'wide', rec: 'B',
  now: 'An agent says what’s there today in its own words, if at all. The design board opens each topic with a Today box: what’s there now, with a screenshot and its problems (topic 10’s screenshot). Decided: including today’s version is one of the design defaults (topic 21). In the mocks, today’s plans page is a plain table.',
  issues: ['Keeping what’s there is a real choice, but nothing lets you pick it'],
  options: [
    { key: 'A', name: 'A Today box over the variants', from: 'the design board',
      desc: 'Today’s words and picture in a box over the cards; “None of these” keeps it.',
      good: 'What you’re changing from is the first thing you read.', cost: 'Today can’t be picked, compared or commented on like a variant.',
      mock: () => plansDesign({ body: topicHead() + todayBox() + grid(variantCards()) }) },
    { key: 'B', name: 'Today as a variant to keep', from: 'new',
      desc: 'Today’s version is the first card, marked Today, with its words, Good and Cost; its button says Keep today’s, and + Also on a variant takes parts of it into today’s.',
      good: 'Keeping today’s is a pick like any other, side by side with the rest.', cost: 'One more card in the grid.',
      mock: () => plansDesign({ body: topicHead({ kind: '<b>Pick one</b>, or keep today’s, then <b>+ Also</b> on any others. Keys: <b>0–5</b> pick.' }) + grid(variantCards({}, [TODAY_VARIANT, ...VARIANTS])), h: 1060 }) },
    { key: 'C', name: 'Today only up close', from: 'new',
      desc: 'Not on the cards: up close (topic 12), each variant shows beside today’s, so you see what it changes.',
      good: 'The cards stay about the variants; the comparison is where you look closely.', cost: 'Today is hidden until you open one.',
      mock: () => closeUpDesign(`<div style="display:grid;grid-template-columns:1fr 1fr;gap:14px;width:100%"><div class="col g1"><span class="xs ph">${ic('history', 'xs')} Today</span>${fitImg(TODAY_VARIANT.img)}</div><div class="col g1"><span class="xs ph">B. Compare table</span>${fitImg(VB.img)}</div></div>`, `<div class="bd-panel">${panelFor(VB, { picked: true, comment: 'Yes, and make Yearly the default.' })}</div>`) },
  ],
});

// 12. One variant up close -----------------------------------------------------------------------
TOPICS.push({
  id: 'closeup', section: 'Designs', title: 'One variant up close', size: 'wide', rec: 'B',
  now: 'On the design board, a click on a mock opens it full size over the board with its caption; Esc or a click closes it. There are no arrows to the next one, and you pick and comment only on the card. A variant published as its own artifact today opens as a whole page, with nothing to pick.',
  issues: [],
  options: [
    { key: 'A', name: 'Full size over the design', from: 'the design board',
      desc: 'A click shows the picture (or runs the page) at full size over the design, its name under it. ← → go to the next variant; Esc closes.',
      good: 'As the design board, with arrows.', cost: 'To pick or comment, close it first.',
      mock: () => plansDesign({ over: `<div class="bd-over" style="z-index:15"><div style="position:relative"><img src="${VB.img}" style="width:1020px;display:block;border-radius:6px">${arrows()}</div><div class="cap">B. Compare table · ← → the other variants · Esc closes</div></div>` }) },
    { key: 'B', name: 'Up close, with its words and Pick', from: 'new',
      desc: 'A click fills the design with the variant, and its words, Pick, + Also and comment in a panel at the right. ← → go through the variants, Esc goes back to the cards. A live page runs here, to scroll and click.',
      good: 'Read, decide and comment without going back.', cost: 'A second place to pick, besides the cards.',
      mock: () => closeUpDesign(`<div style="position:relative;width:100%"><img src="${VB.img}" style="width:100%;display:block;border-radius:6px">${arrows()}<span class="bd-hint" style="bottom:8px;left:8px">2 of 5 · Esc back to the variants</span></div>`, `<div class="bd-panel">${panelFor(VB, { picked: true, comment: 'Yes, and make Yearly the default.' })}</div>`) },
    { key: 'C', name: 'In a tab of its own', from: 'new',
      desc: 'A click opens the variant in a new tab, at the browser’s full width, under a thin bar: the design’s name, the variant, ‹ ›, Pick and a comment. Your pick shows on the design’s tab too.',
      good: 'The variant as it will really be, with the browser’s own zoom and sizes.', cost: 'Tabs to keep track of.',
      mock: () => br2(`<div class="pg"><div class="phd">${ibtn('chev-left')}<span class="ph">Plans page /</span><span class="ptitle">B. Compare table</span><span class="ph sm">2 of 5</span>${ibtn('chev-left')}${ibtn('chev-right')}<span class="grow"></span><span class="btn" style="background:var(--ok);border-color:var(--ok);color:var(--ed)">✓ Picked</span><span class="btn ghost">${ic('chat', 'xs')}Comment</span></div><div class="pbody"><img src="${VB.img}" style="width:100%;display:block"></div></div>`, { title: 'B. Compare table', icon: 'board', url: `${PLANS_URL}/B`, tabs: `<div class="brtab">${ic('board', 'xs')}<span class="grow trunc">Plans page</span>${ic('x', 'xs')}</div>` }) },
  ],
});

// 13. Phone and desktop --------------------------------------------------------------------------
const submitButton = (primary = false) => `<span class="btn ${primary ? 'primary' : ''}">${ic('check', 'xs')}Submit</span>`;
TOPICS.push({
  id: 'sizes', section: 'Designs', title: 'Phone and desktop', size: 'wide', rec: 'B',
  now: 'Nothing for sizes. The five variants are desktop pages, 1280 px wide; a phone version would be another page. The design board draws each mock at one size, and a round that cares about narrow windows mocks them as separate options. Decided: phone, desktop or both is one of the design defaults (topic 21). In the mocks, the agent made phone versions of A and D.',
  issues: ['A variant that works on a desktop can fall apart on a phone, and its picture doesn’t show it'],
  options: [
    { key: 'A', name: 'Both in the card', from: 'new',
      desc: 'A variant can have a phone version too. The card shows the phone over the desktop picture’s corner; up close shows both side by side.',
      good: 'Both sizes at a glance, on one card.', cost: 'The phone is small in its corner, and covers part of the desktop.',
      mock: () => plansDesign({ body: topicHead() + grid(variantCards({ each: (variant) => ({ preview: fitImg(variant.img, PHONES[variant.key] ? `<div class="bd-phone"><img src="${PHONES[variant.key]}"></div>` : '') }) })) }) },
    { key: 'B', name: 'A switch for the whole design', from: 'new, like the browser’s device toolbar',
      desc: 'With phone versions in the design, its bar has Desktop · Phone, which swaps every card; phone pictures are laid out five across. A variant without one says “No phone version”.',
      good: 'Every variant at one size, at a size you can read.', cost: 'One size at a time.',
      mock: () => plansDesign({ bar: designBar({ buttons: `${seg(['Desktop', 'Phone'], 'Phone')}${submitButton()}` }), body: topicHead() + grid(VARIANTS.map((variant) => card(variant, { rec: variant.key === 'B', desc: false, pc: false, foot: false, preview: `${PHONES[variant.key] ? fitImg(PHONES[variant.key]) : '<div class="bd-none">No phone version</div>'}<div class="bd-foot"><span class="bd-pick">Pick ${variant.key}</span><span class="bd-also">+ Also</span></div><span class="bd-cm">Comment on ${variant.key}…</span>` })).join(''), 5) }) },
    { key: 'C', name: 'Tabs on each card', from: 'new',
      desc: 'A card with a phone version has Desktop · Phone over its picture; each card keeps its own.',
      good: 'Look at one variant’s phone without changing the others.', cost: 'A click per card, and cards of different heights.',
      mock: () => plansDesign({ body: topicHead() + grid(variantCards({ each: (variant) => (variant.key === 'A' ? { tags: `<span class="grow"></span>${seg(['Desktop', 'Phone'], 'Phone')}`, preview: `<div class="bd-fit" style="display:grid;place-items:center;padding:8px 0"><img src="${PHONES.A}" style="width:46%"></div>` } : PHONES[variant.key] ? { tags: `<span class="grow"></span>${seg(['Desktop', 'Phone'], 'Desktop')}` } : {}) })) }) },
  ],
});

// 14. Pictures and live pages --------------------------------------------------------------------
const hint = (text, where = 'top:8px;right:8px') => `<span class="bd-hint" style="${where}">${text}</span>`;
const variantsDesign = (options = {}) => plansDesign({ body: topicHead() + grid(variantCards(options)) });
TOPICS.push({
  id: 'live', section: 'Designs', title: 'Pictures and live pages', size: 'wide', rec: 'C',
  now: 'An artifact is one self-contained file, a page or a document, 16 MiB at most, with its images embedded. Each of the five demo variants is a whole page built with the site’s own styles (screenshot: variant A published as its own page). On the design board, an option is a mock drawn in HTML at its real size and zoomed to fit its card. Decided: the agent gives pictures or live pages, and which is one of the design defaults (topic 21).',
  nowImg: 'img/now-variant.png',
  issues: ['A picture can’t show a slider sliding or a menu opening; five running pages are heavy'],
  options: [
    { key: 'A', name: 'Pictures only', from: 'new',
      desc: 'The agent gives a PNG or JPG for each variant: a screenshot of what it built, or a picture it drew. The card shows it fitted to its width; up close shows it full size.',
      good: 'Light, loads at once, and looks the same in every browser.', cost: 'Nothing to click or scroll; a long page is a tall picture.',
      mock: () => variantsDesign() },
    { key: 'B', name: 'Live pages only', from: 'the design board’s mocks, which are HTML',
      desc: 'The agent gives an HTML file for each variant. The card runs it, zoomed to fit like the design board’s mocks: you can scroll it, open its menus and drag C’s slider right in the card.',
      good: 'Hover, menus and sliders work as they will.', cost: 'Five pages running at once, each self-contained.',
      mock: () => variantsDesign({ each: (variant) => ({ preview: fitImg(variant.img, variant.key === 'C' ? hint(`${ic('move', 'xs')}Drag the slider`) : hint('Live')) }) }) },
    { key: 'C', name: 'A picture, a page, or both', from: 'new',
      desc: 'Each variant is a picture, a live page, or both. The card shows the picture (or the page, if that’s all there is); a variant with a page has “Open page”, which runs it full size up close (topic 12) to scroll and click through.',
      good: 'A light design, and the real thing a click away.', cost: 'Two things for the agent to keep alike when it gives both.',
      mock: () => variantsDesign({ each: (variant) => ({ preview: fitImg(variant.img, variant.key === 'D' ? '' : hint(`${ic('external', 'xs')}Open page`, 'bottom:8px;right:8px')) }) }) },
    { key: 'D', name: 'Pages, and agentZ takes their pictures', from: 't3code (the server renders a picture of the page)',
      desc: 'The agent gives only pages. agentZ takes a picture of each in a headless browser for its card, and runs the page when you open it.',
      good: 'One thing for the agent to make.', cost: 'A headless browser on every machine with a server, and a few seconds per variant.',
      mock: () => variantsDesign({ each: (variant) => ({ preview: fitImg(variant.img, `${variant.key === 'A' ? hint(`${ic('eye', 'xs')}Picture taken by agentZ`, 'top:8px;left:8px') : ''}${hint(`${ic('external', 'xs')}Open page`, 'bottom:8px;right:8px')}`) }) }) },
  ],
});

// 15. What the page says about saving ------------------------------------------------------------
const PICKS = { picked: 'B', also: ['C'], comments: { B: 'Yes, and make Yearly the default.', C: 'The slider, for shops over 1,000 products.' } };
const pickedDesign = ({ saved = 'Saved', bar, over = '' } = {}) => plansDesign({ bar: bar ?? designBar({ decided: 1, saved }), over, body: topicHead() + grid(variantCards(PICKS)) });
TOPICS.push({
  id: 'saving', section: 'Picks and Submit', title: 'What the page says about saving', size: 'wide', rec: 'C',
  now: 'There are no designs yet. The design board’s server writes your picks to <code>choices.json</code> and <code>decisions.md</code> as you type, and its bar says “Saved”. An artifact page runs sandboxed with no storage, so a design an agent writes as a page keeps your picks only in its tab (“Kept until you reload”, screenshot), and <code>agentz_artifact_read</code> gives the agent the page’s source, not your picks. Decided: picks and comments are saved on the server with the design, and the agent can read them whenever it likes.',
  nowImg: 'img/now-board-artifact.png',
  issues: ['Today a reload loses every pick on a design an agent wrote as a page'],
  options: [
    { key: 'A', name: 'Saved', from: 'the design board',
      desc: 'The server keeps the picks with the design as you go, and the bar says “Saved”. A reload, another browser, or the design opened from another machine shows them.',
      good: 'Quiet, as the design board.', cost: 'Doesn’t say the agent can read them.',
      mock: () => pickedDesign() },
    { key: 'B', name: 'Saved, for the agent', from: 'new',
      desc: 'As A, saying who can read them: “Saved · Claude Agent can read your picks”.',
      good: 'You know a comment reaches the agent without sending it.', cost: 'A longer line.',
      mock: () => pickedDesign({ saved: 'Saved · Claude Agent can read your picks' }) },
    { key: 'C', name: 'Saved, and when the agent read them', from: 'new',
      desc: 'As B, and once the agent has read them: “Read by Claude Agent 4m ago”, or after you change something, “Saved · changed since Claude Agent read them 4m ago”.',
      good: 'You see whether the agent has your latest.', cost: 'The server notes each read.',
      mock: () => pickedDesign({ saved: 'Saved · changed since Claude Agent read them 4m ago' }) },
  ],
});

// 16. Submit -------------------------------------------------------------------------------------
const submitBox = ({ title = 'Plans page', thread = 'Design the plans page', lines = ['<span><b>The plans page:</b> B. Compare table, also C. Slider by size</span>', '<span>2 comments</span>'] } = {}) => `<div class="subpop" style="right:16px;top:44px"><h4>Submit ${title}</h4><div class="picks">${lines.join('')}</div><div class="row g2">${sw(true)}<span class="grow">Also send to <b style="font-weight:500">${thread}</b></span></div><div class="ta">A note to go with it…</div><div class="row g2" style="justify-content:flex-end"><span class="btn">Cancel</span><span class="btn primary">${ic('check', 'xs')}Submit</span></div></div>`;
const decidedButton = (when = '1m ago') => `<span class="stag2 ok" style="height:24px;padding:0 9px">${ic('check', 'xs')}Decided ${when}</span>`;
const decidedBanner = (text = 'Decided 1m ago, and sent to “Design the plans page”. Change a pick and Submit comes back.') => `<div class="bd-banner ok" style="margin-bottom:12px">${ic('check-circle', 'sm')}<span class="grow">${text}</span></div>`;
const SUMMARY_TOPICS = PRICING_TOPICS.map((topic) => (topic.n === 3 ? { ...topic, label: 'B' } : topic));
const summaryTable = () => `<table class="bd-table"><tr><th>Topic</th><th>Pick</th><th>Comments</th></tr>
  <tr><td>1 · The plans page</td><td>B. Compare table, also C. Slider by size</td><td>B: Yes, and make Yearly the default.</td></tr>
  <tr><td>2 · The plan picker in checkout</td><td>A. Radio list</td><td class="ph">None</td></tr>
  <tr><td>3 · The yearly discount</td><td>B. 20% off</td><td>Say what it saves in dollars.</td></tr></table>`;
TOPICS.push({
  id: 'submit', section: 'Picks and Submit', title: 'Submit', size: 'wide', rec: 'B',
  now: 'Every artifact page has Send to thread in its bar: a box with what the page gave (its <code>window.agentzPicks()</code>) and a note, and Send puts both in the thread as your message (screenshot: a design page, which gives nothing). Nothing marks a page as done. Decided: Submit, which you don’t have to use, marks a design decided and can also send it to the thread.',
  nowImg: 'img/now-send.png',
  issues: ['Nothing says a design is done, so it looks open forever'],
  options: [
    { key: 'A', name: 'Submit in the bar', from: 'artifacts (Send to thread)',
      desc: 'Submit in the bar opens a box with your picks, “Also send to Design the plans page” (on at first) and a note. Afterwards the design is Decided: in the bar, on its card in the thread, in the Artifacts button’s list and on the artifacts page. Changing a pick brings Submit back.',
      good: 'One button, always in the same place; sending is part of it, not a second step.', cost: 'Nothing points you to it once you’ve picked.',
      mock: () => stackOf([['Submit', pickedDesign({ over: submitBox() })], ['Afterwards', plansDesign({ h: 560, bar: designBar({ decided: 1, saved: '', buttons: decidedButton() }), body: decidedBanner() + topicHead() + grid(variantCards(PICKS)) })]]) },
    { key: 'B', name: 'A, and a bar once every topic has a pick', from: 'new',
      desc: 'As A, and once every topic has a pick, a bar at the bottom: “Every topic has a pick. Submit to mark Plans page decided”, with Not yet and Submit…, which opens the same box. Not yet hides it until you change a pick.',
      good: 'Says when you’re done and what’s next.', cost: 'Covers the bottom of the page until you submit or hide it.',
      mock: () => pickedDesign({ over: `<div class="bd-donebar">${ic('check-circle', 'sm')}<span class="grow"><b>Every topic has a pick.</b> Submit to mark Plans page decided, and send it to Claude Agent if you like.</span><span class="btn">Not yet</span><span class="btn primary">${ic('check', 'xs')}Submit…</span></div>` }) },
    { key: 'C', name: 'Submit at the end of Summary', from: 'the design board (its Summary)',
      desc: 'No Submit in the bar: Summary, the last item on the left (and on a one-topic design, a link under the variants), lists each topic’s pick and comments, then the switch, the note and Submit.',
      good: 'You see everything you’re submitting as you submit it.', cost: 'A step away; on a one-topic design, Summary is a page of one row.',
      mock: () => pricingDesign({ on: 'summary', topics: SUMMARY_TOPICS, bar: pricingBar(3, { buttons: '' }), body: `${topicHead({ title: 'Summary', kind: 'What you picked in each topic, and your comments.' })}${summaryTable()}<div class="bd-opt" style="margin-top:16px;max-width:560px"><div class="row g2">${sw(true)}<span class="grow">Also send to <b style="font-weight:500">Design the pricing</b></span></div><div class="ta">A note to go with it…</div><div class="row g2" style="justify-content:flex-end"><span class="btn primary">${ic('check', 'xs')}Submit Pricing</span></div></div>` }) },
  ],
});

// 17. What Submit sends to the thread ------------------------------------------------------------
const youSent = (html) => `<div style="align-self:flex-end;max-width:84%;margin:2px 0 10px">${html}</div>`;
const sentChip = () => `<span class="chip" style="margin-bottom:6px">${ic('board', 'xs')}<span style="color:var(--t)">Plans page · v1</span></span>`;
const SENT_TEXT = 'The plans page: B. Compare table, also C. Slider by size\nB: Yes, and make Yearly the default.\nC: The slider, for shops over 1,000 products.';
const readRow = () => ownRow('Read a design', 'Plans page', '<span class="vtag">v1</span>');
TOPICS.push({
  id: 'send', section: 'Picks and Submit', title: 'What Submit sends to the thread', size: 'wide', rec: 'C',
  now: 'Send to thread puts the page’s text and your note in the thread as your message, behind a chip naming the page and its version; the agent gets the text as it is. With “Also send” on, Submit (topic 16) does the sending here.',
  issues: [],
  options: [
    { key: 'A', name: 'The picks as text', from: 'artifacts (Send to thread)',
      desc: 'Your message is the picks, the comments and your note as text, under a chip naming the design. The agent gets the same text.',
      good: 'The agent has everything without another call.', cost: 'A long message for a big design, and two copies of the picks.',
      mock: () => jbFrame(plansThread({ extra: youSent(`<div class="bub" style="margin:0;max-width:none">${sentChip()}<div style="white-space:pre-line">${SENT_TEXT}\nGo with the table.</div></div>`) }), { h: 620 }) },
    { key: 'B', name: 'A short message; the agent reads the rest', from: 'new',
      desc: 'Your message says what you picked and your note, under the chip. The agent gets a short reference to the design, reads the picks and comments with <code>agentz_artifact_read</code> (topic 24), and the thread shows it did.',
      good: 'Short in the thread; the picks have one home, on the server.', cost: 'Another call before it starts.',
      mock: () => jbFrame(plansThread({ extra: youSent(`<div class="bub" style="margin:0;max-width:none">${sentChip()}<div>Submitted: picked B, also C, with 2 comments. Go with the table.</div></div>`) + readRow() }), { h: 620 }) },
    { key: 'C', name: 'A card with the pictures', from: 'new',
      desc: 'Your message shows a card: the design’s title, its variants’ pictures with the picked one outlined, and “Decided · B. Compare table · also C · 2 comments”, then your note. The agent gets B’s short reference and reads the picks with its tool.',
      good: 'You see what you submitted at a glance; the agent reads the picks as the server has them.', cost: 'A new kind of message to draw.',
      mock: () => stackOf([['In the thread', jbFrame(plansThread({ extra: youSent(`<div class="bub" style="margin:0;max-width:none"><div class="acard" style="margin:0 0 6px;background:var(--panel)"><span class="kindtile">${ic('board', 'sm')}</span><div class="col grow" style="gap:1px"><span class="ttl">Plans page · v1</span><span class="meta row g1">${ic('check', 'xs')}Decided · B. Compare table · also C · 2 comments</span><div class="thumbs">${VARIANTS.map((variant) => `<img src="${variant.img}" class="${variant.key === 'B' ? 'picked' : ''}" style="width:64px;height:40px">`).join('')}</div></div></div>Go with the table.</div>`) + readRow() }), { h: 680 })],
        ['What the agent gets', codeFrame([['Your message', `[@Plans page](agentz://artifact/5d1c9e/1) Go with the table.`], ['The reference beside it, in place of the picks', `Plans page: a design (1 topic, 5 variants, v1), submitted just now with 2 comments.
Read the picks and comments with agentz_artifact_read { "artifact": "5d1c9e" }.`]], { w: 900 })]]) },
  ],
});

// 18. The design in the thread -------------------------------------------------------------------
const incardThread = (card) => jbFrame(plansThread({ card }), { h: 480 });
TOPICS.push({
  id: 'incard', section: 'Picks and Submit', title: 'The design in the thread', size: 'wide', rec: 'C',
  now: 'A publish shows as a card where the agent published: its kind’s tile, the title, “Page · v1 · published 2m ago”, Open and a menu (the first round’s topic 1 B). The newest publish of an artifact is the card; earlier ones become lines. The thread’s header lists the thread’s artifacts (topic 3).',
  issues: [],
  options: [
    { key: 'A', name: 'The card a document has', from: 'the first round (topic 1 B)',
      desc: 'The same card with a design’s tile: “Design · 5 variants · v1 · published 2m ago”.',
      good: 'Nothing new.', cost: 'Says nothing of the variants or your picks.',
      mock: () => incardThread(designCard()) },
    { key: 'B', name: 'With the variants’ pictures', from: 'new',
      desc: 'A’s card with a row of small pictures of the variants (the first topic’s); a click on one opens the design up close on it.',
      good: 'You see what the agent made without opening it.', cost: 'A taller card.',
      mock: () => incardThread(designCard({ thumbs: true })) },
    { key: 'C', name: 'B, showing your picks and Decided', from: 'new',
      desc: 'As B, and the card follows the design: “1 of 1 decided”, the picked picture outlined, as you pick; once you Submit, “Decided · B. Compare table”. A design with several topics says “2 of 3 decided”.',
      good: 'The thread shows where the design stands.', cost: 'The card changes after it’s drawn.',
      mock: () => stackOf([['While you pick', incardThread(designCard({ thumbs: true, picked: 'B', meta: 'Design · 5 variants · v1 · 1 of 1 decided · picked B' }))], ['Submitted', incardThread(designCard({ thumbs: true, picked: 'B', meta: `Design · 5 variants · v1 · <span style="color:var(--ok)">${ic('check', 'xs')} Decided · B. Compare table</span>` }))]]) },
  ],
});

// 19. Published again ----------------------------------------------------------------------------
const AGAIN_VARIANTS = VARIANTS.filter((variant) => variant.key !== 'D');
const v2Design = (banner, cards, decided) => plansDesign({ bar: designBar({ version: 2, decided }), body: `<div class="bd-banner" style="margin-bottom:12px">${ic('history', 'sm')}<span class="grow">${banner}</span></div>${topicHead({ kind: '<b>Pick one</b>, then <b>+ Also</b> on any others to take parts of. Keys: <b>1–4</b> pick.' })}${grid(cards)}` });
TOPICS.push({
  id: 'again', section: 'Picks and Submit', title: 'Published again', size: 'wide', rec: 'A',
  now: 'Each publish is a new version at the same link, and an open page gets a bar, “v2 was just published. Show v2” (the first round’s topic 12 B); the version menu has the earlier ones. In the mocks, Claude Agent published v2 after you submitted: B changed after your comment, and D is gone.',
  issues: ['An agent revises a design after your comments, and you shouldn’t have to pick again'],
  options: [
    { key: 'A', name: 'Picks carry over', from: 'new',
      desc: 'Picks and comments move to the new version by variant name. A changed variant is marked Changed and a new one New; a variant that’s gone takes its pick with it, and the banner says so. A submitted design waits for you again: Submit sends v2’s picks.',
      good: 'Pick once; look only at what changed.', cost: 'A renamed variant loses its pick.',
      mock: () => v2Design('v2 from Claude Agent: B changed after your comment, D is gone. Your picks are kept; Submit when you’re happy with them.', AGAIN_VARIANTS.map((variant) => card(variant, { rec: variant.key === 'B', picked: variant.key === 'B', also: variant.key === 'C', comment: PICKS.comments[variant.key] || '', tags: variant.key === 'B' ? '<span class="bd-tag new">Changed</span>' : '' })).join(''), 1) },
    { key: 'B', name: 'Each version starts empty', from: 'artifacts (each version stands alone)',
      desc: 'A new version has no picks; v1’s stay with v1, in the version menu, and v1 stays Decided.',
      good: 'Simple: each version is what you said about it.', cost: 'Pick again after every revision.',
      mock: () => v2Design('v2 from Claude Agent. Your picks on v1 stay with v1.', AGAIN_VARIANTS.map((variant) => card(variant, { rec: variant.key === 'B' })).join(''), 0) },
    { key: 'C', name: 'Empty, showing what you picked before', from: 'new',
      desc: 'As B, but each variant you picked or commented on in v1 says so (“Picked in v1”, with the comment), so picking again is a click.',
      good: 'Every revision is a fresh decision, with your last one in view.', cost: 'Two sets of marks on a card.',
      mock: () => v2Design('v2 from Claude Agent. Your v1 picks are marked.', AGAIN_VARIANTS.map((variant) => card(variant, { rec: variant.key === 'B', tags: variant.key === 'B' ? '<span class="bd-tag" style="border-color:var(--ok);color:var(--ok)">Picked in v1</span>' : variant.key === 'C' ? '<span class="bd-tag">Also in v1</span>' : '' })).join(''), 0) },
  ],
});

// 20. Artifacts in the @ menu --------------------------------------------------------------------
const typedIn = (html, text) => html.replace('<div class="ph" style="padding:2px 0 12px">Message the agent…</div>', `<div style="padding:2px 0 12px;color:var(--t)">${text}</div>`);
const MENTION_GROUPS = [
  ['In this thread', ['plans', 'research'], (item) => artMeta(item)],
  ['In storefront', ['pricing', 'spec', 'emails', 'notes'], (item) => `${item.thread.title}${item.thread.archived ? ' · archived' : ''}`],
  ['Elsewhere', ['bench', 'async'], (item) => `${fromText(item.thread)}${item.machine ? ` · ${item.machine}` : ''}`],
];
const mentionRows = (ids, hintFn, hlFirst = false) => ids.map((id, index) => mrow(kindIc(art(id)), art(id).title, hintFn(art(id)), { hl: hlFirst && index === 0 }));
const otherGroups = () => [mgroup('Threads', [mrow(ic('chat', 'sm'), 'Add the checkout page', '3m')]), mgroup('Files', [mrow(ic('folder', 'sm'), 'src'), mrow(ic('file', 'sm'), 'plans.html', 'pages')])];
const mentionThread = (over, h = 640) => jbFrame(typedIn(plansThread({ over }), 'Build B from @'), { h });
const MENTION_SENT = `Build B from [@Plans page](agentz://artifact/5d1c9e/1), with Yearly as the default.`;
TOPICS.push({
  id: 'mention', section: 'Mentions', title: 'Artifacts in the @ menu', size: 'wide', rec: 'B',
  now: 'In a thread, @ lists Threads and Files (in a chat, Projects and Threads), each row with its icon, name and a hint (screenshot). A thread mention sends that thread’s conversation with your message, behind a chip. Artifacts can’t be mentioned; an agent finds one with <code>agentz_artifact_list</code>. Decided: artifacts can be mentioned, this thread’s first, then the project’s, then anywhere’s; a mention is a short reference, and the agent reads the artifact with its tools, from any machine.',
  nowImg: 'img/now-mention.png',
  issues: [],
  options: [
    { key: 'A', name: 'One Artifacts group', from: 'agentZ’s @ menu (a group per kind)',
      desc: 'An Artifacts group before Threads and Files: this thread’s artifacts, then the project’s, then anywhere’s, each row’s hint saying where it’s from. Typing filters it, as the other groups.',
      good: 'One more group, like the others.', cost: 'Where an artifact is from is only in the faint hint.',
      mock: () => mentionThread(mentionMenu([mgroup('Artifacts', MENTION_GROUPS.flatMap(([label, ids, hintFn], index) => mentionRows(ids, index === 0 ? () => 'This thread' : hintFn, index === 0))), ...otherGroups()])) },
    { key: 'B', name: 'Three groups', from: 'new',
      desc: 'Three groups before Threads and Files: In this thread, In storefront (a chat has In this chat, then Elsewhere) and Elsewhere, which names the project or chat and the machine. Picked, it’s a chip in the message, as a thread’s is. The agent gets the title, kind, version and id, to read with <code>agentz_artifact_read</code>.',
      good: 'Where each one is from is the first thing you read.', cost: 'Three labels in a menu that’s often short.',
      mock: () => stackOf([['Typing @', mentionThread(mentionMenu([...MENTION_GROUPS.map(([label, ids, hintFn], index) => mgroup(label, mentionRows(ids, hintFn, index === 0))), ...otherGroups()]))],
        ['Picked: the chip', jbFrame(typedIn(threadView({ thread: T2.plans }), `Build B from ${mchip(ic('board', 'xs'), 'Plans page')}, with Yearly as the default.`), { h: 220 })],
        ['What the agent gets', codeFrame([['Your message', MENTION_SENT], ['The reference beside it', `Plans page: a design (1 topic, 5 variants, v1, 1 of 1 decided), from storefront › Design the plans page, on This Mac.
Read it with agentz_artifact_read { "artifact": "5d1c9e" }.`]], { w: 900 })]]) },
    { key: 'C', name: 'An Artifacts row with its own list', from: 'new, like a submenu',
      desc: 'The menu has one Artifacts row, with their count; → or a click opens their list beside it, grouped as B, with a search of its own. Typing after @ still finds artifacts by name.',
      good: 'The menu stays short when you’re after a file.', cost: 'A step more to mention an artifact.',
      mock: () => mentionThread(`${mentionMenu([mgroup('Artifacts', [mrow(ic('layers', 'sm'), 'Artifacts', `15 ${ic('chev-right', 'xs')}`, { hl: true })]), ...otherGroups()], { width: 300 })}${mentionMenu(MENTION_GROUPS.map(([label, ids, hintFn], index) => mgroup(label, mentionRows(ids, hintFn, index === 0))), { style: 'left:calc(50% - 50px);bottom:96px', width: 400 })}`) },
  ],
});

// 21. Design defaults ----------------------------------------------------------------------------
const autoOpenRow = () => settingRow('Open in the browser', 'When a page an agent publishes opens by itself. Otherwise its card’s Open does it.', dropdown('Never'));
const chatsGroup = () => settingGroup('Chats', [settingRow('Chats', 'Threads for general conversation, outside every project, in their own group in the sidebar.', sw(true))]);
const defaultRows = ({ variants = 3, today = true, sizes = 'Desktop', as = 'Pictures' } = {}) => [
  settingRow('Variants', 'How many variants a topic has.', stepper(variants)),
  settingRow('Include today’s', 'Today’s version as a variant to keep (topic 11).', sw(today)),
  settingRow('Sizes', 'The sizes each variant is shown at.', seg(['Desktop', 'Phone', 'Both'], sizes)),
  settingRow('Variants as', 'Pictures, live pages, or both (topic 14).', seg(['Pictures', 'Live pages', 'Both'], as)),
];
const defaultsNote = (text = 'Given to agents as defaults for their designs; they may change them for a project.') => `<div class="xs ph" style="margin:8px 2px 0">${text}</div>`;
const generalDefaults = (h = 620) => settings2(`<div style="font-size:15px;color:var(--t);margin-bottom:4px">General</div>${chatsGroup()}${settingGroup('Artifacts', [autoOpenRow(), ...defaultRows()])}${defaultsNote()}`, { h });
TOPICS.push({
  id: 'defaults', section: 'Settings and the agent', title: 'Design defaults', size: 'wide', rec: 'A',
  now: 'Settings › General › Artifacts has one row, Open in the browser: when a page an agent publishes opens by itself (screenshot). Nothing about designs. Decided: the design defaults are how many variants, whether today’s version is one of them, phone and/or desktop, and pictures and/or live pages. They’re given to the agent as defaults, which it may change for a project, not as rules.',
  nowImg: 'img/now-settings.png',
  issues: [],
  options: [
    { key: 'A', name: 'Rows in General › Artifacts', from: 'agentZ’s Settings',
      desc: 'Under Open in the browser: Variants (a stepper, 3 at first), Include today’s (a switch), Sizes (Desktop · Phone · Both) and Variants as (Pictures · Live pages · Both), with a line under them: given to agents as defaults; they may change them for a project.',
      good: 'Four controls, each plain.', cost: 'One set for every project; a project that needs phones says so in its thread or its AGENTS.md.',
      mock: () => generalDefaults() },
    { key: 'B', name: 'A, and a project’s own', from: 'agentZ’s Settings › Projects',
      desc: 'As A, and each project’s page in Settings › Projects has Design defaults: “Use General’s” on at first; off, the same four rows for that project.',
      good: 'A shop that’s mostly phones says so once.', cost: 'Two places to look for the defaults in use.',
      mock: () => stackOf([['General', generalDefaults(560)], ['Projects › storefront', settings2(`<div class="row g2" style="font-size:15px;color:var(--t);margin-bottom:4px">${mono('ST', 'g')}storefront<span class="sm ph" style="margin-left:4px">~/storefront</span></div>${settingGroup('Design defaults', [settingRow('Use General’s', '3 variants, today’s included, desktop, pictures.', sw(false)), ...defaultRows({ sizes: 'Both', as: 'Both' })])}${defaultsNote('For storefront’s threads, in place of General’s.')}`, { section: 'Projects', h: 460 })]]) },
    { key: 'C', name: 'Written as instructions', from: 'new, like an AGENTS.md',
      desc: 'One text box, Design defaults, in General › Artifacts: what you’d tell an agent, in your words. It’s given to the agent as you wrote it.',
      good: 'Anything you like, not just four settings.', cost: 'Nothing for agentZ to show or check; the agent may read it its own way.',
      mock: () => settings2(`<div style="font-size:15px;color:var(--t);margin-bottom:4px">General</div>${chatsGroup()}${settingGroup('Artifacts', [autoOpenRow(), `<div class="srowx" style="display:block"><div class="n">Design defaults</div><div class="d">Given to agents with their design tool, as you write them.</div><div class="tarea" style="margin-top:8px;min-height:84px">Make three variants of each thing, and keep today’s version as one of them. Desktop only, unless the page is mostly used on phones. Pictures, with a live page for anything that moves.</div></div>`])}`) },
  ],
});

// 22. The agent's tools --------------------------------------------------------------------------
const TOOLS_READ = `agentz_artifact_list { "scope": "all" }   // "thread", "project" or "all"; every machine's
→ [{ "id": "5d1c9e", "title": "Plans page", "kind": "design", "version": 1, "state": "1 of 1 decided",
     "from": "storefront › Design the plans page", "machine": "This Mac" },
   { "id": "b7e210", "title": "Limiter benchmarks", "kind": "document", "version": 5,
     "from": "api › Speed up the rate limiter", "machine": "Devbox 1" }, …]
agentz_artifact_read { "artifact": "b7e210" }   // any machine's; the latest version unless given
→ a document's Markdown, or a design's topics with your picks and comments (topic 24)`;
const TOOLS_A = `agentz_document_publish { "title": "Competitors’ plans pages", "file": "~/storefront/notes/plans.md" }
→ { "artifactId": "91c2e7", "version": 1, "url": "http://127.0.0.1:47120/a/91c2e7?t=…" }
agentz_design_publish { "title": "Plans page", "topics": [ … ] }   // topic 23
→ { "artifactId": "5d1c9e", "version": 1, "url": "http://127.0.0.1:47120/a/5d1c9e?t=…" }
// Either again, with "artifact": "5d1c9e", publishes a new version`;
TOPICS.push({
  id: 'tools', section: 'Settings and the agent', title: 'The agent’s tools', size: 'wide', rec: 'A',
  now: 'Every thread’s and chat’s agent has three: <code>agentz_artifact_publish</code> (one <code>.html</code> page or <code>.md</code> document by its path, a title, files to offer as downloads, and the artifact to add a version to), <code>agentz_artifact_list</code> (its project’s or the chats’, or every one with <code>all</code>) and <code>agentz_artifact_read</code> (an artifact’s source). They see only their machine’s. Decided: two kinds, documents and designs; the agent lists and reads every machine’s artifacts, and an artifact moves with its thread.',
  issues: ['A design is a different thing to hand over from a document, but there’s one publish tool for a file'],
  options: [
    { key: 'A', name: 'A publish tool per kind', from: 'agentZ’s tools (a tool per thing it does)',
      desc: '<code>agentz_document_publish</code> takes a Markdown file; <code>agentz_design_publish</code> takes the design (topic 23). <code>agentz_artifact_list</code> lists the thread’s, the project’s or all, every machine’s, with a design’s state; <code>agentz_artifact_read</code> reads any machine’s.',
      good: 'Each tool’s schema has only what its kind needs, and its description says how to make one.', cost: 'Four tools where there were three.',
      mock: () => codeFrame([['Publishing', TOOLS_A], ['Listing and reading', TOOLS_READ]], { w: 900 }) },
    { key: 'B', name: 'One publish tool for both', from: 'today’s agentz_artifact_publish',
      desc: '<code>agentz_artifact_publish</code> with a <code>kind</code>: a document’s file, or a design’s topics. List and read as A.',
      good: 'Three tools, as today.', cost: 'One schema with both kinds’ fields, and a description that explains both.',
      mock: () => codeFrame([['Publishing', `agentz_artifact_publish { "kind": "document", "title": "Competitors’ plans pages", "file": "…/plans.md" }
agentz_artifact_publish { "kind": "design", "title": "Plans page", "topics": [ … ] }
// "file" only for a document, "topics" only for a design: agentZ checks which it got`], ['Listing and reading', TOOLS_READ]], { w: 900 }) },
    { key: 'C', name: 'A, and a tool to wait for Submit', from: 'new, as an agent waits on a question',
      desc: 'As A, and <code>agentz_design_wait</code>: the agent’s turn stays open, the thread shows “Waiting for you to submit Plans page” and the sidebar shows it waiting for you, until you Submit; then it returns your picks, as <code>agentz_artifact_read</code> gives them.',
      good: 'The agent goes on as soon as you submit; nothing to type.', cost: 'A turn left open for as long as you take, which some agents end on their own.',
      mock: () => stackOf([['The tools', codeFrame([['Publishing', TOOLS_A], ['Waiting', `agentz_design_wait { "artifact": "5d1c9e" }
→ once you Submit: your picks and comments, as agentz_artifact_read gives them`], ['Listing and reading', `agentz_artifact_list, agentz_artifact_read   // as A`]], { w: 900 })],
        ['The thread while it waits', jbFrame(plansThread({ extra: ownRow('Waiting for you to submit', 'Plans page', `<span class="btn">${ic('arrow-up-right', 'xs')}Open</span>`) }), { h: 480 })]]) },
  ],
});

// 23. How the agent hands over a design ----------------------------------------------------------
const VARIANT_JSON = (picture, page = '') => `{ "name": "Three tiers", "picture": "${picture}",${page ? ` "page": "${page}",` : ''}
        "description": "Three plans in cards, Growth marked most popular.",
        "good": "Familiar; the middle plan stands out.",
        "cost": "Comparing one feature means reading three lists." },`;
const DESCRIPTION_END = `Design defaults, from the user's settings (change them for a project when it needs it):
3 variants a topic, with today's version as one to keep. Desktop only.
Pictures for the variants; a live page as well for anything that moves.`;
TOPICS.push({
  id: 'handover', section: 'Settings and the agent', title: 'How the agent hands over a design', size: 'wide', rec: 'B',
  now: 'An agent makes a design one of two ways today. It writes the whole thing as one page (topics, cards, picks, comments, a summary, and <code>window.agentzPicks</code> for Send to thread) and publishes it with <code>agentz_artifact_publish</code> by its path: hundreds of lines each time, different every time. Or it publishes each variant as its own artifact, as an agent in another project did with five: five unrelated pages in All artifacts (screenshot). Whichever is picked, the tool’s description ends with the design defaults (topic 21).',
  nowImg: 'img/now-gallery.png',
  issues: ['Every agent writes the design page again, its own way', 'Five variants as five artifacts can’t be compared or picked between'],
  options: [
    { key: 'A', name: 'A design file, published by its path', from: 'new, as a .md file becomes a document',
      desc: 'The agent writes a <code>design.json</code> with the topics, each variant naming its picture or page beside it, and publishes it with <code>agentz_design_publish</code> by its path.',
      good: 'The file stays on disk to edit and publish again.', cost: 'The agent learns the format only from the tool’s description, so a mistake shows only once it’s published.',
      mock: () => codeFrame([[agentCalls('agentz_design_publish'), `agentz_design_publish { "title": "Plans page", "file": "~/storefront/.designs/plans/design.json" }`], ['design.json, beside the pictures', `{
  "intro": "Five variants of the plans page, built with the site’s components.",
  "topics": [{
    "title": "The plans page",
    "today": { "description": "A plain table of the plans…", "picture": "today.png" },
    "recommended": "B",
    "variants": [
      ${VARIANT_JSON('1-three-tiers.png')}
      { "name": "Compare table", "picture": "2-compare-table.png", … },
      …
    ]
  }]
}`]], { w: 900 }) },
    { key: 'B', name: 'The tool’s arguments', from: 'agentZ’s tools (as agentz_artifact_publish)',
      desc: '<code>agentz_design_publish</code> takes the design itself: the title, an intro, and topics, each with today’s version and its variants (a name, a picture and/or a page, a description, Good and Cost), and the one the agent recommends. Its schema lists every field, so the agent fills it in right the first time, and agentZ checks it before publishing. The answer is the design’s link.',
      good: 'The agent sees exactly what to give; nothing to write but the variants.', cost: 'A long call for a big design.',
      mock: () => codeFrame([[agentCalls('agentz_design_publish'), `agentz_design_publish
{
  "title": "Plans page",
  "intro": "Five variants of the plans page, built with the site’s components.",
  "topics": [{
    "title": "The plans page",
    "today": { "description": "A plain table…", "picture": "~/storefront/.designs/today.png" },
    "recommended": "B",
    "variants": [
      ${VARIANT_JSON('~/storefront/.designs/1.png', '~/storefront/.designs/1.html')}
      { "name": "Compare table", "picture": "~/storefront/.designs/2.png", … },
      …
    ]
  }]
}`], ['agentZ answers', '{ "artifactId": "5d1c9e", "version": 1, "url": "http://127.0.0.1:47120/a/5d1c9e?t=…" }'], ['The end of the tool’s description', DESCRIPTION_END]], { w: 900 }) },
    { key: 'C', name: 'A folder with a Markdown file', from: 'new, like a design round’s folder',
      desc: 'The agent puts the pictures and pages in a folder with a <code>design.md</code>: the title and intro, then a heading per variant with its description, Good and Cost lines, and its picture. It publishes the folder; agentZ reads the headings.',
      good: 'The easiest thing for an agent to write, and readable as it is.', cost: 'Loose: a mistyped heading becomes a variant, and several topics need a convention of their own.',
      mock: () => codeFrame([[agentCalls('agentz_design_publish'), `agentz_design_publish { "title": "Plans page", "folder": "~/storefront/.designs/plans" }`], ['design.md, beside today.png, 1-three-tiers.png, 2-compare-table.html, …', `# Plans page
Five variants of the plans page, built with the site’s components.

## Today
A plain table of the plans… ![](today.png)

## Three tiers
Three plans in cards, Growth marked most popular.
Good: Familiar; the middle plan stands out.
Cost: Comparing one feature means reading three lists.
![](1-three-tiers.png)

## Compare table (recommended)
Every feature in one table, a column a plan, with Monthly and Yearly.
…`]], { w: 900 }) },
  ],
});

// 24. What the agent reads back ------------------------------------------------------------------
const READ_CALL = 'agentz_artifact_read { "artifact": "c41f08" }';
const READ_TEXT = `# Pricing, v2: 2 of 3 decided, not submitted
From storefront › Design the pricing. Your picks and comments so far.

## 1. The plans page: B. Compare table, also C. Slider by size
**B. Compare table:** Every feature in one table, a column a plan, with Monthly and Yearly.
**Comment on B:** Yes, and make Yearly the default.
**Also C. Slider by size:** One plan priced by how many products you sell.
**Comment on C:** The slider, for shops over 1,000 products.

## 2. The plan picker in checkout: A. Radio list
**A. Radio list:** The three plans as rows, the price at the end of each.

## 3. The yearly discount: not decided
**Comment:** Between A and B; what do other shops give?`;
TOPICS.push({
  id: 'read', section: 'Settings and the agent', title: 'What the agent reads back', size: 'wide', rec: 'B',
  now: '<code>agentz_artifact_read</code> gives an artifact’s source by its id and version: a page’s HTML or a document’s Markdown. A page’s picks live only in its tab, so the agent can’t read them. On the design board, the agent reads <code>decisions.md</code>, made from <code>choices.json</code>: a section per topic with the pick, its description and the comments. The mocks read Pricing, a design of three topics with two decided.',
  issues: [],
  options: [
    { key: 'A', name: 'The picks as JSON', from: 'the design board’s choices.json',
      desc: 'The design’s state and, for each topic, the pick, the variants also taken and the comments by variant, as JSON.',
      good: 'Exact, for an agent that processes it.', cost: 'Letters without names: the agent has to match them to what it published.',
      mock: () => codeFrame([[agentCalls('agentz_artifact_read'), READ_CALL], ['agentZ answers', `{ "artifactId": "c41f08", "title": "Pricing", "kind": "design", "version": 2,
  "from": "storefront › Design the pricing", "decided": 2, "submitted": false,
  "topics": [
    { "title": "The plans page", "pick": "B", "also": ["C"],
      "comments": { "B": "Yes, and make Yearly the default.",
                    "C": "The slider, for shops over 1,000 products." } },
    { "title": "The plan picker in checkout", "pick": "A", "comments": {} },
    { "title": "The yearly discount", "pick": null,
      "comments": { "topic": "Between A and B; what do other shops give?" } }
  ] }`]], { w: 900 }) },
    { key: 'B', name: 'As text, like decisions.md', from: 'the design board’s decisions.md',
      desc: 'A heading per topic with the pick by letter and name, each picked variant’s description, and the comments under it; an undecided topic says so. The top says whether it’s submitted.',
      good: 'Reads as the spec it is, and the agent gets its own words back.', cost: 'Longer than JSON.',
      mock: () => codeFrame([[agentCalls('agentz_artifact_read'), READ_CALL], ['agentZ answers', READ_TEXT]], { w: 900 }) },
    { key: 'C', name: 'B, with what changed', from: 'new',
      desc: 'As B, and the top says what changed since this thread’s agent last read it; each changed line is marked. The page’s “read 4m ago” (topic 15 C) comes from the same note.',
      good: 'After a few comments, the agent sees what’s new without comparing.', cost: 'agentZ keeps a copy of each read.',
      mock: () => codeFrame([[agentCalls('agentz_artifact_read'), READ_CALL], ['agentZ answers', READ_TEXT.replace('Your picks and comments so far.', 'Changed since you read it 25m ago: topic 1’s pick, and a comment on topic 3.').replace('Slider by size\n**B.', 'Slider by size (changed: was A. Three tiers)\n**B.').replace('**Comment:**', '**Comment (new):**')]], { w: 900 }) },
  ],
});
