// Helpers for the tool-calls-2 round: a thread's tool calls as agent_view.rs draws them today,
// in One Dark at the app's sizes (rows 24 px, row text 13 px, output 12 px on 17 px lines),
// and the pieces the options add. Loaded after board/mock.js.

// Zed's 16-unit tool icons (assets/icons), drawn at 1.5× in mock.js's 24-unit box.
const z16 = (paths) => `<g transform="scale(1.5)" stroke-width="1.2">${paths}</g>`;
const filled = (d) => `<path d="${d}" fill="currentColor" stroke="none" fill-rule="evenodd"/>`;
Object.assign(ICONS, {
  'z-search': z16('<path d="M13 13L11 11"/><circle cx="7.5" cy="7.5" r="4.5"/>'),
  'z-terminal': z16('<rect x="2.5" y="2.5" width="11" height="11" rx="1.2"/><path d="M8 10.75h2.75"/><path d="m5.25 9.22 1.83-1.83-1.83-1.84"/>'),
  'z-think': z16('<path d="M9.95 10.26c.13-.64 0-.66.98-1.78.57-.66.98-1.41.98-2.24a3.92 3.84 0 0 0-7.83 0c0 .64.13 1.41.98 2.24.98 1.14.85 1.14.98 1.78m3.91 0v2.13c0 .66-.55 1.2-1.23 1.2H7.27c-.68 0-1.23-.54-1.23-1.2v-2.13m3.91 0H6.04"/>'),
  'z-web': z16('<circle cx="8" cy="8" r="5.48"/><path d="M8 3a7.2 7.2 0 0 0 0 10 7.2 7.2 0 0 0 0-10Z"/><path d="M3.24 7.05a6.9 6.9 0 0 0 9.52 0"/>'),
  'z-delete': z16('<path d="M9.5 2.5H5a1 1.1 0 0 0-1 1.1v8.8a1 1.1 0 0 0 1 1.1h6a1 1.1 0 0 0 1-1.1V5.25Z"/><path d="M9.34 6.82 6.66 9.51"/><path d="m6.66 6.82 2.68 2.69"/>'),
  'z-hammer': z16('<path d="M9 8.5 4.95 12.62a1.25 1.25 0 0 1-1.75-1.75L7.5 6.5"/><path d="m10.84 9.98 3-3"/><path d="m12.84 7.42-1.07-1a1 1 0 0 1-.33-.73v-.61L10.17 3.9A3.4 3.4 0 0 0 7.82 3l-1.98-.01.52.43a2.9 2.9 0 0 1 1.15 2.4L7.5 6.5 9 8.5l.5-.5s.37-.2.58-.01l1.07 1"/>'),
  'z-arrows': z16('<path d="M11 2l2 2.5L11 7"/><path d="M12.5 4.5h-10"/><path d="M5 14l-2-2.5L5 9"/><path d="M3 11.5h10"/>'),
  'z-eye': z16('<path d="M2.04 8.21a.6.6 0 0 1 0-.42 6.5 6.5 0 0 1 11.92 0 .6.6 0 0 1 0 .42 6.5 6.5 0 0 1-11.92 0"/><circle cx="8" cy="8" r="1.8"/>'),
  'z-pen': '<g stroke-width="1.8"><path d="M12 3H5a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7"/><path d="M18.375 2.625a1 1 0 0 1 3 3l-9.013 9.014a2 2 0 0 1-.853.505l-2.873.84a.5.5 0 0 1-.62-.62l.84-2.873a2 2 0 0 1 .506-.852z"/></g>',
  'z-plug': z16('<path d="M8 14.67v-3.34"/><path d="M6 5.33v-4"/><path d="M10 5.33v-4"/><path d="M12 5.33v3.34a2.67 2.67 0 0 1-2.67 2.66H6.67A2.67 2.67 0 0 1 4 8.67V5.33Z"/>'),
  'z-load': z16('<path d="M13 8a5 5 0 1 1-3.455-4.755"/>'),
  'z-file': z16('<path d="M3 5h8"/><path d="M3 8h10"/><path d="M3 11h6"/>'),
  'z-ts': z16(`<rect x="1.8" y="1.8" width="12.4" height="12.4" rx="2"/><text x="8" y="11.2" font-size="6.6" font-weight="700" text-anchor="middle" fill="currentColor" stroke="none" font-family="IBM Plex Sans,sans-serif">TS</text>`),
  'z-json': z16('<path d="M5.5 3H5a1.5 1.5 0 0 0-1.5 1.5v2A1.5 1.5 0 0 1 2 8a1.5 1.5 0 0 1 1.5 1.5v2A1.5 1.5 0 0 0 5 13h.5"/><path d="M10.5 3h.5a1.5 1.5 0 0 1 1.5 1.5v2A1.5 1.5 0 0 0 14 8a1.5 1.5 0 0 0-1.5 1.5v2A1.5 1.5 0 0 1 11 13h-.5"/>'),
  'z-image': z16('<path d="M3 11l3-3 2.38 2.38"/><path d="M7 9l3-3 3 3"/><path d="M4.38 3H3.5a.5.5 0 0 0-.5.5v9a.5.5 0 0 0 .5.5h9a.5.5 0 0 0 .5-.5v-9a.5.5 0 0 0-.5-.5h-1.86"/><circle cx="7.5" cy="3.25" r=".6" fill="currentColor"/>'),
  'z-close': z16('<path d="M4 4l8 8"/><path d="M12 4l-8 8"/>'),
  'z-check': z16('<path d="M3 8.5 6.5 12 13 4.5"/>'),
  'z-warn': z16('<path d="M7.13 2.5a1 1 0 0 1 1.74 0l5.2 9a1 1 0 0 1-.87 1.5H2.8a1 1 0 0 1-.87-1.5Z"/><path d="M8 6.2v2.8"/><path d="M8 11.1v.01"/>'),
  'z-list': z16('<path d="M6 4h8"/><path d="M6 8h8"/><path d="M6 12h8"/><path d="M2.5 4h.01"/><path d="M2.5 8h.01"/><path d="M2.5 12h.01"/>'),
  'z-book': z16('<path d="M3 12.5V3.5A1.5 1.5 0 0 1 4.5 2H13v10H4.5A1.5 1.5 0 0 0 3 13.5 1.5 1.5 0 0 0 4.5 15H13"/>'),
  'z-chat': z16('<path d="M13.5 9.5a1.33 1.33 0 0 1-1.33 1.33H4.83L2.5 13.17V3.83A1.33 1.33 0 0 1 3.83 2.5h8.34a1.33 1.33 0 0 1 1.33 1.33Z"/>'),
  'z-stop': z16('<rect x="4" y="4" width="8" height="8" rx="1.2"/>'),
  'z-folder': z16('<path d="M13.5 12.5a1 1 0 0 0 1-1v-6a1 1 0 0 0-1-1H8.3a1 1 0 0 1-.83-.45l-.6-.9A1 1 0 0 0 6.04 2.5H2.5a1 1 0 0 0-1 1v8a1 1 0 0 0 1 1Z"/>'),
  'z-arrow-up-right': z16('<path d="M5 11l6-6"/><path d="M5.5 5H11v5.5"/>'),
  'z-wrap': z16('<path d="M2.5 4h11"/><path d="M2.5 8h9a2 2 0 0 1 0 4H9"/><path d="m10.5 10.5-1.5 1.5 1.5 1.5"/><path d="M2.5 12h4"/>'),
  'z-copy': z16('<rect x="5.5" y="5.5" width="8" height="8" rx="1.2"/><path d="M3.5 10.5h-.3a.7.7 0 0 1-.7-.7V3.2a.7.7 0 0 1 .7-.7h6.6a.7.7 0 0 1 .7.7v.3"/>'),
  agentz: '<rect x="3" y="3" width="18" height="18" rx="4" opacity=".55"/><path d="M8 8h8l-8 8h8"/>',
});

