// Chats: threads for general conversation, outside every project. Where they sit in the
// sidebar, starting one, what a chat shows, mentions, its agent's tools, and the setting.

const NOW_SIDEBAR = 'The sidebar lists thread cards, every one in a project (the project’s icon and name, its state, the title, its branch and machine), and the Shells, Workspaces and Archived shelves rest at the bottom: a label with its count, a rule and a chevron. Threads started in Workspaces panes are the only ones outside the cards today.';

// Sidebar ----------------------------------------------------------------------------------------
TOPICS.push({
  id: 'group', section: 'Sidebar', title: 'Where the Chats group sits', size: 'narrow', rec: 'A',
  now: NOW_SIDEBAR,
  nowImg: 'img/now-sidebar.png',
  options: [
    { key: 'A', name: 'The first shelf at the bottom', from: 'agentZ’s shelves (t3code’s Settled shelf)',
      desc: 'A Chats shelf above Shells, Workspaces and Archived, styled like them, with a + on its header for a new chat. Like them it rests at the bottom while the list is short and scrolls with the cards.',
      good: 'Fits beside the shelves it’s modeled on; projects stay first.', cost: 'Below a long list of cards, chats are a scroll away.',
      mock: () => sbFrame(sidebarJB({ shelves: `${shelf('Chats', CHATS.length, { open: true, plus: true })}${chatRows()}${archivedShelf()}` }), { h: 620 }) },
    { key: 'B', name: 'A group at the top', from: 'new',
      desc: 'Chats come first, above the thread cards, under their own header with the +. The cards follow under a Threads header.',
      good: 'Chats are always in view.', cost: 'Pushes project work down; five chats take the space of four cards.',
      mock: () => sbFrame(sidebarJB({ top: `<div style="flex:none;padding:0 4px 4px">${shelf('Chats', CHATS.length, { open: true, plus: true })}${chatRows({}, CHATS.slice(0, 4))}${shelf('Threads', null, { open: true })}</div>` }), { h: 620 }) },
    { key: 'C', name: 'Fixed above Settings', from: 'new',
      desc: 'Like A, but the Chats shelf stays in place above Settings while the cards scroll above it. Open, it takes up to a third of the sidebar and scrolls on its own.',
      good: 'Reachable however many threads there are.', cost: 'Two lists that scroll apart; less room for cards.',
      mock: () => sbFrame(sidebarJB({ list: `${cards()}${tcard({ id: 'x', project: 'ap', title: 'Retry failed webhooks', state: 'done', branch: 'retries', glyph: 'codex' })}${tcard({ id: 'y', project: 'st', title: 'Lazy-load the product images', state: 'done', branch: 'lazy-images', glyph: 'claude' })}`, shelves: `<div style="border-top:1px solid var(--b);margin:0 -4px;padding:2px 4px 0">${shelf('Chats', CHATS.length, { open: true, plus: true })}${chatRows({}, CHATS.slice(0, 4))}</div>${archivedShelf()}` }), { h: 620 }) },
  ],
});

TOPICS.push({
  id: 'rows', section: 'Sidebar', title: 'What a chat’s row shows', size: 'narrow', rec: 'B',
  now: 'Archived and Workspaces rows are one line: the project’s icon, the title and when it last did something, muted. Thread cards have three lines with the state.',
  nowImg: 'img/now-sidebar.png',
  options: [
    { key: 'A', name: 'Like Archived rows', from: 'agentZ’s Archived shelf',
      desc: 'One muted line: a chat icon, the title and the time.',
      good: 'Quiet; many chats fit.', cost: 'A working chat, or one waiting for you, looks like the rest.',
      mock: () => sbFrame(sidebarJB({ list: cards(''), shelves: `${shelf('Chats', CHATS.length, { open: true, plus: true })}${chatRows({ onId: 'async' })}${archivedShelf()}` }), { h: 620 }) },
    { key: 'B', name: 'One line with its state', from: 'agentZ’s Archived rows, with the cards’ state dots',
      desc: 'Like A, but a chat that’s working or waiting for you reads at full strength with its state’s dot before the time.',
      good: 'Still one line, and says which chats need you.', cost: 'A dot is easy to miss beside the cards’ pills.',
      mock: () => sbFrame(sidebarJB({ list: cards(''), shelves: `${shelf('Chats', CHATS.length, { open: true, plus: true })}${chatRows({ state: true, onId: 'async' })}${archivedShelf()}` }), { h: 620 }) },
    { key: 'C', name: 'The agent’s icon in front', from: 'new',
      desc: 'Like B, with the chat’s agent’s icon where the chat icon is, since every row is a chat anyway.',
      good: 'Says who you were talking to.', cost: 'Agent icons are small and look alike in a column.',
      mock: () => sbFrame(sidebarJB({ list: cards(''), shelves: `${shelf('Chats', CHATS.length, { open: true, plus: true })}${chatRows({ state: true, lead: 'agent', onId: 'async' })}${archivedShelf()}` }), { h: 620 }) },
    { key: 'D', name: 'Cards, like threads', from: 'agentZ’s thread cards',
      desc: 'Three lines like a thread’s card: Chat where the project is, with the state; the title; the agent, machine and agent icon where the branch is.',
      good: 'Same look as threads; the state is as clear.', cost: 'Three times the height; the group fills up fast.',
      mock: () => sbFrame(sidebarJB({ list: cards('', THREADS.slice(0, 2)), shelves: `${shelf('Chats', CHATS.length, { open: true, plus: true })}${CHATS.slice(0, 3).map((chat) => chatCard(chat, { on: chat.id === 'async' })).join('')}${archivedShelf()}` }), { h: 620 }) },
  ],
});

