// Session notices and context compaction, as Zed has them since ACP SDK 3.3 (zed#65414).

const snWide = (options = {}) => snScene(options);
const snDock = ({ above = '', w = 560, h = 250, composer = snComposer(), overlays = '', body = snConversation([SN_TURNS.secondWork, snGenerating()]) } = {}) =>
  snScene({ w, h, header: '', body, above, composer, overlays });

// What the pictures of Today show.
const snTodayNotices = () => `<div>${snScene({ above: snLimitNotice() })}${snCaption('The only notices above the composer are agentZ’s own, such as the usage-limit notice')}</div>`;
const snTodayCompaction = () => `<div>${snScene({
  body: snConversation([SN_TURNS.first, SN_TURNS.firstWork, SN_TURNS.firstReply, snUser('/compact'), SN_TURNS.second, SN_TURNS.secondWork, snGenerating()]),
  composer: snComposer({ ring: 0.16 }),
  overlays: snTip('<div class="sm mu">Context</div><div>16% <span style="color:rgba(135,138,152,.6)">•</span> 31k <span style="color:rgba(135,138,152,.6)">/</span> <span class="mu">200k</span></div>', 'right:96px;bottom:50px'),
})}${snCaption('After the agent’s /compact: no mark in the conversation, only the context ring is lower')}</div>`;

// 1. Where a notice shows ---------------------------------------------------------------------
const snInlineNotice = (notice) => `<div class="sn-line" style="background:${SN_SEVERITY[notice.sev].bg};border:1px solid var(--bv)">${snSevIcon(notice.sev, 'xs')}<span style="color:var(--t)">${notice.title}</span>${notice.desc ? `<span class="trunc">· ${notice.desc}</span>` : ''}<span class="grow"></span>${ic('x', 'xs')}</div>`;
const snToast = (notice, style) => `<div class="sn-toast" style="${style}"><span class="ic" style="height:18px;display:inline-flex;align-items:center">${snSevIcon(notice.sev)}</span><div class="grow"><div>${notice.title}</div>${notice.desc ? `<div class="mu">${notice.desc}</div>` : ''}</div>${snX()}</div>`;
const snBell = (on = false) => `<span class="ibtn ${on ? 'on' : ''}" style="color:var(--warn)">${ic('warning', 'sm')}<span style="position:absolute;top:-3px;right:-4px;min-width:13px;height:13px;border-radius:7px;background:var(--warn);color:#1b1f26;font-size:9px;font-weight:600;display:grid;place-items:center;padding:0 3px">2</span></span>`;
const snNoticeMenu = (style) => `<div class="menu" style="${style};width:330px;padding:0;overflow:hidden"><div class="row" style="height:30px;padding:0 10px;font-size:12px;color:var(--mu);border-bottom:1px solid var(--bv)"><span class="grow">Notices</span><span class="btn ghost sm" style="padding:0 4px">Dismiss All</span></div>${snStack(['indexing', 'mcp'])}</div>`;
const snStatusStrip = (notice, more = '') => `<div class="row g15" style="height:26px;flex:none;padding:0 12px;font-size:12px;color:var(--mu);background:var(--panel);border-top:1px solid var(--bv)">${snSevIcon(notice.sev, 'xs')}<span style="color:var(--t)">${notice.title}</span><span class="trunc">· ${notice.desc}</span><span class="grow"></span>${more ? `<span class="chip" style="height:18px">${more}</span>` : ''}${ic('x', 'xs')}</div>`;

