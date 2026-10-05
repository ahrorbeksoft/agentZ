// Sounds when an agent finishes or needs input, when they play, and macOS notifications while
// agentZ isn't focused: Settings › General's new rows, and agents in Workspaces panes.

ICONS.updown = '<path d="m7 15 5 5 5-5"/><path d="m7 9 5-5 5 5"/>';
ICONS.sparkle = '<path d="M9.937 15.5A2 2 0 0 0 8.5 14.063l-6.135-1.582a.5.5 0 0 1 0-.962L8.5 9.936A2 2 0 0 0 9.937 8.5l1.582-6.135a.5.5 0 0 1 .963 0L14.063 8.5A2 2 0 0 0 15.5 9.937l6.135 1.581a.5.5 0 0 1 0 .964L15.5 14.063a2 2 0 0 0-1.437 1.437l-1.582 6.135a.5.5 0 0 1-.963 0z"/>';
ICONS['arrow-left'] = '<path d="m12 19-7-7 7-7"/><path d="M19 12H5"/>';
ICONS.volume = '<path d="M11 4.702a.705.705 0 0 0-1.203-.498L6.413 7.587A1.4 1.4 0 0 1 5.416 8H3a1 1 0 0 0-1 1v6a1 1 0 0 0 1 1h2.416a1.4 1.4 0 0 1 .997.413l3.383 3.384A.705.705 0 0 0 11 19.298z"/><path d="M16 9a5 5 0 0 1 0 6"/><path d="M19.364 18.364a9 9 0 0 0 0-12.728"/>';

// Settings controls as settings_page.rs draws them: ui's Switch and DropdownMenu, and
// ContextMenu's toggleable entries with the check at the end.
const toggle = (on) => `<span style="width:34px;height:20px;border-radius:10px;flex:none;display:inline-block;position:relative;vertical-align:middle;background:${on ? '#5a7099' : '#353a44'};border:1px solid ${on ? '#6f86b0' : 'var(--b)'}"><i style="position:absolute;top:2px;${on ? 'right:2px' : 'left:2px'};width:14px;height:14px;border-radius:50%;background:${on ? '#e6e9ee' : '#6b717d'}"></i></span>`;
const dd = (label) => `<span class="row" style="gap:4px;white-space:nowrap">${label}${ic('updown', 'xs mu')}</span>`;
const menuOf = (items, chosen, style = 'top:28px;right:-6px') => `<div class="menu" style="${style}">${items.map((item) => `<div class="it ${item === chosen ? 'hl' : ''}"><span class="grow">${item}</span>${item === chosen ? ic('check', 'sm') : ''}</div>`).join('')}</div>`;
const playBtn = (file, label) => `<span class="btn sm" style="cursor:pointer" onclick="event.stopPropagation();new Audio('audio/${file}').play()">${ic('play', 'xs')}${label}</span>`;

// render_row and render_section: a title and description beside the control, in a bordered
// group under a small muted heading.
const setRow = (title, desc, control, extra = '') => `<div class="row" style="padding:12px 16px;gap:24px;position:relative"><div class="col grow" style="gap:2px"><span>${title}</span>${desc ? `<span class="sm mu">${desc}</span>` : ''}</div><div class="none" style="position:relative">${control}${extra}</div></div>`;
const setSection = (title, rows) => `<div class="col" style="gap:8px"><span class="sm mu">${title}</span><div class="col" style="border:1px solid var(--b);border-radius:8px;background:var(--panel)">${rows.map((row, index) => `<div style="${index < rows.length - 1 ? 'border-bottom:1px solid var(--bv)' : ''}">${row}</div>`).join('')}</div></div>`;
const content = (sections, { heading = 'General', pad = 0 } = {}) => `<div class="col" style="padding:24px 32px ${24 + pad}px;gap:24px">${heading ? `<div style="font-size:17px">${heading}</div>` : ''}${sections.join('')}</div>`;
const piece = (html, w = 760) => `<div class="m" style="width:${w}px">${html}</div>`;

