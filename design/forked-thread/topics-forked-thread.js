// How a forked thread looks: when it opens, once it has its own messages, and where Open Source
// and Merge Back live. Today's views are recreated from agent_view.rs (render_fork_dividers,
// render_fork_source_pill), not captured from the running app.

const forkToday = () => fkWin(fkToolbar() + scroller(messages([0, 1, 2]) + forkDivider(), { justify: 'flex-end' }) + bottomComposer());
const forkTodayAfter = () => fkWin(fkToolbar() + scroller(messages([0, 1, 2]) + forkDivider() + OWN_TURN, { justify: 'flex-end' }) + bottomComposer({ working: true }));
const forkTodayMenu = () => frame(fkToolbar({ pill: sourcePill({ open: true }) }) + sourceMenu() + '<div style="height:120px"></div>', { w: 640, h: 190 });

// 1. When it opens -----------------------------------------------------------------------
TOPICS.push({
  id: 'opens', section: 'Fork', title: 'The fork when it opens, before its first message', size: 'wide', rec: 'B',
  now: 'Fork from This Response makes the thread and opens it as a long conversation: the source’s turns up to that reply, in full, ending at a “Forked from conversation” pill divider, with the composer as a bar along the bottom like any thread. The header has a pill with the source’s title. Nothing says what a fork is or that the first message will carry all of it to the agent.',
  nowImg: 'img/now-opens.png',
  issues: [
    'It looks like an old thread you opened, not like something you just started: the only thing to type into is a bar under a wall of earlier messages.',
    'The divider that says it’s a fork is the last thing in a long scroll, below everything, so a first look doesn’t tell you this thread is new.',
    'It doesn’t say that the earlier messages are sent to the agent with your first message, or how many there are.',
  ],
  options: [
    {
      key: 'A', name: 'New thread screen with a source card', from: 'New thread screen, t3code’s fork link',
      desc: 'Exactly the new thread screen (headline, composer card, folder and branch strip), plus one card above the headline: a fork icon, “Forked from Add the checkout page”, “At its reply 3 of 5 · 6 messages come along as context”, and an Open button for the source. The carried conversation isn’t drawn at all.',
      good: 'Reads as a fresh start at once, and says what comes along. Smallest change from the new thread screen.',
      cost: 'You can’t see where the conversation left off without opening the source.',
      mock: () => fkWin(fkToolbar() + newScreen({ above: sourceCard() })),
    },
    {
      key: 'B', name: 'New thread screen with the reply it left off at', from: 'New thread screen, new',
      desc: 'The new thread screen, with the reply you forked from (reply 3) quoted over the composer: three lines, faded at the bottom, headed “Forked from Add the checkout page · its reply 3 of 5”. Clicking it opens the source at that reply. The earlier turns are not drawn; they still go with the first message.',
      good: 'A fresh start that still shows where you are, so you know what to type.',
      cost: 'A quote of one reply doesn’t say that 5 earlier messages come along too (the header line can).',
      mock: () => fkWin(fkToolbar() + newScreen({ between: quotedReply() })),
    },
    {
      key: 'C', name: 'Context chip over the composer', from: 'Merge-back chip (new here)',
      desc: 'The new thread screen with the carried conversation as a chip inside the composer card, like the merge-back chip: “Add the checkout page · 6 messages” with an ×. Clicking the chip previews exactly what goes to the agent; × sends the first message without it (then the thread is a plain new thread, with its source pill still in the header).',
      good: 'Same look and words as the merge-back chip, and you see and control what goes along.',
      cost: 'Dropping it makes “fork” mean nothing for that thread. The chip is small: it says less about where it left off than a card.',
      mock: () => fkWin(fkToolbar() + newScreen({ composer: composerCard({ chips: contextChip() }) }) + fkNote('Click: preview, ×: drop', 'left:390px;top:248px')),
    },
    {
      key: 'D', name: 'Folded conversation with the last reply', from: 'new',
      desc: 'The conversation is kept but folded into one line at the top: “6 earlier messages from Add the checkout page”, which expands in place. Under it, the reply it forked from, the “Forked from conversation” divider, and the composer along the bottom as in any open thread.',
      good: 'Nothing is hidden for good, and the long scroll becomes one line.',
      cost: 'It’s still an open thread, not the new thread screen: the composer stays a bar, not a card.',
      mock: () => fkWin(fkToolbar() + scroller(foldLine() + forkReply() + forkDivider(), { gap: 14, pad: '18px 0 8px' }) + bottomComposer()),
    },
    {
      key: 'E', name: 'The headline says it', from: 'New thread screen, new',
      desc: 'The new thread screen with its headline replaced: “Branching from Add the checkout page” and, under it, “Your first message continues from reply 3. The 6 earlier messages go along as context.” No card or quote; the source is a link in the line. Composer card and strip as in the new thread screen.',
      good: 'Nothing added to the screen but words: it is the new thread screen, and says what it is.',
      cost: 'Shows nothing of the conversation, and the link is small.',
      mock: () => fkWin(fkToolbar() + newScreen({ headline: `Branching from <span style="text-decoration:underline;text-decoration-color:var(--bv);text-underline-offset:4px">${SOURCE}</span>`, sub: 'Your first message continues from reply 3. The 6 earlier messages go along as context.' })),
    },
  ],
});

