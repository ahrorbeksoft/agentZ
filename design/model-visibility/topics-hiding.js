// Hiding models: where you choose them, what the picker does with them, and whose list it is.
// model_picker.rs (the composer's picker) and settings_page.rs render_agent_defaults.

const mvHoverEye = () => `<span class="ibtn sm hov">${ic('eye-off', 'xs')}${mvTip('Hide from picker', 'right:-4px;top:-30px')}</span>`;
const mvChecklist = () => `<div class="menu" style="top:30px;right:0;width:250px;padding:0;overflow:hidden"><div class="row" style="height:34px;padding:0 10px;gap:8px;border-bottom:1px solid var(--bv)">${ic('search', 'sm mu')}<span class="ph">Search models…</span></div><div style="padding:4px">${MV_MODELS.slice(0, 9).map((name) => `<div class="it" style="gap:10px">${mvCheckbox(mvIsShown(name))}<span class="trunc">${name}</span></div>`).join('')}</div></div>`;
const mvFold = (count, { open = false } = {}) => `<div class="row" style="height:28px;padding:0 8px;gap:6px;border-radius:6px;color:var(--mu);flex:none">${ic(open ? 'chev-down' : 'chev-right', 'xs')}<span>${count} hidden</span></div>`;

TOPICS.push({
  id: 'manage', section: 'Hiding models', title: 'Where you choose the models the picker shows', size: 'wide', rec: 'A',
  now: 'The model chip beside Send (“Opus 5.5 · Extra High ⌄”) opens a picker of every model the agent offers, in the agent’s order: here Factory Droid’s 66, with a “Fast Mode” twin for many and three “[Deprecated]” ones. OpenCode offers 111 (“OpenCode Zen/…”, “GitLab Duo/…”), Claude Agent 4. “Search models…” narrows the list, and the effort’s levels follow under it. Settings › Agents › Factory Droid › Defaults has the same 66 in its Model menu, which runs off the screen. Nothing hides a model. In the mocks you use 16 of Droid’s 66.',
  nowImg: 'img/now-model-picker-droid.png',
  issues: ['Nine rows show at a time: Opus 5.5 is the fourth, but GPT-6.1 Sol is the nineteenth.', 'OpenCode’s 111 models are the <a href="img/now-model-picker-opencode.png" target="_blank">same picker</a>, and Defaults’ Model menu <a href="img/now-settings-defaults-menu.png" target="_blank">runs off the window</a>.'],
  options: [
    { key: 'A', name: 'A Models tab on the agent’s page', from: 't3code Settings › Providers › Models',
      desc: 'The agent’s page gets a Models tab after Defaults. It lists every model the agent offers, each with a switch: on, it’s in the picker (“Shown in picker”); off, it’s hidden (“Hidden from picker”) and its name dims. The list’s tools are topic 7, its star topic 9. The picker ends its models with Manage Models…, as the agent picker ends with Manage Agents…, which opens this tab.',
      good: 'One place for it, laid out as t3code does. The picker itself stays as it is.', cost: 'Hiding one more model takes a trip to Settings.',
      mock: () => mvWindow(mvAgentPage('Models', [mvModelsList()])) },
    { key: 'B', name: 'A section on the Defaults tab', from: 't3code’s list, on agentZ’s Defaults tab',
      desc: 'The same list as A, as a second section on the Defaults tab, under Defaults for New Threads: “Models in the picker”.',
      good: 'No new tab; the default model and the shown ones are side by side.', cost: 'Defaults becomes a long page: 66 rows under its two menus.',
      mock: () => mvWindow(mvAgentPage('Defaults', [mvDefaults(), mvModelsList({ title: 'Models in the picker' })], { tabs: MV_TABS })) },
    { key: 'C', name: 'One row with a checklist', from: 'new',
      desc: 'One more row on the Defaults tab, “Models in the picker”, whose menu says how many are shown (“16 of 66”). It opens a list of every model with a check box and a search; the ticked ones are in the picker.',
      good: 'Takes a single row.', cost: 'A long menu of check boxes, with no room for favorites or counts.',
      mock: () => mvWindow(mvAgentPage('Defaults', [mvDefaults({ extraRows: [mvSettingRow('Models in the picker', 'The models the composer’s picker lists.', mvDd('16 of 66', { open: true, menu: mvChecklist() }))] })], { tabs: MV_TABS })) },
    { key: 'D', name: 'Hide them from the picker itself', from: 'new',
      desc: 'Hovering a model in the picker shows an eye at its end: “Hide from picker”. Under the models, Show 50 hidden lists the hidden ones too, dimmed, each with an eye to bring it back.',
      good: 'You fix the list where you see it.', cost: 'Easy to hide one by mistake, and nowhere to see the whole list calmly.',
      mock: () => mvPickerScene(mvPicker({ rows: mvRows(mvShown(), { each: (name) => (name === 'Sonnet 5.5' ? { hover: true, end: mvHoverEye() } : {}) }), below: mvFootRow('eye', 'Show 50 hidden') })) },
    { key: 'E', name: 'As it is', from: 'today',
      desc: 'Every model shows; search finds the one you want.', good: 'Nothing to manage.', cost: 'Ten of 66 at a time, and 111 for OpenCode.',
      mock: () => mvPickerScene(mvPicker({ rows: mvRows(MV_MODELS) })) },
  ],
});

