// Storage: a settings page for what agentZ keeps on disk, with sizes, and deleting it. Where the
// page is, what it lists, deleting, and keeping it small.

const NOW_NONE = 'Nothing yet: no page shows what agentZ keeps on disk or how big it is. Settings has General, Appearance, Notifications, Agents, Usage, Skills, MCP Servers and Machines, then the projects.';

// The page ---------------------------------------------------------------------------------------
TOPICS.push({
  id: 'place', section: 'The page', title: 'Where the page is', size: 'wide', rec: 'A',
  now: NOW_NONE,
  nowImg: 'img/now-settings.png',
  options: [
    { key: 'A', name: 'Storage, after Machines', from: 't3code’s Settings › Storage',
      desc: 'A Storage section of its own, the last before the projects, named as t3code names it.',
      good: 'Easy to find; room for every kind of data.', cost: 'One more section in a long list.',
      mock: () => storagePage() },
    { key: 'B', name: 'In each machine’s settings', from: 'agentZ’s Machines page',
      desc: 'Each machine’s row on the Machines page shows what agentZ uses there, and its Storage… button opens the page for that machine.',
      good: 'Storage is per machine, and this says so.', cost: 'Hidden a level down; This Mac’s storage under Machines is unexpected.',
      mock: () => settingsPage(`<div class="pghead"><span class="tt grow">Machines</span></div>${settingGroup('Machines', [
        settingRow(`<span class="row g2">${ic('laptop', 'sm mu')}This Mac</span>`, 'agentZ uses 6.0 GB here', '<span class="btn sm">Storage…</span>'),
        settingRow(`<span class="row g2">${ic('server', 'sm mu')}Devbox 1</span>`, 'root@devbox1 · agentZ uses 2.7 GB here', '<span class="btn sm">Storage…</span>'),
      ])}`, { section: 'Machines', nav: NAV, h: 360 }) },
    { key: 'C', name: 'A group in General', from: 'agentZ’s General page',
      desc: 'A Storage group at the end of General, with the total and a Manage… button that opens the list over the page.',
      good: 'No new section.', cost: 'General gets longer, and the list needs a window of its own.',
      mock: () => settingsPage(`<div class="pghead"><span class="tt grow">General</span></div>${settingGroup('Server', [settingRow('Start at login', 'Starts the server when you log in, before agentZ opens, so scripts using agentz-server call can reach it.', sw(false))])}${settingGroup('Storage', [settingRow('Data on This Mac', 'agentZ uses 6.0 GB in ~/.agentz', '<span class="btn sm">Manage…</span>')])}`, { section: 'General', nav: NAV, h: 360 }) },
  ],
});

TOPICS.push({
  id: 'machine', section: 'The page', title: 'Which machine it shows', size: 'wide', rec: 'A',
  now: 'Each machine’s server keeps its own data folder. With more than one machine, the Agents page has a machine picker in its header (the machine’s icon and name in a menu), since each machine installs and runs its own agents.',
  options: [
    { key: 'A', name: 'The machine picker in the header', from: 'agentZ’s Agents page',
      desc: 'This Mac first; the picker in the header switches to another machine. Shown only with more than one machine.',
      good: 'Same as the Agents page; one machine’s list at a time.', cost: 'You don’t see at a glance which machine is fullest.',
      mock: () => storagePage({ head: pageHead(machinePicker('This Mac', true)), over: `<div class="menu" style="right:200px;top:52px;width:180px"><div class="it">${ic('laptop', 'sm')}<span class="grow">This Mac</span>${ic('check', 'xs')}</div><div class="it hl">${ic('server', 'sm')}<span class="grow">Devbox 1</span></div></div>` }) },
    { key: 'B', name: 'Every machine on one page', from: 'new',
      desc: 'A group for each machine with its total and a row for each kind of data; a row opens that kind’s list.',
      good: 'Compares machines at once.', cost: 'Two levels: the lists are a click further.',
      mock: () => settingsPage(`${pageHead('')}${settingGroup(`<span class="row g1">${ic('laptop', 'xs')}This Mac</span>`, KINDS.map((kind) => srow({ lead: `<b style="width:8px;height:8px;border-radius:2px;background:${kind.color};display:inline-block"></b>`, name: kind.name, size: kind.size, when: null, end: `<span class="slot">${ic('chev-right', 'xs')}</span>` })), { total: TOTAL })}${settingGroup(`<span class="row g1">${ic('server', 'xs')}Devbox 1</span>`, DEVBOX_KINDS.slice(0, 3).map((kind) => srow({ name: kind.name, size: kind.size, when: null, end: `<span class="slot">${ic('chev-right', 'xs')}</span>` })), { total: '2.7 GB' })}`, { h: 520 }) },
    { key: 'C', name: 'Tabs for machines', from: 'new',
      desc: 'A row of machine tabs under the title, each with its total.',
      good: 'Every machine’s total in view; one click to switch.', cost: 'Doesn’t fit many machines; unlike the Agents page.',
      mock: () => storagePage({ head: `${pageHead('')}<div class="seg" style="margin-top:6px"><span class="on">${ic('laptop', 'xs')}This Mac · 6.0 GB</span><span>${ic('server', 'xs')}Devbox 1 · 2.7 GB</span></div>` }) },
  ],
});

