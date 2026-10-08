// Tool calls and images in a thread: icons, input, long lines, images in tool calls and
// messages, the image viewer, agentZ's own tools as what they did, ToolSearch, and other MCP
// tools' names. Thread-view pieces as agent_view.rs draws them, in JetBrains Dark.

// Zed's tool icons (16-unit paths, drawn at 1.5× in the 24-unit box).
const zedIcon = (paths) => `<g transform="scale(1.5)" stroke-width="1.2">${paths}</g>`;
ICONS['zed-search'] = zedIcon('<path d="M13 13L11 11"/><circle cx="7.5" cy="7.5" r="4.5"/>');
ICONS['zed-hammer'] = zedIcon('<path d="M9 8.5L4.95 12.62a1.25 1.25 0 0 1-1.75-1.75L7.5 6.5"/><path d="M10.84 9.98l3-3"/><path d="M12.84 7.42l-1.07-1a1 1 0 0 1-.33-.73v-.61L10.17 3.9A3.4 3.4 0 0 0 7.82 3l-1.98-.01.52.43a2.9 2.9 0 0 1 1.15 2.4L7.5 6.5 9 8.5l.5-.5s.37-.2.58-.01l1.07 1"/>');
ICONS.image = '<rect width="18" height="18" x="3" y="3" rx="2" ry="2"/><circle cx="9" cy="9" r="2"/><path d="m21 15-3.086-3.086a2 2 0 0 0-2.828 0L6 21"/>';
ICONS['zoom-in'] = '<circle cx="11" cy="11" r="8"/><line x1="21" x2="16.65" y1="21" y2="16.65"/><line x1="11" x2="11" y1="8" y2="14"/><line x1="8" x2="14" y1="11" y2="11"/>';
ICONS.plug = '<path d="M12 22v-5"/><path d="M9 8V2"/><path d="M15 8V2"/><path d="M18 8v5a4 4 0 0 1-4 4h-4a4 4 0 0 1-4-4V8Z"/>';
ICONS['square-pen'] = '<path d="M12 3H5a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7"/><path d="M18.375 2.625a1 1 0 0 1 3 3l-9.013 9.014a2 2 0 0 1-.853.505l-2.873.84a.5.5 0 0 1-.62-.62l.84-2.873a2 2 0 0 1 .506-.852z"/>';
ICONS.wrap = '<path d="M3 6h18"/><path d="M3 12h15a3 3 0 1 1 0 6h-4"/><polyline points="16 16 14 18 16 20"/><path d="M3 18h7"/>';
ICONS['chev-left'] = '<path d="m15 18-6-6 6-6"/>';
ICONS.agentz = '<rect x="3" y="3" width="18" height="18" rx="4" opacity=".55"/><path d="M8 8h8l-8 8h8"/>';
ICONS.spinner = '<path d="M21 12a9 9 0 1 1-6.219-8.56"/>';

const JBT = '--ed:#1e1f22;--panel:#26282b;--title:#26282b;--t:#dfe1e5;--mu:#b0b1b3;--ph:#6f737a;--b:#393b41;--bv:#393b41;--hov:#3c3e41;--sel:#43454a;--ac:#548af7;--ok:#57965d;--del:#fa6675;--warn:#e0b45c;';
// The rows' gray (work_row_color): the muted gray a quarter of the way toward the background.
const DIM = '#8d8e91';
const MONO = "'IBM Plex Mono','SF Mono',Menlo,monospace";
const W = 720;
// The thread's conversation, at the app's margins (mx_5).
const tframe = (html, h, w = W) => frame(`<div style="position:absolute;inset:0;overflow:hidden;background:var(--panel);padding:14px 20px;display:flex;flex-direction:column;gap:2px">${html}</div>`, { w, h, style: JBT });
const pairT = (a, b) => `<div style="display:flex;flex-direction:column;gap:10px">${a}${b}</div>`;

// A row (render_tool_call): a 24 px icon cell, the label in the rows' gray, then what trails.
const trow = (icon, label, { trailing = '', hover = false, chevron = '', iconColor = DIM } = {}) => `<div class="row" style="min-height:24px;gap:6px;padding:0 2px;border-radius:5px;${hover ? 'background:var(--hov);' : ''}">
  <span style="width:24px;display:inline-flex;justify-content:center;color:${iconColor}">${icon}</span>
  <span class="row grow" style="gap:4px;min-width:0;font-size:13px;color:${DIM}">${label}</span>${trailing}${chevron ? `<span style="color:var(--ph);display:inline-flex;margin-right:2px">${ic(chevron, 'xs')}</span>` : ''}</div>`;
const verb = (text) => `<span class="none">${text}</span>`;
const subj = (text) => `<span class="trunc">${text}</span>`;
const codeSubj = (text) => `<span class="trunc" style="font:12px ${MONO}">${text}</span>`;
const bright = (text) => `<span class="trunc" style="color:#c4c6ca">${text}</span>`;
const stat = (add, del) => `<span class="none" style="font-size:12px"><span style="color:#57965d">+${add}</span> <span style="color:#fa6675">−${del}</span></span>`;
const dimText = (text, style = '') => `<span class="none" style="font-size:12px;color:var(--ph);${style}">${text}</span>`;
const link = (text) => `<span class="none row" style="gap:3px;font-size:12px;color:var(--ac)">${text}</span>`;
// What opens under a row: past its icon (ml 30 px), scrolling past 24 rem.
const out = (html, style = '') => `<div style="margin-left:30px;padding:4px 0;display:flex;flex-direction:column;gap:4px;${style}">${html}</div>`;
// A code block in tool output (tool_output_style): 12 px code font, 17 px lines, the editor's
// background with a border. Lines don't wrap and the block clips them, as today.
const codeBlock = (text, { wrap = false, scrollbar = false, buttons = false, style = '' } = {}) => `<div style="position:relative;border:1px solid var(--b);border-radius:6px;background:var(--ed);padding:8px;overflow:hidden;${style}">
  <div style="font:12px/17px ${MONO};color:#c9ccd1;white-space:${wrap ? 'pre-wrap;overflow-wrap:anywhere' : 'pre'}">${text}</div>
  ${scrollbar ? '<div style="position:absolute;left:8px;bottom:2px;width:46%;height:5px;border-radius:3px;background:rgba(223,225,229,.28)"></div>' : ''}
  ${buttons ? `<div class="row" style="position:absolute;top:5px;right:5px;gap:2px">${miniBtn('wrap', true)}${miniBtn('copy')}</div>` : ''}</div>`;
