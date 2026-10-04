// Sidebar topics.

const SB_H = 470;
const sb = (list, agents = '', opts = {}) => frame(sidebar({ list, agents, ...opts }), { w: 290, h: opts.h || SB_H });
const ab = (s) => `${s.ahead ? `<span class="okc">↑${s.ahead}</span>` : ''}${s.behind ? `<span class="delc">↓${s.behind}</span>` : ''}`;
const icon16 = (s) => (s.mono ? mono(s.mono, s.color) : ic('folder', 'sm mu'));
const where = (s) => s.branch || s.path;
const agentGlyphs = { st: ['claude'], bo: ['codex'], ap: ['thread', 'opencode'] };

// 1. Workspace rows ---------------------------------------------------------------------
TOPICS.push({
  id: 'rows', section: 'Sidebar', title: 'Workspace rows', size: 'narrow', rec: 'A',
  now: 'Two lines, laid out like the Agents sidebar\'s shell rows: the project icon (dimmed when not selected), the name and the status; then the branch (or the path outside git), ↑↓, terminal and agent counts, and the machine.',
  nowImg: 'img/now-rows.png',
  issues: [
    'The two counts read as "4 · 1" with tiny icons; the people icon for agents is ambiguous.',
    'Outside git the path is the raw one (<code>/private/tmp/az…/docs</code>), not <code>~/docs</code>.',
    'The machine icon shows even with one machine, and unselected icons at 40% look disabled.',
  ],
  options: [
    {
      key: 'A', name: 'Tidied two lines', from: 'herdr rows',
      desc: 'Same layout, cleaned up: icons at full color, agents shown by their own icons instead of a count, <code>~</code> paths, and the machine only when you have more than one.',
      good: 'Familiar; matches the Agents sidebar; tells you <i>which</i> agents are in there.', cost: 'Still two lines per workspace.',
      mock: () => sb(SPACES.map((s, i) => `<div class="srow ${i === 0 ? 'on' : ''}">
        <div class="l1">${icon16(s)}<span class="grow trunc ${i === 0 ? '' : 'mu'}">${s.name}</span>${pill(s.state)}</div>
        <div class="l2"><span class="trunc">${where(s)}</span>${ab(s)}<span class="grow"></span>
          <span class="row g1">${(agentGlyphs[s.id] || []).map((g) => glyph(g, 'sm')).join('')}${s.terminals ? `<span class="cnt">${ic('terminal', 'xs')}${s.terminals}</span>` : ''}</span>
          ${s.machine === 'devbox' ? `<span class="mu" style="opacity:.7;display:inline-flex">${ic('server', 'xs')}</span>` : ''}</div></div>`).join('')),
    },
    {
      key: 'B', name: 'One compact line', from: 'herdr collapsed density',
      desc: 'A 28px row: state dot, icon, name, and the branch on the right in faint text. Statuses become dots (the label is in the tooltip). Twice as many workspaces fit.',
      good: 'Dense; scales to 20+ workspaces.', cost: 'Status is color-only; less at a glance.',
      mock: () => sb([...SPACES, { name: 'infra', mono: 'IN', color: 'o', branch: 'main', machine: 'devbox' }, { name: 'design-system', mono: 'DS', color: 'p', branch: 'tokens' }, { name: 'blog', path: '~/blog' }].map((s, i) =>
        `<div class="srow ${i === 0 ? 'on' : ''}" style="padding:2px 10px"><div class="l1" style="gap:8px"><span style="width:6px;display:inline-flex">${dot(s.state)}</span>${icon16(s)}<span class="trunc ${i === 0 ? '' : 'mu'}" style="flex:none;max-width:120px">${s.name}</span><span class="grow"></span><span class="xs faint trunc" style="max-width:110px">${where(s)}</span>${ab(s) ? `<span class="xs">${ab(s)}</span>` : ''}</div></div>`).join('')),
    },
    {
      key: 'C', name: 'Tree with its tabs', from: 'herdr tab addressing',
      desc: 'A chevron opens a workspace into its tabs, each with its state and pane count; a click on a tab goes straight there. The selected workspace opens by itself.',
      good: 'Tabs reachable from the sidebar; see which tab needs you.', cost: 'Longer list; tabs shown twice (sidebar and tab bar).',
      mock: () => sb(SPACES.map((s, i) => {
        const open = i === 0 || i === 2;
        const tabs = open ? s.tabs.map(([name, state, panes], t) => `<div class="row" style="height:24px;margin:0 4px;padding:0 10px 0 36px;border-radius:6px;gap:8px;${i === 0 && t === 0 ? 'background:var(--hov)' : ''}"><span style="width:6px;display:inline-flex">${dot(state)}</span><span class="sm ${i === 0 && t === 0 ? '' : 'mu'} grow">${name}</span><span class="xs faint">${panes} ${panes === 1 ? 'pane' : 'panes'}</span></div>`).join('') : '';
        return `<div class="srow ${i === 0 ? 'on' : ''}" style="padding:4px 10px 4px 4px"><div class="l1" style="gap:6px"><span class="mu" style="display:inline-flex;width:16px">${ic(open ? 'chev-down' : 'chev-right', 'xs')}</span>${icon16(s)}<span class="grow trunc ${i === 0 ? '' : 'mu'}">${s.name}</span>${pill(s.state)}</div></div>${tabs}`;
      }).join('')),
    },
    {
      key: 'D', name: 'Layout thumbnail', from: 'new',
      desc: 'In place of the icon, a tiny map of the shown tab\'s panes, each tinted by its agent\'s state. You recognize a workspace by its shape and see which pane needs you.',
      good: 'Very scannable; shows where in the layout the attention is.', cost: 'Loses the project icon; abstract until you\'re used to it.',
      mock: () => {
        const map = (cells) => `<span style="width:30px;height:22px;border:1px solid var(--b);border-radius:3px;display:grid;gap:1px;background:var(--b);flex:none;${cells.grid}">${cells.c.map((c) => `<i style="background:${c};display:block"></i>`).join('')}</span>`;
        const ed = 'var(--ed)', w = 'rgba(116,173,232,.55)', p = 'rgba(222,193,132,.65)', a = 'rgba(180,119,207,.6)', d = 'rgba(161,193,129,.5)';
        const maps = [
          { grid: 'grid-template-columns:1.2fr 1fr;grid-template-rows:1fr 1fr;', c: [w, ed, ed] },
          { grid: 'grid-template-columns:1fr 1fr;', c: [p, ed] },
          { grid: 'grid-template-columns:1fr 1fr;', c: [d, ed] },
          { grid: '', c: [ed] }, { grid: '', c: [ed] },
        ];
        maps[0].c = [w, ed, ed]; maps[0].grid += 'grid-template-areas:"a b" "a c";';
        return sb(SPACES.map((s, i) => `<div class="srow ${i === 0 ? 'on' : ''}"><div class="row g2" style="align-items:center">${i === 0 ? `<span style="width:30px;height:22px;border:1px solid var(--b);border-radius:3px;display:grid;grid-template-columns:1.2fr 1fr;grid-template-rows:1fr 1fr;gap:1px;background:var(--b);flex:none"><i style="grid-row:1/3;background:${w}"></i><i style="background:${ed}"></i><i style="background:${ed}"></i></span>` : map(maps[i])}
          <div class="col grow" style="min-width:0"><div class="row g2"><span class="grow trunc ${i === 0 ? '' : 'mu'}">${s.name}</span>${pill(s.state)}</div><div class="row g1 xs faint"><span class="trunc">${where(s)}</span>${ab(s)}<span class="grow"></span>${s.tabs.length > 1 ? `${s.tabs.length} tabs` : ''}</div></div></div></div>`).join(''));
      },
    },
    {
      key: 'E', name: 'Agents inside the row', from: 'new',
      desc: 'The second line lists the agents in the workspace as chips with their own state ("✻ working", "◎ approval"); shells collapse into "+2 shells". The separate Agents list below can go.',
      good: 'One list for everything; you see each agent, not a rolled-up state.', cost: 'Rows grow with the number of agents.',
      mock: () => {
        const chip = (g, state, text) => `<span class="row" style="gap:4px;height:20px;padding:0 6px;border-radius:5px;background:${state === 'pending' ? 'rgba(222,193,132,.12)' : state === 'awaiting' ? 'rgba(180,119,207,.13)' : 'rgba(255,255,255,.04)'};font-size:11px;color:var(--${state === 'pending' ? 'warn' : state === 'awaiting' ? 'pur' : state === 'working' ? 'ac' : state === 'done' ? 'ok' : 'mu'})">${glyph(g, 'sm')}${text}</span>`;
        const lines = {
          st: chip('claude', 'working', 'working') + '<span class="xs faint">+3 shells</span>',
          bo: chip('codex', 'pending', 'approval'),
          ap: chip('thread', 'awaiting', 'input') + chip('opencode', 'done', 'done'),
          rn: '<span class="xs faint">1 shell</span>', hm: '<span class="xs faint">1 shell</span>',
        };
        return sb(SPACES.map((s, i) => `<div class="srow ${i === 0 ? 'on' : ''}"><div class="l1">${icon16(s)}<span class="trunc ${i === 0 ? '' : 'mu'}">${s.name}</span><span class="xs faint trunc grow">${where(s)}</span>${ab(s) ? `<span class="xs">${ab(s)}</span>` : ''}</div><div class="row g1" style="padding-left:26px;flex-wrap:wrap">${lines[s.id]}</div></div>`).join(''), '', { h: SB_H });
      },
    },
  ],
});

