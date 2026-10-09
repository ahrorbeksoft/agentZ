// agentZ's own tools as they show in a thread (their icons, words, running and failed rows,
// what opening one shows, links and folded runs), subthreads where they started, agents' own
// subagents, and what the parent shows when its delegated tasks end.

// Every one of agentZ's tools (AGENTZ_TOOLS), by group, as OwnTool::sentence words it once done.
// `subject` styles: t a thread's title (brighter), c code, p plain. `result` is what came back.
const GROUPS = [
  { id: 'threads', name: 'Threads', icon: 'chat' },
  { id: 'tasks', name: 'Subthreads and delegated tasks', icon: 'bot' },
  { id: 'workspaces', name: 'Workspaces', icon: 'branch' },
  { id: 'terminals', name: 'Terminals and commands', icon: 'terminal' },
  { id: 'projects', name: 'Projects', icon: 'folder' },
  { id: 'capabilities', name: 'Agents and models', icon: 'list' },
];
const TOOLS = [
  { g: 'threads', title: 'List agentZ threads', verb: 'Listed threads', result: '8 threads', looks: true },
  { g: 'threads', title: 'Read an agentZ thread', verb: 'Read', subject: [OTHER_THREAD, 't'], result: '24 messages', looks: true },
  { g: 'threads', title: 'Launch an agentZ thread', verb: 'Started a thread:', subject: [OTHER_THREAD, 't'], opens: true },
  { g: 'threads', title: 'Create agentZ threads', verb: 'Started 3 threads', result: 'Fix flaky login test, Checkout flow review, 1 more' },
  { g: 'threads', title: 'Send to an agentZ thread', verb: 'Sent a message to', subject: [OTHER_THREAD, 't'] },
  { g: 'threads', title: 'Wait for an agentZ thread', verb: 'Waited for', subject: [OTHER_THREAD, 't'], result: 'Done in 2m 14s', looks: true },
  { g: 'threads', title: 'Interrupt an agentZ thread', verb: 'Interrupted', subject: [OTHER_THREAD, 't'] },
  { g: 'threads', title: 'Rename an agentZ thread', verb: 'Renamed this thread to', subject: ['Checkout flow review', 't'] },
  { g: 'threads', title: 'Organize an agentZ thread', verb: 'Pinned', subject: [OTHER_THREAD, 't'] },
  { g: 'threads', title: 'Read an agentZ thread’s changes', verb: 'Read the changes in', subject: [OTHER_THREAD, 't'], stat: [12, 3], looks: true },
  { g: 'tasks', title: 'Delegate a child task', verb: 'Started a subthread:', subject: [SUBS[0].title, 't'], opens: true },
  { g: 'tasks', title: 'Get delegated task status', verb: 'Checked on', subject: [SUBS[0].title, 't'], result: 'Working', looks: true },
  { g: 'tasks', title: 'Cancel delegated task', verb: 'Cancelled', subject: [SUBS[2].title, 't'] },
  { g: 'workspaces', title: 'Get this thread’s workspace', verb: 'Checked this thread’s workspace', result: 'Pasture, 2 commits ahead', looks: true },
  { g: 'workspaces', title: 'List branches and workspaces', verb: 'Listed branches and workspaces', result: '6 branches, 2 workspaces', looks: true },
  { g: 'workspaces', title: 'Hand off this thread to a new workspace', verb: 'Handed off to a new pasture:', subject: ['agentz/checkout-review', 'c'] },
  { g: 'workspaces', title: 'Sync this pasture from the project', verb: 'Synced this pasture from the project', result: '3 new commits' },
  { g: 'workspaces', title: 'Bring this pasture’s branch to the project', verb: 'Brought this pasture’s branch to the project', result: '2 commits' },
  { g: 'terminals', title: 'List agentZ terminals', verb: 'Listed terminals', result: '3 terminals', looks: true },
  { g: 'terminals', title: 'Start an agentZ terminal', verb: 'Started a terminal:', subject: ['npm run dev', 'c'], opens: true },
  { g: 'terminals', title: 'Type into an agentZ terminal', verb: 'Typed into', subject: ['this thread’s terminal', 'p'], typed: 'npm test ⏎' },
  { g: 'terminals', title: 'Read an agentZ terminal', verb: 'Read', subject: ['this thread’s terminal', 'p'], result: '40 lines', looks: true },
  { g: 'terminals', title: 'Wait for an agentZ terminal', verb: 'Waited for', subject: ['this thread’s terminal', 'p'], result: 'Exit 0', looks: true },
  { g: 'terminals', title: 'Run a command', verb: 'Ran', subject: ['cargo test -p app', 'c'], result: 'Exit 0 · 14s' },
  { g: 'projects', title: 'Add an agentZ project', verb: 'Added a project:', subject: ['~/projects/storefront', 'c'], result: 'storefront' },
  { g: 'capabilities', title: 'Get orchestration capabilities', verb: 'Listed agents and models', result: '4 agents', looks: true },
];
const subjectHtml = ([text, style]) => (style === 't' ? zTitle(text) : style === 'c' ? zCode(text) : zText(text));
const sentence = (tool) => (tool.subject ? zVerb(tool.verb) + subjectHtml(tool.subject) : zText(tool.verb));
const resultHtml = (tool) => (tool.stat ? zStat(...tool.stat) : tool.result ? zDim(tool.result, 'max-width:260px;overflow:hidden;text-overflow:ellipsis;white-space:nowrap') : '');
const groupIcon = (group) => ic(GROUPS.find((g) => g.id === group).icon, 'sm');
const markIcon = () => ic('agentz', 'sm');
// Every tool, grouped under board captions, with a label and what trails it.
function allTools({ label = sentence, trailing = (tool) => (tool.opens ? zOpen() : ''), icon = markIcon, keep = () => true } = {}) {
  return GROUPS.map((group) => {
    const rows = TOOLS.filter((tool) => tool.g === group.id && keep(tool));
    if (!rows.length) return '';
    return zCaption(group.name) + rows.map((tool) => zRow(icon(tool.g), label(tool), { trailing: trailing(tool) })).join('');
  }).join('');
}
const toolsHeight = (keep = () => true) => 36 + GROUPS.filter((group) => TOOLS.some((tool) => tool.g === group.id && keep(tool))).length * 28 + TOOLS.filter(keep).length * 26;

// 1. Icons ------------------------------------------------------------------------------------
const ICON_SAMPLE = [10, 5, 15, 19, 23, 24, 25].map((index) => TOOLS[index]);
// A group's icon with the agentZ mark in its corner.
const badged = (name) => `<span style="position:relative;display:inline-flex">${ic(name, 'sm')}<span style="position:absolute;right:-5px;bottom:-4px;width:10px;height:10px;border-radius:3px;background:var(--panel);display:grid;place-items:center"><svg viewBox="0 0 24 24" style="width:9px;height:9px;stroke:currentColor;fill:none;stroke-width:3;stroke-linecap:round;stroke-linejoin:round"><path d="M7 7h10L7 17h10"/></svg></span></span>`;
const ownKinds = { terminals: 'terminal' };
function iconRows(icon) {
  return [
    ...ICON_SAMPLE.map((tool) => zRow(icon(tool), sentence(tool), { trailing: tool.opens ? zOpen() : '' })),
    zCaption('The agent’s own tools, for comparison'),
    zRow(ic('file', 'sm'), zText('Read src/checkout/total.ts')),
    zRow(ic('terminal', 'sm'), zVerb('Ran') + zCode('npm test')),
  ].join('');
}
TOPICS.push({
  id: 'icons', section: 'agentZ’s tools: the row', title: 'Icons', size: 'medium', rec: 'B',
  now: 'Every one of agentZ’s 26 tools has the agentZ mark (a Z in a square), whatever it did, as the Tool calls round picked. An agent’s own tools get the icon of their kind or their file.',
  nowImg: 'img/now-own-rows.png',
  issues: ['A run of agentZ’s tools is a column of the same mark: a subthread, a terminal and a workspace look alike until you read them.'],
  options: [
    { key: 'A', name: 'The agentZ mark for all', from: 'Tool calls round (t3code marks its own tools with its wordmark)',
      desc: 'As today: the mark on every one of agentZ’s tools, so they read apart from the agent’s own.',
      good: 'You see at once which steps went through agentZ.', cost: 'Nothing tells the groups apart.',
      mock: () => zConv(iconRows(markIcon), 300, 520) },
    { key: 'B', name: 'An icon for each group', from: 'Zed (an icon for each kind of tool)',
      desc: 'Threads a speech bubble, subthreads and delegated tasks a bot (the icon subagents have), workspaces a branch, terminals and commands a terminal, projects a folder, agents and models a list.',
      good: 'A subthread, a terminal and a workspace look different at a glance.', cost: 'agentZ’s tools no longer stand out from the agent’s own: a command looks like one the agent ran itself.',
      mock: () => zConv(iconRows((tool) => groupIcon(tool.g)), 300, 520) },
    { key: 'C', name: 'The group’s icon with the agentZ mark', from: 'new',
      desc: 'B’s icons, each with a small agentZ mark in its bottom-right corner.',
      good: 'Both: what kind of step, and that agentZ did it.', cost: 'Busier icons at 14 px; the mark is small enough to miss.',
      mock: () => zConv(iconRows((tool) => badged(GROUPS.find((g) => g.id === tool.g).icon)), 300, 520) },
    { key: 'D', name: 'Like the agent’s own of the same kind', from: 'Zed (icons by ACP’s kinds)',
      desc: 'Where an agentZ tool does what an agent’s own tool does, it gets that icon: commands and terminals a terminal. The rest keep the agentZ mark.',
      good: 'A command reads as a command, whoever ran it.', cost: 'Two icon schemes in one list.',
      mock: () => zConv(iconRows((tool) => (ownKinds[tool.g] ? ic(ownKinds[tool.g], 'sm') : markIcon())), 300, 520) },
  ],
});

