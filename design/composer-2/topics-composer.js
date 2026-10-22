// The thread's composer: its shape, its footer's controls and its Send button.
// agent_view.rs render_message_editor (ComposerStyle::Bar in a thread, ::Card on the new thread
// screen).

const C2_LONG_TALK = c2Para('The picker now ends each agent’s row with its usage, and a dot while one of its threads is working. I read the usage from the same place the composer’s gauge does, so they always agree.') +
  c2Para('Agents with several accounts show the account a switch would use: the thread’s own when it’s that agent, else the agent’s default. An account that can’t be read shows nothing, rather than a guess.') +
  c2Para('I also added tests for the row: one per state, and one for an agent whose usage read fails.') +
  `<div class="sm ph" style="padding:2px 0 8px">Worked for 2m 14s</div>`;

// The card floats over the end of the conversation, which fades out under it.
const c2Floating = () => `<div style="position:absolute;left:0;right:0;bottom:0;height:150px;background:linear-gradient(to bottom, rgba(47,52,62,0), var(--panel) 45%);pointer-events:none"></div>
  <div style="position:absolute;left:0;right:0;bottom:0">${c2Card()}</div>`;

TOPICS.push({
  id: 'shape', section: 'Composer', title: 'The composer’s shape in a thread', size: 'wide', rec: 'A',
  now: 'Once a thread has its first message, the composer is a bar across the bottom of the thread, as in Zed: a border along its top, the editor’s darker background, and its contents (up to 850 px wide) in the middle. The new thread screen has the same composer as a card: rounded, outlined, with a shadow. The activity bar (the plan’s current step, the Agents list) sits on the bar’s top edge. The footer is the next topic.',
  nowImg: 'img/now-composer.png',
  issues: ['Sending the first message turns the card into a bar, so the composer changes shape as you start.', 'The bar runs edge to edge, so it reads as part of the window rather than of the thread.'],
  options: [
    { key: 'A', name: 'A card, as on the new thread', from: 't3code ChatComposer, agentZ new thread',
      desc: 'The thread keeps the new thread’s card: rounded, outlined and shadowed, 16 px in from the thread’s sides and above its bottom, on the thread’s background. Same contents and width (up to 850 px). Sending the first message no longer changes it.',
      good: 'One composer everywhere; it reads as the thread’s, not the window’s.', cost: 'Takes about 16 px more height than the bar.',
      mock: () => c2Thread(c2Card()) },
    { key: 'B', name: 'A card, with a strip under it', from: 't3code context strip, Codex',
      desc: 'The card holds what you send: the text, + and Send, with the mode, model and effort. Who answers moves to a strip under the card, in small muted type: the agent, the account and its usage at the left, the context ring at the right, as the new thread’s strip shows the checkout and machine.',
      good: 'The card is shorter and calmer; who answers is still one click away.', cost: 'A second row, under the card.',
      mock: () => c2Thread(c2Card({ footer: c2Footer({ left: c2Plus(), right: `<span class="row" style="gap:4px">${c2Settings()}${c2Send()}</span>` }), below: `<div class="row" style="gap:2px;padding:6px 2px 0">${c2AgentChip()}${c2AccountChip()}${c2Gauge()}<span class="grow"></span>${c2Ring()}</div>` }), { h: 360 }) },
    { key: 'C', name: 'One stack with the activity bar', from: 't3code ComposerBannerStack',
      desc: 'As A, and what sits above the composer (the activity bar with the plan’s step or the Agents list, a limit notice) joins the card at its top as one outlined stack, in t3code’s banners’ way. With nothing above it, it’s A’s card.',
      good: 'What the agent is doing and what you type next read as one thing.', cost: 'The stack is taller when there’s a plan and agents.',
      mock: () => c2Thread(c2Card({ above: c2Activity({ attached: true }) }), { h: 370 }) },
    { key: 'D', name: 'A card floating over the conversation', from: 't3code glass composer',
      desc: 'As A, but the card floats over the bottom of the conversation, which fades out under it and scrolls behind it, as t3code’s does. The last line can scroll up from under it.',
      good: 'The conversation gets the whole height.', cost: 'Text behind the card’s edges; GPUI has no blur, only the fade.',
      mock: () => c2Thread('', { talk: C2_LONG_TALK, overlay: c2Floating(), h: 360 }) },
    { key: 'E', name: 'As it is', from: 'today (Zed)',
      desc: 'The bar in a thread, the card on the new thread screen.',
      good: 'Nothing to change; Zed’s look.', cost: 'Two shapes for one composer.',
      mock: () => c2Thread(c2Bar()) },
  ],
});

