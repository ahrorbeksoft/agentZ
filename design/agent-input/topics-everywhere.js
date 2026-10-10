// What every kind of request shares: what stays after you answer, requests from subagents and
// with no row of their own, the keyboard, and how the app shows that an agent waits for you.

const answerLine = (icon, color, html) => `<div class="row g15" style="margin-left:30px;font-size:12px;color:var(--ph);padding:0 0 4px"><span style="display:inline-flex;color:${color}">${ic(icon, 'xs')}</span>${html}</div>`;
const ranRow = qRow('terminal', `${qVerb('Ran')}${qCode('npm test')}`);
const askedRow = qRow('help-circle', qText('Asked: Approach'));

// 10. After you answer ------------------------------------------------------------------------
TOPICS.push({
  id: 'answered', section: 'Every request', title: 'After you answer', size: 'wide', rec: 'A',
  now: 'Once you answer, the card or the choices go away. A permission’s row carries on as the tool runs (“Ran npm test”); a question’s row stays “Asking for your input”, and a plan’s says “Exited Plan Mode”. Only the agent’s next words, if it writes them, say what you chose (“You chose “Yes”.”).',
  nowImg: 'img/now-answered-permission.png',
  issues: ['Scrolling back, nothing shows what you allowed, denied or answered.', 'A question’s row keeps saying it’s asking after it has its answer.'],
  options: [
    { key: 'A', name: 'A line with your answer', from: 'new',
      desc: 'Under the row, one dim line says what you did, with its mark: “You allowed it once”, “You allowed npm test commands from now on”, “You denied it”, “You answered: Design round first”. A question’s row turns to “Asked: Approach”.',
      good: 'The thread records every decision in place.', cost: 'One more line for each request.',
      mock: () => qConv(`${userRun}${agentRun}${ranRow}${answerLine('check', 'var(--mu)', 'You allowed it once')}${qPara('All 12 tests pass.')}${userPort}${askedRow}${answerLine('check', 'var(--ac)', 'You answered: <span style="color:var(--mu)">Design round first</span>')}${qPara('I’ll make a short round on the board.')}`, 420) },
    { key: 'B', name: 'The card folds to one line', from: 'new',
      desc: 'The card stays, folded to one line with your answer at its end (“Approach · Design round first ✓”). Clicking it opens the card as you left it, read-only.',
      good: 'You can go back to the whole question and its choices.', cost: 'Folded cards add a box per request to the thread.',
      mock: () => qConv(`${userRun}${agentRun}${ranRow}<div style="margin-left:30px">${qCard({ head: `<div class="row" style="height:30px;padding:0 10px;gap:8px;font-size:12px;color:var(--mu)"><span style="color:var(--warn);display:inline-flex">${ic('shield', 'xs')}</span><span class="grow">Run npm test</span><span style="color:var(--t)">Yes</span><span style="color:var(--ok);display:inline-flex">${ic('check', 'xs')}</span></div>` })}</div>${userPort}${askedRow}<div style="margin-left:30px">${qCard({ head: `<div class="row" style="height:30px;padding:0 10px;gap:8px;font-size:12px;color:var(--mu)"><span style="display:inline-flex;color:var(--ac)">${ic('help-circle', 'xs')}</span><span class="grow">Approach</span><span style="color:var(--t)">Design round first</span>${ic('chev-down', 'xs')}</div>` })}</div>`, 360) },
    { key: 'C', name: 'A tag at the row’s end', from: 'new',
      desc: 'The row gets a small tag at its end: “Allowed”, “Always allowed”, “Denied” in red, “Answered”. Hovering the tag shows the choice in the agent’s words.',
      good: 'No extra lines.', cost: 'A question’s answer needs a hover to read.',
      mock: () => qConv(`${userRun}${qRow('terminal', `${qVerb('Ran')}${qCode('npm test')}`, { trailing: qTag('Allowed', 'var(--mu)') })}${qRow('terminal', `${qVerb('Run')}${qCode('rm -rf node_modules')}`, { trailing: qTag('Denied', 'var(--del)', 'rgba(250,102,117,.12)') })}${userPort}${qRow('help-circle', qText('Asked: Approach'), { trailing: qTag('Answered', 'var(--ac)', 'rgba(84,138,247,.12)') })}`, 260) },
    { key: 'D', name: 'As today', from: 'Zed',
      desc: 'The request goes away once answered, as in Zed; the agent’s words say what happened, when it writes them.',
      good: 'A short thread.', cost: 'The decisions aren’t recorded.',
      mock: () => qConv(`${userRun}${agentRun}${ranRow}${qPara('You chose “Yes”. All 12 tests pass.')}${userPort}${askRow()}${qPara('Got it: Design round first.')}`, 300) },
  ],
});