// 2. Words ------------------------------------------------------------------------------------
const tagWord = (text) => `<span class="none" style="font-size:10px;font-weight:600;letter-spacing:.4px;text-transform:uppercase;color:var(--ph);width:74px">${text}</span>`;
const GROUP_WORD = { threads: 'Thread', tasks: 'Subthread', workspaces: 'Workspace', terminals: 'Terminal', projects: 'Project', capabilities: 'agentZ' };
TOPICS.push({
  id: 'words', section: 'agentZ’s tools: the row', title: 'What each row says', size: 'wide', rec: 'B',
  now: 'Each tool says what it did, in the past tense, with what it acted on (a thread by its title now, code in the code font), as the Tool calls round picked: “Started a subthread: …”, “Waited for …”, “Listed threads”. What came back shows only when the row is opened. Below, every one of the 26 tools, in its group; the group names are the board’s, not the app’s.',
  nowImg: 'img/now-own-rows.png',
  issues: ['A row says what was asked, not what came of it: “Waited for …” doesn’t say whether it finished, “Listed threads” not how many.', '“Typed into this thread’s terminal” doesn’t say what was typed.'],
  options: [
    { key: 'A', name: 'As today', from: 'Tool calls round (t3code’s own tools’ labels)',
      desc: 'The sentence alone. Rows that made a thread, a subthread or a terminal end in Open.',
      good: 'Short; nothing to change.', cost: 'You open a row to learn how it went.',
      mock: () => zConv(allTools(), toolsHeight()) },
    { key: 'B', name: 'The sentence, then what came back', from: 't3code (a work row’s detail after its label), Zed (an edit’s +/− at its end)',
      desc: 'After the sentence, dimmer, what came of it: “8 threads”, “Done in 2m 14s”, “Working”, “Exit 0 · 14s”, “+12 −3”. A terminal’s row says what was typed. Tools that only made something keep Open.',
      good: 'Most rows no longer need opening.', cost: 'agentZ keeps a short result for each tool, and long rows truncate sooner.',
      mock: () => zConv(allTools({
        label: (tool) => (tool.typed ? zVerb('Typed') + zCode(tool.typed) + zText('into this thread’s terminal') : sentence(tool)),
        trailing: (tool) => (tool.opens ? zOpen() : resultHtml(tool)),
      }), toolsHeight()) },
    { key: 'C', name: 'The tool’s own title', from: 'agentZ’s tool titles (what agents see)',
      desc: 'The title agentZ gives the tool, as agents see it (“Delegate a child task”, “Wait for an agentZ thread”), then what it acted on, brighter.',
      good: 'The same words as the tool list in Settings › MCP Servers.', cost: 'Reads as an order, not as what happened; “agentZ” in most rows.',
      mock: () => zConv(allTools({ label: (tool) => zVerb(tool.title) + (tool.subject ? subjectHtml(tool.subject) : '') }), toolsHeight()) },
    { key: 'D', name: 'The group first', from: 'new',
      desc: 'A small word for the group leads each row (THREAD, SUBTHREAD, WORKSPACE, TERMINAL, PROJECT), then the sentence.',
      good: 'Rows line up by what they touched.', cost: 'A column of capitals in a quiet list; repeats what the icon says.',
      mock: () => zConv(allTools({ label: (tool) => tagWord(GROUP_WORD[tool.g]) + sentence(tool) }), toolsHeight()) },
    { key: 'E', name: 'Only what changed something', from: 'new (as the Tool calls round offered for ToolSearch)',
      desc: 'Tools that only look (lists, reads, waits, checks, the workspace’s status, agents and models) don’t show; a folded run doesn’t count them. A failed one shows.',
      good: 'The thread shows what the agent did, not how it looked around.', cost: 'You can’t see that it waited or what it read.',
      mock: () => zConv(allTools({ keep: (tool) => !tool.looks }), toolsHeight((tool) => !tool.looks)) },
  ],
});

// 3. Running and failed -----------------------------------------------------------------------
const RUNNING = [
  { verb: 'Starting a subthread:', subject: [SUBS[1].title, 't'], g: 'tasks', time: '12s', live: SUBS[1].step },
  { verb: 'Waiting for', subject: [OTHER_THREAD, 't'], g: 'threads', time: '1m 12s', live: ['zed-search', 'Searched “login_test”'] },
  { verb: 'Running', subject: ['cargo test -p app', 'c'], g: 'terminals', time: '8s', liveText: 'test cart::total::rounds_half_up ... ok' },
];
const FAILED = [
  { verb: 'Start a subthread:', subject: ['Build on Devbox 1', 't'], g: 'tasks', error: 'Other machines are reached through the agentZ app, which isn’t connected.' },
  { verb: 'Type into', subject: ['this thread’s terminal', 'p'], g: 'terminals', error: 'The terminal’s process has exited, so it can’t take input.' },
  { verb: 'Wait for', subject: ['a thread', 'p'], g: 'threads', error: 'There is no thread 412 in this project.' },
];
const errorLine = (text) => `<div class="trunc" style="margin-left:30px;font-size:12px;line-height:18px;color:#c9707a">${text}</div>`;
const liveLine = (row) => (row.live ? `<div style="margin-left:30px">${zStep(row.live)}</div>` : `<div class="trunc" style="margin-left:30px;font:12px/20px ${ZMONO};color:var(--ph)">${row.liveText}</div>`);
const stateMock = (running, failed, h = 300) => zConv(zStack([['While it runs', RUNNING.map(running).join('')], ['Failed', FAILED.map(failed).join('')]]), h);
const tip = (text, style) => `<span class="ztip" style="${style}">${text}</span>`;
TOPICS.push({
  id: 'states', section: 'agentZ’s tools: the row', title: 'Running and failed', size: 'wide', rec: 'C',
  now: 'While a tool runs, its row is in the present tense with a spinner (“Starting a subthread: …”, “Waiting for …”). A failed one reads as an order (“Start a subthread: …”) with a red “Failed”; why it failed shows only when the row is opened, as the tool’s text. A wait of minutes looks like a wait of a second.',
  nowImg: 'img/now-own-states.png',
  issues: ['You can’t tell from the row why it failed.', 'A long wait or a running command gives no sign of progress.'],
  options: [
    { key: 'A', name: 'As today', from: 'Tool calls round',
      desc: 'Present tense and a spinner; an order and “Failed”, the reason inside.',
      good: 'Quiet.', cost: 'A click to see why it failed; nothing about progress.',
      mock: () => stateMock((row) => zRow(groupIcon(row.g), zVerb(row.verb) + subjectHtml(row.subject), { trailing: zSpin() }),
        (row) => zRow(groupIcon(row.g), zVerb(row.verb) + subjectHtml(row.subject), { trailing: zFailed() }), 240) },
    { key: 'B', name: 'How long, and why it failed', from: 't3code (a running row’s time; a failed row’s reason)',
      desc: 'While it runs, how long it has been (“1m 12s”) before the spinner. A failed one keeps “Failed”, with the reason on a line under it in red, cut to one line (the whole of it on hover).',
      good: 'You see that a wait is long, and why a call failed, without opening it.', cost: 'A failed call takes two lines.',
      mock: () => stateMock((row) => zRow(groupIcon(row.g), zVerb(row.verb) + subjectHtml(row.subject), { trailing: zDim(row.time) + zSpin() }),
        (row) => zRow(groupIcon(row.g), zVerb(row.verb) + subjectHtml(row.subject), { trailing: zFailed() }) + errorLine(row.error), 300) },
    { key: 'C', name: 'B, with what it’s waiting on', from: 'agentZ’s subagent rows (the step under a running card), after Zed',
      desc: 'B, and under a running row, one line of what it waits on: the step the subthread or thread is on, as its own row shows it, or the command’s last line of output. It goes once the call ends.',
      good: 'A wait shows the other thread’s progress where you are.', cost: 'Running rows take two lines, and the line moves as the other thread works.',
      mock: () => stateMock((row) => zRow(groupIcon(row.g), zVerb(row.verb) + subjectHtml(row.subject), { trailing: zDim(row.time) + zSpin() }) + liveLine(row),
        (row) => zRow(groupIcon(row.g), zVerb(row.verb) + subjectHtml(row.subject), { trailing: zFailed() }) + errorLine(row.error), 370) },
    { key: 'D', name: 'Zed’s status icons', from: 'Zed (a tool call’s status at the end of its row)',
      desc: 'The words stay the same as it runs and ends (the past tense); a spinner, a green check or a red cross at the end says how it went. The reason a call failed shows in a tooltip over the cross.',
      good: 'The row doesn’t change its words as it goes.', cost: '“Started a subthread” while it’s still starting; you hover for the reason.',
      mock: () => frame(`<div style="position:absolute;inset:0">${stateMock((row) => zRow(groupIcon(row.g), zVerb(row.verb.replace('Starting', 'Started').replace('Waiting', 'Waited').replace('Running', 'Ran')) + subjectHtml(row.subject), { trailing: zSpin() }),
        (row, index) => zRow(groupIcon(row.g), zVerb(row.verb.replace('Start a', 'Started a').replace('Type into', 'Typed into').replace('Wait for', 'Waited for')) + subjectHtml(row.subject), { trailing: zCross(), hover: index === 0 }), 240)}${tip(FAILED[0].error, 'right:22px;top:158px')}</div>`, { w: ZW, h: 240, style: ZT }) },
  ],
});

