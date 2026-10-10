// Artifacts: pages agents publish, opened in the browser. The thread's row or card, where the
// app lists them and what they're called, the page's header, versions, export, theme, sending
// back, live updates, Markdown pages, and deleting the thread that published one.

const caption = (text) => `<div class="xs ph" style="margin:0 0 6px;font:12px 'IBM Plex Sans',sans-serif;color:#8d8e91">${text}</div>`;
const stack = (items, gap = 16) => `<div style="display:flex;flex-direction:column;gap:${gap}px;width:max-content">${items.map(([label, html]) => `<div>${caption(label)}${html}</div>`).join('')}</div>`;
const side = (items, gap = 16) => `<div style="display:flex;gap:${gap}px;align-items:flex-start;width:max-content">${items.map(([label, html]) => `<div>${caption(label)}${html}</div>`).join('')}</div>`;
const JB_LIGHT = '--title:#f7f8fa;--panel:#f7f8fa;--ed:#ffffff;--b:#ebecf0;--bv:#ebecf0;--t:#000000;--mu:#404040;--ph:#818594;--hov:#ededed;--sel:#dfe1e5;--ac:#3573f0;--blk:#e4e6ea;--bubble:#ebecf0;--dim:#6c707e;';
const [LAYOUT_ART, BENCH_ART, NOTES_ART, ASYNC_ART] = ARTS;

// A thread with a publish in it, as each option shows it.
const checkoutThread = (publish, { h = 470, w = 900, extra = '', over = '', headExtra = '' } = {}) =>
  threadFrame({ convo: `${CHECKOUT_ASK}${CHECKOUT_WORK}${publish}${CHECKOUT_AFTER}${worked()}${extra}`, over, headExtra }, { w, h });

// The card (topic 1 B): what it is, its title, version and when, then Open and a menu.
const pubCard = (art = LAYOUT_ART, { version = art.version, note = '', meta, dim = false } = {}) => `<div class="acard" style="${dim ? 'opacity:.75' : ''}">
  <span class="kindtile">${ic(kindIcon(art), 'sm')}</span>
  <div class="col grow" style="gap:1px;min-width:0"><span class="ttl trunc">${art.title}</span><span class="meta trunc">${meta || `${art.kind === 'md' ? 'Document' : 'Page'} · v${version} · published ${version === art.version ? art.ago : 'earlier'}`}${note ? ` · ${note}` : ''}</span></div>
  <span class="btn">${ic('arrow-up-right', 'xs')}Open</span>${ibtn('more')}</div>`;
const thumb = (body = layoutsBody(), { w = 168, h = 104 } = {}) => `<div style="width:${w}px;height:${h}px;flex:none;overflow:hidden;border-radius:6px;border:1px solid var(--b);background:var(--ed);position:relative"><div style="zoom:${w / 1000};width:1000px;pointer-events:none">${body}</div></div>`;

// 1. In the thread ------------------------------------------------------------------------------
TOPICS.push({
  id: 'card', section: 'In the thread', title: 'A published page in the thread', size: 'wide', rec: 'B',
  now: 'There are no artifacts yet. agentZ’s own tools say what they did on one line with the agentZ mark, as the Tool calls round picked: “Ran cargo test -p limiter”, “Started a terminal: npm run dev”, and rows that made a subthread, thread or terminal end in Open (screenshot, with the mock agent). Design boards are pages an agent writes in <code>design/</code> and serves with <code>design/server.py</code>; it writes the link in its reply, and you open it. Claude Code prints the page’s link after each publish; t3code shows its HTML page inside the thread, with “Open in panel” on hover.',
  nowImg: 'img/now-thread.png',
  issues: ['Nothing in a thread shows a page yet', 'A link in a reply scrolls away with it'],
  options: [
    { key: 'A', name: 'A line, like agentZ’s other tools', from: 'the Tool calls round (topic 7 A, from t3code)',
      desc: 'The publish is one of agentZ’s tools, so it reads as they do: the agentZ mark, “Published an artifact: Checkout layouts”, its version, and Open at the end, which opens it in your browser. While it runs, “Publishing an artifact…” with a spinner. A folded run counts it with the rest (“Read 4 files and published an artifact”).',
      good: 'Matches the thread’s other rows; one line.', cost: 'The page, the thing you asked for, looks like any other step.',
      mock: () => checkoutThread(ownRow('Published an artifact:', 'Checkout layouts', `<span class="vtag">v1</span>${openLink()}`)) },
    { key: 'B', name: 'A card with its title, version and Open', from: 'Claude Code (the link it prints after each publish), as a card',
      desc: 'A card in the conversation where the agent published: a tile by kind (a page, a document), the title, then “Page · v1 · published 2m ago”, an Open button, and a ⋯ menu (Copy Link, Export, Show in List). Clicking anywhere on it opens it too.',
      good: 'The page stands out from the agent’s steps, and stays easy to find when scrolling.', cost: 'Taller than a row.',
      mock: () => checkoutThread(pubCard(LAYOUT_ART, { version: 1, meta: 'Page · v1 · published 2m ago' })) },
    { key: 'C', name: 'A card with a picture of the page', from: 't3code (the page shown in the thread)',
      desc: 'B with a small picture of the page at its left, so you see what it is before opening it. agentZ draws no web pages, so the server takes the picture in a headless browser after each publish, as t3code’s html_preview does. Markdown pages get the same, of their first screen.',
      good: 'You recognize a page at a glance, and several in a thread are told apart.', cost: 'A headless browser on every machine with a server, and a second or two per publish.',
      mock: () => checkoutThread(`<div class="acard" style="align-items:stretch;padding:8px">${thumb()}<div class="col grow" style="gap:2px;min-width:0;padding:4px 2px"><span class="ttl">Checkout layouts</span><span class="meta">Page · v1 · published 2m ago</span><span class="grow"></span><span class="row g2"><span class="btn">${ic('arrow-up-right', 'xs')}Open</span>${ibtn('more')}</span></div></div>`, { h: 520 }) },
    { key: 'D', name: 'Only the link in the agent’s reply', from: 'Claude Code (the URL it prints)',
      desc: 'The publish shows as a line like A’s, without Open, and the agent writes the page’s link in its reply, as Claude Code prints it. Clicking the link opens it.',
      good: 'Nothing new to build in the thread.', cost: 'Depends on the agent writing the link; a long localhost link with its secret token in the reply.',
      mock: () => checkoutThread(ownRow('Published an artifact:', 'Checkout layouts'), { extra: '' }).replace(CHECKOUT_AFTER, msg('Three layouts are at <span style="color:var(--ac)">http://127.0.0.1:47120/a/7f3a9c?t=Jx8…</span>. Pick one on the page and send it back.')) },
  ],
});

