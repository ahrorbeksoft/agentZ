// Project copies: one project on several machines, and whether its copies are in sync.

// A panel listing a project's copies, used by the popover, settings and comparison mocks.
const panelHead = (project, right = '') => `<div class="row g2" style="padding:10px 12px;border-bottom:1px solid var(--bv)">${projectIcon(project)}<span class="b5">${project.label}</span><span class="grow"></span>${right}</div>`;
const verdict = (project) => (inSync(project)
  ? `<span class="row g1 sm okc">${ic('check-circle', 'xs')}In sync</span>`
  : `<span class="row g1 sm warnc">${ic('alert', 'xs')}${differences(project).length} differences</span>`);
const copyRow = (copy, detail, { last = false, mark = '' } = {}) => `<div class="row g2" style="padding:8px 12px;align-items:flex-start;${last ? '' : 'border-bottom:1px solid var(--bv)'}"><span style="margin-top:2px">${machineIc(copy.machine)}</span><div class="col grow" style="gap:2px;min-width:0"><span class="row g15">${machineName(copy.machine)}<span class="xs ph trunc">${copy.path}</span></span><span class="sm">${detail}</span></div>${mark}</div>`;
const panel = (project, body, { w = 420, head, foot = '', style = '' } = {}) => `<div class="pop" style="position:absolute;${style};width:${w}px;font-size:13px">${head === undefined ? panelHead(project, verdict(project)) : head}${body}${foot ? `<div class="row g2 xs ph" style="padding:7px 12px;border-top:1px solid var(--bv)">${foot}</div>` : ''}</div>`;
const inFrame = (html, w, h) => frame(html, { w, h, style: 'background:var(--ed)' });
const remoteRows = (project) => project.copies.map((copy, index) => copyRow(copy, `${ic('branch', 'xs mu')} ${copy.branch} ${upstreamText(copy)} · ${changesText(copy)}${copy.stash ? ` · <span class="warnc">${copy.stash} stash</span>` : ''}`, { last: index === project.copies.length - 1 })).join('');

// 1. The name --------------------------------------------------------------------------------
TOPICS.push({
  id: 'name', section: 'One project', title: 'One name for a combined project', size: 'narrow', rec: 'A',
  now: 'Copies with the same primary remote are already one project in the project switcher (one row under “On several machines”, its machines after the name), the title bar, the All projects count, New Thread’s project list (it starts in the copy used last; the composer’s machine picker moves it), the sidebar’s project filter (every copy’s threads) and Settings (one page with a machine dropdown, from the settings-projects round). What still differs by machine: the switcher names a project whose copies share the repository’s name by its owner and repository, “ahrorbeksoft/ielts-today”, while each thread card, draft row and Go To entry names its own copy, “ielts-today”, with that copy’s icon. Threads, workspaces and terminals each run on one machine, so they keep its icon. A change in progress (not committed) shows the switcher’s machines as icons instead of names.',
  nowImg: 'img/now-switcher-machines.png',
  issues: [
    'The same project reads “ahrorbeksoft/ielts-today” in the switcher and “ielts-today” on its cards.',
    'A copy whose folder has another name (“fluency” on one machine) shows that name on its cards, so it looks like another project.',
  ],
  options: [
    { key: 'A', name: 'The switcher’s name everywhere', from: 't3code',
      desc: 'Cards, draft rows, Go To and the details popover use the combined project’s name, as t3code’s project groups give one name to “the sidebar and thread lists”. The machine icon at the card’s end still says which copy the thread is in. A name set in Project Settings › Name replaces it everywhere.',
      good: 'One project reads the same wherever it shows.', cost: '“ahrorbeksoft/ielts-today” is long for a 290 px card.',
      mock: () => frame(`<div class="sidebar" style="width:290px">${sbHead()}<div class="sb-list">${card({ project: IT, name: IT.label, active: true })}${card({ project: IT, name: IT.label, machine: 'laptop', title: 'Add reading tests', state: 'working', branch: 'agentz/brave-otter-3fa' })}${card({ project: FU, name: FU.label, machine: 'devbox', title: 'Charge cards with Payme', state: 'pending', branch: 'payments' })}</div></div>`, { w: 290, h: 330 }) },
    { key: 'B', name: 'The project’s own name everywhere', from: 'new',
      desc: 'The other way round: the switcher and title bar say “ielts-today” too, as the cards do. The owner is added only when two projects in the list would share a name.',
      good: 'Short names, and the one the user knows the folder by.', cost: 'Unlike t3code’s rule for naming groups, which agentZ copied.',
      mock: () => frame(`<div class="m" style="width:380px;height:330px;position:relative">${switcher(PROJECTS.map((project, index) => switcherRow({ ...project, label: project.name }, { selected: index === 0, current: index === 0 })).join(''), { style: 'left:8px;top:8px' })}</div>`, { w: 380, h: 330 }) },
    { key: 'C', name: 'As today', from: 'agentZ today',
      desc: 'The switcher combines, and each card names its own copy. Folders named differently on two machines stay told apart on their cards.',
      good: 'Nothing changes.', cost: 'The two names for one project stay.',
      mock: () => frame(`<div class="sidebar" style="width:290px">${sbHead()}<div class="sb-list">${card({ project: IT, active: true })}${card({ project: IT, machine: 'laptop', title: 'Add reading tests', state: 'working', branch: 'agentz/brave-otter-3fa' })}${card({ project: FU, name: 'fluency', machine: 'devbox', title: 'Charge cards with Payme', state: 'pending', branch: 'payments' })}</div></div>`, { w: 290, h: 330 }) },
  ],
});

