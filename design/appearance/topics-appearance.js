// How the window sits on the desktop: solid (today), frosted (GPUI's blurred window, which Zed's
// themes turn on with "background.appearance"), clear, or over a picture of the user's own.
// Colors are JetBrains Dark's, the user's dark theme.

ICONS.loader = '<path d="M21 12a9 9 0 1 1-6.219-8.56"/>';
ICONS.image = '<rect width="18" height="18" x="3" y="3" rx="2" ry="2"/><circle cx="9" cy="9" r="2"/><path d="m21 15-3.086-3.086a2 2 0 0 0-2.828 0L6 21"/>';

const rgba = (rgb, alpha) => `rgba(${rgb.join(',')},${alpha})`;
const JBD = { bg: [38, 40, 43], ed: [30, 31, 34], border: [57, 59, 65], sel: [67, 69, 74], bubble: [52, 54, 58], t: '#dfe1e5', mu: '#b0b1b3', ph: '#6f737a', ac: '#3474f0' };
const WALLPAPER = [
  'radial-gradient(ellipse at 18% 28%, #ff8a5c 0, rgba(255,138,92,0) 42%)',
  'radial-gradient(ellipse at 78% 18%, #7a5cff 0, rgba(122,92,255,0) 48%)',
  'radial-gradient(ellipse at 62% 82%, #18c6a8 0, rgba(24,198,168,0) 46%)',
  'radial-gradient(ellipse at 30% 90%, #ffcf5c 0, rgba(255,207,92,0) 38%)',
  'linear-gradient(135deg, #1f2f74, #5a2b88 55%, #13395c)',
].join(',');

// How much of each surface is the theme's color: 1 is solid; less lets what's behind through.
const LOOKS = {
  solid: { chrome: 1, main: 1 },
  frosted: { chrome: 0.72, main: 0.72, blur: true },
  sidebar: { chrome: 0.72, main: 1, blur: true },
  clear: { chrome: 0.8, main: 0.8 },
  picture: { chrome: 0.8, main: 0.74, picture: 'sharp' },
  'picture-blur': { chrome: 0.6, main: 0.55, picture: 'soft' },
};

// Another app's window behind agentZ, so the mocks show what text through glass is like.
const otherWindow = (style) => `<div style="position:absolute;${style};background:#fbfbfd;border-radius:10px;box-shadow:0 10px 30px rgba(0,0,0,.35);overflow:hidden;color:#1d1d1f;font:13px/1.5 -apple-system,'Helvetica Neue',sans-serif">
  <div style="height:30px;background:#ececf0;display:flex;align-items:center;padding:0 10px;gap:6px"><i style="width:10px;height:10px;border-radius:50%;background:#ff5f57"></i><i style="width:10px;height:10px;border-radius:50%;background:#febc2e"></i><i style="width:10px;height:10px;border-radius:50%;background:#28c840"></i><span style="margin-left:14px;background:#fff;border-radius:5px;padding:1px 10px;font-size:11px;color:#666">docs.stripe.com/payments</span></div>
  <div style="padding:18px 22px">
    <div style="font-size:24px;font-weight:700;margin-bottom:8px">Accept a payment</div>
    <div style="height:90px;border-radius:8px;background:linear-gradient(120deg,#635bff,#00d4ff);margin-bottom:12px"></div>
    <p style="margin:0 0 10px">Build a payment form or use a prebuilt checkout page to start accepting online payments.</p>
    <p style="margin:0 0 10px;color:#425466">Create a PaymentIntent on your server with an amount and currency, then confirm it on the client.</p>
    <div style="height:70px;border-radius:8px;background:#0a2540;margin-bottom:12px"></div>
    <p style="margin:0">Listen for the payment_intent.succeeded event to fulfil orders.</p>
  </div></div>`;

const projectIcon = (color, text) => text
  ? `<span style="width:16px;height:16px;border-radius:4px;background:${color}22;color:${color};font:600 8px 'IBM Plex Mono',monospace;display:inline-grid;place-items:center;flex:none">${text}</span>`
  : `<span style="width:16px;height:16px;border-radius:4px;background:${color};display:inline-grid;place-items:center;flex:none"><i style="width:7px;height:7px;border-radius:2px;background:#fff"></i></span>`;
