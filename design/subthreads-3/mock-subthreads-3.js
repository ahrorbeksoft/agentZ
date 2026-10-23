// Subthreads, again: the demo world and the pieces this round's mocks share. Loaded after
// board/mock.js; names are prefixed with s3 so they don't clash with it.

ICONS['x-circle'] = '<circle cx="12" cy="12" r="10"/><path d="m15 9-6 6"/><path d="m9 9 6 6"/>';
ICONS['arrow-up-right'] = '<path d="M7 7h10v10"/><path d="M7 17 17 7"/>';
ICONS['arrow-up-left'] = '<path d="M7 17V7h10"/><path d="M17 17 7 7"/>';
ICONS.dash = '<path d="M5 12h14"/>';
ICONS.warning = '<path d="m21.73 18-8-14a2 2 0 0 0-3.48 0l-8 14A2 2 0 0 0 4 21h16a2 2 0 0 0 1.73-3"/><path d="M12 9v4"/><path d="M12 17h.01"/>';
ICONS.send = '<path d="m5 12 7-7 7 7"/><path d="M12 19V5"/>';
ICONS.forward = '<polyline points="15 10 20 15 15 20"/><path d="M4 4v7a4 4 0 0 0 4 4h12"/>';
ICONS.circle = '<circle cx="12" cy="12" r="9"/>';
ICONS.pause = '<rect x="14" y="4" width="4" height="16" rx="1"/><rect x="6" y="4" width="4" height="16" rx="1"/>';
GLYPHS.droid = '❋';

// The thread's tool rows draw in one dim gray (thread-rows round).
const S3_DIM = '#8d929c';
const S3_MONO = "'IBM Plex Mono','SF Mono',Menlo,monospace";
const s3Spin = (size = 12) => `<span class="spin" style="width:${size}px;height:${size}px"></span>`;
// Zed's GeneratingSpinner (dots) beside "Working for…", and SpinnerLabel's braille frame.
const s3Dots = () => '<span style="display:inline-flex;width:8px;justify-content:center;color:var(--mu);font-size:14px;line-height:1">⋮</span>';
const s3Braille = () => '<span style="display:inline-flex;width:8px;justify-content:center;color:var(--mu);font-size:12px;line-height:1">⠋</span>';
const s3Check = (cls = 'sm') => `<span style="display:inline-flex;color:var(--ok)">${ic('check', cls)}</span>`;
const s3Cross = (cls = 'sm') => `<span style="display:inline-flex;color:var(--del)">${ic('x', cls)}</span>`;
const s3Warn = (cls = 'sm') => `<span style="display:inline-flex;color:var(--warn)">${ic('warning', cls)}</span>`;
const s3Clock = (cls = 'sm') => `<span style="display:inline-flex;color:var(--ph)">${ic('clock', cls)}</span>`;
const s3Stat = ([files, add, del]) => `<span class="none" style="font-size:12px;color:var(--mu)">${files} ${files === 1 ? 'file' : 'files'}</span><span class="none" style="font-size:12px"><span class="okc">+${add}</span> <span class="delc">−${del}</span></span>`;
const s3Lines = ([, add, del]) => `<span class="none" style="font-size:11px"><span class="okc">+${add}</span> <span class="delc">−${del}</span></span>`;
const s3Stop = (color = 'var(--del)') => `<span class="ibtn sm" style="color:${color}">${ic('stop', 'xs')}</span>`;
const s3Open = (label = 'Open') => `<span class="none row" style="gap:3px;font-size:12px;color:var(--ac);padding:0 4px">${label}${ic('arrow-up-right', 'xs')}</span>`;
const s3Tip = (html, style) => `<div class="pop" style="${style};padding:5px 8px;font-size:12px;white-space:nowrap;display:flex;align-items:center;gap:8px">${html}</div>`;
const s3Az = (size = 14) => `<span class="mono" style="background:#4b3034;color:#e06c75;width:${size}px;height:${size}px;font-size:${size > 14 ? 8 : 7}px">AZ</span>`;
const s3Caption = (text) => `<div style="font:11px -apple-system,sans-serif;color:#e0b45c;margin:2px 0 6px;letter-spacing:.2px">${text}</div>`;

