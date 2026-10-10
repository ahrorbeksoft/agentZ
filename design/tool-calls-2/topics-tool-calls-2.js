// Every kind of tool call an agent makes with its own tools, by ACP's kinds, then their states,
// folded runs, the output's file view, ToolSearch, other MCP servers' tools and images. agentZ's
// own tools and subagents are another round's. Built on mock-tool-calls-2.js.

const TS = I('z-ts');
const SECTION_KINDS = 'Agents’ own tools, by kind';
const SECTION_STATES = 'States and runs';
const SECTION_OUTPUT = 'Tool output in a file view';
const SECTION_SERVERS = 'Tools from MCP servers';
const SECTION_IMAGES = 'Images';

// A diff as Zed's editor draws one: the new file's line numbers in a gutter (none on removed
// lines), the change's color across the line, syntax colors, and word changes if given.
function diffView(lines, { numbers = true, color = true, words = {}, start = 2, collapsed = {}, head = '', style = '' } = {}) {
  let newNumber = start;
  const body = lines.map(([kind, text], index) => {
    const number = kind === '-' ? '' : newNumber++;
    const background = kind === '-' ? COLORS.removed : kind === '+' ? COLORS.added : 'transparent';
    const content = words[index] || (color ? hl(text) : escHtml(text));
    const fold = collapsed[index] ? `<div class="row" style="height:22px;gap:6px;padding:0 10px;background:rgba(116,173,232,.07);color:var(--ph);font:12px 'IBM Plex Sans',sans-serif">${ic('chev-down', 'xs')}${collapsed[index]}</div>` : '';
    return `${fold}<div style="display:flex;background:${background};white-space:pre;color:${color || kind !== ' ' ? 'var(--t)' : 'var(--mu)'}">${numbers ? `<span style="width:30px;flex:none;text-align:right;padding-right:6px;color:${COLORS.lineNumber}">${number}</span>` : ''}<span style="width:14px;flex:none;text-align:center;color:var(--mu)">${kind === ' ' ? '' : kind === '-' ? '−' : '+'}</span><span style="padding-right:10px">${content || ' '}</span></div>`;
  }).join('');
  return `<div style="border:1px solid var(--bv);border-radius:6px;background:var(--ed);overflow:hidden;flex:none;${style}">${head}<div style="padding:4px 0;font:12px/18px ${MONO}">${body}</div></div>`;
}
// Word changes in a changed line: the parts that changed on the stronger red or green.
const wordLine = (parts, kind) => parts.map(([text, changed]) => changed
  ? `<span style="background:${kind === '-' ? COLORS.removedWord : COLORS.addedWord};border-radius:2px">${hl(text)}</span>`
  : hl(text)).join('');
const EDIT_WORDS = {
  3: wordLine([['  const sum = items.reduce((total, item) => total + '], ['roundTotal(', true], ['item.price * item.quantity'], [')', true], [', 0)']], '-'),
  4: wordLine([['  return '], ['sum', true]], '-'),
  5: wordLine([['  const sum = items.reduce((total, item) => total + item.price * item.quantity, 0)']], '+'),
  6: wordLine([['  return '], ['roundTotal(sum)', true]], '+'),
};
const viewHead = (icon, title, meta = '', extra = '') => `<div class="row" style="height:28px;gap:6px;padding:0 4px 0 10px;background:${COLORS.head};border-bottom:1px solid var(--bv);font-size:12px;color:var(--mu)">
  ${icon ? `<span style="display:inline-flex;color:${DIM}">${icon}</span>` : ''}<span class="trunc" style="font:12px ${MONO};color:var(--t)">${title}</span>${meta ? `<span class="none" style="color:var(--ph)">${meta}</span>` : ''}<span class="grow"></span>${extra}</div>`;

// The rows, as each agent titles them today.
const READ_ROW = (options = {}) => row(TS, subj('Read src/cart/total.ts'), options);
const EDIT_ROW = (options = {}) => row(TS, verb('Edited') + code('src/cart/total.ts'), { trailing: stat(2, 2), ...options });
const TEST_ROW = (options = {}) => row(I('z-terminal'), verb('Ran') + code('npm test -- src/cart'), options);
const LONG_CALL = 'src/cart/total.test.ts:12:5 AssertionError: expected 3.02 to be 3.01 // Object.is equality, at Proxy.<anonymous> (node_modules/@vitest/expect/dist/index.js:1135:15)';

// 1. Reads ---------------------------------------------------------------------------------
const READ_100 = [...READ_LINES, '', "export function cartCount(items: CartItem[]): number {", '  return items.reduce((count, item) => count + item.quantity, 0)', '}'];

TOPICS.push({
  id: 'read', section: SECTION_KINDS, title: 'Reads', rec: 'A',
  now: 'A read is a row with its file’s icon and the agent’s title, the project’s folder taken out: “Read src/cart/total.ts”. Claude Agent adds the lines it asked for (“Read src/cart/total.ts (1 - 120)”), Codex says “Read file \'total.ts\'”. Opened, it shows the text the agent got, as printed, in a code block: Claude Agent numbers the lines itself (a number and a tab before each), the others send the bare text. “Input” under it opens the JSON the agent passed.',
  nowImg: 'img/now-read.png',
  issues: ['Claude Agent’s line numbers are part of the text: they wrap with it and get copied with it.', 'No colors: a file reads as plain text.', 'Every agent words the row its own way, and the lines asked for are only in Claude Agent’s title.'],
  options: [
    { key: 'A', name: 'The lines in a file view', from: 'Zed (its editor), new',
      desc: 'Opened, a read shows the lines as a file: a header with the file’s icon, its path and how many lines it got, Wrap and Copy at its right; then the lines with their numbers in a gutter (Claude Agent’s own numbers moved there, the read’s first line for the others) and syntax colors by the file’s type.',
      good: 'Looks like the file it is. Copy takes the code without the numbers.', cost: 'Taking Claude Agent’s numbers out of its text means reading its “number, tab” format; another agent’s format would show as text.',
      mock: () => tframe(stack(READ_ROW({ hover: true, open: true }), out(fileView(highlighted(READ_LINES), { icon: TS, title: 'src/cart/total.ts', meta: '7 lines', buttons: true, numbers: 1 }) + inputLine())), 284) },
    { key: 'B', name: 'One wording, the range in the row', from: 'new',
      desc: 'Every agent’s read says “Read” and the file’s name, with its folder and the lines it asked for after it, dimmer: “Read total.ts  src/cart · lines 1–120”. Opened, it shows today’s block.',
      good: 'A run of reads from any agent looks the same, and says what part of the file each got.', cost: 'The agents’ titles have to be parsed, per agent, to find the path and range; a title nothing parses stays as sent.',
      mock: () => tframe(stack(
        row(TS, verb('Read') + `<span class="none">total.ts</span>` + dimTrunc('src/cart · lines 1–120')),
        row(TS, verb('Read') + `<span class="none">round.ts</span>` + dimTrunc('src/cart')),
        row(I('z-json'), verb('Read') + `<span class="none">package.json</span>` + dimTrunc('lines 1–40')),
        `<div style="font-size:11px;color:var(--ph);margin:2px 0 0 32px">From Claude Agent, Codex and Factory Droid, in that order.</div>`), 136, 560) },
    { key: 'C', name: 'A few lines, then Show all', from: 'new',
      desc: 'Opened, a read shows its first 6 lines in A’s file view, fading out, with “Show all 120 lines” under them. Clicking that shows the rest, scrolling past 24 rem as today.',
      good: 'A long read doesn’t fill the thread when you only wanted to see which part it was.', cost: 'Two clicks to see a whole read.',
      mock: () => tframe(stack(READ_ROW({ hover: true, open: true }), out(fileView(highlighted(READ_LINES.slice(0, 6)), { icon: TS, title: 'src/cart/total.ts', meta: '120 lines', buttons: true, numbers: 1, fade: true, showAll: 'Show all 120 lines' }))), 268) },
    { key: 'D', name: 'Reads don’t open', from: 'Zed (its read tool)',
      desc: 'A read is a row only, as in Zed, where reading a file shows its path and nothing more: what it read is the file. The lines it got show dimmer after the path, and there is no chevron.',
      good: 'The quietest: reads are most of a run and rarely worth opening.', cost: 'No way to see what the agent got, which matters when a read was cut short or the file changed since.',
      mock: () => tframe(stack(row(TS, subj('Read src/cart/total.ts') + dim('lines 1–120')), row(TS, subj('Read src/cart/round.ts') + dim('18 lines')), row(I('z-json'), subj('Read package.json') + dim('lines 1–40'))), 110, 560) },
    { key: 'E', name: 'Today’s block, the numbers in a gutter', from: 't3code (its plain output), new',
      desc: 'Opened, a read shows today’s block with no header or colors, but Claude Agent’s numbers are moved into a dim gutter, so they no longer wrap or copy with the text.',
      good: 'The smallest change that fixes the numbers.', cost: 'Still plain text; a file reads like a command’s output.',
      mock: () => tframe(stack(READ_ROW({ hover: true, open: true }), out(fileView(plain(READ_LINES), { numbers: 1 }) + inputLine())), 250) },
  ],
});

