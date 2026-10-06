// Skills and MCP servers kept in agentZ and given to every agent and account: where skills
// live, Settings › Skills, and Settings › MCP Servers.

Object.assign(ICONS, {
  warn: '<path d="m21.73 18-8-14a2 2 0 0 0-3.48 0l-8 14A2 2 0 0 0 4 21h16a2 2 0 0 0 1.73-3"/><path d="M12 9v4"/><path d="M12 17h.01"/>',
  'arrow-up-right': '<path d="M7 7h10v10"/><path d="M7 17 17 7"/>',
});

const SKILL_LIST = [
  { name: 'review', description: 'Review a diff for bugs and missing tests before merging. Use when asked to review changes or a pull request.' },
  { name: 'release-notes', description: 'Write release notes from the pull requests merged since the last tag.' },
  { name: 'find-skills', description: 'Helps users discover and install agent skills when they ask how to do something a skill might cover.' },
  { name: 'frontend-design', description: 'Build distinctive, production-grade web interfaces.', skipped: 'Claude Agent has its own skill named frontend-design, so it keeps that one.' },
];
const EXTRA_NAV = [['Skills', 'book'], ['MCP Servers', 'plug']];
const pageTitle = (text) => `<div style="font-size:17px;font-weight:600">${text}</div>`;
const settingsPage = (selected, body) => settingsWindow(`<div class="col" style="padding:28px 32px;gap:16px">${pageTitle(selected)}${body}</div>`, { selected, extraNav: EXTRA_NAV, h: 600 });
const agentGlyphRow = (skipOn = '') => ['claude', 'codex', 'droid', 'devin'].map((kind) => `<span style="position:relative;display:inline-flex;${kind === skipOn ? 'opacity:.4' : ''}">${glyph(kind, 'sm')}${kind === skipOn ? `<span class="warnc" style="position:absolute;right:-5px;top:-5px;display:inline-flex">${ic('warn', 'xs')}</span>` : ''}</span>`).join('');
const addMenuButton = (label, menu = '') => `<span class="row g1 sm mu" style="position:relative">${ic('plus', 'xs')}${label}${ic('chev-down', 'xs')}${menu}</span>`;

// 18. Where skills live --------------------------------------------------------------------
TOPICS.push({
  id: 'skills-home', section: 'Skills and MCP servers', title: 'Where agentZ’s skills live', size: 'wide', rec: 'B',
  now: 'agentZ has no skills of its own. Each agent loads skills from its own folders (<code>~/.claude/skills</code>, <code>~/.factory/skills</code>, …), and many also read the shared <code>~/.agents/skills</code>, where you have <code>find-skills</code>. Either way agentZ links each skill into every account’s skills folder, one link per skill, so the agents’ own skills stay beside them, and skips a skill when the agent has its own of the same name. The plan picked A; Zed does B.',
  options: [
    { key: 'A', name: 'agentZ’s own folder', from: 'the plan',
      desc: 'Skills live in agentZ’s data folder and are linked into every account, External included. The agents’ CLIs see them in a terminal only through those links.',
      good: 'agentZ owns its list; removing a skill can’t affect another tool.', cost: 'Skills already in <code>~/.agents/skills</code> don’t show until imported, and agents that read that folder then see them twice.',
      mock: () => piece(`<div style="padding:16px">${term([C('ph', '~/Library/Application Support/agentZ/'), `└ ${B('skills/')}`, '   ├ review/SKILL.md', '   └ release-notes/SKILL.md', '', C('ph', 'linked into each account:'), `accounts/claude/work/skills/review ${C('ac', '→')} …/agentZ/skills/review`, `~/.claude/skills/review ${C('ac', '→')} …/agentZ/skills/review ${C('ph', '(External)')}`], 'border-radius:6px;border:1px solid var(--bv);line-height:19px;padding:10px 12px')}</div>`, 640) },
    { key: 'B', name: '<code>~/.agents/skills</code>, as Zed does', from: 'Zed’s Skills page',
      desc: 'Zed’s global skills folder, which Codex, Devin, Grok, OpenCode, Kilo, Qoder and Amp read already. agentZ links it only into the agents that don’t (Claude Agent, Factory Droid, Gemini CLI, Cursor, Copilot). Settings › Skills lists what’s there, <code>find-skills</code> included.',
      good: 'Zed’s choice; your existing skills show up at once, and the agents’ CLIs get them too.', cost: 'Removing a skill removes it for every tool that reads the folder (Zed’s delete confirm says so).',
      mock: () => piece(`<div style="padding:16px">${term([C('ph', '~/.agents/'), `└ ${B('skills/')}`, '   ├ find-skills/SKILL.md', '   ├ review/SKILL.md', '   └ release-notes/SKILL.md', '', C('ph', 'read as is by Codex, Devin, Grok, OpenCode, Kilo, …'), C('ph', 'linked into Claude, Droid, Gemini, Cursor and Copilot accounts:'), `accounts/claude/work/skills/review ${C('ac', '→')} ~/.agents/skills/review`], 'border-radius:6px;border:1px solid var(--bv);line-height:19px;padding:10px 12px')}</div>`, 640) },
  ],
});

