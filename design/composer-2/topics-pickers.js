// The agent and account pickers: whether each is in use, and every agent's usage.
// agent_view.rs render_agent_picker and render_account_picker (render_account_row).

// A picker row: its icon, its name (with anything after it), a second line, its end and the check.
const c2PickRow = ({ lead, name, after = '', line2 = '', end = '', current = false, hl = false }) => `<div class="it ${hl ? 'hl' : ''}" style="${line2 ? 'height:auto;padding:5px 8px;' : ''}gap:9px">${lead}<span class="col grow" style="min-width:0;gap:1px"><span class="row" style="gap:6px;min-width:0"><span class="trunc">${name}</span>${after}</span>${line2 ? `<span class="xs mu trunc">${line2}</span>` : ''}</span>${end}${current ? c2Check() : c2NoCheck()}</div>`;
const c2AgentLead = (agent, ring = false) => ring && agent.working ? `<span style="display:inline-grid;place-items:center;width:20px;height:20px;border-radius:50%;box-shadow:0 0 0 1.5px var(--ac);flex:none">${glyph(agent.glyph, 'sm')}</span>` : glyph(agent.glyph, 'sm');
const c2AccountLead = (account, ring = false) => ring && account.working ? `<span style="display:inline-grid;place-items:center;border-radius:50%;box-shadow:0 0 0 2px var(--panel),0 0 0 3.5px var(--ac);flex:none">${c2Avatar(account)}</span>` : c2Avatar(account);
const c2Left = (left) => (left === null ? '' : `<span class="xs none" style="color:${c2LeftColor(left)}">${left}% left</span>`);
const c2Threads = (count) => `${count} thread${count === 1 ? '' : 's'}`;
const c2AgentsList = (row) => C2_AGENTS.map((agent) => row(agent)).join('') + c2Manage('Manage Agents…');
const c2AccountsList = (row) => `<div class="lbl">This thread</div>${row(C2_ACCOUNTS[0])}<div class="hr"></div><div class="lbl">Switch to</div>${C2_ACCOUNTS.slice(1).map((account) => row(account)).join('')}${c2Manage('Manage Accounts…')}`;
// The two menus side by side, as they'd open from the composer.
const c2Pickers = (agentRows, accountRows, { h = 300, agentW = 270, accountW = 310 } = {}) => frame(`<div style="position:absolute;inset:0;background:var(--panel);padding:14px;display:flex;gap:18px;align-items:flex-start">
  <div class="col" style="gap:6px"><span class="xs ph">Agent picker</span>${c2Menu(agentRows, 'position:relative', agentW)}</div>
  <div class="col" style="gap:6px"><span class="xs ph">Account picker</span>${c2Menu(accountRows, 'position:relative', accountW)}</div></div>`, { w: agentW + accountW + 60, h });

// Today's ends of the rows: nothing on agents, what's left on accounts.
const c2AccountToday = (account, extra = {}) => c2PickRow({ lead: c2AccountLead(account), name: account.name, after: account.external ? '<span class="xs ph none">· External</span>' : '', line2: account.plan, end: c2Left(account.left), current: account.current, ...extra });
const c2WorkingDot = (count) => (count ? `<span class="dot working" title="Working in ${c2Threads(count)}"></span>` : '');
const c2WorkingCount = (count) => (count ? `<span class="row none" style="gap:4px;font-size:11px;font-weight:500;color:var(--ac)">${c2Spin(10)}${count} working</span>` : '');