const miniBtn = (icon, on = false) => `<span style="width:22px;height:22px;border-radius:5px;display:inline-grid;place-items:center;background:${on ? 'var(--sel)' : 'var(--panel)'};border:1px solid var(--b);color:var(--mu)">${ic(icon, 'xs')}</span>`;
const label = (text) => `<div style="font:11px ${MONO};color:var(--ph)">${text}</div>`;

const para = (html) => `<div style="line-height:22px;color:var(--t);padding:4px 0">${html}</div>`;
const bubble = (html) => `<div style="display:flex;justify-content:flex-end;padding:4px 0 10px"><div style="max-width:78%;background:#2f3134;border-radius:12px;padding:8px 12px;line-height:21px">${html}</div></div>`;
const mention = (text) => `<span style="color:var(--ac);text-decoration:underline;text-decoration-color:rgba(84,138,247,.5)">${text}</span>`;

// The screenshot the agent read: agentZ's welcome screen on Linux, in the desktop around it, as
// the 1280×800 picture in the user's screenshot. Its text scales with it.
function shot(w, { style = '' } = {}) {
  const h = Math.round(w * 0.625);
  const f = (w / 476) * 4.6;
  const item = (text, key = '') => `<div style="display:flex;justify-content:space-between;padding:.25em 0;color:#555"><span>▪ ${text}</span><span style="color:#999">${key}</span></div>`;
  return `<div style="position:relative;width:${w}px;height:${h}px;flex:none;background:#3b3f4a;font:${f}px/1.3 'IBM Plex Sans',sans-serif;${style}">
    <div style="position:absolute;left:4.6%;top:6%;width:90.3%;height:88%;background:#fafafa;display:flex;flex-direction:column;overflow:hidden">
      <div style="height:5%;min-height:3px;background:#ececec;border-bottom:1px solid #ddd;display:flex;align-items:center;gap:1em;padding:0 1em;color:#666"><span>▣ All projects ⌄</span><span style="margin-left:auto">Agents</span><span>Workspaces</span><span>✕</span></div>
      <div style="flex:1;display:flex;min-height:0">
        <div style="width:20%;background:#efefef;border-right:1px solid #ddd;display:flex;flex-direction:column;color:#777;padding:.4em .6em"><span>⌕ Search…</span><span style="margin:auto">No projects yet</span><span>⚙ Settings</span></div>
        <div style="flex:1;display:flex;align-items:center;justify-content:center"><div style="width:34%;color:#333"><div style="text-align:center;font-size:1.25em;margin-bottom:1em">Welcome to agentZ</div><div style="color:#aaa;font-size:.8em;border-bottom:1px solid #e5e5e5;margin-bottom:.3em">GET STARTED</div>${item('Open Folder…', 'Ctrl O')}${item('Install an Agent…')}${item('Add Machine…')}${item('Settings', 'Ctrl ,')}</div></div>
      </div>
    </div></div>`;
}
// A second screenshot for message mocks: a narrow sidebar cut off on the right.
function shot2(w, { style = '' } = {}) {
  const h = Math.round(w * 0.625);
  return `<div style="position:relative;width:${w}px;height:${h}px;flex:none;background:#2b2d30;${style}"><div style="position:absolute;left:0;top:0;bottom:0;width:30%;background:#26282b;border-right:1px solid #393b41"></div><div style="position:absolute;left:6%;top:12%;width:20%;height:6%;background:#43454a;border-radius:2px"></div><div style="position:absolute;left:6%;top:24%;width:16%;height:5%;background:#393b41;border-radius:2px"></div><div style="position:absolute;left:36%;top:14%;width:56%;height:8%;background:#393b41;border-radius:2px"></div><div style="position:absolute;left:36%;top:30%;width:44%;height:5%;background:#393b41;border-radius:2px"></div></div>`;
}

// The read in the user's screenshot.
const READ_PATH = '/tmp/az-linux-rel.png';
const READ_JSON = '{\n  "file_path": "/tmp/az-linux-rel.png"\n}';
const readRow = (icon = ic('zed-search', 'sm'), opts = {}) => trow(icon, subj(`Read ${READ_PATH}`), opts);

// ToolSearch, as in the user's screenshot.
const LOADED = ['orchestrator_capabilities', 'agentz_thread_launch', 'create_threads', 'delegate_task', 'task_status', 'agentz_thread_wait', 'agentz_thread_read', 'agentz_thread_list'];
const LOADED_TITLES = ['Get orchestration capabilities', 'Launch an agentZ thread', 'Create agentZ threads', 'Delegate a child task', 'Get delegated task status', 'Wait for an agentZ thread', 'Read an agentZ thread', 'List agentZ threads'];
const TS_QUERY = `select:${LOADED.map((name) => `agentz___${name}`).join(',')}`;
const TS_JSON = `{\n  "query": "${TS_QUERY}"\n}`;
const TS_OUTPUT = `Loaded 8 tool(s): ${LOADED.map((name) => `agentz___${name}`).join(', ')}`;

// Subthreads the agent started, as in the Agents list in the user's screenshots.
const SUBTHREADS = ['Research: UI for child tasks', 'Research: how agents get agentZ’s tools', 'Research: server side of delegated tasks'];
const userAsk = bubble('i wanna test subthreads, spawn some sub threads to research how child tasks should look');

