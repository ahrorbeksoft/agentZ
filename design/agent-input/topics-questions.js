// Questions and forms: a question with choices, several at once, and an MCP server's form.

const askRow = (title = 'Asking for your input') => qRow('help-circle', qText(title), { trailing: qSpin() });
const recTag = qTag('Recommended', 'var(--ac)', 'rgba(84,138,247,.15)');
const qLabelText = (text) => `<div style="font-size:12px;color:var(--ph);margin-top:2px">${text}</div>`;
const otherToday = (hint = 'Type your own answer, or add a note to the option you chose above (optional).') => `${qLabelText('Other')}${qHelp(hint)}${qInput('')}`;
// Today's card for Claude Agent's question (elicitation_card.rs).
const questionToday = () => qCard({ title: `${AGENT} is asking`, body: `<div style="font-size:13px;color:var(--t);line-height:20px">${QUESTION}</div>${qLabelText('Approach')}${Q_OPTIONS.map(([label, desc, rec]) => qChoice({ label: rec ? `${label} (Recommended)` : label, desc })).join('')}${otherToday()}`, foot: qFoot() });
// A choice that answers when clicked: the label, its description, and its key or a check.
const pickRow = ({ label, desc = '', key = '', on = false, hover = false, rec = false }) => `<div class="row" style="align-items:flex-start;gap:9px;padding:6px 8px;border-radius:6px;${on ? 'background:var(--sel);' : hover ? 'background:var(--hov);' : ''}"><span class="col grow" style="gap:1px;min-width:0"><span class="row g2" style="font-size:13px;color:var(--t);line-height:20px;font-weight:500">${label}${rec ? recTag : ''}</span>${desc ? `<span style="font-size:12px;color:var(--ph);line-height:17px">${desc}</span>` : ''}</span>${on ? `<span style="color:var(--ac);display:inline-flex;padding-top:3px">${ic('check', 'xs')}</span>` : key ? `<span style="font-size:11px;color:var(--ph);padding-top:3px">${key}</span>` : ''}</div>`;
const somethingElse = (open = false) => open
  ? `<div class="col" style="gap:6px;padding:6px 8px;border-radius:6px;background:var(--hov)"><span class="row g2" style="font-size:13px;color:var(--t);font-weight:500">${ic('square-pen', 'xs')}Something else</span><span class="row g2">${qInput('Mock it in the board, but keep Zed’s icons', { focus: true })}${qBtn('Send', { kind: 'primary' })}</span></div>`
  : `<div class="row g2" style="padding:6px 8px;font-size:13px;color:var(--mu)">${ic('square-pen', 'xs')}Something else…</div>`;

