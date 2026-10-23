// Shells next to the Working label: the commands an agent left running (AIR's background
// tasks) and the row a turn ends with. agent_view.rs render_generating,
// render_background_tasks_section, render_activity_bar.

const SHELL_TALK = s3Para('Both halves of the fix are confirmed: the new test fails without each one and passes with it. Updating the architecture doc next.')
  + s3Fold('Ran 3 commands')
  + s3Para('While the server tests run, I’m reading the subthread code (roles, delegation, results, steering) for the remaining items.')
  + s3Fold('Ran 19 commands', '<span class="none delc" style="font-size:12px">2 failed</span>')
  + s3Para('Server tests pass (279 plus the binary tests). I’ll commit the Droid fix by itself, leaving the steering work in progress.')
  + s3Fold('Ran 6 commands and changed 1 file', '<span class="none" style="font-size:12px"><span class="okc">+60</span> <span class="delc">−0</span></span>')
  + s3Para('Status: steering of subthread results passes its new test and clippy is clean. Running the full test suites before committing. Meanwhile I’m checking how agentZ copes with hundreds of subthreads.')
;
const SED_ROW = s3Row('terminal', `<span class="none">Ran</span>${s3Code('sed -n 105,120p crates/app/src/settings_page.rs; wc -l crates/app/src/*.rs | sort -n | tail -30')}`);
const shellThread = (end, { activity = '', h = 440, overlay = '', stop = true } = {}) => s3Thread(SHELL_TALK + SED_ROW + end, { h, activity, composer: s3Composer({ stop }), overlay });
const [TESTS, DEV] = S3_SHELLS;
const shellChip = (task, { hover = false } = {}) => `<span class="row" style="gap:6px;height:22px;padding:0 4px 0 7px;border:1px solid var(--bv);border-radius:5px;min-width:0;max-width:430px;${hover ? 'background:var(--hov)' : ''}">${s3Spin(10)}<span class="trunc" style="font:11.5px ${S3_MONO};color:var(--t)">${task.short}</span><span class="none" style="color:var(--ph)">${task.time}</span>${hover ? s3Stop('var(--mu)') : ''}</span>`;
const shellsChip = (count, open = false) => `<span class="row none" style="gap:5px;height:22px;padding:0 6px 0 7px;border:1px solid var(--bv);border-radius:5px;${open ? 'background:var(--hov);color:var(--t)' : ''}">${s3Spin(10)}${count} shells${ic(open ? 'chev-up' : 'chev-down', 'xs')}</span>`;
const shellsMenu = `<div class="menu" style="bottom:150px;left:200px;width:470px">${S3_SHELLS.map((task) => `<div class="it">${s3Spin(10)}<span class="trunc" style="flex:1;font:11.5px ${S3_MONO}">${task.short}</span><span class="kb" style="margin-left:8px">Shell · ${task.time}</span>${s3Stop('var(--mu)')}</div>`).join('')}</div>`;
const shellRow = (task) => `<div class="row" style="min-height:24px;gap:6px;padding:0 2px 0 22px;color:${S3_DIM};font-size:13px"><span style="width:24px;display:inline-flex;justify-content:center">${s3Spin(10)}</span><span class="row grow" style="gap:4px;min-width:0">${s3Code(task.short)}</span><span class="xs none">Shell · ${task.time}</span>${s3Stop('var(--mu)')}</div>`;

