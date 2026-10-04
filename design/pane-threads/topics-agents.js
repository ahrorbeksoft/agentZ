// Threads started in a Workspaces pane: how they show in the Agents view and in their pane.

const PT_H = 700;

// The Agents sidebar as it is today (sidebar.rs): t3code's thread cards, then the shelf.
const projectIcon = (project) => (project === 'st' ? mono('ST', 'g') : project === 'ap' ? mono('AI', 't') : ic('folder', 'sm mu'));
const threadCard = ({ project, name, time, title, branch = 'main', on = false }) => `<div style="margin:0 4px;padding:8px 10px;border-radius:8px;${on ? 'background:var(--sel)' : ''}">
  <div class="row g2" style="height:20px">${projectIcon(project)}<span class="grow trunc sm mu">${name}</span><span class="sm mu">${time}</span></div>
  <div class="trunc" style="margin-top:4px">${title}</div>
  <div class="row g2 sm faint" style="margin-top:4px"><span class="grow">${branch}</span>${ic('laptop', 'xs')}${ic('terminal', 'xs')}</div></div>`;
const shelfHeader = (label, { open = true, count } = {}) => `<div class="row g2" style="height:30px;margin:6px 4px 0;padding:0 10px;font-size:12px;color:var(--mu)"><span>${label}</span>${!open && count ? `<span class="faint">${count}</span>` : ''}<span class="grow" style="height:1px;background:var(--bv)"></span>${ic(open ? 'chev-up' : 'chev-down', 'xs')}</div>`;
const slimRow = ({ icon, title, time, line2 = '', on = false, hover = false, end, dim = true, titleClass }) => `<div style="margin:0 4px;padding:5px 10px;border-radius:6px;position:relative;${on ? 'background:var(--sel)' : hover ? 'background:var(--hov)' : ''}">
  <div class="row" style="gap:10px;height:24px"><span style="opacity:${dim && !on && !hover ? 0.4 : 1};display:inline-flex">${icon}</span><span class="grow trunc ${titleClass ?? (on ? '' : 'mu')}">${title}</span>${end ?? `<span class="sm mu">${time}</span>`}</div>
  ${line2 ? `<div class="row g1 sm faint" style="padding-left:26px;min-width:0">${line2}</div>` : ''}</div>`;

const CARDS = [
  { project: 'st', name: 'storefront', time: '4m', title: 'Add the checkout page', on: true },
  { project: 'st', name: 'storefront', time: '18m', title: 'Write tests for the rate limiter' },
  { project: 'ap', name: 'api', time: '33m', title: 'Fix flaky login test' },
];
const PANE_THREADS = [
  { id: 'w1', project: 'st', title: 'Tidy the checkout styles', folder: '~/storefront/src', shownIn: 'storefront › agents', space: 'storefront', time: '2m' },
  { id: 'w2', project: null, title: 'Summarize the release notes', folder: '~/docs', shownIn: 'Release notes › 1', space: 'Release notes', time: '25m' },
  { id: 'w3', project: null, title: 'Why is the build slow?', folder: '~', shownIn: null, space: null, time: '3h' },
];
const ARCHIVED = [
  { project: 'st', title: 'Rename the cart store', time: '1h' },
  { project: 'ap', title: 'Bump the API version', time: '1h' },
];
const cards = ({ selected = true } = {}) => CARDS.map((card) => threadCard({ ...card, on: selected && card.on })).join('');
const shells = () => shelfHeader('Shells') + slimRow({ icon: projectIcon('st'), title: 'zsh', time: '10m', line2: '<span class="grow">main</span>' + ic('laptop', 'xs') });
const archived = ({ open = true } = {}) => shelfHeader('Archived', { open, count: ARCHIVED.length }) + (open ? ARCHIVED.map((t) => slimRow({ icon: projectIcon(t.project), title: t.title, time: t.time })).join('') : '');
const agentsSidebar = ({ section = '', head = sbHead(), list, selected = true } = {}) => frame(sidebar({
  head,
  list: list ?? `${cards({ selected })}<div class="grow"></div>${shells()}${section}${archived()}`,
}), { w: 290, h: PT_H });
const paneThreadRow = (t, extra = {}) => slimRow({ icon: projectIcon(t.project), title: t.title, time: t.time, ...extra });