// 2. Edits ---------------------------------------------------------------------------------
TOPICS.push({
  id: 'edit', section: SECTION_KINDS, title: 'Edits and their diffs', type: 'multi', rec: 'ABCD',
  now: 'An edit is a row with its file’s icon, “Edited” and the path in the code font, and the lines added and removed at its end (+2 −2). Opened, it shows the diff with 3 lines of context: a line on top, a − or + before each changed line, the removed on red and the added on green, the context dimmer. No line numbers or syntax colors; long lines scroll sideways. An edit of several files says “Edited 3 files” and shows their diffs one after another.',
  nowImg: 'img/now-edit.png',
  issues: ['Without numbers you can’t tell where in the file the change is.', 'A changed line shows whole; the part that changed has to be found by eye.', 'Several files’ diffs run together with nothing naming each file.'],
  options: [
    { key: 'A', name: 'Line numbers', from: 'Zed (its editor’s diff)',
      desc: 'The new file’s line numbers in a gutter left of the − and +, as Zed’s editor shows a diff. Removed lines have none.',
      good: 'Says where the change is.', cost: 'Some 30 px less for the code.',
      mock: () => tframe(stack(EDIT_ROW({ hover: true, open: true }), out(diffView(EDIT_DIFF, { color: false }))), 230) },
    { key: 'B', name: 'Syntax colors', from: 'Zed (its editor’s diff)',
      desc: 'The diff’s lines in the theme’s syntax colors by the file’s type, over the red and green, as Zed draws them. Context lines keep the text color.',
      good: 'Code reads as code.', cost: 'Needs the language’s highlighting where the diff is drawn.',
      mock: () => tframe(stack(EDIT_ROW({ hover: true, open: true }), out(diffView(EDIT_DIFF, { numbers: false }))), 230) },
    { key: 'C', name: 'Word changes marked', from: 'Zed (its word diff)',
      desc: 'In a changed line, the words that changed get a stronger red or green, as Zed marks them.',
      good: 'The change is seen at once, even in long lines.', cost: 'A word diff for each changed pair of lines.',
      mock: () => tframe(stack(EDIT_ROW({ hover: true, open: true }), out(diffView(EDIT_DIFF, { words: EDIT_WORDS }))), 230) },
    { key: 'D', name: 'A header per file, with Open', from: 'Zed (its edit card), agentZ’s diff panel',
      desc: 'Each file’s diff gets a header: its icon, its path, +2 −2, and “Open”, which shows the file in agentZ’s diff panel. An edit of several files is then one header per file.',
      good: 'Several files no longer run together, and the whole change is one click away.', cost: 'A header even on a one-file edit, where the row already names it.',
      mock: () => tframe(stack(row(TS, subj('Edited 2 files'), { trailing: stat(5, 2), hover: true, open: true }), out(
        diffView(EDIT_DIFF.slice(2, 7), { start: 4, head: viewHead(TS, 'src/cart/total.ts', '', stat(2, 2) + openButton()) }) +
        diffView([[' ', "  expect(cartTotal([item(1.005), item(1.005)])).toBe(2.01)"], ['+', '  expect(cartTotal([item(1.005), item(1.005), item(1.005)])).toBe(3.02)'], [' ', '})']], { start: 11, head: viewHead(TS, 'src/cart/total.test.ts', '', stat(3, 0) + openButton()) }))), 318) },
    { key: 'E', name: 'Unchanged lines folded', from: 'Zed (its multibuffer’s expand)',
      desc: 'Between two parts of one diff, a thin line says how many unchanged lines are left out (“14 unchanged lines”); clicking it shows them.',
      good: 'A file with changes far apart reads as one file, not loose pieces.', cost: 'Needs the file’s text, not only the agent’s old and new text.',
      mock: () => tframe(stack(EDIT_ROW({ hover: true, open: true }), out(diffView([...EDIT_DIFF, [' ', 'export function cartCount(items: CartItem[]): number {'], ['-', '  return items.length'], ['+', '  return items.reduce((count, item) => count + item.quantity, 0)']], { collapsed: { 8: '14 unchanged lines' } }))), 290) },
  ],
});

// 3. Created, deleted and moved files ----------------------------------------------------
const NEW_FILE = ['export function roundTotal(value: number): number {', '  return Math.round((value + Number.EPSILON) * 100) / 100', '}'];
TOPICS.push({
  id: 'files', section: SECTION_KINDS, title: 'Created, deleted and moved files', rec: 'A',
  now: 'A new file shows as an edit: “Edited src/cart/round.ts” (+3 −0), opening to a diff where every line is added. A delete has Zed’s file-with-a-cross icon and the agent’s title (“Delete src/cart/legacy.ts”); a move has two arrows and the agent’s title (“Move src/cart/util.ts to src/cart/round.ts”). Neither opens to anything but the agent’s text.',
  nowImg: 'img/now-files.png',
  issues: ['A new file reads as an edit.', 'A delete or a move says it in each agent’s words, and in the present tense once done.'],
  options: [
    { key: 'A', name: 'Says what happened, a new file opens as a file', from: 'new',
      desc: '“Created src/cart/round.ts” (+3), “Deleted src/cart/legacy.ts” (−42) and “Moved src/cart/util.ts → src/cart/round.ts”. A created file opens to its lines in a file view with numbers and colors, not a diff; a deleted one to its lines, dimmer.',
      good: 'Each row says what it did, the same for every agent.', cost: 'Tells a create from an edit by the diff having no old text, which every agent sends that way but none promises.',
      mock: () => tframe(stack(
        row(TS, verb('Created') + code('src/cart/round.ts'), { trailing: `<span class="none" style="font-size:12px;color:${COLORS.created};margin-right:4px">+3</span>`, hover: true, open: true }),
        out(fileView(highlighted(NEW_FILE), { numbers: 1 })),
        row(I('z-delete'), verb('Deleted') + code('src/cart/legacy.ts'), { trailing: `<span class="none" style="font-size:12px;color:${COLORS.deleted};margin-right:20px">−42</span>` }),
        row(I('z-arrows'), verb('Moved') + codeNone('src/cart/util.ts') + `<span class="none">→</span>` + code('src/cart/round.ts'))), 200) },
    { key: 'B', name: 'Today’s rows, a new file opens as a file', from: 'new',
      desc: 'Rows keep today’s words. Only a new file opens differently: its lines in a file view, not a diff of added lines.',
      good: 'A new file reads as one.', cost: 'Deletes and moves keep each agent’s wording.',
      mock: () => tframe(stack(
        row(TS, verb('Edited') + code('src/cart/round.ts'), { trailing: stat(3, 0), hover: true, open: true }),
        out(fileView(highlighted(NEW_FILE), { numbers: 1 })),
        row(I('z-delete'), subj('Delete src/cart/legacy.ts')),
        row(I('z-arrows'), subj('Move src/cart/util.ts to src/cart/round.ts'))), 200) },
    { key: 'C', name: 'Zed: a new file’s diff, all added', from: 'Zed (its edit card)',
      desc: 'As Zed shows a new file: the diff of added lines, with A’s words in the rows (“Created”, “Deleted”, “Moved”).',
      good: 'One way to show every file change.', cost: 'A wall of green for a new file.',
      mock: () => tframe(stack(
        row(TS, verb('Created') + code('src/cart/round.ts'), { trailing: stat(3, 0), hover: true, open: true }),
        out(diffView(NEW_FILE.map((line) => ['+', line]), { start: 1 })),
        row(I('z-delete'), verb('Deleted') + code('src/cart/legacy.ts'), { trailing: stat(0, 42) }),
        row(I('z-arrows'), verb('Moved') + codeNone('src/cart/util.ts') + `<span class="none">→</span>` + code('src/cart/round.ts'))), 200) },
    { key: 'D', name: 'Deletes and moves in their own color', from: 'new',
      desc: 'A’s rows, with a deleted file’s path struck through in red and a moved file’s new path in green, so they stand out in a run of edits.',
      good: 'A deleted file is hard to miss.', cost: 'Colors in rows that are otherwise one gray.',
      mock: () => tframe(stack(
        row(TS, verb('Created') + code('src/cart/round.ts'), { trailing: `<span class="none" style="font-size:12px;color:${COLORS.created};margin-right:20px">+3</span>` }),
        row(I('z-delete'), verb('Deleted') + `<span class="trunc" style="font:12px ${MONO};color:${COLORS.deleted};text-decoration:line-through">src/cart/legacy.ts</span>`),
        row(I('z-arrows'), verb('Moved') + codeNone('src/cart/util.ts') + `<span class="none">→</span>` + `<span class="trunc" style="font:12px ${MONO};color:${COLORS.created}">src/cart/round.ts</span>`)), 110, 560) },
  ],
});

// 4. Searches ------------------------------------------------------------------------------
const GREP_BY_FILE = [
  ['src/cart/total.ts', [[1, "import { roundTotal } from './round'"], [5, '  const sum = items.reduce((total, item) => total + roundTotal(item.price * item.quantity), 0)']]],
  ['src/cart/round.ts', [[3, 'export function roundTotal(value: number): number {']]],
  ['src/cart/round.test.ts', [[4, '  expect(roundTotal(1.005)).toBe(1.01)']]],
];
const markWord = (html, word) => html.replace(new RegExp(word, 'g'), `<span style="background:rgba(222,193,132,.28);border-radius:2px">${word}</span>`);
function matchesByFile({ numbers = true } = {}) {
  return `<div style="border:1px solid var(--bv);border-radius:6px;background:var(--ed);overflow:hidden;flex:none;font:12px/17px ${MONO}">${GREP_BY_FILE.map(([path, lines], index) => `
    <div class="row" style="height:24px;gap:6px;padding:0 10px;${index ? 'border-top:1px solid var(--bv);' : ''}color:var(--t)"><span style="display:inline-flex;color:${DIM}">${ic('z-ts', 'xs')}</span>${path}<span style="color:var(--ph);font:12px 'IBM Plex Sans',sans-serif">${lines.length}</span></div>
    ${lines.map(([line, text]) => `<div style="display:flex;padding:0 10px;white-space:pre">${numbers ? `<span style="width:22px;flex:none;text-align:right;margin-right:12px;color:${COLORS.lineNumber}">${line}</span>` : ''}<span class="trunc">${markWord(hl(text), 'roundTotal')}</span></div>`).join('')}`).join('')}<div style="height:4px"></div></div>`;
}
TOPICS.push({
  id: 'search', section: SECTION_KINDS, title: 'Searches', rec: 'A',
  now: 'A search is a row with the magnifying glass and the agent’s title. Claude Agent titles a grep with its command line (<code>grep -n "roundTotal" src</code>) and a glob as “Find `src` `**/*.test.ts`”; Codex says “Search for \'roundTotal\' in src” and “List files in \'src\'”; Factory Droid “Grep roundTotal in src”. Opened, it shows what the tool printed: lines of <code>path:line:text</code>, or the paths found.',
  nowImg: 'img/now-search.png',
  issues: ['Each agent words a search differently, some as commands with backticks.', 'How many matches there were shows only when opened.', 'Matches are a run of paths and numbers to read through.'],
  options: [
    { key: 'A', name: 'One wording, matches by file', from: 'Zed (its grep tool’s output)',
      desc: '“Searched for roundTotal in src”, with “4 matches in 3 files” dimmer at its end. Opened, the matches under each file’s name and icon, with their line numbers and the word marked, as Zed’s grep tool shows them. A glob says “Found 3 files for **/*.test.ts” and opens to the paths.',
      good: 'Reads the same from every agent, and the matches are easy to scan.', cost: 'Every agent’s output format has to be parsed; one nothing parses shows as printed.',
      mock: () => tframe(stack(row(I('z-search'), verb('Searched for') + codeNone('roundTotal') + verb('in') + subj('src'), { trailing: dim('4 matches in 3 files', 'margin-right:4px'), hover: true, open: true }), out(matchesByFile()),
        row(I('z-search'), verb('Found 3 files for') + code('**/*.test.ts'))), 270) },
    { key: 'B', name: 'One wording, output as printed', from: 'new',
      desc: 'The same row as A. Opened, the tool’s output as it printed it, in the file view, with the matched word marked.',
      good: 'Nothing is parsed out of the output, so nothing can be lost.', cost: 'Still lines of paths and numbers.',
      mock: () => tframe(stack(row(I('z-search'), verb('Searched for') + codeNone('roundTotal') + verb('in') + subj('src'), { trailing: dim('4 matches', 'margin-right:4px'), hover: true, open: true }), out(fileView(GREP_LINES.map((line) => markWord(escHtml(line), 'roundTotal')), { scroll: true }))), 180) },
    { key: 'C', name: 'Found files as rows', from: 'new',
      desc: 'A’s row. Opened, each file that matched is a row of its own, with its icon, path and number of matches; clicking one shows its matches under it.',
      good: 'A big search stays short: one line a file.', cost: 'A click more to see the lines.',
      mock: () => tframe(stack(row(I('z-search'), verb('Searched for') + codeNone('roundTotal') + verb('in') + subj('src'), { trailing: dim('4 matches in 3 files', 'margin-right:4px'), hover: true, open: true }),
        out(GREP_BY_FILE.map(([path, lines], index) => row(TS, subj(path) + dim(String(lines.length)), { open: index === 0, hover: index === 0 })).join('') + `<div style="margin-left:30px">${fileView(GREP_BY_FILE[0][1].map(([, text]) => markWord(hl(text), 'roundTotal')), { numbers: null, scroll: true })}</div>`, 'gap:0')), 210) },
    { key: 'D', name: 'As titled, with a count', from: 'agentZ’s ToolSearch rows (“N found”)',
      desc: 'Rows keep each agent’s title, and add how many it found at their end, as ToolSearch’s rows do (“4 found”). Opened, as today.',
      good: 'Small, and the count is what you usually want.', cost: 'The agents’ wordings stay mixed.',
      mock: () => tframe(stack(
        row(I('z-search'), code('grep -n "roundTotal" src'), { trailing: dim('4 found', 'margin-right:20px') }),
        row(I('z-search'), subj('Search for \'roundTotal\' in src'), { trailing: dim('4 found', 'margin-right:20px') }),
        row(I('z-search'), subj('Find `src` `**/*.test.ts`'), { trailing: dim('3 found', 'margin-right:20px') })), 110, 560) },
  ],
});