const DIM = '#8b909c'; // work_row_color: the muted text a quarter of the way to the panel
const MONO = "Lilex, 'IBM Plex Mono', 'SF Mono', Menlo, monospace";
const W = 720;
const COLORS = {
  added: 'rgba(39,166,87,.24)', removed: 'rgba(224,108,118,.22)', addedWord: 'rgba(39,166,87,.5)', removedWord: 'rgba(224,108,118,.48)',
  lineNumber: '#5d636f', created: '#a1c181', deleted: '#d07277', head: '#33373f', bubble: '#40454f', error: '#d07277',
};
const escHtml = (text) => String(text).replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');

// The conversation, at its margins: the panel's background, rows 20 px in (mx_5). Mocks take the
// height of what's in them; only their width is fitted to the card.
const tframe = (html, h, w = W) => frame(`<div style="background:var(--panel);padding:12px 20px 14px;display:flex;flex-direction:column;gap:2px">${html}</div>`, { w, h, style: 'height:auto' });
const stack = (...parts) => `<div style="display:flex;flex-direction:column;gap:2px">${parts.join('')}</div>`;

// The user's bubble (rounded_xl, the panel a tenth of the way to the text) and the agent's text.
const you = (text) => `<div style="display:flex;justify-content:flex-end;padding:4px 0 10px"><div style="max-width:80%;background:${COLORS.bubble};border-radius:12px;padding:8px 12px;line-height:21px;font-size:14px">${text}</div></div>`;
const say = (html) => `<div style="line-height:22px;color:var(--t);padding:6px 0;font-size:14px">${html}</div>`;
const inlineCode = (text) => `<code style="font:12.5px ${MONO};background:#3a3f4a;border-radius:4px;padding:1px 4px">${text}</code>`;

