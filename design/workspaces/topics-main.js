// Main area topics.

const W_MAIN = 900;
const area = (html, { w = W_MAIN, h = 360, style = '' } = {}) => frame(`<div class="col" style="height:100%">${html}</div>`, { w, h, style });
const btns = (...names) => names.map((n) => ibtn(n)).join('');

// 7. Workspace header and tabs ---------------------------------------------------------
const tabsBody = (h = 150) => `<div class="tile" style="height:${h}px">${agentsTab({ shell: { buttons: false }, dev: { buttons: false }, claude: { buttons: false } })}</div>`;
TOPICS.push({
  id: 'tabs', section: 'Main area', title: 'Workspace header and tabs', size: 'wide', rec: 'B',
  now: 'Zed\'s tab bar: each tab with its state dot and name (numbered when unnamed, as herdr does), × on hover, + at the end. Nothing above the panes says which workspace you\'re in.',
  nowImg: 'img/now-tabs.png',
  issues: ['Unnamed tabs are "1", "2", "3", which says nothing about what\'s in them.', 'The branch, machine and folder are only in the sidebar, which may be hidden.'],
  options: [
    {
      key: 'A', name: 'Tabs named by what runs', from: 'Zed + iTerm2',
      desc: 'Today\'s bar, but an unnamed tab takes its focused pane\'s title ("Claude Code", "npm run dev") in muted text instead of a number; a name you give it wins. Cmd-1…9 shown in the tooltip.',
      good: 'Smallest change; tabs become readable.', cost: 'Still no workspace context.',
      mock: () => area(tabBar([['agents', 'working'], ['npm run dev'], ['zsh']]).replace('npm run dev</div>', 'npm run dev</div>').replace(/<div class="tab ">npm run dev/, '<div class="tab "><span class="mu" style="font-style:italic">npm run dev</span>').replace(/<div class="tab ">zsh/, '<div class="tab "><span class="mu" style="font-style:italic">zsh</span>') + tabsBody() + `<div class="pop" style="left:200px;top:40px;padding:4px 8px;font-size:12px">npm run dev <span class="kbd" style="margin-left:6px">⌘2</span></div>`, { h: 200 }),
    },
    {
      key: 'B', name: 'Breadcrumb, then tabs', from: 't3code header (your thread header pick)',
      desc: 'The bar starts with the workspace as a breadcrumb, "storefront / ⎇ checkout-flow ↑2", as the thread header does; the name opens the row\'s menu, the branch opens its checkouts. Tabs follow; the machine sits at the right when it isn\'t this Mac.',
      good: 'Context always visible; consistent with the Agents view.', cost: 'Less room for tabs.',
      mock: () => area(tabBar([['agents', 'working'], ['server'], ['3']], { lead: `<div class="row g2" style="padding:0 12px;border-right:1px solid var(--b);white-space:nowrap">${mono('ST', 'g')}<span class="sm">storefront</span>${ic('chev-down', 'xs mu')}<span class="faint">/</span><span class="chip" style="height:20px">${ic('branch', 'xs')}checkout-flow <span class="okc">↑2</span></span></div>`, end: ibtn('plus') }) + tabsBody(), { h: 200 }),
    },
    {
      key: 'C', name: 'Pills with their layout', from: 'new',
      desc: 'Rounded pill tabs in a slim bar; each shows a tiny glyph of its split layout (one pane, side by side, three) next to the name and state.',
      good: 'Tabs are recognizable by shape.', cost: 'Departs from Zed\'s tab look.',
      mock: () => {
        const p = (glyphName, name, st, on) => `<span class="row g15" style="height:26px;padding:0 10px;border-radius:7px;font-size:13px;${on ? 'background:var(--sel);color:var(--t)' : 'color:var(--mu)'}">${ic(glyphName, 'xs')}${name}${dot(st)}</span>`;
        return area(`<div class="row g1" style="height:36px;padding:0 6px;background:var(--panel);border-bottom:1px solid var(--b)">${p('cols3', 'agents', 'working', true)}${p('panel-bottom', 'server')}${p('sidebar', '3')}${ibtn('plus')}<span class="grow"></span></div>` + tabsBody(), { h: 200 });
      },
    },
    {
      key: 'D', name: 'Tabs at the bottom, with a status area', from: 'herdr tab_bar_position, tab_bar_right',
      desc: 'herdr\'s option: the tab row under the panes, and on its right a status area: ZOOM while zoomed, the branch, the machine, the time.',
      good: 'Terminal-multiplexer feel; status in one line.', cost: 'Unlike the rest of the app, whose headers are on top.',
      mock: () => area(`<div class="tile" style="flex:1">${agentsTab({ shell: { buttons: false }, dev: { buttons: false }, claude: { buttons: false } })}</div><div class="tabbar" style="border-top:1px solid var(--b);border-bottom:0">${[['agents', 'working'], ['server'], ['3']].map(([n, s], i) => `<div class="tab ${i === 0 ? 'on' : ''}" style="${i === 0 ? 'margin:-1px 0 0' : ''}">${dot(s)}${n}</div>`).join('')}${ibtn('plus')}<span class="grow"></span><div class="row g2 xs mu" style="padding:0 12px">${ic('branch', 'xs')}checkout-flow <span class="okc">↑2</span><span class="faint">·</span>${ic('laptop', 'xs')}This Mac<span class="faint">·</span>14:32</div></div>`, { h: 220 }),
    },
    {
      key: 'E', name: 'Tabs in the sidebar', from: 'new (pairs with rows C)',
      desc: 'No tab bar: the open workspace\'s tabs are listed under it in the sidebar, and the panes get the full height.',
      good: 'More vertical room; one place to navigate.', cost: 'Hidden sidebar hides the tabs too (Cmd-1…9 still work).',
      mock: () => frame(`<div class="row" style="height:100%;align-items:stretch"><div class="sidebar" style="width:220px"><div class="sb-list">${`<div class="srow on" style="padding:4px 10px"><div class="l1">${mono('ST', 'g')}<span class="grow">storefront</span>${dot('working')}</div></div>` + [['agents', 'working'], ['server'], ['3']].map(([n, s], i) => `<div class="row g2" style="height:26px;margin:0 4px;padding:0 10px 0 36px;border-radius:6px;font-size:13px;${i === 0 ? 'background:var(--hov);color:var(--t)' : 'color:var(--mu)'}">${dot(s) || '<span style="width:6px"></span>'}${n}<span class="grow"></span><span class="xs faint">⌘${i + 1}</span></div>`).join('') + `<div class="row g2 xs faint" style="height:24px;padding:0 0 0 50px">${ic('plus', 'xs')}New Tab</div>` + SPACES.slice(1, 4).map((s) => `<div class="srow" style="padding:4px 10px"><div class="l1">${s.mono ? mono(s.mono, s.color) : ic('folder', 'sm mu')}<span class="grow mu">${s.name}</span>${dot(s.state)}</div></div>`).join('')}</div></div><div class="tile grow">${agentsTab({ shell: { buttons: false }, dev: { buttons: false }, claude: { buttons: false } })}</div></div>`, { w: W_MAIN, h: 260 }),
    },
  ],
});