// 2. Republished --------------------------------------------------------------------------------
const ASK2 = bub('Make Steps shorter: two steps, with the address and payment together.');
const later = (first, second) => threadFrame({ convo: `${CHECKOUT_WORK}${first}${msg('Three layouts. Pick one on the page and send it back.')}${ASK2}${foldRow('Edited checkout-layouts.html')}${second}${msg('Steps now has two. The other layouts are as they were.')}${worked('Worked for 38s')}` }, { w: 900, h: 470 });
TOPICS.push({
  id: 'republish', section: 'In the thread', title: 'When it’s published again', size: 'wide', rec: 'D',
  now: 'Nothing yet. In Claude Code each publish is a new version at the same link, and a page that’s open updates; the link in an earlier message opens the latest. The mocks use topic 1’s card B.',
  issues: ['A long thread can publish the same page many times'],
  options: [
    { key: 'A', name: 'Each card keeps its version', from: 'new',
      desc: 'A card for every publish, each with its version. An earlier one says “v1 · v2 is the latest”, and its Open opens v1, with the page saying it’s not the latest.',
      good: 'The thread shows the page as it was at each point.', cost: 'Opening an old card shows an old page, which may not be what you wanted.',
      mock: () => later(pubCard(LAYOUT_ART, { version: 1, meta: 'Page · v1 · v2 is the latest' }), pubCard(LAYOUT_ART, { version: 2, meta: 'Page · v2 · published just now' })) },
    { key: 'B', name: 'Every card opens the latest', from: 'Claude Code (one link for every version)',
      desc: 'A card for every publish, but each opens the latest version, as one link does in Claude Code. Earlier cards read “Updated since: now v2”, and the page’s version menu (topic 8) has the older ones.',
      good: 'Open always shows the page as it is now.', cost: 'Two cards in a thread that do the same.',
      mock: () => later(pubCard(LAYOUT_ART, { version: 1, meta: 'Page · v1 · updated since: now v2', dim: true }), pubCard(LAYOUT_ART, { version: 2, meta: 'Page · v2 · published just now' })) },
    { key: 'C', name: 'The first a card, then lines', from: 'new',
      desc: 'The first publish is the card. Later ones are lines in the agent’s work, “Updated an artifact: Checkout layouts · v2” with Open, and fold with the other steps (“Edited 1 file and updated an artifact”).',
      good: 'One card per page, where it was first made.', cost: 'The latest version is a line, far below its card.',
      mock: () => later(pubCard(LAYOUT_ART, { version: 1, meta: 'Page · v1 · published 31m ago' }), ownRow('Updated an artifact:', 'Checkout layouts', `<span class="vtag">v2</span>${openLink()}`)) },
    { key: 'D', name: 'The card moves to the latest', from: 'new',
      desc: 'Only the newest publish of a page shows as the card; each earlier one becomes a line, “Published an artifact: Checkout layouts · v1”, which opens that version.',
      good: 'One card per page, at the bottom where you’re reading, and every version is still in place.', cost: 'The thread’s earlier part changes when the agent publishes again.',
      mock: () => later(ownRow('Published an artifact:', 'Checkout layouts', `<span class="vtag">v1</span>${openLink()}`), pubCard(LAYOUT_ART, { version: 2, meta: 'Page · v2 · published just now' })) },
  ],
});

// 3. Opening the browser -------------------------------------------------------------------------
TOPICS.push({
  id: 'autoopen', section: 'In the thread', title: 'Opening the browser by itself', size: 'wide', rec: 'B',
  now: 'Nothing yet. Claude Code opens your browser on a page’s first publish (<code>CLAUDE_CODE_ARTIFACT_AUTO_OPEN=0</code> turns that off), and not when it’s published again. agentZ never brings windows to the front on its own today: an agent waiting shows in the sidebar and in a notification.',
  issues: ['A browser coming to the front takes the keyboard from what you were doing'],
  options: [
    { key: 'A', name: 'On the first publish', from: 'Claude Code',
      desc: 'The first time a page is published, agentZ opens it in your browser; later publishes don’t (an open page updates, topic 12). The card says “Opened in your browser”.',
      good: 'The page is in front of you as soon as it’s made.', cost: 'The browser jumps to the front, even while you type in another thread.',
      mock: () => checkoutThread(pubCard(LAYOUT_ART, { version: 1, meta: 'Page · v1 · opened in your browser' })) },
    { key: 'B', name: 'Never: you open it', from: 'new',
      desc: 'The page waits in the thread for Open. A thread that’s waiting for your pick on the page shows as any thread waiting for you (the agent asks in its reply).',
      good: 'Nothing takes focus; you open it when you’re ready.', cost: 'One click each time.',
      mock: () => checkoutThread(pubCard(LAYOUT_ART, { version: 1, meta: 'Page · v1 · published 2m ago' })) },
    { key: 'C', name: 'Only when you’re in that thread', from: 'new',
      desc: 'As A, but only when the thread is on screen and agentZ is in front, so you were watching it. Otherwise it waits for Open, as B.',
      good: 'Opens when you’re waiting for it, and not otherwise.', cost: 'Two behaviors to learn.',
      mock: () => checkoutThread(pubCard(LAYOUT_ART, { version: 1, meta: 'Page · v1 · opened in your browser, since you were here' })) },
  ],
});

// 4. Name ---------------------------------------------------------------------------------------
const nameMock = (plural, singular, article = 'a') => frame(`<div class="col" style="height:100%;background:var(--panel)">
  <div style="padding:10px 4px 6px">${shelf(plural, 4, { open: true })}${artRow(LAYOUT_ART)}${artRow(BENCH_ART)}</div>
  <div style="border-top:1px solid var(--b);padding:10px 12px">${ownRow(`Published ${article} ${singular}:`, 'Checkout layouts', `<span class="vtag">v1</span>`)}</div>
  <div style="border-top:1px solid var(--b);padding:12px;font-size:13px;color:var(--mu)" class="row g2">${ibtn('home')}All ${plural.toLowerCase()}<span class="ph">/</span><span style="color:var(--t)">Checkout layouts</span></div></div>`, { w: 340, h: 236 }).replace('class="m"', 'class="m jb"');
const artRow = (art, { hov = false, extra = '' } = {}) => `<div class="lrow ${hov ? 'hov' : ''}" style="height:auto;padding:5px 8px;align-items:flex-start;margin:0 4px">
  <span style="color:var(--mu);padding-top:2px">${ic(kindIcon(art), 'sm')}</span><div class="col grow" style="min-width:0;gap:1px"><div class="row g1"><span class="trunc">${art.title}</span><span class="vtag">v${art.version}</span></div><span class="sub trunc">${extra || where(art.thread)}</span></div><span class="sub none" style="padding-top:2px">${art.ago.replace(' ago', '')}</span></div>`;
TOPICS.push({
  id: 'name', section: 'In the app', title: 'What they’re called', size: 'narrow', rec: 'A',
  now: 'Not named yet; the backlog says the name isn’t decided. Claude Code and claude.ai call them artifacts, the word the backlog uses; t3code calls its pages HTML renders.',
  issues: [],
  options: [
    { key: 'A', name: 'Artifacts', from: 'Claude Code',
      desc: '“Artifacts”, “Published an artifact”, “All artifacts”: Claude’s word, which many already know from Claude Code and claude.ai.',
      good: 'Known; covers pages, documents and small tools alike.', cost: 'A jargon word for a page.',
      mock: () => nameMock('Artifacts', 'artifact', 'an') },
    { key: 'B', name: 'Pages', from: 'new',
      desc: '“Pages”, “Published a page”, “All pages”: what they are, a page in the browser.',
      good: 'Plain, and says where it opens.', cost: '“Page” is used for Settings pages and web pages in general.',
      mock: () => nameMock('Pages', 'page') },
    { key: 'C', name: 'Documents', from: 'new',
      desc: '“Documents”, “Published a document”: as Claude Docs calls the pages meant for others.',
      good: 'Fits reports, specs and release notes.', cost: 'Reads oddly for a design board, a chart or a slider.',
      mock: () => nameMock('Documents', 'document') },
    { key: 'D', name: 'Boards', from: 'new, from agentZ’s design boards',
      desc: '“Boards”, “Published a board”: after the design boards they generalize.',
      good: 'Continues the word you use today.', cost: 'Suggests something to pin up and arrange, not a report.',
      mock: () => nameMock('Boards', 'board') },
  ],
});