// 1. Icons ---------------------------------------------------------------------------------
const KINDS = [
  ['read', 'Read cart/total.ts'],
  ['image', `Read ${READ_PATH}`],
  ['search', 'Search “roundTotal”'],
  ['edit', 'Edited cart/total.ts', stat(4, 2)],
  ['command', 'Ran', 'npm test'],
  ['fetch', 'Fetch https://docs.rs/tokio'],
  ['own', 'agentz___delegate_task'],
  ['mcp', 'github___create_issue'],
];
function kindRows(icons) {
  return KINDS.map(([kind, text, extra]) => {
    const labelHtml = kind === 'command' ? verb(text) + codeSubj(extra) : subj(text);
    return trow(icons[kind], labelHtml, { trailing: kind === 'edit' ? extra : '' });
  }).join('');
}
const ZED_ICONS = { read: ic('zed-search', 'sm'), image: ic('zed-search', 'sm'), search: ic('zed-search', 'sm'), edit: ic('pencil', 'sm'), command: ic('terminal', 'sm'), fetch: ic('globe', 'sm'), own: ic('zed-hammer', 'sm'), mcp: ic('zed-hammer', 'sm') };
const T3_ICONS = { read: ic('eye', 'sm'), image: ic('eye', 'sm'), search: ic('zed-search', 'sm'), edit: ic('square-pen', 'sm'), command: ic('terminal', 'sm'), fetch: ic('globe', 'sm'), own: ic('agentz', 'sm'), mcp: ic('plug', 'sm') };
const FILE_ICONS = { ...T3_ICONS, read: ic('file', 'sm'), image: ic('image', 'sm'), edit: ic('file', 'sm') };

TOPICS.push({
  id: 'icons', section: 'A tool call', title: 'Icons by kind', size: 'medium', rec: 'A',
  now: 'Zed’s icons by the kind the agent gives a tool call: a magnifying glass for reads and searches, a pencil for edits, a terminal for commands, a globe for fetches. Every other tool, agentZ’s own and other MCP tools included, gets a hammer. The labels in these mocks are today’s; other topics decide them.',
  nowImg: '../feedback/evidence/12-read-image-tool-call.png',
  issues: ['A file read gets the same magnifying glass as a search.', 'agentZ’s own tools and other MCP tools all get the hammer, the icon for any unknown tool.'],
  options: [
    { key: 'A', name: 't3code’s icons', from: 't3code',
      desc: 'An eye for reads, a magnifying glass for searches, a pen on a square for edits, a terminal for commands, a globe for fetches, the agentZ mark for agentZ’s own tools, and a plug for other MCP tools. The hammer stays for tools nothing names.',
      good: 'A read no longer looks like a search, and agentZ’s tools stand out.', cost: 'Moves away from Zed’s icons.',
      mock: () => tframe(kindRows(T3_ICONS), 232, 520) },
    { key: 'B', name: 'The file’s own icon for reads and edits', from: 'Zed (its edit cards), extended to reads',
      desc: 'Reads and edits get the icon of their file’s type, as Zed’s edit cards do: an image icon for a PNG, a document for code. Other kinds get A’s icons.',
      good: 'You see what kind of file it was.', cost: 'More different icons in a run of rows.',
      mock: () => tframe(kindRows(FILE_ICONS), 232, 520) },
    { key: 'C', name: 'As it is', from: 'Zed (today)',
      desc: 'Zed’s icons as today, with the hammer for every other tool.',
      good: 'Matches Zed.', cost: 'Reads look like searches; agentZ’s tools look unknown.',
      mock: () => tframe(kindRows(ZED_ICONS), 232, 520) },
  ],
});

