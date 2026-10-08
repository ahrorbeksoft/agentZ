// A thread's terminal (terminal_drawer.rs, hosted by agent_view.rs): how many shells it has,
// which buttons it keeps and where, what closing does, where it sits, and how it resizes.

ICONS.send = '<path d="m22 2-7 20-4-9-9-4Z"/><path d="M22 2 11 13"/>';
ICONS.square = '<rect width="18" height="18" x="3" y="3" rx="2"/>';
ICONS.pointer = '<path d="M4 3l7 17 2.5-7.5L21 10z" fill="#fff" stroke="#000" stroke-width="1.4"/>';

// The toolbar today (render_actions): split side by side, split stacked, new, full screen,
// close, as XSmall icon buttons.
const TODAY = ['split', 'split-v', 'plus', 'maximize', 'x'];
// The buttons the placement and later mocks use: the recommended set.
const REC = ['plus', 'maximize', 'x'];
const TIPS = { split: 'Split Terminal Horizontally', 'split-v': 'Split Terminal Vertically', plus: 'New Terminal', maximize: 'Full Screen', minimize: 'Exit Full Screen', x: 'Close Terminal' };

const tb = (name, { on = false } = {}) => `<span class="ibtn" style="width:18px;height:18px;border-radius:4px;${on ? 'background:var(--sel);color:var(--t)' : ''}">${ic(name, 'xs')}</span>`;
const buttons = (names) => names.map((name) => tb(name)).join('');
// The bordered box the toolbar sits in while there's one shell.
const box = (names, style = '') => `<span class="row" style="border:1px solid var(--bv);border-radius:6px;background:var(--ed);padding:1px;${style}">${buttons(names)}</span>`;
const floating = (names, style = 'top:4px;right:8px') => box(names, `position:absolute;${style};z-index:5`);
const tooltip = (text, style, key = '') => `<span style="position:absolute;${style};z-index:25;background:var(--panel);border:1px solid var(--b);border-radius:6px;padding:4px 8px;font-size:12px;white-space:nowrap;box-shadow:0 6px 16px rgba(0,0,0,.4)">${text}${key ? `<span class="ph" style="margin-left:10px">${key}</span>` : ''}</span>`;

// Not part of the app: today's toolbar beside what the option changes.
const legend = (change) => `<div class="row" style="height:30px;flex:none;gap:8px;padding:0 12px;background:#16181d;border-bottom:1px solid #0d0f12;font:11px/1 -apple-system,'IBM Plex Sans',sans-serif;color:#8b919d;white-space:nowrap">Today ${box(TODAY)}<span style="display:inline-flex">${ic('arrow', 'xs')}</span><span style="color:#dce0e5">${change}</span></div>`;

// Screens: a test run whose lines reach the toolbar's corner, a dev server, a fresh shell.
const TESTS = [
  ` ${C('tg', B('PASS'))}  src/app/cart/total.test.ts  ${C('tg', '✓')} rounds once at the end (3 ms)  ${C('tg', '✓')} adds many cheap items (1 ms)  ${C('tg', '✓')} keeps the receipt total (1 ms)`,
  ` ${C('tg', B('PASS'))}  src/app/checkout/page.test.tsx  ${C('tg', '✓')} renders the pay button (12 ms)  ${C('tg', '✓')} disables pay while loading (4 ms)  ${C('tg', '✓')} shows the receipt (6 ms)`,
  '',
  `${B('Test Suites:')} ${C('tg', '2 passed')}, 2 total`,
  `${B('Tests:')}       ${C('tg', '6 passed')}, 6 total`,
  `${B('Time:')}        2.104 s`,
  `${C('faint', 'Ran all test suites.')}`,
  `${prompt()}<span class="cursor"></span>`,
];
const DEV = SCREENS.dev;
const FRESH = SCREENS.shell;