// 2. Where it shows ------------------------------------------------------------------------
const differsMark = `<span class="row" style="color:var(--warn)" title="Copies differ">${ic('alert', 'xs')}</span>`;
const fuTip = `<div class="b5" style="margin-bottom:2px">Copies differ</div>${differences(FU).map((line) => `<div class="mu">${line}</div>`).join('')}`;
TOPICS.push({
  id: 'where', section: 'In sync', title: 'Where you see whether the copies are in sync', size: 'wide', rec: 'B',
  now: 'Nowhere. Each machine’s server reads the branch checked out in each project every 5 seconds and sends it with the projects; cards show their thread’s branch. Nothing reads or shows a copy’s commit, how it stands against its remote, uncommitted changes or stashes, or compares one machine’s copy with another’s. The Workspaces view has a git popover for one workspace (branch → upstream with ↑↓, uncommitted files and lines, the last commit, the path, its worktrees), which the options below borrow from.',
  nowImg: 'img/now-project-settings.png',
  issues: [
    'Starting a thread on Devbox 1 doesn’t tell you its copy is 12 commits behind, or on another branch.',
    'Uncommitted work left on one machine is easy to forget once you work on another.',
  ],
  options: [
    { key: 'A', name: 'A mark on the switcher row', from: 't3code’s machine badge',
      desc: 'A warning-colored mark after the machines of a project whose copies differ, in the switcher and the title bar. Hovering it lists the differences. Projects in sync show nothing extra.',
      good: 'Seen every time you switch projects, with no new place to visit.', cost: 'A hover shows only a short list, and the mark is easy to miss.',
      mock: () => `<div class="m" style="width:720px;height:330px;position:relative">${titleBar(switcherTrigger(AZ, differsMark), { w: 720 })}${switcher(PROJECTS.map((project, index) => switcherRow(project, { selected: index === 2, current: index === 0, after: inSync(project) ? '' : differsMark })).join(''), { w: 380 })}${tip(fuTip, 'left:400px;top:150px;width:300px')}</div>` },
    { key: 'B', name: 'A Copies section in Project Settings', from: 't3code’s Checkouts section',
      desc: 'The project’s page gets a section listing every copy, as t3code’s project settings list each machine’s checkout: its machine, folder, branch against its remote, uncommitted changes and stash, with what differs in the warning color and a verdict at the top. It sits under Repository, above the chosen machine’s own section.',
      good: 'Room for every detail, beside the rest of the project’s settings.', cost: 'You have to open settings to see it.',
      mock: () => settingsPage('fluency.uz', [setSection('Repository', [setRow('Repository', 'ahrorbeksoft/fluency.uz · origin https://github.com/ahrorbeksoft/fluency.uz.git')]), setSection('Copies', FU.copies.map((copy) => setRow(`<span class="row g15">${machineIc(copy.machine)}${machineName(copy.machine)}</span>`, `${copy.path}<br>${ic('branch', 'xs mu')} ${copy.branch} ${upstreamText(copy)} · ${changesText(copy)}${copy.stash ? ` · <span class="warnc">${copy.stash} stash</span>` : ''}`)), { right: verdict(FU) })], { right: machineDropdown() }) },
    { key: 'C', name: 'A popover from the title bar', from: 'agentZ’s Workspaces git popover',
      desc: 'Clicking the machines after the project’s name in the title bar opens a popover with a row per copy, as the Workspaces view’s git popover shows one workspace. Its header says In sync or how many differences there are.',
      good: 'One click from anywhere in the Agents view.', cost: 'A second control in the title bar beside the switcher.',
      mock: () => `<div class="m" style="width:720px;height:330px;position:relative">${titleBar(switcherTrigger(FU), { w: 720 })}${panel(FU, remoteRows(FU), { style: 'left:96px;top:40px', w: 470 })}</div>` },
    { key: 'D', name: 'In New Thread’s machine picker', from: 'agentZ’s machine picker',
      desc: 'The composer’s machine menu, which moves a new thread to another copy, shows each copy’s branch and how it stands: “main ↓12”, “payments, 3 not pushed”, “3 uncommitted”. It shows where you choose the copy and nowhere else.',
      good: 'Right where it matters: before a thread starts in a stale copy.', cost: 'Only seen while starting a thread.',
      mock: () => inFrame(`<div style="position:absolute;left:20px;top:20px;width:640px">${composerStub()}<div class="row g2" style="margin-top:8px;font-size:12px"><span class="row g1 mu">${ic('folder', 'xs')}Local${ic('chev-down', 'xs')}</span><span class="row g1" style="position:relative">${machineIc('devbox', 'xs mu')}Devbox 1${ic('chev-down', 'xs mu')}<div class="menu" style="top:22px;left:0;min-width:330px">${FU.copies.map((copy) => `<div class="it ${copy.machine === 'devbox' ? 'hl' : ''}" style="height:40px">${machineIc(copy.machine)}<span class="col grow" style="gap:0"><span>${machineName(copy.machine)}</span><span class="xs mu">${copy.branch} ${upstreamText(copy)}${copy.changes ? ` · ${copy.changes.files} uncommitted` : ''}${copy.stash ? ` · ${copy.stash} stash` : ''}</span></span>${copy.machine === 'devbox' ? ic('check', 'sm') : ''}</div>`).join('')}</div></span><span class="grow"></span><span class="row g1 mu">${ic('branch', 'xs')}payments</span></div></div>`, 680, 300) },
    { key: 'E', name: 'A line at the top of the sidebar', from: 'new',
      desc: 'With one project shown, a line under the sidebar’s search says “Copies differ: Devbox 1 is on payments” in the warning color, and opens C’s popover. Nothing shows while they’re in sync, or with all projects shown.',
      good: 'Seen while you work in the project, without asking.', cost: 'Takes a row from the thread list, and only for one project at a time.',
      mock: () => frame(`<div class="sidebar" style="width:290px">${sbHead()}<div class="row g15 sm" style="padding:7px 12px;border-bottom:1px solid var(--bv);color:var(--warn)">${ic('alert', 'xs')}<span class="grow trunc">Copies differ: Devbox 1 is on payments</span>${ic('chev-right', 'xs')}</div><div class="sb-list">${card({ project: FU, title: 'Add the lesson player', active: true })}${card({ project: FU, machine: 'devbox', title: 'Charge cards with Payme', state: 'pending', branch: 'payments' })}</div></div>`, { w: 290, h: 300 }) },
  ],
});

