// Favorites: how you mark one, where they show in the picker, other agents’ favorites, and
// keys. Zed’s model selector (agent_ui model_selector.rs, ui/model_selector_components.rs) and
// t3code’s ModelPickerContent and ModelPickerSidebar.

const mvFavorites = () => MV_MODELS.filter(mvIsFavorite);
const mvStarButton = (on, tip = '') => `<span class="ibtn sm ${tip ? 'hov' : ''}" style="position:relative">${mvStar(on)}${tip ? mvTip(tip, 'right:-8px;top:-30px') : ''}</span>`;
/** Zed’s list: Favorite, then All with every shown model, favorites again in their place. */
const mvZedRows = ({ fav = () => ({}), all = () => ({}) } = {}) => mvHead('Favorite') + mvRows(mvFavorites(), { each: fav }) + mvHead('All') + mvRows(mvShown(), { each: (name) => ({ selected: false, ...all(name) }) });
const mvFavPicker = (rows, opts = {}) => mvPickerScene(mvPicker({ rows, below: mvManage(), ...opts }));
const mvLeadStar = (name) => ({ lead: `<span style="display:inline-flex;margin-left:-2px">${mvStar(mvIsFavorite(name), { color: 'var(--warn)' })}</span>` });

TOPICS.push({
  id: 'star', section: 'Favorites', title: 'How you mark a favorite', size: 'wide', rec: 'A',
  now: 'There are no favorites. Zed’s model picker shows a star at a row’s end while you hover it (“Favorite Model”, filled in the accent color for “Unfavorite Model”). t3code’s picker has a star on every row (“Add to favorites”), and its Settings list starts each row with one. In every option but E, the Models list (topic 1’s A) has t3code’s star too. The mocks use Zed’s Favorite section (topic 10’s A) and hover Fable 5.1.',
  nowImg: 'img/now-model-picker-claude.png',
  options: [
    { key: 'A', name: 'A star on hover, at the row’s end', from: 'Zed model picker',
      desc: 'Hovering a model shows a star at its end, in place of the check, as Zed does: “Favorite Model”, or filled “Unfavorite Model” for a favorite. Clicking it doesn’t pick the model.',
      good: 'Clean rows; the Favorite section already says which are favorites.', cost: 'You find it only by hovering.',
      mock: () => mvFavPicker(mvZedRows({ all: (name) => (name === 'Fable 5.1' ? { hover: true, end: mvStarButton(false, 'Favorite Model') } : {}) })) },
    { key: 'B', name: 'A star on every row', from: 't3code model picker',
      desc: 'Every row starts with a star: outlined, or filled yellow for a favorite (“Add to favorites”, “Remove from favorites”).',
      good: 'Always in sight, the same in the picker and in Settings.', cost: 'A column of stars in a short menu.',
      mock: () => mvFavPicker(mvZedRows({ fav: mvLeadStar, all: (name) => ({ ...mvLeadStar(name), hover: name === 'Fable 5.1' }) })) },
    { key: 'C', name: 'Only in Settings', from: 't3code Settings list',
      desc: 'The picker shows the favorites, but you star them in the Models list.',
      good: 'The picker stays as it is.', cost: 'A trip to Settings for each one.',
      mock: () => mvSide([mvFavPicker(mvZedRows()), mvSectionPiece(mvModelsList({ query: 'fable' }), { h: 420, w: 560 })]) },
    { key: 'D', name: 'From the row’s menu', from: 'new',
      desc: 'Right-clicking a model opens a small menu: Add to Favorites, and Hide from Picker (topic 1).',
      good: 'Both, where you see the model.', cost: 'Nobody looks for a right-click menu in a picker.',
      mock: () => mvScene(mvPicker({ rows: mvZedRows({ all: (name) => (name === 'Fable 5.1' ? { hover: true } : {}) }), below: mvManage() }), { extra: mvMenu([[mvStar(false), 'Add to Favorites'], [ic('eye-off', 'sm'), 'Hide from Picker']], 'left:420px;top:316px', 190) }) },
    { key: 'E', name: 'No favorites', from: 'today',
      desc: 'Hiding alone keeps the list short.', good: 'Nothing more to learn.', cost: 'The models you use most sit where the agent puts them.',
      mock: () => mvFavPicker(mvRows(mvShown())) },
  ],
});

const mvSegs = (labels, on) => `<div style="padding:6px 8px 2px;flex:none"><span class="row" style="border:1px solid var(--b);border-radius:6px;overflow:hidden;font-size:12px;height:24px">${labels.map((label, index) => `<span class="row grow" style="height:100%;justify-content:center;${index ? 'border-left:1px solid var(--b);' : ''}${index === on ? 'background:var(--sel);color:var(--t)' : 'color:var(--mu)'}">${label}</span>`).join('')}</span></div>`;
const mvInlineStar = (name) => (mvIsFavorite(name) ? { after: `<span style="display:inline-flex">${mvStar(true, { size: 'xs' })}</span>` } : {});

