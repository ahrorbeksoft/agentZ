// What agentZ's pages share: icons, building elements, links to the server's API (each with
// the token), menus, the theme as the app changes it, and how artifacts are described.
// Everything an agent wrote goes in as text, never as HTML.

const ICONS = {
  home: '<path d="M15 21v-8a1 1 0 0 0-1-1h-4a1 1 0 0 0-1 1v8"/><path d="M3 10a2 2 0 0 1 .709-1.528l7-5.999a2 2 0 0 1 2.582 0l7 5.999A2 2 0 0 1 21 10v9a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z"/>',
  download: '<path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/><polyline points="7 10 12 15 17 10"/><line x1="12" x2="12" y1="15" y2="3"/>',
  file: '<path d="M15 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V7Z"/><path d="M14 2v4a2 2 0 0 0 2 2h4"/><path d="M10 9H8"/><path d="M16 13H8"/><path d="M16 17H8"/>',
  board: '<rect width="7" height="9" x="3" y="3" rx="1"/><rect width="7" height="5" x="14" y="3" rx="1"/><rect width="7" height="9" x="14" y="12" rx="1"/><rect width="7" height="5" x="3" y="16" rx="1"/>',
  layers: '<path d="m12.83 2.18a2 2 0 0 0-1.66 0L2.6 6.08a1 1 0 0 0 0 1.83l8.58 3.91a2 2 0 0 0 1.66 0l8.58-3.9a1 1 0 0 0 0-1.83Z"/><path d="m22 17.65-9.17 4.16a2 2 0 0 1-1.66 0L2 17.65"/><path d="m22 12.65-9.17 4.16a2 2 0 0 1-1.66 0L2 12.65"/>',
  inbox: '<polyline points="22 12 16 12 14 15 10 15 8 12 2 12"/><path d="M5.45 5.11 2 12v6a2 2 0 0 0 2 2h16a2 2 0 0 0 2-2v-6l-3.45-6.89A2 2 0 0 0 16.76 4H7.24a2 2 0 0 0-1.79 1.11z"/>',
  chat: '<path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z"/>',
  archive: '<rect width="20" height="5" x="2" y="3" rx="1"/><path d="M4 8v11a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8"/><path d="M10 12h4"/>',
  server: '<rect width="20" height="8" x="2" y="2" rx="2" ry="2"/><rect width="20" height="8" x="2" y="14" rx="2" ry="2"/><line x1="6" x2="6.01" y1="6" y2="6"/><line x1="6" x2="6.01" y1="18" y2="18"/>',
  check: '<path d="M20 6 9 17l-5-5"/>',
  'check-circle': '<circle cx="12" cy="12" r="10"/><path d="m9 12 2 2 4-4"/>',
  history: '<path d="M3 12a9 9 0 1 0 9-9 9.75 9.75 0 0 0-6.74 2.74L3 8"/><path d="M3 3v5h5"/><path d="M12 7v5l4 2"/>',
  'chev-down': '<path d="m6 9 6 6 6-6"/>',
  'chev-left': '<path d="m15 18-6-6 6-6"/>',
  'chev-right': '<path d="m9 18 6-6-6-6"/>',
  x: '<path d="M18 6 6 18"/><path d="m6 6 12 12"/>',
  search: '<circle cx="11" cy="11" r="8"/><path d="m21 21-4.3-4.3"/>',
  external: '<path d="M15 3h6v6"/><path d="M10 14 21 3"/><path d="M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6"/>',
  wifi: '<path d="M12 20h.01"/><path d="M8.5 16.43a5 5 0 0 1 7 0"/><path d="M5 12.86a10 10 0 0 1 5.17-2.69"/><path d="M19 12.86a10 10 0 0 0-2.007-1.523"/><path d="M2 8.82a15 15 0 0 1 4.177-2.643"/><path d="M22 8.82a15 15 0 0 0-11.288-3.764"/><path d="m2 2 20 20"/>',
  monitor: '<rect width="20" height="14" x="2" y="3" rx="2"/><line x1="8" x2="16" y1="21" y2="21"/><line x1="12" x2="12" y1="17" y2="21"/>',
  phone: '<rect width="14" height="20" x="5" y="2" rx="2" ry="2"/><path d="M12 18h.01"/>',
};

function icon(name, size = 14) {
  const svg = document.createElementNS('http://www.w3.org/2000/svg', 'svg');
  for (const [key, value] of Object.entries({ width: size, height: size, viewBox: '0 0 24 24', fill: 'none', stroke: 'currentColor', 'stroke-width': 2, 'stroke-linecap': 'round', 'stroke-linejoin': 'round' })) {
    svg.setAttribute(key, value);
  }
  svg.innerHTML = ICONS[name] || '';
  return svg;
}

