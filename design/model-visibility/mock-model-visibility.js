// Model visibility and favorites: the demo world and the pieces this round's mocks share.
// Loaded after board/mock.js; names start with mv so they don't clash with it.

Object.assign(ICONS, {
  updown: '<path d="m7 15 5 5 5-5"/><path d="m7 9 5-5 5 5"/>',
  sparkle: '<path d="M9.937 15.5A2 2 0 0 0 8.5 14.063l-6.135-1.582a.5.5 0 0 1 0-.962L8.5 9.936A2 2 0 0 0 9.937 8.5l1.582-6.135a.5.5 0 0 1 .963 0L14.063 8.5A2 2 0 0 0 15.5 9.937l6.135 1.581a.5.5 0 0 1 0 .964L15.5 14.063a2 2 0 0 0-1.437 1.437l-1.582 6.135a.5.5 0 0 1-.963 0z"/>',
  'arrow-left': '<path d="m12 19-7-7 7-7"/><path d="M19 12H5"/>',
  'arrow-up': '<path d="m5 12 7-7 7 7"/><path d="M12 19V5"/>',
  'arrow-down': '<path d="M12 5v14"/><path d="m19 12-7 7-7-7"/>',
  'eye-off': '<path d="M10.733 5.076a10.744 10.744 0 0 1 11.205 6.575 1 1 0 0 1 0 .696 10.747 10.747 0 0 1-1.444 2.49"/><path d="M14.084 14.158a3 3 0 0 1-4.242-4.242"/><path d="M17.479 17.499a10.75 10.75 0 0 1-15.417-5.151 1 1 0 0 1 0-.696 10.75 10.75 0 0 1 4.446-5.143"/><path d="m2 2 20 20"/>',
  info: '<circle cx="12" cy="12" r="10"/><path d="M12 16v-4"/><path d="M12 8h.01"/>',
  'book': '<path d="M4 19.5v-15A2.5 2.5 0 0 1 6.5 2H19a1 1 0 0 1 1 1v18a1 1 0 0 1-1 1H6.5a1 1 0 0 1 0-5H20"/>',
  hammer: '<path d="m15 12-8.373 8.373a1 1 0 1 1-3-3L12 9"/><path d="m18 15 4-4"/><path d="m21.5 11.5-1.914-1.914A2 2 0 0 1 19 8.172V7l-2.26-2.26a6 6 0 0 0-4.202-1.756L9 2.96l.92.82A6.18 6.18 0 0 1 12 8.4V10l2 2h1.172a2 2 0 0 1 1.414.586L18.5 14.5"/>',
  database: '<ellipse cx="12" cy="5" rx="9" ry="3"/><path d="M3 5V19A9 3 0 0 0 21 19V5"/><path d="M3 12A9 3 0 0 0 21 12"/>',
  gauge: '<path d="m12 14 4-4"/><path d="M3.34 19a10 10 0 1 1 17.32 0"/>',
});
GLYPHS.droid = '❋';
GLYPHS.antigravity = '⋀';

