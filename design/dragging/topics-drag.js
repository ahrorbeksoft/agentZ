// Dragging in the Workspaces view: what follows the pointer, and what the drop looks like.

// The pointer, its tip at (x, y).
const pointer = (x, y) => `<svg width="16" height="22" viewBox="0 0 16 22" style="position:absolute;left:${x - 1}px;top:${y - 1}px;z-index:40;filter:drop-shadow(0 1px 2px rgba(0,0,0,.5))"><path d="M1 1v17l4.2-4.1 2.7 6.4 2.6-1.1-2.7-6.3H14z" fill="#111" stroke="#fff" stroke-width="1.3" stroke-linejoin="round"/></svg>`;
const floating = (style, html) => `<div style="position:absolute;z-index:25;${style}">${html}</div>`;
// The label every drag carries today (spaces_view.rs DragPreview): Zed's small elevated chip.
const todayLabel = (text, x, y) => floating(`left:${x + 12}px;top:${y + 12}px;background:var(--panel);border:1px solid var(--b);border-radius:6px;padding:3px 8px;font-size:12px;box-shadow:0 6px 18px rgba(0,0,0,.5)`, text);
const shade = 'background:var(--drop)';

// The agents tab as the app draws it: header with the program and folder, Split Right,
// Split Down, Close.
const paneHead = ({ lead, title, folder = 'storefront', style = '' }) => `<div class="phead" style="${style}">${lead}<span class="trunc" style="flex:none;max-width:60%">${title}</span><span class="trunc mu">${folder}</span><span class="grow"></span>${ibtn('split')}${ibtn('split-v')}${ibtn('x')}</div>`;
const PANES = {
  claude: { lead: glyph('claude', 'sm'), title: 'Claude Code', body: SCREENS.claude },
  shell: { lead: ic('terminal', 'sm'), title: 'Shell', body: SCREENS.shellIdle },
  dev: { lead: ic('terminal', 'sm'), title: 'npm run dev', body: SCREENS.dev },
};
const tabPane = (name, { focus = false, style = '', head = {}, body } = {}) => {
  const p = PANES[name];
  return pane({ head: paneHead({ ...p, ...head }), focus, style, body: body ?? p.body });
};
// npm run dev was clicked to start the drag, so it has focus.
const tabPanes = ({ claude = {}, shell = {}, dev = {} } = {}) => split('h', 0.56,
  tabPane('claude', claude),
  split('v', 0.5, tabPane('shell', shell), tabPane('dev', { focus: true, ...dev })));

// The main area is 760×420: tab bar 36, Claude Code x 0–425, the right column x 426–760
// (Shell above y 228, npm run dev below). The pointer sits in Claude Code's right fifth, so
// dropping splits it there.
const W = 760, H = 420;
const PX = 385, PY = 170;
const RIGHT_HALF = 'left:213px;top:36px;width:212px;height:384px';
const paneScene = ({ panes = {}, drop = floating(`${RIGHT_HALF};${shade};z-index:12`, ''), carried = '', tabs = tabBar([['agents', 'working'], ['server'], ['Tab 3']]) } = {}) =>
  frame(main(tabs, tabPanes(panes), drop + carried + pointer(PX, PY)), { w: W, h: H });

// The small preview: the pane's header and the top of its screen, drawn small.
const miniPane = (name, x, y, { w = 240, h = 150, style = '' } = {}) => {
  const p = PANES[name];
  return floating(`left:${x}px;top:${y}px;width:${w}px;height:${h}px;border:1px solid var(--b);border-radius:6px;overflow:hidden;box-shadow:0 12px 30px rgba(0,0,0,.55);display:flex;flex-direction:column;${style}`,
    `<div class="row g15" style="height:24px;padding:0 8px;background:var(--ed);border-bottom:1px solid var(--bv);font-size:11px;flex:none">${p.lead}<span>${p.title}</span><span class="mu trunc">storefront</span></div>${term(p.body, 'font-size:8.5px;line-height:12px;padding:4px 6px')}`);
};
const PREVIEW = miniPane('dev', PX + 10, PY + 10);