// 1. The section's rows -------------------------------------------------------------------
TOPICS.push({
  id: 'section', section: 'Agents sidebar', title: 'Workspaces section rows', size: 'narrow', rec: 'A',
  now: 'The sidebar lists thread cards, then the Shells and Archived shelves. A thread started in a pane is an ordinary project thread among the cards; outside a project, New Thread… first asks you to pick a project. The Workspaces section goes just above Archived and is styled like it (decided); this is what each row shows.',
  nowImg: 'img/now-sidebar.png',
  options: [
    {
      key: 'A', name: 'Like Archived rows', from: 'agentZ Archived shelf',
      desc: 'One line: the icon of the project its folder is in (a folder icon outside every project), the title, and when it last did something. Dimmed icons and muted titles, like Archived, so nothing asks for attention.',
      good: 'Matches the shelf it sits beside; quietest.', cost: 'Doesn\'t say where the thread works or which pane shows it.',
      mock: () => agentsSidebar({ section: shelfHeader('Workspaces') + PANE_THREADS.map((t) => paneThreadRow(t)).join('') }),
    },
    {
      key: 'B', name: 'Like Shells rows, with the folder', from: 'agentZ Shells shelf',
      desc: 'Two lines, as a shell\'s row: the title and time, then the folder it works in (<code>~</code> paths) and the machine.',
      good: 'You see where each thread works, which matters since they can be anywhere.', cost: 'Twice the height of an Archived row.',
      mock: () => agentsSidebar({ section: shelfHeader('Workspaces') + PANE_THREADS.map((t) => paneThreadRow(t, { line2: `<span class="grow trunc">${t.folder}</span>${ic('laptop', 'xs')}` })).join('') }),
    },
    {
      key: 'C', name: 'Where its pane is', from: 'agentZ Workspaces view Agents list',
      desc: 'Two lines: the title and time, then the workspace and tab showing it (<code>storefront › agents</code>), as the Workspaces view\'s Agents list says. Once its pane is closed, the folder instead.',
      good: 'Tells you where to find it in the Workspaces view.', cost: 'Two lines; the place changes as panes move.',
      mock: () => agentsSidebar({ section: shelfHeader('Workspaces') + PANE_THREADS.map((t) => paneThreadRow(t, { line2: t.shownIn ? `<span class="grow trunc">${t.shownIn}</span>` : `<span class="grow trunc">${t.folder} · no pane</span>` })).join('') }),
    },
    {
      key: 'D', name: 'Grouped by workspace', from: 'herdr workspace grouping',
      desc: 'Rows like A under the name of the workspace showing them; threads whose pane was closed gather under "No pane".',
      good: 'Mirrors the Workspaces view\'s own list.', cost: 'Sub-headers in a short section; more structure than it needs with a few threads.',
      mock: () => {
        const sub = (name) => `<div class="row" style="height:22px;padding:0 14px;font-size:11px;color:var(--ph)">${name}</div>`;
        const groups = [['storefront', [PANE_THREADS[0]]], ['Release notes', [PANE_THREADS[1]]], ['No pane', [PANE_THREADS[2]]]];
        return agentsSidebar({ section: shelfHeader('Workspaces') + groups.map(([name, threads]) => sub(name) + threads.map((t) => paneThreadRow(t)).join('')).join('') });
      },
    },
    {
      key: 'E', name: 'Folder in place of the time', from: 'new',
      desc: 'One line like A, but the right side shows the folder\'s last part (<code>src</code>, <code>docs</code>, <code>~</code>) instead of the time; the full path and the time are in its tooltip.',
      good: 'One line and still says where.', cost: 'Loses the time at a glance; folder names can repeat.',
      mock: () => agentsSidebar({ section: shelfHeader('Workspaces') + PANE_THREADS.map((t) => paneThreadRow(t, { end: `<span class="sm faint trunc" style="max-width:90px">${t.folder.split('/').pop() || '~'}</span>` })).join('') }),
    },
  ],
});