// What General shows today.
const threadsRows = () => [
  setRow('Thread order', 'How threads are sorted in the sidebar.', dd('Newest first')),
  setRow('Use modifier to send', 'Whether to always use cmd-enter (or ctrl-enter on Linux or Windows) to send messages.', toggle(false)),
  setRow('Show thinking', 'Whether the agent’s thinking shows open in threads. Otherwise it’s a Thinking row that opens on click.', toggle(false)),
];
const projectsRows = () => [
  setRow('Combine matching repositories across machines', 'Checkouts of one repository, on this Mac or other machines, share one entry in the projects list.', toggle(true)),
  setRow('Combine by', 'Projects from the same repository share one row.', dd('Group by repository')),
];
const serverRows = () => [setRow('Start at login', 'Starts the server when you log in, before agentZ opens, so scripts using agentz-server call can reach it.', toggle(false))];

const WHEN = ['Never', 'When hidden', 'Always'];
// The recommended rows, which the placement mocks show.
const finishedRow = (extra = '') => setRow('Sound when finished', 'When to play a sound as an agent finishes its turn.', dd('When hidden'), extra);
const inputRow = (extra = '') => setRow('Sound when input is needed', 'When to play a sound as an agent asks for a permission or an answer.', dd('Always'), extra);
const notifyRow = () => setRow('Notify when agentZ isn’t focused', 'Shows a macOS notification when an agent finishes or needs input while another app is in front.', toggle(true));
const recommendedRows = () => [finishedRow(), inputRow(), notifyRow()];

// The whole settings page (settings_page.rs render_nav and the content column), in the window.
function settingsWindow(contentHtml, { extraNav = [], selected = 'General', w = 1010, h = 860 } = {}) {
  const nav = [['General', 'settings'], ['Appearance', 'eye'], ...extraNav, ['Agents', 'sparkle'], ['Machines', 'server']]
    .map(([label, icon]) => `<div class="row g2" style="height:28px;padding:0 8px;border-radius:6px;margin:0 4px;${label === selected ? 'background:var(--sel)' : ''}">${ic(icon, 'sm mu')}<span>${label}</span></div>`).join('');
  return `<div class="m win" style="width:${w}px;height:${h}px">
    <div class="titlebar">${lights()}${ic('sidebar', 'sm mu')}<span class="row g15" style="font-size:13px">${ic('list', 'xs mu')}All projects${ic('chev-down', 'xs mu')}</span><div class="viewtabs"><span class="on">Agents</span><span>Workspaces</span></div></div>
    <div class="body"><div class="sidebar"><div class="sb-head" style="color:var(--t)"><span class="grow b5" style="color:var(--t)">Settings</span>${ic('x', 'sm mu')}</div><div class="sb-list">${nav}</div><div class="sb-foot">${ic('arrow-left', 'sm')}<span>Back</span></div></div>
    <div class="grow" style="height:100%;overflow:hidden;background:var(--ed)">${contentHtml}</div></div></div>`;
}

// A macOS notification banner in the screen's corner, as UNUserNotificationCenter shows it.
const banner = (title, body, style = 'top:14px;right:14px') => `<div style="position:absolute;${style};z-index:25;width:330px;border-radius:14px;padding:10px 12px;display:flex;gap:10px;align-items:flex-start;background:rgba(58,60,66,.94);border:1px solid rgba(255,255,255,.12);box-shadow:0 10px 30px rgba(0,0,0,.45);font:13px/1.3 -apple-system,'IBM Plex Sans',sans-serif">
  <span style="width:32px;height:32px;border-radius:8px;flex:none;background:linear-gradient(135deg,#3d4a5c,#232830);display:grid;place-items:center;color:var(--ac);font-weight:700">Z</span>
  <span class="col grow" style="min-width:0"><span class="row"><b class="grow trunc" style="color:#f2f3f5">${title}</b><span class="xs" style="color:#a9afbc">now</span></span><span style="color:#dfe2e7">${body}</span></span></div>`;
// Another app in front, with agentZ's window behind it.
const desktop = (inner, { w = 940, h = 480 } = {}) => `<div class="m" style="width:${w}px;height:${h}px;background:linear-gradient(160deg,#3a4458,#1f2430)">${inner}</div>`;
const otherApp = () => `<div style="position:absolute;left:470px;top:220px;width:440px;height:236px;border-radius:10px;background:#f4f4f6;box-shadow:0 18px 40px rgba(0,0,0,.4);z-index:5;color:#333;font:13px -apple-system,sans-serif"><div style="height:30px;border-bottom:1px solid #ddd;display:flex;align-items:center;padding:0 10px;gap:7px">${lights()}<span style="margin-left:8px;color:#666">Safari</span></div><div style="padding:18px;color:#888">docs.rs · tokio::sync::mpsc</div></div>`;

