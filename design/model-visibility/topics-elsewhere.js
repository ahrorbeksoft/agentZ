// Where else the shown models and favorites apply: Defaults, the subthread roles, and what
// agents are told. settings_page.rs render_agent_defaults, settings_page/subthread_roles.rs
// pick_menu, agentz_server tools.rs orchestrator_capabilities (advertised_models).

const mvDefaultsMenu = () => mvMenu([['', 'Agent’s choice'], ...mvShown().map((name) => ['', name, { check: name === MV_CURRENT, hl: name === MV_CURRENT }])], 'top:30px;right:0', 200);
const mvRoleChip = () => '<span class="row none" style="gap:6px;height:24px;padding:0 8px;border-radius:12px;border:1px solid var(--bf);background:var(--hov);font-size:12px"><span class="ph">1</span>' + glyph('droid', 'sm') + 'Factory Droid · Opus 5.5 · High</span>';
const mvRoleMenus = () => `${mvMenu([['lbl', 'Agent'], [glyph('droid', 'sm'), 'Factory Droid', { end: ic('chev-right', 'xs') }], ['lbl', 'Model'], ['', 'Opus 5.5', { hl: true, end: ic('chev-right', 'xs') }], ['lbl', 'Effort'], ['', 'High', { end: ic('chev-right', 'xs') }], 'hr', [ic('trash', 'sm'), 'Remove']], 'left:40px;top:96px', 200)}${mvMenu([['', 'Default'], ...mvShown().map((name) => ['', name, { check: name === MV_CURRENT }])], 'left:244px;top:150px', 190)}`;
const mvRolesPiece = () => frame(`<div style="padding:20px 24px">${mvSection('Roles', mvCard([`<div class="row" style="padding:12px 16px;gap:16px"><div class="col" style="gap:2px;width:120px"><span>Worker</span><span class="sm mu">Does the work.</span></div><span class="row g2">${mvRoleChip()}<span class="ibtn sm" style="border:1px solid var(--b);border-radius:12px">${ic('plus', 'xs')}</span></span></div>`]))}</div>${mvRoleMenus()}`, { w: 656, h: 640 });
const mvCapabilities = () => `<div style="padding:16px 22px;display:flex;flex-direction:column;gap:6px">
  <div class="row" style="min-height:24px;gap:6px;color:#8d929c;font-size:13px">${ic('hammer', 'sm')}<span>agentZ · orchestrator_capabilities</span></div>
  <div style="margin-left:22px;border:1px solid var(--bv);border-radius:6px;background:var(--panel);padding:8px 10px;font:12px/1.55 'IBM Plex Mono',Menlo,monospace;color:#c8ccd4;white-space:pre">{ "agentId": "factory-droid",
  "name": "Factory Droid",
  "models": [
    { "id": "auto", "name": "Auto Model" },
    { "id": "claude-fable-5.1", "name": "Fable 5.1" },
    { "id": "claude-opus-5-5", "name": "Opus 5.5" },
    <span class="ph">… 13 more: the shown ones, not all 66</span>
  ], … }</div></div>`;

TOPICS.push({
  id: 'elsewhere', section: 'Beyond the composer', title: 'Where else the list applies', size: 'wide', type: 'multi', rec: 'AB',
  now: 'Models are picked in two more places: Settings › Agents › an agent › Defaults (its Model menu, “Agent’s choice” first, then all 66) and Settings › Subthreads (each role’s pick has a Model submenu). Agents also learn every model an agent offers from agentZ’s orchestrator_capabilities, and pick one for the subthreads they start. The composer’s picker on the new thread screen is the same picker, so it follows anyway. Pick any.',
  nowImg: 'img/now-settings-defaults-menu.png',
  options: [
    { key: 'A', name: 'Defaults’ Model menu', from: 't3code (hidden means hidden from pickers)',
      desc: 'The menu lists Agent’s choice, then only the shown models. A hidden default stays listed as topic 3 decides.',
      good: 'A menu of 17 that fits on the screen.', cost: 'To make a hidden model the default, show it first.',
      mock: () => mvWindow(mvAgentPage('Defaults', [mvDefaults({ modelControl: mvDd('Opus 5.5', { open: true, menu: mvDefaultsMenu() }) })])) },
    { key: 'B', name: 'The subthread roles’ Model menus', from: 'new',
      desc: 'A role’s pick offers Default, then the agent’s shown models; a pick already on a hidden model keeps it.',
      good: 'The same short list wherever you choose a model.', cost: 'A role that should run on a cheap model you hid needs it shown.',
      mock: () => mvRolesPiece() },
    { key: 'C', name: 'What agents are told', from: 'new',
      desc: 'orchestrator_capabilities lists only an agent’s shown models, so agents choose among those for their subthreads.',
      good: 'Agents don’t pick the deprecated or the Fast Mode twins you avoid.', cost: 'An agent can’t use a hidden model even when it would suit the task.',
      mock: () => frame(`<div style="position:absolute;inset:0;background:var(--ed)">${mvCapabilities()}</div>`, { w: 560, h: 280 }) },
  ],
});