TOPICS.push({
  id: 'summary', section: 'The page', title: 'The top of the page', size: 'wide', rec: 'A',
  now: NOW_NONE,
  options: [
    { key: 'A', name: 'The total and a bar', from: 'macOS’s Storage settings',
      desc: 'How much agentZ uses on the machine and where its data folder is, over a bar with a color for each kind, and the kinds with their sizes under it.',
      good: 'What takes the space is clear before you read a row.', cost: 'A bar agentZ draws nowhere else.',
      mock: () => storagePage() },
    { key: 'B', name: 'One line with the total', from: 'new',
      desc: 'A line under the title: how much agentZ uses on the machine, and its data folder.',
      good: 'Quiet; the groups below say the rest.', cost: 'You add up the groups to see what’s biggest.',
      mock: () => storagePage({ summary: summaryLine() }) },
    { key: 'C', name: 'Totals on the groups only', from: 't3code’s settings groups',
      desc: 'No summary; each group’s label has its total beside it.',
      good: 'Nothing extra.', cost: 'No total for the machine.',
      mock: () => storagePage({ summary: '' }) },
  ],
});

// What's listed -----------------------------------------------------------------------------------
TOPICS.push({
  id: 'kinds', section: 'What’s listed', title: 'What the page lists', size: 'medium', type: 'multi', rec: 'ABCDE',
  now: 'The data folder holds, for each thread, its conversation (transcripts), its images and uploaded files (attachments) and its handoffs; threads’ worktrees and pastures; installed agents and the registry’s cache; downloaded Node.js; and the server’s log. Agents’ logins in accounts are never changed by agentZ. Chats would add a folder for each.',
  options: [
    { key: 'A', name: 'Chats', from: 'the backlog',
      desc: 'Each chat with its size: its conversation, images and folder together.',
      good: 'What the backlog asks for.', cost: 'None.',
      mock: () => panel(chatGroup(CHATS.map((chat) => chatRow(chat))), { h: 250 }) },
    { key: 'B', name: 'Project threads', from: 'agentZ’s data folder',
      desc: 'A row for each project with its threads’ conversations and images; it opens to the threads, biggest first.',
      good: 'Long threads with many images show up.', cost: 'Threads are small next to worktrees.',
      mock: () => panel(settingGroup('Threads', [projectRow(PROJECT_THREADS[0], { open: true }), ...PROJECT_THREADS[0].threads.map((thread) => threadRow(thread)), projectRow(PROJECT_THREADS[1])], { total: '104 MB' }), { h: 250 }) },
    { key: 'C', name: 'Worktrees and pastures', from: 'agentZ’s data folder',
      desc: 'Each worktree and pasture agentZ made, with its project, kind and what its thread is doing.',
      good: 'Usually most of the space.', cost: 'Big, so measuring them takes a moment.',
      mock: () => panel(settingGroup('Worktrees and pastures', CHECKOUTS.map((item) => checkoutRow(item)), { total: '4.6 GB' }), { h: 300 }) },
    { key: 'D', name: 'Agents', from: 'agentZ’s data folder',
      desc: 'Installed agents with their sizes (uninstalled on the Agents page, as today) and the registry’s cache, which Clear empties.',
      good: 'Shows what an agent install costs.', cost: 'Uninstalling isn’t here, only the size.',
      mock: () => panel(settingGroup('Agents', agentRows(), { total: '645 MB' }), { h: 250 }) },
    { key: 'E', name: 'Node.js and logs', from: 'agentZ’s data folder',
      desc: 'Downloaded Node.js, deleted until an agent needs it again, and the server’s log, which Clear empties.',
      good: 'Everything agentZ makes is accounted for.', cost: 'Small; rarely worth deleting.',
      mock: () => panel(settingGroup('Node.js and logs', otherRows(), { total: '192 MB' }), { h: 180 }) },
  ],
});