const agentBadge = (color) => `<span style="width:14px;height:14px;border-radius:3px;background:${color};display:inline-block;flex:none"></span>`;
const STATUS = {
  working: (t) => `<span class="row g1" style="color:${t.ac === '#3474f0' ? '#548af7' : t.ac};font-size:12px">${ic('loader', 'xs')}Working</span>`,
  pending: () => '<span class="row g1" style="color:#e8c46a;font-size:12px"><span class="dot" style="background:#e8c46a"></span>Pending Approval</span>',
  awaiting: () => '<span class="row g1" style="color:#b48ef0;font-size:12px"><span class="dot" style="background:#b48ef0"></span>Awaiting Input</span>',
  done: () => '<span class="row g1" style="color:#6fb377;font-size:12px"><span class="dot" style="background:#6fb377"></span>Completed</span>',
};
const STOREFRONT = ['#f07a2a', ''];
const ACME = ['#3fb6a0', 'AI'];
const RANKER = ['#6fb377', 'RR'];
const BILLING = ['#9bc35a', 'BG'];
const THREADS = [
  [STOREFRONT, 'storefront', 'working', 'Fix the flaky cart test', 'main', '#9aa0a6'],
  [STOREFRONT, 'storefront', 'working', 'Speed up product search', 'faster-search', '#2ea44f'],
  [ACME, 'acme/api', 'pending', 'Cursor pagination for /orders', 'main', '#3474f0'],
  [ACME, 'acme/api', 'awaiting', 'Rate-limit the webhooks endpoint', 'main', '#2ea44f'],
  [ACME, 'acme/api', 'done', 'Load-test /orders on staging', 'main', '#9aa0a6'],
  [STOREFRONT, 'storefront', '1m', 'Add Apple Pay to checkout', 'main', '#2ea44f'],
  [ACME, 'acme/api', '50m', 'Export orders as CSV', 'main', '#f0782a'],
  [RANKER, 'ranker', 'done', 'Retrain the search ranker', 'main', '#9aa0a6'],
  [BILLING, 'billing', '40m', 'Migrate invoices to Stripe v2', 'main', '#2ea44f'],
];

function threadCard(t, alpha, [project, name, status, title, branch, agent], selected) {
  const trailing = STATUS[status] ? STATUS[status](t) : `<span style="color:${t.mu};font-size:12px">${status}</span>`;
  return `<div style="margin:0 4px;padding:8px 10px;border-radius:7px;${selected ? `background:${rgba(t.sel, Math.min(1, alpha + 0.15))}` : ''}">
    <div class="row g2" style="height:22px">${projectIcon(...project)}<span class="grow trunc" style="color:${t.mu}">${name}</span>${trailing}</div>
    <div class="trunc" style="height:24px;line-height:24px;color:${t.t};font-size:14.5px">${title}</div>
    <div class="row g2" style="height:20px;color:${t.ph};font-size:12.5px">${branch === 'main' ? '' : ic('worktree', 'xs')}<span class="grow trunc">${branch}</span><span style="color:${t.ph}">${ic('laptop', 'xs')}</span>${agentBadge(agent)}</div></div>`;
}

const toolRow = (t, icon, label, trailing = '') => `<div class="row g2" style="height:28px;color:${t.mu};padding-left:10px">${ic(icon, 'sm')}<span class="grow">${label}</span>${trailing}</div>`;
const pickerChip = (t, label) => `<span class="row g1" style="color:${t.mu};font-size:12.5px">${label}${ic('chev-down', 'xs')}</span>`;

