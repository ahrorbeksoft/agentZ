// Project copies: the demo world and the pieces this round's mocks share. Loaded after
// board/mock.js; nothing here changes the board's own helpers.

ICONS.updown = '<path d="m7 15 5 5 5-5"/><path d="m7 9 5-5 5 5"/>';
ICONS.refresh = '<path d="M3 12a9 9 0 0 1 9-9 9.75 9.75 0 0 1 6.74 2.74L21 8"/><path d="M21 3v5h-5"/><path d="M21 12a9 9 0 0 1-9 9 9.75 9.75 0 0 1-6.74-2.74L3 16"/><path d="M8 16H3v5"/>';
ICONS.commit = '<circle cx="12" cy="12" r="3"/><line x1="3" x2="9" y1="12" y2="12"/><line x1="15" x2="21" y1="12" y2="12"/>';
ICONS['list-tree'] = '<path d="M21 12h-8"/><path d="M21 6H8"/><path d="M21 18h-8"/><path d="M3 6v4c0 1.1.9 2 2 2h3"/><path d="M3 10v6c0 1.1.9 2 2 2h3"/>';
ICONS.archive = '<rect width="20" height="5" x="2" y="3" rx="1"/><path d="M4 8v11a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8"/><path d="M10 12h4"/>';
ICONS.cloud = '<path d="M17.5 19H9a7 7 0 1 1 6.71-9h1.79a4.5 4.5 0 1 1 0 9Z"/>';

// The user's machines and projects, as in their screenshot of the project switcher.
const MACHINES = { mac: ['This Mac', 'laptop'], devbox: ['Devbox 1', 'server'], laptop: ['Ahrorbek’s Laptop', 'laptop'] };
const machineName = (machine) => MACHINES[machine][0];
const machineIc = (machine, cls = 'sm mu') => ic(MACHINES[machine][1], cls);

// Each copy's git state, as its machine's server would read it. `upstream` is what the branch
// tracks; ahead and behind are against it, as last fetched on that machine.
const PROJECTS = [
  { id: 'az', name: 'agentZ', label: 'agentZ', mono: 'AZ', bg: '#4a3533', fg: '#e8836f', copies: [
    { machine: 'mac', path: '~/projects/agentZ', branch: 'main', upstream: 'origin/main', ahead: 0, behind: 0, commit: '6579db5', subject: 'Describe tool output shown as printed', ago: '12m', changes: { files: 3, added: 42, removed: 7 }, stash: 0 },
    { machine: 'devbox', path: '/root/projects/agentZ', branch: 'main', upstream: 'origin/main', ahead: 0, behind: 1, commit: 'a936e5c', subject: 'Give every account a color', ago: '2h', changes: null, stash: 0 },
  ] },
  { id: 'it', name: 'ielts-today', label: 'ahrorbeksoft/ielts-today', mono: 'IT', bg: '#3b3a5c', fg: '#a7a2f0', copies: [
    { machine: 'mac', path: '~/projects/svelte5/ielts-today', branch: 'main', upstream: 'origin/main', ahead: 0, behind: 0, commit: '1c2d3e4', subject: 'Fix the listening timer', ago: '1d', changes: null, stash: 0 },
    { machine: 'laptop', path: '/home/ahrorbek/projects/ielts-today', branch: 'main', upstream: 'origin/main', ahead: 0, behind: 0, commit: '1c2d3e4', subject: 'Fix the listening timer', ago: '1d', changes: null, stash: 0 },
  ] },
  { id: 'fu', name: 'fluency.uz', label: 'ahrorbeksoft/fluency.uz', mono: 'FU', bg: '#4d3238', fg: '#e57887', copies: [
    { machine: 'mac', path: '~/projects/fluency.uz', branch: 'main', upstream: 'origin/main', ahead: 0, behind: 0, commit: '9f8e7d6', subject: 'Add the lesson player', ago: '3h', changes: null, stash: 0 },
    { machine: 'devbox', path: '/root/projects/fluency.uz', branch: 'payments', upstream: null, ahead: 3, behind: 0, commit: '77ab12c', subject: 'Charge cards with Payme', ago: '40m', changes: { files: 1, added: 8, removed: 2 }, stash: 0 },
    { machine: 'laptop', path: '/home/ahrorbek/projects/fluency.uz', branch: 'main', upstream: 'origin/main', ahead: 0, behind: 12, commit: '3a4b5c6', subject: 'Translate the home page', ago: '9d', changes: null, stash: 1, extraRemote: 'upstream' },
  ] },
];
const AZ = PROJECTS[0];
const IT = PROJECTS[1];
const FU = PROJECTS[2];
const projectIcon = (project, size = 16) => `<span style="width:${size}px;height:${size}px;border-radius:${size > 16 ? 6 : 4}px;flex:none;display:inline-grid;place-items:center;background:${project.bg};color:${project.fg};font-size:${Math.round(size * 0.45)}px;font-weight:700;letter-spacing:-.2px">${project.mono}</span>`;
const copyOf = (project, machine) => project.copies.find((copy) => copy.machine === machine);
const machinesText = (project) => project.copies.map((copy) => machineName(copy.machine)).join(', ');

