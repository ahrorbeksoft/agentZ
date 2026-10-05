// Tool-call rows that read apart from the agent's messages: a row's text, and what sets a run of
// rows apart. The mocks replay the turn in the user's screenshot (adding JetBrains Light).

const JBT = '--ed:#1e1f22;--panel:#26282b;--title:#26282b;--t:#dfe1e5;--mu:#b0b1b3;--ph:#6f737a;--b:#393b41;--bv:#393b41;--hov:#3c3e41;--sel:#43454a;--ac:#548af7;--ok:#57965d;--del:#fa6675;';
// t3code's secondary label: the muted gray mixed a quarter of the way toward the background.
const DIM = '#8d8e91';
const W = 640;
const tframe = (html, h) => frame(`<div style="position:absolute;inset:0;overflow:hidden;background:var(--panel);padding:14px 22px;display:flex;flex-direction:column">${html}</div>`, { w: W, h, style: JBT });

const inlineCode = (text) => `<code style="font:12.5px 'IBM Plex Mono',monospace;background:#2b2d30;padding:1px 4px;border-radius:3px">${text}</code>`;
const para = (html) => `<div style="line-height:22px;color:var(--t);padding:4px 0">${html}</div>`;
const user = `<div style="display:flex;justify-content:flex-end;padding:4px 0 10px"><div style="max-width:78%;background:#2f3134;border-radius:12px;padding:8px 12px;line-height:21px">Add jetbrains light theme from zed to this app</div></div>`;

const CD = 'cd /Users/ahrorbek/projects/agentZ; ';
const RUNS = [
  [
    `${CD}find . -path ./target -prune -o -path ./references -prune -o -name '*.json' -print`,
    `${CD}git log --oneline -- assets/themes/jetbrains`,
    `${CD}git show --stat 25fd220; cat assets/themes/LICENSES`,
    `${CD}ls ~/Library/Application\\ Support/Zed/extensions/installed`,
    `${CD}F=~/Library/Application\\ Support/Zed/extensions/installed/jetbrains-themes`,
  ],
  [
    `${CD}F=~/Library/Application\\ Support/Zed/extensions/installed/jetbrains-themes/themes`,
    `${CD}sed -i '' 's/^## \\[JetBrains Dark\\](https://plugins.jetbrains.com/…`,
    `${CD}sed -n 176,182p docs/architecture.md; ls crates/app/src`,
  ],
  [
    `${CD}cargo test -p app --features gpui_platform/runtime_shaders theme`,
    `${CD}cargo fmt --all &amp;&amp; cargo build 2&gt;&amp;1 | tail -3`,
    `${CD}pkill -f "target/debug/agentz$"; python3 -c 'import os, subprocess'`,
  ],
];
const MESSAGES = [
  para('I’m copying JetBrains Light from the JetBrains Themes Zed extension, the same way JetBrains Dark was added, then I’ll test it and commit.'),
  para(`Themes are picked up from ${inlineCode('assets/themes')} automatically. Running the test:`),
  para('I’ve added JetBrains Light to agentZ, and it should appear in the theme list now.') +
    `<div style="line-height:22px;padding:2px 0 0 18px;position:relative"><span style="position:absolute;left:4px">•</span><b style="font-weight:600">Source:</b> Zed doesn’t ship this theme itself; it comes from the JetBrains Themes extension installed in your Zed. I copied it into ${inlineCode('assets/themes/jetbrains/jetbrains-light.json')}.</div>`,
];

// How a row's text is drawn.
const TEXT = {
  // Today: "Ran" in the muted gray, the command in the code font in the text color.
  now: { icon: 'var(--mu)', verb: 'color:var(--mu);font-size:13px', command: "color:var(--t);font:12px 'IBM Plex Mono',monospace" },
  // t3code: one label in its secondary gray, in the UI font.
  t3: { icon: DIM, verb: `color:${DIM};font-size:13px`, command: `color:${DIM};font-size:13px` },
  // Dimmed, keeping the code font for the command.
  dimCode: { icon: DIM, verb: `color:${DIM};font-size:13px`, command: `color:${DIM};font:12px 'IBM Plex Mono',monospace` },
};
const row = (command, text, { iconBackground = '' } = {}) => `<div class="row" style="min-height:24px;gap:6px;padding:0 2px;border-radius:5px">
  <span style="width:24px;height:${iconBackground ? 16 : 20}px;display:inline-flex;align-items:center;justify-content:center;color:${text.icon};${iconBackground ? `background:${iconBackground};position:relative` : ''}">${ic('terminal', 'sm')}</span>
  <span class="row grow" style="gap:4px;min-width:0"><span class="none" style="${text.verb}">Ran</span><span class="trunc" style="${text.command}">${command}</span></span></div>`;

// What holds a run of rows together, between two messages.
const RUN = {
  now: (rows) => `<div>${rows.join('')}</div>`,
  space: (rows) => `<div style="padding:10px 0">${rows.join('')}</div>`,
  rail: (rows) => `<div style="position:relative;margin:8px 0"><div style="position:absolute;left:14px;top:10px;bottom:10px;width:1px;background:#4a4d54"></div>${rows.join('')}</div>`,
  panel: (rows) => `<div style="margin:8px 0;padding:4px 6px;border:1px solid var(--b);border-radius:8px;background:var(--ed)">${rows.join('')}</div>`,
};
const fold = (summary) => `<div class="row" style="min-height:24px;gap:6px;padding:0 2px;margin:8px 0;color:${DIM};font-size:13px">
  <span style="width:24px;display:inline-flex;justify-content:center">${ic('chev-right', 'xs')}</span><span class="grow">${summary}</span></div>`;

