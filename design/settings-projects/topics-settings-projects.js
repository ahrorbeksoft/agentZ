// Projects in Settings: a combined project listed once in the settings sidebar, a machine
// picker on its page, shared settings apart from each copy's, and what Remove removes.

ICONS.updown = '<path d="m7 15 5 5 5-5"/><path d="m7 9 5-5 5 5"/>';
ICONS.sparkle = '<path d="M9.937 15.5A2 2 0 0 0 8.5 14.063l-6.135-1.582a.5.5 0 0 1 0-.962L8.5 9.936A2 2 0 0 0 9.937 8.5l1.582-6.135a.5.5 0 0 1 .963 0L14.063 8.5A2 2 0 0 0 15.5 9.937l6.135 1.581a.5.5 0 0 1 0 .964L15.5 14.063a2 2 0 0 0-1.437 1.437l-1.582 6.135a.5.5 0 0 1-.963 0z"/>';
ICONS['arrow-left'] = '<path d="m12 19-7-7 7-7"/><path d="M19 12H5"/>';
ICONS.gauge = '<path d="m12 14 4-4"/><path d="M3.34 19a10 10 0 1 1 17.32 0"/>';
ICONS.book = '<path d="M4 19.5v-15A2.5 2.5 0 0 1 6.5 2H19a1 1 0 0 1 1 1v18a1 1 0 0 1-1 1H6.5a1 1 0 0 1 0-5H20"/>';
ICONS.hammer = '<path d="m15 12-8.373 8.373a1 1 0 1 1-3-3L12 9"/><path d="m18 15 4-4"/><path d="m21.5 11.5-1.914-1.914A2 2 0 0 1 19 8.172V7l-2.26-2.26a6 6 0 0 0-4.202-1.756L9 2.96l.92.82A6.18 6.18 0 0 1 12 8.4V10l2 2h1.172a2 2 0 0 1 1.414.586L18.5 14.5"/>';

// The demo projects, as in the user's screenshot. Each copy is one machine's folder.
const MACHINES = { mac: ['This Mac', 'laptop'], laptop: ['Ahrorbek’s Laptop', 'laptop'], devbox: ['Devbox 1', 'server'] };
const machineName = (machine) => MACHINES[machine][0];
const machineIc = (machine, cls = 'sm mu') => ic(MACHINES[machine][1], cls);
const PROJECTS = [
  { name: 'agentZ', mono: 'AZ', bg: '#4a3533', fg: '#e8836f', copies: [['mac', '/Users/ahrorbek/projects/agentZ']] },
  { name: 'ielts-today', mono: 'IT', bg: '#3b3a5c', fg: '#a7a2f0', copies: [['mac', '/Users/ahrorbek/projects/svelte5/ielts-today'], ['laptop', '/home/ahrorbek/projects/ielts-today']] },
  { name: 'fluency.uz', mono: 'FU', bg: '#4d3238', fg: '#e57887', copies: [['mac', '/Users/ahrorbek/projects/fluency.uz'], ['devbox', '/root/projects/fluency.uz'], ['laptop', '/home/ahrorbek/projects/fluency.uz']] },
];
const IELTS = PROJECTS[1];
const projectIcon = (project, size = 14) => `<span style="width:${size}px;height:${size}px;border-radius:${size > 16 ? 6 : 4}px;flex:none;display:inline-grid;place-items:center;background:${project.bg};color:${project.fg};font-size:${Math.round(size * 0.45)}px;font-weight:700;letter-spacing:-.2px">${project.mono}</span>`;