// 5. Commands ------------------------------------------------------------------------------
const zedCard = (lines, { failed = false, running = false } = {}) => `<div style="border:1px ${failed ? 'dashed' : 'solid'} ${failed ? 'rgba(208,114,119,.6)' : 'var(--b)'};border-radius:6px;overflow:hidden;flex:none">
  <div class="row" style="height:30px;gap:6px;padding:0 6px 0 8px;background:${COLORS.head};border-bottom:1px solid var(--bv);font-size:12px;color:var(--mu)">
    <span style="display:inline-flex;color:${DIM}">${ic('z-terminal', 'xs')}</span><span class="trunc" style="font:12px ${MONO};color:var(--t)">npm test -- src/cart</span><span class="none" style="color:var(--ph)">~/storefront</span><span class="grow"></span>
    <span class="none" style="color:var(--ph)">${running ? '3s' : '0.4s'}</span>${failed ? `<span class="none" style="color:${COLORS.error}">Exit code 1</span>` : ''}${running ? `<span class="none row" style="gap:3px;color:var(--mu);margin-left:4px">${ic('z-stop', 'xs')}Stop</span>` : ''}${miniButton('z-copy')}</div>
  ${termBlock(lines.map(escHtml), { h: Math.min(lines.length, 11) * 17 + 8 }).replace('border-radius:6px', 'border-radius:0')}</div>`;
TOPICS.push({
  id: 'command', section: SECTION_KINDS, title: 'Commands', rec: 'B',
  now: 'A command is a row with the terminal icon, “Ran” and the command in the code font (“Running” and a spinner while it runs, “Failed” in red at its end if it failed). Opened, a command the agent ran in agentZ’s terminal shows that terminal, live, up to 24 rem; one it ran itself shows its output as printed. Nothing says how long it took or how it ended, other than “Failed”.',
  nowImg: 'img/now-command.png',
  issues: ['A failed command says “Failed” but not its exit code; a test run with failures looks like a broken command.', 'Nothing says where it ran or how long it took.'],
  options: [
    { key: 'A', name: 'Zed’s terminal card', from: 'Zed (its terminal card)',
      desc: 'Opened, a command is a card: a header with the command, the folder it ran in, how long it took, its exit code if it failed and Copy (Stop while it runs), over its output in the terminal’s colors. A failed one gets a dashed red border, as in Zed.',
      good: 'Everything about a command in one place, as Zed shows it.', cost: 'The card repeats the command already in the row.',
      mock: () => tframe(stack(TEST_ROW({ trailing: failedLabel(), hover: true, open: true }), out(zedCard(TEST_OUTPUT, { failed: true }))), 290) },
    { key: 'B', name: 'Time and exit code in the row', from: 'Zed (its elapsed time and exit code), t3code',
      desc: 'The row ends in how long the command took (“0.4 s”, dim) and, if it failed, “Exit 1” in red in place of “Failed”. Opened, its output as today.',
      good: 'How a command ended shows without opening it, also in a run of many.', cost: 'The time shows only for commands whose start and end the agent reports.',
      mock: () => tframe(stack(
        row(I('z-terminal'), verb('Ran') + code('npm run lint'), { trailing: dim('1.2 s', 'margin-right:20px') }),
        TEST_ROW({ trailing: dim('0.4 s', 'margin-right:6px') + failedLabel('Exit 1'), hover: true, open: true }), out(termBlock(TEST_OUTPUT.map(escHtml)))), 280) },
    { key: 'C', name: 'A failure opens by itself', from: 'new',
      desc: 'B’s row. A command that failed opens on its own to its last 8 lines, where the error usually is, with “Show all 11 lines” above them.',
      good: 'The error is in view without a click.', cost: 'A run with several failures gets long; a failing test the agent expected opens too.',
      mock: () => tframe(stack(row(I('z-terminal'), verb('Ran') + code('npm run lint'), { trailing: dim('1.2 s', 'margin-right:20px') }), TEST_ROW({ trailing: dim('0.4 s', 'margin-right:6px') + failedLabel('Exit 1'), open: true }),
        out(`<div class="row" style="height:20px;gap:4px;font-size:12px;color:var(--mu)">${ic('chev-up', 'xs')}Show all 11 lines</div>` + termBlock(TEST_OUTPUT.slice(3).map(escHtml)))), 250) },
    { key: 'D', name: 'Output as the terminal printed it', from: 'herdr (its terminal panes)',
      desc: 'Opened, the output starts with the prompt and command (“$ npm test -- src/cart”) and ends with the exit code, in the terminal’s colors, as if you’d run it in a terminal pane.',
      good: 'Reads as a terminal session; copies as one.', cost: 'Adds two lines to every command’s output.',
      mock: () => tframe(stack(TEST_ROW({ trailing: failedLabel(), hover: true, open: true }), out(termBlock([`<span style="color:#a1c181">~/storefront</span> $ npm test -- src/cart`, ...TEST_OUTPUT.map(escHtml).map((line) => line.replace('✓', '<span style="color:#a1c181">✓</span>').replace('×', `<span style="color:${COLORS.error}">×</span>`)), `<span style="color:${COLORS.error}">exit 1</span>`]))), 290) },
  ],
});

// 6. Thinking tools ------------------------------------------------------------------------
const TODOS = [['done', 'Find where the cart rounds'], ['done', 'Round once, after summing'], ['now', 'Run the cart tests']];
const todoList = () => TODOS.map(([state, text]) => `<div class="row" style="gap:8px;height:22px;font-size:13px;color:${state === 'done' ? 'var(--ph)' : 'var(--t)'}">
  <span style="width:14px;height:14px;border-radius:4px;border:1px solid ${state === 'done' ? 'var(--ac)' : 'var(--b)'};background:${state === 'done' ? 'var(--ac)' : 'transparent'};display:inline-grid;place-items:center;color:var(--ed)">${state === 'done' ? ic('z-check', 'xs') : ''}</span>
  <span style="${state === 'done' ? 'text-decoration:line-through' : ''}">${text}</span>${state === 'now' ? '<span style="font-size:11px;color:var(--ac)">in progress</span>' : ''}</div>`).join('');
const TODO_JSON = ['{', '  "todos": [', '    { "content": "Find where the cart rounds", "status": "completed" },', '    { "content": "Round once, after summing", "status": "completed" },', '    { "content": "Run the cart tests", "status": "in_progress" }', '  ]', '}'];
const compactLine = () => `<div class="row" style="gap:10px;height:26px;margin:4px 0;font-size:12px;color:var(--ph)"><span style="flex:1;height:1px;background:var(--bv)"></span>Conversation compacted<span style="flex:1;height:1px;background:var(--bv)"></span></div>`;
TOPICS.push({
  id: 'think', section: SECTION_KINDS, title: 'Plans, to-dos and compaction', rec: 'C',
  now: 'Tools of the “think” kind get the light-bulb icon and the agent’s title. Claude Agent’s to-do list is “Update TODOs: Find where the cart rounds, Round once, after summing, Run the cart tests”, its task tools “Create task: …”, and both Claude Agent and Codex show “Compact conversation” when they compact. Opened, they show their JSON. The plan they make also shows above the composer, in the plan bar.',
  nowImg: 'img/now-think.png',
  issues: ['A to-do update’s title is every item joined with commas, cut off.', 'Opened, it is JSON.', 'A compaction looks like any other step, though everything before it is now summarized.'],
  options: [
    { key: 'A', name: 'The to-dos as a checklist', from: 'Zed (its plan), t3code (its plan card)',
      desc: '“Updated the to-dos”, with “2 of 3 done” dimmer. Opened, the list with checkboxes, as the plan bar draws it: done items struck through, the one in progress marked.',
      good: 'Says what changed in words, and shows the list as a list.', cost: 'Claude Agent’s task tools (“Create task”) still show one item at a time.',
      mock: () => tframe(stack(row(I('z-think'), verb('Updated the to-dos') + dim('2 of 3 done'), { hover: true, open: true }), out(todoList())), 140, 560) },
    { key: 'B', name: 'Not in the thread', from: 'Zed (its plan entries)',
      desc: 'To-do updates and task tools don’t show as rows: the plan bar above the composer already shows the list, as Zed shows a plan only there.',
      good: 'Runs lose a row that repeats the plan bar.', cost: 'Nothing in the thread says when the plan changed.',
      mock: () => tframe(stack(TEST_ROW(), `<div style="margin-left:32px;font-size:11px;color:var(--ph)">(“Updated the to-dos” was here; the plan bar shows the list.)</div>`, READ_ROW()), 96, 560) },
    { key: 'C', name: 'A’s checklist, and compaction as a divider', from: 'Zed (its plan), new',
      desc: 'A for to-dos. A compaction is a line across the thread, “Conversation compacted”, in place of a row, since everything above it is now a summary for the agent.',
      good: 'Marks the point where the agent stopped seeing the earlier turns as they were.', cost: 'A divider is something new in the thread.',
      mock: () => tframe(stack(TEST_ROW(), compactLine(), row(I('z-think'), verb('Updated the to-dos') + dim('2 of 3 done'))), 110, 560) },
    { key: 'D', name: 'Short titles, JSON highlighted', from: 'new',
      desc: '“Updated the to-dos (3)” in place of the joined items. Opened, the JSON as today, but highlighted in the file view.',
      good: 'Small change; nothing hidden.', cost: 'Still JSON.',
      mock: () => tframe(stack(row(I('z-think'), verb('Updated the to-dos') + dim('3'), { hover: true, open: true }), out(fileView(highlighted(TODO_JSON, 'json'), { scroll: true }))), 200, 560) },
  ],
});

