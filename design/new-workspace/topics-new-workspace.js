// New worktrees and pastures for a new thread: where the choices are, what the branch starts
// from, checking out an existing branch, its name, when it's made, and the defaults.

const NOW_MENU = 'The checkout chip under a new thread’s composer (“Local”) opens Where It Works: Local checkout, New worktree, New pasture, then the project’s existing worktrees and pastures by branch. Picking New worktree at once replaces the draft with one in a new worktree (“Making a worktree…” pulses in the strip): git worktree add on a new branch, agentz/ and 8 hex digits, from whatever the project’s folder has checked out. A pasture copies the folder the same way. The branch shows at the strip’s right end, as plain text. Nothing else can be chosen.';

// 1. Where the choices are ------------------------------------------------------------------------
TOPICS.push({
  id: 'place', section: 'Choosing', title: 'Where a new worktree’s choices are', size: 'wide', rec: 'A',
  now: NOW_MENU,
  nowImg: 'img/now-checkout-menu.png',
  issues: ['A new thread’s worktree always branches from what the project has checked out; another base means checking it out in the project first.', 'The Workspaces view’s New Worktree… dialog has a From picker and a name, but a thread can’t use it.'],
  options: [
    { key: 'A', name: 'The branch becomes “From main”', from: 't3code (BranchToolbar)',
      desc: 'With New worktree or New pasture picked, the branch at the strip’s right end reads “From main” and opens a searchable list of branches to start from. The rest of the strip stays as it is.',
      good: 'One click away, where the branch already shows; t3code users know it.', cost: 'The new branch’s own name isn’t shown until the thread starts (see the name topic).',
      mock: () => newThread({ left: chipWorktree(), right: fromChip('main', { on: true }), overlay: refList({ style: 'top:142px;right:0' }), h: 450 }) },
    { key: 'B', name: 'The New Worktree dialog', from: 'agentZ’s Workspaces dialog (herdr)',
      desc: 'New worktree… and New pasture… in the checkout menu open the dialog the Workspaces view has: the branch name, From and where it’s made. Enter makes it and goes back to the draft.',
      good: 'One dialog for both places, with every choice in view.', cost: 'A dialog for every new worktree, even when the defaults are fine.',
      mock: () => worktreeDialog({ title: 'New worktree for the thread', h: 250 }) },
    { key: 'C', name: 'A submenu of branches', from: 'Zed’s context menu submenus',
      desc: 'New worktree in the checkout menu opens a submenu: From main (the default), then the other branches, and Other branch… for a search. Picking one makes it.',
      good: 'No new controls in the strip.', cost: 'A long submenu in a big repository; the base isn’t shown afterwards.',
      mock: () => newThread({ left: chipLocal(true), right: branchText('main'), h: 480, overlay: checkoutMenu({ style: 'top:142px;left:0', hl: 'worktree', worktree: `<div class="menu" style="top:186px;left:226px;min-width:200px"><div class="it hl">${ic('branch', 'xs')}<span class="grow">From main</span>${badge('default')}</div><div class="hr"></div><div class="it">${ic('branch', 'xs')}<span class="grow">checkout-flow</span></div><div class="it">${ic('branch', 'xs')}<span class="grow">fix-login</span></div><div class="it">${ic('branch', 'xs')}<span class="grow">redesign-cart</span></div><div class="hr"></div><div class="it">${ic('search', 'xs')}<span class="grow">Other branch…</span></div></div>` }) }) },
    { key: 'D', name: 'Two chips: base and name', from: 'new',
      desc: 'With New worktree picked, the strip gets two chips after it: “From main” (the branch list) and the new branch’s name, which turns into a field when clicked.',
      good: 'Base and name both in view before sending.', cost: 'A crowded strip next to the machine and account chips.',
      mock: () => newThread({ left: `${chipWorktree()}${fromChip('main')}${chip('pencil', 'agentz/green-valley-86fb', { chevron: false, style: 'border:1px solid var(--bv)' })}`, right: '', h: 300 }) },
  ],
});