// 8. Pane header --------------------------------------------------------------------------------
const two = (a, b, h = 230) => area(split('h', 0.5, a, b), { h });
TOPICS.push({
  id: 'panehead', section: 'Main area', title: 'Pane header', size: 'wide', rec: 'A',
  now: 'Each pane has a 36px header: icon, title, a detail (a shell\'s window title, a thread\'s agent), the state dot, then Split, Zoom and Close. An ACP thread\'s pane folds its toolbar into the same header.',
  nowImg: 'img/now-pane-headers.png',
  issues: ['A long window title ("ahrorbek@Ahrorbeks-MacBook-Pro:/pri…") squeezes the title down to "…".', 'Every pane spends 36px on a header, even in a 2×2 grid.', 'Nothing shows what a shell is running, or that it exited.'],
  options: [
    {
      key: 'A', name: 'Fixed and quieter', from: 'Zed pane tabs',
      desc: 'The title always fits first; the detail is the folder relative to the workspace and the program, not the raw window title. Unfocused panes show their buttons on hover only.',
      good: 'Fixes the bug; reads cleanly.', cost: 'Same height as today.',
      mock: () => two(pane({ glyphKind: 'claude', title: 'Claude Code', detail: '~/storefront', state: 'working', focus: true, body: SCREENS.claude }), pane({ title: 'zsh', detail: 'storefront/src', body: SCREENS.shellIdle, buttons: false })),
    },
    {
      key: 'B', name: 'Slim header', from: 'tmux / Ghostty splits',
      desc: 'A 24px header with small text; buttons appear on hover. Saves 12px per pane, which adds up in grids.',
      good: 'More terminal rows.', cost: 'Smaller targets; breaks the 36px line-up with the tab bar.',
      mock: () => {
        const slim = (lead, title, detail, st, focus, buttons) => `<div class="phead" style="height:24px;font-size:11px;${focus ? 'background:var(--ed);color:var(--t)' : ''}">${lead}<span style="flex:none">${title}</span><span class="faint trunc">${detail}</span>${dot(st)}<span class="grow"></span>${buttons ? ibtn('split', 'sm') + ibtn('maximize', 'sm') + ibtn('x', 'sm') : ''}</div>`;
        return two(pane({ focus: true, body: SCREENS.claude, head: slim(glyph('claude', 'sm'), 'Claude Code', '~/storefront', 'working', true, true) }), pane({ body: SCREENS.shellIdle, head: slim(ic('terminal', 'xs'), 'zsh', 'storefront/src', null, false, false) }));
      },
    },
    {
      key: 'C', name: 'Title in the border', from: 'herdr / tmux pane-border-format',
      desc: 'No header bar. The title is set into the pane\'s top border; Split, Zoom and Close float over the top-right corner on hover.',
      good: 'Almost no space lost; terminal-native look.', cost: 'Thread panes still need their toolbar somewhere.',
      mock: () => {
        const bordered = (lead, title, st, focus, body) => `<div class="pane" style="border:1px solid ${focus ? 'var(--ac)' : 'var(--b)'};margin:6px 4px 4px;border-radius:4px;flex:1"><div style="position:absolute;top:-9px;left:10px;background:var(--ed);padding:0 6px;font-size:11px;display:flex;gap:5px;align-items:center;color:${focus ? 'var(--t)' : 'var(--mu)'}">${lead}${title}${dot(st)}</div>${focus ? `<div class="row" style="position:absolute;top:4px;right:4px;background:var(--panel);border:1px solid var(--b);border-radius:6px;z-index:2">${btns('split', 'maximize', 'x')}</div>` : ''}${term(body, 'padding-top:10px')}</div>`;
        return area(`<div class="row" style="flex:1;align-items:stretch;background:var(--ed)">${bordered(glyph('claude', 'sm'), 'Claude Code · ~/storefront', 'working', true, SCREENS.claude)}${bordered(ic('terminal', 'xs'), 'zsh · storefront/src', null, false, SCREENS.shellIdle)}</div>`, { h: 230 });
      },
    },
    {
      key: 'D', name: 'Rich header', from: 'new (t3code discovered servers)',
      desc: 'The header also says what\'s running and for how long, the ports it listens on (click to open; forwarded over SSH from remote machines), and how a finished command ended, with Restart.',
      good: 'A pane explains itself; dev servers one click from the browser.', cost: 'Busier; needs process and port tracking on the server.',
      mock: () => two(pane({ title: 'npm run dev', detail: '12m', focus: true, body: SCREENS.dev, head: `<div class="phead" style="background:var(--ed);color:var(--t)">${ic('terminal', 'sm')}<span>npm run dev</span><span class="faint">· 12m</span><span class="chip" style="height:20px;color:var(--ac);border-color:rgba(116,173,232,.4)">${ic('globe', 'xs')}:3000${ic('external', 'xs')}</span><span class="grow"></span>${btns('restart', 'split', 'maximize', 'x')}</div>` }), pane({ title: 'cargo test', body: [`${prompt('api', 'main')}cargo test -p limiter`, 'running 14 tests', `test result: ${C('tr', 'FAILED')}. 13 passed; 1 failed`], head: `<div class="phead">${ic('terminal', 'sm')}<span>cargo test</span><span class="chip" style="height:20px;color:var(--del);border-color:rgba(208,114,119,.4)">✗ exit 101</span><span class="grow"></span><span class="btn sm">${ic('restart', 'xs')}Run Again</span>${btns('x')}</div>` })),
    },
    {
      key: 'E', name: 'Header only when focused', from: 'iTerm2 badges',
      desc: 'The focused pane gets the full header; the others only a small label in their top-right corner, so most of the grid is terminal.',
      good: 'Maximum space for big grids.', cost: 'Clicking a pane reflows its first line.',
      mock: () => area(split('h', 0.5, pane({ glyphKind: 'claude', title: 'Claude Code', detail: '~/storefront', state: 'working', focus: true, body: SCREENS.claude }), split('v', 0.5, pane({ head: `<span style="position:absolute;right:6px;top:6px;z-index:2;font-size:11px;color:var(--mu);background:var(--panel);border:1px solid var(--bv);border-radius:5px;padding:1px 6px">zsh</span>`, body: SCREENS.shellIdle }), pane({ head: `<span style="position:absolute;right:6px;top:6px;z-index:2;font-size:11px;color:var(--mu);background:var(--panel);border:1px solid var(--bv);border-radius:5px;padding:1px 6px">npm run dev</span>`, body: SCREENS.dev }))), { h: 300 }),
    },
  ],
});