// Factory Droid's 66 models on this Mac, in its order and its words. You use 16 of them; three
// are favorites in the topics that have favorites. The thread is on Opus 5.5 at Extra High.
const MV_MODELS = ['Auto Model', 'Fable 5.1', 'Fable 5', 'Opus 5.5', 'Opus 5.5 Fast Mode', 'Opus 5', 'Opus 5 Fast Mode', 'Opus 4.8', 'Opus 4.8 Fast Mode', 'Opus 4.7', 'Opus 4.6', 'Opus 4.5', 'Sonnet 5.5', 'Sonnet 5', 'Sonnet 4.6', 'Sonnet 4.5', 'Haiku 5.5', 'Haiku 4.5', 'GPT-6.1 Sol', 'GPT-6 Astra', 'GPT-6 Sol', 'GPT-6 Sol Fast Mode', 'GPT-6 Luna', 'GPT-6 Luna Fast Mode', 'GPT-5.6 Sol', 'GPT-5.6 Sol Fast Mode', 'GPT-5.6 Terra', 'GPT-5.6 Luna', 'GPT-5.5', 'GPT-5.5 Fast Mode', 'GPT-5.5 Pro', 'GPT-5.4', 'GPT-5.4 Fast Mode', 'GPT-5.4 Mini', 'GPT-5.4 Mini Fast Mode', 'GPT-5.3-Codex', 'GPT-5.3-Codex Fast Mode', 'GPT-5.2', 'Gemini 4 Argon', 'Gemini 3.1 Pro', 'Gemini 3.8 Flash', 'Gemini 3.7 Flash', 'Gemini 3.6 Flash', 'Gemini 3.5 Flash', 'Gemini 3 Flash', 'Inkling', 'Mistral Large 4', 'Mistral Medium 3.5', 'GLM-5.3-Flash', 'GLM-5.3', 'GLM-5.2', 'GLM-5.2 Fast', 'Kimi K3', 'Qwen3.8 Max', 'Nemotron 3.5 Super', 'Nemotron 3 Ultra', 'DeepSeek V4.1 Flash', 'DeepSeek V4 Flash 0731 [Deprecated]', 'DeepSeek V4 Pro [Deprecated]', 'MiniMax M3', 'Muse Spark 1.2', 'Beam 1.0', 'Grok 4.7', 'Grok 4.6', 'Grok 4.5', 'MiniMax M2.7 [Deprecated]'];
const MV_SHOWN = ['Auto Model', 'Fable 5.1', 'Opus 5.5', 'Opus 5.5 Fast Mode', 'Sonnet 5.5', 'Haiku 5.5', 'GPT-6.1 Sol', 'GPT-6 Sol', 'GPT-6 Sol Fast Mode', 'GPT-5.6 Sol', 'GPT-5.5 Pro', 'Gemini 4 Argon', 'Gemini 3.8 Flash', 'GLM-5.3', 'Kimi K3', 'Grok 4.7'];
const MV_FAVORITES = ['Opus 5.5', 'GPT-6.1 Sol', 'Gemini 4 Argon'];
const MV_CURRENT = 'Opus 5.5';
const MV_EFFORTS = ['Low', 'Medium', 'High', 'Extra High', 'Maximum'];
const mvIsShown = (name) => MV_SHOWN.includes(name);
const mvIsFavorite = (name) => MV_FAVORITES.includes(name);
const mvShown = () => MV_MODELS.filter(mvIsShown);
const mvHidden = () => MV_MODELS.filter((name) => !mvIsShown(name));
const mvMatches = (names, query) => names.filter((name) => !query || name.toLowerCase().includes(query.toLowerCase()));

// The other agents' favorites, for the topic on favorites across agents.
const MV_OTHER_FAVORITES = [
  { agent: 'Claude Agent', kind: 'claude', model: 'Opus 5.5' },
  { agent: 'Claude Agent', kind: 'claude', model: 'Sonnet 5.5' },
  { agent: 'Codex', kind: 'codex', model: '6.1 Sol' },
];

const MV_DIM = 'var(--ph)';
const mvCaret = '<span style="display:inline-block;width:1.5px;height:16px;background:var(--ac);vertical-align:-3px;flex:none"></span>';
const mvMark = (name, query) => {
  const index = query ? name.toLowerCase().indexOf(query.toLowerCase()) : -1;
  if (index < 0) return name;
  return `${name.slice(0, index)}<span style="color:var(--ac)">${name.slice(index, index + query.length)}</span>${name.slice(index + query.length)}`;
};
const mvStar = (on, { size = 'sm', color = 'var(--ac)' } = {}) => `<svg class="i ${size}" viewBox="0 0 24 24" style="${on ? `fill:${color};stroke:${color}` : 'color:var(--mu)'}">${ICONS.star}</svg>`;
const mvTag = (text, { color = 'var(--mu)', bg = 'var(--hov)' } = {}) => `<span class="none" style="font-size:11px;line-height:16px;padding:0 6px;border-radius:4px;background:${bg};color:${color}">${text}</span>`;
const mvNewTag = () => mvTag('New', { color: 'var(--ac)', bg: 'rgba(116,173,232,.14)' });
const mvTip = (html, style) => `<div class="pop" style="${style};padding:5px 9px;font-size:12px;white-space:nowrap;z-index:25">${html}</div>`;
const mvCheckSlot = (on) => `<span style="width:14px;display:inline-flex;flex:none">${on ? ic('check', 'sm ac') : ''}</span>`;