// Settings controls as settings_page.rs draws them: ui's DropdownMenu, Button and the
// text inputs' bordered box.
const dd = (label, extra = '') => `<span class="row" style="gap:6px;height:24px;padding:0 6px 0 8px;border-radius:6px;border:1px solid var(--b);background:var(--hov);white-space:nowrap;position:relative;font-size:13px">${label}${ic('updown', 'xs mu')}${extra}</span>`;
const menuOf = (items, chosen, style = 'top:30px;right:0') => `<div class="menu" style="${style}">${items.map(([label, icon]) => `<div class="it ${label === chosen ? 'hl' : ''}">${icon || ''}<span class="grow">${label}</span>${label === chosen ? ic('check', 'sm') : ''}</div>`).join('')}</div>`;
const machineMenu = (project, chosen, style) => menuOf(project.copies.map(([machine]) => [machineName(machine), machineIc(machine)]), chosen, style);
const input = (text, w, placeholder = true) => `<span class="row" style="width:${w}px;height:28px;padding:0 8px;border-radius:6px;border:1px solid var(--b);background:var(--ed);font-size:13px;color:${placeholder ? 'var(--ph)' : 'var(--t)'}">${text}</span>`;
const outlined = (label, cls = '') => `<span class="btn ${cls}" style="background:none">${label}</span>`;
const dangerBtn = (label) => `<span class="btn" style="background:none;color:var(--del)">${ic('trash', 'sm')}${label}</span>`;
const SWATCHES = ['#9aa0aa', '#e06c75', '#ef8a5b', '#e5c07b', '#d6d34f', '#a1d36a', '#6fd38b', '#5ecfa8', '#56c6c2', '#5fb8e8', '#74a8f0', '#8f8ff0', '#b08cf0', '#c678dd', '#e87fd8', '#e78aa8', '#e8737f'];
const swatches = () => `<span class="row" style="width:256px;flex-wrap:wrap;gap:4px;justify-content:flex-end">${SWATCHES.map((color) => `<i style="width:18px;height:18px;border-radius:50%;background:${color};display:block"></i>`).join('')}</span>`;

// render_row and render_section: a title and description beside the control, in a bordered
// group under a small muted heading.
const setRow = (title, desc, control = '', extra = '') => `<div class="row" style="padding:12px 16px;gap:24px;position:relative"><div class="col grow" style="gap:2px"><span>${title}</span>${desc ? `<span class="sm mu" style="overflow-wrap:anywhere">${desc}</span>` : ''}</div>${control ? `<div class="none" style="position:relative">${control}${extra}</div>` : ''}</div>`;
const setSection = (title, rows, { right = '', note = '' } = {}) => `<div class="col" style="gap:8px"><div class="row" style="justify-content:space-between;min-height:16px"><span class="sm mu row g15">${title}</span>${right}</div><div class="col" style="border:1px solid var(--b);border-radius:8px;background:var(--panel)">${rows.map((row, index) => `<div style="${index < rows.length - 1 ? 'border-bottom:1px solid var(--bv)' : ''}">${row}</div>`).join('')}</div>${note ? `<span class="sm mu">${note}</span>` : ''}</div>`;
const page = (heading, sections, { right = '', sub = '' } = {}) => `<div class="col" style="width:720px;margin:0 auto;padding:24px 32px 32px;gap:24px"><div class="row" style="justify-content:space-between;gap:12px;position:relative;min-height:26px"><div class="col" style="gap:2px"><span style="font-size:17px">${heading}</span>${sub}</div>${right}</div>${sections.join('')}</div>`;
const piece = (html, w = 720, h) => `<div class="m" style="width:${w}px;${h ? `height:${h}px;` : ''}">${html}</div>`;

// The project page's rows, as render_project and render_repository draw them.
const nameRow = (project = IELTS) => setRow('Name', 'Shown in the sidebar and thread lists. Leave it empty for the folder name.', input(project.name, 256));
const iconRow = (project = IELTS) => setRow('Icon', 'Automatic: the project’s favicon, or a monogram.', `<span class="row g2">${projectIcon(project, 24)}${outlined('Choose File…')}</span>`);
const monogramRow = (project = IELTS) => setRow('Monogram', 'Letters and a color for a custom monogram icon.', `<span class="col" style="align-items:flex-end;gap:8px">${input(project.mono, 64)}${swatches()}</span>`);
const copyPath = (project, machine) => project.copies.find(([m]) => m === machine)[1];
const folderRow = (machine = 'mac', project = IELTS, title = 'Folder') => setRow(title, copyPath(project, machine));
const repositoryRow = () => setRow('Repository', 'ahrorbeksoft/ielts-today · origin https://github.com/ahrorbeksoft/ielts-today.git');
const groupingRow = () => setRow('Grouping', 'Projects from the same repository share one row.', dd('Default'));
const noCheckouts = () => `<div class="sm mu" style="padding:12px 16px">No worktrees or pastures yet. New Thread offers them for git repositories, and agents can make them too.</div>`;
const checkoutRow = (branch, kind, path, threads, extraDetail = '') => `<div class="row g3" style="padding:12px 16px">${ic(kind === 'Worktree' ? 'worktree' : 'pasture', 'sm mu')}<div class="col grow" style="gap:2px"><span class="trunc">${branch}</span><span class="sm mu trunc">${extraDetail}${kind} · ${path}${threads ? ` · ${threads === 1 ? '1 thread' : `${threads} threads`}` : ''}</span></div>${outlined('Remove…')}</div>`;
// What each copy of ielts-today has under Checkouts: the laptop's has a worktree.
const CHECKOUTS = {
  mac: () => [noCheckouts()],
  laptop: () => [checkoutRow('agentz/brave-otter-3fa', 'Worktree', '~/projects/ielts-today-worktrees/brave-otter', 1)],
};
const removeRow = (title = 'Remove project', desc = 'Removes the project and its threads from agentZ. Files on disk are not touched.', label = 'Remove Project') => setRow(title, desc, dangerBtn(label));