// agentZ's main window: title bar, sidebar of thread cards, and a thread at work.
function appWindow({ look = 'solid', strength, w = 1180, h = 740, x = 60, y = 50, theme = JBD, cards = THREADS.length, compact = false } = {}) {
  const t = theme;
  const inset = compact ? '24px' : '64px';
  const right = compact ? '20px' : '40px';
  const l = { ...LOOKS[look] };
  if (strength != null) {
    l.chrome = strength;
    if (l.main < 1) l.main = strength;
  }
  const line = rgba(t.border, l.chrome < 1 ? 0.75 : 1);
  const mainLine = rgba(t.border, l.main < 1 ? 0.75 : 1);
  const blur = l.blur ? 'backdrop-filter:blur(40px) saturate(170%);-webkit-backdrop-filter:blur(40px) saturate(170%);' : '';
  const picture = l.picture
    ? `<img src="img/picture.svg" style="position:absolute;inset:0;width:100%;height:100%;object-fit:cover;${l.picture === 'soft' ? 'filter:blur(26px) saturate(130%);transform:scale(1.15);' : ''}">`
    : '';
  const titlebar = `<div class="row" style="height:40px;flex:none;padding:0 10px 0 14px;gap:16px;background:${rgba(t.bg, l.chrome)};border-bottom:1px solid ${line};color:${t.t};font-size:13.5px">
    ${lights()}<span style="color:${t.mu}">${ic('sidebar', 'sm')}</span>
    <span class="row g15">${ic('list', 'sm')}All projects${ic('chev-down', 'xs')}</span>
    <span class="row g15">${ic('layers', 'sm')}Artifacts <span style="color:${t.mu}">2</span></span>
    <span class="grow"></span>
    <span class="row" style="font-size:13px;border-radius:7px;padding:2px;gap:2px"><span style="padding:3px 12px;border-radius:5px;background:${rgba(t.sel, Math.min(1, l.chrome + 0.1))}">Agents</span><span style="padding:3px 12px">Workspaces</span></span></div>`;
  const sidebarHtml = `<div class="col" style="width:290px;flex:none;background:${rgba(t.bg, l.chrome)};border-right:1px solid ${line}">
    <div class="row g2" style="height:40px;flex:none;padding:0 12px;color:${t.ph};border-bottom:1px solid ${line}">${ic('search', 'sm')}<span class="grow">Search...</span><span style="color:${t.mu}">${ic('plus', 'sm')}</span></div>
    <div class="col" style="flex:1;min-height:0;overflow:hidden;padding:6px 0;gap:2px">${THREADS.slice(0, cards).map((thread, index) => threadCard(t, l.chrome, thread, index === 1)).join('')}</div>
    <div class="row g2" style="height:38px;flex:none;padding:0 12px;color:${t.mu};border-top:1px solid ${line}">${ic('settings', 'sm')}<span style="color:${t.t}">Settings</span></div></div>`;
  const conversation = `<div class="col" style="flex:1;min-height:0;padding:20px ${right} 0 ${inset};gap:6px;overflow:hidden">
    <div style="display:flex;justify-content:flex-end;margin-bottom:12px"><div style="max-width:72%;background:${rgba(t.bubble, Math.min(1, l.main + 0.12))};border-radius:10px;padding:8px 13px;line-height:21px;color:${t.t}">Product search takes 900 ms on the catalog page. Profile it and make it fast.</div></div>
    ${toolRow(t, 'chev-right', 'Ran 1 command and read 1 file')}
    <div style="line-height:22px;color:${t.t};padding:2px 10px 4px">Each keystroke fetches all 12,000 products and filters them in the browser. I'll debounce the input and move the filtering to an indexed query.</div>
    ${toolRow(t, 'pencil', 'Edit src/app/search/useSearch.ts', `<span style="color:${t.mu}">${ic('loader', 'xs')}</span>`)}
    <div class="row g2" style="height:26px;color:${t.mu};padding-left:12px;font-size:13px">⠋ Working for 37s</div></div>`;
  const composer = `<div style="flex:none;border-top:1px solid ${mainLine};padding:12px ${right} 10px ${inset};background:${rgba(t.bg, l.composer ?? l.main)}">
    <div style="color:${t.ph};height:24px">Message the agent…</div>
    <div class="row g3" style="height:26px;margin-top:6px;color:${t.mu};white-space:nowrap;overflow:hidden">${ic('plus', 'sm')}<span class="row g1" style="color:${t.t};font-size:12.5px">${agentBadge('#2ea44f')}Codex</span><span style="font-size:12px;background:${rgba(t.sel, 0.7)};border-radius:4px;padding:0 5px">73%</span><span class="grow"></span>${pickerChip(t, 'Auto review')}${pickerChip(t, 'Default')}${pickerChip(t, '5.6 Terra')}${pickerChip(t, 'High')}<span style="width:20px;height:20px;border-radius:4px;background:#5a2a2e;display:inline-grid;place-items:center"><i style="width:8px;height:8px;border-radius:1px;background:#f0626e"></i></span></div></div>`;
  const mainHtml = `<div class="col grow" style="background:${rgba(t.bg, l.main)}">
    <div class="row g2" style="height:40px;flex:none;padding:0 12px;border-bottom:1px solid ${mainLine};color:${t.t};font-size:13.5px;white-space:nowrap;overflow:hidden">${projectIcon(...STOREFRONT)}<span style="color:${t.mu}">storefront</span><span style="color:${t.ph}">/</span><span>Speed up product search</span><span style="color:${t.mu}">${ic('chev-down', 'xs')}</span><span class="grow"></span>
      <span class="row g1" style="border:1px solid ${mainLine};border-radius:5px;padding:1px 8px;color:${t.mu};font-size:12.5px">${ic('worktree', 'xs')}faster-search</span><span style="color:${t.mu}">${ic('terminal', 'sm')}</span><span style="color:${t.mu}">${ic('more', 'sm')}</span></div>
    ${conversation}${composer}</div>`;
  return `<div style="position:absolute;left:${x}px;top:${y}px;width:${w}px;height:${h}px;border-radius:12px;overflow:hidden;box-shadow:0 24px 70px rgba(0,0,0,.5),0 0 0 1px rgba(0,0,0,.55);${blur}">
    ${picture}<div class="col" style="position:relative;height:100%">${titlebar}<div class="row" style="flex:1;min-height:0;align-items:stretch">${sidebarHtml}${mainHtml}</div></div></div>`;
}

