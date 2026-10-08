// An agent's own subagents (Factory Droid's Task, Claude Agent's Agent): the call's row, what it
// opens to, the subagent's own steps, and the Agents list. Thread pieces as agent_view.rs draws
// them after the Tool calls round, in JetBrains Dark.

Object.assign(ICONS, {
  'x-circle': '<circle cx="12" cy="12" r="10"/><path d="m15 9-6 6"/><path d="m9 9 6 6"/>',
  'arrow-up-right': '<path d="M7 7h10v10"/><path d="M7 17 17 7"/>',
  send: '<path d="M14.54 21.69a.5.5 0 0 0 .94-.03l6.5-19a.5.5 0 0 0-.64-.64l-19 6.5a.5.5 0 0 0-.03.94l7.93 3.18a2 2 0 0 1 1.11 1.11z"/><path d="m21.85 2.15-10.94 10.94"/>',
  'zed-hammer': '<g transform="scale(1.5)" stroke-width="1.2"><path d="M9 8.5L4.95 12.62a1.25 1.25 0 0 1-1.75-1.75L7.5 6.5"/><path d="M10.84 9.98l3-3"/><path d="M12.84 7.42l-1.07-1a1 1 0 0 1-.33-.73v-.61L10.17 3.9A3.4 3.4 0 0 0 7.82 3l-1.98-.01.52.43a2.9 2.9 0 0 1 1.15 2.4L7.5 6.5 9 8.5l.5-.5s.37-.2.58-.01l1.07 1"/></g>',
  lightbulb: '<path d="M15 14c.2-1 .7-1.7 1.5-2.5 1-.9 1.5-2.2 1.5-3.5A6 6 0 0 0 6 8c0 1 .2 2.2 1.5 3.5.7.7 1.3 1.5 1.5 2.5"/><path d="M9 18h6"/><path d="M10 22h4"/>',
  'zed-search': '<g transform="scale(1.5)" stroke-width="1.2"><path d="M13 13L11 11"/><circle cx="7.5" cy="7.5" r="4.5"/></g>',
  maximize2: '<polyline points="15 3 21 3 21 9"/><polyline points="9 21 3 21 3 15"/><line x1="21" x2="14" y1="3" y2="10"/><line x1="3" x2="10" y1="21" y2="14"/>',
  'arrow-up-left': '<path d="M7 17V7h10"/><path d="M17 17 7 7"/>',
  corner: '<polyline points="15 10 20 15 15 20"/><path d="M4 4v7a4 4 0 0 0 4 4h12"/>',
});
GLYPHS.droid = '❋';

const JBT = '--ed:#1e1f22;--panel:#26282b;--title:#26282b;--t:#dfe1e5;--mu:#b0b1b3;--ph:#6f737a;--b:#393b41;--bv:#393b41;--bf:#3574f0;--hov:#3c3e41;--sel:#43454a;--ac:#548af7;--ok:#57965d;--del:#fa6675;--warn:#e0b45c;';
const DIM = '#8d8e91';
const MONO = "'IBM Plex Mono','SF Mono',Menlo,monospace";
const W = 760;

// The conversation at the app's margins, and its rows (render_tool_call).
const tframe = (html, h, w = W) => frame(`<div style="position:absolute;inset:0;overflow:hidden;background:var(--panel);padding:14px 20px;display:flex;flex-direction:column;gap:2px">${html}</div>`, { w, h, style: JBT });
const trow = (icon, labelHtml, { trailing = '', hover = false, chevron = '', iconColor = DIM, indent = 0 } = {}) => `<div class="row" style="min-height:24px;gap:6px;padding:0 2px;margin-left:${indent}px;border-radius:5px;${hover ? 'background:var(--hov);' : ''}">
  <span style="width:24px;display:inline-flex;justify-content:center;color:${iconColor}">${icon}</span>
  <span class="row grow" style="gap:4px;min-width:0;font-size:13px;color:${DIM}">${labelHtml}</span>${trailing}${chevron ? `<span style="color:var(--ph);display:inline-flex;margin-right:2px">${ic(chevron, 'xs')}</span>` : ''}</div>`;