// 5. Where they're listed -----------------------------------------------------------------------
const sbFrame = (html, h = 640) => frame(html, { w: 290, h }).replace('class="m"', 'class="m jb"');
const shelfOpen = (rows, { head = '' } = {}) => `<div style="flex:none;border-top:1px solid var(--bv);padding:2px 0 6px">${head || shelf('Artifacts', rows.length, { open: true, hl: true })}${rows.map((art, i) => artRow(art, { hov: i === 0 })).join('')}</div>`;
const listPop = (rows, { w = 330, title = 'This thread’s artifacts', style = '' } = {}) => `<div class="pop" style="width:${w}px;padding:6px 2px;${style}"><div class="xs ph" style="padding:2px 12px 6px">${title}</div>${rows.map((art, i) => artRow(art, { hov: i === 0, extra: `${art.kind === 'md' ? 'Document' : 'Page'} · ${art.ago}` })).join('')}<div style="height:1px;background:var(--bv);margin:6px 0"></div><div class="lrow" style="margin:0 4px;color:var(--mu)">${ic('list', 'sm')}All artifacts…</div></div>`;
const SPEC = { id: 'spec', title: 'Checkout spec', kind: 'md', version: 1, thread: THREADS.checkout, ago: '20m ago' };
const settingsMock = () => frame(`<div class="row" style="height:100%;align-items:stretch">
  <div style="width:220px;background:var(--panel);border-right:1px solid var(--b);padding:10px 6px;font-size:13px;color:var(--mu)" class="col g1">${['General', 'Appearance', 'Notifications', 'Agents', 'Usage', 'Skills', 'MCP Servers', 'Artifacts', 'Machines'].map((n) => `<div class="lrow ${n === 'Artifacts' ? 'hov' : ''}" style="height:28px;${n === 'Artifacts' ? 'color:var(--t)' : 'color:var(--mu)'}">${n}</div>`).join('')}</div>
  <div class="grow" style="padding:22px 28px;background:var(--ed)"><div style="font-size:18px;font-weight:600;margin-bottom:4px">Artifacts</div><div class="sm mu" style="margin-bottom:14px">Pages agents published on this Mac, kept in agentZ’s data folder.</div>
  <div class="card" style="background:var(--panel)">${ARTS.map((art, i) => `<div class="row g2" style="padding:9px 12px;${i ? 'border-top:1px solid var(--bv)' : ''}"><span class="mu">${ic(kindIcon(art), 'sm')}</span><div class="col grow" style="min-width:0"><span class="row g1" style="font-size:13px">${art.title}<span class="vtag">v${art.version}</span></span><span class="xs ph">${where(art.thread)} · ${art.ago}</span></div><span class="btn sm">${ic('arrow-up-right', 'xs')}Open</span>${ibtn('more')}</div>`).join('')}</div></div></div>`, { w: 820, h: 360 }).replace('class="m"', 'class="m jb"');
const galleryBody = (filter = 'All') => `<div class="pin" style="max-width:1040px"><div class="row g2" style="margin-bottom:16px"><div class="h1 grow" style="margin:0">All artifacts</div><div class="seg">${['All', 'Threads', 'Chats'].map((f) => `<span class="${f === filter ? 'on' : ''}">${f}</span>`).join('')}</div><div class="field" style="width:200px;height:28px">${ic('search', 'xs')}<span class="ph sm">Search</span></div></div>
  <div style="display:grid;grid-template-columns:repeat(3,1fr);gap:16px">${[LAYOUT_ART, BENCH_ART, NOTES_ART, ASYNC_ART, SPEC].map((art) => `<div class="card" style="overflow:hidden;background:var(--panel)"><div style="height:130px;overflow:hidden;border-bottom:1px solid var(--b);background:var(--ed)"><div style="zoom:.31;width:1000px">${art.id === 'bench' ? benchBody() : art.kind === 'md' ? docBody(art) : layoutsBody()}</div></div><div style="padding:10px 12px" class="col g1"><span class="row g1" style="font-size:13px">${art.title}<span class="vtag">v${art.version}</span></span><span class="xs ph trunc">${where(art.thread)} · ${art.ago}</span></div></div>`).join('')}</div></div>`;
TOPICS.push({
  id: 'place', section: 'In the app', title: 'Where they’re listed', size: 'wide', type: 'multi', rec: 'AB',
  now: 'Nothing lists them yet. The sidebar has shelves under the cards: Workspaces (threads started in panes) and Archived, each closed at first with its count. Claude Code shows a ⧉ pill under the prompt with the session’s artifact (or their count) and lists all of yours with <code>/artifacts</code>, this session’s first; claude.ai has a gallery of every one. The mocks say “artifacts”, the name topic’s A.',
  nowImg: 'img/now-sidebar.png',
  issues: ['A page is only in its thread, so a page from last week means finding that thread'],
  options: [
    { key: 'A', name: 'A shelf in the sidebar', from: 'agentZ’s Workspaces and Archived shelves',
      desc: 'An “Artifacts (4)” shelf above Archived, closed at first, its state kept like the Workspaces shelf’s. Open, a row per page, newest first: its kind, title, version, where it came from (“storefront › Add the checkout page”, or “Chat · …”) and when. A click opens it in the browser; its menu has Open, Show Thread, Copy Link and Delete….',
      good: 'Every page is a click away, wherever you are, as threads are.', cost: 'One more shelf; the sidebar gets busier.',
      mock: () => sbFrame(sidebarJB({ cards: defaultCards(), shelves: shelfOpen(ARTS) + shelf('Archived', 4) })) },
    { key: 'B', name: 'A pill in the thread’s header', from: 'Claude Code (the ⧉ pill under the prompt)',
      desc: 'Once a thread has published, its header shows the page (“Checkout layouts”) or the count (“2 artifacts”) before the branch. A click opens it, or with several, a list of the thread’s pages with “All artifacts…” at the end.',
      good: 'The pages of the thread you’re in, right where you are.', cost: 'Only this thread’s; needs another place for all of them.',
      mock: () => frame(`${threadView({ convo: `${CHECKOUT_ASK}${CHECKOUT_WORK}${pubCard(LAYOUT_ART)}${CHECKOUT_AFTER}`, headExtra: `<span class="chip" style="color:var(--t);border-color:var(--b);background:var(--hov)">${ic('app-window', 'xs')}2 artifacts${ic('chev-down', 'xs')}</span>` })}${listPop([LAYOUT_ART, SPEC], { style: 'top:40px;right:150px' })}`, { w: 900, h: 470, style: 'border-radius:8px;border:1px solid #111' }).replace('class="m"', 'class="m jb"') },
    { key: 'C', name: 'Over the composer', from: 'Claude Code’s pill under the prompt, placed like agentZ’s plan and Agents list',
      desc: 'A strip over the composer, where the plan and the Agents list sit: “2 artifacts · Checkout layouts v3, Checkout spec v1”. It opens to the list, as the Agents list does.',
      good: 'Next to where you type, as in Claude Code.', cost: 'Competes with the plan, the queue and the Agents list for that space.',
      mock: () => threadFrame({ convo: `${CHECKOUT_ASK}${CHECKOUT_WORK}${pubCard(LAYOUT_ART)}${CHECKOUT_AFTER}`, over: `<div class="bar-over"><div class="row g2" style="height:30px;padding:0 10px;font-size:12px;color:var(--mu)">${ic('chev-right', 'xs')}${ic('app-window', 'xs')}<span style="color:var(--t)">2 artifacts</span><span class="trunc">· Checkout layouts v3, Checkout spec v1</span></div></div>` }, { w: 900, h: 470 }) },
    { key: 'D', name: 'All of them on a page in the browser', from: 'Claude Code (the gallery on claude.ai)',
      desc: 'agentz-server serves an “All artifacts” page, reached from each page’s header (its first button) and from the app (the command palette’s “artifacts: show all”, and “All artifacts…” in B). Cards with a picture (topic 1 C’s), title, version, where from and when; All / Threads / Chats and a search.',
      good: 'Room for pictures and search; the app gains nothing to draw.', cost: 'Pictures need topic 1 C’s headless browser; leaves the app to browse.',
      mock: () => browser(pageShell(`<div class="phd">${ibtn('home')}<span class="ptitle">agentZ</span><span class="ph">/</span><span class="ptitle">All artifacts</span></div>`, galleryBody()), { title: 'All artifacts', icon: 'grid', url: '127.0.0.1:47120/a', h: 680 }) },
    { key: 'E', name: 'A Settings page', from: 'agentZ’s Settings pages (as Skills and MCP Servers)',
      desc: 'Settings › Artifacts lists every page on the machine with Open and a menu (Copy Link, Show Thread, Delete…), with the machine picker in the header as the Skills page has.',
      good: 'One place to see and clean them up.', cost: 'Settings isn’t where you look for your work.',
      mock: settingsMock },
  ],
});