// 2. Input ---------------------------------------------------------------------------------
const imageOut = (w = 384) => `<div style="align-self:flex-start">${shot(w)}</div>`;
TOPICS.push({
  id: 'input', section: 'A tool call', title: 'The input, when a row is opened', size: 'wide', rec: 'B',
  now: 'Opening a row that isn’t a command or an edit shows the tool’s input first, as pretty-printed JSON in a code block, then its output. Zed does the same, with "Raw Input:" and "Output:" headings, and leaves the input out when the output has an image. Its permission card hides the input behind "View Raw Input".',
  nowImg: '../feedback/evidence/12-read-image-tool-call.png',
  issues: ['The input repeats the path the row already names.', 'It’s raw JSON, with braces and quotes.', 'It comes before the output, so what the tool gave back starts lower.'],
  options: [
    { key: 'A', name: 'Zed’s: labeled, and none beside an image', from: 'Zed',
      desc: 'Under the row, a small "Input" heading over the JSON, then an "Output" heading over what came back, along a thin line under the icon. A tool call with an image shows only the image.',
      good: 'Matches Zed; you can tell input from output.', cost: 'The input still repeats the row for most tools.',
      mock: () => pairT(
        tframe([readRow(ic('zed-search', 'sm'), { chevron: 'chev-up', hover: true }), out(imageOut(300))].join(''), 238),
        tframe([trow(ic('zed-search', 'sm'), subj('Search “roundTotal”'), { chevron: 'chev-up', hover: true }), out([label('Input'), codeBlock('{\n  "pattern": "roundTotal",\n  "path": "src/cart"\n}'), label('Output'), codeBlock('src/cart/total.ts:14:  return roundTotal(sum)\nsrc/cart/round.ts:3:export function roundTotal(value) {')].join(''), 'border-left:1px solid var(--b);margin-left:13px;padding-left:16px')].join(''), 250)) },
    { key: 'B', name: 'Output first, the input behind “Input”', from: 'Zed (its "View Raw Input")',
      desc: 'Opening a row shows the output only. At its end, a small "Input" line with a chevron opens the JSON, as Zed’s permission card does with "View Raw Input". A tool call with an image shows only the image.',
      good: 'What came back is right under the row; the input is still one click away.', cost: 'Two clicks to see what the agent passed.',
      mock: () => pairT(
        tframe([readRow(ic('zed-search', 'sm'), { chevron: 'chev-up', hover: true }), out(imageOut(300))].join(''), 238),
        tframe([trow(ic('zed-search', 'sm'), subj('Search “roundTotal”'), { chevron: 'chev-up', hover: true }), out([codeBlock('src/cart/total.ts:14:  return roundTotal(sum)\nsrc/cart/round.ts:3:export function roundTotal(value) {'), `<div class="row" style="gap:4px;font:11px ${MONO};color:var(--ph);height:20px">Input${ic('chev-down', 'xs')}</div>`].join(''))].join(''), 150)) },
    { key: 'C', name: 'The input as plain lines', from: 'new',
      desc: 'The input shows as one line per field, the name dim and the value after it, with no braces or quotes, and only the fields the row doesn’t already show. A read of one file shows none.',
      good: 'Short and readable.', cost: 'Nested inputs (lists, objects) still need JSON.',
      mock: () => pairT(
        tframe([readRow(ic('zed-search', 'sm'), { chevron: 'chev-up', hover: true }), out(imageOut(300))].join(''), 238),
        tframe([trow(ic('zed-search', 'sm'), subj('Search “roundTotal”'), { chevron: 'chev-up', hover: true }), out([`<div style="font:12px/17px ${MONO}"><span style="color:var(--ph);display:inline-block;width:60px">path</span><span style="color:#c9ccd1">src/cart</span></div>`, codeBlock('src/cart/total.ts:14:  return roundTotal(sum)\nsrc/cart/round.ts:3:export function roundTotal(value) {')].join(''))].join(''), 160)) },
    { key: 'D', name: 'As it is', from: 'Zed, without its headings (today)',
      desc: 'The JSON input, then the output, for every tool but commands and edits, images included.',
      good: 'Everything in one place.', cost: 'Repeats the row, and pushes the output down.',
      mock: () => tframe([readRow(ic('zed-search', 'sm'), { chevron: 'chev-up', hover: true }), out([codeBlock(READ_JSON), imageOut(300)].join(''))].join(''), 320) },
  ],
});

// 3. Long lines ----------------------------------------------------------------------------
const longRow = trow(ic('zed-search', 'sm'), subj('ToolSearch'), { chevron: 'chev-up', hover: true });
TOPICS.push({
  id: 'long-lines', section: 'A tool call', title: 'Long lines in the output', size: 'medium', rec: 'A',
  now: 'Code blocks in a tool call keep each line on one line. They scroll sideways, as Zed’s do, but show no scrollbar, and Copy shows on hover. Zed’s message code blocks also have a Wrap button on hover.',
  nowImg: '../feedback/evidence/13-toolsearch-call.png',
  issues: ['Lines run past the right edge and are cut mid-word.', 'Nothing shows that they scroll sideways.'],
  options: [
    { key: 'A', name: 'Wrapped', from: 't3code',
      desc: 'Long lines wrap at the block’s edge, breaking anywhere when a word is too long, as t3code’s tool output does. Commands’ terminals stay as they are.',
      good: 'Everything shows without scrolling.', cost: 'A wrapped line can look like two lines.',
      mock: () => tframe([longRow, out([codeBlock(TS_JSON, { wrap: true }), codeBlock(TS_OUTPUT, { wrap: true })].join(''))].join(''), 360, 560) },
    { key: 'B', name: 'Scrolling, with Wrap and Copy on hover', from: 'Zed (its message code blocks)',
      desc: 'Lines stay on one line and scroll sideways. Hovering a block shows a thin scrollbar and Zed’s Wrap and Copy buttons in its corner; Wrap wraps that block.',
      good: 'Code keeps its shape; wrapping is there when you want it.', cost: 'A long line needs a scroll or a click.',
      mock: () => tframe([longRow, out([codeBlock(TS_JSON, { scrollbar: true, buttons: true }), codeBlock(TS_OUTPUT)].join(''))].join(''), 200, 560) },
    { key: 'C', name: 'As it is', from: 'Zed (today)',
      desc: 'Lines scroll sideways with no scrollbar; Copy shows on hover.',
      good: 'Nothing to change.', cost: 'Looks cut off.',
      mock: () => tframe([longRow, out([codeBlock(TS_JSON), codeBlock(TS_OUTPUT)].join(''))].join(''), 200, 560) },
  ],
});

