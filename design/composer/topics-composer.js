// The composer: more than one line, @-mentions, pasted images, and its right-click menu.

ICONS.send = '<path d="M14.54 21.69a.5.5 0 0 0 .94-.03l6.5-19a.5.5 0 0 0-.64-.64l-19 6.5a.5.5 0 0 0-.03.94l7.93 3.18a2 2 0 0 1 1.11 1.11z"/><path d="m21.85 2.15-10.94 10.94"/>';
ICONS.image = '<rect width="18" height="18" x="3" y="3" rx="2" ry="2"/><circle cx="9" cy="9" r="2"/><path d="m21 15-3.09-3.09a2 2 0 0 0-2.82 0L6 21"/>';
ICONS.thread = '<path d="M7.9 20A9 9 0 1 0 4 16.1L2 22Z"/>';
ICONS.code = '<path d="m16 18 6-6-6-6"/><path d="m8 6-6 6 6 6"/>';
ICONS.link = '<path d="M10 13a5 5 0 0 0 7.54.54l3-3a5 5 0 0 0-7.07-7.07l-1.72 1.71"/><path d="M14 11a5 5 0 0 0-7.54-.54l-3 3a5 5 0 0 0 7.07 7.07l1.71-1.71"/>';

const CM_W = 760;
const LINE = 20;
const caret = '<span style="display:inline-block;width:1.5px;height:16px;background:var(--ac);vertical-align:-3px;margin-left:1px"></span>';

// The end of the agent's reply above the composer, as in the screenshot.
const reply = () => `<div style="padding:16px 48px 0;line-height:22px">I added the checkout total and a test for it. The rounding now happens once, at the end, so a cart of many cheap items adds up to what the receipt says.</div>
  <div class="row g3 mu" style="justify-content:flex-end;padding:10px 48px 0">${ic('copy', 'sm')}${ic('undo', 'sm')}${ic('chev-up', 'sm')}</div>`;

const toggle = `<span style="width:26px;height:15px;border-radius:8px;background:var(--sel);display:inline-flex;align-items:center;padding:2px"><i style="width:11px;height:11px;border-radius:50%;background:var(--mu);display:block"></i></span>`;
const footer = ({ plus = false } = {}) => `<div class="row" style="height:26px;gap:12px;font-size:13px;color:var(--mu)">
  ${plus ? `<span class="ibtn" style="margin-right:-6px">${ic('plus', 'sm')}</span>` : ''}
  <span class="row g15">${glyph('claude')}Claude Agent</span><span class="grow"></span>
  <span class="row g1">Bypass permissions ${ic('chev-down', 'xs')}</span>
  <span class="row g1">Opus 5.5 ${ic('chev-down', 'xs')}</span>
  <span class="row g1">Xhigh ${ic('chev-down', 'xs')}</span>
  <span class="row g15">Fast mode ${toggle}</span>
  <span style="color:var(--ac);display:inline-flex">${ic('send')}</span></div>`;

const expandButton = (minimize = false) => `<span class="ibtn sm" style="position:absolute;top:2px;right:-4px;opacity:.5">${ic(minimize ? 'minimize' : 'maximize', 'xs')}</span>`;
const scrollbar = (top = 40, height = 46) => `<span style="position:absolute;right:-2px;top:${top}px;width:4px;height:${height}px;border-radius:2px;background:var(--sel)"></span>`;

/** The composer along the bottom of a conversation (agent_view.rs's Bar style). */
function composer({ body = '', rows = 1, expand = false, minimize = false, scroll = false, plus = false, above = '', menu = '', minRows } = {}) {
  const height = Math.max(rows, minRows ?? rows) * LINE;
  return `<div style="border-top:1px solid var(--b);background:var(--ed);padding:8px 0 10px;position:relative">
    <div style="margin:0 auto;width:640px;position:relative">
      ${above}
      <div style="position:relative;padding:4px 14px 0 0;line-height:${LINE}px">
        <div style="height:${height}px;overflow:hidden;white-space:pre-wrap;word-break:break-word">${body}</div>
        ${expand ? expandButton(minimize) : ''}${scroll ? scrollbar() : ''}
      </div>
      ${menu}
      <div style="margin-top:8px">${footer({ plus })}</div>
    </div></div>`;
}