// 4. Opened -----------------------------------------------------------------------------------
const OPENED = [
  { row: zRow(ic('chat', 'sm'), zText('Listed threads'), { trailing: zDim('2 threads'), chevron: 'chev-up' }),
    json: '{\n  "threads": [\n    { "threadId": 412, "title": "Fix flaky login test", "status": "working",\n      "agentName": "Claude Agent", "model": "Opus 4.1", "lastActivityAt": "2025-10-09T18:02:11Z" },\n    { "threadId": 398, "title": "Checkout flow review", "status": "idle",\n      "agentName": "Factory Droid", "model": "GPT-5", "lastActivityAt": "2025-10-09T16:40:52Z" }\n  ],\n  "nextCursor": null\n}' },
  { row: zRow(ic('terminal', 'sm'), zVerb('Ran') + zCode('cargo test -p app'), { trailing: zDim('Exit 0 · 14s'), chevron: 'chev-up' }),
    json: '{\n  "exitCode": 0,\n  "timedOut": false,\n  "output": "running 214 tests\\n...\\ntest result: ok. 214 passed; 0 failed; 0 ignored"\n}' },
  { row: zRow(ic('branch', 'sm'), zText('Checked this thread’s workspace'), { trailing: zDim('Pasture, 2 commits ahead'), chevron: 'chev-up' }),
    json: '{\n  "kind": "pasture", "name": "brave-otter",\n  "folder": "~/.agentz/pastures/storefront/brave-otter",\n  "branch": "agentz/brave-otter", "baseBranch": "main",\n  "ahead": 2, "behind": 0, "uncommittedFiles": 1\n}' },
  { row: zRow(ic('list', 'sm'), zText('Listed agents and models'), { trailing: zDim('2 agents'), chevron: 'chev-up' }),
    json: '{\n  "agents": [\n    { "agentId": "claude-acp", "name": "Claude Agent", "models": ["Opus 4.1", "Sonnet 4.5"] },\n    { "agentId": "factory-droid", "name": "Factory Droid", "models": ["GPT-5", "Opus 4.1"] }\n  ],\n  "roles": ["implementation", "research", "review", "design", "test", "general"]\n}' },
];
const TEST_OUTPUT = 'running 214 tests\ntest cart::total::rounds_half_up ... ok\ntest agent_view::tests::subagent_rows ... ok\n…\ntest result: ok. 214 passed; 0 failed; 0 ignored';
const line = (html, style = '') => `<div class="row g2" style="font-size:12px;line-height:20px;color:var(--mu);min-width:0;${style}">${html}</div>`;
const statusDot = (color) => `<span style="width:6px;height:6px;border-radius:50%;background:${color};flex:none"></span>`;
const termBlock = (text, foot) => `<div style="border-radius:6px;background:#1e1f22;border:1px solid var(--b);overflow:hidden"><div style="font:12px/17px ${ZMONO};color:#c9ccd1;white-space:pre;padding:6px 8px">${text}</div>${foot ? `<div style="border-top:1px solid var(--b);padding:3px 8px;font-size:11px;color:var(--ph)">${foot}</div>` : ''}</div>`;
const WORDS_VIEW = [
  [line(`${statusDot('var(--ac)')}<span style="color:${ZBRIGHT}">Fix flaky login test</span><span>· Claude Agent · Opus 4.1 · working</span><span class="grow"></span><span class="ph">now</span>`),
    line(`${statusDot('transparent;border:1px solid var(--ph)')}<span style="color:${ZBRIGHT}">Checkout flow review</span><span>· Factory Droid · GPT-5 · idle</span><span class="grow"></span><span class="ph">1h ago</span>`)].join(''),
  termBlock(TEST_OUTPUT),
  [line(`Pasture <span style="color:${ZBRIGHT}">brave-otter</span>, on ${zInline('agentz/brave-otter')}, 2 commits ahead of ${zInline('main')}, 1 uncommitted file`),
    line(`<span class="ph">~/.agentz/pastures/storefront/brave-otter</span>`)].join(''),
  [line(`<span style="color:${ZBRIGHT}">Claude Agent</span><span>Opus 4.1, Sonnet 4.5</span>`), line(`<span style="color:${ZBRIGHT}">Factory Droid</span><span>GPT-5, Opus 4.1</span>`),
    line('<span class="ph">Roles: implementation, research, review, design, test, general</span>')].join(''),
];
const chip = (html) => `<span class="row none" style="gap:5px;height:22px;padding:0 7px;border-radius:5px;border:1px solid var(--b);font-size:12px;color:var(--mu)">${html}</span>`;
const miniCard = (title, state, color, meta) => `<div style="border:1px solid var(--b);border-radius:6px;padding:6px 10px;background:#2b2d30"><div class="row g2"><span class="trunc" style="font-size:13px;color:var(--t)">${title}</span><span class="grow"></span><span class="row none" style="gap:4px;font-size:12px;color:${color}">${statusDot(color)}${state}</span></div><div class="trunc" style="font-size:12px;color:var(--ph)">${meta}</div></div>`;
const PIECES_VIEW = [
  `<div class="col" style="gap:4px">${miniCard('Fix flaky login test', 'Working', 'var(--ac)', 'Claude Agent · Opus 4.1 · now')}${miniCard('Checkout flow review', 'Idle', 'var(--ph)', 'Factory Droid · GPT-5 · 1h ago')}</div>`,
  termBlock(`<span style="color:var(--ph)">$</span> cargo test -p app\n${TEST_OUTPUT}`, 'Exit 0 · 14s'),
  `<div class="row g2" style="flex-wrap:wrap">${chip(`${ic('pasture', 'xs')}brave-otter`)}${chip(`${ic('branch', 'xs')}agentz/brave-otter`)}${chip('↑2 on main')}${chip('1 uncommitted')}</div>`,
  `<div class="row g2" style="flex-wrap:wrap">${chip(`${glyph('claude', 'sm')}Claude Agent`)}${chip('Opus 4.1')}${chip('Sonnet 4.5')}</div><div class="row g2" style="flex-wrap:wrap">${chip(`${glyph('droid', 'sm')}Factory Droid`)}${chip('GPT-5')}${chip('Opus 4.1')}</div>`,
];
const kv = (pairs) => `<div style="display:grid;grid-template-columns:auto 1fr;gap:1px 14px;font-size:12px;line-height:19px">${pairs.map(([k, v]) => `<span style="color:var(--ph)">${k}</span><span style="color:var(--mu)">${v}</span>`).join('')}</div>`;
const TABLE_VIEW = [
  `<div style="display:grid;grid-template-columns:auto auto auto auto auto;gap:1px 14px;font-size:12px;line-height:19px;color:var(--mu)">${['Title', 'Status', 'Agent', 'Model', 'Last active'].map((h) => `<span style="color:var(--ph)">${h}</span>`).join('')}<span style="color:${ZBRIGHT}">Fix flaky login test</span><span>Working</span><span>Claude Agent</span><span>Opus 4.1</span><span>now</span><span style="color:${ZBRIGHT}">Checkout flow review</span><span>Idle</span><span>Factory Droid</span><span>GPT-5</span><span>1h ago</span></div>`,
  kv([['Exit code', '0'], ['Timed out', 'No'], ['Output', `<span style="font:12px ${ZMONO}">test result: ok. 214 passed; 0 failed</span>`]]),
  kv([['Kind', 'Pasture'], ['Name', 'brave-otter'], ['Branch', 'agentz/brave-otter, from main'], ['Ahead', '2 commits'], ['Uncommitted', '1 file'], ['Folder', '~/.agentz/pastures/storefront/brave-otter']]),
  kv([['Claude Agent', 'Opus 4.1, Sonnet 4.5'], ['Factory Droid', 'GPT-5, Opus 4.1'], ['Roles', 'implementation, research, review, design, test, general']]),
];
const openedMock = (views, { input = true, h = 470 } = {}) => zConv(OPENED.map((item, index) => item.row + zOut(views[index] + (input ? zInput() : ''))).join(''), h);
TOPICS.push({
  id: 'opened', section: 'agentZ’s tools: the row', title: 'What opening a row shows', size: 'wide', rec: 'A',
  now: 'Opening one of agentZ’s rows shows what the tool gave back as it was printed. For agentZ’s tools that’s JSON, with fields like <code>taskId</code>, <code>workState</code> and <code>waitTimedOut</code>, and the input as JSON behind “Input” at the end. Four tools opened, one from each of four groups; the mocks use the result after the sentence (What each row says, B).',
  nowImg: 'img/now-own-opened.png',
  issues: ['agentZ’s answers are written for agents; opened, they read as raw data.', 'A command’s output sits inside JSON with <code>\\n</code> for each new line.'],
  options: [
    { key: 'A', name: 'What came back, in words', from: 'new (as the Tool calls round showed ToolSearch’s tools in words)',
      desc: 'Each tool opens to a few lines in words: threads one a line with their agent, model, status and when; a command’s output as it was printed, in the code font; a workspace in a sentence and its folder; each agent with its models. The JSON stays behind “Input”.',
      good: 'Reads like the app, and fits in a few lines.', cost: 'agentZ writes a view for each of its 26 tools.',
      mock: () => openedMock(WORDS_VIEW) },
    { key: 'B', name: 'The app’s own pieces', from: 't3code (its thread hover card), Zed (a command’s output)',
      desc: 'What came back drawn as the app draws it elsewhere: threads as small cards with their status, a command as the agent’s own commands show (the command, its output, the exit code), a workspace as the header’s chips, agents with their icons and models.',
      good: 'A thread or a workspace looks the same wherever it shows.', cost: 'Bigger than lines; more to keep in step with the rest of the app.',
      mock: () => openedMock(PIECES_VIEW, { h: 540 }) },
    { key: 'C', name: 'A table of its fields', from: 'new',
      desc: 'What came back as names and values, the fields renamed in words (“Exit code”, “Last active”); a list as a table with a row each.',
      good: 'Everything the tool said, nothing hidden.', cost: 'Reads like a form; fields a person doesn’t care about show too.',
      mock: () => openedMock(TABLE_VIEW, { h: 470 }) },
    { key: 'D', name: 'Nothing to open', from: 'new',
      desc: 'agentZ’s rows don’t open; what each row says (topic 2) is all. Right-clicking one offers Copy Input and Copy Output.',
      good: 'The thread stays a list of short lines.', cost: 'You can’t see which threads it listed or what the command printed.',
      mock: () => zConv(`<div style="position:relative">${OPENED.map((item, index) => (index === 1 ? item.row.replace('border-radius:5px;', 'border-radius:5px;background:var(--hov);') : item.row).replace(/<span style="color:var\(--ph\);display:inline-flex;margin-right:2px">.*?<\/span><\/div>$/s, '</div>')).join('')}<div class="menu" style="left:250px;top:40px;min-width:170px"><div class="it hl">${ic('copy', 'sm')}Copy Input</div><div class="it">${ic('copy', 'sm')}Copy Output</div></div></div>`, 160) },
    { key: 'E', name: 'As today', from: 'Zed (Raw Input, Output)',
      desc: 'The JSON as printed, and the input behind “Input”.',
      good: 'Exactly what the agent got.', cost: 'Raw data in a conversation.',
      mock: () => openedMock(OPENED.map((item) => zBlock(item.json)), { h: 700 }) },
  ],
});