// 9. Focus and dividers ---------------------------------------------------------------------------
const focusTab = (o = {}) => split('h', 0.55,
  pane({ glyphKind: 'claude', title: 'Claude Code', detail: '~/storefront', state: 'working', focus: true, body: SCREENS.claude, ...o.claude }),
  split('v', 0.5, pane({ glyphKind: 'codex', title: 'Codex', detail: 'brave-otter', state: 'pending', body: SCREENS.codex, ...o.codex }), pane({ title: 'zsh', detail: '~/storefront', body: SCREENS.shellIdle, ...o.shell })));
TOPICS.push({
  id: 'focus', section: 'Main area', title: 'Focus and dividers', size: 'wide', rec: 'A',
  now: 'In a split tab the focused pane has a thin blue-gray border (the theme\'s border.focused); others have none. The focused header uses the active tab background. Dividers are 1px lines you can drag.',
  issues: ['The focus border is faint; in a 2×2 grid it\'s easy to type into the wrong pane.'],
  options: [
    { key: 'A', name: 'Clear accent outline', from: 'Zed active pane', desc: 'The focused pane gets a 1px accent outline and the bright header; everything else as today.', good: 'Unmistakable, still quiet.', cost: 'None really.', mock: () => area(focusTab({ claude: { style: 'border-color:var(--ac)' } }), { h: 320 }) },
    {
      key: 'B', name: 'Dim the others', from: 'tmux window-style',
      desc: 'Unfocused panes\' content fades to about 55%; no outline at all.',
      good: 'Your eye goes to the right pane.', cost: 'Harder to read a log beside you.',
      mock: () => area(focusTab({ claude: { style: 'border-color:transparent' }, codex: { style: 'opacity:.5' }, shell: { style: 'opacity:.5' } }), { h: 320 }),
    },
    {
      key: 'C', name: 'Header shows focus', from: 'Zed tabs',
      desc: 'No outline; the focused pane\'s header is bright with a 2px accent line under it, like Zed\'s active tab.',
      good: 'Clean edges between terminals.', cost: 'Weak signal when headers are hidden (zoom, slim).',
      mock: () => area(focusTab({ claude: { style: 'border-color:transparent', head: `<div class="phead" style="background:var(--ed);color:var(--t);box-shadow:inset 0 -2px 0 var(--ac)">${glyph('claude', 'sm')}Claude Code<span class="mu">~/storefront</span>${dot('working')}<span class="grow"></span>${btns('split', 'maximize', 'x')}</div>` } }), { h: 320 }),
    },
    {
      key: 'D', name: 'Cards with gaps', from: 'Warp',
      desc: 'Panes as rounded cards with 6px gaps; the focused card has a brighter border and a soft glow.',
      good: 'Modern, each pane clearly separate.', cost: 'Loses 6px per gap; less like Zed.',
      mock: () => {
        const card = (p, f) => `<div class="tile" style="flex:1;padding:3px"><div class="tile" style="flex:1;border-radius:8px;overflow:hidden;border:1px solid ${f ? 'var(--ac)' : 'var(--b)'};${f ? 'box-shadow:0 0 0 3px rgba(116,173,232,.12)' : ''}">${p}</div></div>`;
        return area(`<div class="tile h" style="flex:1;padding:3px;background:var(--panel)"><div class="tile" style="flex:.55">${card(pane({ glyphKind: 'claude', title: 'Claude Code', detail: '~/storefront', state: 'working', body: SCREENS.claude, style: 'flex:1;border:0' }), true)}</div><div class="tile v" style="flex:.45">${card(pane({ glyphKind: 'codex', title: 'Codex', detail: 'brave-otter', state: 'pending', body: SCREENS.codex, style: 'flex:1;border:0' }))}${card(pane({ title: 'zsh', detail: '~/storefront', body: SCREENS.shellIdle, style: 'flex:1;border:0' }))}</div></div>`, { h: 320 });
      },
    },
    {
      key: 'E', name: 'Borders show agent state', from: 'new (herdr states)',
      desc: 'Each pane\'s border takes its agent\'s state color: yellow waiting for approval, blue working, green done. Focus moves to the header (as C).',
      good: 'In a big grid, the one that needs you stands out.', cost: 'Colored borders everywhere can get loud.',
      mock: () => area(focusTab({ claude: { style: 'border-color:rgba(116,173,232,.6)', head: `<div class="phead" style="background:var(--ed);color:var(--t);box-shadow:inset 0 -2px 0 var(--ac)">${glyph('claude', 'sm')}Claude Code<span class="mu">~/storefront</span><span class="grow"></span>${btns('split', 'maximize', 'x')}</div>` }, codex: { style: 'border-color:var(--warn)' } }), { h: 320 }),
    },
  ],
});