const view = (html, h = 300) => frame(`<div class="col" style="height:100%"><div class="grow" style="min-height:0;overflow:hidden">${reply()}</div>${html}</div>`, { w: CM_W, h });
const label = (text) => `<div class="sm ph" style="padding:6px 2px 4px">${text}</div>`;
const pair = (left, right, w = CM_W) => `<div style="display:flex;flex-direction:column;gap:4px;width:${w}px">${left}${right}</div>`;

const SHORT = `Fix the rounding in the cart total${caret}`;
const LONG_LINES = [
  'The cart total is off by a cent on some orders. Steps:',
  '',
  '1. Add three items at $0.335 each',
  '2. Open the checkout',
  '3. The total says $1.00, the receipt says $1.01',
  '',
  'Round once at the end, like the receipt does, and add a test',
  'with many cheap items. Keep the existing tests passing.',
  'Don\'t touch the tax code.' + caret,
];
const long = (from = 0) => LONG_LINES.slice(from).join('\n');

// 1. Height -------------------------------------------------------------------------------
TOPICS.push({
  id: 'height', section: 'Composer', title: 'More than one line', size: 'wide', rec: 'A',
  now: 'One line. Text past the edge runs off it without scrolling, a pasted text\'s line breaks turn into spaces, and there\'s no way to type a new line. In every option Shift-Enter makes a new line, Enter sends, pasted text keeps its line breaks, and Up/Down move between lines (Zed\'s message editor). This topic is only how tall the box is.',
  nowImg: 'img/now-composer.png',
  issues: ['Your long draft ran off the right edge', 'Pasted text becomes one line'],
  options: [
    {
      key: 'A', name: 'Four lines tall, grows to eight, Expand', from: 'Zed agent panel',
      desc: 'Always at least 4 lines tall; grows with the text to 8 lines, then scrolls. A faded Expand button in its top right corner makes it 80% of the view\'s height for a long prompt (Zed\'s <code>message_editor_min_lines</code> 4, max twice that).',
      good: 'Exactly Zed\'s; room to write without it jumping.', cost: 'Takes 4 lines even for "yes".',
      mock: () => pair(label('Short') + view(composer({ body: SHORT, rows: 1, minRows: 4, expand: true }), 300), label('Long: 8 lines, then it scrolls') + view(composer({ body: long(), rows: 8, expand: true, scroll: true }), 400)),
    },
    {
      key: 'B', name: 'One line, grows to eight, Expand', from: 'Zed\'s editor with a 1-line minimum',
      desc: 'Starts one line tall as today and grows with the text to 8 lines, then scrolls. The same Expand button.',
      good: 'As compact as today until you write more.', cost: 'The conversation above shifts as it grows.',
      mock: () => pair(label('Short') + view(composer({ body: SHORT, rows: 1, expand: true }), 240), label('Long: 8 lines, then it scrolls') + view(composer({ body: long(), rows: 8, expand: true, scroll: true }), 400)),
    },
    {
      key: 'C', name: 'Three lines tall, grows to eight', from: 't3code composer',
      desc: 'At least about 3 lines (t3code\'s 78px) and at most about 8 (208px), then it scrolls. No Expand button.',
      good: 'A middle ground; one control fewer.', cost: 'A very long prompt stays in an 8-line window.',
      mock: () => pair(label('Short') + view(composer({ body: SHORT, rows: 1, minRows: 3 }), 280), label('Long: 8 lines, then it scrolls') + view(composer({ body: long(), rows: 8, scroll: true }), 400)),
    },
    {
      key: 'D', name: 'One line, grows to eight', from: 'new',
      desc: 'Like B without the Expand button.',
      good: 'Least to look at.', cost: 'No way to see a long prompt whole.',
      mock: () => pair(label('Short') + view(composer({ body: SHORT, rows: 1 }), 240), label('Long: 8 lines, then it scrolls') + view(composer({ body: long(), rows: 8, scroll: true }), 400)),
    },
  ],
});