// 2. Open or closed -----------------------------------------------------------------------
TOPICS.push({
  id: 'open', section: 'Agents sidebar', title: 'Workspaces section: open or closed', size: 'narrow', rec: 'A',
  now: 'Shells starts open each time the app starts. Archived remembers whether you left it open (kept by this Mac\'s server) and starts closed. A closed shelf shows its count.',
  options: [
    {
      key: 'A', name: 'Like Archived: remembered, closed at first', from: 'agentZ Archived shelf',
      desc: 'Starts closed with its count beside the name; opening or closing it is remembered across restarts, as Archived is.',
      good: 'Stays out of the way; matches the shelf it\'s styled after.', cost: 'A thread whose pane you closed is one click further.',
      mock: () => agentsSidebar({ section: shelfHeader('Workspaces', { open: false, count: 3 }) }),
    },
    {
      key: 'B', name: 'Like Shells: open', from: 'agentZ Shells shelf',
      desc: 'Starts open each time; closing it lasts until the app restarts.',
      good: 'Always visible.', cost: 'Takes room in the sidebar every time.',
      mock: () => agentsSidebar({ section: shelfHeader('Workspaces') + PANE_THREADS.map((t) => paneThreadRow(t)).join('') }),
    },
    {
      key: 'C', name: 'Closed, but keeps the open thread', from: 'herdr collapsed groups',
      desc: 'Like A, but while a Workspaces thread is open in the Agents view, its row stays under the closed header, as herdr keeps the focused workspace visible in a collapsed group.',
      good: 'You can always see what\'s selected.', cost: 'A closed section that still shows a row can look odd.',
      mock: () => agentsSidebar({ selected: false, section: shelfHeader('Workspaces', { open: false, count: 3 }) + paneThreadRow(PANE_THREADS[1], { on: true }) }),
    },
  ],
});

// 3. Move to Agents -----------------------------------------------------------------------
const rowMenu = (items, style) => `<div class="menu" style="${style}">${items.map((item) => (item === '-' ? '<div class="hr"></div>' : `<div class="it ${item.cls || ''}">${item.icon ? ic(item.icon, 'sm') : ''}${item.label}${item.kb ? `<span class="kb">${item.kb}</span>` : ''}</div>`)).join('')}</div>`;
const moveButton = `<span class="ibtn sm hov" style="width:20px;height:20px">${ic('corner', 'xs')}</span>`;
TOPICS.push({
  id: 'move', section: 'Agents sidebar', title: 'Move to Agents', size: 'narrow', rec: 'A',
  now: 'Archived rows have an Unarchive button on hover, and Unarchive in their right-click menu, which also has Rename, Project Settings and Delete. Workspaces threads don\'t exist yet.',
  options: [
    {
      key: 'A', name: 'Button on hover, and in the menu', from: 'agentZ Unarchive',
      desc: 'Hovering a row shows a Move to Agents button where the time is, as Unarchive does on Archived rows. Its right-click menu has Move to Agents too, with Rename, Archive and Delete (no Project Settings).',
      good: 'One click, and works like the shelf beside it.', cost: 'Another hover button to learn.',
      mock: () => agentsSidebar({ section: shelfHeader('Workspaces') + paneThreadRow(PANE_THREADS[0]) + paneThreadRow(PANE_THREADS[1], { hover: true, end: moveButton }) + paneThreadRow(PANE_THREADS[2]) })
        .replace(/<\/div>$/, `${note('Move to Agents', 'left:168px;top:524px')}</div>`),
    },
    {
      key: 'B', name: 'Only in the right-click menu', from: 'agentZ row menus',
      desc: 'No hover button; right-click a row for Rename, Move to Agents, Archive and Delete.',
      good: 'Rows stay as plain as Archived\'s without the button.', cost: 'Hidden until you right-click.',
      mock: () => {
        const html = agentsSidebar({ section: shelfHeader('Workspaces') + PANE_THREADS.map((t, i) => paneThreadRow(t, { hover: i === 1 })).join('') });
        return html.replace(/<\/div>$/, `${rowMenu([{ label: 'Rename', icon: 'pencil' }, { label: 'Move to Agents', icon: 'corner', cls: 'hl' }, { label: 'Archive', icon: 'inbox' }, '-', { label: 'Delete', icon: 'trash', cls: 'danger' }], 'left:70px;top:520px')}</div>`);
      },
    },
    {
      key: 'C', name: 'In the thread\'s own menu too', from: 'agentZ thread title menu',
      desc: 'As B, and also in the thread\'s title menu when it\'s open (in the Agents view or its pane): Rename, Move to Agents, Continue with Another Agent, Archive, Delete.',
      good: 'Reachable from wherever you\'re reading the thread.', cost: 'Two places to keep in step.',
      mock: () => frame(`<div class="row g1" style="height:36px;padding:0 8px;border-bottom:1px solid var(--b);background:var(--ed)"><span class="row g1 sm mu" style="padding:0 4px">${ic('folder', 'sm')}docs</span><span class="sm mu">/</span><span class="row g1" style="padding:0 4px">Summarize the release notes ${ic('chev-down', 'xs mu')}</span></div>${rowMenu([{ label: 'Rename', icon: 'pencil' }, { label: 'Move to Agents', icon: 'corner', cls: 'hl' }, { label: 'Continue with Another Agent', icon: 'arrow' }, { label: 'Archive', icon: 'inbox' }, '-', { label: 'Delete', icon: 'trash', cls: 'danger' }], 'left:70px;top:38px')}`, { w: 340, h: 230 }),
    },
    {
      key: 'D', name: 'Drag it into the thread list', from: 'new',
      desc: 'Drag a row up among the thread cards to move it; the list shows where it lands. The menu entry stays for the keyboard.',
      good: 'Direct; you see it join the list.', cost: 'The cards are sorted by time, so where you drop it means nothing.',
      mock: () => agentsSidebar({ section: shelfHeader('Workspaces') + paneThreadRow(PANE_THREADS[0]) + paneThreadRow(PANE_THREADS[2]) })
        .replace(/<\/div>$/, `<div style="position:absolute;left:8px;right:8px;top:206px;height:2px;background:var(--ac)"></div><div style="position:absolute;left:20px;top:214px;width:240px;opacity:.85;background:var(--hov);border:1px solid var(--b);border-radius:6px;box-shadow:0 8px 20px rgba(0,0,0,.4)">${paneThreadRow(PANE_THREADS[1], { dim: false })}</div></div>`),
    },
  ],
});