// 2. Organizing the list --------------------------------------------------------------------
const flatRow = (s, { active, indent = 0, extra = '', right } = {}) => `<div class="srow ${active ? 'on' : ''}" style="padding-left:${10 + indent}px"><div class="l1">${s.kind === 'worktree' ? `<span class="mu" style="display:inline-flex">${ic('worktree', 'sm')}</span>` : icon16(s)}<span class="grow trunc ${active ? '' : 'mu'}">${s.name}</span>${right ?? pill(s.state)}</div><div class="l2" style="padding-left:26px"><span class="trunc">${where(s)}</span>${ab(s)}${extra}</div></div>`;
const subhead = (html) => `<div class="row g2" style="height:28px;padding:0 14px;margin-top:4px;font-size:12px;color:var(--mu);font-weight:500">${html}</div>`;
TOPICS.push({
  id: 'organize', section: 'Sidebar', title: 'Organizing the list', size: 'narrow', rec: 'A',
  now: 'One flat list in the order you drag it to. A worktree opened from a workspace\'s menu becomes an unrelated workspace at the end. Workspaces on other machines are mixed in.',
  issues: ['herdr groups worktrees under the workspace they came from; agentZ doesn\'t yet.', 'Nothing separates machines, or what needs you from what\'s idle.'],
  options: [
    {
      key: 'A', name: 'Flat, worktrees nested', from: 'herdr',
      desc: 'Your order stays, but a worktree or pasture made from a workspace sits under it, indented with a connector. Closing the parent closes the group (checkouts and branches stay). herdr\'s grouped worktrees.',
      good: 'Keeps related checkouts together; least change.', cost: 'Doesn\'t help with many machines.',
      mock: () => sb(flatRow(SPACES[0], { active: true }) + `<div style="position:relative">${flatRow(SPACES[1], { indent: 18 })}<span style="position:absolute;left:20px;top:-6px;width:10px;height:22px;border-left:1px solid var(--b);border-bottom:1px solid var(--b);border-bottom-left-radius:4px"></span></div>` + flatRow(SPACES[2]) + flatRow(SPACES[3]) + flatRow(SPACES[4])),
    },
    {
      key: 'B', name: 'Grouped by project', from: 't3code sidebar',
      desc: 'A header per project, like the Agents sidebar, with its checkout and worktrees under it; folders outside projects under "Folders". New workspace from a header\'s +.',
      good: 'Same mental model as the Agents view.', cost: 'Headers take space when each project has one workspace.',
      mock: () => sb(subhead(`${mono('ST', 'g')}storefront<span class="grow"></span>${ic('plus', 'xs')}`) + flatRow(SPACES[0], { active: true, right: pill('working') }) + flatRow({ ...SPACES[1] }) + subhead(`${mono('AP', 't')}api<span class="grow"></span><span class="faint">${ic('server', 'xs')}</span>`) + flatRow(SPACES[2]) + subhead(`${ic('folder', 'xs')}Folders`) + flatRow(SPACES[3]) + flatRow(SPACES[4])),
    },
    {
      key: 'C', name: 'Grouped by machine', from: 'herdr machines',
      desc: 'A header per machine with its connection state, and + to open a workspace there. Shown only once you have a second machine; with one it stays flat.',
      good: 'Clear where things run; offline machines dim as a block.', cost: 'Projects split across machines.',
      mock: () => sb(subhead(`${ic('laptop', 'sm')}This Mac<span class="dot done" style="margin-left:2px"></span><span class="grow"></span>${ic('plus', 'xs')}`) + flatRow(SPACES[0], { active: true }) + flatRow(SPACES[1]) + flatRow(SPACES[3]) + flatRow(SPACES[4]) + subhead(`${ic('server', 'sm')}Devbox 1<span class="dot done" style="margin-left:2px"></span><span class="grow"></span>${ic('plus', 'xs')}`) + flatRow(SPACES[2])),
    },
    {
      key: 'D', name: 'Pinned, then recent', from: 't3code pins',
      desc: 'Pin the workspaces you live in; the rest sort by last activity with a relative time. Ones idle for days fold into "Older".',
      good: 'Self-cleaning list.', cost: 'Unpinned rows move around as things happen.',
      mock: () => sb(subhead(`${ic('pin', 'xs')}Pinned`) + flatRow(SPACES[0], { active: true }) + flatRow(SPACES[2]) + subhead(`${ic('clock', 'xs')}Recent`) + flatRow(SPACES[1], { right: `${pill('pending')}` }) + flatRow(SPACES[3], { right: '<span class="xs faint">1h</span>' }) + `<div class="row g2" style="height:30px;padding:0 14px;color:var(--ph);font-size:12px">${ic('chev-right', 'xs')}Older<span class="faint">3</span></div>`),
    },
    {
      key: 'E', name: 'Needs you first', from: 'new',
      desc: 'Sorted by state automatically: waiting for approval or input, then working, then done, then idle under a faint rule. A Sort menu in the header switches back to your own order.',
      good: 'A triage list: the top row is always the next thing to do.', cost: 'Rows jump around; muscle memory suffers.',
      mock: () => sb(flatRow(SPACES[1]) + flatRow(SPACES[2]) + flatRow(SPACES[0], { active: true }) + '<div style="height:1px;background:var(--bv);margin:6px 14px"></div>' + flatRow(SPACES[3]) + flatRow(SPACES[4]), '', { head: sbHead(`<span class="row g1 xs mu" style="border:1px solid var(--bv);border-radius:5px;padding:1px 6px">${ic('filter', 'xs')}Attention${ic('chev-down', 'xs')}</span>`) }),
    },
  ],
});

