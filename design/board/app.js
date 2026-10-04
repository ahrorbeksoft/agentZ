// The board: one topic at a time, picks and comments saved to the server as they change.
// Each round is a folder in design/ whose page sets data-area and data-title on <body>.

const AREA = document.body.dataset.area;
const TITLE = document.body.dataset.title || AREA;
const API = `/api/${AREA}/choices`;

let state = { topics: {} };
let onlyUndecided = false;
let saveTimer = null;

const $ = (selector) => document.querySelector(selector);
const esc = (text) => String(text ?? '').replace(/[&<>"]/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' })[c]);
const topicState = (id) => {
  const s = (state.topics[id] ??= { pick: null, picks: [], none: false, comments: {}, note: '' });
  // Parts of other options to combine with a "pick one" topic's pick.
  s.also ??= [];
  return s;
};
const isPicked = (topic, s, key) => (topic.type === 'multi' ? s.picks.includes(key) : s.pick === key);
const isAlso = (topic, s, key) => topic.type !== 'multi' && !!s.pick && s.pick !== key && (s.also || []).includes(key);
const pickText = (topic, picked, key) => (picked ? (topic.type === 'multi' ? '✓ Wanted' : '✓ Picked') : topic.type === 'multi' ? 'Want this' : `Pick ${key}`);

function isDecided(topic) {
  const s = state.topics[topic.id];
  if (!s) return false;
  return s.none || (topic.type === 'multi' ? s.picks.length > 0 || s.decided : !!s.pick);
}
function hasComments(topic) {
  const s = state.topics[topic.id];
  return !!s && (Object.values(s.comments || {}).some((c) => c.trim()) || (s.note || '').trim());
}
function pickLabel(topic) {
  const s = state.topics[topic.id];
  if (!s) return null;
  if (s.none) return '—';
  if (topic.type === 'multi') return s.picks.length ? s.picks.slice().sort().join('') : s.decided ? '∅' : null;
  return s.pick ? [s.pick, ...(s.also || []).filter((key) => key !== s.pick).sort()].join('+') : null;
}

async function load() {
  try {
    const response = await fetch(API);
    const saved = await response.json();
    if (saved && saved.topics) state = saved;
  } catch (error) {
    $('#saved').textContent = 'Offline: not saving';
    $('#saved').classList.add('err');
  }
}
function save() {
  clearTimeout(saveTimer);
  $('#saved').textContent = 'Saving…';
  $('#saved').classList.remove('err');
  saveTimer = setTimeout(async () => {
    state.updated = new Date().toISOString();
    try {
      const decisions = decisionsMarkdown(TOPICS, state, { title: TITLE, area: AREA });
      const response = await fetch(API, { method: 'PUT', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ state, decisions }) });
      if (!response.ok) throw new Error(response.statusText);
      $('#saved').textContent = 'Saved';
    } catch (error) {
      $('#saved').textContent = 'Not saved: is server.py running?';
      $('#saved').classList.add('err');
    }
  }, 350);
}

function renderTop() {
  const decided = TOPICS.filter(isDecided).length;
  $('#bar').style.width = `${(100 * decided) / TOPICS.length}%`;
  $('#count').textContent = `${decided} of ${TOPICS.length} decided`;
  $('#undecided').classList.toggle('on', onlyUndecided);
}

function renderNav(currentId) {
  let html = '';
  let section = null;
  TOPICS.forEach((topic, index) => {
    if (onlyUndecided && isDecided(topic) && topic.id !== currentId) return;
    if (topic.section !== section) {
      section = topic.section;
      html += `<h3>${esc(section)}</h3>`;
    }
    const label = pickLabel(topic);
    html += `<a href="#${topic.id}" class="${topic.id === currentId ? 'on' : ''}"><span class="n">${index + 1}</span><span class="t">${esc(topic.title)}</span>${hasComments(topic) ? '<span class="c" title="Has comments"></span>' : ''}${label ? `<span class="s ${label === '—' || label === '∅' ? 'none' : ''}">${esc(label)}</span>` : ''}</a>`;
  });
  html += `<div class="sum"><a href="#summary" class="${currentId === 'summary' ? 'on' : ''}"><span class="n">∑</span><span class="t">Summary</span></a><a href="#intro" class="${currentId === 'intro' ? 'on' : ''}"><span class="n">?</span><span class="t">How this works</span></a></div>`;
  $('#nav').innerHTML = html;
}