// 1. What follows the pointer ------------------------------------------------------------
TOPICS.push({
  id: 'pane-preview', section: 'Panes', title: 'What follows the pointer when you drag a pane', size: 'wide', rec: 'B',
  now: 'Dragging a pane\'s header carries a small label with its title ("Shell"). It used to stay where the label started, far from the pointer when you grabbed the header by its far end (the screenshot); since the last fix it sits just past the pointer. The half it would split off, or all of the pane for a swap, is shaded.',
  nowImg: 'img/now-pane-drag.jpg',
  issues: ['A bare word floating over the terminal reads like a stray tooltip.', 'It doesn\'t look like the pane you picked up.'],
  options: [
    {
      key: 'A', name: 'A tab of the pane', from: 'Zed dragged tab (workspace/pane.rs DraggedTab)',
      desc: 'A tab-shaped chip with the pane\'s icon, title and folder, as Zed draws a tab you drag between panes, just past the pointer.',
      good: 'Familiar from Zed; small, so it never hides where you\'re dropping.', cost: 'Still a label, only dressed as a tab.',
      mock: () => paneScene({ carried: floating(`left:${PX + 12}px;top:${PY + 12}px;height:32px;display:flex;align-items:center;gap:6px;padding:0 12px;background:var(--ed);border:1px solid var(--b);font-size:13px;box-shadow:0 8px 22px rgba(0,0,0,.5)`, `${ic('terminal', 'sm')}npm run dev<span class="mu sm">storefront</span>`) }),
    },
    {
      key: 'B', name: 'A small preview of the pane', from: 'new',
      desc: 'A card about 240×150 just past the pointer: the pane\'s header (icon, title, folder) and the top of what\'s on its screen in small text, with a shadow. A thread pane shows its title and the start of its chat the same way.',
      good: 'You see what you\'re moving, the way you picked it up.', cost: 'Covers more of the target than a label; the screen text is a still picture taken when the drag starts.',
      mock: () => paneScene({ carried: PREVIEW }),
    },
    {
      key: 'C', name: 'The header lifts off', from: 'GPUI\'s default drag (the element moves, held where you grabbed it)',
      desc: 'The pane\'s whole header follows the pointer, a little see-through, held at the spot you grabbed, so a header grabbed by its far end hangs to the left of the pointer.',
      good: 'Exactly what you grabbed moves with you.', cost: 'As wide as the pane: it can hide much of the target.',
      mock: () => paneScene({ carried: floating(`left:${PX - 274}px;top:${PY - 18}px;width:334px;opacity:.88;box-shadow:0 10px 26px rgba(0,0,0,.5);border:1px solid var(--b);border-radius:4px;overflow:hidden`, paneHead({ ...PANES.dev, style: 'background:var(--ed);color:var(--t);border-bottom:0' })) }),
    },
    {
      key: 'D', name: 'Just its icon', from: 'new (macOS drags a file as its icon)',
      desc: 'A 28-pixel tile with the pane\'s icon (the agent\'s icon for an agent) just past the pointer, no text.',
      good: 'Smallest; never in the way.', cost: 'Two shells look the same.',
      mock: () => paneScene({ carried: floating(`left:${PX + 12}px;top:${PY + 12}px;width:28px;height:28px;border-radius:6px;background:var(--panel);border:1px solid var(--b);display:grid;place-items:center;box-shadow:0 6px 18px rgba(0,0,0,.5)`, ic('terminal', 'sm')) }),
    },
    {
      key: 'E', name: 'Nothing at the pointer', from: 'new',
      desc: 'Only the pointer moves. The pane you\'re moving dims where it is (a pane only moves within its tab, so it\'s always in sight), and the shaded spot says where it goes ("npm run dev → right of Claude Code").',
      good: 'Nothing covers the panes; the words say exactly what the drop does.', cost: 'Relies on the target\'s words; nothing moves with your hand.',
      mock: () => paneScene({
        panes: { dev: { style: 'opacity:.4' } },
        drop: floating(`${RIGHT_HALF};${shade};z-index:12;display:grid;place-items:center`, '<span class="sm" style="background:var(--panel);border:1px solid var(--b);border-radius:5px;padding:3px 8px">npm run dev → right of Claude Code</span>'),
      }),
    },
  ],
});