// A row (render_tool_call): a 24 px icon cell, the label in the rows' gray, what trails, and the
// chevron on hover.
function row(icon, label, { trailing = '', hover = false, open = null, iconColor = DIM, labelColor = DIM, style = '' } = {}) {
  const chevron = open === null ? '' : `<span style="color:var(--ph);display:inline-flex;margin:0 2px;visibility:${hover ? 'visible' : 'hidden'}">${ic(open ? 'chev-up' : 'chev-down', 'xs')}</span>`;
  return `<div class="row" style="min-height:24px;gap:6px;padding:0 2px;border-radius:6px;flex:none;${hover ? 'background:var(--hov);' : ''}${style}">
    <span style="width:24px;display:inline-flex;justify-content:center;color:${iconColor};flex:none">${icon}</span>
    <span class="row grow" style="gap:4px;min-width:0;font-size:13px;color:${labelColor}">${label}</span>${trailing}${chevron}</div>`;
}
const I = (name) => ic(name, 'sm');
const verb = (text) => `<span class="none">${text}</span>`;
const subj = (text) => `<span class="trunc">${text}</span>`;
const code = (text) => `<span class="trunc" style="font:12px ${MONO}">${text}</span>`;
const codeNone = (text) => `<span class="none" style="font:12px ${MONO}">${text}</span>`;
const titled = (text) => `<span class="trunc" style="color:var(--mu)">${text}</span>`;
const dim = (text, style = '') => `<span class="none" style="font-size:12px;color:var(--ph);${style}">${text}</span>`;
const dimTrunc = (text, style = '') => `<span class="trunc" style="font-size:12px;color:var(--ph);${style}">${text}</span>`;
const stat = (added, removed) => `<span class="none" style="font-size:12px;margin-right:4px"><span style="color:${COLORS.created}">+${added}</span> <span style="color:${COLORS.deleted}">−${removed}</span></span>`;
const failedLabel = (text = 'Failed') => `<span class="none" style="font-size:12px;color:${COLORS.error};margin-right:2px">${text}</span>`;
const spinner = () => `<span class="none t2-spin" style="display:inline-flex;color:var(--mu)">${ic('z-load', 'xs')}</span>`;
const openButton = (text = 'Open') => `<span class="none row" style="gap:3px;font-size:12px;color:var(--ac);padding:0 4px">${text}${ic('z-arrow-up-right', 'xs')}</span>`;
const shine = (text) => `<span class="trunc t2-shine">${text}</span>`;

