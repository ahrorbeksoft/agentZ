// Pages to open and logins: a URL an MCP server or the agent sends you to, and an agent that
// needs a login before it can answer.

const LINEAR_URL = 'https://linear.app/oauth/authorize?client_id=8f3c2a&redirect_uri=http%3A%2F%2F127.0.0.1%3A33418%2Fcallback&scope=read%2Cwrite&state=Zk2q';
const urlRow = qRow('plug', `${qText('List issues')}${qDim('linear')}`, { trailing: qSpin() });
const userLinear = qBubble('List my Linear issues');
const urlBox = qTerm(LINEAR_URL, 'word-break:break-all');
const hostLine = `<div class="row g15" style="font-size:12px;color:var(--mu)"><span style="color:var(--ok);display:inline-flex">${ic('lock', 'xs')}</span>Opens <b style="color:var(--t);font-weight:500">linear.app</b></div>`;

// 8. A page to open ---------------------------------------------------------------------------
TOPICS.push({
  id: 'url', section: 'Pages and logins', title: 'A page to open', size: 'wide', rec: 'A',
  now: 'When a server needs you in the browser (linear’s sign-in here), its row spins over a card “Mock wants you to open a page”: the server’s message, “Opens linear.app” with a lock, the whole address in a code box, and Decline and “Open linear.app ↗”. Once opened, the card stays the same until the server goes on.',
  nowImg: 'img/now-url.png',
  issues: ['The long address is the biggest thing on the card, though only the host matters to most people.', 'After Open nothing changes: no sign it’s waiting for the browser, no way to open it again but the same button.', 'No way to copy the link to open it elsewhere (another browser, another machine).'],
  options: [
    { key: 'A', name: 'The host first, the address folded', from: 'Zed (its URL elicitation), shortened',
      desc: 'The card says who asks and why in one line (“linear needs you to sign in”), then the host big with its lock. The full address is folded behind “Show address”. Open, Copy link and Decline at the foot. Zed’s warning for a look-alike host stays.',
      good: 'The part to check is the part you see.', cost: 'The address’s details take a click.',
      mock: () => qConv(`${userLinear}${urlRow}${qCard({ icon: 'globe', title: '<b style="font-weight:500">linear</b> needs you to sign in', body: `<div class="row g2" style="font-size:15px;color:var(--t)"><span style="color:var(--ok);display:inline-flex">${ic('lock', 'sm')}</span>linear.app</div><div class="row g1" style="font-size:12px;color:var(--mu)">Show address${ic('chev-down', 'xs')}</div>`, foot: `${qBtn('Decline', { kind: 'ghost' })}<span class="grow"></span>${qBtn('Copy link', { icon: 'copy' })}${qBtn('Open linear.app', { kind: 'primary', icon: 'arrow-up-right', iconColor: '#fff' })}` })}`, 330) },
    { key: 'B', name: 'It waits for the browser', from: 'new',
      desc: 'After Open, the card says “Waiting for you in the browser”, with a spinner, Open again and Cancel, until the server goes on. Then it folds into the row (After you answer decides how).',
      good: 'You know the click worked and what’s left to do.', cost: 'Only the server knows when you’re done; a page you abandon waits until you cancel.',
      mock: () => qConv(`${userLinear}${urlRow}${qCard({ icon: 'globe', title: '<b style="font-weight:500">linear</b> needs you to sign in', body: `<div class="row g2" style="font-size:13px;color:var(--mu)">${qSpin('var(--ac)')}Waiting for you in the browser at <b style="color:var(--t);font-weight:500">linear.app</b>…</div>`, foot: `${qBtn('Cancel', { kind: 'ghost' })}<span class="grow"></span>${qBtn('Copy link', { icon: 'copy' })}${qBtn('Open again', { icon: 'arrow-up-right' })}` })}`, 290) },
    { key: 'C', name: 'In the row, no card', from: 'new',
      desc: 'No card: the tool’s row gets a second line, “linear needs you to sign in at linear.app”, with Open and Decline at its end.',
      good: 'Takes two lines in the thread.', cost: 'Easy to miss among the other rows.',
      mock: () => qConv(`${userLinear}${urlRow}<div class="row" style="margin-left:30px;gap:8px;font-size:13px;color:var(--mu);padding:2px 0 6px"><span class="grow"><b style="color:var(--t);font-weight:500">linear</b> needs you to sign in at <span style="color:var(--t)">linear.app</span></span>${qBtn('Decline', { kind: 'ghost', small: true })}${qBtn('Open', { kind: 'primary', small: true, icon: 'arrow-up-right', iconColor: '#fff' })}</div>`, 220) },
    { key: 'D', name: 'Today’s card', from: 'Zed',
      desc: 'As today and as Zed: the message, “Opens linear.app”, the whole address in its box, Decline and Open.',
      good: 'Everything in view; nothing new.', cost: 'The address crowds the card; nothing changes after Open.',
      mock: () => qConv(`${userLinear}${urlRow}${qCard({ icon: 'arrow-up-right', title: 'Mock wants you to open a page', body: `<div style="font-size:13px">linear needs you to sign in to Linear and allow access.</div>${hostLine}${urlBox}`, foot: `<span class="grow"></span>${qBtn('Decline', { kind: 'ghost' })}${qBtn('Open linear.app', { kind: 'primary', icon: 'arrow-up-right', iconColor: '#fff' })}` })}`, 360) },
  ],
});