// 2. The pane left behind ----------------------------------------------------------------
TOPICS.push({
  id: 'pane-source', section: 'Panes', title: 'The pane you\'re moving, while you drag', size: 'wide', rec: 'B',
  now: 'It stays exactly as it was, focused (clicking its header to start the drag focuses it). Shown here with the small preview (B above).',
  options: [
    {
      key: 'A', name: 'Unchanged', from: 'today',
      desc: 'It stays as it is.', good: 'Nothing to build.', cost: 'Nothing tells you which pane is in your hand but the preview.',
      mock: () => paneScene({ carried: PREVIEW }),
    },
    {
      key: 'B', name: 'Dimmed', from: 'new',
      desc: 'Drawn at 40% while you drag; back to normal when you drop or let go elsewhere.',
      good: 'Clear which one is moving, and it still shows what it is.', cost: 'None to speak of.',
      mock: () => paneScene({ panes: { dev: { style: 'opacity:.4' } }, carried: PREVIEW }),
    },
    {
      key: 'C', name: 'An empty slot', from: 'new',
      desc: 'Its content gives way to a dashed outline with its icon and title in the middle, as if the pane had been lifted out of its place.',
      good: 'Reads as "picked up"; pairs with the preview carrying its screen.', cost: 'The output vanishes for the moment you drag.',
      mock: () => paneScene({
        panes: { dev: { style: 'opacity:.7', body: `<div style="flex:1;margin:8px;border:1.5px dashed var(--b);border-radius:6px;display:grid;place-items:center;color:var(--ph)" class="sm"><span class="row g15">${ic('terminal', 'sm')}npm run dev</span></div>` } },
        carried: PREVIEW,
      }),
    },
    {
      key: 'D', name: 'Outlined', from: 'new',
      desc: 'A dashed accent outline around it in place of the focus border, its content unchanged.',
      good: 'Marks it without hiding anything.', cost: 'Close to the focus border; easy to miss.',
      mock: () => paneScene({ panes: { dev: { style: 'border:1.5px dashed var(--ac)' } }, carried: PREVIEW }),
    },
  ],
});

// 3. Where it will land ------------------------------------------------------------------
const verbBadge = (icon, text) => `<span class="row g1" style="height:22px;padding:0 8px;border-radius:4px;border:1px solid rgba(116,173,232,.4);background:rgba(116,173,232,.12);color:var(--ac);font-size:12px;font-weight:500">${ic(icon, 'xs')}${text}</span>`;
TOPICS.push({
  id: 'pane-drop', section: 'Panes', title: 'Where the pane will land', size: 'wide', rec: 'C',
  now: 'As in Zed: near an edge (a fifth of the pane\'s shorter side), the half the dragged pane would take is shaded gray; in the middle, all of the pane is, and dropping swaps the two. Shown here with the small preview.',
  options: [
    {
      key: 'A', name: 'Shaded (today)', from: 'Zed pane drop targets',
      desc: 'The half it would take, or the whole pane for a swap, shaded gray.', good: 'Matches Zed.', cost: 'Doesn\'t say whether it splits or swaps; gray over a dark terminal is faint.',
      mock: () => paneScene({ carried: PREVIEW }),
    },
    {
      key: 'B', name: 'Accent outline', from: 'new',
      desc: 'The same area tinted with the accent color and outlined, so it stands out from the terminal under it.',
      good: 'Easier to see.', cost: 'Still says nothing about split or swap.',
      mock: () => paneScene({ drop: '<div class="drop" style="left:215px;top:38px;width:208px;height:380px"></div>', carried: PREVIEW }),
    },
    {
      key: 'C', name: 'Says what happens', from: 't3code\'s drop badge on a dragged sidebar row ("Pin")',
      desc: 'Shaded as today, with a small badge in the middle of it: "Split Right" (Left, Up, Down) near an edge, "Swap" in the middle.',
      good: 'No guessing which drop you\'re about to make.', cost: 'A badge to keep clear of the preview.',
      mock: () => paneScene({ drop: floating(`${RIGHT_HALF};${shade};z-index:12;display:grid;place-items:center`, verbBadge('split', 'Split Right')), carried: PREVIEW }),
    },
    {
      key: 'D', name: 'A ghost of the pane in its new place', from: 'new',
      desc: 'The shaded area shows the dragged pane\'s header at its top, so you see the layout you\'ll get: npm run dev on the right of Claude Code. For a swap, its header sits over the whole pane.',
      good: 'Shows the result rather than describing it.', cost: 'Two copies of the header on screen with the preview.',
      mock: () => paneScene({ drop: floating(`${RIGHT_HALF};${shade};z-index:12;display:flex;flex-direction:column`, paneHead({ ...PANES.dev, style: 'background:rgba(40,44,51,.75);color:var(--t);opacity:.85' })), carried: PREVIEW }),
    },
  ],
});

