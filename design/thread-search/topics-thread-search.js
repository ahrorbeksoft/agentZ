// Search in a thread: find messages and plans in the open thread, as t3code’s chat find does.

const tsFound = (kinds) => tsTotal(kinds);

// What the pictures of Today show.
const tsTodayThread = () => `<div>${tsScene({ conv: { highlight: false } })}${tsCaption('A thread: ⌘F does nothing here. Work between messages is folded into one line.')}</div>`;
const tsTodayTerminal = () => `<div>${frame(`<div class="col" style="height:100%"><div class="phead" style="background:var(--ed);color:var(--t)">${ic('terminal', 'sm')}<span>zsh</span><span class="mu">~/storefront</span><span class="grow"></span>${ibtn('x')}</div>${tsTermStrip({ query: 'error', count: '2/3' })}${term([
  `${prompt()}npm test`,
  '',
  ` ${C('tg', 'PASS')} src/app/cart/totals.test.ts`,
  ` ${C('tr', 'FAIL')} src/app/checkout/page.test.tsx`,
  `   ● shows an <mark class="ts-m">error</mark> when the card is declined`,
  `     Expected the <mark class="ts-m on">error</mark> message to be visible`,
  '',
  `Tests: 1 failed, 14 passed, <mark class="ts-m">error</mark>s: 1`,
])}</div>`, { w: 620, h: 250, style: 'border:1px solid var(--b);border-radius:8px' })}${tsCaption('The terminal’s find bar (⌘F with a terminal focused): the only find in agentZ')}</div>`;
const tsTodaySidebar = () => `<div>${tsSidebar([tsCard(TS_THREADS[0], { titleMatch: true })], { h: 300 })}${tsCaption('The sidebar’s search matches titles only')}</div>`;

// 1. The find bar ----------------------------------------------------------------------------
const tsHeaderSearch = () => `<span class="row" style="gap:4px;min-width:0;flex:1"><span class="ts-input" style="height:26px;max-width:300px">${ic('search', 'xs mu')}<span class="q" style="margin-left:4px">${TS_QUERY}<span class="caret"></span></span></span>${tsCount('2/5')}<span class="ts-ib sq">${ic('chev-up', 'sm')}</span><span class="ts-ib sq">${ic('chev-down', 'sm')}</span><span class="ts-ib sq">${ic('x', 'sm')}</span></span>`;
const tsBottomBar = () => `<div style="flex:none;border-top:1px solid var(--b)">${tsTermStrip({ placeholder: 'Find in thread' }).replace('border-bottom-color:var(--bv)', 'border-bottom:0')}</div>`;

TOPICS.push({
  id: 'bar', section: 'Find', title: 'The find bar', size: 'wide', rec: 'A',
  now: 'There’s no way to search a thread’s conversation. ⌘F (Ctrl-Shift-F on Linux) opens find only in a focused terminal: a strip above it with a text field, ‹ › (“Select Previous Match”, “Select Next Match”), the count (“2/3”, “0/0”, the field turning red when nothing matches) and × (“Close Search Bar”). In a thread ⌘F does nothing. The pictures are recreated from the code.',
  nowImg: 'img/now-terminal.png',
  issues: [
    'Finding where the agent said something, or which plan step mentioned a file, means scrolling and reading.',
    'The conversation is a virtualized list: only the rows on screen exist, so a match far up has to be brought into view by the app.',
  ],
  options: [
    {
      key: 'A', name: 'Floating at the top right', from: 't3code',
      desc: 't3code’s ThreadFindBar: a rounded bar floating over the conversation’s top right corner, 280 px wide. “Find in thread”, the count (“2/5”; “0/0” in red when nothing matches; “Searching…” while it looks), ↑ “Previous match”, ↓ “Next match”, × “Close find”. No case, word or regex options: matching ignores case.',
      good: 't3code’s find, which the backlog names. Nothing in the thread moves when it opens.',
      cost: 'Covers the conversation’s top right, where your messages sit.',
      mock: () => tsScene({ conv: { active: 2 }, overlays: tsFloat() }),
    },
    {
      key: 'B', name: 'A strip under the header', from: 'Zed',
      desc: 'Zed’s thread search: a full-width strip between the header and the conversation. “Search this thread…”, with Case Sensitive (Aa), Whole Word (ab) and Regex (.*) inside the box, then ‹ › and the count, and ×.',
      good: 'Covers nothing; the options find exact words or patterns.',
      cost: 'The conversation shifts down 42 px when it opens. Options most searches won’t need.',
      mock: () => tsScene({ conv: { active: 2 }, strip: tsZedStrip() }),
    },
    {
      key: 'C', name: 'The terminal’s find bar', from: 'agentZ (terminal)',
      desc: 'The find bar agentZ’s terminal already has, under the header: a text field, ‹ ›, the count and ×, with the same keys. One find bar across the app.',
      good: 'Already built and known from the terminal.',
      cost: 'Shifts the conversation down like B; looks like a terminal tool in a conversation.',
      mock: () => tsScene({ conv: { active: 2 }, strip: tsTermStrip({ placeholder: 'Find in thread' }) }),
    },
    {
      key: 'D', name: 'In the header, in place of the title', from: 'new',
      desc: 'While finding, the header’s title gives way to the search field, the count, ↑ ↓ and ×; the project crumb, branch and buttons stay. Closing brings the title back.',
      good: 'No extra bar at all, and nothing covered.',
      cost: 'The title is hidden while finding, and the field is short in a narrow thread.',
      mock: () => tsScene({ conv: { active: 2 }, header: tsHeader({ titleSlot: tsHeaderSearch() }) }),
    },
    {
      key: 'E', name: 'Above the composer', from: 'new',
      desc: 'The bar sits over the composer, along the bottom, as a browser’s find bar can.',
      good: 'Near where your hands and eyes already are.',
      cost: 'Matches are revealed near the top while the field is at the bottom.',
      mock: () => tsScene({ conv: { active: 2 }, above: tsBottomBar() }),
    },
  ],
});

