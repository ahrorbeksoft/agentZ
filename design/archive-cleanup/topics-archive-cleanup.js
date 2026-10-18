// Archived workspace cleanup: remove archived threads’ worktrees and pastures on their own,
// following t3code’s settled retention (#15208), shared worktrees (#19748) and orphans (#19749).

const AC_ARCHIVED_DESC = 'Removes the worktree or pasture of a thread archived this long ago. Its branch and conversation stay.';
const acArchivedRow = (control = acDropdown('After a week')) => acRow('Remove archived threads’ workspaces', AC_ARCHIVED_DESC, control);
const acDeletedRow = (on = true) => acRow('Remove deleted threads’ workspaces', 'Removes the worktree or pasture of a thread you delete, once no other thread uses it.', acSwitch(on));
const acMenu = (items, style) => `<div class="menu" style="${style}">${items.map(([label, on]) => `<div class="it ${on ? 'hl' : ''}"><span>${label}</span>${on ? `<span class="kb">${ic('check', 'xs')}</span>` : ''}</div>`).join('')}</div>`;

// What the pictures of Today show.
const acTodayStorage = () => `<div>${acSettings(`${acPageHead('Storage', acMachine())}${acSummary()}${acCheckoutGroup(['checkout', 'notes', 'images', 'search', 'webhooks'], { images: { hov: true } })}`, { h: 530 })}${acCaption('Settings › Storage: worktrees and pastures, deleted by hand (the trash shows on hover)')}</div>`;
const acTodayThreads = () => `<div>${acSettings(`${acPageHead('General')}${acGroup('Threads', [acRow('Auto-archive threads', 'Archives a thread nobody has used for this long, going by its last activity. Pinned threads, ones that are working or waiting for you, and ones that are open stay.', acDropdown('After 3 days'))])}`, { section: 'General', h: 300 })}${acCaption('Settings › General: Auto-archive')}</div>`;

// 1. What gets cleaned up ---------------------------------------------------------------------
const acRule = (row) => acPanel(acGroup('Cleaning up', [row]), { h: 150 });

TOPICS.push({
  id: 'rules', section: 'Rules', title: 'What gets cleaned up', size: 'medium', type: 'multi', rec: 'AB',
  now: 'agentZ removes nothing on its own; the Storage round decided “by hand only” (<code>design/storage/</code>, topic 12), and this backlog entry asks for automatic cleanup now. Archiving a thread leaves its worktree or pasture as it is, and so does deleting it; Settings › Storage lists each one with its size and state (“Its thread was archived 9 days ago”, “Its thread was deleted”) and a trash button, except those whose thread is working or that have changes. The pictures are recreated from the code.',
  nowImg: 'img/now-storage.png',
  issues: [
    'A worktree of a big project takes a gigabyte or more; archived threads’ workspaces pile up until you delete them one by one.',
    'Auto-archive already archives idle threads, so their workspaces stay behind without anyone looking.',
  ],
  options: [
    { key: 'A', name: 'Archived threads’ workspaces', from: 't3code (settled retention)', desc: 'The backlog’s rule, t3code’s “Delete settled worktrees”: a thread’s worktree or pasture is removed some time after the thread is archived (next topics for how long). Its branch and conversation stay.', good: 'Archiving, by hand or by Auto-archive, becomes what frees the space.', cost: '—', mock: () => acRule(acArchivedRow()) },
    { key: 'B', name: 'Deleted threads’ workspaces', from: 't3code', desc: 't3code’s “Delete worktrees with deleted threads”: deleting a thread removes its workspace too, once no other thread uses it. Changes keep it, as in every rule.', good: 'No leftovers named after threads that no longer exist.', cost: 'Delete… then also removes files, which its confirmation must say.', mock: () => acRule(acDeletedRow()) },
    { key: 'C', name: 'Inactive threads’ workspaces', from: 't3code', desc: 't3code’s “Delete inactive worktrees”: removed when the thread hasn’t been active for a number of days, archived or not.', good: 'Covers threads you never archive.', cost: 'Overlaps with Auto-archive plus A; a thread you mean to come back to loses its folder.', mock: () => acRule(acRow('Remove inactive threads’ workspaces', 'Removes the worktree or pasture of a thread inactive this long, archived or not. Its branch and conversation stay.', acDropdown('After 30 days'))) },
    { key: 'D', name: 'Merged worktrees', from: 't3code', desc: 't3code’s “Delete merged worktrees”: removed once the worktree’s branch is merged into the project’s default branch.', good: 'Done work goes away by itself.', cost: 'Needs the default branch fetched to know; pastures have no branch to merge unless brought back.', mock: () => acRule(acRow('Remove merged worktrees', 'Removes worktrees whose branch is merged into the project’s default branch.', acSwitch(true))) },
    { key: 'E', name: 'Workspaces with nothing of their own', from: 't3code', desc: 't3code’s “Delete unchanged worktrees”: removed when they have no commits beyond the default branch and no changes, whatever their thread’s state.', good: 'Clears threads that never changed anything.', cost: 'Also removes the workspace of a thread that is about to start work.', mock: () => acRule(acRow('Remove workspaces with nothing of their own', 'Removes worktrees and pastures with no commits beyond the default branch and no changes.', acSwitch(true))) },
  ],
});

