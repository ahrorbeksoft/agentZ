// Dragging in the Workspaces view: what you see while moving a pane, a tab or a workspace row.

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
const tabPanes = () => split('h', 0.56, tabPane('claude'), split('v', 0.5, tabPane('shell'), tabPane('dev', { focus: true })));

// The small preview: the pane's header and the top of its screen, drawn small.
const miniPane = (name, x, y, { w = 240, h = 150, style = '' } = {}) => {
  const p = PANES[name];
  return floating(`left:${x}px;top:${y}px;width:${w}px;height:${h}px;border:1px solid var(--b);border-radius:6px;overflow:hidden;box-shadow:0 12px 30px rgba(0,0,0,.55);display:flex;flex-direction:column;${style}`,
    `<div class="row g15" style="height:24px;padding:0 8px;background:var(--ed);border-bottom:1px solid var(--bv);font-size:11px;flex:none">${p.lead}<span>${p.title}</span><span class="mu trunc">storefront</span></div>${term(p.body, 'font-size:8.5px;line-height:12px;padding:4px 6px')}`);
};

// 1–2. Panes: the tab shows the result as you drag ---------------------------------------
// The main area is 760×420: tab bar 36, Claude Code x 0–425, the right column x 426–760
// (Shell above y 228, npm run dev below). On a 9-second loop, npm run dev, grabbed by the icon
// in its header, goes to Claude Code's right edge (it would split off there), on to the middle
// of what's left of Claude Code (the two would swap), over to Claude Code in its old place
// (swapping back) and home. Times are percentages of the loop.
const LIVE_S = 9;
const SPOTS = { own: [436, 248], right: [385, 170], swap: [106, 228], back: [593, 325] };
const PATH = [[0, 'own'], [8, 'own'], [22, 'right'], [40, 'right'], [52, 'swap'], [64, 'swap'], [78, 'back'], [88, 'back'], [98, 'own'], [100, 'own']];
// The layout changes when the pointer crosses into a new drop spot along PATH, or, for `rest`,
// once it stops there.
const CROSSINGS = [[11, 'right'], [49.2, 'swap'], [74.3, 'own']];
const MOVES = {
  slide: { changes: CROSSINGS, span: 2 },
  jump: { changes: CROSSINGS, span: 0.15 },
  rest: { changes: [[24.2, 'right'], [54.2, 'swap'], [80.2, 'own']], span: 2 },
};
// Every layout on the loop, as the sizes of one set of tiles: a place left of Claude Code,
// Claude Code, a place on its right, then the right column: Shell, a place below it (npm run
// dev's own) and Claude Code once it's swapped there.
const LAYOUTS = {
  own: { placeLeft: 0, claude: 0.56, placeRight: 0, shell: 0.5, placeBelow: 0.5, claudeBelow: 0 },
  right: { placeLeft: 0, claude: 0.28, placeRight: 0.28, shell: 1, placeBelow: 0, claudeBelow: 0 },
  swap: { placeLeft: 0.56, claude: 0, placeRight: 0, shell: 0.5, placeBelow: 0, claudeBelow: 0.5 },
};
// The divider before a tile, drawn as its border so it goes when the tile shrinks to nothing.
const EDGES = { placeRight: 'left', placeBelow: 'top', claudeBelow: 'top' };
const liveKeyframes = (move) => {
  const { changes, span } = MOVES[move];
  const size = (tile, layout) => {
    const grow = LAYOUTS[layout][tile];
    return `flex-grow: ${grow}${EDGES[tile] ? `; border-${EDGES[tile]}-width: ${grow ? 1 : 0}px` : ''}`;
  };
  const tiles = Object.keys(LAYOUTS.own).map((tile) => {
    let layout = 'own';
    const frames = [`0% { ${size(tile, layout)} }`];
    for (const [at, next] of changes) {
      frames.push(`${at}% { ${size(tile, layout)} }`, `${at + span}% { ${size(tile, next)} }`);
      layout = next;
    }
    frames.push(`100% { ${size(tile, layout)} }`);
    return `@keyframes dzl-${move}-${tile} { ${frames.join(' ')} }`;
  });
  const [x0, y0] = SPOTS.own;
  const path = PATH.map(([at, spot]) => `${at}% { transform: translate(${SPOTS[spot][0] - x0}px, ${SPOTS[spot][1] - y0}px) }`).join(' ');
  return `<style>${tiles.join('\n')}\n@keyframes dzl-pointer { ${path} }</style>`;
};
const liveTile = (move, tile, html) => `<div style="flex:0 1 0;min-width:0;min-height:0;overflow:hidden;display:flex;${EDGES[tile] ? `border-${EDGES[tile]}:0 solid var(--b);` : ''}animation:dzl-${move}-${tile} ${LIVE_S}s ease-in-out infinite">${html}</div>`;
// `place` is what fills the dragged pane's place, wherever that is at the moment.
const liveScene = (place, move = 'slide') => {
  const [x, y] = SPOTS.own;
  const layout = `<div class="tile h" style="flex:1">${liveTile(move, 'placeLeft', place)}${liveTile(move, 'claude', tabPane('claude'))}${liveTile(move, 'placeRight', place)}<div class="tile v" style="flex:.44 1 0;min-width:0;border-left:1px solid var(--b)">${liveTile(move, 'shell', tabPane('shell'))}${liveTile(move, 'placeBelow', place)}${liveTile(move, 'claudeBelow', tabPane('claude'))}</div></div>`;
  const carried = `<div style="position:absolute;left:0;top:0;z-index:30;animation:dzl-pointer ${LIVE_S}s linear infinite">${miniPane('dev', x + 10, y + 10, { w: 200, h: 124 })}${pointer(x, y)}</div>`;
  return frame(liveKeyframes(move) + main(tabBar([['agents', 'working'], ['server'], ['Tab 3']]), layout, carried), { w: 760, h: 420 });
};
const outlined = (color, inner = '', style = '') => `<div class="pane"><div style="flex:1;margin:6px;border:1.5px dashed ${color};border-radius:6px;display:grid;place-items:center;white-space:nowrap;overflow:hidden;${style}">${inner}</div></div>`;
const PLACES = {
  dimmed: tabPane('dev', { style: 'opacity:.45' }),
  empty: outlined('var(--b)'),
  dropHere: outlined('var(--ac)', '<span class="sm" style="color:var(--ac)">Drop here</span>', 'background:rgba(116,173,232,.1)'),
  named: outlined('var(--b)', `<span class="row g15 sm mu">${ic('terminal', 'sm')}npm run dev</span>`),
  header: tabPane('dev', { style: 'opacity:.7', body: '<div style="flex:1;background:var(--term)"></div>' }),
  solid: tabPane('dev', { style: 'border:1.5px solid var(--ac)' }),
};