// 2. What it searches -------------------------------------------------------------------------
const tsScope = (kinds, open = []) => tsScene({ conv: { kinds, active: 1, open }, overlays: tsFloat({ count: `1/${tsFound(kinds)}` }) });

TOPICS.push({
  id: 'scope', section: 'Find', title: 'What it searches', size: 'wide', type: 'multi', rec: 'ABC',
  now: 'Nothing is searched. A conversation holds your messages, the agent’s messages (Markdown), its thoughts and tool calls (folded into one line per run once a message follows them), plans it proposes (cards in the conversation) and ACP plan entries (the plan section above the composer, folded by default).',
  issues: ['Searching everything finds the word in every file the agent read; searching only messages misses a command it ran.'],
  options: [
    {
      key: 'A', name: 'Your messages', from: 't3code',
      desc: 'The text of your messages, as shown (attachments’ names aside).',
      good: 'Finds what you asked for and when.', cost: '—',
      mock: () => tsScope(['user']),
    },
    {
      key: 'B', name: 'The agent’s messages', from: 't3code',
      desc: 'The agent’s replies as rendered text, code blocks included (not the Markdown’s symbols).',
      good: 'Finds what the agent said or decided.', cost: '—',
      mock: () => tsScope(['agent']),
    },
    {
      key: 'C', name: 'Plans', from: 't3code',
      desc: 'Plans the agent proposes, and the steps in the plan section above the composer.',
      good: 'Finds which step covers something.', cost: 'The plan section is folded by default; a match there opens it.',
      mock: () => tsScope(['plan']),
    },
    {
      key: 'D', name: 'Thoughts', from: 'Zed',
      desc: 'The agent’s thinking. Zed searches thoughts only while they’re open.',
      good: 'Finds why the agent chose something.', cost: 'Thoughts are long and repetitive: many matches.',
      mock: () => tsScope(['thought'], [1]),
    },
    {
      key: 'E', name: 'Tool calls’ titles', from: 'Zed',
      desc: 'Each tool call’s line: “Read src/app/checkout/page.tsx”, “Searched for “promo””, the command it ran.',
      good: 'Finds when a file was read or a command was run.', cost: 'Every match is inside a folded line.',
      mock: () => tsScope(['tool'], [1, 5]),
    },
    {
      key: 'F', name: 'Tool output and diffs', from: 'Zed',
      desc: 'What tools returned: command output, files read, the diff of an edit. Zed searches it only while it’s shown.',
      good: 'Finds an error a test printed.', cost: 'Big outputs flood the count; slower on long threads.',
      mock: () => tsScope(['output'], [5]),
    },
  ],
});

