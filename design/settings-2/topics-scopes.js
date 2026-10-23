// A setting for all machines, and one for all accounts: an "All" value each machine (or each of
// an agent's accounts) follows unless it sets its own. The two topics offer the same shapes
// under the same letters.

const devIc = s2MachineIc('devbox', 'xs');
const workAv = s2Avatar(S2_ACCOUNTS.work, 12);
const withTip = (control, tip, style) => `<span style="position:relative;display:inline-flex">${control}${s2Tip(tip, style)}</span>`;
const scopeChip = (icon, label, open = false) => `<span class="chip" style="height:24px;gap:4px;${open ? 'border-color:var(--bf);color:var(--t)' : ''}">${icon}${label}${ic('chev-down', 'xs')}</span>`;
const scoped = (chip, control) => `<span class="row g2">${chip}${control}</span>`;
// Zed's "— Modified in <file>" after a row's title.
const modifiedIn = (who) => `<span class="sm mu" style="margin-left:4px">—  Set on ${who}</span>`;
// t3code's inheritance popover: where a row's value comes from on each machine or account.
const layers = (title, rows, style) => `<div class="pop" style="${style};width:300px;padding:6px;z-index:25"><div class="xs mu" style="padding:4px 8px 6px">${title}</div>${rows.map(([icon, who, value, own]) => `<div class="row g2" style="height:30px;padding:0 8px;border-radius:5px;${own ? 'background:var(--hov)' : ''}">${icon}<span class="grow">${who}</span><span class="${own ? '' : 'mu'}">${value}</span>${own ? s2Reset() : '<span style="width:20px"></span>'}</div>`).join('')}</div>`;

// ---- 2. A setting for all machines ---------------------------------------------------------
const titlesSection = (rows, note) => s2Section('Thread titles', rows, { note });
const titlesPage = (right, rows, note) => s2Piece(s2Page('Thread titles', [titlesSection(rows, note)], { right }));
const MACHINE_ROWS = [['mac', 'Sonnet 5.5'], ['devbox', 'Opus 5.5'], ['cloud', 'Sonnet 5.5']];