TOPICS.push({
  id: 'favorites-place', section: 'Favorites', title: 'Where favorites show in the picker', size: 'wide', rec: 'A',
  now: 'There are none yet. Zed’s picker starts with a Favorite heading over them, then All (or the agent’s own groups) with every model, the favorites again among them. t3code’s picker opens on its Favorites view whenever you have some. The mocks use topic 2’s A (hidden models gone) and stars on hover (topic 9’s A).',
  options: [
    { key: 'A', name: 'On top, and in the list too', from: 'Zed model picker',
      desc: 'A Favorite heading over the favorites at the top, then All with every shown model in the agent’s order, favorites included. An agent that groups its models keeps its group headings in place of All.',
      good: 'Favorites are one look away, and the rest keep their order.', cost: 'Each favorite is listed twice.',
      mock: () => mvFavPicker(mvZedRows()) },
    { key: 'B', name: 'On top, once', from: 't3code Settings groups',
      desc: 'As A, but the favorites leave the list below.', good: 'No model twice.', cost: 'The rest of the list has gaps where the favorites were.',
      mock: () => mvFavPicker(mvHead('Favorites') + mvRows(mvFavorites()) + mvHead('All') + mvRows(mvShown().filter((name) => !mvIsFavorite(name)), { each: () => ({ selected: false }) })) },
    { key: 'C', name: 'Opens on favorites alone', from: 't3code Favorites view',
      desc: 'Under the search, Favorites | All. The picker opens on Favorites when there are some, as t3code’s does; All lists every shown model, and search looks through all of them.',
      good: 'The shortest picker: three rows.', cost: 'Any other model is a click or a search away.',
      mock: () => mvFavPicker(mvRows(mvFavorites()), { above: mvSegs(['Favorites', 'All'], 0) }) },
    { key: 'D', name: 'Starred in place', from: 'new',
      desc: 'No heading: every model in the agent’s order, favorites marked with a small filled star after the name.',
      good: 'One list, nothing moves.', cost: 'Favorites don’t come first, so they don’t save scrolling.',
      mock: () => mvFavPicker(mvRows(mvShown(), { each: mvInlineStar })) },
  ],
});

// Favorites across agents: Droid’s, then Claude Agent’s and Codex’s.
const mvOtherRows = ({ hoverFirst = false } = {}) => MV_OTHER_FAVORITES.map((favorite, index) => mvRow(favorite.model, { current: false, lead: glyph(favorite.kind, 'sm'), after: `<span class="xs ph trunc">${favorite.agent}</span>`, hover: hoverFirst && index === 0, })).join('');
const mvSwitchTip = () => mvTip('Switch the thread to Claude Agent on Opus 5.5', 'position:absolute;right:346px;top:236px');
const mvDroidFavRows = () => mvRows(mvFavorites(), { each: () => ({ lead: glyph('droid', 'sm') }) });
const mvRail = () => `<div class="col" style="width:40px;flex:none;border-right:1px solid var(--bv);padding:6px 0;gap:4px;align-items:center"><span class="ibtn on">${mvStar(true)}</span>${['droid', 'claude', 'codex'].map((kind) => `<span class="ibtn">${glyph(kind, 'sm')}</span>`).join('')}</div>`;
const mvNewThread = () => '<div class="col" style="padding-top:150px;gap:6px;width:240px"><span style="font-size:18px">New thread</span><span class="sm mu">agentZ · main</span></div>';