// A row of the model picker (model_picker.rs render_entry): an inset ListItem with the name, the
// fuzzy match in the accent color, and the check on the current model.
const mvRow = (name, { current = name === MV_CURRENT, selected = false, hover = false, dim = false, lead = '', after = '', end = '', query = '' } = {}) => `<div class="row" style="height:28px;padding:0 8px;gap:8px;border-radius:6px;flex:none;position:relative;${selected ? 'background:var(--sel);' : hover ? 'background:var(--hov);' : ''}${dim ? `color:${MV_DIM};` : ''}">${lead}<span class="trunc">${mvMark(name, query)}</span>${after}<span class="grow"></span>${end}${mvCheckSlot(current)}</div>`;
const mvHead = (text, extra = '') => `<div class="row" style="padding:6px 8px 2px;gap:6px;font-size:11px;color:var(--mu);flex:none">${text}${extra}</div>`;
/** The picker's rows for these names; `each(name)` returns a row's own options. Opened, the current
 * model is selected; searching selects the first match. */
const mvRows = (names, { query = '', each = () => ({}) } = {}) => names.map((name, index) => mvRow(name, { selected: query ? index === 0 : name === MV_CURRENT, query, ...each(name) })).join('');
const mvAvatar = (letter, hue, size = 16) => `<span style="width:${size}px;height:${size}px;border-radius:50%;flex:none;display:inline-grid;place-items:center;font-size:${Math.round(size * 0.5)}px;font-weight:600;color:#1b1f26;background:linear-gradient(135deg,hsl(${hue} 70% 82%),hsl(${hue} 42% 60%))">${letter}</span>`;
/** Menus: items are [icon html, label, flags] with flags { hl, check, end }, 'hr', or ['lbl', text]. */
const mvMenu = (items, style, width = 210) => `<div class="menu" style="${style};min-width:${width}px">${items.map((item) => {
  if (item === 'hr') return '<div class="hr"></div>';
  if (item[0] === 'lbl') return `<div class="lbl">${item[1]}</div>`;
  const [icon, label, flags = {}] = item;
  return `<div class="it ${flags.hl ? 'hl' : ''}">${icon || ''}<span class="grow">${label}</span>${flags.end || ''}${flags.check ? ic('check', 'sm') : ''}</div>`;
}).join('')}</div>`;
const mvSearch = (query = '') => `<div class="row" style="height:38px;padding:0 12px;gap:8px;border-bottom:1px solid var(--bv);flex:none;font-size:14px">${ic('search', 'sm mu')}${query ? `<span class="row">${query}${mvCaret}</span>` : `<span class="row">${mvCaret}<span class="ph">Search models…</span></span>`}</div>`;
const mvEffort = (current = 'Extra High') => `<div style="border-top:1px solid var(--bv);padding:4px;flex:none"><div class="sm mu" style="padding:2px 8px">Reasoning Effort</div>${MV_EFFORTS.map((level) => mvRow(level, { current: level === current })).join('')}</div>`;
const mvFootRow = (icon, label, { end = '', hover = false } = {}) => `<div style="border-top:1px solid var(--bv);padding:4px;flex:none"><div class="row" style="height:28px;padding:0 8px;gap:8px;border-radius:6px;${hover ? 'background:var(--hov);' : ''}">${ic(icon, 'sm mu')}<span class="grow">${label}</span>${end}</div></div>`;
const mvManage = () => mvFootRow('settings', 'Manage Models…');
// The picker (model_picker.rs): 18rem wide, the search, nine models at a time, then the effort's
// levels.
const mvPicker = ({ query = '', rows, above = '', below = '', effort = true, w = 288, maxH = 260, lead = '' } = {}) => `<div class="pop" style="position:relative;width:${w}px;display:flex;overflow:hidden"><div style="display:flex;flex:none">${lead}</div><div class="col grow" style="min-width:0">${mvSearch(query)}${above}<div style="max-height:${maxH}px;overflow:hidden;padding:4px;display:flex;flex-direction:column">${rows}</div>${below}${effort ? mvEffort() : ''}</div></div>`;

