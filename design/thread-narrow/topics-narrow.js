// The thread's account in the composer's account picker, and the thread's header and composer
// when the thread is narrow. Sizes from agent_view.rs: a 36 px header with 22 px pills, 14 px
// avatars in the composer and 18 px ones in the picker.

Object.assign(ICONS, {
  gauge: '<path d="m12 14 4-4"/><path d="M3.34 19a10 10 0 1 1 17.32 0"/>',
  send: '<path d="M14.54 21.69a.5.5 0 0 0 .94-.03l6.5-19a.5.5 0 0 0-.64-.64l-19 6.5a.5.5 0 0 0-.03.94l7.93 3.18a2 2 0 0 1 1.11 1.11z"/><path d="m21.85 2.15-10.94 10.94"/>',
});

const BRANCH = 'agentz/fix-login-redirect';
const THREAD_TITLE = 'Fix the login redirect loop after OAuth sign-in';
const WIDTHS = [560, 440, 320];
const chev = ic('chev-down', 'xs mu');

const avatar = (letter, hue, size = 18) => `<span style="width:${size}px;height:${size}px;border-radius:50%;flex:none;display:inline-grid;place-items:center;font-size:9px;font-weight:600;color:#1b1f26;background:linear-gradient(135deg,hsl(${hue} 70% 82%),hsl(${hue} 42% 60%))">${letter}</span>`;

/** Widths stacked, each with its size above it. */
const stack = (rows, extra = '') => `<div class="col" style="gap:14px;padding:14px;background:#1e2127;height:100%">${rows.map(([w, html]) => `<div class="col" style="gap:4px"><span class="xs ph">${w} px</span>${html}</div>`).join('')}${extra}</div>`;
const caption = (text) => `<span class="xs ph">${text}</span>`;

// 1. The account picker ------------------------------------------------------------------------
const ACCOUNTS = [
  { name: 'ahror@example.com', letter: 'A', hue: 212, plan: 'Pro', left: 46, external: true },
  { name: 'Personal', letter: 'P', hue: 140, plan: 'Max', left: 69, current: true },
  { name: 'Work', letter: 'W', hue: 28, plan: 'Max', left: 88 },
];

/** A row of the picker (account_row): avatar, name over plan, what's left, then the mark. */
function accountItem(a, { note = null, check = 'right', selected = false, ring = false, hover = false } = {}) {
  const mark = a.current && check ? ic('check', 'sm ac') : '';
  const face = ring && a.current
    ? `<span style="border-radius:50%;display:inline-flex;box-shadow:0 0 0 1.5px var(--panel),0 0 0 3px var(--ac)">${avatar(a.letter, a.hue)}</span>`
    : avatar(a.letter, a.hue);
  const background = selected && a.current ? 'background:var(--sel);' : hover ? 'background:var(--hov);' : '';
  const slot = `<span class="none" style="width:14px;display:inline-flex">${mark}</span>`;
  return `<div class="row g2" style="padding:5px 8px;border-radius:5px;${background}">
    ${check === 'left' ? slot : ''}${face}
    <span class="col grow" style="gap:1px"><span class="row g1" style="min-width:0"><span class="trunc">${a.name}</span>${a.external ? '<span class="xs ph none">· External</span>' : ''}</span><span class="sm mu">${(a.current && note) || a.plan}</span></span>
    <span class="sm b5 none" style="text-align:right">${a.left}% left</span>
    ${check === 'right' ? slot : ''}
  </div>`;
}
const manageAccounts = `<div class="hr"></div><div class="it">${ic('settings', 'sm')}Manage Accounts…</div>`;
const accountChip = `<span class="row g1 sm mu" style="height:22px;padding:0 6px;border-radius:5px;background:var(--hov)">${avatar('P', 140, 14)}Personal${chev}</span>`;
/** The menu open over the composer's account chip. */
const picker = (rows) => frame(`<div class="col" style="padding:12px;gap:6px;height:100%"><div class="menu" style="position:relative;width:330px">${rows}${manageAccounts}</div><div class="row g1" style="padding-left:20px">${accountChip}</div></div>`, { w: 354, h: 250 });
const allRows = (options) => ACCOUNTS.map((a) => accountItem(a, options)).join('');