// 5. Links ------------------------------------------------------------------------------------
const LINK_ROWS = [
  { icon: 'bot', verb: 'Started a subthread:', subject: SUBS[0].title, made: true, thread: true },
  { icon: 'chat', verb: 'Waited for', subject: OTHER_THREAD, thread: true, result: 'Done in 2m 14s' },
  { icon: 'chat', verb: 'Read', subject: OTHER_THREAD, thread: true, result: '24 messages' },
  { icon: 'terminal', verb: 'Started a terminal:', subject: 'npm run dev', code: true, made: true, terminal: true },
  { icon: 'terminal', verb: 'Read', subject: 'this thread’s terminal', plain: true, terminal: true, result: '40 lines' },
  { icon: 'branch', verb: 'Handed off to a new pasture:', subject: 'agentz/checkout-review', code: true },
  { icon: 'folder', verb: 'Added a project:', subject: '~/projects/storefront', code: true, result: 'storefront' },
];
const linkSubject = (row, linked, hover) => (linked ? `<span class="trunc zlink ${hover ? 'hov' : ''}" style="${row.code ? `font:12px ${ZMONO}` : ''}">${row.subject}</span>` : row.code ? zCode(row.subject) : row.plain ? zText(row.subject) : zTitle(row.subject));
const linkMock = (render, h = 220) => zConv(`<div style="position:relative">${LINK_ROWS.map(render).join('')}</div>`, h);
TOPICS.push({
  id: 'links', section: 'agentZ’s tools: the row', title: 'Links to what a tool acted on', size: 'wide', rec: 'C',
  now: 'Rows that made a subthread, a thread or a terminal end in “Open”, which shows it. Rows that name a thread or a terminal they didn’t make (“Waited for …”, “Read …”) link to nothing. The mocks use the result after the sentence (What each row says, B).',
  nowImg: 'img/now-own-rows.png',
  issues: ['A thread named in a row (one it waited for, read or messaged) is a search away.'],
  options: [
    { key: 'A', name: 'Open for what it made', from: 't3code (“Open chat” on a thread it created)',
      desc: 'As today: Open at the end of rows that made a subthread, a thread or a terminal.',
      good: 'Open appears only where something new is.', cost: 'Rows about other threads don’t link them.',
      mock: () => linkMock((row) => zRow(ic(row.icon, 'sm'), zVerb(row.verb) + linkSubject(row, false), { trailing: row.made ? zOpen() : row.result ? zDim(row.result) : '' })) },
    { key: 'B', name: 'Titles are links', from: 'new (as a thread’s @-mention links in a message)',
      desc: 'A thread’s or subthread’s title in a row is underlined and opens it; no Open button. Terminals and code don’t link.',
      good: 'Every thread a row names is a click away, and the row stays short.', cost: 'Underlines in a quiet list; the click target is only the title.',
      mock: () => linkMock((row, index) => zRow(ic(row.icon, 'sm'), zVerb(row.verb) + linkSubject(row, row.thread, index === 1), { trailing: row.result ? zDim(row.result) : '' })) },
    { key: 'C', name: 'Open on every row with a thread or terminal', from: 'new, after t3code’s “Open chat”',
      desc: 'Open at the end of every row that names a thread, a subthread or a terminal, made or not. Workspaces and projects have none: a handoff already moved the thread, and a new project is in the sidebar.',
      good: 'One way to open things, where you’d look.', cost: 'Many Opens in a busy run.',
      mock: () => linkMock((row) => zRow(ic(row.icon, 'sm'), zVerb(row.verb) + linkSubject(row, false), { trailing: (row.result ? zDim(row.result) : '') + (row.thread || row.terminal ? zOpen() : '') })) },
    { key: 'D', name: 'The row opens it', from: 't3code (a subagent’s row opens its thread)',
      desc: 'Clicking a row that names a thread or a terminal opens it; an arrow shows at its end on hover. Its details open from a chevron beside the arrow.',
      good: 'The biggest target, nothing added to the row.', cost: 'A click no longer opens the details, unlike every other row.',
      mock: () => linkMock((row, index) => zRow(ic(row.icon, 'sm'), zVerb(row.verb) + linkSubject(row, false), { trailing: (row.result ? zDim(row.result) : '') + (index === 1 ? `<span class="none row" style="gap:6px;color:var(--ph)">${ic('chev-down', 'xs')}<span style="color:var(--ac);display:inline-flex">${ic('arrow-up-right', 'xs')}</span></span>` : ''), hover: index === 1 })) },
  ],
});

// 6. Folded runs ------------------------------------------------------------------------------
const foldLine = (text) => `<div class="row" style="min-height:24px;gap:6px;padding:0 2px"><span style="width:24px;display:inline-flex;justify-content:center;color:var(--ph)">${ic('chev-right', 'xs')}</span><span class="grow trunc" style="font-size:13px;color:${ZDIM}">${text}</span></div>`;
// The subthread row topic 7 recommends: the bot, the title, the role, Open, and its state.
function subRow(sub, { hover = false, chevron = '', step = true, open = true } = {}) {
  const end = sub.state === 'working' ? zSpin() : sub.state === 'failed' ? zFailed() : `${zDim(sub.time)}${zCheck()}`;
  const head = zRow(ic('bot', 'sm'), `${zTitle(sub.title)}${zTag(sub.role)}`, { trailing: `${open ? zOpen() : ''}${end}`, hover, chevron });
  return head + (step && sub.state === 'working' ? `<div style="margin-left:30px">${zStep(sub.step)}</div>` : '');
}
const ranRest = zPara('All three are running. Meanwhile, the tests pass: 214 passed.');
TOPICS.push({
  id: 'folded', section: 'agentZ’s tools: the row', title: 'In a folded run', size: 'wide', rec: 'B',
  now: 'A finished run of work folds to one line in t3code’s words: at most two kinds, those that made something first (subthreads, threads, terminals, commands, edits), then “and performed N other actions”. agentZ’s lists, reads, waits and checks count as other actions. Here the run is: listed agents and models, listed threads, started 3 subthreads, checked on 2, waited for 2, ran a command.',
  nowImg: 'img/now-folded.png',
  issues: ['The subthreads fold away with the rest, so their progress is out of sight once the run ends.', '“6 other actions” says nothing.'],
  options: [
    { key: 'A', name: 'As today', from: 't3code (summarizeToolGroup)',
      desc: 'One line for the run, the subthreads in it.',
      good: 'The shortest.', cost: 'The subthreads are behind a click.',
      mock: () => zConv([userAsk, foldLine('Started 3 subthreads, ran a command, and performed 6 other actions'), ranRest].join(''), 170) },
    { key: 'B', name: 'Subthreads stay out of the fold', from: 'agentZ’s subagent rows (they never fold)',
      desc: 'A subthread’s row stands on its own, as an agent’s own subagents’ rows do, so several show one under another; the rest of the run folds around them.',
      good: 'Every subthread, and how it’s going, stays where it started.', cost: 'A run that starts many subthreads stays long (topic 9 is about that).',
      mock: () => zConv([userAsk, foldLine('Listed agents and models, and listed threads'), ...SUBS.map((sub) => subRow(sub)), foldLine('Ran a command, and performed 4 other actions'), ranRest].join(''), 300) },
    { key: 'C', name: 'Looks don’t count', from: 'new',
      desc: 'The line names only what changed something; lists, reads, waits and checks are in the run but not in its line.',
      good: 'The line says what happened.', cost: 'Open it to see the agent waited.',
      mock: () => zConv([userAsk, foldLine('Started 3 subthreads, and ran a command'), ranRest].join(''), 170) },
    { key: 'D', name: 'Every kind named', from: 'new',
      desc: 'The line names every kind in the run, in the order they came, with no “other actions”.',
      good: 'Nothing hidden in a count.', cost: 'The line runs long and truncates.',
      mock: () => zConv([userAsk, foldLine('Listed agents and models, listed threads, started 3 subthreads, checked on 2 subthreads, waited for 2 threads, and ran a command'), ranRest].join(''), 170) },
  ],
});