// The thread around the terminal (agent_view.rs): its header, the conversation, the composer.
function header({ terminalOn = true, running = false, menu = false } = {}) {
  const terminal = `<span style="position:relative;display:inline-flex">${ibtn('terminal', terminalOn ? 'on' : '')}${running ? '<span style="position:absolute;top:3px;right:3px;width:6px;height:6px;border-radius:50%;background:var(--ac)"></span>' : ''}</span>`;
  const chevron = menu ? `<span class="ibtn sm on" style="margin-left:-6px;width:16px">${ic('chev-down', 'xs')}</span>` : '';
  return `<div class="row" style="height:36px;flex:none;padding:0 8px;gap:4px;border-bottom:1px solid var(--b);background:var(--panel)">
    <span class="row g15" style="padding:2px 4px">${mono('ST', 'g')}<span class="sm mu">storefront</span></span><span class="sm mu">/</span>
    <span class="grow trunc" style="padding:0 4px;font-size:13px">Add the checkout page with a pay button</span>
    <span class="row g1 sm mu" style="height:22px;padding:0 6px;border:1px solid var(--bv);border-radius:4px;flex:none">${ic('branch', 'xs')}checkout-flow</span>
    <span class="row" style="gap:6px;margin-left:6px;flex:none"><span class="btn sm" style="height:22px;padding:0 6px;gap:4px"><span class="okc">+24</span><span class="delc">−3</span></span>${terminal}${chevron}${ibtn('more')}</span></div>`;
}
const column = 'width:100%;max-width:680px;margin:0 auto;padding:0 20px;box-sizing:border-box';
const conversation = () => `<div class="grow" style="min-height:0;overflow:hidden;background:var(--panel)"><div class="col" style="${column};padding-top:16px;gap:10px">
  <div style="border:1px solid var(--b);border-radius:6px;background:var(--ed);padding:8px 12px">Add the checkout page with a pay button, and a test for the total</div>
  <div class="row g2 sm mu" style="height:22px">${ic('file', 'sm')}Read src/app/cart/page.tsx</div>
  <div class="row g2 sm mu" style="height:22px">${ic('pencil', 'sm')}Edit src/app/checkout/page.tsx <span class="okc">+24</span> <span class="delc">−3</span></div>
  <div style="line-height:22px">The checkout page has a pay button now, and the total rounds once, at the end. I added tests for both, and they pass. Run <code style="font:12px 'IBM Plex Mono',monospace;background:var(--ed);padding:1px 4px;border-radius:3px">npm run dev</code> to try it.</div>
</div></div>`;
const composer = () => `<div style="flex:none;border-top:1px solid var(--b);background:var(--ed);padding:8px 0 10px"><div style="${column}">
  <div class="ph" style="height:24px;line-height:24px">Message the agent…</div>
  <div class="row sm mu" style="height:26px;gap:12px;margin-top:6px"><span class="row g15">${glyph('claude')}Claude Agent</span><span class="grow"></span><span class="row g1">Opus 5.5 ${ic('chev-down', 'xs')}</span><span class="ac" style="display:inline-flex">${ic('send')}</span></div>
</div></div>`;

// The terminal area: a border on top, the terminal background, its own contents.
const area = (inner, { h = 280, full = false, style = '' } = {}) => `<div class="col" style="position:relative;${full ? 'flex:1;min-height:0' : `height:${h}px;flex:none`};border-top:1px solid var(--b);background:var(--term);${style}">${inner}</div>`;
const shell = (screen, extra = '', style = '') => `<div class="row" style="flex:1;min-height:0;min-width:0;position:relative;align-items:stretch;padding-top:4px;${style}">${term(screen, 'min-width:0')}${extra}</div>`;

function thread(drawer, { w = 900, h = 620, top = '', head = header(), full = false, place = 'below', extra = '' } = {}) {
  let body;
  if (full) body = drawer;
  else if (place === 'above') body = conversation() + drawer + composer();
  else if (place === 'side') body = `<div class="row grow" style="min-height:0;align-items:stretch"><div class="col grow" style="min-width:0">${conversation()}${composer()}</div>${drawer}</div>`;
  else body = conversation() + composer() + drawer;
  return `<div class="m" style="width:${w}px;height:${h}px;display:flex;flex-direction:column;background:var(--panel)">${top}${head}${body}${extra}</div>`;
}