const projectSection = (rows) => setSection('Project', rows);
const repositorySection = (rows = [repositoryRow(), groupingRow()]) => setSection('Repository', rows);
const checkoutsSection = (machine = 'mac', title = 'Checkouts', opts) => setSection(title, CHECKOUTS[machine](), opts);
const dangerSection = (rows = [removeRow()], title = 'Danger') => setSection(title, rows);

// The settings window: render_nav's fixed pages, then "Projects" and one row per entry.
const FIXED_NAV = [['General', 'settings'], ['Appearance', 'eye'], ['Notifications', 'bell'], ['Agents', 'sparkle'], ['Usage', 'gauge'], ['Skills', 'book'], ['MCP Servers', 'hammer'], ['Machines', 'server']];
const navItem = (label, icon, { on = false, indent = 0, end = '', muted = false } = {}) => `<div class="row g2" style="height:28px;padding:0 8px 0 ${8 + indent}px;border-radius:6px;margin:0 4px;${on ? 'background:var(--sel)' : ''}">${icon}<span class="grow trunc" style="${muted ? 'color:var(--mu)' : ''}">${label}</span>${end}</div>`;
const fixedNav = () => FIXED_NAV.map(([label, icon]) => navItem(label, ic(icon, 'sm mu'))).join('');
const projectsHeading = () => '<div class="sm mu" style="padding:12px 12px 4px">Projects</div>';
// Today: one row per copy, the remote ones with their machine's name.
const todayProjectRows = (selected = 'ielts-today') => PROJECTS.flatMap((project) => project.copies.map(([machine]) => {
  const label = machine === 'mac' ? project.name : `${project.name} · ${machineName(machine)}`;
  return navItem(label, projectIcon(project), { on: label === selected });
})).join('');
function settingsWindow(contentHtml, { projects = todayProjectRows(), w = 1010, h = 640, contentStyle = '' } = {}) {
  return `<div class="m win" style="width:${w}px;height:${h}px">
    <div class="titlebar">${lights()}${ic('sidebar', 'sm mu')}<span class="row g15" style="font-size:13px">${ic('list', 'xs mu')}All projects${ic('chev-down', 'xs mu')}</span><div class="viewtabs"><span class="on">Agents</span><span>Workspaces</span></div></div>
    <div class="body"><div class="sidebar"><div class="sb-head" style="color:var(--t)"><span class="grow b5" style="color:var(--t)">Settings</span>${ic('x', 'sm mu')}</div><div class="sb-list" style="gap:1px">${fixedNav()}${projectsHeading()}${projects}</div><div class="sb-foot">${ic('arrow-left', 'sm')}<span>Back</span></div></div>
    <div class="grow" style="height:100%;overflow:hidden;background:var(--ed);${contentStyle}">${contentHtml}</div></div></div>`;
}
// Only the settings sidebar, for the list topic.
const navOnly = (projects, h = 470) => frame(`<div class="sidebar" style="width:290px"><div class="sb-head" style="color:var(--t)"><span class="grow b5" style="color:var(--t)">Settings</span>${ic('x', 'sm mu')}</div><div class="sb-list" style="gap:1px">${fixedNav()}${projectsHeading()}${projects}</div><div class="sb-foot">${ic('arrow-left', 'sm')}<span>Back</span></div></div>`, { w: 290, h });
const tooltip = (text, style) => `<div class="pop" style="${style};padding:4px 8px;font-size:12px;white-space:nowrap;z-index:25">${text}</div>`;

