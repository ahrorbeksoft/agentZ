// What each subthread role runs on: the four roles agents delegate by (researcher, reviewer,
// worker, other), each with the user's ordered picks of agent, model and effort, and the account
// a pick's agent would use, chosen for the user by usage to spare.

const ROLE_INFO = {
  researcher: { name: 'Researcher', desc: 'Finds things out.' },
  reviewer: { name: 'Reviewer', desc: 'Checks work.' },
  worker: { name: 'Worker', desc: 'Does the work.' },
  other: { name: 'Other', desc: 'Anything else, and tasks given no role.' },
};
const ROLE_PICKS = {
  researcher: [{ agent: 'codex', model: 'GPT-5.5', effort: 'High' }, { agent: 'claude', model: 'Sonnet 5.5', effort: 'Medium' }],
  reviewer: [{ agent: 'claude', model: 'Opus 5.5', effort: 'High' }],
  worker: [{ agent: 'claude', model: 'Sonnet 5.5', effort: 'Medium' }, { agent: 'droid', model: 'GPT-5.5', effort: 'Default' }],
  other: [],
};
// Which account each agent would run a task on now, on This Mac. Claude Agent's Work is used
// up, so Personal; Droid's one login is used up, so its picks are skipped.
const ROLE_NOW = {
  codex: { account: S2_ACCOUNTS.codex, text: 'Personal · 81% left' },
  claude: { account: S2_ACCOUNTS.personal, text: 'Personal · 62% left' },
  droid: { skip: 'Used up until 4:10 PM' },
};
const ROLE_EMPTY = 'None: the agent that delegates chooses.';
const roleAgent = (kind) => `<span class="row g15 none" style="width:142px">${glyph(kind)}<span class="trunc">${S2_AGENTS[kind].name}</span></span>`;
const roleNowLine = (kind) => {
  const now = ROLE_NOW[kind];
  if (now.skip) return `<span class="row g1 xs none" style="color:var(--warn)">${ic('alert', 'xs')}${now.skip}</span>`;
  return `<span class="row g1 xs mu none">${s2Avatar(now.account, 12)}${now.text}</span>`;
};
/** One pick in a role's list: grip, place, agent, model and effort menus, and remove. */
const rolePickRow = (pick, place, { now = false, modelMenu = '', after = '' } = {}) => `<div class="row g2" style="height:40px;padding:0 12px 0 10px;position:relative;${modelMenu ? 'z-index:4' : ''}">${ic('grip', 'sm faint')}<span class="xs mu" style="width:10px;text-align:center">${place}</span>${roleAgent(pick.agent)}<span style="position:relative">${s2Dd(pick.model, { open: !!modelMenu, menu: modelMenu })}</span>${s2Dd(pick.effort)}<span class="grow"></span>${now ? roleNowLine(pick.agent) : ''}${after}<span class="ibtn sm">${ic('x', 'xs mu')}</span></div>`;
const roleAdd = (menu = '') => `<div class="row g1 sm mu" style="height:32px;padding:0 14px;position:relative;${menu ? 'z-index:5' : ''}">${ic('plus', 'xs')}Add agent${menu}</div>`;
const roleAgentMenu = (style = 'top:28px;left:8px') => s2Menu([
  ['lbl', 'Installed on a machine'],
  ...['claude', 'codex', 'droid', 'antigravity'].map((kind) => [glyph(kind), S2_AGENTS[kind].name]),
], style, 220);
const roleModelMenu = s2Menu([
  ['', 'The account’s default'],
  'hr',
  ['', 'Opus 5.5'],
  ['', 'Sonnet 5.5', { check: true, hl: true }],
], 'top:30px;left:0', 200);
/** A role, its picks under it, and Add agent: one row of the Roles section. */
const roleBlock = (role, { now = false, add = '', modelMenuAt = -1, title = true } = {}) => {
  const info = ROLE_INFO[role];
  const picks = ROLE_PICKS[role];
  const list = picks.length
    ? picks.map((pick, index) => rolePickRow(pick, index + 1, { now, modelMenu: index === modelMenuAt ? roleModelMenu : '' })).join('')
    : `<div class="sm mu" style="padding:4px 16px 2px 36px">${ROLE_EMPTY}</div>`;
  return `<div class="col" style="padding:${title ? '12px' : '8px'} 0 4px;position:relative;${add || modelMenuAt >= 0 ? 'z-index:4' : ''}">${title ? `<div class="col" style="padding:0 16px 4px;gap:2px"><span>${info.name}</span><span class="sm mu">${info.desc}</span></div>` : ''}${list}${roleAdd(add)}</div>`;
};
const ROLES_NOTE = 'Each role tries its agents in this order; the first with an account to spare runs the task. Of an agent’s accounts, the one with the most usage to spare is used.';
const rolesSection = (blocks, opts = {}) => s2Section('Roles', blocks, { note: ROLES_NOTE, ...opts });
const subthreadsPage = (sections, opts) => s2Piece(s2Page('Subthreads', [s2Section('Subthreads', [S2_ROWS.limit()]), ...sections], opts));
const roleSummary = (role) => {
  const picks = ROLE_PICKS[role];
  if (!picks.length) return '<span class="mu">The agent chooses</span>';
  return picks.map((pick) => `${S2_AGENTS[pick.agent].name} · ${pick.model}`).join('<span class="mu">, then </span>');
};
const roleSummaryControl = (role, open = false) => `<span class="row g1 sm" style="max-width:330px;padding:3px 6px 3px 8px;border-radius:6px;${open ? 'background:var(--hov);border:1px solid var(--bf)' : 'border:1px solid transparent'}"><span class="trunc">${roleSummary(role)}</span>${ic('chev-right', 'xs mu')}</span>`;
const roleChip = (pick, place, open = false) => `<span class="chip" style="height:26px;gap:5px;${open ? 'border-color:var(--bf);color:var(--t)' : ''}"><span class="xs mu">${place}</span>${glyph(pick.agent)}${S2_AGENTS[pick.agent].name} · ${pick.model} · ${pick.effort}</span>`;
const roleNowPicker = (machine = 'mac') => `<span class="row g15 sm mu">Now on ${s2MachineDd(machine, { all: false })}</span>`;