// 7. Fetches and web searches --------------------------------------------------------------
const FETCH_MD = ['## toBeCloseTo', '', '`toBeCloseTo` compares floating-point numbers. Use it in place of **toBe** when a sum may be', 'off in the last digits.', '', '- `numDigits` sets how many digits after the point are checked; it defaults to **2**.'];
const SEARCH_OUT = ['Web search results for query: "vitest toBeCloseTo"', '', 'Links: [{"title":"expect | Vitest","url":"https://vitest.dev/api/expect"},{"title":"Floating point', 'comparisons in tests","url":"https://jestjs.io/docs/expect#tobeclosetonumber-numdigits"}]'];
const linkRow = (title, url) => `<div class="row" style="gap:8px;height:22px;font-size:13px"><span style="display:inline-flex;color:${DIM}">${ic('z-web', 'xs')}</span><span class="none" style="color:var(--t)">${title}</span><span class="trunc" style="font-size:12px;color:var(--ac)">${url}</span></div>`;
const fetchMarkdown = () => `<div style="font-size:13px;line-height:20px;color:var(--t);border-left:1px solid var(--b);padding:2px 0 2px 12px"><div style="font-weight:600;font-size:14px;margin-bottom:4px">toBeCloseTo</div>${inlineCode('toBeCloseTo')} compares floating-point numbers. Use it in place of <b>toBe</b> when a sum may be off in the last digits.<ul style="margin:4px 0 0;padding-left:18px"><li>${inlineCode('numDigits')} sets how many digits after the point are checked; it defaults to <b>2</b>.</li></ul></div>`;
const host = (text) => `<span class="trunc" style="color:var(--ac)">${text}</span>`;
TOPICS.push({
  id: 'fetch', section: SECTION_KINDS, title: 'Fetched pages and web searches', rec: 'A',
  now: 'A fetch or web search is a row with the globe and the agent’s title: Claude Agent’s “Fetch https://vitest.dev/api/expect” and “Search \"vitest toBeCloseTo\"”, Codex’s “Web search: vitest toBeCloseTo”. Opened, it shows what came back as printed: a fetch gives the agent’s notes on the page in markdown, so its <code>##</code> and <code>**</code> show; a search gives Claude Agent’s results as JSON.',
  nowImg: 'img/now-fetch.png',
  issues: ['A page’s notes are prose written in markdown, shown as code.', 'Search results are a JSON line with the links in it, not links you can click.'],
  options: [
    { key: 'A', name: 'Pages as text, searches as links', from: 't3code (its web tools), new',
      desc: '“Fetched vitest.dev/api/expect”, the address a link that opens in the browser. Opened, the notes drawn as markdown, set off by a line on their left, since they are prose and not printed output. “Searched the web for “vitest toBeCloseTo”” opens to the results as links, a title and an address each.',
      good: 'Each reads as what it is.', cost: 'Two exceptions to “output as printed”, and searches need each agent’s result format read.',
      mock: () => tframe(stack(row(I('z-web'), verb('Fetched') + host('vitest.dev/api/expect'), { hover: true, open: true }), out(fetchMarkdown()),
        row(I('z-web'), verb('Searched the web for') + subj('“vitest toBeCloseTo”'), { open: true }), out(linkRow('expect | Vitest', 'vitest.dev/api/expect') + linkRow('Floating point comparisons in tests', 'jestjs.io/docs/expect'), 'gap:0')), 300) },
    { key: 'B', name: 'As printed, like every output', from: 'new',
      desc: 'A’s rows. Opened, what came back as printed, in the file view, as every other tool’s output.',
      good: 'One rule for every tool.', cost: 'Markdown signs and JSON stay in view.',
      mock: () => tframe(stack(row(I('z-web'), verb('Fetched') + host('vitest.dev/api/expect'), { hover: true, open: true }), out(fileView(plain(FETCH_MD))),
        row(I('z-web'), verb('Searched the web for') + subj('“vitest toBeCloseTo”'))), 270) },
    { key: 'C', name: 'Links only', from: 'Zed (its fetch tool)',
      desc: 'A fetch is its address as a link, and doesn’t open: what the agent made of the page is in its reply. A search opens to its results as links, as in A.',
      good: 'The quietest; the page itself is a click away.', cost: 'The agent’s notes on the page are hidden.',
      mock: () => tframe(stack(row(I('z-web'), verb('Fetched') + host('vitest.dev/api/expect')), row(I('z-web'), verb('Searched the web for') + subj('“vitest toBeCloseTo”'), { hover: true, open: true }), out(linkRow('expect | Vitest', 'vitest.dev/api/expect') + linkRow('Floating point comparisons in tests', 'jestjs.io/docs/expect'), 'gap:0')), 150, 600) },
    { key: 'D', name: 'Today’s, worded the same', from: 'new',
      desc: 'Only the rows change, to A’s words for every agent; opened, as today.',
      good: 'Small.', cost: 'Leaves the markdown and JSON as they are.',
      mock: () => tframe(stack(row(I('z-web'), verb('Fetched') + subj('vitest.dev/api/expect')), row(I('z-web'), verb('Searched the web for') + subj('“vitest toBeCloseTo”'), { hover: true, open: true }), out(printed(SEARCH_OUT.map(escHtml)))), 190, 600) },
  ],
});

// 8. Switching modes -----------------------------------------------------------------------
const PLAN_MD = () => `<div style="font-size:13px;line-height:20px;color:var(--t)"><div style="font-weight:600;font-size:14px;margin-bottom:4px">Round once, after summing</div><ol style="margin:0;padding-left:20px"><li>In ${inlineCode('cartTotal')}, sum the raw prices, then call ${inlineCode('roundTotal')} once.</li><li>Add a test for three items of 1.005.</li><li>Run ${inlineCode('npm test -- src/cart')}.</li></ol></div>`;
const PLAN_TEXT = ['## Round once, after summing', '', '1. In `cartTotal`, sum the raw prices, then call `roundTotal` once.', '2. Add a test for three items of 1.005.', '3. Run `npm test -- src/cart`.'];
const choice = (text, primary = false) => `<span class="row" style="height:24px;padding:0 9px;border-radius:5px;font-size:12px;${primary ? 'background:#3d4a5c;color:var(--t)' : 'border:1px solid var(--b);color:var(--mu)'}">${text}</span>`;
const PLAN_CHOICES = `<div class="row" style="gap:6px;flex-wrap:wrap">${choice('Yes, and auto-accept edits', true)}${choice('Yes, and manually approve edits')}${choice('No, keep planning')}</div>`;
const modeLine = () => `<div class="row" style="gap:10px;height:26px;margin:4px 0;font-size:12px;color:var(--ph)"><span style="flex:1;height:1px;background:var(--bv)"></span><span class="row" style="gap:5px">${ic('z-arrows', 'xs')}Plan → Default</span><span style="flex:1;height:1px;background:var(--bv)"></span></div>`;
TOPICS.push({
  id: 'mode', section: SECTION_KINDS, title: 'Leaving plan mode', rec: 'A',
  now: 'When an agent asks to leave plan mode, its row has two arrows and its title: Claude Agent’s “Approve Plan” (then “Exited Plan Mode”), Codex’s “Implement this plan?”, Factory Droid’s “Approve Spec”. The permission card under it offers the agent’s choices. The plan is the tool’s text, so opened it shows as printed, with its markdown signs.',
  nowImg: 'img/now-mode.png',
  issues: ['The plan you’re asked to approve is shown as code, not as the document it is.', 'Once approved, nothing marks that the agent left plan mode.'],
  options: [
    { key: 'A', name: 'The plan as a card, then the switch', from: 'Zed (its permission card), new',
      desc: 'The plan opens by itself, drawn as markdown in a card with “Plan” at its top, and the agent’s choices under it. Once answered, it folds to “Approved the plan” and a line “Plan → Default” marks the switch.',
      good: 'The plan reads as a document right where you decide on it.', cost: 'The plan is a long card in the thread while it waits.',
      mock: () => tframe(stack(row(I('z-arrows'), subj('Approve Plan'), { open: true }), out(`<div style="border:1px solid var(--b);border-radius:6px;background:var(--ed);overflow:hidden"><div style="height:26px;display:flex;align-items:center;padding:0 10px;font-size:12px;color:var(--ph);border-bottom:1px solid var(--bv)">Plan</div><div style="padding:10px 12px">${PLAN_MD()}</div></div>` + PLAN_CHOICES)), 250) },
    { key: 'B', name: 'A line across the thread', from: 'new',
      desc: 'Today’s card while it waits. Once answered, the row goes and a line “Plan → Default” marks the switch; clicking it shows the plan.',
      good: 'The switch is easy to find later.', cost: 'The plan shows as today while you decide.',
      mock: () => tframe(stack(TEST_ROW(), modeLine(), EDIT_ROW()), 110, 560) },
    { key: 'C', name: 'Today’s row, the plan as markdown', from: 'new',
      desc: 'Rows as today. Opened, the plan is drawn as markdown, the one exception to output as printed.',
      good: 'The plan reads well; nothing else changes.', cost: 'Nothing marks the switch.',
      mock: () => tframe(stack(row(I('z-arrows'), subj('Exited Plan Mode'), { hover: true, open: true }), out(`<div style="border-left:1px solid var(--b);padding:2px 0 2px 12px">${PLAN_MD()}</div>`)), 150, 600) },
    { key: 'D', name: 'As printed', from: 'new',
      desc: 'The plan in the file view as printed, with its markdown signs, as every output.',
      good: 'One rule for every tool.', cost: 'A plan is prose; read as code it’s harder to judge.',
      mock: () => tframe(stack(row(I('z-arrows'), subj('Approve Plan'), { open: true }), out(fileView(plain(PLAN_TEXT)) + PLAN_CHOICES)), 210, 600) },
  ],
});