// 1. The settings sidebar ------------------------------------------------------------------
const otherMachines = (project) => project.copies.filter(([machine]) => machine !== 'mac');
TOPICS.push({
  id: 'list', section: 'Settings sidebar', title: 'How the sidebar lists a combined project', size: 'narrow', rec: 'B',
  now: 'Under “Projects”, one row per copy: “ielts-today”, then “ielts-today · Ahrorbek’s Laptop”, and “fluency.uz” three times. This Mac’s copy has the plain name; every other machine’s copy adds “ · ” and the machine’s name. Each row opens that one copy’s page.',
  nowImg: 'img/now-nav.png',
  issues: [
    'A project combined across three machines takes three rows, while the Agents view’s project switcher and New Thread list it once.',
    'Nothing marks the first “fluency.uz” as also being on Devbox 1 and Ahrorbek’s Laptop.',
  ],
  options: [
    { key: 'A', name: 'Once, by its name only', from: 't3code',
      desc: 'One row per project, as t3code’s settings list them. Nothing shows which machines it’s on; the page says that. A project that’s only on another machine keeps “ · Devbox 1” after its name.',
      good: 'The plainest list, and the shortest rows.', cost: 'You can’t tell from the list which projects are on other machines.',
      mock: () => navOnly(PROJECTS.map((project) => navItem(project.name, projectIcon(project), { on: project === IELTS })).join('')) },
    { key: 'B', name: 'Once, with a machine icon at the end', from: 't3code',
      desc: 'One row per project. A project that’s also on another machine gets that machine’s icon at the row’s end, as t3code’s project picker does. Hovering it says “Also on Devbox 1, Ahrorbek’s Laptop”. Projects only on this Mac show no icon.',
      good: 'The rows stay short and still show which projects are on other machines.', cost: 'Which machines it is takes a hover.',
      mock: () => navOnly(PROJECTS.map((project) => navItem(project.name, projectIcon(project), { on: project === IELTS, end: otherMachines(project).length ? machineIc(otherMachines(project)[0][0]) : '' })).join('')
        + tooltip('Also on Devbox 1, Ahrorbek’s Laptop', 'position:absolute;right:8px;top:394px')) },
    { key: 'C', name: 'Once, with its machines after the name', from: 'agentZ’s project switcher',
      desc: 'One row per project, with its machines in muted text after the name: “fluency.uz  This Mac, Devbox 1, Ahrorbek’s Laptop”, as the project switcher in the title bar shows it. Projects only on this Mac show no machines.',
      good: 'The same as the project switcher, and no hover needed.', cost: 'Long machine names are cut off in the 290 px sidebar.',
      mock: () => navOnly(PROJECTS.map((project) => navItem(`${project.name}${project.copies.length > 1 ? `<span class="sm mu" style="margin-left:6px">${project.copies.map(([machine]) => machineName(machine)).join(', ')}</span>` : ''}`, projectIcon(project), { on: project === IELTS })).join('')) },
    { key: 'D', name: 'Once, with its machines nested while open', from: 'new',
      desc: 'One row per project. The open project shows its copies as indented rows under it, each with its machine’s icon and name. Clicking one shows that copy on the page. This also decides “Choosing the machine” (its option E).',
      good: 'Choosing the machine happens where you chose the project.', cost: 'The list grows and shifts as you open projects, and no other settings list does that.',
      mock: () => navOnly(PROJECTS.map((project) => navItem(project.name, projectIcon(project), { on: false }) + (project === IELTS ? project.copies.map(([machine], index) => navItem(machineName(machine), machineIc(machine), { on: index === 0, indent: 20 })).join('') : '')).join('')) },
  ],
});