TOPICS.push({
  id: 'open', section: 'Sidebar', title: 'Open or closed', size: 'narrow', rec: 'C',
  now: 'Shells starts open each time the app starts. Workspaces and Archived remember whether you left them open (kept by this Mac’s server) and start closed; a closed shelf shows its count. Archived shows its rows a page at a time, with Show more.',
  nowImg: 'img/now-sidebar.png',
  options: [
    { key: 'A', name: 'Remembered, closed at first', from: 'agentZ’s Archived and Workspaces shelves',
      desc: 'Starts closed with its count; opening or closing it is remembered across restarts.',
      good: 'Out of the way for project work.', cost: 'A chat waiting for you stays hidden until a notification says so.',
      mock: () => sbFrame(sidebarJB({ shelves: `${shelf('Chats', CHATS.length, { plus: true })}${archivedShelf()}` }), { h: 520 }) },
    { key: 'B', name: 'Remembered, open at first', from: 'agentZ’s shelves',
      desc: 'Like A, but it starts open the first time.',
      good: 'New chats are seen at once.', cost: 'Every chat ever made, until you close it.',
      mock: () => sbFrame(sidebarJB({ shelves: `${shelf('Chats', CHATS.length, { open: true, plus: true })}${chatRows({ state: true })}${archivedShelf()}` }), { h: 580 }) },
    { key: 'C', name: 'Open, the latest few', from: 'agentZ’s Archived pages',
      desc: 'Remembered like A and B, but open it shows the five latest chats and Show 12 more, as Archived pages its rows.',
      good: 'Recent chats in view without a long list.', cost: 'Older chats need a click or a search.',
      mock: () => sbFrame(sidebarJB({ shelves: `${shelf('Chats', 17, { open: true, plus: true })}${chatRows({ state: true })}<div class="lrow quiet sm" style="height:26px;padding-left:30px">Show 12 more</div>${archivedShelf()}` }), { h: 610 }) },
  ],
});

const searchRow = (icon, title, { label = '', ago = '1h', on = false } = {}) => `<div class="lrow ${on ? 'on' : ''}">${icon}<span class="grow trunc">${title}</span>${label ? `<span class="xs" style="color:var(--ph)">${label}</span>` : ''}<span class="when">${ago}</span></div>`;
const searchHead = (query) => `${ic('search', 'sm')}<span class="grow" style="color:var(--t)">${query}</span>${ic('x', 'sm mu')}${ic('plus', 'sm mu')}`;
const RESULTS = [
  { icon: () => projectMono('st'), title: 'Add the checkout page', ago: 'now' },
  { icon: () => chatIcon(), title: 'Postgres or SQLite for the cache', ago: '4m', chat: true },
  { icon: () => projectMono('st'), title: 'Rename the cart store', ago: '2d', label: 'Archived' },
  { icon: () => chatIcon(), title: 'A regex for the access log', ago: '3d', chat: true },
];
TOPICS.push({
  id: 'search', section: 'Sidebar', title: 'Chats in search', size: 'narrow', rec: 'A',
  now: 'Typing in Search lists matching threads as one line each: the project’s icon, the title, Archived for archived ones, and the time, newest first.',
  nowImg: 'img/now-search.png',
  options: [
    { key: 'A', name: 'Among the threads, with the chat icon', from: 'agentZ’s search',
      desc: 'Chats are results like threads, in the same order, with the chat icon where a project’s icon is.',
      good: 'One list, nothing new to learn.', cost: 'The icon alone tells chats apart.',
      mock: () => sbFrame(sidebarJB({ head: searchHead('ca'), list: `<div class="col" style="gap:2px">${RESULTS.map((r, i) => searchRow(r.icon(), r.title, { label: r.label, ago: r.ago, on: i === 0 })).join('')}</div>`, shelves: '' }), { h: 300 }) },
    { key: 'B', name: 'Among the threads, marked Chat', from: 'agentZ’s Archived label',
      desc: 'Like A, with Chat in faint text before the time, as Archived is.',
      good: 'Says it in words.', cost: 'Less room for the title.',
      mock: () => sbFrame(sidebarJB({ head: searchHead('ca'), list: `<div class="col" style="gap:2px">${RESULTS.map((r, i) => searchRow(r.icon(), r.title, { label: r.chat ? 'Chat' : r.label, ago: r.ago, on: i === 0 })).join('')}</div>`, shelves: '' }), { h: 300 }) },
    { key: 'C', name: 'Chats after the threads', from: 'new',
      desc: 'Threads first, then a Chats label and the matching chats.',
      good: 'Project work first, chats apart.', cost: 'A recent chat sits below old threads.',
      mock: () => sbFrame(sidebarJB({ head: searchHead('ca'), list: `<div class="col" style="gap:2px">${RESULTS.filter((r) => !r.chat).map((r, i) => searchRow(r.icon(), r.title, { label: r.label, ago: r.ago, on: i === 0 })).join('')}${shelf('Chats', null, { open: true })}${RESULTS.filter((r) => r.chat).map((r) => searchRow(r.icon(), r.title, { ago: r.ago })).join('')}</div>`, shelves: '' }), { h: 320 }) },
  ],
});