// An element: `h('a', { class: 'x', text: 'Open', onclick }, child, …)`. Strings are text.
function h(tag, props, ...children) {
  const element = document.createElement(tag);
  for (const [key, value] of Object.entries(props || {})) {
    if (value === undefined || value === null || value === false) continue;
    if (key === 'class') element.className = value;
    else if (key === 'text') element.textContent = value;
    else if (key.startsWith('on')) element.addEventListener(key.slice(2), value);
    else if (key === 'style' && typeof value === 'object') Object.assign(element.style, value);
    else element.setAttribute(key, value === true ? '' : value);
  }
  for (const child of children.flat()) {
    if (child === null || child === undefined || child === false) continue;
    element.append(child instanceof Node ? child : String(child));
  }
  return element;
}

const $ = (id) => document.getElementById(id);

// `replaceChildren`, skipping what's left out: the DOM's own would write "null".
function fill(element, ...children) {
  element.replaceChildren(...children.flat().filter((child) => child !== null && child !== undefined && child !== false));
}

// An agent's icon is SVG from its registry entry: drawn as a mask in the text color, so
// nothing in it can run.
function agentIcon(svg, size = 14) {
  const url = `url("data:image/svg+xml,${encodeURIComponent(svg)}")`;
  return h('span', { class: 'agent', style: { width: size + 'px', height: size + 'px', background: 'currentColor', maskImage: url, webkitMaskImage: url, maskSize: 'contain', webkitMaskSize: 'contain', maskRepeat: 'no-repeat', webkitMaskRepeat: 'no-repeat', maskPosition: 'center', webkitMaskPosition: 'center', display: 'inline-block', flex: 'none' } });
}

function ago(ms) {
  if (!ms) return '';
  const seconds = Math.max(1, Math.round((Date.now() - ms) / 1000));
  if (seconds < 60) return 'just now';
  const minutes = Math.round(seconds / 60);
  if (minutes < 60) return minutes + 'm ago';
  const hours = Math.round(minutes / 60);
  if (hours < 24) return hours + 'h ago';
  const days = Math.round(hours / 24);
  if (days < 30) return days + 'd ago';
  return new Date(ms).toLocaleDateString();
}

function query(params) {
  const search = new URLSearchParams();
  for (const [key, value] of Object.entries(params || {})) {
    if (value !== undefined && value !== null && value !== '') search.set(key, value);
  }
  const text = search.toString();
  return text ? '?' + text : '';
}

const api = (path, params = {}) => path + query({ t: DATA.token, ...params });

async function getJson(path, params) {
  const answer = await fetch(api(path, params));
  if (!answer.ok) throw new Error((await answer.text()) || answer.statusText);
  return answer.json();
}

// `{ error }` when it didn't go through.
async function postJson(path, body) {
  try {
    const answer = await fetch(api(path), { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify(body) });
    if (answer.ok) return {};
    const reply = await answer.json().catch(() => ({}));
    return { error: reply.error || answer.statusText || 'It didn’t go through.' };
  } catch (error) {
    return { error: 'agentZ’s server isn’t answering.' };
  }
}

const artifactBase = (machine, id) => (machine ? '/m/' + encodeURIComponent(machine) : '') + '/a/' + encodeURIComponent(id);
const artifactUrl = (machine, id, version) => artifactBase(machine, id) + query({ t: DATA.token, v: version });
const fileUrl = (machine, id, version, name, key) => artifactBase(machine, id) + '/files/' + version + '/' + encodeURIComponent(name) + query({ key });
const homeUrl = (project) => '/' + query({ t: DATA.token, p: project });

let closing = null;
// A menu under `anchor`: items of `{ label, when, href, download, click }`.
function menu(items, anchor, { align = 'left' } = {}) {
  closeMenus();
  const element = h('div', { class: 'menu' });
  const rect = anchor.getBoundingClientRect();
  element.style.top = rect.bottom + 4 + 'px';
  if (align === 'right') element.style.right = Math.max(8, window.innerWidth - rect.right) + 'px';
  else element.style.left = Math.max(8, rect.left) + 'px';
  for (const item of items) {
    const row = h(item.href ? 'a' : 'button', { href: item.href, download: item.download });
    row.append(item.label);
    if (item.click) row.onclick = (event) => { event.preventDefault(); closeMenus(); item.click(); };
    else row.addEventListener('click', () => closeMenus());
    if (item.when) row.append(h('span', { class: 'when', text: item.when }));
    element.append(row);
  }
  document.body.append(element);
  const away = (event) => { if (!element.contains(event.target) && !anchor.contains(event.target)) closeMenus(); };
  closing = () => { element.remove(); window.removeEventListener('mousedown', away); closing = null; };
  setTimeout(() => window.addEventListener('mousedown', away), 0);
}
function closeMenus() { if (closing) closing(); }
window.addEventListener('keydown', (event) => { if (event.key === 'Escape' && closing) { closeMenus(); event.stopImmediatePropagation(); } }, true);