// The thread under it: its header, the end of the conversation, and the composer's card with
// the agent chip at the left and the model chip beside Send, the machine under it.
const mvChip = (label = 'Opus 5.5 · Extra High', { open = true } = {}) => `<span class="row none" style="gap:4px;height:24px;padding:0 6px;border-radius:5px;font-size:12.5px;${open ? 'background:var(--hov);color:var(--t)' : 'color:var(--mu)'}">${label}${ic('chev-down', 'xs')}</span>`;
const mvSend = () => `<span style="width:24px;height:24px;border-radius:50%;background:var(--sel);color:var(--ph);display:inline-grid;place-items:center;flex:none">${ic('arrow-up', 'xs')}</span>`;
const mvAgentChip = () => `<span class="row none" style="gap:5px;height:24px;padding:0 6px;font-size:12.5px;color:var(--mu)">${glyph('droid', 'sm')}Factory Droid${ic('chev-down', 'xs')}</span>`;
const mvTalk = () => `<div style="display:flex;justify-content:flex-end;padding:0 0 12px"><div style="max-width:70%;background:#3a3f4b;border-radius:10px;padding:8px 12px;line-height:21px">The checkout page takes 3 seconds to load. Find out why.</div></div><div style="line-height:22px">Most of it is the order summary, which asks for each item’s price one at a time. I’ll batch those into one request.</div>`;
function mvScene(over, { w = 640, h = 660, chip = mvChip(), talk = mvTalk(), bottom = 70, right = 50, extra = '' } = {}) {
  return frame(`<div class="col" style="position:absolute;inset:0;background:var(--ed)">
    <div class="row g15" style="height:36px;flex:none;padding:0 12px;border-bottom:1px solid var(--bv);font-size:13px"><span class="mono" style="background:#4b3034;color:#e06c75">AZ</span><span class="mu">agentZ</span><span class="mu">/</span><span>Speed up the checkout page</span>${ic('chev-down', 'xs mu')}</div>
    <div style="padding:20px 22px">${talk}</div></div>
  <div style="position:absolute;left:14px;right:14px;bottom:34px;height:92px;border:1px solid var(--b);border-radius:10px;background:var(--ed);box-shadow:0 6px 18px rgba(0,0,0,.28);display:flex;flex-direction:column;justify-content:space-between;padding:10px 8px 8px">
    <span class="ph" style="padding:0 4px">Message the agent…</span>
    <div class="row" style="gap:4px"><span class="ibtn">${ic('plus', 'sm')}</span>${mvAgentChip()}<span class="grow"></span>${chip}${mvSend()}</div></div>
  <div class="row g15 sm mu" style="position:absolute;left:24px;bottom:9px">${ic('server', 'xs')}This Mac</div>
  ${over ? `<div style="position:absolute;right:${right}px;bottom:${bottom}px;z-index:20">${over}</div>` : ''}${extra}`, { w, h });
}
const mvPickerScene = (picker, opts = {}) => mvScene(picker, opts);

// Settings controls as settings_page.rs draws them.
const mvSwitch = (on, { disabled = false } = {}) => `<span style="width:34px;height:20px;border-radius:10px;flex:none;display:inline-block;position:relative;vertical-align:middle;background:${on ? '#5a7099' : '#353a44'};border:1px solid ${on ? '#6f86b0' : 'var(--b)'};${disabled ? 'opacity:.4;' : ''}"><i style="position:absolute;top:2px;${on ? 'right:2px' : 'left:2px'};width:14px;height:14px;border-radius:50%;background:${on ? '#e6e9ee' : '#6b717d'}"></i></span>`;
const mvDd = (label, { icon = '', open = false, menu = '' } = {}) => `<span class="row" style="gap:6px;height:24px;padding:0 6px 0 8px;border-radius:6px;border:1px solid ${open ? 'var(--bf)' : 'var(--b)'};background:var(--hov);white-space:nowrap;position:relative;font-size:13px">${icon}${label}${ic('updown', 'xs mu')}${menu}</span>`;
const mvCheckbox = (on) => `<span style="width:14px;height:14px;border-radius:3px;flex:none;display:inline-grid;place-items:center;border:1px solid ${on ? 'var(--ac)' : 'var(--b)'};background:${on ? 'var(--ac)' : 'transparent'};color:#1b1f26">${on ? '<svg class="i xs" viewBox="0 0 24 24" style="stroke-width:3">' + ICONS.check + '</svg>' : ''}</span>`;
const mvSettingRow = (title, desc, control = '') => `<div class="row" style="padding:12px 16px;gap:24px;position:relative"><div class="col grow" style="gap:2px"><span>${title}</span>${desc ? `<span class="sm mu">${desc}</span>` : ''}</div>${control ? `<div class="none" style="position:relative">${control}</div>` : ''}</div>`;
const mvCard = (rows) => `<div class="col" style="border:1px solid var(--b);border-radius:8px;background:var(--panel)">${rows.map((row, index) => `<div style="${index < rows.length - 1 ? 'border-bottom:1px solid var(--bv)' : ''}">${row}</div>`).join('')}</div>`;
const mvSection = (title, body, { right = '', sub = '', note = '' } = {}) => `<div class="col" style="gap:8px"><div class="row" style="justify-content:space-between;gap:12px;min-height:24px;position:relative;z-index:6"><span class="row g2"><span class="sm mu">${title}</span>${sub ? `<span class="xs ph">${sub}</span>` : ''}</span>${right}</div>${body}${note ? `<span class="sm mu">${note}</span>` : ''}</div>`;