// 2. What @ adds ----------------------------------------------------------------------------
TOPICS.push({
  id: 'sources', section: '@-mentions', title: 'What @ can add', size: 'medium', type: 'multi', rec: ['A', 'B'],
  now: 'Nothing: @ is just a character. Zed\'s @ adds files, folders, symbols, threads, rules and fetched pages; t3code\'s adds files and folders. agentZ has no language servers, so no symbols. Pick any.',
  options: [
    {
      key: 'A', name: 'Files and folders', from: 'Zed, t3code',
      desc: 'Files and folders of the folder the thread works in, found by name on its machine (local or SSH). A file goes to the agent as its contents (ACP embedded context) or as a link when the agent doesn\'t take those; a folder as a link.',
      good: 'The common case: "look at @total.ts".', cost: 'Searching a big repository needs an index on the server.',
      mock: () => frame(mentionMenu({ query: 'tot', items: FILE_ITEMS }), { w: 360, h: 220 }),
    },
    {
      key: 'B', name: 'Threads', from: 'Zed',
      desc: 'Another thread of the project: its conversation goes along as a transcript, as Continue with another agent sends one.',
      good: 'Carries what another thread found without copying it.', cost: 'A long thread is a lot of context.',
      mock: () => frame(mentionMenu({ query: 'login', items: THREAD_ITEMS }), { w: 360, h: 220 }),
    },
    {
      key: 'C', name: 'Fetched pages', from: 'Zed',
      desc: 'Paste a URL after @ and the page goes along as text, fetched by agentZ.',
      good: 'Docs pages without the agent fetching them.', cost: 'Many agents fetch pages themselves already.',
      mock: () => frame(mentionMenu({ query: 'https://docs.stripe.com/api/refunds', items: [{ icon: 'link', name: 'Fetch docs.stripe.com/api/refunds', path: '' }] }), { w: 360, h: 220 }),
    },
    {
      key: 'D', name: 'Terminal output', from: 't3code terminal contexts',
      desc: 'A terminal of the thread (its drawer\'s, or a terminal thread\'s): what\'s on its screen goes along as text.',
      good: '"Why did @npm test fail?" without copying the output.', cost: 'Only the screen and recent scrollback.',
      mock: () => frame(mentionMenu({ query: 'test', items: [{ icon: 'terminal', name: 'npm test', path: 'Terminal 1 · drawer' }, { icon: 'terminal', name: 'zsh', path: 'Terminal 2 · drawer' }] }), { w: 360, h: 220 }),
    },
  ],
});

const FILE_ITEMS = [
  { icon: 'file', name: 'total.ts', path: 'src/cart' },
  { icon: 'file', name: 'total.test.ts', path: 'src/cart' },
  { icon: 'folder', name: 'totals', path: 'src/reports' },
];
const THREAD_ITEMS = [
  { icon: 'thread', name: 'Fix flaky login test', path: '33m ago' },
  { icon: 'thread', name: 'Login page copy', path: 'yesterday' },
];

/** A completion list, as agentZ's slash-command menu draws one. */
function mentionMenu({ query, items, selected = 0, kinds = false, style = 'left:10px;bottom:10px;width:330px' }) {
  const rows = items.map((item, index) => `<div class="it ${index === selected ? 'hl' : ''}" style="height:30px">${ic(item.icon, 'sm')}<span class="trunc">${item.name}</span><span class="sm ph trunc" style="margin-left:auto;padding-left:10px">${item.path}</span></div>`).join('');
  const kindRows = kinds ? `<div class="hr"></div>${KINDS.map((kind) => `<div class="it">${ic(kind.icon, 'sm')}<span>${kind.name}</span></div>`).join('')}` : '';
  return `<div class="m" style="position:absolute;inset:0;background:var(--ed)"><div style="position:absolute;left:10px;bottom:40px" class="sm ph">@${query}${caret}</div>
    <div class="menu" style="${style}">${rows}${kindRows}</div></div>`;
}
const KINDS = [
  { icon: 'file', name: 'Files & Directories' },
  { icon: 'thread', name: 'Threads' },
];