// A composer with nothing typed, as the new thread screen draws it.
function composerStub() {
  return `<div style="border:1px solid var(--b);border-radius:8px;background:var(--panel);padding:10px 10px 8px"><div class="ph" style="height:34px">Message the agent…</div><div class="row g2 sm mu">${ic('plus', 'sm')}${glyph('claude', 'sm')}Claude Agent${ic('chev-down', 'xs')}<span class="grow"></span>Default${ic('chev-down', 'xs')} Sonnet${ic('chev-down', 'xs')}<span class="ibtn sm">${ic('arrow', 'xs')}</span></div></div>`;
}

// 3. What it compares ----------------------------------------------------------------------
const compareFrame = (body, h = 230) => inFrame(panel(FU, body, { style: 'left:12px;top:12px', w: 470 }), 494, h);
TOPICS.push({
  id: 'compare', section: 'In sync', title: 'What each copy is compared with', size: 'medium', rec: 'A',
  now: 'Nothing is compared. Each server knows only its own copy; the app gets each copy’s branch name and nothing else. Two copies can only be compared by commit where one machine has the other’s commit: after it fetched what the other pushed.',
  issues: [
    '“In sync” has to mean something definite: the same branch and commit, nothing uncommitted, and up to date with the remote?',
  ],
  options: [
    { key: 'A', name: 'Each copy against its remote', from: 't3code’s git status',
      desc: 'Every copy says how its branch stands against what it tracks, as t3code’s and the Workspaces view’s status do: “main → origin/main ↓12”. The copies are in sync when they’re on the same branch and commit, with nothing uncommitted and no stash. Each server answers for its own copy, so nothing has to cross machines.',
      good: 'Each line is true on its own machine, with no cross-machine git.', cost: 'Two copies with unpushed work on the same branch both say “↑2”, and you can’t tell whether it’s the same work.',
      mock: () => compareFrame(remoteRows(FU)) },
    { key: 'B', name: 'Each copy against This Mac’s', from: 'new',
      desc: 'This Mac’s copy (or the first one) is the reference, and every other copy is described against it: “1 commit behind This Mac”, “on payments, not main”. Counting needs This Mac’s commit on that machine; without it, the line says “has commits This Mac doesn’t”.',
      good: 'Answers “is the devbox where my Mac is?” directly.', cost: 'Needs the other machine to have the commit, which it only has after a fetch of pushed work.',
      mock: () => compareFrame(FU.copies.map((copy, index) => copyRow(copy, index === 0 ? `${ic('branch', 'xs mu')} main · 9f8e7d6 <span class="mu">reference</span>` : copy.branch !== 'main' ? `<span class="warnc">on payments, not main</span> · has commits This Mac doesn’t` : `<span class="delc">12 commits behind This Mac</span> · ${copy.stash} stash`, { last: index === FU.copies.length - 1 })).join('')) },
    { key: 'C', name: 'Copies grouped by commit', from: 'new',
      desc: 'The copies are grouped by the commit they have checked out, with its short hash and subject, so copies at the same commit share a line. One group means the same code everywhere; uncommitted changes are marked on their machine.',
      good: 'Plainly shows which copies have the same code.', cost: 'Says nothing about which commit is newer.',
      mock: () => compareFrame([['9f8e7d6', 'Add the lesson player', 'main', ['mac']], ['77ab12c', 'Charge cards with Payme', 'payments', ['devbox']], ['3a4b5c6', 'Translate the home page', 'main', ['laptop']]].map(([hash, subject, branch, machines], index) => `<div class="row g2" style="padding:8px 12px;${index < 2 ? 'border-bottom:1px solid var(--bv)' : ''}">${ic('commit', 'sm mu')}<span class="mono-font sm ac">${hash}</span><span class="col grow" style="gap:1px;min-width:0"><span class="trunc">${subject}</span><span class="xs mu">${branch}</span></span>${machines.map((machine) => `<span class="chip">${machineIc(machine, 'xs mu')}${machineName(machine)}${machine === 'devbox' ? ' · 1 uncommitted' : ''}</span>`).join('')}</div>`).join('')) },
    { key: 'D', name: 'Each copy against the default branch', from: 't3code’s “ahead of default”',
      desc: 'Every copy says how far its branch is from the remote’s default branch, origin/main, as t3code counts a branch’s commits ahead of the default: “payments: 3 ahead of origin/main”, “main: 12 behind origin/main”.',
      good: 'One fixed point for every copy, whatever branch it’s on.', cost: 'A copy on its own feature branch always shows as different, though that may be the plan.',
      mock: () => compareFrame(FU.copies.map((copy, index) => copyRow(copy, `${ic('branch', 'xs mu')} ${copy.branch}: ${copy.machine === 'devbox' ? '<span class="okc">3 ahead</span> of origin/main' : copy.behind ? `<span class="delc">${copy.behind} behind</span> origin/main` : '<span class="mu">at origin/main</span>'}`, { last: index === FU.copies.length - 1 })).join('')) },
  ],
});