// A strip above the shell, as Zed's terminal panel has: tabs on the left, buttons on the right.
function strip(tabs, { active = 0, right = ['plus', 'maximize'], closeOnTabs = true } = {}) {
  const items = tabs.map((name, index) => {
    const on = index === active;
    return `<div class="row g15" style="height:100%;padding:0 8px 0 10px;border-right:1px solid var(--bv);font-size:12px;white-space:nowrap;${on ? 'background:var(--term);color:var(--t);margin-bottom:-1px' : 'color:var(--mu)'}">${ic('terminal', 'xs')}<span>${name}</span>${closeOnTabs ? `<span style="display:inline-flex;color:var(--mu);${on ? '' : 'visibility:hidden'}">${ic('x', 'xs')}</span>` : ''}</div>`;
  }).join('');
  return `<div class="row" style="height:30px;flex:none;background:var(--panel);border-bottom:1px solid var(--bv)">${items}<span class="grow"></span><span class="row" style="gap:2px;padding:0 6px">${buttons(right)}</span></div>`;
}

// Today's list of groups and shells, 144 pixels wide, with the toolbar on top (render_list).
function todayList() {
  const head = (label, icon, count, on) => `<div class="row g1" style="height:22px;padding:0 6px;border-radius:4px;${on ? 'background:var(--sel)' : ''}">${ic(icon, 'xs mu')}<span class="xs grow ${on ? '' : 'mu'}">${label}</span><span class="xs mu">${count}</span></div>`;
  const row = (number, on) => `<div class="row g1" style="height:24px;padding:0 8px 0 4px;border-radius:6px;${on ? 'background:var(--sel)' : ''}"><span style="width:16px;display:inline-grid;place-items:center">${ic('terminal', 'xs mu')}</span><span class="sm ${on ? '' : 'mu'}">Terminal ${number}</span></div>`;
  return `<div class="col" style="width:144px;flex:none;height:100%;border-left:1px solid var(--bv)">
    <div class="row" style="height:22px;flex:none;justify-content:flex-end;border-bottom:1px solid var(--bv)">${buttons(TODAY)}</div>
    <div class="col" style="padding:4px;gap:2px">${head('Side by side', 'split', 2, true)}${row(1, false)}${row(2, true)}<div style="height:4px"></div>${head('Single', 'square', 1, false)}${row(3, false)}</div></div>`;
}
const sideBySide = (a, b, { activeRight = true } = {}) => `<div class="row" style="flex:1;min-height:0;min-width:0;align-items:stretch">${a}<div style="width:1px;flex:none;background:${activeRight ? 'var(--b)' : 'var(--bv)'}"></div>${b}</div>`;

// A small title bar on each split, as herdr's panes and Workspaces' have, shorter for the drawer.
const splitHead = (name, focus, right = []) => `<div class="row g15" style="height:24px;flex:none;padding:0 4px 0 8px;font-size:12px;border-bottom:1px solid var(--bv);background:${focus ? 'var(--term)' : 'var(--panel)'};color:${focus ? 'var(--t)' : 'var(--mu)'}">${ic('terminal', 'xs')}<span class="grow trunc">${name}</span>${buttons(right)}</div>`;
const splitCell = (name, screen, focus, right = []) => `<div class="col" style="flex:1;min-width:0;border:1px solid ${focus ? 'var(--bf)' : 'transparent'}">${splitHead(name, focus, right)}${shell(screen)}</div>`;

const menuAt = (items, style) => `<div class="menu" style="${style}">${items.map((item) => (item === '-' ? '<div class="hr"></div>' : `<div class="it ${item.hl ? 'hl' : ''} ${item.danger ? 'danger' : ''}">${item.icon ? ic(item.icon, 'sm') : ''}<span class="grow">${item.label}</span>${item.key ? `<span class="kb">${item.key}</span>` : ''}</div>`)).join('')}</div>`;
const pointer = (style) => `<span style="position:absolute;${style};z-index:30;display:inline-flex">${ic('pointer', 'lg')}</span>`;
const ZED_MENU = [{ label: 'New Terminal' }, '-', { label: 'Copy' }, { label: 'Paste' }, { label: 'Select All' }, { label: 'Clear' }, '-', { label: 'Close Terminal' }];