TOPICS.push({
  id: 'pane-live', section: 'Panes', title: 'The tab shows the result as you drag', size: 'wide', rec: 'D',
  now: 'Today a small label follows the pointer, the pane stays where it is, and the spot it would take is shaded (the screenshot). In all of these, the tab shows the result while you drag instead: the pane leaves its place, and the other panes move to make room for it where it would land, so what you see is what you get when you let go. The small preview of the pane follows the pointer. Pointing at its new place changes nothing; pointing at another pane moves it there (near an edge it splits that pane, in the middle the two swap); back over its own place, everything is as it was. Here npm run dev goes to Claude Code\'s right edge, swaps with it, swaps back and goes home. The options differ in what fills its place while you drag.',
  nowImg: 'img/now-pane-drag.jpg',
  options: [
    {
      key: 'A', name: 'The pane, dimmed', from: 'new',
      desc: 'The pane itself, its header and screen, at 45%, at the size it will get.',
      good: 'The truest picture of the result.', cost: 'Its screen shows twice, there and in the small preview; and it\'s cut to the new size until you drop it and it redraws.',
      mock: () => liveScene(PLACES.dimmed),
    },
    {
      key: 'B', name: 'An empty place', from: 'new',
      desc: 'A dashed outline with nothing inside.',
      good: 'Quiet; the small preview says what\'s coming.', cost: 'Reads as a gap; nothing there says which pane will fill it.',
      mock: () => liveScene(PLACES.empty),
    },
    {
      key: 'C', name: '"Drop here"', from: 'new',
      desc: 'The place tinted with the accent color and outlined in it, with "Drop here" in the middle.',
      good: 'Impossible to miss.', cost: 'Loud, and the words state the obvious once you know how it works.',
      mock: () => liveScene(PLACES.dropHere),
    },
    {
      key: 'D', name: 'Its icon and name', from: 'new',
      desc: 'A dashed outline with the pane\'s icon and title in the middle (an agent\'s icon for an agent).',
      good: 'Says which pane goes there, and its screen is in the small preview, so nothing shows twice.', cost: 'Shows the result\'s shape, not how the pane will look in it.',
      mock: () => liveScene(PLACES.named),
    },
    {
      key: 'E', name: 'Its header, no screen', from: 'new',
      desc: 'The pane\'s real header with its buttons, a little dimmed, over a blank screen.',
      good: 'Looks like the pane will, without a cut-off copy of its screen.', cost: 'A blank screen can look like the terminal was cleared.',
      mock: () => liveScene(PLACES.header),
    },
    {
      key: 'F', name: 'The pane, outlined', from: 'new',
      desc: 'The pane at full strength in its new place, with an accent outline to say it isn\'t dropped yet.',
      good: 'Exactly the result.', cost: 'Easy to forget you\'re still dragging; its screen shows twice, as in A.',
      mock: () => liveScene(PLACES.solid),
    },
  ],
});