const subj = (text) => `<span class="trunc">${text}</span>`;
const bright = (text) => `<span class="trunc" style="color:#c4c6ca">${text}</span>`;
const dimText = (text, style = '') => `<span class="none" style="font-size:12px;color:var(--ph);${style}">${text}</span>`;
const link = (text) => `<span class="none row" style="gap:3px;font-size:12px;color:var(--ac)">${text}</span>`;
const spin = (size = 12) => `<span class="spin" style="width:${size}px;height:${size}px;border-color:rgba(84,138,247,.3);border-top-color:var(--ac);margin:0 1px"></span>`;
const okMark = ic('check', 'sm okc');
const out = (html, style = '') => `<div style="margin-left:30px;padding:4px 0 8px;display:flex;flex-direction:column;gap:6px;${style}">${html}</div>`;
const codeBlock = (text) => `<div style="border:1px solid var(--b);border-radius:6px;background:var(--ed);padding:8px;overflow:hidden"><div style="font:12px/17px ${MONO};color:#c9ccd1;white-space:pre-wrap;overflow-wrap:anywhere">${text}</div></div>`;
const inputLine = (open = false) => `<div class="row g1" style="font-size:12px;color:var(--ph)">Input${ic(open ? 'chev-up' : 'chev-down', 'xs')}</div>`;
const para = (html, style = '') => `<div style="line-height:22px;color:var(--t);padding:4px 0;${style}">${html}</div>`;
const bubble = (html) => `<div style="display:flex;justify-content:flex-end;padding:4px 0 10px"><div style="max-width:78%;background:#2f3134;border-radius:12px;padding:8px 12px;line-height:21px">${html}</div></div>`;
const code = (text) => `<code style="font:12px ${MONO};background:var(--hov);padding:1px 4px;border-radius:3px">${text}</code>`;
const caption = (text) => `<div class="xs ph" style="margin:0 0 6px">${text}</div>`;
const stack = (items) => `<div class="col" style="gap:12px">${items.map(([label, html]) => `${caption(label)}${html}`).join('')}</div>`;
const tag = (text) => `<span class="none" style="font-size:11px;padding:1px 6px;border-radius:4px;background:var(--hov);color:var(--mu)">${text}</span>`;

// The demo: a Droid Task, and two Claude Agent subagents at once.
const TASK = {
  agent: 'Factory Droid', type: 'worker', title: 'Build subthreads round picks', complexity: 'heavy', time: '4m 12s',
  prompt: 'Build what the user picked in the “Subthreads” design round of agentZ. Repo: /root/projects/agentZ. Read design/subthreads/decisions.md first, then the Subthreads section of docs/architecture.md, and build each pick as described.',
  report: `Built the seven picks in ${code('design/subthreads/decisions.md')}: the Agents list’s rows, the list folding once all are done, the subthread bar, the task card, the bottom bar and Ctrl-minus. ${code('cargo test -p app')} passes.`,
};
const TASK_JSON = `{\n  "subagent_type": "worker",\n  "description": "Build subthreads round picks",\n  "await": true,\n  "complexity": "heavy",\n  "prompt": "${TASK.prompt.replace(/“|”/g, '\\"')}"\n}`;
const EXPLORE = [
  { title: 'Find where the login view is drawn', type: 'Explore', time: '38s', state: 'done',
    prompt: 'Find the code that draws the login view in a thread.',
    steps: [['zed-search', 'Searched “render_centered”', '2 results'], ['file', 'Read crates/app/src/agent_login.rs', ''], ['file', 'Read crates/app/src/agent_view.rs', '']],
    report: `The thread’s login is drawn by ${code('AgentLogin::render_centered')} in ${code('crates/app/src/agent_login.rs')}; ${code('agent_view.rs')} puts it in the thread.` },
  { title: 'Find how Add Account logs in', type: 'Explore', time: '21s', state: 'working',
    prompt: 'Find how the Add Account dialog logs an account in.',
    steps: [['zed-search', 'Searched “LoginLayout::Dialog”', '1 result'], ['file', 'Read crates/app/src/settings_page.rs', '']],
    report: '' },
];
const userDroid = bubble('build the subthreads round’s picks');
const userClaude = bubble('where is the login view drawn, and how does Add Account share it?');
const stepRow = ([icon, text, extra], indent = 0) => trow(ic(icon, 'sm'), subj(text), { trailing: extra ? dimText(extra) : '', indent });