// 6. Filters --------------------------------------------------------------------------------------
const filterMenu = `<div class="menu" style="top:-142px;right:10px;min-width:180px"><div class="it hl">${ic('check', 'sm')}All</div><div class="it"><span style="width:14px"></span>Threads</div><div class="it"><span style="width:14px"></span>Chats</div><div class="hr"></div><div class="it"><span style="width:14px"></span>This thread</div></div>`;
TOPICS.push({
  id: 'filter', section: 'In the app', title: 'Narrowing the list', size: 'wide', type: 'multi', rec: 'AD',
  now: 'The sidebar shows the project picked in the title bar (or All projects), and its search finds threads by title, archived ones and Workspaces threads included. Artifacts belong to no project, only to the thread or chat that published them. Claude Code’s <code>/artifacts</code> puts “This session” first. The mocks use the shelf from the topic before.',
  issues: ['Artifacts have no project of their own'],
  options: [
    { key: 'A', name: 'Follows the project picker', from: 'agentZ’s sidebar',
      desc: 'With a project picked in the title bar, the list shows the pages its threads published, and chats’ pages, which belong to no project. All projects shows every one.',
      good: 'No new control; the list matches the cards above it.', cost: 'A chat’s pages show under every project.',
      mock: () => side([['storefront picked', sbFrame(sidebarJB({ cards: tcard(THREADS.checkout, { on: true }) + tcard(THREADS.notes), shelves: shelfOpen([LAYOUT_ART, NOTES_ART, ASYNC_ART]) + shelf('Archived', 2) }), 520)], ['All projects', sbFrame(sidebarJB({ shelves: shelfOpen(ARTS) + shelf('Archived', 4) }), 520)]]) },
    { key: 'B', name: 'A menu: All, Threads, Chats, This thread', from: 'new',
      desc: 'A filter button on the shelf’s header opens a menu: All, Threads, Chats, and This thread (the open one’s). The shelf’s title says what’s shown (“Artifacts · Chats”).',
      good: 'Any cut you need.', cost: 'One more control on the shelf.',
      mock: () => sbFrame(`<div class="asb" style="width:100%;position:relative">${`<div class="asb-list">${defaultCards()}</div>`}<div style="position:relative">${filterMenu}${shelfOpen(ARTS, { head: shelf('Artifacts', 4, { open: true, hl: true, extra: '' }).replace('<span class="rule"></span>', `<span class="rule"></span><span class="ibtn sm on">${ic('filter', 'xs')}</span>`) })}</div>${shelf('Archived', 4)}<div class="asb-foot">${ic('settings', 'sm')}Settings</div></div>`, 640) },
    { key: 'C', name: 'Grouped by thread', from: 'Claude Code (/artifacts, “This session” first)',
      desc: 'Rows under the title of the thread or chat that published them, the open thread’s group first, then by latest publish.',
      good: 'Shows which work made what.', cost: 'Longer, with a header per thread.',
      mock: () => sbFrame(sidebarJB({ shelves: `<div style="flex:none;border-top:1px solid var(--bv);padding:2px 0 6px">${shelf('Artifacts', 5, { open: true, hl: true })}<div class="xs ph" style="padding:4px 14px 2px">Add the checkout page</div>${artRow(LAYOUT_ART, { hov: true, extra: 'storefront · 2m ago' })}${artRow(SPEC, { extra: 'storefront · 20m ago' })}<div class="xs ph" style="padding:6px 14px 2px">Speed up the rate limiter</div>${artRow(BENCH_ART, { extra: 'api · Devbox 1 · 10m ago' })}<div class="xs ph" style="padding:6px 14px 2px">Chat · Async runtimes</div>${artRow(ASYNC_ART, { extra: '3d ago' })}</div>${shelf('Archived', 4)}` }), 700) },
    { key: 'D', name: 'Found by the sidebar’s search', from: 'agentZ’s sidebar search',
      desc: 'The sidebar’s search finds pages by title as it finds threads, and opens the shelf to show them.',
      good: 'The way you already find a thread.', cost: 'Titles only, not what’s on the page.',
      mock: () => sbFrame(sidebarJB({ cards: tcard(THREADS.limiter), shelves: shelfOpen([BENCH_ART]) + shelf('Archived', 0) }).replace('<span class="grow">Search…</span>', '<span class="grow" style="color:var(--t)">bench</span>'), 420) },
  ],
});