// The desktop: a bright wallpaper and a browser window, with agentZ over them.
function onDesktop(win, { w = 1300, h = 840, other = 'left:0;top:170px;width:440px;height:560px' } = {}) {
  return frame(`<div style="position:absolute;inset:0;background:${WALLPAPER}"></div>${other ? otherWindow(other) : ''}${win}`, { w, h, style: 'background:#111' });
}
const look = (name, options = {}) => onDesktop(appWindow({ look: name, ...options }));

TOPICS.push({
  id: 'look', section: 'Window', title: "What's behind the window", size: 'wide', rec: 'C',
  now: "Solid. GPUI can draw a window clear or frosted (on macOS an NSVisualEffectView behind it), and Zed's themes turn that on with \"background.appearance\": \"blurred\" and colors with some transparency. agentZ has the code from Zed but never applies it to the window, so it's always solid, whatever the theme says.",
  nowImg: 'img/now-app.png',
  options: [
    {
      key: 'A', name: 'Solid', from: 'today',
      desc: 'Every surface is the theme\'s solid color.',
      good: 'The most readable; nothing to build.', cost: 'Looks like every other editor.',
      mock: () => look('solid'),
    },
    {
      key: 'B', name: 'Frosted window', from: 'Zed\'s "blurred" themes',
      desc: 'The whole window lets the desktop through, blurred and tinted with the theme\'s colors, as macOS\'s own sidebars and Zed\'s blurred themes do. Wallpaper colors and other windows show as soft color under everything.',
      good: 'The most "native glass" look; text stays readable because the blur removes detail.', cost: 'The conversation\'s color changes with whatever is behind it. On Linux, blur works only on KDE (its blur protocol); GNOME and others get plain translucency.',
      mock: () => look('frosted'),
    },
    {
      key: 'C', name: 'Frosted sidebar, solid conversation', from: 'macOS sidebars (Finder, Notes, Xcode)',
      desc: 'Only the title bar and sidebar are frosted; the thread, its composer and terminals stay solid. It\'s the same blurred window as B, with the theme\'s main colors kept opaque.',
      good: 'Modern and native, while what you read and type stays on a steady background.', cost: 'Subtler than B.',
      mock: () => look('sidebar'),
    },
    {
      key: 'D', name: 'Clear', from: 'Zed\'s "transparent" themes, terminals\' opacity',
      desc: 'The desktop shows through sharp, under a tint, like a terminal with background opacity. No blur.',
      good: 'You can watch what\'s behind the window.', cost: 'Text over text: other windows\' words show under the thread\'s.',
      mock: () => look('clear'),
    },
    {
      key: 'E', name: 'Your picture', from: 'new (as Warp and iTerm2 do)',
      desc: 'A picture you choose fills the window behind everything, and each surface is the theme\'s color over it, partly see-through. Works the same on every desktop, since agentZ draws it.',
      good: 'Personal; looks the same on macOS and Linux.', cost: 'Busy pictures fight the text; the tint has to be strong.',
      mock: () => look('picture'),
    },
    {
      key: 'F', name: 'Your picture, softened', from: 'new (as macOS\'s lock screen does)',
      desc: 'Like E, but the picture is blurred once when you pick it, so it reads as color and light rather than detail. GPUI can\'t blur at draw time, so agentZ keeps a blurred copy.',
      good: 'The glass look of B on any desktop, and calmer than E.', cost: 'The picture\'s detail is lost; it\'s a mood, not a photo.',
      mock: () => look('picture-blur'),
    },
  ],
});