function renderTopic(topic) {
  const s = topicState(topic.id);
  const index = TOPICS.indexOf(topic);
  const isMulti = topic.type === 'multi';
  const options = topic.options.map((option) => {
    const picked = isPicked(topic, s, option.key);
    return `<div class="opt ${picked ? 'picked' : ''} ${isAlso(topic, s, option.key) ? 'also' : ''}" data-key="${option.key}">
      <div class="oh"><span class="letter">${option.key}</span><span class="oname">${esc(option.name)}</span>${topic.rec && topic.rec.includes(option.key) ? '<span class="tag rec">Recommended</span>' : ''}${option.from ? `<span class="tag">${esc(option.from)}</span>` : ''}</div>
      <p class="desc">${option.desc}</p>
      ${option.good || option.cost ? `<div class="pc">${option.good ? `<b>Good</b><span>${option.good}</span>` : ''}${option.cost ? `<b>Cost</b><span>${option.cost}</span>` : ''}</div>` : ''}
      <div class="fit" data-key="${option.key}">${option.mock()}</div>
      <div class="foot"><button class="pick" data-key="${option.key}">${pickText(topic, picked, option.key)}</button>${isMulti ? '' : `<button class="also-btn" data-key="${option.key}" title="Combine parts of this with your pick">${isAlso(topic, s, option.key) ? '✓ Also' : '+ Also'}</button>`}
      <textarea class="cm" rows="1" data-key="${option.key}" placeholder="Comment on ${option.key}…">${esc(s.comments[option.key] || '')}</textarea></div>
    </div>`;
  }).join('');
  const issues = topic.issues?.length ? `<ul>${topic.issues.map((issue) => `<li>${issue}</li>`).join('')}</ul>` : '';
  return `<div class="crumb">${esc(topic.section)} · ${index + 1} of ${TOPICS.length}</div>
    <h2>${esc(topic.title)}</h2>
    <div class="kind">${isMulti ? '<b>Pick any</b> — these are separate features; want as many as you like.' : '<b>Pick one</b>, then <b>+ Also</b> on any others to take parts of (say which in their comment).'} Keys: <b>1–5</b> pick, <b>← →</b> topics.</div>
    <div class="today ${topic.nowImg ? '' : 'noimg'}"><div><h4>Today</h4><p>${topic.now}</p>${issues}</div>${topic.nowImg ? `<img src="${topic.nowImg}" alt="The current ${esc(topic.title)}" data-cap="Today: ${esc(topic.title)}">` : ''}</div>
    <div class="grid ${topic.size || 'wide'}">${options}</div>
    <div class="after">
      <label><input type="checkbox" id="none" ${s.none ? 'checked' : ''}> ${isMulti ? 'None of these' : 'None of these — keep it as it is, or see my note'}</label>
      ${isMulti ? `<label><input type="checkbox" id="decided" ${s.decided ? 'checked' : ''}> Done with this topic</label>` : ''}
      <textarea class="cm" id="note" placeholder="Anything else about ${esc(topic.title.toLowerCase())}: a mix of options, something missing, a different idea…">${esc(s.note || '')}</textarea>
    </div>
    <div class="pager">${index > 0 ? `<a class="tbtn" href="#${TOPICS[index - 1].id}" style="text-decoration:none;color:inherit">← ${esc(TOPICS[index - 1].title)}</a>` : '<span></span>'}${index < TOPICS.length - 1 ? `<a class="tbtn" href="#${TOPICS[index + 1].id}" style="text-decoration:none;color:inherit">${esc(TOPICS[index + 1].title)} →</a>` : '<a class="tbtn" href="#summary" style="text-decoration:none;color:inherit">Summary →</a>'}</div>`;
}