// 2. Once it has its own messages -------------------------------------------------------
TOPICS.push({
  id: 'after', section: 'Fork', title: 'Once the fork has its own messages', size: 'wide', rec: 'E',
  now: 'The carried conversation stays in the thread in full, like any other messages, and its “Forked from conversation” pill divider sits where the fork’s own messages start. The thread scrolls from the source’s first message on, so the fork’s own work starts far down.',
  nowImg: 'img/now-after.png',
  issues: [
    'The fork’s own messages are the part you care about, and they begin below the source’s whole conversation.',
    'The carried messages look just like the fork’s own, so the thread reads as one long conversation by one task.',
    'The only difference is a thin divider, which scrolls out of sight as soon as the thread grows.',
  ],
  options: [
    {
      key: 'A', name: 'Kept in full, a stronger divider', from: 't3code dividers',
      desc: 'The carried messages stay as they are. The divider becomes a labelled band: “Fork of Add the checkout page, from its reply 3”, with the pill opening the source. Its own messages follow.',
      good: 'Nothing is hidden, the boundary is easy to spot.',
      cost: 'The thread is still a long scroll with the fork’s work at the end.',
      mock: () => fkWin(fkToolbar() + scroller(messages([2]) + forkDivider('Fork of Add the checkout page, from its reply 3', { hover: true }) + OWN_TURN, { justify: 'flex-end' }) + bottomComposer({ working: true }) + fkNote('Earlier turns above, scrolled', 'left:40px;top:50px')),
    },
    {
      key: 'B', name: 'Dimmed above the divider', from: 'new',
      desc: 'The carried messages stay, at half opacity, so the fork’s own messages stand out. The pill divider stays between. Hover brings them back to full.',
      good: 'You can scroll back to everything, and still tell mine from carried.',
      cost: 'Dimmed text is harder to read, and the long scroll remains.',
      mock: () => fkWin(fkToolbar() + scroller(`<div style="opacity:.45">${messages([1, 2])}</div>` + forkDivider() + OWN_TURN, { justify: 'flex-end' }) + bottomComposer({ working: true })),
    },
    {
      key: 'C', name: 'Folded into one line', from: 'new',
      desc: 'All the carried messages fold into one line at the top, “6 earlier messages from Add the checkout page”, which expands in place. The thread starts with the fork’s own first message.',
      good: 'The fork’s own work starts at the top. The long conversation costs one line.',
      cost: 'The reply it forked from isn’t visible without expanding.',
      mock: () => fkWin(fkToolbar() + scroller(foldLine() + OWN_TURN, { gap: 14 }) + bottomComposer({ working: true })),
    },
    {
      key: 'D', name: 'Not shown, a source row', from: 'new, t3code’s fork link',
      desc: 'Nothing carried is shown. The thread starts with a “Forked from Add the checkout page · its reply 3 of 5” pill row that opens the source, then its own messages. The conversation still goes to the agent with the first message.',
      good: 'The cleanest: the thread is only what you did here.',
      cost: 'You can’t check what the agent was told without opening the source.',
      mock: () => fkWin(fkToolbar() + scroller(forkDivider(`Forked from ${SOURCE} · its reply 3 of 5`, { hover: true }) + OWN_TURN, { gap: 14 }) + bottomComposer({ working: true })),
    },
    {
      key: 'E', name: 'Folded, with the reply it left off at', from: 'new',
      desc: 'Earlier turns fold into one line (“4 earlier messages from Add the checkout page”). The reply it forked from stays, dimmed, with the divider under it, then the fork’s own messages. Same pieces as option B of the first topic, so it opens and goes on in one look.',
      good: 'Shows where it left off and what the fork added, in about a screen.',
      cost: 'The most parts: a fold, a dimmed reply and a divider.',
      mock: () => fkWin(fkToolbar() + scroller(foldLine({ label: `4 earlier messages from ${SOURCE}` }) + `<div style="opacity:.5">${forkReply()}</div>` + forkDivider() + OWN_TURN, { gap: 12 }) + bottomComposer({ working: true })),
    },
  ],
});

