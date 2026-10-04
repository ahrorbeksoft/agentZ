// Navigation, attention, and feature menus.

const fullWin = (overlay = '', { w = 1000, h = 560, tabsExtra = {}, panes = agentsTab(), mainExtra = '', titleExtra = '', viewBadge = '', list = currentList(), agents = currentAgents() } = {}) =>
  win(`${sidebar({ list, agents })}${main(tabBar(undefined, tabsExtra), panes, mainExtra)}${overlay}`, { w, h, titleExtra, viewBadge });
const back = '<div class="modal-back" style="top:34px"></div>';

// 16. Jump to anything ---------------------------------------------------------------------
TOPICS.push({
  id: 'goto', section: 'Navigation and attention', title: 'Jump to anything', size: 'wide', rec: 'A',
  now: 'You get around with the sidebar and the Agents list, Cmd-} / Cmd-{ for tabs and Cmd-Option-arrows between panes. There\'s no search across panes, and no way to jump to tab 3 or workspace 2 directly.',
  options: [
    {
      key: 'A', name: 'herdr\'s Go To list', from: 'herdr Goto picker',
      desc: 'Cmd-P opens every agent and terminal as its own row, grouped by workspace. Type to search names, agents, tabs, branches and paths; filter by state; the selected row\'s location shows below.',
      good: 'Proven in herdr; fast with many workspaces.', cost: 'One more shortcut to learn.',
      mock: () => {
        const row = (st, g, title, tab, on) => `<div class="row g2" style="height:28px;padding:0 10px 0 24px;border-radius:6px;font-size:13px;${on ? 'background:var(--hov)' : ''}"><span style="width:6px">${dot(st)}</span>${g === 'shell' ? ic('terminal', 'sm mu') : glyph(g, 'sm')}<span class="grow trunc">${title}</span><span class="xs faint">${tab}</span></div>`;
        const ws = (s) => `<div class="row g2 xs mu" style="padding:8px 10px 2px">${s.mono ? mono(s.mono, s.color) : ic('folder', 'xs')}${s.name}<span class="faint">${s.branch || s.path}</span></div>`;
        return fullWin(`${back}<div class="pop" style="left:250px;top:70px;width:500px;padding:8px;z-index:21"><div class="field focus">${ic('search', 'sm mu')}<span>cod</span><span class="cursor" style="height:15px;width:1.5px"></span><span class="grow"></span><span class="row g1 xs mu"><span class="kbd">b</span>needs you<span class="kbd">w</span>working<span class="kbd">d</span>done</span></div>${ws(SPACES[0])}${row('working', 'claude', 'Claude Code', 'agents')}${row(null, 'shell', 'zsh', 'agents')}${row(null, 'shell', 'npm run dev', 'server')}${ws(SPACES[1])}${row('pending', 'codex', 'Codex', '1', true)}${ws(SPACES[2])}${row('awaiting', 'thread', 'Fix flaky login test', 'review')}<div class="row g2 hint" style="border-top:1px solid var(--bv);margin-top:6px;padding:8px 10px 2px">${ic('worktree', 'xs')}brave-otter › 1 · ~/…/worktrees/storefront/brave-otter</div></div>`);
      },
    },
    {
      key: 'B', name: 'Command palette', from: 'Zed / t3code command palette',
      desc: 'Cmd-K mixes places (workspaces, tabs, panes) with actions (Split Right, New Worktree…, Rename Tab), each with its shortcut.',
      good: 'One box for everything; teaches shortcuts.', cost: 'Results mix two kinds of things.',
      mock: () => fullWin(`${back}<div class="pop" style="left:250px;top:70px;width:500px;padding:8px;z-index:21"><div class="field focus">${ic('command', 'sm mu')}<span>split</span><span class="cursor" style="height:15px;width:1.5px"></span></div>${sub('Actions')}${[['split', 'Split Right', '⌘D', true], ['split-v', 'Split Down', '⌘⇧D'], ['grid', 'Arrange: Even Columns', '']].map(([i, t, k, on]) => `<div class="row g2" style="height:28px;padding:0 10px;border-radius:6px;font-size:13px;${on ? 'background:var(--hov)' : ''}">${ic(i, 'sm mu')}<span class="grow">${t}</span>${k ? `<span class="kbd">${k}</span>` : ''}</div>`).join('')}${sub('Places')}${[['terminal', 'npm run dev', 'storefront › server'], ['codex', 'Codex', 'brave-otter › 1']].map(([g, t, w]) => `<div class="row g2" style="height:28px;padding:0 10px;font-size:13px">${GLYPHS[g] ? glyph(g, 'sm') : ic(g, 'sm mu')}<span class="grow">${t}</span><span class="xs faint">${w}</span></div>`).join('')}</div>`),
    },
    {
      key: 'C', name: 'Overview of every pane', from: 'macOS Mission Control',
      desc: 'A shortcut zooms out to live thumbnails of every tab in every workspace, grouped by workspace, states on their borders. Click or arrow + Enter to go.',
      good: 'See everything at once, visually.', cost: 'Streams every screen while open; heavy for remote machines.',
      mock: () => {
        const thumb = (lines, title, st, w = 190) => `<div class="col g1" style="width:${w}px"><div style="height:110px;border-radius:6px;overflow:hidden;border:1.5px solid ${st === 'pending' ? 'var(--warn)' : st === 'working' ? 'var(--ac)' : st === 'awaiting' ? 'var(--pur)' : 'var(--b)'};display:flex"><div class="term" style="font-size:5.5px;line-height:7.5px;padding:4px">${lines.join('\n')}</div></div><span class="xs mu row g1">${dot(st)}${title}</span></div>`;
        return win(`<div class="col" style="flex:1;padding:18px 24px;gap:14px;background:#1d2026;overflow:hidden">${[[SPACES[0], [[SCREENS.claude, 'agents', 'working'], [SCREENS.dev, 'server'], [SCREENS.shellIdle, '3']]], [SPACES[1], [[SCREENS.codex, '1', 'pending']]], [SPACES[2], [[SCREENS.opencode, '1', 'done'], [SCREENS.logs, 'review', 'awaiting']]]].map(([s, tabs]) => `<div class="col g1"><span class="row g2 sm">${s.mono ? mono(s.mono, s.color) : ''}${s.name}<span class="xs faint">${s.branch}</span></span><div class="row g3">${tabs.map(([l, t, st]) => thumb(l, t, st)).join('')}</div></div>`).join('')}</div>`, { w: 1000, h: 560 });
      },
    },
    {
      key: 'D', name: 'Numbers on panes', from: 'tmux display-panes',
      desc: 'Cmd-; puts a big number on each pane of the tab and a letter on each tab; type one to go there. Cmd-1…9 still switch tabs.',
      good: 'Two keystrokes to any visible pane.', cost: 'Only covers what\'s on screen.',
      mock: () => {
        const n = (k, l, t) => `<span style="position:absolute;z-index:20;left:${l}px;top:${t}px;width:54px;height:54px;border-radius:12px;background:rgba(116,173,232,.9);color:#1b1f26;font:600 30px/54px 'IBM Plex Sans';text-align:center;box-shadow:0 8px 24px rgba(0,0,0,.5)">${k}</span>`;
        const l = (k, left) => `<span style="position:absolute;z-index:20;left:${left}px;top:42px;width:20px;height:20px;border-radius:5px;background:var(--warn);color:#1b1f26;font:600 12px/20px 'IBM Plex Sans';text-align:center">${k}</span>`;
        return fullWin(n(1, 490, 230) + n(2, 820, 140) + n(3, 820, 400) + l('a', 360) + l('s', 430) + l('d', 470));
      },
    },
    {
      key: 'E', name: 'Next that needs you', from: 'new (herdr attention states)',
      desc: 'Cmd-. jumps to the next agent waiting for approval or input, then to finished ones, cycling through workspaces. A small HUD says where you landed and how many are left.',
      good: 'Triage without looking anywhere: press, act, press.', cost: 'Only for attention, not general navigation.',
      mock: () => fullWin(`<div class="pop row g2" style="left:470px;top:84px;padding:8px 12px;z-index:21;font-size:13px">${dot('pending')}${glyph('codex', 'sm')}<span>Codex</span><span class="mu">brave-otter › 1</span><span class="xs faint" style="margin-left:6px">1 of 3</span><span class="kbd">⌘.</span><span class="xs faint">next</span></div>`, { list: SPACES.map((s) => currentRow(s, { active: s.id === 'bo' })).join(''), panes: split('h', 0.5, pane({ glyphKind: 'codex', title: 'Codex', detail: 'brave-otter', state: 'pending', focus: true, body: SCREENS.codex }), pane({ title: 'zsh', detail: 'brave-otter', body: SCREENS.shell })), tabsExtra: {} }),
    },
  ],
});
// Option E shows brave-otter's own tab.
TOPICS[TOPICS.length - 1].options[4].mock = ((original) => () => original().replace(tabBar(), tabBar([['1', 'pending']])))(TOPICS[TOPICS.length - 1].options[4].mock);

