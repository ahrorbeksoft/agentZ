// Pinned threads in the Agents sidebar: the mark on a pinned card, the pinned cards at rest,
// dragging cards between pinned, the rest and Archived, and Pin in the menus.

ICONS.archive = '<rect width="20" height="5" x="2" y="3" rx="1"/><path d="M4 8v11a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8"/><path d="M10 12h4"/>';
ICONS['pin-off'] = '<path d="M12 17v5"/><path d="M15 9.34V7a1 1 0 0 1 1-1 2 2 0 0 0 0-4H7.89"/><path d="m2 2 20 20"/><path d="M9 9v1.76a2 2 0 0 1-1.11 1.79l-1.78.9A2 2 0 0 0 5 15.24V16a1 1 0 0 0 1 1h11"/>';

const PIN_W = 290;
const PIN_H = 780;
// The list's height inside the sidebar: less its search and settings rows, and its padding.
const LIST_H = PIN_H - 80;

const PIN_PROJECTS = {
  st: { mono: 'ST', color: 'g', name: 'storefront', machine: 'laptop' },
  ap: { mono: 'AP', color: 't', name: 'api', machine: 'server' },
};
// Two pinned threads, the rest newest first, and an agent CLI's card.
const PT = {
  checkout: { p: 'st', title: 'Add the checkout page', branch: 'checkout-flow', state: 'working', time: 'now', pinned: true },
  login: { p: 'ap', title: 'Fix flaky login test', branch: 'main', state: 'awaiting', time: '5m', pinned: true },
  tests: { p: 'st', title: 'Write tests for the rate limiter', branch: 'main', time: '19m' },
  grid: { p: 'st', title: 'Speed up the product grid', branch: 'grid-perf', time: '53m' },
  orders: { p: 'ap', title: 'Add pagination to /orders', branch: 'main', time: '1h' },
  cli: { p: 'st', title: 'Claude Code', branch: 'main', time: '2h', agent: 'claude' },
};
const PINNED = ['checkout', 'login'];
const REST = ['tests', 'grid', 'orders', 'cli'];
const ARCHIVED_ROWS = [
  { p: 'st', title: 'Rename the cart store', time: '1h' },
  { p: 'ap', title: 'Bump the API version', time: '2h' },
];

const pinIcon = ({ accent = false } = {}) =>
  `<span style="display:inline-flex;flex:none;color:${accent ? 'var(--ac)' : 'rgba(169,175,188,.65)'}">${ic('pin', 'xs')}</span>`;
const working = () => '<span class="pill working"><span class="spin"></span>Working</span>';
const cardStatus = (thread) => (thread.state === 'working' ? working() : thread.state ? pill(thread.state) : `<span class="sm mu">${thread.time}</span>`);
// t3code's drop badge: what letting go does, in the accent.
const VERB_ICONS = { Pin: 'pin', Unpin: 'pin-off', Archive: 'archive', Unarchive: 'undo' };
const verbBadge = (verb) => `<span class="row xs b5" style="gap:4px;height:20px;padding:0 6px;border-radius:4px;border:1px solid rgba(116,173,232,.4);background:rgba(116,173,232,.1);color:var(--ac);flex:none">${ic(VERB_ICONS[verb], 'xs')}${verb}</span>`;

/**
 * A thread card as sidebar.rs draws it: the project line with the time or state, the title,
 * and the branch with the machine and agent. `pin` is where a pinned card's mark goes.
 */