TOPICS.push({
  id: 'in-use', section: 'Agent and account pickers', title: 'Whether an agent or account is in use', size: 'wide', rec: 'A',
  now: 'The agent picker (“Claude Agent ⌄” in the composer, and on the new thread screen) lists the installed agents: icon, name and a check on the current one, then Manage Agents…. The account picker, shown with two or more accounts, lists “This thread”, then “Switch to”: each account’s avatar, name and plan, what’s left of its usage (“69% left”) and a check. Neither says whether an agent or account is working in another thread. In the mocks Claude Agent is working in two threads (this one, on Personal, and one on Work) and Codex in one.',
  nowImg: 'img/now-agent-picker.png',
  options: [
    { key: 'A', name: 'A working dot', from: 'agentZ sidebar status dot',
      desc: 'The sidebar’s blue working dot after the name of an agent or account with a thread working right now. Hovering the row says where: “Working in 2 threads”.',
      good: 'Quiet, and the same sign the sidebar uses for Working.', cost: 'The count is a hover away.',
      mock: () => c2Pickers(c2AgentsList((agent) => c2PickRow({ lead: c2AgentLead(agent), name: agent.name, after: c2WorkingDot(agent.working), current: agent.id === 'claude' })), c2AccountsList((account) => c2AccountToday(account, { after: `${account.external ? '<span class="xs ph none">· External</span>' : ''}${c2WorkingDot(account.working)}` }))) },
    { key: 'B', name: 'How many are working', from: 'agentZ sidebar “Working”',
      desc: 'At the row’s end, in the accent color with the sidebar’s spinner: “2 working”. Nothing when none is.',
      good: 'Says how busy each one is without hovering.', cost: 'More words in the menus, beside the usage.',
      mock: () => c2Pickers(c2AgentsList((agent) => c2PickRow({ lead: c2AgentLead(agent), name: agent.name, end: c2WorkingCount(agent.working), current: agent.id === 'claude' })), c2AccountsList((account) => c2AccountToday(account, { end: `<span class="row none" style="gap:8px">${c2WorkingCount(account.working)}${c2Left(account.left)}</span>` })), { accountW: 350 }) },
    { key: 'C', name: 'A ring on the icon', from: 'new',
      desc: 'A thin accent ring around the agent’s icon or the account’s avatar while it has a thread working.',
      good: 'Takes no room in the row.', cost: 'Easy to miss on small icons.',
      mock: () => c2Pickers(c2AgentsList((agent) => c2PickRow({ lead: c2AgentLead(agent, true), name: agent.name, current: agent.id === 'claude' })), c2AccountsList((account) => c2AccountToday(account, { lead: c2AccountLead(account, true) }))) },
    { key: 'D', name: 'A second line', from: 'agentZ account rows',
      desc: 'Under the name, in the row’s muted second line: “Working in 2 threads”. Accounts add it after the plan (“Max · working in 1 thread”).',
      good: 'Plain words.', cost: 'Every agent row becomes two lines tall.',
      mock: () => c2Pickers(c2AgentsList((agent) => c2PickRow({ lead: c2AgentLead(agent), name: agent.name, line2: agent.working ? `<span style="color:var(--ac)">Working in ${c2Threads(agent.working)}</span>` : 'Not working', current: agent.id === 'claude' })), c2AccountsList((account) => c2AccountToday(account, { line2: account.working ? `${account.plan} · <span style="color:var(--ac)">working in ${c2Threads(account.working)}</span>` : account.plan })), { h: 340 }) },
    { key: 'E', name: 'As it is', from: 'today',
      desc: 'No sign of use.', good: 'Short menus.', cost: 'You find out an account is busy only in the sidebar.',
      mock: () => c2Pickers(c2AgentsList((agent) => c2PickRow({ lead: c2AgentLead(agent), name: agent.name, current: agent.id === 'claude' })), c2AccountsList((account) => c2AccountToday(account))) },
  ],
});