// 3. The Agents list ----------------------------------------------------------------------------
TOPICS.push({
  id: 'agents', section: 'Sidebar', title: 'Agents list', size: 'narrow', rec: 'A',
  now: 'An "Agents" section under the workspaces: one slim row per agent in a pane (agent CLIs and ACP threads), with its state dot, name and "machine · workspace › tab". Clicking focuses its pane.',
  nowImg: 'img/now-agents.png',
  issues: ['Every agent shows the same terminal icon.', 'Nothing says what an agent is doing or how long ago it finished.', 'The location is cut off when names are long.'],
  options: [
    {
      key: 'A', name: 'Two lines, herdr\'s default', from: 'herdr rows',
      desc: 'herdr\'s default agent layout: the state and where it is on top (machine, workspace, tab), the agent\'s icon and name below.',
      good: 'Location never truncates; agent icons tell them apart.', cost: 'Twice the height.',
      mock: () => sb(currentList(), agentsSection(AGENTS.map((a, i) => `<div class="arow ${i === 0 ? 'on' : ''}" style="height:auto;padding:5px 8px;flex-direction:column;align-items:stretch;gap:1px"><div class="row g2 xs mu"><span style="width:6px;display:inline-flex">${dot(a.state)}</span><span class="trunc">${a.machine === 'Devbox 1' ? 'Devbox 1 · ' : ''}${a.space} › ${a.tab}</span></div><div class="row g2" style="padding-left:14px">${glyph(a.kind, 'sm')}<span class="trunc sm ${i === 0 ? '' : 'mu'}">${a.title}</span></div></div>`).join('')), { h: 560 }),
    },
    {
      key: 'B', name: 'Grouped by state', from: 't3code statuses',
      desc: 'Subheaders "Needs you", "Working" and "Done", empty ones hidden. Idle agents fold away.',
      good: 'What to do next is at the top.', cost: 'Rows move between groups as states change.',
      mock: () => {
        const r = (a) => `<div class="arow">${glyph(a.kind, 'sm')}<span class="grow trunc">${a.title}</span><span class="xs trunc faint" style="max-width:110px">${a.space} › ${a.tab}</span></div>`;
        const h = (state, text, n) => `<div class="row g2 xs" style="height:22px;padding:0 12px;color:var(--${state})">${dot(state === 'warn' ? 'pending' : state === 'ac' ? 'working' : 'done')}${text}<span class="faint">${n}</span></div>`;
        return sb(currentList(), agentsSection(h('warn', 'Needs you', 2) + r(AGENTS[1]) + r(AGENTS[2]) + h('ac', 'Working', 1) + r(AGENTS[0]) + h('ok', 'Done', 1) + r(AGENTS[3])), { h: 560 });
      },
    },
    {
      key: 'C', name: 'Collapsible, with counts', from: 'Zed panels',
      desc: 'The header carries a count per state and folds the list away; a drag handle on its top edge sets how much room it gets.',
      good: 'Out of the way when you don\'t need it.', cost: 'Folded, you see counts but not who.',
      mock: () => sb(currentList(), `<div style="height:5px;cursor:row-resize;display:flex;justify-content:center;align-items:center"><span style="width:28px;height:3px;border-radius:2px;background:var(--b)"></span></div><div class="section-h">Agents<span class="row g1 xs" style="margin-left:4px">${dot('pending')}<span class="warnc">1</span>${dot('awaiting')}<span class="purc">1</span>${dot('working')}<span class="ac">1</span>${dot('done')}<span class="okc">1</span></span><span class="rule"></span>${ic('chev-down', 'xs')}</div>${AGENTS.map((a, i) => currentAgentRow(a, { active: i === 0 })).join('')}`, { h: 560 }),
    },
    {
      key: 'D', name: 'What each is doing', from: 't3code cards',
      desc: 'A second line with the agent\'s last step, read from its screen or thread ("Update(src/app/checkout/page.tsx)", "Allow command? …") and how long ago.',
      good: 'You can decide whether to look without switching.', cost: 'Screen-reading is a best guess for CLIs.',
      mock: () => sb(currentList(), agentsSection(AGENTS.map((a, i) => `<div class="arow ${i === 0 ? 'on' : ''}" style="height:auto;padding:5px 8px;align-items:flex-start"><span style="width:6px;margin-top:7px;display:inline-flex">${dot(a.state)}</span><div class="col grow" style="min-width:0"><div class="row g2">${glyph(a.kind, 'sm')}<span class="trunc sm ${i === 0 ? '' : 'mu'}">${a.title}</span><span class="grow"></span><span class="xs faint">${a.ago}</span></div><span class="xs faint trunc" style="padding-left:22px">${a.activity}</span></div></div>`).join('')), { h: 560 }),
    },
    {
      key: 'E', name: '"Needs you" strip on top', from: 'new',
      desc: 'No list at the bottom. Agents waiting on you show in a tinted strip above the workspaces, with a Go button; working and done ones show in their workspace\'s row (pairs with rows E).',
      good: 'Only what needs you takes space; very hard to miss.', cost: 'No single list of every agent.',
      mock: () => sb(`<div style="margin:2px 6px 6px;border:1px solid rgba(222,193,132,.35);background:rgba(222,193,132,.07);border-radius:8px;padding:4px">${[AGENTS[1], AGENTS[2]].map((a) => `<div class="row g2" style="height:30px;padding:0 6px">${dot(a.state)}${glyph(a.kind, 'sm')}<div class="col grow" style="min-width:0;line-height:1.15"><span class="sm trunc">${a.title}</span><span class="xs faint trunc">${a.space} › ${a.tab}</span></div><span class="btn sm">Go</span></div>`).join('')}</div>` + currentList(), '', { h: 470 }),
    },
  ],
});