TOPICS.push({
  id: 'shells', section: 'Shells', title: 'Shells next to the Working label', size: 'wide', rec: 'A',
  now: 'While a turn runs, the conversation ends with Zed’s dots and “Working for 13m 58s”. The commands Claude Agent leaves running (JetBrains AIR’s background tasks, which agentZ takes) are in the activity bar on the composer’s top edge: “⌄ 1 Background Task”, then a row each with a spinner, the command, “Shell · 13s” and Stop. A command doesn’t hold the turn, so a dev server left running stays in that panel after the turn ends, with no Working row. The picture is the backlog’s: a Claude Agent thread working for 13m 58s, its test run in the panel.',
  nowImg: 'img/now-shells.png',
  issues: ['What the agent is waiting on and the row that says it’s working are apart, with the end of the conversation between them.'],
  options: [
    { key: 'A', name: 'On the Working row', from: 't3code (a chip on its “Working for” row)',
      desc: 't3code puts a setup script still running as a chip on its “Working for” row. Here each shell is a chip after the label: a spinner, the command cut short in the code font, its time, and Stop on hover, with the whole command in its tooltip. Two or more make one chip, “2 shells ⌄”, which opens a menu of them, each with Stop. The panel goes. Once the turn ends, a shell still running (a dev server) keeps its chip on a row of its own where the Working row was.',
      good: 'Right next to the label, as the backlog asks, in one line.', cost: 'Several shells are a click away.',
      mock: () => s3Stack(shellThread(s3Working('13m 58s', shellChip(TESTS, { hover: true }))), shellThread(s3Working('13m 58s', shellsChip(2, true)), { h: 380, overlay: shellsMenu }), { topCap: 'One shell (hovered)', bottomCap: 'Two shells, the menu open' }) },
    { key: 'B', name: 'Rows under the Working row', from: 'agentZ tool rows',
      desc: 'Under “Working for 13m 58s”, a row per shell in the tool rows’ style, indented: a spinner, the command in the code font, “Shell · 13s” and Stop. The panel goes. Once the turn ends, the shells still running keep their rows at the end of the conversation.',
      good: 'Every shell and its Stop in sight, however many.', cost: 'Three shells make three more rows at the end of the conversation.',
      mock: () => shellThread(s3Working('13m 58s') + shellRow(TESTS) + shellRow(DEV), { h: 460 }) },
    { key: 'C', name: 'The count on the Working row', from: 'new',
      desc: '“Working for 13m 58s · 1 shell”, where “1 shell” opens the panel above the composer, which now starts folded. Once the turn ends with a shell running, the panel stays as today.',
      good: 'Says a shell is running, at the label, with the least moved.', cost: 'The shell itself is still in the panel.',
      mock: () => s3Stack(shellThread(s3Working('13m 58s', `<span class="none" style="color:var(--ac);text-decoration:underline;text-underline-offset:3px;text-decoration-color:rgba(116,173,232,.4)">· 1 shell</span>`), { activity: s3Activity([s3BgSection(undefined, false)]) }), shellThread(s3Working('13m 58s', `<span class="none" style="color:var(--ac)">· 1 shell</span>`), { h: 420, activity: s3Activity([s3BgSection()]) }), { topCap: 'Folded', bottomCap: 'After a click on “1 shell”' }) },
    { key: 'D', name: 'Live where it started', from: 'Zed (a running command’s terminal card)',
      desc: 'Zed shows a running command in its terminal card, where the agent ran it, and drops its spinner while the turn waits on it. Here the row where the agent sent the shell to the background (“Ran cargo test … in the background”) shows it running, with a spinner, its time and Stop, until it ends. The panel goes.',
      good: 'The shell stays with the step that started it.', cost: 'It scrolls up and away as the agent goes on, as it has in the picture.',
      mock: () => s3Thread(SHELL_TALK + s3Row('terminal', `<span class="none">Ran</span>${s3Code(TESTS.short)}<span class="none">in the background</span>`, { iconHtml: s3Spin(10), trailing: `<span class="xs none" style="color:${S3_DIM}">Shell · 13s</span>${s3Stop('var(--mu)')}` }) + SED_ROW + s3Working('13m 58s'), { h: 460, composer: s3Composer({ stop: true }) }) },
    { key: 'E', name: 'As it is', from: 'today',
      desc: 'The Working row in the conversation, the shells in the “1 Background Task” panel above the composer.',
      good: 'Nothing to change.', cost: 'The two stay apart.',
      mock: () => shellThread(s3Working('13m 58s'), { activity: s3Activity([s3BgSection()]) }) },
  ],
});