// Starting a chat --------------------------------------------------------------------------------
const pickerModal = (rows, { title = 'New thread in…', query = '' } = {}) => `<div class="pop" style="left:50%;top:40px;transform:translateX(-50%);width:520px;padding:0">
  <div class="row g2" style="height:40px;padding:0 12px;border-bottom:1px solid var(--b);font-size:13px">${ic('search', 'sm mu')}<span class="mu">${title}</span><span class="grow" style="${query ? 'color:var(--t)' : 'color:var(--ph)'}">${query || 'Search projects…'}</span></div>
  <div class="col" style="padding:4px;gap:1px">${rows.join('')}</div></div>`;
const pickRow = (icon, name, hint, { hl = false } = {}) => `<div class="lrow ${hl ? 'hov' : ''}">${icon}<span>${name}</span><span class="grow trunc xs" style="color:var(--ph)">${hint}</span></div>`;
const PROJECT_ROWS = [
  pickRow(projectMono('st'), 'storefront', '~/code/storefront'),
  pickRow(projectMono('ap'), 'api', '~/code/api · Devbox 1'),
  pickRow(projectMono('dc'), 'docs', '~/code/docs'),
];
TOPICS.push({
  id: 'start', section: 'Starting', title: 'How a chat is started', size: 'medium', type: 'multi', rec: 'ABC',
  now: 'New Thread (the + at the top of the sidebar, or Ctrl-N) opens New thread in…, a list of projects to pick from, when several are shown; with one it opens a draft there. Only a Workspaces pane starts a thread outside a project.',
  nowImg: 'img/now-project-picker.png',
  options: [
    { key: 'A', name: 'The + on the Chats header', from: 'new (the shelf’s own button)',
      desc: 'Hovering the Chats header shows a +, which opens a new chat on this Mac.',
      good: 'Right where chats are.', cost: 'Hidden while the shelf is scrolled away.',
      mock: () => sbFrame(sidebarJB({ list: cards('', THREADS.slice(0, 3)), shelves: `${shelf('Chats', CHATS.length, { open: true, plus: true, hov: true, tone: 'var(--t)' })}${chatRows({ state: true }, CHATS.slice(0, 3))}${archivedShelf()}` }), { h: 480, over: `<span class="overlay-note" style="right:22px;top:258px">New Chat</span>` }) },
    { key: 'B', name: 'Chat first in New thread in…', from: 't3code (No project)',
      desc: 'The project picker New Thread opens lists Chat first, above the projects: Ctrl-N and Enter start a chat.',
      good: 'One way in for both, from the keyboard.', cost: 'A project thread is one key further when Chat is first; t3code puts No project first too.',
      mock: () => jb(frame(pickerModal([pickRow(chatIcon(), 'Chat', 'outside every project', { hl: true }), '<div class="hr" style="height:1px;background:var(--bv);margin:3px 0"></div>', ...PROJECT_ROWS]), { w: 600, h: 230, style: 'background:var(--ed)' })) },
    { key: 'C', name: 'A New Chat command and shortcut', from: 't3code (mod+alt+n)',
      desc: 'New Chat in the command palette and on Ctrl-Alt-N (Cmd-Alt-N on the Mac).',
      good: 'Straight to a chat from anywhere.', cost: 'One more shortcut.',
      mock: () => jb(frame(`<div class="pop" style="left:50%;top:30px;transform:translateX(-50%);width:520px;padding:0"><div class="row g2" style="height:40px;padding:0 12px;border-bottom:1px solid var(--b);font-size:13px">${ic('command', 'sm mu')}<span style="color:var(--t)">chat</span></div><div class="col" style="padding:4px;gap:1px"><div class="lrow hov">${ic('chat', 'sm mu')}<span class="grow">New Chat</span><span class="kbd">Ctrl-Alt-N</span></div><div class="lrow">${ic('plus', 'sm mu')}<span class="grow">New Thread</span><span class="kbd">Ctrl-N</span></div></div></div>`, { w: 600, h: 160, style: 'background:var(--ed)' })) },
    { key: 'D', name: 'The headline’s project is a menu', from: 't3code (DraftHeroHeadline)',
      desc: 'A new thread’s headline names its project, “What should we build in storefront?”, and the name opens a menu with Chat, the projects and Add Project…. Under it, “or start a chat”.',
      good: 'Change your mind on the draft itself.', cost: 'Changes every new thread’s screen.',
      mock: () => jb(frame(`<div class="col" style="height:100%;align-items:center;padding-top:40px;gap:6px;position:relative"><div style="font-size:22px">What should we build in <span style="border-bottom:1px dotted var(--mu)">storefront</span> ${chev}?</div><div class="sm" style="color:var(--ph);border-bottom:1px dotted var(--ph)">or start a chat</div>
        <div class="menu" style="top:80px;left:310px;width:220px"><div class="it">${ic('chat', 'sm')}<span class="grow">Chat</span></div><div class="hr"></div><div class="it hl">${projectMono('st')}<span class="grow">storefront</span>${ic('check', 'sm ac')}</div><div class="it">${projectMono('ap')}<span class="grow">api</span></div><div class="it">${projectMono('dc')}<span class="grow">docs</span></div><div class="hr"></div><div class="it">${ic('folder-plus', 'sm')}<span class="grow">Add Project…</span></div></div></div>`, { w: 760, h: 290, style: 'background:var(--ed)' })) },
  ],
});