// 17. Attention ----------------------------------------------------------------------------
TOPICS.push({
  id: 'attention', section: 'Navigation and attention', title: 'Getting your attention', size: 'wide', rec: 'B',
  now: 'States show as the row\'s status, a dot on the tab and the pane header, and in the Agents list. ACP threads also send macOS notifications when you aren\'t looking; agent CLIs in panes don\'t.',
  issues: ['In the Agents view you don\'t see that a pane in Workspaces is waiting, and the other way round.'],
  options: [
    {
      key: 'A', name: 'Notify for pane agents too', from: 'herdr system toasts + sounds',
      desc: 'Agent CLIs in panes send the same macOS notifications as threads ("Waiting for approval", "Finished"), with herdr\'s per-agent sound choice. Clicking one opens the pane.',
      good: 'Works when the app is in the background.', cost: 'Notifications need the app bundle; can get noisy.',
      mock: () => fullWin(`<div class="pop" style="right:14px;top:44px;width:330px;padding:10px 12px;z-index:21;border-radius:14px;background:rgba(58,62,70,.96);font:13px -apple-system,sans-serif"><div class="row g2"><span style="width:20px;height:20px;border-radius:5px;background:#3d4a5c;display:grid;place-items:center;color:var(--ac);font-weight:700;font-size:11px">Z</span><span class="b6 grow">agentZ</span><span class="xs faint">now</span></div><div style="margin:6px 0 0 28px"><div class="b6">Codex needs approval</div><div class="mu sm">brave-otter › 1 · Allow command? cargo clippy --fix</div></div></div>`, { titleExtra: '' }),
    },
    {
      key: 'B', name: 'In-app toasts', from: 'herdr in-app toasts',
      desc: 'Bottom-right toasts while the app is open, for anything not on screen: waiting ones stay until handled or dismissed, finished ones fade. Go jumps there.',
      good: 'Seen in either view; one click to act.', cost: 'Covers a corner of the panes.',
      mock: () => fullWin(`<div class="col g2" style="position:absolute;right:14px;bottom:14px;z-index:22">${[['pending', 'codex', 'Codex needs approval', 'brave-otter › 1', true], ['awaiting', 'thread', 'Fix flaky login test needs input', 'Devbox 1 · api › review', true], ['done', 'opencode', 'OpenCode finished', 'Devbox 1 · api › 1', false]].map(([s, g, t, w, go]) => `<div class="toast" style="position:static"><div class="row g2">${dot(s)}${glyph(g, 'sm')}<div class="col grow" style="min-width:0;line-height:1.25"><span class="trunc">${t}</span><span class="xs faint trunc">${w}</span></div>${go ? '<span class="btn sm">Go</span>' : ''}${ic('x', 'xs mu')}</div></div>`).join('')}</div>`),
    },
    {
      key: 'C', name: 'Badges on the view switch', from: 'macOS dock badges',
      desc: 'The Agents | Workspaces switch shows a count on the view you\'re not in when something there needs you, colored by the most urgent state.',
      good: 'Tiny, always visible, no interruptions.', cost: 'Says how many, not who.',
      mock: () => {
        const html = fullWin('', { viewBadge: '' });
        return html.replace('<div class="viewtabs"><span>Agents</span>', `<div class="viewtabs"><span>Agents <span style="display:inline-grid;place-items:center;min-width:16px;height:16px;border-radius:8px;background:var(--warn);color:#1b1f26;font-size:10px;font-weight:700;margin-left:4px;padding:0 4px">2</span></span>`) + '';
      },
    },
    {
      key: 'D', name: 'Glow where it\'s waiting', from: 'new',
      desc: 'A pane waiting on you gets a soft pulsing border in its state color; its tab gets the same color underline, so you spot it in a crowded tab bar or grid.',
      good: 'Points at the exact pane.', cost: 'Only helps for what\'s on screen.',
      mock: () => fullWin('', { panes: split('h', 0.5, pane({ glyphKind: 'claude', title: 'Claude Code', detail: '~/storefront', state: 'working', focus: true, body: SCREENS.claude }), pane({ glyphKind: 'codex', title: 'Codex', detail: 'brave-otter', state: 'pending', body: SCREENS.codex, style: 'border:1px solid var(--warn);box-shadow:0 0 0 3px rgba(222,193,132,.18) inset' })), tabsExtra: {} }).replace('<div class="tab on">', '<div class="tab on" style="box-shadow:inset 0 -2px 0 var(--warn)">'),
    },
    {
      key: 'E', name: 'An inbox', from: 'GitHub notifications',
      desc: 'A bell in the sidebar header with a count; it opens a list of what happened (asked for approval, finished, failed) across both views, with Go and Mark All Read.',
      good: 'Nothing lost while you were away.', cost: 'One more place to check.',
      mock: () => fullWin(`<div class="pop" style="left:120px;top:72px;width:330px;padding:6px;z-index:21"><div class="row g2" style="padding:4px 8px 6px"><span class="b5 grow">Inbox</span><span class="xs ac">Mark All Read</span></div>${AGENTS.slice(1).concat([AGENTS[0]]).map((a, i) => `<div class="row g2" style="padding:6px 8px;border-radius:6px;${i === 0 ? 'background:var(--hov)' : ''}">${dot(a.state)}${glyph(a.kind, 'sm')}<div class="col grow" style="min-width:0;line-height:1.25"><span class="sm trunc">${a.title}: ${a.state === 'pending' ? 'needs approval' : a.state === 'awaiting' ? 'needs input' : a.state === 'done' ? 'finished' : 'started working'}</span><span class="xs faint">${a.space} › ${a.tab} · ${a.ago}</span></div></div>`).join('')}</div>`).replace('<div class="sb-head">', `<div class="sb-head">`).replace(`${ibtn('plus')}</div><div class="sb-list">`, `<span class="ibtn on">${ic('bell', 'sm')}<span class="badge-dot" style="background:var(--warn)"></span></span>${ibtn('plus')}</div><div class="sb-list">`),
    },
  ],
});