function pcard(key, { on = false, hover = false, pin = 'line1', badge = null, raised = false, place = false, buttons = null } = {}) {
  const thread = PT[key];
  const project = PIN_PROJECTS[thread.p];
  const mark = thread.pinned && !badge ? pin : null;
  const status = badge ? verbBadge(badge) : cardStatus(thread);
  const hoverCover = buttons ? `<span class="row" style="position:absolute;right:0;top:0;bottom:0;background:var(--hov)"><span style="width:24px;height:100%;background:linear-gradient(90deg,rgba(54,60,70,0),var(--hov))"></span>${buttons}</span>` : '';
  const background = raised ? 'background:var(--sel);box-shadow:0 10px 24px rgba(0,0,0,.55)' : on ? 'background:var(--sel)' : hover ? 'background:var(--hov)' : '';
  return `<div style="margin:0 4px;padding:8px 10px;border-radius:6px;height:78px;${background};${place ? 'visibility:hidden' : ''}">
    <div class="row g15" style="height:20px;position:relative">${mono(project.mono, project.color)}<span class="grow trunc sm mu">${project.name}</span>${mark === 'line1' ? pinIcon() : mark === 'accent' ? pinIcon({ accent: true }) : ''}${status}${hoverCover}</div>
    <div class="row g15" style="margin-top:4px">${mark === 'title' ? pinIcon() : ''}<span class="trunc b5">${thread.title}</span></div>
    <div class="row g15 sm faint" style="margin-top:2px"><span class="grow trunc">${thread.branch}</span>${mark === 'bottom' ? pinIcon() : ''}<span style="opacity:.6;color:var(--mu);display:inline-flex">${ic(project.machine, 'sm')}</span><span style="opacity:.6;display:inline-flex">${thread.agent ? glyph(thread.agent, 'sm') : ic('terminal', 'sm mu')}</span></div>
  </div>`;
}

// The shelves' header and their slim rows (render_shelf_header, render_slim_row).
const shelfHeader = (label, { expanded = true, count = 0, tone = 'muted' } = {}) => {
  const color = tone === 'accent' ? 'var(--ac)' : tone === 'strong' ? 'var(--t)' : 'var(--mu)';
  const rule = tone === 'accent' ? 'rgba(116,173,232,.5)' : 'var(--bv)';
  return `<div class="row g2" style="height:32px;margin:0 2px;padding:0 8px;font-size:12px;font-weight:500;color:${color}">${expanded ? label : `${label} (${count})`}<span class="grow" style="height:1px;background:${rule}"></span>${ic(expanded ? 'chev-up' : 'chev-down', 'xs')}</div>`;
};
const slimRow = (row, { line2 = '', badge = null, raised = false, place = false } = {}) => {
  const project = PIN_PROJECTS[row.p];
  const background = raised ? 'background:var(--sel);box-shadow:0 10px 24px rgba(0,0,0,.55)' : '';
  return `<div style="margin:0 4px;padding:6px 10px;border-radius:6px;${background};${place ? 'visibility:hidden' : ''}">
    <div class="row g2" style="height:22px"><span style="opacity:${raised ? 1 : 0.4}">${mono(project.mono, project.color)}</span><span class="grow trunc ${raised ? '' : 'mu'}">${row.title}</span>${badge ? verbBadge(badge) : `<span class="sm mu">${row.time}</span>`}</div>
    ${line2 ? `<div class="sm faint" style="padding-left:26px">${line2}</div>` : ''}</div>`;
};
const shellsShelf = () => shelfHeader('Shells') + slimRow({ p: 'st', title: 'zsh', time: '11m' }, { line2: 'main' });
const archivedShelf = ({ expanded = true, tone = 'muted', rows = ARCHIVED_ROWS.map((row) => slimRow(row)) } = {}) =>
  shelfHeader('Archived', { expanded, count: ARCHIVED_ROWS.length, tone }) + (expanded ? rows.join('') : '');

// A label with a rule: t3code's drag boundaries (24 points, only while a card is held), or a
// header at rest in the shelf headers' style.
const sectionLabel = (label, { height = 24, color = 'rgba(220,224,229,.8)', rule = 'rgba(220,224,229,.25)' } = {}) =>
  `<div class="row g2" style="height:${height}px;flex:none;margin:0 2px;padding:0 8px;font-size:12px;font-weight:500;color:${color}">${label}<span class="grow" style="height:1px;background:${rule}"></span></div>`;
const dragLabel = (label, { target = false } = {}) =>
  target ? sectionLabel(label, { color: 'var(--ac)', rule: 'rgba(116,173,232,.5)' }) : sectionLabel(label);
const restHeader = (label) => sectionLabel(label, { height: 32, color: 'var(--mu)', rule: 'var(--bv)' });
const dragLine = ({ target = false } = {}) =>
  `<div style="height:9px;flex:none;margin:0 12px;display:flex;align-items:center"><span class="grow" style="height:1px;background:${target ? 'var(--ac)' : 'rgba(220,224,229,.25)'}"></span></div>`;
// The faint rule under t3code's draft block, as agentZ draws it.
const restRule = () => '<div style="margin:6px 10px;height:1px;flex:none;background:var(--bv);opacity:.6"></div>';