// 2. Where the setting lives ------------------------------------------------------------------
const acCleanGroup = (rows = [acArchivedRow(), acDeletedRow()]) => acGroup('Cleaning up', rows, { note: 'Workspaces with changes, and ones a working thread or another thread still uses, are never removed.' });

TOPICS.push({
  id: 'place', section: 'Rules', title: 'Where the setting lives', size: 'wide', rec: 'A',
  now: 'Settings › General › Threads has “Auto-archive threads” (Never, After a day, After 3 days, After a week). Settings › Storage shows what each machine’s data takes, with a machine picker in its header, and lists worktrees and pastures. Each project has its own settings page under the sections.',
  nowImg: 'img/now-threads.png',
  issues: ['Each machine’s server keeps its own workspaces, so the rule runs per machine.'],
  options: [
    { key: 'A', name: 'On the Storage page', from: 't3code', desc: 't3code’s Settings › Storage: a “Cleaning up” group heads the page, above the lists, for the machine picked in its header. The note under it says what’s never removed.', good: 'Next to the sizes it changes; per machine like the rest of the page.', cost: 'Away from Auto-archive, which decides when threads get archived.', mock: () => acSettings(`${acPageHead('Storage', acMachine())}${acSummary()}${acCleanGroup()}${acCheckoutGroup(['checkout', 'notes'])}`, { h: 610 }) },
    { key: 'B', name: 'Beside Auto-archive', from: 'new', desc: 'Under “Auto-archive threads” in Settings › General › Threads: one more menu, “Remove archived threads’ workspaces”. Applies on every machine.', good: 'Archiving and what follows it, side by side.', cost: 'Far from the Storage page; one rule only fits there.', mock: () => acSettings(`${acPageHead('General')}${acGroup('Threads', [acRow('Auto-archive threads', 'Archives a thread nobody has used for this long, going by its last activity. Pinned threads, ones that are working or waiting for you, and ones that are open stay.', acDropdown('After 3 days')), acArchivedRow()])}`, { section: 'General', h: 410 }) },
    { key: 'C', name: 'Storage, with a project override', from: 't3code', desc: 'As A, and each project’s settings get “Clean up on its own”: Use each machine’s rules (t3code’s Inherit), Off (“Keep this project’s workspaces until you delete them”), or Custom (its own rules).', good: 'Keep everything for one precious project.', cost: 'Two places to look to know what happens.', mock: () => acSettings(`${acPageHead('storefront')}${acGroup('Worktrees and pastures', [acRow('Clean up on its own', 'Uses the rules on the Storage page of each machine.', acDropdown('Use each machine’s rules', true))])}`, { section: 'storefront', h: 410, over: acMenu([['Use each machine’s rules', true], ['Off', false], ['Custom', false]], 'right:40px;top:126px;min-width:200px') }) },
  ],
});