// 2. Choosing the machine ------------------------------------------------------------------
// The top of the page with Ahrorbek's Laptop's copy chosen, so its folder and worktree show.
const laptopTop = (header) => page('ielts-today', [projectSection([nameRow(), iconRow(), monogramRow()]), setSection('Ahrorbek’s Laptop', [folderRow('laptop'), groupingRow()]), checkoutsSection('laptop')], header);
const listB = () => PROJECTS.map((project) => navItem(project.name, projectIcon(project), { on: project === IELTS, end: otherMachines(project).length ? machineIc(otherMachines(project)[0][0]) : '' })).join('');
const segOf = (project, chosen) => `<span class="seg" style="font-size:13px">${project.copies.map(([machine]) => `<span class="row g15 ${machine === chosen ? 'on' : ''}" style="padding:3px 10px">${machineIc(machine, 'sm')}${machineName(machine)}</span>`).join('')}</span>`;
const zedTab = (label, on) => `<span class="row g15" style="height:24px;padding:0 8px;border-radius:6px;font-size:13px;${on ? 'background:rgba(116,173,232,.16);color:var(--ac)' : 'color:var(--t)'}">${label}</span>`;
TOPICS.push({
  id: 'machine', section: 'Project page', title: 'Choosing the machine', size: 'wide', rec: 'A',
  now: 'No choice on the page: each copy has its own page, opened from its own row in the settings sidebar. The page’s Repository section lists the other copies in a “Combined with” row (“Ahrorbek’s Laptop: /home/ahrorbek/projects/ielts-today. Name and icon changes apply to all of them.”). The Agents and Usage pages pick their machine with a dropdown in the top right (“This Mac”), its menu listing each machine with a check on the one shown.',
  nowImg: '../feedback/evidence/15-settings-projects-per-machine.png',
  issues: [
    'To see another machine’s folder or checkouts you go back to the sidebar and find the other row.',
    '“Combined with” is plain text, so you can’t go to the copy it names.',
  ],
  options: [
    { key: 'A', name: 'A dropdown in the top right, as on Usage', from: 'agentZ’s Usage and Agents pages',
      desc: 'The machine dropdown from Usage, beside the page’s title. Its menu lists each copy’s machine with its icon, and a check on the one shown. The page opens on the machine you came from (a thread’s Project Settings), else This Mac. Two copies on one machine are told apart by folder, as New Thread’s machine menu does (“This Mac · ~/projects/agentZ-2”). A project on one machine shows no dropdown.',
      good: 'Matches Usage and Agents, so it’s one control to learn.', cost: 'The other machines are hidden until you open the menu.',
      mock: () => settingsWindow(laptopTop({ right: `<span style="position:relative">${dd(`${machineIc('laptop')}Ahrorbek’s Laptop`)}${machineMenu(IELTS, 'Ahrorbek’s Laptop', 'top:30px;right:0;min-width:220px')}</span>` }), { projects: listB() }) },
    { key: 'B', name: 'A segmented machine selector', from: 'new',
      desc: 'Every machine’s name, with its icon, in a segmented control under the title, the chosen one highlighted. One click switches. With many copies the names would wrap, so above four it falls back to A’s dropdown.',
      good: 'All machines in view, and switching takes one click.', cost: 'A second kind of machine picker beside Usage’s dropdown, and three long names take a lot of width.',
      mock: () => settingsWindow(page('fluency.uz', [projectSection([nameRow(PROJECTS[2]), iconRow(PROJECTS[2]), monogramRow(PROJECTS[2])]), setSection('Devbox 1', [folderRow('devbox', PROJECTS[2]), groupingRow()])], { sub: `<div style="margin-top:10px">${segOf(PROJECTS[2], 'devbox')}</div>` }), { projects: PROJECTS.map((project) => navItem(project.name, projectIcon(project), { on: project === PROJECTS[2], end: otherMachines(project).length ? machineIc(otherMachines(project)[0][0]) : '' })).join('') }) },
    { key: 'C', name: 'Buttons with a “+N” menu, as Zed’s settings files', from: 'Zed',
      desc: 'Zed’s row of settings files under the title: This Mac’s button always, then the chosen machine’s, then “+1” with a menu of the rest. The chosen one is tinted with the accent color.',
      good: 'Zed’s exact control for the same question (which copy am I editing).', cost: 'Hides some machines behind “+N”, and looks unlike the rest of agentZ’s settings.',
      mock: () => settingsWindow(page('fluency.uz', [projectSection([nameRow(PROJECTS[2]), iconRow(PROJECTS[2]), monogramRow(PROJECTS[2])]), setSection('Devbox 1', [folderRow('devbox', PROJECTS[2]), groupingRow()])], { sub: `<div class="row g1" style="margin-top:10px;position:relative">${zedTab(`${machineIc('mac', 'sm')}This Mac`, false)}${zedTab(`${machineIc('devbox', 'sm')}Devbox 1`, true)}<span style="position:relative">${zedTab(`+1${ic('chev-down', 'xs mu')}`, false)}${menuOf([['Ahrorbek’s Laptop', machineIc('laptop')]], '', 'top:28px;left:0;min-width:200px')}</span></div>` }), { projects: PROJECTS.map((project) => navItem(project.name, projectIcon(project), { on: project === PROJECTS[2], end: otherMachines(project).length ? machineIc(otherMachines(project)[0][0]) : '' })).join('') }) },
    { key: 'D', name: 'No picker: every copy on the page', from: 't3code',
      desc: 'As t3code’s project settings: a “Machines” section has a row per copy (its machine, folder and a Remove button), and Checkouts lists every copy’s worktrees and pastures with their machine’s name. Nothing to switch.',
      good: 'Everything about the project is on one page.', cost: 'The page gets long with many copies and checkouts, and every per-copy row has to name its machine.',
      mock: () => settingsWindow(page('ielts-today', [projectSection([nameRow(), iconRow(), monogramRow()]), setSection('Machines', IELTS.copies.map(([machine]) => setRow(`<span class="row g15">${machineIc(machine)}${machineName(machine)}</span>`, copyPath(IELTS, machine), outlined('Remove…')))), setSection('Checkouts', [checkoutRow('agentz/brave-otter-3fa', 'Worktree', '~/projects/ielts-today-worktrees/brave-otter', 1, 'Ahrorbek’s Laptop · ')])]), { projects: listB(), h: 720 }) },
    { key: 'E', name: 'Machines in the settings sidebar', from: 'new',
      desc: 'Goes with “How the sidebar lists a combined project” option D: the copies are rows under the project in the settings sidebar, and the page’s title names the chosen one (“ielts-today” over “Ahrorbek’s Laptop”).',
      good: 'No control on the page at all.', cost: 'Only works with that sidebar, which grows as you open projects.',
      mock: () => settingsWindow(laptopTop({ sub: `<span class="sm mu row g15">${machineIc('laptop', 'xs mu')}Ahrorbek’s Laptop</span>` }), { projects: PROJECTS.map((project) => navItem(project.name, projectIcon(project)) + (project === IELTS ? project.copies.map(([machine]) => navItem(machineName(machine), machineIc(machine), { on: machine === 'laptop', indent: 20 })).join('') : '')).join('') }) },
  ],
});