// 2. The default base --------------------------------------------------------------------------
TOPICS.push({
  id: 'base', section: 'Choosing', title: 'What a new worktree starts from by default', size: 'wide', rec: 'B',
  now: 'What the project’s folder has checked out (its branch, or its commit when it’s detached).',
  nowImg: 'img/now-worktree-chosen.png',
  issues: ['A project left on a feature branch makes every new worktree branch from that feature.'],
  options: [
    { key: 'A', name: 'What the project has checked out', from: 'today',
      desc: 'As now. The list starts on the project folder’s branch.',
      good: 'Follows what the user is doing in the project.', cost: 'Easy to branch from the wrong place.',
      mock: () => newThread({ left: chipWorktree(), right: fromChip('checkout-flow'), h: 260 }) },
    { key: 'B', name: 'The repository’s default branch', from: 't3code',
      desc: 'The branch origin/HEAD points at (main), else the checked-out one when there’s no remote, as t3code’s worktree base.',
      good: 'New work starts from the trunk, wherever the project is.', cost: 'Stacked work has to pick its parent branch each time.',
      mock: () => newThread({ left: chipWorktree(), right: fromChip('main'), h: 260 }) },
    { key: 'C', name: 'The last base picked in this project', from: 't3code (the next draft keeps its base)',
      desc: 'The project remembers the last branch a worktree started from, on each machine; the first time it’s the default branch.',
      good: 'A run of threads on one feature branch needs one pick.', cost: 'The remembered base can be stale or deleted.',
      mock: () => newThread({ left: chipWorktree(), right: fromChip('redesign-cart'), h: 260 }) },
  ],
});

// 3. The branch list ---------------------------------------------------------------------------
TOPICS.push({
  id: 'list', section: 'Choosing', title: 'What the branch list shows', size: 'medium', type: 'multi', rec: 'ABC',
  now: 'Only the Workspaces view’s New Worktree… dialog lists branches: its From menu has the local branches, without search or marks.',
  nowImg: 'img/now-new-worktree-from.png',
  options: [
    { key: 'A', name: 'Search', from: 't3code (“Search refs…”)',
      desc: 'A field at the top filters the list as you type.',
      good: 'Fast in a repository with many branches.', cost: 'None to speak of.',
      mock: () => frame(refList({ style: 'top:16px;left:16px', query: 'fix', rows: [BRANCHES[2], REMOTES[2]].map((ref, index) => refRow(ref, { hl: index === 0 })).join('') }), { w: 340, h: 160 }) },
    { key: 'B', name: 'Marks for each branch', from: 't3code (BranchPickerRefItem)',
      desc: 'Faint words at the row’s end: current (the project folder’s), default, worktree (checked out in another worktree or pasture), remote.',
      good: 'Says which branch is which without opening anything.', cost: 'Small text to read.',
      mock: () => frame(refList({ style: 'top:16px;left:16px' }), { w: 340, h: 230 }) },
    { key: 'C', name: 'Remote branches too', from: 't3code',
      desc: 'origin’s branches that have no local branch follow the local ones, so a teammate’s branch can be a base without fetching it by hand.',
      good: 'Start from what’s pushed.', cost: 'Only as fresh as the last fetch (see Start from origin).',
      mock: () => frame(refList({ style: 'top:16px;left:16px', rows: `${BRANCHES.slice(0, 3).map((ref, index) => refRow(ref, { check: index === 0 })).join('')}<div class="hr"></div>${REMOTES.slice(1, 2).map((ref) => refRow(ref)).join('')}` }), { w: 340, h: 210 }) },
    { key: 'D', name: 'Any tag or commit typed', from: 'new (the server already takes one)',
      desc: 'Typing a tag or a commit hash that isn’t a branch offers “Start from v1.4.0”.',
      good: 'Reproduce a bug from a release.', cost: 'Rarely needed.',
      mock: () => frame(refList({ style: 'top:16px;left:16px', query: 'v1.4.0', rows: `<div class="it hl">${ic('hash', 'xs')}<span class="grow">Start from v1.4.0</span>${badge('tag')}</div>` }), { w: 340, h: 110 }) },
  ],
});