// The Workspaces tab the pane mocks show: Codex finished beside a shell, in a small window.
function workspacesWindow({ state = 'done', w = 570, h = 370, style = 'position:absolute;left:16px;top:70px' } = {}) {
  const codexDone = [`${B('>_ OpenAI Codex')} (v0.46)`, '', '▌ Write tests for the rate limiter', '', '• Ran cargo test -p limiter', '  └ 20 passed', '', `${C('tg', '•')} Added 6 tests for the rate limiter`];
  const codexAsk = SCREENS.codex;
  return `<div class="m win" style="width:${w}px;height:${h}px;${style}">
    <div class="titlebar">${lights()}${ic('sidebar', 'sm mu')}<div class="viewtabs"><span>Agents</span><span class="on">Workspaces</span></div></div>
    <div class="body"><div class="col grow" style="height:100%">${tabBar([['agents', state], ['server'], ['3']])}<div class="tile" style="flex:1;min-height:0">${split('h', 0.6,
      pane({ glyphKind: 'codex', title: 'Codex', detail: '~/storefront', state, focus: true, body: state === 'pending' ? codexAsk : codexDone }),
      pane({ title: 'zsh', detail: '~/storefront', body: SCREENS.shellIdle }))}</div></div></div></div>`;
}

// 1. The sounds ----------------------------------------------------------------------------
const soundCard = (rows, footer = '') => piece(`<div class="col" style="padding:16px;gap:10px">${rows.map(([label, file, detail]) => `<div class="row g3"><span class="grow">${label}<div class="sm mu">${detail}</div></span>${file ? playBtn(file, 'Play') : '<span class="sm ph">played in the chat</span>'}</div>`).join('')}${footer}</div>`, 420);

TOPICS.push({
  id: 'sounds', section: 'Sounds', title: 'The two sounds', size: 'medium', rec: 'A',
  now: 'No sounds. You heard these in the chat and picked t3code’s; this topic records it, and its buttons play them again.',
  options: [
    { key: 'A', name: 't3code’s completion and input', from: 't3code',
      desc: 'Two short chimes (0.65 s and 0.76 s), bundled as <code>assets/sounds/</code> and credited in the architecture doc.',
      good: 'Short and distinct from each other. Public domain (CC0), from freesound.org.', cost: 'Two files in the app (12 KB each).',
      mock: () => soundCard([['Finished', 'finished.mp3', 'notification-completion.mp3 · 0.65 s'], ['Needs input', 'needs-input.mp3', 'notification-input.mp3 · 0.76 s']]) },
    { key: 'B', name: 'herdr’s done and request', from: 'herdr',
      desc: 'herdr’s two built-in sounds for an agent going idle and one getting blocked.',
      good: 'Made for exactly these two events.', cost: 'Longer (1.1 s and 1.5 s). Apache-2.0.',
      mock: () => soundCard([['Finished', 'herdr-done.mp3', 'done.mp3 · 1.1 s'], ['Needs input', 'herdr-request.mp3', 'request.mp3 · 1.5 s']]) },
    { key: 'C', name: 'Zed’s agent done, and one for input', from: 'Zed',
      desc: 'Zed’s only agent sound for finishing; Zed has no second one, so input would take t3code’s or herdr’s.',
      good: 'What Zed users know.', cost: 'The longest (1.6 s), and the pair doesn’t match.',
      mock: () => soundCard([['Finished', 'zed-agent-done.m4a', 'agent_done.wav · 1.6 s'], ['Needs input', 'needs-input.mp3', 't3code’s, for example']]) },
    { key: 'D', name: 'macOS’s alert sounds', from: 'new',
      desc: 'Glass, Ping, Pop, Tink and the rest, from <code>/System/Library/Sounds</code>, played by name.',
      good: 'Nothing to bundle; familiar.', cost: 'The same sounds other apps use, so they don’t say “agentZ”.',
      mock: () => soundCard([['Finished', null, 'Glass, for example'], ['Needs input', null, 'Ping, for example']]) },
  ],
});

