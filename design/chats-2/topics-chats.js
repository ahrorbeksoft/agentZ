// Chats as threads: a chat's card among the threads, Chats in the project picker, the sidebar
// with Chats picked, archiving chats, and the Chats setting.

const CH2_NOW_SIDEBAR = 'Thread cards fill the list, each in a project: the project’s icon and name with the state, the title, then the branch with the machine and agent icons. Chats are their own Chats shelf at the bottom, above Archived, with a + for New Chat: one line each (the chat icon, the title, a dot while it works or waits for you, the time), the five latest, then Show more.';

TOPICS.push({
  id: 'card', section: 'Chats', title: 'A chat’s card among the threads', size: 'narrow', rec: 'A',
  now: `${CH2_NOW_SIDEBAR} In every option the shelf goes and chats are cards among the threads, in the thread order (here Newest first).`,
  nowImg: 'img/now-sidebar.png',
  options: [
    { key: 'A', name: 'Chat in the project’s place', from: 'new (the Chats round’s chat card)',
      desc: 'The first line has the chat icon and “Chat” where a thread has its project, with the state. The last line has no branch, only the machine and agent icons, as a thread in a folder without git.',
      good: 'Reads like every other card; Chat is where you look for the project.', cost: 'The last line is mostly empty.',
      mock: () => sbFrame(sidebarJB({ list: ch2Mixed((chat, opts) => ch2ChatCard(chat, { ...opts, look: 'A' })) }), { h: 600 }) },
    { key: 'B', name: 'With the agent’s name for the branch', from: 'the Chats round (topic 2 D)',
      desc: 'Like A, with the agent’s name where a thread’s branch is (“Claude Agent”).',
      good: 'No empty line, and says who you’re talking to.', cost: 'The agent’s icon already says it.',
      mock: () => sbFrame(sidebarJB({ list: ch2Mixed((chat, opts) => ch2ChatCard(chat, { ...opts, look: 'B' })) }), { h: 600 }) },
    { key: 'C', name: 'Two lines', from: 'new',
      desc: 'Chat and the state, then the title with the machine and agent icons at its end. No third line.',
      good: 'Chats take less room than threads.', cost: 'Two heights of card in one list; a long title is cut sooner.',
      mock: () => sbFrame(sidebarJB({ list: ch2Mixed((chat, opts) => ch2ChatCard(chat, { ...opts, look: 'C' })) }), { h: 600 }) },
    { key: 'D', name: 'No project', from: 't3code (No project)',
      desc: 'Like A, with “No project” in place of the project’s name, as t3code names a thread without one.',
      good: 't3code’s word.', cost: 'Says what it isn’t; the picker and New Chat call them chats.',
      mock: () => sbFrame(sidebarJB({ list: ch2Mixed((chat, opts) => ch2ChatCard(chat, { ...opts, look: 'D' })) }), { h: 600 }) },
  ],
});

TOPICS.push({
  id: 'picker', section: 'Chats', title: 'Chats in the project picker', size: 'narrow', rec: 'A',
  now: 'The title bar’s button (“All projects ⌄”) opens the project picker: Search projects…, All projects with the count and a check on the picked one, then each machine’s projects (with other machines, under their names), each with its state’s dot and a gear for Project Settings, then Add Project…. The picked project’s threads are the only cards in the sidebar. Chats aren’t in it. In every option, picking Chats shows only chats, and the button reads “Chats” with the chat icon.',
  nowImg: 'img/now-project-switcher.png',
  options: [
    { key: 'A', name: 'Under All projects', from: 'new (Zed’s recent projects list)',
      desc: 'Chats comes second, under All projects and above every machine’s projects: the chat icon, “Chats”, how many (“5 chats”) and the most urgent chat’s dot. No gear. Search finds it as it finds a project.',
      good: 'The two ways to see more than a project sit together, above the projects.', cost: 'One more row before the projects.',
      mock: () => ch2SwitcherFrame({ chats: 'top' }, { h: 340 }) },
    { key: 'B', name: 'After the projects', from: 'new',
      desc: 'Like A, but last, after every machine’s projects, above Add Project….',
      good: 'Projects stay first.', cost: 'Far from All projects; with many projects, a scroll away.',
      mock: () => ch2SwitcherFrame({ chats: 'bottom' }, { h: 340 }) },
    { key: 'C', name: 'As a project named Chat', from: 'new',
      desc: 'Chat is first among This Mac’s projects, like one of them: the chat icon, “Chat”, its dot, and a gear that opens Settings › Threads › Chats. It holds every machine’s chats.',
      good: 'The same word as on the cards.', cost: 'It sits under This Mac, but has chats from every machine.',
      mock: () => ch2SwitcherFrame({ chats: 'project' }, { h: 340 }) },
  ],
});

