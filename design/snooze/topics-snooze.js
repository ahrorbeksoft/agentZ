// Snooze: hide a thread until a chosen time, then put it back in the list (t3code’s snooze).

// What the pictures of Today show.
const szTodaySidebar = () => `<div>${szSidebar([...szActiveCards({ tests: { hov: true } }), szRule(), szRestShelves()], { h: 600 })}${szCaption('A hovered card: its buttons replace the time')}</div>`;
const szTodayMenu = () => `<div>${szSidebar([...szActiveCards({ checkout: { hov: true } }), szRule(), szRestShelves()], { h: 600, over: szMenu(szThreadMenu(), 'left:80px;top:72px;min-width:200px') })}${szCaption('The thread menu (right-click a card)')}</div>`;
const szTodayLimit = () => `<div>${szThread({ above: szLimitNotice() })}${szCaption('A thread stopped at its account’s limit')}</div>`;

// 1. Where you snooze from --------------------------------------------------------------------
const szSnoozeItem = (hl = true) => szItem('clock', 'Snooze', { arrow: true, hl });

TOPICS.push({
  id: 'where', section: 'Snoozing', title: 'Where you snooze from', size: 'wide', type: 'multi', rec: 'ABC',
  now: 'Nothing snoozes a thread. A thread leaves the list only by being archived: the card’s hover button (“Archive”, beside an unpin button for pinned threads), the thread menu (Unpin, Rename, Regenerate Title, Archive, Project Settings, Delete…), or Auto-archive after a day, 3 days or a week without activity. The pictures are recreated from the code.',
  nowImg: 'img/now-sidebar.png',
  issues: [
    'A thread you can’t act on until later (waiting for a review, for a limit to reset) sits in the list among the ones you can.',
    'Archiving hides it for good: nothing brings it back at a time.',
  ],
  options: [
    {
      key: 'A', name: 'A clock on the card', from: 't3code',
      desc: 't3code: hovering a card shows a clock button (“Snooze Thread”) before Archive; it opens the snooze menu under it. Not shown while the thread waits for your approval or input.',
      good: 'One move from the list, next to Archive.',
      cost: 'A third hover button on pinned cards.',
      mock: () => szSidebar([...szActiveCards({ tests: { hov: true, buttons: szHoverButtons(SZ_THREADS.tests, szClockButton(true)) } }), szRule(), szRestShelves()], { h: 600, w: 420, over: szMenu(szPresetItems(), 'left:96px;top:272px;min-width:190px') }),
    },
    {
      key: 'B', name: 'Snooze ▸ in the thread menu', from: 't3code',
      desc: '“Snooze” with the same presets as a submenu, after Archive in the thread menu (t3code puts it after Settle). A snoozed thread’s menu has “Wake Now” there instead.',
      good: 'Found where the other thread actions are.',
      cost: '—',
      mock: () => szSidebar([...szActiveCards({ checkout: { hov: true } }), szRule(), szRestShelves()], { h: 600, w: 420, over: szMenu(szThreadMenu({ snooze: szSnoozeItem() }), 'left:16px;top:72px;min-width:180px') + szMenu(szPresetItems({ hl: 3 }), 'left:120px;top:182px;min-width:160px') }),
    },
    {
      key: 'C', name: 'Snooze Until Reset on the limit notice', from: 't3code',
      desc: 't3code’s usage-limit banner: “Snooze Until Reset” beside “Continue at 16:10” (and its own limit topic below). Hidden once the thread is snoozed.',
      good: 'Right where you learn the thread can’t go on until later.',
      cost: 'One more button on a notice that can already have four.',
      mock: () => szThread({ above: szLimitNotice(`<span class="btn sm">${ic('clock', 'xs')}Snooze Until Reset</span>`), body: szConversation() }),
    },
    {
      key: 'D', name: 'In the thread’s title menu', from: 'new',
      desc: 'The header’s title menu (Rename, Regenerate Title, …) gets “Snooze ▸”, so the open thread can be snoozed without the sidebar.',
      good: 'Works with the sidebar hidden, or in a pane of the Workspaces view.',
      cost: 'Another copy of the same menu.',
      mock: () => szThread({ overlays: szMenu([szItem('pencil', 'Rename'), szItem('refresh', 'Regenerate Title'), szHr, szSnoozeItem(), szItem('archive', 'Archive')], 'left:130px;top:34px;min-width:200px') }),
    },
  ],
});