// 4. How differences are listed ------------------------------------------------------------
const listFrame = (body, h = 250, w = 470) => inFrame(panel(FU, body, { style: 'left:12px;top:12px', w }), w + 24, h);
const tableCell = (html, warn = false, w = 'auto') => `<td style="padding:6px 8px;border-top:1px solid var(--bv);${warn ? 'color:var(--warn)' : ''};width:${w};white-space:nowrap">${html}</td>`;
TOPICS.push({
  id: 'list', section: 'In sync', title: 'How the differences are listed', size: 'medium', rec: 'C',
  now: 'Not shown anywhere. The Workspaces view’s git popover writes one checkout as a few lines: the branch → its upstream with ↑ and ↓, uncommitted files with +added −removed, the last commit with its time, and the path.',
  issues: [
    'With three copies and five things to compare, a full listing gets long; the user asked “how they differ”.',
  ],
  options: [
    { key: 'A', name: 'A row per copy', from: 'agentZ’s Workspaces git popover',
      desc: 'Each copy is a row: its machine and folder, then its branch → upstream with ↑↓, its uncommitted files and lines, and its stash. Values that differ from the first copy are in the warning color.',
      good: 'Complete, and in the words the Workspaces view already uses.', cost: 'You read every row to find the one difference.',
      mock: () => listFrame(remoteRows(FU)) },
    { key: 'B', name: 'A table', from: 'new',
      desc: 'Machines down the side; Branch, Commit, Remote, Changes and Stash across. A cell that differs from This Mac’s is in the warning color.',
      good: 'Differences line up in columns.', cost: 'Wide: it needs about 520 px, more than a popover usually has.',
      mock: () => listFrame(`<table style="border-collapse:collapse;width:100%;font-size:12px"><tr class="ph">${['', 'Branch', 'Commit', 'Remote', 'Changes', 'Stash'].map((h) => `<td style="padding:6px 8px">${h}</td>`).join('')}</tr>${FU.copies.map((copy) => `<tr>${tableCell(`<span class="row g1">${machineIc(copy.machine, 'xs mu')}${machineName(copy.machine)}</span>`)}${tableCell(copy.branch, copy.branch !== 'main')}${tableCell(`<span class="mono-font">${copy.commit}</span>`, copy.commit !== FU.copies[0].commit)}${tableCell(copy.upstream ? (copy.behind ? `↓${copy.behind}` : 'up to date') : 'not pushed', copy.behind || !copy.upstream)}${tableCell(copy.changes ? `${copy.changes.files} files` : '—', !!copy.changes)}${tableCell(copy.stash || '—', !!copy.stash)}</tr>`).join('')}</table>`, 210, 540) },
    { key: 'C', name: 'Only what differs, in sentences', from: 'new',
      desc: 'A sentence per difference: “Devbox 1 is on payments, not main.” “Devbox 1 has 3 commits on payments that aren’t pushed.” “Ahrorbek’s Laptop has 1 stash.” In sync, it says “Same branch and commit on This Mac, Devbox 1 and Ahrorbek’s Laptop, nothing uncommitted.” Each copy’s full state is a click on its machine away.',
      good: 'Answers “how do they differ” and nothing else.', cost: 'The sentences have to be written for each kind of difference.',
      mock: () => listFrame(`<div class="col" style="padding:8px 12px;gap:6px">${differences(FU).map((line) => `<div class="row g2" style="align-items:flex-start">${ic('alert', 'xs warnc')}<span class="sm mu">${line}</span></div>`).join('')}</div>`) },
    { key: 'D', name: 'Grouped by kind', from: 'new',
      desc: 'Headings for Branch, Commits, Uncommitted, Stash and Remotes, each naming the machines that differ there. Kinds with no difference are left out.',
      good: 'Easy to see what kind of difference there is.', cost: 'A copy’s state is split across several headings.',
      mock: () => listFrame(`<div class="col" style="padding:6px 12px 10px;gap:4px">${[['Branch', 'Devbox 1 is on payments; the others on main'], ['Commits', 'Devbox 1: 3 not pushed · Ahrorbek’s Laptop: 12 behind origin/main'], ['Uncommitted', 'Devbox 1: 1 file <span class="okc">+8</span> <span class="delc">−2</span>'], ['Stash', 'Ahrorbek’s Laptop: 1'], ['Remotes', 'Ahrorbek’s Laptop also has upstream']].map(([kind, text]) => `<div class="xs ph" style="margin-top:4px">${kind}</div><div class="sm">${text}</div>`).join('')}</div>`, 290) },
    { key: 'E', name: 'A verdict, with details folded', from: 'new',
      desc: 'One line, “Copies differ: branch, commits, uncommitted, stash”, with a chevron that unfolds A’s rows.',
      good: 'The smallest when you only want to know.', cost: 'A second click for anything useful.',
      mock: () => listFrame(`<div class="row g2 sm" style="padding:10px 12px;border-bottom:1px solid var(--bv)">${ic('chev-down', 'xs mu')}<span class="warnc">Copies differ:</span><span class="mu">branch, commits, uncommitted, stash</span></div>${remoteRows(FU)}`, 290) },
  ],
});