// 3. The @ menu ---------------------------------------------------------------------------
const typed = (text) => `Look at ${text}${caret}`;
TOPICS.push({
  id: 'menu', section: '@-mentions', title: 'The @ menu', size: 'wide', rec: 'A',
  now: 'Typing / opens the slash-command menu: above the composer\'s left edge, 26rem wide, a name and a description per row; Up/Down, Tab or Enter to pick, Escape to close. Nothing opens for @.',
  options: [
    {
      key: 'A', name: 'At the cursor, kinds to narrow by', from: 'Zed message editor completions',
      desc: '@ opens a list at the cursor: recent files and threads first, then the kinds (Files & Directories, Threads) to narrow to one. Typing searches them all. Enter or Tab inserts the mention.',
      good: 'Zed\'s; the list sits by what you\'re typing.', cost: 'Jumps around with the cursor.',
      mock: () => view(composer({ body: typed('@'), rows: 1, minRows: 4, expand: true, menu: `<div class="menu" style="left:56px;bottom:124px;width:330px">${FILE_ITEMS.slice(0, 2).map((item, index) => `<div class="it ${index === 0 ? 'hl' : ''}" style="height:30px">${ic(item.icon, 'sm')}<span class="trunc">${item.name}</span><span class="sm ph" style="margin-left:auto;padding-left:10px">${item.path}</span></div>`).join('')}<div class="it" style="height:30px">${ic('thread', 'sm')}<span class="trunc">Fix flaky login test</span><span class="sm ph" style="margin-left:auto;padding-left:10px">33m ago</span></div><div class="hr"></div>${KINDS.map((kind) => `<div class="it">${ic(kind.icon, 'sm')}<span>${kind.name}</span>${ic('chev-right', 'xs')}</div>`).join('')}</div>` }), 380),
    },
    {
      key: 'B', name: 'Matches right away, grouped', from: 't3code composer menu',
      desc: '@ opens a list of matches at once, grouped under Files and Threads, the path dimmed beside each name. No kinds to pick first.',
      good: 'One step fewer.', cost: 'A long list when nothing\'s typed yet.',
      mock: () => view(composer({ body: typed('@tot'), rows: 1, minRows: 4, expand: true, menu: `<div class="menu" style="left:0;bottom:124px;width:360px"><div class="lbl">Files</div>${FILE_ITEMS.map((item, index) => `<div class="it ${index === 0 ? 'hl' : ''}" style="height:28px">${ic(item.icon, 'sm')}<span>${item.name}</span><span class="sm ph" style="margin-left:auto">${item.path}</span></div>`).join('')}<div class="lbl">Threads</div><div class="it" style="height:28px">${ic('thread', 'sm')}<span>Order totals report</span><span class="sm ph" style="margin-left:auto">2d ago</span></div></div>` }), 380),
    },
    {
      key: 'C', name: 'Where the slash menu is', from: 'agentZ slash-command menu',
      desc: 'The same place and rows as the slash-command menu: above the composer\'s left edge, 26rem wide, the name in the code font and its folder or time beneath. Matches right away.',
      good: 'One menu style for / and @.', cost: 'Far from the cursor on a long line.',
      mock: () => view(composer({ body: typed('@tot'), rows: 1, minRows: 4, expand: true, menu: `<div class="menu" style="left:0;bottom:124px;width:364px">${FILE_ITEMS.map((item, index) => `<div class="it ${index === 0 ? 'hl' : ''}" style="height:42px;flex-direction:column;align-items:flex-start;justify-content:center;gap:0"><span class="mono-font" style="font-size:13px">@${item.name}</span><span class="sm mu">${item.path}</span></div>`).join('')}</div>` }), 380),
    },
  ],
});