// A smaller window, for comparing strengths side by side.
const small = (options, extra = '') => onDesktop(appWindow({ w: 780, h: 420, x: 50, y: 30, cards: 4, compact: true, ...options }) + extra, { w: 880, h: 480, other: 'left:0;top:120px;width:300px;height:340px' });
const sliderCard = () => `<div style="position:absolute;right:24px;bottom:18px;border:1px solid ${rgba(JBD.border, 1)};border-radius:8px;background:${rgba(JBD.bg, 1)};padding:10px 14px;box-shadow:0 8px 24px rgba(0,0,0,.45)"><div style="color:${JBD.t};font-size:13px;margin-bottom:8px">Shows through</div>${slider(28)}</div>`;

TOPICS.push({
  id: 'strength', section: 'Window', title: 'How much shows through', size: 'medium', rec: 'B',
  now: 'Nothing does.',
  options: [
    {
      key: 'A', name: 'A little', from: 'new',
      desc: 'Surfaces are 88% the theme\'s color: a hint of the wallpaper\'s color, mostly the theme.',
      good: 'Safe with any wallpaper.', cost: 'Easy to miss.',
      mock: () => small({ look: 'frosted', strength: 0.88 }),
    },
    {
      key: 'B', name: 'Some', from: 'Zed\'s blurred themes',
      desc: 'Around 72%, about what Zed\'s blurred themes use.',
      good: 'Clearly glass, still calm.', cost: 'Bright wallpapers lift the darks a little.',
      mock: () => small({ look: 'frosted', strength: 0.72 }),
    },
    {
      key: 'C', name: 'A lot', from: 'new',
      desc: 'Around 50%: the desktop\'s colors lead.',
      good: 'Striking.', cost: 'Contrast drops; muted text gets hard to read over light areas.',
      mock: () => small({ look: 'frosted', strength: 0.5 }),
    },
    {
      key: 'D', name: 'Your choice, with a slider', from: 'new',
      desc: 'Settings has a slider from solid to see-through, starting at B.',
      good: 'Fits any wallpaper or picture.', cost: 'One more setting.',
      mock: () => small({ look: 'frosted', strength: 0.72 }, sliderCard()),
    },
  ],
});