// Usage in the agent picker. The mocks use the working dot of A above.
const c2UsageBar = (left) => (left === null ? '' : `<span style="display:block;height:3px;border-radius:2px;background:var(--bv);width:120px;margin-top:3px"><i style="display:block;height:100%;width:${100 - left}%;border-radius:2px;background:${left <= 15 ? 'var(--warn)' : 'var(--ac)'}"></i></span>`);
const c2GaugeEnd = (left) => (left === null ? '' : `<span class="row none" style="gap:3px;font-size:12px;color:${c2LeftColor(left)}">${ic('gauge', 'xs')}${left}%</span>`);
const c2AgentOnly = (rowFor, { h = 260, w = 320, rows } = {}) => frame(`<div style="position:absolute;inset:0;background:var(--panel);padding:14px">${c2Menu(rows ?? c2AgentsList(rowFor), 'position:relative', w)}</div>`, { w: w + 28, h });
const c2NestedAgents = () => C2_AGENTS.map((agent) => {
  const head = c2PickRow({ lead: c2AgentLead(agent), name: agent.name, after: c2WorkingDot(agent.working), end: agent.accounts === 1 ? c2Left(agent.left) : '' });
  if (agent.accounts === 1) return head;
  return head + C2_ACCOUNTS.map((account) => `<div class="it ${account.current ? 'hl' : ''}" style="padding-left:30px;gap:8px">${c2Avatar(account, 14)}<span class="row grow" style="gap:6px;min-width:0"><span class="trunc">${account.name}</span>${c2WorkingDot(account.working)}</span>${c2Left(account.left)}${account.current ? c2Check() : c2NoCheck()}</div>`).join('');
}).join('') + c2Manage('Manage Agents…');

TOPICS.push({
  id: 'agent-usage', section: 'Agent and account pickers', title: 'Every agent’s usage in the agent picker', size: 'medium', rec: 'A',
  now: 'An agent’s usage shows in its account picker (“69% left” on each account) and in the composer’s gauge, for the thread’s own account. An agent with one account (Codex and Factory Droid here) has no account picker, so its usage shows nowhere until a thread of it is open. Google Antigravity’s usage isn’t read in the mocks. The mocks use the working dot of A in the topic before.',
  nowImg: 'img/now-account-picker.png',
  options: [
    { key: 'A', name: 'What’s left, at the row’s end', from: 'agentZ account picker',
      desc: 'Each agent ends with what’s left of the account a switch would use, in the account picker’s words (“41% left”): yellow at 15% or less, red when it’s used up. For Claude Agent that’s this thread’s account; for another agent, its default. Nothing when its usage isn’t read.',
      good: 'The same words as the account picker; you see who has room before you switch.', cost: 'Agents with several accounts show only one of them.',
      mock: () => c2AgentOnly((agent) => c2PickRow({ lead: c2AgentLead(agent), name: agent.name, after: c2WorkingDot(agent.working), end: c2Left(agent.left), current: agent.id === 'claude' })) },
    { key: 'B', name: 'A thin bar under the name', from: 't3code usage window bars',
      desc: 'A short bar under the name fills with what’s used, yellow when little is left. Hovering the row gives the percentage and when it resets.',
      good: 'Reads at a glance, like a battery.', cost: 'Two lines per agent; the exact number is a hover away.',
      mock: () => c2AgentOnly((agent) => c2PickRow({ lead: c2AgentLead(agent), name: agent.name, after: c2WorkingDot(agent.working), line2: c2UsageBar(agent.left), current: agent.id === 'claude' }), { h: 290 }) },
    { key: 'C', name: 'The composer’s gauge', from: 'agentZ usage gauge',
      desc: 'The row ends with the gauge icon and the percentage, as the composer’s usage gauge shows it.',
      good: 'Matches the gauge you already know.', cost: 'The gauge shows what’s left, the icon suggests a speed.',
      mock: () => c2AgentOnly((agent) => c2PickRow({ lead: c2AgentLead(agent), name: agent.name, after: c2WorkingDot(agent.working), end: c2GaugeEnd(agent.left), current: agent.id === 'claude' })) },
    { key: 'D', name: 'Every account under its agent', from: 'new',
      desc: 'An agent with several accounts lists them under it, each with what’s left (as the account picker does); one with a single account shows its usage on its own row. Picking an account row switches to that agent on that account.',
      good: 'Every account’s usage in one menu.', cost: 'A longer menu, and the account picker repeats it.',
      mock: () => c2AgentOnly(null, { h: 340, rows: c2NestedAgents() }) },
    { key: 'E', name: 'As it is', from: 'today',
      desc: 'Usage only in the account picker and the gauge.', good: 'Short rows.', cost: 'A single-account agent’s usage is hidden until you use it.',
      mock: () => c2AgentOnly((agent) => c2PickRow({ lead: c2AgentLead(agent), name: agent.name, after: c2WorkingDot(agent.working), current: agent.id === 'claude' })) },
  ],
});