TOPICS.push({
  id: 'chat-row', section: 'What’s listed', title: 'A chat’s row', size: 'medium', rec: 'A',
  now: 'Nothing yet. The sidebar’s Archived rows are one line: the icon, the title and when it last did something.',
  options: [
    { key: 'A', name: 'One line', from: 'agentZ’s Archived rows',
      desc: 'The chat icon (archived ones have the archive icon), the title, the size and when it was last used.',
      good: 'Many fit; sizes line up to compare.', cost: 'Doesn’t say what in it is big.',
      mock: () => panel(chatGroup(CHATS.map((chat) => chatRow(chat))), { h: 250 }) },
    { key: 'B', name: 'Two lines, with what’s inside', from: 'new',
      desc: 'As A, with a second line: the conversation’s, images’ and folder’s sizes.',
      good: 'Says whether the agent’s files or the images take the space.', cost: 'Twice the height.',
      mock: () => panel(chatGroup(CHATS.map((chat) => chatRow(chat, { sub: chat.parts.join(' · ') }))), { h: 330 }) },
    { key: 'C', name: 'A table', from: 'new',
      desc: 'Columns for the conversation, images, folder and the total.',
      good: 'Compares each part across chats.', cost: 'Narrow columns of small numbers; more than you need to free space.',
      mock: () => panel(chatGroup([`<div class="thd"><span class="grow">Chat</span><span style="width:76px;text-align:right">Conversation</span><span style="width:52px;text-align:right">Images</span><span style="width:56px;text-align:right">Folder</span><span style="width:56px;text-align:right">Total</span></div>`, ...CHATS.map((chat) => `<div class="strow">${chatLead(chat)}<span class="grow trunc">${chat.title}</span>${chat.parts.map((part, index) => `<span class="sz" style="width:${[76, 52, 56][index]}px">${part.split(' ').slice(1).join(' ')}</span>`).join('')}<span class="sz" style="width:56px;color:var(--t)">${chat.size}</span></div>`)]), { h: 270 }) },
  ],
});

TOPICS.push({
  id: 'threads', section: 'What’s listed', title: 'Project threads', size: 'medium', rec: 'A',
  now: 'Deleting a thread (its right-click menu, Delete…) removes its conversation, images and handoffs; its worktree or pasture stays. Archived threads are in the sidebar’s Archived shelf.',
  nowImg: 'img/now-delete-thread.png',
  issues: ['On Linux the Delete dialog cuts off its own text.'],
  options: [
    { key: 'A', name: 'By project, opening to threads', from: 'new',
      desc: 'A row for each project with its threads’ total; opening one lists its biggest threads first, then Show more.',
      good: 'Finds the one huge thread.', cost: 'Another tree to open.',
      mock: () => panel(settingGroup('Threads', [projectRow(PROJECT_THREADS[0], { open: true }), ...PROJECT_THREADS[0].threads.map((thread) => threadRow(thread)), srow({ cls: 'sub', name: '<span class="link">Show 28 more</span>', when: null }), projectRow(PROJECT_THREADS[1])], { total: '104 MB' }), { h: 290 }) },
    { key: 'B', name: 'Archived threads only', from: 'agentZ’s Archived shelf',
      desc: 'Only archived threads, the ones you’re done with, by project. Active threads aren’t listed.',
      good: 'Lists only what’s likely safe to delete.', cost: 'A big active thread is never shown.',
      mock: () => panel(settingGroup('Archived threads', [projectRow(PROJECT_THREADS[0], { open: true, sub: '9 archived threads', size: '24 MB' }), threadRow(PROJECT_THREADS[0].threads[1]), threadRow({ title: 'Try a CSS grid for the cart', size: '3.1 MB', ago: '3w', archived: true }), projectRow(PROJECT_THREADS[1], { sub: '3 archived threads', size: '6.1 MB' })], { total: '30 MB' }), { h: 250 }) },
    { key: 'C', name: 'Projects only', from: 'new',
      desc: 'A row for each project with its threads’ total and a Delete Archived… button for its archived threads. Single threads are deleted from the sidebar, as today.',
      good: 'Short; one button frees a project’s old threads.', cost: 'No way to see which thread is big.',
      mock: () => panel(settingGroup('Threads', PROJECT_THREADS.map((item) => srow({ lead: mono(item.mono, item.color), name: item.project, sub: `${item.count} threads, ${item.archived} archived (${item.archivedSize})`, size: item.size, when: null, end: '<span class="btn sm">Delete Archived…</span>' })), { total: '104 MB' }), { h: 200 }) },
  ],
});