TOPICS.push({
  id: 'hidden-picker', section: 'Hiding models', title: 'What the picker does with hidden models', size: 'medium', rec: 'A',
  now: 'Every model is in the picker, and search matches any of them. In every option the hidden models are the 50 you don’t use, chosen as topic 1 says; the mocks search for “gpt”, which matches 5 shown models and 15 hidden ones. These mocks leave favorites out.',
  nowImg: 'img/now-model-picker-droid.png',
  options: [
    { key: 'A', name: 'Gone, even from search', from: 't3code model picker',
      desc: 'Hidden models aren’t listed and search doesn’t find them, as t3code’s picker leaves them out. To use one, show it again in Settings (Manage Models…).',
      good: 'The picker is only what you use.', cost: 'A model you want once means a trip to Settings.',
      mock: () => mvPickerScene(mvPicker({ query: 'gpt', rows: mvRows(mvMatches(mvShown(), 'gpt'), { query: 'gpt' }), below: mvManage() })) },
    { key: 'B', name: 'Gone, but search finds them', from: 'new',
      desc: 'The list leaves them out, but typing finds them too: after the shown matches, under a “Hidden” heading, dimmed. Picking one uses it without showing it again.',
      good: 'Every model is a search away.', cost: 'Searches come back longer, with models you hid on purpose.',
      mock: () => mvPickerScene(mvPicker({ query: 'gpt', rows: mvRows(mvMatches(mvShown(), 'gpt'), { query: 'gpt' }) + mvHead('Hidden') + mvRows(mvMatches(mvHidden(), 'gpt'), { query: 'gpt', each: () => ({ dim: true, selected: false }) }), below: mvManage() })) },
    { key: 'C', name: 'Folded at the end', from: 't3code “Legacy models”',
      desc: 'The models end with a folded “50 hidden” row, as t3code’s picker folds its legacy models. Clicking it opens them in place, dimmed; searching opens it by itself when only hidden models match.',
      good: 'Out of the way, but still in the picker.', cost: 'A row that is always there, and a long list once it’s open.',
      mock: () => mvPickerScene(mvPicker({ rows: mvRows(mvShown().slice(-9), { each: () => ({ selected: false }) }) + mvFold(50), below: mvManage() })) },
    { key: 'D', name: 'As it is', from: 'today',
      desc: 'Nothing is hidden: “gpt” finds all 20.', good: 'Nothing new.', cost: 'Fast Mode twins and old versions crowd every search.',
      mock: () => mvPickerScene(mvPicker({ query: 'gpt', rows: mvRows(mvMatches(MV_MODELS, 'gpt'), { query: 'gpt' }) })) },
  ],
});