// Defaults for New Threads as it is (render_agent_defaults), in Factory Droid's words.
const MV_MODEL_DESC = 'The model used for this session. Changing this may also change the available Reasoning Effort options.';
const MV_EFFORT_DESC = 'Controls how much thinking the model performs before responding. The available options depend on the selected model.';
const mvDefaults = ({ modelControl = mvDd('Opus 5.5'), extraRows = [] } = {}) => mvSection('Defaults for New Threads', mvCard([mvSettingRow('Model', MV_MODEL_DESC, modelControl), mvSettingRow('Reasoning Effort', MV_EFFORT_DESC, mvDd('Agent’s choice')), ...extraRows]), { note: 'Choosing one in a thread also makes it the default.' });

// A model in the Models list (t3code's ProviderModelsSection row): the favorite star, the name,
// then the switch, on while it's in the picker. A hidden model's name dims.
const mvModelRow = (name, { shown = mvIsShown(name), star = true, favorite = mvIsFavorite(name), tags = '', lead = '', end = '', hover = false, control } = {}) => `<div class="row" style="height:38px;padding:0 14px 0 ${star || lead ? 10 : 16}px;gap:8px;position:relative;${hover ? 'background:var(--hov);' : ''}">${lead}${star ? `<span class="ibtn sm">${mvStar(favorite, { color: 'var(--warn)' })}</span>` : ''}<span class="row g2 grow" style="min-width:0;${shown ? '' : 'opacity:.5'}"><span class="trunc">${name}</span>${tags}</span>${end}${control ?? mvSwitch(shown)}</div>`;
const mvGroupLabel = (text) => `<div class="xs ph" style="padding:10px 16px 4px">${text}</div>`;
/** t3code's Models section: the filter, the counts, Disable all, and the list in its groups. */
function mvModelsList({ filter = true, counts = true, bulk = true, groups = true, star = true, query = '', names = MV_MODELS, each = () => ({}), right = '', note = '', title = 'Models' } = {}) {
  const shownFor = (name) => each(name).shown ?? mvIsShown(name);
  const favoriteFor = (name) => star && (each(name).favorite ?? mvIsFavorite(name));
  const rowFor = (name) => mvModelRow(name, { star, shown: shownFor(name), favorite: favoriteFor(name), ...each(name) });
  const matching = mvMatches(names, query);
  const favorites = matching.filter(favoriteFor);
  const parts = groups
    ? [['Favorites', favorites], [favorites.length ? 'All' : '', matching.filter((name) => shownFor(name) && !favorites.includes(name))], ['Hidden from picker', matching.filter((name) => !shownFor(name) && !favorites.includes(name))]]
    : [['', matching]];
  const body = `<div class="col" style="border:1px solid var(--b);border-radius:8px;background:var(--panel);overflow:hidden">${parts.filter(([, list]) => list.length).map(([label, list]) => `${label ? mvGroupLabel(label) : ''}${list.map((name, index) => `<div style="${index < list.length - 1 ? 'border-bottom:1px solid var(--bv)' : ''}">${rowFor(name)}</div>`).join('')}`).join('<div style="border-top:1px solid var(--bv)"></div>')}</div>`;
  const filterField = filter ? `<div class="field" style="height:30px">${ic('search', 'sm mu')}${query ? `<span class="row">${query}${mvCaret}</span>` : '<span class="ph">Filter models</span>'}</div>` : '';
  const sub = counts ? `${names.length} models${star ? ` · ${names.filter(favoriteFor).length} favorites` : ''} · ${names.filter((name) => !shownFor(name)).length} hidden` : '';  const controls = `<span class="row g2">${right}${bulk ? '<span class="btn sm ghost">Hide all</span>' : ''}</span>`;
  return mvSection(title, `<div class="col" style="gap:10px">${filterField}${body}</div>`, { right: controls, sub, note });
}