TOPICS.push({
  id: 'place', section: 'Notices', title: 'Where a notice shows', size: 'wide', rec: 'A',
  now: 'agentZ doesn’t tell agents it takes notices (its session capabilities list only config options) and drops any session update it doesn’t know, without logging it. Above the composer it already stacks its own callouts: request elicitations, the usage-limit notice, the login notice, “Conversation not loaded”, errors, then the activity bar. Each is a Callout: a rule along its top, the severity’s icon, a small title, a muted description and ×. The picture is recreated from the code.',
  nowImg: 'img/now-notices.png',
  issues: [
    'An agent can’t say that an MCP server is unavailable, that the service is slow, or that maintenance is coming, except in its reply.',
    'Notices are live only: they aren’t saved and don’t come back when the thread loads again, so the place must work for something that disappears.',
  ],
  options: [
    {
      key: 'A', name: 'Callouts above the composer', from: 'Zed',
      desc: 'Zed’s SessionNotice: each notice is a Callout above the composer, after agentZ’s other notices and before errors. Info has the muted ⓘ, Warning ⚠, Error ⊗; another severity (such as “maintenance”) looks like Info without an icon. Title and description are plain text. × (“Dismiss Notice”) removes it and puts focus back in the thread; nothing is sent to the agent.',
      good: 'The same look and place as agentZ’s usage-limit and error notices. Nothing new to learn.',
      cost: 'A long-lived notice keeps the composer pushed up until it’s dismissed.',
      mock: () => snWide({ above: snStack(['indexing', 'mcp']) }),
    },
    {
      key: 'B', name: 'A line in the conversation', from: 'new',
      desc: 'A slim line in the conversation where the notice arrived: the severity icon, the title and the description on one line, × at the end. It scrolls away with the conversation; since notices aren’t saved, it is gone when the thread loads again.',
      good: 'Says when it happened, between which messages. The composer never moves.',
      cost: 'Scrolls out of sight while the agent keeps working. A line that vanishes on reload looks like part of the history but isn’t.',
      mock: () => snWide({ body: snDefaultBody([snInlineNotice(SN_NOTICES.indexing), snInlineNotice(SN_NOTICES.mcp)]) }),
    },
    {
      key: 'C', name: 'Toasts at the top of the thread', from: 'new',
      desc: 'A toast in the thread’s top right corner, under the header. Info leaves on its own after 8 seconds; warnings and errors stay until ×. Several stack downward.',
      good: 'Seen at once wherever the conversation is scrolled. Nothing in the thread moves.',
      cost: 'Covers the conversation’s top right. Info that leaves on its own can be missed when you look away.',
      mock: () => snWide({ overlays: snToast(SN_NOTICES.indexing, 'right:10px;top:46px') + snToast(SN_NOTICES.mcp, 'right:10px;top:92px') }),
    },
    {
      key: 'D', name: 'A button in the header', from: 'new',
      desc: 'While a thread has notices, a ⚠ (or ⓘ, by the highest severity) with a count sits before the changes button. Its menu lists them as callouts, with Dismiss All. Nothing shows in the thread.',
      good: 'Takes no room in the thread, however many notices come.',
      cost: 'Easy to miss: a notice that the service is degraded only shows as a small icon.',
      mock: () => snWide({ header: snHeader({ extra: snBell(true) }), overlays: snNoticeMenu('right:100px;top:38px') }),
    },
    {
      key: 'E', name: 'One line under the composer', from: 'new',
      desc: 'The newest notice as one line along the bottom, under the composer: icon, title, description cut to fit, a count of the others (“+1”) that opens them all, and ×.',
      good: 'One fixed line, whatever comes. Near where you type, out of the conversation.',
      cost: 'A long description is cut off. A new kind of bar the app has nowhere else.',
      mock: () => snWide({ overlays: '', composer: snComposer() + snStatusStrip(SN_NOTICES.mcp, '+1') }),
    },
  ],
});

// 2. Several notices and how long they stay -------------------------------------------------
const snScrollStack = (keys) => `<div style="max-height:150px;overflow:hidden;position:relative">${snStack(keys)}<span style="position:absolute;right:1px;top:2px;width:4px;height:70px;border-radius:2px;background:rgba(169,175,188,.35)"></span></div>`;
const snPager = () => `<span class="row g1 xs mu" style="margin-right:4px"><span class="sn-x">${ic('chev-left', 'xs')}</span>1 of 3<span class="sn-x">${ic('chev-right', 'xs')}</span></span>`;
ICONS['chev-left'] = '<path d="m15 18-6-6 6-6"/>';