// 4. Mentions in the text ---------------------------------------------------------------------
const zedChip = (icon, name) => `<span style="display:inline-flex;align-items:center;gap:4px;height:19px;padding:0 5px;border:1px solid var(--b);border-radius:4px;vertical-align:-4px;font:12px 'IBM Plex Mono',monospace">${ic(icon, 'xs')}${name}</span>`;
const tintChip = (icon, name) => `<span style="display:inline-flex;align-items:center;gap:4px;height:19px;padding:0 6px;border-radius:4px;background:rgba(116,173,232,.16);color:var(--ac);vertical-align:-4px;font-size:13px">${ic(icon, 'xs')}${name}</span>`;
const plainMention = (path) => `<span style="color:var(--ac)">@${path}</span>`;
const mentionBody = (chip) => `Round once in ${chip('file', 'total.ts')}, like ${chip('thread', 'Fix flaky login test')} did for dates, and add a case to ${chip('file', 'total.test.ts')}${caret}`;
TOPICS.push({
  id: 'chips', section: '@-mentions', title: 'A mention in the text', size: 'wide', rec: 'A',
  now: 'There are no mentions yet. Once picked, a mention stays one piece in the text: Backspace removes it whole, and the agent gets the file or thread beside the message.',
  options: [
    {
      key: 'A', name: 'Outlined chip', from: 'Zed MentionCrease',
      desc: 'An outlined chip with the kind\'s icon and the name in the code font, a line tall. Hovering shows the full path.',
      good: 'Zed\'s; quiet next to the text.', cost: 'Code font in the middle of prose.',
      mock: () => view(composer({ body: mentionBody(zedChip), rows: 2, minRows: 4, expand: true }), 300),
    },
    {
      key: 'B', name: 'Tinted chip', from: 't3code inline chip',
      desc: 'A chip tinted with the accent color, the icon and name in the text\'s font.',
      good: 'Stands out as something attached.', cost: 'Louder; several turn the prompt blue.',
      mock: () => view(composer({ body: mentionBody(tintChip), rows: 2, minRows: 4, expand: true }), 300),
    },
    {
      key: 'C', name: 'Colored @path text', from: 'Claude Code',
      desc: 'No chip: the path as text after @, in the accent color. Still removed whole.',
      good: 'Reads as typed; shows the folder too.', cost: 'Long paths take room; less clearly attached.',
      mock: () => view(composer({ body: `Round once in ${plainMention('src/cart/total.ts')}, like ${plainMention('thread:Fix flaky login test')} did for dates, and add a case to ${plainMention('src/cart/total.test.ts')}${caret}`, rows: 2, minRows: 4, expand: true }), 300),
    },
  ],
});

// 5. The Add Context button ---------------------------------------------------------------
const addContextMenu = (bottom = 46) => `<div class="menu" style="left:-4px;bottom:${bottom}px;width:210px">
  <div class="it">${ic('file', 'sm')}Files &amp; Directories</div>
  <div class="it">${ic('thread', 'sm')}Threads</div>
  <div class="it">${ic('image', 'sm')}Image</div></div>`;
TOPICS.push({
  id: 'add', section: '@-mentions', title: 'Adding without typing @', size: 'wide', rec: 'A',
  now: 'The composer\'s footer starts with the agent\'s icon and name.',
  options: [
    {
      key: 'A', name: 'A + button with a menu', from: 'Zed Add Context',
      desc: 'A + button at the start of the footer (tooltip "Add Context") opens Files &amp; Directories, Threads and Image. The first two type @ and narrow the list to that kind; Image opens a file picker. Image is greyed out when the agent can\'t take images.',
      good: 'Zed\'s; findable without knowing @.', cost: 'One more button in the footer.',
      mock: () => view(composer({ body: SHORT, rows: 1, minRows: 4, expand: true, plus: true, menu: addContextMenu() }), 320),
    },
    {
      key: 'B', name: 'Only @ and pasting', from: 'new',
      desc: 'No button: type @, paste an image, or drop a file.',
      good: 'The footer stays as it is.', cost: 'Nothing tells you @ exists.',
      mock: () => view(composer({ body: SHORT, rows: 1, minRows: 4, expand: true }), 280),
    },
  ],
});

// 6. Pasted images --------------------------------------------------------------------------
const thumb = (size, hue, x = false) => `<span style="position:relative;display:inline-block;width:${size}px;height:${size}px;border-radius:${size > 40 ? 8 : 6}px;border:1px solid var(--b);overflow:hidden;background:linear-gradient(135deg,hsl(${hue},35%,42%),hsl(${hue + 40},30%,26%))">
  <span style="position:absolute;left:18%;right:18%;top:22%;height:10%;background:rgba(255,255,255,.55);border-radius:2px"></span>
  <span style="position:absolute;left:18%;right:38%;top:42%;height:8%;background:rgba(255,255,255,.35);border-radius:2px"></span>
  ${x ? `<span style="position:absolute;top:3px;right:3px;width:16px;height:16px;border-radius:50%;background:rgba(0,0,0,.6);display:grid;place-items:center;color:#fff">${ic('x', 'xs')}</span>` : ''}</span>`;
