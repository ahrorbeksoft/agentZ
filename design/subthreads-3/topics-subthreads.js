// Subthreads, again: the parent while they work, their rows in the sidebar, hundreds of them,
// and the open subthread's bars. agent_view.rs render_waiting_for_subthreads, render_generating,
// render_delegation_batch, render_task_results, render_subthread_title_bar,
// render_subthread_bar; sidebar.rs render_subthread_row, SubthreadFold.

const s3Pair = (left, right, { leftCap, rightCap } = {}) => `<div style="display:flex;gap:12px;align-items:flex-start"><div>${leftCap ? s3Caption(leftCap) : ''}${left}</div><div>${rightCap ? s3Caption(rightCap) : ''}${right}</div></div>`;
const s3Stack = (top, bottom, { topCap, bottomCap } = {}) => `<div style="display:flex;flex-direction:column;gap:8px">${topCap ? s3Caption(topCap) : ''}${top}${bottomCap ? s3Caption(bottomCap) : ''}${bottom}</div>`;

// 1. The parent while its subthreads work ---------------------------------------------------
const parentConversation = (end) => S3_SAID + s3Batch() + s3Results(5, 1) + S3_AFTER + end;

TOPICS.push({
  id: 'parent-working', section: 'While subthreads work', title: 'The parent while its subthreads work', size: 'wide', rec: 'B',
  now: 'Once the parent’s turn ends with subthreads still at work, its conversation ends with a spinner and “Waiting for 3 subthreads · the agent continues when they end” (picked in Subthreads, Machines and Shells, topic 8), where a turn has “Working for 4m 12s”. The composer shows Send, not Stop: a message goes to the agent at once and starts a turn, and a result that comes back meanwhile is steered into that turn when the agent steers its own turns (Claude Agent, Codex on ACP v1), or waits for it to end. In the sidebar the parent already shows Working while a subthread works.',
  nowImg: 'img/now-parent-waiting.png',
  issues: ['The turn looks over while the work goes on: the waiting row reads like a note, not like the agent at work.'],
  options: [
    { key: 'A', name: 'Working for…, as in a turn', from: 'Zed and t3code “Working for”',
      desc: 'The waiting row becomes the row a turn ends with: Zed’s dots and “Working for 4m 12s”, timed from the start of the turn that started the subthreads, counting on until the turn that takes the last result ends. The composer stays as it is: Send, and a message starts a turn at once. The sidebar keeps Working.',
      good: 'The parent reads as one piece of work until it’s done, as the backlog asks.', cost: 'Nothing says what it’s working on, and Send where a turn would have Stop.',
      mock: () => s3Thread(parentConversation(s3Working('4m 12s')), { h: 560 }) },
    { key: 'B', name: 'Working for…, with the count', from: 'Zed and t3code “Working for”, with t3code Lineage’s “· 3 running”',
      desc: 'As A, with how many are still at work after the time: “Working for 4m 12s · 3 subthreads running” (queued ones count as running). The count goes when the last one ends, while the agent takes in the results.',
      good: 'Says why it’s still working although the agent’s turn ended.', cost: 'A slightly longer row.',
      mock: () => s3Thread(parentConversation(s3Working('4m 12s', '<span style="color:var(--ph)">· 3 subthreads running</span>')), { h: 560 }) },
    { key: 'C', name: 'Hold the turn until they end', from: 'agentZ background work (linger_for_background_work)',
      desc: 'The parent’s turn stays open, as it does for Droid’s and Claude Agent’s background work: “Working for 4m 12s · 3 subthreads running”, Stop in the composer, which stops the agent and every subthread still running (queued ones never start), and messages you send queue above the composer until the last subthread has ended and the agent has answered. Send Now in the queue still sends one at once.',
      good: 'Exactly a turn: Stop and the queue work as everywhere else.', cost: 'With 100 subthreads, 4 at a time, a queued message could wait hours, and a Stop meant for the agent ends all their work.',
      mock: () => s3Thread(parentConversation(s3Working('4m 12s', '<span style="color:var(--ph)">· 3 subthreads running</span>')), { h: 620, activity: s3Activity([s3QueueSection(['Also update docs/architecture.md for the steering.'])]), composer: s3Composer({ stop: true }) }) },
    { key: 'D', name: 'Waiting, as t3code shows it', from: 't3code (its sidebar’s “Waiting” state)',
      desc: 't3code has a state for an agent that stopped with work that will wake it (subagents, monitors): “Waiting”, gray and without a spinner, neither Working nor Done. The parent’s card says Waiting while its subthreads work, and its conversation keeps today’s “Waiting for 3 subthreads” row. The composer stays as it is.',
      good: 'True to what happens: the agent itself isn’t running.', cost: 'The opposite of what the backlog asks, and one more state in the sidebar.',
      mock: () => s3Pair(s3Sidebar(s3Parent({ state: 'waiting' }) + s3SubRows(S3_SUBS) + s3Others(), { h: 760 }), s3Thread(parentConversation(s3Waiting(3)), { h: 560, w: 720 })) },
    { key: 'E', name: 'As it is', from: 'today',
      desc: 'The “Waiting for 3 subthreads” row with a spinner, Send in the composer, and Working in the sidebar.',
      good: 'Nothing to change.', cost: 'The conversation says the turn is over while the sidebar says Working.',
      mock: () => s3Thread(parentConversation(s3Waiting(3)), { h: 560 }) },
  ],
});