// Whether a project's copies agree, and the differences in plain words. The first copy is the
// one compared against (This Mac's).
function differences(project) {
  const [first, ...rest] = project.copies;
  const lines = [];
  for (const copy of project.copies) {
    const name = machineName(copy.machine);
    if (copy !== first && copy.branch !== first.branch) lines.push(`${name} is on ${B2(copy.branch)}, not ${B2(first.branch)}`);
    else if (copy !== first && copy.commit !== first.commit && copy.behind) lines.push(`${name} is ${copy.behind} ${copy.behind === 1 ? 'commit' : 'commits'} behind ${machineName(first.machine)}`);
    if (!copy.upstream && copy.ahead) lines.push(`${name} has ${copy.ahead} commits on ${B2(copy.branch)} that aren’t pushed`);
    if (copy.changes) lines.push(`${name} has ${copy.changes.files} uncommitted ${copy.changes.files === 1 ? 'file' : 'files'} ${lines2(copy.changes)}`);
    if (copy.stash) lines.push(`${name} has ${copy.stash} stash`);
    if (copy.extraRemote) lines.push(`${name} also has the remote ${B2(copy.extraRemote)}`);
  }
  return lines;
}
const inSync = (project) => differences(project).length === 0;
const B2 = (text) => `<span style="color:var(--t)">${text}</span>`;
const lines2 = (changes) => `<span class="okc">+${changes.added}</span> <span class="delc">−${changes.removed}</span>`;

// One copy's git state on one line, as the Workspaces view's git popover writes it.
function upstreamText(copy) {
  if (!copy.upstream) return `<span class="mu">no upstream</span>${copy.ahead ? ` <span class="okc">↑${copy.ahead}</span>` : ''}`;
  const marks = `${copy.ahead ? ` <span class="okc">↑${copy.ahead}</span>` : ''}${copy.behind ? ` <span class="delc">↓${copy.behind}</span>` : ''}`;
  return `<span class="mu">→ ${copy.upstream}</span>${marks || ' <span class="mu">up to date</span>'}`;
}
const changesText = (copy) => (copy.changes ? `${copy.changes.files} ${copy.changes.files === 1 ? 'file' : 'files'} ${lines2(copy.changes)}` : '<span class="mu">no changes</span>');

// The title bar's project switcher trigger.
const switcherTrigger = (project, extra = '') => `<span class="row g15" style="font-size:13px;position:relative">${projectIcon(project, 14)}${project.label === project.name ? project.name : project.label}<span class="mu">${machinesText(project)}</span>${extra}${ic('chev-down', 'xs mu')}</span>`;
function titleBar(content, { w = 900 } = {}) {
  return `<div class="titlebar" style="width:${w}px">${lights()}${ic('sidebar', 'sm mu')}${content}<div class="viewtabs"><span class="on">Agents</span><span>Workspaces</span></div></div>`;
}