function applyTheme(theme) {
  const root = document.documentElement;
  root.style.colorScheme = theme.dark ? 'dark' : 'light';
  for (const [name, value] of theme.variables || []) root.style.setProperty(name, value);
  root.style.setProperty('--picture', theme.picture ? `url("${theme.picture}")` : 'none');
}

// Publishes, picks, deletions and theme changes, as the server tells them.
function listenEvents(handler) {
  const events = new EventSource(api('/events'));
  events.onmessage = (message) => {
    let event;
    try { event = JSON.parse(message.data); } catch (_) { return; }
    if (event.type === 'theme') applyTheme(event);
    handler(event);
  };
}

const KINDS = { document: { icon: 'file', label: 'Document' }, design: { icon: 'board', label: 'Design' } };

// "5 variants", or with several topics "3 topics".
function designSize(state) {
  if (state.topics > 1) return state.topics + ' topics';
  return state.variants === 1 ? '1 variant' : state.variants + ' variants';
}

// "Document · v2", "Design · 5 variants · v1".
function describe(artifact) {
  const kind = KINDS[artifact.kind].label;
  const version = 'v' + artifact.versions.length;
  return artifact.design ? `${kind} · ${designSize(artifact.design)} · ${version}` : `${kind} · ${version}`;
}

const publishedAt = (artifact) => (artifact.versions[artifact.versions.length - 1] || {}).published_at || 0;

// A design's state, as lists show it: Decided, 2 of 3 decided, or Waiting for you.
function stateTag(state, { short = false } = {}) {
  if (!state) return null;
  if (state.submitted_at) return h('span', { class: 'tag ok' }, icon('check', 11), 'Decided' + (!short && state.pick ? ' · ' + state.pick : ''));
  if (state.decided > 0) return h('span', { class: 'tag accent', text: `${state.decided} of ${state.topics} decided` });
  return h('span', { class: 'tag accent', text: 'Waiting for you' });
}

// Where an artifact came from, for its page's bar: the agent, and its thread to show in the app.
function whereLine(listing, machine) {
  const where = h('span', { class: 'where' });
  if (listing.agent_icon) where.append(agentIcon(listing.agent_icon));
  const place = h(listing.has_thread ? 'a' : 'span', { class: 'trunc', text: listing.place, title: listing.has_thread ? 'Show it in agentZ' : undefined });
  if (listing.has_thread) place.onclick = () => postJson('/api/show-thread', { machine, thread: listing.artifact.thread_id });
  where.append(place);
  return where;
}

// The version tag and its menu of every version.
function versionTag(artifact, version, urlFor) {
  const latest = artifact.versions.length;
  const tag = h('button', { class: 'vtag' + (version === latest ? '' : ' accent') }, version === latest ? 'v' + version : `v${version} of ${latest}`, icon('chev-down', 11));
  tag.onclick = () => menu(artifact.versions.map((entry, index) => ({ label: 'v' + (index + 1) + (index + 1 === latest ? ' (latest)' : ''), when: ago(entry.published_at), href: urlFor(index + 1) })).reverse(), tag);
  return tag;
}

// "This is v1, from 2h ago. The latest is v2. Show v2", over an earlier version.
function earlierBar(artifact, version, urlFor) {
  const latest = artifact.versions.length;
  const bar = h('div', { class: 'versionbar' });
  bar.append(h('span', {}, 'This is ', h('b', { text: `v${version}, from ${ago(artifact.versions[version - 1].published_at)}` }), `. The latest is v${latest}.`));
  bar.append(h('a', { href: urlFor(latest), text: 'Show v' + latest }));
  return bar;
}

// "v3 was just published. Show v3", over a page open at the version before.
function showUpdate(latest, urlFor) {
  let bar = document.querySelector('.updatebar');
  if (!bar) { bar = h('div', { class: 'updatebar' }); document.body.append(bar); }
  bar.replaceChildren(`v${latest} was just published.`, h('a', { href: urlFor(latest), text: 'Show v' + latest }));
}