// 3. Shared settings and the copy's own --------------------------------------------------
const pickerA = (machine = 'laptop') => dd(`${machineIc(machine)}${machineName(machine)}`);
const fullWindow = (content, h = 1180) => settingsWindow(content, { projects: listB(), h });
TOPICS.push({
  id: 'split', section: 'Project page', title: 'Shared settings and the copy’s own', size: 'wide', rec: 'A',
  now: 'One list for one copy: Project (Name, Icon, Monogram, Folder), Repository (Repository, Grouping, Combined with), Checkouts and Danger. Name, Icon and Monogram already change every copy (an icon file only This Mac’s copies, since it’s a file on this Mac). Folder, Grouping, Checkouts and Remove change only the copy whose page it is. Only “Combined with” says which is which.',
  nowImg: '../feedback/evidence/15-settings-projects-per-machine.png',
  issues: [
    'Folder sits in the same section as Name and Icon, though it belongs to one machine and they belong to all.',
    'Grouping looks like a project setting but is saved for this copy only: “Keep separate” takes just this copy out.',
    'The only hint that a change reaches other machines is a sentence at the end of “Combined with”.',
  ],
  options: [
    { key: 'A', name: 'Sections named after the machine', from: 'new',
      desc: 'Project (Name, Icon, Monogram) and Repository come first and apply to every copy. Then a section titled with the chosen machine, “Ahrorbek’s Laptop”, holds Folder and Grouping, followed by its Checkouts and Danger. “Combined with” goes away, since the machine picker lists the copies. The picker is the one chosen in “Choosing the machine”.',
      good: 'Section titles say which machine a row belongs to, and nothing new is added to the rows.', cost: 'The picker at the top is far from the sections it changes.',
      mock: () => fullWindow(page('ielts-today', [projectSection([nameRow(), iconRow(), monogramRow()]), repositorySection([repositoryRow()]), setSection('Ahrorbek’s Laptop', [folderRow('laptop'), groupingRow()]), checkoutsSection('laptop'), dangerSection([removeRow('Remove from Ahrorbek’s Laptop', 'Removes this copy and its threads from agentZ. Other machines keep theirs. Files on disk are not touched.', 'Remove…')])], { right: pickerA() }), 940) },
    { key: 'B', name: 'Today’s sections, with a note under each', from: 'agentZ’s section notes',
      desc: 'Today’s order, with Folder moved under Repository. A muted note under the shared sections says “Applies on This Mac and Ahrorbek’s Laptop.” and one under each copy’s section says “Only on Ahrorbek’s Laptop.”, as settings sections already carry notes.',
      good: 'The smallest change to today’s page.', cost: 'Notes are easy to skip, and they repeat on every section.',
      mock: () => fullWindow(page('ielts-today', [setSection('Project', [nameRow(), iconRow(), monogramRow()], { note: 'Applies on This Mac and Ahrorbek’s Laptop.' }), setSection('Repository', [repositoryRow(), folderRow('laptop'), groupingRow()], { note: 'Folder and Grouping: only on Ahrorbek’s Laptop.' }), checkoutsSection('laptop', 'Checkouts', { note: 'Only on Ahrorbek’s Laptop.' }), dangerSection([removeRow('Remove from Ahrorbek’s Laptop', 'Removes this copy and its threads from agentZ. Other machines keep theirs. Files on disk are not touched.', 'Remove…')])], { right: pickerA() }), 990) },
    { key: 'C', name: 'The picker between the two parts', from: 't3code’s scope sentence',
      desc: 'The shared sections first, with no picker above them. Then a line, “On [Ahrorbek’s Laptop ▾]”, as t3code’s “Applying settings for … on …” sentence, and under it the copy’s sections. The picker sits right above what it changes. This replaces the top-right place in “Choosing the machine” A.',
      good: 'Clear which rows follow the machine and which don’t.', cost: 'The picker is mid-page, unlike Usage and Agents, and scrolls out of view.',
      mock: () => fullWindow(page('ielts-today', [projectSection([nameRow(), iconRow(), monogramRow()]), repositorySection([repositoryRow()]), `<div class="row g2" style="border-top:1px solid var(--bv);padding-top:20px"><span class="mu">On</span>${pickerA()}</div>`, setSection('This copy', [folderRow('laptop'), groupingRow()]), checkoutsSection('laptop'), dangerSection([removeRow('Remove from Ahrorbek’s Laptop', 'Removes this copy and its threads from agentZ. Other machines keep theirs. Files on disk are not touched.', 'Remove…')])]), 1010) },
    { key: 'D', name: 'A machine label on each copy’s row', from: 'new',
      desc: 'Today’s sections, with Folder moved under Repository. Each row that belongs to one copy shows a small chip with the machine’s icon and name beside its title, and section titles get the same chip.',
      good: 'You see it on the row itself, wherever you scroll.', cost: 'Chips everywhere, and they all say the same machine.',
      mock: () => {
        const chip = `<span class="chip" style="height:18px;margin-left:8px;font-size:11px">${machineIc('laptop', 'xs mu')}Ahrorbek’s Laptop</span>`;
        return fullWindow(page('ielts-today', [projectSection([nameRow(), iconRow(), monogramRow()]), repositorySection([repositoryRow(), setRow(`<span class="row">Folder${chip}</span>`, copyPath(IELTS, 'laptop')), setRow(`<span class="row">Grouping${chip}</span>`, 'Projects from the same repository share one row.', dd('Default'))]), setSection(`Checkouts${chip}`, CHECKOUTS.laptop()), dangerSection([removeRow(`<span class="row">Remove copy${chip}</span>`, 'Removes this copy and its threads from agentZ. Other machines keep theirs. Files on disk are not touched.', 'Remove…')])], { right: pickerA() }), 910);
      } },
  ],
});