TOPICS.push({
  id: 'several', section: 'Notices', title: 'Several notices, and how long they stay', size: 'medium', rec: 'A',
  now: 'There are no notices from agents. agentZ’s own notices each show once (one limit notice, one error), so stacking never comes up.',
  issues: [
    'An agent can send a notice per retry or per failing server, so they can pile up.',
    'A notice has no id, so a repeat of the same notice is a new one.',
  ],
  options: [
    {
      key: 'A', name: 'Each one, until dismissed', from: 'Zed',
      desc: 'Zed: every notice is its own callout, oldest first, repeats included. The list is at most 192 px tall and scrolls beyond that. Each stays until ×. A thread in the background keeps its notices until you open it.',
      good: 'Exactly what the agent sent. Nothing disappears behind your back.',
      cost: 'A chatty agent fills 192 px with repeats.',
      mock: () => snDock({ above: snScrollStack(['indexing', 'mcp', 'network', 'failed']) }),
    },
    {
      key: 'B', name: 'Repeats merge with a count', from: 'new',
      desc: 'As A, but a notice with the same severity, title and description as one still shown merges into it, and its title gets a count (“×3”). × dismisses all of them.',
      good: 'Retries don’t fill the stack.',
      cost: 'Hides that the repeats came at different times.',
      mock: () => snDock({ above: snCallout(SN_NOTICES.mcp, { count: ' <span class="mu">×3</span>' }) + snCallout(SN_NOTICES.failed) }),
    },
    {
      key: 'C', name: 'Only the newest, with a pager', from: 'new',
      desc: 'One callout: the newest notice, with “1 of 3” and arrows to go through the others. × dismisses the one shown; Dismiss All sits in the pager’s menu.',
      good: 'Never more than one callout tall.',
      cost: 'An error can sit behind a newer info notice.',
      mock: () => snDock({ above: snCallout(SN_NOTICES.failed, { actions: snPager() }) }),
    },
    {
      key: 'D', name: 'Info leaves on its own', from: 'new',
      desc: 'As A, but an info notice leaves by itself after 10 seconds, or when your next turn starts if you weren’t looking. Warnings, errors and other severities stay until ×.',
      good: 'Passing news (“Indexing resumed”) cleans itself up; what matters stays.',
      cost: 'Behavior Zed doesn’t have; a notice can go before you read it.',
      mock: () => snPair(
        `<div>${snDock({ w: 420, above: snStack(['indexing', 'mcp']) })}${snCaption('When they arrive')}</div>`,
        `<div>${snDock({ w: 420, above: snStack(['mcp']) })}${snCaption('10 seconds later')}</div>`,
      ),
    },
  ],
});

// 3. A notice in a thread you aren't looking at ----------------------------------------------
const snCards = (marks = {}) => SN_THREADS.map((thread) => snCard(thread, marks[thread.title] || {}));
const snCardMark = (sev) => `<span style="display:inline-flex;color:${SN_SEVERITY[sev].color}">${ic(SN_SEVERITY[sev].icon, 'sm')}</span>`;
const snNoticePill = (sev, label = 'Notice') => `<span class="sn-badge" style="color:${SN_SEVERITY[sev].color}">${ic(SN_SEVERITY[sev].icon, 'xs')}${label}</span>`;
const snSystem = (style) => `<div style="position:absolute;z-index:30;${style};width:300px;border-radius:12px;background:rgba(58,62,70,.96);border:1px solid #555b66;box-shadow:0 10px 30px rgba(0,0,0,.5);padding:10px 12px;font:12px/1.35 -apple-system,'IBM Plex Sans',sans-serif;color:#eef0f3"><div class="row g2"><span style="width:20px;height:20px;border-radius:5px;background:#1b1f26;display:grid;place-items:center;font-weight:700;font-size:11px;color:var(--ac)">Z</span><b style="font-weight:600">agentZ</b><span class="grow"></span><span style="color:#a7adb8">now</span></div><div style="margin-top:6px;font-weight:600">Write tests for the rate limiter</div><div style="color:#cfd3da">Optional integration failed. Work will continue without this integration.</div></div>`;