// The project switcher (project_switcher.rs, Zed's recent projects): search, All projects, a
// section per machine or "On several machines", then Add Project….
function switcherRow(project, { selected = false, current = false, machines = 'names', end = '', after = '' } = {}) {
  const label = machines === 'names' ? `<span class="sm mu trunc">${machinesText(project)}</span>` : machines === 'icons' ? `<span class="row" style="gap:2px">${project.copies.map((copy) => machineIc(copy.machine, 'xs mu')).join('')}</span>` : machines;
  return `<div class="row g2" style="height:32px;padding:0 8px;border-radius:6px;margin:0 4px;position:relative;${selected ? 'background:var(--hov)' : ''}">${projectIcon(project)}<span class="row g15 grow" style="min-width:0"><span class="none">${project.label}</span>${label}${current ? `<span class="dot done"></span>${ic('check', 'sm ac')}` : ''}${after}</span>${end}${ic('settings', 'sm mu')}</div>`;
}
function switcher(rows, { w = 352, style = 'left:8px;top:40px' } = {}) {
  return `<div class="pop" style="position:absolute;${style};width:${w}px;font-size:13px;z-index:20">
    <div class="row g2" style="padding:8px 12px;border-bottom:1px solid var(--bv)">${ic('search', 'sm mu')}<span class="ph">Search projects…</span></div>
    <div style="padding:4px 0">
      <div class="row g2" style="height:32px;padding:0 8px;margin:0 4px">${ic('list-tree', 'sm mu')}All projects<span class="mu">3 projects</span></div>
      <div style="height:1px;background:var(--bv);margin:4px 0"></div>
      <div class="xs ph" style="padding:4px 12px">On several machines</div>
      ${rows}
    </div>
    <div class="row g2" style="padding:6px 12px;border-top:1px solid var(--bv)">${ic('folder-open', 'sm mu')}<span class="grow">Add Project…</span><span class="xs ph">Ctrl-O</span></div>
  </div>`;
}
const switcherRows = (options = () => ({})) => PROJECTS.map((project, index) => switcherRow(project, { selected: index === 0, current: index === 0, ...options(project) })).join('');
const tip = (html, style) => `<div class="pop" style="position:absolute;${style};padding:6px 9px;font-size:12px;z-index:25;line-height:1.45">${html}</div>`;

// Settings rows and sections as settings_page.rs draws them.
const dd = (label) => `<span class="row" style="gap:6px;height:24px;padding:0 6px 0 8px;border-radius:6px;border:1px solid var(--b);background:var(--hov);white-space:nowrap;font-size:13px">${label}${ic('updown', 'xs mu')}</span>`;
const outlined = (label) => `<span class="btn" style="background:none">${label}</span>`;
const setRow = (title, desc, control = '') => `<div class="row" style="padding:12px 16px;gap:24px;position:relative"><div class="col grow" style="gap:2px;min-width:0"><span>${title}</span>${desc ? `<span class="sm mu" style="overflow-wrap:anywhere">${desc}</span>` : ''}</div>${control ? `<div class="none" style="position:relative">${control}</div>` : ''}</div>`;
const setSection = (title, rows, { right = '', note = '' } = {}) => `<div class="col" style="gap:8px"><div class="row" style="justify-content:space-between;min-height:16px"><span class="sm mu row g15">${title}</span>${right}</div><div class="col" style="border:1px solid var(--b);border-radius:8px;background:var(--panel)">${rows.map((row, index) => `<div style="${index < rows.length - 1 ? 'border-bottom:1px solid var(--bv)' : ''}">${row}</div>`).join('')}</div>${note ? `<span class="sm mu">${note}</span>` : ''}</div>`;
const settingsPage = (heading, sections, { right = '', w = 720 } = {}) => `<div class="m" style="width:${w}px"><div class="col" style="padding:24px 32px 32px;gap:24px"><div class="row" style="justify-content:space-between;gap:12px;min-height:26px;position:relative"><span style="font-size:17px">${heading}</span>${right}</div>${sections.join('')}</div></div>`;
const machineDropdown = (machine = 'mac') => dd(`${machineIc(machine)}${machineName(machine)}`);

// A sidebar thread card with all projects shown (sidebar.rs render_thread_card).
function card({ project = IT, name, machine = 'mac', title = 'Fix the listening timer', state = 'done', branch = 'main', active = false } = {}) {
  return `<div style="margin:2px 4px;padding:8px 10px;border-radius:6px;height:82px;${active ? 'background:var(--sel)' : ''}">
    <div class="row g15" style="height:20px">${projectIcon(project, 14)}<span class="sm mu grow trunc">${name || project.name}</span>${pill(state)}</div>
    <div class="trunc" style="margin-top:4px">${title}</div>
    <div class="row g15 xs faint" style="margin-top:4px">${ic('branch', 'xs')}<span class="grow trunc">${branch}</span><span style="opacity:.7;display:inline-flex">${machineIc(machine, 'xs')}</span>${glyph('claude', 'sm')}</div>
  </div>`;
}