// 10. Rearranging panes --------------------------------------------------------------------------
const dragLabel = (text, style) => `<span style="position:absolute;z-index:25;${style};background:var(--panel);border:1px solid var(--b);border-radius:6px;padding:3px 8px;font-size:12px;box-shadow:0 6px 18px rgba(0,0,0,.5);display:flex;gap:6px;align-items:center">${ic('terminal', 'xs')}${text}</span>`;
TOPICS.push({
  id: 'dnd', section: 'Main area', title: 'Rearranging panes', size: 'wide', rec: 'B',
  now: 'Dragging a pane\'s header onto another pane swaps the two. Tabs and workspaces reorder by dragging. Panes can\'t move to another tab or workspace, or into a new split.',
  options: [
    { key: 'A', name: 'Swap only (today)', from: 'herdr swap', desc: 'Keep swapping as the only drag.', good: 'Simple.', cost: 'Can\'t build a layout by dragging.', mock: () => area(main(tabBar(), agentsTab({ claude: { style: 'border-color:var(--ac)' } })) + dragLabel('npm run dev', 'left:300px;top:150px'), { h: 330 }) },
    {
      key: 'B', name: 'Edges split, middle swaps', from: 'Zed pane drag targets',
      desc: 'Over another pane, the half you\'re nearest lights up: dropping there splits it that way; the middle swaps. Zed does this for its pane items.',
      good: 'Builds any layout with the mouse.', cost: 'Needs a move-pane request on the server.',
      mock: () => area(main(tabBar(), agentsTab()) + '<div class="drop" style="left:250px;top:38px;width:246px;height:290px"></div>' + note('Split Right', 'left:330px;top:170px') + dragLabel('npm run dev', 'left:300px;top:130px'), { h: 330 }),
    },
    {
      key: 'C', name: 'Drop on a tab or workspace', from: 'Zed tab drag',
      desc: 'Drag a pane onto another tab to move it there (split right), onto + to make it a tab of its own, or onto a workspace row to move it into that workspace on the same machine.',
      good: 'Reorganize across tabs without closing anything.', cost: 'Long drags; the sidebar may be hidden.',
      mock: () => area(main(tabBar([['agents', 'working'], ['server'], ['3']]).replace('<div class="tab ">server', '<div class="tab " style="background:var(--drop);color:var(--t)">server'), agentsTab()) + dragLabel('zsh', 'left:150px;top:24px') + note('Move to "server"', 'left:120px;top:62px'), { h: 330 }),
    },
    {
      key: 'D', name: 'Arrange mode (keys)', from: 'herdr resize mode, swap keys',
      desc: 'Cmd-Shift-R enters an arrange mode: arrows move the focused pane, Shift-arrows resize it, Enter finishes. A bar says what keys do what.',
      good: 'Keyboard users get the whole thing.', cost: 'A mode to learn.',
      mock: () => area(main(tabBar(), agentsTab({ dev: { style: 'border:2px dashed var(--ac)' } }), `<div class="row g3" style="height:30px;padding:0 12px;background:#33404f;border-top:1px solid var(--bf);font-size:12px"><span class="b6 ac">ARRANGE</span><span>${keycap('←')}${keycap('↑')}${keycap('→')}${keycap('↓')} move</span><span>${keycap('⇧')}+arrows resize</span><span>${keycap('⇥')} next pane</span><span class="grow"></span><span>${keycap('↩')} done</span><span>${keycap('esc')} cancel</span></div>`), { h: 330 }),
    },
    {
      key: 'E', name: 'Layout picker while dragging', from: 'Windows snap layouts',
      desc: 'Start dragging a pane and a small strip of layouts appears at the top of the tab; drop on a slot to put the pane there and arrange the rest.',
      good: 'Even, tidy layouts without fiddling.', cost: 'Fixed set of layouts.',
      mock: () => {
        const lay = (cells, hot) => `<span style="width:54px;height:34px;display:grid;gap:2px;padding:2px;border:1px solid var(--b);border-radius:5px;background:var(--panel);${cells}">${hot}</span>`;
        const c = (on) => `<i style="display:block;border-radius:2px;background:${on ? 'var(--ac)' : 'var(--sel)'}"></i>`;
        return area(main(tabBar(), agentsTab()) + `<div class="pop row g2" style="left:300px;top:44px;padding:8px">${lay('grid-template-columns:1fr 1fr', c(0) + c(1))}${lay('grid-template-columns:1fr 1fr 1fr', c(0) + c(0) + c(0))}${lay('grid-template-columns:1fr 1fr;grid-template-rows:1fr 1fr', c(0) + c(0) + c(0) + c(0))}${lay('grid-template-columns:2fr 1fr;grid-template-rows:1fr 1fr', '<i style="grid-row:1/3;background:var(--sel);border-radius:2px"></i>' + c(0) + c(0))}</div>` + dragLabel('npm run dev', 'left:420px;top:110px'), { h: 330 });
      },
    },
  ],
});