// 4. Asking to add the folder ---------------------------------------------------------------
const alert = (title, detail, buttons) => `<div style="position:absolute;left:50%;top:50%;transform:translate(-50%,-50%);width:260px;background:#3a3f4a;border:1px solid #555b67;border-radius:12px;box-shadow:0 18px 44px rgba(0,0,0,.55);padding:18px 16px 14px;text-align:center;z-index:20">
  <div style="width:44px;height:44px;margin:0 auto 10px;border-radius:10px;background:#4a90e2;display:grid;place-items:center;color:#fff">${ic('folder', 'lg')}</div>
  <div class="b6" style="font-size:13px">${title}</div><div class="sm mu" style="margin-top:6px;line-height:1.4">${detail}</div>
  <div class="col g1" style="margin-top:14px">${buttons.map(([label, primary]) => `<span style="height:26px;border-radius:6px;display:grid;place-items:center;font-size:13px;${primary ? 'background:#3a82f7;color:#fff' : 'background:#555b67'}">${label}</span>`).join('')}</div></div>`;
TOPICS.push({
  id: 'ask', section: 'Agents sidebar', title: 'Asking to add the folder as a project', size: 'medium', rec: 'A',
  now: 'Decided: a Workspaces thread outside every project moves to Agents only after you agree to add its folder as a project. Closing a pane that runs something asks first with a macOS alert ("Close “claude”?", Close / Cancel). Adding a project today is Open Folder…, a dialog where you type or browse to a path.',
  options: [
    {
      key: 'A', name: 'A macOS alert', from: 'agentZ close confirmations (Zed prompts)',
      desc: '"Add “~/docs” as a project?" with "Threads in the Agents list belong to a project." and Add Project / Cancel. Add Project adds it and moves the thread there.',
      good: 'Same as the other confirmations; one click.', cost: 'A modal alert for a small step.',
      mock: () => frame(`<div style="position:absolute;inset:0;background:rgba(0,0,0,.25)"></div>${alert('Add “~/docs” as a project?', 'Threads in the Agents list belong to a project. This thread works in ~/docs, which isn\'t in one.', [['Add Project', true], ['Cancel', false]])}`, { w: 420, h: 300 }),
    },
    {
      key: 'B', name: 'A popover by the row', from: 't3code inline confirm',
      desc: 'Move to Agents opens a small popover beside the row: "~/docs isn\'t in a project" and an Add Project and Move button.',
      good: 'Stays next to what you clicked; not modal.', cost: 'Easy to dismiss by accident; a new kind of popover in the sidebar.',
      mock: () => frame(`${sidebar({ list: `${cards()}<div class="grow"></div>${shelfHeader('Workspaces')}${PANE_THREADS.map((t, i) => paneThreadRow(t, { hover: i === 1, end: i === 1 ? moveButton : undefined })).join('')}${archived({ open: false })}` })}<div class="pop" style="left:200px;top:596px;width:230px;padding:10px 12px"><div class="sm">~/docs isn't in a project</div><div class="xs mu" style="margin-top:4px">Threads in the Agents list belong to one.</div><div class="row g2" style="margin-top:10px;justify-content:flex-end"><span class="btn sm ghost">Cancel</span><span class="btn sm primary">Add Project and Move</span></div></div>`, { w: 440, h: PT_H }),
    },
    {
      key: 'C', name: 'Open Folder… with the folder filled in', from: 'agentZ Open Folder dialog',
      desc: 'Opens the usual Open Folder… dialog with <code>~/docs</code> already typed, so you can also pick a folder above it (<code>~</code>) as the project. Adding it moves the thread.',
      good: 'Lets you choose a bigger project folder.', cost: 'More steps for the usual case.',
      mock: () => frame(`<div style="position:absolute;inset:0;background:rgba(0,0,0,.3)"></div><div class="card" style="position:absolute;left:50%;top:40px;transform:translateX(-50%);width:380px;padding:12px;box-shadow:0 12px 32px rgba(0,0,0,.45)"><div class="b5" style="font-size:13px">Add a Project</div><div class="xs mu" style="margin:4px 0 10px">Moving “Summarize the release notes” to Agents needs its folder in a project.</div><div class="field focus">${ic('folder', 'sm mu')}~/docs<span class="cursor" style="width:1px;height:14px;margin-left:1px"></span></div><div class="col" style="margin-top:6px">${['~/docs/notes', '~/docs/specs'].map((p) => `<div class="row g2 sm mu" style="height:24px;padding:0 6px">${ic('folder', 'xs')}${p}</div>`).join('')}</div><div class="row g2" style="margin-top:10px;justify-content:flex-end"><span class="btn sm ghost">Cancel</span><span class="btn sm primary">Add Project</span></div></div>`, { w: 440, h: 280 }),
    },
  ],
});

