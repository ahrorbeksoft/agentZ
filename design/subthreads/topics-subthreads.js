// Subthreads: the Agents list above a parent's composer, how an open subthread looks, the way
// back to its parent, and what sits where the composer would be. agent_view.rs's
// render_agents_section, render_toolbar, render_entry's "Sent by" label and render_subthread_bar.

ICONS['x-circle'] = '<circle cx="12" cy="12" r="10"/><path d="m15 9-6 6"/><path d="m9 9 6 6"/>';
ICONS['arrow-up-right'] = '<path d="M7 7h10v10"/><path d="M7 17 17 7"/>';
ICONS['arrow-up-left'] = '<path d="M7 17V7h10"/><path d="M17 17 7 7"/>';
ICONS['arrow-left'] = '<path d="m12 19-7-7 7-7"/><path d="M19 12H5"/>';
ICONS.dash = '<path d="M5 12h14"/>';
ICONS.sparkle = '<path d="M9.937 15.5A2 2 0 0 0 8.5 14.063l-6.135-1.582a.5.5 0 0 1 0-.962L8.5 9.936A2 2 0 0 0 9.937 8.5l1.582-6.135a.5.5 0 0 1 .963 0L14.063 8.5A2 2 0 0 0 15.5 9.937l6.135 1.581a.5.5 0 0 1 0 .964L15.5 14.063a2 2 0 0 0-1.437 1.437l-1.582 6.135a.5.5 0 0 1-.963 0z"/>';
ICONS.warning = '<path d="m21.73 18-8-14a2 2 0 0 0-3.48 0l-8 14A2 2 0 0 0 4 21h16a2 2 0 0 0 1.73-3"/><path d="M12 9v4"/><path d="M12 17h.01"/>';
ICONS.send = '<path d="M14.54 21.69a.5.5 0 0 0 .94-.03l6.5-19a.5.5 0 0 0-.64-.64l-19 6.5a.5.5 0 0 0-.03.94l7.93 3.18a2 2 0 0 1 1.11 1.11z"/><path d="m21.85 2.15-10.94 10.94"/>';
ICONS.gauge = '<path d="m12 14 4-4"/><path d="M3.34 19a10 10 0 1 1 17.32 0"/>';
GLYPHS.droid = '❋';

const SW = 780;
const PARENT = 'i wanna test subthreads, spawn some sub threads to research how they work';
const PARENT_SHORT = 'i wanna test subthreads, spawn some sub threads…';
const SUB = 'Research: UI for child tasks';

// Newest first, as ProjectStore::subthreads orders them.
const SUBS = [
  { title: 'Test: run the delegation tests', agent: 'Claude Agent', glyph: 'claude', model: 'Opus 5.5', role: 'test', state: 'approval', time: '31s', files: [1, 12, 0] },
  { title: 'Review: server side of delegated tasks', agent: 'Codex', glyph: 'codex', model: 'GPT-5.5', role: 'review', state: 'working', time: '52s' },
  { title: 'Research: how agents get agentZ’s tools', agent: 'Factory Droid', glyph: 'droid', model: 'Opus 5.5', role: 'research', state: 'done', time: '1m 48s' },
  { title: SUB, agent: 'Factory Droid', glyph: 'droid', model: 'Opus 5.5', role: 'research', state: 'done', time: '2m 14s' },
];
const ALL_DONE = SUBS.map((s, i) => ({ ...s, state: 'done', time: ['3m 02s', '2m 40s', '1m 48s', '2m 14s'][i] }));
const RUNNING = (subs) => subs.filter((s) => s.state !== 'done' && s.state !== 'failed').length;

// Today's status words and colors (render_agents_section).
const STATUS = {
  done: { word: 'Done', color: 'var(--ok)', icon: () => ic('check', 'sm okc') },
  working: { word: 'Working', color: 'var(--ac)', icon: () => '<span class="spin" style="width:12px;height:12px;margin:0 1px"></span>' },
  approval: { word: 'Needs approval', color: 'var(--warn)', icon: () => ic('warning', 'sm warnc') },
  failed: { word: 'Failed', color: 'var(--del)', icon: () => ic('x-circle', 'sm delc') },
};

const chevD = ic('chev-down', 'xs mu');
const code = (text) => `<code style="font:12px 'IBM Plex Mono',monospace;background:var(--hov);padding:1px 4px;border-radius:3px">${text}</code>`;
const tip = (html, style) => `<div class="pop" style="${style};padding:5px 8px;font-size:12px;white-space:nowrap;display:flex;align-items:center;gap:10px">${html}</div>`;