TOPICS.push({
  id: 'screen', section: 'Starting', title: 'The new chat screen', size: 'wide', rec: 'A',
  now: 'A new thread’s screen: “What should we work on?”, the composer card with the agent, and a strip under it with the checkout (Local, New worktree, New pasture), the machine and account when there are several, and the branch at the right end.',
  nowImg: 'img/now-new-thread.png',
  options: [
    { key: 'A', name: 'A thread’s screen, without the checkout', from: 't3code (threads without a project)',
      desc: 'The same headline and composer. The strip keeps the machine (a chat starts on this Mac and can move before its first message) and the account; the checkout and branch are gone.',
      good: 'Nothing new to learn.', cost: 'The headline reads as project work.',
      mock: () => jb(frame(newScreen({ left: `${machineChip()}${accountChip()}` }), { w: 820, h: 400, style: 'border-radius:8px;border:1px solid #111' })) },
    { key: 'B', name: 'Its own headline', from: 'new',
      desc: 'Like A, with “What do you want to talk about?” as the headline.',
      good: 'Says it’s a conversation, not work in a project.', cost: 'One more string to keep in step.',
      mock: () => jb(frame(newScreen({ headline: 'What do you want to talk about?', left: `${machineChip()}${accountChip()}` }), { w: 820, h: 400, style: 'border-radius:8px;border:1px solid #111' })) },
    { key: 'C', name: 'Where it works, at the right', from: 'new',
      desc: 'Like B, and the strip’s right end, where a thread’s branch is, says “Its own folder”, opening the folder once it exists.',
      good: 'Says where the agent’s files go.', cost: 'A folder most people never open.',
      mock: () => jb(frame(newScreen({ headline: 'What do you want to talk about?', left: `${machineChip()}${accountChip()}`, right: `${ic('folder', 'xs')}Its own folder` }), { w: 820, h: 400, style: 'border-radius:8px;border:1px solid #111' })) },
  ],
});

// In a chat ------------------------------------------------------------------------------------
const headFrame = (opts) => jb(frame(threadView({ convo: ASYNC_CONVO, ...opts }), { w: 900, h: 300, style: 'border-radius:8px;border:1px solid #111' }));
TOPICS.push({
  id: 'header', section: 'In a chat', title: 'A chat’s header', size: 'wide', rec: 'B',
  now: 'A thread’s header has the project’s icon and name, the title with its menu, then the branch, Diff, Terminal and More. The backlog hides worktrees, pastures, diffs, branches and project scripts in a chat.',
  nowImg: 'img/now-sidebar.png',
  options: [
    { key: 'A', name: 'Chat and the title, More only', from: 't3code (hidden branch and diff)',
      desc: 'Chat where the project is, then the title. Only More stays at the right.',
      good: 'The quietest header.', cost: 'No terminal in the chat’s folder.',
      mock: () => headFrame({ right: ibtn('more') }) },
    { key: 'B', name: 'With the terminal', from: 'agentZ’s thread header',
      desc: 'Like A, with Terminal kept: it opens in the chat’s folder, where the agent’s files are.',
      good: 'Run what the agent made, or look at its files.', cost: 'A button few chats need.',
      mock: () => headFrame({ right: `${ibtn('terminal')}${ibtn('more')}` }) },
    { key: 'C', name: 'The title alone', from: 'new',
      desc: 'Like B, without the Chat lead: the chat icon, then the title.',
      good: 'More room for the title.', cost: 'Doesn’t say it’s a chat in words.',
      mock: () => headFrame({ lead: chatIcon(), right: `${ibtn('terminal')}${ibtn('more')}` }).replace('<span class="ph">/</span>', '') },
  ],
});