// The question Workspaces panes ask before ending a program (spaces_view.rs confirm_close).
const closeAlert = (title, detail) => `<div style="position:absolute;left:50%;top:50%;transform:translate(-50%,-50%);width:260px;background:#3a3f4a;border:1px solid #555b67;border-radius:12px;box-shadow:0 18px 44px rgba(0,0,0,.55);padding:18px 16px 14px;text-align:center;z-index:20">
  <div style="width:44px;height:44px;margin:0 auto 10px;border-radius:10px;background:#e5a23b;display:grid;place-items:center;color:#1b1f26">${ic('alert', 'lg')}</div>
  <div class="b6" style="font-size:13px">${title}</div><div class="sm mu" style="margin-top:6px;line-height:1.4">${detail}</div>
  <div class="col g1" style="margin-top:14px"><span style="height:26px;border-radius:6px;display:grid;place-items:center;font-size:13px;background:#3a82f7;color:#fff">Close</span><span style="height:26px;border-radius:6px;display:grid;place-items:center;font-size:13px;background:#555b67">Cancel</span></div></div>`;

// 1. How many shells -----------------------------------------------------------------------
TOPICS.push({
  id: 'shells', section: 'Shells', title: 'One shell, tabs or splits', size: 'wide', rec: 'B',
  now: 'A thread’s terminal starts with one shell. + adds a shell in a group of its own, and each split button adds one beside or under the shell in front, up to four in a group. One group shows at a time. With two shells or more, a list 144 pixels wide on the right names the groups (Single, Side by side, Stacked) and the shells (Terminal 1, Terminal 2), and the toolbar moves into it. The other topics depend on this pick.',
  nowImg: '../feedback/evidence/10-thread-terminal.png',
  issues: [
    'There are two ways to have several shells: groups, which show one at a time, and splits inside a group.',
    'The second shell brings in the list, which takes 144 pixels from the terminal, and the buttons jump from the corner into it.',
    'Splits always share the space evenly. There’s no edge to drag between them.',
    'Shells are named by number (Terminal 1, Terminal 2), not by what runs in them.',
  ],
  options: [
    { key: 'A', name: 'One shell', from: 'new',
      desc: 'Each thread has one shell. No +, no splits, no list. For more shells, or shells side by side, open a Workspaces tab.',
      good: 'The least to learn, and the shell always has the full width.', cost: 'A dev server and a free shell at once need Workspaces.',
      mock: () => thread(area(strip(['zsh'], { right: ['maximize', 'x'], closeOnTabs: false }) + shell(TESTS)), { top: legend('No +, no splits, no list') }) },
    { key: 'B', name: 'Several, as tabs', from: 'Zed, without splits',
      desc: '+ adds a shell as a tab in a strip above the terminal. One shows at a time, at full width. Each tab is named by what runs in it (zsh, npm run dev), as Workspaces panes are, and has its own ×.',
      good: 'Several shells, one way to switch, and nothing moves when you add one.', cost: 'You can’t watch two shells at once here.',
      mock: () => thread(area(strip(['zsh', 'npm run dev', 'npm test'], { active: 1 }) + shell(DEV)), { top: legend('Tabs instead of groups and splits') }) },
    { key: 'C', name: 'Tabs, and splits from a menu', from: 'Zed',
      desc: 'Zed’s terminal panel: tabs as in B, plus a Split button whose menu has Split Right, Split Left, Split Up and Split Down. Each split has its own tabs, and the edge between them drags. Zed names tabs by folder and program.',
      good: 'Everything Zed’s panel does.', cost: 'The most to build, and a second set of panes beside Workspaces.',
      mock: () => thread(area(sideBySide(
        `<div class="col" style="flex:1;min-width:0">${strip(['storefront — zsh', 'storefront — npm run dev'], { right: [] })}${shell(TESTS)}</div>`,
        `<div class="col" style="flex:1;min-width:0">${strip(['storefront — npm test'], { right: ['plus', 'split', 'maximize'] })}${shell(FRESH)}</div>`)), {
        top: legend('Tabs, and one Split button with a menu'),
        extra: menuAt([{ label: 'Split Right', hl: true }, { label: 'Split Left' }, { label: 'Split Up' }, { label: 'Split Down' }], 'right:10px;top:372px;min-width:150px') }) },
    { key: 'D', name: 'Splits only', from: 'herdr',
      desc: 'Every shell is on screen, as panes are in a herdr tab. Split adds one beside the shell in front, each split has a small title bar, and the edges drag. No tabs, no groups.',
      good: 'You see every shell at once.', cost: 'A terminal 280 pixels tall gets cramped past two or three shells.',
      mock: () => thread(area(`<div class="row" style="flex:1;min-height:0;align-items:stretch">${splitCell('zsh', TESTS, false)}${splitCell('npm run dev', DEV, true, ['split', 'split-v', 'maximize', 'x'])}</div>`), { top: legend('Splits on the shells, no groups or list') }) },
    { key: 'E', name: 'Groups and splits, as today', from: 't3code',
      desc: 'Keep t3code’s groups of up to four splits, shown one at a time, with the list on the right once there are two shells.',
      good: 'Nothing changes.', cost: 'Keeps everything listed under Today.',
      mock: () => thread(area(`<div class="row" style="flex:1;min-height:0;align-items:stretch">${sideBySide(shell(TESTS), shell(DEV))}${todayList()}</div>`), { top: legend('No change') }) },
  ],
});