// The header (render_toolbar): project crumb, "/", the title as its menu's trigger, then the
// branch and the thread's buttons.
const azBadge = '<span class="mono" style="background:#4b3034;color:#e06c75;width:14px;height:14px;font-size:7px">AZ</span>';
const crumbProject = `<span class="row g15" style="padding:2px 4px;flex:none">${azBadge}<span class="sm mu">agentZ</span></span><span class="sm mu" style="flex:none">/</span>`;
const titleMenu = (title) => `<span class="row g1" style="padding:2px 4px;min-width:0;font-size:13px"><span class="trunc">${title}</span>${chevD}</span>`;
const parentCrumb = ({ hover = false } = {}) => `<span class="row" style="padding:2px 4px;border-radius:4px;min-width:0;max-width:230px;flex:0 1 auto;${hover ? 'background:var(--hov);color:var(--t)' : 'color:var(--mu)'}"><span class="trunc sm">${PARENT}</span></span><span class="sm mu" style="flex:none">/</span>`;
const headerRight = (extra = '') => `<span class="row g15" style="flex:none">${extra}<span class="btn sm" style="gap:4px;height:22px">${ic('branch', 'xs')}main</span><span class="ibtn" style="font-size:15px">±</span>${ibtn('terminal')}${ibtn('more')}</span>`;
const threadHeader = ({ lead = '', crumbs = crumbProject, title = titleMenu(SUB), right = headerRight(), style = '' } = {}) =>
  `<div class="row g1" style="height:36px;flex:none;padding:0 8px;border-bottom:1px solid var(--b);background:var(--panel);position:relative;${style}">${lead}${crumbs}<div class="row grow" style="min-width:0">${title}</div>${right}</div>`;
const breadcrumbHeader = (opts = {}) => threadHeader({ crumbs: crumbProject + parentCrumb(opts), ...opts });

// The subthread's conversation: the task (its only prompt, from the parent), the folded work
// and the start of its answer.
const sentByLabel = `<div class="row g1" style="justify-content:flex-end;padding:0 4px;font-size:11px;color:var(--mu)">${ic('sparkle', 'xs')}Sent by the agent in “${PARENT_SHORT}”</div>`;
const TASK = `Read-only research in the agentZ repo. Do not edit any files. Goal: explain how the app shows a thread’s delegated child tasks (subagents) to the user, mainly in ${code('crates/app')}. Start with ${code('docs/architecture.md')}, then read the code.`;
const taskBubble = `<div style="display:flex;justify-content:flex-end"><div style="max-width:80%;background:#3a3f4b;border-radius:10px;padding:8px 12px;line-height:21px">${TASK}</div></div>`;
const sentTime = '<div class="xs ph" style="text-align:right;padding:0 4px">08:12</div>';
const workGroup = `<div class="row g2 mu" style="padding:4px 18px;font-size:13px">${ic('chev-right', 'xs')}Searched code 15 times, ran 2 commands, and performed 18 other actions</div>`;
const answer = `<div style="line-height:22px;padding:0 4px">The app treats a delegated child task as a “subthread”: a normal thread with a ${code('task')} field. The sidebar hides subthreads, and the parent thread lists them in its Agents control.</div>`;
const subConversation = ({ top = sentByLabel, bubble = taskBubble } = {}) => `<div class="col" style="padding:12px 20px 0;gap:8px">${top}${bubble}${sentTime}${workGroup}${answer}</div>`;

// The activity bar over the composer (render_activity_bar), and its Plan row.
const activityBar = (sections) => `<div style="padding:0 8px;flex:none;position:relative"><div style="background:#31353e;border:1px solid var(--b);border-bottom:0;border-radius:6px 6px 0 0;box-shadow:1px -1px 2px rgba(0,0,0,.12)">${sections.join('<div style="height:1px;background:var(--b)"></div>')}</div></div>`;
const planRow = `<div class="row g1" style="padding:4px;font-size:12px"><span class="ibtn sm">${ic('chev-right', 'xs')}</span><span class="mu grow">Plan</span><span class="mu">All Done</span><span class="ibtn sm">${ic('x', 'xs')}</span></div>`;

// The composer as a Factory Droid thread shows it (ComposerStyle::Bar).
const composerBar = () => `<div style="border-top:1px solid var(--b);background:var(--ed);padding:8px 16px;flex:none">
  <div class="ph" style="padding:4px 4px 12px">Message the agent…</div>
  <div class="row" style="gap:10px;font-size:12px;color:var(--mu)">${ic('plus', 'sm')}<span class="row g1">${glyph('droid', 'sm')}Factory Droid</span><span class="chip" style="height:18px;padding:0 4px;font-size:11px;gap:3px">${ic('gauge', 'xs')}88%</span><span class="grow"></span><span class="row g1">Auto (High) ${chevD}</span><span class="row g1">Opus 5.5 ${chevD}</span><span class="row g1">Extra High ${chevD}</span><span class="ibtn" style="background:var(--sel);color:var(--t)">${ic('send', 'xs')}</span></div></div>`;