// Today: Droid's Task is a hammer and "Task"; Claude Agent's is a “think” call, so a lightbulb.
const todayDroid = (running) => trow(ic('zed-hammer', 'sm'), subj('Task'), { trailing: running ? spin() : '' });
const todayClaude = (s) => trow(ic('lightbulb', 'sm'), subj(s.title), { trailing: s.state === 'working' ? spin() : '' });

// 1. The row --------------------------------------------------------------------------------------
// A: Zed's subagent card header: status, title, "· type", time once done.
const card = (header, body = '', { dashed = false } = {}) => `<div style="border:1px ${dashed ? 'dashed' : 'solid'} var(--b);border-radius:6px;overflow:hidden;margin:4px 0">${header}${body}</div>`;
const cardHead = (s, { hover = false } = {}) => `<div class="row" style="height:32px;padding:0 8px;gap:6px;background:${hover ? 'var(--hov)' : '#2b2d30'}"><span style="width:16px;display:inline-flex;justify-content:center">${s.state === 'working' ? spin() : s.state === 'failed' ? ic('x', 'sm delc') : okMark}</span><span class="trunc" style="font-size:13px;flex:0 1 auto">${s.title}</span><span class="sm mu" style="flex:none">· ${s.type}</span><span class="grow"></span>${s.state === 'done' ? dimText(s.time) : ''}${hover ? `<span style="color:var(--ph);display:inline-flex">${ic('chev-down', 'xs')}</span>` : ''}</div>`;
// B: a sentence, as agentZ's own tools read.
const sentence = (s) => trow(s.state === 'working' ? spin() : ic('users', 'sm'), `${s.state === 'working' ? '<span class="none">Running a subagent:</span>' : '<span class="none">Ran a subagent:</span>'}${bright(s.title)}`, { trailing: `${dimText(s.type)}${s.state === 'done' ? dimText(`· ${s.time}`) : ''}` });
// C: the title, its type, and the status at the end.
const plainRow = (s) => trow(ic('bot', 'sm'), `${bright(s.title)}${tag(s.type)}`, { trailing: s.state === 'working' ? spin() : `${dimText(s.time)}<span style="display:inline-flex;margin-left:4px">${okMark}</span>` });

const DROID_RUNNING = { ...TASK, state: 'working' };
const DROID_DONE = { ...TASK, state: 'done' };
const rowMock = (render) => stack([
  ['Factory Droid, running and done', tframe(`${userDroid}${render(DROID_RUNNING)}${render(DROID_DONE)}`, 150)],
  ['Claude Agent, two at once', tframe(`${userClaude}${EXPLORE.map(render).join('')}`, 150)],
]);

