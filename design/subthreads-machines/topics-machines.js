// The machine picker under a new thread's composer: how its menu looks, and what happens with a
// machine that doesn't have the selected agent. agent_view.rs render_machine_picker,
// project_copies.rs render_copy_row.

const MENU_POS = 'top:236px;left:118px';
const machineRowToday = (copy, { current = false, reason = '', hl = false } = {}) => `<div class="it ${hl ? 'hl' : ''}" style="height:auto;padding:5px 8px;gap:10px;align-items:center">${ic(copy.icon, 'sm')}<div class="col grow" style="min-width:0;gap:1px"><div class="row g2"><span style="${reason ? 'color:var(--ph)' : ''}">${copy.name}</span><span class="xs ph trunc">${copy.path}</span></div><div class="xs" style="color:${reason ? 'var(--ph)' : 'var(--mu)'}">${reason || `<span style="color:var(--t)">${copy.branch}</span> · ${rUpstream(copy)} · ${rChanges(copy)}`}</div></div><span style="width:14px;display:inline-flex">${current ? ic('check', 'sm ac') : ''}</span></div>`;
const footerToday = `<div class="hr"></div><div class="xs mu" style="padding:4px 8px 3px;max-width:330px">As of each machine’s last fetch: This Mac 9m ago, agents-1 never</div>`;

// One line per machine, with what differs as short marks, and the rest in Zed's aside.
const marks = (copy) => {
  if (copy.offline) return '<span class="xs ph none">Offline</span>';
  const parts = [];
  if (copy.behind) parts.push(`↓${copy.behind}`);
  if (copy.ahead) parts.push(`↑${copy.ahead}`);
  if (copy.changes) parts.push(`${copy.changes[0]} files`);
  return parts.length ? `<span class="xs warnc none">${parts.join(' · ')}</span>` : '';
};
const lineRow = (copy, { current = false, hl = false, reason = '', end } = {}) => `<div class="it ${hl ? 'hl' : ''} ${copy.offline || reason ? 'dis' : ''}" style="height:28px;gap:8px">${ic(copy.icon, 'sm')}<span class="none">${copy.name}</span><span class="grow"></span>${end ?? (reason ? `<span class="xs ph none">${reason}</span>` : marks(copy))}<span style="width:14px;display:inline-flex">${current ? ic('check', 'sm ac') : ''}</span></div>`;
const aside = (copy, style) => `<div class="pop" style="${style};width:270px;padding:10px 12px;font-size:12px;display:flex;flex-direction:column;gap:5px;line-height:1.4">
  <span class="row g2"><span class="b5" style="font-size:13px">${copy.name}</span><span class="ph trunc">${copy.path}</span></span>
  <span class="row g1">${ic('branch', 'xs mu')}<span>${copy.branch}</span> ${rUpstream(copy)}</span>
  <span class="row g1">${ic('diff', 'xs mu')}${rChanges(copy)}</span>
  <span class="ph">${copy.fetched ? `Fetched ${copy.fetched} ago` : 'Never fetched'}</span></div>`;

// Two tidy lines: the name, then the git state, all at the menu's sizes; no footer.
const tidyRow = (copy, { current = false, hl = false, reason = '' } = {}) => `<div class="it ${hl ? 'hl' : ''}" style="height:auto;padding:5px 8px;gap:10px;align-items:flex-start">${ic(copy.icon, 'sm')}<div class="col grow" style="min-width:0;gap:1px"><span style="${copy.offline || reason ? 'color:var(--ph)' : ''}">${copy.name}</span><span class="sm" style="color:${copy.offline || reason ? 'var(--ph)' : 'var(--mu)'}">${copy.offline ? 'Offline' : reason || `${copy.branch} · ${copy.behind ? `<span class="warnc">${copy.behind} behind</span>` : 'up to date'} · ${copy.changes ? `<span class="warnc">${copy.changes[0]} changed files</span>` : 'no changes'} · <span class="faint">${copy.fetched ? `fetched ${copy.fetched} ago` : 'never fetched'}</span>`}</span></div><span style="width:14px;display:inline-flex;margin-top:2px">${current ? ic('check', 'sm ac') : ''}</span></div>`;

// The project switcher's look: one line with the folder, and a warning line where it differs.
const switcherRow = (copy, { current = false, hl = false } = {}) => {
  const differs = [copy.behind ? `${copy.behind} commits behind origin/main` : '', copy.changes ? `${copy.changes[0]} uncommitted files` : ''].filter(Boolean).join(' · ');
  return `<div class="it ${hl ? 'hl' : ''} ${copy.offline ? 'dis' : ''}" style="height:auto;min-height:28px;padding:4px 8px;gap:8px;align-items:flex-start"><span style="margin-top:2px;display:inline-flex">${ic(copy.icon, 'sm')}</span><div class="col grow" style="min-width:0"><div class="row g2" style="height:20px"><span class="none">${copy.name}</span><span class="sm ph trunc">${copy.path}</span>${copy.offline ? '<span class="xs ph none">Offline</span>' : ''}</div>${differs && !copy.offline ? `<div class="row g1 xs warnc">${ic('warning', 'xs')}${differs}</div>` : ''}</div><span style="width:14px;display:inline-flex;margin-top:3px">${current ? ic('check', 'sm ac') : ''}</span></div>`;
};