// 19. Settings › Skills --------------------------------------------------------------------
const skillRow = (skill, { trailing = '', index = 0 } = {}) => `<div class="row g3" style="padding:11px 14px;${index ? 'border-top:1px solid var(--bv)' : ''}"><span class="col grow" style="gap:2px;min-width:0"><span class="row g15">${skill.name}${skill.skipped ? `<span class="warnc" style="display:inline-flex">${ic('warn', 'xs')}</span>` : ''}</span><span class="sm mu">${skill.description}</span>${skill.skipped ? `<span class="xs warnc">${skill.skipped}</span>` : ''}</span>${trailing}</div>`;
const skillButtons = () => `<span class="row g1 none">${ibtn('trash')}${obtn(`Open${ic('arrow-up-right', 'xs')}`)}</span>`;
const SKILL_ADD_MENU = `<div class="menu" style="top:22px;right:0;min-width:200px"><div class="it hl">${ic('folder', 'sm')}<span class="grow">Add from Folder…</span></div><div class="it">${ic('plus', 'sm')}<span class="grow">Create a Skill</span></div></div>`;

TOPICS.push({
  id: 'skills', section: 'Skills and MCP servers', title: 'Settings › Skills', size: 'wide', rec: 'A',
  now: 'There’s no Skills page. The mocks put Skills and MCP Servers after Agents in the settings list.',
  options: [
    { key: 'A', name: 'Zed’s Skills page', from: 'Zed skills_setup.rs',
      desc: 'Each skill as a row: its name and description from <code>SKILL.md</code>, a warning when an agent skips it (and why), a delete button that asks first, and Open ↗ for its <code>SKILL.md</code>. Add Skill offers Add from Folder… and Zed’s Create a Skill (a folder with a new <code>SKILL.md</code>). Empty, it says Zed’s “No global skills installed.”',
      good: 'Zed’s page; short and clear.', cost: 'Which agents load a skill only shows when one doesn’t.',
      mock: () => settingsPage('Skills', `<div class="col" style="gap:8px">${listHead('Every agent and account loads these in agentZ threads.', addMenuButton('Add Skill', SKILL_ADD_MENU))}<div class="card" style="overflow:visible">${SKILL_LIST.map((skill, index) => skillRow(skill, { index, trailing: skillButtons() })).join('')}</div></div>`) },
    { key: 'B', name: 'With the agents that load each', from: 'new',
      desc: 'As A, plus the icons of the agents that load each skill; an agent that skips one is faded, with the reason on hover.',
      good: 'Shows reach at a glance.', cost: 'Four to ten icons on every row, and most rows show all of them.',
      mock: () => settingsPage('Skills', `<div class="col" style="gap:8px">${listHead('Every agent and account loads these in agentZ threads.', addMenuButton('Add Skill'))}<div class="card">${SKILL_LIST.map((skill, index) => skillRow({ ...skill, skipped: '' }, { index, trailing: `<span class="row g2 none" style="margin-right:6px">${agentGlyphRow(skill.skipped ? 'claude' : '')}</span>${skillButtons()}` })).join('')}</div></div>`) },
    { key: 'C', name: 'A Skills tab on each agent', from: 'new',
      desc: 'No Skills page: each agent’s page gets a Skills tab with agentZ’s skills (added and removed there) and, below, the agent’s own, read-only.',
      good: 'Shows exactly what one agent gets, its own skills included.', cost: 'The same list repeated on every agent; adding one for all agents means doing it on any one of them.',
      mock: () => settingsWindow(agentPage(`<div class="col" style="gap:8px">${listHead('From agentZ', addMenuButton('Add Skill'))}<div class="card">${SKILL_LIST.slice(0, 3).map((skill, index) => skillRow(skill, { index, trailing: ibtn('trash') })).join('')}</div></div><div class="col" style="gap:8px"><span class="sm mu">Claude Agent’s own</span><div class="card">${skillRow({ name: 'frontend-design', description: 'In ~/.claude/skills. Used instead of agentZ’s skill of the same name.' })}</div></div>`, { tab: 'Skills', tabs: ['Account', 'Defaults', 'Environment', 'Skills', 'Threads'] }), { h: 700 }) },
  ],
});