TOPICS.push({
  id: 'order', section: 'What’s listed', title: 'The order of rows', size: 'medium', rec: 'A',
  now: 'The sidebar sorts threads by Thread order (Settings › General), newest first by default.',
  options: [
    { key: 'A', name: 'Biggest first', from: 'macOS’s Storage settings',
      desc: 'Each group’s rows by size, biggest first.',
      good: 'What to delete is at the top.', cost: 'A chat you just used may sit anywhere.',
      mock: () => panel(chatGroup(CHATS.map((chat) => chatRow(chat))), { h: 250 }) },
    { key: 'B', name: 'Newest first', from: 'agentZ’s sidebar',
      desc: 'Last used first, as the sidebar sorts by default.',
      good: 'Old ones gather at the bottom.', cost: 'The big ones have to be spotted.',
      mock: () => panel(chatGroup(NEWEST_CHATS.map((chat) => chatRow(chat))), { h: 250 }) },
    { key: 'C', name: 'A menu on each group', from: 'new',
      desc: 'Biggest first, with a menu on the group’s label to sort by last used instead.',
      good: 'Both orders.', cost: 'A control on every group.',
      mock: () => panel(chatGroup(CHATS.map((chat) => chatRow(chat)), { right: `<span class="dd hov" style="font-size:12px;height:20px">Biggest first${chev}</span>` }), { h: 250, over: `<div class="menu" style="right:20px;top:40px;width:160px"><div class="it">${ic('check', 'xs')}<span class="grow">Biggest first</span></div><div class="it hl"><span style="width:12px"></span><span class="grow">Last used</span></div></div>` }) },
  ],
});

TOPICS.push({
  id: 'checkouts', section: 'What’s listed', title: 'Worktrees and pastures', size: 'medium', rec: 'A',
  now: 'In the Workspaces view, a worktree’s right-click menu has Delete Worktree Checkout…, which deletes its folder (“Its folder … is deleted from disk, and its workspace closes.”) and asks again if it has changes. Deleting a thread keeps its worktree or pasture.',
  nowImg: 'img/now-worktree-menu.png',
  options: [
    { key: 'A', name: 'Each with its thread', from: 'agentZ’s Workspaces view',
      desc: 'Each one with its branch, project and kind, and what its thread is doing: working, finished, archived, or deleted.',
      good: 'Leftovers of deleted and archived threads are easy to spot.', cost: 'A second line on every row.',
      mock: () => panel(settingGroup('Worktrees and pastures', CHECKOUTS.map((item) => checkoutRow(item)), { total: '4.6 GB' }), { h: 300 }) },
    { key: 'B', name: 'Grouped by their thread', from: 'new',
      desc: 'Under three labels: no thread (deleted), thread archived, and in use.',
      good: 'The leftovers come first.', cost: 'Three small lists.',
      mock: () => panel(`${settingGroup('No thread', [checkoutRow(CHECKOUTS[3], { showState: false })], { total: '0.9 GB' })}${settingGroup('Thread archived', [checkoutRow(CHECKOUTS[2], { showState: false })], { total: '1.1 GB' })}${settingGroup('In use', [CHECKOUTS[0], CHECKOUTS[1], CHECKOUTS[4]].map((item) => checkoutRow(item, { showState: false })), { total: '2.6 GB' })}`, { h: 440 }) },
    { key: 'C', name: 'Only the leftovers', from: 't3code’s worktree cleanup',
      desc: 'Only the ones whose thread was deleted or archived; a line under them says how many more are in use and their size.',
      good: 'Lists only what you can likely delete.', cost: 'The big ones in use aren’t listed.',
      mock: () => panel(settingGroup('Worktrees and pastures', [checkoutRow(CHECKOUTS[3]), checkoutRow(CHECKOUTS[2]), srow({ dim: true, name: '3 more in use by threads', size: '2.6 GB', when: null })], { total: '4.6 GB' }), { h: 220 }) },
  ],
});