const ch2ScopedSidebar = (list, shelves = shelf('Archived', 1)) => ch2Window(ch2TitleBar('Chats', chatIcon()), sidebarJB({ list, shelves }), { h: 560 });
TOPICS.push({
  id: 'scoped', section: 'Chats', title: 'The sidebar with Chats picked', size: 'narrow', rec: 'A',
  now: 'With one project picked, its cards drop the project’s line: the title with the state first, then the branch and icons. The + (New Thread, Cmd-N) starts a thread in that project, and Archived lists that project’s. Nothing shows only chats today. In every option, the + and Cmd-N start a chat while Chats is picked, and Archived lists only chats. The mocks use A of the first topic.',
  nowImg: 'img/now-sidebar.png',
  options: [
    { key: 'A', name: 'Like one project’s cards', from: 'agentZ (a picked project’s cards)',
      desc: 'Chat’s line goes, as a project’s does: the title with the state, then the machine and agent icons.',
      good: 'Works as picking a project does.', cost: 'Nothing on a card says it’s a chat; the title bar does.',
      mock: () => ch2ScopedSidebar(CHATS.map((chat) => ch2ScopedCard(chat, { on: chat.id === 'async' })).join('')) },
    { key: 'B', name: 'Chat stays on every card', from: 'new',
      desc: 'The cards keep their first line with Chat, as under All projects.',
      good: 'Cards look the same everywhere.', cost: 'A line that says the same on every card.',
      mock: () => ch2ScopedSidebar(CHATS.map((chat) => ch2ChatCard(chat, { on: chat.id === 'async' })).join('')) },
    { key: 'C', name: 'One line each', from: 'agentZ’s Chats shelf',
      desc: 'Chats as today’s shelf shows them: the chat icon, the title, a dot while it works or waits for you, and the time.',
      good: 'Many chats fit; what you know from the shelf.', cost: 'Unlike a picked project’s cards; no agent or machine.',
      mock: () => ch2ScopedSidebar(`<div class="col" style="gap:2px">${chatRows({ state: true, onId: 'async' })}</div>`) },
  ],
});