// 2. When each sound plays -----------------------------------------------------------------
TOPICS.push({
  id: 'sound-settings', section: 'Settings', title: 'When each sound plays', size: 'wide', rec: 'B',
  now: 'Nothing to set: there are no sounds. General has Threads, Projects and Server sections.',
  nowImg: 'img/now-general.png',
  options: [
    { key: 'A', name: 'One dropdown for both', from: 'Zed',
      desc: 'Zed’s “Play sound when agent done”, in its words: Never, When hidden, Always. Each event plays its own sound, but they share the rule.',
      good: 'Zed’s setting exactly; one row.', cost: 'Can’t have input always and finishing only when hidden.',
      mock: () => piece(content([setSection('Notifications', [setRow('Play sound when agent done', 'When to play a sound when the agent has either completed its response, or needs user input.', dd('When hidden'), menuOf(WHEN, 'When hidden'))])], { heading: '', pad: 70 })) },
    { key: 'B', name: 'A dropdown for each sound', from: 'Zed, per sound',
      desc: 'Zed’s three values on each event’s row. Defaults: finishing plays when hidden, input always (as herdr does, since you need to answer either way).',
      good: 'Both of your asks: a different sound per event, and on screen or hidden decided per event.', cost: 'Two rows instead of one.',
      mock: () => piece(content([setSection('Notifications', [finishedRow(), inputRow(menuOf(WHEN, 'Always'))])], { heading: '', pad: 70 })) },
    { key: 'C', name: 'On screen and hidden, as a grid', from: 'new',
      desc: 'One row with a small table: each event’s sound on or off for a thread on screen and a hidden one.',
      good: 'All four cases visible at once.', cost: 'A table in a list of rows; “on screen only” is a combination no one wants.',
      mock: () => piece(content([setSection('Notifications', [`<div class="col" style="padding:12px 16px;gap:10px"><div class="col" style="gap:2px"><span>Sounds</span><span class="sm mu">Which sounds play for a thread on screen, and for one that’s hidden.</span></div>
        <div style="display:grid;grid-template-columns:1fr 110px 110px;row-gap:10px;align-items:center"><span></span><span class="sm mu" style="text-align:center">On screen</span><span class="sm mu" style="text-align:center">Hidden</span>
        <span class="row g2">${playBtn('finished.mp3', '')}Finished</span><span style="text-align:center">${toggle(false)}</span><span style="text-align:center">${toggle(true)}</span>
        <span class="row g2">${playBtn('needs-input.mp3', '')}Needs input</span><span style="text-align:center">${toggle(true)}</span><span style="text-align:center">${toggle(true)}</span></div></div>`])], { heading: '' })) },
    { key: 'D', name: 'Two switches with fixed rules', from: 'herdr',
      desc: 'herdr’s rules: the finished sound only when hidden, the input sound always. A switch turns each off.',
      good: 'The simplest; the rules are what most people want.', cost: 'No finished sound for a thread you’re watching, and no way to quiet input on screen.',
      mock: () => piece(content([setSection('Notifications', [setRow('Sound when finished', 'Plays as an agent finishes its turn, unless you’re looking at it.', toggle(true)), setRow('Sound when input is needed', 'Plays as an agent asks for a permission or an answer, even on screen.', toggle(true))])], { heading: '' })) },
    { key: 'E', name: 'One mode for sounds and notifications', from: 't3code',
      desc: 't3code’s “Thread notifications”: Off, Notifications only, Sound only, Notifications with sound. Sounds play on screen too; notifications only while agentZ isn’t focused.',
      good: 'One control for everything in this round.', cost: 'No on screen or hidden choice for sounds, which was your first ask.',
      mock: () => piece(content([setSection('Notifications', [setRow('Thread notifications', 'Alerts when a thread finishes or needs input or approval. Applies to this Mac.', dd('Notifications with sound'), menuOf(['Off', 'Notifications only', 'Sound only', 'Notifications with sound'], 'Notifications with sound', 'top:28px;right:-6px;min-width:240px'))])], { heading: '', pad: 100 })) },
  ],
});