// 4–5. Tabs -----------------------------------------------------------------------------
// Tabs: agents 0–83, server 83–154, Tab 3 154–218. server (docker compose up beside a shell)
// is dragged onto agents.
const TABS = { agents: ['agents', 'working'], server: ['server'], three: ['Tab 3'] };
const tabEl = ([name, state], { on = false, style = '' } = {}) => `<div class="tab ${on ? 'on' : ''}" style="${style}">${dot(state)}${name}</div>`;
const bar = (items) => `<div class="tabbar">${items}<span class="grow"></span><div class="end">${ibtn('plus')}</div></div>`;
const TX = 70, TY = 20;
const tabScene = (tabs, carried = '') => frame(main(tabs, tabPanes()) + carried + pointer(TX, TY), { w: 560, h: 240 });
const shadedTabs = ({ serverStyle = '' } = {}) => bar(tabEl(TABS.agents, { on: true, style: shade }) + tabEl(TABS.server, { style: serverStyle }) + tabEl(TABS.three));
const DOCKER = [`${prompt()}docker compose up`, ` ${C('tg', '✔')} Container db     Started`, ` ${C('tg', '✔')} Container redis  Started`, 'db-1    | ready to accept connections', 'redis-1 | Ready to accept connections'];
// A copy of the server tab, held where it was grabbed (64, 8), past its name.
const liftedTab = (extra = '') => floating(`left:${TX - 64}px;top:${TY - 8}px;opacity:.94;height:36px;display:flex;align-items:center;gap:6px;padding:0 14px;background:var(--panel);border:1px solid var(--b);color:var(--t);font-size:13px;box-shadow:0 8px 22px rgba(0,0,0,.55)`, `server${extra}`);
// The small preview of a tab: its name, then its panes as they're laid out, each with its
// header and the top of its screen, drawn small.
const miniHead = (lead, title) => `<div class="row g1" style="height:16px;padding:0 5px;background:var(--panel);border-bottom:1px solid var(--bv);font-size:9px;flex:none;color:var(--mu)">${lead}<span class="trunc">${title}</span></div>`;
const miniBox = (lead, title, lines) => `<div class="col" style="flex:1;min-width:0;min-height:0;overflow:hidden">${miniHead(lead, title)}${term(lines, 'font-size:6.5px;line-height:9px;padding:3px 4px')}</div>`;
const TAB_PREVIEW = floating(`left:${TX + 10}px;top:${TY + 10}px;width:230px;height:150px;border:1px solid var(--b);border-radius:6px;overflow:hidden;box-shadow:0 12px 30px rgba(0,0,0,.55);display:flex;flex-direction:column;background:var(--ed)`,
  `<div class="row g15" style="height:24px;padding:0 8px;border-bottom:1px solid var(--bv);font-size:11px;flex:none"><span>server</span><span class="mu">2 panes</span></div>
  <div class="row" style="flex:1;min-height:0;align-items:stretch">${miniBox(ic('terminal', 'xs'), 'docker compose up', DOCKER)}<div style="width:1px;background:var(--b)"></div>${miniBox(ic('terminal', 'xs'), 'Shell', SCREENS.shell)}</div>`);