const pointer = (style) => `<svg style="position:absolute;z-index:31;width:16px;height:20px;${style}" viewBox="0 0 16 20"><path d="M1 1v15l4-4 3 7 3-1-3-7h6z" fill="#fff" stroke="#111" stroke-width="1.2" stroke-linejoin="round"/></svg>`;
// A raised card held over the list, `top` from the list's top.
const held = (html, top) => `<div style="position:absolute;left:0;right:0;top:${top}px;z-index:25">${html}</div>`;

/** The Agents sidebar: `rows` in the list, then the shelves at the bottom. */
function pinSidebar(rows, { shelf = shellsShelf() + shelfHeader('Workspaces', { expanded: false, count: 2 }) + archivedShelf({ expanded: false }), over = '', h = PIN_H } = {}) {
  return frame(sidebar({
    list: `<div class="col" style="position:relative;flex:1;min-height:0;gap:2px">${rows.join('')}<div class="col" style="margin-top:auto;padding-top:8px">${shelf}</div>${over}</div>`,
  }), { w: PIN_W, h });
}
const restingCards = (options = {}) => [...PINNED, ...REST].map((key) => pcard(key, { ...options, on: key === 'checkout' }));

// A card dragged up among the pinned ones: Write tests for the rate limiter, held below Fix
// flaky login test, its place opened there.
function dragUp({ labels = 'labels', badge = true, extraNote = '' } = {}) {
  const rows = [];
  if (labels === 'labels') rows.push(dragLabel('Pinned', { target: true }));
  rows.push(pcard('checkout', { on: true }), pcard('login'), pcard('tests', { place: true }));
  if (labels === 'labels') rows.push(dragLabel('Active'));
  if (labels === 'line') rows.push(dragLine({ target: true }));
  rows.push(pcard('grid'), pcard('orders'), pcard('cli'));
  // Held a little above its place: cards are 78 points with 2 between, labels 24.
  const top = (labels === 'labels' ? 26 : 0) + 2 * 80 - 6;
  const over = held(pcard('tests', { raised: true, badge: badge ? 'Pin' : null }), top) + pointer(`left:150px;top:${top + 40}px`) + extraNote;
  return pinSidebar(rows, { over, shelf: shellsShelf() + shelfHeader('Workspaces', { expanded: false, count: 2 }) + archivedShelf({ expanded: false, tone: 'strong' }) });
}

// 1. The mark ----------------------------------------------------------------------------
TOPICS.push({
  id: 'mark', section: 'Cards', title: 'The pin on a pinned card', size: 'narrow', rec: 'A',
  now: 'Nothing is pinned today. The cards follow the thread order setting (Newest first, or Latest activity), and the shelves for shells, Workspaces threads and archived threads rest at the bottom. In these, Add the checkout page and Fix flaky login test are pinned.',
  nowImg: 'img/now-sidebar.png',
  options: [
    {
      key: 'A', name: 'A muted pin before the time', from: 't3code pinned card',
      desc: 'A small pin in a dim gray on the project line, just before the time or the state. Clicking it unpins the thread (“Unpin thread” on hover). With one project selected, which drops the project line, it sits before the time on the title line.',
      good: 'Says pinned where you look for state, and is the quickest way out.', cost: 'A small target beside the time.',
      mock: () => pinSidebar(restingCards({ pin: 'line1' })),
    },
    {
      key: 'B', name: 'Before the title', from: 'new',
      desc: 'The same dim pin leads the title line, so the title starts after it. Clicking it unpins.',
      good: 'Reads with the title.', cost: 'Pinned titles start further in than the rest.',
      mock: () => pinSidebar(restingCards({ pin: 'title' })),
    },
    {
      key: 'C', name: 'With the machine and agent', from: 'new',
      desc: 'The pin goes on the bottom line, before the machine and agent icons. Clicking it unpins.',
      good: 'Keeps the top lines as they are.', cost: 'Easy to miss among the other icons.',
      mock: () => pinSidebar(restingCards({ pin: 'bottom' })),
    },
    {
      key: 'D', name: 'An accent pin', from: 'new',
      desc: 'As A, in the accent color.',
      good: 'Pinned cards stand out at a glance.', cost: 'Competes with the Working state’s accent.',
      mock: () => pinSidebar(restingCards({ pin: 'accent' })),
    },
    {
      key: 'E', name: 'No mark', from: 'new',
      desc: 'Nothing on the card; their place at the top says they’re pinned. Unpinning is in the menu or by dragging. Needs a label above them (next topic) to read at all.',
      good: 'Cards unchanged.', cost: 'Without a label, a pinned card looks like any other.',
      mock: () => pinSidebar(restingCards({ pin: null })),
    },
  ],
});