// 3. How long after archiving -----------------------------------------------------------------
TOPICS.push({
  id: 'delay', section: 'Rules', title: 'How long after archiving', size: 'medium', rec: 'B',
  now: 'Auto-archive is a menu of four choices. Settings rows put a switch or a menu on the right.',
  issues: ['Unarchiving within the delay gets the thread back with its folder; after it, the folder is made again (see “After its workspace is gone”).'],
  options: [
    { key: 'A', name: 'A number of days', from: 't3code', desc: 't3code’s retention control: a switch and, when on, a number of days with − and + (8 when first turned on). 0 days removes at the next cleanup, which runs every hour, and a thread archived less than a minute ago is skipped.', good: 'Any delay you like.', cost: 'A control no other setting here uses.', mock: () => acPanel(acGroup('Cleaning up', [acArchivedRow(acStepper(7)).replace(AC_ARCHIVED_DESC, `${AC_ARCHIVED_DESC} Set 0 days to remove them at the next cleanup.`)]), { h: 160 }) },
    { key: 'B', name: 'A menu, like Auto-archive', from: 'new', desc: 'The same kind of menu as Auto-archive: Never, Right away, After a day, After a week, After a month.', good: 'Reads like the setting beside it; covers the usual choices.', cost: 'No 3 days or 2 weeks.', mock: () => acPanel(acGroup('Cleaning up', [acArchivedRow(acDropdown('After a week', true))]), { h: 300, over: acMenu([['Never', false], ['Right away', false], ['After a day', false], ['After a week', true], ['After a month', false]], 'right:22px;top:118px;min-width:150px') }) },
    { key: 'C', name: 'Right away, or never', from: 'new', desc: 'A switch: “Remove a thread’s workspace when it’s archived”. On, archiving removes it at once (unless kept).', good: 'Simplest: archive means done.', cost: 'Unarchiving a minute later already needs the folder made again.', mock: () => acPanel(acGroup('Cleaning up', [acRow('Remove a thread’s workspace when it’s archived', 'Its branch and conversation stay.', acSwitch(true))]), { h: 150 }) },
  ],
});

// 4. What keeps a workspace -------------------------------------------------------------------
const acKeepMenu = () => `<div class="menu" style="right:22px;top:118px;width:300px">${[['Any local files', 'Includes ignored files, except node_modules.', false], ['Uncommitted changes', 'Keeps edits and untracked files. Removes ignored files, including .env.', true], ['Edited tracked files', 'Removes untracked and ignored files, including .env.', false]].map(([label, desc, on]) => `<div class="it ${on ? 'hl' : ''}" style="height:auto;padding:5px 8px;align-items:flex-start"><div class="grow"><div>${label}</div><div class="xs ph" style="white-space:normal">${desc}</div></div>${on ? `<span class="kb">${ic('check', 'xs')}</span>` : ''}</div>`).join('')}</div>`;

TOPICS.push({
  id: 'protect', section: 'What stays', title: 'What keeps a workspace', size: 'medium', rec: 'A',
  now: 'The Storage page won’t delete a workspace whose thread is working (tag “Working”) or that has changes (tag “Has changes”: “Uncommitted changes in 3 files”, or “Commits the project doesn’t have”); those rows have no trash button. Workspaces’ “Delete Worktree Checkout…” asks again with Delete Anyway when there are changes.',
  issues: [
    'The backlog says active sessions, shared worktrees and the chosen local-change protection must be respected. Active sessions (a thread working, a session or terminal open) and shared workspaces always keep it; this topic picks the local-change rule.',
    'A worktree’s commits are safe on its branch, but a pasture’s commits exist only in its folder.',
  ],
  options: [
    { key: 'A', name: 'Has changes, as Storage says', from: 'agentZ (Storage)', desc: 'The rule the Storage page already uses: kept when it has uncommitted changes (untracked files included) or commits the project doesn’t have. No setting. Ignored files such as .env don’t keep it.', good: 'One meaning of “Has changes” everywhere; protects a pasture’s own commits.', cost: 'A .env that exists only there is lost.', mock: () => acPanel(acCleanGroup([acArchivedRow()]) + acCheckoutGroup(['notes', 'images'], { images: { state: 'Commits the project doesn’t have', tag: ['Has changes', 'keep'] } }), { h: 330 }) },
    { key: 'B', name: 'A choice, as t3code has', from: 't3code', desc: 't3code’s “Keep worktrees with local changes”: Any local files (ignored ones too, except node_modules), Uncommitted changes (the default), or Edited tracked files.', good: 'Choose how careful to be, .env included.', cost: 'Three subtle choices; doesn’t look at a pasture’s commits.', mock: () => acPanel(acGroup('Cleaning up', [acArchivedRow(), acRow('Keep workspaces with local changes', 'Keeps edits and untracked files. Removes ignored files, including .env.', acDropdown('Uncommitted changes', true))]), { h: 380, over: acKeepMenu().replace('top:118px', 'top:186px') }) },
    { key: 'C', name: 'Has changes, or not pushed', from: 'new', desc: 'As A, and also kept while its branch has commits that aren’t pushed to its remote (or has no remote branch).', good: 'Work that exists only on this machine is never at risk.', cost: 'Branches that are never pushed keep their workspaces forever.', mock: () => acPanel(acCleanGroup([acArchivedRow()]) + acCheckoutGroup(['notes', 'limits'], { limits: { state: 'Its thread was archived 2 days ago · 2 commits not pushed', tag: ['Not pushed', 'keep'] } }), { h: 330 }) },
  ],
});