TOPICS.push({
  id: 'folder', section: 'In a chat', title: 'The chat’s folder', size: 'medium', rec: 'A',
  now: 'A thread works in its project’s folder, worktree or pasture, and a Workspaces thread in its pane’s folder. A chat needs a folder too (ACP’s session/new takes one), so each would get its own in the data folder, named as t3code’s are: the date, the first words of its first message and a short id.',
  options: [
    { key: 'A', name: 'Not shown', from: 't3code',
      desc: 'The folder is made when the first message is sent and never shown. The agent’s files are there for the agent; Storage settings show its size.',
      good: 'Nothing to explain.', cost: 'Files the agent wrote are hard to find.',
      mock: () => jb(frame(threadView({ convo: ASYNC_CONVO, composer: '', right: `${ibtn('terminal')}${ibtn('more')}` }), { w: 640, h: 210, style: 'border-radius:8px;border:1px solid #111' })) },
    { key: 'B', name: 'Open Folder in More', from: 'new',
      desc: 'More has Open Folder (in Finder or the file manager) and Copy Path.',
      good: 'There when wanted, out of the way otherwise.', cost: 'One more menu item.',
      mock: () => jb(frame(`${threadView({ convo: ASYNC_CONVO, right: `${ibtn('terminal')}<span class="ibtn on">${ic('more', 'sm')}</span>` })}<div class="menu" style="right:8px;top:36px;width:210px"><div class="it">${ic('pencil', 'sm')}<span class="grow">Rename</span></div><div class="it hl">${ic('folder-open', 'sm')}<span class="grow">Open Folder</span></div><div class="it">${ic('copy', 'sm')}<span class="grow">Copy Path</span></div><div class="hr"></div><div class="it">${ic('archive', 'sm')}<span class="grow">Archive</span></div><div class="it">${ic('trash', 'sm')}<span class="grow">Delete…</span></div></div>`, { w: 640, h: 300, style: 'border-radius:8px;border:1px solid #111' })) },
    { key: 'C', name: 'A folder button in the header', from: 'agentZ’s branch button',
      desc: 'Where a thread’s branch button is, a button with the folder’s name opens it.',
      good: 'Always in view.', cost: 'A long, dated name in the header.',
      mock: () => jb(frame(threadView({ convo: ASYNC_CONVO, composer: '', right: `<span class="bbtn">${ic('folder', 'xs')}2026-10-10-compare-tokio-smol-a1b2</span>${ibtn('terminal')}${ibtn('more')}` }), { w: 640, h: 210, style: 'border-radius:8px;border:1px solid #111' })) },
  ],
});

const mentionFrame = (menu, { text = '@', h = 400 } = {}) => jb(frame(threadView({ convo: ASYNC_CONVO, right: `${ibtn('terminal')}${ibtn('more')}`, composer: composerBox({ text: `${text}<span class="cursor" style="width:1px;height:16px;background:var(--t)"></span>`, overlay: menu }) }), { w: 640, h, style: 'border-radius:8px;border:1px solid #111' }));
const projectRows = (hl = 0) => ['st', 'ap', 'dc'].map((key, i) => mrow(projectMono(key), PROJECTS[key].name, key === 'ap' ? 'Devbox 1' : '', { hl: i === hl }));
const threadRows = (list = THREADS.slice(0, 3)) => list.map((thread) => mrow(ic('chat', 'sm'), thread.title, PROJECTS[thread.project].name));
TOPICS.push({
  id: 'mentions', section: 'In a chat', title: 'The @ menu in a chat', size: 'medium', rec: 'A',
  now: 'In a thread, @ lists Files (in the thread’s folder) and Threads (the project’s other threads), up to eight and five. A mentioned file goes as a link, and a mentioned thread as its conversation. A chat has no project, so the menu needs projects and every project’s threads.',
  nowImg: 'img/now-mention.png',
  options: [
    { key: 'A', name: 'Projects, Threads and Files', from: 'agentZ’s @ menu (t3code’s groups)',
      desc: 'Three groups: Projects (a project goes as its folder), Threads from every project with the project’s name, and Files in the chat’s own folder. Typing filters them all.',
      good: 'Today’s menu with one more group.', cost: 'Another project’s files can’t be mentioned one by one.',
      mock: () => mentionFrame(mentionMenu([mgroup('Projects', projectRows()), mgroup('Threads', threadRows()), mgroup('Files', [mrow(ic('file', 'sm'), 'proxy-bench.md')])])) },
    { key: 'B', name: 'A project’s files after a slash', from: 'new',
      desc: 'Like A, and typing a project’s name and a slash (storefront/) lists that project’s files and folders.',
      good: 'Any project’s file, from the keyboard.', cost: 'A rule to know about.',
      mock: () => mentionFrame(mentionMenu([mgroup('storefront', [mrow(ic('folder', 'sm'), 'src', '', { hl: true }), mrow(ic('file', 'sm'), 'README.md'), mrow(ic('file', 'sm'), 'package.json'), mrow(ic('file', 'sm'), 'checkout.tsx', 'src/app/checkout')])]), { text: '@storefront/' }) },
    { key: 'C', name: 'Go into a project', from: 'Zed (the @ menu’s categories)',
      desc: 'Projects first; picking one with the right arrow goes into it, listing its files and threads, and Left comes back.',
      good: 'Every project’s files and threads, in reach.', cost: 'Two steps for everything inside a project.',
      mock: () => mentionFrame(mentionMenu([`<div class="it" style="color:var(--mu)">${ic('chev-right', 'xs')}${projectMono('st')}<span class="grow">storefront</span></div><div class="hr"></div>`, mgroup('Files', [mrow(ic('folder', 'sm'), 'src', '', { hl: true }), mrow(ic('file', 'sm'), 'README.md')]), mgroup('Threads', threadRows([THREADS[0], THREADS[2]]))]), { text: '@storefront ' }) },
    { key: 'D', name: 'Projects and threads only', from: 'new',
      desc: 'Projects and Threads, no files: a mentioned project’s folder is enough for the agent to read what it needs.',
      good: 'The shortest menu.', cost: 'You can’t point at one file.',
      mock: () => mentionFrame(mentionMenu([mgroup('Projects', projectRows()), mgroup('Threads', threadRows())])) },
  ],
});