// 4. Images in tool calls ------------------------------------------------------------------
const zoomCursor = (style) => `<span style="position:absolute;${style};z-index:5;width:22px;height:22px;border-radius:50%;background:rgba(30,31,34,.85);border:1px solid #555;display:grid;place-items:center;color:#fff">${ic('zoom-in', 'xs')}</span>`;
const imageBox = (w, extra = '') => `<div style="position:relative;align-self:flex-start;border:1px solid var(--b);border-radius:6px;overflow:hidden;line-height:0">${shot(w)}${extra}</div>`;
TOPICS.push({
  id: 'tool-images', section: 'Images', title: 'Images in tool calls', size: 'wide', rec: 'B',
  now: 'A tool call that gives back an image (a read of a PNG, a screenshot) starts closed like any row. Opened, it shows the input, then the image at most 384 px wide and tall, Zed’s size. A click opens it in the image viewer. The input topic decides whether the JSON shows here; these mocks leave it out.',
  nowImg: '../feedback/evidence/12-read-image-tool-call.png',
  issues: ['You see the image only after opening the row.', 'At 384 px, a screenshot’s text can’t be read.', 'Only the pointer cursor hints that a click opens it larger.', 'The gray around the window is part of the screenshot itself (the desktop it caught), so the window is smaller still.'],
  options: [
    { key: 'A', name: 'Opens to the image, 256 px tall, zoom on click', from: 't3code',
      desc: 'The row starts closed. Opened, it shows the image at most 256 px tall (t3code’s 16 rem), in a thin border, with a zoom-in cursor; a click opens it in the image viewer.',
      good: 'Short rows; the image is a click away.', cost: 'About today’s size, and still behind a click.',
      mock: () => tframe([trow(ic('eye', 'sm'), subj(`Read ${READ_PATH}`), { chevron: 'chev-up', hover: true }), out(imageBox(410, zoomCursor('left:200px;top:110px')))].join(''), 320) },
    { key: 'B', name: 'Shown at once, 384 px, zoom on click', from: 'new, at Zed’s size',
      desc: 'A tool call with an image starts open, so the image shows under its row without a click, at most 384 px wide and tall (today’s size), in a thin border, with a zoom-in cursor; a click opens the viewer. Clicking the row closes it as any row.',
      good: 'You see what the agent saw as it scrolls by.', cost: 'Taller turns when an agent looks at many images.',
      mock: () => tframe([trow(ic('eye', 'sm'), subj(`Read ${READ_PATH}`), { chevron: 'chev-up' }), out(imageBox(384, zoomCursor('left:180px;top:100px'))), para('The welcome screen draws on Linux, but the sidebar’s text is clipped at the bottom.')].join(''), 360) },
    { key: 'C', name: 'As wide as the thread', from: 'new',
      desc: 'As B, but the image takes the thread’s width (up to its own size), so a screenshot’s text can often be read in place.',
      good: 'Readable without opening the viewer.', cost: 'A large image takes most of the screen.',
      mock: () => tframe([trow(ic('eye', 'sm'), subj(`Read ${READ_PATH}`), { chevron: 'chev-up' }), out(imageBox(646))].join(''), 470) },
    { key: 'D', name: 'A thumbnail on the row', from: 'new',
      desc: 'The row stays one line, with a small thumbnail at its end (about 40 × 24 px). Hovering it shows the 320 × 240 preview the composer’s chips use; a click opens the viewer.',
      good: 'Keeps the thread short and still shows there’s an image.', cost: 'The thumbnail is too small to tell much.',
      mock: () => tframe([trow(ic('eye', 'sm'), subj(`Read ${READ_PATH}`), { trailing: `<span style="border:1px solid var(--b);border-radius:3px;overflow:hidden;line-height:0;flex:none">${shot(40)}</span>` }), trow(ic('eye', 'sm'), subj('Read /tmp/az-linux-settings.png'), { trailing: `<span style="border:1px solid var(--b);border-radius:3px;overflow:hidden;line-height:0;flex:none">${shot2(40)}</span>` }), `<div style="position:relative;height:250px"><div class="pop" style="right:0;top:6px;padding:4px;line-height:0">${shot(320)}</div></div>`].join(''), 320) },
    { key: 'E', name: 'As it is', from: 'Zed (today)',
      desc: 'Closed at first; opened, the input then the image, at most 384 px; a click opens the viewer.',
      good: 'Nothing to change.', cost: 'Small, and hidden until opened.',
      mock: () => tframe([readRow(ic('zed-search', 'sm'), { chevron: 'chev-up', hover: true }), out([codeBlock(READ_JSON), `<div style="align-self:flex-start;line-height:0">${shot(384)}</div>`].join(''))].join(''), 380) },
  ],
});

// 5. Images in messages --------------------------------------------------------------------
const thumbs = (images) => `<div style="display:grid;grid-template-columns:repeat(2,100px);gap:8px;margin-bottom:8px">${images.map((image) => `<div style="position:relative;width:100px;height:75px;border:1px solid #4a4d54;border-radius:8px;overflow:hidden;line-height:0">${image}</div>`).join('')}</div>`;
const croppedShot = (draw) => `<div style="width:100px;height:75px;overflow:hidden;display:flex;justify-content:center">${draw(120)}</div>`;
const MESSAGE_TEXT = 'The sidebar looks cut off on Linux. Can you fix it?';
const agentWithImage = (image) => para(`Here’s the window after the fix:${image}`);
TOPICS.push({
  id: 'message-images', section: 'Images', title: 'Images in messages', size: 'medium', rec: 'A',
  now: 'An image you paste into a message, or one an agent puts in its reply, shows as an "@Image" link in the text, as Zed writes a mention. Hovering it shows a 320 × 240 preview; a click opens the image viewer.',
  issues: ['The message doesn’t show the image, only the word @Image.', 'Several images read "@Image @Image" with nothing to tell them apart.'],
  options: [
    { key: 'A', name: 'Thumbnails in the bubble', from: 't3code',
      desc: 'Your message’s images show as thumbnails above its text, two to a row, each 100 × 75 px, cropped to fill. An image in an agent’s reply shows the same way where it is. A click opens the viewer, with arrows to the message’s other images.',
      good: 'You see what you sent at a glance.', cost: 'Crops show only the middle of a wide screenshot.',
      mock: () => tframe([bubble(`${thumbs([croppedShot(shot), croppedShot(shot2)])}${MESSAGE_TEXT}`), agentWithImage(`<div style="margin-top:6px">${thumbs([croppedShot(shot)])}</div>`)].join(''), 330, 560) },
    { key: 'B', name: 'Whole, up to 240 px tall', from: 'new',
      desc: 'Each image shows whole, not cropped, at most 240 px tall and the message’s width, above your text or in the agent’s reply. A click opens the viewer.',
      good: 'The whole picture shows.', cost: 'Messages with images get tall.',
      mock: () => tframe([bubble(`<div style="display:flex;gap:8px;margin-bottom:8px;line-height:0"><div style="border-radius:6px;overflow:hidden">${shot(176)}</div><div style="border-radius:6px;overflow:hidden">${shot2(176)}</div></div>${MESSAGE_TEXT}`), agentWithImage(`<div style="margin-top:6px;line-height:0;border-radius:6px;overflow:hidden;width:240px">${shot(240)}</div>`)].join(''), 380, 560) },
    { key: 'C', name: 'As it is', from: 'Zed (today)',
      desc: 'An "@Image" link that previews on hover and opens on click.',
      good: 'Messages stay short.', cost: 'You can’t see the images without hovering.',
      mock: () => tframe([bubble(`${mention('@Image')} ${mention('@Image')} ${MESSAGE_TEXT}`), `<div style="position:relative;height:200px">${agentWithImage(` ${mention('@Image')}`)}<div class="pop" style="left:180px;top:30px;padding:4px;line-height:0">${shot(256)}</div></div>`].join(''), 300, 560) },
  ],
});