const IMAGE_TEXT = 'The total on this receipt is off by a cent. Find where we round.';
TOPICS.push({
  id: 'images', section: 'Images', title: 'A pasted image', size: 'wide', rec: 'A',
  now: 'Pasting an image does nothing. Every option sends images only to agents that say they take them (ACP\'s prompt capabilities); for the others, pasting an image does nothing, as in Zed. Dropping an image file works like pasting it.',
  options: [
    {
      key: 'A', name: 'An Image chip in the text', from: 'Zed message editor',
      desc: 'Pasting puts an "Image" chip where the cursor is, like a mention (Zed\'s crease). Hovering shows the picture; Backspace removes it.',
      good: 'Zed\'s; the image sits where you mean it.', cost: 'You don\'t see the picture until you hover.',
      mock: () => view(composer({ body: `${IMAGE_TEXT} ${zedChip('image', 'Image')}${caret}`, rows: 2, minRows: 4, expand: true, menu: `<div class="pop" style="left:250px;bottom:124px;padding:4px">${thumb(150, 205)}</div>` }), 380),
    },
    {
      key: 'B', name: 'Thumbnails above the text', from: 't3code composer',
      desc: '64px thumbnails in a row above the text, each with × on hover; a click shows it large.',
      good: 'You see what you attached.', cost: 'Pushes the text down; separate from where you mention it.',
      mock: () => view(composer({ body: `${IMAGE_TEXT}${caret}`, rows: 1, minRows: 4, expand: true, above: `<div class="row g2" style="padding:4px 0 6px">${thumb(64, 205, true)}${thumb(64, 25)}</div>` }), 380),
    },
    {
      key: 'C', name: 'Small thumbnails in the footer', from: 't3code resting composer',
      desc: '28px thumbnails at the start of the footer, before the agent\'s name; a click shows it large, × on hover.',
      good: 'The text area stays as tall as before.', cost: 'Small; crowds the footer.',
      mock: () => view(composer({ body: `${IMAGE_TEXT}${caret}`, rows: 1, minRows: 4, expand: true }).replace('<span class="row g15">', `<span class="row g1">${thumb(28, 205)}${thumb(28, 25)}</span><span class="row g15">`), 300),
    },
  ],
});

// 7. Right-click ------------------------------------------------------------------------------
const editMenu = `<div class="menu" style="left:210px;bottom:52px;width:200px">
  <div class="it">Cut</div><div class="it">Copy</div><div class="it">Paste</div><div class="it">Paste as Plain Text</div></div>`;
TOPICS.push({
  id: 'edit', section: 'Composer', title: 'Right-clicking the composer', size: 'medium', rec: 'A',
  now: 'Nothing opens. Cmd-X, Cmd-C, Cmd-V and Cmd-A are the only way to cut, copy and paste.',
  options: [
    {
      key: 'A', name: 'Cut, Copy, Paste, Paste as Plain Text', from: 'Zed message editor',
      desc: 'Zed\'s menu for its message editor. Copy is greyed out with nothing selected. Paste as Plain Text pastes without turning paths into mentions or images into chips.',
      good: 'Zed\'s, word for word.', cost: 'None really.',
      mock: () => frame(`<div class="col" style="height:100%"><div class="grow"></div>${composer({ body: `Round once in the cart <span style="background:var(--bsel)">total and add a test</span>${caret}`, rows: 1, minRows: 4, expand: true, menu: editMenu })}</div>`, { w: CM_W, h: 260 }),
    },
    {
      key: 'B', name: 'No menu', from: 'today',
      desc: 'Keys only, as today.',
      good: 'Nothing to build.', cost: 'Nothing to find by mouse.',
      mock: () => frame(`<div class="col" style="height:100%"><div class="grow"></div>${composer({ body: `Round once in the cart <span style="background:var(--bsel)">total and add a test</span>${caret}`, rows: 1, minRows: 4, expand: true })}</div>`, { w: CM_W, h: 260 }),
    },
  ],
});
