// How a thread reads: the agent's work, tool calls, edits, thinking, your messages, its status,
// and steering queued messages. t3code's timeline beside Zed's (today's).

ICONS.thought = '<path d="M15 14c.2-1 .7-1.7 1.5-2.5 1-.9 1.5-2.2 1.5-3.5A6 6 0 0 0 6 8c0 1 .2 2.2 1.5 3.5.7.7 1.3 1.5 1.5 2.5"/><path d="M9 18h6"/><path d="M10 22h4"/>';
ICONS.send = '<path d="M14.54 21.69a.5.5 0 0 0 .94-.03l6.5-19a.5.5 0 0 0-.64-.64l-19 6.5a.5.5 0 0 0-.03.94l7.93 3.18a2 2 0 0 1 1.11 1.11z"/><path d="m21.85 2.15-10.94 10.94"/>';
ICONS.steer = '<path d="M3 12h4l3 8 4-16 3 8h4"/>';

const JBT = '--ed:#1e1f22;--panel:#26282b;--title:#26282b;--t:#dfe1e5;--mu:#9da0a6;--ph:#6f737a;--b:#393b41;--bv:#393b41;--hov:#2e3033;--sel:#43454a;--ac:#548af7;--ok:#57965d;--del:#fa6675;';
const W = 640;
const tframe = (html, h = 520) => frame(`<div style="position:absolute;inset:0;overflow:hidden;background:#26282b;padding:14px 22px;display:flex;flex-direction:column;gap:6px">${html}</div>`, { w: W, h, style: JBT });

const PROMPT = 'Add the checkout total and a test for it.';
const para = (text) => `<div style="line-height:22px;color:var(--t)">${text}</div>`;
const ANSWER = para('The total now rounds once, at the end, so many cheap items add up to what the receipt says. I added <code style="font:12px \'IBM Plex Mono\',monospace;background:#2b2d30;padding:1px 4px;border-radius:3px">total.test.ts</code> for it, and the tests pass.');

// t3code's compact row: a 24px icon cell, a one-line label, something trailing.
const row = (icon, label, trailing = '', { open = false, indent = 0, muted = true } = {}) => `<div class="row" style="min-height:24px;gap:6px;padding-left:${indent}px;border-radius:5px">
  <span style="width:24px;display:inline-flex;justify-content:center;color:var(--mu)">${ic(icon, 'sm')}</span>
  <span class="grow trunc" style="color:${muted ? 'var(--mu)' : 'var(--t)'}">${label}</span>${trailing}${open ? `<span style="color:var(--ph)">${ic('chev-down', 'xs')}</span>` : ''}</div>`;
const code = (text) => `<span style="font:12px 'IBM Plex Mono',monospace;color:var(--t)">${text}</span>`;
const dim = (text) => `<span class="sm" style="color:var(--ph)">${text}</span>`;
const stat = (add, del) => `<span class="sm" style="color:#57965d">+${add}</span> <span class="sm" style="color:#fa6675">−${del}</span>`;
const groupHeader = (summary, open, trailing = '1m 08s') => `<div class="row" style="min-height:26px;gap:6px;border-radius:5px">
  <span style="width:24px;display:inline-flex;justify-content:center;color:var(--ph)">${ic(open ? 'chev-down' : 'chev-right', 'xs')}</span>
  <span class="grow trunc" style="color:var(--mu)">${summary}</span>${dim(trailing)}</div>`;

const T3_ROWS = [
  row('thought', 'Thought', dim('4s')),
  row('file', `Read ${code('cart/total.ts')}`),
  row('file', `Read ${code('cart/item.ts')}`),
  row('search', `Searched code for ${code('roundTotal')}`),
  row('pencil', `Edited ${code('cart/total.ts')}`, stat(4, 2)),
  row('pencil', `Edited ${code('cart/total.test.ts')}`, stat(12, 0)),
  row('terminal', `Ran ${code('npm test')}`, dim('3s')),
];
const SUMMARY = 'Read 2 files, changed 2 files, and performed 2 other actions';
const userT3 = `<div style="display:flex;justify-content:flex-end"><div style="max-width:78%;background:#2f3134;border-radius:12px;padding:8px 12px;line-height:21px">${PROMPT}</div></div>`;