// 18. Terminal pane features (pick any) --------------------------------------------------------
const termPane = (body, { head, h = 300, extra = '' } = {}) => area(`<div class="tile" style="flex:1;position:relative">${pane({ title: 'npm run dev', detail: '~/storefront', focus: true, body, head })}${extra}</div>`, { w: 640, h });
TOPICS.push({
  id: 'termfeat', section: 'Navigation and attention', title: 'Terminal pane features', type: 'multi', size: 'medium', rec: 'ABE',
  now: 'Panes run Zed\'s terminal element: selection, mouse, IME, font size. Zed\'s terminal also has find; herdr has copy mode and search; t3code finds local servers. None of these are in agentZ\'s panes yet.',
  options: [
    {
      key: 'A', name: 'Open ports as links', from: 't3code discovered servers',
      desc: 'When something in a pane starts listening (a dev server), its header shows the port; a click opens it in the browser, forwarded over SSH for remote machines (the login code already does this).',
      good: 'Dev servers on remote machines just work.', cost: 'The server has to watch listening sockets per pane.',
      mock: () => termPane(SCREENS.dev, { head: `<div class="phead" style="background:var(--ed);color:var(--t)">${ic('terminal', 'sm')}npm run dev<span class="mu">~/storefront</span><span class="chip" style="height:20px;color:var(--ac);border-color:rgba(116,173,232,.4)">${ic('globe', 'xs')}localhost:3000${ic('external', 'xs')}</span><span class="grow"></span>${btns('split', 'maximize', 'x')}</div>` }),
    },
    {
      key: 'B', name: 'Find in the terminal', from: 'Zed terminal search',
      desc: 'Cmd-F opens a find bar in the pane: matches highlighted in the screen and scrollback, a count, Enter for next.',
      good: 'Find that error in 5,000 lines.', cost: 'Scrollback lives on the server; search runs there.',
      mock: () => termPane(SCREENS.dev.map((l) => l.replace('Compiled', '<span style="background:rgba(222,193,132,.5);color:#fff">Compiled</span>').replace('Compiling', '<span style="background:rgba(222,193,132,.25)">Compiling</span>')), { extra: `<div class="pop row g2" style="right:10px;top:42px;padding:4px 6px;z-index:5"><div class="field focus" style="height:26px;width:180px;font-size:12px">Compil</div><span class="xs mu">2 of 7</span>${ibtn('chev-up', 'sm')}${ibtn('chev-down', 'sm')}${ibtn('x', 'sm')}</div>` }),
    },
    {
      key: 'C', name: 'Command marks', from: 'Ghostty / iTerm2 shell integration',
      desc: 'With shell integration, each command gets a mark in the gutter (✓ or ✗ with its exit code); Cmd-↑/↓ jump between prompts, and a hover offers Copy Output.',
      good: 'Navigate long sessions by command.', cost: 'Needs shell integration in zsh/bash/fish.',
      mock: () => termPane([`${prompt()}npm test`, ' PASS  src/cart.test.ts', ' FAIL  src/checkout.test.ts', '   ✕ shows the pay button (14 ms)', `${prompt()}npm run lint`, ' ✓ No problems', `${prompt()}<span class="cursor"></span>`], { extra: `<div style="position:absolute;left:2px;top:44px;z-index:3;display:flex;flex-direction:column;gap:0;font-size:9px">${[['var(--del)', 0], ['var(--ok)', 68]].map(([c, t]) => `<span style="position:absolute;top:${t}px;width:4px;height:${t ? 34 : 68}px;border-radius:2px;background:${c}"></span>`).join('')}</div><div class="pop row g1" style="left:180px;top:58px;padding:2px 4px;z-index:5;font-size:11px"><span class="delc" style="padding:0 4px">exit 1 · 2.4s</span><span class="btn sm" style="height:20px">${ic('copy', 'xs')}Copy Output</span></div>` }),
    },
    {
      key: 'D', name: 'Type into every pane', from: 'tmux synchronize-panes',
      desc: 'Broadcast Input in a tab\'s menu sends what you type to all its shells at once (several machines, the same command). A bar and outlines make it impossible to miss.',
      good: 'Run the same thing in many places.', cost: 'Dangerous if forgotten, hence the loud bar.',
      mock: () => area(`<div class="row g2" style="height:28px;padding:0 12px;background:rgba(222,193,132,.15);border-bottom:1px solid var(--warn);font-size:12px" >${ic('broadcast', 'sm warnc')}<span class="warnc b5">Typing into 3 panes</span><span class="grow"></span><span class="btn sm">Stop</span></div><div class="tile" style="flex:1">${split('h', 0.5, pane({ title: 'zsh', detail: 'This Mac', body: [`${prompt()}git pull<span class="cursor"></span>`], style: 'border-color:var(--warn)' }), split('v', 0.5, pane({ title: 'zsh', detail: 'Devbox 1', body: [`${prompt('api', 'main')}git pull<span class="cursor"></span>`], style: 'border-color:var(--warn)' }), pane({ title: 'zsh', detail: 'Ahrorbek\'s Laptop', body: [`${prompt('api', 'main')}git pull<span class="cursor"></span>`], style: 'border-color:var(--warn)' })))}</div>`, { w: 640, h: 300 }),
    },
    {
      key: 'E', name: 'When a command ends', from: 'herdr pane exit + new',
      desc: 'A pane whose command exited keeps its output and shows a bar: how it ended, Run Again and Close. A pane started with a command can be set to restart on its own.',
      good: 'Crashed dev servers are one click from back.', cost: 'A bar over the last lines.',
      mock: () => termPane([...SCREENS.dev, ` ${C('tr', '⨯')} Error: listen EADDRINUSE: address already in use :::3000`], { extra: `<div class="row g2" style="position:absolute;left:1px;right:1px;bottom:1px;height:34px;padding:0 10px;background:var(--panel);border-top:1px solid var(--b);z-index:4;font-size:12px"><span class="delc row g1">${ic('alert', 'sm')}Exited with code 1</span><span class="faint">after 12m</span><span class="grow"></span><label class="row g1 xs mu"><span style="width:12px;height:12px;border:1px solid var(--b);border-radius:3px"></span>Restart automatically</label><span class="btn sm">${ic('restart', 'xs')}Run Again</span><span class="btn sm ghost">Close</span></div>` }),
    },
  ],
});