// 9. Other tools ---------------------------------------------------------------------------
const SKILL_JSON = ['{', '  "skill": "review"', '}'];
const FINDINGS = ['**src/cart/total.ts:5** — rounds each item, so three items of 1.005 sum to 3.02.', '**src/cart/round.ts:3** — `roundTotal` adds `Number.EPSILON`; keep it.'];
TOPICS.push({
  id: 'other', section: SECTION_KINDS, title: 'Other tools', rec: 'B',
  now: 'A tool of the “other” kind (or none) gets the hammer and the agent’s title: “Load skill: review”, “Report 2 findings”, or the tool’s bare name (“NotebookEdit”) when the agent has nothing better. Opened, it shows its output as printed and “Input” for the JSON.',
  nowImg: 'img/now-other.png',
  issues: ['Tools the agents give a title read fine, but the hammer says nothing about them.', 'Some send prose in markdown (Claude Agent’s findings), shown with their signs.'],
  options: [
    { key: 'A', name: 'As titled, the input highlighted', from: 'new',
      desc: 'Rows as today. Opened, the output as printed and the input JSON, both in the file view, the JSON in colors.',
      good: 'Nothing guessed about tools agentZ doesn’t know.', cost: 'Still a hammer and the agent’s words.',
      mock: () => tframe(stack(row(I('z-hammer'), subj('Load skill: review'), { hover: true, open: true }), out(fileView(['Loaded skill review from ~/.claude/skills/review/SKILL.md']) + inputLine(true) + fileView(highlighted(SKILL_JSON, 'json')))), 200, 600) },
    { key: 'B', name: 'Known tools in words, with their icons', from: 't3code (its labels for each tool)',
      desc: 'Tools many agents share get a sentence and an icon: a skill is “Loaded the review skill” with a book, findings are “Reported 2 findings” opening to them drawn as markdown, a question to you is “Asked you” with a speech bubble. Tools nothing knows keep the hammer and their title.',
      good: 'Most rows say what happened.', cost: 'A list of tool names per agent to keep current.',
      mock: () => tframe(stack(row(I('z-book'), verb('Loaded the') + `<span class="none" style="color:var(--mu)">review</span>` + verb('skill')), row(I('z-list'), verb('Reported 2 findings'), { hover: true, open: true }),
        out(`<div style="font-size:13px;line-height:20px;color:var(--t);border-left:1px solid var(--b);padding:2px 0 2px 12px"><div><b>src/cart/total.ts:5</b> — rounds each item, so three items of 1.005 sum to 3.02.</div><div><b>src/cart/round.ts:3</b> — ${inlineCode('roundTotal')} adds ${inlineCode('Number.EPSILON')}; keep it.</div></div>`),
        row(I('z-chat'), verb('Asked you') + subj('Which rounding should the cart use?')), row(I('z-hammer'), subj('NotebookEdit'))), 210, 600) },
    { key: 'C', name: 'The tool’s name, then its input', from: 'new',
      desc: 'Every other tool says its name in words, then its input’s main values dimmer: “Skill  review”, “NotebookEdit  notebooks/cart.ipynb · cell 3”.',
      good: 'No list to keep; works for any tool.', cost: 'Reads as data, not as what happened.',
      mock: () => tframe(stack(row(I('z-hammer'), subj('Skill') + dimTrunc('review')), row(I('z-hammer'), subj('Report findings') + dimTrunc('2 findings')), row(I('z-hammer'), subj('NotebookEdit') + dimTrunc('notebooks/cart.ipynb · cell 3'))), 110, 600) },
    { key: 'D', name: 'Markdown when it’s prose', from: 'new',
      desc: 'Rows as today. Output that is mostly sentences (findings, a skill’s text) is drawn as markdown; output that looks printed (paths, JSON, columns) stays as printed.',
      good: 'Prose reads as prose without a list of tools.', cost: 'A guess, which will sometimes guess wrong, which is how the markdown problem started.',
      mock: () => tframe(stack(row(I('z-hammer'), subj('Report 2 findings'), { hover: true, open: true }), out(`<div style="font-size:13px;line-height:20px;color:var(--t);border-left:1px solid var(--b);padding:2px 0 2px 12px"><div><b>src/cart/total.ts:5</b> — rounds each item, so three items of 1.005 sum to 3.02.</div><div><b>src/cart/round.ts:3</b> — ${inlineCode('roundTotal')} adds ${inlineCode('Number.EPSILON')}; keep it.</div></div>`)), 120, 600) },
  ],
});

// 10. Failed, cancelled and denied --------------------------------------------------------
const alertIcon = () => `<span style="display:inline-flex;color:${COLORS.error}">${I('z-warn')}</span>`;
const errorLine = (text) => `<div style="margin:-2px 0 2px 32px;font:12px/17px ${MONO};color:rgba(208,114,119,.85);white-space:nowrap;overflow:hidden;text-overflow:ellipsis">${text}</div>`;
TOPICS.push({
  id: 'failed', section: SECTION_STATES, title: 'Failed, cancelled and denied', rec: 'C',
  now: 'A tool call that failed keeps its icon and title and ends in “Failed” in red; opened, it shows what the agent sent back, the error. One that was cancelled when you stopped the turn also says “Failed”, and so does one you denied permission for.',
  nowImg: 'img/now-failed.png',
  issues: ['Stopping a turn or denying a command looks the same as an error.', 'What went wrong shows only when opened.'],
  options: [
    { key: 'A', name: 'Zed’s icons for each', from: 'Zed (its failed and interrupted tool calls)',
      desc: 'A failed call’s icon turns into a red cross; one cut off when you stopped the turn gets a crossed circle and “Interrupted”; a denied one stays dim with “Denied”.',
      good: 'Each state has its own look, as in Zed.', cost: 'The kind’s icon is lost on a failure.',
      mock: () => tframe(stack(
        row(`<span style="display:inline-flex;color:${COLORS.error}">${I('z-close')}</span>`, verb('Ran') + code('npm test -- src/cart')),
        row(`<span style="display:inline-flex;color:var(--ph)">${I('z-stop')}</span>`, verb('Edited') + code('src/cart/total.ts'), { trailing: dim('Interrupted', 'margin-right:20px') }),
        row(I('z-terminal'), verb('Ran') + code('rm -rf dist'), { trailing: dim('Denied', 'margin-right:20px'), labelColor: 'var(--ph)' })), 110, 560) },
    { key: 'B', name: 'A word for each', from: 'Zed (its wording)',
      desc: 'Icons stay. The row ends in “Failed” in red, “Stopped” dim when you stopped the turn, or “Denied” dim when you said no.',
      good: 'Small, and the three no longer look alike.', cost: 'The error still shows only when opened.',
      mock: () => tframe(stack(
        TEST_ROW({ trailing: failedLabel() + '<span style="width:16px"></span>' }),
        EDIT_ROW({ trailing: dim('Stopped', 'margin-right:20px') }),
        row(I('z-terminal'), verb('Ran') + code('rm -rf dist'), { trailing: dim('Denied', 'margin-right:20px') })), 110, 560) },
    { key: 'C', name: 'The error under the row', from: 't3code (its error rows)',
      desc: 'B’s words, and a failed call’s icon becomes a red alert with the first line of its error under the row, in red, without a click, as t3code shows a failed step.',
      good: 'What went wrong is in view.', cost: 'A failed row takes two lines.',
      mock: () => tframe(stack(
        row(alertIcon(), verb('Ran') + code('npm test -- src/cart'), { trailing: failedLabel() + '<span style="width:16px"></span>' }), errorLine('× cartTotal &gt; rounds once, after summing → expected 3.02 to be 3.01'),
        EDIT_ROW({ trailing: dim('Stopped', 'margin-right:20px') }),
        row(I('z-terminal'), verb('Ran') + code('rm -rf dist'), { trailing: dim('Denied', 'margin-right:20px') })), 130, 560) },
    { key: 'D', name: 'Failures open by themselves', from: 'new',
      desc: 'B’s words, and a failed call opens on its own, showing its whole error.',
      good: 'Nothing to click.', cost: 'Long errors push the thread down; a failure the agent expected and fixed stays open.',
      mock: () => tframe(stack(TEST_ROW({ trailing: failedLabel(), open: true, hover: true }), out(termBlock(TEST_OUTPUT.slice(3, 7).map(escHtml))), EDIT_ROW({ trailing: dim('Stopped', 'margin-right:20px') })), 170, 560) },
  ],
});

// 11. While it runs -----------------------------------------------------------------------
TOPICS.push({
  id: 'running', section: SECTION_STATES, title: 'While a tool call runs', rec: 'C',
  now: 'A tool call that runs says it in the present tense where it can (“Running npm test”) and ends in a spinning circle. While the agent works, its current run is one live line: the row of the step in progress (or the one asking for permission, with its buttons); clicking it opens the run.',
  nowImg: 'img/now-running.png',
  issues: ['A long command looks the same at 2 seconds and at 2 minutes.', 'The live line doesn’t say how many steps are behind it.'],
  options: [
    { key: 'A', name: 'Today’s spinner', from: 'Zed',
      desc: 'As today: the present tense and a spinner.',
      good: 'Quiet.', cost: 'No sense of time or progress.',
      mock: () => tframe(stack(row(I('z-terminal'), verb('Running') + code('npm test -- src/cart'), { trailing: spinner() + '<span style="width:20px"></span>' })), 54, 560) },
    { key: 'B', name: 'Shimmer and time', from: 't3code (its live work row), Zed (its elapsed time)',
      desc: 'The running row’s label shimmers, as “Thinking” does, and after 5 seconds the time it’s been running shows dim at its end (“12 s”). The live line starts with how many steps are behind it (“4 steps ·”).',
      good: 'Says it’s alive and for how long.', cost: 'Movement in the thread while the agent works.',
      mock: () => tframe(stack(runRow(`<span class="none">4 steps ·</span>` + `<span class="none" style="display:inline-flex">${I('z-terminal')}</span>` + shine('Running npm test -- src/cart'), { trailing: dim('12 s', 'margin-right:8px') })), 56, 560) },
    { key: 'C', name: 'A running command’s last lines', from: 'Zed (its live terminal), cut to 3 lines',
      desc: 'B, and a running command shows its last 3 lines under its row as they print, dim, so a build or test run can be followed without opening it. They go when it ends.',
      good: 'Shows what a long command is doing.', cost: 'The thread’s bottom moves as lines print.',
      mock: () => tframe(stack(runRow(`<span class="none">4 steps ·</span>` + `<span class="none" style="display:inline-flex">${I('z-terminal')}</span>` + shine('Running npm run build'), { trailing: dim('12 s', 'margin-right:8px') }),
        `<div style="margin:0 0 0 32px;font:12px/17px ${MONO};color:var(--ph);white-space:pre">${BUILD_LINES(117, 119).join('\n')}</div>`), 110, 560) },
    { key: 'D', name: 'Progress as a bar', from: 'new',
      desc: 'The live line gets a thin bar under it that fills as the agent’s to-dos are done (2 of 3), so a long turn shows how far along it is.',
      good: 'How much is left, at a glance.', cost: 'Only for agents that keep to-dos, and the to-dos are their guess.',
      mock: () => tframe(stack(runRow(`<span class="none" style="display:inline-flex">${I('z-terminal')}</span>` + verb('Running') + code('npm test -- src/cart'), { trailing: spinner() + '<span style="width:8px"></span>' }), `<div style="margin:2px 0 0 32px;height:3px;border-radius:2px;background:var(--bv)"><div style="width:66%;height:100%;border-radius:2px;background:var(--ac)"></div></div>`), 60, 560) },
  ],
});

