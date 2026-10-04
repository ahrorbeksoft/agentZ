// Creating workspaces, layouts and worktrees.

const pickerRow = (lead, text, detail, { on = false, tag = '' } = {}) => `<div class="row g2" style="height:30px;padding:0 8px;border-radius:6px;font-size:13px;${on ? 'background:var(--hov)' : ''}">${lead}<span class="trunc" style="flex:none;max-width:150px">${text}</span><span class="xs faint trunc grow">${detail}</span>${tag}</div>`;
const sub = (text) => `<div class="xs ph" style="padding:8px 8px 2px">${text}</div>`;
const openTag = '<span class="xs" style="color:var(--ac);border:1px solid rgba(116,173,232,.35);border-radius:4px;padding:0 5px">Open</span>';
const withSidebar = (overlay, { w = 760, h = 470, list = currentList() } = {}) => frame(`<div class="row" style="height:100%;align-items:stretch">${sidebar({ list })}<div class="grow" style="background:var(--ed)"></div></div>${overlay}`, { w, h });

// 13. New workspace ------------------------------------------------------------------------
TOPICS.push({
  id: 'newspace', section: 'Creating', title: 'New workspace', size: 'medium', rec: 'A',
  now: 'The sidebar\'s + opens a popover: a search field, then each machine\'s home folder and each project\'s checkout, worktrees and pastures, under their machine. Picking one opens a workspace with a shell there.',
  issues: ['Picking a folder that already has a workspace makes a second one.', 'Only home and project folders: no way to type another path.'],
  options: [
    {
      key: 'A', name: 'Today\'s picker, refined', from: 'Zed recent projects',
      desc: 'Recent folders first, then projects and homes. A folder that already has a workspace is marked Open, and picking it goes there instead of making another.',
      good: 'No duplicates; the usual choice is on top.', cost: 'Still only known folders.',
      mock: () => withSidebar(`<div class="pop" style="left:200px;top:38px;width:340px;padding:6px"><div class="field focus" style="margin-bottom:4px">${ic('search', 'sm mu')}<span class="ph">Search folders…</span></div>${sub('Recent')}${pickerRow(mono('ST', 'g'), 'storefront', '~/w/storefront', { on: true, tag: openTag })}${pickerRow(ic('worktree', 'sm mu'), 'brave-otter', 'worktree of storefront', { tag: openTag })}${sub('This Mac')}${pickerRow(ic('folder', 'sm mu'), '~', 'Home')}${pickerRow(mono('DS', 'p'), 'design-system', '~/w/design-system')}${sub('Devbox 1')}${pickerRow(ic('folder', 'sm mu'), '~', 'Home')}${pickerRow(mono('AP', 't'), 'api', '~/w/api', { tag: openTag })}<div class="row hint" style="padding:6px 8px 2px;gap:10px"><span>↩ open</span><span>⌘↩ open another</span></div></div>`),
    },
    {
      key: 'B', name: 'Type any path', from: 'Add Project\'s remote path field',
      desc: 'The field also takes a path, completed from that machine\'s folders as you type (the same completion Add Project uses). A machine chip picks where.',
      good: 'Any folder, on any machine.', cost: 'Two kinds of input in one field.',
      mock: () => withSidebar(`<div class="pop" style="left:200px;top:38px;width:360px;padding:6px"><div class="row g1" style="margin-bottom:4px"><span class="chip" style="height:32px">${ic('server', 'xs')}Devbox 1${ic('chev-down', 'xs')}</span><div class="field focus grow mono-font" style="font-size:12px">~/w/inf<span class="cursor" style="height:14px;width:1.5px"></span></div></div>${['~/w/infra', '~/w/infra-old', '~/w/inflight'].map((p, i) => pickerRow(ic('folder', 'sm mu'), p, i === 0 ? 'git: main' : '', { on: i === 0 })).join('')}<div class="hint" style="padding:6px 8px 2px">⇥ completes · ↩ opens a workspace there</div></div>`),
    },
    {
      key: 'C', name: 'A dialog with tabs', from: 't3code New Thread',
      desc: 'A centered dialog with Projects, Folders, Worktrees and Recent tabs, and a machine picker in its header.',
      good: 'Room for everything; easy to scan.', cost: 'Heavier than a popover.',
      mock: () => withSidebar(`<div class="modal-back"></div><div class="pop" style="left:170px;top:50px;width:420px;padding:12px;z-index:21"><div class="row g2" style="margin-bottom:10px"><span class="b5">New Workspace</span><span class="grow"></span><span class="chip">${ic('laptop', 'xs')}This Mac${ic('chev-down', 'xs')}</span></div><div class="seg" style="margin-bottom:8px"><span class="on">Projects</span><span>Folders</span><span>Worktrees</span><span>Recent</span></div><div class="field" style="margin-bottom:6px">${ic('search', 'sm mu')}<span class="ph">Search…</span></div>${pickerRow(mono('ST', 'g'), 'storefront', 'checkout-flow', { on: true })}${pickerRow(mono('DS', 'p'), 'design-system', 'tokens')}${pickerRow(mono('BL', 'o'), 'blog', 'main')}</div>`),
    },
    {
      key: 'D', name: 'Where, then how', from: 'zellij new-session layouts',
      desc: 'After the folder, a second step asks how to start: a shell, an agent CLI beside a shell, or a saved layout. Enter keeps "Shell".',
      good: 'Start an agent workspace in one go.', cost: 'One more step every time (Enter skips).',
      mock: () => withSidebar(`<div class="pop" style="left:200px;top:38px;width:340px;padding:6px"><div class="row g2 sm" style="padding:4px 6px 8px">${ic('chev-right', 'xs mu')}<span class="mu">storefront</span><span class="faint">/</span><span>Start with</span></div>${[['terminal', 'Shell', '↩', true], ['claude', 'Claude Code + shell', '1'], ['codex', 'Codex + shell', '2'], ['layers', 'Layout: dev (3 panes)', '3']].map(([g, t, k, on]) => `<div class="row g2" style="height:30px;padding:0 8px;border-radius:6px;font-size:13px;${on ? 'background:var(--hov)' : ''}">${GLYPHS[g] ? glyph(g, 'sm') : ic(g, 'sm mu')}<span class="grow">${t}</span>${keycap(k)}</div>`).join('')}</div>`),
    },
    {
      key: 'E', name: 'From a thread in Agents', from: 'new (reverse of "Open in Agents")',
      desc: 'A new entry point: the thread menu in the Agents view gets Open in Workspaces, which opens (or reuses) a workspace in the thread\'s checkout with the thread in a pane beside a shell.',
      good: 'Go from chatting to hands-on in one click.', cost: 'Doesn\'t change the + picker.',
      mock: () => frame(`<div class="col" style="height:100%"><div class="phead" style="height:36px;background:var(--panel);color:var(--t);font-size:13px">${mono('ST', 'g')}<span class="mu">storefront</span><span class="faint">/</span><span class="b5">Checkout page with pay button</span>${ic('chev-down', 'xs mu')}<span class="grow"></span><span class="chip">${ic('worktree', 'xs')}brave-otter</span>${ibtn('terminal')}${ibtn('more')}</div><div style="flex:1;background:var(--ed)"></div></div><div class="menu" style="left:180px;top:36px">${it('pencil', 'Rename')}${it('', 'Continue with Another Agent', ic('chev-right', 'xs'))}<div class="it hl">${ic('layers', 'sm')}Open in Workspaces</div>${hrr}${it('', 'Archive')}${it('', 'Delete…')}</div>`, { w: 760, h: 300 }),
    },
  ],
});