// 4. Start from origin -----------------------------------------------------------------------
TOPICS.push({
  id: 'origin', section: 'Choosing', title: 'Starting from what’s on origin', size: 'wide', rec: 'A',
  now: 'Nothing fetches. A worktree starts from the local branch, however far behind origin it is.',
  issues: ['A project folder not pulled for a week gives new threads a week-old main.'],
  options: [
    { key: 'A', name: 'A Start from origin switch', from: 't3code (Start from origin)',
      desc: 'A switch at the bottom of the branch list: on, the server fetches the branch from origin first and starts from origin/main; a branch origin doesn’t have starts from the local one. The switch is remembered, and its default is in Settings.',
      good: 'Fresh bases without touching the project’s checkout.', cost: 'A fetch before each new worktree takes a few seconds.',
      mock: () => newThread({ left: chipWorktree(), right: fromChip('main', { on: true, origin: true }), overlay: refList({ style: 'top:142px;right:0', foot: originFoot(true) }), h: 480 }) },
    { key: 'B', name: 'A fetch button in the list', from: 'new',
      desc: 'A Fetch button beside the search updates the remote branches; picking origin/main (with Remote branches too) starts from it.',
      good: 'Fetches only when asked.', cost: 'Two steps, and origin/main and main look alike.',
      mock: () => frame(refList({ style: 'top:16px;left:16px', head: `<div class="row g2 sm" style="height:30px;padding:0 10px;border-bottom:1px solid var(--bv);color:var(--mu)"><span class="grow">Start from</span><span class="btn sm">${ic('restart', 'xs')}Fetch</span></div>`, rows: `${refRow(BRANCHES[0], { check: true })}<div class="hr"></div>${refRow(REMOTES[0])}${refRow(REMOTES[1])}` }), { w: 340, h: 230 }) },
    { key: 'C', name: 'Leave it out', from: 'today',
      desc: 'Worktrees start from local branches; the user pulls in the project first.',
      good: 'Nothing to learn, no network.', cost: 'Stale bases stay easy.',
      mock: () => newThread({ left: chipWorktree(), right: fromChip('main'), h: 260 }) },
  ],
});

// 5. An existing branch -------------------------------------------------------------------------
TOPICS.push({
  id: 'existing', section: 'Choosing', title: 'Checking out an existing branch', size: 'wide', rec: 'A',
  now: 'A new worktree always gets a new branch. An existing branch can only be worked on in a worktree that already has it (the menu’s Existing list), or in the project’s folder. The Workspaces dialog refuses a name that exists: “the branch fix-login already exists; choose another name”.',
  nowImg: 'img/now-branch-exists.png',
  issues: ['Picking up a branch someone pushed, or one left from earlier work, means checking it out in the project’s own folder.'],
  options: [
    { key: 'A', name: 'Check Out beside each branch', from: 't3code’s list, with git worktree add on an existing branch',
      desc: 'Clicking a branch starts from it on a new branch, as now. Hovering a branch shows Check Out at its end: the worktree is made on that branch itself, no new branch. A branch with a worktree already says so and opens it (t3code reuses that worktree).',
      good: 'Both in one list; the click everyone makes stays the same.', cost: 'A hover button is easy to miss.',
      mock: () => newThread({ left: chipWorktree(), right: fromChip('main', { on: true }), h: 470, overlay: refList({ style: 'top:142px;right:0', rows: [BRANCHES[0], BRANCHES[1], BRANCHES[2], BRANCHES[3]].map((ref, index) => refRow(ref, { check: index === 0, hl: index === 2, action: index === 2 ? '<span class="btn sm" style="margin-left:6px">Check Out</span>' : index === 1 ? '<span class="btn sm ghost" style="margin-left:6px">Open</span>' : '' })).join(''), foot: hintFoot('Click starts a new branch from it; Check Out works on the branch itself.') }) }) },
    { key: 'B', name: 'New branch or existing, at the top', from: 'new',
      desc: 'Two tabs over the list: New branch (pick what it starts from) and Existing branch (pick the branch to work on). Branches with a worktree are greyed in Existing, with the worktree’s name.',
      good: 'Says plainly which one you’re doing.', cost: 'One more choice before every pick.',
      mock: () => newThread({ left: chipWorktree(), right: chip('branch', 'fix-login', { on: true }), h: 470, overlay: refList({ style: 'top:142px;right:0', head: `<div style="padding:6px 8px;border-bottom:1px solid var(--bv)"><span class="seg"><span>New branch</span><span class="on">Existing branch</span></span></div>`, rows: [BRANCHES[0], BRANCHES[1], BRANCHES[2], BRANCHES[3]].map((ref, index) => refRow(ref, { check: index === 2, dim: index < 2 })).join('') }) }) },
    { key: 'C', name: 'The name decides', from: 'new, from the Workspaces dialog’s error',
      desc: 'In the dialog (or a name field), typing the name of a branch that exists checks it out instead of failing: “Checks out the existing branch fix-login”. A new name makes a new branch from From.',
      good: 'No extra control; today’s error becomes the feature.', cost: 'A typo in an existing name silently makes a new branch.',
      mock: () => worktreeDialog({ name: 'fix-login', where: 'fix-login', note: `${ic('branch', 'xs')} Checks out the existing branch <b style="color:var(--t)">fix-login</b>; From doesn’t apply.`, h: 270 }) },
    { key: 'D', name: 'Only through existing worktrees', from: 't3code (Previous worktree, reuse)',
      desc: 'As now: an existing branch is worked on in the worktree that has it, from the Existing list. A new worktree is always a new branch.',
      good: 'Nothing new.', cost: 'A pushed branch still needs the project’s folder.',
      mock: () => newThread({ left: chipLocal(true), h: 460, overlay: checkoutMenu({ style: 'top:142px;left:0' }) }) },
  ],
});