// 6. The image viewer ----------------------------------------------------------------------
const viewerFrame = (inner, w = 900, h = 560) => frame(`<div style="position:absolute;inset:0;background:var(--panel)">${[userAsk, para('…')].join('')}</div><div style="position:absolute;inset:0;background:rgba(0,0,0,.8);display:flex;align-items:center;justify-content:center">${inner}</div>`, { w, h, style: JBT });
const closeBtn = `<span style="width:24px;height:24px;border-radius:5px;display:inline-grid;place-items:center;color:#dfe1e5">${ic('x', 'sm')}</span>`;
const arrow = (dir, style) => `<span style="position:absolute;${style};width:32px;height:32px;border-radius:50%;background:rgba(255,255,255,.12);display:grid;place-items:center;color:#fff">${ic(dir, 'sm')}</span>`;
TOPICS.push({
  id: 'viewer', section: 'Images', title: 'An image opened larger', size: 'medium', rec: 'A',
  now: 'A click on an image opens t3code’s viewer in its simplest form: the image as large as the window allows, over a dark backdrop, with a close button above it. Escape, the button or a click beside the image closes it.',
  issues: ['It shows one image; to see the next one in a message, you close it and click again.', 'A large screenshot is shrunk to fit, with no way to zoom in.', 'Nothing says which image it is.'],
  options: [
    { key: 'A', name: 't3code’s viewer, whole', from: 't3code',
      desc: 'Arrows (and the ← → keys) step through the other images in the same message or tool call. The name shows under the image ("az-linux-rel.png · 1 of 2"). A click zooms to 200% where you clicked and back to fit; scrolling zooms, dragging moves the zoomed image, and "200% zoom" shows while zoomed.',
      good: 'You can read a screenshot’s small text.', cost: 'More to build than the rest.',
      mock: () => viewerFrame(`<div style="position:relative;display:flex;flex-direction:column;align-items:flex-end;gap:8px">${closeBtn}<div style="line-height:0;border-radius:8px;overflow:hidden;position:relative">${shot(680)}${zoomCursor('left:330px;top:200px')}</div><div style="align-self:center;font-size:12px;color:rgba(255,255,255,.8)">az-linux-rel.png · 1 of 2</div></div>${arrow('chev-left', 'left:22px;top:250px')}${arrow('chev-right', 'right:22px;top:250px')}`) },
    { key: 'B', name: 'As it is, with the name', from: 't3code (its caption)',
      desc: 'Today’s viewer, with the image’s name and size under it ("az-linux-rel.png · 1280 × 800").',
      good: 'Small change; you know which image it is.', cost: 'Still no zoom or way to the next image.',
      mock: () => viewerFrame(`<div style="display:flex;flex-direction:column;align-items:flex-end;gap:8px">${closeBtn}<div style="line-height:0;border-radius:8px;overflow:hidden">${shot(680)}</div><div style="align-self:center;font-size:12px;color:rgba(255,255,255,.8)">az-linux-rel.png · 1280 × 800</div></div>`) },
    { key: 'C', name: 'As it is', from: 't3code, in part (today)',
      desc: 'The image as large as the window allows, with a close button.',
      good: 'Nothing to change.', cost: 'No zoom, no next image, no name.',
      mock: () => viewerFrame(`<div style="display:flex;flex-direction:column;align-items:flex-end;gap:8px">${closeBtn}<div style="line-height:0;border-radius:8px;overflow:hidden">${shot(700)}</div></div>`) },
  ],
});

// 7. agentZ's own tools --------------------------------------------------------------------
const Z = ic('agentz', 'sm');
const spin = `<span class="spin" style="border-color:rgba(141,142,145,.3);border-top-color:${DIM}"></span>`;
const openLink = link(`Open${ic('external', 'xs')}`);
const ownRows = () => [
  trow(Z, subj('Listed agents and models')),
  ...SUBTHREADS.map((title) => trow(Z, verb('Started a subthread:') + bright(title), { trailing: openLink })),
  trow(Z, verb('Started a terminal:') + codeSubj('npm run dev'), { trailing: openLink }),
  trow(Z, verb('Waited for') + bright(SUBTHREADS[0])),
  trow(Z, verb('Starting a subthread:') + bright('Research: what Zed’s subagent card shows'), { trailing: spin }),
];
const ownFolded = () => `<div class="row" style="min-height:24px;gap:6px;padding:0 2px"><span style="width:24px;display:inline-flex;justify-content:center;color:var(--ph)">${ic('chev-right', 'xs')}</span><span class="grow trunc" style="font-size:13px;color:${DIM}">Started 3 subthreads, started a terminal, and performed 2 other actions</span></div>`;
// t3code's SubagentTimelineLink: the agent's icon with a status dot, the title over its status
// or result, the time, and a chevron; the whole row opens the subthread.
const subCard = (title, status, statusColor, detail, time, dotColor) => `<div class="row" style="gap:10px;padding:6px 8px;border-radius:6px;margin-left:28px">
  <span style="position:relative;width:24px;height:24px;border-radius:50%;border:1px solid #4a4d54;background:#2f3134;display:grid;place-items:center;flex:none;font-size:11px;color:#dfe1e5">✻<i style="position:absolute;right:-1px;bottom:-1px;width:8px;height:8px;border-radius:50%;background:${dotColor};box-shadow:0 0 0 2px var(--panel)"></i></span>
  <span class="col grow" style="min-width:0;gap:1px"><span class="row" style="gap:8px"><span class="trunc" style="font-size:12px;font-weight:500;color:var(--t)">${title}</span></span><span class="trunc" style="font-size:11px;color:${statusColor}">${status}${detail ? ` · <span style="color:var(--ph)">${detail}</span>` : ''}</span></span>
  <span style="font-size:12px;color:var(--ph)">${time}</span><span style="color:var(--ph);display:inline-flex">${ic('chev-right', 'xs')}</span></div>`;