// 2. At rest -----------------------------------------------------------------------------
TOPICS.push({
  id: 'block', section: 'Cards', title: 'The pinned cards at rest', size: 'narrow', rec: 'A',
  now: 'There’s no pinned block today. The drafts already sit above the cards, with a faint line under them. Each option here uses A’s pin.',
  nowImg: 'img/now-sidebar.png',
  options: [
    {
      key: 'A', name: 'Nothing between them', from: 't3code (the pinned block has no header)',
      desc: 'The pinned cards are simply first. Nothing marks where they end; the pins do.',
      good: 'One list, nothing added at rest.', cost: 'Where the pinned cards end is only in their pins.',
      mock: () => pinSidebar(restingCards()),
    },
    {
      key: 'B', name: 'A faint line under them', from: 't3code’s line under the drafts',
      desc: 'The line the drafts have under them, between the pinned cards and the rest.',
      good: 'Where they end is clear, quietly.', cost: 'A second line when there are drafts too.',
      mock: () => pinSidebar([pcard('checkout', { on: true }), pcard('login'), restRule(), ...REST.map((key) => pcard(key))]),
    },
    {
      key: 'C', name: 'A Pinned header', from: 'agentZ’s shelf headers',
      desc: '“Pinned” over them in the shelf headers’ style, without the chevron (they don’t fold). Nothing above the rest.',
      good: 'Names the block.', cost: 'A row of height, and the rest have no header of their own.',
      mock: () => pinSidebar([restHeader('Pinned'), pcard('checkout', { on: true }), pcard('login'), ...REST.map((key) => pcard(key))]),
    },
    {
      key: 'D', name: 'Pinned and Active headers', from: 't3code’s drag labels, kept',
      desc: 'The two labels t3code shows only while dragging, always there: “Pinned” over the pinned cards and “Active” over the rest.',
      good: 'Both blocks named; nothing appears when a drag starts.', cost: 'Two rows of height for a few cards.',
      mock: () => pinSidebar([restHeader('Pinned'), pcard('checkout', { on: true }), pcard('login'), restHeader('Active'), ...REST.map((key) => pcard(key))]),
    },
  ],
});

// 3. Pinning from the card ---------------------------------------------------------------
const hoverButton = (icon, label) => `<span class="row sm" style="gap:4px;padding:0 6px;height:100%;color:var(--t)">${ic(icon, 'xs')}${label}</span>`;
TOPICS.push({
  id: 'hover', section: 'Cards', title: 'Pinning from the card', size: 'narrow', rec: 'A',
  now: 'A card shows Archive over its time while the mouse is on it (and Discard over a draft’s). Here the mouse is on Write tests for the rate limiter.',
  nowImg: 'img/now-sidebar.png',
  options: [
    {
      key: 'A', name: 'The menu and dragging only', from: 't3code (pinning lives in the menu)',
      desc: 'Hover stays as it is, Archive alone. Pinning is in the menu or by dragging; the pin on a pinned card unpins it.',
      good: 'Nothing new on every card.', cost: 'Pinning is a right-click away.',
      mock: () => pinSidebar([pcard('checkout', { on: true }), pcard('login'), pcard('tests', { hover: true, buttons: hoverButton('archive', 'Archive') }), pcard('grid'), pcard('orders'), pcard('cli')], { over: pointer('left:200px;top:200px') }),
    },
    {
      key: 'B', name: 'Pin beside Archive', from: 'new',
      desc: 'Hover shows Pin and Archive over the time. A pinned card shows Archive only, since its pin unpins.',
      good: 'One click to pin.', cost: 'Covers more of the project line, and another button on every card.',
      mock: () => pinSidebar([pcard('checkout', { on: true }), pcard('login'), pcard('tests', { hover: true, buttons: hoverButton('pin', 'Pin') + hoverButton('archive', 'Archive') }), pcard('grid'), pcard('orders'), pcard('cli')], { over: pointer('left:200px;top:200px') }),
    },
  ],
});