function renderSummary() {
  const rows = TOPICS.map((topic, index) => {
    const s = state.topics[topic.id] || { comments: {} };
    const keys = topic.type === 'multi' ? s.picks || [] : s.pick ? [s.pick] : [];
    const also = topic.type === 'multi' || !s.pick ? [] : (s.also || []).filter((key) => key !== s.pick);
    const name = (key) => esc(topic.options.find((o) => o.key === key)?.name);
    const chosen = s.none ? 'None of these' : [...keys.map((key) => `${key}. ${name(key)}`), ...also.map((key) => `<span style="color:var(--muted)">also ${key}. ${name(key)}</span>`)].join('<br>') || '<span style="color:var(--dim)">—</span>';
    const comments = Object.entries(s.comments || {}).filter(([, c]) => c.trim()).map(([key, c]) => `<b>${key}:</b> ${esc(c)}`);
    if ((s.note || '').trim()) comments.push(`<b>Note:</b> ${esc(s.note)}`);
    return `<tr><td>${index + 1}</td><td><a href="#${topic.id}">${esc(topic.title)}</a></td><td>${chosen}</td><td class="cms">${comments.join('\n')}</td></tr>`;
  }).join('');
  return `<div class="summary"><div class="crumb">All topics</div><h2>Summary</h2>
    <p class="intro">Everything you picked and wrote. It's saved as you go in <code>design/${AREA}/choices.json</code>, and as a spec agents can read in <code>design/${AREA}/decisions.md</code>.</p>
    <table><tr><th>#</th><th>Topic</th><th>Picked</th><th>Comments</th></tr>${rows}</table></div>`;
}

function renderIntro() {
  return `<div class="crumb">Start here</div><h2>How this works</h2>
    <div class="intro">
    ${window.ROUND_INTRO || ''}
    <p><b>Pick one</b> per design topic; on topics marked "Pick any", <b>Want</b> as many as you like. Every option has a comment box, and every topic has a note for mixes and other ideas. Click a mock to see it bigger. Changes save to the server as you type.</p>
    <p>Each option says where it comes from: <b>herdr</b>, <b>t3code</b> and <b>Zed</b> are the references agentZ follows; <b>new</b> marks an idea none of them has. Recommendations are mine; ignore them freely.</p>
    <p><a class="tbtn" href="#${TOPICS[0].id}" style="text-decoration:none;color:inherit;display:inline-block;margin-top:6px">Start with ${esc(TOPICS[0].title)} →</a></p></div>`;
}

// Mocks keep their real size and are zoomed down to fit their card.
function fitMocks() {
  document.querySelectorAll('.fit').forEach((fit) => {
    const mock = fit.firstElementChild;
    if (!mock) return;
    mock.style.zoom = 1;
    const natural = mock.offsetWidth;
    const available = fit.clientWidth;
    mock.style.zoom = natural > available ? available / natural : 1;
  });
}

function route() {
  const id = location.hash.slice(1) || 'intro';
  const topic = TOPICS.find((t) => t.id === id);
  renderTop();
  renderNav(topic ? topic.id : id);
  $('#page').innerHTML = topic ? renderTopic(topic) : id === 'summary' ? renderSummary() : renderIntro();
  window.scrollTo(0, 0);
  if (topic) bindTopic(topic);
  requestAnimationFrame(fitMocks);
}

function autosize(textarea) {
  textarea.style.height = 'auto';
  textarea.style.height = `${Math.max(34, textarea.scrollHeight + 2)}px`;
}

function refreshCards(topic) {
  const s = topicState(topic.id);
  document.querySelectorAll('.opt').forEach((card) => {
    const key = card.dataset.key;
    const picked = isPicked(topic, s, key);
    card.classList.toggle('picked', picked);
    card.classList.toggle('also', isAlso(topic, s, key));
    card.querySelector('.pick').textContent = pickText(topic, picked, key);
    const also = card.querySelector('.also-btn');
    if (also) also.textContent = isAlso(topic, s, key) ? '✓ Also' : '+ Also';
  });
  const none = $('#none');
  if (none) none.checked = s.none;
  renderTop();
  renderNav(topic.id);
}