// 9. When the agent needs a login -------------------------------------------------------------
const loginBar = (inner) => `<div class="row" style="margin:0 20px;padding:8px 10px;gap:8px;border:1px solid var(--b);border-bottom:none;border-radius:8px 8px 0 0;background:${QCARD};font-size:13px">${inner}</div>`;
const failedSend = `${qBubble('Run the tests')}<div class="row g1" style="justify-content:flex-end;font-size:12px;color:var(--del);margin-top:-6px">${ic('alert', 'xs')}Not sent</div>`;

TOPICS.push({
  id: 'login', section: 'Pages and logins', title: 'When the agent needs a login', size: 'wide', rec: 'A',
  now: 'When the agent answers a message with “authentication required”, the message is marked “Not sent” with Retry, a bar over the composer says “Mock needs a login” with “Log in…”, and the composer says “Log in to Mock to send a message”. Log in… opens the agent’s login panel, with its ways to log in.',
  nowImg: 'img/now-login.png',
  issues: ['The bar names no way to log in: it takes a click to learn there’s a browser login or an API key.', 'Your message is marked as failed; after logging in you have to press Retry.'],
  options: [
    { key: 'A', name: 'The ways to log in, in the bar', from: 'Zed (its authentication callout)',
      desc: 'The bar names the agent and offers each of its ways to log in as a button, as Zed’s “Authentication Required” callout does (“Log in with Claude”, “API key…”). Once logged in, your message is sent by itself.',
      good: 'One click to the right login; nothing to resend.', cost: 'An agent with five ways makes a crowded bar (the rest go in a ⋯ menu).',
      mock: () => qThread({ h: 360, conv: failedSend, dock: loginBar(`<span style="color:var(--warn);display:inline-flex">${ic('lock', 'sm')}</span><span class="grow">${AGENT} needs a login to answer</span>${qBtn('API key…', { small: true })}${qBtn('Log in with Claude', { kind: 'primary', small: true })}`), composer: qComposer({ placeholder: 'Your message is sent once you’re logged in', style: 'margin-top:0;border-radius:0 0 8px 8px' }) }) },
    { key: 'B', name: 'A card where it failed', from: 'Zed (in its thread)',
      desc: 'The login shows in the conversation, under your message, as a card with the agent’s ways to log in, as Zed puts its callout at the end of the thread. The composer stays as it is.',
      good: 'It’s where you look after sending.', cost: 'Scrolls away like any message.',
      mock: () => qThread({ h: 380, conv: `${qBubble('Run the tests')}${qCard({ icon: 'lock', iconColor: 'var(--warn)', title: `${AGENT} needs a login`, close: false, body: `<div style="font-size:13px;color:var(--mu)">Log in to send your message. It waits until you do.</div>`, foot: `<span class="grow"></span>${qBtn('API key…')}${qBtn('Log in with Claude', { kind: 'primary' })}` })}` }) },
    { key: 'C', name: 'Today’s bar, and the message waits', from: 'new',
      desc: 'Today’s bar and Log in…, but your message isn’t marked failed: it waits in the queue (“Waiting for the login”) and goes once you’re logged in.',
      good: 'A small change that removes the Retry.', cost: 'Still a click to see how to log in.',
      mock: () => qThread({ h: 360, conv: `${qBubble('Run the tests')}<div class="row g1" style="justify-content:flex-end;font-size:12px;color:var(--ph);margin-top:-6px">${ic('clock', 'xs')}Waiting for the login</div>`, dock: loginBar(`<span class="grow">Mock needs a login</span>${qBtn('Log in…', { kind: 'primary', small: true })}`), composer: qComposer({ placeholder: 'Log in to Mock to send a message', style: 'margin-top:0;border-radius:0 0 8px 8px' }) }) },
    { key: 'D', name: 'As today', from: 'agentZ today',
      desc: 'The bar, Log in…, and Retry on the message once you’re logged in.',
      good: 'Nothing new.', cost: 'Two extra clicks: the ways to log in, then Retry.',
      mock: () => qThread({ h: 360, conv: `${failedSend}<div class="row g1" style="justify-content:flex-end;font-size:12px;color:var(--ac)">Retry</div>`, dock: loginBar(`<span class="grow">Mock needs a login</span>${qBtn('Log in…', { kind: 'primary', small: true })}`), composer: qComposer({ placeholder: 'Log in to Mock to send a message', style: 'margin-top:0;border-radius:0 0 8px 8px' }) }) },
  ],
});