// 3. Notifications -------------------------------------------------------------------------
TOPICS.push({
  id: 'notifications', section: 'Settings', title: 'Notifications', size: 'wide', rec: 'A',
  now: 'A macOS notification for every thread that isn’t on screen, even with agentZ in front and another thread open. No setting turns it off (only macOS’s own). Clicking one opens its thread.',
  options: [
    { key: 'A', name: 'One switch: only while agentZ isn’t focused', from: 't3code',
      desc: 't3code’s rule: a system notification only when its window isn’t focused. With agentZ in front you get the sidebar’s state and the sound instead. On by default.',
      good: 'What you asked for, in one row.', cost: 'No notification for another thread while you work in agentZ (the sound and the sidebar still tell you).',
      mock: () => piece(content([setSection('Notifications', [notifyRow()])], { heading: '' })) },
    { key: 'B', name: 'A dropdown that keeps today’s rule', from: 'Zed’s dropdown, new values',
      desc: 'When agentZ isn’t focused (the default), When the thread is hidden (today’s), or Never.',
      good: 'Today’s behavior stays a choice.', cost: 'One more value to explain.',
      mock: () => piece(content([setSection('Notifications', [setRow('Notifications', 'When to show a macOS notification as an agent finishes or needs input.', dd('When agentZ isn’t focused'), menuOf(['Never', 'When the thread is hidden', 'When agentZ isn’t focused'], 'When agentZ isn’t focused', 'top:28px;right:-6px;min-width:250px'))])], { heading: '', pad: 70 })) },
    { key: 'C', name: 'A switch for each event', from: 'new',
      desc: 'Notify when finished, and notify when input is needed, each only while agentZ isn’t focused.',
      good: 'You can be called back only for questions, say.', cost: 'Two rows where one usually does.',
      mock: () => piece(content([setSection('Notifications', [setRow('Notify when finished', 'A macOS notification as an agent finishes while another app is in front.', toggle(true)), setRow('Notify when input is needed', 'A macOS notification as an agent asks for a permission or an answer while another app is in front.', toggle(true))])], { heading: '' })) },
    { key: 'D', name: 'In one table with the sounds', from: 'new',
      desc: 'A row per event, with its sound’s dropdown and a Notify switch side by side.',
      good: 'Everything about an event in one place.', cost: 'A table unlike the other rows.',
      mock: () => piece(content([setSection('Notifications', [`<div class="col" style="padding:12px 16px;gap:10px"><div style="display:grid;grid-template-columns:1fr 150px 80px;row-gap:12px;align-items:center"><span></span><span class="sm mu">Sound</span><span class="sm mu" style="text-align:center">Notify</span>
        <span>Finished<div class="sm mu">An agent finishes its turn.</div></span><span>${dd('When hidden')}</span><span style="text-align:center">${toggle(true)}</span>
        <span>Needs input<div class="sm mu">A permission or a question.</div></span><span>${dd('Always')}</span><span style="text-align:center">${toggle(true)}</span></div><span class="xs ph">Notifications show while agentZ isn’t focused.</span></div>`])], { heading: '' })) },
  ],
});

// 4. Where the rows go ---------------------------------------------------------------------
TOPICS.push({
  id: 'placement', section: 'Settings', title: 'Where the rows go', size: 'wide', rec: 'A',
  now: 'Settings › General has Threads (order, modifier to send, show thinking), Projects and Server. The mocks show the recommended rows; they’ll be the ones you pick above.',
  nowImg: 'img/now-general.png',
  options: [
    { key: 'A', name: 'A Notifications section in General', from: 't3code',
      desc: 'Its own section under Threads, as t3code keeps its notification setting in General.',
      good: 'Easy to find, with a clear heading.', cost: 'General grows a fourth section.',
      mock: () => settingsWindow(content([setSection('Threads', threadsRows()), setSection('Notifications', recommendedRows()), setSection('Projects', projectsRows())])) },
    { key: 'B', name: 'At the end of Threads', from: 'new',
      desc: 'The rows join Threads, after Show thinking.',
      good: 'No new section; they are about threads.', cost: 'Threads gets long, and the sound rows mix with typing and display ones.',
      mock: () => settingsWindow(content([setSection('Threads', [...threadsRows(), ...recommendedRows()]), setSection('Projects', projectsRows())])) },
    { key: 'C', name: 'A Notifications page', from: 'new',
      desc: 'A page of its own in the settings list, with a bell, after Appearance.',
      good: 'Room for more later.', cost: 'A page for three rows.',
      mock: () => settingsWindow(content([setSection('Sounds', [finishedRow(), inputRow()]), setSection('macOS notifications', [notifyRow()])], { heading: 'Notifications' }), { extraNav: [['Notifications', 'bell']], selected: 'Notifications' }) },
  ],
});