TOPICS.push({
  id: 'all-machines', section: 'Machines and accounts', title: 'A setting for all machines', size: 'wide', rec: 'A',
  now: 'What each machine’s server keeps is shown one machine at a time. Agents, Usage, Skills, MCP Servers and Storage share a machine picker in their header: Agents shows which agents that machine has installed, and an agent’s page belongs to the machine it was opened on, with Defaults for New Threads and Environment Variables for each of its accounts there; Usage is read only; Skills and MCP Servers are each machine’s lists; Storage has that machine’s Cleaning up rules. General › Thread titles has a picker of its own as its first row, for Generate thread titles, Provider, Model and Reasoning effort. Start at login is This Mac’s alone. What the app sends to every server (Auto-archive, Subthreads at once, the design defaults, ACP v2, the subthread roles) is the same everywhere and has no picker. So a cleanup rule, a skill or the titles’ model, wanted on three machines, takes three visits, and nothing says Devbox 1 differs. The mocks use Thread titles as a page with the picker in its header, as topic 1’s A makes it.',
  nowImg: 'img/now-settings-agents.png',
  issues: [
    'A machine added later starts with nothing set rather than with what the others have.',
    'Two pickers for one question: the header’s on five pages, and Thread titles’ row on General.',
    'What can’t be shared stays per machine either way: which agents are installed, the Usage page’s reads, Start at login.',
  ],
  options: [
    { key: 'A', name: '“All machines” first in the machine picker', from: 't3code’s “All environments”',
      desc: 'The machine picker gets <b>All machines</b> at the top, over a line, as t3code’s scope menu has “All environments”, and pages open on it. A change there applies to every machine that doesn’t set its own, and to machines added later. On All machines, a row some machine sets differently says so under its description, “Devbox 1 sets its own: Opus 5.5”. On one machine, a row it sets itself has Zed’s ↺ beside its title (“Reset to All machines’: Sonnet 5.5”); the rest look as they do now. Skills and MCP servers added on All machines go to every machine; a machine’s view lists those and its own. Agents and Usage keep a machine’s view only, since installs and reads are each machine’s.',
      good: 'One place to set something everywhere, in the picker the pages already have.', cost: 'Every page has two views to keep apart. A machine that’s offline takes the change when it reconnects.',
      mock: () => s2Stack([
        titlesPage(s2MachineDd('all', { open: true }), [S2_ROWS.titlesOn(), S2_ROWS.titlesProvider(), S2_ROWS.titlesModel({ after: s2Differs(devIc, 'Devbox 1', 'Opus 5.5') }), S2_ROWS.titlesEffort()], 'Every machine follows these unless it sets its own, and so do machines added later.'),
        titlesPage(s2MachineDd('devbox'), [S2_ROWS.titlesOn(), S2_ROWS.titlesProvider(), S2_ROWS.titlesModel({ titleExtra: withTip(s2Reset(true), 'Reset to All machines’: Sonnet 5.5', 'top:24px;left:0') }, 'Opus 5.5'), S2_ROWS.titlesEffort()], 'A row with ↺ is Devbox 1’s own; the others follow All machines.'),
      ]) },
    { key: 'B', name: 'One view, “Set on Devbox 1” on the rows that differ', from: 'Zed’s “Modified in”, t3code’s inheritance popover',
      desc: 'Pages that can share show one value, every machine’s, with no picker. A row a machine sets differently says so after its title, as Zed’s rows say “— Modified in settings.json”. Clicking it opens t3code’s popover of where the value comes from: each machine with its value, its own ones with ↺. Changing one machine’s value happens there. Agents, Usage and Storage’s sizes keep the picker, since what they show is each machine’s.',
      good: 'A single view, and differences show only where there are some.', cost: 'Giving one machine its own value takes the popover, and there’s no page for one machine’s skills or MCP servers.',
      mock: () => titlesPage('', [S2_ROWS.titlesOn(), S2_ROWS.titlesProvider(), `<div style="position:relative">${S2_ROWS.titlesModel({ titleExtra: modifiedIn('Devbox 1') })}${layers('Model on each machine', MACHINE_ROWS.map(([machine, value]) => [s2MachineIc(machine), s2MachineName(machine), value, machine === 'devbox']), 'top:40px;left:120px')}</div>`, S2_ROWS.titlesEffort()], 'For every machine. A row that says “Set on …” differs there.') },
    { key: 'C', name: 'A “Use All machines’” switch per section', from: 'agentZ’s project “Use General’s”',
      desc: 'As a project’s Design defaults follow General’s: the picker gets All machines, and on one machine each section starts with a <b>Use All machines’</b> switch, on, with what that means (“On · Claude · Sonnet 5.5 · Low”). Turning it off copies All machines’ values for that machine and shows its rows to change; turning it back on drops them. On All machines, a section a machine has its own of says “Devbox 1 sets its own.”',
      good: 'Already how projects work in agentZ, and a section follows or doesn’t, never half.', cost: 'One different row makes the whole section that machine’s: changing Devbox 1’s model also stops it following the provider.',
      mock: () => s2Stack([
        titlesPage(s2MachineDd('devbox'), [s2Row('Use All machines’', 'On · Claude · Sonnet 5.5 · Low', s2Switch(true))]),
        s2Piece(s2Page('Storage', [s2Section('Cleaning up', [s2Row('Use All machines’', 'Off: Devbox 1’s own rules, in place of All machines’.', s2Switch(false)), s2Row('Remove archived threads’ workspaces', 'When an archived thread’s worktree or pasture goes.', s2Dd('After a week')), s2Row('Remove merged worktrees', 'Removes worktrees whose branch is merged into the project’s default branch.', s2Switch(true))])], { right: s2MachineDd('devbox') })),
      ]) },
    { key: 'D', name: 'A scope menu on each row', from: 'new, with t3code’s mixed switch',
      desc: 'No picker on the page: each row’s control has a small menu beside it, “All machines ⌄”, listing All machines and each machine with its value. Picking Devbox 1 shows and changes its own value, and the chip says Devbox 1 while it’s shown. Where machines disagree, All machines shows “Mixed”, as t3code’s switch does when its targets differ.',
      good: 'Every row says whose value it is, with no second view.', cost: 'A menu on every row; busy, and unlike Zed’s and t3code’s rows.',
      mock: () => titlesPage('', [
        s2Row('Generate thread titles', 'Titles a thread from its first message when its agent doesn’t name it, with a coding agent’s CLI installed on the machine.', scoped(scopeChip(s2MachineIc('all', 'xs'), 'All machines'), s2Switch(true))),
        s2Row('Provider', 'The CLI that writes the titles.', scoped(scopeChip(s2MachineIc('all', 'xs'), 'All machines'), s2Dd('Claude'))),
        `<div style="position:relative;z-index:4">${s2Row('Model', 'The model the CLI writes titles with.', `<span style="position:relative">${scoped(scopeChip(s2MachineIc('all', 'xs'), 'All machines', true), s2Dd('<span class="mu">Mixed</span>'))}${s2Menu([[s2MachineIc('all'), 'All machines', { end: '<span class="mu">Mixed</span>', check: true }], 'hr', ...MACHINE_ROWS.map(([machine, value]) => [s2MachineIc(machine), s2MachineName(machine), { end: `<span class="mu">${value}</span>` }])], 'top:30px;left:0', 240)}</span>`)}</div>`,
        s2Row('Reasoning effort', 'How much the model thinks before it writes a title.', scoped(scopeChip(s2MachineIc('all', 'xs'), 'All machines'), s2Dd('Low'))),
      ], 'Each row’s menu says whose value it shows; Mixed when the machines differ.') },
    { key: 'E', name: 'As it is: each machine its own', from: 'herdr’s config on each machine',
      desc: 'Each machine keeps its own values, as herdr keeps a config file on each machine, and the pickers stay as they are.',
      good: 'Nothing new to learn; what a page shows is what that machine does.', cost: 'The same change once per machine, and a new machine starts from nothing.',
      mock: () => titlesPage(s2MachineDd('devbox', { open: true, all: false }), [S2_ROWS.titlesOn(), S2_ROWS.titlesProvider(), S2_ROWS.titlesModel({}, 'Opus 5.5'), S2_ROWS.titlesEffort()], 'Each machine titles its own threads, with the CLIs installed on it.') },
  ],
});