// 3. Open Source and Merge Back ---------------------------------------------------------
const bar = (inner) => frame(inner + '<div style="height:90px"></div>', { w: 640, h: 190 });
TOPICS.push({
  id: 'actions', section: 'Fork', title: 'Open Source and Merge Back in this look', size: 'medium', rec: 'E',
  now: 'Both live in the menu of the header pill that holds the source’s title: Open Source Thread, and Merge Back, which is off while the fork is working. The pill is there before the first message and after.',
  nowImg: 'img/now-actions.png',
  issues: [
    'Both actions are behind one pill that looks like a label.',
    'Merge Back is the fork’s whole point, and it has no visible button.',
    'It’s the same place whether the fork has anything to merge or not.',
  ],
  options: [
    {
      key: 'A', name: 'The pill’s menu, as today', from: 't3code merge button, as built',
      desc: 'The header pill opens Open Source Thread and Merge Back. No change.',
      good: 'Nothing to change; works with every look above.', cost: 'The issues above stay.',
      mock: () => bar(fkToolbar({ pill: sourcePill({ open: true }) }) + sourceMenu()),
    },
    {
      key: 'B', name: 'On the source card or fold line', from: 'new',
      desc: 'The actions sit on the card or fold line that stands for the source: Open and Merge Back as small buttons at its end. The header pill goes.',
      good: 'The actions are beside what they act on.', cost: 'Needs a card or fold line in the view; the card goes away if the first topic picks the context chip.',
      mock: () => bar(fkToolbar({ pill: '' }) + `<div style="padding:12px 16px">${foldLine({ label: `6 earlier messages from ${SOURCE}`, actions: '<span class="btn sm">Open</span><span class="btn sm">Merge Back</span>' })}</div>`),
    },
    {
      key: 'C', name: 'In the chip’s menu', from: 'Merge-back chip',
      desc: 'Before the first message only: the context chip has a chevron that opens Open Source Thread. Merge Back has no place until the fork has messages, then it is the header pill’s.',
      good: 'Nothing extra on screen.', cost: 'Two places, one before and one after the first message.',
      mock: () => frame(fkToolbar({ pill: '' }) + `<div style="padding:16px"><div style="border:1px solid var(--b);border-radius:10px;background:var(--panel);padding:12px 14px">${contextChip(SOURCE, '6 messages', { menu: true })}<span class="ph">Message the agent…</span></div></div><div class="menu" style="left:24px;top:116px;min-width:190px"><div class="it hl">${ic('arrow-up-right', 'sm')}Open Source Thread</div><div class="it dis">${ic('eye', 'sm')}Preview what goes along</div></div>`, { w: 640, h: 190 }),
    },
    {
      key: 'D', name: 'Two icon buttons in the header', from: 'new',
      desc: 'The pill goes. In the header, an Open Source icon button and a Merge Back icon button, each with a tooltip.',
      good: 'Both one click away, and quiet.', cost: 'Icons alone say less than a title; the source’s name is no longer shown.',
      mock: () => bar(fkToolbar({ pill: `${ibtn('arrow-up-right')}${ibtn('merge')}` }) + fkNote('Open Source Thread', 'right:50px;top:40px')),
    },
    {
      key: 'E', name: 'Pill opens the source, Merge Back beside it', from: 'new',
      desc: 'Clicking the pill’s title opens the source directly (its chevron still has the menu). A “Merge Back” button sits beside it, shown once the fork has messages of its own, and off while it’s working.',
      good: 'Merge Back is visible exactly when there’s something to merge, and opening the source takes one click.',
      cost: 'The header grows a button once the fork has messages.',
      mock: () => bar(fkToolbar({ pill: `${sourcePill()}<span class="btn sm" style="margin-left:4px">${ic('merge', 'xs')}Merge Back</span>` })),
    },
  ],
});