// 11. Requests from subagents, and with no row -----------------------------------------------
const subagentRow = (trailing = qSpin()) => qRow('bot', `${qBright('Find where the login view is drawn')}${qTag('Explore')}`, { trailing: `${qOpen()}${trailing}` });
const envRow = qRow('pencil', `${qVerb('Edit')}${qCode('.env')}`);
const envCard = (extra = '') => permCard({ title: `${extra}${AGENT} wants to edit a file`, body: qDiff([['-', 'API_URL=http://localhost:3000'], ['+', 'API_URL=https://staging.acme.dev']]), options: [['once', 'Allow once'], ['reject', 'Deny']] });

TOPICS.push({
  id: 'elsewhere', section: 'Every request', title: 'Requests from subagents, and with no row', size: 'wide', rec: 'A',
  now: 'A subagent’s request shows inside its row, which opens by itself to it: the subagent’s step (“Edit .env”) and its choices as lines. A request for a tool call the agent never showed gets a row of its own, with its choices under it. Under the turn, “Awaiting Confirmation” shows either way.',
  nowImg: 'img/now-permission-subagent.png',
  issues: ['A subagent’s request is nested two levels deep, in a smaller box, among its steps.', 'With two subagents at work, the request can sit above the fold of the thread.'],
  options: [
    { key: 'A', name: 'At the end of the thread, saying who asks', from: 'new',
      desc: 'A subagent’s request shows at the end of the thread, as the agent’s would, in the same card, its header naming the subagent (“Find where the login view is drawn wants to edit a file”). The subagent’s row shows “Waiting for you” in place of its step.',
      good: 'Every request is where you look, the same however deep it comes from.', cost: 'The request is away from the subagent’s other steps.',
      mock: () => qConv(`${qBubble('Look into the login view')}${subagentRow(qTag('Waiting for you', 'var(--warn)', 'rgba(224,180,92,.12)'))}${qRow('bot', `${qBright('Find how Add Account logs in')}${qTag('Explore')}`, { trailing: `${qOpen()}${qSpin()}` })}${envCard('<span style="color:var(--mu)">Find where the login view is drawn</span> · ')}${qGen('Awaiting Confirmation', 'Working for 27s')}`, 400) },
    { key: 'B', name: 'In the subagent’s row, as a card', from: 'agentZ today, with topic 1’s card',
      desc: 'As today, inside the subagent’s opened row, but in the same card as any request, full width.',
      good: 'Stays with what the subagent was doing.', cost: 'Still deep in the thread; may be scrolled out of view.',
      mock: () => qConv(`${qBubble('Look into the login view')}${subagentRow()}<div style="margin-left:30px;border-left:1px solid var(--b);padding-left:10px">${qRow('search', `${qVerb('Search for')}${qCode('"render_centered"')}`)}${envRow}${envCard()}</div>${qRow('bot', `${qBright('Find how Add Account logs in')}${qTag('Explore')}`, { trailing: `${qOpen()}${qSpin()}` })}`, 420) },
    { key: 'C', name: 'Over the composer, one at a time', from: 't3code (its approval panel)',
      desc: 'Every request, whoever asks, waits over the composer, as t3code’s approvals do: one at a time, “1 of 2” when more wait, naming who asks. The thread keeps only the rows.',
      good: 'Always in view; several requests line up.', cost: 'Far from the step that asked; covers the end of the thread.',
      mock: () => qThread({ h: 470, conv: `${qBubble('Look into the login view')}${subagentRow()}${qRow('bot', `${qBright('Find how Add Account logs in')}${qTag('Explore')}`, { trailing: `${qOpen()}${qSpin()}` })}`, dock: `<div style="border:1px solid ${WARN_BORDER};border-bottom:none;border-radius:8px 8px 0 0;background:${QCARD};padding:8px 10px"><div class="row g2" style="font-size:12px;color:var(--mu);margin-bottom:6px"><span style="color:var(--warn);display:inline-flex">${ic('shield', 'xs')}</span><span class="grow"><b style="color:var(--t);font-weight:500">Find where the login view is drawn</b> wants to edit .env</span><span>1 of 2</span></div>${qDiff([['-', 'API_URL=http://localhost:3000'], ['+', 'API_URL=https://staging.acme.dev']])}<div class="row" style="gap:6px;justify-content:flex-end;margin-top:8px">${qBtn('Deny', { icon: 'x', iconColor: 'var(--del)' })}${qBtn('Allow once', { kind: 'primary', icon: 'check', iconColor: '#fff' })}</div></div>`, composer: qComposer({ style: 'margin-top:0;border-radius:0 0 8px 8px' }) }) },
    { key: 'D', name: 'As today', from: 'agentZ today',
      desc: 'Inside the subagent’s row, as lines; a request with no row gets one.',
      good: 'Nothing new.', cost: 'Easy to miss.',
      mock: () => qConv(`${qBubble('Look into the login view')}${subagentRow()}<div style="margin-left:30px;border:1px solid var(--b);border-radius:6px;padding:4px 6px">${envRow}${qTodayButtons([['once', 'Allow once'], ['reject', 'Deny']])}</div>${qRow('bot', `${qBright('Find how Add Account logs in')}${qTag('Explore')}`, { trailing: `${qOpen()}${qSpin()}` })}${qGen('Awaiting Confirmation', 'Working for 27s')}`, 330) },
  ],
});