// A folded run (render_work_run_header): a chevron in the icon cell, then what it did.
const runRow = (summary, { open = false, hover = false, trailing = '' } = {}) => `<div class="row" style="min-height:24px;gap:6px;padding:0 2px;border-radius:6px;margin:4px 0;flex:none;${hover ? 'background:var(--hov);' : ''}">
  <span style="width:24px;display:inline-flex;justify-content:center;color:${DIM}">${ic(open ? 'chev-down' : 'chev-right', 'xs')}</span>
  <span class="row grow" style="gap:6px;min-width:0;font-size:13px;color:${DIM}">${summary}</span>${trailing}</div>`;

// What opens under a row: past its icon (ml 30 px), scrolling past 24 rem.
const out = (html, style = '') => `<div style="margin-left:30px;padding:4px 0;display:flex;flex-direction:column;gap:4px;flex:none;${style}">${html}</div>`;
// The thoughts' line under the icon (render_thinking_block).
const thoughtText = (html) => `<div style="margin-left:13px;padding:4px 0 4px 16px;border-left:1px solid var(--b);color:var(--mu);font-size:14px;line-height:21px;flex:none">${html}</div>`;

// Tool output as printed today (as_code_block): one block in the code font on the editor's
// background, long lines wrapped at its edge.
function printed(text, { maxH = 0, style = '' } = {}) {
  const body = Array.isArray(text) ? text.join('\n') : text;
  return `<div style="position:relative;border:1px solid var(--bv);border-radius:6px;background:var(--ed);padding:8px 10px;overflow:hidden;${maxH ? `max-height:${maxH}px;` : ''}${style}">
    <div style="font:12px/17px ${MONO};color:var(--t);white-space:pre-wrap;overflow-wrap:anywhere">${body}</div></div>`;
}
// The Input line at an opened output's end (render_tool_input).
const inputLine = (open = false) => `<div class="row" style="gap:4px;font:11px ${MONO};color:var(--ph);height:20px">Input${ic(open ? 'chev-up' : 'chev-down', 'xs')}</div>`;
// A client terminal shown live in a tool call.
const termBlock = (lines, { h = 0 } = {}) => `<div style="background:var(--term);border-radius:6px;padding:4px 8px;font:12px/17px ${MONO};color:#c8ccd4;white-space:pre;overflow:hidden;${h ? `height:${h}px;` : ''}">${lines.join('\n')}</div>`;
// Today's diff (render_diff): a line on top, a − or + before each line, the removed on red and
// the added on green, context muted; no numbers or colors.
function diffToday(lines) {
  return `<div style="border-top:1px solid rgba(70,75,87,.8);font:12px/18px ${MONO};overflow:hidden">${lines.map(([kind, text]) => {
    const background = kind === '-' ? COLORS.removed : kind === '+' ? COLORS.added : 'transparent';
    return `<div style="display:flex;padding:0 8px;white-space:pre;background:${background};color:${kind === ' ' ? 'var(--mu)' : 'var(--t)'}"><span style="width:14px;flex:none;color:var(--mu)">${kind === ' ' ? ' ' : kind}</span>${escHtml(text)}</div>`;
  }).join('')}</div>`;
}