// ---- 3. A setting for all accounts ---------------------------------------------------------
const P = S2_ACCOUNTS.personal;
const W = S2_ACCOUNTS.work;
const ALL_ACCOUNTS_ICON = ic('layers', 'sm mu');
const defaultsRows = ({ model = {}, modelValue = 'Opus 5.5', wrap = (row) => row } = {}) => [
  wrap(s2Row('Model', '', s2Dd(modelValue), model), 'model'),
  wrap(s2Row('Effort', '', s2Dd('High')), 'effort'),
  wrap(s2Row('Mode', '', s2Dd('Default')), 'mode'),
];
const accountMenu = (chosen, all = true) => s2Menu([
  ...(all ? [[ALL_ACCOUNTS_ICON, 'All accounts', { check: chosen === 'all', hl: chosen === 'all' }], 'hr'] : []),
  [s2Avatar(P), 'Personal', { end: s2Tag('Default'), check: chosen === 'personal', hl: !all && chosen === 'personal' }],
  [s2Avatar(W), 'Work', { check: chosen === 'work' }],
], 'top:30px;left:0', 220);
const allPicker = (open = false) => s2AccountPicker(`<span class="row g15">${ALL_ACCOUNTS_ICON}All accounts</span>`, { open, menu: open ? accountMenu('all') : '' });
const accountPicker = (account, open = false, all = true) => s2AccountPicker(s2AccountEntry(account), { open, menu: open ? accountMenu(account === P ? 'personal' : 'work', all) : '' });
const agentPiece = (tab, sections) => s2Piece(s2AgentPage(tab, sections));
const envRow = (key, value) => `<div class="row g2" style="padding:8px 16px">${`<span class="row" style="width:180px;height:28px;padding:0 8px;border-radius:6px;border:1px solid var(--b);background:var(--ed);font:12px 'IBM Plex Mono',monospace">${key}</span>`}<span class="mu">=</span><span class="row grow" style="height:28px;padding:0 8px;border-radius:6px;border:1px solid var(--b);background:var(--ed);font:12px 'IBM Plex Mono',monospace">${value}</span>${ic('x', 'sm mu')}</div>`;
const accountLine = (account, extra = '') => `<div class="row g3" style="padding:10px 16px">${s2Avatar(account, 24)}<div class="col grow" style="gap:1px"><span class="row g2">${account.name}${account === P ? s2Tag('Default') : ''}</span><span class="sm mu">${account.email} · ${account.plan}</span></div>${extra}${ic('chev-right', 'sm mu')}</div>`;