TOPICS.push({
  id: 'row', section: 'In the conversation', title: 'A subagent’s row', size: 'wide', rec: 'A',
  now: 'A subagent is a tool call like any other. Factory Droid’s reads “Task” with the hammer every unnamed tool gets, and a spinner while it runs. Claude Agent marks its call as thinking, so it gets the lightbulb, with the subagent’s description as its label (“Find where the login view is drawn”). Nothing says it’s a subagent, what kind, or how long it ran. With the mock agent, both kinds: <a href="img/now-subagent-calls-closed.png">now-subagent-calls-closed.png</a>.',
  nowImg: '../feedback/evidence/24-agent-subagent-call.png',
  issues: ['Droid’s “Task” doesn’t say what it’s doing', 'Neither looks like a subagent; Claude’s looks like thinking'],
  options: [
    { key: 'A', name: 'Zed’s subagent card', from: 'Zed (render_subagent_card)',
      desc: 'A bordered card with Zed’s subagent header: a spinner, a green check or a red cross, the description (“Build subthreads round picks”), then the subagent’s type, muted (“· worker”, “· Explore”), and how long it ran once done. A chevron shows on hover; a click opens it. Zed puts the model there; agents don’t report their subagents’ models.',
      good: 'Stands apart from tool rows, and matches the Agents list’s rows (picked from the same Zed header).', cost: 'Taller than a row: a run of subagents is a stack of cards.',
      mock: () => rowMock((s) => card(cardHead(s))) },
    { key: 'B', name: 'A sentence, like agentZ’s own tools', from: 'the Tool calls round (topic 7 A, from t3code)',
      desc: 'A row that says what happened, as agentZ’s own tools now do: “Running a subagent: Build subthreads round picks” with a spinner, then “Ran a subagent: …”, with the people icon, the type and the time. A folded run counts them (“Ran 2 subagents”).',
      good: 'Reads like the rest of the thread; one line each.', cost: 'Looks like any tool call at a glance.',
      mock: () => rowMock(sentence) },
    { key: 'C', name: 'Its title and type on a row', from: 'new',
      desc: 'A row with a bot icon, the description in the row’s brighter gray, the type as a small tag, and a spinner, or the time and a check once done.',
      good: 'The least change: a row with a better label.', cost: 'Says less than A or B that it’s an agent at work.',
      mock: () => rowMock(plainRow) },
    { key: 'D', name: 'As it is', from: 'today',
      desc: 'The hammer and “Task” for Droid, the lightbulb and the description for Claude Agent.',
      good: 'No change.', cost: 'The issues stay.',
      mock: () => stack([
        ['Factory Droid, running and done', tframe(`${userDroid}${todayDroid(true)}${todayDroid(false)}`, 150)],
        ['Claude Agent, two at once', tframe(`${userClaude}${EXPLORE.map(todayClaude).join('')}`, 150)],
      ]) },
  ],
});