// 7. A subthread where it started -------------------------------------------------------------
const t3Sub = (sub) => zT3Card({ glyphText: '✻', dotColor: sub.state === 'working' ? 'var(--ac)' : 'var(--ok)', title: sub.title,
  status: sub.state === 'working' ? 'Working' : 'Completed', statusColor: 'var(--ph)', detail: sub.state === 'working' ? sub.step[1] : sub.summary, time: sub.time });
const zedSub = (sub) => zZedCard(zZedHead({ state: sub.state, title: sub.title, meta: sub.model, files: sub.state === 'done' ? '' : '', stop: sub.state === 'working' }),
  sub.state === 'working' ? `<div style="border-top:1px solid var(--b);padding:2px 6px">${zStep(sub.step)}</div>${zStrip()}` : '');
const listSub = (sub) => `<div class="row" style="height:28px;gap:6px;padding:0 4px 0 6px;margin-left:24px"><span style="width:16px;display:inline-flex;justify-content:center">${sub.state === 'working' ? zBlueSpin() : zCheck()}</span><span class="trunc" style="font-size:13px;color:var(--t);flex:0 1 auto">${sub.title}</span><span class="none" style="font-size:13px;color:var(--ph)">· ${sub.model}</span><span class="grow"></span>${sub.state === 'working' ? zStop() : ''}</div>`;
const sentenceSub = (sub) => zRow(ic('bot', 'sm'), zVerb('Started a subthread:') + zTitle(sub.title), { trailing: `${sub.state === 'working' ? '<span class="none" style="font-size:12px;color:var(--ac)">Working</span>' : `<span class="none" style="font-size:12px;color:var(--ok)">Done in ${sub.time}</span>`}${zOpen()}` });
const subMock = (render, h = 250) => zConv([userAsk, ...SUBS.map(render), agentSaid].join(''), h);
TOPICS.push({
  id: 'sub-row', section: 'Subthreads in the parent', title: 'A subthread where it started', size: 'wide', rec: 'A',
  now: 'A <code>delegate_task</code> call is one of agentZ’s rows: “Started a subthread: &lt;title&gt;” and Open. Opened, it shows the tool’s answer, raw (<code>taskId</code>, <code>childThreadId</code>, <code>role</code>, <code>status</code>, <code>workState</code>, …), above a collapsed Input (your screenshot). How it’s going shows only in the Agents list over the composer; the row stays the same once it ends.',
  nowImg: 'img/backlog-subthread-view.png',
  issues: ['Opened, it’s raw JSON.', 'Nothing in the row says whether the subthread is working, done or failed.', 'An agent’s own subagents have their own, different row.'],
  options: [
    { key: 'A', name: 'The same row as an agent’s own subagents', from: 'agentZ’s subagent rows (the Agents’ own subagents round, after Zed)',
      desc: 'A subthread gets the row a subagent has: the bot, its title in the brighter gray, its role as a tag, Open, then a spinner, or how long it ran and a check (“Failed”, “Stopped”). While it runs, the step it’s on shows under it. Opening it is the next topic.',
      good: 'One look for every agent working for this thread, whoever started it.', cost: 'Its model and the files it changed are only in the Agents list.',
      mock: () => subMock((sub) => subRow(sub), 300) },
    { key: 'B', name: 't3code’s subagent card', from: 't3code (SubagentTimelineLink)',
      desc: 'The agent’s icon on a round tile with a status dot, the title over the step it’s on or the first line of its summary, how long it ran, and a chevron. The whole row opens the subthread; hovering shows its details.',
      good: 'Each subthread’s progress or result, in two lines.', cost: 'Taller rows, and a third style beside agentZ’s rows and subagents.',
      mock: () => subMock(t3Sub, 290) },
    { key: 'C', name: 'Zed’s subagent card', from: 'Zed (render_subagent_card)',
      desc: 'A bordered card: a spinner or check, the title, “· model”, the files it changed, and Stop while it runs. A running one shows the step it’s on inside, and a strip at the bottom opens it full screen.',
      good: 'Stands out as work going on elsewhere; Stop is at hand.', cost: 'Cards are heavy when there are several.',
      mock: () => subMock(zedSub, 360) },
    { key: 'D', name: 'The Agents list’s row', from: 'agentZ’s Agents list (the Subthreads round, after Zed’s card header)',
      desc: 'The row from the list over the composer, repeated where it started: a spinner or check, the title, “· model”, the files it changed, and Stop.',
      good: 'The same row in the list and in the conversation.', cost: 'Doesn’t say it was a step of the agent’s; no role.',
      mock: () => subMock(listSub, 250) },
    { key: 'E', name: 'Today’s sentence, live', from: 'new',
      desc: 'Today’s “Started a subthread: &lt;title&gt;” and Open, with its state before Open: “Working”, or “Done in 2m 14s”. Opening it is the next topic; no JSON either way.',
      good: 'The smallest change.', cost: 'Reads unlike an agent’s own subagents.',
      mock: () => subMock(sentenceSub, 230) },
  ],
});

// 8. Opening a subthread's row -----------------------------------------------------------------
const DONE = SUBS[0];
const TASK_TEXT = 'Research how t3code shows child tasks in the parent thread: which rows, what they say while they run and once done, and what opening one does. Read references/t3code/apps/web/src/components/chat. Reply in under 200 words.';
const summaryHtml = `<div style="font-size:13px;line-height:20px;color:var(--t)">${DONE.summary} A notification about a child that ended draws that child’s row again, with “Finished” and the time.</div>`;
const preview = ({ task = false } = {}) => `<div style="margin:4px 0 4px 30px;border:1px solid var(--b);border-radius:6px;background:var(--ed);overflow:hidden">
  ${task ? `<div style="padding:6px 10px;border-bottom:1px solid var(--b)">${zHeading('Task', zTag('research'))}<div class="zclamp" style="-webkit-line-clamp:2;font-size:12px;line-height:18px;color:var(--mu)">${TASK_TEXT}</div></div>` : ''}
  ${zFade(SUB_STEPS.map((step) => zStep(step)).join(''), 150)}
  <div style="padding:8px 12px;border-top:1px solid var(--b)">${summaryHtml}</div>${zStrip()}</div>`;