// 12. Folded runs -------------------------------------------------------------------------
TOPICS.push({
  id: 'runs', section: SECTION_STATES, title: 'A folded run', rec: 'B',
  now: 'Once the agent writes after them, a run of tool calls folds to one line that says what it did in t3code’s words: “Read 2 files, changed 1 file, ran 2 commands, and performed 1 other action”. A chevron opens it to its rows. A lone call stays a row.',
  nowImg: 'img/now-runs.png',
  issues: ['Says how many, not which: you can’t tell which files changed without opening it.', 'Nothing says a command in it failed.'],
  options: [
    { key: 'A', name: 'Today’s words', from: 't3code (its work groups)',
      desc: 'As today.',
      good: 'Short.', cost: 'As above.',
      mock: () => tframe(stack(runRow(subj('Read 2 files, changed 1 file, ran 2 commands, and performed 1 other action'))), 54, 600) },
    { key: 'B', name: 'The words, then what changed and what failed', from: 't3code, new',
      desc: 'Today’s words, then at the line’s end the lines it changed (+2 −2) and, if a command or tool failed, “1 failed” in red.',
      good: 'The two things you look for in a run, without opening it.', cost: 'A little more on the line.',
      mock: () => tframe(stack(runRow(subj('Read 2 files, changed 1 file, ran 2 commands, and performed 1 other action'), { trailing: failedLabel('1 failed') + '<span style="width:8px"></span>' + stat(2, 2) })), 54, 600) },
    { key: 'C', name: 'Names, not counts', from: 'new',
      desc: 'The line names what it acted on: “Read total.ts and round.ts, edited total.ts, ran npm test and npm run lint”, cut off with “…” when it doesn’t fit.',
      good: 'Says which files without opening.', cost: 'Long runs are cut off; names of tools nothing knows read oddly.',
      mock: () => tframe(stack(runRow(subj('Read total.ts and round.ts, edited total.ts, ran npm test and npm run lint, loaded the review skill'))), 54, 600) },
    { key: 'D', name: 'Icons with counts', from: 'new',
      desc: 'The line is the kinds’ icons, each with its count, then the changed lines: [read] 2 [edit] 1 [terminal] 2 [hammer] 1 · +2 −2.',
      good: 'The shortest, and scans fast in a long thread.', cost: 'Icons to learn; nothing in words.',
      mock: () => tframe(stack(runRow([['z-eye', 2], ['z-pen', 1], ['z-terminal', 2], ['z-hammer', 1]].map(([icon, count]) => `<span class="none row" style="gap:3px">${I(icon)}${count}</span>`).join('<span style="width:4px"></span>') + `<span class="none" style="margin-left:6px">${stat(2, 2)}</span>`)), 54, 600) },
    { key: 'E', name: 'Nothing folds', from: 'Zed (every tool call a row)',
      desc: 'Every tool call stays a row, as in Zed.',
      good: 'Everything in view.', cost: 'A long turn is a long list of rows between messages.',
      mock: () => tframe(stack(READ_ROW(), row(TS, subj('Read src/cart/round.ts')), EDIT_ROW(), TEST_ROW({ trailing: failedLabel() + '<span style="width:16px"></span>' }), row(I('z-terminal'), verb('Ran') + code('npm run lint')), row(I('z-hammer'), subj('Load skill: review'))), 180, 600) },
  ],
});

// 13. The file view -----------------------------------------------------------------------
TOPICS.push({
  id: 'output-frame', section: SECTION_OUTPUT, title: 'What holds the output', rec: 'B',
  now: 'Since the last change, a tool’s output shows as it was printed: one block in the code font on the editor’s background, with a thin border, long lines wrapped. There is nothing over it: no name, no line count, no Copy. Commands run in agentZ’s terminal show that terminal instead. These mocks show a <code>git diff</code>’s output, the one from the backlog.',
  nowImg: 'img/now-output.png',
  issues: ['Nothing says what the block is, or how long it is.', 'No way to copy the whole output except selecting it.'],
  options: [
    { key: 'A', name: 'Today’s block', from: 'agentZ',
      desc: 'As today.',
      good: 'Plain.', cost: 'As above.',
      mock: () => tframe(stack(row(I('z-terminal'), verb('Ran') + code('git diff src/cart'), { hover: true, open: true }), out(printed(GIT_DIFF.map(escHtml)) + inputLine())), 290) },
    { key: 'B', name: 'A file view with a header', from: 'Zed (its code blocks’ header)',
      desc: 'A header over the output: the file’s icon and path for a read, the command for a command, the tool’s name for others; how many lines; and Wrap and Copy at its right, as Zed’s code blocks have.',
      good: 'Says what the output is, and copies it in one click.', cost: 'A header on every opened output, also short ones.',
      mock: () => tframe(stack(row(I('z-terminal'), verb('Ran') + code('git diff src/cart'), { hover: true, open: true }), out(fileView(GIT_DIFF.map(escHtml), { icon: I('z-terminal'), title: 'git diff src/cart', meta: '12 lines', buttons: true }) + inputLine())), 320) },
    { key: 'C', name: 'Buttons on hover', from: 'Zed (its code blocks in messages)',
      desc: 'Today’s block, with Wrap and Copy in its top right corner when the pointer is over it, as Zed’s message code blocks show Copy.',
      good: 'Copy without a header.', cost: 'Still nothing says what it is or how long.',
      mock: () => tframe(stack(row(I('z-terminal'), verb('Ran') + code('git diff src/cart'), { open: true }), out(`<div style="position:relative">${printed(GIT_DIFF.map(escHtml))}<div class="row" style="position:absolute;top:5px;right:5px;gap:2px;background:var(--ed);border:1px solid var(--bv);border-radius:6px;padding:1px">${miniButton('z-wrap')}${miniButton('z-copy')}</div></div>` + inputLine())), 290) },
    { key: 'D', name: 'No box, a line on the left', from: 't3code (its tool output)',
      desc: 'The output sits on the thread’s background with a thin line on its left, as the thoughts’ text does; no border or background of its own.',
      good: 'Lightest; the thread doesn’t turn into a column of boxes.', cost: 'Output and the agent’s message are less clearly apart.',
      mock: () => tframe(stack(row(I('z-terminal'), verb('Ran') + code('git diff src/cart'), { hover: true, open: true }), out(`<div style="border-left:1px solid var(--b);padding:2px 0 2px 12px;font:12px/17px ${MONO};color:var(--t);white-space:pre-wrap">${GIT_DIFF.map(escHtml).join('\n')}</div>` + inputLine())), 280) },
    { key: 'E', name: 'Row and output in one card', from: 'Zed (its tool call card)',
      desc: 'An opened row becomes a card: the row is its header, the output its body, as Zed draws an opened tool call.',
      good: 'Clearly one thing; closing it is a click on its top.', cost: 'Opening a row changes its look, and the thread shifts sideways at its edge.',
      mock: () => tframe(`<div style="border:1px solid var(--b);border-radius:6px;overflow:hidden;background:var(--ed);flex:none">${row(I('z-terminal'), verb('Ran') + code('git diff src/cart'), { open: true, hover: true, style: 'border-radius:0;background:' + COLORS.head + ';border-bottom:1px solid var(--bv)' })}<div style="padding:6px 10px;font:12px/17px ${MONO};white-space:pre-wrap">${GIT_DIFF.map(escHtml).join('\n')}</div></div>`, 290) },
  ],
});

TOPICS.push({
  id: 'output-numbers', section: SECTION_OUTPUT, title: 'Line numbers', rec: 'B',
  now: 'Output has no line numbers of its own. Claude Agent numbers a read’s lines in its text (a number and a tab before each), so those show as text; other agents’ reads and every command have none.',
  nowImg: 'img/now-read.png',
  issues: ['The same read has numbers from one agent and not from another.', 'Claude Agent’s numbers wrap and copy with the code.'],
  options: [
    { key: 'A', name: 'As sent', from: 'agentZ',
      desc: 'As today: whatever numbers the agent put in the text.',
      good: 'Nothing parsed.', cost: 'As above.',
      mock: () => tframe(stack(READ_ROW({ open: true }), out(printed(CLAUDE_READ.map((line) => line.replace('\t', '&#9;')), { style: 'tab-size:8' }))), 200, 600) },
    { key: 'B', name: 'In a gutter, for files', from: 'Zed (its editor)',
      desc: 'File contents (reads, created files) get numbers in a dim gutter: Claude Agent’s moved there from its text, the read’s first line for the others. Commands and other output get none.',
      good: 'A file looks like a file from any agent; copying takes the code only.', cost: 'Reads Claude Agent’s format to take its numbers out.',
      mock: () => tframe(stack(READ_ROW({ open: true }), out(fileView(highlighted(READ_LINES), { numbers: 1 })), TEST_ROW({ open: true }), out(fileView(TEST_OUTPUT.slice(0, 4).map(escHtml)))), 330, 600) },
    { key: 'C', name: 'On every output', from: 'new',
      desc: 'Every opened output gets a gutter, a read’s from its first line and everything else from 1.',
      good: 'One look for every output; easy to say “line 40 of the output”.', cost: 'Numbers on a command’s output mean little.',
      mock: () => tframe(stack(READ_ROW({ open: true }), out(fileView(highlighted(READ_LINES), { numbers: 1 })), TEST_ROW({ open: true }), out(fileView(TEST_OUTPUT.slice(0, 4).map(escHtml), { numbers: 1 }))), 330, 600) },
    { key: 'D', name: 'None at all', from: 't3code',
      desc: 'Claude Agent’s numbers are taken out, and nothing gets a gutter.',
      good: 'The plainest.', cost: 'Where in the file a read was is lost, unless the row says it.',
      mock: () => tframe(stack(READ_ROW({ open: true }), out(fileView(highlighted(READ_LINES)))), 190, 600) },
  ],
});