// 2. Subthread rows in the sidebar ------------------------------------------------------------
const SB_H = 560;
const doneCard = (people) => s3Parent({ state: 'done', time: '1m', people });
const completedFold = (open = false, hover = false) => s3TreeRow(`${ic(open ? 'chev-down' : 'chev-right', 'xs')}${s3Check('xs')}<span>5 completed</span><span class="delc">· 1 failed</span>`, { hover });
const allShown = () => s3Sidebar(doneCard(s3Toggle(true, 8, false)) + s3SubRows(S3_ALL_DONE) + s3Others(), { h: SB_H + 80 });
const doneMenu = `<div class="menu" style="top:236px;left:250px;width:300px">${S3_ENDED.map((s, i) => `<div class="it ${i === 0 ? 'hl' : ''}"><span style="width:12px;text-align:center;color:var(--${s.state === 'failed' ? 'del' : 'ok'})">${s.state === 'failed' ? '×' : '✓'}</span><span class="trunc" style="flex:1">${s.title}</span><span class="kb">${s.ago}</span></div>`).join('')}</div>`;

TOPICS.push({
  id: 'sidebar-rows', section: 'In the sidebar', title: 'Subthread rows in the sidebar', size: 'wide', rec: 'A',
  now: 'Each subthread is a row under its parent’s card, newest first: the agent’s icon, the title and its state (Working, Queued, a status pill, or how long ago it ended), then the agent, model, worktree and the lines it changed. While any runs, all of them show, the finished ones too; once the last one ends they fold away, and the count on the card (“⌄ 👥 8”) folds and unfolds them (“Hide Subthreads”). What you chose holds until another starts or the last one ends.',
  nowImg: 'img/now-sidebar.png',
  issues: ['Five finished rows sit among the three running ones and push the other threads down.'],
  options: [
    { key: 'A', name: 'Running rows, then a Completed fold', from: 't3code Lineage (“Previous agents”, folded)',
      desc: 'Under the card, only the subthreads still running or waiting for you, then one more row in the tree: “› ✓ 5 completed · 1 failed”, which opens the finished ones under it, newest first. Once none runs, they fold into the count as today, and opening the count shows all of them, with no Completed row.',
      good: 'What’s running is right under its parent; the rest is one click away, in the same place.', cost: 'One more row while any run.',
      mock: () => s3Pair(s3Sidebar(s3Parent() + s3SubRows(S3_RUNNING, { more: true }) + completedFold() + s3Others(), { h: SB_H + 80 }), allShown(), { leftCap: 'While 3 run', rightCap: 'Once none runs, opened with the count' }) },
    { key: 'B', name: 'Completed in a menu', from: 'Zed context menu',
      desc: 'The same Completed row, but a click opens a menu beside it with the finished ones: a check or a cross, the title and how long ago it ended. Picking one opens it. The rows under the parent never grow. Once none runs, as A.',
      good: 'The sidebar stays the same height however many finish.', cost: 'A finished one is a menu away, and the menu covers the conversation.',
      mock: () => frame(`<div style="position:absolute;inset:0;background:var(--panel)">${sidebar({ list: s3Parent() + s3SubRows(S3_RUNNING, { more: true }) + completedFold(true, true) + s3Others() })}</div>${doneMenu}`, { w: 560, h: SB_H + 80 }) },
    { key: 'C', name: 'A Completed count on the card', from: 'new',
      desc: 'Running rows only. The parent’s card has a second count beside the subthreads’: “✓ 5”, faint. A click on it shows the finished rows after the running ones, and hides them again. Once none runs, as today: the count opens all of them.',
      good: 'No extra row in the tree.', cost: 'Two small counts side by side on the card, easy to miss.',
      mock: () => s3Pair(
        s3Sidebar(s3Parent({ people: `${s3Toggle(true, 3, true)}<span class="row none" style="gap:2px;font-size:11px;color:var(--faint)">${ic('check', 'xs')}5</span>` }) + s3SubRows(S3_RUNNING) + s3Others(), { h: SB_H + 80 }),
        s3Sidebar(s3Parent({ people: `${s3Toggle(true, 3, true)}<span class="row none" style="gap:2px;font-size:11px;color:var(--mu);background:var(--hov);border-radius:3px;padding:0 2px">${ic('check', 'xs')}5</span>` }) + s3SubRows(S3_SUBS) + s3Others(), { h: SB_H + 80 }),
        { leftCap: 'While 3 run', rightCap: 'With ✓ 5 opened' }) },
    { key: 'D', name: 'Only the running ones', from: 'Subthreads, Machines and Shells, topic 2, option C',
      desc: 'A subthread leaves the sidebar as it ends; the finished ones stay in the parent’s batch card and results. Once none runs, the count opens all of them, as the backlog allows.',
      good: 'The shortest list.', cost: 'While others run, a finished one can only be opened from its parent.',
      mock: () => s3Sidebar(s3Parent() + s3SubRows(S3_RUNNING) + s3Others(), { h: SB_H }) },
    { key: 'E', name: 'As it is', from: 'today',
      desc: 'All of them while any runs, newest first; folded once the last one ends.',
      good: 'Nothing to change.', cost: 'Finished rows mix with running ones.',
      mock: () => s3Sidebar(s3Parent() + s3SubRows(S3_SUBS) + s3Others(), { h: SB_H + 180 }) },
  ],
});