// 7. The page's header --------------------------------------------------------------------------
const titleMenu = `<div class="menu" style="top:38px;left:40px;min-width:280px"><div class="lbl">Versions</div><div class="it hl">${ic('check', 'sm')}v3<span class="kb">Latest · 2m ago</span></div><div class="it"><span style="width:14px"></span>v2<span class="kb">14m ago</span></div><div class="it"><span style="width:14px"></span>v1<span class="kb">31m ago</span></div><div class="hr"></div><div class="it">${glyph('claude', 'sm')}Show Thread: Add the checkout page</div><div class="it">${ic('copy', 'sm')}Copy Link</div><div class="hr"></div><div class="it">${ic('save', 'sm')}Save as HTML</div><div class="it">${ic('printer', 'sm')}Print or Save as PDF…</div><div class="it">${ic('code', 'sm')}Copy Source</div></div>`;
TOPICS.push({
  id: 'header', section: 'The page in the browser', title: 'The page’s header', size: 'wide', rec: 'A',
  now: 'There are no artifact pages yet. A design board, the nearest thing, has its own top bar: the round’s title, how many topics are decided, “Saved”, Only undecided and Summary (screenshot). Claude Code’s page has a thin bar over the page: a button to all your artifacts, the title with a menu, Share and your avatar; Share has the version picker. Claude Design’s Export menu holds every format and “Send to Claude Code”.',
  nowImg: 'img/now-board.png',
  issues: ['The page needs to say what it is, which version, and where it came from'],
  options: [
    { key: 'A', name: 'A thin bar with everything on it', from: 'Claude Code (its page header)',
      desc: 'A 44 px bar in the app’s colors: All artifacts (home), the title, its version with a menu (topic 8), the agent’s icon and where it came from (“storefront › Add the checkout page”, a link that opens the thread in agentZ), then Export (topic 9) and Send to thread (topic 11). The page is below it.',
      good: 'Everything in view; the page keeps the rest.', cost: 'A long thread title is cut short.',
      mock: () => browser(pageShell(pageHead(), layoutsBody()), { h: 620 }) },
    { key: 'B', name: 'The title’s menu', from: 'Claude Code (the title menu)',
      desc: 'The bar holds only All artifacts, the title with a chevron, and Send to thread. The title’s menu has the versions, Show Thread, Copy Link and the export items.',
      good: 'Quieter; the page is what you see.', cost: 'Which version and where from are a click away.',
      mock: () => browser(pageShell(`<div class="phd">${ibtn('home')}<span class="ph">/</span><span class="ptitle">Checkout layouts</span>${ic('chev-down', 'xs')}<span class="grow"></span><span class="btn primary">${ic('send', 'xs')}Send to thread</span></div>`, layoutsBody(), { over: titleMenu }), { h: 620 }) },
    { key: 'C', name: 'A floating bar', from: 'new',
      desc: 'No header: a small bar floats at the bottom right over the page, with the version, Export and Send to thread; hovering it shows the title and where it came from.',
      good: 'The whole window for the page, as if it were any web page.', cost: 'Covers a corner of the page; the title is only in the tab.',
      mock: () => browser(`<div class="pg"><div class="pbody">${layoutsBody()}</div><div class="ptoast" style="padding:6px;gap:6px;border-radius:10px"><span class="vtag" style="height:22px">v3 ⌄</span><span class="btn">${ic('download', 'xs')}Export${ic('chev-down', 'xs')}</span><span class="btn primary">${ic('send', 'xs')}Send to thread</span></div></div>`, { h: 620 }) },
    { key: 'D', name: 'A side rail', from: 'new',
      desc: 'A 240 px rail at the left, which can be closed: the title, where it came from, the versions with their times, the agent’s files, and Export and Send to thread at its foot.',
      good: 'Versions and files in view, with room for long titles.', cost: 'Takes width from the page, which matters for side-by-side designs.',
      mock: () => browser(`<div class="pg" style="flex-direction:row"><div style="width:240px;flex:none;background:var(--panel);border-right:1px solid var(--b);padding:14px 12px;display:flex;flex-direction:column;gap:10px;font-size:13px"><div class="row g2">${ibtn('panel-left')}<span class="grow"></span>${ibtn('home')}</div><div style="font-weight:600;font-size:15px">Checkout layouts</div><div class="sm mu row g1">${glyph('claude', 'sm')}storefront › Add the checkout page</div><div class="xs ph" style="margin-top:6px">Versions</div>${[['v3', 'Latest · 2m ago', true], ['v2', '14m ago'], ['v1', '31m ago']].map(([v, t, on]) => `<div class="lrow ${on ? 'hov' : ''}" style="height:26px;margin:0 -4px">${v}<span class="grow"></span><span class="sub">${t}</span></div>`).join('')}<div class="xs ph" style="margin-top:6px">Files</div><div class="lrow" style="height:26px;margin:0 -4px">${ic('file', 'xs')}checkout-spec.docx</div><span class="grow"></span><span class="btn">${ic('download', 'xs')}Export${ic('chev-down', 'xs')}</span><span class="btn primary" style="justify-content:center">${ic('send', 'xs')}Send to thread</span></div><div class="pbody grow">${layoutsBody()}</div></div>`, { h: 620 }) },
  ],
});

// 8. Versions -------------------------------------------------------------------------------------
const versionMenu = `<div class="menu" style="top:38px;left:190px;min-width:230px">${[['v3', 'Latest · 2m ago'], ['v2', '14m ago'], ['v1', '31m ago']].map(([v, t], i) => `<div class="it ${i === 1 ? 'hl' : ''}">${i === 0 ? ic('check', 'sm') : '<span style="width:14px"></span>'}${v}<span class="kb">${t}</span></div>`).join('')}</div>`;
const oldBanner = `<div class="banner">${ic('history', 'sm')}<span>This is v2, from 14 minutes ago. The latest is v3.</span><span class="lnk" style="font-size:13px">Show v3</span></div>`;
TOPICS.push({
  id: 'versions', section: 'The page in the browser', title: 'Earlier versions', size: 'wide', rec: 'A',
  now: 'Nothing yet. Each publish of an artifact is a version (the backlog). Claude Code keeps each publish as a version at the same link, picked from its Share menu (“Sharing version 2”), with version history to restore one. The mocks use the header topic’s A.',
  issues: [],
  options: [
    { key: 'A', name: 'A menu on the version', from: 'Claude Code (its version picker)',
      desc: 'The version in the header (“v3”) opens a menu of every version with when it was published, the latest first. An earlier one shows under a banner, “This is v2, from 14 minutes ago. The latest is v3. Show v3”, and the header’s version reads “v2 of 3” in the accent. Each version has its own link.',
      good: 'Out of the way until you need it.', cost: 'Comparing two means switching back and forth.',
      mock: () => stack([['The menu', browser(pageShell(pageHead(), layoutsBody(), { over: versionMenu }), { h: 440 })], ['An earlier version open', browser(pageShell(pageHead(LAYOUT_ART, { version: 2 }), layoutsBody({ picked: 'a', comment: '' }), { banner: oldBanner }), { h: 440 })]]) },
    { key: 'B', name: 'Arrows', from: 'new',
      desc: '“‹ v2 of 3 ›” in the header steps through the versions, and so do the [ and ] keys; an earlier one shows A’s banner.',
      good: 'Quick to flip between versions to see what changed.', cost: 'Many versions take many steps.',
      mock: () => browser(pageShell(pageHead(LAYOUT_ART, { version: 2 }).replace(/<span class="vtag ac">v2 of 3<\/span><svg[^>]*>.*?<\/svg>/, `<span class="row" style="gap:0;border:1px solid var(--b);border-radius:6px">${ibtn('chev-left')}<span class="sm" style="padding:0 4px;color:var(--ac)">v2 of 3</span>${ibtn('chev-right')}</span>`), layoutsBody({ picked: 'a', comment: '' }), { banner: oldBanner }), { h: 440 }) },
    { key: 'C', name: 'A list beside the page', from: 'new',
      desc: 'The version opens a list down the right side with every version and its time; clicking one shows it, the list staying open until closed.',
      good: 'Every version in view while you look.', cost: 'Takes width from the page while open.',
      mock: () => browser(pageShell(pageHead(LAYOUT_ART, { version: 2 }), `<div class="row" style="height:100%;align-items:stretch"><div class="grow" style="overflow:hidden">${layoutsBody({ picked: 'a', comment: '' })}</div><div style="width:220px;flex:none;border-left:1px solid var(--b);background:var(--panel);padding:12px 8px" class="col g1"><div class="row xs ph" style="padding:0 6px 6px">Versions<span class="grow"></span>${ic('x', 'xs')}</div>${[['v3', 'Latest · 2m ago'], ['v2', '14m ago', true], ['v1', '31m ago']].map(([v, t, on]) => `<div class="lrow ${on ? 'hov' : ''}" style="height:28px">${v}<span class="grow"></span><span class="sub">${t}</span></div>`).join('')}</div></div>`), { h: 440 }) },
  ],
});