// 2. The snooze menu --------------------------------------------------------------------------
const szMenuScene = (menu) => szSidebar([...szActiveCards({ tests: { hov: true, buttons: szHoverButtons(SZ_THREADS.tests, szClockButton(true)) } })], { h: 520, w: 420, over: menu });
const szTypedMenu = () => `<div class="menu" style="left:70px;top:272px;width:230px"><div style="padding:4px 4px 6px"><div class="sz-field focus" style="width:100%">${ic('clock', 'xs mu')}fri 10<span style="display:inline-block;width:1.5px;height:15px;background:var(--ac)"></span></div><div class="hint" style="padding:4px 2px 0">Fri 10:00 · in 3 days</div></div>${szHr}${SZ_PRESETS.slice(0, 4).map(([label, time]) => szItem('', label, { time })).join('')}</div>`;
const szChipsMenu = () => `<div class="pop" style="left:60px;top:272px;width:236px;padding:8px"><div class="xs ph" style="margin-bottom:6px">Snooze until</div><div style="display:grid;grid-template-columns:1fr 1fr;gap:6px">${['1 hour', '3 hours', 'Tomorrow', 'Next week'].map((label, i) => `<span class="btn sm" style="justify-content:center;${i === 2 ? 'background:var(--hov)' : ''}">${label}</span>`).join('')}</div><div class="sm mu row g15" style="margin-top:8px">${ic('calendar', 'xs')}Pick a date and time…</div></div>`;

TOPICS.push({
  id: 'menu', section: 'Snoozing', title: 'The snooze menu', size: 'medium', rec: 'A',
  now: 'There is no snooze. agentZ writes times on a 24-hour clock (“16:10”, “Mon 16:10”), and knows when an account’s used-up limit resets.',
  issues: [],
  options: [
    {
      key: 'A', name: 'Presets with their times', from: 't3code',
      desc: 't3code’s presets, each with its time on the right: In 1 hour, In 3 hours, This evening (18:00, only when that’s more than an hour away), Tomorrow (9:00), Next week (Monday 9:00, left out when that’s tomorrow), then Custom…. When the thread’s account has a used-up limit, “Until the limit resets” and its time comes first. Snoozing shows a toast with Undo for 5 seconds, as t3code does.',
      good: 'The usual choices in one click, and you see exactly when each one is.',
      cost: 'Six or seven rows.',
      mock: () => szMenuScene(szMenu(szPresetItems({ reset: true, hl: 2 }), 'left:70px;top:272px;min-width:220px')),
    },
    {
      key: 'B', name: 'Short, without times', from: 'new',
      desc: 'Four buttons, 1 hour, 3 hours, Tomorrow and Next week, and “Pick a date and time…”. Times show in a tooltip.',
      good: 'Small and quick to scan.',
      cost: 'You don’t see when “Tomorrow” is until you hover; no evening or reset choice.',
      mock: () => szMenuScene(szChipsMenu()),
    },
    {
      key: 'C', name: 'Type a time', from: 'new',
      desc: 'A field heads the menu: type “2h”, “tomorrow 9” or “fri 10” and it shows the time it understood; Enter snoozes. The presets follow.',
      good: 'Any time without a dialog.',
      cost: 'A small parser to learn and to build; English words only.',
      mock: () => szMenuScene(szTypedMenu()),
    },
  ],
});

// 3. Another time -----------------------------------------------------------------------------
const szCalendar = () => {
  const heads = ['Mo', 'Tu', 'We', 'Th', 'Fr', 'Sa', 'Su'].map((d) => `<span class="h">${d}</span>`).join('');
  const days = [];
  for (let i = 0; i < 35; i++) {
    const day = i - 1;
    const label = day < 1 ? 30 + day : day > 31 ? day - 31 : day;
    const cls = day < 1 || day > 31 ? 'dim' : day === 23 ? 'on' : '';
    days.push(`<span class="${cls}">${label}</span>`);
  }
  return `<div class="sz-cal">${heads}${days.join('')}</div>`;
};
const szDialog = (body, { w = 330, left = 150, top = 60, title = 'Custom snooze' } = {}) => `<div class="modal-back"></div><div class="sz-dlg" style="left:${left}px;top:${top}px;width:${w}px"><h4>${title}</h4><p>Choose when the thread comes back to the list.</p>${body}<div class="row g2" style="justify-content:flex-end;margin-top:14px"><span class="btn">Cancel</span><span class="btn primary">Snooze</span></div></div>`;
const szSeg = (items, on) => `<span class="seg">${items.map((item) => `<span class="${item === on ? 'on' : ''}">${item}</span>`).join('')}</span>`;
const szBackdrop = (over) => frame(`<div style="height:100%;background:var(--ed)"></div>${over}`, { w: 640, h: 420, style: 'border:1px solid var(--b);border-radius:8px' });