// Today's (Zed's) pieces.
const userZed = `<div style="border:1px solid var(--b);border-radius:6px;background:var(--ed);padding:9px 12px;line-height:21px">${PROMPT}</div>`;
const zedRow = (icon, label) => `<div class="row g2" style="min-height:26px;color:var(--mu);padding:0 6px">${ic(icon, 'sm')}<span class="trunc">${label}</span></div>`;
const zedThinking = `<div class="row g2" style="min-height:26px;color:var(--mu);padding:0 6px">${ic('thought', 'sm')}<span>Thinking</span></div>`;
const zedDiff = (path, lines) => `<div style="border:1px solid var(--b);border-radius:6px;overflow:hidden">
  <div class="row g2" style="height:28px;padding:0 10px;background:#2b2d30;color:var(--mu)">${ic('pencil', 'sm')}<span>Edit ${path}</span></div>
  <div style="font:12px/18px 'IBM Plex Mono',monospace;background:var(--ed)">${lines.map(([kind, text]) => `<div style="padding:0 10px;white-space:pre;background:${kind === '+' ? 'rgba(84,145,89,.12)' : kind === '-' ? '#2b2322' : 'transparent'}"><span style="color:var(--ph);display:inline-block;width:14px">${kind === ' ' ? '' : kind}</span>${text}</div>`).join('')}</div></div>`;
const zedCommand = `<div style="border:1px solid var(--b);border-radius:6px;background:#2b2d30;padding:8px 12px"><div class="sm" style="color:var(--ph);font-family:'IBM Plex Mono',monospace">Run Command</div><div style="font:13px 'IBM Plex Mono',monospace;margin-top:4px">npm test</div></div>`;
const DIFF = [[' ', 'export function total(items) {'], ['-', '  return round(items.map(price).reduce(add))'], ['+', '  const sum = items.reduce((s, i) => s + i.price, 0)'], ['+', '  return round(sum)'], [' ', '}']];
const zedTurn = () => [userZed, zedThinking, zedRow('file', 'Read cart/total.ts'), zedRow('file', 'Read cart/item.ts'), zedRow('search', 'Search roundTotal'), zedDiff('cart/total.ts', DIFF), zedCommand, ANSWER].join('');

TOPICS.push({
  id: 'work', section: "The agent's work", title: 'Tool calls between messages', size: 'wide', rec: 'A',
  now: 'Each tool call is its own row, and edits and commands are cards with their diff or command, so a long turn is mostly tool calls and the answer is far down.',
  nowImg: 'img/now-turn.png',
  options: [
    {
      key: 'A', name: 'Folded into a line that says what it did', from: 't3code work groups',
      desc: 'Between the agent\'s messages, its work is one row: t3code\'s sentence of its two biggest kinds and a count of the rest ("Read 2 files, changed 2 files, and performed 2 other actions"), and how long it took. A click opens compact one-line rows. While the turn runs, the rows show live as they come; once it ends, they fold.',
      good: 'The conversation reads as messages; the work is one click away.', cost: 'A finished turn\'s steps aren\'t visible at a glance.',
      mock: () => pairT(tframe([userT3, groupHeader(SUMMARY, false), ANSWER].join(''), 190), tframe([userT3, groupHeader(SUMMARY, true), `<div>${T3_ROWS.join('')}</div>`, ANSWER].join(''), 360)),
    },
    {
      key: 'B', name: 'Compact rows, always open', from: 't3code rows, ungrouped',
      desc: 'Every tool call is t3code\'s compact one-line row, always shown, with no folding line. Edits and commands are rows too, not cards.',
      good: 'Everything visible, still much shorter than cards.', cost: 'Long turns are still long.',
      mock: () => tframe([userT3, `<div>${T3_ROWS.join('')}</div>`, ANSWER].join(''), 330),
    },
    {
      key: 'C', name: 'As it is', from: 'Zed (today)',
      desc: 'A row per tool call; edits and commands as cards.',
      good: 'Nothing to change; diffs at a glance.', cost: 'Long and busy.',
      mock: () => tframe(zedTurn(), 520),
    },
  ],
});

const pairT = (a, b) => `<div style="display:flex;flex-direction:column;gap:10px">${a}${b}</div>`;