// The footer's controls. The mocks use the card of A above.
const c2ModelMenu = () => c2Menu(`<div class="lbl">Model</div>${['Opus 5.5', 'Sonnet 5', 'Haiku 4.5'].map((model, i) => `<div class="it ${i === 0 ? 'hl' : ''}"><span class="grow">${model}</span>${i === 0 ? c2Check() : ''}</div>`).join('')}<div class="hr"></div><div class="lbl">Effort</div><div style="padding:3px 8px 6px">${c2Segments(['Low', 'Medium', 'High', 'Xhigh'], 2)}</div>`, 'bottom:66px;right:52px', 240);
const c2Combined = (open = false) => c2Chip(`${glyph('claude', 'sm')}<span>Claude Agent</span><span class="faint">·</span>${c2Avatar(C2_ACCOUNTS[0], 14)}<span>Personal</span><span class="faint">·</span><span class="row" style="gap:2px">${ic('gauge', 'xs')}69%</span>`, '', { open });
const c2CombinedMenu = () => c2Menu(C2_AGENTS.map((agent) => {
  const row = `<div class="it ${agent.id === 'claude' ? '' : ''}">${glyph(agent.glyph, 'sm')}<span class="grow">${agent.name}</span>${agent.left !== null && agent.accounts === 1 ? `<span class="xs" style="color:${c2LeftColor(agent.left)}">${agent.left}% left</span>` : ''}${c2NoCheck()}</div>`;
  if (agent.id !== 'claude') return row;
  return `<div class="it">${glyph(agent.glyph, 'sm')}<span class="grow">${agent.name}</span>${c2NoCheck()}</div>` + C2_ACCOUNTS.map((account) => `<div class="it ${account.current ? 'hl' : ''}" style="padding-left:28px;gap:8px">${c2Avatar(account, 14)}<span class="grow trunc">${account.name}</span><span class="xs" style="color:${c2LeftColor(account.left)}">${account.left}% left</span>${account.current ? c2Check() : c2NoCheck()}</div>`).join('');
}).join('') + c2Manage('Manage Agents…'), 'bottom:66px;left:56px', 290);
const c2Sep = () => '<span style="width:1px;height:14px;background:var(--b);margin:0 4px;flex:none"></span>';
const c2CompactMenu = () => c2Menu(`<div class="lbl">Mode</div>${['Default', 'Accept edits', 'Plan', 'Bypass permissions'].map((mode, i) => `<div class="it"><span class="grow">${mode}</span>${i === 0 ? c2Check() : ''}</div>`).join('')}<div class="hr"></div><div class="lbl">Model</div><div class="it"><span class="grow">Opus 5.5</span>${ic('chev-right', 'xs')}</div><div class="hr"></div><div class="lbl">Effort</div><div style="padding:3px 8px 6px">${c2Segments(['Low', 'Medium', 'High', 'Xhigh'], 2)}</div>`, 'bottom:66px;right:30px', 220);