const openRow = (extra = {}) => subRow(DONE, { hover: true, chevron: 'chev-up', ...extra });
TOPICS.push({
  id: 'sub-open', section: 'Subthreads in the parent', title: 'Opening a subthread’s row', size: 'wide', rec: 'A',
  now: 'Opened, the row shows the tool’s answer as raw JSON and a collapsed Input (your screenshot, in the topic before). The subthread’s steps and its summary are only in the subthread. The mocks use the row the topic before recommends.',
  nowImg: 'img/now-subthread-opened.png',
  issues: ['Nothing about what the subthread did, or found.'],
  options: [
    { key: 'A', name: 'Zed’s preview of its work', from: 'Zed (render_subagent_expanded_content), as agentZ’s subagents open',
      desc: 'Its last 8 steps, fading at the top when there are more, then its summary once it ends, and a strip that opens the subthread (“Make Subagent Full Screen”). The same as a Claude Agent subagent opens.',
      good: 'Its work and result where it started; the same as subagents.', cost: 'The task it was given isn’t shown.',
      mock: () => zConv([openRow(), preview()].join(''), 360) },
    { key: 'B', name: 'The task and the summary', from: 'agentZ’s Factory Droid Task view',
      desc: '“Task” with its role and the task it was given, cut to four lines (all of it on a click), then “Summary” with what it reported once it ended. No steps.',
      good: 'What it was asked and what it said, side by side.', cost: 'You open the subthread to see how it got there.',
      mock: () => zConv([openRow(), zOut(`${zHeading('Task', zTag('research'))}<div class="zclamp" style="-webkit-line-clamp:4;font-size:13px;line-height:20px;color:var(--mu)">${TASK_TEXT}</div>${zHeading('Summary')}${summaryHtml}`)].join(''), 260) },
    { key: 'C', name: 'Its whole conversation, inline', from: 'new',
      desc: 'The subthread’s conversation inside the row, as it shows in the subthread (the task card, its steps folded as runs, its words), up to 24 rem tall and scrolling.',
      good: 'Everything without leaving the parent.', cost: 'A conversation inside a conversation; easy to lose your place.',
      mock: () => zConv([openRow(), `<div style="margin:4px 0 4px 30px;border:1px solid var(--b);border-radius:6px;background:var(--ed);padding:8px 10px;position:relative;height:250px;overflow:hidden">
        <div style="border:1px solid var(--b);border-radius:8px;padding:6px 10px;background:#2b2d30;margin-bottom:6px">${zHeading(`Task from “i wanna test subthreads…”`, zTag('Research'))}<div class="zclamp" style="-webkit-line-clamp:2;font-size:12px;line-height:18px;color:var(--mu)">${TASK_TEXT}</div></div>
        ${foldLine('Read 5 files, searched code 2 times, and ran a command')}${zPara(DONE.summary, 'font-size:13px;line-height:20px')}
        <div style="position:absolute;right:3px;top:8px;width:5px;height:90px;border-radius:3px;background:rgba(223,225,229,.25)"></div></div>`].join(''), 330) },
    { key: 'D', name: 'The row opens the subthread', from: 't3code (the whole row opens the child thread)',
      desc: 'Nothing opens inline: a click on the row opens the subthread; a chevron on hover says so.',
      good: 'One place for a subthread’s work: the subthread.', cost: 'You leave the parent to see anything.',
      mock: () => zConv([userAsk, subRow(DONE, { hover: true, chevron: 'chev-right', open: false }), subRow(SUBS[1], { open: false }), agentSaid].join(''), 200) },
    { key: 'E', name: 'A, with the task at the top', from: 'Zed’s preview, with agentZ’s Task view',
      desc: 'A’s preview, with the task it was given at its top, cut to two lines.',
      good: 'What it was asked, what it did and what it found, in one box.', cost: 'The tallest of these.',
      mock: () => zConv([openRow(), preview({ task: true })].join(''), 420) },
  ],
});

// 9. Several at once ---------------------------------------------------------------------------
const stack = () => `<span class="row none" style="margin-right:4px">${SUBS.map((sub, index) => `<span style="margin-left:${index ? -8 : 0}px">${zAvatar('✻', '')}</span>`).join('')}</span>`;
const groupHead = (open) => zRow(ic('bot', 'sm'), `${zText('3 subthreads')}${zDim('· 2 running')}`, { chevron: open ? 'chev-up' : 'chev-down' });
const gridCard = (sub) => `<div style="flex:1;min-width:0;border:1px solid var(--b);border-radius:6px;padding:6px 8px;background:#2b2d30"><div class="row g1">${sub.state === 'working' ? zBlueSpin() : zCheck()}<span class="trunc" style="font-size:12px;color:var(--t)">${sub.title.replace('Research: ', '')}</span></div><div class="trunc" style="font-size:11px;color:var(--ph);margin-top:2px">${sub.state === 'working' ? sub.step[1] : `Done in ${sub.time}`}</div></div>`;
TOPICS.push({
  id: 'sub-many', section: 'Subthreads in the parent', title: 'Several subthreads at once', size: 'wide', rec: 'B',
  now: 'An agent that starts several subthreads calls <code>delegate_task</code> once for each, one row after another, and the run folds to “Started 3 subthreads” once it ends. <code>create_threads</code>, which starts several threads in one call, is one row: “Started 3 threads”. The mocks use the row topic 7 recommends.',
  nowImg: 'img/now-subthreads-many.png',
  issues: ['Three subthreads read as three of the same line.', 'Once folded, nothing shows that any are still running.'],
  options: [
    { key: 'A', name: 'A row each', from: 'Zed (a card for each subagent)',
      desc: 'Each subthread its own row, one under another, as they were started.',
      good: 'Each one’s state at a glance.', cost: 'Long when there are many.',
      mock: () => zConv([userAsk, ...SUBS.map((sub) => subRow(sub)), agentSaid].join(''), 300) },
    { key: 'B', name: 'A group that folds', from: 'agentZ’s Agents list (its line and rows), after t3code',
      desc: 'Subthreads started together sit under one line, “3 subthreads · 2 running”, with a row each under it. The group folds like the Agents list: open while any runs, closed once all are done (“3 subthreads · all done”).',
      good: 'Many subthreads take one line once they’re done.', cost: 'One more level to open.',
      mock: () => zConv([userAsk, groupHead(true), ...SUBS.map((sub) => `<div style="margin-left:24px">${subRow(sub)}</div>`), agentSaid].join(''), 320) },
    { key: 'C', name: 'One row with their icons', from: 't3code (its stack of subagent avatars)',
      desc: 'One row: “Started 3 subthreads”, the agents’ icons overlapping, and “2 running · 1 done”. Opening it shows a row each.',
      good: 'One line however many there are.', cost: 'Which ones are running is behind a click.',
      mock: () => zConv([userAsk, zRow(ic('bot', 'sm'), `${zText('Started 3 subthreads')}`, { trailing: `${stack()}${zDim('2 running · 1 done')}${zSpin()}`, chevron: 'chev-down' }), agentSaid].join(''), 170) },
    { key: 'D', name: 'Side by side', from: 'new',
      desc: 'Subthreads started together show as small cards in a row, up to three across: the state, the title, and the step it’s on or how long it ran. A click opens one.',
      good: 'Parallel work looks parallel.', cost: 'Titles cut short; a new kind of element in the conversation.',
      mock: () => zConv([userAsk, `<div class="row" style="gap:8px;margin:4px 0 4px 30px">${SUBS.map(gridCard).join('')}</div>`, agentSaid].join(''), 200) },
  ],
});

// 10. A subagent at work ----------------------------------------------------------------------
const EXPLORE = [
  { title: 'Find where the login view is drawn', type: 'Explore', time: '21s',
    steps: [['zed-search', 'Searched “render_centered”', '2 results'], ['file', 'Read crates/app/src/agent_login.rs'], ['file', 'Read crates/app/src/agent_view.rs'], ['zed-search', 'Searched “LoginLayout”', '4 results'], ['file', 'Read crates/app/src/settings_page.rs']] },
  { title: 'Find how Add Account logs in', type: 'Explore', time: '9s',
    steps: [['zed-search', 'Searched “LoginLayout::Dialog”', '1 result'], ['file', 'Read crates/app/src/accounts_view.rs']] },
];
const userClaude = zBubble('where is the login view drawn, and how does Add Account share it?');
const agentRow = (s, { trailing, hover = false, chevron = '' } = {}) => zRow(ic('bot', 'sm'), `${zTitle(s.title)}${zTag(s.type)}`, { trailing: trailing ?? `${zOpen()}${zSpin()}`, hover, chevron });
const corner = `<span style="color:var(--ph);font:12px ${ZMONO};width:14px;flex:none">⎿</span>`;
const lastSteps = (s, n = 3) => {
  const shown = s.steps.slice(-n);
  const more = s.steps.length - shown.length;
  return `<div style="margin-left:30px">${shown.map((step, index) => `<div class="row" style="gap:2px">${index === 0 ? corner : '<span style="width:14px;flex:none"></span>'}<div class="grow" style="min-width:0">${zStep(step, { spins: index === shown.length - 1 })}</div></div>`).join('')}${more ? `<div style="margin-left:46px;font-size:12px;line-height:20px;color:var(--ph)">+${more} more steps</div>` : ''}</div>`;
};
TOPICS.push({
  id: 'subagent-running', section: 'Agents’ own subagents', title: 'A subagent at work', size: 'wide', rec: 'A',
  now: 'Claude Agent sends its subagents’ steps in sessions of their own, which agentZ makes subthreads. Its row: the bot, the subagent’s description, its type as a tag, Open and a spinner; under it, the one step it’s on. Opening it shows its last 8 steps. Factory Droid sends no steps (topic 12). Here two Claude Agent subagents run at once, from the mock agent.',
  nowImg: 'img/now-subagents-running.png',
  issues: ['One step at a time, which changes every second, says little of what it has done.', 'You said the view is stepless: the steps are there only once it’s opened.'],
  options: [
    { key: 'A', name: 'Its last steps under it', from: 'Claude Code (a subagent’s last tool uses, “+N more tool uses”)',
      desc: 'Under the row, its last three steps, the newest at the bottom with a spinner, and “+2 more steps” above them when there are more. Once it ends they go, and the next topic decides what stays.',
      good: 'You see it working without opening it, and the list doesn’t grow.', cost: 'Four lines for each running subagent.',
      mock: () => zConv([userClaude, ...EXPLORE.map((s) => agentRow(s) + lastSteps(s))].join(''), 260) },
    { key: 'B', name: 'Zed’s card while it runs', from: 'Zed (render_subagent_card)',
      desc: 'A bordered card: a spinner, the description, “· Explore”, Stop. Inside, the step it’s on, as its own row, with any output or question of its own; a strip at the bottom opens it full screen.',
      good: 'Clearly a box of work going on elsewhere; a question it asks shows in its card.', cost: 'One step at a time, as today; heavy cards.',
      mock: () => zConv([userClaude, ...EXPLORE.map((s) => zZedCard(zZedHead({ state: 'working', title: s.title, meta: s.type, stop: false }), `<div style="border-top:1px solid var(--b);padding:2px 6px">${zStep(s.steps[s.steps.length - 1], { spins: true })}</div>${zStrip()}`))].join(''), 290) },
    { key: 'C', name: 'A live preview while it runs', from: 'Zed (its preview of a subagent’s work), open while it runs',
      desc: 'While it runs, the row is open: a box of its last steps, up to 8, fading at the top, the newest at the bottom. It closes to the row once the subagent ends.',
      good: 'The most of its work without a click.', cost: 'Two running subagents take a screen’s third.',
      mock: () => zConv([userClaude, ...EXPLORE.map((s) => agentRow(s, { chevron: 'chev-up' }) + `<div style="margin:2px 0 6px 30px;border:1px solid var(--b);border-radius:6px;background:var(--ed)">${zFade(s.steps.map((step, index) => zStep(step, { spins: index === s.steps.length - 1 })).join(''), 100)}</div>`)].join(''), 340) },
    { key: 'D', name: 't3code’s card', from: 't3code (SubagentTimelineLink)',
      desc: 'The agent’s icon with a blue dot, the description over the step it’s on, how long it has run, and a chevron that opens it.',
      good: 'Two lines, live.', cost: 'One step at a time; no type tag.',
      mock: () => zConv([userClaude, ...EXPLORE.map((s) => zT3Card({ glyphText: '✻', dotColor: 'var(--ac)', title: s.title, status: 'Running', detail: s.steps[s.steps.length - 1][1], time: s.time }))].join(''), 190) },
    { key: 'E', name: 'As today', from: 'agentZ (the Agents’ own subagents round)',
      desc: 'The row, and under it the one step it’s on.',
      good: 'Two lines a subagent.', cost: 'What it did before this step is behind a click.',
      mock: () => zConv([userClaude, ...EXPLORE.map((s) => agentRow(s) + `<div style="margin-left:30px">${zStep(s.steps[s.steps.length - 1])}</div>`)].join(''), 200) },
  ],
});