TOPICS.push({
  id: 'machines', section: 'In a chat', title: 'Projects and threads on other machines', size: 'medium', rec: 'B',
  now: 'A mentioned thread’s conversation is read by the server the message goes to, so only that machine’s threads can be mentioned. A chat runs on one machine (This Mac here), while api is on Devbox 1 and storefront on both.',
  options: [
    { key: 'A', name: 'Only the chat’s machine', from: 'agentZ (the server reads the mention)',
      desc: 'The @ menu lists the projects and threads on the chat’s machine only.',
      good: 'Every mention works as it does today.', cost: 'Devbox 1’s projects are missing with no word why.',
      mock: () => mentionFrame(mentionMenu([mgroup('Projects', [mrow(projectMono('st'), 'storefront', '', { hl: true }), mrow(projectMono('dc'), 'docs')]), mgroup('Threads', threadRows([THREADS[0], THREADS[2]]))]), { h: 340 }) },
    { key: 'B', name: 'Others shown, but not offered', from: 'new',
      desc: 'Other machines’ projects and threads are listed dimmed, with the machine’s name; picking one says to start the chat on that machine.',
      good: 'Says why it can’t be mentioned.', cost: 'Rows that can’t be picked.',
      mock: () => mentionFrame(mentionMenu([mgroup('Projects', [mrow(projectMono('st'), 'storefront', '', { hl: true }), mrow(projectMono('dc'), 'docs'), mrow(projectMono('ap'), 'api', `${ic('server', 'xs')} Devbox 1`, { dim: true })]), mgroup('Threads', [...threadRows([THREADS[0]]), mrow(ic('chat', 'sm'), THREADS[1].title, `${ic('server', 'xs')} Devbox 1`, { dim: true })])]), { h: 360 }) },
    { key: 'C', name: 'Every machine’s', from: 'new (agentZ’s cross-machine tools)',
      desc: 'Everything is offered. A thread on another machine goes as its conversation, fetched by the app; a project there goes as its name, machine and path, which the agent reaches through agentZ’s tools.',
      good: 'One chat about work on every machine.', cost: 'The agent can’t read another machine’s files itself; it needs the tools topic’s C or D.',
      mock: () => mentionFrame(mentionMenu([mgroup('Projects', [mrow(projectMono('st'), 'storefront', '', { hl: true }), mrow(projectMono('dc'), 'docs'), mrow(projectMono('ap'), 'api', `${ic('server', 'xs')} Devbox 1`)]), mgroup('Threads', [...threadRows([THREADS[0]]), mrow(ic('chat', 'sm'), THREADS[1].title, `${ic('server', 'xs')} Devbox 1`)])]), { h: 360 }) },
  ],
});