// 4. Remove ---------------------------------------------------------------------------------
const alertBox = (title, body, buttons) => `<div class="modal-back"></div><div style="position:absolute;left:50%;top:50%;transform:translate(-50%,-50%);z-index:20;width:260px;border-radius:12px;padding:18px 16px 16px;background:#3a3d44;border:1px solid rgba(255,255,255,.12);box-shadow:0 18px 40px rgba(0,0,0,.5);font:13px/1.35 -apple-system,'IBM Plex Sans',sans-serif;text-align:center;display:flex;flex-direction:column;gap:6px;align-items:center">
  <span style="width:44px;height:44px;border-radius:10px;background:linear-gradient(135deg,#3d4a5c,#232830);display:grid;place-items:center;color:var(--ac);font-weight:700;font-size:18px">Z</span>
  <b style="color:#f2f3f5">${title}</b><span style="color:#dfe2e7;font-size:12px">${body}</span>
  <div class="col" style="gap:6px;width:100%;margin-top:8px">${buttons.map(([label, kind]) => `<span style="height:24px;border-radius:6px;display:grid;place-items:center;${kind === 'default' ? 'background:#3b82f6;color:#fff' : kind === 'danger' ? 'background:#55595f;color:#ff6b6b' : 'background:#55595f;color:#f2f3f5'}">${label}</span>`).join('')}</div></div>`;