// The Agents list ---------------------------------------------------------------------------
const agentsHeader = (subs, { open = true, done = false } = {}) => {
  const running = RUNNING(subs);
  const extra = done ? '<span class="sm mu">· all done</span>' : running ? `<span class="sm mu">· ${running} running</span>` : '';
  return `<div class="row g1" style="padding:4px;${open ? 'border-bottom:1px solid var(--b)' : ''}"><span class="ibtn sm">${ic(open ? 'chev-down' : 'chev-right', 'xs')}</span><span class="sm mu">${subs.length} Agents</span>${extra}</div>`;
};
const rowWrap = (html, last, { h, pad = '6px', hover = false, align = 'center' } = {}) => `<div class="row" style="${h ? `height:${h}px;` : ''}padding:${pad};gap:6px;align-items:${align};background:${hover ? 'var(--hov)' : 'var(--ed)'};${last ? '' : 'border-bottom:1px solid var(--bv)'};position:relative">${html}</div>`;
const listOf = (subs, row, opts = {}) => agentsHeader(subs, opts) + subs.map((s, i) => row(s, i === subs.length - 1, i)).join('');

// Today: status icon, title, "Agent · role", status word.
const rowToday = (s, last) => rowWrap(`${STATUS[s.state].icon()}<span class="grow trunc sm">${s.title}</span><span class="sm mu" style="flex:none">${s.agent} · ${s.role}</span><span class="sm" style="flex:none;color:${STATUS[s.state].color}">${STATUS[s.state].word}</span>`, last);

// t3code's lineage row: the agent's icon with a status dot, the title, one trailing item.
const dotColor = (state) => ({ done: 'var(--ok)', working: 'var(--ac)', approval: 'var(--warn)', failed: 'var(--del)' })[state];
const agentWithDot = (s) => `<span style="position:relative;display:inline-flex;flex:none;margin-right:2px">${glyph(s.glyph)}<i style="position:absolute;right:-3px;bottom:-3px;width:9px;height:9px;border-radius:50%;background:${dotColor(s.state)};border:2px solid var(--ed)"></i></span>`;
const t3Trailing = (s, hover) => {
  if (hover && s.state !== 'done') return `<span class="ibtn sm hov" style="color:var(--del)"><svg class="i xs" viewBox="0 0 24 24" style="fill:currentColor">${ICONS.stop}</svg></span>`;
  if (s.state === 'approval') return '<span class="xs warnc" style="flex:none">Needs approval</span>';
  if (s.state === 'failed') return '<span class="xs delc" style="flex:none">Failed</span>';
  return `<span class="xs mu" style="flex:none">${s.time}</span>`;
};
const rowT3 = (s, last, i, hoverIndex = -1) => rowWrap(`${agentWithDot(s)}<span class="grow trunc" style="font-size:13px;font-weight:500">${s.title}</span>${t3Trailing(s, i === hoverIndex)}`, last, { h: 32, pad: '0 8px', hover: i === hoverIndex });
const hoverCard = (s, style) => `<div class="pop" style="${style};width:300px;padding:10px 12px;display:flex;flex-direction:column;gap:6px;font-size:12px">
  <span class="b5" style="font-size:13px">${s.title}</span>
  <span class="row g2 mu">${glyph(s.glyph, 'sm')}${s.agent} · ${s.model}</span>
  <span class="row" style="justify-content:space-between"><span class="row g1" style="color:${dotColor(s.state)}">${s.state === 'done' ? ic('check', 'xs') : '<span class="spin" style="width:9px;height:9px"></span>'}${s.state === 'done' ? 'Done' : 'Working'}</span><span class="mu">${s.time}</span></span>
  <span class="row g2 mu" style="min-width:0">${ic('terminal', 'xs')}<span class="trunc">${s.state === 'done' ? 'The app treats a delegated child task as a “subthread”…' : 'Reading crates/agentz_server/src/server/tools.rs'}</span></span></div>`;