TOPICS.push({
  id: 'controls', section: 'Composer', title: 'The footer’s controls', size: 'wide', rec: 'A', 
  now: 'At the left: + (Add Context), the agent (“Claude Agent ⌄”), the account (an avatar and “Personal ⌄”, with two or more accounts) and the usage gauge (“69%”). At the right: the context ring, then the agent’s own settings, each its own menu (mode “Default ⌄”, model “Opus 5.5 ⌄”, effort “High ⌄”), and Send. Narrow, the right group wraps to a second line and then within it. (The screenshot is of the mock agent, whose settings are Default, Sonnet, Medium and a Fast switch.) The mocks use the card of A in the topic before.',
  nowImg: 'img/now-composer.png',
  issues: ['Up to nine controls of the same weight in one row.', 'Model and effort are two menus for what is one choice in most agents.'],
  options: [
    { key: 'A', name: 'Model and effort in one menu', from: 'Codex /model (a model, then its effort)',
      desc: 'The model and effort show as one chip, “Opus 5.5 · High ⌄”. Its menu lists the models, with the effort as a row of segments under them. An agent without effort keeps the model alone. Mode stays its own menu.',
      good: 'One control less, and the two are picked together.', cost: 'Changing the effort takes the menu, not one click on its chip.',
      mock: () => c2Thread(c2Card({ footer: c2Footer({ right: `<span class="row" style="gap:4px">${c2Ring()}${c2Chip('', 'Default')}${c2Chip('', 'Opus 5.5 · High', { open: true })}${c2Send()}</span>` }) }), { overlay: c2ModelMenu(), h: 420 }) },
    { key: 'B', name: 'Agent, account and usage as one chip', from: 'new',
      desc: 'Who answers is one chip: the agent’s icon and name, the account’s avatar and name, and what’s left of its usage (“Claude Agent · Personal · 69%”). Its menu lists the agents, the current agent’s accounts under it with what each has left.',
      good: 'The left side reads as one sentence; switching account or agent is one menu.', cost: 'A longer menu; the gauge’s own popover (windows, reset) moves into it.',
      mock: () => c2Thread(c2Card({ footer: c2Footer({ left: `<span class="row" style="gap:2px">${c2Plus()}${c2Combined(true)}</span>` }) }), { overlay: c2CombinedMenu(), h: 440 }) },
    { key: 'C', name: 'Narrow, the settings fold into one menu', from: 't3code CompactComposerControlsMenu',
      desc: 'When the footer doesn’t fit on one line, the mode, model and effort fold into one “⋯” button rather than wrapping to a second line. Its menu has a section for each, as t3code’s compact menu does. Wide, nothing changes.',
      good: 'The composer stays one footer line in narrow panes.', cost: 'In a narrow pane you don’t see the model at a glance.',
      mock: () => c2Thread(c2Card({ footer: c2Footer({ left: `<span class="row" style="gap:2px">${c2Plus()}${c2AgentChip()}${c2Chip(c2Avatar(C2_ACCOUNTS[0], 14), '')}${c2Gauge()}</span>`, right: `<span class="row" style="gap:4px">${c2Ring()}${c2Chip(ic('more', 'sm'), '', { chev: false, open: true })}${c2Send()}</span>` }) }), { w: 460, h: 470, overlay: c2CompactMenu() }) },
    { key: 'D', name: 'Separators between groups', from: 't3code ComposerControlSeparator',
      desc: 'Thin vertical separators group the controls: + | who answers (agent, account, usage) on the left; context | the agent’s settings | Send on the right. The controls themselves stay as they are.',
      good: 'Easier to scan, with nothing moved.', cost: 'Still every control in the row.',
      mock: () => c2Thread(c2Card({ footer: c2Footer({ left: `<span class="row" style="gap:2px">${c2Plus()}${c2Sep()}${c2AgentChip()}${c2AccountChip()}${c2Gauge()}</span>`, right: `<span class="row" style="gap:4px">${c2Ring()}${c2Sep()}${c2Settings()}${c2Sep()}${c2Send()}</span>` }) })) },
    { key: 'E', name: 'As they are', from: 'today',
      desc: 'Each control its own chip; narrow, the right group wraps.',
      good: 'Every setting one click away.', cost: 'A long, even row.',
      mock: () => c2Thread(c2Card()) },
  ],
});