// 9. Export ---------------------------------------------------------------------------------------
const exportItems = (art = LAYOUT_ART) => `<div class="it hl">${ic('save', 'sm')}${art.kind === 'md' ? 'Save as Markdown' : 'Save as HTML'}</div><div class="it">${ic('printer', 'sm')}Print or Save as PDF…<span class="kb">Ctrl-P</span></div><div class="it">${ic('code', 'sm')}Copy Source</div>`;
const FILES = [['file', 'checkout-spec.docx', '24 KB'], ['sheet', 'orders-sample.csv', '3 KB']];
const fileItems = FILES.map(([icon, name, size]) => `<div class="it">${ic(icon, 'sm')}${name}<span class="kb">${size}</span></div>`).join('');
TOPICS.push({
  id: 'export', section: 'The page in the browser', title: 'Export, and the agent’s files', size: 'wide', rec: 'A',
  now: 'Nothing yet. The backlog: save the HTML or Markdown, print or save as PDF with the browser’s print, copy the source, and files the agent made with its own tools (Word, PowerPoint, CSV) offered as downloads. Claude Design’s Export menu lists each format; Claude Code’s pages offer a file only through a button the page itself has. The mocks use the header topic’s A.',
  issues: [],
  options: [
    { key: 'A', name: 'One Export menu', from: 'Claude Design (its Export menu)',
      desc: 'Export opens a menu: Save as HTML (Save as Markdown for a Markdown page), Print or Save as PDF… (the browser’s print, with the header left out), Copy Source; then “Files from the agent”, each with its size, downloading on a click. The version saved is the one shown.',
      good: 'One place for everything you can take away.', cost: 'The files are hidden until you open the menu.',
      mock: () => browser(pageShell(pageHead(LAYOUT_ART, { exportOpen: true }), layoutsBody(), { over: `<div class="menu" style="top:40px;right:140px;min-width:260px">${exportItems()}<div class="hr"></div><div class="lbl">Files from the agent</div>${fileItems}</div>` }), { h: 520 }) },
    { key: 'B', name: 'Export, and Files apart', from: 'new',
      desc: 'Export holds A’s first three; a “Files 2” button beside it, shown only when the agent attached files, lists them.',
      good: 'You see there are files without opening anything.', cost: 'Two buttons.',
      mock: () => browser(pageShell(pageHead(LAYOUT_ART, { extra: `<span class="btn">${ic('file', 'xs')}Files<span class="vtag">2</span></span>` }), layoutsBody(), { over: `<div class="menu" style="top:40px;right:236px;min-width:240px">${fileItems}</div>` }), { h: 520 }) },
    { key: 'C', name: 'Files at the end of the page', from: 'new',
      desc: 'A’s menu without the files; the files are a row of cards at the bottom of the page, under what the agent wrote, with Download on each.',
      good: 'The files read as part of the page.', cost: 'Out of sight at the end of a long page.',
      mock: () => browser(pageShell(pageHead(), `${layoutsBody()}<div class="pin" style="padding-top:0"><div class="sm mu" style="margin-bottom:8px">Files from the agent</div><div class="row g3">${FILES.map(([icon, name, size]) => `<div class="card row g2" style="padding:10px 12px;background:var(--panel)">${ic(icon, 'sm')}<div class="col"><span style="font-size:13px">${name}</span><span class="xs ph">${size}</span></div><span class="btn sm" style="margin-left:12px">${ic('download', 'xs')}Download</span></div>`).join('')}</div></div>`), { h: 640 }) },
    { key: 'D', name: 'Icon buttons', from: 'new',
      desc: 'Save, Print and Copy Source as three icon buttons with tooltips, and the files as small chips beside them.',
      good: 'Each a single click.', cost: 'Icons alone say less; the header fills up.',
      mock: () => browser(pageShell(pageHead(LAYOUT_ART).replace(/<span class="btn " style="background:var\(--panel\)">.*?Export.*?<\/span>/, `${ibtn('save')}${ibtn('printer')}${ibtn('code')}${FILES.map(([icon, name]) => `<span class="chip">${ic(icon, 'xs')}${name}</span>`).join('')}`), layoutsBody()), { h: 520 }) },
  ],
});

// 10. Theme ---------------------------------------------------------------------------------------
const small = (style, extra = {}) => browser(pageShell(pageHead(LAYOUT_ART, { send: false }), layoutsBody()), { w: 760, h: 440, style, ...extra });
TOPICS.push({
  id: 'theme', section: 'The page in the browser', title: 'The app’s theme on the page', size: 'wide', rec: 'A',
  now: 'Nothing yet. The backlog: pages use the app’s theme, its colors and fonts. agentZ has System, Light and Dark, with one theme for each (yours is JetBrains Dark). t3code gives its pages the theme as CSS variables (<code>--background</code>, <code>--foreground</code>, <code>--border</code>, <code>--accent</code>, <code>--font-sans</code>, chart colors…) before the first paint, and sends them again when you change it; a page opened outside the app follows the system’s light or dark. Claude Code’s design skill picks colors itself, or a design system written in CLAUDE.md.',
  issues: [],
  options: [
    { key: 'A', name: 'The app’s theme, live', from: 't3code (its theme as CSS variables)',
      desc: 'The server puts the app’s theme on each page as CSS variables, which the header and Markdown pages use and the agent’s skill tells it to use. Changing the theme or the mode in agentZ changes open pages at once. A page that sets its own colors keeps them.',
      good: 'Pages look like the app, now and after you change it.', cost: 'The server needs to know the app’s current theme.',
      mock: () => side([['JetBrains Dark', small('')], ['After picking JetBrains Light in agentZ', small(JB_LIGHT)]]) },
    { key: 'B', name: 'Light or dark with the system', from: 't3code (a page outside its app)',
      desc: 'Each page gets the themes for both of agentZ’s modes, and follows the system’s light or dark, as the browser reports it. A sun and moon switch in the header overrides it for that page.',
      good: 'Matches the rest of the browser and the desktop.', cost: 'Can differ from the app when the app isn’t on System.',
      mock: () => browser(pageShell(pageHead(LAYOUT_ART, { send: false, extra: `<span class="seg"><span>${ic('sun', 'xs')}</span><span class="on">${ic('moon', 'xs')}</span></span>` }), layoutsBody()), { w: 760, h: 440 }) },
    { key: 'C', name: 'Kept as published', from: 'new',
      desc: 'Each version keeps the theme it was published in, so it looks the same later and when saved. Changing the app’s theme changes only pages published afterwards.',
      good: 'A page never changes after the fact; a saved file looks like the page.', cost: 'Old pages stop matching the app.',
      mock: () => side([['v1, published in JetBrains Dark', small('')], ['The app is now JetBrains Light: v1 is unchanged', small('', { title: 'Checkout layouts' })]]) },
    { key: 'D', name: 'Colors for the agent to copy', from: 'Claude Code (its design skill)',
      desc: 'No variables: the skill gives the agent the theme’s colors and fonts, and the agent writes them into its page. Only the header is drawn in the app’s theme.',
      good: 'Simplest: the page is just what the agent wrote.', cost: 'Each page’s look is up to the agent, and it doesn’t follow later changes.',
      mock: () => browser(pageShell(pageHead(LAYOUT_ART, { send: false }), `<div style="background:#f4f1ea;color:#2b2a27;height:100%">${layoutsBody().replace(/class="ocard/g, 'style="background:#fff;border-color:#ddd8cc" class="ocard')}</div>`), { w: 760, h: 440 }) },
  ],
});

// 11. Sending back ------------------------------------------------------------------------------
const sendPop = `<div class="pop" style="top:40px;right:12px;width:380px;padding:12px;font-size:13px"><div style="font-weight:600;margin-bottom:8px">Send to “Add the checkout page”</div>
  <div style="border:1px solid var(--b);border-radius:6px;background:var(--ed);padding:8px 10px;line-height:1.5;margin-bottom:8px"><div class="xs ph">From the page</div>Picked <b>Summary beside</b>.<br>Comment: Keep the totals sticky on phones too.</div>
  <div class="field" style="height:30px;margin-bottom:10px"><span class="ph">Add a note…</span></div>
  <div class="row g2"><span class="xs ph grow">Claude Agent is idle: it starts at once.</span><span class="btn ghost">Cancel</span><span class="btn primary">${ic('send', 'xs')}Send</span></div></div>`;