const MV_OLD = 'Opus 4.8';
const mvWithOld = () => MV_MODELS.filter((name) => mvIsShown(name) || name === MV_OLD);
const mvDefaultSwitch = () => `<span style="position:relative;display:inline-flex">${mvSwitch(true, { disabled: true })}${mvTip('The default model is always shown in the picker', 'right:-6px;top:-32px')}</span>`;

TOPICS.push({
  id: 'hidden-current', section: 'Hiding models', title: 'When the model in use is hidden', size: 'medium', rec: 'A',
  now: 'Can’t happen today. With hiding, a thread can be on a model you hide later (here Opus 4.8, from before you hid the 4.x models), and the default for new threads (Defaults’ Model) can be one you hid. t3code moves its composer to the first shown model when its selection is hidden, and keeps custom models always shown.',
  options: [
    { key: 'A', name: 'Listed while it’s in use', from: 'new',
      desc: 'The thread’s model stays in the picker in its place, dimmed, tagged “Hidden”, with its check, until you pick another. A hidden default works the same in Defaults’ Model menu. The chip names it as always.',
      good: 'Nothing changes under you, and the picker always has the thread’s model.', cost: 'A dimmed row you might not expect.',
      mock: () => mvPickerScene(mvPicker({ rows: mvRows(mvWithOld(), { each: (name) => (name === MV_OLD ? { current: true, selected: true, dim: true, after: mvTag('Hidden') } : { current: false, selected: false }) }), below: mvManage() }), { chip: mvChip('Opus 4.8 · Extra High') }) },
    { key: 'B', name: 'The default can’t be hidden', from: 't3code “always shown” custom models',
      desc: 'In the Models list, the default model is tagged “Default” and its switch is off-limits: “The default model is always shown in the picker”. A thread already on a model you hide keeps it listed, as in A.',
      good: 'New threads never start on a hidden model.', cost: 'To hide the default, you first change it in Defaults.',
      mock: () => mvSectionPiece(mvModelsList({ query: 'opus', each: (name) => (name === MV_CURRENT ? { tags: mvTag('Default'), control: mvDefaultSwitch() } : {}) }), { h: 420 }) },
    { key: 'C', name: 'It moves to the first shown model', from: 't3code composer fallback',
      desc: 'As t3code does: a thread on a hidden model moves to the first one shown (here Auto Model), and a hidden default gives way to the first shown one for new threads.',
      good: 'The picker never shows a hidden model.', cost: 'The model changes without you choosing it.',
      mock: () => mvPickerScene(mvPicker({ rows: mvRows(mvShown(), { each: (name) => ({ current: name === 'Auto Model', selected: name === 'Auto Model' }) }), below: mvManage() }), { chip: mvChip('Auto Model · Extra High') }) },
    { key: 'D', name: 'Nothing special', from: 'new',
      desc: 'The chip still says “Opus 4.8”, but the picker has no row for it and no check.', good: 'Nothing to build.', cost: 'The picker looks as if no model is chosen.',
      mock: () => mvPickerScene(mvPicker({ rows: mvRows(mvShown(), { each: (name) => ({ current: false, selected: name === 'Auto Model' }) }), below: mvManage() }), { chip: mvChip('Opus 4.8 · Extra High') }) },
  ],
});

const MV_NEW = 'Opus 5.6';
const mvWithNew = (names) => names.flatMap((name) => (name === 'Opus 5.5' ? [MV_NEW, name] : [name]));