TOPICS.push({
  id: 'roles', section: 'Subthreads', title: 'What each subthread role runs on', size: 'wide', rec: 'A',
  now: 'Built without a screen (3b4bf5a). Agents delegate by four roles: researcher (finds things out), reviewer (checks work), worker (does it) and other (the default). For each, the user keeps an ordered list of picks, each an agent with a model and effort (both may be left to the default). A task tries its role’s picks in order: the first whose agent is installed on the task’s machine and has an account logged in, not used up and offering the model runs it; of that agent’s accounts, the one with the most usage to spare goes first. An empty list leaves the choice to the agent that delegates. The picks are an app setting the app sends every server, so a task started on Devbox 1 uses Devbox 1’s accounts. Agents see the picks in their tools. There’s no row for them in Settings yet; the backlog suggests one beside Subthreads at once in General › Threads. Neither Zed nor t3code has anything like it. The mocks use topic 1’s A (a Subthreads page) where they need a page.',
  nowImg: 'img/now-settings-general.png',
  issues: [
    'Four lists of up to a few picks each, each pick with three choices: more than a row’s control holds.',
    'The model and effort menus come from what each agent last reported, and differ by agent.',
    'An agent installed on one machine only is still a fine pick: elsewhere it’s skipped.',
  ],
  options: [
    { key: 'A', name: 'A Roles section, each role’s picks listed under it', from: 'new, with Settings’ own list rows (Environment Variables)',
      desc: 'On the Subthreads page (topic 1’s A), under Subthreads at once, a <b>Roles</b> section with the four roles in it. Each has its name and what it’s for, then its picks as rows: a grip to drag them into order, its place, the agent, and Model and Effort menus with the agent’s choices (the first entry is the default), and × to remove. <b>Add agent</b> under them lists the agents installed on any machine. A role without picks says “None: the agent that delegates chooses.” A note under the section says how the order and accounts are used.',
      good: 'Every pick, its order and its choices in view, edited where they’re shown.', cost: 'The longest of the four: a full page of its own with five or six picks.',
      mock: () => subthreadsPage([rolesSection([roleBlock('researcher', { add: roleAgentMenu() }), roleBlock('reviewer'), roleBlock('worker'), roleBlock('other')])]) },
    { key: 'B', name: 'A row per role beside Subthreads at once, edited in a popover', from: 'the backlog’s place for it',
      desc: 'In General › Threads, where the backlog put it, under Subthreads at once: four rows, Researcher, Reviewer, Worker and Other, each with its picks in a line (“Codex · GPT-5.5, then Claude Agent · Sonnet 5.5”), or “The agent chooses”. Clicking one opens a popover with A’s list for that role, to reorder, change and add to. Works whether General is split up or not.',
      good: 'Four rows, so it fits in General as it is.', cost: 'Efforts don’t show until you open a role, and a long list is cut short in its row.',
      mock: () => s2Piece(s2Page('', [s2Section('Threads', [S2_ROWS.archive(), S2_ROWS.limit(), s2Row('Researcher', 'Subthreads that find things out.', roleSummaryControl('researcher')), s2Row('Reviewer', 'Subthreads that check work.', roleSummaryControl('reviewer')), `<div style="position:relative;z-index:4">${s2Row('Worker', 'Subthreads that do the work.', roleSummaryControl('worker', true))}<div class="pop" style="position:absolute;top:46px;right:14px;width:560px;padding:4px 0;z-index:20">${roleBlock('worker', { title: false })}</div></div>`, s2Row('Other', 'Anything else, and tasks given no role.', roleSummaryControl('other'))]), '<div style="height:40px"></div>'], { top: 12 }), 720) },
    { key: 'C', name: 'One role at a time, by tabs', from: 'Zed’s ToggleButtonGroup, as Settings uses for Sizes',
      desc: 'The Roles section’s heading has Researcher, Reviewer, Worker and Other as a segmented control, and one role’s list shows under it, with what the role is for, as in A. Shown here with a pick’s Model menu open.',
      good: 'Compact, and each list has room.', cost: 'Only one role in view: you can’t see at a glance which roles have picks.',
      mock: () => subthreadsPage([rolesSection([`<div class="sm mu" style="padding:12px 16px 0">${ROLE_INFO.worker.desc} Tasks agents give the worker role.</div>${roleBlock('worker', { title: false, modelMenuAt: 0 })}`], { right: s2Seg(['Researcher', 'Reviewer', 'Worker', 'Other'], 2) })]) },
    { key: 'D', name: 'A role per row, its picks as chips', from: 'new',
      desc: 'Each role is one row: its name, then its picks as chips in order (“1 Codex · GPT-5.5 · High”), dragged to reorder, and a + chip to add one. Clicking a chip opens a small popover with the agent, Model and Effort menus and Remove.',
      good: 'All four roles and every pick, efforts included, in four rows.', cost: 'Chips wrap when a role has three or more, and dragging chips is fiddly.',
      mock: () => subthreadsPage([rolesSection(['researcher', 'reviewer', 'worker', 'other'].map((role) => {
        const picks = ROLE_PICKS[role];
        const chips = picks.length ? picks.map((pick, index) => role === 'worker' && index === 0
          ? `<span style="position:relative">${roleChip(pick, index + 1, true)}<div class="pop" style="position:absolute;top:32px;left:0;width:300px;padding:10px;z-index:20;display:flex;flex-direction:column;gap:8px">${[['Agent', `${glyph('claude')} Claude Agent`], ['Model', 'Sonnet 5.5'], ['Effort', 'Medium']].map(([label, value]) => `<div class="row" style="justify-content:space-between"><span class="sm mu">${label}</span>${s2Dd(value)}</div>`).join('')}<div class="hr"></div><span class="row g1 sm delc">${ic('trash', 'xs')}Remove</span></div></span>`
          : roleChip(pick, index + 1)).join('') : `<span class="sm mu">${ROLE_EMPTY}</span>`;
        return `<div class="row g3" style="padding:10px 16px;position:relative;${role === 'worker' ? 'z-index:4' : ''}"><span class="none" style="width:86px">${ROLE_INFO[role].name}</span><span class="row g15" style="flex-wrap:wrap">${chips}<span class="chip" style="height:26px">${ic('plus', 'xs')}</span></span></div>`;
      })), '<div style="height:50px"></div>']) },
  ],
});