// 2. Which buttons -------------------------------------------------------------------------
const buttonsPiece = (names, change, extra = '') => `<div class="m" style="width:470px;height:270px;display:flex;flex-direction:column">${legend(change)}${area(shell(TESTS, floating(names) + extra), { full: true, style: 'border-top:0' })}</div>`;

TOPICS.push({
  id: 'buttons', section: 'Buttons', title: 'Which buttons stay', size: 'medium', rec: 'B',
  now: 'Five buttons, in this order: Split Terminal Horizontally (side by side), Split Terminal Vertically (top and bottom), New Terminal, Full Screen, and Close Terminal. The split buttons turn gray once a group has four shells. The terminal button in the thread’s header shows and hides the terminal; it isn’t part of this toolbar. These mocks keep the buttons where they are today, so only the set changes; where they go is the next topic. With one shell (A in the first topic), New goes too.',
  nowImg: 'img/now-toolbar.png',
  issues: [
    'Five buttons for what is usually one shell.',
    'The split tooltips say “Horizontally” for side by side and “Vertically” for top and bottom.',
  ],
  options: [
    { key: 'A', name: 'All five, as today', from: 't3code, with full screen',
      desc: 'Split side by side, split top and bottom, New Terminal, Full Screen and Close Terminal.',
      good: 'Nothing to relearn.', cost: 'The busiest set, for what is usually one shell.',
      mock: () => buttonsPiece(TODAY, 'No change') },
    { key: 'B', name: 'New, Full Screen and Close', from: 't3code, without splits',
      desc: 'The two split buttons go. With tabs (B in the first topic), × sits on each tab instead, leaving + and Full Screen here.',
      good: 'Three buttons that each do one clear thing.', cost: 'No splits from here; Workspaces has them.',
      mock: () => buttonsPiece(REC, 'New, Full Screen, Close', tooltip('Full Screen', 'top:30px;right:12px')) },
    { key: 'C', name: 'New and Close', from: 'new',
      desc: 'Full Screen goes too. Dragging the top edge makes the terminal taller.',
      good: 'The smallest set that still adds and ends shells.', cost: 'Filling the thread takes a drag, or a key (see the last topic).',
      mock: () => buttonsPiece(['plus', 'x'], 'New, Close') },
    { key: 'D', name: 'New, Split and Zoom', from: 'Zed',
      desc: 'Zed’s terminal panel buttons: + for a new shell, Split with a menu (Split Right, Split Left, Split Up, Split Down), and Zoom to fill the thread. × is on each tab.',
      good: 'One button for every split direction.', cost: 'Fits only if shells keep splits (C in the first topic).',
      mock: () => buttonsPiece(['plus', 'split', 'maximize'], 'New, Split menu, Zoom', menuAt([{ label: 'Split Right', hl: true }, { label: 'Split Left' }, { label: 'Split Up' }, { label: 'Split Down' }], 'top:30px;right:8px;min-width:150px')) },
    { key: 'E', name: 'No buttons', from: 'Zed’s terminal menu',
      desc: 'Nothing on the terminal. Right-click it for New Terminal, Copy, Paste, Select All, Clear and Close Terminal, as in Zed. Cmd-J and the header button hide it; typing exit ends a shell.',
      good: 'The shell’s text is never covered.', cost: 'Nothing on screen says the menu is there.',
      mock: () => `<div class="m" style="width:470px;height:270px;display:flex;flex-direction:column">${legend('None; a right-click menu')}${area(shell(TESTS, menuAt(ZED_MENU, 'left:150px;top:30px;min-width:170px') + pointer('left:142px;top:20px')), { full: true, style: 'border-top:0' })}</div>` },
  ],
});