// 4. Where it lands ----------------------------------------------------------------------
TOPICS.push({
  id: 'labels', section: 'Dragging', title: 'Where a dragged card lands', size: 'narrow', rec: 'A',
  now: 'Cards don’t drag today. They will lift as workspace rows do: the card is raised and held where you grabbed it, and the cards it passes slide over to leave its place open where it would land. Here Write tests for the rate limiter is dragged up among the pinned cards, which pins it. The options differ in what shows where the pinned cards end; each shows the next topic’s badge.',
  options: [
    {
      key: 'A', name: 'Pinned and Active labels open up', from: 't3code drag boundaries',
      desc: 'While a card is held, “Pinned” opens above the pinned cards and “Active” under them, each a small label with a rule, 24 points tall; the cards move down to make room. The section the card would land in takes the accent. They close when it’s dropped. With nothing pinned yet, “Pinned” and an empty slot open at the top, so there’s somewhere to drop the first.',
      good: 'Says where the card goes, and only while it matters.', cost: 'The list shifts down a little as the drag starts.',
      mock: () => dragUp({ labels: 'labels' }),
    },
    {
      key: 'B', name: 'Only a line between them', from: 'new',
      desc: 'A rule opens between the pinned cards and the rest while a card is held; in the accent when the card is above it. With nothing pinned, the line opens at the top.',
      good: 'Less moves.', cost: 'A line doesn’t say which side is pinned.',
      mock: () => dragUp({ labels: 'line' }),
    },
    {
      key: 'C', name: 'Nothing opens', from: 'agentZ workspace rows',
      desc: 'The cards only slide, as workspace rows do; the badge on the card says it will be pinned.',
      good: 'Nothing new in the list.', cost: 'With nothing pinned, there’s no place to drop the first one: pinning starts in the menu.',
      mock: () => dragUp({ labels: 'none' }),
    },
  ],
});

// 5. The dragged card --------------------------------------------------------------------
TOPICS.push({
  id: 'lifted', section: 'Dragging', title: 'The dragged card', size: 'narrow', rec: 'A',
  now: 'Workspace rows drag this way already: the row is raised, opaque in the selected color with a shadow, and held where you grabbed it. The same drag as before, with the previous topic’s A.',
  options: [
    {
      key: 'A', name: 'Raised, saying what letting go does', from: 't3code drop badge, agentZ workspace rows',
      desc: 'The card raised as workspace rows are. Over another section, its time or state gives way to a small badge in the accent: Pin, Unpin, Archive or Unarchive. Over its own section, nothing changes (a pinned card keeps its pin).',
      good: 'The result is on the thing you hold.', cost: 'Hides the card’s state while it’s held elsewhere.',
      mock: () => dragUp({ labels: 'labels', badge: true }),
    },
    {
      key: 'B', name: 'Raised, nothing more', from: 'agentZ workspace rows',
      desc: 'The card raised as workspace rows are; only the labels say where it lands.',
      good: 'The card stays itself.', cost: 'Over Archived, nothing on the card says it will be archived.',
      mock: () => dragUp({ labels: 'labels', badge: false }),
    },
  ],
});

// 6. Archiving by dragging ---------------------------------------------------------------
function dragToArchive(option) {
  const rows = [dragLabel('Pinned'), pcard('checkout', { on: true }), pcard('login'), dragLabel('Active'), pcard('grid'), pcard('orders'), pcard('cli')];
  const card = pcard('tests', { raised: true, badge: 'Archive' });
  // Grabbed near its bottom, so the target shows under it.
  if (option === 'A') {
    const shelf = shellsShelf() + shelfHeader('Workspaces', { expanded: false, count: 2 }) + archivedShelf({ expanded: false, tone: 'accent' });
    const header = LIST_H - 32;
    return pinSidebar(rows, { shelf, over: held(card, header - 72) + pointer(`left:120px;top:${header - 2}px`) });
  }
  if (option === 'B') {
    const shelf = shelfHeader('Shells', { expanded: false, count: 1 }) + shelfHeader('Workspaces', { expanded: false, count: 2 }) + shelfHeader('Archived', { tone: 'accent' }) + '<div style="height:78px;flex:none"></div>' + ARCHIVED_ROWS.map((row) => slimRow(row)).join('');
    const place = LIST_H - 2 * 36 - 80;
    return pinSidebar(rows, { shelf, over: held(card, place + 6) + pointer(`left:120px;top:${place + 46}px`) });
  }
  const strip = `<div class="row" style="margin:6px 4px 0;height:44px;flex:none;border:1.5px dashed var(--ac);border-radius:6px;justify-content:center;gap:6px;color:var(--ac);font-size:12px;background:rgba(116,173,232,.08)">${ic('archive', 'xs')}Drop to archive</div>`;
  const shelf = shellsShelf() + shelfHeader('Workspaces', { expanded: false, count: 2 }) + archivedShelf({ expanded: false }) + strip;
  const stripTop = LIST_H - 44;
  return pinSidebar(rows, { shelf, over: held(card, stripTop - 72) + pointer(`left:120px;top:${stripTop}px`) });
}