// 20. Settings › MCP Servers ---------------------------------------------------------------
const MCP_SERVERS = [
  { name: 'github', kind: 'Local', detail: 'npx -y @modelcontextprotocol/server-github', on: true },
  { name: 'linear', kind: 'Remote', detail: 'https://mcp.linear.app/mcp', on: true, note: 'Not given to Factory Droid, which takes local servers only.' },
  { name: 'postgres', kind: 'Local', detail: 'uvx mcp-server-postgres postgresql://localhost/storefront', on: false },
];
const serverRow = (server, index) => `<div class="row g3" style="padding:11px 14px;${index ? 'border-top:1px solid var(--bv)' : ''}"><span class="col grow" style="gap:2px;min-width:0"><span class="row g2">${server.name}${tag(server.kind)}</span><span class="xs mu mono-font trunc">${server.detail}</span>${server.note ? `<span class="xs ph">${server.note}</span>` : ''}</span><span class="row g1 none">${ibtn('settings')}${ibtn('trash')}</span>${toggle(server.on)}</div>`;
const formField = (label, value, detail = '') => `<div class="col" style="gap:4px"><span class="sm">${label}</span>${detail ? `<span class="xs ph">${detail}</span>` : ''}<span class="field">${value}</span></div>`;
const MCP_ADD_MENU = `<div class="menu" style="top:22px;right:0;min-width:200px"><div class="it hl">${ic('terminal', 'sm')}<span class="grow">Add Local Server</span></div><div class="it">${ic('globe', 'sm')}<span class="grow">Add Remote Server</span></div></div>`;
const MCP_NOTE = 'Every agent and account gets these in agentZ threads.';