// 3. A match inside something folded ---------------------------------------------------------
const TS_REVEAL_KINDS = ['user', 'agent', 'plan', 'tool'];
const tsFoldChip = (text) => `<span class="chip" style="height:18px;margin-left:6px;color:var(--ac);border-color:rgba(116,173,232,.4)">${text}</span>`;
const tsFoldedWithChip = () => tsScene({
  conv: { kinds: TS_REVEAL_KINDS, active: 2 },
  overlays: tsFloat({ count: '2/7' }),
}).replace('Thought, read 3 files and searched code 1 time</span>', `Thought, read 3 files and searched code 1 time</span>${tsFoldChip('1 match')}`).replace('Changed 2 files and ran 1 command</span>', `Changed 2 files and ran 1 command</span>${tsFoldChip('1 match')}`);

TOPICS.push({
  id: 'reveal', section: 'Find', title: 'A match inside something folded', size: 'wide', rec: 'A',
  now: 'Runs of tool calls and thoughts fold into one line (“Read 3 files and searched code 1 time”) once a message follows them, and the plan section is folded. The conversation follows its end while the agent works. The picture is recreated from the code.',
  nowImg: 'img/now-thread.png',
  issues: ['If tool calls or plans are searched, most of their matches are folded away.'],
  options: [
    {
      key: 'A', name: 'Open it and scroll to it', from: 't3code',
      desc: 't3code: going to a match opens what holds it (a folded run, the plan section, a long message) and scrolls so the match sits about 96 px below the top. While find is open, the conversation stops following the end; new text is searched as it streams in, keeping the current match.',
      good: 'Every match can be reached with Enter.',
      cost: 'Going through matches leaves runs open behind you.',
      mock: () => tsScene({ conv: { kinds: TS_REVEAL_KINDS, active: 3, open: [1], from: 40 }, overlays: tsFloat({ count: '3/7' }) }),
    },
    {
      key: 'B', name: 'Only what’s shown', from: 'Zed',
      desc: 'Zed: only what’s on display is searched. A folded run’s calls aren’t, until you open it; then its matches join the count.',
      good: 'Nothing opens by itself; the count is what you see.',
      cost: 'A word in a folded run isn’t found unless you guess where it is.',
      mock: () => tsScene({ conv: { kinds: TS_REVEAL_KINDS, active: 2 }, overlays: tsFloat({ count: '2/5' }) }),
    },
    {
      key: 'C', name: 'Counted on the fold', from: 'new',
      desc: 'Folded lines that hold matches say how many (“1 match”). Enter stops on the fold first; Enter again opens it and goes on inside.',
      good: 'You see where matches hide without opening everything.',
      cost: 'Two steps per fold, and a badge the folded line doesn’t have elsewhere.',
      mock: () => tsFoldedWithChip(),
    },
  ],
});

// 4. How matches look -------------------------------------------------------------------------
const tsLook = (cls, sb = '') => tsScene({ w: 600, h: 360, conv: { active: 2, from: 0 }, overlays: tsFloat({ count: '2/5' }), cls, sb });
const tsScrollMarks = () => '<div class="ts-sb"><b style="top:6px;height:120px"></b><i style="top:14px"></i><i class="on" style="top:62px"></i><i style="top:90px"></i><i style="top:98px"></i><i style="top:250px"></i></div>';

TOPICS.push({
  id: 'look', section: 'Find', title: 'How matches look', size: 'wide', rec: 'A',
  now: 'agentZ’s Markdown already paints search matches (Zed’s, used for nothing yet) in the theme’s colors: One Dark’s are blue for matches and orange for the current one, both see-through. The terminal’s find uses the same colors.',
  issues: [],
  options: [
    {
      key: 'A', name: 'The theme’s search colors', from: 'Zed',
      desc: 'Every match in the theme’s search color, the current one in its active color (One Dark: blue, orange), as Zed and agentZ’s terminal do.',
      good: 'Follows the theme, light or dark; already in the Markdown.',
      cost: 'Blue matches look like selected text in One Dark.',
      mock: () => tsLook(''),
    },
    {
      key: 'B', name: 'Yellow and amber', from: 't3code',
      desc: 't3code’s colors: matches pale yellow (dark yellow in dark themes) and the current one amber, the same in every theme.',
      good: 'Reads as “found” anywhere, like a highlighter.',
      cost: 'Ignores the theme’s own search colors.',
      mock: () => tsLook('t3'),
    },
    {
      key: 'C', name: 'Only the current one filled', from: 'new',
      desc: 'The current match filled in the active color; the others only underlined.',
      good: 'Calm when a word matches many times.',
      cost: 'Underlines are easy to miss in dense text.',
      mock: () => tsLook('under'),
    },
    {
      key: 'D', name: 'With marks on the scrollbar', from: 'Zed (editor)',
      desc: 'As A, plus a tick on the scrollbar for each match, as Zed’s editor marks search results, the current one in orange.',
      good: 'Shows where matches are in a long thread.',
      cost: 'Rows above the screen have estimated heights in a virtualized list, so ticks can sit a little off.',
      mock: () => tsLook('', tsScrollMarks()),
    },
  ],
});