// 3. Where the buttons go ------------------------------------------------------------------
TOPICS.push({
  id: 'placement', section: 'Buttons', title: 'Where the buttons go', size: 'wide', rec: 'B',
  now: 'The toolbar floats over the shell’s top right corner in a bordered box, whatever the shell prints there. With two shells or more, it moves into a thin strip at the top of the list on the right. The mocks show one shell and the three buttons of the previous topic’s recommendation; they’ll be the ones you pick.',
  nowImg: 'img/now-toolbar.png',
  issues: [
    'The box covers the end of the shell’s top lines: long output or a long prompt runs under it.',
    'It shows even while you aren’t using the terminal.',
    'Where it sits depends on how many shells there are: the corner for one, the list for more.',
  ],
  options: [
    { key: 'A', name: 'Over the shell, as today', from: 't3code',
      desc: 'The same bordered box at the top right, with fewer buttons.',
      good: 'Takes no room from the shell.', cost: 'Still covers the end of the top lines.',
      mock: () => thread(area(shell(TESTS, floating(REC))), { top: legend('The same box, fewer buttons') }) },
    { key: 'B', name: 'A strip above the shell', from: 'Zed',
      desc: 'A strip 30 pixels tall across the top of the terminal, as Zed’s terminal panel has: the shell’s name on the left (its tabs, if there are several), the buttons on the right.',
      good: 'Nothing covers the shell, and the buttons stay put however many shells there are.', cost: 'The shell loses 30 pixels, about two lines.',
      mock: () => thread(area(strip(['zsh'], { right: REC, closeOnTabs: false }) + shell(TESTS)), { top: legend('A strip with the name and the buttons') }) },
    { key: 'C', name: 'Over the shell, only on hover', from: 'agentZ Workspaces panes',
      desc: 'The box from A shows only while the pointer is over the terminal, as an unfocused Workspaces pane’s buttons wait for the pointer.',
      good: 'Out of the way while you read or type.', cost: 'Still covers the text when you point there, and hidden buttons are easy to miss.',
      mock: () => thread(area(shell(TESTS, floating(REC) + pointer('left:520px;top:90px') + note('shows while the pointer is over the terminal', 'top:34px;right:10px'))), { top: legend('The box, only on hover') }) },
    { key: 'D', name: 'In the thread’s header', from: 'new',
      desc: 'The header’s terminal button gets a small menu beside it: New Terminal, Full Screen and Close Terminal. The terminal shows only the shell.',
      good: 'All of the thread’s controls are in one row.', cost: 'Far from the shell, and tabs would still need a place.',
      mock: () => thread(area(shell(TESTS)), { top: legend('A menu beside the header’s terminal button'), head: header({ menu: true }),
        extra: menuAt([{ label: 'New Terminal', icon: 'plus', hl: true }, { label: 'Full Screen', icon: 'maximize' }, '-', { label: 'Close Terminal', icon: 'x' }], 'top:64px;right:30px;min-width:190px') }) },
    { key: 'E', name: 'In the right-click menu', from: 'Zed',
      desc: 'Right-click the shell for New Terminal, Full Screen and Close Terminal, beside Copy, Paste, Select All and Clear, as in Zed’s terminal menu.',
      good: 'Nothing on screen but the shell.', cost: 'Nothing tells you the menu is there.',
      mock: () => thread(area(shell(TESTS, menuAt([{ label: 'New Terminal', hl: true }, { label: 'Full Screen' }, '-', { label: 'Copy' }, { label: 'Paste' }, { label: 'Select All' }, { label: 'Clear' }, '-', { label: 'Close Terminal' }], 'left:330px;top:14px;min-width:180px') + pointer('left:322px;top:4px'))), { top: legend('A right-click menu') }) },
  ],
});