TOPICS.push({
  id: 'pane-motion', section: 'Panes', title: 'How the panes move to the new layout', size: 'wide', rec: 'A',
  now: 'Nothing moves today. The same drag as in the topic before, shown with its D (icon and name); only the way the panes get from one layout to the next differs.',
  options: [
    {
      key: 'A', name: 'Slide', from: 't3code sidebar rows (dnd-kit eases rows into place)',
      desc: 'As soon as the pointer crosses into a new spot, the panes glide to their new sizes, in about 150 ms.',
      good: 'Easy to follow what went where.', cost: 'Sweeping the pointer across the tab sets panes sliding back and forth.',
      mock: () => liveScene(PLACES.named, 'slide'),
    },
    {
      key: 'B', name: 'Jump', from: 'new',
      desc: 'The panes snap to the new layout at once.',
      good: 'Never behind the pointer.', cost: 'Hard to see what moved; a sweep flickers.',
      mock: () => liveScene(PLACES.named, 'jump'),
    },
    {
      key: 'C', name: 'Only when you pause', from: 'new',
      desc: 'Nothing moves while the pointer moves. Once it rests for about 200 ms, the panes slide to the layout for that spot.',
      good: 'Calm on a long drag: the layout changes only where you stop.', cost: 'A beat behind; letting go quickly drops it in a spot you haven\'t seen the result of.',
      mock: () => liveScene(PLACES.named, 'rest'),
    },
  ],
});

// 3. Tabs -------------------------------------------------------------------------------
// The tab sliding along the bar, on a 6-second loop: nine tabs, more than the 523-pixel bar
// holds; server, grabbed 50 pixels in, slides left past agents (which moves over to make room)
// and back.
const SLIDE = [['agents', 'working', 84], ['server', null, 72], ['Tab 3', null, 64], ['tests', null, 64], ['docs', null, 60], ['deploy', null, 72], ['logs', null, 60], ['review', null, 70], ['Tab 9', null, 64]];
const BAR_W = 523, SLIDE_GRAB = 50, TY = 20;
const SLIDE_MOTION = `<style>
@keyframes dz-drag { 0%, 8% { transform: translateX(0) } 40%, 60% { transform: translateX(-78px) } 92%, 100% { transform: translateX(0) } }
@keyframes dz-room { 0%, 24% { transform: translateX(0) } 30%, 76% { transform: translateX(72px) } 82%, 100% { transform: translateX(0) } }
@keyframes dz-hop { 0%, 24% { transform: translateX(0) } 26%, 76% { transform: translateX(72px) } 78%, 100% { transform: translateX(0) } }
@keyframes dz-jump { 0%, 24% { transform: translateX(0) } 26%, 76% { transform: translateX(-84px) } 78%, 100% { transform: translateX(0) } }
@keyframes dz-scroll { 0%, 12% { transform: translateX(0) } 70%, 90% { transform: translateX(-87px) } 100% { transform: translateX(0) } }
@keyframes dz-fade { 0%, 12% { opacity: 0 } 30%, 90% { opacity: 1 } 100% { opacity: 0 } }
</style>`;
const loop = (name) => `animation:${name} 6s ease-in-out infinite`;
const slideTab = ([name, state, w], x, { on = false, style = '' } = {}) => `<div class="tab ${on ? 'on' : ''}" style="position:absolute;top:0;left:${x}px;width:${w}px;height:${on ? 36 : 35}px;margin:0;${style}">${dot(state)}${name}</div>`;
const slideBar = (inner) => `<div class="tabbar"><div style="position:relative;flex:1;align-self:flex-start;height:36px;overflow:hidden">${inner}</div><div class="end">${ibtn('plus')}</div></div>`;
const PLAIN = 'background:var(--panel);border-left:1px solid var(--b);z-index:2';
const RAISED = 'background:var(--sel);color:var(--t);border:1px solid #5a606c;height:36px;box-shadow:0 4px 14px rgba(0,0,0,.6);z-index:2';
// The bar with server carried by `motion`, and agents making room by `room`.
const slidingTabs = (look, { motion = 'dz-drag', room = 'dz-room' } = {}) => {
  let x = 0;
  return slideBar(SLIDE.map((tab) => {
    const left = x;
    x += tab[2];
    if (tab[0] === 'server') return slideTab(tab, left, { style: `${look};${loop(motion)}` });
    return slideTab(tab, left, { on: tab[0] === 'agents', style: tab[0] === 'agents' ? loop(room) : '' });
  }).join(''));
};
// Held at the bar's right end: the other tabs scroll left under it, and the left end fades
// once tabs are out of sight there.
const scrollingTabs = () => {
  let x = 0;
  const others = SLIDE.filter(([name]) => name !== 'server').map((tab) => {
    const left = x;
    x += tab[2];
    return slideTab(tab, left, { on: tab[0] === 'agents' });
  }).join('');
  return slideBar(`<div style="position:absolute;left:0;top:0;right:${SLIDE[1][2]}px;height:36px;overflow:hidden"><div style="position:absolute;inset:0;${loop('dz-scroll')}">${others}</div><div style="position:absolute;left:0;top:0;width:56px;height:35px;background:linear-gradient(90deg,var(--panel) 20%,transparent);z-index:3;${loop('dz-fade')}"></div></div>${slideTab(SLIDE[1], BAR_W - SLIDE[1][2], { style: RAISED })}`);
};
const slideScene = (bar, { pointerX = SLIDE[0][2] + SLIDE_GRAB, motion = loop('dz-drag') } = {}) =>
  frame(SLIDE_MOTION + main(bar, tabPanes()) + `<div style="position:absolute;left:0;top:0;z-index:40;${motion}">${pointer(pointerX, TY)}</div>`, { w: 560, h: 240 });

