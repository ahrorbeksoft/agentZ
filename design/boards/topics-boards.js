// Boards: the agent gives the designs and the words, agentZ draws the board. What the agent hands
// over, what a design is, sizes, the board's start, layout, close-up and topics, what a topic
// asks, saving, being done and sending, and the board in the thread, the list and a republish.

const labelled = (text) => `<div style="margin:0 0 6px;font:12px 'IBM Plex Sans',sans-serif;color:#8d8e91">${text}</div>`;
const stackOf = (items, gap = 16) => `<div style="display:flex;flex-direction:column;gap:${gap}px;width:max-content">${items.map(([label, html]) => `<div>${labelled(label)}${html}</div>`).join('')}</div>`;
const fitImg = (src, extra = '') => `<div class="bd-fit"><img src="${src}">${extra}</div>`;
const [DA, DB, DC, DD, DE] = DESIGNS;

// The agent's call, highlighted as the app's code blocks are in JetBrains Dark.
function highlight(text) {
  return text.split('\n').map((line) => {
    const escaped = line.replace(/[&<>]/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;' })[c]);
    if (/^#/.test(line)) return `<span class="hd">${escaped}</span>`;
    if (/^\s*\/\//.test(line)) return `<span class="cm2">${escaped}</span>`;
    // Strings first: the spans added after them have quoted attributes of their own.
    return escaped
      .replace(/"([^"\n]*)"(\s*:)?/g, (match, text, colon) => (colon ? `<span class="k">"${text}"</span>${colon}` : `<span class="s">"${text}"</span>`))
      .replace(/^(\s*)(agentz_\w+)/, '$1<span class="fn">$2</span>')
      .replace(/(→ .*)$/, '<span class="cm2">$1</span>');
  }).join('\n');
}
const codeFrame = (blocks, { w = 780, h = 30 + blocks.reduce((sum, [, text]) => sum + 56 + 18.6 * text.split('\n').length, 0) } = {}) => frame(`<div class="code">${blocks.map(([label, text]) => `<div class="lbl">${label}</div><pre>${highlight(text)}</pre>`).join('')}</div>`, { w, h, style: 'border-radius:8px;border:1px solid #111' }).replace('class="m"', 'class="m jb"');
const AGENT_LABEL = `${glyph('claude', 'sm')} Claude Agent calls`;

// 1. What the agent hands over ------------------------------------------------------------------
const OPTION_JSON = (design) => `{ "name": "Three tiers", "design": "${design}",
        "description": "Three plans in cards, Growth marked most popular.",
        "good": "Familiar; the middle plan stands out.",
        "cost": "Comparing one feature means reading three lists." },`;
TOPICS.push({
  id: 'handover', section: 'What the agent gives', title: 'How the agent hands over a board', size: 'wide', rec: 'B',
  now: 'An agent makes a board one of two ways today. It writes the whole board as one page (topics, cards, picks, comments, a summary, and <code>window.agentzPicks</code> for Send to thread) and publishes it with <code>agentz_artifact_publish</code> by its path: hundreds of lines each time, different every time. Or it publishes each design as its own artifact, as an agent in another project did with five designs: five unrelated pages in All artifacts, each with its own Send to thread (screenshot, the demo’s five).',
  nowImg: 'img/now-gallery.png',
  issues: ['Every agent writes the board again, its own way', 'Five designs as five artifacts can’t be compared or picked between'],
  options: [
    { key: 'A', name: 'A board file, published like a page', from: 'new, as a .md file becomes a document page',
      desc: 'The agent writes a <code>.json</code> file with the intro and topics, each option naming its design’s file beside it, and publishes it with <code>agentz_artifact_publish</code> as it does a page. A <code>.json</code> file becomes a board, as a <code>.md</code> file becomes a document.',
      good: 'No new tool; the file stays on disk to edit and publish again.', cost: 'The agent learns the format only from the tool’s description, so a mistake shows only once it’s published.',
      mock: () => codeFrame([[`${AGENT_LABEL} agentz_artifact_publish`, `agentz_artifact_publish
{ "title": "Plans page", "file": "~/storefront/.designs/plans/board.json" }`], ['board.json, beside the designs', `{
  "intro": "Five designs for the plans page, built with the site’s components.",
  "topics": [{
    "title": "The plans page",
    "today": { "text": "A plain table of the plans…", "image": "today.png" },
    "recommended": "B",
    "options": [
      ${OPTION_JSON('design-1.png')}
      { "name": "Compare table", "design": "design-2.png", … },
      …
    ]
  }]
}`]]) },
    { key: 'B', name: 'A tool of its own, with the board as its arguments', from: 'agentZ’s tools (as agentz_artifact_publish)',
      desc: 'A new tool, <code>agentz_board_publish</code>: the title, an intro, and topics, each with what’s there today and its options (a name, the design’s file, a description, Good and Cost), and which one the agent recommends. Its schema lists every field, so the agent fills it in right the first time, and agentZ checks it before publishing. The answer is the board’s link, as a page’s is.',
      good: 'The agent sees exactly what to give; nothing to write but the designs.', cost: 'A long call for a big board, and a second publish tool.',
      mock: () => codeFrame([[`${AGENT_LABEL} agentz_board_publish`, `agentz_board_publish
{
  "title": "Plans page",
  "intro": "Five designs for the plans page, built with the site’s components.",
  "topics": [{
    "title": "The plans page",
    "today": { "text": "A plain table…", "image": "~/storefront/.designs/today.png" },
    "recommended": "B",
    "options": [
      ${OPTION_JSON('~/storefront/.designs/design-1.png')}
      { "name": "Compare table", "design": "~/storefront/.designs/design-2.html", … },
      …
    ]
  }]
}`], ['agentZ answers', `{ "artifactId": "5d1c9e", "version": 1, "url": "http://127.0.0.1:47120/a/5d1c9e?t=…" }`]]) },
    { key: 'C', name: 'A folder of designs and a Markdown file', from: 'new, like a design round’s folder',
      desc: 'The agent puts the designs in a folder with a <code>board.md</code>: the title and intro, then a heading per design with its description, Good and Cost lines, and its picture. It publishes the folder; agentZ reads the headings.',
      good: 'The easiest thing for an agent to write, and readable as it is.', cost: 'Loose: a mistyped heading becomes a design, and several topics need a convention of their own.',
      mock: () => codeFrame([[`${AGENT_LABEL} agentz_artifact_publish`, `agentz_artifact_publish
{ "title": "Plans page", "folder": "~/storefront/.designs/plans" }`], ['~/storefront/.designs/plans/board.md, beside today.png, 1-three-tiers.png, 2-compare-table.html, …', `# Plans page
Five designs for the plans page, built with the site’s components.

## Today
A plain table of the plans… ![](today.png)

## Three tiers
Three plans in cards, Growth marked most popular.
Good: Familiar; the middle plan stands out.
Cost: Comparing one feature means reading three lists.
![](1-three-tiers.png)

## Compare table (recommended)
Every feature in one table, a column a plan, with Monthly and Yearly.
…`]]) },
    { key: 'D', name: 'Pages first, then a board over them', from: 'new, from what agents do today',
      desc: 'The agent publishes each design as an artifact, as it already does, then <code>agentz_board_publish</code> gathers them by artifactId, with a description, Good and Cost for each.',
      good: 'Works with what agents do on their own; each design keeps its link and its versions.', cost: 'Two steps, and six artifacts in the list for one board.',
      mock: () => codeFrame([[`${AGENT_LABEL} agentz_artifact_publish, five times`, `agentz_artifact_publish { "title": "Three tiers", "file": "…/design-1.html" }     → 7f3a9c
agentz_artifact_publish { "title": "Compare table", "file": "…/design-2.html" }   → 0b44e1
// … three more`], ['then agentz_board_publish', `agentz_board_publish
{
  "title": "Plans page",
  "intro": "Five designs for the plans page, built with the site’s components.",
  "topics": [{
    "title": "The plans page",
    "recommended": "B",
    "options": [
      { "artifact": "7f3a9c", "description": "Three plans in cards, Growth marked most popular.",
        "good": "Familiar; the middle plan stands out.", "cost": "Comparing one feature means…" },
      { "artifact": "0b44e1", "description": "Every feature in one table…", … },
      …
    ]
  }]
}`]]) },
  ],
});

// 2. What one design is ------------------------------------------------------------------------
const cardsBoard = (options = {}, { top, bar } = {}) => plansBoard({ top, bar, body: topicHead() + grid(designCards(options)) });
const liveHint = (text) => `<span class="bd-hint" style="top:8px;right:8px">${text}</span>`;
TOPICS.push({
  id: 'design', section: 'What the agent gives', title: 'What one design is', size: 'wide', rec: 'C',
  now: 'An artifact is one self-contained file, an <code>.html</code> page or an <code>.md</code> document, 16 MiB at most, with its images embedded. Each of the five demo designs is a whole page built with the site’s own styles (screenshot: design A as its own artifact). On the design board, an option is a mock drawn in HTML at its real size and zoomed to fit its card; a click shows it full size. The mocks show the board in its browser tab.',
  nowImg: 'img/now-variant.png',
  issues: ['A picture can’t show a slider sliding or a menu opening; a running page is heavy five at a time'],
  options: [
    { key: 'A', name: 'A picture', from: 'new',
      desc: 'The agent gives a PNG or JPG for each design: a screenshot of what it built, or a picture it drew. The card shows it fitted to its width; a click shows it full size (topic 6).',
      good: 'Light, loads at once, and looks the same in every browser.', cost: 'Nothing to click or scroll; a long page is a tall picture.',
      mock: () => cardsBoard() },
    { key: 'B', name: 'A running page', from: 'the design board’s mocks, which are HTML',
      desc: 'The agent gives an HTML file for each design. The card runs it, zoomed to fit like the design board’s mocks: you can scroll it, open its menus and drag design C’s slider right in the card.',
      good: 'Hover, menus and sliders work as they will.', cost: 'Five pages running at once; each must be self-contained, as artifacts are.',
      mock: () => cardsBoard({ each: (design) => ({ preview: fitImg(design.img, design.key === 'C' ? liveHint(`${ic('move', 'xs')}Drag the slider`) : liveHint('Live')) }) }) },
    { key: 'C', name: 'A picture, a page, or both', from: 'new',
      desc: 'Each design is a picture, a page, or both. The card shows the picture (or the page, if that’s all there is); a design with a page has “Open page”, which runs it full size in the close-up (topic 6) to scroll and click through.',
      good: 'A light board, and the real thing one click away.', cost: 'Two things for the agent to keep alike when it gives both.',
      mock: () => cardsBoard({ each: (design) => ({ preview: fitImg(design.img, design.key === 'D' ? '' : `<span class="bd-hint" style="bottom:8px;right:8px">${ic('external', 'xs')}Open page</span>`) }) }) },
    { key: 'D', name: 'A page, and agentZ takes its picture', from: 't3code (the server renders a picture of the page)',
      desc: 'The agent gives only pages. agentZ takes a picture of each in a headless browser for its card, and runs the page when you open it.',
      good: 'One thing for the agent to make.', cost: 'A headless browser on every machine with a server, and a few seconds per design (the artifacts round didn’t pick pictures for this reason).',
      mock: () => cardsBoard({ each: (design) => ({ preview: fitImg(design.img, `${design.key === 'A' ? `<span class="bd-hint" style="top:8px;left:8px">${ic('eye', 'xs')}Picture taken by agentZ</span>` : ''}<span class="bd-hint" style="bottom:8px;right:8px">${ic('external', 'xs')}Open page</span>`) }) }) },
  ],
});

// 3. Phone and desktop -------------------------------------------------------------------------
const PHONES = { A: 'img/design-1-phone.jpg', D: 'img/design-4-phone.jpg' };
const seg = (items, on) => `<span class="seg">${items.map((item) => `<span class="${item === on ? 'on' : ''}">${item}</span>`).join('')}</span>`;
TOPICS.push({
  id: 'sizes', section: 'What the agent gives', title: 'Phone and desktop', size: 'wide', rec: 'C',
  now: 'Nothing for sizes. The five designs are desktop pages, 1280 px wide; a phone version would be another page. The design board draws each mock at one size, and a round that cares about narrow windows mocks them as separate options. In the mocks, the agent made phone designs for A and D.',
  issues: ['A design that works on a desktop can fall apart on a phone, and you can’t tell from its picture'],
  options: [
    { key: 'A', name: 'One size each', from: 'the design board',
      desc: 'Each design is one picture or page. A phone version is another design, F. Three tiers on a phone, picked like the rest.',
      good: 'Nothing new.', cost: 'A phone design competes with the desktop ones it belongs to.',
      mock: () => plansBoard({ body: topicHead() + grid(designCards() + card({ key: 'F', name: 'Three tiers on a phone', img: PHONES.A, desc: 'A on a phone: the plans stacked, the price beside each name.', good: 'Fits a phone.', cost: 'Only A’s.' }, { preview: `<div class="bd-fit" style="height:190px">${`<img src="${PHONES.A}" style="width:40%;margin:0 auto">`}</div>` })) }) },
    { key: 'B', name: 'Both in the card', from: 'new',
      desc: 'The agent can give each design a phone version too. The card shows the phone over the desktop picture’s corner; a click shows both side by side.',
      good: 'Both sizes at a glance, on one card.', cost: 'The phone is small in its corner, and covers part of the desktop.',
      mock: () => cardsBoard({ each: (design) => ({ preview: fitImg(design.img, PHONES[design.key] ? `<div class="bd-phone"><img src="${PHONES[design.key]}"></div>` : '') }) }) },
    { key: 'C', name: 'A switch for the whole board', from: 'new, like the browser’s device toolbar',
      desc: 'With phone designs on the board, its bar has Desktop · Phone, which swaps every card; phone pictures are laid out five across. A design without one says “No phone design”.',
      good: 'Compare every design at one size, at a size you can read.', cost: 'You see one size at a time.',
      mock: () => plansBoard({ top: boardTop({ buttons: `${seg(['Desktop', 'Phone'], 'Phone')}<span class="bd-tbtn">Only undecided</span><span class="bd-tbtn">Summary</span>` }), body: topicHead() + grid(DESIGNS.map((design) => card(design, { desc: false, pc: false, foot: false, preview: `${PHONES[design.key] ? fitImg(PHONES[design.key]) : '<div class="bd-none">No phone design</div>'}<div class="bd-foot"><span class="bd-pick">Pick ${design.key}</span><span class="bd-also">+ Also</span></div><span class="bd-cm">Comment on ${design.key}…</span>` })).join(''), 5) }) },
    { key: 'D', name: 'Tabs on each card', from: 'new',
      desc: 'A card with a phone version has Desktop and Phone over its picture; each card keeps its own.',
      good: 'Look at one design’s phone without changing the others.', cost: 'A click per card, and cards of different heights.',
      mock: () => cardsBoard({ each: (design) => (PHONES[design.key] && design.key === 'A' ? { tags: `<span class="grow"></span>${seg(['Desktop', 'Phone'], 'Phone')}`, preview: `<div class="bd-fit" style="display:grid;place-items:center;padding:8px 0"><img src="${PHONES.A}" style="width:46%"></div>` } : PHONES[design.key] ? { tags: `<span class="grow"></span>${seg(['Desktop', 'Phone'], 'Desktop')}` } : {}) }) },
  ],
});

// 4. Before the designs ------------------------------------------------------------------------
const INTRO = 'Five designs for the plans page, built with the site’s components and colors. They differ in how you compare the plans: by card, in a table, by size, or not at all. I recommend B; C’s slider could go under it for big shops.';
const introBox = (hide = true) => `<div class="bd-intro">${glyph('claude', 'sm')}<div class="grow"><b>From Claude Agent</b> · ${INTRO}</div>${hide ? '<span class="xs ph" style="flex:none">Hide</span>' : ''}</div>`;
TOPICS.push({
  id: 'start', section: 'The board', title: 'Before the designs', size: 'wide', rec: 'B',
  now: 'The design board starts on “How this works”: the round’s intro (what it’s about, the references, the words used), then how to pick, and a link to the first topic. Each topic then opens with a Today box: what’s there now, with its screenshot and problems (see topic 5’s screenshot). A board an agent writes itself starts however it likes.',
  issues: ['Most boards have one topic, so a start page is a click before anything to see'],
  options: [
    { key: 'A', name: 'A start page', from: 'the design board (How this works)',
      desc: 'The board opens on a page with the agent’s intro, then agentZ’s few lines on how to pick (the same on every board), and “Start with The plans page →”. It stays in the list as How this works.',
      good: 'Room for a long intro; as the design board is.', cost: 'A click before the designs.',
      mock: () => plansBoard({ nav: false, body: `<div class="bd-crumb">Plans page · start here</div><div class="bd-h">Plans page</div><div class="bd-intro" style="flex-direction:column;gap:8px;max-width:720px;font-size:13px"><div class="row g2">${glyph('claude', 'sm')}<b>From Claude Agent</b></div><div>${INTRO}</div></div><div style="max-width:720px;color:var(--mu);font-size:13px;line-height:1.55"><p style="margin:0 0 8px"><b style="color:var(--t)">Pick one</b> per topic, then <b style="color:var(--t)">+ Also</b> on any others to take parts of. Every design has a comment box, and the topic a note. A click shows a design up close.</p><p style="margin:0">Your picks are saved as you go, for Claude Agent to read. Send to thread tells it you’re done.</p></div><div><span class="bd-tbtn" style="display:inline-flex">Start with The plans page →</span></div>` }).replace('<div class="bd-page">', `${boardNav([{ n: 1, title: 'The plans page', label: null }], { on: 0 }).replace('<a><span class="n">?</span>', '<a class="on"><span class="n">?</span>')}<div class="bd-page">`) },
    { key: 'B', name: 'The intro over the first topic', from: 'new',
      desc: 'The agent’s intro sits over the first topic in a box you can hide, then Today and the designs. How to pick is the line under the topic’s title, as on the design board.',
      good: 'The designs are on the first screen, with the agent’s words above them.', cost: 'A long intro pushes the designs down.',
      mock: () => plansBoard({ body: introBox() + topicHead() + todayBox() + grid(designCards()) }) },
    { key: 'C', name: 'No intro, only Today', from: 'new',
      desc: 'The board has no intro; the agent says it in its reply in the thread. Each topic starts with Today, if the agent gave one, then the designs.',
      good: 'The least to read.', cost: 'The agent’s reasons are in the thread, not beside the designs.',
      mock: () => plansBoard() },
  ],
});

// 5. Layout ------------------------------------------------------------------------------------
const panelFor = (design, { picked = false, comment = '' } = {}) => `<div class="col g2" style="min-width:0">
  <div class="bd-oh"><span class="bd-letter" style="${picked ? 'background:var(--ok);color:var(--ed)' : ''}">${design.key}</span><span class="bd-oname">${design.name}</span>${design.key === 'B' ? '<span class="bd-tag rec">Recommended</span>' : ''}</div>
  <p class="bd-desc">${design.desc}</p><div class="bd-pc"><b>Good</b><span>${design.good}</span><b>Cost</b><span>${design.cost}</span></div>
  <div class="row g1"><span class="bd-pick" style="${picked ? 'background:var(--ok);border-color:var(--ok);color:var(--ed)' : ''}">${picked ? '✓ Picked' : `Pick ${design.key}`}</span>${picked ? '' : '<span class="bd-also">+ Also</span>'}</div>
  <div class="bd-cm ${comment ? 'typed' : ''}" style="min-height:56px">${comment || `Comment on ${design.key}…`}</div></div>`;
const strip = (on, picked = '') => `<div class="bd-strip">${DESIGNS.map((design) => `<div class="th ${design.key === on ? 'on' : ''} ${design.key === picked ? 'picked' : ''}"><img src="${design.img}"><span>${design.key}</span></div>`).join('')}</div>`;
const arrows = () => `<span class="bd-arrow" style="left:10px">${ic('chev-left', 'sm')}</span><span class="bd-arrow" style="right:10px">${ic('chev-right', 'sm')}</span>`;
TOPICS.push({
  id: 'layout', section: 'The board', title: 'How the designs are laid out', size: 'wide', rec: 'A',
  now: 'The design board puts a topic’s options in cards, in columns as wide as the topic asks: each with its letter, name, description, Good and Cost, the mock, Pick, + Also and a comment (screenshot: the Artifacts round). The mocks have five designs at 1280×800 in a 1280 px browser.',
  nowImg: 'img/now-local-board.png',
  issues: ['A whole page shrunk to a third of the screen is hard to read'],
  options: [
    { key: 'A', name: 'Cards in a grid', from: 'the design board',
      desc: 'Three cards across (two in a narrow window), each with everything the design board’s cards have. A click on a picture shows it up close (topic 6).',
      good: 'Every design in view at once, and the board you know.', cost: 'Pictures a third of the width; reading one means opening it.',
      mock: () => plansBoard({ top: boardTop({ decided: 1 }), body: topicHead() + grid(designCards({ picked: 'B', comments: { B: 'Yes, and make Yearly the default.' } })) }) },
    { key: 'B', name: 'One at a time, with a strip', from: 'new, like a photo viewer',
      desc: 'One design large with its words, Pick and comment beside it; a strip of the others under it, and ← → to move. The strip marks the picked one.',
      good: 'Each design big enough to read.', cost: 'You see one at a time, so comparing means flipping.',
      mock: () => plansBoard({ top: boardTop({ decided: 1 }), body: `${topicHead({ kind: '<b>Pick one</b>, then <b>+ Also</b> on any others. Keys: <b>← →</b> designs, <b>1–5</b> pick.' })}<div style="display:grid;grid-template-columns:1fr 280px;gap:18px;align-items:start"><div style="position:relative">${fitImg(DB.img)}${arrows()}</div>${panelFor(DB, { picked: true, comment: 'Yes, and make Yearly the default.' })}</div>${strip('B', 'B')}` }) },
    { key: 'C', name: 'Two side by side', from: 'new',
      desc: 'Two designs at half the width each, with a menu over each to choose which; they scroll together. Pick under either.',
      good: 'The best for weighing two finalists.', cost: 'Only two at a time; you choose the pair first.',
      mock: () => plansBoard({ top: boardTop({ decided: 1 }), body: `${topicHead()}<div class="row g2 sm mu">Compare<span class="bd-tbtn">B. Compare table ${ic('chev-down', 'xs')}</span>with<span class="bd-tbtn">C. Slider by size ${ic('chev-down', 'xs')}</span></div>${grid(card(DB, { rec: true, picked: true }) + card(DC, { also: true, comment: 'The slider, for shops over 1,000 products.' }), 2)}` }) },
    { key: 'D', name: 'A row each', from: 'new',
      desc: 'One design a row: its picture at half the width on the left, its words, Pick and comment on the right. Scroll down for the next.',
      good: 'Big pictures, and every design’s words beside it.', cost: 'Two designs a screen; a long scroll for five.',
      mock: () => plansBoard({ top: boardTop({ decided: 1 }), body: topicHead() + DESIGNS.slice(0, 2).map((design) => `<div class="bd-opt ${design.key === 'B' ? 'picked' : ''}" style="display:grid;grid-template-columns:560px 1fr;gap:16px;align-items:start;flex-direction:row">${fitImg(design.img)}${panelFor(design, { picked: design.key === 'B' })}</div>`).join('') }) },
  ],
});

// 6. Up close ----------------------------------------------------------------------------------
const closeUpBoard = (stage, panel) => browser(page(pageBar(), `<div class="bd">${boardTop({ decided: 1 })}<div class="bd-body"><div style="flex:1;min-width:0;position:relative;background:#111214;display:grid;place-items:center;padding:18px">${stage}</div>${panel}</div></div>`));
TOPICS.push({
  id: 'closeup', section: 'The board', title: 'One design up close', size: 'wide', rec: 'B',
  now: 'On the design board, a click on a mock opens it full size over the board, with its caption; Esc or a click closes it. There are no arrows to the next one, and you pick and comment only on the card. A design published as its own artifact opens as a whole page, with nothing to pick.',
  issues: [],
  options: [
    { key: 'A', name: 'Full size over the board', from: 'the design board',
      desc: 'A click shows the picture (or runs the page) at full size over the board, its name under it. ← → go to the next design; Esc closes.',
      good: 'As today, with arrows.', cost: 'To pick or comment, close it first.',
      mock: () => plansBoard({ over: `<div class="bd-over" style="z-index:15"><div style="position:relative"><img src="${DB.img}" style="width:1020px;display:block;border-radius:6px">${arrows()}</div><div class="cap">B. Compare table · ← → the other designs · Esc closes</div></div>` }) },
    { key: 'B', name: 'Up close, with its words and Pick', from: 'new',
      desc: 'A click fills the board with the design, its words, Pick, + Also and comment in a panel at the right. ← → go through the designs, Esc goes back to the cards. A page runs here to scroll and click.',
      good: 'Read, decide and comment without going back.', cost: 'A second way to pick, besides the cards.',
      mock: () => closeUpBoard(`<div style="position:relative;width:100%"><img src="${DB.img}" style="width:100%;display:block;border-radius:6px">${arrows()}<span class="bd-hint" style="bottom:8px;left:8px">2 of 5 · Esc back to the board</span></div>`, `<div class="bd-panel">${panelFor(DB, { picked: true, comment: 'Yes, and make Yearly the default.' })}</div>`) },
    { key: 'C', name: 'In a tab of its own', from: 'new',
      desc: 'A click opens the design in a new tab, at the browser’s full width, under a thin bar: the board’s name, the design, ‹ ›, Pick and a comment. Your pick shows on the board’s tab too.',
      good: 'The design as it will really be, with the browser’s own zoom and sizes.', cost: 'Tabs to keep track of.',
      mock: () => browser(`<div class="pg"><div class="phd">${ibtn('chev-left')}<span class="ph">Plans page /</span><span class="ptitle">B. Compare table</span><span class="ph sm">2 of 5</span>${ibtn('chev-left')}${ibtn('chev-right')}<span class="grow"></span><span class="btn" style="background:var(--ok);color:var(--ed)">✓ Picked</span><span class="btn ghost">${ic('chat', 'xs')}Comment</span></div><div class="pbody"><img src="${DB.img}" style="width:100%;display:block"></div></div>`, { title: 'B. Compare table', tabs: `<div class="brtab">${ic('board', 'xs')}<span class="grow trunc">Plans page</span>${ic('x', 'xs')}</div>` }) },
  ],
});

// 7. Several topics ----------------------------------------------------------------------------
const PRICING = { title: 'Pricing', where: 'storefront › Design the pricing' };
const lightMock = (inner) => `<div style="background:#fff;color:#16181d;font:13px/1.4 -apple-system,BlinkMacSystemFont,sans-serif;padding:16px 18px;height:178px;display:flex;flex-direction:column;gap:8px">${inner}</div>`;
const pickerRow = (name, price, on) => `<div style="display:flex;align-items:center;gap:10px;border:1px solid ${on ? '#4f46e5' : '#e6e8ec'};border-radius:9px;padding:8px 10px"><span style="width:14px;height:14px;border-radius:50%;border:${on ? '4px solid #4f46e5' : '1.5px solid #b8bcc6'}"></span><b style="font-weight:600">${name}</b><span style="flex:1"></span>${price}</div>`;
const PICKERS = [
  { key: 'A', name: 'Radio list', desc: 'The three plans as rows, the price at the end of each.', good: 'All three in view.', cost: 'Tall on a phone.', html: lightMock(`<b>Your plan</b>${pickerRow('Starter', '$0', false)}${pickerRow('Growth', '$29 / mo', true)}${pickerRow('Scale', '$99 / mo', false)}`) },
  { key: 'B', name: 'Segmented', desc: 'Starter · Growth · Scale in one control, what you get under it.', good: 'Short.', cost: 'The others’ prices are a click away.', html: lightMock(`<b>Your plan</b><div style="display:flex;background:#f0f1f4;border-radius:9px;padding:3px">${['Starter', 'Growth', 'Scale'].map((n) => `<span style="flex:1;text-align:center;padding:6px;border-radius:7px;${n === 'Growth' ? 'background:#fff;box-shadow:0 1px 3px rgba(0,0,0,.15);font-weight:600' : 'color:#5b616e'}">${n}</span>`).join('')}</div><div style="font-size:22px;font-weight:700">$29<small style="font-size:13px;color:#7a808c;font-weight:500"> / month</small></div><div style="color:#5b616e">3 stores, unlimited products, discount codes</div>`) },
  { key: 'C', name: 'Dropdown', desc: 'A menu like the address form’s fields.', good: 'The shortest.', cost: 'Hides that there are other plans.', html: lightMock(`<b>Your plan</b><div style="display:flex;align-items:center;border:1px solid #d9dce2;border-radius:9px;padding:9px 12px"><span>Growth · $29 / month</span><span style="flex:1"></span>▾</div><div style="color:#7a808c">Change it any time from your shop’s settings.</div>`) },
];
const pricingBar = () => pageBar({ title: PRICING.title, where: PRICING.where });
const pricingBoard = ({ nav = true, body, top, on = 1, topics = CHECKOUT_TOPICS, h = 800 } = {}) => browser(page(pricingBar(), board({ top: top ?? boardTop({ title: PRICING.title, decided: 2, total: 3 }), nav: nav ? boardNav(topics, { on, section: 'Pricing' }) : '', body })), { title: PRICING.title, h });
const plansTopic = (crumb = 'Pricing · 1 of 3') => topicHead({ crumb }) + grid(designCards({ picked: 'B', comments: { B: 'Yes, and make Yearly the default.' } }));
TOPICS.push({
  id: 'topics', section: 'The board', title: 'A board with several topics', size: 'wide', rec: 'D',
  now: 'The design board lists a round’s topics on the left by section, each with what you picked (a green letter) and a dot when there’s a comment; ← → move between them, and Summary and How this works close the list. Most agent boards have one topic: five designs of one thing. The mocks’ bigger board, Pricing, has three: the plans page, the plan picker in checkout, and the yearly discount.',
  issues: [],
  options: [
    { key: 'A', name: 'A list on the left', from: 'the design board',
      desc: 'Topics on the left with what you picked, Summary at the end, on every board, even one with a single topic.',
      good: 'As the design board; every topic and your picks in view.', cost: 'A list of one on most boards.',
      mock: () => pricingBoard({ body: plansTopic() }) },
    { key: 'B', name: 'Steps across the top', from: 'new, like a checkout’s steps',
      desc: 'The topics as steps over the page, each ticked with your pick once decided; Back and Next at the bottom, and Summary as the last step.',
      good: 'Walks you through in order; reads well for two or three topics.', cost: 'Long titles get cut; ten topics don’t fit.',
      mock: () => pricingBoard({ nav: false, body: `<div class="bd-steps">${CHECKOUT_TOPICS.map((topic) => `<span class="bd-step ${topic.n === 1 ? 'on' : ''} ${topic.label ? 'done' : ''}"><span class="dotn">${topic.label ? '✓' : topic.n}</span>${topic.title}${topic.label ? ` · ${topic.label}` : ''}</span>${topic.n < 3 ? `<span class="ph">${ic('chev-right', 'xs')}</span>` : ''}`).join('')}<span class="ph">${ic('chev-right', 'xs')}</span><span class="bd-step"><span class="dotn">∑</span>Summary</span></div>${plansTopic()}` }) },
    { key: 'C', name: 'All on one page', from: 'new',
      desc: 'Every topic one after the other on one long page, each under its heading; the bar counts what’s decided.',
      good: 'Nothing to navigate; scroll and pick.', cost: 'A long page; harder to see what’s left.',
      mock: () => pricingBoard({ nav: false, body: `<div style="margin-top:-480px;display:flex;flex-direction:column;gap:12px">${grid(designCards({ picked: 'B' }))}<div class="bd-section">2 · The plan picker in checkout</div><div class="bd-kind" style="margin-top:0"><b>Pick one</b>, then <b>+ Also</b> on any others.</div>${grid(PICKERS.map((picker) => card(picker, { picked: picker.key === 'A', rec: picker.key === 'A', preview: `<div class="bd-fit">${picker.html}</div>` })).join(''))}</div>` }) },
    { key: 'D', name: 'A list only with two or more', from: 'the design board, without the list for one topic',
      desc: 'A board with one topic is just that topic, with Summary in the bar; with two or more, the list on the left as A.',
      good: 'Nothing extra on the usual board; the list when it helps.', cost: 'Boards look different by their size.',
      mock: () => stackOf([['One topic', plansBoard({ nav: false, top: boardTop({ buttons: '<span class="bd-tbtn">Summary</span>' }), h: 520 })], ['Three topics', pricingBoard({ body: plansTopic(), h: 520 })]]) },
  ],
});

// 8. What a topic asks -------------------------------------------------------------------------
const FEATURES = [
  { key: 'A', name: 'Monthly and Yearly', desc: 'A switch over the plans; Yearly shows each price with 20% off.' },
  { key: 'B', name: 'Questions under the plans', desc: 'Six common questions, each opening to its answer.' },
  { key: 'C', name: 'Shops’ logos', desc: 'A row of shops on Storefront over the plans.' },
  { key: 'D', name: 'Talk to sales', desc: 'A line under Scale for shops selling over $20k a month.' },
];
const wantCard = (feature, wanted, comment = '') => `<div class="bd-opt ${wanted ? 'picked' : ''}"><div class="bd-oh"><span class="bd-letter">${feature.key}</span><span class="bd-oname">${feature.name}</span></div><p class="bd-desc">${feature.desc}</p><div class="bd-foot"><span class="bd-pick">${wanted ? '✓ Wanted' : 'Want this'}</span><span class="bd-cm ${comment ? 'typed' : ''}">${comment || `Comment on ${feature.key}…`}</span></div></div>`;
const ASK_TOPICS = [{ n: 1, title: 'The plans page', label: 'B' }, { n: 2, title: 'What else it shows', label: 'AD' }, { n: 3, title: 'What Growth costs', label: null, comment: true }];
TOPICS.push({
  id: 'asks', section: 'The board', title: 'What a topic can ask', size: 'wide', rec: 'B',
  now: 'The design board’s topics are “Pick one”, with + Also to take parts of others, or “Pick any” for a list of features, each wanted or not. Every option has a comment, every topic a note and “None of these”.',
  issues: [],
  options: [
    { key: 'A', name: 'Pick one, always', from: 'the design board’s usual topic',
      desc: 'Every topic is pick one, with + Also on others to take parts of them, a comment on each design and a note on the topic.',
      good: 'One way to answer; it fits designs.', cost: 'A list of extras to want has to be asked in the thread.',
      mock: () => plansBoard({ top: boardTop({ decided: 1 }), body: topicHead() + grid(designCards({ picked: 'B', also: ['C'], comments: { C: 'The slider, for shops over 1,000 products.' } })) }) },
    { key: 'B', name: 'Pick one or pick any, as the agent asks', from: 'the design board (both kinds)',
      desc: 'The agent marks a topic pick any for a list of features, each with Want this, besides the usual pick one. A feature can have no picture.',
      good: 'Designs and lists of extras on one board, as design rounds have them.', cost: 'Two kinds of topic to read.',
      mock: () => pricingBoard({ topics: ASK_TOPICS, on: 2, body: `${topicHead({ crumb: 'Pricing · 2 of 3', title: 'What else the plans page shows', kind: '<b>Pick any</b>: these are separate; want as many as you like.' })}${grid(FEATURES.map((feature) => wantCard(feature, 'AD'.includes(feature.key), feature.key === 'D' ? 'Only on Scale.' : '')).join(''), 2)}` }) },
    { key: 'C', name: 'B, and questions in words', from: 'new',
      desc: 'As B, and the agent can ask a question with no designs: you answer in a text box, or click one of its suggestions.',
      good: 'Settle a price or a name on the same board.', cost: 'The agent could just ask in the thread.',
      mock: () => pricingBoard({ topics: ASK_TOPICS, on: 3, body: `${topicHead({ crumb: 'Pricing · 3 of 3', title: 'What Growth costs', kind: '<b>Answer</b> in your words, or pick a suggestion.' })}<div class="bd-opt" style="max-width:640px"><p class="bd-desc" style="font-size:13px;color:var(--t)">What should Growth cost a month?</p><p class="bd-desc">It’s $29 today. Shops its size pay $25–$39 elsewhere.</p><div class="bd-chips"><span>$25</span><span class="on">$29</span><span>$39</span></div><div class="bd-cm typed" style="min-height:64px">$29, and $24 a month paid yearly.</div></div>` }) },
  ],
});

// 9. Saving ------------------------------------------------------------------------------------
const pickedBoard = (saved) => plansBoard({ top: boardTop({ decided: 1, saved }), body: topicHead() + grid(designCards({ picked: 'B', also: ['C'], comments: { B: 'Yes, and make Yearly the default.' } })) });
TOPICS.push({
  id: 'saving', section: 'Picks and the agent', title: 'What the board says about saving', size: 'wide', rec: 'C',
  now: 'The design board’s server writes your picks to <code>choices.json</code> and <code>decisions.md</code> as you type, and its bar says “Saved”. An artifact page runs sandboxed with no storage, so a board published as one keeps picks only in its tab (“Kept until you reload”, screenshot); <code>agentz_artifact_read</code> gives the agent the page’s source, not your picks. Decided before this round: the server keeps a board’s picks, and the agent can read them whenever it likes.',
  nowImg: 'img/now-board-artifact.png',
  issues: ['Today a reload loses every pick on an artifact board'],
  options: [
    { key: 'A', name: 'Saved', from: 'the design board',
      desc: 'The server keeps the picks with the board as you go, and the bar says “Saved”. A reload, another browser, or the board from another machine shows them.',
      good: 'Quiet, as the design board.', cost: 'Doesn’t say the agent can read them.',
      mock: () => pickedBoard('Saved') },
    { key: 'B', name: 'Saved, for the agent', from: 'new',
      desc: 'As A, saying who can read them: “Saved · Claude Agent can read your picks”.',
      good: 'You know a comment reaches the agent without sending.', cost: 'A longer line.',
      mock: () => pickedBoard('Saved · Claude Agent can read your picks') },
    { key: 'C', name: 'Saved, and when the agent read them', from: 'new',
      desc: 'As B, and once the agent reads them: “Read by Claude Agent 4m ago”, or after you change something, “Saved · changed since Claude Agent read them”.',
      good: 'You see whether the agent has your latest.', cost: 'The server notes each read.',
      mock: () => pickedBoard('Saved · changed since Claude Agent read them 4m ago') },
  ],
});

// 10. Done -------------------------------------------------------------------------------------
const PICKS_TEXT = `Plans page, v1
The plans page: B. Compare table, also C. Slider by size
  B: Yes, and make Yearly the default.
  C: The slider, for shops over 1,000 products.`;
const sendBox = (text = PICKS_TEXT) => `<div class="sd-over"><div class="sd"><h4>Send to thread</h4><pre>${text}</pre><div class="ta">A note to go with it…</div><div class="row g2" style="justify-content:flex-end;margin-top:10px"><span class="btn">Cancel</span><span class="btn primary">${ic('send', 'xs')}Send</span></div></div></div>`;
const zrow = (text, end = '') => `<div class="trow"><span class="zmark">Z</span>${text}<span class="grow"></span>${end}</div>`;
const boardCard = ({ meta = 'Board · 5 designs · v1 · published 2m ago', thumbs = false, picked = '' } = {}) => `<div class="acard" style="align-items:flex-start"><span class="kindtile">${ic('board', 'sm')}</span><div class="col grow" style="gap:1px;min-width:0"><span class="ttl trunc">Plans page</span><span class="meta trunc">${meta}</span>${thumbs ? `<div class="thumbs">${DESIGNS.map((design) => `<img src="${design.img}" class="${design.key === picked ? 'picked' : ''}">`).join('')}</div>` : ''}</div><span class="btn">${ic('arrow-up-right', 'xs')}Open</span>${ibtn('more')}</div>`;
TOPICS.push({
  id: 'done', section: 'Picks and the agent', title: 'Telling the agent you’re done', size: 'wide', rec: 'B',
  now: 'Every artifact page has Send to thread in its bar: a box with what the page gave (its <code>window.agentzPicks</code>) and a note, and Send puts both in the thread as your message (screenshot: a design page, which gives nothing). With the design board you tell the agent in its thread to read <code>decisions.md</code>.',
  nowImg: 'img/now-send.png',
  issues: [],
  options: [
    { key: 'A', name: 'Send to thread, whenever', from: 'artifacts (Send to thread)',
      desc: 'The bar’s Send to thread, any time: the box shows your picks and comments as text, with a note. The agent can also read the picks itself whenever you tell it to.',
      good: 'As every artifact page; nothing new.', cost: 'Nothing says when you’re done; easy to send half a board.',
      mock: () => plansBoard({ top: boardTop({ decided: 1 }), body: topicHead() + grid(designCards({ picked: 'B', also: ['C'] })), over: sendBox() }) },
    { key: 'B', name: 'A bar once every topic is decided', from: 'new',
      desc: 'As A, and once every topic has a pick, a bar at the bottom: “Every topic is decided. Send your picks to Claude Agent”, which opens the same box. Summary ends with it too.',
      good: 'Says when you’re done and what to do next.', cost: 'Covers the bottom of the page until you send or close it.',
      mock: () => plansBoard({ top: boardTop({ decided: 1 }), body: topicHead() + grid(designCards({ picked: 'B', also: ['C'] })) + `<div class="bd-donebar">${ic('check-circle', 'sm')}<span class="grow"><b>Every topic is decided.</b> Send your picks and comments to Claude Agent in “Design the plans page”.</span><span class="btn">Not yet</span><span class="btn primary">${ic('send', 'xs')}Send to Claude Agent</span></div>` }) },
    { key: 'C', name: 'The agent waits for you', from: 'new, as an agent waits on a question',
      desc: 'The agent can wait on the board, as it waits on a question: its turn stays open, the thread shows “Waiting for your picks on Plans page” and the sidebar shows it waiting for you. Done on the board sends the picks, and the agent goes on.',
      good: 'The agent goes on as soon as you’re done; nothing to type.', cost: 'A turn left open for as long as you take, which some agents end on their own.',
      mock: () => threadFrame(ASK + boardCard() + AFTER + zrow('Waiting for your picks on <span class="subj">Plans page</span>', `<span class="btn">${ic('arrow-up-right', 'xs')}Open</span>`)) },
    { key: 'D', name: 'Nothing to send', from: 'new',
      desc: 'The board has no Send. When you write in the thread (“done on the board”), the agent reads the picks itself, and the thread shows that it did.',
      good: 'The picks never go through the thread as text.', cost: 'You have to say you’re done, and the agent has to think of reading.',
      mock: () => threadFrame(ASK + boardCard() + AFTER + bub('Done on the board.') + zrow('Read the board: <span class="subj">Plans page</span>', '<span class="vtag">v1</span>') + msg('Building B, the compare table, with Yearly as the default, and C’s slider under it for shops over 1,000 products.')) },
  ],
});

// 11. What Send puts in the thread ---------------------------------------------------------------
const youSent = (html) => `<div style="align-self:flex-end;max-width:84%;margin:2px 0 10px">${html}</div>`;
TOPICS.push({
  id: 'send', section: 'Picks and the agent', title: 'What Send puts in the thread', size: 'wide', rec: 'C',
  now: 'Send to thread puts the page’s text and your note in the thread as your message, with a chip naming the page and its version; the agent reads the text as it is.',
  issues: [],
  options: [
    { key: 'A', name: 'The picks as text', from: 'artifacts (Send to thread)',
      desc: 'Your message is the picks, comments and note as text, under a chip naming the board.',
      good: 'The agent has everything without another call.', cost: 'A long message for a big board.',
      mock: () => threadFrame(ASK + boardCard() + AFTER + youSent(`<div class="bub" style="margin:0;max-width:none"><span class="chip" style="margin-bottom:6px">${ic('board', 'xs')}Plans page · v1</span><div style="white-space:pre-line">${PICKS_TEXT.split('\n').slice(1).join('\n')}\nGo with the table.</div></div>`), { h: 500 }) },
    { key: 'B', name: 'A short message; the agent reads the rest', from: 'new',
      desc: 'Your message says only what you picked and your note, with the board’s chip; the agent reads the comments with the board’s read tool.',
      good: 'Short in the thread, and the agent reads the picks as data.', cost: 'Another call before it starts, and the comments aren’t in the thread.',
      mock: () => threadFrame(ASK + boardCard() + AFTER + youSent(`<div class="bub" style="margin:0;max-width:none"><span class="chip" style="margin-bottom:6px">${ic('board', 'xs')}Plans page · v1</span><div>Picked B, also C. Go with the table.</div></div>`) + zrow('Read the board: <span class="subj">Plans page</span>', '<span class="vtag">v1</span>'), { h: 500 }) },
    { key: 'C', name: 'A card with the pictures', from: 'new',
      desc: 'Your message shows a card: the board’s title, its designs’ pictures with the picked one outlined, and “Picked B · also C · 2 comments”, then your note. The agent gets A’s text.',
      good: 'You see what you sent at a glance.', cost: 'A new kind of message to draw.',
      mock: () => threadFrame(ASK + boardCard() + AFTER + youSent(`<div class="bub" style="margin:0;max-width:none"><div class="acard" style="margin:0 0 6px;background:var(--panel)"><span class="kindtile">${ic('board', 'sm')}</span><div class="col grow" style="gap:1px"><span class="ttl">Plans page · v1</span><span class="meta">Picked B. Compare table · also C · 2 comments</span><div class="thumbs">${DESIGNS.map((design) => `<img src="${design.img}" class="${design.key === 'B' ? 'picked' : ''}" style="width:64px;height:40px">`).join('')}</div></div></div>Go with the table.</div>`), { h: 520 }) },
  ],
});

// 12. The card in the thread -------------------------------------------------------------------
TOPICS.push({
  id: 'card', section: 'Around the board', title: 'The board in the thread', size: 'wide', rec: 'C',
  now: 'A publish shows as a card where the agent published: a page tile, the title, “Page · v1 · published 2m ago”, Open and a menu (artifacts topic 1 B). The newest publish of a page is the card; earlier ones become lines. The thread’s header lists the thread’s artifacts.',
  issues: [],
  options: [
    { key: 'A', name: 'The card a page has', from: 'artifacts (topic 1 B)',
      desc: 'The same card with a board tile: “Board · 5 designs · v1 · published 2m ago”.',
      good: 'Nothing new.', cost: 'Says nothing of the designs or your picks.',
      mock: () => threadFrame(ASK + boardCard() + AFTER, { h: 250 }) },
    { key: 'B', name: 'With the designs’ pictures', from: 'new',
      desc: 'A’s card with a row of small pictures of the designs (the first topic’s); a click on one opens the board on it.',
      good: 'You see what the agent made without opening it.', cost: 'A taller card.',
      mock: () => threadFrame(ASK + boardCard({ thumbs: true }) + AFTER, { h: 250 }) },
    { key: 'C', name: 'B, showing your picks', from: 'new',
      desc: 'As B, and the card follows the board as you pick: “1 of 1 decided · picked B”, the picked picture outlined.',
      good: 'The thread shows where the board stands.', cost: 'The card changes after it’s drawn.',
      mock: () => threadFrame(ASK + boardCard({ thumbs: true, picked: 'B', meta: 'Board · 5 designs · v1 · 1 of 1 decided · picked B' }) + AFTER, { h: 250 }) },
  ],
});

// 13. All artifacts ----------------------------------------------------------------------------
const mosaic = (picked = '') => `<div class="thumbs" style="margin:2px 0 6px">${DESIGNS.slice(0, 4).map((design) => `<img src="${design.img}" class="${design.key === picked ? 'picked' : ''}" style="width:56px;height:36px">`).join('')}<span class="xs ph" style="align-self:center">+1</span></div>`;
const gcard = (title, meta, where, { tile = 'app-window', extra = '' } = {}) => `<div class="gcard"><span style="color:var(--mu)">${ic(tile, 'sm')}</span>${extra}<b>${title}</b><span>${meta}</span><span>${where}</span></div>`;
const gallery = (boardCardHtml, filters = ['All', 'Threads', 'Chats']) => browser(`<div class="gal"><div class="gal-head"><b>All artifacts</b>${seg(filters, 'All')}<span class="grow"></span><div class="field" style="width:220px;height:28px">${ic('search', 'xs')}<span class="ph sm">Search artifacts…</span></div></div><div class="gal-grid">${boardCardHtml}${gcard('Checkout spec', 'Document · v1 · 1h ago', 'storefront › Add the checkout page', { tile: 'file' })}${gcard('Limiter benchmarks', 'Page · v3 · 3h ago', 'limiter › Rate limiter tests')}</div></div>`, { title: 'All artifacts', icon: 'grid', url: '127.0.0.1:47120/a', h: 300 });
TOPICS.push({
  id: 'gallery', section: 'Around the board', title: 'The board in All artifacts', size: 'wide', rec: 'B',
  now: 'All artifacts shows a card per artifact: its kind, title, “Page · v1 · 5m ago” and where it came from, with All / Threads / Chats and a search (screenshot: the demo’s five designs as five artifacts).',
  nowImg: 'img/now-gallery.png',
  issues: [],
  options: [
    { key: 'A', name: 'A card like a page’s', from: 'artifacts (All artifacts)',
      desc: 'One card for the board, with its tile and “Board · 5 designs · v1”.',
      good: 'Nothing new; five designs are one card.', cost: 'Looks like any page.',
      mock: () => gallery(gcard('Plans page', 'Board · 5 designs · v1 · 2m ago', 'storefront › Design the plans page', { tile: 'board' })) },
    { key: 'B', name: 'With the designs’ pictures and your progress', from: 'new',
      desc: 'A’s card with small pictures of the designs, the picked one outlined, and “1 of 1 decided”.',
      good: 'A board is easy to tell from a page, and you see which still need you.', cost: 'Taller than the other cards.',
      mock: () => gallery(gcard('Plans page', 'Board · 5 designs · v1 · 1 of 1 decided', 'storefront › Design the plans page', { tile: 'board', extra: mosaic('B') })) },
    { key: 'C', name: 'B, with a Boards filter', from: 'new',
      desc: 'As B, and All / Threads / Chats gains Boards, to find the boards that wait for you.',
      good: 'All your open decisions in one place.', cost: 'A filter by kind next to filters by place.',
      mock: () => gallery(gcard('Plans page', 'Board · 5 designs · v1 · 1 of 1 decided', 'storefront › Design the plans page', { tile: 'board', extra: mosaic('B') }), ['All', 'Threads', 'Chats', 'Boards']) },
  ],
});

// 14. Published again --------------------------------------------------------------------------
const AGAIN_DESIGNS = DESIGNS.filter((design) => design.key !== 'D');
const v2Board = (banner, cards) => plansBoard({ bar: pageBar({ version: 2 }), top: boardTop({ decided: cards.includes('✓ Picked') ? 1 : 0 }), body: `<div class="bd-banner">${ic('history', 'sm')}<span class="grow">${banner}</span></div>${topicHead()}${grid(cards)}` });
TOPICS.push({
  id: 'again', section: 'Around the board', title: 'When the agent publishes it again', size: 'wide', rec: 'A',
  now: 'Each publish is a new version at the same link, and an open page gets a bar, “v2 was just published. Show v2” (artifacts topic 12 B); the version menu has the earlier ones. In the mocks, Claude Agent published v2 after your comments: B changed, D gone.',
  issues: ['An agent revises a board after your comments, and you shouldn’t have to pick again'],
  options: [
    { key: 'A', name: 'Picks carry over', from: 'new',
      desc: 'Picks and comments move to the new version by design name. A changed design is marked Changed and a new one New; a design that’s gone takes its pick with it, and the banner says so.',
      good: 'Pick once; see only what changed.', cost: 'A renamed design loses its pick.',
      mock: () => v2Board('v2 from Claude Agent: B changed after your comment, D is gone. Your picks are kept.', AGAIN_DESIGNS.map((design) => card(design, { rec: design.key === 'B', picked: design.key === 'B', also: design.key === 'C', comment: design.key === 'B' ? 'Yes, and make Yearly the default.' : '', tags: design.key === 'B' ? '<span class="bd-tag new">Changed</span>' : '' })).join('')) },
    { key: 'B', name: 'Each version starts empty', from: 'artifacts (each version stands alone)',
      desc: 'A new version has no picks; v1’s stay with v1, in the version menu.',
      good: 'Simple; each version is what you said about it.', cost: 'Pick again after every revision.',
      mock: () => v2Board('v2 from Claude Agent. Your picks on v1 stay with v1.', AGAIN_DESIGNS.map((design) => card(design, { rec: design.key === 'B' })).join('')) },
    { key: 'C', name: 'Empty, showing what you picked before', from: 'new',
      desc: 'As B, but each design you picked or commented on in v1 says so (“Picked in v1”, with the comment), so picking again is a click.',
      good: 'Every revision is a fresh decision, with your last one in view.', cost: 'Two sets of marks on a card.',
      mock: () => v2Board('v2 from Claude Agent. Your v1 picks are marked.', AGAIN_DESIGNS.map((design) => card(design, { rec: design.key === 'B', tags: design.key === 'B' ? '<span class="bd-tag" style="border-color:var(--ok);color:var(--ok)">Picked in v1</span>' : design.key === 'C' ? '<span class="bd-tag">Also in v1</span>' : '' })).join('')) },
  ],
});