TOPICS.push({
  id: 'own-tools', section: 'agentZ’s tools and other tools', title: 'agentZ’s own tools, as what they did', size: 'wide', rec: 'A',
  now: 'A call to one of agentZ’s tools shows the name the agent sends, with the MCP server’s prefix: <code>agentz___delegate_task</code> from Factory Droid. The hammer icon, nothing about what it did, and opening it shows the JSON input and output. The subthreads it started are only in the Agents list above the composer.',
  nowImg: '../feedback/evidence/13-delegate-task-calls.png',
  issues: ['Three calls read "agentz___delegate_task" three times, with the prefix and triple underscore.', 'Nothing names the subthread each one started, or opens it.', 'The hammer is the icon for any unknown tool.'],
  options: [
    { key: 'A', name: 'A line that says what it did, with Open', from: 't3code (its own tools’ labels and "Open chat")',
      desc: 'Each of agentZ’s tools gets a sentence in the past tense with what it acted on, and the agentZ mark: "Started a subthread: &lt;title&gt;", "Started a terminal: npm run dev", "Waited for &lt;title&gt;", "Listed agents and models". While it runs, the present tense and a spinner ("Starting a subthread…"). Rows that made a subthread, a thread or a terminal end in "Open", which shows it. A folded run counts them too ("Started 3 subthreads").',
      good: 'Reads like what happened, and each subthread is one click away.', cost: 'agentZ keeps a sentence for each of its tools.',
      mock: () => pairT(tframe([userAsk, ownFolded()].join(''), 124), tframe([userAsk, ...ownRows()].join(''), 296)) },
    { key: 'B', name: 'Subthreads as cards, other tools as lines', from: 't3code (its subagent rows) and Zed (its subagent card)',
      desc: 'A started subthread shows as t3code’s subagent row: the agent’s icon with a status dot, the title in the text color over its status or its result’s first line, how long it ran, and a chevron. The whole row opens the subthread. agentZ’s other tools are A’s lines. These rows should match the Agents list, which the subthreads round redesigns.',
      good: 'You see each subthread’s progress where it started.', cost: 'Two styles of row in one run; must stay in step with the Agents list.',
      mock: () => tframe([userAsk, trow(Z, subj('Listed agents and models')), trow(Z, subj('Started 3 subthreads')),
        subCard(SUBTHREADS[0], 'Done', 'var(--ok)', 'The Agents list shows each child’s title, agent and status', '2m 14s', 'var(--ok)'),
        subCard(SUBTHREADS[1], 'Working', 'var(--ac)', 'Reading crates/agentz_server/src/mcp_bridge.rs', '1m 02s', 'var(--ac)'),
        subCard(SUBTHREADS[2], 'Waiting', 'var(--warn)', 'Allow command? cargo test -p agentz_server', '48s', 'var(--warn)'),
        trow(Z, verb('Started a terminal:') + codeSubj('npm run dev'), { trailing: openLink })].join(''), 316) },
    { key: 'C', name: 'The tool’s own title', from: 't3code (its tool names)',
      desc: 'Each call shows its tool’s title in the past tense, without what it acted on: "Delegated a child task", "Started an agentZ terminal", with the agentZ mark. No Open.',
      good: 'Simple, and the same words for every agent.', cost: 'Three subthreads read the same three times.',
      mock: () => tframe([userAsk, trow(Z, subj('Got orchestration capabilities')), ...SUBTHREADS.map(() => trow(Z, subj('Delegated a child task'))), trow(Z, subj('Started an agentZ terminal'))].join(''), 226) },
    { key: 'D', name: 'As it is', from: 'Zed (today)',
      desc: 'The name the agent sends, with the hammer.',
      good: 'Nothing to change.', cost: 'Raw and repetitive.',
      mock: () => tframe([userAsk, trow(ic('zed-hammer', 'sm'), subj('agentz___orchestrator_capabilities')), ...SUBTHREADS.map(() => trow(ic('zed-hammer', 'sm'), subj('agentz___delegate_task'))), trow(ic('zed-hammer', 'sm'), subj('agentz___agentz_terminal_start'))].join(''), 226) },
  ],
});

