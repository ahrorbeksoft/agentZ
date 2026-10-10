// Permission requests and plans: how a tool call asks, what it shows while it waits, saying no
// with a reason, and a plan to approve.

const qMenu = (items, style) => `<div class="menu" style="font-size:12px;min-width:0;${style}">${items.map(([icon, text, cls = '']) => (text === '-' ? '<div class="hr"></div>' : `<div class="it ${cls}" style="height:24px">${icon ? ic(icon, 'xs') : '<span style="width:12px;flex:none"></span>'}${text}</div>`)).join('')}</div>`;
const WARN_BORDER = 'rgba(224,180,92,.5)';
const shieldHead = (text, right = '') => `<div class="row" style="height:34px;padding:0 10px 0 12px;gap:8px;background:rgba(224,180,92,.07);border-bottom:1px solid var(--b)"><span style="display:inline-flex;color:var(--warn)">${ic('shield', 'sm')}</span><span class="grow trunc" style="font-size:13px;color:var(--t)">${text}</span>${right}</div>`;
const cmdTerm = (cmd = 'npm test') => qTerm(`<span style="color:var(--ph)">$</span> ${cmd}`);
const choiceButtons = (options, { wrap = true } = {}) => `<span class="row" style="${wrap ? 'flex-wrap:wrap;' : ''}gap:6px;justify-content:flex-end;flex:1">${options.map(([kind, text], index) => qBtn(text, { kind: index === 0 ? 'primary' : 'outline', icon: OPT_ICON[kind][0], iconColor: index === 0 ? '#fff' : OPT_ICON[kind][1] })).join('')}</span>`;
// Topic 1's A, which the other topics' mocks use for a permission.
const permCard = ({ agent = AGENT, title = `${agent} wants to run a command`, body = cmdTerm(), options = CLAUDE_BASH, foot = '' } = {}) => qCard({ head: shieldHead(title), border: WARN_BORDER, body: `<div style="height:6px"></div>${body}`, foot: foot || choiceButtons(options) });

// 1. A permission request ------------------------------------------------------------------
function permB(agent, options, open) {
  const always = options.filter(([kind]) => kind === 'always').map(([, text]) => ['', text]);
  const menu = open ? qMenu([['check', 'Only this time'], ...always], 'left:10px;bottom:40px') : '';
  return qCard({ head: shieldHead(`${agent} wants to run a command`), border: WARN_BORDER, style: 'position:relative;overflow:visible',
    body: `<div style="height:6px"></div>${cmdTerm()}`,
    foot: `${qBtn('Only this time', { chev: true })}${menu}<span class="grow"></span>${qBtn('Deny', { icon: 'x', iconColor: 'var(--del)' })}${qBtn('Allow', { kind: 'primary', icon: 'check', iconColor: '#fff' })}` });
}
const ZED_KEYS = { once: '⌘Y', always: '⌘⌥Y', reject: '⌘⌥Z' };
function permC(agent, options) {
  const seen = {};
  const rows = options.map(([kind, text], index) => {
    const key = seen[kind] ? '' : ZED_KEYS[kind];
    seen[kind] = true;
    return `<div class="row" style="height:26px;gap:8px;padding:0 8px;border-radius:5px;font-size:13px;color:var(--t);${index === 0 ? 'background:var(--hov);' : ''}"><span style="display:inline-flex;color:${OPT_ICON[kind][1]}">${ic(OPT_ICON[kind][0], 'xs')}</span><span class="grow trunc">${text}</span>${key ? `<span style="font-size:11px;color:var(--ph)">${key}</span>` : ''}</div>`;
  }).join('');
  return qCard({ head: shieldHead(`Allow ${agent} to run this command?`), border: WARN_BORDER, body: `<div style="height:6px"></div>${cmdTerm()}<div class="col" style="gap:1px">${rows}</div>` });
}
function permD(agent, options) {
  const rows = options.map(([, text], index) => `<div class="row" style="height:26px;gap:8px;padding:0 8px;border-radius:5px;font-size:13px;${index === 0 ? 'background:rgba(84,138,247,.14);color:var(--t);' : 'color:var(--mu);'}"><span style="width:10px;color:var(--ac)">${index === 0 ? '›' : ''}</span>${qKey(index + 1)}<span class="grow trunc">${text}</span></div>`).join('');
  return qCard({ head: shieldHead('Do you want to run <code style="font:12px ' + QMONO + '">npm test</code>?'), border: WARN_BORDER,
    body: `<div style="height:6px"></div>${cmdTerm()}<div class="col" style="gap:1px">${rows}</div><div class="hint" style="font-size:11px;color:var(--ph)">↑ ↓ to choose · ⏎ to answer · Esc for No</div>` });
}
function permE(agent, options, open) {
  const first = options[0];
  const no = options.find(([kind]) => kind === 'reject');
  const rest = options.filter((option) => option !== first && option !== no);
  const menu = open ? qMenu(rest.map(([kind, text]) => [OPT_ICON[kind][0], text]), 'right:10px;bottom:40px') : '';
  return qCard({ head: shieldHead(`${agent} wants to run a command`), border: WARN_BORDER, style: 'position:relative;overflow:visible',
    body: `<div style="height:6px"></div>${cmdTerm()}`,
    foot: `<span class="grow"></span>${qBtn(no[1], { icon: 'x', iconColor: 'var(--del)' })}${qBtn(first[1], { kind: 'primary', icon: 'check', iconColor: '#fff' })}<span style="width:26px;height:26px;border:1px solid var(--b);border-radius:6px;display:inline-grid;place-items:center;color:var(--mu);${open ? 'background:var(--hov);' : ''}">${ic('more', 'xs')}</span>${menu}` });
}
const runRow = qRow('terminal', `${qVerb('Run')}${qCode('npm test')}`);
const twoAgents = (render) => qConv(`${qCaption('Claude Agent: three choices')}${runRow}${render('Claude Agent', CLAUDE_BASH, false)}${qCaption('Factory Droid: six choices')}${runRow}${render('Factory Droid', DROID_BASH, true)}`, 470);