// 11. A finished subagent ---------------------------------------------------------------------
const FINISHED = { ...EXPLORE[0], time: '38s', count: 12,
  report: `The thread’s login is drawn by ${zInline('AgentLogin::render_centered')} in ${zInline('crates/app/src/agent_login.rs')}; ${zInline('agent_view.rs')} puts it in the thread when the agent says it’s logged out. Add Account opens the same view in a dialog (${zInline('LoginLayout::Dialog')}), so the methods and their order are the same in both.` };
const doneRow = (opts = {}) => agentRow(FINISHED, { trailing: `${zOpen()}${zDim(FINISHED.time)}${zCheck()}`, ...opts });
const reportHtml = (style = '') => `<div style="font-size:13px;line-height:20px;color:var(--t);${style}">${FINISHED.report}</div>`;
const STEPS_12 = [...FINISHED.steps, ['zed-search', 'Searched “add_account”', '3 results'], ['file', 'Read crates/app/src/accounts_view.rs'], ['file', 'Read crates/app/src/agent_login.rs'], ['zed-search', 'Searched “Dialog”', '6 results']];
TOPICS.push({
  id: 'subagent-done', section: 'Agents’ own subagents', title: 'A finished subagent', size: 'wide', rec: 'D',
  now: 'Once a Claude Agent subagent ends, its row shows how long it ran and a check (“Failed”, “Stopped”). Opened: its last 8 steps fading at the top, its report, and a strip that opens its subthread. Closed, nothing says what it did or found.',
  nowImg: 'img/now-subagents-done.png',
  issues: ['Closed, a finished subagent is one line with a time.', 'The report, the part you want, is under its steps.'],
  options: [
    { key: 'A', name: 'A count of its steps, then the report', from: 'Claude Code (“Done (12 tool uses · 38s)”)',
      desc: 'Under the row, “12 steps · 38s”. Opened: its report; the steps are behind “12 steps”, which opens them as a list.',
      good: 'How much it did at a glance; the report first when opened.', cost: 'Two lines when closed.',
      mock: () => zConv([userClaude, agentRow(FINISHED, { trailing: `${zOpen()}${zCheck()}`, hover: true, chevron: 'chev-up' }), `<div style="margin-left:30px;display:flex;flex-direction:column;gap:6px;padding:2px 0 6px"><div class="row" style="gap:2px">${corner}<span class="row g1" style="font-size:12px;color:var(--ph)">12 steps · 38s${ic('chev-down', 'xs')}</span></div>${reportHtml('padding-left:16px')}</div>`].join(''), 220) },
    { key: 'B', name: 'The report’s first lines, shown', from: 't3code (the result’s first line under the title), longer',
      desc: 'Closed, the first three lines of its report show under the row, fading out. A click opens the rest of it, and its steps above it as Zed’s preview has them.',
      good: 'What it found, without a click.', cost: 'Every finished subagent takes four lines.',
      mock: () => zConv([userClaude, doneRow(), `<div style="margin-left:30px;position:relative;max-height:62px;overflow:hidden">${reportHtml()}<div style="position:absolute;left:0;right:0;bottom:0;height:22px;background:linear-gradient(180deg,rgba(38,40,43,0),var(--panel))"></div></div>`, agentRow(EXPLORE[1], { trailing: `${zOpen()}${zDim('24s')}${zCheck()}` })].join(''), 210) },
    { key: 'C', name: 'As today', from: 'Zed (render_subagent_expanded_content)',
      desc: 'Closed, the row with its time and check. Opened, its last 8 steps fading at the top, its report, and the strip that opens it.',
      good: 'Its work and its report in one box.', cost: 'Closed, it says nothing of what it found.',
      mock: () => zConv([userClaude, doneRow({ hover: true, chevron: 'chev-up' }), `<div style="margin:4px 0 4px 30px;border:1px solid var(--b);border-radius:6px;background:var(--ed);overflow:hidden">${zFade(STEPS_12.slice(-8).map((step) => zStep(step)).join(''), 150)}<div style="padding:8px 12px;border-top:1px solid var(--b)">${reportHtml()}</div>${zStrip()}</div>`].join(''), 380) },
    { key: 'D', name: 'Its steps folded, then the report', from: 't3code (its folded runs’ words), with Zed’s preview',
      desc: 'Opened: its steps folded to one line in the words a run of work gets (“Searched code 4 times, and read 7 files”), which opens to the steps, then its report, then the strip. Closed, the row as today. A running one shows topic 10’s pick.',
      good: 'Reads like the rest of the thread; the report isn’t under a list of steps.', cost: 'The steps are one more click.',
      mock: () => zConv([userClaude, doneRow({ hover: true, chevron: 'chev-up' }), `<div style="margin:4px 0 4px 30px;border:1px solid var(--b);border-radius:6px;background:var(--ed);overflow:hidden"><div style="padding:2px 4px">${foldLine('Searched code 4 times, and read 7 files')}</div><div style="padding:8px 12px;border-top:1px solid var(--b)">${reportHtml()}</div>${zStrip()}</div>`].join(''), 250) },
    { key: 'E', name: 'The row opens its subthread', from: 't3code (a subagent’s row opens its thread)',
      desc: 'Nothing inline: the row, with its time and check, opens the subagent’s subthread, where its steps and report are.',
      good: 'One line each, however much it did.', cost: 'The report is a page away.',
      mock: () => zConv([userClaude, agentRow(FINISHED, { trailing: `${zDim(FINISHED.time)}${zCheck()}`, hover: true, chevron: 'chev-right' }), agentRow(EXPLORE[1], { trailing: `${zDim('24s')}${zCheck()}` })].join(''), 140) },
  ],
});

// 12. Subagents that send no steps ------------------------------------------------------------
const DROID = { title: 'Build subthreads round picks', type: 'worker', options: ['heavy'], time: '4m 12s',
  prompt: 'Build what the user picked in the “Subthreads” design round of agentZ. Repo: /root/projects/agentZ. Read design/subthreads/decisions.md first, then the Subthreads section of docs/architecture.md, and build each pick as described. Run cargo test -p app before you finish.',
  report: `Built the seven picks in ${zInline('design/subthreads/decisions.md')}: the Agents list’s rows, the list folding once all are done, the subthread bar, the task card, the bottom bar and Ctrl-minus. ${zInline('cargo test -p app')} passes.` };
const BACKGROUND = { title: 'Trace delegate_task in server', type: 'explorer',
  prompt: 'Read-only research. Do not edit, create or delete any files. Question: when a thread calls the delegate_task agent-control tool, what happens in the server? Reply in under 250 words.',
  launch: 'Task launched in background.\ntask_id: 2a17779f-cee3-40e7-9be9-4b8b849d484c\nsession_id: 2a17779f-cee3-40e7-9be9-4b8b849d484c\nsubagent_type: explorer\ndescription: Trace delegate_task in server\nThe task is running in a subagent session.\nYou will be notified automatically when it completes and its report will be delivered to you: do not wait for it or poll for it.' };