// The parent and its eight subthreads, newest first as ProjectStore::subthreads orders them.
const S3_PARENT = 'Implement the backlog in one go';
const S3_SUBS = [
  { id: 308, title: 'Worker: steer results into the running turn', glyph: 'claude', agent: 'Claude Agent', model: 'Opus 5.5', role: 'Worker', state: 'working', ran: '3m 40s', where: 'agentz/steer-results', files: [2, 60, 4], step: 'Editing crates/agentz_server/src/server/tasks.rs' },
  { id: 307, title: 'Researcher: how Droid reports background work', glyph: 'droid', agent: 'Factory Droid', model: 'Opus 5.5', role: 'Researcher', state: 'working', ran: '2m 05s', step: 'Reading crates/agent_thread/src/agent_thread.rs' },
  { id: 306, title: 'Reviewer: server side of delegated tasks', glyph: 'codex', agent: 'Codex', model: 'GPT-5.5', role: 'Reviewer', state: 'approval', ran: '1m 30s' },
  { id: 305, title: 'Worker: send only changed threads to the app', glyph: 'claude', agent: 'Claude Agent', model: 'Opus 5.5', role: 'Worker', state: 'done', ran: '6m 12s', ago: '4m', where: 'agentz/changed-threads', files: [4, 210, 88] },
  { id: 304, title: 'Researcher: account limits and their pace', glyph: 'droid', agent: 'Factory Droid', model: 'Opus 5.5', role: 'Researcher', state: 'done', ran: '3m 02s', ago: '9m' },
  { id: 303, title: 'Worker: roles in orchestrator_capabilities', glyph: 'codex', agent: 'Codex', model: 'GPT-5.5', role: 'Worker', state: 'done', ran: '7m 40s', ago: '12m', where: 'agentz/roles-capabilities', files: [3, 96, 12] },
  { id: 302, title: 'Reviewer: the roles’ account choice', glyph: 'claude', agent: 'Claude Agent', model: 'Opus 5.5', role: 'Reviewer', state: 'failed', ran: '1m 10s', ago: '15m' },
  { id: 301, title: 'Worker: raise delegate_tasks to 100', glyph: 'claude', agent: 'Claude Agent', model: 'Opus 5.5', role: 'Worker', state: 'done', ran: '2m 44s', ago: '18m', files: [2, 14, 3] },
];
const s3IsRunning = (s) => s.state === 'working' || s.state === 'approval';
const S3_RUNNING = S3_SUBS.filter(s3IsRunning);
const S3_ENDED = S3_SUBS.filter((s) => !s3IsRunning(s) && s.state !== 'queued');
const S3_ALL_DONE = S3_SUBS.map((s) => (s3IsRunning(s) ? { ...s, state: 'done', ago: '1m', ran: s.ran } : s));

// Topic 3 scales the same parent up to 100 tasks: 8 running (one waiting for approval),
// 30 ended (one failed) and 62 queued. Only the first few of each have names in the mocks.
const s3Sub = (title, glyph, state, extra = {}) => {
  const agents = { claude: ['Claude Agent', 'Opus 5.5'], codex: ['Codex', 'GPT-5.5'], droid: ['Factory Droid', 'Opus 5.5'] };
  return { title, glyph, agent: agents[glyph][0], model: agents[glyph][1], role: title.split(':')[0], state, ...extra };
};
const S3_MANY = {
  running: [
    S3_SUBS[2],
    S3_SUBS[0],
    S3_SUBS[1],
    s3Sub('Worker: fold queued subthreads in the card', 'claude', 'working', { ran: '1m 02s' }),
    s3Sub('Worker: the Linux tray icon', 'codex', 'working', { ran: '58s', where: 'agentz/linux-tray' }),
    s3Sub('Researcher: how t3code pages long lists', 'droid', 'working', { ran: '41s' }),
    s3Sub('Worker: the context ring in the composer', 'claude', 'working', { ran: '30s' }),
    s3Sub('Reviewer: queued tasks across a restart', 'codex', 'working', { ran: '12s' }),
  ],
  ended: [
    S3_SUBS[3], S3_SUBS[4], S3_SUBS[5], S3_SUBS[6], S3_SUBS[7],
    s3Sub('Worker: Subthreads at once in Settings', 'claude', 'done', { ran: '4m 18s', ago: '21m', files: [3, 74, 9] }),
  ],
  queued: [
    s3Sub('Worker: show shells beside Working', 'claude', 'queued'),
    s3Sub('Worker: the subthread title bar', 'codex', 'queued'),
    s3Sub('Researcher: Zed’s terminal cards', 'droid', 'queued'),
    s3Sub('Reviewer: the sidebar’s completed fold', 'claude', 'queued'),
    s3Sub('Worker: archive cleanup for subthreads', 'claude', 'queued'),
    s3Sub('Worker: images in the composer', 'codex', 'queued'),
  ],
  counts: { total: 100, running: 8, approval: 1, ended: 30, failed: 1, queued: 62 },
};

