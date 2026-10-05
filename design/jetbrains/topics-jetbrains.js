// JetBrains Dark in agentZ: changed lines and the Agents | Workspaces switch.

const JB = '--ed:#1e1f22;--panel:#26282b;--title:#26282b;--t:#dfe1e5;--mu:#b0b1b3;--ph:#6f737a;--b:#393b41;--bv:#393b41;--hov:#3c3e41;--sel:#43454a;';
const jbFrame = (html, { w = 640, h = 230 } = {}) => frame(html, { w, h, style: JB });

const DIFF_LINES = [
  [' ', 'export default function Checkout() {'],
  ['-', '  return <div>TODO</div>'],
  ['+', '  const cart = useCart()'],
  ['+', '  return <CheckoutLayout cart={cart} />'],
  [' ', '}'],
];
/** A tool call's edit, as agent_view.rs draws it. */
function diffBlock({ added, deleted }) {
  const rows = DIFF_LINES.map(([kind, text]) => {
    const background = kind === '+' ? added : kind === '-' ? deleted : 'transparent';
    return `<div class="row" style="height:20px;background:${background};font:13px 'IBM Plex Mono',monospace;white-space:pre"><span style="width:22px;text-align:center;color:var(--ph)">${kind === ' ' ? '' : kind}</span>${text.replace(/</g, '&lt;')}</div>`;
  }).join('');
  return `<div style="margin:14px;border:1px solid var(--b);border-radius:6px;overflow:hidden;background:var(--ed)">
    <div class="row g2" style="height:30px;padding:0 10px;background:var(--panel);border-bottom:1px solid var(--b)">${ic('pencil', 'sm mu')}<span>Edit src/app/checkout/page.tsx</span><span class="grow"></span><span class="sm" style="color:#57965d">+2</span><span class="sm" style="color:#fa6675">−1</span></div>
    <div style="padding:4px 0">${rows}</div></div>`;
}

TOPICS.push({
  id: 'diff', section: 'JetBrains Dark', title: 'Changed lines', size: 'medium', rec: 'A',
  now: 'Added and removed lines are filled with the theme\'s solid created and deleted backgrounds (#447152, #8f5247). One Dark sets those at 10%, so they only look heavy here.',
  nowImg: 'img/now-diff.png',
  options: [
    {
      key: 'A', name: 'Zed\'s diff colors', from: 'Zed editor',
      desc: 'Use the colors Zed\'s own editor uses for changed lines (<code>editor.diff_hunk.*.background</code>): in JetBrains Dark, the theme\'s green at 12% and its dark red #2b2322. For every theme, so diffs look as they do in Zed; One Dark barely changes.',
      good: 'What you see in Zed with this theme; a fix for every theme.', cost: 'Removed lines are faint in this theme.',
      mock: () => jbFrame(diffBlock({ added: 'rgba(84,145,89,.12)', deleted: '#2b2322' })),
    },
    {
      key: 'B', name: 'The theme\'s colors, tinted', from: 'One Dark\'s 10% tints',
      desc: 'Keep the keys agentZ uses, and change only JetBrains Dark: its created and deleted backgrounds become its green and red at about 15%, as One Dark tints them.',
      good: 'Removed lines stay clearly red.', cost: 'Only for this theme; agentZ still differs from Zed.',
      mock: () => jbFrame(diffBlock({ added: 'rgba(115,189,122,.15)', deleted: 'rgba(247,84,100,.15)' })),
    },
    {
      key: 'C', name: 'As it is', from: 'today',
      desc: 'Solid backgrounds.',
      good: 'Nothing to change.', cost: 'Heavy.',
      mock: () => jbFrame(diffBlock({ added: '#447152', deleted: '#8f5247' })),
    },
  ],
});

/** The view switch in the title bar. */
function viewSwitch({ selectedBackground, selectedText, border = 'transparent' }) {
  return `<div style="position:absolute;inset:0;background:var(--title);display:flex;align-items:center;justify-content:center">
    <div class="row" style="border:1px solid var(--b);border-radius:7px;overflow:hidden;font-size:14px;height:30px">
      <span class="row" style="height:100%;padding:0 14px;background:${selectedBackground};color:${selectedText};box-shadow:inset 0 0 0 1px ${border};border-radius:6px 0 0 6px">Agents</span>
      <span class="row" style="height:100%;padding:0 14px;color:var(--t)">Workspaces</span></div></div>`;
}
TOPICS.push({
  id: 'switch', section: 'JetBrains Dark', title: 'Agents | Workspaces', size: 'narrow', rec: 'A',
  now: 'The selected side is Zed\'s toggle button: the theme\'s info background (gray #393b41 here) with its accent text (saturated blue #3474f0). The same accent colors links and other highlights.',
  nowImg: 'img/now-toggle.png',
  options: [
    {
      key: 'A', name: 'A softer blue', from: 'JetBrains\' lighter blue, One Dark\'s tint',
      desc: 'Change JetBrains Dark only: its accent becomes the lighter blue it already uses for changed files (#70aeff), and the selected background that blue at about 15%. Links and other accents turn this blue too.',
      good: 'Reads like One Dark\'s switch; still clearly selected.', cost: 'Every accent in this theme changes a little.',
      mock: () => jbFrame(viewSwitch({ selectedBackground: 'rgba(112,174,255,.15)', selectedText: '#70aeff' }), { w: 300, h: 90 }),
    },
    {
      key: 'B', name: 'Gray, no blue', from: 'JetBrains segmented buttons',
      desc: 'The selected side is a lighter gray with the normal text color. A change to the switch itself, so in every theme.',
      good: 'Quietest.', cost: 'Changes One Dark\'s switch too; less obvious which is on.',
      mock: () => jbFrame(viewSwitch({ selectedBackground: '#43454a', selectedText: '#dfe1e5' }), { w: 300, h: 90 }),
    },
    {
      key: 'C', name: 'As it is', from: 'today',
      desc: 'Gray background, saturated blue text.',
      good: 'Nothing to change.', cost: 'The blue is harsh on gray.',
      mock: () => jbFrame(viewSwitch({ selectedBackground: '#393b41', selectedText: '#3474f0' }), { w: 300, h: 90 }),
    },
  ],
});