// 5. Opening it -------------------------------------------------------------------------------
const tsKeys = (rows) => frame(`<div style="padding:14px 16px;display:flex;flex-direction:column;gap:10px;font-size:13px">${rows.map(([keys, text]) => `<div class="row g3"><span class="row g1" style="width:120px;flex:none">${keys.map(keycap).join('')}</span><span class="mu">${text}</span></div>`).join('')}</div>`, { w: 420, h: 30 + rows.length * 32, style: 'border:1px solid var(--b);border-radius:8px' });
const tsPalette = () => frame(`<div class="col" style="height:100%"><div class="row g2" style="height:40px;padding:0 12px;border-bottom:1px solid var(--b)">find<span class="caret"></span></div><div class="col" style="padding:4px">
  <div class="row g2" style="height:28px;padding:0 8px;border-radius:5px;background:var(--hov)"><span class="grow"><b style="font-weight:600">Find</b> in Thread</span>${keycap('⌘F')}</div>
  <div class="row g2" style="height:28px;padding:0 8px"><span class="grow">Terminal: <b style="font-weight:600">Find</b></span>${keycap('⌘F')}</div></div></div>`, { w: 420, h: 112, style: 'border:1px solid var(--b);border-radius:8px' });
const tsMoreMenu = () => tsScene({ h: 300, overlays: `<div class="menu" style="right:8px;top:38px;min-width:220px"><div class="it hl">${ic('search', 'sm')}Find in Thread<span class="kb">⌘F</span></div><div class="hr"></div><div class="it">Reauthenticate</div><div class="it">Log Out…</div><div class="hr"></div><div class="it">Reload Agent</div></div>` });

TOPICS.push({
  id: 'open', section: 'Find', title: 'Opening it and moving between matches', size: 'medium', type: 'multi', rec: 'ABC',
  now: '⌘F and Ctrl-Shift-F are bound only in the terminal. Nothing in the command palette or the thread’s menus finds text.',
  issues: [],
  options: [
    {
      key: 'A', name: '⌘F, Enter, Esc', from: 't3code',
      desc: 't3code’s keys: ⌘F (Ctrl-F on Linux) opens find unless a terminal is focused, which keeps its own; pressed again it selects the query. Enter and ⇧Enter go to the next and previous match. Esc closes it and puts the cursor back in the composer.',
      good: 'The keys every app uses.', cost: '—',
      mock: () => tsKeys([[['⌘', 'F'], 'Open, or select the query'], [['↩'], 'Next match'], [['⇧', '↩'], 'Previous match'], [['esc'], 'Close, back to the composer']]),
    },
    {
      key: 'B', name: '⌘G and ⌘⇧G', from: 'Zed',
      desc: 'Zed’s ⌘G and ⌘⇧G go to the next and previous match from anywhere in the thread, even with the cursor in the composer.',
      good: 'Step through matches while typing a reply.', cost: 'Two more keys to learn.',
      mock: () => tsKeys([[['⌘', 'G'], 'Next match'], [['⌘', '⇧', 'G'], 'Previous match']]),
    },
    {
      key: 'C', name: 'In the command palette', from: 't3code',
      desc: '“Find in Thread” in the command palette, found by “find”, “search”, “messages” or “plans”, as t3code’s “Find in current thread”.',
      good: 'Discoverable without knowing the key.', cost: '—',
      mock: () => tsPalette(),
    },
    {
      key: 'D', name: 'In the thread’s ⋯ menu', from: 'new',
      desc: 'Find in Thread heads the header’s ⋯ menu, with its key.',
      good: 'Found where the thread’s other actions are.', cost: 'A menu entry for something most people reach by key.',
      mock: () => tsMoreMenu(),
    },
    {
      key: 'E', name: 'A search button in the header', from: 'new',
      desc: 'A magnifier before the changes button opens find.',
      good: 'Visible all the time.', cost: 'One more button in a full header.',
      mock: () => tsScene({ h: 220, header: tsHeader({ extra: `<span class="ibtn hov">${ic('search', 'sm')}</span>` }) }),
    },
  ],
});