// The Agents view's sidebar (sidebar.rs render_thread_card): t3code's thread cards.
const s3Status = (state, time) => {
  if (state === 'working') return `<span class="row g1 none" style="color:var(--ac);font-size:12px;font-weight:500">${s3Spin(11)}Working</span>`;
  if (state === 'approval') return `<span class="pill pending">${dot('pending')}Pending Approval</span>`;
  if (state === 'waiting') return '<span class="none" style="font-size:12px;font-weight:500;color:var(--mu)">Waiting</span>';
  if (state === 'done') return `<span class="pill done">${dot('done')}Completed</span>`;
  return `<span class="sm mu none">${time || ''}</span>`;
};
// The count on a parent's card, which folds its subthreads' rows.
const s3Toggle = (open, count, running) => `<span class="row none" style="gap:2px;font-size:11px;color:${running ? 'var(--ac)' : 'var(--faint)'}">${ic(open ? 'chev-down' : 'chev-right', 'xs')}${ic('users', 'xs')}${count}</span>`;
function s3Card({ title, state, time = '1h', glyphKind = 'claude', people = '', active = false, machine = 'laptop', end = '' } = {}) {
  return `<div style="margin:2px 4px;padding:8px 10px;border-radius:6px;position:relative;${active ? 'background:var(--sel)' : ''}">
    <div class="row g15" style="height:20px">${s3Az()}<span class="sm mu grow trunc">agentZ</span>${end || s3Status(state, time)}</div>
    <div class="trunc b5" style="margin-top:4px">${title}</div>
    <div class="row g15 xs faint" style="margin-top:2px;min-width:0"><span class="grow trunc">main</span>${people}<span style="opacity:.6;display:inline-flex;color:var(--mu)">${ic(machine, 'sm')}</span>${glyph(glyphKind, 'sm')}</div>
  </div>`;
}
const s3Parent = (opts = {}) => s3Card({ title: S3_PARENT, state: 'working', active: true, people: s3Toggle(true, 8, true), ...opts });
const S3_OTHERS = [
  { title: 'Record Bugs and Features in Backlog', time: '1h', glyphKind: 'claude' },
  { title: 'Update Composer and Mac App UI', time: '3h', glyphKind: 'droid' },
  { title: 'Recent Zed changes impact', time: '5h', glyphKind: 'codex', machine: 'server' },
];
const s3Others = () => S3_OTHERS.map((t) => s3Card(t)).join('');