// 6. The branch name ------------------------------------------------------------------------------
TOPICS.push({
  id: 'name', section: 'Choosing', title: 'The new branch’s name', size: 'wide', rec: 'C',
  now: 'agentz/ and 8 hex digits (agentz/3f671a7e) for a thread’s worktree; herdr’s words (agentz/green-valley-86fb) in the Workspaces dialog, where it can be edited. A thread’s branch can’t be named before or after.',
  nowImg: 'img/now-worktree-chosen.png',
  options: [
    { key: 'A', name: 'agentz/ and an id', from: 'today',
      desc: 'As now: agentz/3f671a7e.',
      good: 'Never collides.', cost: 'Says nothing in git branch or a pull request.',
      mock: () => newThread({ left: chipWorktree(), right: branchText('agentz/3f671a7e'), h: 260 }) },
    { key: 'B', name: 'Words, as the Workspaces dialog', from: 'herdr',
      desc: 'agentz/green-valley-86fb, the same generator in both places.',
      good: 'Easier to say and tell apart.', cost: 'Still says nothing about the work.',
      mock: () => newThread({ left: chipWorktree(), right: branchText('agentz/green-valley-86fb'), h: 260 }) },
    { key: 'C', name: 'Named after the thread', from: 't3code (Worktree branch naming)',
      desc: 'It starts with a temporary name; once the thread has a title (the agent’s, or the title generator’s), the branch is renamed to it: agentz/fix-checkout-rounding. A taken name keeps the temporary one.',
      good: 'Branches read like the work in them.', cost: 'The branch’s name changes once, after the first reply.',
      mock: () => threadStart(`${userBubble('The checkout total rounds twice. Fix it so the total is rounded once.')}<div class="row g2 sm mu" style="margin-top:auto">${ic('worktree', 'xs')}Worktree<span class="grow"></span>${ic('branch', 'xs')}<s class="ph">agentz/3f671a7e</s> ${ic('arrow', 'xs')} <span style="color:var(--t)">agentz/fix-checkout-rounding</span></div>`, { h: 180 }) },
    { key: 'D', name: 'Typed before sending', from: 'new',
      desc: 'Clicking the branch at the strip’s end turns it into a field; Enter keeps the name.',
      good: 'Exactly the name you want.', cost: 'Most people won’t bother, and get the id.',
      mock: () => newThread({ left: chipWorktree(), right: `<span class="field focus" style="height:22px;width:220px;font-size:12px;padding:0 6px">${ic('branch', 'xs')}fix-checkout-rounding</span>`, h: 260 }) },
  ],
});