/** The turn, with each run drawn by `run` and each row's text by `text`. */
function turn({ text, run, runs = 3, withUser = true, rowOptions = {} }) {
  const parts = withUser ? [user] : [];
  for (let index = 0; index < runs; index++) {
    parts.push(run(RUNS[index].map((command) => row(command, text, rowOptions))));
    parts.push(MESSAGES[index]);
  }
  return parts.join('');
}

TOPICS.push({
  id: 'text', section: 'Rows', title: 'A row’s text', size: 'wide', rec: 'B',
  now: 'A row says what the tool call did: "Ran" in the theme’s muted gray, then the command in the code font in the text color, the same color as the agent’s messages. In JetBrains Dark the muted gray (#b0b1b3) is close to the text color (#dfe1e5), so the whole row is nearly as bright as a message.',
  nowImg: 'img/now.png',
  issues: ['A run of commands looks like more paragraphs of the answer.'],
  options: [
    {
      key: 'A', name: 'One dim gray, in the UI font', from: 't3code',
      desc: 'The whole row, icon and label, in t3code’s secondary gray: the muted gray mixed a quarter of the way toward the background. The command is in the UI font like the rest of the label, as t3code draws it. Bright text is only for messages.',
      good: 'Rows recede; the answer stands out.', cost: 'Commands are harder to tell apart from the verb without the code font.',
      mock: () => tframe(turn({ text: TEXT.t3, run: RUN.now, runs: 2, withUser: false }), 300),
    },
    {
      key: 'B', name: 'One dim gray, the command in the code font', from: 't3code’s gray, today’s code font',
      desc: 'The same dim gray for the whole row, but the command keeps the code font, so it still reads as code.',
      good: 'Rows recede, and commands and paths still scan as code.', cost: 'Two fonts in one row.',
      mock: () => tframe(turn({ text: TEXT.dimCode, run: RUN.now, runs: 2, withUser: false }), 300),
    },
    {
      key: 'C', name: 'As it is', from: 'today',
      desc: '"Ran" in the muted gray, the command in the code font in the text color.',
      good: 'Nothing to change; commands are easy to read.', cost: 'Rows read like the answer.',
      mock: () => tframe(turn({ text: TEXT.now, run: RUN.now, runs: 2, withUser: false }), 300),
    },
  ],
});

TOPICS.push({
  id: 'run', section: 'Rows', title: 'Between messages', size: 'wide', rec: 'A',
  now: 'Rows sit right under and above the agent’s messages, with no space and nothing around them, so where a message ends and the work starts is unclear.',
  nowImg: 'img/now.png',
  options: [
    {
      key: 'A', name: 'Space around each run', from: 't3code',
      desc: 'A run of rows gets 10px of space above and below it, as t3code spaces a work block from the messages. Nothing else around the rows. (The mocks draw rows as the first topic’s B.)',
      good: 'Quiet; messages read as paragraphs again.', cost: 'Leans on the dim color to tell rows apart.',
      mock: () => tframe(turn({ text: TEXT.dimCode, run: RUN.space }), 620),
    },
    {
      key: 'B', name: 'A line joining the icons', from: 'new',
      desc: 'A thin gray line runs down through a run’s icons, so its rows read as one block of work hanging off the left edge. Messages have no line.',
      good: 'Each run is plainly one block, even in a light theme.', cost: 'Something neither Zed nor t3code draws.',
      mock: () => tframe(turn({ text: TEXT.dimCode, run: RUN.rail, rowOptions: { iconBackground: 'var(--panel)' } }), 620),
    },
    {
      key: 'C', name: 'A panel around each run', from: 'new, in the look of Zed’s tool cards',
      desc: 'Each run sits in one rounded panel with the editor’s darker background and a border, as Zed draws a tool call’s card, but one panel per run instead of one per call.',
      good: 'The strongest split between work and answer.', cost: 'Heavier; boxes between every paragraph.',
      mock: () => tframe(turn({ text: TEXT.dimCode, run: RUN.panel }), 640),
    },
    {
      key: 'D', name: 'Folded once the turn ends', from: 't3code work groups',
      desc: 'While the agent works, rows show as they come. When the turn ends, each run folds into one line that says what it did ("Ran 5 commands"), opening to its rows on click. The thread round picked rows always open; this is t3code’s default.',
      good: 'A finished turn reads as its messages.', cost: 'The steps are a click away once it’s done.',
      mock: () => tframe([user, fold('Ran 5 commands'), MESSAGES[0], fold('Ran 3 commands'), MESSAGES[1], fold('Ran 3 commands'), MESSAGES[2]].join(''), 400),
    },
    {
      key: 'E', name: 'As it is', from: 'today',
      desc: 'No space or anything else around a run. (Rows drawn as the first topic’s B, to compare only the spacing.)',
      good: 'Most compact.', cost: 'Where a message ends is unclear.',
      mock: () => tframe(turn({ text: TEXT.dimCode, run: RUN.now }), 580),
    },
  ],
});