function setPick(topic, key) {
  const s = topicState(topic.id);
  if (topic.type === 'multi') {
    s.picks = s.picks.includes(key) ? s.picks.filter((k) => k !== key) : [...s.picks, key];
  } else {
    s.pick = s.pick === key ? null : key;
    s.also = s.also.filter((k) => k !== key);
  }
  if (s.pick || s.picks.length) s.none = false;
  save();
  refreshCards(topic);
}

function setAlso(topic, key) {
  const s = topicState(topic.id);
  if (s.pick === key) return;
  s.also = s.also.includes(key) ? s.also.filter((k) => k !== key) : [...s.also, key];
  // Combining needs something to combine with: the first one becomes the pick.
  if (!s.pick) {
    s.pick = key;
    s.also = s.also.filter((k) => k !== key);
  }
  s.none = false;
  save();
  refreshCards(topic);
}

function bindTopic(topic) {
  const s = topicState(topic.id);
  document.querySelectorAll('.pick').forEach((button) => button.addEventListener('click', () => setPick(topic, button.dataset.key)));
  document.querySelectorAll('.also-btn').forEach((button) => button.addEventListener('click', () => setAlso(topic, button.dataset.key)));
  document.querySelectorAll('textarea.cm[data-key]').forEach((textarea) => {
    autosize(textarea);
    textarea.addEventListener('input', () => {
      s.comments[textarea.dataset.key] = textarea.value;
      autosize(textarea);
      save();
      renderNav(topic.id);
    });
  });
  $('#note').addEventListener('input', (event) => {
    s.note = event.target.value;
    save();
    renderNav(topic.id);
  });
  $('#none').addEventListener('change', (event) => {
    s.none = event.target.checked;
    if (s.none) { s.pick = null; s.picks = []; }
    save();
    route();
  });
  $('#decided')?.addEventListener('change', (event) => {
    s.decided = event.target.checked;
    save();
    renderTop();
    renderNav(topic.id);
  });
  document.querySelectorAll('.fit').forEach((fit) => fit.addEventListener('click', () => {
    const option = topic.options.find((o) => o.key === fit.dataset.key);
    openLightbox(option.mock(), `${option.key}. ${option.name}`);
  }));
  document.querySelector('.today img')?.addEventListener('click', (event) => {
    openLightbox(`<img src="${event.target.src}">`, event.target.dataset.cap);
  });
}

function openLightbox(html, caption) {
  const body = $('#lb-body');
  body.innerHTML = html;
  $('#lb-cap').textContent = `${caption} — click anywhere or Esc to close`;
  $('#lightbox').classList.add('on');
  const mock = body.firstElementChild;
  if (mock && mock.classList.contains('m')) {
    const scale = Math.min((innerWidth * 0.94) / mock.offsetWidth, (innerHeight * 0.86) / mock.offsetHeight, 1.6);
    mock.style.zoom = scale;
  }
}

document.addEventListener('keydown', (event) => {
  if (event.key === 'Escape') $('#lightbox').classList.remove('on');
  if (event.target.closest('textarea, input') || event.metaKey || event.ctrlKey || event.altKey) return;
  const id = location.hash.slice(1);
  const index = TOPICS.findIndex((t) => t.id === id);
  if (event.key === 'ArrowRight' || event.key === 'ArrowLeft') {
    const step = event.key === 'ArrowRight' ? 1 : -1;
    if (index === -1) location.hash = step > 0 ? TOPICS[0].id : TOPICS[TOPICS.length - 1].id;
    else if (TOPICS[index + step]) location.hash = TOPICS[index + step].id;
    else if (step > 0) location.hash = 'summary';
  }
  if (index >= 0 && /^[1-5]$/.test(event.key)) {
    const option = TOPICS[index].options[Number(event.key) - 1];
    if (option) setPick(TOPICS[index], option.key);
  }
});
$('#lightbox').addEventListener('click', () => $('#lightbox').classList.remove('on'));
$('#undecided').addEventListener('click', () => { onlyUndecided = !onlyUndecided; route(); });
window.addEventListener('hashchange', route);
window.addEventListener('resize', fitMocks);
document.fonts?.ready.then(fitMocks);

// Draw at once, then again with the saved choices.
route();
load().then(route);