// Deleting ----------------------------------------------------------------------------------------
TOPICS.push({
  id: 'select', section: 'Deleting', title: 'How you delete', size: 'medium', rec: 'B',
  now: 'One at a time: a thread from its right-click menu in the sidebar, a worktree from its menu in the Workspaces view, an agent from the Agents page.',
  options: [
    { key: 'A', name: 'A trash button on each row', from: 'agentZ’s Skills and MCP Servers rows',
      desc: 'Hovering a row shows a trash button; it asks first.',
      good: 'Simple; one row at a time is hard to get wrong.', cost: 'Freeing space from ten chats takes ten dialogs.',
      mock: () => panel(chatGroup(CHATS.map((chat, index) => chatRow(chat, { hov: index === 1, end: index === 1 ? trash() : '<span class="slot"></span>' }))), { h: 250 }) },
    { key: 'B', name: 'Checkboxes and one Delete', from: 'new',
      desc: 'A checkbox on each row; the group’s foot says how many are picked and their size, with Delete.',
      good: 'Many at once, with the space they free shown before you delete.', cost: 'Checkboxes on every row.',
      mock: () => panel(`${chatGroup([...CHATS.map((chat, index) => chatRow(chat, { lead: `${chat.working ? '<span class="check off"></span>' : check(index === 1 || index === 2)}${chatLead(chat)}` })), `<div class="selbar"><span class="grow">2 picked, 14 MB</span><span class="btn sm">Delete…</span></div>`])}`, { h: 290 }) },
    { key: 'C', name: 'Both', from: 'new',
      desc: 'A trash button on hover, and Select on the group’s label to pick several.',
      good: 'Quick for one, and many when you need it.', cost: 'Two ways to do one thing.',
      mock: () => panel(chatGroup(CHATS.map((chat, index) => chatRow(chat, { hov: index === 1, end: index === 1 ? trash() : '<span class="slot"></span>' })), { right: '<span class="btn sm" style="height:20px">Select</span>' }), { h: 250 }) },
  ],
});

const PICKED_PAGE = (over) => panel(chatGroup([...CHATS.map((chat, index) => chatRow(chat, { lead: `${chat.working ? '<span class="check off"></span>' : check(index === 1 || index === 2)}${chatLead(chat)}` })), `<div class="selbar"><span class="grow">2 picked, 14 MB</span><span class="btn sm">Delete…</span></div>`]), { h: 290, over });
TOPICS.push({
  id: 'confirm', section: 'Deleting', title: 'What Delete does', size: 'medium', rec: 'A',
  now: 'Deleting a thread asks first: “Delete ‘hello’? The thread and its conversation will be removed. This can’t be undone.” Delete Worktree Checkout… asks the same way, and again with Delete Anyway when it has changes.',
  nowImg: 'img/now-delete-thread.png',
  options: [
    { key: 'A', name: 'Asks, then it’s gone', from: 'agentZ’s Delete dialogs',
      desc: 'A dialog with how many, their size and what goes with them; Delete removes them for good.',
      good: 'Same as deleting anything else in agentZ.', cost: 'No way back from a wrong pick.',
      mock: () => PICKED_PAGE(dialog('Delete 2 chats?', 'They take 14 MB: their conversations, images and folders. This can’t be undone.', '<span class="btn">Cancel</span><span class="btn danger">Delete</span>', { left: 140, top: 60 })) },
    { key: 'B', name: 'To the Trash', from: 'macOS’s Finder',
      desc: 'The same dialog, but folders and images go to the machine’s Trash; conversations are removed.',
      good: 'A wrong pick can be brought back.', cost: 'The space isn’t freed until the Trash is emptied, and nobody empties an SSH machine’s Trash.',
      mock: () => PICKED_PAGE(dialog('Move 2 chats to the Trash?', 'Their folders and images go to the Trash on This Mac, and their conversations are removed. The 14 MB is freed when you empty the Trash.', '<span class="btn">Cancel</span><span class="btn primary" style="background:var(--ac);border-color:var(--ac)">Move to Trash</span>', { left: 140, top: 50 })) },
    { key: 'C', name: 'No dialog, with Undo', from: 'new',
      desc: 'Deletes at once, and a note at the foot offers Undo for a few seconds; the server waits that long before removing anything.',
      good: 'Fast, and a slip is undone.', cost: 'A note that goes away; data waits on the server.',
      mock: () => panel(chatGroup(CHATS.filter((chat, index) => index !== 1 && index !== 2).map((chat) => chatRow(chat)), { total: '404 MB' }), { h: 290, over: `<div class="toast" style="left:150px;bottom:20px">Deleted 2 chats, 14 MB<span class="link">Undo</span></div>` }) },
  ],
});

