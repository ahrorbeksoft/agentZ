// Thread cards with unsent text, and the draft rows above the cards: what marks them, and where.
// Cards are drawn as sidebar.rs draws them (render_thread_card, render_draft_row).

// Lucide's square-pen, Zed's IconName::SquarePen.
ICONS['square-pen'] = '<path d="M12 3H5a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7"/><path d="M18.375 2.625a1 1 0 0 1 3 3l-9.013 9.014a2 2 0 0 1-.853.505l-2.873.84a.5.5 0 0 1-.62-.62l.84-2.873a2 2 0 0 1 .506-.852z"/>';

const DC_W = 290;
const DC_H = 600;

const DC_PROJECTS = {
  st: { mono: 'ST', color: 'g', name: 'storefront' },
  ap: { mono: 'AP', color: 't', name: 'api' },
};
// `unsent` is the text left in the thread's composer.
const DC = {
  checkout: { p: 'st', title: 'Add the checkout page', branch: 'checkout-flow', state: 'working', agent: 'claude' },
  login: { p: 'ap', title: 'Fix flaky login test', branch: 'main', state: 'awaiting', agent: 'claude' },
  tests: { p: 'st', title: 'Write tests for the rate limiter', branch: 'main', state: 'done', agent: 'claude', unsent: 'Also cover the burst limit' },
  version: { p: 'ap', title: 'Bump the API version', branch: 'release-2.4', state: 'pending', agent: 'codex', account: '#b477cf' },
  grid: { p: 'st', title: 'Speed up the product grid', branch: 'grid-perf', time: '53m', agent: 'codex', account: '#74ade8', unsent: 'Try virtualizing the rows first' },
  orders: { p: 'ap', title: 'Add pagination to /orders', branch: 'main', time: '1h', agent: 'claude' },
};
const DC_ORDER = ['checkout', 'login', 'tests', 'version', 'grid', 'orders'];
// New threads with text typed and nothing sent, for the draft rows.
const DRAFT_ROWS = [
  { p: 'st', text: 'Add a dark mode toggle to the header' },
];

const WARN = '#dec184';
const pen = (color) => `<span style="display:inline-flex;flex:none;color:${color}">${ic('square-pen', 'xs')}</span>`;
const MARKS = {
  pen: () => pen(WARN),
  muted: () => pen('rgba(169,175,188,.65)'),
  label: () => '<span class="row xs b5" style="height:16px;padding:0 5px;border-radius:4px;flex:none;color:var(--mu);background:rgba(169,175,188,.12)">Draft</span>',
};
const working = () => '<span class="pill working"><span class="spin"></span>Working</span>';
const cardStatus = (thread) => (thread.state === 'working' ? working() : thread.state ? pill(thread.state) : `<span class="sm mu">${thread.time}</span>`);
// The agent's icon, on a rounded square in the account's color while its agent lists more than
// one account (controls::AgentIcon).
const agentIcon = (thread) => thread.account
  ? `<span style="width:14px;height:14px;border-radius:4px;background:${thread.account};display:inline-grid;place-items:center;flex:none;color:#fff;font-size:9px;line-height:1">${GLYPHS[thread.agent]}</span>`
  : `<span style="width:14px;height:14px;display:inline-grid;place-items:center;flex:none;color:rgba(169,175,188,.6);font-size:13px;line-height:1">${GLYPHS[thread.agent]}</span>`;
const machine = () => `<span style="opacity:.6;color:var(--mu);display:inline-flex">${ic('laptop', 'sm')}</span>`;

/**
 * A card. On a card with unsent text that isn't open, `mark` (a key of MARKS) goes at `place`:
 * 'project' (before the project icon), 'status' (before the state or time), 'title' (after the
 * title) or 'bottom' (before the machine icon). `textLine` puts the unsent text in place of the
 * branch; `tint` gives the card t3code's draft tint.
 */
function dcard(key, { on = false, mark = 'pen', place = 'project', textLine = false, tint = false } = {}) {
  const thread = DC[key];
  const project = DC_PROJECTS[thread.p];
  const unsent = thread.unsent && !on;
  const m = (where) => (unsent && mark && place === where ? MARKS[mark]() : '');
  const background = on ? 'background:var(--sel)' : unsent && tint ? 'background:rgba(222,193,132,.04)' : '';
  const bottom = unsent && textLine
    ? `${pen('rgba(169,175,188,.65)')}<span class="grow trunc" style="font-style:italic;color:var(--mu)">${thread.unsent}</span>`
    : `<span class="grow trunc">${thread.branch}</span>`;
  return `<div style="margin:0 4px;padding:8px 10px;border-radius:6px;height:78px;flex:none;${background}">
    <div class="row g15" style="height:20px">${m('project')}${mono(project.mono, project.color)}<span class="grow trunc sm mu">${project.name}</span>${m('status')}${cardStatus(thread)}</div>
    <div class="row g15" style="margin-top:4px;min-width:0"><span class="trunc b5">${thread.title}</span>${m('title')}</div>
    <div class="row g15 sm faint" style="margin-top:2px">${bottom}${m('bottom')}${machine()}${agentIcon(thread)}</div>
  </div>`;
}