TOPICS.push({
  id: 'custom', section: 'Snoozing', title: 'Picking another time', size: 'medium', rec: 'A',
  now: 'There is no date or time picker anywhere in agentZ yet.',
  issues: [],
  options: [
    {
      key: 'A', name: 'A dialog: date or duration', from: 't3code',
      desc: 't3code’s “Custom snooze” dialog: Date (a calendar and a time field) or Duration (“Snooze for” an amount in minutes, hours or days), and Snooze.',
      good: 'Any moment, either way you think of it.',
      cost: 'A dialog and a calendar to build.',
      mock: () => szBackdrop(szDialog(`<div style="margin-bottom:10px">${szSeg(['Date', 'Duration'], 'Date')}</div><div class="row" style="gap:16px;align-items:flex-start">${szCalendar()}<div class="col g1"><span class="xs ph">Time</span><span class="sz-field" style="width:80px">9:00</span></div></div>`, { w: 360, left: 140, top: 50 })),
    },
    {
      key: 'B', name: 'Fields in the menu', from: 'new',
      desc: 'Custom… opens two fields in the menu itself, a date (“Thu 23 Oct”) and a time (“9:00”), with Snooze. ↑ ↓ change the part under the cursor.',
      good: 'No dialog over the app.',
      cost: 'No calendar to see the week.',
      mock: () => szMenuScene(`<div class="menu" style="left:70px;top:272px;width:230px">${SZ_PRESETS.slice(0, 3).map(([label, time]) => szItem('', label, { time })).join('')}${szHr}<div style="padding:4px"><div class="xs ph" style="margin-bottom:4px">Custom</div><div class="row g1"><span class="sz-field focus" style="flex:1">Thu 23 Oct</span><span class="sz-field" style="width:62px">9:00</span></div><div class="row" style="justify-content:flex-end;margin-top:6px"><span class="btn sm primary">Snooze</span></div></div></div>`),
    },
    {
      key: 'C', name: 'A duration only', from: 't3code (Duration)',
      desc: 'A small dialog with t3code’s Duration mode only: “Snooze for” an amount and minutes, hours or days.',
      good: 'Simplest to build and to use.',
      cost: '“Until Thursday morning” has to be worked out in hours.',
      mock: () => szBackdrop(szDialog(`<div class="row g2"><span class="sm mu">Snooze for</span><span class="sz-field focus" style="width:60px">2</span><span class="sz-field">days ${szChev()}</span></div><div class="hint" style="margin-top:6px">Until Thu 14:42</div>`, { w: 320, left: 160, top: 110 })),
    },
  ],
});

// 4. Where snoozed threads go -----------------------------------------------------------------
const szSnoozedShelf = ({ open = true, hov = '' } = {}) => szShelf('Snoozed', { count: 2, open, tone: 'accent' }) + (open ? szSlim('docs', { hov: hov === 'docs' }) + szSlim('orders', { hov: hov === 'orders' }) : '');
const szCountLink = () => `<div class="row g15 sm" style="height:28px;margin:2px 4px;padding:0 10px;color:var(--ac)">${ic('clock', 'xs')}2 snoozed${szChev()}</div>`;
const szSnoozedPop = () => `<div class="pop" style="left:24px;top:424px;width:250px;padding:4px">${szSlim('docs')}${szSlim('orders')}</div>`;
const szDimCard = (key) => szCard(key, { dim: true, status: `<span class="sz-left">${ic('clock', 'xs')} ${SZ_THREADS[key].snoozed}</span>` });