TOPICS.push({
  id: 'safe', section: 'Deleting', title: 'What can’t be deleted', size: 'medium', rec: 'A',
  now: 'The backlog keeps these safe: a working thread’s or chat’s data, worktrees with uncommitted changes, and agents’ logins in account homes, which agentZ never changes. Workspaces’ Delete Worktree Checkout… asks again with Delete Anyway when a worktree has changes.',
  nowImg: 'img/now-worktree-menu.png',
  options: [
    { key: 'A', name: 'Shown, with the reason', from: 'new',
      desc: 'Listed with their size, without a checkbox, and a tag saying why: Working, or Has changes. Account homes aren’t listed.',
      good: 'Every byte is accounted for, and you see why it stays.', cost: 'Rows you can’t act on.',
      mock: () => panel(settingGroup('Worktrees and pastures', CHECKOUTS.map((item) => checkoutRow(item, { keep: true, lead: `${item.state === 'working' || item.state === 'changes' ? '<span class="check off"></span>' : check(item.state === 'none')}<span class="mu" style="display:inline-flex">${ic(item.kind === 'pasture' ? 'pasture' : 'worktree', 'sm')}</span>` })), { total: '4.6 GB' }), { h: 300 }) },
    { key: 'B', name: 'Not listed', from: 'new',
      desc: 'Only what can be deleted is listed; a line says how much more is in use.',
      good: 'Every row can be acted on.', cost: 'What stays isn’t named, only counted.',
      mock: () => panel(settingGroup('Worktrees and pastures', [...[CHECKOUTS[2], CHECKOUTS[3], CHECKOUTS[4]].map((item) => checkoutRow(item, { lead: `${check(item.state === 'none')}<span class="mu" style="display:inline-flex">${ic(item.kind === 'pasture' ? 'pasture' : 'worktree', 'sm')}</span>` })), srow({ dim: true, name: '2 more are in use: one is working, one has changes', size: '2.3 GB', when: null })], { total: '4.6 GB' }), { h: 260 }) },
    { key: 'C', name: 'Changes ask again', from: 'agentZ’s Delete Worktree Checkout…',
      desc: 'As A, but a worktree with changes can be picked, and Delete asks again with Delete Anyway, as the Workspaces view does. A working thread’s data still can’t be.',
      good: 'Matches the Workspaces view.', cost: 'Uncommitted work can be lost in two clicks.',
      mock: () => panel(settingGroup('Worktrees and pastures', CHECKOUTS.map((item) => checkoutRow(item, { keep: true, lead: `${item.state === 'working' ? '<span class="check off"></span>' : check(item.state === 'changes')}<span class="mu" style="display:inline-flex">${ic(item.kind === 'pasture' ? 'pasture' : 'worktree', 'sm')}</span>` })), { total: '4.6 GB' }), { h: 300, over: dialog('Delete the release-notes worktree anyway?', 'It has uncommitted changes in 3 files, which will be lost.', '<span class="btn">Cancel</span><span class="btn danger">Delete Anyway</span>', { left: 140, top: 70 }) }) },
  ],
});