// A subthread's row under its parent (render_subthread_row), with herdr's tree lines.
const s3Tree = (last) => `<span style="position:absolute;left:18px;top:0;${last ? 'height:15px' : 'bottom:0'};border-left:1px solid var(--b)"></span><span style="position:absolute;left:18px;top:15px;width:8px;border-top:1px solid var(--b)"></span>`;
const s3RowState = (s) => (s.state === 'queued' ? '<span class="sm mu none">Queued</span>' : s3IsRunning(s) ? s3Status(s.state) : `<span class="sm mu none">${s.ago || ''}</span>`);
function s3SubRow(s, last, { lines = 2, on = false, hover = false } = {}) {
  const second = lines === 2 ? `<div class="row g1 xs faint" style="padding-left:24px;min-width:0;margin-top:1px"><span class="trunc grow">${s.agent} · ${s.model}</span>${s.where ? `<span class="row g1 none">${ic('worktree', 'xs')}${s.where.replace(/^agentz\//, '')}</span>` : ''}${s.files ? s3Lines(s.files) : ''}</div>` : '';
  return `<div style="position:relative;padding-left:24px">${s3Tree(last)}<div style="margin:0 4px 0 0;padding:4px 8px;border-radius:6px;${on ? 'background:var(--sel)' : hover ? 'background:var(--hov)' : ''}">
    <div class="row" style="gap:8px;height:22px"><span style="display:inline-flex;opacity:.6">${glyph(s.glyph, 'sm')}</span><span class="grow trunc ${on ? '' : 'mu'}" style="font-size:13px">${s.title}</span>${s3RowState(s)}</div>${second}</div></div>`;
}
const s3SubRows = (subs, opts = {}) => subs.map((s, i) => s3SubRow(s, i === subs.length - 1 && !opts.more, opts)).join('');
// A row in the tree that stands for several subthreads (folded ones, or Show more).
const s3TreeRow = (html, { last = true, hover = false, end = '' } = {}) => `<div style="position:relative;padding-left:24px">${s3Tree(last)}<div class="row" style="gap:6px;height:28px;margin:0 4px 0 0;padding:0 8px;border-radius:6px;font-size:12px;color:var(--mu);${hover ? 'background:var(--hov)' : ''}">${html}<span class="grow"></span>${end}</div></div>`;
const s3Sidebar = (list, { h = 600, w = 290 } = {}) => frame(sidebar({ list }), { w, h });

// The thread: its header (render_toolbar), the conversation, the activity bar and composer.
const s3HeaderRight = () => `<span class="row g15" style="flex:none"><span class="btn sm" style="gap:4px;height:22px">Open ${ic('chev-down', 'xs')}</span><span class="ibtn" style="font-size:15px">±</span>${ibtn('panel-bottom')}${ibtn('more')}</span>`;
const s3Header = (middle = `<span class="row g1" style="padding:2px 4px;min-width:0;font-size:13px"><span class="trunc">${S3_PARENT}</span>${ic('chev-down', 'xs mu')}</span>`) => `<div class="row g1" style="height:36px;flex:none;padding:0 8px;border-bottom:1px solid var(--b);background:var(--panel)"><span class="row g15" style="padding:2px 4px;flex:none">${s3Az()}<span class="sm mu">agentZ</span></span><span class="sm mu" style="flex:none">/</span><div class="row grow" style="min-width:0">${middle}</div>${s3HeaderRight()}</div>`;
const s3Para = (html, style = '') => `<div style="line-height:22px;color:var(--t);padding:4px 0;${style}">${html}</div>`;
const s3Bubble = (html) => `<div style="display:flex;justify-content:flex-end;padding:4px 0 10px"><div style="max-width:78%;background:#3a3f4b;border-radius:10px;padding:8px 12px;line-height:21px">${html}</div></div>`;
const s3Row = (icon, html, { trailing = '', iconHtml } = {}) => `<div class="row" style="min-height:24px;gap:6px;padding:0 2px;border-radius:5px;color:${S3_DIM};font-size:13px"><span style="width:24px;display:inline-flex;justify-content:center;color:${S3_DIM}">${iconHtml || ic(icon, 'sm')}</span><span class="row grow" style="gap:4px;min-width:0">${html}</span>${trailing}</div>`;
const s3Fold = (text, trailing = '') => s3Row('chev-right', `<span class="trunc">${text}</span>`, { trailing });
const s3Code = (text) => `<span class="trunc" style="font:12px ${S3_MONO}">${text}</span>`;

// "Started 8 subthreads · 3 running" and a row each (render_delegation_batch).
const s3Mark = (s) => (s.state === 'queued' ? s3Clock() : s.state === 'approval' ? s3Warn() : s.state === 'working' ? s3Braille() : s.state === 'failed' ? s3Cross() : s3Check());
const s3BatchRow = (s) => `<div class="row" style="height:30px;padding:0 4px 0 12px;gap:6px;border-top:1px solid var(--b)"><span style="width:16px;display:inline-flex;justify-content:center">${s3Mark(s)}</span><span style="display:inline-flex;opacity:.7">${glyph(s.glyph, 'sm')}</span><span class="trunc" style="font-size:13px;flex:0 1 auto">${s.title}</span><span class="sm mu none">· ${s.model}</span>${s.where ? `<span class="row g1 xs none" style="color:var(--ph)">${ic('worktree', 'xs')}${s.where}</span>` : ''}<span class="grow"></span>${s.files ? s3Stat(s.files) : ''}${s.state === 'queued' ? '<span class="sm mu none">Queued</span>' : s3IsRunning(s) ? s3Stop() : `<span class="xs mu none">${s.ran}</span>`}${s3Open()}</div>`;
const s3BatchHead = (title, extra) => `<div class="row" style="height:32px;padding:0 8px;gap:6px;background:#2c313a"><span style="font-size:13px">${title}</span>${extra}<span class="grow"></span></div>`;
const s3BatchCard = (inner) => `<div style="border:1px solid var(--b);border-radius:6px;overflow:hidden;margin:3px 0;background:var(--ed)">${inner}</div>`;
const s3Batch = (subs = S3_SUBS) => s3BatchCard(s3BatchHead(`Started ${subs.length} subthreads`, subs.some(s3IsRunning) ? `<span class="sm mu">· ${subs.filter(s3IsRunning).length} running</span>` : '') + subs.map(s3BatchRow).join(''));
// "5 subthreads finished · 1 failed" (render_task_results_row).
const s3Results = (count, failed, open = false) => s3Row('check', `<span style="color:${S3_DIM}">${count} subthreads finished</span>${failed ? `<span class="delc">· ${failed} failed</span>` : ''}`, { iconHtml: s3Check(), trailing: ic(open ? 'chev-up' : 'chev-down', 'xs mu') });
const s3ResultRow = (s) => s3Row('check', `<span class="none">${s.state === 'failed' ? 'Subthread failed:' : 'Subthread finished:'}</span><span class="trunc" style="color:var(--t)">${s.title}</span>`, { iconHtml: s.state === 'failed' ? s3Cross() : s3Check(), trailing: `${s.files ? `<span class="row g1 none">${s3Stat(s.files)}</span>` : ''}<span class="xs none" style="color:${S3_DIM};padding:0 4px">${s.ran}</span>${s3Open()}${ic('chev-down', 'xs mu')}` });

// The rows that end a conversation: render_generating and render_waiting_for_subthreads.
const s3Working = (time = '4m 12s', extra = '') => `<div class="row g2" style="padding:8px 2px;font-size:12px;color:var(--mu)">${s3Dots()}<span class="none">Working for ${time}</span>${extra}</div>`;
const s3Waiting = (count = 3) => `<div class="row g2" style="padding:8px 2px;font-size:12px;color:var(--mu)">${s3Braille()}<span>Waiting for ${count} subthreads</span><span style="color:var(--ph)">· the agent continues when they end</span></div>`;

// The activity bar: a tab on the composer card's top edge (render_activity_bar).
const s3Activity = (sections) => `<div style="padding:0 12px;flex:none"><div style="background:#31353e;border:1px solid var(--b);border-bottom:0;border-radius:6px 6px 0 0;box-shadow:1px -1px 2px rgba(0,0,0,.12)">${sections.join('<div style="height:1px;background:var(--b)"></div>')}</div></div>`;
const s3SectionHead = (label, open = true) => `<div class="row g1" style="padding:4px;${open ? 'border-bottom:1px solid var(--b)' : ''}"><span class="ibtn sm">${ic(open ? 'chev-down' : 'chev-right', 'xs')}</span><span class="sm mu">${label}</span></div>`;
// A background task's row (render_background_tasks_section): spinner, name, "Shell · 13s", Stop.
const S3_SHELLS = [
  { name: '(cargo test -p agent_thread -p agentz_server -p agentz_client -p agentz_protocol > /tmp/tests2.log 2>&1; echo done >> /tmp/tests2.log)', short: 'cargo test -p agent_thread -p agentz_server …', kind: 'Shell', time: '13s' },
  { name: 'npm run dev --prefix site', short: 'npm run dev --prefix site', kind: 'Shell', time: '9m 02s' },
];
const s3BgRow = (task, last = true) => `<div class="row" style="padding:6px;gap:6px;background:var(--ed);${last ? '' : 'border-bottom:1px solid var(--bv)'}">${s3Spin(12)}<span class="grow trunc" style="font-size:12px">${task.name}</span><span class="sm mu none">${task.kind} · ${task.time}</span>${s3Stop('var(--mu)')}</div>`;
const s3BgSection = (tasks = S3_SHELLS.slice(0, 1), open = true) => s3SectionHead(tasks.length === 1 ? '1 Background Task' : `${tasks.length} Background Tasks`, open) + (open ? tasks.map((t, i) => s3BgRow(t, i === tasks.length - 1)).join('') : '');
const s3QueueSection = (messages) => s3SectionHead(messages.length === 1 ? '1 Queued Message' : `${messages.length} Queued Messages`) + messages.map((m) => `<div class="row" style="padding:6px 8px;gap:8px;background:var(--ed)"><span class="grow trunc" style="font-size:12px">${m}</span><span class="btn ghost sm">Edit</span><span class="btn ghost sm">Steer</span><span class="btn ghost sm" style="color:var(--t)">Send Now</span></div>`).join('');

// The composer card (Zed's message editor in t3code's layout), with Stop while a turn runs.
const s3Composer = ({ stop = false } = {}) => `<div style="padding:0 0 8px;flex:none"><div style="border:1px solid var(--b);border-radius:8px;background:var(--ed);padding:10px 12px 8px">
  <div class="ph" style="padding:0 2px 18px;font-size:13px">Message the agent…</div>
  <div class="row" style="gap:10px;font-size:12px;color:var(--mu)">${ic('plus', 'sm')}<span class="row g1">${glyph('claude', 'sm')}Claude Agent</span><span class="row g1">${ic('circle', 'xs')}70%</span><span class="grow"></span><span class="row g1">Bypass permissions ${ic('chev-down', 'xs')}</span><span class="row g1">Opus 5.5 · Xhigh ${ic('chev-down', 'xs')}</span>${stop ? `<span style="width:22px;height:22px;border-radius:50%;background:#c4686d;display:inline-grid;place-items:center;color:#fff">${ic('stop', 'xs')}</span>` : `<span style="width:22px;height:22px;border-radius:50%;background:var(--sel);display:inline-grid;place-items:center;color:var(--t)">${ic('send', 'xs')}</span>`}</div></div>
  <div class="row xs mu" style="padding:6px 4px 0;gap:4px">${ic('laptop', 'xs')}This Mac<span class="grow"></span>${ic('branch', 'xs')}main</div></div>`;

// A thread at the end of its conversation: the top scrolls away, the bottom shows.
const s3Thread = (html, { h = 520, w = 820, header = s3Header(), activity = '', composer = s3Composer(), overlay = '' } = {}) => frame(`<div class="col" style="height:100%;background:var(--panel)">${header}<div class="grow" style="min-height:0;overflow:hidden;display:flex;flex-direction:column;justify-content:flex-end;padding:0 34px"><div style="display:flex;flex-direction:column;gap:2px">${html}</div></div><div style="padding:0 22px;flex:none">${activity}${composer}</div>${overlay}</div>`, { w, h });

// The parent's conversation in the demo: the ask, the batch, the first results.
const S3_ASK = s3Bubble('Implement the backlog in one go. Split it up by role: workers for the code, researchers for what other apps do, reviewers for the server side.');
const S3_SAID = s3Para('I started eight subthreads, one per backlog item. I’ll put their work together as they finish.');
const S3_AFTER = s3Para('Five are back. The changed-threads and roles work is in its worktrees; the account-choice review failed on a test setup, so I’ll redo it once the reviewer of the server side reports. Three are still running.');