TOPICS.push({
  id: 'shelf', section: 'While snoozed', title: 'Where snoozed threads go', size: 'medium', rec: 'A',
  now: 'The sidebar lists draft rows, then the thread cards (pinned first), then folded shelves: Working (when that setting is on), Chats, Shells, Workspaces and Archived. A shelf’s header says “Archived (6)” folded and “Archived” open; its rows are slim (icon, title, time).',
  issues: ['A snoozed thread must leave the cards but stay findable, and you should see when it comes back.'],
  options: [
    {
      key: 'A', name: 'A Snoozed shelf', from: 't3code',
      desc: 't3code’s Snoozed shelf, as a shelf here: after the cards and before Chats, “Snoozed (2)” folded, in the accent color. Slim rows, soonest first, each with the time left (“3h”, “1d”) in the accent color; hovering a row shows “Wake Now” in its place. The open thread stays visible even when the shelf is folded.',
      good: 'Like Archived, which you already know; you see what comes back next.',
      cost: 'One more shelf.',
      mock: () => szSidebar([...szActiveCards({}, ['checkout', 'login', 'version', 'grid']), szRule(), szSnoozedShelf({ hov: 'orders' }), szRestShelves()], { h: 600 }),
    },
    {
      key: 'B', name: 'A count under the cards', from: 'new',
      desc: 'Snoozed threads leave the sidebar; a line under the cards, “2 snoozed”, opens a popover listing them with their times.',
      good: 'Takes one line, whatever the number.',
      cost: 'Out of sight is out of mind: you can’t see them without a click.',
      mock: () => szSidebar([...szActiveCards({}, ['checkout', 'login', 'version', 'grid']), szCountLink(), szRule(), szRestShelves()], { h: 600, over: szSnoozedPop() }),
    },
    {
      key: 'C', name: 'Dimmed cards at the end', from: 'new',
      desc: 'Snoozed threads keep their cards, dimmed, after the others, with a clock and the time left in place of their status.',
      good: 'Nothing new to look for; the whole card stays readable.',
      cost: 'They still take a card’s height each, which snoozing was meant to save.',
      mock: () => szSidebar([...szActiveCards({}, ['checkout', 'login', 'version', 'grid']), szDimCard('docs'), szDimCard('orders'), szRule(), szRestShelves()], { h: 680 }),
    },
  ],
});

// 5. When it wakes ----------------------------------------------------------------------------
const szWokeCard = (key, extra = {}) => szCard(key, { status: szWokeBadge(), ...extra });
const szNotification = () => `<div style="position:absolute;z-index:30;right:12px;top:16px;width:270px;border-radius:12px;background:rgba(58,62,70,.96);border:1px solid #555b66;box-shadow:0 10px 30px rgba(0,0,0,.5);padding:10px 12px;font:12px/1.35 -apple-system,'IBM Plex Sans',sans-serif;color:#eef0f3"><div class="row g2"><span style="width:20px;height:20px;border-radius:5px;background:#1b1f26;display:grid;place-items:center;font-weight:700;font-size:11px;color:var(--ac)">Z</span><b style="font-weight:600">agentZ</b><span class="grow"></span><span style="color:#a7adb8">now</span></div><div style="margin-top:6px;font-weight:600">Document the webhook retries</div><div style="color:#cfd3da">Back from snooze</div></div>`;

TOPICS.push({
  id: 'wake', section: 'Waking', title: 'When it comes back', size: 'narrow', rec: 'A',
  now: 'Cards are ordered by when they last became active (pinned ones first, then by drag). Attention states (Pending Approval, Awaiting Input, Completed) show in the status slot until you open the thread.',
  issues: ['Waking sends no prompt (the backlog), so a woken thread does nothing by itself: you have to notice it.'],
  options: [
    {
      key: 'A', name: 'In its place, marked Woke', from: 't3code',
      desc: 't3code: the thread returns to the cards where it was and shows “Woke” (an alarm clock, in yellow) in its status slot until you open it or click the mark.',
      good: 'Noticed without moving anything around.',
      cost: 'A thread that was far down comes back far down.',
      mock: () => szSidebar([...szActiveCards({}, ['checkout', 'login', 'version']), szWokeCard('docs'), szCard('grid')], { h: 480 }),
    },
    {
      key: 'B', name: 'At the top, marked Woke', from: 'new',
      desc: 'As A, but it comes back first among the unpinned cards, as a thread that just became active does.',
      good: 'Hard to miss.',
      cost: 'The list reorders by itself while you look at it.',
      mock: () => szSidebar([szCard('checkout', { on: true }), szWokeCard('docs'), szCard('login'), szCard('version'), szCard('grid')], { h: 480 }),
    },
    {
      key: 'C', name: 'Plus a system notification', from: 'new',
      desc: 'As A, and when Notifications are on, a system notification: the thread’s title and “Back from snooze”.',
      good: 'Reaches you when agentZ is in the background.',
      cost: 'A notification for something you scheduled yourself.',
      mock: () => szSidebar([...szActiveCards({}, ['checkout', 'login', 'version']), szWokeCard('docs'), szCard('grid')], { h: 480, w: 600, over: szNotification() }),
    },
  ],
});

