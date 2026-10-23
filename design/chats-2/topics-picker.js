// The account picker moves into the agent picker as a submenu, and the composer shows the
// agent's icon in the account's color.

const CH2_NOW_PICKERS = 'In the composer, “Claude Agent ⌄” opens the agent picker: the installed agents, each with the working dot while one of its threads works, what’s left of the account a switch would use (“69% left”) and a check on this thread’s, then Manage Agents…. Beside it, with two or more accounts, “Personal ⌄” opens the account picker: This thread, then Switch to, each account’s avatar, name, plan, working dot and what’s left, then Manage Accounts…. The new thread screen has the same two. In the mocks only Claude Agent has several accounts: Personal (this thread’s), Work and the External account.';

TOPICS.push({
  id: 'submenu', section: 'The agent picker', title: 'Accounts inside the agent picker', size: 'wide', rec: 'A',
  now: `${CH2_NOW_PICKERS} In every option the account chip goes from the composer (its look is the last topic), and an agent with one account picks as today. The mocks use B of the next topic for the accounts.`,
  nowImg: 'img/now-account-picker.png',
  options: [
    { key: 'A', name: 'A submenu on hover', from: 'Zed’s context menu submenus',
      desc: 'An agent with two or more accounts ends with ›, and hovering its row (or → on it) opens its accounts beside the menu. Clicking the row itself picks the agent as today: this thread’s account for its own agent, the default for another.',
      good: 'One click for the agent as before; the accounts are one hover away.', cost: 'The menu shows one account’s usage on the agent’s row, the rest in the submenu.',
      mock: () => ch2WithSubmenu(c2Menu(ch2AgentRows() + c2Manage('Manage Agents…'), 'position:relative', 300), c2Menu(ch2AccountRows('B') + c2Manage('Manage Accounts…'), 'position:relative', 280), { w: 640, h: 230 }) },
    { key: 'B', name: 'The row only opens its accounts', from: 'macOS menus',
      desc: 'Like A, but the row of an agent with accounts doesn’t pick: hovering or clicking opens the submenu, and you pick an account there. The row shows no usage of its own; each account shows its own.',
      good: 'You always see which account you’re picking.', cost: 'Picking such an agent always takes the submenu.',
      mock: () => ch2WithSubmenu(c2Menu(ch2AgentRows({ ownEnd: false }) + c2Manage('Manage Agents…'), 'position:relative', 300), c2Menu(ch2AccountRows('B') + c2Manage('Manage Accounts…'), 'position:relative', 280), { w: 640, h: 230 }) },
    { key: 'C', name: 'Accounts under their agent', from: 'the composer round (topic 10 D)',
      desc: 'No submenu: an agent’s accounts are listed under it, indented, each with its usage and the working dot. Picking one switches to that agent on that account.',
      good: 'Every account at a glance.', cost: 'A long menu with several agents of several accounts.',
      mock: () => ch2PickerAlone(c2Menu(`${ch2AgentRows({ only: C2_AGENTS.slice(0, 1), chevrons: false, ownEnd: false, hl: '' })}${ch2AccountRows('B', { indent: 30, size: 14 })}${ch2AgentRows({ only: C2_AGENTS.slice(1), chevrons: false, hl: '' })}${c2Manage('Manage Agents…')}`, 'position:relative', 320), { w: 360, h: 300 }) },
    { key: 'D', name: 'This agent’s accounts first', from: 'new',
      desc: 'The menu starts with this thread’s agent’s accounts, inline, then the other agents, each with a submenu if it has accounts.',
      good: 'Switching accounts, the common case, takes no hover.', cost: 'Two kinds of rows in one menu.',
      mock: () => ch2PickerAlone(c2Menu(`<div class="lbl">Claude Agent</div>${ch2AccountRows('B')}<div class="hr"></div><div class="lbl">Other agents</div>${ch2AgentRows({ only: C2_AGENTS.slice(1), chevrons: false, hl: '' })}${c2Manage('Manage Agents…')}`, 'position:relative', 300), { w: 340, h: 310 }) },
  ],
});