TOPICS.push({
  id: 'permission', section: 'Permission to use a tool', title: 'A permission request', size: 'wide', rec: 'A',
  now: 'When an agent asks before it uses a tool, the tool’s row stays, with its spinner, and the agent’s choices list under it as plain lines: a check for once, a double check for always, a red × for no. The agent names them, so there are three (Claude Agent: “Yes”, “Yes, and don’t ask again for npm test commands”, “No”), four (Codex) or six (Factory Droid: “Allow”, “Allow always”, three “Allow &amp; auto-run (… risk)”, “Cancel”). Under the turn, “Awaiting Confirmation” shows before “Working for 23s”.',
  nowImg: 'img/now-permission.png',
  issues: ['Nothing frames it as a question: it reads as more lines of the turn, and the spinner says the tool is already running.', 'No choice stands out as the main one, and No is told apart only by its red ×.', 'The rows say nothing about who asks or why; a long list (Droid’s six) is a wall of lines.'],
  options: [
    { key: 'A', name: 'A card that asks, buttons in a row', from: 'Zed’s tool cards, t3code’s buttons',
      desc: 'The request is a card with a yellow edge, as Zed outlines a tool that waits for you. Its header says who asks and what: “Claude Agent wants to run a command”, with a shield. The command shows under it, then the agent’s choices as buttons at the bottom right, in the agent’s order and words: the first is the main one, filled. Buttons that don’t fit wrap to a second line.',
      good: 'Reads as a question at a glance; every agent’s words kept.', cost: 'Six long choices wrap into two or three lines of buttons.',
      mock: () => twoAgents((agent, options) => permCard({ agent, options })) },
    { key: 'B', name: 'Allow, Deny, and how long', from: 'Zed (its permission dropdown)',
      desc: 'The same card, but always two buttons, Allow and Deny, as Zed’s newer permission buttons. A menu at the left says for how long: “Only this time” at first, and it lists the agent’s other allowing choices in its words (“Allow always”, “Allow &amp; auto-run (low risk)”…). Allow does what the menu says.',
      good: 'The same two buttons for every agent and every tool.', cost: 'A second No (Codex’s “No, and tell Codex what to do differently”) needs a menu on Deny too; the agent’s words hide in a menu.',
      mock: () => twoAgents(permB) },
    { key: 'C', name: 'Today’s list, in a card, with keys', from: 'Zed (its flat permission buttons)',
      desc: 'The choices stay a list, one per line, as today and as Zed’s first permission buttons. The card and its header frame them (“Allow Claude Agent to run this command?”), the first choice is highlighted, and the first of each kind shows Zed’s key: ⌘Y once, ⌘⌥Y always, ⌘⌥Z no.',
      good: 'Smallest change; long names never wrap into buttons.', cost: 'Still a list of lines with no main button.',
      mock: () => twoAgents(permC) },
    { key: 'D', name: 'A numbered list', from: 'Claude Code (its terminal)',
      desc: 'As Claude Code asks in its terminal: “Do you want to run npm test?”, then the choices numbered 1, 2, 3, the first highlighted. ↑ ↓ move, ⏎ answers, a number answers at once, Esc says No. A click answers too.',
      good: 'Fast from the keyboard, and the same for three or six choices.', cost: 'Looks like a terminal menu more than a button; the keys only work while the card has focus (see Answering from the keyboard).',
      mock: () => twoAgents(permD) },
    { key: 'E', name: 'Main choice, No, and the rest in a menu', from: 't3code (its approval panel)',
      desc: 'Two buttons, as t3code’s approval: the agent’s first choice (filled) and its first No, each in the agent’s words. Every other choice is in a ⋯ menu beside them (“Allow always”, the auto-run levels, Codex’s second No).',
      good: 'Always one line, however many choices the agent sends.', cost: 'The “always” choices take a second click and are easy to miss.',
      mock: () => twoAgents(permE) },
  ],
});