// 5. Workspaces several threads use -----------------------------------------------------------
TOPICS.push({
  id: 'shared', section: 'What stays', title: 'Workspaces several threads use', size: 'medium', rec: 'A',
  now: 'Several threads can work in one folder: a fork works in its source’s folder, subthreads and delegated tasks run in their thread’s, and you can start threads in a project’s checkout. The Storage page lists the folder once.',
  issues: ['One thread being archived must not take the folder from another that still works there.'],
  options: [
    { key: 'A', name: 'When all of them are archived', from: 't3code (#19748, widened)', desc: 'A shared workspace is removed only once every thread using it is archived, timed from the last one archived, and none is working.', good: 'Forks and subthreads clean up together with their thread.', cost: '—', mock: () => acPanel(acCheckoutGroup(['limits', 'webhooks'], { limits: { state: 'Its 2 threads were archived; removed in 5 days' }, webhooks: { state: 'Used by 2 threads; 1 isn’t archived', tag: ['Shared', 'info'] } }), { h: 200 }) },
    { key: 'B', name: 'Never on its own', from: 't3code', desc: 't3code’s base rule: a workspace two threads use is kept (“shared by 2 threads”); you delete it by hand.', good: 'Can’t surprise anyone.', cost: 'Every forked thread’s workspace stays forever.', mock: () => acPanel(acCheckoutGroup(['limits', 'webhooks'], { limits: { state: 'Used by 2 threads', tag: ['Shared', 'info'] }, webhooks: { state: 'Used by 2 threads', tag: ['Shared', 'info'] } }), { h: 200 }) },
    { key: 'C', name: 'A thread with its subthreads', from: 't3code (#19748)', desc: 't3code’s exception: a thread and its subthreads count as one, all idle and archived; any other sharing (a fork) keeps it.', good: 'Exactly t3code’s rule.', cost: 'Forks share folders here far more often than in t3code.', mock: () => acPanel(acCheckoutGroup(['limits', 'webhooks'], { limits: { state: 'Its thread and 2 subthreads were archived; removed in 5 days' }, webhooks: { state: 'Shared with a fork', tag: ['Shared', 'info'] } }), { h: 200 }) },
  ],
});