TOPICS.push({
  id: 'rows', section: 'The agent picker', title: 'The account submenu’s rows', size: 'wide', rec: 'B',
  now: `${CH2_NOW_PICKERS} The mocks open the submenu of A in the topic before.`,
  nowImg: 'img/now-account-picker.png',
  options: [
    { key: 'A', name: 'As the account picker', from: 'agentZ’s account picker',
      desc: 'This thread, then Switch to, each account with its plan under its name and what’s left, then Manage Accounts…. Another agent’s accounts have no This thread: its default comes first.',
      good: 'Nothing changes in the rows themselves.', cost: 'Headers and two-line rows in a submenu.',
      mock: () => ch2WithSubmenu(c2Menu(ch2AgentRows() + c2Manage('Manage Agents…'), 'position:relative', 300), c2Menu(ch2AccountRows('A') + c2Manage('Manage Accounts…'), 'position:relative', 290), { w: 650, h: 270 }) },
    { key: 'B', name: 'One list', from: 'Zed’s context menus',
      desc: 'The accounts in their order, one line each: the avatar, the name (External marked), the working dot, what’s left and a check on the current one. Manage Accounts… at the end.',
      good: 'Short, and the same for every agent.', cost: 'No plan.',
      mock: () => ch2WithSubmenu(c2Menu(ch2AgentRows() + c2Manage('Manage Agents…'), 'position:relative', 300), c2Menu(ch2AccountRows('B') + c2Manage('Manage Accounts…'), 'position:relative', 280), { w: 640, h: 230 }) },
    { key: 'C', name: 'One list, with the plan', from: 'agentZ’s account rows',
      desc: 'Like B, with each account’s plan under its name.',
      good: 'Tells a Pro from a Max at a glance.', cost: 'Twice the height.',
      mock: () => ch2WithSubmenu(c2Menu(ch2AgentRows() + c2Manage('Manage Agents…'), 'position:relative', 300), c2Menu(ch2AccountRows('C') + c2Manage('Manage Accounts…'), 'position:relative', 280), { w: 640, h: 240 }) },
  ],
});

const ch2ComposerFrame = (chips) => jb(frame(`<div style="position:absolute;inset:0;background:var(--panel);padding-top:14px">${ch2Composer({ chips })}</div>`, { w: 720, h: 140 }));
TOPICS.push({
  id: 'chip', section: 'The agent picker', title: 'The agent in the composer', size: 'wide', rec: 'A',
  now: 'The composer shows the agent’s icon as it is in “Claude Agent ⌄”, then the account’s chip (its avatar, “Personal ⌄”) and the usage gauge (“69%”). Thread cards, their details, Go To and the Workspaces view already draw the agent’s icon white on a rounded square in the thread’s account’s color while the agent has more than one account. In every option the new thread screen’s chip does the same, and narrow, the chip is the icon alone.',
  nowImg: 'img/now-composer.png',
  options: [
    { key: 'A', name: 'On the account’s color, as the cards draw it', from: 'agentZ’s thread cards (AgentIcon)',
      desc: 'The agent’s icon white on a rounded square in the account’s color, then “Claude Agent ⌄” and the gauge. The account’s name is in the chip’s tooltip (“Claude Agent · Personal”).',
      good: 'The same mark as the thread’s card in the sidebar.', cost: 'The account’s name is a hover away.',
      mock: () => ch2ComposerFrame(`${c2Chip(ch2OnColor('claude', CH2_FILL.Personal), 'Claude Agent')}${c2Gauge(69)}`) },
    { key: 'B', name: 'The icon in the account’s color', from: 'new',
      desc: 'The icon itself is drawn in the account’s color, with no square behind it.',
      good: 'Light; the icon keeps its size.', cost: 'Unlike the cards; a thin icon shows little color.',
      mock: () => ch2ComposerFrame(`${c2Chip(ch2Tinted('claude', C2_ACCOUNTS[0].color), 'Claude Agent')}${c2Gauge(69)}`) },
    { key: 'C', name: 'With the account’s name', from: 'new',
      desc: 'Like A, with the account after the agent: “Claude Agent · Personal ⌄”.',
      good: 'Says which account without a hover.', cost: 'A wider chip, which goes to the icon sooner when narrow.',
      mock: () => ch2ComposerFrame(`${c2Chip(ch2OnColor('claude', CH2_FILL.Personal), 'Claude Agent <span style="color:var(--ph)">· Personal</span>')}${c2Gauge(69)}`) },
    { key: 'D', name: 'The account chip stays', from: 'today',
      desc: 'Like A, and the account’s chip stays beside it, opening the account picker as today.',
      good: 'Switching accounts stays one click.', cost: 'Two ways to the same accounts.',
      mock: () => ch2ComposerFrame(`${c2Chip(ch2OnColor('claude', CH2_FILL.Personal), 'Claude Agent')}${c2AccountChip()}${c2Gauge(69)}`) },
  ],
});