TOPICS.push({
  id: 'all-accounts', section: 'Machines and accounts', title: 'A setting for all accounts', size: 'wide', rec: 'A',
  now: '“Profile” isn’t a word agentZ uses. I read it as an agent’s <b>accounts</b> (Claude Agent’s Personal and Work), whose settings are each account’s own. On an agent’s page, Defaults for New Threads (model, effort, mode) and Environment Variables are per account, through the “Account” menu over those tabs, which opens on the account for new threads; choosing a model in a thread on an account makes it that account’s default. The Account tab’s cards each have When a limit is reached (Stop or Continue at reset), and Factory Droid’s own one, which Factory keeps for each login. A new account starts from Copy settings from (another account, or Nothing), copied once. Skills and MCP servers already have an accounts menu (“Every account”, “2 of 3”). If you meant something else, say so in a comment. Two other readings: a project’s own settings (its Design defaults, with “Use General’s”), or Zed’s settings profiles (named sets of settings laid over yours, which agentZ doesn’t have). The options use the same letters as the topic before.',
  nowImg: 'img/now-settings-agent-account.png',
  issues: [
    'Copy settings from copies once: a default changed later on Personal doesn’t reach Work.',
    'An account on a smaller plan may not offer the model the others use.',
    'Accounts belong to one machine, so “all accounts” here means one agent’s accounts on one machine; with topic 2, All machines would sit above it.',
  ],
  options: [
    { key: 'A', name: '“All accounts” first in the Account menu', from: 't3code’s “All environments”, for accounts',
      desc: 'The Account menu over Defaults and Environment gets <b>All accounts</b> first, over a line, and opens on it. What’s set there applies to every one of the agent’s accounts that doesn’t set its own, and to accounts added later, so Copy settings from goes. Under a row, “Work sets its own: Sonnet 5.5”; on Work, its own rows have ↺ (“Reset to All accounts’”). An account that doesn’t offer the model picked for all uses its own default, and its row says why. Choosing a model in a thread changes the value its row shows: the account’s own where it has one, else All accounts’. On the Account tab, When a limit is reached becomes one row over the accounts for all of them, and a card shows its own only where it differs. Droid’s own stays on each card, since Factory keeps it per login.',
      good: 'The same as A in the topic before: one place, the menu already there.', cost: 'A model chosen in a Work thread changes Personal’s new threads too, unless Work sets its own.',
      mock: () => s2Stack([
        agentPiece('Defaults', [allPicker(), s2Section('Defaults for New Threads', defaultsRows({ model: { after: s2Differs(workAv, 'Work', 'Sonnet 5.5') } }), { note: 'Every account follows these unless it sets its own, and so do accounts added later.' })]),
        agentPiece('Defaults', [accountPicker(W), s2Section('Defaults for New Threads', defaultsRows({ model: { titleExtra: withTip(s2Reset(true), 'Reset to All accounts’: Opus 5.5', 'top:24px;left:0') }, modelValue: 'Sonnet 5.5' }), { note: 'A row with ↺ is Work’s own; the others follow All accounts.' })]),
        s2Piece(`<div class="col" style="padding:20px 32px;gap:10px"><div class="row" style="justify-content:space-between"><span class="sm mu">Accounts</span><span class="row g1 sm mu">${ic('plus', 'xs')}Add Account</span></div><div class="card">${s2Row('When a limit is reached', 'For every account that doesn’t set its own.', s2Dd('Stop'))}</div><div class="card">${accountLine(P)}<div style="border-top:1px solid var(--bv)">${accountLine(W, `<span class="sm" style="color:var(--warn)">Continue at reset</span>`)}</div></div></div>`),
      ]) },
    { key: 'B', name: 'One view, “Set on Work” on the rows that differ', from: 'Zed’s “Modified in”, t3code’s inheritance popover',
      desc: 'Defaults and Environment show one value for the agent’s accounts, with no Account menu (Threads keeps it, since sessions are each account’s). A row an account sets differently says “— Set on Work” after its title; clicking it shows each account with its value, its own ones with ↺, and changing one account’s value happens there.',
      good: 'A single view; differences show only where they exist.', cost: 'An account’s own variable or default is a popover away, and adding one for Work only needs a way in from there.',
      mock: () => agentPiece('Defaults', [s2Section('Defaults for New Threads', [`<div style="position:relative;z-index:4">${s2Row('Model', '', s2Dd('Opus 5.5'), { titleExtra: modifiedIn('Work') })}${layers('Model for each account', [[s2Avatar(P), 'Personal', 'Opus 5.5', false], [s2Avatar(W), 'Work', 'Sonnet 5.5', true]], 'top:40px;left:80px')}</div>`, s2Row('Effort', '', s2Dd('High')), s2Row('Mode', '', s2Dd('Default'))], { note: 'For every account. A row that says “Set on …” differs there.' })]) },
    { key: 'C', name: 'A “Use All accounts’” switch per tab', from: 'agentZ’s project “Use General’s”',
      desc: 'The Account menu gets All accounts. On one account, Defaults and Environment each start with a <b>Use All accounts’</b> switch, on, with what that means (“Opus 5.5 · High · Default”). Off, the account gets a copy to change; back on, the copy goes. All accounts says which accounts have their own.',
      good: 'Already how projects follow General, and a tab follows or doesn’t, never half.', cost: 'One different default makes all of that account’s defaults its own.',
      mock: () => s2Stack([
        agentPiece('Defaults', [accountPicker(W), s2Section('Defaults for New Threads', [s2Row('Use All accounts’', 'Opus 5.5 · High · Default', s2Switch(true))])]),
        agentPiece('Environment', [accountPicker(W), s2Section('Environment Variables', [s2Row('Use All accounts’', 'Off: Work’s own variables, in place of All accounts’.', s2Switch(false)), envRow('HTTPS_PROXY', 'http://proxy.acme.co:8080')], { right: `<span class="row g1 sm mu">${ic('plus', 'xs')}Add Variable</span>` })]),
      ]) },
    { key: 'D', name: 'A scope menu on each row', from: 'new, with t3code’s mixed switch',
      desc: 'No Account menu over Defaults and Environment: each row’s control has “All accounts ⌄” beside it, listing the accounts with their values. Picking Work shows and changes its own value. Where accounts disagree, All accounts shows “Mixed”.',
      good: 'Every row says whose value it is.', cost: 'A menu on every row, unlike the rest of Settings.',
      mock: () => agentPiece('Defaults', [s2Section('Defaults for New Threads', [
        `<div style="position:relative;z-index:4">${s2Row('Model', '', `<span style="position:relative">${scoped(scopeChip(ic('layers', 'xs'), 'All accounts', true), s2Dd('<span class="mu">Mixed</span>'))}${s2Menu([[ALL_ACCOUNTS_ICON, 'All accounts', { end: '<span class="mu">Mixed</span>', check: true }], 'hr', [s2Avatar(P), 'Personal', { end: '<span class="mu">Opus 5.5</span>' }], [s2Avatar(W), 'Work', { end: '<span class="mu">Sonnet 5.5</span>' }]], 'top:30px;left:0', 230)}</span>`)}</div>`,
        s2Row('Effort', '', scoped(scopeChip(ic('layers', 'xs'), 'All accounts'), s2Dd('High'))),
        s2Row('Mode', '', scoped(scopeChip(ic('layers', 'xs'), 'All accounts'), s2Dd('Default'))),
      ])]) },
    { key: 'E', name: 'As it is: each account its own', from: 'today',
      desc: 'Each account keeps its own defaults and variables; a new one copies another’s once, with Copy settings from.',
      good: 'Accounts never change each other.', cost: 'The same change once per account, and copies drift apart.',
      mock: () => agentPiece('Defaults', [accountPicker(P, true, false), s2Section('Defaults for New Threads', defaultsRows(), { note: 'Choosing one in a thread on this account also makes it the default.' })]) },
  ],
});