// 8. ToolSearch ----------------------------------------------------------------------------
const tsList = (items, style) => `<div style="display:flex;flex-wrap:wrap;gap:${style === 'chips' ? '4px' : '0 18px'};${style === 'chips' ? '' : 'flex-direction:column;'}">${items.map((item) => style === 'chips' ? `<span style="font:12px ${MONO};color:#c9ccd1;border:1px solid var(--b);border-radius:4px;padding:1px 5px;background:var(--ed)">${item}</span>` : `<span class="row" style="gap:6px;font-size:12px;line-height:20px;color:var(--mu)">${ic('agentz', 'xs')}${item}</span>`).join('')}</div>`;
TOPICS.push({
  id: 'toolsearch', section: 'agentZ’s tools and other tools', title: 'ToolSearch', size: 'wide', rec: 'A',
  now: 'Some agents load tools only when they need them, with a tool called ToolSearch. Its row says "ToolSearch" with a magnifying glass; opened, it shows the JSON query ("select:" and the tools’ full names) and the output ("Loaded 8 tool(s): …"). A query can also be words, which finds tools by what they do. Zed and t3code show it like any other tool.',
  nowImg: '../feedback/evidence/13-toolsearch-call.png',
  issues: ['The row says only "ToolSearch".', 'Every name carries the agentz___ prefix.', 'Both lines are cut off at the right edge.'],
  options: [
    { key: 'A', name: '"Loaded 8 tools", opening to their names in words', from: 'new',
      desc: 'The row says what it did: "Loaded 8 agentZ tools" for a "select:" query, or "Searched tools for “subthread”" for words. Opened, it lists the tools one per line by what they do (agentZ’s tools by their own titles, others as the MCP names topic picks), with no JSON.',
      good: 'Reads as a step, not raw data.', cost: 'You don’t see the exact query.',
      mock: () => tframe([trow(ic('zed-search', 'sm'), subj('Loaded 8 agentZ tools'), { chevron: 'chev-up', hover: true }), out(tsList(LOADED_TITLES)), trow(ic('zed-search', 'sm'), subj('Searched tools for “subthread”'), { trailing: dimText('2 found') })].join(''), 250) },
    { key: 'B', name: '"Loaded 8 tools", opening to their names', from: 'new',
      desc: 'The same row. Opened, the tools’ names without the prefix, as small code-font tags that wrap.',
      good: 'Exact names, still short.', cost: 'Names read like code.',
      mock: () => tframe([trow(ic('zed-search', 'sm'), subj('Loaded 8 agentZ tools'), { chevron: 'chev-up', hover: true }), out(tsList(LOADED, 'chips')), trow(ic('zed-search', 'sm'), subj('Searched tools for “subthread”'), { trailing: dimText('2 found') })].join(''), 150) },
    { key: 'C', name: 'Hidden', from: 'new',
      desc: 'ToolSearch calls don’t show in the thread, and a folded run doesn’t count them. Only a failed one shows.',
      good: 'The thread shows only work you care about.', cost: 'You can’t see which tools the agent loaded.',
      mock: () => tframe([userAsk, trow(ic('agentz', 'sm'), subj('Listed agents and models')), trow(ic('agentz', 'sm'), verb('Started a subthread:') + bright(SUBTHREADS[0]), { trailing: openLink })].join(''), 170) },
    { key: 'D', name: 'As it is, with wrapped lines', from: 'Zed',
      desc: 'The same row and JSON as today; long lines follow the long-lines topic (here wrapped).',
      good: 'Shows exactly what the agent sent and got.', cost: 'Still raw, with every prefix.',
      mock: () => tframe([trow(ic('zed-search', 'sm'), subj('ToolSearch'), { chevron: 'chev-up', hover: true }), out([codeBlock(TS_JSON, { wrap: true }), codeBlock(TS_OUTPUT, { wrap: true })].join(''))].join(''), 250) },
  ],
});

// 9. Other MCP tools' names ----------------------------------------------------------------
const MCP_CALLS = [
  ['github', 'create_issue', 'Create issue', 'github___create_issue'],
  ['linear', 'list_issues', 'List issues', 'mcp__linear__list_issues'],
  ['context7', 'get-library-docs', 'Get library docs', 'context7___get-library-docs'],
];
const tooltip = (text, style) => `<span style="position:absolute;${style};z-index:5;background:#2b2d30;border:1px solid var(--b);border-radius:6px;padding:4px 8px;font-size:12px;color:var(--t);box-shadow:0 6px 18px rgba(0,0,0,.4);white-space:nowrap">${text}</span>`;
TOPICS.push({
  id: 'mcp-names', section: 'agentZ’s tools and other tools', title: 'Other MCP tools’ names', size: 'medium', rec: 'A',
  now: 'A tool from an MCP server you added shows the name the agent sends, which joins the server’s name and the tool’s, each agent its own way: <code>github___create_issue</code> from Factory Droid, <code>mcp__linear__list_issues</code> from Claude Agent. agentZ’s own tools are decided in their topic; this is for every other MCP tool.',
  nowImg: '../feedback/evidence/13-delegate-task-calls.png',
  issues: ['The prefix and its underscores come first, so rows from one server all start alike.', 'Each agent spells the same tool differently.'],
  options: [
    { key: 'A', name: 'The tool in words, then its server', from: 't3code',
      desc: 'The tool’s name with its underscores and dashes as spaces and a capital first letter ("Create issue"), then the server’s name, dimmer ("github"), with a plug icon. Every agent’s spelling reads the same.',
      good: 'Reads like an action, and you still see where it came from.', cost: 'Turns a name like "get-library-docs" into words that may read oddly.',
      mock: () => tframe(MCP_CALLS.map(([server, , words]) => trow(ic('plug', 'sm'), subj(words) + dimText(server, 'margin-left:4px'))).join(''), 110, 520) },
    { key: 'B', name: 'The tool’s own name, the server on hover', from: 'Zed (its "Tool:" tooltip)',
      desc: 'The tool’s name without the prefix, in the code font ("create_issue"). Hovering its icon shows the server and the full name, as Zed’s icon tooltip shows "Tool: …".',
      good: 'Exact and short.', cost: 'You hover to see which server it was.',
      mock: () => tframe(`<div style="position:relative">${MCP_CALLS.map(([, tool], index) => trow(ic('plug', 'sm'), codeSubj(tool), { hover: index === 2 })).join('')}${tooltip('context7 · get-library-docs', 'left:4px;top:74px')}</div>`, 130, 520) },
    { key: 'C', name: 'As it is', from: 'Zed (today)',
      desc: 'The name the agent sends, with the hammer.',
      good: 'Exactly what the agent called.', cost: 'Raw, and different for each agent.',
      mock: () => tframe(MCP_CALLS.map(([, , , raw]) => trow(ic('zed-hammer', 'sm'), subj(raw))).join(''), 110, 520) },
  ],
});