TOPICS.push({
  id: 'output-color', section: SECTION_OUTPUT, title: 'Colors', rec: 'C',
  now: 'Output is in the text color only. A terminal agentZ runs for the agent keeps the command’s own colors; output an agent sends as text has none.',
  nowImg: 'img/now-output.png',
  issues: ['A file’s code reads as plain text.', 'A <code>git diff</code>’s output has no red or green.'],
  options: [
    { key: 'A', name: 'None', from: 'agentZ',
      desc: 'As today: the text color.',
      good: 'Exactly as printed.', cost: 'Harder to read.',
      mock: () => tframe(stack(READ_ROW({ open: true }), out(fileView(plain(READ_LINES), { numbers: 1 }))), 190, 600) },
    { key: 'B', name: 'Syntax colors for files', from: 'Zed (its editor)',
      desc: 'File contents in the theme’s syntax colors by the file’s type; the input’s JSON in colors too. Commands’ output stays plain.',
      good: 'Code reads as code.', cost: 'Highlighting for each file type, in the thread.',
      mock: () => tframe(stack(READ_ROW({ open: true }), out(fileView(highlighted(READ_LINES), { numbers: 1 }) + inputLine(true) + fileView(highlighted(READ_JSON, 'json')))), 300, 600) },
    { key: 'C', name: 'B, and diffs in commands’ output', from: 'Zed (its editor), a terminal’s git',
      desc: 'B, and output that is a diff (from <code>git diff</code>, <code>git show</code>) gets a terminal’s colors: removed lines red, added green, the hunks’ headers blue. Colors an agent sends with the text (ANSI codes) are kept.',
      good: 'The backlog’s <code>git diff</code> reads as it would in a terminal.', cost: 'Telling a diff from other output is a guess (its <code>diff --git</code> or <code>@@</code> lines). Whether agents keep ANSI colors wasn’t checked.',
      mock: () => tframe(stack(row(I('z-terminal'), verb('Ran') + code('git diff src/cart'), { open: true }), out(fileView(GIT_DIFF.map(diffColored), { icon: I('z-terminal'), title: 'git diff src/cart', meta: '12 lines', buttons: true }))), 300, 600) },
    { key: 'D', name: 'Only diffs', from: 'new',
      desc: 'Diffs in commands’ output get C’s colors; files stay plain.',
      good: 'Colors only where they say something (what was removed or added).', cost: 'Code in reads stays plain.',
      mock: () => tframe(stack(row(I('z-terminal'), verb('Ran') + code('git diff src/cart'), { open: true }), out(fileView(GIT_DIFF.map(diffColored))), READ_ROW()), 290, 600) },
  ],
});

const LONG_TEST = ['> storefront@0.4.0 test', '> vitest run', '', ...Array.from({ length: 9 }, (_, index) => ` ✓ src/${['cart', 'checkout', 'catalog'][index % 3]}/${['round', 'pay', 'search', 'total', 'tax', 'list'][index % 6]}.test.ts (${3 + (index % 4)} tests) ${4 + index}ms`)];
const LONG_TAIL = [' ✓ src/catalog/list.test.ts (6 tests) 31ms', ' ❯ src/cart/total.test.ts (2 tests | 1 failed) 7ms', '   × cartTotal > rounds once, after summing', '     → expected 3.02 to be 3.01', '', ' Test Files  1 failed | 41 passed (42)', '      Tests  1 failed | 213 passed (214)', '   Duration  3.12s'];
const scrollbarOverlay = '<div style="position:absolute;right:3px;top:34px;width:5px;height:60px;border-radius:3px;background:rgba(220,224,229,.25)"></div>';
TOPICS.push({
  id: 'output-height', section: SECTION_OUTPUT, title: 'Long output', rec: 'C',
  now: 'An opened output is at most 24 rem (384 px) tall and scrolls inside, with a scrollbar. A full test run or a big read is mostly out of view, and it opens at its top.',
  nowImg: 'img/now-output.png',
  issues: ['A command’s result is at its end, which opens out of view.', 'Scrolling inside the thread’s scroll is fiddly.'],
  options: [
    { key: 'A', name: '24 rem, scrolling', from: 'agentZ (Zed’s size)',
      desc: 'As today, in the file view.',
      good: 'The thread never grows by more than 24 rem a row.', cost: 'As above.',
      mock: () => tframe(stack(TEST_ROW({ open: true }), out(`<div style="position:relative">${fileView(LONG_TEST.map(escHtml), { icon: I('z-terminal'), title: 'npm test', meta: '214 lines', buttons: true, maxH: 200 })}${scrollbarOverlay}</div>`)), 270, 600) },
    { key: 'B', name: 'A few lines, then Show all', from: 'new',
      desc: 'The first 10 lines, fading out, with “Show all 214 lines” under them; that shows the whole output at its full height, with no scroll inside.',
      good: 'Short until you ask; no scroll inside a scroll.', cost: 'A huge output, shown all, makes the thread very long.',
      mock: () => tframe(stack(TEST_ROW({ open: true }), out(fileView(LONG_TEST.slice(0, 10).map(escHtml), { icon: I('z-terminal'), title: 'npm test', meta: '214 lines', buttons: true, fade: true, showAll: 'Show all 214 lines' }))), 290, 600) },
    { key: 'C', name: 'Files from the top, commands from the end', from: 'new',
      desc: 'As B, but a command shows its last 8 lines, where its result is, with “Show 206 earlier lines” over them; a read shows its first lines.',
      good: 'Opens on what you want from each: the start of a file, the end of a command.', cost: 'Two rules to know.',
      mock: () => tframe(stack(TEST_ROW({ open: true, trailing: failedLabel() + '<span style="width:16px"></span>' }), out(`<div style="border:1px solid var(--bv);border-radius:6px;overflow:hidden">${viewHead(I('z-terminal'), 'npm test', '214 lines', miniButton('z-wrap') + miniButton('z-copy'))}<div class="row" style="height:26px;justify-content:center;gap:4px;border-bottom:1px solid var(--bv);font-size:12px;color:var(--mu);background:var(--ed)">${ic('chev-up', 'xs')}Show 206 earlier lines</div>${fileView(LONG_TAIL.map(escHtml), { style: 'border:0;border-radius:0' })}</div>`)), 290, 600) },
    { key: 'D', name: 'Full height', from: 't3code (its expanded work rows, uncapped)',
      desc: 'An opened output shows whole, as tall as it is.',
      good: 'One scroll, the thread’s.', cost: 'A 2,000-line output is 2,000 lines of thread.',
      mock: () => tframe(stack(TEST_ROW({ open: true }), out(fileView([...LONG_TEST, ...LONG_TAIL].map(escHtml), { icon: I('z-terminal'), title: 'npm test', meta: '214 lines', buttons: true }))), 400, 600) },
  ],
});

TOPICS.push({
  id: 'output-wrap', section: SECTION_OUTPUT, title: 'Long lines', rec: 'D',
  now: 'Long lines wrap at the block’s edge, breaking anywhere when a word is too long (picked in the last round). Diffs and terminals don’t wrap: they scroll sideways.',
  nowImg: 'img/now-output.png',
  issues: ['A wrapped line and the next line are hard to tell apart in a column of output.'],
  options: [
    { key: 'A', name: 'Wrapped', from: 't3code',
      desc: 'As today.',
      good: 'Every character in view.', cost: 'As above.',
      mock: () => tframe(stack(TEST_ROW({ open: true }), out(fileView([...TEST_OUTPUT.slice(4, 7), LONG_CALL].map(escHtml)))), 190, 600) },
    { key: 'B', name: 'One line each, scrolling sideways', from: 'Zed (its code blocks)',
      desc: 'Each line stays on one line; the block scrolls sideways, with a scrollbar under it.',
      good: 'Lines stay lines; columns line up.', cost: 'The end of a long line is out of view.',
      mock: () => tframe(stack(TEST_ROW({ open: true }), out(fileView([...TEST_OUTPUT.slice(4, 7), LONG_CALL].map(escHtml), { scroll: true }))), 170, 600) },
    { key: 'C', name: 'B, with Wrap in the header', from: 'Zed (its Wrap button)',
      desc: 'B, and the header’s Wrap button wraps that output; it stays as you left it for every output.',
      good: 'Both, and you choose.', cost: 'A setting hidden in a button.',
      mock: () => tframe(stack(TEST_ROW({ open: true }), out(fileView([...TEST_OUTPUT.slice(4, 7), LONG_CALL].map(escHtml), { icon: I('z-terminal'), title: 'npm test -- src/cart', meta: '11 lines', buttons: true, scroll: true }))), 200, 600) },
    { key: 'D', name: 'Wrapped, marked in the gutter', from: 'Zed (its soft wrap), new',
      desc: 'Wrapped, with a small ↪ in the gutter on each line that continues the one above, so wrapped lines read apart from new ones.',
      good: 'Every character in view, and lines still read as lines.', cost: 'A gutter on all output for the marks.',
      mock: () => tframe(stack(TEST_ROW({ open: true }), out(`<div style="border:1px solid var(--bv);border-radius:6px;background:var(--ed);padding:6px 0;font:12px/17px ${MONO}">${[...TEST_OUTPUT.slice(4, 7).map((line) => [line]), ['src/cart/total.test.ts:12:5 AssertionError: expected 3.02 to be 3.01 // Object.is equality, at', 'Proxy.<anonymous> (node_modules/@vitest/expect/dist/index.js:1135:15)']].map((parts) => parts.map((part, index) => `<div style="display:flex;padding:0 10px 0 0"><span style="width:20px;flex:none;text-align:center;color:${COLORS.lineNumber}">${index ? '↪' : ''}</span><span style="white-space:pre">${escHtml(part)}</span></div>`).join('')).join('')}</div>`)), 190, 600) },
  ],
});