// 5. A question with choices ------------------------------------------------------------------
TOPICS.push({
  id: 'question', section: 'Questions and forms', title: 'A question with choices', size: 'wide', rec: 'A',
  now: 'Claude Agent’s questions come as a form: a row “Asking for your input” with a spinner, then a card “Claude Agent is asking” with a ×. In it the question, its header (“Approach”) as a small label, each choice as a radio with its description under it (“(Recommended)” is part of the label), an “Other” field with a long hint, and “⏎ to submit”, Decline and Submit at the foot.',
  nowImg: 'img/now-question.png',
  issues: ['Two titles say the same thing: the row’s “Asking for your input” and the card’s “Claude Agent is asking”.', 'Answering takes two clicks, the radio then Submit; “(Recommended)” is plain text.', 'The Other field is always open under the choices, with a two-line hint.'],
  options: [
    { key: 'A', name: 'One click answers', from: 't3code (its pending input panel)',
      desc: 'Each choice is a row you click, its description under it, as t3code’s. On a question with one answer, the click answers it: no radio, no Submit. Its key (1, 2…) sits at the right, and a check once picked. “Recommended” is a tag. The last row, “Something else…”, opens a field with Send for an answer of your own. The card has one title, the question’s header (“Approach”), and Decline as its ×.',
      good: 'One click for the common case; the agent’s pick stands out.', cost: 'A misclick answers at once (t3code waits 200 ms, then sends).',
      mock: () => qConv(`${userPort}${qCard({ icon: 'help-circle', title: `<span style="color:var(--mu)">Approach</span>&nbsp; ${AGENT} asks`, body: `<div style="font-size:13px;color:var(--t);line-height:20px">${QUESTION}</div><div class="col" style="gap:2px">${Q_OPTIONS.map(([label, desc, rec], index) => pickRow({ label, desc, rec, key: index + 1, hover: index === 0 })).join('')}${somethingElse()}</div>` })}${qGen('', 'Working for 27s')}`, 400) },
    { key: 'B', name: 'Today’s card, tidied', from: 'Zed (its elicitation card)',
      desc: 'The card stays as it is, with radios and Submit, as Zed’s. The “Asking for your input” row goes, so the card is the one title. “Recommended” becomes a tag, and Other is a closed “Other…” line that opens its field when clicked.',
      good: 'Smallest change; you can still change your mind before Submit.', cost: 'Still two clicks.',
      mock: () => qConv(`${userPort}${qCard({ title: `${AGENT} is asking`, body: `<div style="font-size:13px;color:var(--t);line-height:20px">${QUESTION}</div>${qLabelText('Approach')}${Q_OPTIONS.map(([label, desc, rec], index) => qChoice({ label, desc, rec, on: index === 0 })).join('')}<div class="row g1" style="font-size:12px;color:var(--mu);padding:2px 6px">${ic('plus', 'xs')}Other…</div>`, foot: qFoot() })}`, 400) },
    { key: 'C', name: 'Docked over the composer', from: 't3code (its pending input panel)',
      desc: 'The question sits over the composer, where t3code puts it, and the thread shows only the row. Its header can fold it to one line to read the thread. The choices answer with a click or their key; typing in the composer and sending is the answer of your own.',
      good: 'Always in view, however far you scroll; your own answer goes in the box you write in.', cost: 'Takes room from the thread while it waits; far from what the agent wrote before it.',
      mock: () => qThread({ h: 470, conv: `${userPort}${askRow()}`, dock: `<div style="border:1px solid var(--b);border-bottom:none;border-radius:8px 8px 0 0;background:${QCARD};padding:8px 10px 6px"><div class="row g2" style="font-size:12px;color:var(--mu);margin-bottom:4px">${ic('help-circle', 'xs')}<span class="b5">Approach</span><span class="grow"></span>${ic('chev-down', 'xs')}${ic('x', 'xs')}</div><div style="font-size:13px;line-height:20px;margin-bottom:4px">${QUESTION}</div>${Q_OPTIONS.map(([label, desc, rec], index) => pickRow({ label, desc, rec, key: index + 1 })).join('')}</div>`, composer: qComposer({ placeholder: 'Or type your own answer', style: 'margin-top:0;border-radius:0 0 8px 8px' }) }) },
    { key: 'D', name: 'A numbered list', from: 'Claude Code (its terminal)',
      desc: 'As Claude Code asks in its terminal: the question, then its choices numbered, the first highlighted, the last “Type something.” for an answer of your own. ↑ ↓ move, ⏎ or a number answers. A click answers too.',
      good: 'The keyboard does it all; looks the same as the permission topic’s D.', cost: 'Descriptions are dimmer lines between the numbers; reads as a menu.',
      mock: () => qConv(`${userPort}${qCard({ icon: 'help-circle', title: `${AGENT} asks: Approach`, body: `<div style="font-size:13px;color:var(--t);line-height:20px">${QUESTION}</div><div class="col" style="gap:2px">${[...Q_OPTIONS, ['Type something.', '']].map(([label, desc], index) => `<div class="row" style="align-items:flex-start;gap:8px;padding:4px 8px;border-radius:5px;${index === 0 ? 'background:rgba(84,138,247,.14);' : ''}"><span style="width:10px;color:var(--ac)">${index === 0 ? '›' : ''}</span>${qKey(index + 1)}<span class="col grow"><span style="font-size:13px;color:${index === 0 ? 'var(--t)' : 'var(--mu)'}">${label}</span>${desc ? `<span style="font-size:12px;color:var(--ph);line-height:17px">${desc}</span>` : ''}</span></div>`).join('')}</div><div style="font-size:11px;color:var(--ph)">↑ ↓ to choose · ⏎ to answer · Esc to decline</div>` })}`, 420) },
    { key: 'E', name: 'Choices as buttons', from: 'new',
      desc: 'Short choices become buttons at the card’s foot, the recommended one filled, as a permission’s are in the first topic’s A. Their descriptions show as a list above them, each under its choice’s name in bold. “Other…” is the last button and opens a field.',
      good: 'Questions and permissions look alike.', cost: 'Long choice names make long buttons; descriptions sit apart from their buttons.',
      mock: () => qConv(`${userPort}${qCard({ icon: 'help-circle', title: `${AGENT} asks: Approach`, body: `<div style="font-size:13px;color:var(--t);line-height:20px">${QUESTION}</div>${Q_OPTIONS.map(([label, desc]) => `<div style="font-size:12px;line-height:17px;color:var(--ph)"><b style="color:var(--mu);font-weight:500">${label}:</b> ${desc}</div>`).join('')}`, foot: `${qBtn('Decline', { kind: 'ghost' })}<span class="grow"></span>${qBtn('Other…')}${qBtn(Q_OPTIONS[1][0])}${qBtn(Q_OPTIONS[0][0], { kind: 'primary' })}` })}`, 340) },
  ],
});