// What a chat's agent may do with agentZ's tools, as a table of the tool groups.
const TOOL_GROUPS = [
  ['Delegated tasks', 'delegate a task to another agent, its status, cancel'],
  ['Terminals and commands', 'start, send to and read terminals; run a command'],
  ['Reading threads', 'list threads, read one, its diff, workspace status'],
  ['Starting and changing threads', 'launch, send, wait, interrupt, rename, organize'],
  ['Workspaces', 'hand off, sync, bring a branch back'],
  ['Projects', 'add a project'],
];
const access = { yes: `<span class="row g1 okc">${ic('check', 'xs')}Yes</span>`, no: '<span class="ph">No</span>', mentioned: '<span class="row g1" style="color:var(--ac)">Mentioned projects</span>', all: `<span class="row g1 okc">${ic('check', 'xs')}Every project</span>`, folder: `<span class="row g1 okc">${ic('check', 'xs')}In its folder</span>` };
const toolTable = (marks) => jb(frame(`<div style="padding:14px 16px"><div class="sm mu" style="margin-bottom:8px">agentZ’s tools for a chat’s agent</div><div class="scard">${TOOL_GROUPS.map(([name, description], i) => `<div class="srowx" style="padding:9px 12px"><div class="grow"><div class="n">${name}</div><div class="d">${description}</div></div><span class="sm">${access[marks[i]]}</span></div>`).join('')}</div></div>`, { w: 470, h: 430, style: 'border-radius:8px;border:1px solid #111' }));
TOPICS.push({
  id: 'tools', section: 'In a chat', title: 'agentZ’s tools in a chat', size: 'medium', rec: 'B',
  now: 'A thread’s agent gets agentZ’s tools (threads, delegated tasks, workspaces, terminals, commands, add project). The thread tools work only in the caller’s project; a Workspaces thread’s work in the folder it’s in. A chat has no project.',
  options: [
    { key: 'A', name: 'Only its own folder', from: 'agentZ’s policy (the caller’s project only)',
      desc: 'Delegated tasks, terminals and commands, in the chat’s folder. No thread, workspace or project tools.',
      good: 'A chat can’t touch project work.', cost: 'Can’t look at a thread unless it’s mentioned.',
      mock: () => toolTable(['folder', 'folder', 'no', 'no', 'no', 'no']) },
    { key: 'B', name: 'Read anywhere', from: 'new',
      desc: 'Like A, plus listing and reading every project’s threads, diffs and workspaces. Nothing that starts or changes a thread.',
      good: 'Ask about any work without mentioning it first.', cost: 'Can’t act on what it finds.',
      mock: () => toolTable(['folder', 'folder', 'all', 'no', 'no', 'no']) },
    { key: 'C', name: 'Everything, in any project', from: 't3code (t3_thread_launch with scratch)',
      desc: 'All of agentZ’s tools; the thread and workspace tools take the project to act in, so a chat can start a thread in storefront.',
      good: 'A chat can plan and hand work to projects.', cost: 'An agent outside every project can change all of them.',
      mock: () => toolTable(['folder', 'folder', 'all', 'all', 'all', 'yes']) },
    { key: 'D', name: 'In the projects it mentions', from: 'new',
      desc: 'Like A, and once a chat mentions a project, its agent may read and start threads there, as a thread in that project can.',
      good: 'You decide where it may act, by mentioning.', cost: 'A rule people have to know.',
      mock: () => toolTable(['folder', 'folder', 'mentioned', 'mentioned', 'mentioned', 'no']) },
  ],
});

const deleteDialog = (body) => `<div class="modal-back"></div><div class="dlg" style="left:24px;top:150px;width:300px"><h4>Delete “Async runtimes compared”?</h4><p>${body}</p><div class="row g2" style="justify-content:flex-end"><span class="btn">Cancel</span><span class="btn danger">Delete</span></div></div>`;
const chatMenu = (items) => `<div class="menu" style="left:70px;top:330px;width:190px">${items.join('')}</div>`;
const mi = (icon, label, { hl = false } = {}) => (icon === '-' ? '<div class="hr"></div>' : `<div class="it ${hl ? 'hl' : ''}">${ic(icon, 'sm')}<span class="grow">${label}</span></div>`);
const sidebarWithChats = (onId = 'async') => sidebarJB({ list: cards('', THREADS.slice(0, 3)), shelves: `${shelf('Chats', CHATS.length, { open: true, plus: true })}${chatRows({ state: true, onId })}${archivedShelf()}` });
TOPICS.push({
  id: 'archive', section: 'In a chat', title: 'Archiving and deleting a chat', size: 'narrow', rec: 'A',
  now: 'A thread’s right-click menu has Pin, Rename, Archive, Project Settings and Delete…; Delete asks first: “The thread and its conversation will be removed. This can’t be undone.” Archived threads sit in the Archived shelf.',
  nowImg: 'img/now-thread-menu.png',
  options: [
    { key: 'A', name: 'Like threads; Delete takes the folder', from: 'agentZ’s threads',
      desc: 'Pin, Rename, Archive and Delete… (no Project Settings). Archived chats go to Archived with the chat icon. Delete removes the chat with its folder and attachments.',
      good: 'Same as threads; nothing left behind.', cost: 'Files the agent wrote there go too.',
      mock: () => sbFrame(sidebarWithChats(), { h: 600, over: chatMenu([mi('pin', 'Pin'), mi('pencil', 'Rename'), mi('archive', 'Archive'), mi('-'), mi('trash', 'Delete…', { hl: true })]) }) },
    { key: 'B', name: 'Like threads; Delete keeps the folder', from: 't3code',
      desc: 'As A, but Delete keeps the chat’s folder, so the agent’s files stay until you delete them in Storage settings.',
      good: 'Nothing the agent made is lost by accident.', cost: 'Folders pile up in the data folder.',
      mock: () => sbFrame(sidebarWithChats(), { h: 600, over: deleteDialog('The chat and its conversation will be removed. Its folder stays, with the files the agent wrote; Storage settings can delete it.') }) },
    { key: 'C', name: 'Ask about the folder', from: 'new',
      desc: 'As A, and when the folder has files in it, Delete asks with a box to keep it.',
      good: 'Your choice each time.', cost: 'A question on every delete.',
      mock: () => sbFrame(sidebarWithChats(), { h: 600, over: deleteDialog(`The chat and its conversation will be removed. This can’t be undone.</p><p class="row g2" style="color:var(--t)"><span class="check"></span>Keep its folder (3 files the agent wrote)`) }) },
  ],
});