// 12. Answering from the keyboard ---------------------------------------------------------------
const keyHint = (keys) => `<div class="row" style="gap:12px;font-size:11px;color:var(--ph);margin-top:2px">${keys.map(([key, text]) => `<span class="row g1">${qKey(key)}${text}</span>`).join('')}</div>`;

TOPICS.push({
  id: 'keys', section: 'Every request', title: 'Answering from the keyboard', size: 'wide', type: 'multi', rec: 'AB',
  now: 'A form submits with ⏎ while one of its fields has focus (“⏎ to submit”). A permission’s choices and a question’s have no keys; nothing takes focus when a request comes, so the composer keeps it, and answering takes the mouse.',
  nowImg: 'img/now-permission.png',
  issues: ['A request in a thread you’re typing in needs the mouse.', 'Nothing says which keys would work.'],
  options: [
    { key: 'A', name: 'Zed’s keys for permissions', from: 'Zed (its permission keys)',
      desc: 'While the thread has focus, ⌘Y allows once, ⌘⌥Y picks the first “always” choice and ⌘⌥Z the first No, as in Zed. Each shows on its button.',
      good: 'Works with the cursor in the composer; the same keys as Zed.', cost: 'Only three kinds; a sixth choice (Droid’s auto-run levels) has no key.',
      mock: () => qConv(`${runRow}${qCard({ head: shieldHead(`${AGENT} wants to run a command`), border: WARN_BORDER, body: `<div style="height:6px"></div>${cmdTerm()}`, foot: `<span class="grow"></span>${qBtn('No', { icon: 'x', iconColor: 'var(--del)', key: '⌘⌥Z' })}${qBtn('Yes, and don’t ask again', { key: '⌘⌥Y' })}${qBtn('Yes', { kind: 'primary', icon: 'check', iconColor: '#fff', key: '⌘Y' })}` })}`, 230) },
    { key: 'B', name: 'Number keys pick a choice', from: 't3code (its 1–9 keys)',
      desc: 'With the cursor outside a text field, 1–9 pick the request’s choices, as t3code’s questions take them. Each choice shows its number.',
      good: 'One key for any choice, permissions and questions alike.', cost: 'The cursor is usually in the composer, where numbers type.',
      mock: () => qConv(`${userPort}${qCard({ icon: 'help-circle', title: `<span style="color:var(--mu)">Approach</span>&nbsp; ${AGENT} asks`, body: `<div style="font-size:13px;line-height:20px">${QUESTION}</div><div class="col" style="gap:2px">${Q_OPTIONS.map(([label, desc, rec], index) => pickRow({ label, desc, rec, key: qKey(index + 1) })).join('')}</div>${keyHint([['1', 'Design round'], ['2', 'Port as is'], ['Esc', 'Decline']])}` })}`, 330) },
    { key: 'C', name: 'The request takes focus', from: 'new',
      desc: 'When a request comes in the thread you’re in and the composer is empty, focus moves to it: ↑ ↓ choose, ⏎ answers, Esc goes back to the composer. While you’re typing, it waits for Esc or ⌘⏎.',
      good: 'Answer with no mouse and no chords.', cost: 'Focus that moves on its own can surprise; never while typing.',
      mock: () => qConv(`${runRow}${qCard({ head: shieldHead(`${AGENT} wants to run a command`), border: WARN_BORDER, style: 'box-shadow:0 0 0 2px rgba(53,116,240,.55)', body: `<div style="height:6px"></div>${cmdTerm()}${keyHint([['↑↓', 'choose'], ['⏎', 'answer'], ['Esc', 'back to the composer']])}`, foot: choiceButtons(CLAUDE_BASH) })}`, 260) },
    { key: 'D', name: 'Submit and Decline anywhere in a form', from: 'new',
      desc: 'In a form or a question, ⌘⏎ submits from any field (⏎ in a text field may need a new line), and Esc declines.',
      good: 'The same two keys for every form.', cost: 'Esc declining by accident loses the answers typed.',
      mock: () => qConv(`${userIssue}${formRow}${qCard({ title: `${AGENT} is asking`, body: `<div style="font-size:13px">github wants details for the new issue.</div>${qLabel('Title')}${qInput('Checkout total rounds twice', { focus: true })}`, foot: qFoot(`${qKey('⌘⏎')}<span class="sm" style="color:var(--ph)">to submit</span>${qKey('Esc')}<span class="sm" style="color:var(--ph)">to decline</span>`) })}`, 300) },
  ],
});