// 11. What a new pane opens ------------------------------------------------------------------
const newRight = (content) => area(main(tabBar(), split('h', 0.5, pane({ glyphKind: 'claude', title: 'Claude Code', detail: '~/storefront', state: 'working', body: SCREENS.claude }), content)), { h: 380 });
TOPICS.push({
  id: 'newpane', section: 'Main area', title: 'What a new pane opens', size: 'wide', rec: 'D',
  now: 'New Tab, Split Right and Split Down open a shell in the workspace\'s folder. To get an agent you start it in the shell, or use the pane menu\'s New Thread… / Show Thread ▸.',
  issues: ['A split from a shell in <code>storefront/src</code> opens back in <code>storefront</code>.'],
  options: [
    {
      key: 'A', name: 'Choices in the split menu', from: 'Zed split menu',
      desc: 'Split Right / Down stay a shell, but each gets a submenu: Shell, the agent CLIs found on that machine (Claude Code, Codex…), and New Thread with an installed ACP agent.',
      good: 'No new screens.', cost: 'Two levels deep.',
      mock: () => newRight(pane({ title: 'zsh', detail: '~/storefront', focus: true, body: SCREENS.shell })) + '',
    },
    {
      key: 'B', name: 'Launcher in the new pane', from: 'new',
      desc: 'A new pane opens on a small launcher: Shell (Enter), agent CLIs on that machine, ACP agents for a thread, and recent commands. One key picks.',
      good: 'Agents one keystroke away; teaches what\'s possible.', cost: 'An extra step when you just wanted a shell (Enter).',
      mock: () => newRight(pane({ title: 'New Pane', icon: 'plus', focus: true, body: `<div style="flex:1;background:var(--ed);display:flex;align-items:center;justify-content:center"><div class="col g1" style="width:280px">${[['terminal', 'Shell', '↩', true], ['claude', 'Claude Code', '1'], ['codex', 'Codex', '2'], ['opencode', 'OpenCode', '3'], ['chat', 'New Thread with Claude Agent…', '4'], ['history', 'npm run dev', '5']].map(([g, t, k, on]) => `<div class="row g2" style="height:30px;padding:0 10px;border-radius:6px;font-size:13px;${on ? 'background:var(--hov)' : ''}">${GLYPHS[g] ? glyph(g, 'sm') : ic(g, 'sm mu')}<span class="grow">${t}</span>${keycap(k)}</div>`).join('')}<span class="hint" style="padding:6px 10px">On This Mac · ~/storefront</span></div></div>` })),
    },
    {
      key: 'C', name: 'Command line first', from: 'Warp / Raycast',
      desc: 'A new pane starts with one field: type a command (or an agent\'s name) with suggestions from history, or press Enter for a plain shell.',
      good: 'Fast for people who know what they want.', cost: 'Another input before the shell.',
      mock: () => newRight(pane({ title: 'New Pane', icon: 'plus', focus: true, body: `<div style="flex:1;background:var(--ed);padding:40px 30px"><div class="field focus">${ic('terminal', 'sm mu')}npm run<span class="cursor" style="height:15px;width:1.5px"></span></div><div class="card" style="margin-top:6px;padding:4px">${[['history', 'npm run dev', 'ran 2h ago'], ['history', 'npm run test', 'yesterday'], ['history', 'npm run lint', '']].map(([g, t, h], i) => `<div class="row g2" style="height:28px;padding:0 8px;border-radius:5px;font-size:13px;${i === 0 ? 'background:var(--hov)' : ''}">${ic(g, 'sm mu')}<span class="grow mono-font" style="font-size:12px">${t}</span><span class="xs faint">${h}</span></div>`).join('')}</div><div class="hint" style="margin-top:8px">↩ runs it · empty ↩ opens a shell</div></div>` })),
    },
    {
      key: 'D', name: 'Like the focused pane', from: 'iTerm2 "reuse previous directory"',
      desc: 'Split from a shell opens a shell in that shell\'s current folder; split from an agent CLI opens a fresh one of the same agent there. New Tab still opens the workspace\'s folder.',
      good: 'Does what you meant without asking.', cost: 'Implicit; Option-click for a plain shell.',
      mock: () => area(main(tabBar(), split('h', 0.5, pane({ title: 'zsh', detail: 'storefront/src/app', body: [`${prompt()}cd src/app`, `${C('tg', '➜')}  ${C('tc', B('app'))} ${C('tb', 'git:(')}${C('tr', 'checkout-flow')}${C('tb', ')')} ls`, 'cart  checkout  layout.tsx  page.tsx'] }), pane({ title: 'zsh', detail: 'storefront/src/app', focus: true, body: [`${C('tg', '➜')}  ${C('tc', B('app'))} ${C('tb', 'git:(')}${C('tr', 'checkout-flow')}${C('tb', ')')} <span class="cursor"></span>`] }))) + note('new split, same folder', 'left:600px;top:120px'), { h: 300 }),
    },
    {
      key: 'E', name: 'A default per workspace', from: 'new',
      desc: 'Each workspace has a "New panes open" choice (Shell, an agent CLI, or Ask each time) in its menu; the + and split buttons follow it.',
      good: 'Agent-heavy workspaces open agents by default.', cost: 'Another setting to remember.',
      mock: () => frame(`<div class="row" style="height:100%;align-items:stretch">${sidebar({ list: currentList() })}<div class="grow" style="background:var(--ed)"></div></div><div class="menu" style="left:220px;top:60px">${it('plus', 'New Tab', '⌘T')}${it('pencil', 'Rename')}<div class="it hl">${ic('layers', 'sm')}New Panes Open<span class="kb">${ic('chev-right', 'xs')}</span></div>${hrr}${it('x', 'Close Workspace')}</div><div class="menu" style="left:436px;top:118px;min-width:170px"><div class="it">${ic('check', 'sm')}Shell</div><div class="it"><span style="width:14px"></span>${glyph('claude', 'sm')}Claude Code</div><div class="it"><span style="width:14px"></span>${glyph('codex', 'sm')}Codex</div><div class="it"><span style="width:14px"></span>Ask Each Time</div></div>`, { w: 660, h: 380 }),
    },
  ],
});
// Option A needs its submenu drawn over the pane.
TOPICS[TOPICS.length - 1].options[0].mock = () => area(main(tabBar(), split('h', 0.5, pane({ glyphKind: 'claude', title: 'Claude Code', detail: '~/storefront', state: 'working', focus: true, body: SCREENS.claude, head: `<div class="phead" style="background:var(--ed);color:var(--t)">${glyph('claude', 'sm')}Claude Code<span class="mu">~/storefront</span>${dot('working')}<span class="grow"></span>${ibtn('split', 'on')}${btns('maximize', 'x')}</div>` }), pane({ title: 'zsh', detail: '~/storefront', body: SCREENS.shellIdle })), `<div class="menu" style="left:190px;top:72px">${'<div class="it hl">' + ic('split', 'sm') + 'Split Right<span class="kb">' + ic('chev-right', 'xs') + '</span></div>'}${it('split-v', 'Split Down', ic('chev-right', 'xs'))}</div><div class="menu" style="left:405px;top:72px;min-width:200px">${it('terminal', 'Shell', '⌘D')}<div class="hr"></div><div class="lbl">Agent CLIs on This Mac</div><div class="it">${glyph('claude', 'sm')}Claude Code</div><div class="it">${glyph('codex', 'sm')}Codex</div>${hrr}${it('chat', 'New Thread…')}</div>`), { h: 330 });