// 18. ToolSearch --------------------------------------------------------------------------
const LOADED = ['Create issue', 'List issues', 'Add comment'];
TOPICS.push({
  id: 'toolsearch', section: SECTION_SERVERS, title: 'ToolSearch', size: 'medium', rec: 'C',
  now: 'An agent that loads tools when it needs them does it with ToolSearch. Its row says what it did, with the magnifying glass: “Loaded 3 github tools” for named tools, “Searched tools for “issues”” and “3 found” for words. Opened, it lists the tools one per line by what they do (picked in the last round).',
  nowImg: 'img/now-toolsearch.png',
  issues: ['It says how many, not which, until opened.', 'It is plumbing: the agent getting ready, not doing.'],
  options: [
    { key: 'A', name: 'Today’s', from: 'agentZ',
      desc: 'As today.',
      good: 'Says what it did.', cost: 'As above.',
      mock: () => tframe(stack(row(I('z-search'), subj('Loaded 3 github tools'), { hover: true, open: true }), out(LOADED.map((name) => `<div style="font-size:13px;color:var(--mu);line-height:20px">${name}</div>`).join(''), 'gap:0')), 120, 440) },
    { key: 'B', name: 'Not in the thread', from: 'Zed, t3code (neither knows ToolSearch)',
      desc: 'ToolSearch rows don’t show, and runs don’t count them. The tools it loaded show when they’re used.',
      good: 'Runs show only work.', cost: 'A ToolSearch that failed or found nothing is hidden too.',
      mock: () => tframe(stack(row(I('z-plug'), subj('Create issue') + dim('github'))), 54, 440) },
    { key: 'C', name: 'The tools’ names in the row', from: 'new',
      desc: '“Loaded Create issue, List issues and Add comment” with “github” dimmer, cut off with “…” when long; nothing to open.',
      good: 'Says which without a click.', cost: 'Long when it loads many.',
      mock: () => tframe(stack(row(I('z-search'), subj('Loaded Create issue, List issues and Add comment') + dim('github')), row(I('z-search'), subj('Searched tools for “issues”') + dim('3 found', 'margin-right:4px'))), 84, 440) },
    { key: 'D', name: 'The tools as chips', from: 'new',
      desc: 'Today’s row, opening to the tools as chips on one line, each with its server’s plug.',
      good: 'Compact when opened.', cost: 'Chips are new in the thread.',
      mock: () => tframe(stack(row(I('z-search'), subj('Loaded 3 github tools'), { hover: true, open: true }), out(`<div class="row" style="gap:6px;flex-wrap:wrap">${LOADED.map((name) => `<span class="chip">${ic('z-plug', 'xs')}${name}</span>`).join('')}</div>`)), 90, 440) },
  ],
});

// 19. Other MCP servers' tools ------------------------------------------------------------
const ISSUE_JSON = ['{', '  "number": 412,', '  "html_url": "https://github.com/ana/storefront/issues/412",', '  "state": "open",', '  "title": "Cart total off by a cent"', '}'];
const serverBadge = (letter, color) => `<span style="width:14px;height:14px;border-radius:4px;background:${color};color:#282c33;font:600 9px 'IBM Plex Sans',sans-serif;display:inline-grid;place-items:center">${letter}</span>`;
TOPICS.push({
  id: 'mcp', section: SECTION_SERVERS, title: 'Other MCP servers’ tools', rec: 'B',
  now: 'A tool from an MCP server you added shows its name in words, then its server dimmer, with a plug: “Create issue  github” (picked in the last round). Opened, it shows what the server sent back as printed, usually JSON, and “Input” for what the agent passed.',
  nowImg: 'img/now-mcp.png',
  issues: ['The row says which tool, not what it acted on.', 'Its JSON output is plain text.'],
  options: [
    { key: 'A', name: 'Today’s, JSON in colors', from: 'agentZ',
      desc: 'Rows as today; the output’s JSON highlighted in the file view.',
      good: 'Small.', cost: 'The row still doesn’t say what it acted on.',
      mock: () => tframe(stack(row(I('z-plug'), subj('Create issue') + dim('github'), { hover: true, open: true }), out(fileView(highlighted(ISSUE_JSON, 'json')))), 190, 600) },
    { key: 'B', name: 'What it acted on, from its input', from: 'new',
      desc: 'After the tool’s name, its input’s first short text value, in the muted color: “Create issue  Cart total off by a cent  github”. Opened, A’s output.',
      good: 'Says what the tool did with no list of tools to keep.', cost: 'The first value is sometimes an ID, not a title.',
      mock: () => tframe(stack(row(I('z-plug'), verb('Create issue') + titled('Cart total off by a cent') + dim('github', 'margin-right:4px'), { hover: true, open: true }), out(fileView(highlighted(ISSUE_JSON, 'json'))), row(I('z-plug'), verb('List issues') + titled('label: billing') + dim('github', 'margin-right:20px'))), 220, 600) },
    { key: 'C', name: 'The server’s letter for an icon', from: 'agentZ’s machine icons, new',
      desc: 'In place of the plug, a small square in a color with the server’s first letter, the same in Settings › MCP Servers, so a github tool and a linear tool look apart.',
      good: 'Tells servers apart at a glance.', cost: 'Colors to give each server.',
      mock: () => tframe(stack(row(serverBadge('G', '#a1c181'), subj('Create issue') + dim('github')), row(serverBadge('L', '#b477cf'), subj('List issues') + dim('linear')), row(serverBadge('P', '#73ade9'), subj('Run query') + dim('postgres'))), 110, 600) },
    { key: 'D', name: 'The output as fields', from: 'new',
      desc: 'Opened, a JSON object is shown as its fields, one a line, name and value, links clickable; anything else as printed.',
      good: 'Reads like a record, not code.', cost: 'Nested JSON doesn’t fit; it falls back to printed.',
      mock: () => tframe(stack(row(I('z-plug'), subj('Create issue') + dim('github'), { hover: true, open: true }), out([['number', '412'], ['html_url', `<span style="color:var(--ac)">github.com/ana/storefront/issues/412</span>`], ['state', 'open'], ['title', 'Cart total off by a cent']].map(([key, value]) => `<div class="row" style="gap:10px;height:20px;font-size:12px"><span style="width:70px;color:var(--ph);font-family:${MONO}">${key}</span><span style="color:var(--t)">${value}</span></div>`).join(''), 'gap:0')), 140, 600) },
  ],
});

// 20. Images ------------------------------------------------------------------------------
const pageShot = (w, h) => `<div style="width:${w}px;height:${h}px;flex:none;background:#f6f6f4;border-radius:4px;overflow:hidden;display:flex;flex-direction:column">
  <div style="height:12%;background:#24292f;display:flex;align-items:center;padding:0 6%;gap:6%"><span style="width:18%;height:30%;background:#fff;opacity:.85;border-radius:2px"></span><span style="margin-left:auto;width:10%;height:26%;background:#fff;opacity:.4;border-radius:2px"></span></div>
  <div style="flex:1;padding:6% 8%;display:flex;flex-direction:column;gap:7%">${[0, 1, 2].map(() => '<div style="display:flex;gap:6%;align-items:center;height:14%"><span style="width:14%;height:100%;background:#d8dbe0;border-radius:3px"></span><span style="flex:1;height:40%;background:#c9ccd2;border-radius:2px"></span><span style="width:14%;height:40%;background:#9aa0aa;border-radius:2px"></span></div>').join('')}
  <div style="margin-top:auto;display:flex;justify-content:flex-end;gap:4%"><span style="width:22%;height:${Math.round(h * 0.07)}px;background:#9aa0aa;border-radius:2px"></span><span style="width:20%;height:${Math.round(h * 0.07)}px;background:#2e7d4f;border-radius:3px"></span></div></div></div>`;
const imageRow = (options = {}) => row(I('z-image'), subj('Read screenshots/cart.png'), options);
TOPICS.push({
  id: 'images', section: SECTION_IMAGES, title: 'Images in tool calls', rec: 'C',
  now: 'A tool call that gives back an image (a read of a PNG, a screenshot) opens by itself to it, at most 384 px wide and tall, in a thin border; a click opens the viewer (picked in the last round).',
  nowImg: 'img/now-images.png',
  issues: ['A run with several screenshots fills the thread with them.', 'Nothing says the image’s size.'],
  options: [
    { key: 'A', name: 'Today’s, 384 px', from: 'Zed (its size)',
      desc: 'As today.',
      good: 'Big enough to read.', cost: 'As above.',
      mock: () => tframe(stack(imageRow({ open: true }), out(`<div style="border:1px solid var(--bv);border-radius:6px;padding:2px;width:fit-content">${pageShot(384, 240)}</div>`)), 310, 600) },
    { key: 'B', name: 'Thumbnails', from: 't3code (its message thumbnails)',
      desc: 'Images show 160 px wide, a few to a line when a call gives back several, as the last round picked for messages; a click opens the viewer.',
      good: 'Several images fit in a line.', cost: 'Text in a screenshot can’t be read without opening it.',
      mock: () => tframe(stack(imageRow({ open: true }), out(`<div class="row" style="gap:8px">${[0, 1].map(() => `<div style="border:1px solid var(--bv);border-radius:6px;padding:2px">${pageShot(160, 100)}</div>`).join('')}</div>`)), 170, 600) },
    { key: 'C', name: 'A’s, with what it is under it', from: 'new',
      desc: 'A, with the image’s name, its size in pixels and on disk under it, dim: “cart.png · 1280 × 800 · 212 KB”.',
      good: 'Says which image and how big, which matters for screenshots of a page.', cost: 'One more line.',
      mock: () => tframe(stack(imageRow({ open: true }), out(`<div style="border:1px solid var(--bv);border-radius:6px;padding:2px;width:fit-content">${pageShot(384, 240)}</div><div style="font-size:12px;color:var(--ph)">cart.png · 1280 × 800 · 212 KB</div>`)), 330, 600) },
    { key: 'D', name: 'Closed until clicked', from: 'Zed (its tool calls start closed)',
      desc: 'An image call starts closed like any row, with a small thumbnail at the row’s end; opening it shows A’s image.',
      good: 'Runs with screenshots stay short.', cost: 'A click to see each image.',
      mock: () => tframe(stack(imageRow({ trailing: `<span style="border:1px solid var(--bv);border-radius:3px;margin-right:4px">${pageShot(32, 20)}</span>` }), row(I('z-image'), subj('Read screenshots/checkout.png'), { trailing: `<span style="border:1px solid var(--bv);border-radius:3px;margin-right:4px">${pageShot(32, 20)}</span>` })), 90, 600) },
  ],
});