// One Dark's syntax colors, and a small highlighter for the mocks' TypeScript and JSON.
const SYNTAX = { keyword: '#b477cf', string: '#a1c181', number: '#bf956a', comment: '#5d636f', function: '#73ade9', type: '#6eb4bf', punctuation: '#acb2be', property: '#d07277', text: '#dce0e5' };
const KEYWORDS = new Set('import from export function const let return type interface if else for of new await async default'.split(' '));
const span = (color, text) => `<span style="color:${color}">${escHtml(text)}</span>`;
function hl(line, lang = 'ts') {
  const pattern = /(\/\/.*$)|('(?:[^'\\]|\\.)*'|"(?:[^"\\]|\\.)*"|`[^`]*`)|(-?\b\d+(?:\.\d+)?\b)|([A-Za-z_$][\w$]*)|(\s+)|([^\sA-Za-z_$\d'"`]+)/g;
  let html = '';
  let match;
  while ((match = pattern.exec(line))) {
    const [, comment, string, number, word, space, punctuation] = match;
    const rest = line.slice(pattern.lastIndex);
    if (comment) html += span(SYNTAX.comment, comment);
    else if (string) html += span(lang === 'json' && /^\s*:/.test(rest) ? SYNTAX.property : SYNTAX.string, string);
    else if (number) html += span(SYNTAX.number, number);
    else if (word) {
      const color = lang === 'json' ? SYNTAX.number
        : KEYWORDS.has(word) ? SYNTAX.keyword
        : /^\s*\(/.test(rest) ? SYNTAX.function
        : /^[A-Z]/.test(word) ? SYNTAX.type
        : /^\s*:/.test(rest) && !/^\s*::/.test(rest) ? SYNTAX.property : SYNTAX.text;
      html += span(color, word);
    } else if (space) html += space;
    else html += span(SYNTAX.punctuation, punctuation);
  }
  return html;
}
// A git diff as a terminal colors it.
function diffColored(line) {
  if (/^(diff --git|index )/.test(line)) return `<span style="color:var(--t);font-weight:600">${escHtml(line)}</span>`;
  if (/^(---|\+\+\+) /.test(line)) return `<span style="color:var(--t);font-weight:600">${escHtml(line)}</span>`;
  if (line.startsWith('@@')) { const [, hunk, rest] = line.match(/^(@@[^@]*@@)(.*)$/) || [null, line, '']; return `<span style="color:#6eb4bf">${escHtml(hunk)}</span>${escHtml(rest)}`; }
  if (line.startsWith('+')) return `<span style="color:${COLORS.created}">${escHtml(line)}</span>`;
  if (line.startsWith('-')) return `<span style="color:${COLORS.deleted}">${escHtml(line)}</span>`;
  return escHtml(line);
}

// A file view, the options' block: a header (an icon, what it is, the line count, Copy), then the
// lines, each with a number in a gutter if asked, wrapped at the edge or scrolling sideways.
function fileView(lines, { icon = '', title = '', meta = '', buttons = false, numbers = null, maxH = 0, fade = false, showAll = '', scroll = false, marks = [], style = '' } = {}) {
  const gutter = numbers === null ? 0 : String(numbers + lines.length - 1).length * 7.3 + 6;
  const body = lines.map((line, index) => {
    const mark = marks[index];
    const background = mark === '+' ? COLORS.added : mark === '-' ? COLORS.removed : mark === 'hl' ? 'rgba(116,173,232,.12)' : 'transparent';
    return `<div style="display:flex;background:${background};padding:0 10px">${numbers === null ? '' : `<span style="width:${gutter}px;flex:none;text-align:right;color:${COLORS.lineNumber};margin-right:14px">${numbers + index}</span>`}<span style="white-space:${scroll ? 'pre' : 'pre-wrap'};overflow-wrap:anywhere;min-width:0;flex:1">${line || ' '}</span></div>`;
  }).join('');
  const head = title ? `<div class="row" style="height:28px;gap:6px;padding:0 4px 0 10px;background:${COLORS.head};border-bottom:1px solid var(--bv);font-size:12px;color:var(--mu)">
      ${icon ? `<span style="display:inline-flex;color:${DIM}">${ic(icon, 'xs')}</span>` : ''}<span class="trunc" style="font:12px ${MONO};color:var(--t)">${title}</span>${meta ? `<span class="none" style="color:var(--ph)">${meta}</span>` : ''}<span class="grow"></span>
      ${buttons ? `<span class="row" style="gap:2px">${miniButton('z-wrap')}${miniButton('z-copy')}</span>` : ''}</div>` : '';
  return `<div style="border:1px solid var(--bv);border-radius:6px;background:var(--ed);overflow:hidden;flex:none;${style}">${head}
    <div style="position:relative;padding:6px 0;font:12px/17px ${MONO};color:var(--t);overflow:hidden;${maxH ? `max-height:${maxH}px;` : ''}">${body}
      ${fade ? `<div style="position:absolute;left:0;right:0;bottom:0;height:56px;background:linear-gradient(rgba(40,44,51,0),var(--ed))"></div>` : ''}
      ${scroll ? '<div style="position:absolute;left:10px;bottom:1px;width:42%;height:5px;border-radius:3px;background:rgba(220,224,229,.25)"></div>' : ''}</div>
    ${showAll ? `<div class="row" style="height:26px;justify-content:center;gap:4px;border-top:1px solid var(--bv);font-size:12px;color:var(--mu)">${showAll}${ic('chev-down', 'xs')}</div>` : ''}</div>`;
}
const miniButton = (icon) => `<span style="width:22px;height:22px;border-radius:5px;display:inline-grid;place-items:center;color:var(--mu)">${ic(icon, 'xs')}</span>`;
const highlighted = (lines, lang = 'ts') => lines.map((line) => hl(line, lang));
const plain = (lines) => lines.map(escHtml);