// 6. What wakes it early ----------------------------------------------------------------------
TOPICS.push({
  id: 'early', section: 'Waking', title: 'What wakes it early', size: 'narrow', rec: 'A',
  now: 'A thread that needs you shows it on its card: Pending Approval, Awaiting Input, or an error; one whose turn ended shows Completed. Agents can keep working in the background, and a delegated task or subthread can finish later.',
  issues: ['A thread snoozed while its agent still works may finish, or stop to ask, long before its time.'],
  options: [
    {
      key: 'A', name: 'When it needs you, or finishes', from: 't3code',
      desc: 't3code’s “raised hand”: a snoozed thread wakes early when it asks for permission or input, fails, or a turn finishes after it was snoozed. Snoozing isn’t offered while it already waits for you.',
      good: 'Snooze never hides a request.',
      cost: 'A thread snoozed mid-turn comes back as soon as the turn ends.',
      mock: () => szSidebar([szCard('checkout', { on: true }), szCard('docs', { status: pill('pending') }), szCard('login'), szRule(), szShelf('Snoozed', { count: 1, tone: 'accent' })], { h: 400 }),
    },
    {
      key: 'B', name: 'When it needs you, not when it finishes', from: 'new',
      desc: 'Requests for permission or input and errors wake it; a turn that just ends doesn’t, so you can snooze a working thread and look at it later.',
      good: 'Snooze a long run and come back at your time.',
      cost: 'Two rules to remember.',
      mock: () => szSidebar([szCard('checkout', { on: true }), szCard('docs', { status: pill('pending') }), szCard('login'), szRule(), szShelf('Snoozed', { count: 1, tone: 'accent', open: true }) + szSlim('orders', { end: `<span class="sz-left" style="color:var(--ok)">done · 1d</span>` })], { h: 440 }),
    },
    {
      key: 'C', name: 'Only at its time', from: 'new',
      desc: 'Nothing wakes it early except Wake Now. Its slim row in the shelf shows its status (Pending Approval) beside its time.',
      good: 'Snooze means snooze.',
      cost: 'A thread asking for permission can wait hours behind a fold.',
      mock: () => szSidebar([szCard('checkout', { on: true }), szCard('login'), szCard('version'), szRule(), szShelf('Snoozed', { count: 2, tone: 'accent', open: true }) + szSlim('docs', { end: `<span class="row g15">${dot('pending')}<span class="sz-left">3h</span></span>` }) + szSlim('orders')], { h: 480 }),
    },
  ],
});

// 7. The open thread while snoozed ------------------------------------------------------------
const szStatusLine = (label, action) => `<div class="sz-status">${ic('alarm-clock', 'sm')}<span>${label}</span><span>·</span><span class="lnk">${action}</span></div>`;
const szSnoozeCallout = () => `<div style="padding:0 8px 8px"><div class="sz-callout"><span style="height:18px;display:inline-flex;align-items:center" class="mu">${ic('info', 'sm')}</span><div class="grow col" style="gap:6px"><div>Snoozed until Wed 9:00</div><div class="mu">It comes back to the list then. Sending a message wakes it now.</div><div><span class="btn sm">Wake Now</span></div></div></div></div>`;
const szHeaderPill = () => `<span class="sz-pill">${ic('alarm-clock', 'xs')}Wed 9:00${szChev()}</span>`;