// A segmented control in the strip, in place of the menu.
const segmented = (selected = 0, hover = -1) => `<span class="row none" style="border:1px solid var(--b);border-radius:6px;overflow:hidden;font-size:12px;height:24px">${R_COPIES.map((copy, i) => `<span class="row g1" style="height:100%;padding:0 9px;${i ? 'border-left:1px solid var(--b);' : ''}${i === selected ? 'background:var(--sel);color:var(--t)' : i === hover ? 'background:var(--hov);color:var(--t)' : `color:${copy.offline ? 'var(--ph)' : 'var(--mu)'}`}">${ic(copy.icon, 'xs')}${copy.name}${!copy.offline && (copy.behind || copy.changes) ? '<span class="dot pending" style="margin-left:2px"></span>' : ''}</span>`).join('')}</span>`;

const claudeComposer = () => rNewComposer({ agent: 'Claude Agent', agentGlyph: 'claude', model: 'Opus 5.5' });

TOPICS.push({
  id: 'machine-menu', section: 'Machine picker', title: 'The machine menu', size: 'wide', rec: 'A',
  now: 'Under a new thread’s composer, “This Mac ⌄” opens a menu of the project’s copies: each machine’s name with its folder in small gray type, then a line like “main · → origin/main up to date · no changes”, and a check on the current one. A machine that’s offline or lacks the selected agent is grayed with the reason. A footer says “As of each machine’s last fetch: This Mac 9m ago, agents-1 never”. The mocks use Claude Agent, which both machines have; Devbox 1 is offline.',
  nowImg: 'img/now-machine-menu.png',
  issues: ['Three type sizes in one row, and the folder in faint gray beside the name.', 'Every row spells out its git state, even when all agree.', 'The footer is a long sentence for something rarely needed.'],
  options: [
    { key: 'A', name: 'One line each, the details beside it', from: 'Zed context menu asides',
      desc: 'A row per machine: its icon, its name, and at the right only what differs from the others, in yellow (“↓2 · 3 files”), or “Offline”. Hovering a row shows Zed’s aside beside the menu: the folder, the branch and its upstream, the changes, and when it was last fetched. No footer.',
      good: 'A short, even menu; the details are there when you hover.', cost: 'The folder and the full git state are a hover away.',
      mock: () => rNewThread(rMenu(R_COPIES.map((copy, i) => lineRow(copy, { current: i === 0, hl: i === 1 })).join(''), MENU_POS, { w: 300 }) + aside(R_COPIES[1], 'top:262px;left:424px'), { composer: claudeComposer(), h: 400 }) },
    { key: 'B', name: 'Two tidy lines', from: 'today, tidied',
      desc: 'Today’s two lines at the menu’s sizes: the name, then “main · 2 behind · 3 changed files · fetched 9m ago” in one muted size, with what differs in yellow. No folder (it’s in the row’s tooltip) and no footer.',
      good: 'Everything still shows at a glance, in plainer words.', cost: 'Still two lines per machine.',
      mock: () => rNewThread(rMenu(R_COPIES.map((copy, i) => tidyRow(copy, { current: i === 0, hl: i === 0 })).join(''), MENU_POS, { w: 360 }), { composer: claudeComposer(), h: 420 }) },
    { key: 'C', name: 'Like the project switcher', from: 'agentZ project switcher (Zed recent projects)',
      desc: 'One line per machine with its folder after the name in the same muted size, as the project switcher lists projects. A copy that differs gets a yellow warning line under it (“2 commits behind origin/main · 3 uncommitted files”); copies that agree are one line.',
      good: 'Matches the switcher, and only differences take a second line.', cost: 'The folder is still on every row.',
      mock: () => rNewThread(rMenu(R_COPIES.map((copy, i) => switcherRow(copy, { current: i === 0, hl: i === 0 })).join(''), MENU_POS, { w: 400 }), { composer: claudeComposer(), h: 400 }) },
    { key: 'D', name: 'A segmented control, no menu', from: 'new',
      desc: 'With up to three machines, the strip shows them side by side as one segmented control; a click picks one. A yellow dot marks a copy that differs, and hovering a machine shows the aside of A.',
      good: 'One click, and you see every machine without opening anything.', cost: 'Takes room in the strip; long machine names don’t fit.',
      mock: () => rNewThread(aside(R_COPIES[1], 'top:222px;left:150px'), { composer: claudeComposer(), strip: rStrip({ middle: `${rChip(ic('folder', 'sm'), 'Local')}${segmented(0, 1)}` }), h: 360 }) },
    { key: 'E', name: 'As it is', from: 'today',
      desc: 'Two lines per machine with the folder, and the fetch footer.',
      good: 'Nothing to change.', cost: 'Busy and uneven.',
      mock: () => rNewThread(rMenu(R_COPIES.map((copy, i) => machineRowToday(copy, { current: i === 0, hl: i === 0, reason: copy.offline ? 'offline' : '' })).join('') + footerToday, MENU_POS, { w: 360 }), { composer: claudeComposer(), h: 420 }) },
  ],
});