// 7. Pull requests ------------------------------------------------------------------------------
TOPICS.push({
  id: 'pr', section: 'Choosing', title: 'Starting from a pull request', size: 'medium', rec: 'B',
  now: 'agentZ has no GitHub or GitLab integration; a pull request’s branch has to be fetched by hand.',
  options: [
    { key: 'A', name: 'Type its number or link', from: 't3code (Checkout Pull Request)',
      desc: 'Typing #42 or a pull request’s link in the branch search offers “Check out pull request #42”, which fetches its branch (git fetch origin pull/42/head, GitHub only) and makes the worktree on it.',
      good: 'Review or fix a pull request in its own worktree in one step.', cost: 'Works only with GitHub-style refs; no title or state without the GitHub API.',
      mock: () => frame(refList({ style: 'top:16px;left:16px', query: '#42', rows: `<div class="it hl" style="height:40px">${ic('merge', 'sm')}<span class="col grow"><span>Check out pull request</span><span class="xs ph">#42 from origin</span></span></div>` }), { w: 340, h: 130 }) },
    { key: 'B', name: 'Leave it out', from: 'today',
      desc: 'Not part of this change; a pushed pull request branch shows as a remote branch (with Remote branches too) and can be checked out from there.',
      good: 'No source control integration to build.', cost: 'Finding the branch by name, not number.',
      mock: () => frame(refList({ style: 'top:16px;left:16px', query: 'pay', rows: refRow(REMOTES[1], { hl: true }) }), { w: 340, h: 110 }) },
  ],
});

// 8. When it's made ---------------------------------------------------------------------------------
TOPICS.push({
  id: 'when', section: 'Making it', title: 'When the worktree is made', size: 'wide', rec: 'B',
  now: 'At once, when New worktree is picked: the draft is replaced by one whose agent starts in the new worktree. “Making a worktree…” pulses in the strip meanwhile. Switching back to Local leaves the worktree behind until drafts are swept.',
  nowImg: 'img/now-worktree-chosen.png',
  issues: ['With a base and a name to choose, each change would make and remove a worktree.'],
  options: [
    { key: 'A', name: 'At once, remade on each change', from: 'today',
      desc: 'As now; picking another base or name removes the draft’s worktree and makes a new one.',
      good: 'The agent is ready in the worktree before you send.', cost: 'Worktrees made and thrown away while choosing; a slow fetch or submodules block the screen.',
      mock: () => newThread({ left: '<span class="ph" style="opacity:.7">Making a worktree from main…</span>', right: '', h: 260 }) },
    { key: 'B', name: 'When the first message is sent', from: 't3code (thread setup)',
      desc: 'The strip only records the choice (“New worktree · From main”). Sending makes it, and the thread shows its steps above the reply: fetch (with Start from origin), check out, submodules; then the agent starts there. A failure shows with Retry and Use Local.',
      good: 'Nothing is made until it’s needed; slow steps show what they’re doing.', cost: 'The first reply waits for the worktree; the draft’s agent restarts in it (as a handoff does).',
      mock: () => threadStart(`${userBubble('The checkout total rounds twice. Fix it so the total is rounded once.')}<div class="card col" style="padding:10px 12px;gap:2px;width:360px"><div class="row g2 sm" style="margin-bottom:4px">${ic('worktree', 'xs')}<span class="b5">New worktree</span><span class="ph">from origin/main</span></div>${stepRow('done', 'Fetched main from origin', '0.8s')}${stepRow('run', 'Checking out', '1,204 files')}${stepRow('wait', 'Submodules')}</div>`, { h: 230 }) },
  ],
});

// 9. Pastures -------------------------------------------------------------------------------------
TOPICS.push({
  id: 'pasture', section: 'Making it', title: 'A new pasture’s choices', size: 'wide', rec: 'B',
  now: 'A pasture copies the project’s whole folder (uncommitted and ignored files too), then makes its branch from what’s checked out (cow’s create). It gets the same kind of name, agentz/ and an id.',
  options: [
    { key: 'A', name: 'The same choices as a worktree', from: 'new',
      desc: 'From, Start from origin, Check Out and the name apply to pastures too; the copy switches to the base after copying, and uncommitted changes that don’t fit it are left out.',
      good: 'One way to pick for both.', cost: 'A copy that drops the changes it was made to keep defeats the point of a pasture.',
      mock: () => newThread({ left: chipPasture(), right: fromChip('redesign-cart'), h: 260 }) },
    { key: 'B', name: 'A copy of the folder as it is', from: 'cow',
      desc: 'A pasture always starts from the project’s current state, uncommitted changes included, on a new branch; only the name follows the name topic. The strip says “Copies the folder as it is”.',
      good: 'Keeps what pastures are for: carrying unfinished work into a thread.', cost: 'Another base means a worktree, or checking it out in the project first.',
      mock: () => newThread({ left: chipPasture(), right: '<span class="ph">Copies the folder as it is, on checkout-flow</span>', h: 260 }) },
  ],
});

