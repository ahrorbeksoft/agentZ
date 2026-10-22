// Subthreads: how they show in the sidebar and in their parent, the task and the results, and
// what agents can do with them. sidebar.rs render_thread_card, agent_view.rs
// render_agents_section, render_subthread_card, render_task_card; server/tools.rs.

const SB_H = 560;
const parentCard = (opts = {}) => rCard({ title: R_PARENT, state: 'working', glyphKind: 'claude', active: true, people: rPeople(4, true), ...opts });
const shortBranch = (where) => where.replace(/^agentz\//, '');

// A subthread nested under its parent as a worktree under its workspace (spaces_view.rs
// render_space_row): indented 20px, with herdr's tree lines in the border color.
function subRow(s, last, { lines = 2, hover = false, on = false, tree: withTree = true } = {}) {
  const status = s.state === 'done' ? `<span class="sm mu none">${s.time}</span>` : rStatus(s.state);
  const lineTop = 4 + 11;
  const tree = `<span style="position:absolute;left:18px;top:0;${last ? `height:${lineTop}px` : 'bottom:0'};border-left:1px solid var(--b)"></span><span style="position:absolute;left:18px;top:${lineTop}px;width:8px;border-top:1px solid var(--b)"></span>`;
  const second = lines === 2 ? `<div class="row g1 xs faint" style="padding-left:24px;min-width:0;margin-top:1px"><span class="trunc grow">${s.agent} · ${s.model}</span>${s.where ? `<span class="row g1 none">${rWorktree()}${shortBranch(s.where)}</span>` : ''}${s.files ? `<span class="none"><span class="okc">+${s.files[1]}</span> <span class="delc">−${s.files[2]}</span></span>` : ''}</div>` : '';
  return `<div style="position:relative;padding-left:${withTree ? 24 : 6}px">${withTree ? tree : ''}<div style="margin:0 4px 0 0;padding:4px 8px;border-radius:6px;${on ? 'background:var(--sel)' : hover ? 'background:var(--hov)' : ''}">
    <div class="row" style="gap:8px;height:22px">${glyph(s.glyph, 'sm')}<span class="grow trunc ${on ? '' : 'mu'}" style="font-size:13px">${s.title}</span>${status}</div>${second}</div></div>`;
}
const toggle = (open, count, running) => `<span class="row none" style="gap:2px;font-size:11px;color:${running ? 'var(--ac)' : 'var(--faint)'}">${ic(open ? 'chev-down' : 'chev-right', 'xs')}${ic('users', 'xs')}${count}</span>`;
const nested = (subs, opts) => subs.map((s, i) => subRow(s, i === subs.length - 1, opts)).join('');

TOPICS.push({
  id: 'sidebar', section: 'Subthreads in the sidebar', title: 'Subthreads in the sidebar', size: 'narrow', rec: 'A',
  now: 'Subthreads have no rows of their own. The parent’s card counts them with a people icon (“👥 4”, blue while one runs), and its details popover says “4 subthreads, 2 running”. A subthread opens from its parent’s Agents list or its card in the conversation, and the sidebar then highlights the parent.',
  nowImg: 'img/now-sidebar.png',
  issues: ['You can’t see or open a subthread from the sidebar.', 'The count doesn’t say which ones are running or waiting for you.'],
  options: [
    { key: 'A', name: 'Nested under the parent, like worktrees', from: 'Workspaces worktree rows (herdr)',
      desc: 'Each subthread is a row under its parent’s card, indented with tree lines, as a worktree sits under its workspace in the Workspaces view. The first line has the agent’s icon, the title and its status (Working, Needs Approval, or how long ago it ended); the second line has the agent, the model and, when it works in one, its worktree, with the lines it changed. The count on the parent’s card becomes a fold toggle (“Hide Subthreads”). A click opens the subthread.',
      good: 'Every subthread is one click away, and you see which needs you.', cost: 'A thread with many subthreads makes the list long until folded.',
      mock: () => rSidebar(parentCard({ people: toggle(true, 4, true) }) + nested(R_SUBS) + rOthers() + `<div class="grow"></div>` + rShells(), { h: SB_H + 80 }) },
    { key: 'B', name: 'Nested, one line each', from: 'Workspaces worktree rows, folded density',
      desc: 'The same nesting and tree lines, but one line per subthread: the agent’s icon, the title and its status. The agent, model and worktree are in the row’s details popover.',
      good: 'Twice as many fit, and they stay quiet under the parent.', cost: 'You don’t see which agent or worktree each one uses without hovering.',
      mock: () => rSidebar(parentCard({ people: toggle(true, 4, true) }) + nested(R_SUBS, { lines: 1 }) + rOthers() + `<div class="grow"></div>` + rShells(), { h: SB_H }) },
    { key: 'C', name: 'Folded behind the count', from: 'new',
      desc: 'The rows of A, but folded until you open them: the count on the parent’s card has a chevron, and a click on it shows the subthreads under the card. It stays open or folded as you leave it.',
      good: 'The sidebar looks as it does now until you want them.', cost: 'Which subthread needs you is still hidden behind a click.',
      mock: () => rSidebar(parentCard({ people: toggle(false, 4, true) }) + rOthers() + `<div class="grow"></div>` + rShells() + rTip('Show 4 Subthreads', 'top:96px;left:150px'), { h: SB_H - 80 }) },
    { key: 'D', name: 'As it is', from: 'today',
      desc: 'Only the count on the parent’s card.',
      good: 'Nothing to change.', cost: 'Subthreads stay out of reach from the sidebar.',
      mock: () => rSidebar(parentCard() + rOthers() + `<div class="grow"></div>` + rShells(), { h: SB_H - 80 }) },
  ],
});

TOPICS.push({
  id: 'sidebar-finished', section: 'Subthreads in the sidebar', title: 'Finished subthreads in the sidebar', size: 'narrow', rec: 'A',
  now: 'Nothing is in the sidebar today. In the parent’s Agents list, every subthread stays listed, and the list folds to “4 Agents · all done” once the last one ends (the Subthreads round’s pick). The mocks use rows A from the topic before.',
  nowImg: 'img/now-sidebar.png',
  options: [
    { key: 'A', name: 'Fold once all are done', from: 'the Agents list (Subthreads round)',
      desc: 'While any subthread runs, they’re all shown under the parent. Once the last one ends, they fold away into the count on the parent’s card, as the Agents list folds, and show again when a new one starts. A click on the count opens them.',
      good: 'The sidebar shows subthreads only while they matter, the same way the Agents list does.', cost: 'Rows disappear on their own once the work is done.',
      mock: () => `<div style="display:flex;gap:10px">${rSidebar(parentCard({ people: toggle(true, 4, true) }) + nested(R_SUBS) + rOthers(), { h: 520 })}${rSidebar(parentCard({ state: 'done', people: toggle(false, 4, false) }) + rOthers(), { h: 520 })}</div>` },
    { key: 'B', name: 'All stay until you fold them', from: 'Workspaces worktree groups',
      desc: 'Every subthread stays under its parent, running or ended, until you fold them with the count, as a workspace’s worktrees stay until you hide them. They leave with the parent when it’s archived.',
      good: 'Nothing moves on its own.', cost: 'Ended subthreads take room under every parent that used them.',
      mock: () => rSidebar(parentCard({ state: 'done', people: toggle(true, 4, false) }) + nested(R_DONE) + rOthers(), { h: 560 }) },
    { key: 'C', name: 'Only the ones still running', from: 'new',
      desc: 'Only running subthreads and those waiting for you are under the parent. One that ends leaves the sidebar at once; it stays in the parent’s Agents list.',
      good: 'The shortest list: just what’s still going.', cost: 'A finished one can only be reached from its parent.',
      mock: () => rSidebar(parentCard({ people: toggle(true, 4, true) }) + nested(R_SUBS.filter((s) => s.state !== 'done')) + rOthers(), { h: 520 }) },
  ],
});

// In the parent ----------------------------------------------------------------------------
const parentWith = (list, { h = 460, html = rAsk + rSaid } = {}) => rConversation(html, { h, bottom: rActivity([list]) + rComposer() });
const subRowInList = (s, last) => `<div style="position:relative;background:var(--ed);${last ? '' : 'border-bottom:1px solid var(--bv)'}">${subRow(s, true, { tree: false })}</div>`;

TOPICS.push({
  id: 'agents-list', section: 'In the parent thread', title: 'The Agents list above the composer', size: 'wide', rec: 'A',
  now: 'A parent lists its subthreads above its composer, under “4 Agents · 2 running”: Zed’s subagent headers as rows, with a spinner, check or cross, the title, “· model”, the files it changed and Stop while it runs. It folds to “· all done” once the last one ends. If you pick sidebar rows, the same subthreads show there too.',
  nowImg: 'img/now-parent.png',
  options: [
    { key: 'A', name: 'Keep it as it is', from: 'Zed subagent cards (built)',
      desc: 'The list stays as built. With sidebar rows, the sidebar is how you find them from anywhere, and this list is how you see them while in the parent.',
      good: 'Nothing to change, and Stop is right there.', cost: 'The same subthreads show in two places.',
      mock: () => parentWith(rAgentsList()) },
    { key: 'B', name: 'Rows like the sidebar’s', from: 'Workspaces worktree rows',
      desc: 'The list takes the sidebar rows’ look: the agent’s icon, the title and its status, then the agent, model, worktree and lines changed. Stop shows on a running row when you hover it.',
      good: 'A subthread looks the same wherever it shows.', cost: 'Two lines each, so fewer fit before the list scrolls.',
      mock: () => parentWith(rAgentsList(R_SUBS, subRowInList), { h: 520 }) },
    { key: 'C', name: 'No list', from: 'new',
      desc: 'The list goes. Subthreads show in the sidebar under the parent and as their cards in the conversation; the bar above the composer keeps the plan and background tasks.',
      good: 'One place for subthreads, and more room for the conversation.', cost: 'Needs sidebar rows; the cards scroll away with the conversation.',
      mock: () => rConversation(rAsk + R_SUBS.slice().reverse().map((s) => rZedCard(s)).join('') + rSaid, { h: 460, bottom: rComposer() }) },
  ],
});

// The start cards and the results.
const startCards = (subs = R_SUBS) => subs.slice().reverse().map((s) => rZedCard(s, { body: s.state === 'working' ? rZedStep(s.id === 282 ? 'Editing crates/app/src/agent_view.rs' : 'Reading crates/agentz_server/src/accounts/usage.rs') + rZedStrip() : '' })).join('');
const groupCard = (subs = R_SUBS) => `<div style="border:1px solid var(--b);border-radius:6px;overflow:hidden;margin:3px 0;background:var(--ed)"><div class="row" style="height:32px;padding:0 8px;gap:6px;background:#2c313a">${ic('chev-down', 'xs mu')}<span style="font-size:13px">Started ${subs.length} subthreads</span><span class="sm mu">· ${rRunning(subs)} running</span><span class="grow"></span></div>${subs.slice().reverse().map((s, i) => `<div class="row" style="height:30px;padding:0 8px 0 12px;gap:6px;border-top:1px solid var(--bv)"><span style="width:16px;display:inline-flex;justify-content:center">${rStatusIcon(s)}</span>${glyph(s.glyph, 'sm')}<span class="trunc" style="font-size:13px;flex:0 1 auto">${s.title}</span><span class="sm mu none">· ${s.model}</span>${s.where ? `<span class="row g1 xs faint none">${rWorktree()}${shortBranch(s.where)}</span>` : ''}<span class="grow"></span>${s.files ? rStat(s.files) : ''}${s.state === 'working' || s.state === 'approval' ? rStop() : `<span class="xs mu none">${s.time}</span>`}${rOpen()}</div>`).join('')}</div>`;
const startRows = (subs = R_SUBS) => rFold(`Started ${subs.length} subthreads`, true) + subs.slice().reverse().map((s) => rRow('users', `${rVerb('Started a subthread:')}${rTitle(s.title)}`, { indent: 0, trailing: `${s.where ? `<span class="row g1 xs none" style="color:${R_DIM}">${rWorktree()}${shortBranch(s.where)}</span>` : ''}<span style="display:inline-flex;margin:0 4px">${rStatusIcon(s, 'xs')}</span>${rOpen()}` })).join('');

TOPICS.push({
  id: 'start', section: 'In the parent thread', title: 'Where the parent started them', size: 'wide', rec: 'B',
  now: 'Each <code>delegate_task</code> call in the parent’s conversation is Zed’s subagent card: a spinner, check or cross, the title, “· model”, the files it changed, and Stop while it runs. A running card shows the step it’s on and a strip that opens it full screen; opened, it shows its last steps and, once it ended, its summary. Calls made together fold into “Started 4 subthreads”, which opens to a card each (the picture shows it opened, above the Agents list).',
  nowImg: 'img/now-parent.png',
  options: [
    { key: 'A', name: 'One card per subthread', from: 'Zed subagent cards (built)',
      desc: 'As it is: a card per call inside the “Started 4 subthreads” fold, each with its live step while it runs.',
      good: 'Shows what each one is doing right now.', cost: 'Four subthreads stack four cards, each with its strip.',
      mock: () => rConversation(rAsk + startCards() + rSaid, { h: 470 }) },
    { key: 'B', name: 'One card for the batch', from: 'Zed subagent card, grouped',
      desc: 'Subthreads started together share one card: “Started 4 subthreads · 2 running”, then a row each with its status, the agent’s icon, title, model, worktree, the lines it changed, and Stop or how long it ran, and Open. A click on a row opens its preview (its last steps and summary) under it.',
      good: 'One block per split of the work, which reads as one decision.', cost: 'Doesn’t show each one’s current step until you open its row.',
      mock: () => rConversation(rAsk + groupCard() + rSaid, { h: 400 }) },
    { key: 'C', name: 'Tool-call rows', from: 'agentZ tool rows',
      desc: 'Each call is a row like other tool calls: “Started a subthread: Build the indicator in the picker”, its worktree, its status and Open. Calls made together keep today’s “Started 4 subthreads” fold.',
      good: 'Quiet, and reads like the rest of the agent’s work.', cost: 'No live step or Stop in the conversation; those are in the Agents list.',
      mock: () => rConversation(rAsk + startRows() + rSaid, { h: 360 }) },
  ],
});

// The results, where the parent hears of them.
const resultRow = (s, { open = false } = {}) => rRow(s.state === 'done' ? 'check-circle' : 'alert', `${rVerb(s.state === 'done' ? 'Subthread finished:' : 'Subthread failed:')}${rTitle(s.title)}`, {
  iconHtml: s.state === 'done' ? `<span style="display:inline-flex;color:var(--ok)">${ic('check-circle', 'sm')}</span>` : undefined,
  trailing: `${s.files ? `<span class="row g1 none" style="font-size:12px">${rStat(s.files)}</span>` : ''}<span class="xs none" style="color:${R_DIM};padding:0 4px">${s.time}</span>${rOpen()}${ic(open ? 'chev-up' : 'chev-down', 'xs')}`,
}) + (open ? `<div style="margin-left:30px;padding:4px 0 8px;display:flex;flex-direction:column;gap:6px"><div style="border-left:2px solid var(--bv);padding:2px 0 2px 10px;line-height:21px;font-size:13px;color:var(--t)">${R_SUMMARY}</div></div>` : '');
const doneSubs = R_DONE.slice().reverse();
const afterAll = rPara('All four are done. The indicator is in the picker, its tests pass, and the review found nothing to change on the server side. Both worktrees are ready to merge.');
const taskStatusRows = doneSubs.map((s) => rRow('terminal', `${rVerb('Called')}<span class="trunc" style="font:12px ${R_MONO}">agentz · task_status {"taskId": ${s.id}}</span>`)).join('');

TOPICS.push({
  id: 'result', section: 'In the parent thread', title: 'The results', size: 'wide', rec: 'A',
  now: 'When subthreads end, the parent gets a message it doesn’t show: “Delegated tasks 281, 282 reached terminal states. Use task_status with each taskId to read the results.” Its agent then calls <code>task_status</code> for each, shown as ordinary tool calls, and only then answers. Each result also shows in its start card once you open it. The mocks use the batch card B from the topic before.',
  issues: ['Nothing marks the moment a result came back.', 'The agent needs a task_status call per subthread to read what the message could have said.'],
  options: [
    { key: 'A', name: 'A tool-call row per result', from: 'agentZ tool rows',
      desc: 'Where the parent hears of a result, a row like a tool call: a green check, “Subthread finished: Research how Zed shows usage”, the lines it changed, how long it ran, and Open. It opens to the subthread’s summary. The hidden message and the task_status calls go: the result comes in the message itself (see “What agents can do”).',
      good: 'Each result shows where it came back, in the agent’s own flow.', cost: 'Four results make four rows.',
      mock: () => rConversation(groupCard(R_DONE) + rSaid + doneSubs.map((s, i) => resultRow(s, { open: i === 0 })).join('') + afterAll, { h: 480 }) },
    { key: 'B', name: 'One row for results that came together', from: 'agentZ tool rows, folded',
      desc: 'Results that reach the parent in one message share a row, “4 subthreads finished”, which opens to the rows of A.',
      good: 'One line per batch.', cost: 'A failed one hides in the fold until you open it (the row says “1 failed” in red).',
      mock: () => rConversation(groupCard(R_DONE) + rSaid + rRow('check-circle', `${rVerb('4 subthreads finished')}`, { iconHtml: `<span style="display:inline-flex;color:var(--ok)">${ic('check-circle', 'sm')}</span>`, trailing: ic('chev-down', 'xs') }) + afterAll, { h: 380 }) },
    { key: 'C', name: 'Only in the start card', from: 'today, tidied',
      desc: 'As it is, but the task_status calls fold into the agent’s work like any other calls, and the start card’s row shows the summary when opened.',
      good: 'The least to change.', cost: 'The moment a result came back still isn’t marked.',
      mock: () => rConversation(groupCard(R_DONE) + rSaid + rFold('Called task_status 4 times') + afterAll, { h: 380 }) },
    { key: 'D', name: 'As it is', from: 'today',
      desc: 'The hidden message, then the agent’s task_status calls as ordinary tool calls.',
      good: 'Nothing to change.', cost: 'Four raw calls, and the results only in the start cards.',
      mock: () => rConversation(groupCard(R_DONE) + rSaid + taskStatusRows + afterAll, { h: 420 }) },
  ],
});

// An open subthread's task -------------------------------------------------------------------
const TASK_LINES = [
  'Build a usage indicator for the agent picker in <code>crates/app/src/agent_view.rs</code>: under each agent with an account, a thin bar with the share of its 5-hour window used.',
  '',
  '<b>Context:</b> you are a subagent started by another agent. You don’t have its conversation. Work only in your worktree; don’t touch main. Don’t push. Don’t ask the user questions unless you’re blocked.',
  '<b>Rules:</b> follow AGENTS.md. Run <code>cargo fmt</code>, <code>cargo clippy</code> and the app’s tests before you finish. Commit with a clear message.',
  '<b>Report:</b> end with a short summary: what you changed, which files, how you tested, and anything left undone.',
];
const taskHead = (extra = '') => `<div class="row g2" style="padding:6px 10px;background:#2c313a;border-bottom:1px solid var(--bv);font-size:12px">${ic('note', 'xs mu')}<span class="b5">Task</span><span class="mu trunc">from “${R_PARENT_SHORT}”</span><span class="grow"></span>${extra}<span class="chip" style="height:18px">Implementation</span></div>`;
const taskBody = (lines, style = '') => `<div style="padding:8px 12px;line-height:21px;font-size:13px;${style}">${lines.map((line) => (line ? `<div>${line.replace(/<code>(.*?)<\/code>/g, (m, t) => rCode(t))}</div>` : '<div style="height:8px"></div>')).join('')}</div>`;
const taskCard = (body, extra = '') => `<div style="border:1px solid var(--b);border-radius:8px;background:var(--ed);overflow:hidden">${taskHead(extra)}${body}</div>`;
const showMore = (label = 'Show more') => `<div class="row g1" style="padding:0 12px 8px;font-size:12px;color:var(--ac)">${label}${ic('chev-down', 'xs')}</div>`;
const subTitleBar = () => `<div class="row g2" style="height:36px;flex:none;padding:0 6px 0 10px;border-bottom:1px solid var(--b);background:rgba(40,44,51,.45)">${ic('forward', 'sm mu')}<span class="trunc" style="font-size:13px">Build the indicator in the picker</span><span class="grow"></span><span class="ibtn" style="color:var(--del)">${ic('stop', 'sm')}</span><span class="ibtn">${ic('dash', 'sm')}</span></div>`;
const subBar = () => `<div class="row" style="min-height:44px;gap:10px;padding:6px 8px 6px 20px;background:var(--ed);border-top:1px solid var(--b);flex:none;font-size:12px">${glyph('codex', 'sm')}<span class="b5">GPT-5.5</span><span class="mu">High</span><span class="mu">Working 1m 20s</span><span class="grow"></span><span class="mu">Runs on its own</span><span class="btn ghost sm" style="color:var(--t)">${ic('stop', 'xs delc')}Stop</span><span class="btn ghost sm" style="color:var(--t)">${ic('arrow-up-left', 'xs')}Open Parent</span></div>`;
const subWork = rFold('Read 6 files, searched code 4 times') + rPara('I’ll add the bar under each account row in the picker, reading the window from the account’s last usage read.');
const subView = (card, h = 470) => frame(`<div class="col" style="height:100%;background:var(--panel)">${rHeader()}${subTitleBar()}<div class="grow" style="min-height:0;overflow:hidden;padding:12px 24px 0;display:flex;flex-direction:column;gap:8px">${card}${subWork}</div>${subBar()}</div>`, { w: 780, h });
const fadeBody = (lines, height) => `<div style="position:relative;max-height:${height}px;overflow:hidden">${taskBody(lines)}<div style="position:absolute;left:0;right:0;bottom:0;height:28px;background:linear-gradient(rgba(40,44,51,0),var(--ed))"></div></div>`;
const instructionsRow = (open = false) => `<div class="row g1" style="padding:6px 12px;border-top:1px solid var(--bv);font-size:12px;color:var(--mu)">${ic(open ? 'chev-down' : 'chev-right', 'xs')}Instructions from the parent<span class="faint">· 3 lines</span></div>`;

TOPICS.push({
  id: 'task', section: 'An open subthread', title: 'The task and its instructions', size: 'wide', rec: 'A',
  now: 'A subthread’s first message is the task its parent wrote, in a card: “Task from ‘…’” with the role, and the whole prompt under it. Parent agents fill it with the same instructions every time (you are a subagent, work only in your worktree, run the checks, report like this), so the task itself is a line or two lost above a screen of rules. Later messages from the parent show whole too, with “Sent by the agent in …” above them.',
  nowImg: 'img/now-task.png',
  issues: ['The instructions push the actual task and the work below the fold.', 'Every subthread repeats them.'],
  options: [
    { key: 'A', name: 'Folded after a few lines', from: 'new',
      desc: 'The card shows the task’s first four lines, fading out, and “Show more” opens the rest. Later messages from the parent fold the same way past four lines.',
      good: 'Works for any agent’s prompts, with nothing asked of them.', cost: 'The cut falls wherever line four ends, not where the instructions start.',
      mock: () => subView(taskCard(fadeBody(TASK_LINES, 106) + showMore())) },
    { key: 'B', name: 'The task, with the instructions folded under it', from: 'new',
      desc: '<code>delegate_task</code> takes the instructions apart from the task (see “What agents can do”): the card shows the task, and “Instructions from the parent · 3 lines” under it opens to them. A task without separate instructions shows whole, as now.',
      good: 'The task reads at a glance, and the rules are a click away.', cost: 'Only works when the parent agent passes them apart.',
      mock: () => subView(taskCard(taskBody(TASK_LINES.slice(0, 1)) + instructionsRow())) },
    { key: 'C', name: 'Folded like a tool call’s input', from: 'Zed tool call cards',
      desc: 'The card is one line, “Task from ‘…’ · Implementation”, with the task’s first line after it. A click opens the whole prompt, as a tool call’s Input opens.',
      good: 'The conversation starts with the work, not the prompt.', cost: 'You have to open it to read even the task itself.',
      mock: () => subView(`<div style="border:1px solid var(--b);border-radius:8px;background:var(--ed);overflow:hidden"><div class="row g2" style="padding:7px 10px;font-size:12px">${ic('chev-right', 'xs mu')}${ic('note', 'xs mu')}<span class="b5 none">Task</span><span class="trunc mu">Build a usage indicator for the agent picker in crates/app/src/agent_view.rs: under each agent…</span><span class="chip none" style="height:18px">Implementation</span></div></div>`, 360) },
    { key: 'D', name: 'As it is', from: 'today',
      desc: 'The whole prompt in the card.',
      good: 'Nothing to change.', cost: 'A screen of rules before the work.',
      mock: () => subView(taskCard(taskBody(TASK_LINES)), 520) },
  ],
});

// What agents can do -------------------------------------------------------------------------
const jsonBlock = (text) => `<div style="border:1px solid var(--b);border-radius:6px;background:var(--ed);padding:8px 10px;font:12px/17px ${R_MONO};color:#c9ccd1;white-space:pre;overflow:hidden">${text}</div>`;
const caption = (text) => `<div style="font:11px -apple-system,sans-serif;color:#e0b45c;margin:6px 0 4px;letter-spacing:.2px">${text}</div>`;
const toolFrame = (html, h) => frame(`<div style="position:absolute;inset:0;overflow:hidden;background:var(--panel);padding:12px 18px;display:flex;flex-direction:column;gap:2px">${html}</div>`, { w: 640, h });
const J = (key) => `<span style="color:#e06c75">"${key}"</span>`;
const S = (text) => `<span style="color:#98c379">"${text}"</span>`;

TOPICS.push({
  id: 'controls', section: 'What agents can do', title: 'More control, with good defaults', size: 'wide', type: 'multi', rec: 'ABCD',
  now: 'An agent starts one subthread per <code>delegate_task</code> call. It gets the task prompt only, works in the parent’s folder unless the agent asks for a worktree or pasture, and runs once: there’s no way to send it more. Its end is announced with “Delegated task 282 reached a terminal state. Use task_status with taskId 282 to read the result.”, so the agent then calls <code>task_status</code> to read each result. The agent can also wait, blocking its turn (<code>mode: "wait"</code>, up to an hour). Pick any of these.',
  options: [
    { key: 'A', name: 'Several subthreads in one call', from: 't3code create_threads',
      desc: '<code>delegate_tasks</code> takes a list of tasks, each with its own title, agent, model and worktree, plus instructions given once for all of them. They start together and show as one batch (“Started 4 subthreads”). <code>delegate_task</code> stays for one.',
      good: 'Splitting work up is one call, and the shared rules aren’t repeated in every prompt.', cost: 'One more tool for agents to learn.',
      mock: () => toolFrame(caption('The call') + jsonBlock(`delegate_tasks({\n  ${J('instructions')}: ${S('Work only in your worktree. Run the checks. End with a summary.')},\n  ${J('tasks')}: [\n    { ${J('title')}: ${S('Research how Zed shows usage')}, ${J('role')}: ${S('research')} },\n    { ${J('title')}: ${S('Build the indicator in the picker')}, ${J('agentId')}: ${S('codex')} },\n    { ${J('title')}: ${S('Write the picker’s tests')}, ${J('agentId')}: ${S('factory-droid')} },\n    { ${J('title')}: ${S('Review the server side of usage reads')}, ${J('role')}: ${S('review')} }\n  ]\n})`) + caption('In the parent') + groupCard(), 470) },
    { key: 'B', name: 'A worktree each by default', from: 'new',
      desc: 'A subthread that edits files (the implementation and test roles) gets its own worktree by default when others run beside it, so they don’t edit the same checkout at once. Research and review work in the parent’s folder. A <code>workspaceStrategy</code> the agent gives still wins.',
      good: 'Parallel work is safe without the agent having to think of it.', cost: 'More worktrees to merge and clean up afterwards.',
      mock: () => toolFrame(caption('With no workspaceStrategy given') + groupCard() + caption('Each implementation and test subthread has its own worktree (⑂); research and review share the parent’s folder.'), 260) },
    { key: 'C', name: 'Send a subthread more work', from: 't3code thread_send',
      desc: '<code>task_send</code> sends a message to a subthread it started, running or ended, which goes on in the same conversation with what it already knows: “Also cover the logged-out state”. It shows in the subthread as a message from the parent, and the parent hears of its end again.',
      good: 'Follow-ups keep the subthread’s context instead of starting over.', cost: 'A subthread can be ended, then running again.',
      mock: () => toolFrame(caption('In the parent') + rRow('reply', `${rVerb('Sent more work to:')}${rTitle('Build the indicator in the picker')}`, { trailing: rOpen() }) + rPara('“Also cover the logged-out state: show no bar, and “Logged out” in the tooltip.”', 'color:var(--mu);padding-left:30px;font-size:13px') + caption('The call') + jsonBlock(`task_send({ ${J('taskId')}: 282, ${J('message')}: ${S('Also cover the logged-out state…')} })`), 250) },
    { key: 'D', name: 'Results in the message', from: 'new',
      desc: 'The message that tells the parent its subthreads ended carries each one’s outcome, summary and changed files, so the agent reads them without calling <code>task_status</code>. Pairs with the result rows in “The results”.',
      good: 'One step less for every result, and the parent answers sooner.', cost: 'Long summaries make the message long (each is cut at the same length task_status cuts it).',
      mock: () => toolFrame(caption('What the parent’s agent receives (not shown in the thread)') + jsonBlock(`Subthreads ended:\n\n• 281 “Research how Zed shows usage”: completed in 2m 14s, no files changed.\n  Zed shows an account’s usage as a thin bar under the model picker’s\n  rows, and the percentage in the row’s tooltip…\n\n• 282 “Build the indicator in the picker”: completed in 5m 12s,\n  3 files changed (+84 −12) in worktree agentz/usage-picker.\n  Added UsageBar under each account row…`) + caption('In the thread') + resultRow(R_DONE[3]) + resultRow(R_DONE[2]), 360) },
    { key: 'E', name: 'Wait for the first of several', from: 'new',
      desc: '<code>task_wait</code> takes several task ids and returns as soon as any one ends (or all of them, if asked), with a short timeout, so the agent can act on the first result and wait again.',
      good: 'The agent can start on results as they come.', cost: 'Keeps the agent’s turn open while it waits, which “How the parent waits” would rather avoid.',
      mock: () => toolFrame(caption('In the parent') + rRow('clock', `${rVerb('Waited for')}${rTitle('1 of 4 subthreads')}`, { trailing: `<span class="xs none" style="color:${R_DIM}">48s</span>` }) + caption('The call') + jsonBlock(`task_wait({ ${J('taskIds')}: [281, 282, 283, 284], ${J('until')}: ${S('any')}, ${J('timeoutMs')}: 120000 })`), 200) },
    { key: 'F', name: 'Limits with defaults', from: 'new',
      desc: 'At most 4 subthreads run at once for a thread; more wait in a queue (“Queued” in their row) and start as others end. A subthread can’t start subthreads of its own unless its parent allows it (<code>canDelegate</code>).',
      good: 'A runaway split can’t start twenty agents at once.', cost: 'A fifth task waits, even when the machine could run it.',
      mock: () => toolFrame(caption('A fifth subthread, queued') + `<div style="border:1px solid var(--b);border-radius:6px;overflow:hidden;background:var(--ed)"><div class="row" style="height:30px;padding:0 8px 0 12px;gap:6px"><span style="width:16px;display:inline-flex;justify-content:center;color:var(--ph)">${ic('clock', 'sm')}</span>${glyph('claude', 'sm')}<span class="trunc" style="font-size:13px">Update the docs for usage indicators</span><span class="sm mu none">· Opus 5.5</span><span class="grow"></span><span class="xs mu none">Queued · starts when one of 4 ends</span></div></div>`, 120) },
  ],
});

// How the parent waits.
const waitingLine = (count = 3) => `<div class="row g2" style="padding:8px 2px;font-size:13px;color:var(--mu)">${rSpin(11)}<span>Waiting for ${count} subthreads</span><span class="faint">· the agent continues when they end</span></div>`;
const blockedRow = rRow('users', `${rVerb('Waiting for')}${rTitle('Build the indicator in the picker')}`, { iconHtml: rSpin(11), trailing: `<span class="xs none" style="color:${R_DIM}">4m 10s</span>` });
const waitFrame = (html, h = 450) => rConversation(rAsk + groupCard() + html, { h, bottom: rComposer() });

TOPICS.push({
  id: 'waiting', section: 'What agents can do', title: 'How the parent waits', size: 'wide', rec: 'A',
  now: 'Agents are told to end their turn and get a message when a subthread ends, but they may also call <code>delegate_task</code> with <code>mode: "wait"</code> (or <code>agentz_thread_wait</code>), which holds their turn open for up to 10 minutes by default and an hour at most, ending early only if the subthread asks a question. Meanwhile the parent’s turn looks stuck on one tool call, and its agent can do nothing else.',
  nowImg: 'img/now-waiting.png',
  issues: ['A waiting agent can’t stop and pick up later; its turn stays open.', 'Once its turn ends, nothing in the parent says it’s waiting for subthreads.'],
  options: [
    { key: 'A', name: 'Always end the turn', from: 't3code delegated tasks',
      desc: '<code>mode: "wait"</code> goes. <code>delegate_task</code> answers at once and tells the agent to end its turn; the results come back in a message that starts its next turn. While it waits, the end of the parent’s conversation says “Waiting for 3 subthreads” with a spinner, and the sidebar keeps showing it as Working.',
      good: 'An agent never sits in a blocked call, and you can see what it’s waiting for.', cost: 'An agent that needs one quick answer before going on must still end its turn.',
      mock: () => waitFrame(rSaid + waitingLine(3)) },
    { key: 'B', name: 'Wait briefly, then end the turn', from: 'new',
      desc: '<code>mode: "wait"</code> stays for quick tasks but returns after 2 minutes at most, saying the subthread is still running and to end the turn. After that, as A: the waiting line, and a message with the results.',
      good: 'A quick task can still be answered in the same turn.', cost: 'Two ways to wait, and agents may still sit for 2 minutes.',
      mock: () => waitFrame(blockedRow.replace('4m 10s', '1m 50s') + rSaid + waitingLine(3), 490) },
    { key: 'C', name: 'As it is, with the waiting line', from: 'today',
      desc: 'Agents may still wait up to an hour. Only the “Waiting for 3 subthreads” line is added, for an agent that ended its turn.',
      good: 'The least to change.', cost: 'Blocked turns stay possible.',
      mock: () => `<div style="display:flex;flex-direction:column;gap:10px">${waitFrame(blockedRow, 400)}${waitFrame(rSaid + waitingLine(3))}</div>` },
  ],
});