// 19. Workspace features (pick any) -----------------------------------------------------------------
TOPICS.push({
  id: 'spacefeat', section: 'Navigation and attention', title: 'Workspace features', type: 'multi', size: 'wide', rec: 'BCE',
  now: 'A workspace holds tabs and panes; closing is final; there\'s no place for scripts, notes or a list of shortcuts.',
  options: [
    {
      key: 'A', name: 'Status bar', from: 'herdr tab_bar_right / Zed status bar',
      desc: 'A thin bar under the panes: machine, branch with ↑↓, uncommitted changes, open ports, ZOOM while zoomed, and the focused pane\'s folder.',
      good: 'Context without the sidebar.', cost: '24px of height.',
      mock: () => fullWin('', { mainExtra: `<div class="row g3" style="height:26px;padding:0 10px;border-top:1px solid var(--b);background:var(--title);font-size:11.5px;color:var(--mu)"><span class="row g1">${ic('laptop', 'xs')}This Mac</span><span class="row g1">${ic('branch', 'xs')}checkout-flow <span class="okc">↑2</span></span><span class="row g1">${ic('diff', 'xs')}3 files <span class="okc">+24</span> <span class="delc">−3</span></span><span class="row g1 ac">${ic('globe', 'xs')}:3000</span><span class="grow"></span><span>~/storefront</span><span class="b6 ac" style="border:1px solid rgba(116,173,232,.4);border-radius:4px;padding:0 5px">ZOOM</span></div>` }),
    },
    {
      key: 'B', name: 'Reopen what you closed', from: 'Zed / browsers Cmd-Shift-T',
      desc: 'Cmd-Shift-T brings back the last closed pane, tab or workspace, with its scrollback (the server keeps closed terminals\' screens for a while). The + menu lists Recently Closed.',
      good: 'Accidental Cmd-W stops hurting.', cost: 'The process itself is gone; commands rerun.',
      mock: () => fullWin(`<div class="menu" style="right:6px;top:72px">${it('terminal', 'New Tab', '⌘T')}${it('layers', 'Layouts', ic('chev-right', 'xs'))}${hrr}<div class="lbl">Recently Closed</div><div class="it hl">${ic('undo', 'sm')}npm run test<span class="kb">⌘⇧T</span></div>${it('terminal', 'zsh · storefront/src')}${it('folder', 'Workspace: blog')}</div>`),
    },
    {
      key: 'C', name: 'Scripts as buttons', from: 't3code project scripts',
      desc: 'Named commands for a workspace (dev, test, lint), from t3code-style project scripts or added by hand, as buttons in the header. A click runs one in its own pane, or focuses it if it\'s running.',
      good: 'Your everyday commands, one click.', cost: 'Space in the header; a place to store them.',
      mock: () => fullWin('', { tabsExtra: { end: `<div class="row g1" style="padding-right:4px">${['dev', 'test', 'lint'].map((n, i) => `<span class="chip" style="height:24px;${i === 0 ? 'color:var(--ok);border-color:rgba(161,193,129,.4)' : ''}">${ic(i === 0 ? 'stop' : 'play', 'xs')}${n}</span>`).join('')}${ibtn('plus')}</div>` } }),
    },
    {
      key: 'D', name: 'A notes pane', from: 'new',
      desc: 'A pane type for a workspace\'s own markdown note: a checklist, context to paste to agents, commands to remember. Kept by the server with the workspace.',
      good: 'Context lives next to the work.', cost: 'A small editor to build.',
      mock: () => fullWin('', { panes: split('h', 0.6, pane({ glyphKind: 'claude', title: 'Claude Code', detail: '~/storefront', state: 'working', focus: true, body: SCREENS.claude }), pane({ icon: 'note', title: 'Notes', body: `<div style="flex:1;padding:12px 14px;font-size:13px;line-height:1.6;color:var(--t);background:var(--ed)"><div class="b6" style="font-size:15px;margin-bottom:4px">Checkout</div><div>☑ pay button</div><div>☐ error states</div><div>☐ Stripe test keys in <span class="mono-font" style="font-size:12px">.env.local</span></div><div class="mu" style="margin-top:8px">Ask Claude to use the existing <span class="mono-font" style="font-size:12px">useCart()</span> hook.</div></div>` })) }),
    },
    {
      key: 'E', name: 'Shortcut sheet', from: 'herdr prefix+? help',
      desc: 'Cmd-/ shows every Workspaces shortcut in an overlay, filterable as you type, as herdr\'s keybind help does.',
      good: 'Learn the keys without leaving.', cost: 'Small.',
      mock: () => fullWin(`${back}<div class="pop" style="left:260px;top:70px;width:480px;padding:12px;z-index:21"><div class="field focus" style="margin-bottom:8px">${ic('keyboard', 'sm mu')}<span class="ph">Filter shortcuts…</span></div><div style="display:grid;grid-template-columns:1fr 1fr;gap:4px 18px;font-size:13px">${[['New workspace', '⌘⇧N'], ['New tab', '⌘T'], ['Next / previous tab', '⌘} ⌘{'], ['Go to tab 1–9', '⌘1–9'], ['Split right', '⌘D'], ['Split down', '⌘⇧D'], ['Move between panes', '⌘⌥←↑→↓'], ['Zoom pane', '⌘⇧↩'], ['Close pane', '⌘W'], ['Toggle sidebar', '⌘B']].map(([a, k]) => `<div class="row"><span class="mu grow">${a}</span><span class="kbd">${k}</span></div>`).join('')}</div></div>`),
    },
  ],
});