// 2. Opened ---------------------------------------------------------------------------------------
const taskText = (s, tags) => `<div class="col" style="gap:4px"><div class="row g2" style="font-size:12px;color:var(--ph)">Task${tags.map(tag).join('')}</div><div style="font-size:13px;line-height:20px;color:#c4c6ca">${s.prompt}</div></div>`;
const reportText = (html) => `<div class="col" style="gap:4px"><div style="font-size:12px;color:var(--ph)">Report</div>${para(html, 'padding:0')}</div>`;
const preview = (steps, { running = false } = {}) => `<div style="position:relative;padding:4px 6px">${steps.map((s) => stepRow(s)).join('')}${running ? '' : ''}</div>`;
const fullStrip = `<div class="row" style="justify-content:center;height:24px;border-top:1px solid var(--b);color:var(--ph)">${ic('maximize2', 'xs')}</div>`;
TOPICS.push({
  id: 'open', section: 'In the conversation', title: 'What opening it shows', size: 'wide', rec: 'A',
  now: 'As picked in the Tool calls round, an opened row shows the tool’s output, with its input as JSON behind “Input” at the end. Factory Droid’s Task has no output while it runs, so it opens to only “Input”: the type, description, await, complexity and prompt as JSON. Once done, its output is the subagent’s report. Claude Agent’s shows its prompt while it runs and its report once done. The mocks use row A from the topic before.',
  nowImg: 'img/now-subagent-calls.png',
  issues: ['What the subagent was asked to do is only in raw JSON', 'The report is plain tool output'],
  options: [
    { key: 'A', name: 'The task, then the report', from: 'new',
      desc: 'Opened, it shows “Task” with the prompt as text (wrapped, not JSON) and the options as small tags (“heavy”), then “Report” with the subagent’s report as markdown once it’s done. “Input” stays at the end for the JSON.',
      good: 'Reads as what it was asked and what it said.', cost: 'Long prompts make it tall.',
      mock: () => tframe(`${userDroid}${card(cardHead(DROID_DONE), out(`${taskText(TASK, ['heavy'])}${reportText(TASK.report)}${inputLine()}`, 'margin:0;padding:10px 12px'))}`, 400) },
    { key: 'B', name: 'Zed’s preview of its work', from: 'Zed (render_subagent_expanded_content)',
      desc: 'Zed’s: while it runs, the card shows the step the subagent is on. Opened, its last steps (up to 8, fading at the top) and its report; a strip at the bottom opens all of it (as a subthread, if topic 3 picks B). Only agents that report the steps (Claude Agent) have them; Droid’s opens as A.',
      good: 'You see the subagent at work, as in Zed.', cost: 'Needs topic 3’s A or B; nothing for Droid.',
      mock: () => tframe(`${userClaude}${card(cardHead(EXPLORE[1]), `<div style="border-top:1px solid var(--b)">${preview(EXPLORE[1].steps.slice(-1))}</div>`)}${card(cardHead(EXPLORE[0]), `<div style="border-top:1px solid var(--b)">${preview(EXPLORE[0].steps)}${out(para(EXPLORE[0].report, 'padding:0'), 'margin:0;padding:4px 12px 10px')}</div>${fullStrip}`)}`, 400) },
    { key: 'C', name: 'Only the report', from: 'new',
      desc: 'Opened, it shows the report. The prompt is behind a “Task” line at the end, beside “Input”, as text.',
      good: 'Short.', cost: 'What it was asked is a click away.',
      mock: () => tframe(`${userDroid}${card(cardHead(DROID_DONE), out(`${para(TASK.report, 'padding:0')}<div class="row g3">${inputLine().replace('Input', 'Task')}${inputLine()}</div>`, 'margin:0;padding:10px 12px'))}`, 400) },
    { key: 'D', name: 'As it is', from: 'today',
      desc: 'The output, then “Input” with the JSON.',
      good: 'No change.', cost: 'The issues stay.',
      mock: () => tframe(`${userDroid}${todayDroid(false)}${out(`${para(TASK.report, 'padding:0')}${inputLine(true)}${codeBlock(TASK_JSON)}`)}`, 400) },
  ],
});