// Send, Queue and Stop.
const c2Round = ({ state = 'ready' } = {}) => {
  if (state === 'stop') return '<span style="width:26px;height:26px;border-radius:50%;background:rgba(208,114,119,.9);display:inline-grid;place-items:center;flex:none"><i style="width:9px;height:9px;border-radius:2px;background:#fff;display:block"></i></span>';
  const ready = state !== 'empty';
  return `<span style="width:26px;height:26px;border-radius:50%;background:${ready ? 'var(--ac)' : 'var(--sel)'};color:${ready ? '#1b1f26' : 'var(--ph)'};display:inline-grid;place-items:center;flex:none">${ic(state === 'queue' ? 'queue' : 'arrow-up', 'sm')}</span>`;
};
const c2Square = ({ state = 'ready' } = {}) => {
  if (state === 'stop') return `<span class="ibtn" style="background:rgba(208,114,119,.18);color:var(--del)">${ic('stop', 'xs')}</span>`;
  const ready = state !== 'empty';
  return `<span class="ibtn" style="background:${ready ? 'var(--ac)' : 'var(--sel)'};color:${ready ? '#1b1f26' : 'var(--ph)'}">${ic(state === 'queue' ? 'queue' : 'send', 'xs')}</span>`;
};
const c2Labeled = ({ state = 'ready' } = {}) => {
  if (state === 'stop') return `<span class="btn sm" style="color:var(--del);border-color:rgba(208,114,119,.5);gap:5px">${ic('stop', 'xs')}Stop <span class="xs ph">Esc</span></span>`;
  const ready = state !== 'empty';
  return `<span class="btn sm ${ready ? 'primary' : ''}" style="gap:5px;${ready ? '' : 'color:var(--ph)'}">${state === 'queue' ? 'Queue' : 'Send'} <span class="xs" style="opacity:.7">⏎</span></span>`;
};
const c2Today = ({ state = 'ready' } = {}) => {
  if (state === 'stop') return `<span class="ibtn" style="background:rgba(208,114,119,.18);color:var(--del)">${ic('stop', 'xs')}</span>`;
  return `<span class="ibtn" style="background:var(--sel);color:${state === 'empty' ? 'var(--ph)' : 'var(--ac)'}">${ic(state === 'queue' ? 'queue' : 'send', 'xs')}</span>`;
};
const c2SendStates = (button) => {
  const card = (state, body, label) => `<div class="col" style="gap:4px"><div class="xs ph">${label}</div>${c2Card({ body, footer: c2Footer({ right: `<span class="row" style="gap:4px">${c2Ring()}${c2Settings()}${button({ state })}</span>` }), pad: '0' })}</div>`;
  return frame(`<div style="position:absolute;inset:0;background:var(--panel);padding:14px 16px;display:flex;flex-direction:column;gap:12px">
    ${card('empty', c2Placeholder, 'Nothing typed')}${card('ready', `Also show it in the account picker${c2Caret}`, 'Ready to send')}${card('stop', c2Placeholder, 'Working, nothing typed: Stop')}${card('queue', `Then add a test for it${c2Caret}`, 'Working, something typed: Queue')}</div>`, { w: 700, h: 520 });
};

TOPICS.push({
  id: 'send', section: 'Composer', title: 'Send, Queue and Stop', size: 'wide', rec: 'A',
  now: 'A small square button in the corner, the same gray whether or not there’s anything to send; only its icon turns blue when there is. While the agent works it’s a red Stop with nothing typed, and Queue (a list icon) with something typed. The mocks use the card of A in the first topic.',
  nowImg: 'img/now-composer.png',
  issues: ['Ready and not ready look nearly the same.'],
  options: [
    { key: 'A', name: 'A round button, filled when ready', from: 't3code ComposerPrimaryActions',
      desc: 'A round button with an up arrow: gray while there’s nothing to send, filled with the accent color once there is. Stop is a red round button with a white square; Queue keeps the round shape with the list icon.',
      good: 'You see at a glance whether Enter will send.', cost: 'A new shape among the square icon buttons.',
      mock: () => c2SendStates(c2Round) },
    { key: 'B', name: 'Today’s square, filled when ready', from: 'Zed filled button, accent',
      desc: 'Today’s square button, filled with the accent color (and a dark icon) once there’s something to send. Stop and Queue as today.',
      good: 'The smallest change that shows readiness.', cost: 'Still a small target.',
      mock: () => c2SendStates(c2Square) },
    { key: 'C', name: 'A labeled button', from: 'new',
      desc: 'A button that says what it does, with its key: “Send ⏎”, “Queue ⏎”, “Stop Esc”. Filled with the accent color when ready.',
      good: 'No guessing what the icon means, and the key is on it.', cost: 'Wider: takes room from the settings in narrow panes.',
      mock: () => c2SendStates(c2Labeled) },
    { key: 'D', name: 'As it is', from: 'today',
      desc: 'The gray square with a blue icon when ready.',
      good: 'Quiet.', cost: 'Ready and not ready look alike.',
      mock: () => c2SendStates(c2Today) },
  ],
});