TOPICS.push({
  id: 'background', section: 'Notices', title: 'A notice in a thread you aren’t looking at', size: 'medium', rec: 'A',
  now: 'A thread card shows its project, title, branch, machine and agent, and its status: Working, Pending Approval, Awaiting Input, Completed, or the time. Only those states ask for attention (and their sounds and notifications).',
  issues: ['Notices aren’t saved, so one that arrives while you’re elsewhere is only seen if it’s still there when you open the thread.'],
  options: [
    {
      key: 'A', name: 'Nothing outside the thread', from: 'Zed',
      desc: 'Zed: a thread in the background keeps its notices and shows them when you open it. The card doesn’t change.',
      good: 'Notices stay what the protocol says they are: information, not a request for you.',
      cost: 'An error notice in a working thread waits until you happen to open it.',
      mock: () => snSidebar(snCards()),
    },
    {
      key: 'B', name: 'An icon on the card', from: 'new',
      desc: 'While a thread has a warning or error notice, its card shows that severity’s icon before the machine icon; hovering it shows the title. Info and other severities add nothing.',
      good: 'You see which thread has a problem without opening it, and the status stays as it is.',
      cost: 'One more mark on a card that already has several.',
      mock: () => snSidebar(snCards({ 'Write tests for the rate limiter': { mark: snCardMark('error') }, 'Bump the API version': { mark: snCardMark('warning') } }), { over: `<div class="sn-tip" style="left:96px;top:200px;font-size:12px">Optional integration failed</div>` }),
    },
    {
      key: 'C', name: 'Errors take the status slot', from: 'new',
      desc: 'An error notice replaces the card’s time or Completed with “Notice” in red (warnings in yellow) until you open the thread, as the attention states do. Working and requests for you still win.',
      good: 'Treats an error like the other things that want you.',
      cost: 'Hides “Working” behind a notice that may say work continues fine.',
      mock: () => snSidebar(snCards({ 'Bump the API version': { status: snNoticePill('warning') } })),
    },
    {
      key: 'D', name: 'A system notification for errors', from: 'new',
      desc: 'An error notice in a thread you aren’t looking at sends a system notification (thread title, notice title and description), when Notifications are on. Nothing on the card.',
      good: 'Reaches you outside agentZ for the rare notice that matters.',
      cost: 'Notifications for something the agent says it will work around.',
      mock: () => snSidebar(snCards(), { w: 640, over: snSystem('right:12px;top:12px') }),
    },
  ],
});

// 4. Compaction in the conversation ---------------------------------------------------------
const snCompactBody = (entry, { work = SN_TURNS.secondWork, gen = snGenerating('Working for 3m 40s') } = {}) => snConversation([SN_TURNS.first, SN_TURNS.firstWork, SN_TURNS.firstReply, SN_TURNS.second, work, entry, gen]);
const snStates = (render) => frame(`<div class="col" style="padding:6px 0">${['progress', 'failed', 'cancelled'].map((status) => render(status)).join('')}</div>`, { w: SN_W, h: 112, style: 'border:1px solid var(--b);border-radius:8px' });
const snWithStates = (scene, render) => `<div style="display:flex;flex-direction:column;gap:10px">${scene}${snStates(render)}${snCaption('Above: completed. Below: in progress, failed, canceled.')}</div>`;
const snPillDivider = (status) => `<div class="row" style="gap:12px;padding:6px 20px"><span class="sn-rule"></span><span class="row g1" style="padding:2px 8px;border-radius:999px;border:1px solid var(--bv);flex:none">${status === 'progress' ? snSpin() : `<span class="mu" style="display:inline-flex">${ic('compact', 'xs')}</span>`}<span class="sm" style="color:${status === 'failed' ? 'var(--del)' : 'var(--mu)'}">${{ progress: 'Compacting context…', done: 'Context compacted', failed: 'Compaction failed', cancelled: 'Compaction canceled' }[status]}</span></span><span class="sn-rule"></span></div>`;
const snHandoffDivider = (status) => `<div class="row" style="gap:12px;padding:10px 20px 4px"><span class="sn-rule"></span><span class="row g15 sm mu none">${status === 'progress' ? snSpin() : ic('compact', 'xs')}<span style="color:${status === 'failed' ? 'var(--del)' : 'inherit'}">${{ progress: 'Compacting context…', done: 'Context compacted', failed: 'Compaction failed', cancelled: 'Compaction canceled' }[status]}</span>${status === 'done' ? '<span class="ph">· 84% → 16% of the context</span>' : ''}</span><span class="sn-rule"></span></div>`;
const snWorkCompaction = (status) => snWorkRow('compact', { progress: 'Compacting context', done: 'Compacted the context', failed: 'Compaction failed', cancelled: 'Compaction canceled' }[status], { shimmer: status === 'progress' });