const sentChip = `<div class="row" style="justify-content:flex-end;margin-bottom:-6px"><span class="chip">${ic('app-window', 'xs')}Checkout layouts · v3</span></div>`;
const sentThread = (text) => threadFrame({ convo: `${CHECKOUT_WORK}${pubCard(LAYOUT_ART)}${CHECKOUT_AFTER}${worked()}${sentChip}${bub(text)}${trow(spinner(), '<span>Working</span>')}` }, { w: 900, h: 400 });
TOPICS.push({
  id: 'send', section: 'The page in the browser', title: 'Sending back to the thread', size: 'wide', rec: 'A',
  now: 'A design board, the page this generalizes, saves your picks and comments as you go into <code>choices.json</code> and <code>decisions.md</code> beside it; then you tell the agent in its thread to read them. Claude Code’s pages can only give you text to paste (“Copy as prompt”). Claude Design sends a design to Claude Code from its Export menu. t3code’s pages talk to the app (the theme, their height, links) but send nothing to the agent.',
  nowImg: 'img/now-board.png',
  issues: ['Today you go back to the thread and type “read decisions.md”'],
  options: [
    { key: 'A', name: 'Send to thread, with a look first', from: 'the design board’s saving, and Claude Design’s Send to Claude Code',
      desc: 'The page gives what you picked or wrote (the agent’s page says what, through agentZ’s script on it). Send to thread opens a box with that text and a note field; Send puts it in the thread as your message, with a chip naming the page and version. If the agent is working, it joins the thread’s queue. The page says “Sent 2m ago”.',
      good: 'You see what goes to the agent; nothing is sent by accident.', cost: 'Two clicks to send.',
      mock: () => stack([['The page', browser(pageShell(pageHead(), layoutsBody(), { over: sendPop }), { h: 470 })], ['The thread', sentThread('Picked Summary beside. Comment: Keep the totals sticky on phones too.')]]) },
    { key: 'B', name: 'The page’s own buttons', from: 't3code (its page-to-app messages)',
      desc: 'No Send in the header. The agent puts its own button on the page (“Build this one”), which sends straight to the thread; the header then says “Sent to thread · just now”.',
      good: 'Each page sends at the right moment, its own way.', cost: 'Sends on one click without a look; every page has to build its button.',
      mock: () => stack([['The page', browser(pageShell(pageHead(LAYOUT_ART, { send: false, extra: `<span class="sm okc row g1">${ic('check', 'xs')}Sent to thread · just now</span>` }), layoutsBody().replace('</div></div></div>', '</div></div><div class="row" style="justify-content:flex-end;margin-top:16px"><span class="btn primary">Build Summary beside</span></div></div>')), { h: 470 })], ['The thread', sentThread('Build Summary beside. Keep the totals sticky on phones too.')]]) },
    { key: 'C', name: 'Saved as you go, then told', from: 'the design board today',
      desc: 'What you pick and write is saved with the page as you go, as the board saves <code>choices.json</code>; the agent reads it with a tool. The header’s “Tell the agent” sends a short message (“I’ve answered on Checkout layouts.”) and the agent reads the answers itself.',
      good: 'Nothing is lost if you close the page; long pages answered over days.', cost: 'The agent needs a tool to read the answers, and the thread doesn’t show what you picked.',
      mock: () => stack([['The page', browser(pageShell(pageHead(LAYOUT_ART, { send: false, extra: `<span class="sm ph">Saved</span><span class="btn primary">${ic('send', 'xs')}Tell the agent</span>` }), layoutsBody()), { h: 470 })], ['The thread', sentThread('I’ve answered on Checkout layouts.')]]) },
    { key: 'D', name: 'Copy as prompt', from: 'Claude Code (its “Copy as prompt” pages)',
      desc: 'The header’s Copy as Prompt copies the text; you paste it into the thread’s composer and send it.',
      good: 'Works with no link between the page and the app.', cost: 'Switching windows and pasting each time.',
      mock: () => browser(pageShell(pageHead(LAYOUT_ART, { send: false, extra: `<span class="btn primary">${ic('copy', 'xs')}Copy as Prompt</span>` }), layoutsBody()), { h: 470 }) },
    { key: 'E', name: 'Into the composer', from: 'new (agentZ’s unsent text)',
      desc: 'Send to thread puts the text in the thread’s composer with the page’s chip, unsent, and agentZ shows that thread; you edit and send it there.',
      good: 'You send from the app, with all of the composer’s tools.', cost: 'agentZ comes to the front; one more step.',
      mock: () => threadFrame({ convo: `${CHECKOUT_WORK}${pubCard(LAYOUT_ART)}${CHECKOUT_AFTER}${worked()}`, composerTop: `<div class="row g2" style="padding:0 0 6px"><span class="chip">${ic('app-window', 'xs')}Checkout layouts · v3</span></div><div style="padding:0 0 4px">Picked Summary beside. Comment: Keep the totals sticky on phones too.</div>` }, { w: 900, h: 400 }).replace('<div class="ph" style="padding:2px 0 12px">Message the agent…</div>', '') },
  ],
});

// 12. Live updates --------------------------------------------------------------------------------
const liveBanner = `<div class="banner">${ic('refresh', 'sm')}<span>v4 was just published.</span><span class="lnk" style="font-size:13px">Show v4</span><span class="grow"></span><span class="xs ph">What you picked here stays until you send it</span></div>`;
const liveToast = `<div class="ptoast">${ic('refresh', 'sm')}<span>Updated to v4</span><span class="lnk" style="font-size:13px">Show v3</span></div>`;
TOPICS.push({
  id: 'live', section: 'The page in the browser', title: 'When it’s published again while open', size: 'wide', rec: 'C',
  now: 'Nothing yet. Claude Code’s open pages update in place when the agent publishes again, for a page the agent keeps up to date as it works (a checklist, a timeline). A design board shows a round’s latest when you reload it.',
  issues: ['An update while you’re picking on the page could lose what you did'],
  options: [
    { key: 'A', name: 'Updates in place', from: 'Claude Code',
      desc: 'The page reloads to the new version where you were scrolled, and a note at the bottom says “Updated to v4”, with a way back to v3.',
      good: 'A page of progress keeps up without a click.', cost: 'What you picked or typed on the page and didn’t send is gone.',
      mock: () => browser(pageShell(pageHead(LAYOUT_ART, { version: 4, latest: 4 }), layoutsBody({ picked: 'b', comment: '' }), { over: liveToast }), { h: 470 }) },
    { key: 'B', name: 'A bar to show it', from: 'new',
      desc: 'The page stays on v3 with a bar over it: “v4 was just published. Show v4”.',
      good: 'Nothing moves while you read or pick.', cost: 'A progress page needs a click for each update.',
      mock: () => browser(pageShell(pageHead(LAYOUT_ART, { version: 3, latest: 4 }), layoutsBody(), { banner: liveBanner.replace(/<span class="grow"><\/span>.*?<\/span>/, '') }), { h: 470 }) },
    { key: 'C', name: 'In place, unless you’ve picked something', from: 'new',
      desc: 'As A, unless you’ve picked or typed something on the page that isn’t sent; then B’s bar, so what you did isn’t lost.',
      good: 'Progress pages update by themselves, and your answers are safe.', cost: 'Two behaviors; needs agentZ’s script to see input on the page.',
      mock: () => stack([['Nothing picked: it updates', browser(pageShell(pageHead(BENCH_ART, { version: 6, latest: 6, send: false }), benchBody({ rows: [...BENCH, ['Leaky bucket', 11.1, 57]] }), { over: liveToast.replace('v4', 'v6').replace('v3', 'v5') }), { title: 'Limiter benchmarks', icon: 'chart', h: 400 })], ['A pick not sent: the bar', browser(pageShell(pageHead(LAYOUT_ART, { version: 3, latest: 4 }), layoutsBody(), { banner: liveBanner }), { h: 400 })]]) },
  ],
});