// 4. Hiding and closing --------------------------------------------------------------------
const twoTabs = (screen = DEV, options = {}) => area(strip(['zsh', 'npm run dev'], { active: 1, ...options }) + shell(screen));

TOPICS.push({
  id: 'closing', section: 'Closing', title: 'Hiding and closing', size: 'wide', rec: 'B',
  now: 'The header’s terminal button and Cmd-J hide the terminal and keep its shells running; a dot on the button says something still runs. × ends the shell in front at once, even while a program runs in it. Typing exit ends a shell too. When the last shell ends, the terminal hides, and the next Cmd-J starts a new shell. Closing a Workspaces pane or a terminal thread asks first while something runs in it. The mocks show two tabs, as the first topic recommends.',
  issues: [
    '× ends a shell, and the header button hides the terminal. Both look like closing it.',
    'Nothing asks before × ends a running npm run dev, unlike Workspaces panes.',
  ],
  options: [
    { key: 'A', name: 'As today', from: 't3code',
      desc: '× ends the shell at once. The header button and Cmd-J hide the terminal.',
      good: 'One click, no questions.', cost: 'A slip ends a dev server or a long test run.',
      mock: () => thread(twoTabs(), { top: legend('No change'), extra: note('× ends npm run dev at once', 'top:380px;left:150px') + pointer('left:178px;top:362px') }) },
    { key: 'B', name: 'Ask first while something runs', from: 'agentZ Workspaces panes',
      desc: 'An idle shell’s × ends it at once. While a program runs in it, × first asks the question closing a Workspaces pane asks, with Close and Cancel.',
      good: 'The same as panes, and nothing ends by accident.', cost: 'One more click while something runs.',
      mock: () => thread(twoTabs(), { top: legend('× asks while something runs'), extra: '<div class="modal-back"></div>' + closeAlert('Close “npm”?', 'It’s still running, and closing the terminal ends it.') }) },
    { key: 'C', name: '× hides, it doesn’t end', from: 'new',
      desc: 'The terminal’s × hides it, as Cmd-J does. A shell ends when you type exit, or with Close Terminal in its right-click menu. Tabs have no ×.',
      good: 'Nothing on screen ends a shell.', cost: 'Ending a stuck shell is harder to find.',
      mock: () => thread(area(strip(['zsh', 'npm run dev'], { active: 1, right: ['plus', 'maximize', 'x'], closeOnTabs: false }) + shell(DEV, tooltip('Hide Terminal', 'top:4px;right:12px', '⌘J'))), { top: legend('× hides, as the header button does') }) },
  ],
});

// 5. Where the terminal sits ---------------------------------------------------------------
const recArea = (options = {}) => area(strip(['zsh'], { right: REC, closeOnTabs: false }) + shell(TESTS), options);