// 12. Empty states --------------------------------------------------------------------------------
const emptyWin = (center) => win(`${sidebar({ list: `<div class="sm mu" style="padding:8px 14px">No workspaces yet</div>` })}<div class="grow" style="display:flex;align-items:center;justify-content:center;background:var(--ed)">${center}</div>`, { w: 980, h: 520 });
TOPICS.push({
  id: 'empty', section: 'Main area', title: 'Empty state', size: 'wide', rec: 'A',
  now: 'With no workspaces: "Workspaces put terminals and threads side by side" and a New Workspace button that opens the + picker.',
  options: [
    { key: 'A', name: 'Today\'s, plus recent folders', from: 'Zed welcome', desc: 'The same line and button, then your projects as one-click rows that open a workspace there.', good: 'One click to get going.', cost: 'Minor.', mock: () => emptyWin(`<div class="col g2" style="align-items:center;width:340px"><span class="mu">Workspaces put terminals and threads side by side</span><span class="btn">New Workspace <kbd>⌘⇧N</kbd></span><div class="col" style="width:100%;margin-top:14px;gap:2px"><span class="xs ph" style="padding:0 8px">Open a project</span>${[[mono('ST', 'g'), 'storefront', '~/w/storefront', 'laptop'], [mono('AP', 't'), 'api', '~/w/api', 'server'], [ic('folder', 'sm mu'), '~', 'Home', 'laptop']].map(([i, n, p, m], k) => `<div class="row g2" style="height:30px;padding:0 8px;border-radius:6px;${k === 0 ? 'background:var(--hov)' : ''}">${i}<span class="sm">${n}</span><span class="xs faint grow">${p}</span><span class="mu">${ic(m, 'xs')}</span></div>`).join('')}</div></div>`) },
    { key: 'B', name: 'Project cards', from: 't3code NoProjectsHero', desc: 'A grid of cards, one per project (icon, name, branch, machine), plus Open Folder…; a click opens a workspace there.', good: 'Visual, shows everything you have.', cost: 'Big for people with one project.', mock: () => emptyWin(`<div class="col g3" style="align-items:center"><span class="mu">Open a workspace</span><div style="display:grid;grid-template-columns:repeat(3,160px);gap:10px">${[['ST', 'g', 'storefront', 'checkout-flow', 'laptop'], ['AP', 't', 'api', 'main', 'server'], ['DS', 'p', 'design-system', 'tokens', 'laptop']].map(([m, c, n, b, mi]) => `<div class="card" style="padding:12px;display:flex;flex-direction:column;gap:8px">${mono(m, c, 'lg')}<span class="sm">${n}</span><span class="row g1 xs faint">${ic('branch', 'xs')}${b}<span class="grow"></span>${ic(mi, 'xs')}</span></div>`).join('')}</div><span class="btn ghost">${ic('folder-open', 'sm')}Open Folder…</span></div>`) },
    { key: 'C', name: 'Explain the model', from: 'herdr "learn these five first"', desc: 'A small drawing of workspace → tabs → panes, and the five shortcuts worth learning first.', good: 'Teaches the view once.', cost: 'Text-heavy; seen rarely.', mock: () => emptyWin(`<div class="col g3" style="align-items:center;width:420px"><div class="row g2" style="align-items:stretch">${['Workspace<br><span class="xs faint">a folder</span>', 'Tabs<br><span class="xs faint">layouts</span>', 'Panes<br><span class="xs faint">shells, agents, threads</span>'].map((t, i) => `${i ? `<span class="mu" style="align-self:center">${ic('arrow', 'sm')}</span>` : ''}<div class="card sm" style="padding:8px 12px;text-align:center">${t}</div>`).join('')}</div><div class="col" style="width:100%;gap:4px;font-size:13px">${[['New workspace', '⌘⇧N'], ['New tab', '⌘T'], ['Split right / down', '⌘D / ⌘⇧D'], ['Move between panes', '⌘⌥ arrows'], ['Zoom a pane', '⌘⇧↩']].map(([a, k]) => `<div class="row"><span class="mu grow">${a}</span><span class="kbd">${k}</span></div>`).join('')}</div><span class="btn primary">New Workspace</span></div>`) },
    { key: 'D', name: 'Start from a layout', from: 'zellij layouts', desc: 'Pick how the first tab looks (one shell, agent + shell, two agents, a grid), then where.', good: 'You start with the setup you use.', cost: 'A choice before you\'ve done anything.', mock: () => emptyWin(`<div class="col g3" style="align-items:center"><span class="mu">How should it start?</span><div class="row g3">${[['Shell', '<i></i>', ''], ['Agent + shell', '<i style="background:rgba(116,173,232,.4)"></i><i></i>', 'grid-template-columns:1.3fr 1fr'], ['Two agents', '<i style="background:rgba(116,173,232,.4)"></i><i style="background:rgba(116,173,232,.4)"></i>', 'grid-template-columns:1fr 1fr'], ['Agent + 2 shells', '<i style="grid-row:1/3;background:rgba(116,173,232,.4)"></i><i></i><i></i>', 'grid-template-columns:1.3fr 1fr;grid-template-rows:1fr 1fr']].map(([n, cells, g], i) => `<div class="col g2" style="align-items:center"><span style="width:110px;height:70px;display:grid;gap:3px;padding:4px;border-radius:8px;border:1px solid ${i === 1 ? 'var(--ac)' : 'var(--b)'};background:var(--panel);${g}">${cells.replace(/<i>/g, '<i style="background:var(--sel)">').replace(/<i style="/g, '<i style="display:block;border-radius:3px;')}</span><span class="sm ${i === 1 ? '' : 'mu'}">${n}</span></div>`).join('')}</div><span class="btn primary">Choose Folder…</span></div>`) },
    { key: 'E', name: 'Minimal', from: 'Zed empty pane', desc: 'Just "No workspaces" and the shortcut, like Zed\'s empty editor.', good: 'Quiet.', cost: 'New users get no help.', mock: () => emptyWin(`<div class="col g1" style="align-items:center"><span class="mu">No workspaces</span><span class="row g2 sm ph">New Workspace <span class="kbd">⌘⇧N</span></span></div>`) },
  ],
});