// 13. Markdown ------------------------------------------------------------------------------------
const mdPage = (body) => browser(pageShell(pageHead(NOTES_ART, { send: false }), body), { title: 'Release notes 0.9', icon: 'file', url: '127.0.0.1:47120/a/0c55d2', h: 620 });
TOPICS.push({
  id: 'markdown', section: 'The page in the browser', title: 'Markdown pages', size: 'wide', rec: 'B',
  now: 'Nothing yet. An artifact can be a .md file. Claude Code shows one as “a styled document page with syntax-highlighted code”. agentZ draws Markdown in threads with Zed’s markdown: 14 px text, its headings, code blocks with the theme’s colors. The mocks use the header topic’s A.',
  issues: [],
  options: [
    { key: 'A', name: 'Like the thread’s messages', from: 'agentZ’s thread (Zed’s markdown)',
      desc: 'The thread’s look: 14 px text, its headings and code blocks, in a column as wide as the thread’s (760 px).',
      good: 'Reads as the agent’s reply did.', cost: 'Small for a long document.',
      mock: () => mdPage(`<div class="pin doc" style="max-width:760px;font-size:14px;line-height:22px">${NOTES_MD.replace('<h1>', '<h1 style="font-size:20px">')}</div>`) },
    { key: 'B', name: 'A document page', from: 'Claude Code (its styled document pages)',
      desc: 'A page for reading: 15 px text with more space between lines, a larger title, code in the theme’s syntax colors, ruled tables, in a 720 px column. Prints well.',
      good: 'Comfortable for specs, reports and release notes.', cost: 'Looks unlike the thread.',
      mock: () => mdPage(`<div class="pin doc" style="max-width:720px">${NOTES_MD}</div>`) },
    { key: 'C', name: 'With contents beside', from: 'new',
      desc: 'B, with the headings listed at the left, the one in view in the accent; a click scrolls to it. Left out for a page with fewer than three headings.',
      good: 'Long documents are easy to move around.', cost: 'Takes width; little use for a short page.',
      mock: () => mdPage(`<div class="row" style="height:100%;align-items:flex-start;justify-content:center"><div class="toc"><span class="xs ph">On this page</span><span>Release notes 0.9</span><span class="on">New</span><span>Changed</span></div><div class="pin doc" style="max-width:720px;margin:0">${NOTES_MD}</div><div style="width:200px"></div></div>`) },
    { key: 'D', name: 'As paper', from: 'new',
      desc: 'A sheet the width of A4 on a darker backdrop, with margins as it prints, so what you see is what Print or Save as PDF gives.',
      good: 'For documents meant for other people.', cost: 'Less room; reads like a file, not a page.',
      mock: () => mdPage(`<div style="background:#141517;height:100%;padding:24px 0"><div class="doc" style="width:640px;margin:0 auto;background:var(--ed);border:1px solid var(--b);box-shadow:0 8px 30px rgba(0,0,0,.5);padding:44px 56px;font-size:14px">${NOTES_MD}</div></div>`) },
  ],
});

// 14. Deleting the thread -----------------------------------------------------------------------
const dialog = (title, body, { extra = '', w = 560, h = 230 } = {}) => frame(`<div style="position:absolute;inset:0;background:rgba(0,0,0,.45)"></div><div class="dlg" style="left:${(w - 380) / 2}px;top:24px"><h4>${title}</h4><p>${body}</p>${extra}<div class="row g2" style="justify-content:flex-end"><span class="btn">Cancel</span><span class="btn danger">Delete</span></div></div>`, { w, h, style: 'background:var(--panel);border-radius:8px' }).replace('class="m"', 'class="m jb"');
const DEL_TITLE = 'Delete “Add the checkout page”?';
TOPICS.push({
  id: 'delete', section: 'When the thread goes', title: 'Deleting the thread or chat that published it', size: 'wide', rec: 'C',
  now: 'Undecided in the backlog. Deleting a thread asks first, “Delete “Rate limiter tests”? The thread and its conversation will be removed…” (screenshot); archiving keeps everything. An artifact records only the thread or chat that published it. Claude Code’s artifacts live in your claude.ai account apart from sessions, so they stay; t3code keeps its pages with the thread, so they go with it.',
  nowImg: 'img/now-delete.png',
  issues: ['Some pages are worth keeping (a report, a spec), others were for one question'],
  options: [
    { key: 'A', name: 'Kept', from: 'Claude Code (artifacts apart from sessions)',
      desc: 'The thread goes, its pages stay. The list says “from a deleted thread” where the thread was, and the header no longer links to it. Delete one from the list’s menu.',
      good: 'Nothing worth keeping is lost by deleting a thread.', cost: 'Pages pile up unless you clear them.',
      mock: () => stack([['The dialog, as today', dialog(DEL_TITLE, 'The thread and its conversation will be removed. Its 2 artifacts are kept.')], ['The list afterwards', sbFrame(`<div class="asb" style="width:100%;padding-top:4px">${shelf('Artifacts', 4, { open: true, hl: true })}${artRow(LAYOUT_ART, { extra: 'From a deleted thread' })}${artRow(SPEC, { extra: 'From a deleted thread' })}${artRow(BENCH_ART)}</div>`, 170)]]) },
    { key: 'B', name: 'Deleted with it', from: 't3code (pages kept with the thread)',
      desc: 'Deleting the thread deletes its pages and their versions. The dialog says so when it has some.',
      good: 'Nothing left behind.', cost: 'A report you wanted goes with a thread you didn’t.',
      mock: () => dialog(DEL_TITLE, 'The thread, its conversation and its 2 artifacts (Checkout layouts, Checkout spec) will be removed.') },
    { key: 'C', name: 'Asked in the dialog', from: 'new',
      desc: 'When the thread has pages, the dialog has “Also delete its 2 artifacts (Checkout layouts, Checkout spec)”, off at first, so they’re kept unless you tick it.',
      good: 'You decide each time, with the names in front of you.', cost: 'One more thing in the dialog.',
      mock: () => dialog(DEL_TITLE, 'The thread and its conversation will be removed.', { extra: `<div class="row g2" style="margin:-2px 0 14px;color:var(--t)"><span class="check"></span>Also delete its 2 artifacts<span class="ph">(Checkout layouts, Checkout spec)</span></div>` }) },
    { key: 'D', name: 'Deleted, unless kept', from: 'new',
      desc: 'Pages go with their thread, but any you mark Keep (in its menu, or the card’s) stay, as A’s. The dialog names the ones that will go.',
      good: 'Keeping is a choice you make once, when you see the page is worth it.', cost: 'You have to remember to mark them.',
      mock: () => side([['The card’s menu', frame(`<div style="padding:12px">${pubCard(LAYOUT_ART)}</div><div class="menu" style="top:58px;right:12px;min-width:200px"><div class="it">${ic('copy', 'sm')}Copy Link</div><div class="it">${ic('download', 'sm')}Export</div><div class="hr"></div><div class="it hl">${ic('pin', 'sm')}Keep<span class="kb">after the thread</span></div></div>`, { w: 520, h: 210, style: 'background:var(--panel);border-radius:8px' }).replace('class="m"', 'class="m jb"')], ['The dialog', dialog(DEL_TITLE, 'The thread, its conversation and its artifact Checkout spec will be removed. Checkout layouts is kept.', { w: 480 })]]) },
  ],
});