TOPICS.push({
  id: 'new-models', section: 'Hiding models', title: 'Models the agent adds later', size: 'medium', rec: 'A',
  now: 'Agents change their lists: Droid adds a model every few weeks, and a plan can offer more. Once some are hidden, a model that wasn’t there when you chose is either shown or hidden by default. In the mocks Droid has just added Opus 5.6.',
  options: [
    { key: 'A', name: 'Shown', from: 't3code (keeps the hidden ones)',
      desc: 'agentZ keeps the models you hid, as t3code keeps hiddenModels; anything new is in the picker until you hide it.',
      good: 'You see new models as soon as they come.', cost: 'Your short list grows by itself.',
      mock: () => mvPickerScene(mvPicker({ rows: mvRows(mvWithNew(mvShown())), below: mvManage() })) },
    { key: 'B', name: 'Hidden until you show it', from: 'new (keeps the shown ones)',
      desc: 'agentZ keeps the models you show; anything new waits in the Models list under Hidden from picker, tagged New.',
      good: 'The picker changes only when you change it.', cost: 'You find new models only by looking in Settings.',
      mock: () => mvSectionPiece(mvModelsList({ query: 'opus', names: mvWithNew(MV_MODELS), each: (name) => (name === MV_NEW ? { tags: mvNewTag() } : {}) }), { h: 420 }) },
    { key: 'C', name: 'Shown, tagged New', from: 'new',
      desc: 'As A, with a small New tag on it in the picker and the Models list until you pick it or open the Models tab.',
      good: 'New models stand out.', cost: 'One more thing in the rows.',
      mock: () => mvPickerScene(mvPicker({ rows: mvRows(mvWithNew(mvShown()), { each: (name) => (name === MV_NEW ? { after: mvNewTag() } : {}) }), below: mvManage() })) },
  ],
});

const MV_PERSONAL = mvAvatar('P', 210);
const MV_WORK = mvAvatar('W', 150);
const mvAccountPicker = (label, avatar = ic('layers', 'sm mu'), { menu = '', note = '' } = {}) => `<div class="row g3" style="position:relative;z-index:9"><span class="sm mu">Account</span>${mvDd(label, { icon: avatar, open: !!menu, menu })}${note ? `<span class="sm mu">${note}</span>` : ''}</div>`;
const mvAccountMenu = (current, { shared = true } = {}) => mvMenu([...(shared ? [[ic('layers', 'sm mu'), 'Shared', { check: current === 'Shared', hl: current === 'Shared' }], 'hr'] : []), [MV_PERSONAL, 'Personal', { check: current === 'Personal', hl: current === 'Personal' }], [MV_WORK, 'Work', { check: current === 'Work', hl: current === 'Work' }]], 'top:30px;left:0', 190);
const mvShortList = (opts = {}) => mvModelsList({ query: 'opus', ...opts });
const mvUseShared = (on, desc) => mvSection('Models', mvCard([mvSettingRow('Use shared settings', desc, mvSwitch(on))]));
// Work keeps the Fast Mode twin out of its picker.
const mvWorkOwn = (name) => (name === 'Opus 5.5 Fast Mode' ? { shown: false } : {});

TOPICS.push({
  id: 'accounts', section: 'Whose list it is', title: 'Across an agent’s accounts', size: 'wide', rec: 'A',
  now: 'In the mocks Factory Droid has two accounts here, Personal and Work. Its Defaults and Environment tabs open with an Account menu: Shared first (the layers icon), what every account uses, then each account, whose tab starts with “Use shared settings”. Zed and t3code have no accounts: Zed keeps favorites per agent, t3code hidden models per provider. Favorites (topic 9) follow the same choice. The mocks filter the list to “opus”.',
  options: [
    { key: 'A', name: 'Shared by all accounts, an account can keep its own', from: 'agentZ Defaults and Environment (Shared)',
      desc: 'The Models tab gets Defaults’ Account menu, Shared first: Shared’s list is every account’s, and beside the menu a note names the accounts that keep their own. On an account, the tab starts with “Use shared settings”: on, it says what Shared has (“16 of 66 shown · 3 favorites”) and nothing more; off (“Work sets its own.”), the account gets a copy of Shared’s list to change, and its threads’ picker uses that copy. Turned back on, the copy goes.',
      good: 'Hide a model once for every account, the way Defaults already works; an account kept to cheaper models can hide more alone.', cost: 'Once an account keeps its own, a change on Shared no longer reaches it.',
      mock: () => mvStack([
        mvPagePiece('Models', [mvShortList()], { above: mvAccountPicker('Shared', undefined, { menu: mvAccountMenu('Shared'), note: 'Work sets its own.' }), h: 440 }),
        mvPagePiece('Models', [mvUseShared(true, '16 of 66 shown · 3 favorites')], { above: mvAccountPicker('Personal', MV_PERSONAL), h: 330 }),
        mvPagePiece('Models', [mvUseShared(false, 'Work sets its own.'), mvShortList({ each: mvWorkOwn })], { above: mvAccountPicker('Work', MV_WORK), h: 560 }),
      ]) },
    { key: 'B', name: 'One list for all accounts', from: 't3code (one list per provider)',
      desc: 'The Models tab has no Account menu: one list is every account’s.',
      good: 'Nothing to keep in step.', cost: 'Accounts can’t differ, say a Work account kept to cheaper models.',
      mock: () => mvPagePiece('Models', [mvShortList()], { above: '<span class="sm mu" style="margin-top:-8px">Every account’s.</span>', h: 560 }) },
    { key: 'C', name: 'Each account’s own', from: 'agentZ Threads tab (one account at a time)',
      desc: 'The Account menu has no Shared: each account keeps a list of its own, and a new account starts with every model shown.',
      good: 'Each list stands alone.', cost: 'Hide the same 50 models again for each account.',
      mock: () => mvPagePiece('Models', [mvShortList()], { above: mvAccountPicker('Personal', MV_PERSONAL, { menu: mvAccountMenu('Personal', { shared: false }) }), h: 560 }) },
  ],
});