// 5. Search results --------------------------------------------------------------------------
const searchHead = () => `<div class="sb-head">${ic('search', 'sm')}<span class="grow" style="color:var(--t)">e</span>${ibtn('x')}${ibtn('plus')}</div>`;
const RESULTS = [
  { icon: projectIcon('st'), title: 'Add the checkout page', time: '4m' },
  { icon: projectIcon('st'), title: 'Write tests for the rate limiter', time: '18m' },
  { icon: projectIcon('ap'), title: 'Fix flaky login test', time: '33m' },
  { icon: projectIcon('st'), title: 'Tidy the checkout styles', time: '2m', kind: 'Workspaces' },
  { icon: projectIcon(null), title: 'Summarize the release notes', time: '25m', kind: 'Workspaces' },
  { icon: projectIcon('st'), title: 'Rename the cart store', time: '2h', kind: 'Archived' },
  { icon: projectIcon('ap'), title: 'Bump the API version', time: '2h', kind: 'Archived' },
];
const resultRow = (r, { on = false, tag = false, dim = false } = {}) => `<div class="row" style="height:30px;margin:0 4px;padding:0 10px;gap:10px;border-radius:6px;${on ? 'background:var(--sel)' : ''}"><span style="display:inline-flex;opacity:${dim ? 0.4 : 1}">${r.icon}</span><span class="grow trunc ${on ? '' : 'mu'}">${r.title}</span>${tag && r.kind ? `<span class="xs faint">${r.kind}</span>` : ''}<span class="sm mu">${r.time}</span></div>`;
const searchSidebar = (rows) => frame(sidebar({ head: searchHead(), list: rows }), { w: 290, h: 380 });
TOPICS.push({
  id: 'search', section: 'Agents sidebar', title: 'Search results', size: 'narrow', rec: 'B',
  now: 'Typing in the search swaps the list for t3code\'s flat results: matching threads, then shells, then archived ones, each with its project\'s icon, title and time. Decided: it also finds Workspaces threads (under a project, those whose folder is in it). Up and down move the highlight; Enter opens.',
  nowImg: 'img/now-search.png',
  options: [
    {
      key: 'A', name: 'Same rows', from: 't3code search',
      desc: 'Workspaces threads join the flat list after shells, with the icon of the project their folder is in (a folder icon outside every project). Nothing marks which list a result is in.',
      good: 'Simplest; what t3code does.', cost: 'You can\'t tell a Workspaces or archived thread from an active one.',
      mock: () => searchSidebar(RESULTS.map((r, i) => resultRow(r, { on: i === 0 })).join('')),
    },
    {
      key: 'B', name: 'With a faint list name', from: 'new',
      desc: 'Like A, with "Workspaces" or "Archived" in faint text before the time on those results.',
      good: 'Tells you where it lives before you open it.', cost: 'A little more text per row.',
      mock: () => searchSidebar(RESULTS.map((r, i) => resultRow(r, { on: i === 0, tag: true })).join('')),
    },
    {
      key: 'C', name: 'Under small headers', from: 'new',
      desc: 'Results grouped under Threads, Workspaces and Archived headers, in that order; the arrow keys skip the headers.',
      good: 'Clearest grouping.', cost: 'Headers take room in a short list.',
      mock: () => {
        const head = (name) => `<div class="row" style="height:22px;padding:0 14px;font-size:11px;color:var(--ph)">${name}</div>`;
        const of = (kind) => RESULTS.filter((r) => (r.kind || 'Threads') === kind);
        return searchSidebar(['Threads', 'Workspaces', 'Archived'].map((kind) => head(kind) + of(kind).map((r) => resultRow(r, { on: r === RESULTS[0] })).join('')).join(''));
      },
    },
    {
      key: 'D', name: 'Dimmed like their sections', from: 'agentZ shelves',
      desc: 'Like A, but Workspaces and archived results keep their sections\' dimmed icon, as in the sidebar.',
      good: 'Quiet, consistent with the shelves.', cost: 'Subtle; easy to miss.',
      mock: () => searchSidebar(RESULTS.map((r, i) => resultRow(r, { on: i === 0, dim: !!r.kind })).join('')),
    },
  ],
});