// Zed's subagent card header: status, title, "· model", "— N files changed +a −d", Stop.
const rowZed = (s, last) => {
  const changed = s.files ? `<span class="sm mu" style="flex:none">— ${s.files[0]} file changed</span><span class="sm" style="flex:none;color:var(--ok)">+${s.files[1]}</span><span class="sm" style="flex:none;color:var(--del)">−${s.files[2]}</span>` : '';
  const stop = s.state === 'working' || s.state === 'approval' ? `<span class="ibtn sm" style="color:var(--del)">${ic('stop', 'xs')}</span>` : '';
  return rowWrap(`<span style="width:16px;display:inline-flex;justify-content:center">${STATUS[s.state].icon()}</span><span class="trunc" style="font-size:13px;flex:0 1 auto">${s.title}</span><span class="sm mu" style="flex:none">· ${s.model}</span><span class="grow"></span>${changed}${stop}`, last, { h: 32, pad: '0 8px' });
};

// Two lines: the title, then the agent, model, role and time in muted text.
const roleName = (role) => role[0].toUpperCase() + role.slice(1);
const rowTwoLine = (s, last) => {
  const time = s.state === 'done' ? `Done in ${s.time}` : s.state === 'approval' ? `<span class="warnc">Needs approval</span>` : `Working ${s.time}`;
  return rowWrap(`<span style="width:16px;display:inline-flex;justify-content:center;margin-top:2px">${STATUS[s.state].icon()}</span><div class="col grow" style="min-width:0;gap:1px"><span class="trunc" style="font-size:13px">${s.title}</span><span class="row g1 xs mu" style="min-width:0">${glyph(s.glyph, 'sm')}<span class="trunc">${s.agent} · ${s.model} · ${roleName(s.role)} · ${time}</span></span></div>`, last, { pad: '6px 8px', align: 'flex-start' });
};

const parentTail = `<div style="padding:16px 24px 0;line-height:22px">I started four subthreads: two research how the UI and the tools work, one reviews the server side, and one runs the tests. I’ll put their findings together once they’re done.</div>`;
const parentView = (list, { h = 330, overlay = '', sections } = {}) => frame(`<div class="col" style="height:100%;background:var(--panel)"><div class="grow" style="min-height:0;overflow:hidden">${parentTail}</div>${activityBar(sections || [list])}${composerBar()}</div>${overlay}`, { w: SW, h });

TOPICS.push({
  id: 'rows', section: 'Agents list', title: 'The rows', size: 'wide', rec: 'A',
  now: 'The list above a parent’s composer, under a line like “3 Agents · 1 running”. Each row has a status icon, the title, the agent and the task’s role (“Factory Droid · research”), and a status word: Done, Working, Needs approval, Waiting, Failed, Cancelled or Stopped. The role is what the parent agent passed to <code>delegate_task</code>: implementation, research, review, design, test or general. Newest first; six rows show, then it scrolls. A click opens the subthread. In every option the “4 Agents” line above the rows stays as it is.',
  nowImg: '../feedback/evidence/11-agents-list.png',
  issues: [
    'The role repeats the title’s first word: “Research: …” and “research”',
    'Every row repeats the same agent name',
    'The status shows twice: a green check and a green “Done”',
    'The lowercase role sits beside the capitalized agent name',
  ],
  options: [
    { key: 'A', name: 'Agent icon with a status dot, title, time', from: 't3code lineage rows',
      desc: 'Each row is the agent’s icon with a small status dot on its corner (green done, blue working, yellow needs approval, red failed), the title, and one thing at the end: how long it ran, or “Needs approval” or “Failed” in color. Hovering a row shows t3code’s card beside it: the agent and model, the status and time, and what it’s doing or the start of its result. While it runs, hovering swaps the time for Stop. The role goes.',
      good: 'One status mark per row, and the title gets the room.', cost: 'The agent and model are only on hover, behind its small icon.',
      mock: () => parentView(listOf(SUBS, (s, last, i) => rowT3(s, last, i, 1)), { h: 360, overlay: hoverCard(SUBS[1], 'top:12px;right:24px') }) },
    { key: 'B', name: 'Status, title and model, with changed files', from: 'Zed subagent cards',
      desc: 'Zed’s subagent header as a row: a spinner, green check or red cross, the title, then “· Opus 5.5” muted. When the subthread changed files, “— 1 file changed +12 −0” follows. A running row has a red Stop button at its end. No status word and no role.',
      good: 'Shows what each subthread changed, as Zed does.', cost: 'Research tasks change nothing, so their rows show only the title and model.',
      mock: () => parentView(listOf(SUBS, rowZed), { h: 330 }) },
    { key: 'C', name: 'Two lines: title, then details', from: 'new',
      desc: 'The title gets its own line. Under it, in small muted text, the agent’s icon and “Factory Droid · Opus 5.5 · Research · Done in 2m 14s”. The role is capitalized, and the time joins the status.',
      good: 'Everything is still there, and nothing crowds the title.', cost: 'Rows are taller, so fewer fit before the list scrolls.',
      mock: () => parentView(listOf(SUBS, rowTwoLine), { h: 400 }) },
    { key: 'D', name: 'As it is', from: 'today',
      desc: 'Status icon, title, “Factory Droid · research”, and the status word.',
      good: 'Nothing to change.', cost: 'The labels repeat what the title and icon already say.',
      mock: () => parentView(listOf(SUBS, rowToday), { h: 330 }) },
  ],
});