TOPICS.push({
  id: 'thread', section: 'While snoozed', title: 'The open thread while snoozed', size: 'wide', rec: 'A',
  now: 'An archived thread shows an Info callout in place of its composer, “This thread is archived”, with Unarchive. Nothing else marks a thread’s state inside it.',
  issues: ['Opening a snoozed thread from the shelf shouldn’t wake it, but you should see it’s snoozed and until when.'],
  options: [
    {
      key: 'A', name: 'A status line above the composer', from: 't3code',
      desc: 't3code’s status line, centered above the composer in small muted text: an alarm clock, “Snoozed until Wed 9:00” · “Wake Now”. Once it wakes: “Woke from snooze” · “Dismiss”. Sending a message wakes it.',
      good: 'Quiet, one line, and the composer stays usable.',
      cost: 'Easy to overlook.',
      mock: () => szPair(
        `<div>${szThread({ w: 520, above: szStatusLine('Snoozed until Wed 9:00', 'Wake Now') })}${szCaption('Snoozed')}</div>`,
        `<div>${szThread({ w: 520, above: szStatusLine('Woke from snooze', 'Dismiss') })}${szCaption('After it wakes')}</div>`,
      ),
    },
    {
      key: 'B', name: 'A callout, like an archived thread’s', from: 'agentZ (archived notice)',
      desc: 'An Info callout above the composer: “Snoozed until Wed 9:00”, “It comes back to the list then. Sending a message wakes it now.”, and Wake Now. Unlike an archived thread, the composer stays.',
      good: 'Matches the archived thread’s notice.',
      cost: 'Takes three lines over the composer.',
      mock: () => szThread({ above: szSnoozeCallout() }),
    },
    {
      key: 'C', name: 'A pill in the header', from: 'new',
      desc: 'A pill before the branch: an alarm clock and “Wed 9:00”. Its menu has Wake Now and Snooze ▸ to change the time.',
      good: 'Takes no room in the conversation; change the time in place.',
      cost: 'The header is already crowded; it hides in “⋯” when narrow.',
      mock: () => szThread({ header: szHeader(szHeaderPill()), overlays: szMenu([szItem('alarm-clock-off', 'Wake Now', { hl: true }), szItem('clock', 'Snooze', { arrow: true })], 'right:150px;top:34px;min-width:170px') }),
    },
  ],
});

// 8. Snooze and usage limits ------------------------------------------------------------------
const szSettingsRow = (name, description, control) => `<div class="row" style="gap:24px;padding:12px 16px;border-top:1px solid var(--bv)"><div class="grow col" style="gap:2px"><span style="font-size:13px">${name}</span><span class="sm mu">${description}</span></div>${control}</div>`;
const szLimitSettings = () => frame(`<div style="padding:16px 20px"><div class="sm mu" style="margin-bottom:8px">When a limit is reached</div><div style="border:1px solid var(--b);border-radius:8px;background:var(--panel)">${szSettingsRow('Continue at the reset', 'Sends “Continue.” to a thread its account’s limit stopped, once the limit resets.', '<span class="sz-sw on"><i></i></span>').replace('border-top:1px solid var(--bv)', '')}${szSettingsRow('Snooze stopped threads', 'Snoozes a thread its account’s limit stopped until the reported reset. With Continue at the reset, it wakes and goes on.', '<span class="sz-sw on"><i></i></span>')}</div></div>`, { w: 600, h: 200, style: 'border:1px solid var(--b);border-radius:8px' });

TOPICS.push({
  id: 'limit', section: 'Usage limits', title: 'Snoozing until a limit resets', size: 'wide', type: 'multi', rec: 'AB',
  now: 'When an account reaches a limit, the thread shows a warning callout: “Your account reached its 5-hour limit”, “Claude Agent stopped. The limit resets at 16:10, in 1h 28m.”, with “Continue at 16:10” (the server sends “Continue.” then), Usage ↗, and others when the account has them. An account can also continue at the reset on its own (“When a limit is reached”). The picture is recreated from the code.',
  nowImg: 'img/now-limit.png',
  issues: ['Snooze stays separate from continuing: waking doesn’t send a prompt, and continuing doesn’t need a snooze.'],
  options: [
    {
      key: 'A', name: 'On the limit notice', from: 't3code',
      desc: '“Snooze Until Reset” beside “Continue at 16:10”. Both can be used: the thread leaves the list and comes back at 16:10, when the server also sends “Continue.”.',
      good: 'Offered exactly when the thread can’t go on.', cost: 'A fifth button on a busy notice.',
      mock: () => szThread({ above: szLimitNotice(`<span class="btn sm">${ic('clock', 'xs')}Snooze Until Reset</span>`) }),
    },
    {
      key: 'B', name: 'In the snooze menu', from: 'backlog',
      desc: '“Until the limit resets” heads the snooze menu with its time, when the thread’s account has a used-up limit (the backlog’s “including its quota reset when known”).',
      good: 'The same menu everywhere, with the useful time in it.', cost: '—',
      mock: () => szMenuScene(szMenu(szPresetItems({ reset: true, hl: 'reset' }), 'left:70px;top:272px;min-width:220px')),
    },
    {
      key: 'C', name: 'Snooze stopped threads by themselves', from: 't3code',
      desc: 't3code’s “Snooze limited threads” as a switch beside continuing at the reset: a thread a limit stops is snoozed until the reset, and with Continue it wakes and goes on.',
      good: 'Stopped threads clear out of the list with no clicks.', cost: 'Threads disappear by themselves, which can surprise.',
      mock: () => szLimitSettings(),
    },
  ],
});