/**
 * A draft row: the project, then the first line of the text in place of a title. `mark` leads
 * the project line, `tint` is t3code's, `label` puts Draft at the end of the project line, and
 * `asCard` adds a card's bottom line.
 */
function draftRow(row, { mark = 'pen', tint = true, label = false, italic = false, asCard = false } = {}) {
  const project = DC_PROJECTS[row.p];
  const text = italic
    ? `<span class="trunc" style="font-style:italic;color:var(--mu)">${row.text}</span>`
    : `<span class="trunc b5">${row.text}</span>`;
  return `<div style="margin:0 4px;padding:8px 10px;border-radius:6px;height:78px;flex:none;${tint ? 'background:rgba(222,193,132,.04)' : ''}">
    <div class="row g15" style="height:20px">${mark ? MARKS[mark]() : ''}${mono(project.mono, project.color)}<span class="grow trunc sm mu">${project.name}</span>${label ? MARKS.label() : ''}</div>
    <div class="row" style="margin-top:${asCard ? 4 : 2}px;min-width:0">${text}</div>
    ${asCard ? `<div class="row g15 sm faint" style="margin-top:2px"><span class="grow trunc">main</span>${machine()}${agentIcon({ agent: 'claude' })}</div>` : ''}
  </div>`;
}
// The faint line under the draft rows.
const draftRule = () => '<div style="margin:6px 10px;height:1px;flex:none;background:var(--bv);opacity:.6"></div>';
const archivedHeader = () => `<div class="row g2" style="height:32px;margin:0 2px;padding:0 8px;font-size:12px;font-weight:500;color:var(--mu)">Archived (2)<span class="grow" style="height:1px;background:var(--bv)"></span>${ic('chev-down', 'xs')}</div>`;

/** The Agents sidebar with `rows` in its list and the Archived shelf at the bottom. */
function dcSidebar(rows, { h = DC_H } = {}) {
  return frame(sidebar({
    list: `<div class="col" style="flex:1;min-height:0;gap:2px">${rows.join('')}<div class="col" style="margin-top:auto">${archivedHeader()}</div></div>`,
  }), { w: DC_W, h });
}
const cards = (options = {}) => DC_ORDER.map((key) => dcard(key, { ...options, on: key === 'checkout' }));

// 1. The mark ----------------------------------------------------------------------------
TOPICS.push({
  id: 'mark', section: 'Cards', title: 'What marks a card with unsent text', size: 'narrow', rec: 'B',
  now: 'A card whose thread has unsent text gets a small yellow pen before its project icon (tooltip “Unsent draft”), and a Discard draft × beside Archive on hover. The open thread’s card has none. Here Write tests for the rate limiter and Speed up the product grid have unsent text.',
  nowImg: '../feedback/evidence/01-draft-thread-card.png',
  issues: [
    'The pen is the same yellow as Pending Approval, so a card that only has unsent text looks like it needs you.',
    'It pushes the project icon and name to the right, so they no longer line up with the cards above and below.',
    'Nothing on the card says what the unsent text is.',
  ],
  options: [
    {
      key: 'A', name: 'As it is', from: 't3code’s pen',
      desc: 'The yellow pen before the project icon.',
      good: 'Nothing to change, and it matches t3code.', cost: 'Loud, and the yellow reads like Pending Approval.',
      mock: () => dcSidebar(cards({ mark: 'pen' })),
    },
    {
      key: 'B', name: 'A gray pen', from: 't3code’s pen, in the pin’s gray',
      desc: 'The same pen in the dim gray the pin uses on a pinned card. The next topic decides where it goes.',
      good: 'Still says draft, without competing with the states.', cost: 'Easier to miss.',
      mock: () => dcSidebar(cards({ mark: 'muted' })),
    },
    {
      key: 'C', name: 'A Draft label', from: 'new',
      desc: 'The word “Draft” in a small gray badge in place of the pen, drawn here before the state. The next topic decides where it goes.',
      good: 'Says what it means with no icon to learn.', cost: 'Takes more room on a line that’s already full.',
      mock: () => dcSidebar(cards({ mark: 'label', place: 'status' })),
    },
    {
      key: 'D', name: 'The unsent text on the bottom line', from: 'new (Zed shows a draft’s text in its row)',
      desc: 'The bottom line shows a gray pen and the first line of the unsent text in italic, in place of the branch. The branch comes back once the text is sent or discarded.',
      good: 'You see what you were about to send.', cost: 'The branch is hidden while there’s unsent text.',
      mock: () => dcSidebar(cards({ mark: null, textLine: true })),
    },
    {
      key: 'E', name: 'A yellow tint', from: 't3code (it tints these cards as well as drawing the pen)',
      desc: 'No pen. The card gets the faint yellow tint the draft rows have.',
      good: 'Cards and draft rows read as one kind of thing.', cost: 'The tint is faint, and hard to see in some themes.',
      mock: () => dcSidebar(cards({ mark: null, tint: true })),
    },
    {
      key: 'F', name: 'No mark', from: 'Zed (it doesn’t mark a thread with unsent text)',
      desc: 'Nothing on the card. Discard draft still shows beside Archive on hover, and the text is still there when you open the thread.',
      good: 'Calm cards.', cost: 'You forget you left text in a thread.',
      mock: () => dcSidebar(cards({ mark: null })),
    },
  ],
});