TOPICS.push({
  id: 'compaction', section: 'Compaction', title: 'Compaction in the conversation', size: 'wide', rec: 'A',
  now: 'Nothing. An agent compacts on its own when its context fills, or when you send its slash command (Claude’s <code>/compact</code>); agentZ drops the compaction updates, so the conversation goes on with no mark, and only the context ring under the composer drops (its tooltip: “Context 16% • 31k / 200k”). The picture is recreated from the code.',
  nowImg: 'img/now-compaction.png',
  issues: [
    'After a compaction the agent remembers only its summary, but nothing shows where earlier messages stopped counting.',
    'A compaction can take a minute; meanwhile the thread just says “Working”.',
  ],
  options: [
    {
      key: 'A', name: 'A divider with a button', from: 'Zed',
      desc: 'Zed’s entry, where the first update arrived: a rule on each side of a small button with the Compact icon. It reads “Compacting Context…” with a spinner while it runs (the “Working for…” row hides meanwhile), then “Context Compacted”, “Compaction Failed” or “Compaction Canceled”. With a summary or an error it has a chevron and opens (next topic); without, it is a plain label.',
      good: 'Zed’s look and words. Marks clearly where the agent’s memory starts over.',
      cost: 'A full-width divider for something that happens inside a turn.',
      mock: () => snWithStates(snScene({ body: snCompactBody(snCompaction()), composer: snComposer({ ring: 0.16 }) }), (status) => snCompaction({ status })),
    },
    {
      key: 'B', name: 'A row in the turn’s work', from: 'new',
      desc: 'A row among the turn’s tool calls and thoughts, as a thought is: the Compact icon, “Compacting context” with a shimmer, then “Compacted the context”. It folds with the rest into the group’s line (“Read 2 files, searched code 1 time, and compacted the context”). The summary opens under it, as a thought’s text does.',
      good: 'Looks like the agent’s other work; adds no divider.',
      cost: 'Folded away by default, so where memory starts over is hidden.',
      mock: () => snWithStates(snScene({ body: snCompactBody('', { work: `${snWork('Read 2 files, searched code 1 time, and compacted the context', { open: true })}${snWorkRow('file', 'Read src/app/checkout/page.tsx')}${snWorkRow('file', 'Read src/app/cart/totals.ts')}${snWorkRow('search', 'Searched for “promo”')}${snWorkRow('compact', 'Compacted the context')}` }), composer: snComposer({ ring: 0.16 }) }), snWorkCompaction),
    },
    {
      key: 'C', name: 'A pill on a rule', from: 't3code (fork divider)',
      desc: 'The pill divider a fork shows (“Forked from conversation”): a rounded pill with the Compact icon and “Context compacted”, on a rule. A click opens the summary.',
      good: 'Already the app’s way of marking a break in a conversation.',
      cost: 'Looks like a fork’s divider, which means something else.',
      mock: () => snWithStates(snScene({ body: snCompactBody(snPillDivider('done')), composer: snComposer({ ring: 0.16 }) }), snPillDivider),
    },
    {
      key: 'D', name: 'A labelled rule with the context used', from: 'agentZ (Context handoff)',
      desc: 'Like the “Context handoff” divider: rules on each side of the icon and a muted label, plus how full the context was before and after (“84% → 16% of the context”), taken from the agent’s usage updates around it. A click opens the summary.',
      good: 'Shows what compaction did, in the numbers the ring already has.',
      cost: 'Compaction updates carry no token counts, so the numbers need the agent to report usage; some agents don’t.',
      mock: () => snWithStates(snScene({ body: snCompactBody(snHandoffDivider('done')), composer: snComposer({ ring: 0.16 }) }), snHandoffDivider),
    },
    {
      key: 'E', name: 'Only in the context ring', from: 'new',
      desc: 'No entry in the conversation. While it runs, the ring spins; after, its tooltip adds “Compacted 2 minutes ago”. The summary isn’t shown.',
      good: 'Nothing new in the conversation.',
      cost: 'You can’t see where the agent’s memory starts over, or read its summary.',
      mock: () => snScene({ body: snCompactBody(''), composer: snComposer({ ring: 0.16 }), overlays: snTip('<div class="sm mu">Context</div><div>16% <span style="color:rgba(135,138,152,.6)">•</span> 31k <span style="color:rgba(135,138,152,.6)">/</span> <span class="mu">200k</span></div><div class="sm mu" style="margin-top:4px">Compacted 2 minutes ago</div>', 'right:96px;bottom:50px') }),
    },
  ],
});