// Finished subthreads ------------------------------------------------------------------------
const previousRow = (count, last = true) => rowWrap(`<span style="width:16px;display:inline-flex;justify-content:center" class="mu">${ic('chev-right', 'xs')}</span><span class="sm mu grow">Previous agents (${count})</span>`, last, { h: 30, pad: '0 8px' });

TOPICS.push({
  id: 'finished', section: 'Agents list', title: 'Finished subthreads', size: 'wide', rec: 'A',
  now: 'Finished subthreads stay in the list for good, and can’t be hidden one by one. You said that’s fine. A click on the “4 Agents” line folds the whole list, which starts open each time the thread opens. The mocks use rows A from the topic before.',
  options: [
    { key: 'A', name: 'All stay in the list', from: 'today',
      desc: 'Every subthread stays listed, running or finished, newest first. The “4 Agents” line folds the whole list.',
      good: 'Nothing moves while you look at it.', cost: 'A thread that started many subthreads has a long list.',
      mock: () => parentView(listOf(SUBS, rowT3), { h: 330 }) },
    { key: 'B', name: 'Finished ones folded under “Previous agents”', from: 't3code lineage',
      desc: 'Running subthreads are listed first. Finished ones go under a “Previous agents (2)” row at the end, which starts folded and opens on a click.',
      good: 'What’s still running is all you see.', cost: 'A subthread moves into the folded group as it finishes.',
      mock: () => parentView(agentsHeader(SUBS) + SUBS.slice(0, 2).map((s, i) => rowT3(s, false, i)).join('') + previousRow(2), { h: 290 }) },
    { key: 'C', name: 'The list folds once all are done', from: 'new',
      desc: 'While any subthread runs, the list is open. Once the last one finishes, it folds to its “4 Agents · all done” line, and opens again if a new one starts. A click on that line opens it.',
      good: 'Finished work takes one line until you want it.', cost: 'The list folds on its own, which can surprise.',
      mock: () => parentView(agentsHeader(ALL_DONE, { open: false, done: true }), { h: 230 }) },
  ],
});

// An open subthread ---------------------------------------------------------------------------
const subthreadBarToday = `<div class="row g2" style="padding:8px 16px;background:var(--ed);border-top:1px solid var(--b);flex:none;font-size:12px">${glyph('droid', 'sm')}<span class="grow trunc mu">A subagent of “${PARENT_SHORT}”. It runs on its own; message its parent instead.</span><span class="btn ghost sm" style="color:var(--t)">${ic('arrow-up-right', 'xs')}Open Parent</span></div>`;
const t3Bar = ({ running = false } = {}) => `<div class="row" style="min-height:44px;gap:10px;padding:6px 8px 6px 20px;background:var(--ed);border-top:1px solid var(--b);flex:none;font-size:12px">${glyph('droid', 'sm')}<span class="b5">Opus 5.5</span><span class="mu">Extra High</span><span class="mu">${running ? 'Working 1m 20s' : 'Done in 2m 14s'}</span><span class="grow"></span><span class="mu">Runs on its own</span>${running ? `<span class="btn ghost sm" style="color:var(--t)">${ic('stop', 'xs delc')}Stop</span>` : ''}<span class="btn ghost sm" style="color:var(--t)">${ic('arrow-up-left', 'xs')}Open Parent</span></div>`;
const subView = ({ header = threadHeader(), top, bubble, bottom = subthreadBarToday, h = 400, convoBg = 'var(--panel)', overlay = '', extra = '' } = {}) =>
  frame(`<div class="col" style="height:100%;background:var(--panel)">${header}${extra}<div class="grow" style="min-height:0;overflow:hidden;background:${convoBg}">${subConversation({ top, bubble })}</div>${activityBar([planRow])}${bottom}</div>${overlay}`, { w: SW, h });