// 6. A new thread in a pane -----------------------------------------------------------------
const composer = (strip) => `<div class="col" style="position:absolute;left:24px;right:24px;top:70px"><div style="text-align:center;font-size:22px;margin-bottom:18px">What should we work on?</div>
  <div style="border:1px solid var(--b);border-radius:8px;background:var(--ed);padding:10px 12px"><div class="ph" style="font-size:13px">Message the agent…</div><div class="row g2 sm mu" style="margin-top:22px">${ic('terminal', 'xs')}Mock ${ic('chev-down', 'xs')}<span class="grow"></span>Default ${ic('chev-down', 'xs')} Sonnet ${ic('chev-down', 'xs')}</div></div>
  <div class="row g2 sm mu" style="margin-top:8px;padding:0 4px">${strip}</div></div>`;
const draftPane = (strip, header = '') => frame(`<div class="pane focus" style="position:absolute;inset:0"><div class="phead">${ic('terminal', 'sm')}<span>New thread</span><span class="mu">Mock</span><span class="grow"></span>${ibtn('split')}${ibtn('x')}</div>${header}<div style="flex:1;background:#2f343e;position:relative">${composer(strip)}</div></div>`, { w: 600, h: 320 });
const branchRight = `<span class="grow"></span>${ic('branch', 'xs')}checkout-flow`;
TOPICS.push({
  id: 'draft', section: 'Workspaces view', title: 'A new thread in a pane: where it works', size: 'medium', rec: 'A',
  now: 'New Thread… in a pane opens a draft in the workspace\'s project, with the Agents view\'s checkout picker under the composer (Local, New worktree, New pasture, existing ones) and the branch on the right. Decided: a pane\'s thread works in the shell\'s current folder (else the workspace\'s) and isn\'t a project thread, so that picker doesn\'t fit.',
  nowImg: 'img/now-pane-draft.png',
  options: [
    {
      key: 'A', name: 'The folder, as a plain chip', from: 'agentZ static chips',
      desc: 'A chip with a folder icon and <code>~/storefront/src</code> (full path in its tooltip), not a menu; the branch stays on the right when the folder is in git.',
      good: 'Says exactly where the agent will work; nothing to choose.', cost: 'To work elsewhere you <code>cd</code> in a shell first.',
      mock: () => draftPane(`<span class="chip">${ic('folder', 'xs')}~/storefront/src</span>${branchRight}`),
    },
    {
      key: 'B', name: 'Shell\'s folder or the workspace\'s', from: 'new',
      desc: 'Like A, but the chip is a menu with the two folders it could start in: where the shell is now, and the workspace\'s folder.',
      good: 'Fixes a wrong guess without leaving the pane.', cost: 'A menu for a choice that\'s usually obvious.',
      mock: () => draftPane(`<span class="chip">${ic('folder', 'xs')}~/storefront/src ${ic('chev-down', 'xs')}</span>${branchRight}`).replace(/<\/div>$/, `${rowMenu([{ label: '~/storefront/src', icon: 'terminal', kb: '✓' }, { label: '~/storefront', icon: 'folder' }], 'left:28px;top:296px;transform:translateY(-100%)')}</div>`),
    },
    {
      key: 'C', name: 'Only the branch', from: 'agentZ new thread strip',
      desc: 'No chip on the left; the branch on the right, with the folder in its tooltip. The pane header already shows the folder once the thread starts.',
      good: 'Least on screen.', cost: 'Where it works is hidden until you hover.',
      mock: () => draftPane(`${branchRight}`),
    },
    {
      key: 'D', name: 'Folder with a Workspaces tag', from: 'new',
      desc: 'Like A, with a faint "Workspaces" before the folder, to say the thread will list in the Agents sidebar\'s Workspaces section rather than under the project.',
      good: 'Explains where it goes.', cost: 'More words in a small strip.',
      mock: () => draftPane(`<span class="faint">Workspaces</span><span class="chip">${ic('folder', 'xs')}~/storefront/src</span>${branchRight}`),
    },
  ],
});