// 5. Hearing a sound from Settings ---------------------------------------------------------
TOPICS.push({
  id: 'preview', section: 'Settings', title: 'Hearing a sound from Settings', size: 'wide', rec: 'A',
  now: 'Nothing to preview yet.',
  options: [
    { key: 'A', name: 'It plays when picked', from: 'macOS Sound settings',
      desc: 'Choosing When hidden or Always plays that event’s sound once, as picking an alert sound in macOS does.',
      good: 'No extra control; you hear what you turned on.', cost: 'No way to hear it again without picking again.',
      mock: () => piece(content([setSection('Notifications', [finishedRow(), inputRow(menuOf(WHEN, 'Always'))])], { heading: '', pad: 70 }) + note('♪ the input sound plays', 'top:184px;left:300px'), 760) },
    { key: 'B', name: 'A play button on each row', from: 'new',
      desc: 'A small ▶ before each sound’s dropdown.',
      good: 'Hear it any time.', cost: 'Another control on each row.',
      mock: () => piece(content([setSection('Notifications', [setRow('Sound when finished', 'When to play a sound as an agent finishes its turn.', `<span class="row g2">${playBtn('finished.mp3', '')}${dd('When hidden')}</span>`), setRow('Sound when input is needed', 'When to play a sound as an agent asks for a permission or an answer.', `<span class="row g2">${playBtn('needs-input.mp3', '')}${dd('Always')}</span>`)])], { heading: '' })) },
    { key: 'C', name: 'No preview', from: 'Zed',
      desc: 'As Zed: you hear it the next time an agent finishes.',
      good: 'Nothing to build.', cost: 'You find out what it sounds like later.',
      mock: () => piece(content([setSection('Notifications', [finishedRow(), inputRow()])], { heading: '' })) },
  ],
});

// 6. Agents in Workspaces panes ------------------------------------------------------------
TOPICS.push({
  id: 'panes', section: 'Workspaces', title: 'Agents in terminal panes', size: 'wide', rec: 'A',
  now: 'An agent CLI in a Workspaces pane (Claude Code, Codex…) shows its state on the pane and in the sidebar’s Agents list, but makes no sound and no notification. Threads in panes already count as threads.',
  options: [
    { key: 'A', name: 'The same as threads', from: 'herdr',
      desc: 'As herdr does for its panes: the finished sound when one goes idle after working, the input sound when it gets blocked on a prompt, by the same settings, with the pane counting as on screen when its tab is showing. While agentZ isn’t focused, a notification titled with the agent, “storefront › agents · Finished”; clicking it shows and focuses the pane.',
      good: 'Every agent behaves the same, wherever it runs.', cost: 'Terminal agents’ states are guessed from the screen, so a sound can come early or late now and then.',
      mock: () => desktop(`${workspacesWindow()}${otherApp()}${banner('Codex', 'storefront › agents · Finished')}${note('♪ the finished sound plays', 'top:86px;right:24px')}`) },
    { key: 'B', name: 'Sounds only', from: 'new',
      desc: 'Pane agents play the sounds by the same settings, but never post a notification.',
      good: 'Fewer notifications.', cost: 'Away from agentZ, you only hear that some agent finished, not which.',
      mock: () => desktop(`${workspacesWindow()}${otherApp()}${note('♪ the finished sound plays', 'top:24px;right:24px')}`) },
    { key: 'C', name: 'Neither, as today', from: 'today',
      desc: 'Only threads make sounds and notifications.',
      good: 'Nothing changes for panes.', cost: 'A finished Codex in a pane goes unnoticed until you look.',
      mock: () => desktop(`${workspacesWindow()}${otherApp()}`) },
  ],
});