// 2. What a request shows ------------------------------------------------------------------
const pendingTag = qTag('Waiting for you', 'var(--warn)', 'rgba(224,180,92,.12)');
const EDIT_LINES = [['-', 'export function cartTotal(items: Item[]) {'], ['-', '  return items.reduce((sum, item) => sum + round(item.price * item.quantity), 0);'], ['+', 'export function cartTotal(items: Item[], discount = 0) {'], ['+', '  const sum = items.reduce((sum, item) => sum + item.price * item.quantity, 0);'], ['+', '  return round(sum * (1 - discount));'], [' ', '}']];
const CODEX_EDIT = [['once', 'Yes, proceed'], ['always', 'Yes, and don’t ask again for these files'], ['reject', 'No, and tell Codex what to do differently']];

TOPICS.push({
  id: 'shows', section: 'Permission to use a tool', title: 'What a request shows', size: 'wide', type: 'multi', rec: 'ABE',
  now: 'The tool’s row is the same as a running one’s: a spinner, and the agent’s title. Codex’s edit says “Edited src/cart/total.ts +3 −2”, past tense, with its diff open under it. A command opens like any command, an MCP tool shows its name and server, and its input (the JSON) is behind “Input”. These mocks use A’s card from the topic before.',
  nowImg: 'img/now-permission-edit.png',
  issues: ['The spinner and the past tense (“Edited”) say it already happened.', 'Claude Agent sends a reason with each command (“Run the unit tests”) and Codex can too; it isn’t shown.', 'An MCP tool’s arguments are only in raw JSON.'],
  options: [
    { key: 'A', name: 'Present tense, no spinner, until you answer', from: 'new',
      desc: 'While it waits the row says what it will do, in the present tense (“Edit src/cart/total.ts”, “Run npm test”), with “Waiting for you” where the spinner was. Once allowed, it runs and reads as today.',
      good: 'No longer looks done or running.', cost: 'Each agent’s title has to be turned around when it’s in the past tense; titles agentZ doesn’t write stay as sent.',
      mock: () => qConv(`${qCaption('Waiting')}${qRow('pencil', `${qVerb('Edit')}${qCode('src/cart/total.ts')}`, { trailing: `${qStat(3, 2)}${pendingTag}` })}${permCard({ agent: 'Codex', title: 'Codex wants to edit a file', body: qDiff(EDIT_LINES.slice(0, 3)), options: CODEX_EDIT })}${qCaption('Allowed')}${qRow('pencil', `${qVerb('Edited')}${qCode('src/cart/total.ts')}`, { trailing: qStat(3, 2) })}`, 420) },
    { key: 'B', name: 'The agent’s reason', from: 'new (from what Claude Agent and Codex send)',
      desc: 'When the agent says why (Claude Agent’s description of a command, Codex’s reason), it shows under the header in the agent’s words, before the command.',
      good: 'You see why before you allow it.', cost: 'Only some agents and tools send one.',
      mock: () => qConv(`${runRow}${permCard({ body: `<div style="font-size:13px;color:var(--mu)">Run the unit tests to check the new total.</div>${cmdTerm()}` })}`, 230) },
    { key: 'C', name: 'Where it runs', from: 'new',
      desc: 'A command’s card says which folder it runs in when it isn’t the project’s root, and in which worktree or pasture, as a dim line under the command.',
      good: 'A command in the wrong place is caught before it runs.', cost: 'One more line on every command.',
      mock: () => qConv(`${runRow}${permCard({ body: `${cmdTerm()}<div class="row g15" style="font-size:12px;color:var(--ph)">${ic('folder', 'xs')}packages/web<span>·</span>${ic('worktree', 'xs')}checkout-flow worktree</div>` })}`, 230) },
    { key: 'D', name: 'Long diffs and commands cut short', from: 'new',
      desc: 'A diff or command longer than 12 lines shows its first 12, fading out, with “Show all 48 lines” under it. Today a long diff pushes the choices off the screen.',
      good: 'The choices stay near the top of a big edit.', cost: 'You have to open it to read all of it before allowing.',
      mock: () => qConv(`${qRow('pencil', `${qVerb('Edit')}${qCode('src/cart/total.ts')}`, { trailing: qStat(31, 17) })}${permCard({ agent: 'Codex', title: 'Codex wants to edit a file', options: CODEX_EDIT, body: `<div class="qfade">${qDiff([...EDIT_LINES, ...EDIT_LINES])}</div><div class="row g1" style="font-size:12px;color:var(--ac)">Show all 48 lines${ic('chev-down', 'xs')}</div>` })}`, 400) },
    { key: 'E', name: 'Fetches and MCP tools in words', from: 'new',
      desc: 'A fetch shows the whole address with its host bright (“Fetch <b>docs.stripe.com</b>/api/charges”). An MCP tool shows its arguments as named lines (“Title: Checkout total rounds twice”), not JSON; “Input” still opens the JSON.',
      good: 'The parts that matter are readable without opening JSON.', cost: 'Arguments that are long or nested still need the JSON.',
      mock: () => qConv(`${permCard({ title: 'Claude Agent wants to fetch a page', body: `<div style="font:12px ${QMONO};color:var(--ph)">https://<span style="color:var(--t)">docs.stripe.com</span>/api/charges</div>`, options: [['once', 'Yes'], ['always', 'Yes, and don’t ask again for docs.stripe.com'], ['reject', 'No']] })}${permCard({ title: 'Claude Agent wants to use Create issue <span style="color:var(--ph)">github</span>', body: `<div class="col" style="gap:3px;font-size:13px"><div><span style="color:var(--ph)">Title</span>&nbsp; Checkout total rounds twice</div><div><span style="color:var(--ph)">Repository</span>&nbsp; acme/storefront</div><div><span style="color:var(--ph)">Labels</span>&nbsp; bug, checkout</div></div><div class="row g1" style="font:11px ${QMONO};color:var(--ph)">Input${ic('chev-down', 'xs')}</div>`, options: [['once', 'Allow'], ['always', 'Allow for this session'], ['reject', 'Deny']] })}`, 400) },
  ],
});

// 3. Saying no, and why ---------------------------------------------------------------------
const codexAsk = (body = '') => `${qRow('terminal', `${qVerb('Run')}${qCode('rm -rf node_modules')}`)}${body}`;
const deniedRow = qRow('terminal', `${qVerb('Run')}${qCode('rm -rf node_modules')}`, { trailing: `<span class="none" style="font-size:12px;color:var(--del)">Denied</span>` });
TOPICS.push({
  id: 'deny', section: 'Permission to use a tool', title: 'Saying no, and why', size: 'wide', rec: 'A',
  now: 'A No is sent as the agent’s choice and nothing more. Some agents offer a No that waits for your words: Codex’s “No, and tell Codex what to do differently” and Claude Agent’s plan “No, keep planning”. agentZ sends it and the agent ends its turn; you then write in the composer, which has no hint that it’s waiting for that.',
  nowImg: 'img/now-permission-edit.png',
  issues: ['“Tell Codex what to do differently” has nowhere to tell it.', 'Other agents’ No gives no way to say why.'],
  options: [
    { key: 'A', name: 'The composer asks for it', from: 't3code (its plan feedback)',
      desc: 'Picking a No that waits for words, or any No with ⌥ held, puts the cursor in the composer with “Tell Codex what to do differently” as its placeholder and the request named over it. Sending sends the No, then your message. Esc sends the No alone.',
      good: 'Uses the box you already write in; works for every agent.', cost: 'The request and the composer can be far apart in a long thread.',
      mock: () => qThread({ h: 330, conv: `${qBubble('Clean the install and run the tests')}${deniedRow}`, composer: qComposer({ placeholder: 'Tell Codex what to do differently', style: 'border-color:var(--bf);box-shadow:0 0 0 1px var(--bf)', top: `<div class="row g15" style="padding:7px 10px 0;font-size:12px;color:var(--mu)">${ic('corner', 'xs')}No to <code style="font:12px ${QMONO}">rm -rf node_modules</code><span class="grow"></span><span style="color:var(--ph)">Esc to send only No</span></div>` }) }) },
    { key: 'B', name: 'A field in the card', from: 'new',
      desc: 'Picking it opens a one-line field in the card, under the choices, with Send. Empty, Send sends the No alone.',
      good: 'You answer where it asked.', cost: 'A second text box beside the composer.',
      mock: () => qConv(`${qBubble('Clean the install and run the tests')}${codexAsk(permCard({ agent: 'Codex', body: cmdTerm('rm -rf node_modules'), foot: `<span class="col grow" style="gap:6px"><span class="row g2" style="font-size:12px;color:var(--mu)">${ic('x', 'xs')}No, and tell Codex what to do differently</span><span class="row g2">${qInput('Keep node_modules, just run npm ci', { focus: true })}${qBtn('Send', { kind: 'primary' })}</span></span>` }))}`, 300) },
    { key: 'C', name: 'Every No can say why', from: 'new',
      desc: 'Every request’s No has a menu with “No, and say why…”, for every agent, which does what A does. The agent’s own Nos stay as they are.',
      good: 'You can steer any agent, not only Codex.', cost: 'One more menu on every request.',
      mock: () => qConv(`${codexAsk(qCard({ head: shieldHead('Claude Agent wants to run a command'), border: WARN_BORDER, style: 'position:relative;overflow:visible', body: `<div style="height:6px"></div>${cmdTerm('rm -rf node_modules')}`, foot: `<span class="grow"></span>${qBtn('No', { icon: 'x', iconColor: 'var(--del)', chev: true })}${qBtn('Yes, and don’t ask again for rm commands')}${qBtn('Yes', { kind: 'primary', icon: 'check', iconColor: '#fff' })}${qMenu([['x', 'No'], ['square-pen', 'No, and say why…']], 'right:250px;bottom:40px')}` }))}`, 260) },
    { key: 'D', name: 'Only the choice', from: 'Zed',
      desc: 'As today and as Zed: the No is sent, the row says “Denied”, and whatever you want to say goes in the composer as usual.',
      good: 'Nothing new.', cost: 'Codex’s “tell Codex” choice still leads nowhere.',
      mock: () => qThread({ h: 300, conv: `${qBubble('Clean the install and run the tests')}${deniedRow}${qPara('Okay, I won’t delete node_modules. What would you like me to do instead?')}`, composer: qComposer({ placeholder: 'Message Codex — @ to add context, / for commands' }) }) },
    { key: 'E', name: 'No stops the turn', from: 'new',
      desc: 'A No that waits for words also stops the agent’s turn, so it doesn’t go on without you, and puts the cursor in the composer.',
      good: 'The agent never carries on around a No you meant to explain.', cost: 'Stopping may lose work the agent had in flight.',
      mock: () => qThread({ h: 300, conv: `${qBubble('Clean the install and run the tests')}${deniedRow}<div class="row g15" style="font-size:12px;color:var(--ph);margin-top:6px">${ic('stop', 'xs')}You stopped the turn</div>`, composer: qComposer({ placeholder: 'Tell Codex what to do differently', style: 'border-color:var(--bf);box-shadow:0 0 0 1px var(--bf)' }) }) },
  ],
});

// 4. A plan to approve -----------------------------------------------------------------------
const planBadge = qTag('Plan', 'var(--pur)', 'rgba(177,137,245,.14)');
const planHead = (right = '') => `<div class="row" style="height:36px;padding:0 10px 0 12px;gap:8px">${ic('map', 'sm')}${planBadge}<span class="grow trunc" style="font-size:13px;color:var(--t)">Add a checkout total</span>${right}<span style="color:var(--ph);display:inline-flex;gap:8px">${ic('copy', 'sm')}${ic('download', 'sm')}</span></div>`;
const planCard = ({ full = false, foot = '', style = '', right = '' } = {}) => qCard({ head: planHead(right), style, body: full ? PLAN_MD : `<div class="qfade">${PLAN_MD}</div><div class="row g1" style="font-size:12px;color:var(--ac)">Expand plan${ic('chev-down', 'xs')}</div>`, foot });
const planAsk = qBubble('Plan the checkout total');

TOPICS.push({
  id: 'plan', section: 'Plans', title: 'A plan to approve', size: 'wide', rec: 'B',
  now: 'Claude Agent’s plan comes as a permission request on a tool called “Approve Plan”: its row shows the plan as plain text in the code font, behind “Input”, and five choices under it: “Yes, clear context (34% used) and use auto mode”, “Yes, and use auto mode”, “Yes, and bypass permissions”, “Yes, manually approve edits”, “No, keep planning”. Once answered the row says “Exited Plan Mode” and the plan is folded away. Codex asks “Implement this plan?”, and Factory Droid’s spec mode “Proceed with implementation” in three levels.',
  nowImg: 'img/now-plan.png',
  issues: ['The plan is markdown shown as raw text, in the code font.', 'After you answer, the plan is gone from the thread (only in the tool’s input).', 'Five long choices of the same weight; “keep planning” gives no place to say what to change.'],
  options: [
    { key: 'A', name: 'A plan card with the choices', from: 't3code (its proposed plan card)',
      desc: 'The plan is a card of its own, as t3code’s: a “Plan” tag and its title, the markdown rendered, cut at about 150 px with “Expand plan”, and Copy and Save as a file. The agent’s choices are its footer, as a request’s (the permission topic decides how). The card stays in the thread after you answer.',
      good: 'Readable, and it stays where you can find it.', cost: 'Long plans need a click to read in full.',
      mock: () => qConv(`${planAsk}${planCard({ foot: `${qBtn('No, keep planning', { kind: 'ghost' })}<span class="grow"></span>${qBtn('Yes, manually approve edits')}${qBtn('Yes, and use auto mode', { kind: 'primary' })}<span style="width:26px;height:26px;border:1px solid var(--b);border-radius:6px;display:inline-grid;place-items:center;color:var(--mu)">${ic('more', 'xs')}</span>` })}${qGen('Awaiting Confirmation', 'Working for 1m 12s')}`, 380) },
    { key: 'B', name: 'Implement or refine, from the composer', from: 't3code (its plan follow-up)',
      desc: 'A’s card, without buttons. The composer says “Plan ready” and “Add feedback to refine the plan, or leave this blank to implement it”. Sending empty implements it (“Implement”, with the agent’s other yes choices in its menu); typing turns the button to “Refine”, which sends “No, keep planning” and your words.',
      good: 'Saying what to change is the natural path; one button.', cost: 'The choice is in the composer, away from the plan in a long thread.',
      mock: () => qThread({ h: 470, conv: `${planAsk}${planCard()}`, composer: qComposer({ placeholder: 'Add feedback to refine the plan, or leave this blank to implement it', top: `<div class="row g15" style="padding:7px 10px 0;font-size:12px;color:var(--pur)">${ic('map', 'xs')}Plan ready<span style="color:var(--ph)">· Claude Agent is waiting</span></div>`, send: qBtn('Implement', { kind: 'primary', chev: true, small: true }) }) }) },
    { key: 'C', name: 'The plan in a tab beside the thread', from: 'new',
      desc: 'The thread shows one line, “Plan ready: Add a checkout total”, with Open. It opens in a tab beside the thread as a document, the choices at its foot. It stays there to read while the agent works.',
      good: 'A long plan gets the room of a document, beside the work.', cost: 'Another tab to look at; the thread alone doesn’t show the plan.',
      mock: () => frame(`<div style="position:absolute;inset:0;display:flex;background:var(--panel)"><div style="width:380px;border-right:1px solid var(--b);padding:14px 16px;display:flex;flex-direction:column;justify-content:flex-end;gap:2px">${planAsk}${qRow('map', `${qVerb('Plan ready:')}${qBright('Add a checkout total')}`, { trailing: qOpen() })}${qGen('Awaiting Confirmation', 'Working for 1m 12s')}</div><div class="col grow"><div class="row" style="height:36px;border-bottom:1px solid var(--b);padding:0 12px;gap:6px;font-size:13px">${ic('map', 'sm')}Plan: Add a checkout total</div><div style="flex:1;padding:12px 18px;overflow:hidden">${PLAN_MD}</div><div class="row" style="border-top:1px solid var(--b);padding:8px 12px;gap:6px">${qBtn('No, keep planning', { kind: 'ghost' })}<span class="grow"></span>${qBtn('Yes, manually approve edits')}${qBtn('Yes, and use auto mode', { kind: 'primary' })}</div></div></div>`, { w: 900, h: 420, style: QT }) },
    { key: 'D', name: 'Rendered in the row, today’s choices', from: 'Zed',
      desc: 'As Zed shows a tool’s content: the plan rendered as markdown under the “Approve Plan” row (not in the code font, not behind “Input”), and the choices as the permission topic picks. After you answer it folds into the row, which opens to it again.',
      good: 'Small change; it stays a tool call.', cost: 'Still a tool row; folded after answering.',
      mock: () => qConv(`${planAsk}${qRow('swap', qText('Approve Plan'))}<div style="margin-left:30px;padding:4px 0">${PLAN_MD}</div>${qTodayButtons(PLAN_OPTIONS)}`, 470) },
    { key: 'E', name: 'Approve, and pick how it goes on', from: 'Zed (its permission dropdown)',
      desc: 'A’s card, with two buttons: “Keep planning” and “Approve plan”. A menu beside them says how the work goes on: auto mode (the default), auto mode after clearing the context, bypassing permissions, or approving each edit. Each is one of the agent’s choices, in shorter words.',
      good: 'One decision, then a detail; short labels.', cost: 'Rewords the agent’s choices; other agents’ plans have other choices.',
      mock: () => qConv(`${planAsk}${planCard({ style: 'position:relative;overflow:visible', foot: `${qBtn('Keep planning', { kind: 'ghost' })}<span class="grow"></span><span class="sm" style="color:var(--ph)">Then</span>${qBtn('Auto mode', { chev: true })}${qBtn('Approve plan', { kind: 'primary' })}${qMenu([['check', 'Auto mode'], ['', 'Auto mode, clear context first (34% used)'], ['', 'Bypass permissions'], ['', 'Approve each edit']], 'right:110px;bottom:40px')}` })}`, 420) },
  ],
});