// Zed's subagent title bar under the header: an arrow, the title, its status, Stop and Minimize.
const zedBar = (running = false) => `<div class="row g2" style="height:36px;flex:none;padding:0 6px 0 10px;border-bottom:1px solid var(--b);background:rgba(40,44,51,.45)">${ic('corner', 'sm mu')}<span class="trunc" style="font-size:13px">${SUB}</span>${running ? '' : ic('check', 'sm okc')}<span class="grow"></span>${running ? `<span class="ibtn" style="color:var(--del)">${ic('stop', 'sm')}</span>` : ''}<span class="ibtn">${ic('dash', 'sm')}</span></div>`;

TOPICS.push({
  id: 'header', section: 'An open subthread', title: 'The header', size: 'wide', rec: 'A',
  now: 'A subthread’s header is a main thread’s: “agentZ / Research: UI for child tasks ⌄”, then the branch and buttons. Nothing in it says it’s a subthread, or which thread started it. The sidebar highlights the parent’s card, since subthreads aren’t in the sidebar.',
  nowImg: 'img/now-header.png',
  issues: ['Looks the same as a main thread', 'The parent’s name is only in the label above the first message and in the bar at the bottom'],
  options: [
    { key: 'A', name: 'The parent in the breadcrumb', from: 't3code breadcrumb, extended',
      desc: 'The header reads “agentZ / i wanna test subthreads, spawn some s… / Research: UI for child tasks ⌄”. The parent’s title is muted and shortened to fit; a click opens the parent, and its tooltip says “Open Parent”. A subthread of a subthread shows each thread above it. The rest of the header stays.',
      good: 'Says where you are and is the way back, in the place you look first.', cost: 'A long parent title leaves less room for the subthread’s own.',
      mock: () => subView({ header: breadcrumbHeader({ hover: true }), h: 300, overlay: tip('Open Parent', 'top:40px;left:120px') }) },
    { key: 'B', name: 'A back button at the start', from: 'new',
      desc: 'An arrow button sits first in the header, before “agentZ / Research: UI for child tasks ⌄”. A click opens the parent; its tooltip names it: “Back to ‘i wanna test subthreads…’”.',
      good: 'A familiar back arrow, in the corner where people look for one.', cost: 'Doesn’t say which thread is the parent until you hover.',
      mock: () => subView({ header: threadHeader({ lead: `<span class="ibtn hov">${ic('arrow-left', 'sm')}</span><span style="width:1px;height:16px;background:var(--b);margin:0 4px"></span>` }), h: 300, overlay: tip(`Back to “${PARENT_SHORT}”`, 'top:40px;left:8px') }) },
    { key: 'C', name: 'A subthread bar under the parent’s header', from: 'Zed subagent title bar',
      desc: 'The header shows the parent: “agentZ / i wanna test subthreads, spawn some sub threads… ⌄”. Under it, Zed’s subagent bar: an arrow into the subthread, its title, a green check once done, Stop while it runs, and a Minimize button (–) that opens the parent.',
      good: 'Reads as being inside the parent, as Zed shows it.', cost: 'Two bars along the top, and the header’s buttons belong to the parent.',
      mock: () => subView({ header: threadHeader({ title: titleMenu(PARENT) }), extra: zedBar(), h: 330 }) },
    { key: 'D', name: 'A “Subthread” badge after the title', from: 'new',
      desc: 'The header stays as it is, with a small “Subthread” badge after the title. A click on the badge opens the parent.',
      good: 'The smallest change.', cost: 'The badge doesn’t look like a way back.',
      mock: () => subView({ header: threadHeader({ title: `${titleMenu(SUB)}<span class="chip" style="height:20px;margin-left:4px">${ic('users', 'xs')}Subthread</span>` }), h: 300 }) },
    { key: 'E', name: 'As it is', from: 'today',
      desc: 'The same header as a main thread’s.',
      good: 'Nothing to change.', cost: 'You can’t tell you’re in a subthread from the header.',
      mock: () => subView({ h: 300 }) },
  ],
});

TOPICS.push({
  id: 'look', section: 'An open subthread', title: 'Anything else that sets it apart', size: 'wide', rec: 'A',
  now: 'Besides its header (the topic before), a subthread looks like a main thread: the same background and the same conversation. The mocks use header A and the bottom bar A from below.',
  options: [
    { key: 'A', name: 'Nothing else', from: 'Zed, t3code',
      desc: 'Only the header, the start of the conversation and the bottom bar change. Neither Zed nor t3code tints a subthread.',
      good: 'Every theme looks right.', cost: 'Below the header it reads like any thread.',
      mock: () => subView({ header: breadcrumbHeader(), top: '', bottom: t3Bar(), h: 360 }) },
    { key: 'B', name: 'A darker conversation', from: 'new',
      desc: 'The conversation sits on the editor color, a shade darker than a main thread’s panel color. Zed draws its subagent bar on the editor color too.',
      good: 'You can tell at a glance, even scrolled to the middle.', cost: 'In themes where the two colors match, nothing changes.',
      mock: () => subView({ header: breadcrumbHeader(), top: '', bottom: t3Bar(), convoBg: 'var(--ed)', h: 360 }) },
    { key: 'C', name: 'An accent line under the header', from: 'new',
      desc: 'A thin line in the theme’s accent color runs under the header, in place of its usual border.',
      good: 'Works in every theme.', cost: 'Accent lines elsewhere mean focus, not “subthread”.',
      mock: () => subView({ header: breadcrumbHeader({ style: 'border-bottom:2px solid var(--ac)' }), top: '', bottom: t3Bar(), h: 360 }) },
  ],
});