TOPICS.push({
  id: 'roles-now', section: 'Subthreads', title: 'Which account a pick would use', size: 'wide', rec: 'AC',
  now: 'The server chooses the account when a task starts: of the pick’s agent’s accounts on the task’s machine, logged in, not used up and offering the model, the one with the most usage to spare per hour until its limit resets (60% left resetting in an hour beats 80% left resetting in six days), then the account for new threads. Settings doesn’t say which that would be. In the mocks, on This Mac: Claude Agent’s Work is used up (resets in 1h 52m), so Personal (62% left); Codex has one account (81% left); Factory Droid’s login is used up until 4:10 PM, so its picks are skipped. Devbox 1 has only Claude Agent, with its own login. The mocks use this round’s A for the list.',
  nowImg: 'img/now-settings-usage.png',
  issues: [
    'The answer changes by the minute, and differs by machine: the roles are every machine’s, the accounts each machine’s.',
    'Why an account was passed over (used up, logged out, no such model) is what makes the choice make sense.',
  ],
  options: [
    { key: 'A', name: 'The account at each pick’s end, for one machine', from: 'new, with the Usage page’s reads',
      desc: 'Each pick ends with the account its agent would use now, with its avatar and what’s left in the window Usage shown in the composer picks (“Personal · 62% left”). A pick that would be skipped says why instead, in the warning color (“Used up until 4:10 PM”, “Not installed”, “No account offers GPT-5.5”). It’s for one machine, picked in the section’s header, This Mac first.',
      good: 'Shows at a glance which picks would run and on what.', cost: 'Another line on every pick, and only one machine at a time.',
      mock: () => subthreadsPage([rolesSection([roleBlock('researcher', { now: true }), roleBlock('worker', { now: true })], { right: roleNowPicker() })]) },
    { key: 'B', name: 'One line per role: what a task would run on now', from: 'new',
      desc: 'Under each role’s list, one muted line: “A worker task on This Mac now: Claude Agent · Sonnet 5.5, on Personal (62% left).” When nothing could run it: “None could now: Claude Agent and Factory Droid are used up.” The machine is picked in the section’s header, as in A.',
      good: 'One sentence per role, the answer itself.', cost: 'Doesn’t say why the picks before it were skipped.',
      mock: () => subthreadsPage([rolesSection(['researcher', 'worker'].map((role) => {
        const line = role === 'researcher'
          ? `A researcher task on This Mac now: Codex · GPT-5.5, on ${s2Avatar(S2_ACCOUNTS.codex, 12)} Personal (81% left).`
          : `A worker task on This Mac now: Claude Agent · Sonnet 5.5, on ${s2Avatar(S2_ACCOUNTS.personal, 12)} Personal (62% left).`;
        return roleBlock(role).replace(/<div class="row g1 sm mu" style="height:32px[^>]*>/, `<div class="row g1 sm" style="padding:2px 16px 4px 36px;color:var(--mu)">${line}</div>$&`);
      }), { right: roleNowPicker() })]) },
    { key: 'C', name: 'The choice explained on hover', from: 'new, with the sidebar’s details popover',
      desc: 'Hovering a pick’s account (A’s line, or the pick itself in B) opens a popover beside it, as the sidebar’s details popover sits beside a row: each of the agent’s accounts on each machine, in the order they’d be tried, with why each is used or passed over (“Most to spare: 62% left, resets in 3h”, “Used up, resets in 1h 52m”, “Logged out on agents-1”). It goes with A or B: pick it with + Also.',
      good: 'Answers “why not Work?” without a trip to Usage, and covers every machine.', cost: 'A hover to learn anything, and one more popover to keep current.',
      mock: () => subthreadsPage([rolesSection([`<div style="position:relative;z-index:4">${roleBlock('worker', { now: true })}<div class="pop" style="position:absolute;top:96px;right:-4px;width:380px;padding:6px;z-index:20">${[
        ['lbl', 'Claude Agent’s accounts, in the order tried · This Mac'],
        [s2Avatar(S2_ACCOUNTS.personal), 'Personal', 'Most to spare: 62% left, resets in 3h', true],
        [s2Avatar(S2_ACCOUNTS.work), 'Work', 'Used up, resets in 1h 52m'],
        ['lbl', 'Devbox 1'],
        [s2Avatar({ name: 'Main', hue: 280 }), 'Main', '40% left, resets in 5d'],
        ['lbl', 'agents-1'],
        [ic('x-circle', 'sm mu'), 'Main', 'Logged out'],
      ].map((row) => row[0] === 'lbl' ? `<div class="xs mu" style="padding:6px 8px 2px">${row[1]}</div>` : `<div class="row g2" style="min-height:28px;padding:0 8px;border-radius:5px;${row[3] ? 'background:var(--hov)' : ''}">${row[0]}<span class="none" style="width:76px">${row[1]}</span><span class="sm ${row[3] ? '' : 'mu'}">${row[2]}</span></div>`).join('')}</div></div>`], { right: roleNowPicker() }), '<div style="height:120px"></div>']) },
    { key: 'D', name: 'Nothing in Settings', from: 'today',
      desc: 'Settings lists the picks only. Which account runs a task stays the server’s choice when it starts, and Usage shows what each account has left.',
      good: 'A calmer page, and nothing that goes stale while it’s open.', cost: 'No way to see why a role ran on its second pick without comparing Usage by hand.',
      mock: () => subthreadsPage([rolesSection([roleBlock('researcher'), roleBlock('worker')])]) },
  ],
});