const mvMachinePicker = (label, menu = '') => mvDd(label, { icon: label === 'Shared' ? ic('layers', 'sm mu') : ic('laptop', 'sm mu'), open: !!menu, menu });
const mvMachineMenu = () => mvMenu([[ic('layers', 'sm mu'), 'Shared', { check: true, hl: true }], 'hr', [ic('laptop', 'sm mu'), 'This Mac'], [ic('server', 'sm mu'), 'Devbox 1']], 'top:30px;right:0', 180);

TOPICS.push({
  id: 'machines', section: 'Whose list it is', title: 'Across machines', size: 'wide', rec: 'A',
  now: 'An agent’s page is one machine’s: the header’s machine picker opens Factory Droid on This Mac or Devbox 1. Accounts live on their machine, so Defaults’ Shared is that machine’s accounts’. Thread titles, Skills, MCP Servers and Storage have a Shared every machine follows until it sets its own. t3code syncs hidden models to every device. The mocks show topic 5’s A, on Shared.',
  options: [
    { key: 'A', name: 'Shared’s list is every machine’s', from: 't3code (synced)',
      desc: 'agentZ keeps Shared’s list in the app and sends it to every machine, as it sends Thread titles’ Shared, so Shared on Devbox 1 is the same list. An account that keeps its own lives on one machine, so its list is that machine’s. A machine whose agent lacks a model just doesn’t list it.',
      good: 'Hide a model once, everywhere.', cost: 'A machine can’t differ other than through its accounts.',
      mock: () => mvPagePiece('Models', [mvShortList()], { above: mvAccountPicker('Shared', undefined, { note: 'The same on every machine.' }), h: 560 }) },
    { key: 'B', name: 'Each machine’s, with Shared', from: 'agentZ Shared (Thread titles, Skills)',
      desc: 'The header’s machine picker gets Shared, as Thread titles has: Shared’s list is every machine’s that doesn’t set its own, and on a machine “Use shared settings” comes first. The machine’s accounts then follow its list, or keep their own.',
      good: 'A machine can differ.', cost: 'Two Shareds on one tab: the machines’ and the accounts’.',
      mock: () => mvPagePiece('Models', [mvShortList()], { right: mvMachinePicker('Shared', mvMachineMenu()), h: 560 }) },
    { key: 'C', name: 'Each machine’s own', from: 'agentZ Defaults',
      desc: 'As Defaults works today: each machine keeps its accounts’ lists, Shared included.',
      good: 'No new kind of sharing.', cost: 'Hide the same models again on every machine.',
      mock: () => mvPagePiece('Models', [mvShortList()], { above: mvAccountPicker('Shared', undefined, { note: 'This Mac’s. Devbox 1 keeps its own.' }), h: 560 }) },
  ],
});