// 14. Layouts and templates --------------------------------------------------------------------------
TOPICS.push({
  id: 'layouts', section: 'Creating', title: 'Layouts and templates', size: 'medium', rec: 'A',
  now: 'Every new tab is one shell; you build splits by hand each time. herdr and t3code leave this to plugins and project scripts; zellij and tmuxinator have layout files.',
  options: [
    {
      key: 'A', name: 'Built-in layouts on +', from: 'zellij',
      desc: 'The tab bar\'s + gets a chevron with a few layouts: Shell, Side by side, Agent + shell, Agent + two shells, Grid. The agent is the last one you used.',
      good: 'Common setups in one click, nothing to configure.', cost: 'Fixed set.',
      mock: () => frame(`<div class="col" style="height:100%">${tabBar(undefined, { end: `${ibtn('plus')}${ibtn('chev-down', 'sm on')}` })}<div class="tile" style="flex:1">${agentsTab({ claude: { buttons: false }, shell: { buttons: false }, dev: { buttons: false } })}</div></div><div class="menu" style="right:6px;top:38px">${[['terminal', 'Shell', '⌘T'], ['split', 'Side by Side'], ['sidebar', 'Claude Code + Shell'], ['cols3', 'Claude Code + 2 Shells'], ['grid', 'Grid of 4']].map(([i, t, k], n) => `<div class="it ${n === 2 ? 'hl' : ''}">${ic(i, 'sm')}${t}${k ? `<span class="kb">${k}</span>` : ''}</div>`).join('')}</div>`, { w: 640, h: 300 }),
    },
    {
      key: 'B', name: 'Save a tab as a layout', from: 'iTerm2 arrangements',
      desc: 'A tab\'s menu gets Save Layout…: name it, and choose which panes keep their command (npm run dev, claude). Saved layouts join the + menu.',
      good: 'Your own setups, made by doing.', cost: 'Kept per machine; not shared.',
      mock: () => frame(`<div class="col" style="height:100%">${tabBar()}<div class="tile" style="flex:1">${agentsTab({ claude: { buttons: false }, shell: { buttons: false }, dev: { buttons: false } })}</div></div><div class="modal-back"></div><div class="pop" style="left:150px;top:50px;width:340px;padding:14px;z-index:21;display:flex;flex-direction:column;gap:10px"><span class="b5">Save Layout</span><div class="field focus">dev<span class="cursor" style="height:15px;width:1.5px"></span></div><div class="col" style="gap:4px;font-size:13px">${[['claude', 'Claude Code', true], ['terminal', 'zsh', false], ['terminal', 'npm run dev', true]].map(([g, t, on]) => `<div class="row g2"><span style="width:14px;height:14px;border-radius:3px;border:1px solid var(--b);background:${on ? 'var(--ac)' : 'none'};display:grid;place-items:center;color:#1b1f26">${on ? ic('check', 'xs') : ''}</span>${g === 'claude' ? glyph('claude', 'sm') : ic('terminal', 'sm mu')}<span class="grow">${t}</span><span class="xs faint">${on ? 'runs again' : 'plain shell'}</span></div>`).join('')}</div><div class="row g2"><span class="grow"></span><span class="btn">Cancel</span><span class="btn primary">Save</span></div></div>`, { w: 640, h: 330 }),
    },
    {
      key: 'C', name: 'A layout file in the repo', from: 't3code t3.json scripts + zellij layouts',
      desc: 'A checked-in <code>.agentz/layouts.json</code> describes tabs, splits and commands. The workspace menu offers Apply Layout ▸ with them, and teammates get the same setups.',
      good: 'Shareable, reviewable, per project.', cost: 'A file format to define and document.',
      mock: () => frame(`<div class="row" style="height:100%;align-items:stretch"><div class="term" style="background:var(--ed);font-size:11.5px;line-height:16px;padding:12px">${C('faint', '// .agentz/layouts.json')}\n{\n  ${C('tr', '"dev"')}: {\n    ${C('tr', '"tabs"')}: [{\n      ${C('tr', '"name"')}: ${C('tg', '"agents"')},\n      ${C('tr', '"split"')}: ${C('tg', '"right"')}, ${C('tr', '"ratio"')}: ${C('ty', '0.55')},\n      ${C('tr', '"panes"')}: [\n        { ${C('tr', '"run"')}: ${C('tg', '"claude"')} },\n        { ${C('tr', '"run"')}: ${C('tg', '"npm run dev"')} }\n      ]\n    }]\n  }\n}</div><div style="width:320px;position:relative;background:var(--panel);border-left:1px solid var(--b)"><div class="menu" style="left:10px;top:30px">${it('plus', 'New Tab', '⌘T')}<div class="it hl">${ic('layers', 'sm')}Apply Layout<span class="kb">${ic('chev-right', 'xs')}</span></div>${it('pencil', 'Rename')}</div><div class="menu" style="left:150px;top:60px;min-width:150px">${it('', 'dev')}${it('', 'review')}${hrr}${it('file', 'Edit layouts.json')}</div></div></div>`, { w: 640, h: 300 }),
    },
    {
      key: 'D', name: 'Arrange what\'s there', from: 'tmux select-layout',
      desc: 'No templates; instead a tab\'s menu offers Arrange ▸ Even Columns, Even Rows, Main + Stack, Grid, which re-tile its current panes.',
      good: 'Tidies messy splits instantly.', cost: 'Doesn\'t start anything for you.',
      mock: () => frame(`<div class="col" style="height:100%">${tabBar()}<div class="tile" style="flex:1">${split('h', 0.34, pane({ glyphKind: 'claude', title: 'Claude Code', body: SCREENS.claude, buttons: false }), split('h', 0.5, pane({ title: 'zsh', body: SCREENS.shellIdle, buttons: false }), pane({ title: 'npm run dev', body: SCREENS.dev, buttons: false })))}</div></div><div class="menu" style="left:30px;top:38px">${it('pencil', 'Rename')}${it('plus', 'New Tab', '⌘T')}<div class="it hl">${ic('grid', 'sm')}Arrange<span class="kb">${ic('chev-right', 'xs')}</span></div>${hrr}${it('x', 'Close Tab')}</div><div class="menu" style="left:245px;top:90px;min-width:170px"><div class="it hl">${ic('cols3', 'sm')}Even Columns</div>${it('panel-bottom', 'Even Rows')}${it('sidebar', 'Main + Stack')}${it('grid', 'Grid')}</div>`, { w: 640, h: 300 }),
    },
    {
      key: 'E', name: 'Remember per folder', from: 'new (herdr restores sessions)',
      desc: 'No template UI: a new workspace in a folder you\'ve used before reopens its last tabs and panes, rerunning the commands they ran. A toast offers Undo.',
      good: 'Zero setup; just works the second time.', cost: 'Surprising if you wanted a clean start.',
      mock: () => frame(`<div class="col" style="height:100%">${tabBar([['agents', 'working'], ['server'], ['3']])}<div class="tile" style="flex:1">${agentsTab({ claude: { buttons: false }, shell: { buttons: false }, dev: { buttons: false } })}</div></div><div class="toast" style="right:12px;bottom:12px"><div class="row g2">${ic('history', 'sm mu')}<span class="grow">Restored 3 tabs from last time</span><span class="btn sm">Undo</span></div></div>`, { w: 640, h: 300 }),
    },
  ],
});