// 3. The subagent's own steps --------------------------------------------------------------------------
const interleaved = `${userClaude}${todayClaude(EXPLORE[0])}${todayClaude(EXPLORE[1])}${stepRow(EXPLORE[0].steps[0])}${stepRow(EXPLORE[1].steps[0])}${stepRow(EXPLORE[0].steps[1])}${para('I’ll look at how the dialog lays the login out.', 'font-size:14px')}${stepRow(EXPLORE[1].steps[1])}${stepRow(EXPLORE[0].steps[2])}${para(EXPLORE[0].report, 'font-size:14px')}`;
const foldedSteps = (s) => `<div class="row g1" style="padding:4px 8px 6px 30px;font-size:12px;color:var(--ph);border-top:1px solid var(--bv)">${ic('chev-right', 'xs')}${s.steps.length === 3 ? 'Searched once, read 2 files' : 'Searched once, read 1 file'}</div>`;
const agentsBar = (rows, head) => `<div style="padding:0 8px;flex:none"><div style="background:#2b2d30;border:1px solid var(--b);border-bottom:0;border-radius:6px 6px 0 0">${head}${rows}</div></div>`;
const composer = () => `<div style="border-top:1px solid var(--b);background:var(--ed);padding:8px 16px;flex:none"><div class="ph" style="padding:4px 4px 12px">Message the agent…</div><div class="row" style="gap:10px;font-size:12px;color:var(--mu)">${ic('plus', 'sm')}<span class="row g1">${glyph('claude', 'sm')}Claude Agent</span><span class="grow"></span><span class="ibtn" style="background:var(--sel);color:var(--t)">${ic('send', 'xs')}</span></div></div>`;
const listHead = (count, extra) => `<div class="row g1" style="padding:4px;border-bottom:1px solid var(--b)"><span class="ibtn sm">${ic('chev-down', 'xs')}</span><span class="sm mu">${count} ${count === 1 ? 'Agent' : 'Agents'}</span>${extra ? `<span class="sm mu">· ${extra}</span>` : ''}</div>`;
const listRow = (s, { last = false, own = false } = {}) => `<div class="row" style="height:32px;padding:0 8px;gap:6px;${last ? '' : 'border-bottom:1px solid var(--bv)'}"><span style="width:16px;display:inline-flex;justify-content:center">${s.state === 'working' ? spin() : okMark}</span><span class="trunc" style="font-size:13px;flex:0 1 auto">${s.title}</span><span class="sm mu" style="flex:none">· ${s.model || s.type}</span><span class="grow"></span>${own ? dimText('Claude Agent’s') : ''}</div>`;
const withList = (convo, bar, h = 420) => frame(`<div class="col" style="height:100%;background:var(--panel)"><div class="grow" style="min-height:0;overflow:hidden;padding:14px 20px">${convo}</div>${bar}${composer()}</div>`, { w: W, h, style: JBT });
const openRow = (s) => card(cardHead(s).replace('<span class="grow"></span>', `<span class="grow"></span>${link(`Open${ic('arrow-up-right', 'xs')}`)}`));
TOPICS.push({
  id: 'steps', section: 'The subagent’s work', title: 'Claude Agent’s subagents’ own steps', size: 'wide', rec: 'B',
  now: 'Claude Agent sends its subagents’ tool calls and text into the thread, each marked with the call it belongs to (<code>parentToolUseId</code>). agentZ doesn’t read the mark, so they show as the agent’s own rows and words, between its real ones; two subagents at once interleave, as in option D’s mock. Claude Agent can instead give each subagent a session of its own, when the app says it takes them (the <code>subagents</code> capability): it then announces the subagent (<code>subagent_spawned</code>, with its name, task and prompt), sends its steps to that session, and says when it ends. Devin marks its subagents in its updates too, which t3code reads. Factory Droid sends no steps. From the agents’ code, not seen in agentZ: that takes a prompt to your real Claude.',
  nowImg: 'img/now-subagent-calls.png',
  issues: ['A subagent’s searches, reads and words look like the agent’s own', 'Several at once are mixed together'],
  options: [
    { key: 'A', name: 'Inside the subagent’s card', from: 'Zed (its subagent cards hold their thread)',
      desc: 'agentZ reads the mark and puts each step in its subagent’s card instead of the thread, folded under one line (“Searched once, read 2 files”) that opens them. The thread shows only the agent’s own steps and words.',
      good: 'The thread reads as the agent’s own again; no new kind of thread.', cost: 'Each step is only in its card, so a long subagent’s work is nested deep.',
      mock: () => tframe(`${userClaude}${EXPLORE.map((s) => card(cardHead(s), foldedSteps(s))).join('')}${para('The thread’s login is drawn by <code style="font:12px monospace">AgentLogin::render_centered</code>; the Add Account dialog…', 'font-size:14px')}`, 330) },
    { key: 'B', name: 'As subthreads', from: 't3code (child threads), with Claude Agent’s subagent sessions',
      desc: 'agentZ says it takes subagent sessions, and makes each subagent a subthread of the thread, as agentZ’s own are: in the Agents list (topic 4), and opening with the subthread header and bar the Subthreads round picked, its steps and words in its own conversation. It runs on its own, so its bar has no composer. The card in the thread ends in Open. Devin’s marked subagents could go the same way; Droid’s, which send no steps, stay a card.',
      good: 'One idea of a subagent across the app, and a full view of its work.', cost: 'The most work, and Claude Agent’s subagent sessions are new.',
      mock: () => withList(`${userClaude}${EXPLORE.map(openRow).join('')}`, agentsBar(EXPLORE.map((s, i) => listRow(s, { last: i === EXPLORE.length - 1 })).join(''), listHead(2, '1 running')), 330) },
    { key: 'C', name: 'Left out', from: 'new',
      desc: 'agentZ drops what’s marked as a subagent’s; only the call and its report show.',
      good: 'Simple; the thread is the agent’s own.', cost: 'You can’t see what a subagent did.',
      mock: () => tframe(`${userClaude}${EXPLORE.map((s) => card(cardHead(s))).join('')}`, 200) },
    { key: 'D', name: 'As it is', from: 'today',
      desc: 'The steps show as the agent’s own.',
      good: 'No change.', cost: 'The issues stay.',
      mock: () => tframe(interleaved, 330) },
  ],
});