// 13. Showing that the agent waits --------------------------------------------------------------
TOPICS.push({
  id: 'waiting', section: 'Every request', title: 'Showing that the agent waits', size: 'wide', type: 'multi', rec: 'AD',
  now: 'A thread waiting on you says so on its sidebar card: “Pending Approval” in yellow for a permission, “Awaiting Input” in purple for a question or form. In the thread, “Awaiting Confirmation” shows before “Working for 23s” under the turn, and the request is wherever it is in the conversation, maybe scrolled out of view.',
  nowImg: 'img/now-sidebar-pending.png',
  issues: ['Scrolled up in a long thread, nothing near the composer says the agent waits.', '“Awaiting Confirmation” is used for questions and forms too, which confirm nothing.'],
  options: [
    { key: 'A', name: 'A pill over the composer', from: 'new',
      desc: 'While a request is out of view, a pill over the composer says “Claude Agent is waiting for you” with a ↓; clicking it scrolls to the request.',
      good: 'You never miss a request while reading back.', cost: 'One more floating control.',
      mock: () => qThread({ h: 330, conv: `${qPara('The total sums each line, then rounds the result once. Rounding each line first made totals one cent off the receipt.')}${qPara('I’ll add tests for an empty cart, a discount and rounding next.')}<div style="display:flex;justify-content:center;margin-top:8px"><span class="row g15" style="height:26px;padding:0 12px;border-radius:13px;background:rgba(224,180,92,.16);border:1px solid ${WARN_BORDER};font-size:12px;color:var(--warn)">${ic('shield', 'xs')}${AGENT} is waiting for you${ic('chev-down', 'xs')}</span></div>` }) },
    { key: 'B', name: 'One state in the sidebar', from: 'new',
      desc: 'The sidebar shows one state for all of them, “Needs you”, in one color, since the next step is the same: open the thread and answer.',
      good: 'One color to look for.', cost: 'You can’t tell a permission from a question before opening it.',
      mock: () => qSidebar(`${qSideRow({ title: 'Run the tests', state: 'needs' })}${qSideRow({ title: 'Port the icon picker', state: 'needs' })}${qSideRow({ title: 'Plan the checkout total', state: 'done' })}`, { h: 260 }) },
    { key: 'C', name: 'A count on the app’s icon', from: 'new',
      desc: 'The Dock icon (the taskbar’s on Linux) shows how many threads wait for you, as mail apps count unread mail.',
      good: 'Seen from any app.', cost: 'Another badge; some find them noisy.',
      mock: () => frame(`<div style="position:absolute;inset:0;display:grid;place-items:center;background:#1b1c1f"><div class="row" style="gap:14px;padding:10px 16px;border-radius:16px;background:rgba(255,255,255,.08)">${['#4a6cf7', '#2f4548', '#6b4c9a'].map((color, index) => `<div style="position:relative;width:46px;height:46px;border-radius:11px;background:${color};display:grid;place-items:center;color:#fff;font:600 18px sans-serif">${index === 1 ? 'Z' : ''}${index === 1 ? '<span style="position:absolute;top:-6px;right:-6px;min-width:20px;height:20px;border-radius:10px;background:#e5484d;color:#fff;font:600 12px/20px sans-serif;text-align:center">2</span>' : ''}</div>`).join('')}</div></div>`, { w: 300, h: 140, style: QT }) },
    { key: 'D', name: 'The turn’s line says what it waits for', from: 'new',
      desc: 'Under the turn, the line says what the agent waits for, in yellow: “Waiting for your permission”, “Waiting for your answer”, “Waiting for you in the browser”, in place of “Awaiting Confirmation”.',
      good: 'Right words for each kind.', cost: 'Only seen at the end of the thread.',
      mock: () => qConv(`${userPort}${askRow()}${qGen('Waiting for your answer', 'Working for 27s', 'var(--warn)')}${userRun}${runRow}${qGen('Waiting for your permission', 'Working for 12s', 'var(--warn)')}`, 260) },
  ],
});