// 6. Several questions at once ------------------------------------------------------------------
const RUNNERS = [['Vitest', 'Already used by src/cart: fast, runs in watch mode.'], ['Jest', 'What the rest of the app uses.']];
const CASES = [['Empty cart', ''], ['Discounts', 'Percent and fixed amounts.'], ['Rounding', 'Totals that end in half a cent.'], ['Currencies', '']];
const userTests = qBubble('Write tests for the total');
const stepDots = (index, count) => `<span class="row" style="gap:4px">${Array.from({ length: count }, (_, step) => `<i style="width:${step === index ? 14 : 6}px;height:6px;border-radius:3px;background:${step <= index ? 'var(--ac)' : '#4a4d52'};display:inline-block"></i>`).join('')}</span>`;

TOPICS.push({
  id: 'questions', section: 'Questions and forms', title: 'Several questions at once', size: 'wide', rec: 'A',
  now: 'When the agent asks several questions in one go (Claude Agent asks up to four), the card lists them one after another: each one’s header, question, choices (radios for one answer, checkboxes for several) and its own Other field with its hint. Submit at the very end sends them all.',
  nowImg: 'img/now-questions.png',
  issues: ['A tall card: two questions already fill the thread, and Submit is at the bottom of it.', 'Each question repeats the Other field and its hint.', 'Nothing says how many questions there are or which are still unanswered.'],
  options: [
    { key: 'A', name: 'One at a time', from: 't3code (its question steps)',
      desc: 'The card shows one question, with “1 of 2” and Back at its top. A one-answer question goes on to the next when clicked (A in the topic before); several answers take Next. The last one’s button is Submit, which sends them all.',
      good: 'A short card whatever the count; each question gets full attention.', cost: 'You don’t see the later questions until you get there.',
      mock: () => qConv(`${userTests}${qCard({ icon: 'help-circle', title: `<span style="color:var(--mu)">Cases</span>&nbsp; ${AGENT} asks`, body: `<div class="row g2" style="font-size:12px;color:var(--ph)">${ic('chev-left', 'xs')}Back<span class="grow"></span>${stepDots(1, 2)}<span>2 of 2</span></div><div style="font-size:13px;line-height:20px">Which cases should the tests cover? <span style="color:var(--ph)">Pick any.</span></div>${CASES.map(([label, desc], index) => qChoice({ kind: 'check', label, desc, on: index < 3 })).join('')}`, foot: qFoot(`<span class="sm" style="color:var(--ph)">Test runner: Vitest</span>`, [qBtn('Decline', { kind: 'ghost' }), qBtn('Submit', { kind: 'primary' })]) })}`, 440) },
    { key: 'B', name: 'All in one card, numbered', from: 'Zed (its elicitation card)',
      desc: 'Today’s card, with each question numbered (“1 Test runner”), a line between them, and one Other line at the end for a note on any of them. Submit shows how many are answered (“Submit 1 of 2”) until all are.',
      good: 'Everything in view at once, as a form.', cost: 'Still tall with four questions.',
      mock: () => qConv(`${userTests}${qCard({ title: `${AGENT} is asking 2 questions`, body: `<div class="row g2" style="font-size:12px;color:var(--mu)">${qKey(1)}Test runner</div>${RUNNERS.map(([label, desc], index) => qChoice({ label, desc, on: index === 0 })).join('')}<div style="border-top:1px solid var(--b);margin:2px 0"></div><div class="row g2" style="font-size:12px;color:var(--mu)">${qKey(2)}Cases</div>${CASES.map(([label, desc]) => qChoice({ kind: 'check', label, desc })).join('')}<div class="row g1" style="font-size:12px;color:var(--mu);padding:2px 6px">${ic('plus', 'xs')}Add a note…</div>`, foot: qFoot(undefined, [qBtn('Decline', { kind: 'ghost' }), qBtn('Submit 1 of 2', { kind: 'primary', style: 'opacity:.6' })]) })}`, 520) },
    { key: 'C', name: 'Tabs, and a last look', from: 'Claude Code (its terminal)',
      desc: 'As Claude Code asks several questions: a tab for each, named by its header, and a last “Submit” tab listing your answers to check before you send them. ← → move between tabs; a tab you answered gets a check.',
      good: 'You can jump to any question and see every answer before sending.', cost: 'One more step; tabs are small targets.',
      mock: () => qConv(`${userTests}${qCard({ icon: 'help-circle', title: `${AGENT} asks`, body: `<div class="row" style="gap:4px;border-bottom:1px solid var(--b);padding-bottom:6px">${[['Test runner', true, false], ['Cases', true, false], ['Submit', false, true]].map(([name, done, on]) => `<span class="row g1" style="height:24px;padding:0 9px;border-radius:5px;font-size:12px;${on ? 'background:var(--sel);color:var(--t)' : 'color:var(--mu)'}">${done ? `<span style="color:var(--ok);display:inline-flex">${ic('check', 'xs')}</span>` : ''}${name}</span>`).join('')}</div><div style="font-size:13px">Review your answers</div><div class="col" style="gap:4px;font-size:13px"><div><span style="color:var(--ph)">Test runner</span>&nbsp; Vitest</div><div><span style="color:var(--ph)">Cases</span>&nbsp; Empty cart, Discounts, Rounding</div></div>`, foot: qFoot(`<span class="sm" style="color:var(--ph)">← → tabs</span>`, [qBtn('Decline', { kind: 'ghost' }), qBtn('Submit answers', { kind: 'primary' })]) })}`, 340) },
    { key: 'D', name: 'Over the composer, one at a time', from: 't3code (its pending input panel)',
      desc: 'C of the topic before, one question at a time: “1/2” in the panel’s header, a click on a one-answer question goes to the next, and sending from the composer answers the current one in your words.',
      good: 'In view while you scroll, and short.', cost: 'Takes room from the thread; the questions are away from the agent’s words.',
      mock: () => qThread({ h: 470, conv: `${userTests}${askRow()}`, dock: `<div style="border:1px solid var(--b);border-bottom:none;border-radius:8px 8px 0 0;background:${QCARD};padding:8px 10px 6px"><div class="row g2" style="font-size:12px;color:var(--mu);margin-bottom:4px">${ic('help-circle', 'xs')}<span class="b5">Cases</span><span class="grow"></span><span>2/2</span>${ic('chev-down', 'xs')}${ic('x', 'xs')}</div><div style="font-size:13px;line-height:20px">Which cases should the tests cover?</div><div style="font-size:12px;color:var(--ph);margin-bottom:4px">Select one or more options.</div>${CASES.map(([label, desc], index) => pickRow({ label, desc, key: index + 1, on: index < 2 })).join('')}</div>`, composer: qComposer({ placeholder: 'Or type your own answer', style: 'margin-top:0;border-radius:0 0 8px 8px', send: qBtn('Submit', { kind: 'primary', small: true }) }) }) },
  ],
});