TOPICS.push({
  id: 'tab-preview', section: 'Tabs', title: 'What follows the pointer when you drag a tab', size: 'medium', rec: 'C',
  now: 'The same small label as for panes follows the pointer, and the tab it\'s over is shaded; dropping puts the dragged tab in that tab\'s place. Here server (docker compose up beside a shell) is dragged onto agents.',
  options: [
    {
      key: 'A', name: 'Label (today)', from: 'Zed project panel drag',
      desc: 'The tab\'s name in a small chip just past the pointer.', good: 'Built.', cost: 'Doesn\'t look like a tab, and says nothing about what\'s in it.',
      mock: () => tabScene(shadedTabs(), todayLabel('server', TX, TY)),
    },
    {
      key: 'B', name: 'The tab itself', from: 'Zed dragged tab (workspace/pane.rs DraggedTab)',
      desc: 'A copy of the tab, with its state dot, follows the pointer held where you grabbed it, as Zed drags a tab.',
      good: 'Feels like moving the tab; it\'s small, so holding the grab point never leaves it far away.', cost: 'Two of the same tab on screen while dragging.',
      mock: () => tabScene(shadedTabs(), liftedTab()),
    },
    {
      key: 'C', name: 'A small preview of the tab', from: 'new',
      desc: 'A card about 230×150 just past the pointer: the tab\'s name and pane count, then its panes as they\'re laid out, each with its header and the top of its screen in small text. Matches the small preview for panes.',
      good: 'You see which tab you\'re moving by what\'s in it, not only its name.', cost: 'Hides some of the panes under it; the screens are a still picture taken when the drag starts.',
      mock: () => tabScene(shadedTabs(), TAB_PREVIEW),
    },
    {
      key: 'D', name: 'The tab with its panes\' icons', from: 'new',
      desc: 'The tab itself as in B, with the icons of the panes it holds after its name (an agent\'s icon for an agent, a terminal for a shell).',
      good: 'Small like B, and still hints at what\'s inside.', cost: 'Icons alone can\'t tell two shells apart.',
      mock: () => tabScene(shadedTabs(), liftedTab(`<span class="row g1 mu" style="margin-left:4px">${ic('terminal', 'xs')}${ic('terminal', 'xs')}</span>`)),
    },
    {
      key: 'E', name: 'The tab, and the panes dim', from: 'new',
      desc: 'The tab itself as in B, and the open tab\'s panes dim while you drag, so the tab bar is all that stands out.',
      good: 'Puts your eye on the tab bar, where the drop happens.', cost: 'The whole view flickers darker for a short drag.',
      mock: () => frame(main(shadedTabs(), tabPanes({ claude: { style: 'opacity:.4' }, shell: { style: 'opacity:.4' }, dev: { style: 'opacity:.4' } })) + liftedTab() + pointer(TX, TY), { w: 560, h: 240 }),
    },
  ],
});

TOPICS.push({
  id: 'tab-drop', section: 'Tabs', title: 'Where the tab will land', size: 'medium', rec: 'C',
  now: 'The tab under the pointer is shaded gray, and the dragged tab takes its place. The tab you\'re moving stays as it is. Shown here with the small preview of the tab.',
  options: [
    {
      key: 'A', name: 'Shaded tab (today)', from: 'Zed tab drop',
      desc: 'The tab it\'s over, shaded.', good: 'Matches Zed.', cost: 'Doesn\'t show whether it goes before or after that tab.',
      mock: () => tabScene(shadedTabs(), TAB_PREVIEW),
    },
    {
      key: 'B', name: 'An insertion line', from: 'new',
      desc: 'A 2-pixel accent line where it will land, in place of the shading; the tab you\'re moving dims.',
      good: 'Says exactly where it goes without covering a tab.', cost: 'A thin line is easy to miss.',
      mock: () => tabScene(bar(tabEl(TABS.agents, { on: true, style: 'box-shadow:inset 2px 0 0 var(--ac)' }) + tabEl(TABS.server, { style: 'opacity:.4' }) + tabEl(TABS.three)), TAB_PREVIEW),
    },
    {
      key: 'C', name: 'Tabs make room', from: 't3code sidebar rows (dnd-kit sortable)',
      desc: 'The tab leaves its place and the others slide aside, opening a gap where it will land; nothing is shaded. Letting go drops it into the gap.',
      good: 'You see the new order before you let go.', cost: 'The most to build: tabs move while you drag.',
      mock: () => tabScene(bar(`<div style="width:71px;border-right:1px solid var(--b);flex:none;background:rgba(116,173,232,.08)"></div>` + tabEl(TABS.agents, { on: true }) + tabEl(TABS.three)), TAB_PREVIEW),
    },
    {
      key: 'D', name: 'Accent outline', from: 'new',
      desc: 'The tab it\'s over gets an accent outline and tint in place of the gray, as for panes in B of "Where the pane will land".',
      good: 'Easier to see than gray.', cost: 'Still doesn\'t say before or after.',
      mock: () => tabScene(bar(tabEl(TABS.agents, { on: true, style: 'background:rgba(116,173,232,.18);box-shadow:inset 0 0 0 1.5px var(--ac)' }) + tabEl(TABS.server) + tabEl(TABS.three)), TAB_PREVIEW),
    },
  ],
});