// 4. Collapsed sidebar -----------------------------------------------------------------------
const mini = (content, w = 640, h = 400) => frame(`<div class="row" style="height:100%;align-items:stretch">${content}</div>`, { w, h });
const miniMain = (lead = '') => `<div class="col grow" style="min-width:0">${tabBar(undefined, { lead })}<div class="tile" style="flex:1">${pane({ glyphKind: 'claude', title: 'Claude Code', detail: '~/storefront', state: 'working', focus: true, body: SCREENS.claude, buttons: false })}</div></div>`;
const railItem = (s, { active, n } = {}) => `<div class="col" style="align-items:center;gap:1px;position:relative;padding:4px 0">${active ? '<span style="position:absolute;left:-6px;top:8px;width:3px;height:20px;border-radius:2px;background:var(--t)"></span>' : ''}<span style="width:30px;height:30px;border-radius:8px;display:grid;place-items:center;background:${active ? 'var(--sel)' : 'transparent'};position:relative">${s.mono ? mono(s.mono, s.color, 'lg').replace('lg', '" style="width:24px;height:24px;font-size:10px') : s.kind === 'worktree' ? `<span class="mu">${ic('worktree')}</span>` : `<span class="mu">${ic('folder')}</span>`}${s.state ? `<span class="dot ${s.state}" style="position:absolute;right:0;top:0;width:8px;height:8px;border:2px solid var(--panel)"></span>` : ''}</span>${n ? `<span style="font-size:9px;color:var(--ph)">⌘${n}</span>` : ''}</div>`;
TOPICS.push({
  id: 'collapsed', section: 'Sidebar', title: 'Collapsed sidebar', size: 'medium', rec: 'B',
  now: 'Cmd-B hides the sidebar completely. With it hidden there\'s no way to see other workspaces or their states, or to switch, except by showing it again.',
  options: [
    { key: 'A', name: 'Hidden (today)', from: 'Zed', desc: 'Keep it as is: hidden is hidden.', good: 'Most room; nothing new.', cost: 'You lose sight of every other workspace.', mock: () => mini(miniMain()) },
    {
      key: 'B', name: 'Icon rail', from: 'herdr collapsed sidebar',
      desc: 'A 48px rail of workspace icons with a state badge each; hover for the name, click to switch, + at the top. herdr\'s collapsed mode.',
      good: 'States stay visible for 48px.', cost: 'Monograms look alike with many projects.',
      mock: () => mini(`<div class="col none" style="width:48px;background:var(--panel);border-right:1px solid var(--b);align-items:center;padding-top:6px;gap:2px">${ibtn('plus')}<div style="height:1px;width:24px;background:var(--bv);margin:4px 0"></div>${SPACES.map((s, i) => railItem(s, { active: i === 0 })).join('')}<span class="grow"></span><span style="padding:8px 0" class="mu">${ic('settings', 'sm')}</span></div><div style="position:absolute;left:52px;top:118px;z-index:5" class="pop"><div style="padding:5px 9px;font-size:12px" class="row g2">api ${pill('awaiting')}</div></div>` + miniMain()),
    },
    {
      key: 'C', name: 'Numbered rail', from: 'herdr indexed jumps',
      desc: 'The rail, with each workspace numbered for Cmd-Option-1…9 (herdr\'s switch_workspace), worktrees indented under their parent.',
      good: 'Keyboard switching you can see.', cost: 'A little taller per item.',
      mock: () => mini(`<div class="col none" style="width:52px;background:var(--panel);border-right:1px solid var(--b);align-items:center;padding-top:6px">${ibtn('plus')}${SPACES.map((s, i) => railItem(s, { active: i === 0, n: i + 1 })).join('')}</div>` + miniMain()),
    },
    {
      key: 'D', name: 'Switcher in the tab bar', from: 'Agents view project switcher',
      desc: 'No rail. The tab bar starts with the workspace\'s name and a chevron; it opens a list of all workspaces with their states, like the Agents view\'s project switcher.',
      good: 'Zero width; one control you already know.', cost: 'States of other workspaces hidden until you open it.',
      mock: () => mini(miniMain(`<div class="row g2" style="padding:0 10px;border-right:1px solid var(--b)">${mono('ST', 'g')}<span class="sm">storefront</span>${ic('chev-down', 'xs mu')}${dot('pending')}</div>`) + `<div class="menu" style="left:6px;top:38px;width:250px">${SPACES.map((s, i) => `<div class="it ${i === 0 ? 'hl' : ''}">${icon16(s)}<span class="grow trunc">${s.name}</span>${s.state ? dot(s.state) : ''}<span class="kb">⌘⌥${i + 1}</span></div>`).join('')}<div class="hr"></div><div class="it">${ic('sidebar', 'sm')}Show Sidebar<span class="kb">⌘B</span></div></div>`),
    },
    {
      key: 'E', name: 'Peek on the edge', from: 'macOS Dock / Arc',
      desc: 'Hidden, but moving the pointer to the window\'s left edge slides the full sidebar over the panes until you leave it. Doesn\'t take room from the layout.',
      good: 'Full sidebar on demand, no layout shift.', cost: 'Hover-to-reveal can trigger by accident.',
      mock: () => mini(miniMain() + `<div style="position:absolute;left:0;top:0;bottom:0;z-index:6;box-shadow:12px 0 30px rgba(0,0,0,.5)">${sidebar({ list: currentList(), foot: '', style: 'height:100%' })}</div>` + note('pointer at the left edge', 'left:300px;top:200px')),
    },
  ],
});