// 6. Switching threads ------------------------------------------------------------------------
const tsMini = (title, bar) => frame(`<div class="col" style="height:100%"><div class="row ts-head"><span class="row g15 sm mu none" style="padding:2px 4px">${mono('ST', 'g')}</span><span class="sm mu">/</span><span class="grow trunc" style="font-size:13px;padding:0 6px">${title}</span>${ibtn('more')}</div><div class="ts-conv" style="padding:14px 20px;align-items:stretch;justify-content:flex-end;font-size:13px;line-height:20px;color:var(--t)">The route returns 429 after 20 requests a minute.</div></div>${bar}`, { w: 380, h: 130, style: 'border:1px solid var(--b);border-radius:8px' });
const tsSwitch = (second, caption) => `<div style="display:flex;flex-direction:column;gap:8px">${tsMini('Add a promo code field', tsFloat({ style: 'top:44px;right:8px;width:250px' }))}${tsCaption('In “Add a promo code field”')}${tsMini('Rate limit the coupon endpoint', second)}${tsCaption(caption)}</div>`;

TOPICS.push({
  id: 'switch', section: 'Find', title: 'Switching threads', size: 'narrow', rec: 'A',
  now: 'There is no find in threads. Each pane of the Workspaces view and each tab can show a different thread.',
  issues: [],
  options: [
    {
      key: 'A', name: 'It closes', from: 't3code',
      desc: 't3code: find belongs to the thread it was opened in and closes when you go to another.',
      good: 'Never a stale bar on a thread you didn’t search.', cost: 'Searching several threads for the same word means typing it each time.',
      mock: () => tsSwitch('', 'After switching: no bar'),
    },
    {
      key: 'B', name: 'Each thread keeps its own', from: 'new',
      desc: 'Each thread remembers whether find was open and its query, until it’s closed or the app quits.',
      good: 'Come back to where you were in each thread.', cost: 'A bar left open somewhere you forgot.',
      mock: () => tsSwitch('', 'After switching: no bar here; going back shows “promo” again'),
    },
    {
      key: 'C', name: 'The query comes along', from: 'new',
      desc: 'The bar stays open with its query as you switch, searching each thread you open.',
      good: 'Look for one word across several threads quickly.', cost: 'Surprising when you just wanted to move on.',
      mock: () => tsSwitch(tsFloat({ count: '1/3', style: 'top:44px;right:8px;width:250px' }), 'After switching: “promo” searched here too'),
    },
  ],
});

// 7. Searching every thread -------------------------------------------------------------------
const tsMoreRow = () => `<div class="row g2" style="margin:6px 4px 0;height:30px;padding:0 10px;border-radius:6px;font-size:12px;color:var(--ac)">${ic('search', 'xs')}Search messages in every thread for “promo”</div>`;

TOPICS.push({
  id: 'sidebar', section: 'Every thread', title: 'The sidebar’s search', size: 'medium', rec: 'A',
  now: 'The search field in the sidebar’s head matches thread titles only, ignoring case; ↑ ↓ move, Enter opens, Esc clears. The picture is recreated from the code.',
  nowImg: 'img/now-sidebar.png',
  issues: ['A thread that talked about promo codes but is titled “Add the checkout page” isn’t found.'],
  options: [
    {
      key: 'A', name: 'Titles only, as today', from: 'backlog',
      desc: 'The backlog asks for search inside the open thread. The sidebar keeps matching titles.',
      good: 'Fast, and nothing to load from the server.', cost: 'Finding which thread said something stays guesswork.',
      mock: () => tsSidebar([tsCard(TS_THREADS[0], { titleMatch: true })], { h: 300 }),
    },
    {
      key: 'B', name: 'Titles and conversations', from: 'new',
      desc: 'The sidebar’s search also matches messages and plans in every thread on every machine; a card found by its conversation shows the matching line in place of its branch. Opening it opens find with the query.',
      good: 'Finds the thread from anything said in it.', cost: 'Each machine’s server has to search every conversation as you type.',
      mock: () => tsSidebar(TS_THREADS.slice(0, 3).map((thread, i) => tsCard(thread, { snippet: true, titleMatch: i === 0 })), { h: 300 }),
    },
    {
      key: 'C', name: 'A row to search messages', from: 'new',
      desc: 'Titles match as today; at the end of the results a row, “Search messages in every thread for “promo””, runs the wider search on demand.',
      good: 'Fast by default, deep when asked.', cost: 'Two steps, and the results need a list of their own.',
      mock: () => tsSidebar([tsCard(TS_THREADS[0], { titleMatch: true })], { h: 300, after: tsMoreRow() }),
    },
  ],
});