// 10. Settings ------------------------------------------------------------------------------------
TOPICS.push({
  id: 'defaults', section: 'Making it', title: 'Defaults in Settings', size: 'wide', type: 'multi', rec: 'AB',
  now: 'There are none: new threads start in the project’s folder (an open question in the architecture asks whether they should default to a worktree or pasture), worktrees branch from the checked-out branch, and submodules are always fetched recursively.',
  options: [
    { key: 'A', name: 'Where new threads work', from: 't3code (New threads)',
      desc: 'In Project Settings, and as a default for all projects: Local, New worktree or New pasture.',
      good: 'Answers the open question per project.', cost: 'A thread can start somewhere the user didn’t expect.',
      mock: () => frame(`<div class="col" style="padding:18px 20px;gap:4px"><div class="b5">New threads</div><div class="row" style="padding:10px 0;border-bottom:1px solid var(--bv)"><span class="col grow"><span>Where they work</span><span class="sm ph">The checkout chip under the composer starts here.</span></span><span class="seg"><span>Local</span><span class="on">New worktree</span><span>New pasture</span></span></div></div>`, { w: 560, h: 110 }) },
    { key: 'B', name: 'Start from origin', from: 't3code (Start from origin)',
      desc: 'Whether the branch list’s switch starts on.',
      good: 'Set once for people who always want origin.', cost: 'Only matters with Start from origin.',
      mock: () => frame(`<div class="col" style="padding:18px 20px"><div class="row" style="padding:10px 0"><span class="col grow"><span>Start from origin</span><span class="sm ph">New worktrees fetch their base and start from origin’s.</span></span>${toggle(true)}</div></div>`, { w: 560, h: 90 }) },
    { key: 'C', name: 'Branch prefix', from: 't3code (static prefix)',
      desc: 'What branch names start with: agentz/ by default, empty for none.',
      good: 'Fits a team’s branch rules (feature/, the user’s initials).', cost: 'One more setting.',
      mock: () => frame(`<div class="col" style="padding:18px 20px"><div class="row" style="padding:10px 0"><span class="col grow"><span>Branch prefix</span><span class="sm ph">New worktrees’ and pastures’ branches start with it.</span></span><span class="field" style="width:140px;height:28px">agentz/</span></div></div>`, { w: 560, h: 90 }) },
    { key: 'D', name: 'Submodules', from: 't3code (Submodules)',
      desc: 'Recursive (today), Top level only, or Skip, for repositories whose submodules are slow.',
      good: 'Fast worktrees in big repositories.', cost: 'Rarely needed.',
      mock: () => frame(`<div class="col" style="padding:18px 20px"><div class="row" style="padding:10px 0"><span class="col grow"><span>Submodules</span><span class="sm ph">Fetched in each new worktree.</span></span><span class="row g1" style="border:1px solid var(--b);border-radius:6px;padding:3px 8px;font-size:13px">Recursive${chev}</span></div></div>`, { w: 560, h: 90 }) },
  ],
});

// 11. The Workspaces dialog -------------------------------------------------------------------------
TOPICS.push({
  id: 'dialog', section: 'Making it', title: 'The Workspaces view’s New Worktree dialog', size: 'medium', rec: 'A',
  now: 'A workspace row’s New Worktree… has the branch name (herdr’s words), From (a plain list of local branches), where it’s made, and Worktree or Pasture.',
  nowImg: 'img/now-new-worktree-dialog.png',
  options: [
    { key: 'A', name: 'The same branch list', from: 't3code’s list, in herdr’s dialog',
      desc: 'Its From opens the branch list picked above (search, marks, remote branches, Start from origin), and an existing branch’s name is handled as picked above.',
      good: 'One way to choose a base everywhere.', cost: 'None to speak of.',
      mock: () => worktreeDialog({ h: 400, overlay: refList({ style: 'top:76px;left:56px', foot: originFoot(false) }) }) },
    { key: 'B', name: 'As it is', from: 'today',
      desc: 'The dialog keeps its plain From menu.',
      good: 'Nothing changes there.', cost: 'Two ways to pick a base.',
      mock: () => worktreeDialog({}) },
  ],
});