// 3. Hundreds of subthreads -------------------------------------------------------------------
const N = S3_MANY.counts;
const manyRunning = S3_MANY.running;
const manyCard = (people = s3Toggle(true, 100, true)) => s3Parent({ people });
const showMore = (count = 12) => `<div class="row g2" style="height:28px;padding:0 12px;border-top:1px solid var(--b);font-size:12px;color:var(--mu)">${ic('plus', 'xs')}Show ${count} more</div>`;
const groupHead = (open, label, count, extra = '') => `<div class="row g15" style="height:26px;padding:0 8px;border-top:1px solid var(--b);background:#2a2e36;font-size:12px;color:var(--mu)">${ic(open ? 'chev-down' : 'chev-right', 'xs')}<span style="color:var(--t)">${label}</span><span>${count}</span>${extra}<span class="grow"></span></div>`;
const head100 = (extra = `<span class="sm mu">· ${N.running} running</span>`) => s3BatchHead(`Started ${N.total} subthreads`, extra);
// The parent's conversation beside its sidebar: the batch card, a results row, Working.
const scaleView = (list, card, { results = s3Results(12, 1), h = 640 } = {}) => frame(`<div class="body" style="height:100%">${sidebar({ list })}<div class="col grow" style="height:100%;background:var(--panel)">${s3Header()}<div class="grow" style="min-height:0;overflow:hidden;padding:4px 30px 0;display:flex;flex-direction:column;gap:2px">${S3_SAID.replace('eight subthreads, one per backlog item', 'a hundred subthreads, one per backlog item')}${card}${results}${s3Waiting(N.running + N.queued)}</div></div></div>`, { w: 1180, h });