// Half see-through, half solid, for one part of the window.
const glassBackdrop = (inner, { w = 640, h = 270 } = {}) => onDesktop(`<div style="position:absolute;left:30px;top:20px;width:${w - 60}px;height:${h - 40}px;border-radius:12px;overflow:hidden;backdrop-filter:blur(40px) saturate(170%);-webkit-backdrop-filter:blur(40px) saturate(170%);background:${rgba(JBD.bg, 0.72)};box-shadow:0 0 0 1px rgba(0,0,0,.5);padding:16px;display:flex;gap:14px">${inner}</div>`, { w, h, other: 'left:0;top:60px;width:240px;height:240px' });
const half = (label, html) => `<div class="col grow" style="gap:8px"><div class="sm" style="color:${JBD.mu}">${label}</div>${html}</div>`;
const termBox = (alpha) => `<div style="flex:1;border:1px solid ${rgba(JBD.border, 0.8)};border-radius:6px;background:${rgba(JBD.ed, alpha)};padding:8px 10px;font:12px/18px 'IBM Plex Mono',monospace;color:${JBD.t}">
  <div><span style="color:#6fb377">storefront</span> <span style="color:#548af7">faster-search</span> ❯ npm test</div><div style="color:${JBD.mu}"> PASS  src/search/useSearch.test.ts</div><div style="color:${JBD.mu}"> PASS  src/cart/total.test.ts</div><div>Tests: <span style="color:#6fb377">24 passed</span>, 24 total</div></div>`;
const diffBox = (alpha) => `<div style="flex:1;border:1px solid ${rgba(JBD.border, 0.8)};border-radius:6px;overflow:hidden;font:12px/19px 'IBM Plex Mono',monospace">
  <div style="padding:4px 10px;background:${rgba(JBD.bg, alpha)};color:${JBD.mu};font-family:'IBM Plex Sans'">useSearch.ts</div>
  <div style="background:${rgba(JBD.ed, alpha)};color:${JBD.t}"><div style="padding:0 10px;background:rgba(250,102,117,${0.14 * alpha + 0.04})">- const all = await fetchAll()</div><div style="padding:0 10px;background:rgba(87,150,93,${0.18 * alpha + 0.05})">+ const hits = await query(term)</div><div style="padding:0 10px">  return hits.slice(0, 50)</div></div></div>`;
const menuBox = (alpha) => `<div style="flex:1;border:1px solid ${rgba(JBD.border, 0.9)};border-radius:8px;background:${rgba(JBD.bg, alpha)};padding:4px;box-shadow:0 8px 24px rgba(0,0,0,.35);font-size:13px;color:${JBD.t}">
  ${['5.6 Terra', '5.5 Terra', '5.6 Terra mini'].map((name, index) => `<div class="row" style="height:28px;padding:0 8px;border-radius:5px;${index === 0 ? `background:${rgba(JBD.sel, alpha)}` : ''}"><span class="grow">${name}</span>${index === 0 ? ic('check', 'xs') : ''}</div>`).join('')}</div>`;
const composerBox = (alpha) => `<div style="flex:1;border:1px solid ${rgba(JBD.border, 0.8)};border-radius:8px;background:${rgba(JBD.bg, alpha)};padding:10px 12px;color:${JBD.ph}">Message the agent…<div class="row g2" style="margin-top:22px;color:${JBD.mu};font-size:12.5px">${ic('plus', 'sm')}${agentBadge('#2ea44f')}<span style="color:${JBD.t}">Codex</span><span class="grow"></span>High${ic('chev-down', 'xs')}</div></div>`;
const solidPart = (box) => glassBackdrop(half('See-through', box(0.35)) + half('Solid', box(1)));