TOPICS.push({
  id: 'picker-current', section: 'Account picker', title: 'How the picker marks this thread’s account', size: 'narrow', rec: 'A',
  now: 'In a sent thread, the current account has a blue check at the right and “This thread’s account” in place of its plan under its name. A new thread’s picker marks it with the check alone.',
  nowImg: 'img/now-account-picker.png',
  issues: ['The note hides the account’s plan', 'It’s the only row with a second label, and reads like one', 'The two pickers mark it differently'],
  options: [
    { key: 'A', name: 'The check alone', from: 'Zed (model picker)',
      desc: 'Every row keeps its plan under the name; the blue check at the right is the only mark, as in a new thread’s picker.',
      good: 'Nothing new to read; both pickers alike.', cost: 'The check is small, at the far right.',
      mock: () => picker(allRows()) },
    { key: 'B', name: 'A lit row', from: 'Zed (a list item’s selected state)',
      desc: 'The current row has the selected background, and keeps the check.',
      good: 'Seen at a glance.', cost: 'It looks like the hovered row: with the pointer on another one, two rows are lit (shown here on Work).',
      mock: () => picker(ACCOUNTS.map((a) => accountItem(a, { selected: true, hover: a.name === 'Work' })).join('')) },
    { key: 'C', name: 'The check at the left', from: 'Zed (a context menu’s toggled entries)',
      desc: 'A column before the avatars holds the check, as Zed’s menus mark a toggled item. The plan stays.',
      good: 'Where reading starts.', cost: 'Every row moves right by the column.',
      mock: () => picker(allRows({ check: 'left' })) },
    { key: 'D', name: 'Under its own heading', from: 'new',
      desc: '“This thread” over the current account, then “Switch to” over the others. No check.',
      good: 'Says what picking another one does.', cost: 'Two headings for three rows; the order changes with the account.',
      mock: () => picker(`<div class="lbl">This thread</div>${accountItem(ACCOUNTS[1], { check: null })}<div class="hr"></div><div class="lbl">Switch to</div>${ACCOUNTS.filter((a) => !a.current).map((a) => accountItem(a, { check: null })).join('')}`) },
    { key: 'E', name: 'A ring around the avatar', from: 'new',
      desc: 'An accent ring around the current account’s avatar; no check and no note.',
      good: 'The quietest.', cost: 'Easy to miss; the avatars’ own colors compete with it.',
      mock: () => picker(allRows({ check: null, ring: true })) },
  ],
});

// 2. The thread header -------------------------------------------------------------------------
const stat = '<span class="sm" style="color:var(--ok)">+42</span><span class="sm" style="color:var(--del)">−9</span>';

/**
 * The header (render_toolbar): the project, "/", the title, the branch, the changes, Terminal
 * and ⋯. `project`: full | icon. `branch`: full | short (truncated) | icon | none.
 * `changes`: stat | icon.
 */
function header(w, { project = 'full', branch = 'full', changes = 'stat', terminal = true, titleMin = 0 } = {}) {
  const crumb = `<span class="row g15 none" style="padding:2px 4px">${mono('ST', 'g')}${project === 'full' ? '<span class="sm mu">storefront</span>' : ''}</span><span class="sm mu none">/</span>`;
  const branchHtml = {
    full: `<span class="row g1 none" style="height:22px;padding:0 6px;border:1px solid var(--bv);border-radius:5px;max-width:200px">${ic('worktree', 'xs mu')}<span class="sm mu trunc">${BRANCH}</span></span>`,
    short: `<span class="row g1" style="height:22px;padding:0 6px;border:1px solid var(--bv);border-radius:5px;max-width:110px;flex:0 1 auto;min-width:0">${ic('worktree', 'xs mu')}<span class="sm mu trunc">${BRANCH}</span></span>`,
    icon: `<span class="row none" style="height:22px;width:24px;justify-content:center;border:1px solid var(--bv);border-radius:5px">${ic('worktree', 'xs mu')}</span>`,
    none: '',
  }[branch];
  const changesHtml = changes === 'stat'
    ? `<span class="row g1 none" style="height:22px;padding:0 6px;border:1px solid var(--b);border-radius:5px">${stat}</span>`
    : `<span class="ibtn">${ic('diff', 'sm')}<span class="badge-dot" style="background:var(--ac)"></span></span>`;
  return `<div class="row" style="width:${w}px;height:36px;padding:0 8px;gap:4px;border:1px solid var(--b);background:var(--ed)">
    ${crumb}
    <span class="row g1 grow" style="min-width:${titleMin}px"><span class="trunc" style="font-size:13px">${THREAD_TITLE}</span>${chev}</span>
    ${branchHtml}${changesHtml}
    <span class="row none" style="gap:6px">${terminal ? ibtn('terminal') : ''}${ibtn('more')}</span>
  </div>`;
}
const headers = (stateAt, extra = '', h = 250) => frame(stack(WIDTHS.map((w) => [w, header(w, stateAt(w))]), extra), { w: 600, h });
const optionsMenu = (top = '') => `<div class="menu" style="position:relative;width:240px;align-self:flex-end;margin-right:20px">${top}<div class="it">${ic('restart', 'sm')}Restart Agent</div><div class="it">${ic('x', 'sm')}Log Out</div></div>`;