TOPICS.push({
  id: 'archive', section: 'Dragging', title: 'Archiving by dragging', size: 'narrow', rec: 'A',
  now: 'A card archives from its Archive button on hover or its menu; an archived row comes back from its Unarchive button or menu. Here Write tests for the rate limiter is dragged down to Archived. The other way, an archived row dragged up into the list says Unarchive, and lands where it’s let go: among the pinned cards, it’s pinned too.',
  nowImg: 'img/now-sidebar.png',
  options: [
    {
      key: 'A', name: 'The Archived header takes the accent', from: 't3code Settled header',
      desc: 'While a card is held, the Archived header reads at full strength; with the card over it (or over its rows), it turns accent, and the card’s badge says Archive. Letting go archives the thread, and its card leaves the list; a folded shelf stays folded. With nothing archived yet, the header shows while a card is held.',
      good: 'One clear target, folded or not.', cost: 'The target is a single row at the bottom.',
      mock: () => dragToArchive('A'),
    },
    {
      key: 'B', name: 'The shelf opens under the card', from: 'new',
      desc: 'As A, and held over the folded header for a moment, the shelf opens with a place at its top for the card.',
      good: 'You see it join the archived threads.', cost: 'The list jumps while you hold; more to go wrong.',
      mock: () => dragToArchive('B'),
    },
    {
      key: 'C', name: 'A drop strip under the shelves', from: 'new',
      desc: 'While a card is held, a dashed “Drop to archive” strip opens at the bottom of the list.',
      good: 'A big, plain target.', cost: 'Something that exists only to be dropped on.',
      mock: () => dragToArchive('C'),
    },
  ],
});

// 7. The menus ---------------------------------------------------------------------------
const menuItem = (icon, label, { hl = false } = {}) => `<div class="it ${hl ? 'hl' : ''}">${ic(icon, 'sm')}${label}</div>`;
const cardMenu = (order) => {
  const items = {
    rename: menuItem('pencil', 'Rename'),
    pin: menuItem('pin', 'Pin', { hl: true }),
    archive: menuItem('archive', 'Archive'),
  };
  return `<div class="menu" style="left:64px;top:212px">${order.map((key) => items[key]).join('')}${menuItem('settings', 'Project Settings')}<div class="hr"></div>${menuItem('trash', 'Delete…')}</div>`;
};
TOPICS.push({
  id: 'menu', section: 'Menus', title: 'Pin in the menus', size: 'narrow', rec: 'A',
  now: 'A card’s menu (right-click) has Rename, Archive, Project Settings and Delete…; the thread’s title menu has Rename, Continue with Another Agent, Archive and Delete…. Pin goes in both, and reads Unpin on a pinned thread. Shells, drafts, Workspaces threads and agent CLI cards don’t get it. Here the menu of Write tests for the rate limiter.',
  options: [
    {
      key: 'A', name: 'Just before Archive', from: 't3code (Pin before Settle)',
      desc: 'Rename, Pin, Archive, … in the card’s menu; in the title menu, Pin heads the group with Archive and Delete….',
      good: 'Next to the other way a thread leaves or stays.', cost: '',
      mock: () => pinSidebar(restingCards(), { over: cardMenu(['rename', 'pin', 'archive']) + pointer('left:58px;top:202px') }),
    },
    {
      key: 'B', name: 'First', from: 't3code (Pin near the top)',
      desc: 'Pin, Rename, Archive, … in the card’s menu, and first in the title menu.',
      good: 'The first thing in the menu.', cost: 'Moves Rename, which is first today.',
      mock: () => pinSidebar(restingCards(), { over: cardMenu(['pin', 'rename', 'archive']) + pointer('left:58px;top:202px') }),
    },
  ],
});