const ROLE_GROUPS = [
  { label: 'Workers', running: manyRunning.filter((s) => s.role === 'Worker'), done: 14, total: 60 },
  { label: 'Researchers', running: manyRunning.filter((s) => s.role === 'Researcher'), done: 9, total: 20 },
  { label: 'Reviewers', running: manyRunning.filter((s) => s.role === 'Reviewer'), done: 7, total: 20, failed: 1 },
];
const roleExtra = (g) => `<span style="color:var(--ac)">· ${g.running.length} running</span>${g.failed ? `<span class="delc">· ${g.failed} failed</span>` : ''}`;

TOPICS.push({
  id: 'hundreds', section: 'Hundreds of subthreads', title: 'Hundreds of subthreads', size: 'wide', rec: 'C',
  now: 'Nothing is capped. With 100 tasks (8 running, one of them waiting for approval, 30 ended, 62 queued), the sidebar puts all 100 rows under the parent while any runs, the queued ones saying Queued. The parent’s batch card, “Started 100 subthreads · 8 running”, has a 30px row each, so it’s 3,000px tall, in the order the agent listed them. Each results notice, “12 subthreads finished · 1 failed”, opens to a row per result. The server side holds up already: it sends the app only the threads that changed, <code>delegate_tasks</code> takes 100 at once, and Subthreads at once (1–16) keeps the rest queued. The picture is the parent with 100 tasks; the sidebar with them is <a href="img/now-sidebar-hundred.png" target="_blank">now-sidebar-hundred.png</a>.',
  nowImg: 'img/now-parent-hundred.png',
  issues: ['The 8 running and the one waiting for you are lost among 92 others, in the sidebar and in the card.', 'The parent’s conversation is mostly one card.'],
  options: [
    { key: 'A', name: 'Caps, with Show more', from: 't3code (Lineage’s and the sidebar’s Show more)',
      desc: 't3code shows a few related threads, then “Show 12 more”. Here: under the parent, the first 6 subthreads (waiting for you, then running, then ended, then queued), then “Show 12 more”; the batch card the same; a results row opens to 6, then Show more.',
      good: 'Simple, and the same in all three places.', cost: 'What’s behind Show more is a mix; finding one finished task can take several clicks.',
      mock: () => scaleView(manyCard() + s3SubRows(manyRunning.slice(0, 6), { more: true }) + s3TreeRow(`${ic('plus', 'xs')}Show 12 more`) + s3Others(),
        s3BatchCard(head100() + manyRunning.slice(0, 6).map(s3BatchRow).join('') + showMore())) },
    { key: 'B', name: 'A progress bar', from: 'new',
      desc: 'The batch card’s header has a bar in four colors with “30 done · 1 failed · 8 running · 62 queued”, and rows only for the ones running or failed; “Show all 100” opens every row. Under the parent in the sidebar, the running rows, then one row with the same bar and “62 queued · 30 done”, which opens the batch card. Results rows stay as they are.',
      good: 'How far along the whole project is, at a glance.', cost: 'Queued and finished ones are only in the parent’s card.',
      mock: () => {
        const bar = (wide) => `<span class="row" style="width:${wide}px;height:6px;border-radius:3px;overflow:hidden;flex:none;background:var(--bv)"><i style="width:29%;height:100%;background:var(--ok)"></i><i style="width:1%;height:100%;background:var(--del)"></i><i style="width:8%;height:100%;background:var(--ac)"></i></span>`;
        const card = s3BatchCard(head100(`<span class="grow"></span>${bar(160)}<span class="sm mu">30 done · <span class="delc">1 failed</span> · 8 running · 62 queued</span>`) + manyRunning.map(s3BatchRow).join('') + s3BatchRow(S3_SUBS[6]) + `<div class="row g2" style="height:28px;padding:0 12px;border-top:1px solid var(--b);font-size:12px;color:var(--mu)">${ic('chev-down', 'xs')}Show all 100</div>`);
        return scaleView(manyCard() + s3SubRows(manyRunning, { more: true }) + s3TreeRow(`${bar(54)}<span>62 queued · 30 done</span>`) + s3Others(), card, { h: 700 });
      } },
    { key: 'C', name: 'Grouped by state', from: 't3code Lineage’s groups (active, previous) and workflow phases',
      desc: 'Rows group by state: running (waiting for you first) shows; “› 62 Queued” and “› 30 Done · 1 failed” are folded, each opening to its rows 12 at a time with Show more. The batch card has these groups under its header; in the sidebar, the running rows and then a fold row for each of the others (topic 2’s Completed row, with a Queued one beside it). A results row that opens lists the failed ones first.',
      good: 'What needs you and what runs are always on top, and the rest is counted, not listed.', cost: 'Two fold rows in the sidebar while a big batch runs.',
      mock: () => scaleView(manyCard() + s3SubRows(manyRunning, { more: true }) + s3TreeRow(`${ic('chev-right', 'xs')}${s3Clock('xs')}<span>62 queued</span>`, { last: false }) + s3TreeRow(`${ic('chev-right', 'xs')}${s3Check('xs')}<span>30 completed</span><span class="delc">· 1 failed</span>`) + s3Others(),
        s3BatchCard(head100() + groupHead(true, 'Running', 8) + manyRunning.map(s3BatchRow).join('') + groupHead(false, 'Queued', 62) + groupHead(false, 'Done', 30, '<span class="delc">· 1 failed</span>')), { h: 700 }) },
    { key: 'D', name: 'Grouped by role', from: 't3code workflow phases (“12/40 agents”)',
      desc: 't3code groups a workflow’s agents by phase, each with how many have settled. Here the groups are the roles: “Workers · 4 running · 14/60”, “Researchers · 2 running · 9/20”, “Reviewers · 2 running · 1 failed · 7/20”. In the card, a group with one waiting for you starts open; each opens to its rows, running first. In the sidebar, a row per role under the parent, which opens the same way.',
      good: 'Matches how the work was split up, and the sidebar stays at three rows.', cost: 'Which one runs is a click into its group; a task with no role lands in Other.',
      mock: () => scaleView(manyCard() + ROLE_GROUPS.map((g, i) => s3TreeRow(`${ic('chev-right', 'xs')}<span style="color:var(--t)">${g.label}</span>${roleExtra(g)}`, { last: i === ROLE_GROUPS.length - 1, end: `<span class="xs faint">${g.done}/${g.total}</span>` })).join('') + s3Others(),
        s3BatchCard(head100() + ROLE_GROUPS.map((g) => groupHead(g.label === 'Reviewers', g.label, `${g.done}/${g.total}`, roleExtra(g)) + (g.label === 'Reviewers' ? g.running.map(s3BatchRow).join('') : '')).join('')), { h: 560 }) },
    { key: 'E', name: 'As it is', from: 'today',
      desc: 'Every row, everywhere, in the order they were created or listed.',
      good: 'Nothing to change.', cost: 'Hundreds of rows in the sidebar and a card thousands of pixels tall.',
      mock: () => {
        const mixed = [S3_SUBS[4], S3_MANY.queued[0], manyRunning[1], S3_SUBS[7], S3_MANY.queued[1], manyRunning[0], S3_SUBS[6], S3_MANY.queued[2], S3_MANY.queued[3], manyRunning[5], S3_SUBS[3], S3_MANY.queued[4], S3_MANY.queued[5], manyRunning[2]];
        return `<div style="position:relative">${scaleView(manyCard() + s3SubRows(mixed, { more: true }), s3BatchCard(head100() + mixed.map(s3BatchRow).join('')), { results: '' })}<span class="overlay-note" style="position:absolute;left:90px;bottom:48px;font:11px -apple-system,sans-serif;background:#f5d76e;color:#1b1f26;padding:3px 7px;border-radius:4px">…and 86 more rows</span><span style="position:absolute;left:640px;bottom:12px;font:11px -apple-system,sans-serif;background:#f5d76e;color:#1b1f26;padding:3px 7px;border-radius:4px">…and 86 more rows, 2,600px</span></div>`;
      } },
  ],
});