// 7. The thread's title bar in the Agents view ----------------------------------------------
const toolbar = (crumb, extra = '') => frame(`<div class="row g1" style="height:36px;padding:0 8px;border-bottom:1px solid var(--b);background:var(--ed)">${crumb}<span class="row g1" style="padding:0 4px">Tidy the checkout styles ${ic('chev-down', 'xs mu')}</span><span class="grow"></span>${extra}<span class="chip">${ic('branch', 'xs')}checkout-flow</span>${ibtn('diff')}${ibtn('terminal')}${ibtn('more')}</div>`, { w: 720, h: 36 });
const crumb = (icon, name) => `<span class="row g15 sm mu" style="padding:2px 4px">${icon}${name}</span><span class="sm mu">/</span>`;
TOPICS.push({
  id: 'crumb', section: 'Agents view', title: 'The thread\'s title bar', size: 'wide', rec: 'A',
  now: 'Opened in the Agents view, a thread\'s title bar starts with its project (icon and name; a click starts a new thread there), then the title with its menu, then the branch and buttons. A Workspaces thread has no project.',
  nowImg: 'img/now-toolbar.png',
  options: [
    {
      key: 'A', name: 'Its folder', from: 'agentZ shell rows',
      desc: 'A folder icon and the folder\'s name (<code>src</code>), full path in the tooltip; clicking it does nothing.',
      good: 'Says where it works, in the project\'s place.', cost: 'Folder names can be vague (<code>src</code>).',
      mock: () => toolbar(crumb(ic('folder', 'sm'), 'src')),
    },
    {
      key: 'B', name: '"Workspaces"', from: 'new',
      desc: 'The Workspaces view\'s icon and "Workspaces"; a click shows the thread in its pane there (while a pane shows it).',
      good: 'Says which list it\'s in and takes you to it.', cost: 'Doesn\'t say where it works.',
      mock: () => toolbar(crumb(ic('layers', 'sm'), 'Workspaces')),
    },
    {
      key: 'C', name: 'Just the title', from: 'agentZ without a project',
      desc: 'No crumb: the title, its menu, the branch and buttons.',
      good: 'Nothing to explain.', cost: 'Looks like something is missing next to project threads.',
      mock: () => toolbar(''),
    },
    {
      key: 'D', name: 'Project it\'s in, else folder', from: 'agentZ shell rows',
      desc: 'The project its folder is in (<code>ST storefront</code>), as shell rows show, or the folder outside every project; no click action.',
      good: 'Familiar next to project threads.', cost: 'Looks like a project thread though it isn\'t one.',
      mock: () => toolbar(crumb(mono('ST', 'g'), 'storefront')),
    },
    {
      key: 'E', name: 'Folder and a Move to Agents button', from: 'new',
      desc: 'Like A, plus a Move to Agents button before the branch.',
      good: 'The move is one click from the thread.', cost: 'Another button in the title bar.',
      mock: () => toolbar(crumb(ic('folder', 'sm'), 'src'), `<span class="btn sm">${ic('corner', 'xs')}Move to Agents</span>`),
    },
  ],
});