// 6. What it tells you ------------------------------------------------------------------------
const acResult = (name, text, size, { failed = false, kept = false } = {}) => `<div class="strow"><div class="grow" style="min-width:0"><div class="trunc">${name}</div><div class="sub2 trunc" style="${failed ? 'color:var(--del)' : ''}">${text}</div></div>${kept ? acTag('Kept', 'keep') : acTag('Removed', '')}<span class="sz">${size}</span></div>`;
const acReport = () => acGroup('Cleaning up', [
  acArchivedRow(),
  acRow('Clean up now', 'Last run 2 hours ago. 2.0 GB recovered.', '<span class="btn sm">Clean Up Now</span>'),
  `<div class="row g15" style="padding:8px 16px;border-top:1px solid var(--bv);font-size:12px;color:var(--mu)">${acChev()}Last run: 2 removed, 2 kept</div>`,
  `<div style="border-top:1px solid var(--bv);padding-left:16px">${acResult('lazy-images', 'Lazy-load the product images. Archived for 9 days', '1.1 GB')}${acResult('old-search', 'Its thread was deleted', '0.9 GB')}${acResult('release-notes', 'Write the 0.9 release notes. Has uncommitted changes (3 files)', '1.1 GB', { kept: true })}${acResult('~/.agentz/worktrees/api/tmp-4c1', 'No thread uses this worktree', '210 MB', { kept: true })}</div>`,
]);
const acToastScene = () => frame(`<div style="height:100%;background:var(--ed)"></div><div class="ac-toast" style="right:14px;bottom:14px">${ic('check-circle', 'sm')}<span>Removed 2 workspaces of archived threads, 2.0 GB</span><span class="ac" style="font-size:12px">Show</span>${ic('x', 'sm mu')}</div>`, { w: 560, h: 160, style: 'border:1px solid var(--b);border-radius:8px' });

TOPICS.push({
  id: 'report', section: 'Results', title: 'What it tells you', size: 'wide', rec: 'A',
  now: 'Nothing runs, so nothing reports. Deleting by hand on the Storage page says “Couldn’t delete {name}: {reason}” when it fails.',
  issues: ['A folder that vanished on its own should be explainable; one that stayed should say why.'],
  options: [
    { key: 'A', name: 'Last run on the Storage page', from: 't3code', desc: 't3code’s Cleanup group: “Clean Up Now”, with “Last run 2 hours ago. 2.0 GB recovered.” under it, and a fold, “Last run: 2 removed, 2 kept”, listing each folder with its thread and the reason (“Archived for 9 days”, “Has uncommitted changes (3 files)”), and the size. A failure shows in red.', good: 'Every removal and every keep is explained, in one place.', cost: 'A report to look for; nothing tells you when it ran.', mock: () => acSettings(`${acPageHead('Storage', acMachine())}${acSummary()}${acReport()}`, { h: 580 }) },
    { key: 'B', name: 'A toast when it frees space', from: 'new', desc: 'After a cleanup that removed something, a toast: “Removed 2 workspaces of archived threads, 2.0 GB”, with Show (opens Storage). Nothing when it removed nothing.', good: 'You know it happened.', cost: 'A toast you didn’t ask for, at a random moment.', mock: () => acToastScene() },
    { key: 'C', name: 'Only on the rows', from: 'new', desc: 'No report: removed folders leave the Storage list, and a kept one says why in its line (“Archived 9 days ago, kept for its changes”).', good: 'Nothing new to read.', cost: 'Nothing says what was removed, or when.', mock: () => acSettings(`${acPageHead('Storage', acMachine())}${acSummary()}${acCleanGroup([acArchivedRow()])}${acCheckoutGroup(['checkout', 'notes', 'webhooks'], { notes: { state: 'Archived 9 days ago, kept for its changes' } })}`, { h: 580 }) },
  ],
});

// 7. After its workspace is gone --------------------------------------------------------------
const acGenerating = (text) => `<div class="row g2 sm mu" style="padding:6px 22px;flex:none"><span class="spin"></span>${text}</div>`;
const acRemovedCallout = () => `<div style="padding:0 8px 8px"><div class="ac-callout"><span class="mu" style="height:18px;display:inline-flex;align-items:center">${ic('info', 'sm')}</span><div class="grow col" style="gap:6px"><div>Its pasture was removed to save space</div><div class="mu">It was archived for 9 days. Sending a message copies the project again.</div><div><span class="btn sm">Copy Now</span></div></div></div></div>`;
const acDetails = () => `<div class="ac-details" style="left:298px;top:66px"><div style="font-weight:500;color:var(--t)">Lazy-load the product images</div><div class="row g2">${ic('pasture', 'xs')}Pasture removed 2 days ago</div><div class="row g2">${ic('branch', 'xs')}lazy-images</div><div class="row g2">${ic('laptop', 'xs')}This Mac</div></div>`;