// 4. The open subthread's bars ----------------------------------------------------------------
const SUB = S3_SUBS[0];
const taskCard = () => `<div style="border:1px solid var(--b);border-radius:8px;background:var(--ed);overflow:hidden"><div class="row g2" style="padding:6px 10px;background:#2c313a;border-bottom:1px solid var(--bv);font-size:12px">${ic('note', 'xs mu')}<span class="b5">Task</span><span class="mu trunc">from “${S3_PARENT}”</span><span class="grow"></span><span class="chip" style="height:18px">Worker</span></div>
  <div style="position:relative;max-height:66px;overflow:hidden;padding:8px 12px;line-height:21px;font-size:13px">When a subthread ends while its parent’s turn runs, tell the parent in that turn if its agent steers its own turns (Claude Agent, Codex on ACP v1); otherwise keep the notice until the turn ends.<div style="position:absolute;left:0;right:0;bottom:0;height:24px;background:linear-gradient(rgba(40,44,51,0),var(--ed))"></div></div>
  <div class="row g1" style="padding:0 12px 8px;font-size:12px;color:var(--ac)">Show more${ic('chev-down', 'xs')}</div></div>`;
const subWork = (done) => taskCard() + s3Fold('Read 9 files, searched code 5 times') + s3Para('The server already knows which agents steer their own turns, so the notice can go into the turn through the same path as a steered message.') + s3Fold('Changed 2 files and ran 4 commands', '<span class="none" style="font-size:12px"><span class="okc">+60</span> <span class="delc">−4</span></span>') + (done ? s3Para('Done. A result that ends mid-turn now reaches Claude Agent and Codex in that turn; other agents get it when the turn ends. Tests pass, committed in <code>agentz/steer-results</code>.') : s3Working('3m 40s'));
const subMark = (done) => (done ? s3Check() : s3Braille());
const titleBar = ({ done = false, right = '' } = {}) => `<div class="row g2" style="height:36px;flex:none;padding:0 4px 0 10px;border-bottom:1px solid var(--b);background:rgba(40,44,51,.45)">${ic('forward', 'sm mu')}<span class="trunc" style="font-size:13px">${SUB.title}</span>${done ? s3Check() : ''}<span class="grow"></span>${right}</div>`;
const todayTitleRight = (done) => `${done ? '' : `<span class="ibtn" style="color:var(--del)">${ic('stop', 'sm')}</span>`}<span class="ibtn">${ic('dash', 'sm')}</span>`;
const subBar = ({ done = false, buttons = '' } = {}) => `<div class="row" style="min-height:44px;gap:10px;padding:6px 8px 6px 20px;background:var(--ed);border-top:1px solid var(--b);flex:none;font-size:12px">${glyph('claude', 'sm')}<span>Opus 5.5</span><span class="mu">Xhigh</span><span class="mu">${done ? 'Done in 6m 12s' : 'Working 3m 40s'}</span><span class="grow"></span><span class="mu">Runs on its own</span>${buttons}</div>`;
const todayBarButtons = (done) => `${done ? '' : `<span class="btn ghost sm" style="color:var(--t)">${ic('stop', 'xs delc')}Stop</span>`}<span class="btn ghost sm" style="color:var(--t)">${ic('arrow-up-left', 'xs')}Open Parent</span>`;
const subView = ({ done = false, header = s3Header(), bar = '', bottom, h = 430 } = {}) => frame(`<div class="col" style="height:100%;background:var(--panel)">${header}${bar}<div class="grow" style="min-height:0;overflow:hidden;padding:12px 34px 0;display:flex;flex-direction:column;gap:4px">${subWork(done)}</div>${bottom ?? subBar({ done })}</div>`, { w: 820, h });
const subPair = (view) => s3Stack(view(false), view(true), { topCap: 'Running', bottomCap: 'Done' });
const crumbHeader = (done) => s3Header(`<span class="row g1" style="min-width:0;font-size:13px"><span class="trunc mu" style="flex:0 1 auto;text-decoration:underline;text-decoration-color:var(--b);text-underline-offset:3px">${S3_PARENT}</span><span class="mu none">›</span>${subMark(done)}<span class="trunc" style="flex:0 1 auto">${SUB.title}</span>${ic('chev-down', 'xs mu')}</span>`);