// 5. Its summary ------------------------------------------------------------------------------
const snSummaryPop = () => `<div class="pop" style="left:150px;top:150px;width:420px;max-height:230px;overflow:hidden"><div class="row" style="height:30px;padding:0 10px;font-size:12px;color:var(--mu);border-bottom:1px solid var(--bv)">${ic('compact', 'xs')}<span style="margin-left:6px" class="grow">Context Compacted</span>${ic('copy', 'xs')}</div><div style="padding:8px 10px;font-size:13px;line-height:20px">${SN_SUMMARY_HTML}</div></div>`;
const snPreview = () => `<div style="padding:6px 20px"><div class="row" style="gap:8px"><span class="sn-rule"></span><span class="sn-cbtn"><span class="mu" style="display:inline-flex">${ic('compact', 'xs')}</span>Context Compacted</span><span class="sn-rule"></span></div><div style="margin:6px 0 0;padding-left:12px;border-left:2px solid var(--bv);font-size:13px;line-height:20px;color:var(--mu);max-height:60px;overflow:hidden;position:relative">${SN_SUMMARY_HTML}<div style="position:absolute;left:0;right:0;bottom:0;height:22px;background:linear-gradient(transparent,#282c33)"></div></div><div class="sm ac" style="padding-left:12px;margin-top:2px">Show all</div></div>`;
const snSummaryBody = (entry) => snConversation([SN_TURNS.firstReply, SN_TURNS.second, SN_TURNS.secondWork, entry, snGenerating('Working for 3m 40s')]);

TOPICS.push({
  id: 'read-summary', section: 'Compaction', title: 'Reading the summary', size: 'wide', rec: 'A',
  now: 'agentZ never shows a compaction’s summary. The agent can send one (rich content like a tool’s output, streamed in pieces and replaced by later updates) or an error saying why it failed.',
  issues: ['The summary is all the agent remembers of what came before; without it you can’t check what it kept.'],
  options: [
    {
      key: 'A', name: 'Opens in place', from: 'Zed',
      desc: 'Zed: a click on the divider opens the summary right under it, in a bordered box rendered like a tool’s output, with a bar at its foot that closes it. A failure shows its error in red there. Opening it stops the conversation following the end, so it doesn’t scroll away.',
      good: 'Read in context, at full width, as long as it is.',
      cost: 'A long summary pushes the rest of the conversation down while open.',
      mock: () => snScene({ body: snSummaryBody(snCompaction({ open: true, hov: true })), composer: snComposer({ ring: 0.16 }) }),
    },
    {
      key: 'B', name: 'In a popover', from: 'new',
      desc: 'A click opens the summary in a popover by the divider, scrolling inside past 230 px, with a copy button. The conversation doesn’t move.',
      good: 'Nothing shifts; quick to glance at and close.',
      cost: 'A long summary scrolls in a small box.',
      mock: () => snScene({ body: snSummaryBody(snCompaction({ hov: true })), composer: snComposer({ ring: 0.16 }), overlays: snSummaryPop() }),
    },
    {
      key: 'C', name: 'Its start, always shown', from: 'new',
      desc: 'The summary’s first three lines show under the divider, faded, with “Show all” to open the rest in place.',
      good: 'You see what it kept without a click.',
      cost: 'Three more lines in the conversation at every compaction.',
      mock: () => snScene({ body: snSummaryBody(snPreview()), composer: snComposer({ ring: 0.16 }) }),
    },
  ],
});