// t3code's TimelineSystemDivider at the top of a subagent's timeline: a rule, then a pill.
const t3Divider = `<div class="row g2" style="padding:4px 0 6px;font-size:11px;color:var(--mu)"><span class="grow" style="height:1px;background:var(--bv)"></span><span class="row g15" style="border:1px solid var(--b);border-radius:999px;padding:3px 10px;background:var(--ed);max-width:420px;min-width:0">${ic('users', 'xs')}<span class="b5" style="flex:none">Subthread of</span><span class="trunc" style="opacity:.75">· ${PARENT}</span></span><span class="grow" style="height:1px;background:var(--bv)"></span></div>`;
const taskCard = `<div style="border:1px solid var(--b);border-radius:8px;background:var(--ed);overflow:hidden"><div class="row g2" style="padding:6px 10px;background:#2c313a;border-bottom:1px solid var(--bv);font-size:12px">${ic('note', 'xs mu')}<span class="b5">Task</span><span class="mu trunc">from “${PARENT_SHORT}”</span><span class="grow"></span><span class="chip" style="height:18px">Research</span></div><div style="padding:8px 12px;line-height:21px">${TASK}</div></div>`;

TOPICS.push({
  id: 'start', section: 'An open subthread', title: 'Above the first message', size: 'wide', rec: 'A',
  now: 'The first message is the task the parent sent, in your own message’s bubble on the right. A small label above it says “✧ Sent by the agent in ‘i wanna test subthreads, spawn some sub threads…’”. Later messages from the parent get the same label. The mocks use header A.',
  nowImg: 'img/now-start.png',
  issues: ['The task looks like a message you wrote', 'The label is small, and nothing in it can be clicked'],
  options: [
    { key: 'A', name: 'A “Subthread of” divider at the top', from: 't3code',
      desc: 'Above the first message, a line runs across the conversation with a pill in its middle: “Subthread of · i wanna test subthreads…” (t3code says “Subagent of”). A click on the pill opens the parent. The first message loses its “Sent by” label; later ones from the parent keep theirs.',
      good: 'Marks the start of a subthread, and is one more way back.', cost: 'It scrolls away with the conversation.',
      mock: () => subView({ header: breadcrumbHeader(), top: t3Divider, bottom: t3Bar(), h: 360 }) },
    { key: 'B', name: 'The task as a card', from: 'new',
      desc: 'The first message is a full-width card instead of a bubble: “Task from ‘i wanna test subthreads…’” with the role (“Research”) in its header, and the task under it.',
      good: 'Clearly a task from another agent, not your message.', cost: 'A long task makes a tall card.',
      mock: () => subView({ header: breadcrumbHeader(), top: '', bubble: taskCard, bottom: t3Bar(), h: 360 }) },
    { key: 'C', name: 'As it is', from: 'today',
      desc: 'The “Sent by the agent in …” label over the bubble.',
      good: 'Nothing to change.', cost: 'Easy to miss.',
      mock: () => subView({ header: breadcrumbHeader(), bottom: t3Bar(), h: 360 }) },
  ],
});

// Where the composer would be -----------------------------------------------------------------
const bottomView = (bottom, { header = breadcrumbHeader(), extra = '', h = 250 } = {}) => frame(`<div class="col" style="height:100%;background:var(--panel)">${header}${extra}<div class="grow" style="min-height:0;overflow:hidden">${`<div class="col" style="padding:12px 20px 0;gap:8px">${workGroup}${answer}</div>`}</div>${activityBar([planRow])}${bottom}</div>`, { w: SW, h });
const pairB = (a, b) => `<div style="display:flex;flex-direction:column;gap:10px">${a}${b}</div>`;