TOPICS.push({
  id: 'position', section: 'The terminal', title: 'Where the terminal sits', size: 'wide', rec: 'A',
  now: 'At the bottom of the thread, under the composer, as wide as the conversation (an open Changes panel stays to its right). It opens 280 pixels tall. The mocks show the recommended strip with one shell.',
  options: [
    { key: 'A', name: 'Under the composer, as today', from: 't3code',
      desc: 'The conversation, then the composer, then the terminal along the bottom.',
      good: 'The conversation and the composer stay together.', cost: 'The composer moves up when the terminal opens.',
      mock: () => thread(recArea()) },
    { key: 'B', name: 'Above the composer', from: 'new',
      desc: 'Between the conversation and the composer, which stays at the bottom of the window.',
      good: 'The composer never moves.', cost: 'The terminal sits between the reply you read and the box you answer in.',
      mock: () => thread(recArea({ style: 'border-bottom:0' }), { place: 'above' }) },
    { key: 'C', name: 'Beside the conversation', from: 'Zed (terminal panel docked right)',
      desc: 'A column on the thread’s right, full height under the header, 360 pixels wide at first. Its left edge drags.',
      good: 'Tall, for long output.', cost: 'Narrows the conversation, and shares the right side with the Changes panel.',
      mock: () => thread(`<div class="col" style="width:360px;flex:none;border-left:1px solid var(--b);background:var(--term)">${strip(['zsh'], { right: REC, closeOnTabs: false })}${shell(DEV)}</div>`, { place: 'side' }) },
  ],
});

// 6. Resizing and full screen --------------------------------------------------------------
const dragEdge = (top) => `<span style="position:absolute;left:0;right:0;top:${top}px;height:3px;background:var(--ac);z-index:6"></span>`;

TOPICS.push({
  id: 'size', section: 'The terminal', title: 'Resizing and full screen', size: 'wide', rec: 'A',
  now: 'The top edge drags, from 100 pixels up to where the conversation keeps 160. Each open thread has its own height, back to 280 when the app restarts. Full Screen hides the conversation and the composer until you press it again. Workspaces panes fill their tab with Zoom In (Cmd-Shift-Enter). This goes with the buttons topic: its B keeps the Full Screen button, and C and E drop it.',
  options: [
    { key: 'A', name: 'Drag, and a Full Screen button', from: 'agentZ today',
      desc: 'As today: drag the top edge, or press Full Screen to fill the thread and press it again to come back.',
      good: 'Both are a click away.', cost: 'One more button.',
      mock: () => thread(area(strip(['zsh'], { right: ['plus', 'minimize', 'x'], closeOnTabs: false }) + shell(TESTS, tooltip('Exit Full Screen', 'top:4px;right:34px')), { full: true }), { full: true }) },
    { key: 'B', name: 'Drag only', from: 't3code',
      desc: 'The top edge drags; pull it up for all the room the conversation leaves. No full screen.',
      good: 'One way to resize.', cost: 'The conversation always keeps a strip, and getting back takes another drag.',
      mock: () => thread(area(strip(['zsh'], { right: ['plus', 'x'], closeOnTabs: false }) + shell(TESTS), { h: 300 }), { extra: dragEdge(319) + note('drag the top edge', 'top:330px;left:50%') }) },
    { key: 'C', name: 'Drag, and Zoom with a key', from: 'agentZ Workspaces panes, Zed',
      desc: 'No button. Cmd-Shift-Enter fills the thread with the terminal and brings it back, as it zooms a Workspaces pane (Zed’s Zoom In and Zoom Out).',
      good: 'One key for panes and terminals, and nothing on screen.', cost: 'Only for those who know the key (the shortcuts sheet lists it).',
      mock: () => thread(area(strip(['zsh'], { right: ['plus', 'x'], closeOnTabs: false }) + shell(TESTS), { full: true }), { full: true, extra: note('Cmd-Shift-Enter again brings the conversation back', 'top:80px;right:12px') }) },
  ],
});