// The setting ----------------------------------------------------------------------------------
const GENERAL_THREADS = settingGroup('Threads', [
  settingRow('Thread order', 'How threads are sorted in the sidebar.', '<span class="btn sm">Newest first ⌃</span>'),
  settingRow('Use modifier to send', 'Whether to always use ctrl-enter to send messages.', sw(false)),
]);
const CHATS_ROW = settingRow('Chats', 'Threads for general conversation, outside every project, in their own group in the sidebar.', sw(true));
TOPICS.push({
  id: 'setting', section: 'Turning chats off', title: 'Where the setting is', size: 'wide', rec: 'A',
  now: 'Settings › General has Threads (Thread order, Use modifier to send, Show thinking), Thread titles, Projects and Server groups of rows, each with a switch or a menu.',
  nowImg: 'img/now-settings-general.png',
  options: [
    { key: 'A', name: 'A Chats group in General', from: 'agentZ’s General page',
      desc: 'Its own group after Threads, with one row and a switch, on by default.',
      good: 'Easy to find; room for chat settings later.', cost: 'A group for one row.',
      mock: () => settingsPage(`<div style="font-size:16px;margin-bottom:4px">General</div>${GENERAL_THREADS}${settingGroup('Chats', [CHATS_ROW])}`, { w: 900, h: 420 }) },
    { key: 'B', name: 'A row in Threads', from: 'agentZ’s General page',
      desc: 'A Chats row at the end of the Threads group.',
      good: 'No new group.', cost: 'Easy to miss among the thread rows.',
      mock: () => settingsPage(`<div style="font-size:16px;margin-bottom:4px">General</div>${settingGroup('Threads', [settingRow('Thread order', 'How threads are sorted in the sidebar.', '<span class="btn sm">Newest first ⌃</span>'), settingRow('Use modifier to send', 'Whether to always use ctrl-enter to send messages.', sw(false)), CHATS_ROW])}`, { w: 900, h: 420 }) },
    { key: 'C', name: 'Also on the shelf', from: 'new',
      desc: 'As A, and right-clicking the Chats header offers Hide Chats, which turns the setting off.',
      good: 'Off from where you see them.', cost: 'Easy to hide them by accident.',
      mock: () => sbFrame(sidebarJB({ list: cards('', THREADS.slice(0, 3)), shelves: `${shelf('Chats', CHATS.length, { plus: true, tone: 'var(--t)' })}${archivedShelf()}` }), { h: 470, over: `<div class="menu" style="left:60px;top:400px;width:200px"><div class="it">${ic('plus', 'sm')}<span class="grow">New Chat</span></div><div class="hr"></div><div class="it hl">${ic('eye-off', 'sm')}<span class="grow">Hide Chats</span></div></div>` }) },
  ],
});

TOPICS.push({
  id: 'off', section: 'Turning chats off', title: 'What happens to chats while it’s off', size: 'medium', rec: 'A',
  now: 'Nothing yet: chats don’t exist. The backlog leaves open what happens to existing chats while the setting is off.',
  options: [
    { key: 'A', name: 'Hidden and kept', from: 'new',
      desc: 'The Chats group, New Chat and search leave them out; their data stays. Turning chats back on brings them back as they were. A chat that’s working finishes its turn.',
      good: 'Nothing lost; easy to undo.', cost: 'Space taken by chats you can’t see (Storage settings still list them).',
      mock: () => settingsPage(`${settingGroup('Chats', [settingRow('Chats', 'Threads for general conversation, outside every project. 5 chats are kept while this is off.', sw(false))])}`, { w: 760, h: 220 }) },
    { key: 'B', name: 'Hidden, but found by search', from: 'agentZ’s Archived (in search)',
      desc: 'As A, but search still finds them, marked Chat, and opening one shows it as usual.',
      good: 'Old answers stay reachable.', cost: 'Off isn’t quite off.',
      mock: () => sbFrame(sidebarJB({ head: searchHead('cache'), list: `<div class="col" style="gap:2px">${searchRow(chatIcon(), 'Postgres or SQLite for the cache', { label: 'Chat', ago: '4m', on: true })}${searchRow(projectMono('ap'), 'Cache the rate limits', { ago: '2d' })}</div>`, shelves: '' }), { h: 200 }) },
    { key: 'C', name: 'Asked when turned off', from: 'new',
      desc: 'Turning it off with chats asks what to do with them: keep them hidden, or delete them with their folders.',
      good: 'Clear about what happens.', cost: 'A dialog in settings.',
      mock: () => settingsPage(`${settingGroup('Chats', [settingRow('Chats', 'Threads for general conversation, outside every project.', sw(false))])}<div class="modal-back"></div><div class="dlg" style="left:250px;top:40px;width:340px"><h4>Turn off chats?</h4><p>You have 5 chats. Keep them hidden until chats are turned back on, or delete them with their folders.</p><div class="row g2" style="justify-content:flex-end"><span class="btn">Cancel</span><span class="btn">Delete 5 Chats</span><span class="btn primary">Keep Them</span></div></div>`, { w: 760, h: 240 }) },
  ],
});