TOPICS.push({
  id: 'across', section: 'Favorites', title: 'Other agents’ favorites', size: 'wide', rec: 'A',
  now: 'A thread can switch agents between turns from the agent picker; the conversation is handed to the new agent. Zed keeps favorites per agent (for an ACP agent, per setting and value) and lists only the current agent’s. t3code’s favorites are global: its picker’s Favorites lists them from every provider, and picking one switches the provider. In the mocks Claude Agent’s favorites are Opus 5.5 and Sonnet 5.5, Codex’s is 6.1 Sol.',
  options: [
    { key: 'A', name: 'Each agent’s own', from: 'Zed',
      desc: 'The picker lists only this agent’s favorites. Changing agents stays in the agent picker.',
      good: 'The model picker only ever changes the model.', cost: 'Your Claude favorites aren’t one click away in a Droid thread.',
      mock: () => mvFavPicker(mvZedRows()) },
    { key: 'B', name: 'Every agent’s, under Favorite', from: 't3code Favorites',
      desc: 'Favorite lists this agent’s favorites, then other agents’, each with its agent’s icon and name. Picking another agent’s switches the thread to that agent and model, as the agent picker does.',
      good: 'One menu for “Opus on Claude” or “GPT on Droid”.', cost: 'The model picker can change the agent, which hands the thread over.',
      mock: () => mvScene(mvPicker({ rows: mvHead('Favorite') + mvDroidFavRows() + mvOtherRows({ hoverFirst: true }) + mvHead('All') + mvRows(mvShown(), { each: () => ({ selected: false }) }), below: mvManage() }), { extra: mvSwitchTip() }) },
    { key: 'C', name: 'Every agent’s, on new threads only', from: 'new',
      desc: 'As B, but only on the new thread screen, before the first message, where changing the agent hands nothing over. A sent thread’s picker is A’s.',
      good: 'The quick pick where it costs nothing.', cost: 'The picker differs between the two screens.',
      mock: () => mvScene(mvPicker({ rows: mvHead('Favorite') + mvDroidFavRows() + mvOtherRows() + mvHead('All') + mvRows(mvShown(), { each: () => ({ selected: false }) }), below: mvManage() }), { talk: mvNewThread() }) },
    { key: 'D', name: 'A rail of agents', from: 't3code ModelPickerSidebar',
      desc: 'A narrow rail at the picker’s left, as t3code’s: a star for every agent’s favorites, then each agent’s icon for its own models. It opens on the star.',
      good: 'Every agent’s models without leaving the picker.', cost: 'The widest, busiest picker, for a thread that mostly stays on one agent.',
      mock: () => mvPickerScene(mvPicker({ w: 328, lead: mvRail(), rows: mvDroidFavRows() + mvOtherRows(), below: mvManage() })) },
  ],
});

const mvKey = (keys) => `<span class="row none" style="gap:2px">${keys.map((key) => keycap(key)).join('')}</span>`;
const mvKeyLine = (label, keys, { rule = false } = {}) => `<div class="row" style="gap:16px;justify-content:space-between;${rule ? 'border-top:1px solid var(--bv);padding-top:5px;margin-top:5px;' : ''}"><span>${label}</span>${mvKey(keys)}</div>`;
const mvChipTip = (html) => mvScene(`<div class="pop" style="position:relative;padding:7px 10px;font-size:12px;max-width:300px;white-space:normal">${html}</div>`, { chip: mvChip('Opus 5.5 · Extra High', { open: false }), h: 330, bottom: 72, talk: '' });

TOPICS.push({
  id: 'keys', section: 'Favorites', title: 'Keys for the model picker', size: 'medium', rec: 'A',
  now: 'No key opens the model picker or changes the model; hovering the chip shows the setting’s name and description. In Zed, ⌥⌘/ opens the model selector, ⌥⇥ moves to the next favorite, and the chip’s tooltip names both (the second only with favorites).',
  options: [
    { key: 'A', name: 'Both of Zed’s', from: 'Zed agent::ToggleModelSelector, agent::CycleFavoriteModels',
      desc: '⌥⌘/ opens the model picker; ⌥⇥ moves the thread to the next favorite, in their order, wrapping around. The chip’s tooltip lists both, as Zed’s does.',
      good: 'Switch between your usual two or three without the mouse.', cost: 'Two keys to learn.',
      mock: () => mvChipTip(mvKeyLine('Change Model', ['⌥', '⌘', '/']) + mvKeyLine('Cycle Favorite Models', ['⌥', '⇥'], { rule: true })) },
    { key: 'B', name: 'Only opening it', from: 'Zed agent::ToggleModelSelector',
      desc: '⌥⌘/ opens the picker with its search ready, so typing and Enter pick a model.', good: 'One key, and it works without favorites.', cost: 'Switching to a favorite takes a few keys.',
      mock: () => mvChipTip(mvKeyLine('Change Model', ['⌥', '⌘', '/'])) },
    { key: 'C', name: 'Only cycling favorites', from: 'Zed agent::CycleFavoriteModels',
      desc: '⌥⇥ moves to the next favorite.', good: 'The fastest switch.', cost: 'Does nothing until you have favorites.',
      mock: () => mvChipTip(mvKeyLine('Cycle Favorite Models', ['⌥', '⇥'])) },
    { key: 'D', name: 'As it is', from: 'today',
      desc: 'The mouse opens the picker; the tooltip describes the setting.', good: 'Nothing new.', cost: 'Changing the model always takes the mouse.',
      mock: () => mvChipTip(`<div class="b5" style="padding-bottom:2px">Model and Reasoning Effort</div><div class="mu">${MV_MODEL_DESC}</div>`) },
  ],
});