TOPICS.push({
  id: 'header', section: 'Thread header', title: 'What gives way when the header is narrow', size: 'wide', rec: 'A',
  now: 'The project’s icon and name, “/”, the title, the fork and artifact pills when there are any, the branch (up to 200 px), the changes, Terminal and ⋯. The branch and everything after it never shrink, so the project’s name goes first, then the title: at 440 px the title is “Fix the logi…”, at 360 px it’s gone, and at 320 px the project’s icon, “/” and the title’s chevron draw over each other.',
  nowImg: 'img/now-header.png',
  issues: ['The title is the first thing to go', 'At 320 px the crumb draws over itself', 'The branch keeps its whole 200 px'],
  options: [
    { key: 'A', name: 'The branch gives way first', from: 't3code (the project leads, the title takes the rest)',
      desc: 'The branch truncates, then shows only its icon (its name and folder in the tooltip, as now); then the project shows only its icon. The title keeps at least 120 px and truncates last. The changes, Terminal and ⋯ stay.',
      good: 'Everything stays where it is; only the order of shrinking changes.', cost: 'Under 440 px the branch is only its icon.',
      mock: () => headers((w) => ({ branch: w >= 560 ? 'short' : 'icon', project: w > 360 ? 'full' : 'icon', titleMin: 120 })) },
    { key: 'B', name: 'The branch under the composer', from: 't3code (BranchToolbar)',
      desc: 'As t3code: the header has only the project, the title and its buttons, and the branch moves to a line under the composer, where a new thread shows where it works. The project shows only its icon before the title shrinks.',
      good: 'The title gets the header at every width, and the branch is where you picked it.', cost: 'Moves the branch at every width, not only narrow ones; one more line under the composer.',
      mock: () => headers((w) => ({ branch: 'none', project: w > 360 ? 'full' : 'icon', titleMin: 120 }),
        `${caption('Under the composer, at every width')}<div style="width:440px;border:1px solid var(--b);background:var(--ed)"><div class="ph" style="padding:10px 12px;font-size:13px">Message the agent…</div><div class="row g15 xs mu" style="padding:6px 12px;border-top:1px solid var(--bv)">${ic('worktree', 'xs')}<span class="trunc">${BRANCH}</span><span class="ph none">· Worktree</span></div></div>`, 340) },
    { key: 'C', name: 'The branch and Terminal into ⋯', from: 't3code (compact controls menu)',
      desc: 'Below 480 px the branch and Terminal leave the header and head the ⋯ menu: the branch as a line at its top, then Toggle Terminal. Above 480 px, as A.',
      good: 'The header stays one calm row with the title in full view.', cost: 'Terminal takes two clicks when narrow, and its running dot moves onto ⋯.',
      mock: () => headers((w) => (w >= 480 ? { branch: 'short', titleMin: 120 } : { branch: 'none', terminal: false, project: w > 360 ? 'full' : 'icon', titleMin: 120 }),
        `${caption('⋯ at 320 px')}${optionsMenu(`<div class="lbl row g1">${ic('worktree', 'xs')}${BRANCH}</div><div class="it">${ic('terminal', 'sm')}Toggle Terminal<span class="kb">⌃\`</span></div><div class="hr"></div>`)}`, 400) },
    { key: 'D', name: 'Icons for all but the title', from: 'new',
      desc: 'From 560 px down, the project, the branch and the changes show as icons only (the changes as the Diff icon with its dot, the counts in its tooltip), and the title takes the rest.',
      good: 'The most room for the title.', cost: 'The +42 −9 is no longer seen at a glance.',
      mock: () => headers((w) => (w > 560 ? {} : { project: 'icon', branch: 'icon', changes: 'icon' })) },
  ],
});

// 3. The composer ------------------------------------------------------------------------------
/**
 * The composer's footer (render_composer's controls). `pickers`: full | icons. `settings`:
 * full | menu (Mode and Effort in ⋯). `layout`: wrap (today) | corner (the wrapped line kept
 * to the right) | line (one line, the model's name truncating).
 */
function composer(w, { pickers = 'full', settings = 'full', layout = 'corner' } = {}) {
  const chip = (html, extra = '') => `<span class="row g1 sm mu none" style="height:22px;padding:0 5px;border-radius:5px;${extra}">${html}</span>`;
  const left = [
    ibtn('plus'),
    chip(`${glyph('claude', 'sm')}${pickers === 'full' ? 'Claude Agent' : ''}${chev}`),
    chip(`${avatar('P', 140, 14)}${pickers === 'full' ? 'Personal' : ''}${chev}`),
    chip(`${ic('gauge', 'xs')}69%`),
  ].join('');
  const ring = '<span class="none" style="width:14px;height:14px;margin:0 4px;border-radius:50%;border:2px solid #3b414d;border-top-color:var(--mu);border-right-color:var(--mu)"></span>';
  const model = layout === 'line'
    ? `<span class="row g1 sm mu" style="height:22px;padding:0 5px;min-width:0;flex:0 1 auto"><span class="trunc">Sonnet</span>${chev}</span>`
    : chip(`Sonnet${chev}`);
  const right = [
    ring,
    settings === 'full' ? chip(`Default${chev}`) : '',
    model,
    settings === 'full' ? chip(`High${chev}`) : ibtn('more'),
    `<span class="ibtn none" style="width:26px;height:26px;background:var(--hov);color:var(--t)">${ic('send', 'sm')}</span>`,
  ].join('');
  const footer = layout === 'line'
    ? `<div class="row" style="gap:2px;flex-wrap:nowrap"><div class="row none" style="gap:2px">${left}</div><div class="row" style="gap:2px;margin-left:auto;min-width:0">${right}</div></div>`
    : `<div class="row" style="flex-wrap:wrap;justify-content:space-between;row-gap:6px"><div class="row" style="gap:2px;flex-wrap:wrap">${left}</div><div class="row" style="gap:2px;flex-wrap:wrap;${layout === 'corner' ? 'margin-left:auto;justify-content:flex-end' : ''}">${right}</div></div>`;
  return `<div style="width:${w}px;border:1px solid var(--b);background:var(--ed);padding:6px 12px 8px"><div class="ph" style="padding:4px 0 10px;font-size:13px">Message the agent…</div>${footer}</div>`;
}
const composers = (stateAt, extra = '') => frame(stack(WIDTHS.map((w) => [w, composer(w, stateAt(w))]), extra), { w: 600, h: extra ? 620 : 400 });
const settingsMenu = `<div class="menu" style="position:relative;width:200px;align-self:flex-end;margin-right:40px"><div class="lbl">Mode</div><div class="it">Default<span class="kb">${ic('check', 'sm ac')}</span></div><div class="it">Plan</div><div class="hr"></div><div class="lbl">Effort</div><div class="it">Low</div><div class="it">High<span class="kb">${ic('check', 'sm ac')}</span></div></div>`;

TOPICS.push({
  id: 'composer', section: 'Composer', title: 'How the composer’s controls fit when narrow', size: 'wide', rec: 'D',
  now: 'Zed’s footer: the agent, the account and the gauge on the left; the context ring, Mode, Model, Effort and Send on the right. Under about 560 px the right group wraps to a second line that starts at the left, so Send leaves the corner and the ring sits at the far left.',
  nowImg: 'img/now-composer.png',
  issues: ['Send leaves the bottom-right corner', 'The second line isn’t anchored to either side', 'Nothing gets smaller before it wraps'],
  options: [
    { key: 'A', name: 'Wrap, with Send in the corner', from: 'Zed (its footer’s wrap)',
      desc: 'Today’s wrap, but the second line keeps to the right: Send stays in the bottom-right corner, the ring beside Mode.',
      good: 'The smallest change; every control keeps its name.', cost: 'Still two lines under about 560 px.',
      mock: () => composers(() => ({ layout: 'corner' })) },
    { key: 'B', name: 'Icons for the agent and account', from: 't3code (icon-only controls)',
      desc: 'Below 560 px the agent picker shows only its icon and the account only its avatar (their names in the tooltips). One line down to about 400 px; under that, A’s wrap.',
      good: 'The pickers keep working as they are, just smaller.', cost: 'Which account is in use is only its avatar’s letter and color.',
      mock: () => composers((w) => ({ pickers: w < 560 ? 'icons' : 'full' })) },
    { key: 'C', name: 'Mode and Effort into ⋯', from: 't3code (CompactComposerControlsMenu)',
      desc: 'Below 620 px, as t3code: Mode and Effort go into one ⋯ menu with a section each, and the model stays. One line down to about 460 px; under that, A’s wrap.',
      good: 'The pickers keep their names.', cost: 'Mode and Effort are no longer seen at a glance when narrow.',
      mock: () => composers(() => ({ settings: 'menu' }), `${caption('⋯ opens')}${settingsMenu}`) },
    { key: 'D', name: 'Both, as t3code does', from: 't3code (its composer footer)',
      desc: 'B’s icons below 560 px, C’s ⋯ menu below 620 px, and the model’s name truncates last, so it stays one line down to 320 px. Send never moves.',
      good: 'One line at every width; the composer never grows taller.', cost: 'The most changes: names and Mode and Effort hide when narrow.',
      mock: () => composers((w) => ({ pickers: w < 560 ? 'icons' : 'full', settings: 'menu', layout: 'line' }), `${caption('⋯ opens')}${settingsMenu}`) },
  ],
});