// 5. Row actions and menu --------------------------------------------------------------------
const withMenu = (list, menu, { w = 560, h = 440 } = {}) => frame(`<div class="row" style="height:100%;align-items:stretch">${sidebar({ list })}<div class="grow" style="background:var(--ed)"></div></div>${menu}`, { w, h });
const it = (icon, text, kb = '', cls = '') => `<div class="it ${cls}">${icon ? ic(icon, 'sm') : '<span style="width:14px"></span>'}${text}${kb ? `<span class="kb">${kb}</span>` : ''}</div>`;
const hrr = '<div class="hr"></div>';
TOPICS.push({
  id: 'actions', section: 'Sidebar', title: 'Row actions and menu', size: 'medium', rec: 'A',
  now: 'Right-click a row: Rename, Close, and in a git repository New Worktree and Open Worktree…. Double-click renames. No buttons on hover.',
  issues: ['No way to open a new tab, copy the path or reveal the folder from the row.', 'No way to remove a worktree checkout you made (herdr has Delete worktree checkout…).'],
  options: [
    {
      key: 'A', name: 'A fuller right-click menu', from: 'herdr + Zed',
      desc: 'The same menu with what\'s missing: New Tab, Copy Path, Reveal in Finder (this Mac only), New Thread Here (opens the Agents view in that folder), and on a worktree, Delete Worktree Checkout… (herdr\'s safe remove).',
      good: 'Discoverable in one place; no new UI.', cost: 'Right-click only.',
      mock: () => withMenu(currentList('bo'), `<div class="menu" style="left:220px;top:96px">${it('plus', 'New Tab', '⌘T')}${it('pencil', 'Rename')}${it('copy', 'Copy Path')}${it('folder-open', 'Reveal in Finder')}${it('chat', 'New Thread Here')}${hrr}${it('worktree', 'New Worktree…')}${it('folder-open', 'Open Worktree…')}${it('trash', 'Delete Worktree Checkout…', '', 'danger')}${hrr}${it('x', 'Close Workspace')}</div>`),
    },
    {
      key: 'B', name: 'Buttons on hover', from: 't3code thread rows',
      desc: 'Hovering a row swaps its counts for two buttons: + (new tab) and ⋯ (the menu). Right-click still works.',
      good: 'Mouse users find the menu without right-clicking.', cost: 'Hover-only controls.',
      mock: () => withMenu(SPACES.map((s, i) => i === 0 ? `<div class="srow on"><div class="l1">${icon16(s)}<span class="grow trunc">${s.name}</span>${pill(s.state)}</div><div class="l2"><span class="trunc">${where(s)}</span>${ab(s)}<span class="grow"></span>${ibtn('plus', 'sm hov')}${ibtn('more', 'sm')}</div></div>` : currentRow(s)).join(''), `<div class="menu" style="left:250px;top:58px">${it('pencil', 'Rename')}${it('worktree', 'New Worktree…')}${it('folder-open', 'Open Worktree…')}${hrr}${it('x', 'Close Workspace')}</div>`),
    },
    {
      key: 'C', name: 'Grouped submenus', from: 'Zed project panel',
      desc: 'A short top level with Git ▸ and Open In ▸ submenus: Git has the worktree actions and Copy Branch Name; Open In has Finder, Zed and Agents.',
      good: 'Room to grow without a long menu.', cost: 'One more hover to reach worktrees.',
      mock: () => withMenu(currentList(), `<div class="menu" style="left:220px;top:40px">${it('plus', 'New Tab', '⌘T')}${it('pencil', 'Rename')}${it('copy', 'Copy Path')}${hrr}<div class="it hl">${ic('branch', 'sm')}Git<span class="kb">${ic('chev-right', 'xs')}</span></div>${it('external', 'Open In', ic('chev-right', 'xs'))}${hrr}${it('x', 'Close Workspace')}</div><div class="menu" style="left:436px;top:124px;min-width:190px">${it('worktree', 'New Worktree…')}${it('folder-open', 'Open Worktree…')}${hrr}${it('copy', 'Copy Branch Name')}</div>`, { w: 680 }),
    },
    {
      key: 'D', name: 'Action strip on the selected row', from: 'new',
      desc: 'The selected workspace grows a row of icon buttons under it: New Tab, New Thread, New Worktree, Rename, Close. Others stay compact.',
      good: 'Common actions one click away, no hunting.', cost: 'The selected row is taller.',
      mock: () => withMenu(SPACES.map((s, i) => i === 0 ? `<div class="srow on"><div class="l1">${icon16(s)}<span class="grow trunc">${s.name}</span>${pill(s.state)}</div><div class="l2"><span class="trunc">${where(s)}</span>${ab(s)}</div><div class="row g1" style="padding:6px 0 0 22px">${['plus', 'chat', 'worktree', 'pencil', 'x'].map((n) => ibtn(n, 'sm')).join('')}</div></div>` : currentRow(s)).join(''), ''),
    },
    {
      key: 'E', name: 'Workspace inspector', from: 'new',
      desc: '⋯ opens a card instead of a menu: the name (editable), the folder with Copy, the branch with worktree actions, its tabs with ×, a color, and Close.',
      good: 'Everything about a workspace in one place.', cost: 'Heavier than a menu for one action.',
      mock: () => withMenu(currentList(), `<div class="pop" style="left:240px;top:30px;width:300px;padding:12px;display:flex;flex-direction:column;gap:10px;font-size:13px"><div class="field focus">storefront</div><div class="row g2 sm mu">${ic('folder', 'sm')}<span class="grow trunc">~/w/storefront</span>${ibtn('copy', 'sm')}</div><div class="row g2 sm mu">${ic('branch', 'sm')}<span class="grow">checkout-flow <span class="okc">↑2</span></span><span class="btn sm">New Worktree</span></div><div class="col" style="gap:2px"><span class="xs ph">Tabs</span>${SPACES[0].tabs.map(([n, st, p]) => `<div class="row g2 sm" style="height:24px">${dot(st) || '<span style="width:6px"></span>'}<span class="grow">${n}</span><span class="xs faint">${p} panes</span>${ic('x', 'xs mu')}</div>`).join('')}</div><div class="row g2"><span class="xs ph">Color</span>${['#74ade8', '#a1c181', '#dec184', '#d07277', '#b477cf'].map((c, i) => `<span style="width:14px;height:14px;border-radius:50%;background:${c};${i === 1 ? 'box-shadow:0 0 0 2px var(--panel),0 0 0 3px var(--t)' : ''}"></span>`).join('')}</div><div class="row"><span class="grow"></span><span class="btn sm" style="color:var(--del)">Close Workspace</span></div></div>`, { h: 470 }),
    },
  ],
});