// Keeping it small ----------------------------------------------------------------------------
const RETENTION = (days) => `<span class="row g2"><span class="btn sm">− ${days} days +</span>${sw(true)}</span>`;
TOPICS.push({
  id: 'rules', section: 'Keeping it small', title: 'Cleaning up on its own', size: 'wide', rec: 'A',
  now: 'agentZ deletes nothing on its own, except the server’s log, which starts over once it passes its limit. t3code’s Storage page has only rules: where new worktrees go, deleting worktrees with deleted threads, after some days without use, once merged, or with no commits of their own (never with changes or a running session), and how many days to keep browser captures and old logs.',
  options: [
    { key: 'A', name: 'By hand only', from: 'the backlog',
      desc: 'No rules: the page shows sizes and you delete.',
      good: 'Nothing disappears without you.', cost: 'Space grows until you look.',
      mock: () => storagePage() },
    { key: 'B', name: 't3code’s worktree rules', from: 't3code’s Settings › Storage',
      desc: 'An Automatic cleanup group at the top, for each machine: delete worktrees and pastures with deleted threads, after days without use, once merged, or with no commits of their own. Never one with changes or a working thread.',
      good: 'Leftovers go on their own.', cost: 'A rule can delete a worktree you meant to come back to.',
      mock: () => settingsPage(`${pageHead()}${settingGroup('Automatic cleanup', [
        settingRow('Delete worktrees with deleted threads', 'Worktrees and pastures go when their thread is deleted. Ones with changes are kept.', sw(true)),
        settingRow('Delete inactive worktrees', 'After their thread has done nothing for this many days. Branches and the thread are kept.', RETENTION(8)),
        settingRow('Delete merged worktrees', 'Once their branch is merged into the default branch.', sw(false)),
        settingRow('Delete unchanged worktrees', 'With no commits beyond the default branch.', sw(false)),
      ])}`, { h: 430 }) },
    { key: 'C', name: 'Rules for chats too', from: 't3code, with chats',
      desc: 'As B, and a rule for archived chats: delete them after this many days.',
      good: 'Old chats go too.', cost: 'A chat you archived to keep goes after a while.',
      mock: () => settingsPage(`${pageHead()}${settingGroup('Automatic cleanup', [
        settingRow('Delete worktrees with deleted threads', 'Worktrees and pastures go when their thread is deleted. Ones with changes are kept.', sw(true)),
        settingRow('Delete inactive worktrees', 'After their thread has done nothing for this many days. Branches and the thread are kept.', RETENTION(8)),
        settingRow('Delete archived chats', 'With their folders, this many days after they were archived.', RETENTION(30)),
      ])}`, { h: 380 }) },
  ],
});

TOPICS.push({
  id: 'sizes', section: 'Keeping it small', title: 'When sizes are measured', size: 'wide', rec: 'A',
  now: 'Nothing measures agentZ’s data today. A worktree of a big project can hold hundreds of thousands of files, so measuring it takes a few seconds.',
  options: [
    { key: 'A', name: 'When the page opens', from: 'macOS’s Storage settings',
      desc: 'The machine’s server measures when you open the page; each size shows Measuring… until it’s known.',
      good: 'Always current; no work while you’re elsewhere.', cost: 'Big worktrees take a moment each time.',
      mock: () => storagePage({ summary: `<div style="margin:14px 0 4px;font-size:13px;color:var(--mu)">Measuring what agentZ uses on This Mac…</div><div class="sbar" style="margin-bottom:8px"><i style="flex:1;background:var(--sel)"></i></div>`, groups: `${chatGroup(CHATS.slice(0, 4).map((chat) => chatRow(chat)))}${settingGroup('Worktrees and pastures', CHECKOUTS.slice(0, 3).map((item, index) => srow({ lead: `<span class="mu" style="display:inline-flex">${ic(item.kind === 'pasture' ? 'pasture' : 'worktree', 'sm')}</span>`, name: item.branch, sub: `${item.project} · ${CHECKOUT_STATE[item.state]}`, size: item.size, measuring: index > 0, when: null })), { total: 'Measuring…' })}` }) },
    { key: 'B', name: 'Kept from the last time, with Refresh', from: 'new',
      desc: 'The page shows the sizes from the last time they were measured, with when, and Refresh measures again.',
      good: 'Opens at once.', cost: 'Sizes can be old; one more button.',
      mock: () => storagePage({ head: pageHead(`<span class="xs" style="color:var(--ph)">Measured 2 days ago</span><span class="btn sm">${ic('restart', 'xs')}Refresh</span>${machinePicker()}`) }) },
    { key: 'C', name: 'Kept current by the server', from: 'new',
      desc: 'Each server measures in the background as threads change, so the page opens with current sizes.',
      good: 'Current and at once.', cost: 'Disk work on every machine all the time, for a page you rarely open.',
      mock: () => storagePage() },
  ],
});