// The demo world: storefront's cart, as the agent fixes its rounding.
const READ_PATH = 'src/cart/total.ts';
const READ_LINES = [
  "import { roundTotal } from './round'",
  "import type { CartItem } from './types'",
  '',
  'export function cartTotal(items: CartItem[]): number {',
  '  const sum = items.reduce((total, item) => total + roundTotal(item.price * item.quantity), 0)',
  '  return sum',
  '}',
];
// Claude Agent numbers a read's lines itself: "1\t…".
const CLAUDE_READ = READ_LINES.map((line, index) => `${index + 1}\t${escHtml(line)}`);
const READ_JSON = ['{', '  "file_path": "/Users/ana/storefront/src/cart/total.ts",', '  "limit": 120', '}'];
const EDIT_DIFF = [
  [' ', "import type { CartItem } from './types'"],
  [' ', ''],
  [' ', 'export function cartTotal(items: CartItem[]): number {'],
  ['-', '  const sum = items.reduce((total, item) => total + roundTotal(item.price * item.quantity), 0)'],
  ['-', '  return sum'],
  ['+', '  const sum = items.reduce((total, item) => total + item.price * item.quantity, 0)'],
  ['+', '  return roundTotal(sum)'],
  [' ', '}'],
];
const GREP_LINES = [
  "src/cart/total.ts:1:import { roundTotal } from './round'",
  'src/cart/total.ts:5:  const sum = items.reduce((total, item) => total + roundTotal(item.price * item.quantity), 0)',
  'src/cart/round.ts:3:export function roundTotal(value: number): number {',
  'src/cart/round.test.ts:4:  expect(roundTotal(1.005)).toBe(1.01)',
];
const GLOB_LINES = ['src/cart/round.test.ts', 'src/cart/total.test.ts', 'src/checkout/pay.test.ts'];
const TEST_OUTPUT = [
  '> storefront@0.4.0 test',
  '> vitest run src/cart',
  '',
  ' ✓ src/cart/round.test.ts (3 tests) 4ms',
  ' ❯ src/cart/total.test.ts (2 tests | 1 failed) 7ms',
  '   × cartTotal > rounds once, after summing',
  '     → expected 3.02 to be 3.01',
  '',
  ' Test Files  1 failed | 1 passed (2)',
  '      Tests  1 failed | 4 passed (5)',
  '   Duration  412ms',
];
const GIT_DIFF = [
  'diff --git a/src/cart/total.ts b/src/cart/total.ts',
  'index 3f2a9c1..8b41d07 100644',
  '--- a/src/cart/total.ts',
  '+++ b/src/cart/total.ts',
  "@@ -3,6 +3,6 @@ import type { CartItem } from './types'",
  ' ',
  ' export function cartTotal(items: CartItem[]): number {',
  '-  const sum = items.reduce((total, item) => total + roundTotal(item.price * item.quantity), 0)',
  '-  return sum',
  '+  const sum = items.reduce((total, item) => total + item.price * item.quantity, 0)',
  '+  return roundTotal(sum)',
  ' }',
];
const BUILD_LINES = (from, to) => Array.from({ length: to - from + 1 }, (_, index) => `  Compiling page ${from + index}/240`);
const ASK = you('The cart total is off by a cent. Fix it and run the tests.');