// 5. How it's kept current -----------------------------------------------------------------
const refreshFrame = (foot, body = remoteRows(FU), h = 250) => inFrame(panel(FU, body, { style: 'left:12px;top:12px', w: 470, foot }), 494, h);
TOPICS.push({
  id: 'refresh', section: 'In sync', title: 'How it’s kept current', size: 'medium', rec: 'D',
  now: 'Each server reads the branch of its projects’ folders, worktrees and pastures every 5 seconds and when a new one appears (`Server::refresh_git_heads`, `read_git_head`, which reads `.git/HEAD` without running git), and sends it with the projects. It looks each project’s repository up again every 15 minutes. The Workspaces view’s workspaces also get ahead/behind, uncommitted changes and the last commit (`SpaceGit`), read with git. Nothing fetches; ahead and behind are against whatever the machine fetched last.',
  issues: [
    'A copy that’s behind only shows it after something on that machine fetched.',
    'Reading git status for every project on every machine every 5 seconds costs processes on each server.',
  ],
  options: [
    { key: 'A', name: 'With the branch reads, every 5 seconds', from: 'agentZ’s branch reads',
      desc: 'Each server adds the commit, the upstream with ahead and behind, uncommitted changes and the stash count to what it reads every 5 seconds, using the reads the Workspaces view already has, and sends them with the projects. No fetch, so the remote side is as of each machine’s last fetch, and the panel says so.',
      good: 'Always there, with no request; the switcher’s mark can use it.', cost: 'git status for every project on every machine every 5 seconds, and remote state that may be days old.',
      mock: () => refreshFrame('As of each machine’s last fetch: This Mac 2h ago, Devbox 1 3d ago, Ahrorbek’s Laptop 9d ago') },
    { key: 'B', name: 'Read when shown', from: 'new',
      desc: 'Opening the place it shows asks each copy’s server for its state (a new request: git status, the commit, ahead/behind and `git stash list`), and again every 5 seconds while it’s open. Copies still answering show a spinner; an offline machine says “Devbox 1 is offline”. No fetch.',
      good: 'No cost while nobody is looking.', cost: 'A mark in the switcher (Where, option A) can’t use it; and remote state is as old as the last fetch.',
      mock: () => refreshFrame('Read just now · not fetched', remoteRows(FU).replace(/<span class="sm">[^]*?<\/span><\/div><\/div>$/, '<span class="sm row g15"><span class="spin"></span><span class="mu">Reading…</span></span></div></div>')) },
    { key: 'C', name: 'Read when shown, fetched in the background', from: 't3code',
      desc: 'B, and each server fetches the project’s remote in the background, as t3code’s server does for the status it shows (at most every 15 seconds while it’s watched, backing off after failures): here, at most every 15 minutes, with the repository lookup. A fetch that can’t sign in (no SSH agent on the machine) is skipped and said.',
      good: 'Behind means behind the remote as it is now.', cost: 'agentZ fetches in the user’s repositories without being asked, on every machine.',
      mock: () => refreshFrame(`${ic('refresh', 'xs')}Fetched 4m ago on every machine`) },
    { key: 'D', name: 'Read when shown, with a Fetch button', from: 't3code’s Pull button',
      desc: 'B, plus a Fetch button that fetches on every machine and reads again, as t3code’s git actions fetch only when asked. The footer says when each was fetched last.',
      good: 'Current when you want it, and nothing touches the repository unasked.', cost: 'One more click to trust “behind”.',
      mock: () => refreshFrame(`<span class="grow">Last fetched: This Mac 2h ago, Devbox 1 3d ago, Laptop 9d ago</span><span class="btn sm">${ic('refresh', 'xs')}Fetch</span>`) },
    { key: 'E', name: 'Fetch every time it opens', from: 'new',
      desc: 'Opening it fetches on every machine first, then reads; the rows show “Fetching…” for a second or two, longer over SSH.',
      good: 'Always against the remote as it is now.', cost: 'A wait every time, and a fetch every time someone looks.',
      mock: () => refreshFrame('Fetching on 3 machines…', FU.copies.map((copy, index) => copyRow(copy, '<span class="row g15"><span class="spin"></span><span class="mu">Fetching…</span></span>', { last: index === 2 })).join('')) },
  ],
});