TOPICS.push({
  id: 'row', section: "The agent's work", title: 'A tool call, opened', size: 'medium', rec: 'A',
  now: 'A command is a card with its command; its output shows in the card. Reads and searches are rows that open to their text.',
  options: [
    {
      key: 'A', name: 'The row opens to its output', from: 't3code WorkLogDetails',
      desc: 'A click on the row opens its output under it, indented past the icon, in the code font, at most 24rem tall then scrolling. The row says what ran; the output is only there when asked for.',
      good: 'Output never crowds the thread.', cost: 'One click to see an error.',
      mock: () => tframe([row('terminal', `Ran ${code('npm test')}`, dim('3s'), { open: true }), `<div style="margin-left:30px;font:12px/18px 'IBM Plex Mono',monospace;color:var(--mu);white-space:pre">PASS  src/cart/total.test.ts\n  ✓ rounds once (3 ms)\n  ✓ many cheap items (1 ms)\n\nTests: 2 passed, 2 total</div>`, row('file', `Read ${code('cart/item.ts')}`)].join(''), 230),
    },
    {
      key: 'B', name: 'A card', from: 'Zed (today)',
      desc: 'The command in a card, its output inside it.',
      good: 'The command stands out.', cost: 'Takes room even when it passed.',
      mock: () => tframe([zedCommand, `<div style="border:1px solid var(--b);border-top:0;margin-top:-7px;border-radius:0 0 6px 6px;background:var(--ed);padding:8px 12px;font:12px/18px 'IBM Plex Mono',monospace;color:var(--mu);white-space:pre">PASS  src/cart/total.test.ts\n  ✓ rounds once (3 ms)</div>`].join(''), 230),
    },
  ],
});

TOPICS.push({
  id: 'edits', section: "The agent's work", title: 'Edits', size: 'medium', rec: 'A',
  now: 'An edit is a card that always shows its diff.',
  options: [
    {
      key: 'A', name: 'A row with its line counts, opening to the diff', from: 't3code',
      desc: '"Edited cart/total.ts +4 −2" as a compact row; a click opens the diff under it, the same diff as the card has today.',
      good: 'Many edits stay short; the counts say how big each was.', cost: 'Diffs are a click away.',
      mock: () => tframe([row('pencil', `Edited ${code('cart/total.ts')}`, stat(4, 2), { open: true }), `<div style="margin-left:30px">${zedDiff('cart/total.ts', DIFF).replace(/<div class="row g2" style="height:28px[^]*?<\/div>/, '')}</div>`, row('pencil', `Edited ${code('cart/total.test.ts')}`, stat(12, 0))].join(''), 260),
    },
    {
      key: 'B', name: 'A row that opens the Changes panel', from: 'new',
      desc: 'The same row; a click shows the file in the Changes panel, at the turn.',
      good: 'The thread never holds a diff.', cost: 'Leaves the conversation to look.',
      mock: () => tframe([row('pencil', `Edited ${code('cart/total.ts')}`, stat(4, 2) + ' ' + `<span style="color:var(--ph)">${ic('external', 'xs')}</span>`), row('pencil', `Edited ${code('cart/total.test.ts')}`, stat(12, 0) + ' ' + `<span style="color:var(--ph)">${ic('external', 'xs')}</span>`)].join(''), 120),
    },
    {
      key: 'C', name: 'As it is', from: 'Zed (today)',
      desc: 'A card with the diff, always open.',
      good: 'Diffs at a glance.', cost: 'Long.',
      mock: () => tframe(zedDiff('cart/total.ts', DIFF), 200),
    },
  ],
});

TOPICS.push({
  id: 'thinking', section: "The agent's work", title: 'Thinking', size: 'medium', rec: 'A',
  now: 'A "Thinking" row that opens to the agent\'s thoughts, shown while they stream.',
  options: [
    {
      key: 'A', name: '"Thinking" while it thinks, then a Thought row', from: 't3code',
      desc: 'While the agent thinks, one "Thinking" line with a shimmer. After, a "Thought" row among the work rows (with how long), opening to the text.',
      good: 'Quiet; you see that it thought, and for how long.', cost: 'The thoughts aren\'t visible as they stream.',
      mock: () => tframe([row('thought', `<span style="background:linear-gradient(90deg,var(--mu),#dfe1e5,var(--mu));-webkit-background-clip:text;color:transparent">Thinking</span>`), '<div style="height:10px"></div>', row('thought', 'Thought', dim('4s'))].join(''), 120),
    },
    {
      key: 'B', name: 'As it is', from: 'Zed (today)',
      desc: 'A Thinking block that shows the thoughts as they stream, height-limited, closed after.',
      good: 'You can follow its reasoning live.', cost: 'Busy while it streams.',
      mock: () => tframe([zedThinking, `<div style="margin-left:28px;color:var(--mu);line-height:20px">The receipt rounds once at the end, but total() rounds each item. Summing first and rounding once should match…</div>`].join(''), 120),
    },
  ],
});