TOPICS.push({
  id: 'subthread-bars', section: 'An open subthread', title: 'The open subthread’s bars', size: 'wide', rec: 'B',
  now: 'An open subthread has two bars. Under the parent’s header (“agentZ / Implement the backlog in one go ⌄”, whose menu acts on the parent), Zed’s subagent title bar: ↳, the subthread’s title, a green check once done (a red × and a dashed border once failed or stopped), and at its right Stop, a red square, while it can be stopped, and – (Minimize Subthread, back to the parent, Ctrl-−). In the composer’s place, t3code’s bar: the agent’s icon, model and effort, “Working 3m 40s” or “Done in 6m 12s”, “Runs on its own”, then the buttons “■ Stop” and “↖ Open Parent”. Once it has ended both Stops go, and Minimize and Open Parent stay. Ended, it looks like <a href="img/now-subthread-done.png" target="_blank">now-subthread-done.png</a>.',
  nowImg: 'img/now-subthread-running.png',
  issues: ['While it runs there are two Stops and two ways back.', 'The labelled buttons at the bottom right are heavier than anything else in the bar.'],
  options: [
    { key: 'A', name: 'A breadcrumb, no buttons', from: 'new',
      desc: 'The header names both: “agentZ / Implement the backlog in one go › ⠋ Worker: steer results into the running turn ⌄”, with the subthread’s state before its title. A click on the parent’s title opens it (its tooltip shows Ctrl-−). The title bar goes, and the bottom bar keeps only its words. Stop is on its row in the parent’s batch card, and in the command palette (“Stop Subthread”).',
      good: 'The quietest: nothing to press but the way back, which reads as where you are.', cost: 'Stopping a subthread from inside it takes the command palette.',
      mock: () => subPair((done) => subView({ done, header: crumbHeader(done) })) },
    { key: 'B', name: 'Zed’s title bar alone', from: 'Zed subagent title bar',
      desc: 'The title bar stays as it is, with Zed’s small Stop and – (Minimize, Ctrl-−), and the bottom bar loses its buttons: the agent’s icon, model, effort, the time and “Runs on its own”. One Stop and one way back, both icon buttons with tooltips, as Zed has them.',
      good: 'The least to change, and Zed’s look.', cost: 'The way back is a – whose meaning you learn from its tooltip.',
      mock: () => subPair((done) => subView({ done, bar: titleBar({ done, right: todayTitleRight(done) }) })) },
    { key: 'C', name: 't3code’s bar alone', from: 't3code ProviderSubagentBar',
      desc: 'The title bar keeps the title and its state but no buttons. The bottom bar is t3code’s as t3code has it: no Stop, and a quiet ghost “↖ Open parent” at its end. Stop is in the parent’s batch card and the command palette.',
      good: 'One way back, where your eyes end up, in t3code’s quiet style.', cost: 'No Stop inside the subthread.',
      mock: () => subPair((done) => subView({ done, bar: titleBar({ done }), bottom: subBar({ done, buttons: `<span class="btn ghost sm">${ic('arrow-up-left', 'xs')}Open parent</span>` }) })) },
    { key: 'D', name: 'Everything on the title bar', from: 'Zed subagent title bar with t3code’s words',
      desc: 'The bottom bar goes and the conversation runs to the bottom. The title bar has ↳, the title and its state, then muted “Opus 5.5 · Working 3m 40s”, Stop while it runs, and –.',
      good: 'One bar, one line, every control once.', cost: 'Long titles cut sooner, and nothing marks where the conversation ends.',
      mock: () => subPair((done) => subView({ done, bar: titleBar({ done, right: `<span class="row g1 sm mu none" style="padding-right:6px">${glyph('claude', 'sm')}Opus 5.5 · ${done ? 'Done in 6m 12s' : 'Working 3m 40s'}</span>${todayTitleRight(done)}` }), bottom: '' })) },
    { key: 'E', name: 'As it is', from: 'today',
      desc: 'Both bars, each with its Stop and its way back.',
      good: 'Nothing to change.', cost: 'Doubled buttons, and the heavy ones at the bottom.',
      mock: () => subPair((done) => subView({ done, bar: titleBar({ done, right: todayTitleRight(done) }), bottom: subBar({ done, buttons: todayBarButtons(done) }) })) },
  ],
});