TOPICS.push({
  id: 'solid', section: 'Window', title: 'What stays solid', size: 'medium', type: 'multi', rec: ['A', 'B', 'C'],
  now: 'Everything is solid.',
  issues: ['In Zed\'s blurred themes, terminals and code blocks are as see-through as the rest, which is where legibility suffers most.'],
  options: [
    {
      key: 'A', name: 'Terminals', from: 'new',
      desc: 'Terminal panes and a thread\'s terminals keep the theme\'s terminal background, solid.',
      good: 'Colored output and TUIs (Claude, htop) read as designed.', cost: 'Terminals look heavier than the frosted parts.',
      mock: () => solidPart(termBox),
    },
    {
      key: 'B', name: 'Code, diffs and command output', from: 'new',
      desc: 'Edit diffs, code blocks and tool output in a thread stay solid.',
      good: 'Red and green diff lines keep their contrast.', cost: 'Cards stand out more against the glass.',
      mock: () => solidPart(diffBox),
    },
    {
      key: 'C', name: 'Menus, popovers and dialogs', from: 'Zed (elevated surfaces)',
      desc: 'Anything that opens over the window stays solid, so it reads over a busy background.',
      good: 'Menus never show the thread through them.', cost: 'None really.',
      mock: () => solidPart(menuBox),
    },
    {
      key: 'D', name: 'The composer', from: 'new',
      desc: 'The message box at the bottom of a thread stays solid.',
      good: 'What you type is always on a steady background.', cost: 'Breaks the glass along the bottom.',
      mock: () => solidPart(composerBox),
    },
  ],
});

// Settings › Appearance, as the page draws it, with the theme section and what's added.
const settingRow = (title, desc, control, last = false) => `<div class="row" style="padding:12px 16px;gap:16px;${last ? '' : `border-bottom:1px solid ${rgba(JBD.border, 1)}`}"><div class="col grow" style="gap:3px"><span style="color:${JBD.t};font-size:14px">${title}</span><span style="color:${JBD.mu};font-size:12px">${desc}</span></div>${control}</div>`;
const dropdown = (label) => `<span class="row g1" style="background:${rgba(JBD.sel, 0.8)};border-radius:5px;padding:2px 8px;color:${JBD.t};font-size:13px">${label}${ic('chev-down', 'xs')}</span>`;
const slider = (value) => `<span class="row g2" style="width:180px"><span class="xs" style="color:${JBD.ph}">Solid</span><span style="flex:1;height:4px;border-radius:2px;background:${rgba(JBD.sel, 1)};position:relative"><i style="position:absolute;left:0;top:0;bottom:0;width:${value}%;background:#3474f0;border-radius:2px"></i><i style="position:absolute;left:calc(${value}% - 7px);top:-5px;width:14px;height:14px;border-radius:50%;background:#dfe1e5"></i></span><span class="xs" style="color:${JBD.ph}">Clear</span></span>`;
const section = (title, rows) => `<div style="color:${JBD.mu};font-size:12.5px;margin:18px 0 8px">${title}</div><div style="border:1px solid ${rgba(JBD.border, 1)};border-radius:8px;background:${rgba(JBD.bg, 1)}">${rows}</div>`;
const settingsNav = () => `<div class="col" style="width:220px;flex:none;background:${rgba(JBD.bg, 1)};border-right:1px solid ${rgba(JBD.border, 1)};padding:10px 6px;gap:2px;font-size:14px;color:${JBD.t}">
  <div class="row" style="height:28px;padding:0 6px;margin-bottom:6px">Settings</div>
  ${[['settings', 'General'], ['eye', 'Appearance'], ['bell', 'Notifications'], ['zap', 'Agents'], ['activity', 'Usage']].map(([icon, name]) => `<div class="row g2" style="height:28px;padding:0 8px;border-radius:6px;${name === 'Appearance' ? `background:${rgba(JBD.sel, 1)}` : ''}"><span style="color:${JBD.mu}">${ic(icon, 'sm')}</span>${name}</div>`).join('')}</div>`;
const settingsPage = (content, h = 470, overlay = '') => frame(`<div class="row" style="height:100%;align-items:stretch;background:${rgba(JBD.ed, 1)}">${settingsNav()}<div class="grow" style="padding:16px 60px;overflow:hidden"><div style="color:${JBD.t};font-size:17px">Appearance</div>${content}</div></div>${overlay}`, { w: 1000, h });
const themeRows = (dark = 'JetBrains Dark', last = true) => settingRow('Mode', 'Follow macOS, or always use the light or dark theme.', dropdown('System'))
  + settingRow('Light theme', 'Used when the appearance is light.', dropdown('Catppuccin Latte'))
  + settingRow('Dark theme', 'Used when the appearance is dark.', dropdown(dark), last);