// 4. The Agents list ------------------------------------------------------------------------------
const SUBTHREAD = { title: 'Research: UI for child tasks', model: 'Opus 5.5', state: 'done' };
TOPICS.push({
  id: 'list', section: 'The subagent’s work', title: 'In the Agents list', size: 'wide', rec: 'A',
  now: 'The Agents list over the composer lists only agentZ’s subthreads (started with <code>delegate_task</code>), each with its status, title, “· model” and the files it changed, as the Subthreads round picked. It opens while one runs and folds to “N Agents · all done” once the last ends. An agent’s own subagents aren’t in it. t3code’s bar, which the list was based on, lists the agent’s own with its threads. The mocks use row A from topic 1.',
  nowImg: 'img/now-subagent-calls-closed.png',
  issues: ['A thread busy with subagents shows nothing running over the composer'],
  options: [
    { key: 'A', name: 'Listed with subthreads', from: 't3code (ProviderSubagentBar)',
      desc: 'An agent’s own subagents get rows in the list, the same as subthreads’ (status, title, “· worker”), newest first, with the agent’s name at the end. They count toward “N Agents · 1 running” and the list’s folding. A click opens it as a subthread (topic 3 B), or scrolls to its card.',
      good: 'One place to see everything at work in the thread.', cost: 'The list grows with every subagent, and Claude Agent starts many.',
      mock: () => withList(`${userClaude}${EXPLORE.map((s) => card(cardHead(s))).join('')}`, agentsBar([...EXPLORE.map((s) => listRow(s, { own: true })), listRow(SUBTHREAD, { last: true })].join(''), listHead(3, '1 running')), 380) },
    { key: 'B', name: 'Only while they run', from: 'new',
      desc: 'As A, but an agent’s own subagent leaves the list once it ends; its card stays in the conversation.',
      good: 'Shows what’s at work without filling up.', cost: 'A finished one is only in the conversation.',
      mock: () => withList(`${userClaude}${EXPLORE.map((s) => card(cardHead(s))).join('')}`, agentsBar([listRow(EXPLORE[1], { own: true }), listRow(SUBTHREAD, { last: true })].join(''), listHead(2, '1 running')), 330) },
    { key: 'C', name: 'Not listed', from: 'Zed',
      desc: 'The list stays for agentZ’s subthreads; an agent’s own subagents are only in the conversation, as in Zed.',
      good: 'The list keeps one meaning.', cost: 'Nothing over the composer says a subagent is running.',
      mock: () => withList(`${userClaude}${EXPLORE.map((s) => card(cardHead(s))).join('')}`, agentsBar(listRow(SUBTHREAD, { last: true }), listHead(1, '')), 330) },
  ],
});