// 6. Asking for a compaction ------------------------------------------------------------------
const snSlashMenu = () => `<div class="menu" style="left:30px;bottom:66px;width:420px"><div class="it">${ic('slash', 'sm')}<span>/review</span><span class="kb" style="margin-left:12px">Review a pull request</span></div><div class="it hl">${ic('slash', 'sm')}<span>/compact</span><span class="kb" style="margin-left:12px">Clear history but keep a summary in context</span></div><div class="it">${ic('slash', 'sm')}<span>/init</span><span class="kb" style="margin-left:12px">Initialize a new CLAUDE.md file</span></div></div>`;
const snContextCallout = () => `<div class="sn-callout" style="background:rgba(222,193,132,.05)"><span class="ic">${snSevIcon('warning')}</span><div class="grow col" style="min-width:0;gap:6px"><div class="row" style="min-height:18px"><span class="tt">Thread reaching the context limit soon</span></div><div class="ds">The agent compacts on its own when it’s full. To do it now, compact it, or start a new thread and @-mention this one.</div><div class="row g1"><span class="btn sm">Compact</span><span class="btn sm">Start New Thread</span></div></div></div>`;
const snRingMenu = () => `<div class="menu" style="right:70px;bottom:46px;min-width:220px"><div class="lbl">Context 87% • 174k / 200k</div><div class="it hl">${ic('compact', 'sm')}Compact Context</div></div>`;
const snMoreMenu = () => `<div class="menu" style="right:8px;top:38px;min-width:220px"><div class="it hl">${ic('compact', 'sm')}Compact Context</div><div class="hr"></div><div class="it">Reauthenticate</div><div class="it">Log Out…</div><div class="hr"></div><div class="it">Reload Agent</div></div>`;

TOPICS.push({
  id: 'ask', section: 'Compaction', title: 'Asking for a compaction', size: 'wide', rec: 'A',
  now: 'The agent decides when to compact. You can ask for one only with the agent’s own slash command, if it has one (Claude Agent’s <code>/compact</code>), which agentZ lists in the composer’s / menu with the agent’s other commands. The context ring turns yellow at 85%.',
  issues: ['Not every agent has a compact command, and nothing says which do.'],
  options: [
    {
      key: 'A', name: 'Only the agent’s own command', from: 'Zed',
      desc: 'Zed for ACP agents: nothing new. /compact is in the / menu when the agent offers it, and the divider shows what happened.',
      good: 'No button that does nothing for agents without the command.',
      cost: 'You must know the command exists.',
      mock: () => snScene({ composer: snComposer({ ring: 0.87, text: '/comp', working: false }), body: snConversation([SN_TURNS.first, SN_TURNS.firstWork, SN_TURNS.firstReply]), overlays: snSlashMenu() }),
    },
    {
      key: 'B', name: 'A notice when the context is nearly full', from: 'Zed (its own agent)',
      desc: 'Zed’s token-limit callout, which it shows for its own agent: at 85%, a warning above the composer, “Thread reaching the context limit soon”, with Compact (which sends the agent’s compact command; only when it has one) and Start New Thread. × hides it for this thread.',
      good: 'Tells you before the agent compacts on its own, with what you can do.',
      cost: 'A warning about something the agent handles by itself.',
      mock: () => snScene({ composer: snComposer({ ring: 0.87, working: false }), body: snConversation([SN_TURNS.first, SN_TURNS.firstWork, SN_TURNS.firstReply]), above: snContextCallout() }),
    },
    {
      key: 'C', name: 'In the context ring', from: 'new',
      desc: 'A click on the context ring opens a menu with its numbers and Compact Context, which sends the agent’s compact command. Without the command, the ring only has its tooltip.',
      good: 'Where you look when the context is filling up.',
      cost: 'The ring isn’t a button today; easy to miss.',
      mock: () => snScene({ composer: snComposer({ ring: 0.87, working: false }), body: snConversation([SN_TURNS.first, SN_TURNS.firstWork, SN_TURNS.firstReply]), overlays: snRingMenu() }),
    },
    {
      key: 'D', name: 'In the thread’s ⋯ menu', from: 'new',
      desc: 'Compact Context heads the header’s ⋯ (the agent’s options) when the agent has a compact command.',
      good: 'Out of the way, next to the agent’s other actions.',
      cost: 'Hidden two clicks away from the ring it relates to.',
      mock: () => snScene({ composer: snComposer({ ring: 0.87, working: false }), body: snConversation([SN_TURNS.first, SN_TURNS.firstWork, SN_TURNS.firstReply]), overlays: snMoreMenu() }),
    },
  ],
});