const pictureThumb = `<span class="row g2"><img src="img/picture.svg" style="width:54px;height:34px;border-radius:4px;object-fit:cover;border:1px solid ${rgba(JBD.border, 1)}"><span style="border:1px solid ${rgba(JBD.border, 1)};border-radius:5px;padding:2px 8px;color:${JBD.t};font-size:13px">Choose…</span></span>`;
const themeMenu = `<div style="position:absolute;right:76px;top:226px;width:230px;border:1px solid ${rgba(JBD.border, 1)};border-radius:8px;background:${rgba(JBD.bg, 1)};padding:4px;box-shadow:0 8px 24px rgba(0,0,0,.45);font-size:13px;color:${JBD.t}">
  ${['Ayu Dark', 'Gruvbox Dark', 'JetBrains Dark', 'JetBrains Dark Glass', 'One Dark', 'One Dark Glass'].map((name) => `<div class="row" style="height:28px;padding:0 8px;border-radius:5px;${name === 'JetBrains Dark Glass' ? `background:${rgba(JBD.sel, 1)}` : ''}"><span class="grow">${name}</span>${name === 'JetBrains Dark Glass' ? ic('check', 'xs') : ''}</div>`).join('')}</div>`;

TOPICS.push({
  id: 'setting', section: 'Settings', title: 'Where you turn it on', size: 'wide', rec: 'B',
  now: 'Settings › Appearance has the mode and a light and a dark theme. A theme can say "background.appearance", but agentZ ignores it.',
  options: [
    {
      key: 'A', name: 'Glass themes', from: 'Zed',
      desc: 'Nothing new in Settings: agentZ applies a theme\'s "background.appearance" as Zed does, and ships glass versions of its themes (their colors partly transparent) beside the solid ones. A picture can\'t be a theme, so E and F wouldn\'t fit here.',
      good: 'Exactly Zed\'s model; each theme tunes its own glass.', cost: 'Twice the themes; no pictures; no slider.',
      mock: () => settingsPage(section('Theme', themeRows('JetBrains Dark Glass')), 430, themeMenu),
    },
    {
      key: 'B', name: 'A Window section for any theme', from: 'new',
      desc: 'A Window section under Theme: Background (Solid, Frosted, or Picture), how much shows through, and for Picture, the picture and whether to soften it. It applies to whichever theme is on, light or dark, by making its surfaces partly transparent.',
      good: 'Works with every theme, and holds pictures and the slider.', cost: 'agentZ decides the transparency, not the theme\'s author.',
      mock: () => settingsPage(section('Theme', themeRows('JetBrains Dark', true)) + section('Window', settingRow('Background', 'What shows behind the window.', dropdown('Picture')) + settingRow('Picture', 'Shown behind the window, covering it.', pictureThumb) + settingRow('Soften picture', 'Blur it, so it reads as color rather than detail.', `<span style="width:34px;height:20px;border-radius:10px;background:#3474f0;position:relative;display:inline-block"><i style="position:absolute;right:3px;top:3px;width:14px;height:14px;border-radius:50%;background:#fff"></i></span>`) + settingRow('Shows through', 'How much of the theme\'s color is left.', slider(35), true)), 610),
    },
    {
      key: 'C', name: 'Both', from: 'Zed, plus new',
      desc: 'Themes that ask for glass get it, as in Zed, and the Window section of B can turn it on for any theme or choose a picture. B\'s setting wins when it\'s set.',
      good: 'Zed\'s glass themes work as their authors meant.', cost: 'Two places that decide the same thing.',
      mock: () => settingsPage(section('Theme', themeRows('JetBrains Dark Glass', true)) + section('Window', settingRow('Background', 'What shows behind the window. "As the theme says" follows the theme.', dropdown('As the theme says')) + settingRow('Shows through', 'How much of the theme\'s color is left.', slider(28), true)), 470),
    },
  ],
});