TOPICS.push({
  id: 'mcp', section: 'Skills and MCP servers', title: 'Settings › MCP Servers', size: 'wide', rec: 'A',
  now: 'There’s no MCP Servers page. agentZ gives every thread’s agent only its own <code>agentz</code> server. The servers added here go to every agent and account in agentZ threads (not to the CLIs in a terminal); remote ones only to agents that take them, and agents that ignore ACP’s servers (Cline, Cortex Code) get none.',
  options: [
    { key: 'A', name: 'Zed’s MCP Servers page', from: 'Zed mcp_servers_page.rs',
      desc: 'Servers as rows: name, Local or Remote, the command or URL, configure and delete buttons, and a switch to turn one off. A line says which agents can’t take a server. Add Server offers Add Local Server and Add Remote Server, each a dialog in Zed’s words: Server Name, Command, Arguments, Environment Variables; or URL and Headers.',
      good: 'Zed’s page and wording.', cost: 'No live status: each agent starts its own copy of a server, so there’s no one status to show.',
      mock: () => settingsPage('MCP Servers', `<div class="col" style="gap:8px">${listHead(MCP_NOTE, addMenuButton('Add Server', MCP_ADD_MENU))}<div class="card" style="overflow:visible">${MCP_SERVERS.map(serverRow).join('')}</div></div>`) },
    { key: 'B', name: 'A JSON editor', from: 'Zed’s settings.json',
      desc: 'One editor with the servers as JSON, in the shape agents use in their own configs, so a server’s README snippet pastes in as is.',
      good: 'Fast to copy servers in; everything in one place.', cost: 'Typing JSON; mistakes only show on save.',
      mock: () => settingsPage('MCP Servers', `<span class="sm mu">${MCP_NOTE}</span><div class="card" style="padding:10px 0">${term(['{', `  ${C('ac', '"github"')}: {`, `    ${C('ac', '"command"')}: ${C('ok', '"npx"')},`, `    ${C('ac', '"args"')}: [${C('ok', '"-y"')}, ${C('ok', '"@modelcontextprotocol/server-github"')}]`, '  },', `  ${C('ac', '"linear"')}: {`, `    ${C('ac', '"url"')}: ${C('ok', '"https://mcp.linear.app/mcp"')}`, '  }', '}'], 'background:none;line-height:19px')}</div><span class="sm ph">Saved when you leave the page.</span>`) },
    { key: 'C', name: 'A page per server', from: 'agentZ’s Add Custom Agent page',
      desc: 'Add Server and each server’s row open a page like Add Custom Agent: name, command, arguments, environment variables (or URL and headers), saved as you type.',
      good: 'Matches a page agentZ already has.', cost: 'A whole page for a few fields; the list and the form are never in view together.',
      mock: () => settingsWindow(`<div class="col" style="padding:28px 32px;gap:16px"><span class="row g15 sm mu">${ic('arrow-left', 'xs')}MCP Servers</span>${pageTitle('github')}<div class="col" style="gap:14px">${formField('Server Name', 'github')}${formField('Command', '<span class="mono-font sm">npx</span>', 'The program that starts the server, on the machine the agent runs on.')}${formField('Arguments', '<span class="mono-font sm">-y @modelcontextprotocol/server-github</span>')}<div class="col" style="gap:4px"><span class="sm">Environment Variables</span><div class="row g2"><span class="field grow mono-font sm">GITHUB_PERSONAL_ACCESS_TOKEN</span><span class="field grow mono-font sm">••••••••••••</span>${ibtn('x')}</div><span class="row g1 sm mu">${ic('plus', 'xs')}Add Variable</span></div></div></div>`, { selected: 'MCP Servers', extraNav: EXTRA_NAV, h: 600 }) },
  ],
});

// 21. Which accounts get a skill or server ---------------------------------------------------
const ACME_DOCS = { name: 'acme-docs', kind: 'Remote', detail: 'https://mcp.acme.co/mcp', on: true, note: 'The company’s docs. Also loads on Side, a personal account.' };
/** The "which accounts" menu on a row: a section per agent, a check where it loads. */
const reachMenu = (style = 'top:28px;right:0') => menuList([
  ['lbl', 'Claude Agent'],
  ['', `${avatar(ACCTS.ext, 16)}&nbsp; alex@hey.com`, { check: true }],
  ['', `${avatar(ACCTS.work, 16)}&nbsp; Work`, { check: true }],
  ['', `${avatar(ACCTS.side, 16)}&nbsp; Side`, { hl: true }],
  ['lbl', 'Codex'],
  ['', `${avatar(CODEX_WORK, 16)}&nbsp; Work`, { check: true }],
], style, 240);

TOPICS.push({
  id: 'reach', section: 'Skills and MCP servers', title: 'Keeping one to some accounts', size: 'wide', rec: 'A',
  now: 'Everything agentZ manages loads everywhere: skills are linked into every account’s skills folder, and MCP servers go to every session’s agent. A work-only server (the company’s docs, a database) then also loads on a personal account, and its tools fill every thread’s tool list.',
  options: [
    { key: 'A', name: 'Every account', from: 't3code',
      desc: 'One list, no choice: every account of every agent loads agentZ’s skills and MCP servers. t3code does the same: its MCP servers go to every provider instance.',
      good: 'Nothing to set; nothing to forget to turn on.', cost: 'A work-only server reaches personal accounts too.',
      mock: () => settingsPage('MCP Servers', `<div class="col" style="gap:8px">${listHead(MCP_NOTE, addMenuButton('Add Server', MCP_ADD_MENU))}<div class="card" style="overflow:visible">${[...MCP_SERVERS, ACME_DOCS].map(serverRow).join('')}</div></div>`) },
    { key: 'B', name: 'An accounts menu on each row', from: 'new',
      desc: 'Each skill and server gets a menu beside its controls: “Every account” at first, or the accounts it loads on, grouped by agent, with a check each. A new account starts checked, as Zed’s new threads follow the default profile.',
      good: 'Keeps work tools off personal accounts.', cost: 'A control on every row; the answer to “why doesn’t the agent see it” lives in a menu.',
      mock: () => settingsPage('Skills', `<div class="col" style="gap:8px">${listHead('agentZ’s skills, and the accounts that load them.', addMenuButton('Add Skill'))}<div class="card" style="overflow:visible">${SKILL_LIST.slice(0, 3).map((skill, index) => skillRow(skill, { index, trailing: `<span class="row g2 none" style="margin-right:6px">${dropdown(skill.name === 'release-notes' ? '3 of 4' : 'Every account', skill.name === 'release-notes' ? reachMenu() : '')}${skillButtons()}</span>` })).join('')}</div></div>`) },
    { key: 'C', name: 'Switches on each account', from: 'Zed’s profiles',
      desc: 'Zed’s tool profiles work the other way around: the profile names the tools it keeps. Here each account’s card gets “Skills and MCP servers” with a switch per item, on at first.',
      good: 'One account’s whole toolset in one place, as in Zed.', cost: 'Adding a skill for every account means visiting each account; nothing shows it on the Skills page.',
      mock: () => piece(agentPage(accountsList([
        accountCard(ACCTS.ext),
        accountCard(ACCTS.work, { body: windowsBlock(ACCTS.work) + `<div style="padding:0 16px 14px 56px"><div class="sm mu" style="margin-bottom:8px">Skills and MCP servers</div>${[['review', true], ['release-notes', true], ['github', true], ['acme-docs', true], ['postgres', false]].map(([name, on], index) => `<div class="row g3" style="height:26px"><span class="grow sm">${name}</span>${toggle(on)}</div>`).join('')}</div>` }),
        accountCard(ACCTS.side),
      ]))) },
  ],
});