// A machine without the selected agent.
const agentMark = (name, kind) => `<span class="row g1 xs mu none">${glyph(kind, 'sm')}Uses ${name}</span>`;
const installMark = () => `<span class="row g1 none"><span class="xs ph">Not installed</span><span class="btn sm" style="height:20px;font-size:11px;padding:0 6px;gap:3px">${ic('download', 'xs')}Install</span></span>`;
const groupedAgents = () => {
  const section = (copy, agents, current) => `<div class="lbl row g1" style="padding-top:6px">${ic(copy.icon, 'xs')}${copy.name}</div>` + agents.map(([name, kind]) => `<div class="it ${current === name ? 'hl' : ''}">${glyph(kind, 'sm')}<span class="grow">${name}</span>${current === name ? ic('check', 'sm ac') : ''}</div>`).join('');
  return rMenu(section(R_COPIES[0], [['Google Antigravity', 'antigravity'], ['Claude Agent', 'claude'], ['Codex', 'codex']], 'Google Antigravity') + section(R_COPIES[1], [['Claude Agent', 'claude'], ['Codex', 'codex'], ['Factory Droid', 'droid']]) + '<div class="hr"></div><div class="it">Terminal</div><div class="it">Manage Agents…</div>', 'top:166px;left:40px', { w: 260 });
};
const agentOnChip = rStrip({ middle: rChip(ic('folder', 'sm'), 'Local') });

TOPICS.push({
  id: 'missing-agent', section: 'Machine picker', title: 'A machine without the selected agent', size: 'wide', rec: 'A', 
  now: 'The menu checks only the draft’s agent: a machine where it isn’t installed is grayed (“Google Antigravity isn’t installed”) and can’t be picked. The agent menu lists only the current machine’s agents. So with Antigravity on This Mac and Claude Agent on agents-1, you must first switch to an agent both have, then switch machines. The mocks use the menu A from the topic before.',
  nowImg: 'img/now-machine-menu.png',
  options: [
    { key: 'A', name: 'Pick it, and take an agent it has', from: 'agentZ’s default agent for a machine',
      desc: 'Every online machine can be picked. One without the draft’s agent says which agent it would use instead (“Uses Claude Agent”): the agent of that machine’s newest thread, else its first installed one, as a new thread there would start with. Picking it moves the draft there with that agent, keeping what you typed; the agent menu then lists that machine’s agents.',
      good: 'Two agents on two machines is one click, and the row says what you’ll get.', cost: 'Picking a machine can change the agent too.',
      mock: () => `<div style="display:flex;flex-direction:column;gap:10px">${rNewThread(rMenu(R_COPIES.map((copy, i) => lineRow(copy, { current: i === 0, hl: i === 1, end: i === 1 ? agentMark('Claude Agent', 'claude') : undefined })).join(''), MENU_POS, { w: 300 }))}${rNewThread('', { composer: claudeComposer(), strip: rStrip({ machine: 'agents-1', machineIcon: 'cloud', open: false }), h: 230 })}</div>` },
    { key: 'B', name: 'Pick the agent and machine together', from: 'new',
      desc: 'The agent menu lists every machine’s agents, under each machine’s name. Picking an agent on another machine moves the draft there. The machine menu stays for the copies, and a machine without the agent is still grayed there.',
      good: 'You pick “Claude Agent on agents-1” in one place.', cost: 'A longer agent menu, with the same agent listed once per machine.',
      mock: () => rNewThread(groupedAgents(), { strip: rStrip({ open: false }), h: 470 }) },
    { key: 'C', name: 'Install it there', from: 'agentZ Settings › Agents install',
      desc: 'The grayed row offers Install, which installs the agent on that machine from the ACP Registry, as Settings › Agents does. Once it’s installed, the row can be picked.',
      good: 'Keeps your agent, and fixes the gap for next time.', cost: 'Installing takes a while, and some agents then need a login there.',
      mock: () => rNewThread(rMenu(R_COPIES.map((copy, i) => lineRow(copy, { current: i === 0, hl: i === 1, reason: i === 1 ? ' ' : '', end: i === 1 ? installMark() : undefined })).join(''), MENU_POS, { w: 320 })) },
    { key: 'D', name: 'As it is', from: 'today',
      desc: 'Grayed, with “Google Antigravity isn’t installed”.',
      good: 'Nothing changes behind your back.', cost: 'Two steps, through an agent both machines have.',
      mock: () => rNewThread(rMenu(R_COPIES.map((copy, i) => lineRow(copy, { current: i === 0, reason: i === 1 ? 'Google Antigravity isn’t installed' : '' })).join(''), MENU_POS, { w: 360 })) },
  ],
});