// The settings window: its sidebar under the headings, beside an agent's page.
const MV_NAV_ICONS = { General: 'settings', Appearance: 'eye', Notifications: 'bell', Threads: 'chat', Subthreads: 'users', 'Thread titles': 'pencil', Artifacts: 'file', Agents: 'sparkle', Usage: 'gauge', Skills: 'book', 'MCP Servers': 'hammer', Machines: 'server', Storage: 'database' };
const MV_NAV = [['h', 'agentZ'], 'General', 'Appearance', 'Notifications', ['h', 'Threads'], 'Threads', 'Subthreads', 'Thread titles', 'Artifacts', ['h', 'Agents'], 'Agents', 'Usage', 'Skills', 'MCP Servers', ['h', 'Machines'], 'Machines', 'Storage'];
const mvNav = (on = 'Agents') => MV_NAV.map((entry) => (Array.isArray(entry)
  ? `<div class="sm mu" style="padding:12px 12px 4px;flex:none">${entry[1]}</div>`
  : `<div class="row g2" style="height:28px;padding:0 8px;border-radius:6px;margin:0 4px;flex:none;${entry === on ? 'background:var(--sel)' : ''}">${ic(MV_NAV_ICONS[entry], 'sm mu')}<span class="grow trunc">${entry}</span></div>`)).join('');
function mvWindow(content, { on = 'Agents', w = 1010, h = 720 } = {}) {
  return `<div class="m win" style="width:${w}px;height:${h}px">
    <div class="titlebar">${lights()}${ic('sidebar', 'sm mu')}<div class="viewtabs"><span class="on">Agents</span><span>Workspaces</span></div></div>
    <div class="body"><div class="sidebar"><div class="sb-head" style="color:var(--t)"><span class="grow b5" style="color:var(--t)">Settings</span>${ic('x', 'sm mu')}</div><div class="sb-list" style="gap:1px">${mvNav(on)}</div><div class="sb-foot">${ic('arrow-left', 'sm')}<span>Back</span></div></div>
    <div class="grow" style="height:100%;overflow:hidden;background:var(--ed);position:relative">${content}</div></div></div>`;
}
const MV_TABS = ['Account', 'Defaults', 'Environment', 'Threads'];
const MV_TABS_MODELS = ['Account', 'Defaults', 'Models', 'Environment', 'Threads'];
const mvAgentHeading = () => `<div class="row" style="gap:16px"><span style="width:52px;height:52px;border-radius:10px;background:#3f4552;display:grid;place-items:center;flex:none">${ic('sparkle', 'lg')}</span><div class="col grow" style="gap:4px"><span class="row g2"><span style="font-size:18px;font-weight:600">Factory Droid</span><span class="row g1 xs" style="padding:1px 7px;border-radius:9px;background:rgba(161,193,129,.14);color:var(--ok)"><span class="dot done"></span>Logged in</span></span><span class="sm mu">v0.234.0 · Factory’s coding agent</span></div><span class="ibtn" style="border:1px solid var(--b);width:28px;height:28px">${ic('more', 'sm')}</span></div>`;
const mvTabs = (current, tabs) => `<div class="row" style="gap:20px;border-bottom:1px solid var(--bv);font-size:13px">${tabs.map((tab) => `<span style="padding-bottom:10px;margin-bottom:-1px;border-bottom:2px solid ${tab === current ? 'var(--ac)' : 'transparent'};color:${tab === current ? 'var(--t)' : 'var(--mu)'}">${tab}</span>`).join('')}</div>`;
const mvMachineLabel = (name = 'This Mac') => `<span class="row g15 mu">${ic('server', 'sm mu')}${name}</span>`;
/** An agent's page in Settings › Agents: back, its heading, the tabs, then the tab's sections. */
const mvAgentPage = (tab, sections, { tabs = MV_TABS_MODELS, right = mvMachineLabel(), above = '' } = {}) => `<div class="col" style="width:720px;padding:24px 32px 32px;gap:22px"><div class="row" style="justify-content:space-between;height:28px;position:relative;z-index:8"><span class="row g15 sm mu">${ic('arrow-left', 'xs')}Agents</span>${right}</div>${mvAgentHeading()}${mvTabs(tab, tabs)}${above}${sections.join('')}</div>`;
/** A piece of an agent's page, without the window, cut to `h`. */
const mvPagePiece = (tab, sections, { h = 520, ...opts } = {}) => frame(mvAgentPage(tab, sections, opts), { w: 720, h });
/** Just a section, on the page's background. */
const mvSectionPiece = (section, { h = 460, w = 656 } = {}) => frame(`<div style="padding:20px 24px">${section}</div>`, { w, h });
const mvSide = (frames, gap = 16) => `<div style="display:flex;gap:${gap}px;align-items:flex-start">${frames.join('')}</div>`;
const mvStack = (frames, gap = 12) => `<div style="display:flex;flex-direction:column;gap:${gap}px;align-items:flex-start">${frames.join('')}</div>`;