// 6. Hover details ---------------------------------------------------------------------------
const detailRow = (icon, text) => `<div class="row g2" style="height:26px;font-size:13px">${icon}<span class="trunc">${text}</span></div>`;
const withPop = (body, { w = 600, h = 420, top = 6 } = {}) => frame(`<div class="row" style="height:100%;align-items:stretch">${sidebar({ list: SPACES.map((s, i) => currentRow(s, { active: i === 0, hover: i === 0 })).join('') })}<div class="grow" style="background:var(--ed)"></div></div><div class="pop" style="left:292px;top:${top}px;width:290px;padding:10px 12px">${body}</div>`, { w, h });
const baseDetails = () => `<div class="b5" style="margin-bottom:4px">storefront</div>${detailRow(mono('ST', 'g'), 'storefront')}${detailRow(ic('laptop', 'sm mu'), 'This Mac')}${detailRow(ic('branch', 'sm mu'), 'checkout-flow')}`;
TOPICS.push({
  id: 'details', section: 'Sidebar', title: 'Hover details', size: 'medium', rec: 'A',
  now: 'Half a second on a row shows the thread cards\' details popover: name, project, machine, branch, path and what\'s inside ("3 terminals, 1 agent"). It hides while the counts show their own tooltip.',
  options: [
    {
      key: 'A', name: 'Today\'s, plus its tabs', from: 'herdr Goto',
      desc: 'The same popover, with the tabs listed at the bottom: each tab\'s state and what runs in its panes.',
      good: 'Answers "where is it" without switching.', cost: 'Taller popover.',
      mock: () => withPop(`${baseDetails()}${detailRow(ic('folder', 'sm mu'), '~/w/storefront')}<div style="height:1px;background:var(--bv);margin:6px 0"></div>${[['agents', 'working', 'Claude Code · zsh · npm run dev'], ['server', null, 'npm run dev'], ['3', null, 'zsh']].map(([n, s, p]) => `<div class="row g2" style="height:24px;font-size:12px"><span style="width:6px">${dot(s)}</span><span style="width:52px">${n}</span><span class="mu trunc">${p}</span></div>`).join('')}`),
    },
    {
      key: 'B', name: 'Map of each tab', from: 'new',
      desc: 'A small drawing of each tab\'s split layout, panes labeled and tinted by state.',
      good: 'You see the shape you\'ll land in.', cost: 'Needs space; labels get tiny.',
      mock: () => {
        const box = (label, st, style = '') => `<div style="border:1px solid ${st === 'working' ? 'var(--ac)' : 'var(--b)'};background:${st === 'working' ? 'rgba(116,173,232,.12)' : 'var(--ed)'};border-radius:3px;font-size:10px;color:var(--mu);padding:2px 4px;overflow:hidden;white-space:nowrap;${style}">${label}</div>`;
        return withPop(`<div class="b5" style="margin-bottom:8px">storefront <span class="xs faint">checkout-flow ↑2</span></div><div class="row g2" style="align-items:flex-start">${['agents', 'server', '3'].map((n, i) => `<div class="col g1" style="flex:1"><span class="xs ${i === 0 ? '' : 'mu'}">${n}</span><div style="height:62px;display:grid;gap:2px;${i === 0 ? 'grid-template-columns:1.2fr 1fr;grid-template-rows:1fr 1fr' : ''}">${i === 0 ? box('✻ Claude', 'working', 'grid-row:1/3') + box('zsh') + box('npm run') : box(i === 1 ? 'npm run dev' : 'zsh')}</div></div>`).join('')}</div>`);
      },
    },
    {
      key: 'C', name: 'Git at a glance', from: 'Zed git panel',
      desc: 'Focused on the checkout: branch and upstream, ahead/behind, uncommitted changes as +/−, the last commit, and the worktree\'s path.',
      good: 'Tells you if work is unsaved before you close it.', cost: 'The server has to run git status for every row it shows.',
      mock: () => withPop(`<div class="b5" style="margin-bottom:4px">storefront</div>${detailRow(ic('branch', 'sm mu'), 'checkout-flow → origin/main <span class="okc">↑2</span>')}${detailRow(ic('diff', 'sm mu'), '3 files changed <span class="okc">+24</span> <span class="delc">−3</span>')}${detailRow(ic('clock', 'sm mu'), '"Add pay button" · 12 min ago')}${detailRow(ic('folder', 'sm mu'), '~/w/storefront')}${detailRow(ic('worktree', 'sm mu'), '1 worktree: brave-otter')}`),
    },
    {
      key: 'D', name: 'Recent activity', from: 'new',
      desc: 'The last few things that happened inside: agents starting, asking, finishing; servers coming up.',
      good: 'Catch up on a workspace you left an hour ago.', cost: 'A new event history to keep on the server.',
      mock: () => withPop(`<div class="b5" style="margin-bottom:6px">storefront</div>${[['working', 'Claude Code started working', 'now'], ['done', 'npm run dev: ready on :3000', '4m'], ['pending', 'Claude Code asked to run npm test', '9m'], ['done', 'Claude Code finished', '22m']].map(([s, t, a]) => `<div class="row g2" style="height:26px;font-size:12px">${dot(s)}<span class="grow trunc">${t}</span><span class="xs faint">${a}</span></div>`).join('')}`),
    },
    {
      key: 'E', name: 'No popover', from: 'Zed',
      desc: 'Drop the hover popover; a plain tooltip names the full path only when the row truncates it. The workspace header (next topics) shows the rest.',
      good: 'Nothing pops up while you move the mouse.', cost: 'Details only for the open workspace.',
      mock: () => frame(`<div class="row" style="height:100%;align-items:stretch">${sidebar({ list: SPACES.map((s, i) => currentRow(s, { active: i === 0, hover: i === 2 })).join('') })}<div class="grow" style="background:var(--ed)"></div></div><div class="pop" style="left:120px;top:150px;padding:4px 8px;font-size:12px">~/w/api on Devbox 1</div>`, { w: 600, h: 420 }),
    },
  ],
});