TOPICS.push({
  id: 'user', section: 'Messages', title: 'Your messages', size: 'medium', rec: 'A',
  now: 'A full-width box with a border, like the composer.',
  options: [
    {
      key: 'A', name: 'A bubble on the right', from: 't3code',
      desc: 'Your message in a rounded bubble with a soft background, on the right, at most about four fifths wide; its time and copy show on hover under it.',
      good: 'Who said what is clear at a glance.', cost: 'Long messages are narrower.',
      mock: () => tframe([userT3, `<div class="row" style="justify-content:flex-end;gap:8px;color:var(--ph)">${dim('09:14')}${ic('copy', 'xs')}</div>`, ANSWER].join(''), 170),
    },
    {
      key: 'B', name: 'As it is', from: 'Zed (today)',
      desc: 'A full-width box with a border.',
      good: 'Full width for long prompts.', cost: 'Looks like a form field.',
      mock: () => tframe([userZed, ANSWER].join(''), 150),
    },
  ],
});

TOPICS.push({
  id: 'status', section: 'Messages', title: 'While it works, and after', size: 'medium', rec: 'A',
  now: 'While it works, a spinner and the elapsed time at the end. After, copy, scroll-to-message and scroll-to-top buttons under the answer.',
  nowImg: 'img/now-end.png',
  options: [
    {
      key: 'A', name: '"Working for 1m 12s", then the turn\'s time', from: 't3code',
      desc: 'While it works, "Working for 1m 12s" at the end of the thread. After, a quiet line under the answer with how long the turn took and Copy.',
      good: 'Says how long, without buttons in the way.', cost: 'Scroll buttons go (the scrollbar remains).',
      mock: () => tframe([groupHeader('Read 2 files and changed 2 files', false, ''), `<div style="border-bottom:1px solid var(--b);padding:2px 4px 8px;color:var(--mu)">Working for 1m 12s</div>`, '<div style="height:16px"></div>', ANSWER, `<div class="row g2" style="color:var(--ph)">${dim('1m 12s')}${ic('copy', 'xs')}</div>`].join(''), 230),
    },
    {
      key: 'B', name: 'As it is', from: 'Zed (today)',
      desc: 'Spinner and time while it works; copy and scroll buttons after.',
      good: 'Nothing to change.', cost: 'Buttons under every answer.',
      mock: () => tframe([ANSWER, `<div class="row g3" style="justify-content:flex-end;color:var(--ph)">${ic('copy', 'sm')}${ic('undo', 'sm')}${ic('chev-up', 'sm')}</div>`].join(''), 130),
    },
  ],
});

const queued = (actions) => `<div style="border:1px solid var(--b);border-radius:8px;background:var(--panel);overflow:hidden">
  <div class="row g2 sm" style="height:28px;padding:0 10px;color:var(--ph)">${ic('chev-down', 'xs')}<span class="grow">1 queued message</span></div>
  <div class="row g2" style="min-height:34px;padding:0 10px;border-top:1px solid var(--b);background:var(--ed)"><span style="width:8px;height:8px;border-radius:50%;background:var(--ac)"></span><span class="grow trunc sm">Use the receipt's rounding mode, not banker's</span>${actions}</div></div>`;
const btn = (label, primary = false) => `<span class="btn sm" style="${primary ? 'border-color:var(--ac);color:var(--ac)' : ''}">${label}</span>`;
TOPICS.push({
  id: 'steer', section: 'Queued messages', title: 'Steering', size: 'medium', rec: 'C',
  now: 'A message typed while the agent works waits in the queue above the composer, sent when the turn ends. Send Now cancels the turn and sends it; you can also edit or remove it. Over ACP an agent takes one message at a time, so a message can\'t join a running turn: Zed steers only its own agent, and t3code steers ACP agents by cancelling and resending.',
  options: [
    {
      key: 'A', name: 'Steer at the next step', from: 'Zed\'s Steer toggle',
      desc: 'A Steer button on the next queued message: the agent finishes the step it\'s on (the tool call running), then its turn ends and your message goes at once, so it carries on with it. Nothing it\'s in the middle of is cut off.',
      good: 'Changes course without losing the current step.', cost: 'Waits for that step, which can be long.',
      mock: () => tframe(queued(btn('Steer', true) + btn('Edit') + btn('Remove')), 140),
    },
    {
      key: 'B', name: 'Steer now', from: 't3code\'s Steer (for ACP agents)',
      desc: 'Steer cancels the turn and sends the message right away, as Send Now does today.',
      good: 'Immediate.', cost: 'Cuts off the step it was on.',
      mock: () => tframe(queued(btn('Steer', true) + btn('Edit') + btn('Remove')), 140),
    },
    {
      key: 'C', name: 'Both: Steer, and Send Now', from: 'Zed',
      desc: 'Steer waits for the current step (A); Send Now cuts in at once, as today.',
      good: 'Both ways when you need them.', cost: 'Two buttons.',
      mock: () => tframe(queued(btn('Steer', true) + btn('Send Now') + btn('Edit') + btn('Remove')), 140),
    },
  ],
});