const removeHere = () => removeRow('Remove from Ahrorbek’s Laptop', 'Removes this copy and its threads from agentZ. This Mac keeps its copy. Files on disk are not touched.', 'Remove…');
const dangerPiece = (rows, extra = '', h) => piece(`<div style="padding:24px 32px">${dangerSection(rows)}</div>${extra}`, 720, h);
TOPICS.push({
  id: 'remove', section: 'Project page', title: 'What Remove does for a combined project', size: 'wide', rec: 'B',
  now: 'Danger has one row, “Remove project”: “Removes the project and its threads from agentZ. Files on disk are not touched.” Its Remove Project button asks “Remove “ielts-today” from agentZ?” (“Its threads are removed too. Nothing on disk is touched.”) and then removes only the copy whose page is open. The other machines keep theirs, though nothing says so.',
  nowImg: 'img/now-repository.png',
  issues: [
    '“Remove project” on a combined project removes one machine’s copy, while its wording says the whole project.',
    'Removing a project from every machine takes one visit per copy.',
  ],
  options: [
    { key: 'A', name: 'Only the chosen machine’s copy', from: 't3code',
      desc: 'Today’s behavior, named for what it does: “Remove from Ahrorbek’s Laptop”, as t3code’s “Remove checkout”. Other machines keep their copies. After it, the page shows the next copy. A project on one machine keeps today’s “Remove Project”.',
      good: 'One button, and it says exactly what it removes.', cost: 'Removing it everywhere is still one copy at a time.',
      mock: () => dangerPiece([removeHere()]) },
    { key: 'B', name: 'This copy, or every copy', from: 't3code',
      desc: 'Two rows: “Remove from Ahrorbek’s Laptop” (A’s) and “Remove from all machines”, which removes the copies on This Mac and Ahrorbek’s Laptop and all their threads, as t3code’s “Remove this project everywhere”. Each asks first, naming what goes.',
      good: 'Both cases in one place, each saying what it removes.', cost: 'Two red buttons in Danger.',
      mock: () => dangerPiece([removeHere(), removeRow('Remove from all machines', 'Removes the copies on This Mac and Ahrorbek’s Laptop, and all their threads. Files on disk are not touched.', 'Remove All…')]) },
    { key: 'C', name: 'One button that asks which', from: 'new',
      desc: 'Today’s single Remove Project button. Its dialog asks: “Remove from Ahrorbek’s Laptop”, “Remove from All Machines” or Cancel.',
      good: 'Danger stays one row, as today.', cost: 'You find out it’s per machine only in the dialog, and three-button dialogs are easy to misclick.',
      mock: () => dangerPiece([removeRow('Remove project', 'Removes the project and its threads from agentZ. Files on disk are not touched.', 'Remove Project…')],
        alertBox('Remove “ielts-today” from agentZ?', 'It’s on This Mac and Ahrorbek’s Laptop. Its threads are removed too. Nothing on disk is touched.', [['Remove from Ahrorbek’s Laptop', 'danger'], ['Remove from All Machines', 'danger'], ['Cancel', 'default']]), 360) },
    { key: 'D', name: 'Every copy only', from: 'new',
      desc: 'Danger’s Remove Project removes every copy. One machine’s copy is removed from its row in the Machines section, as t3code’s Checkouts rows each have a Remove button. Goes with “Choosing the machine” option D.',
      good: 'Remove Project does what it says.', cost: 'Needs D’s Machines section; with a picker there’s no place for one copy’s Remove.',
      mock: () => piece(`<div class="col" style="padding:24px 32px;gap:24px">${setSection('Machines', IELTS.copies.map(([machine]) => setRow(`<span class="row g15">${machineIc(machine)}${machineName(machine)}</span>`, copyPath(IELTS, machine), outlined('Remove…'))))}${dangerSection([removeRow('Remove project', 'Removes it from This Mac and Ahrorbek’s Laptop, with all its threads. Files on disk are not touched.', 'Remove Project…')])}</div>`) },
  ],
});