// 2. Where it goes -----------------------------------------------------------------------
TOPICS.push({
  id: 'place', section: 'Cards', title: 'Where the mark goes', size: 'narrow', rec: 'B',
  now: 'The pen comes first on the project line, before the project icon. With one project selected, which drops the project line, it comes first on the title line. This topic applies if the first topic’s pick is a pen or the Draft label; the mocks draw the first topic’s B (a gray pen).',
  nowImg: '../feedback/evidence/01-draft-thread-card.png',
  options: [
    {
      key: 'A', name: 'Before the project icon', from: 't3code',
      desc: 'Where it is today.',
      good: 'Seen first.', cost: 'Project icons stop lining up down the list.',
      mock: () => dcSidebar(cards({ mark: 'muted', place: 'project' })),
    },
    {
      key: 'B', name: 'Before the state', from: 'new, where t3code puts the pin',
      desc: 'At the end of the project line, just before the state or the time, where a pinned card’s pin goes. With one project selected, it sits before the state on the title line.',
      good: 'Project icons stay in line, and the pen sits with the other small marks.', cost: 'Beside a pin on a pinned card, two icons in a row.',
      mock: () => dcSidebar(cards({ mark: 'muted', place: 'status' })),
    },
    {
      key: 'C', name: 'After the title', from: 'new',
      desc: 'Right after the title, on the title line.',
      good: 'Reads with the thread’s name.', cost: 'Lands in a different spot on each card, and a long title cuts it off.',
      mock: () => dcSidebar(cards({ mark: 'muted', place: 'title' })),
    },
    {
      key: 'D', name: 'With the machine and agent', from: 'new',
      desc: 'On the bottom line, before the machine icon.',
      good: 'Leaves the top two lines as they are.', cost: 'Easy to miss among the other icons.',
      mock: () => dcSidebar(cards({ mark: 'muted', place: 'bottom' })),
    },
  ],
});

// 3. Draft rows --------------------------------------------------------------------------
const withDraftRows = (rowOptions) => dcSidebar([
  ...DRAFT_ROWS.map((row) => draftRow(row, rowOptions)),
  draftRule(),
  ...DC_ORDER.filter((key) => !DC[key].unsent).map((key) => dcard(key, { on: key === 'checkout' })),
]);
TOPICS.push({
  id: 'rows', section: 'Draft rows', title: 'New threads with nothing sent', size: 'narrow', rec: 'B',
  now: 'A new thread with text typed and nothing sent is a draft row above the cards: the yellow pen, its project, and the first line of the text in place of a title, on a faint yellow tint, with a faint line under the rows. Discard draft shows on hover. Here Add a dark mode toggle to the header is one; the cards below it have no unsent text.',
  issues: ['It uses the same yellow pen as the cards, so whatever bothers you about the cards may bother you here too.'],
  options: [
    {
      key: 'A', name: 'As it is', from: 't3code’s draft rows',
      desc: 'The yellow pen, the tint, and the text as the title.',
      good: 'Nothing to change, and it matches t3code.', cost: 'Keeps the yellow, whatever the cards get.',
      mock: () => withDraftRows({ mark: 'pen', tint: true }),
    },
    {
      key: 'B', name: 'The same mark as the cards', from: 'new',
      desc: 'Whatever the first topic picks for cards, drawn here as the first topic’s B (a gray pen), and no tint.',
      good: 'One way to show unsent work everywhere.', cost: 'Without the tint, the rows stand apart only by their place and the line under them.',
      mock: () => withDraftRows({ mark: 'muted', tint: false }),
    },
    {
      key: 'C', name: 'A card, with Draft for its state', from: 'new, close to Zed (drafts are rows like any thread)',
      desc: 'No pen or tint. The row is a full card: the project with “Draft” where a card shows its state, the text as the title, then the branch, machine and agent it will start with.',
      good: 'Looks like the cards, and says plainly what it is.', cost: 'Easy to take for a thread that has started.',
      mock: () => withDraftRows({ mark: null, tint: false, label: true, asCard: true }),
    },
    {
      key: 'D', name: 'The text in gray italic', from: 'new',
      desc: 'No pen or tint. The text is in gray italic instead of a title’s bright text, like something not yet sent.',
      good: 'Quiet, and plainly not a thread yet.', cost: 'Gray italic is harder to read.',
      mock: () => withDraftRows({ mark: null, tint: false, italic: true }),
    },
  ],
});