TOPICS.push({
  id: 'after', section: 'Results', title: 'After its workspace is gone', size: 'wide', rec: 'A',
  now: 'An archived thread shows “This thread is archived” with Unarchive in place of its composer. Its folder is always there, so unarchiving just goes on.',
  issues: ['A removed worktree keeps its branch; a removed pasture kept nothing of its own (changes keep it), so a fresh copy of the project loses nothing.'],
  options: [
    { key: 'A', name: 'Made again with the next message', from: 't3code', desc: 't3code: the branch and conversation stay, and the next turn makes the checkout again: a worktree on its branch, a pasture as a new copy of the project. Meanwhile the activity row says so. Nothing else marks it.', good: 'No new UI; unarchive and type.', cost: 'The first message waits for the copy, a few seconds for a big project.', mock: () => acPair(`<div>${acThread({ w: 480, above: acArchivedNotice(), composer: false })}${acCaption('Archived, its pasture removed: looks as today')}</div>`, `<div>${acThread({ w: 480, above: acGenerating('Copying the project…') })}${acCaption('After Unarchive and a message')}</div>`) },
    { key: 'B', name: 'A notice after unarchiving', from: 'new', desc: 'Unarchived, the thread says why its folder is missing: “Its pasture was removed to save space”, with Copy Now; sending a message also makes it again.', good: 'No surprise about the wait or a missing file.', cost: 'A notice for something that mends itself.', mock: () => acThread({ above: acRemovedCallout() }) },
    { key: 'C', name: 'Said in the details', from: 'new', desc: 'As A, and the thread’s details popover (hover its card in the Archived shelf) says “Pasture removed 2 days ago” in place of its folder.', good: 'Findable when you wonder, invisible otherwise.', cost: 'Only seen by hovering.', mock: () => acArchivedSidebar({ over: acDetails() }) },
  ],
});

// 8. Folders no thread uses -------------------------------------------------------------------
const AC_ORPHAN = { name: 'tmp-4c1', project: 'api', kind: 'worktree', size: '210 MB', state: 'No thread uses this worktree' };
AC_CHECKOUTS.orphan = AC_ORPHAN;

TOPICS.push({
  id: 'orphans', section: 'Results', title: 'Folders no thread uses', size: 'medium', rec: 'A',
  now: 'The Storage page lists leftover worktrees and pastures of removed projects. A folder in agentZ’s worktrees or pastures that no thread points at (a crash while creating one, a thread from an older build) isn’t listed.',
  issues: [],
  options: [
    { key: 'A', name: 'Listed, deleted by hand', from: 't3code (#19749)', desc: 't3code reports folders no thread uses, with their size, and never removes them. Here they join the Storage list: “No thread uses this worktree”, with the trash button.', good: 'Nothing unknown is ever removed by itself.', cost: 'You still have to delete them.', mock: () => acPanel(acCheckoutGroup(['images', 'orphan'], { orphan: { hov: true } }), { h: 170 }) },
    { key: 'B', name: 'Removed by the rules too', from: 'cow (gc)', desc: 'cow’s gc: folders no thread uses are removed after the same delay as archived threads’, unless they have changes.', good: 'Nothing piles up at all.', cost: 'Removes something agentZ can’t explain.', mock: () => acPanel(acCheckoutGroup(['images', 'orphan'], { orphan: { state: 'No thread uses this worktree; removed in 5 days' } }), { h: 170 }) },
    { key: 'C', name: 'Only in the cleanup report', from: 't3code', desc: 'Listed in the report’s last run (“No thread uses this worktree”), not in the Storage list.', good: 'Keeps the list to threads’ folders.', cost: 'Needs the report (topic 6, A) and isn’t deletable from there.', mock: () => acPanel(acGroup('Last run: 1 removed, 1 kept', [acResult('lazy-images', 'Lazy-load the product images. Archived for 9 days', '1.1 GB'), acResult('~/.agentz/worktrees/api/tmp-4c1', 'No thread uses this worktree', '210 MB', { kept: true })]), { h: 170 }) },
  ],
});