const ch2Archived = () => `${shelf('Archived', 7, { open: true })}<div class="lrow quiet">${chatIcon()}<span class="grow trunc">A regex for the access log</span><span class="when">3d</span></div><div class="lrow quiet">${projectMono('st')}<span class="grow trunc">Rename the cart store</span><span class="when">4d</span></div>`;
const ch2HoverSidebar = ({ end = ch2Hover(), over = '', shelves = ch2Archived() } = {}) => sbFrame(sidebarJB({ list: ch2Mixed((chat, opts) => ch2ChatCard(chat, { ...opts, end: chat.id === 'async' ? end : undefined }), { on: 'checkout', hov: 'async' }), shelves }), { h: 620, over });
TOPICS.push({
  id: 'archive', section: 'Chats', title: 'Archiving a chat', size: 'narrow', rec: 'A',
  now: 'Chats can’t be archived, as you asked in the Chats round (“and not archivable”): their menu is Pin, Rename and Delete…, and auto-archive skips them. A thread’s card shows Snooze and Archive on hover, its menu has Archive, and Settings › Threads › Auto-archive threads (Never by default) archives the ones nobody used for that long. Archived threads are rows on the Archived shelf. The mocks hover “Async runtimes compared”.',
  nowImg: 'img/now-sidebar.png',
  options: [
    { key: 'A', name: 'Like threads', from: 'agentZ’s threads',
      desc: 'Archive on hover and in the menu; auto-archive takes chats too. Archived chats are rows on the Archived shelf with the chat icon, and unarchive as threads do.',
      good: 'A chat is just a thread, as the backlog says.', cost: 'Old chats leave the list by themselves when auto-archive is on.',
      mock: () => ch2HoverSidebar() },
    { key: 'B', name: 'By hand only', from: 'new',
      desc: 'Like A, but auto-archive leaves chats alone: they’re archived only from their card or menu.',
      good: 'A chat you keep coming back to stays.', cost: 'One difference from threads to remember.',
      mock: () => ch2HoverSidebar({ over: note('Auto-archive skips chats', 'left:60px;top:300px') }) },
    { key: 'C', name: 'Not archived, as today', from: 'today',
      desc: 'Chats keep no Archive: Snooze alone on hover, and Pin, Rename, Snooze and Delete… in the menu.',
      good: 'As you picked in the Chats round.', cost: 'Old chats stay among the cards until deleted.',
      mock: () => ch2HoverSidebar({ end: `<span class="row" style="gap:2px">${ibtn('clock', 'sm')}</span>`, shelves: archivedShelf(), over: ch2CardMenu(['Pin', 'Rename', 'Snooze', '-', 'Delete…'], 'left:120px;top:395px') }) },
  ],
});

const ch2SettingsPanel = (html) => jb(frame(`<div style="position:absolute;inset:0;background:var(--ed);padding:0 22px">${html}</div>`, { w: 600, h: 300 }));
const ch2ThreadRows = () => settingGroup('Threads', [
  settingRow('Auto-archive threads', 'Archives a thread nobody has used for this long, going by its last activity.', `<span class="btn sm">Never ${chev}</span>`),
  settingRow('Usage shown in the composer', 'The window of an account’s limits the composer’s gauge and its account picker show.', `<span class="btn sm">Current session ${chev}</span>`),
]);
TOPICS.push({
  id: 'setting', section: 'Chats', title: 'The Chats setting', size: 'medium', rec: 'A',
  now: 'Settings › Threads ends with a Chats group: one switch, on by default, “Threads for general conversation, outside every project, in their own group in the sidebar.” While it’s off, the Chats shelf, New Chat, Chat in the pickers and chats in search and Go To are gone; chats are kept.',
  nowImg: 'img/now-settings-threads.png',
  options: [
    { key: 'A', name: 'Kept, with new words', from: 'agentZ’s Chats setting',
      desc: 'The switch stays and does what it does, with the picker’s Chats entry hidden too. Its words lose the shelf: “Threads for general conversation, outside every project. They show among the threads, with Chat in place of a project.”',
      good: 'Someone who never chats keeps chats out of every menu.', cost: 'A setting for something that’s now only a kind of thread.',
      mock: () => ch2SettingsPanel(`${ch2ThreadRows()}${settingGroup('Chats', [settingRow('Chats', 'Threads for general conversation, outside every project. They show among the threads, with Chat in place of a project.', sw(true))])}`) },
    { key: 'B', name: 'Gone: chats are always on', from: 'new',
      desc: 'The Chats group goes, and chats are always offered, as threads are.',
      good: 'One setting fewer.', cost: 'New Chat, and Chat in New thread in…, show for everyone.',
      mock: () => ch2SettingsPanel(ch2ThreadRows()) },
  ],
});