TOPICS.push({
  id: 'bottom', section: 'Where the composer would be', title: 'In place of the composer', size: 'wide', rec: 'A',
  now: 'A subthread takes messages only from its parent, so a bar replaces the composer: the agent’s icon, “A subagent of ‘…’. It runs on its own; message its parent instead.”, Stop while it runs, and “↗ Open Parent” at the far right. The mocks use header A; option B fits header C.',
  nowImg: 'img/now-bottom.png',
  issues: ['The sentence is long and says “subagent”, while the rest of agentZ says “subthread”', 'Open Parent is far from the sentence, in the corner'],
  options: [
    { key: 'A', name: 'What runs, for how long, and Open Parent', from: 't3code ProviderSubagentBar',
      desc: 'The bar t3code puts there: the agent’s icon, the model and effort, “Working 1m 20s” or “Done in 2m 14s”, then “Runs on its own” in muted text, Stop while it runs, and an Open Parent button with an arrow pointing back.',
      good: 'Useful facts in place of a long sentence.', cost: 'The parent isn’t named here; the header names it.',
      mock: () => pairB(bottomView(t3Bar({ running: true })), bottomView(t3Bar())) },
    { key: 'B', name: 'Nothing', from: 'Zed',
      desc: 'No bar. The conversation and its Plan row reach the bottom. Stop and the way back are in the header (header C has both).',
      good: 'More room for the conversation.', cost: 'Needs a header with Stop and a way back.',
      mock: () => bottomView('', { header: threadHeader({ title: titleMenu(PARENT) }), extra: zedBar(true) }) },
    { key: 'C', name: 'As it is', from: 'today',
      desc: 'The sentence, Stop while it runs, and “↗ Open Parent”.',
      good: 'Nothing to change.', cost: 'A long sentence, and the way back sits in the corner.',
      mock: () => bottomView(subthreadBarToday) },
  ],
});

// The way back from the keyboard ----------------------------------------------------------------
const palette = (key) => frame(`<div style="padding:6px"><div class="field focus" style="margin-bottom:6px">parent</div><div class="menu" style="position:static;box-shadow:none;border:0;padding:0;min-width:0;background:none"><div class="it hl"><span class="grow">Open Parent Thread</span>${key ? `<span class="kb">${key}</span>` : ''}</div></div></div>`, { w: 420, h: 76, style: 'border:1px solid var(--b);border-radius:8px;background:var(--panel);box-shadow:0 12px 32px rgba(0,0,0,.45)' });
const headerWithTip = (key) => frame(`${breadcrumbHeader({ hover: true })}${tip(`Open Parent${key ? ` <span class="kbd">${key}</span>` : ''}`, 'top:40px;left:120px')}`, { w: 620, h: 76, style: 'background:var(--panel)' });
const keyView = (key, { tooltip = true } = {}) => `<div style="display:flex;flex-direction:column;gap:10px;align-items:center">${tooltip ? headerWithTip(key) : ''}${palette(key)}</div>`;

TOPICS.push({
  id: 'shortcut', section: 'The way back', title: 'A shortcut to the parent', size: 'medium', rec: 'A',
  now: 'No shortcut, and no command for it. Open Parent is only a button at the bottom right. In every option the command palette gets “Open Parent Thread”, and the way back you pick above shows the shortcut in its tooltip. The mocks show both.',
  options: [
    { key: 'A', name: 'Ctrl-minus', from: 'Zed',
      desc: 'Ctrl-− opens the parent, on macOS and Linux. Zed binds Ctrl-− in its agent thread to Go Back. It works anywhere in a subthread.',
      good: 'The key Zed users already know.', cost: 'Not a key most people guess.',
      mock: () => keyView('Ctrl-−') },
    { key: 'B', name: 'Cmd-Up (Alt-Up on Linux)', from: 'new',
      desc: 'Cmd-↑ on macOS and Alt-↑ on Linux open the parent, as they open the enclosing folder in Finder and in Files.',
      good: 'Reads as “up one level”.', cost: 'Cmd-↑ scrolls to the top in other apps.',
      mock: () => keyView('⌘↑') },
    { key: 'C', name: 'Escape', from: 'new',
      desc: 'With no menu open, Esc in a subthread opens the parent. A subthread has no composer, so nothing else takes Esc there.',
      good: 'The easiest key to find.', cost: 'An extra Esc after closing a menu leaves the subthread.',
      mock: () => keyView('Esc') },
    { key: 'D', name: 'No shortcut', from: 'new',
      desc: 'Only the command palette’s “Open Parent Thread”, with no key of its own.',
      good: 'No key to learn or to press by mistake.', cost: 'Slower from the keyboard.',
      mock: () => keyView('', { tooltip: false }) },
  ],
});