// 7. A form to fill in -----------------------------------------------------------------------
const formRow = qRow('plug', `${qText('Create issue')}${qDim('github')}`, { trailing: qSpin() });
const formFields = ({ error = false } = {}) => `${qChoice({ kind: 'check', on: true, label: 'Assign to me' })}${qLabel('Estimate (days)')}${qInput('', { placeholder: '1–30' })}${qLabel('Priority')}<div class="row" style="gap:14px;font-size:13px">${['Low', 'Medium', 'High'].map((name, index) => `<span class="row g15">${qRadio(index === 0)}${name}</span>`).join('')}</div>${qLabel(`Title${error ? '<span style="color:var(--del)">&nbsp;*</span>' : ''}`)}${qHelp('A short summary.')}${qInput('', { error })}${error ? qErr('Title is required') : ''}`;
const userIssue = qBubble('File an issue for it');

TOPICS.push({
  id: 'form', section: 'Questions and forms', title: 'A form to fill in', size: 'wide', type: 'multi', rec: 'ABD',
  now: 'An MCP server can ask for details through the agent (an elicitation), as github does here before it creates an issue. Its tool’s row, “Create issue github”, spins over a card titled “Mock is asking”: the server’s message, then each field with its label above it (a checkbox, a number, checkboxes and radios in a row, text with its description). Submitting with a problem marks the field red with the reason under it.',
  nowImg: 'img/now-form.png',
  issues: ['The card says the agent asks, but github does; nothing says who gets what you type.', 'Nothing marks a required field until you submit.', 'Every field takes a full line, so a short form is tall.'],
  options: [
    { key: 'A', name: 'Say who asks', from: 'new (from what the request carries)',
      desc: 'When the request comes from an MCP server, the card names it: the plug, “github asks, through Claude Agent”. The agent’s own forms keep “Claude Agent asks”.',
      good: 'You know who gets the answers.', cost: 'Only when the agent passes the server’s name on (Claude Agent and Codex do).',
      mock: () => qConv(`${userIssue}${formRow}${qCard({ icon: 'plug', iconColor: 'var(--mu)', title: `<b style="font-weight:500">github</b> asks, through ${AGENT}`, body: `<div style="font-size:13px">Details for the new issue.</div>${formFields()}`, foot: qFoot() })}`, 520) },
    { key: 'B', name: 'Required fields marked', from: 'Zed (its required fields)',
      desc: 'A required field’s label ends in a red *, and Submit stays dim until each one is filled. A wrong value shows its reason when you leave the field, not only when you submit.',
      good: 'No failed Submit to learn what’s missing.', cost: 'A dim Submit can look broken if the * is missed.',
      mock: () => qConv(`${userIssue}${formRow}${qCard({ title: `${AGENT} is asking`, body: `<div style="font-size:13px">github wants details for the new issue.</div>${formFields({ error: true })}`, foot: qFoot(undefined, [qBtn('Decline', { kind: 'ghost' }), qBtn('Submit', { kind: 'primary', style: 'opacity:.5' })]) })}`, 540) },
    { key: 'C', name: 'Short fields side by side', from: 'new',
      desc: 'Short fields (numbers, yes or no, a few choices) sit two to a line; text fields and long lists keep a line of their own.',
      good: 'A short form is half as tall.', cost: 'A narrow thread falls back to one a line.',
      mock: () => qConv(`${userIssue}${formRow}${qCard({ title: `${AGENT} is asking`, body: `<div style="font-size:13px">github wants details for the new issue.</div><div class="row" style="gap:16px;align-items:flex-start"><div class="col grow" style="gap:4px">${qLabel('Estimate (days)')}${qInput('', { placeholder: '1–30' })}</div><div class="col grow" style="gap:6px">${qLabel('Priority')}<div class="row" style="gap:12px;font-size:13px;height:30px">${['Low', 'Medium', 'High'].map((name, index) => `<span class="row g15">${qRadio(index === 0)}${name}</span>`).join('')}</div></div></div>${qChoice({ kind: 'check', on: true, label: 'Assign to me' })}${qLabel('Title')}${qInput('')}`, foot: qFoot() })}`, 420) },
    { key: 'D', name: 'Long choices in a dropdown', from: 'Zed (its select fields)',
      desc: 'A choice of more than five (a repository, an assignee) is a dropdown with search, as Zed’s select fields, instead of a wall of radios.',
      good: 'A form with a long list stays short.', cost: 'One more click to see the choices.',
      mock: () => qConv(`${userIssue}${formRow}${qCard({ title: `${AGENT} is asking`, style: 'position:relative;overflow:visible', body: `<div style="font-size:13px">github wants details for the new issue.</div>${qLabel('Assignee')}${qInput('Ahrorbek', { trailing: ic('chev-down', 'xs') })}<div class="menu" style="position:absolute;left:12px;right:12px;top:110px;font-size:12px"><div class="it" style="height:24px">${ic('search', 'xs')}<span style="color:var(--ph)">Search 14 people…</span></div><div class="hr"></div>${['Ahrorbek', 'Dana', 'Lee', 'Sam'].map((name, index) => `<div class="it" style="height:24px;${index === 0 ? 'background:var(--hov)' : ''}">${index === 0 ? ic('check', 'xs') : '<span style="width:12px"></span>'}${name}</div>`).join('')}</div><div style="height:150px"></div>`, foot: qFoot() })}`, 460) },
    { key: 'E', name: 'Docked over the composer', from: 't3code (where it asks)',
      desc: 'The form sits over the composer, as t3code places its questions, scrolling inside when it’s long; the thread keeps only the row.',
      good: 'In view however far you scroll.', cost: 'A long form covers much of the thread.',
      mock: () => qThread({ h: 520, conv: `${userIssue}${formRow}`, dock: `<div style="border:1px solid var(--b);border-bottom:none;border-radius:8px 8px 0 0;background:${QCARD};padding:8px 12px"><div class="row g2" style="font-size:12px;color:var(--mu);margin-bottom:6px">${ic('plug', 'xs')}<span class="b5">github</span> wants details for the new issue<span class="grow"></span>${ic('x', 'xs')}</div><div class="col" style="gap:6px">${formFields()}</div></div>`, composer: qComposer({ placeholder: '', style: 'margin-top:0;border-radius:0 0 8px 8px', send: qBtn('Submit', { kind: 'primary', small: true }) }) }) },
  ],
});