// 6. Workspace rows ----------------------------------------------------------------------
// Rows are about 54 pixels apart from y 40: storefront, brave-otter, api, Release notes, ~.
// api is dragged onto storefront.
const space = (id) => SPACES.find((s) => s.id === id);
const row = (id, { style = '' } = {}) => currentRow(space(id), { active: id === 'st' }).replace('<div class="srow', `<div style="${style}" class="srow`);
const RX = 150, RY = 62;
const rowScene = (list, carried = '') => frame(sidebar({ list }) + carried + pointer(RX, RY), { w: 290, h: 330 });
const todayRows = ({ apiStyle = '' } = {}) => row('st', { style: shade }) + row('bo') + row('ap', { style: apiStyle }) + row('rn') + row('hm');
const liftedRow = (y) => floating(`left:4px;top:${y}px;width:290px`, currentRow(space('ap'), {}).replace('<div class="srow', '<div style="background:var(--sel);box-shadow:0 10px 24px rgba(0,0,0,.55)" class="srow'));
TOPICS.push({
  id: 'row', section: 'Sidebar', title: 'Dragging a workspace row', size: 'narrow', rec: 'C',
  now: 'The same small label follows the pointer, and the row it\'s over is shaded; dropping puts the dragged workspace in that row\'s place. Here api is dragged onto storefront.',
  options: [
    {
      key: 'A', name: 'Label (today)', from: 'Zed project panel drag',
      desc: 'The workspace\'s name in a small chip just past the pointer; the row it\'s over shaded.', good: 'Built.', cost: 'Doesn\'t look like the row.',
      mock: () => rowScene(todayRows(), todayLabel('api', RX, RY)),
    },
    {
      key: 'B', name: 'The row lifts', from: 't3code dragged sidebar row (an opaque card with a shadow)',
      desc: 'A copy of the row, opaque with a shadow, follows the pointer held where you grabbed it; its own place dims, and the row it\'s over is shaded as today.',
      good: 'You carry the row itself, with its branch and counts.', cost: 'The shading under the lifted row is partly hidden.',
      mock: () => rowScene(todayRows({ apiStyle: 'opacity:.35' }), liftedRow(RY - 12)),
    },
    {
      key: 'C', name: 'Rows make room', from: 't3code sidebar rows (dnd-kit sortable)',
      desc: 'The lifted row follows the pointer and the others slide down to open a gap where it will land, as t3code\'s sidebar does; nothing is shaded.',
      good: 'You see the new order before you let go; the sidebar\'s model.', cost: 'Rows move while you drag: the most to build.',
      mock: () => rowScene('<div style="height:52px;flex:none"></div>' + row('st') + row('bo') + row('rn') + row('hm'), liftedRow(RY - 18)),
    },
    {
      key: 'D', name: 'Icon and name', from: 'Zed project panel drag (icon and file name)',
      desc: 'The label as today, with the workspace\'s icon before its name, as Zed\'s project panel drags a file.',
      good: 'A small step from today; the icon tells workspaces apart.', cost: 'Still a label.',
      mock: () => rowScene(todayRows(), floating(`left:${RX + 12}px;top:${RY + 12}px;background:var(--panel);border:1px solid var(--b);border-radius:6px;padding:3px 8px;font-size:12px;box-shadow:0 6px 18px rgba(0,0,0,.5);display:flex;gap:6px;align-items:center`, `${mono('AP', 't')}api`)),
    },
  ],
});