TOPICS.push({
  id: 'tab-slide', section: 'Tabs', title: 'The tab slides along the bar', size: 'medium', rec: 'B',
  now: 'Today the tab\'s name follows the pointer in a small label and the tab under it is shaded; letting go puts it in that tab\'s place. In all of these, the tab itself moves instead, only sideways: it stays in the bar however far up or down the pointer goes. As its edge passes the middle of the next tab, that tab slides over into its old place, so the order changes while you drag, and letting go leaves it where it is. When there are more tabs than fit, holding it near an end scrolls the bar. Each one moves: server slides left past agents and back.',
  options: [
    {
      key: 'A', name: 'Slides, as it is', from: 't3code sidebar rows (dnd-kit, held to one axis), turned sideways',
      desc: 'The tab keeps its usual look and moves with the pointer.',
      good: 'Plain; the motion alone says what\'s happening.', cost: 'A tab that isn\'t open looks like any other while it moves.',
      mock: () => slideScene(slidingTabs(PLAIN)),
    },
    {
      key: 'B', name: 'Slides, raised', from: 'Zed dragged tab (a copy held where you grabbed it), kept in the bar',
      desc: 'As A, and the tab is raised while you hold it: a lighter background, full-strength text and a shadow, so it stands apart from the tabs it passes.',
      good: 'Always clear which tab is in your hand.', cost: 'A shadow in a 36-pixel bar can look heavy.',
      mock: () => slideScene(slidingTabs(RAISED)),
    },
    {
      key: 'C', name: 'Jumps from place to place', from: 'new',
      desc: 'Nothing floats. The tab stays lined up with the others, and when the pointer passes the middle of the next tab, the two swap at once.',
      good: 'Simplest to build; the bar is always neat.', cost: 'Moves in jumps, and between them the tab lags behind the pointer.',
      mock: () => slideScene(slidingTabs(PLAIN, { motion: 'dz-jump', room: 'dz-hop' })),
    },
    {
      key: 'D', name: 'At the ends, faster and faded', from: 't3code sidebar (dnd-kit scrolls its list faster nearer the end)',
      desc: 'Add to any of the above: held near an end, the tab stays put and the bar scrolls under it, faster the closer to the edge, and the end with tabs out of sight fades out.',
      good: 'Quick to reach far tabs, and you can see there are more.', cost: 'Easy to overshoot when it scrolls fast.',
      mock: () => slideScene(scrollingTabs(), { pointerX: BAR_W - SLIDE[1][2] + SLIDE_GRAB, motion: '' }),
    },
  ],
});

// 4. Workspace rows ----------------------------------------------------------------------
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