// 15. Worktrees --------------------------------------------------------------------------------
const nestedList = (child) => SPACES.slice(0, 1).map((s) => currentRow(s, { active: true })).join('') + child + SPACES.slice(2).map((s) => currentRow(s)).join('');
const childRow = (s, extra = '', { on = false, below = '' } = {}) => `<div class="srow ${on ? 'on' : ''}" style="padding-left:28px;position:relative"><span style="position:absolute;left:20px;top:-4px;width:8px;height:18px;border-left:1px solid var(--b);border-bottom:1px solid var(--b);border-bottom-left-radius:4px"></span><div class="l1"><span class="mu" style="display:inline-flex">${ic(s.kind === 'pasture' ? 'pasture' : 'worktree', 'sm')}</span><span class="grow trunc mu">${s.name}</span>${pill(s.state)}</div><div class="l2"><span class="trunc">${s.branch}</span>${extra}</div>${below}</div>`;
TOPICS.push({
  id: 'worktrees', section: 'Creating', title: 'Worktrees', size: 'medium', rec: 'A',
  now: 'A git workspace\'s menu has New Worktree (a dialog: branch name, Worktree or Pasture, "the branch starts from … · Enter makes it and opens a terminal there") and Open Worktree… (the repository\'s other checkouts). Either opens a separate workspace.',
  issues: ['The new workspace isn\'t linked to the one it came from.', 'No way to delete a checkout from here; Project Settings › Checkouts only covers projects.'],
  options: [
    {
      key: 'A', name: 'Grouped, with Delete Checkout', from: 'herdr worktrees',
      desc: 'herdr\'s model: a worktree opens under its source workspace; its menu adds Delete Worktree Checkout…, which runs <code>git worktree remove</code>, asks again before forcing if files changed, and keeps the branch.',
      good: 'Complete lifecycle, safe by default.', cost: 'None beyond herdr\'s.',
      mock: () => withSidebar(`<div class="modal-back"></div><div class="pop" style="left:200px;top:110px;width:380px;padding:16px;z-index:21;display:flex;flex-direction:column;gap:10px"><span class="b5">Delete the brave-otter checkout?</span><span class="sm mu">Removes <span class="mono-font">~/…/worktrees/storefront/brave-otter</span>. The branch <span class="mono-font">agentz/brave-otter-3fa</span> stays.</span><div class="row g2 sm warnc">${ic('alert', 'sm')}2 files have uncommitted changes, which will be lost.</div><div class="row g2"><span class="grow"></span><span class="btn">Cancel</span><span class="btn" style="background:var(--del);border-color:var(--del);color:#1b1f26">Delete Anyway</span></div></div>`, { list: nestedList(childRow(SPACES[1])) }),
    },
    {
      key: 'B', name: 'Make it inline', from: 'Zed project panel new file',
      desc: 'New Worktree adds an editable row under the workspace with a generated branch name selected and a Worktree | Pasture toggle; Enter makes it, Escape cancels.',
      good: 'No dialog; you see where it will appear.', cost: 'Cramped for errors and options.',
      mock: () => withSidebar('', { list: nestedList(`<div class="srow" style="padding-left:28px"><div class="l1"><span class="mu">${ic('worktree', 'sm')}</span><div class="field focus grow" style="height:24px;font-size:12px;padding:0 6px"><span style="background:#3d5577">agentz/calm-heron-7c2</span></div></div><div class="row g2" style="padding:4px 0 0 26px"><span class="seg"><span class="on">Worktree</span><span>Pasture</span></span></div></div>` + childRow(SPACES[1])) }),
    },
    {
      key: 'C', name: 'A fuller dialog', from: 't3code new worktree',
      desc: 'Today\'s dialog plus: the base branch as a picker, what each kind means in one line, what to open in it (Shell, Claude Code, Codex), and where it will live.',
      good: 'Everything decided up front.', cost: 'More to read for a quick worktree.',
      mock: () => withSidebar(`<div class="modal-back"></div><div class="pop" style="left:170px;top:30px;width:420px;padding:16px;z-index:21;display:flex;flex-direction:column;gap:10px;font-size:13px"><span class="b5">New worktree of storefront</span><div class="field focus mono-font" style="font-size:12px">agentz/calm-heron-7c2</div><div class="row g2"><span class="mu" style="width:70px">From</span><span class="chip">${ic('branch', 'xs')}checkout-flow${ic('chev-down', 'xs')}</span></div><div class="row g2" style="align-items:flex-start"><span class="mu" style="width:70px;padding-top:4px">Kind</span><div class="col g1 grow"><div class="card" style="padding:6px 8px;border-color:var(--ac)"><div class="row g2">${ic('worktree', 'sm')}Worktree</div><span class="xs faint">git worktree: shares the repository, needs installs</span></div><div class="card" style="padding:6px 8px"><div class="row g2">${ic('pasture', 'sm')}Pasture</div><span class="xs faint">instant copy-on-write copy, node_modules included</span></div></div></div><div class="row g2"><span class="mu" style="width:70px">Open with</span><span class="seg"><span>Shell</span><span class="on">Claude Code</span><span>Codex</span></span></div><div class="hint">In ~/Library/…/agentZ/worktrees/storefront/calm-heron-7c2</div><div class="row g2"><span class="grow"></span><span class="btn">Cancel</span><span class="btn primary">Create</span></div></div>`, { h: 500 }),
    },
    {
      key: 'D', name: 'Switch from the branch chip', from: 'Zed branch picker',
      desc: 'The header\'s branch chip (see tabs B) lists the repository\'s checkouts: the main one, worktrees and pastures, each Open or not, and New Worktree… at the bottom.',
      good: 'Worktrees live where the branch is shown.', cost: 'Depends on the breadcrumb header.',
      mock: () => frame(`<div class="col" style="height:100%">${tabBar([['agents', 'working'], ['server'], ['3']], { lead: `<div class="row g2" style="padding:0 12px;border-right:1px solid var(--b)">${mono('ST', 'g')}<span class="sm">storefront</span><span class="faint">/</span><span class="chip" style="height:20px;background:var(--sel);color:var(--t)">${ic('branch', 'xs')}checkout-flow${ic('chev-down', 'xs')}</span></div>` })}<div style="flex:1;background:var(--ed)"></div></div><div class="pop" style="left:110px;top:36px;width:330px;padding:6px"><div class="field" style="margin-bottom:4px">${ic('search', 'sm mu')}<span class="ph">Switch checkout…</span></div>${pickerRow(ic('check', 'sm ac'), 'checkout-flow', 'main checkout', { tag: openTag })}${pickerRow(ic('worktree', 'sm mu'), 'agentz/brave-otter-3fa', 'worktree', { on: true, tag: openTag })}${pickerRow(ic('pasture', 'sm mu'), 'agentz/sunny-lark-91d', 'pasture')}<div class="hr" style="height:1px;background:var(--bv);margin:4px 0"></div>${pickerRow(ic('plus', 'sm mu'), 'New Worktree…', '')}</div>`, { w: 640, h: 300 }),
    },
    {
      key: 'E', name: 'Pastures first, with sync', from: 'cow',
      desc: 'New Worktree defaults to a pasture (an instant copy with node_modules). A pasture\'s row shows how far it\'s drifted and has Sync and Bring Back, cow\'s sync and extract.',
      good: 'Fast to make; easy to keep current.', cost: 'APFS only on macOS; full copies on ext4.',
      mock: () => withSidebar('', { list: nestedList(childRow({ ...SPACES[1], name: 'sunny-lark', kind: 'pasture', branch: 'agentz/sunny-lark-91d', state: 'done' }, '<span class="okc">↑3</span>', { below: `<div class="row g1" style="padding:5px 0 0 26px"><span class="btn sm" style="height:20px">${ic('restart', 'xs')}Sync</span><span class="btn sm" style="height:20px">${ic('merge', 'xs')}Bring Back</span><span class="xs faint">3 commits to bring back</span></div>` }) + childRow(SPACES[1])) }),
    },
  ],
});