const userDroid = zBubble('build the subthreads round’s picks, and find out what delegate_task does in the server');
const droidRow = (s, trailing, opts = {}) => zRow(ic('bot', 'sm'), `${zTitle(s.title)}${zTag(s.type)}${(s.options || []).map(zTag).join('')}`, { trailing, ...opts });
const promptText = (text, lines) => `<div class="${lines ? 'zclamp' : ''}" style="${lines ? `-webkit-line-clamp:${lines};` : ''}font-size:13px;line-height:20px;color:var(--mu)">${text}</div>`;
TOPICS.push({
  id: 'no-steps', section: 'Agents’ own subagents', title: 'Subagents that send no steps', size: 'wide', rec: 'A',
  now: 'Factory Droid’s Task sends only its input (type, description, options, prompt) and, once done, what came back. Its row: the bot, the description, the type as a tag, the time and a check. Opened: “Task” with the whole prompt, “Report”, and Input. A Task Droid runs in the background ends at once (448 ms in your screenshot), so its report is Droid’s launch notice, task_id and all, and the real report never shows where it started.',
  nowImg: 'img/backlog-subagent-view.png',
  issues: ['The prompt shows whole, however long.', 'A background Task looks done, with a time and a check, while it’s still running, and its “report” is a notice meant for the agent.'],
  options: [
    { key: 'A', name: 'The task cut short, the report as written', from: 'new',
      desc: 'Its options as tags beside its type. Opened: the task cut to three lines (all of it on a click), then the report as markdown. A background Task reads “In the background” in place of a time and check, and opened shows only its task: the launch notice is left out.',
      good: 'A background Task no longer looks done; the report reads as text.', cost: 'agentZ recognizes Droid’s launch notice by its words.',
      mock: () => zConv([userDroid, droidRow(DROID, `${zDim(DROID.time)}${zCheck()}`, { hover: true, chevron: 'chev-up' }), zOut(`${zHeading('Task')}${promptText(DROID.prompt, 3)}<span class="none" style="font-size:12px;color:var(--ac)">Show all</span>${zHeading('Report')}<div style="font-size:13px;line-height:20px;color:var(--t)">${DROID.report}</div>`), droidRow(BACKGROUND, zDim('In the background'))].join(''), 330) },
    { key: 'B', name: 'The report first', from: 'Tool calls round (output first, the input behind “Input”)',
      desc: 'Opened: its report alone, with “Task” at the end, a line that opens the task as Input opens JSON. A background one opens to its task.',
      good: 'What you open it for comes first.', cost: 'The task is a second click.',
      mock: () => zConv([userDroid, droidRow(DROID, `${zDim(DROID.time)}${zCheck()}`, { hover: true, chevron: 'chev-up' }), zOut(`<div style="font-size:13px;line-height:20px;color:var(--t)">${DROID.report}</div><div class="row g1" style="font-size:12px;color:var(--ph)">Task${ic('chev-down', 'xs')}</div>`), droidRow(BACKGROUND, zDim('In the background'))].join(''), 220) },
    { key: 'C', name: 'A subthread with no steps', from: 'agentZ’s Claude Agent subagents (as subthreads)',
      desc: 'agentZ makes Droid’s Task a subthread too: the row ends in Open, which shows it with its task card and the report as its one reply. Opened inline, it shows the report, as Zed’s preview without steps.',
      good: 'Every subagent opens the same way.', cost: 'A subthread with nothing in it but a reply.',
      mock: () => frame(`<div style="position:absolute;inset:0;display:flex">${zConv([userDroid, droidRow(DROID, `${zOpen()}${zDim(DROID.time)}${zCheck()}`), droidRow(BACKGROUND, `${zOpen()}${zDim('In the background')}`)].join(''), 300, 420)}<div style="flex:1;background:var(--ed);border-left:1px solid var(--b);padding:12px 14px;display:flex;flex-direction:column;gap:8px">${zCaption('Open shows')}<div style="border:1px solid var(--b);border-radius:8px;padding:6px 10px;background:#2b2d30">${zHeading('Task from “build the subthreads round’s picks…”', zTag('worker'))}${promptText(DROID.prompt, 2)}</div><div style="font-size:13px;line-height:20px;color:var(--t)">${DROID.report}</div></div></div>`, { w: ZW, h: 300, style: ZT }) },
    { key: 'D', name: 't3code’s card', from: 't3code (SubagentTimelineLink)',
      desc: 'Droid’s icon with a status dot, the description over the report’s first line once done (its task’s first line while it runs), how long it ran, and a chevron that opens the task and report.',
      good: 'The result’s gist without a click.', cost: 'A style unlike Claude Agent’s subagents.',
      mock: () => zConv([userDroid, zT3Card({ glyphText: '❋', dotColor: 'var(--ok)', title: DROID.title, status: 'Completed', detail: 'Built the seven picks in design/subthreads/decisions.md: the Agents list’s rows, …', time: DROID.time }), zT3Card({ glyphText: '❋', dotColor: 'var(--ac)', title: BACKGROUND.title, status: 'Running', detail: 'Read-only research. Do not edit, create or delete any files.', time: 'in the background' })].join(''), 180) },
    { key: 'E', name: 'As today', from: 'agentZ (the Agents’ own subagents round)',
      desc: 'Opened: “Task” with the whole prompt, “Report” with what came back, Input.',
      good: 'Everything Droid sent.', cost: 'Long, and a background Task’s notice shows as its report.',
      mock: () => zConv([userDroid, droidRow(BACKGROUND, `${zDim('448ms')}${zCheck()}`, { hover: true, chevron: 'chev-up' }), zOut(`${zHeading('Task')}${promptText(BACKGROUND.prompt)}${zHeading('Report')}<div style="font-size:13px;line-height:20px;color:var(--t);white-space:pre-line">${BACKGROUND.launch}</div>${zInput()}`)].join(''), 380) },
  ],
});

// 13. When delegated tasks end ----------------------------------------------------------------
const doneSubs = SUBS.map((sub) => ({ ...sub, state: 'done', time: sub.time === '2m 14s' ? '2m 14s' : sub.time === '1m 02s' ? '3m 40s' : '2m 51s' }));
const before = [userAsk, ...doneSubs.map((sub) => subRow(sub)), agentSaid];
const after = [foldLine('Checked on 3 subthreads'), zPara('All three are done. t3code draws each child as a row with its status and last step; agents get agentZ’s tools from the MCP bridge each session starts; the server ends a task once its thread is idle with nothing queued.')];
const endedMock = (marker, h = 330) => zConv([...before, marker, ...after].join(''), h);
const finishedCard = (sub) => zT3Card({ glyphText: '✻', dotColor: 'var(--ok)', title: sub.title, status: 'Finished', detail: '', statusColor: 'var(--ph)', time: '14:02' });
TOPICS.push({
  id: 'tasks-ended', section: 'When delegated tasks end', title: 'What the parent shows when its tasks end', size: 'wide', rec: 'A',
  now: 'When subthreads end, agentZ tells the parent’s agent with t3code’s message, sent in your place and marked “Sent by the agent in “…””: “Delegated tasks 282, 283 reached terminal states. Use task_status with each taskId to read the results.” (your screenshot). Its next turn starts from that message. Another change is hiding that message from you now; the agent still gets it, as it’s the only thing that starts a turn while the agent is idle. The mocks use topic 7’s recommended row.',
  nowImg: 'img/backlog-delegated-tasks-message.png',
  issues: ['A message meant for the agent shows as if you’d sent it.'],
  options: [
    { key: 'A', name: 'Nothing', from: 'new (the change being made now)',
      desc: 'The message stays hidden. The subthreads’ rows and the Agents list already show that they ended, and the agent’s next turn just follows.',
      good: 'Nothing new in the thread.', cost: 'A turn starts with no message of yours above it.',
      mock: () => endedMock('', 300) },
    { key: 'B', name: 'A line across the thread', from: 't3code (its system dividers: “Run interrupted”, “Context handoff”)',
      desc: 'Where the agent picks up, a thin line across the conversation: “Research: UI for child tasks and 2 others finished”, with Open for one, or the Agents list for several.',
      good: 'Says why the agent started again, quietly.', cost: 'A line between turns that no other app shows for this.',
      mock: () => endedMock('<div class="zdivider">3 subthreads finished · 14:02</div>', 330) },
    { key: 'C', name: 'Their rows again', from: 't3code (SubagentNotificationLink)',
      desc: 'Where the agent picks up, each subthread that ended shows again as its card, with “Finished” and the time; a click opens it.',
      good: 'You see which ended, and can open each from there.', cost: 'The same subthreads twice in a screen.',
      mock: () => endedMock(doneSubs.map(finishedCard).join(''), 430) },
    { key: 'D', name: 'A small note above the reply', from: 'new',
      desc: 'Above the agent’s next turn, one dim line with the agentZ mark: “Picked up after 3 subthreads finished”.',
      good: 'Explains the turn in the fewest words.', cost: 'One more kind of line.',
      mock: () => endedMock(`<div class="row" style="gap:6px;font-size:12px;color:var(--ph);margin:6px 0 2px 2px">${ic('agentz', 'xs')}Picked up after 3 subthreads finished</div>`, 330) },
    { key: 'E', name: 'Don’t wake the agent', from: 'new',
      desc: 'agentZ sends no message: the agent learns its tasks ended only when it waits for them or checks on them. If its turn ended first, nothing happens until you write.',
      good: 'No message at all, hidden or not.', cost: 'An agent that started subthreads and stopped never reads their results unless you ask.',
      mock: () => zConv([...before, zBubble('they’re done, what did they find?'), ...after].join(''), 360) },
  ],
});
