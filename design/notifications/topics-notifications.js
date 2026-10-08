// More choices for when each sound plays (away from the thread or away from agentZ), a volume
// for agentZ's sounds, and the system notifications row: Settings › Notifications.

ICONS.updown = '<path d="m7 15 5 5 5-5"/><path d="m7 9 5-5 5 5"/>';
ICONS.sparkle = '<path d="M9.937 15.5A2 2 0 0 0 8.5 14.063l-6.135-1.582a.5.5 0 0 1 0-.962L8.5 9.936A2 2 0 0 0 9.937 8.5l1.582-6.135a.5.5 0 0 1 .963 0L14.063 8.5A2 2 0 0 0 15.5 9.937l6.135 1.581a.5.5 0 0 1 0 .964L15.5 14.063a2 2 0 0 0-1.437 1.437l-1.582 6.135a.5.5 0 0 1-.963 0z"/>';
ICONS['arrow-left'] = '<path d="m12 19-7-7 7-7"/><path d="M19 12H5"/>';
ICONS.volume = '<path d="M11 4.702a.705.705 0 0 0-1.203-.498L6.413 7.587A1.4 1.4 0 0 1 5.416 8H3a1 1 0 0 0-1 1v6a1 1 0 0 0 1 1h2.416a1.4 1.4 0 0 1 .997.413l3.383 3.384A.705.705 0 0 0 11 19.298z"/><path d="M16 9a5 5 0 0 1 0 6"/><path d="M19.364 18.364a9 9 0 0 0 0-12.728"/>';
ICONS['volume-low'] = '<path d="M11 4.702a.705.705 0 0 0-1.203-.498L6.413 7.587A1.4 1.4 0 0 1 5.416 8H3a1 1 0 0 0-1 1v6a1 1 0 0 0 1 1h2.416a1.4 1.4 0 0 1 .997.413l3.383 3.384A.705.705 0 0 0 11 19.298z"/>';

// Settings controls as settings_page.rs draws them: ui's Switch and DropdownMenu (its label
// turns the accent color while its menu is open), and ContextMenu's entries.
const toggle = (on) => `<span style="width:34px;height:20px;border-radius:10px;flex:none;display:inline-block;position:relative;vertical-align:middle;background:${on ? '#5a7099' : '#353a44'};border:1px solid ${on ? '#6f86b0' : 'var(--b)'}"><i style="position:absolute;top:2px;${on ? 'right:2px' : 'left:2px'};width:14px;height:14px;border-radius:50%;background:${on ? '#e6e9ee' : '#6b717d'}"></i></span>`;
const dd = (label, open = false) => `<span class="row" style="gap:4px;white-space:nowrap;${open ? 'color:var(--ac)' : ''}">${label}${ic('updown', 'xs mu')}</span>`;
const menuOf = (items, chosen, style = 'top:28px;right:-6px') => `<div class="menu" style="${style}">${items.map((item) => `<div class="it ${item === chosen ? 'hl' : ''}"><span class="grow">${item}</span>${item === chosen ? ic('check', 'sm') : ''}</div>`).join('')}</div>`;
const AUDIO = { finished: '../sounds/audio/zed-agent-done.m4a', input: '../sounds/audio/needs-input.mp3' };
const playAt = (file, volume, label) => `<span class="btn sm" style="cursor:pointer" onclick="event.stopPropagation();const a=new Audio('${file}');a.volume=${volume};a.play()">${ic('play', 'xs')}${label}</span>`;

// render_row and render_section: a title and description beside the control, in a bordered
// group under a small muted heading.
const setRow = (title, desc, control, extra = '') => `<div class="row" style="padding:12px 16px;gap:24px;position:relative"><div class="col grow" style="gap:2px"><span>${title}</span>${desc ? `<span class="sm mu">${desc}</span>` : ''}</div><div class="none" style="position:relative">${control}${extra}</div></div>`;
const setSection = (title, rows) => `<div class="col" style="gap:8px"><span class="sm mu">${title}</span><div class="col" style="border:1px solid var(--b);border-radius:8px;background:var(--panel)">${rows.map((row, index) => `<div style="${index < rows.length - 1 ? 'border-bottom:1px solid var(--bv)' : ''}">${row}</div>`).join('')}</div></div>`;
const content = (sections, { heading = 'Notifications', pad = 0 } = {}) => `<div class="col" style="padding:24px 32px ${24 + pad}px;gap:24px">${heading ? `<div style="font-size:17px">${heading}</div>` : ''}${sections.join('')}</div>`;
const piece = (html, w = 760) => `<div class="m" style="width:${w}px">${html}</div>`;

// A slider, which the ported ui doesn't have yet: a track filled to the level, with a knob.
const slider = (level, { w = 170, ends = true } = {}) => `<span class="row g2" style="white-space:nowrap">${ends ? ic('volume-low', 'sm mu') : ''}<span style="position:relative;display:inline-block;width:${w}px;height:4px;border-radius:2px;background:#3d424d"><i style="position:absolute;left:0;top:0;bottom:0;width:${level * 100}%;border-radius:2px;background:#5a7099"></i><i style="position:absolute;top:-6px;left:calc(${level * 100}% - 8px);width:16px;height:16px;border-radius:50%;background:#e6e9ee;box-shadow:0 1px 3px rgba(0,0,0,.5)"></i></span>${ends ? ic('volume', 'sm mu') : ''}</span>`;

// Rows as they are today.
const ZED_WHEN = ['Never', 'When hidden', 'Always'];
const FINISHED_DESC = 'When to play a sound as an agent finishes its turn.';
const INPUT_DESC = 'When to play a sound as an agent asks for a permission or an answer.';
const notifyRow = (title = 'Notify when agentZ isn’t focused', desc = 'Shows a macOS notification when an agent finishes or needs input while another app is in front.') => setRow(title, desc, toggle(true));

// The recommended choices (topics 1 and 2), which the later mocks show.
const WHEN = ['Never', 'When away from the thread', 'When away from agentZ', 'Always'];
const finishedRow = (value = 'When away from the thread', extra = '') => setRow('Sound when finished', FINISHED_DESC, dd(value, !!extra), extra);
const inputRow = (value = 'Always', extra = '') => setRow('Sound when input is needed', INPUT_DESC, dd(value, !!extra), extra);
const volumeRow = (level = 0.6) => setRow('Volume', 'How loud agentZ’s sounds play. Letting go plays the finished sound.', slider(level));
const wideMenu = (items, chosen, top = 28) => menuOf(items, chosen, `top:${top}px;right:-6px;min-width:230px`);

// The settings window around the page (settings_page.rs render_nav), for placement mocks.
function settingsWindow(contentHtml, { w = 1010, h = 640 } = {}) {
  const nav = [['General', 'settings'], ['Appearance', 'eye'], ['Notifications', 'bell'], ['Agents', 'sparkle'], ['Machines', 'server']]
    .map(([label, icon]) => `<div class="row g2" style="height:28px;padding:0 8px;border-radius:6px;margin:0 4px;${label === 'Notifications' ? 'background:var(--sel)' : ''}">${ic(icon, 'sm mu')}<span>${label}</span></div>`).join('');
  return `<div class="m win" style="width:${w}px;height:${h}px">
    <div class="titlebar">${lights()}${ic('sidebar', 'sm mu')}<span class="row g15" style="font-size:13px">${ic('list', 'xs mu')}All projects${ic('chev-down', 'xs mu')}</span><div class="viewtabs"><span class="on">Agents</span><span>Workspaces</span></div></div>
    <div class="body"><div class="sidebar"><div class="sb-head" style="color:var(--t)"><span class="grow b5" style="color:var(--t)">Settings</span>${ic('x', 'sm mu')}</div><div class="sb-list">${nav}</div><div class="sb-foot">${ic('arrow-left', 'sm')}<span>Back</span></div></div>
    <div class="grow" style="height:100%;overflow:hidden;background:var(--ed)">${contentHtml}</div></div></div>`;
}

// 1. The choices -----------------------------------------------------------------------------
TOPICS.push({
  id: 'when', section: 'Sounds', title: 'When each sound plays', size: 'wide', rec: 'A',
  now: 'Each sound has Never, When hidden or Always. When hidden plays whenever the thread isn’t on screen: another app in front, Settings open, another thread open, or its Workspaces tab not showing. So it also plays while you work in agentZ on another thread. Defaults: finished When hidden, input Always.',
  nowImg: 'img/now.png',
  issues: ['No choice for “only when I’m in another app”.', '“When hidden” doesn’t say hidden from what.'],
  options: [
    { key: 'A', name: 'Four choices in each dropdown', from: 'Zed, one value added',
      desc: 'Never, When away from the thread (today’s When hidden), When away from agentZ (only while another app is in front), Always. Away from the thread includes away from agentZ. Settings open counts as away from the thread, not from agentZ. Defaults stay: finished away from the thread, input always.',
      good: 'One row per sound, as now. Each sound can have its own rule.', cost: 'A fourth value to read.',
      mock: () => piece(content([setSection('Sounds', [finishedRow('When away from agentZ', wideMenu(WHEN, 'When away from agentZ')), inputRow()])], { pad: 70 })) },
    { key: 'B', name: 'Zed’s three, and a switch for what hidden means', from: 'new',
      desc: 'The dropdowns keep Never, When hidden, Always. A row under them, “Only when agentZ isn’t in front”, makes When hidden mean away from agentZ for both sounds.',
      good: 'Zed’s values stay as Zed users know them.', cost: 'The switch only matters when a sound is set to When hidden, and it’s shared, so finished and input can’t differ.',
      mock: () => piece(content([setSection('Sounds', [finishedRow('When hidden'), inputRow(), setRow('Only when agentZ isn’t in front', 'When hidden waits until another app is in front, not just another thread.', toggle(true))])])) },
    { key: 'C', name: 'A grid of where you are', from: 'new',
      desc: 'One row per sound, with a switch for each place you might be: looking at the thread, in another thread, in another app.',
      good: 'Every case shown at once, and any mix possible.', cost: 'A table unlike the other rows, and mixes no one wants (on screen but not in another thread).',
      mock: () => piece(content([setSection('Sounds', [`<div class="col" style="padding:12px 16px;gap:10px"><div class="col" style="gap:2px"><span>When sounds play</span><span class="sm mu">Where you are as an agent finishes or asks for input.</span></div>
        <div style="display:grid;grid-template-columns:1fr 130px 130px 130px;row-gap:12px;align-items:center"><span></span><span class="sm mu" style="text-align:center">On the thread</span><span class="sm mu" style="text-align:center">In another thread</span><span class="sm mu" style="text-align:center">In another app</span>
        <span>Finished</span><span style="text-align:center">${toggle(false)}</span><span style="text-align:center">${toggle(true)}</span><span style="text-align:center">${toggle(true)}</span>
        <span>Needs input</span><span style="text-align:center">${toggle(true)}</span><span style="text-align:center">${toggle(true)}</span><span style="text-align:center">${toggle(true)}</span></div></div>`])])) },
    { key: 'D', name: 'Hidden means away from agentZ', from: 't3code’s focus rule',
      desc: 'Keep three values, and When hidden plays only while another app is in front, as t3code’s notifications decide.',
      good: 'No new value; what you asked for becomes the only meaning.', cost: 'Today’s choice is gone: no sound for another thread while you work in agentZ, unless you pick Always.',
      mock: () => piece(content([setSection('Sounds', [finishedRow('When hidden', menuOf(ZED_WHEN, 'When hidden')), inputRow()])], { pad: 50 })) },
  ],
});

// 2. Names and descriptions -------------------------------------------------------------------
const namesMock = (thread, app, desc = FINISHED_DESC) => piece(content([setSection('Sounds', [setRow('Sound when finished', desc, dd(app, true), wideMenu(['Never', thread, app, 'Always'], app)), setRow('Sound when input is needed', INPUT_DESC, dd('Always'))])], { pad: 70 }));
TOPICS.push({
  id: 'names', section: 'Sounds', title: 'What the two new choices are called', size: 'wide', rec: 'A',
  now: 'One choice, When hidden, from Zed. The rows’ descriptions say when the sound is for, not where you have to be.',
  nowImg: 'img/now.png',
  options: [
    { key: 'A', name: 'Away from the thread, away from agentZ', from: 'new',
      desc: '“When away from the thread” and “When away from agentZ”. Same shape, so the difference is the last word. Descriptions unchanged.',
      good: 'Short, and the pair reads as one scale.', cost: '“Away” is a little loose: Settings open counts as away from the thread.',
      mock: () => namesMock('When away from the thread', 'When away from agentZ') },
    { key: 'B', name: 'In another thread, in another app', from: 'new',
      desc: '“When in another thread” and “When in another app”. Descriptions unchanged.',
      good: 'Says where you are.', cost: '“In another thread” also plays in another app, and with Settings open, which the name doesn’t say.',
      mock: () => namesMock('When in another thread', 'When in another app') },
    { key: 'C', name: 'Unless it’s showing, unless agentZ is in front', from: 'new',
      desc: '“Unless it’s on screen” and “Unless agentZ is in front”, named by when it stays quiet.',
      good: 'Exact: says when you won’t hear it.', cost: 'Negatives are slower to read, and longer.',
      mock: () => namesMock('Unless it’s on screen', 'Unless agentZ is in front') },
    { key: 'D', name: 'Zed’s word, twice', from: 'Zed',
      desc: '“When thread is hidden” and “When agentZ is hidden”, keeping Zed’s word.',
      good: 'Closest to Zed.', cost: '“agentZ is hidden” reads as minimized, but it also means behind another app.',
      mock: () => namesMock('When thread is hidden', 'When agentZ is hidden') },
    { key: 'E', name: 'Option A, with descriptions that explain', from: 'new',
      desc: 'Option A’s names, and each row’s description gains a sentence: “Away from the thread means it isn’t on screen; away from agentZ means another app is in front.”',
      good: 'Nothing left to guess.', cost: 'Two long descriptions saying the same thing.',
      mock: () => namesMock('When away from the thread', 'When away from agentZ', `${FINISHED_DESC} Away from the thread means it isn’t on screen; away from agentZ means another app is in front.`) },
  ],
});

// 3. The volume control -----------------------------------------------------------------------
TOPICS.push({
  id: 'volume', section: 'Volume', title: 'How you set the volume', size: 'wide', rec: 'A',
  now: 'No volume: both sounds play at the system’s volume. Zed, t3code and herdr have none either. macOS’s Sound settings have one “Alert volume” slider for all alert sounds. agentZ can set it on both systems: NSSound’s volume on macOS, pw-play’s or paplay’s --volume on Linux.',
  nowImg: 'img/now.png',
  options: [
    { key: 'A', name: 'One slider for both sounds', from: 'macOS Alert volume',
      desc: 'A Volume row with a slider from silent to full, applied to both sounds, on top of the system volume. Default full, so nothing changes until you move it.',
      good: 'What people expect for volume; one setting.', cost: 'The ported ui has no slider yet, so it’s a new small component.',
      mock: () => piece(content([setSection('Sounds', [volumeRow(), finishedRow(), inputRow()])])) },
    { key: 'B', name: 'A slider for each sound', from: 'new',
      desc: 'Each sound’s row gets its own slider under its dropdown.',
      good: 'Quiet finished sounds and loud questions, say.', cost: 'Two sliders where one usually does, and taller rows.',
      mock: () => piece(content([setSection('Sounds', [
        setRow('Sound when finished', FINISHED_DESC, `<span class="col" style="gap:10px;align-items:flex-end">${dd('When away from the thread')}${slider(0.4, { w: 140 })}</span>`),
        setRow('Sound when input is needed', INPUT_DESC, `<span class="col" style="gap:10px;align-items:flex-end">${dd('Always')}${slider(0.8, { w: 140 })}</span>`)])])) },
    { key: 'C', name: 'Three steps in a dropdown', from: 'Zed’s dropdown, new values',
      desc: 'A Volume row with Quiet (25%), Medium (50%) and Loud (full). Picking one plays the finished sound at that level. The buttons here play the three levels.',
      good: 'No new component; picks like the rows under it.', cost: 'Only three levels.',
      mock: () => piece(content([setSection('Sounds', [setRow('Volume', 'How loud agentZ’s sounds play.', dd('Medium', true), menuOf(['Quiet', 'Medium', 'Loud'], 'Medium', 'top:28px;right:-6px;min-width:150px')), finishedRow(), inputRow()]),
        `<span class="row g2">${playAt(AUDIO.finished, 0.25, 'Quiet')}${playAt(AUDIO.finished, 0.5, 'Medium')}${playAt(AUDIO.finished, 1, 'Loud')}</span>`])) },
    { key: 'D', name: 'A percentage', from: 'Zed’s number fields',
      desc: 'A Volume row with a number from 0 to 100% and − and + buttons, in steps of 10, as Zed’s settings set font sizes.',
      good: 'Exact, and the same as Zed’s number settings.', cost: 'Slower to change than a slider, and the number means little.',
      mock: () => piece(content([setSection('Sounds', [setRow('Volume', 'How loud agentZ’s sounds play.', `<span class="row" style="gap:0;border:1px solid var(--b);border-radius:6px;height:26px;white-space:nowrap"><span style="padding:0 8px;color:var(--mu)">−</span><span style="padding:0 10px;border-left:1px solid var(--b);border-right:1px solid var(--b)">60%</span><span style="padding:0 8px;color:var(--mu)">+</span></span>`), finishedRow(), inputRow()])])) },
    { key: 'E', name: 'No setting: the system’s volume', from: 'Zed, t3code, herdr',
      desc: 'As today. You turn the computer down, or pick Never.',
      good: 'Nothing to build.', cost: 'Not what you asked for: agentZ can’t be quieter than music.',
      mock: () => piece(content([setSection('Sounds', [finishedRow(), inputRow()])])) },
  ],
});

// 4. Where the volume sits and how you hear it ---------------------------------------------
TOPICS.push({
  id: 'volume-place', section: 'Volume', title: 'Where the volume goes, and hearing it', size: 'wide', rec: 'A',
  now: 'Picking When hidden or Always plays that sound once. Nothing else plays a sound in Settings. The mocks show the slider (topic 3’s A); the same goes for any control picked there.',
  options: [
    { key: 'A', name: 'First in Sounds, plays as you let go', from: 'macOS Alert volume',
      desc: 'The Volume row heads the Sounds section, since it applies to both rows under it. Letting go of the slider plays the finished sound at the new level, as macOS does.',
      good: 'You hear the level you chose, and you see first that the sounds can be quieter.', cost: 'None beyond the row.',
      mock: () => piece(content([setSection('Sounds', [volumeRow(), finishedRow(), inputRow()]), note('♪ the finished sound plays at 60%', 'top:26px;right:32px')])) },
    { key: 'B', name: 'Last in Sounds', from: 'new',
      desc: 'The same row after the two sound rows, plays as you let go.',
      good: 'The “when” rows, which you change more often, come first.', cost: 'Reads as if it belongs only to the input sound above it.',
      mock: () => piece(content([setSection('Sounds', [finishedRow(), inputRow(), volumeRow()])])) },
    { key: 'C', name: 'First, with a play button and no sound on move', from: 'new',
      desc: 'Moving the slider is silent; a ▶ beside it plays the finished sound at that level.',
      good: 'No sound while you drag.', cost: 'Another control, and a step to hear it.',
      mock: () => piece(content([setSection('Sounds', [setRow('Volume', 'How loud agentZ’s sounds play.', `<span class="row g3">${playAt(AUDIO.finished, 0.6, '')}${slider(0.6)}</span>`), finishedRow(), inputRow()])])) },
    { key: 'D', name: 'First, with a mute button', from: 'new',
      desc: 'Option A, and the speaker icon at the slider’s left mutes both sounds and remembers the level.',
      good: 'Quick quiet without losing your level.', cost: 'An extra: Never in both rows, or the slider at zero, already does it.',
      mock: () => piece(content([setSection('Sounds', [setRow('Volume', 'How loud agentZ’s sounds play. Click the speaker to mute.', `<span class="row g2" style="white-space:nowrap"><span class="ibtn">${ic('volume', 'sm')}</span>${slider(0.6, { ends: false })}</span>`), finishedRow(), inputRow()])])) },
  ],
});

// 5. The system notifications row ----------------------------------------------------------
const NOTIFY = ['Never', 'When away from the thread', 'When away from agentZ'];
TOPICS.push({
  id: 'notify', section: 'Notifications', title: 'When system notifications show', size: 'wide', rec: 'A',
  now: 'One switch, “Notify when agentZ isn’t focused”, on by default: a system notification as an agent finishes or needs input while another app is in front. With agentZ in front you get the sidebar’s state and the sound.',
  nowImg: 'img/now.png',
  options: [
    { key: 'A', name: 'Keep the switch', from: 't3code',
      desc: 'As today. It already means away from agentZ, the case you asked for with sounds.',
      good: 'Nothing changes; one row.', cost: 'No notification for another thread while you work in agentZ.',
      mock: () => piece(content([setSection('macOS notifications', [notifyRow()])], { heading: '' })) },
    { key: 'B', name: 'The sounds’ choices, without Always', from: 'new',
      desc: 'A dropdown: Never, When away from the thread, When away from agentZ (the default). No Always: a notification for the thread you’re looking at says nothing new.',
      good: 'The same words as the sounds, and the older rule (any thread not on screen) comes back as a choice.', cost: 'A dropdown where a switch did.',
      mock: () => piece(content([setSection('macOS notifications', [setRow('Notifications', 'When to show a macOS notification as an agent finishes or needs input.', dd('When away from agentZ', true), wideMenu(NOTIFY, 'When away from agentZ'))])], { heading: '', pad: 60 })) },
    { key: 'C', name: 'A dropdown for each event', from: 'new',
      desc: 'Notify when finished and notify when input is needed, each with option B’s three choices, like the sounds.',
      good: 'Matches the Sounds section row for row.', cost: 'Two rows where one usually does.',
      mock: () => piece(content([setSection('macOS notifications', [setRow('Notify when finished', 'When to show a macOS notification as an agent finishes its turn.', dd('When away from agentZ')), setRow('Notify when input is needed', 'When to show a macOS notification as an agent asks for a permission or an answer.', dd('When away from agentZ'))])], { heading: '' })) },
  ],
});

// 6. The section's title ------------------------------------------------------------------
const titled = (title, desc) => piece(content([setSection('Sounds', [finishedRow(), inputRow()]), setSection(title, [notifyRow('Notify when agentZ isn’t focused', desc)])]));
TOPICS.push({
  id: 'title', section: 'Notifications', title: 'The notifications section’s name', size: 'wide', rec: 'C',
  now: 'The section is called “macOS notifications”, and the switch says “Shows a macOS notification…”, on Linux too, where the desktop shows them.',
  nowImg: 'img/now.png',
  options: [
    { key: 'A', name: 'System notifications', from: 'new',
      desc: 'One name on both systems; the description says “a system notification”.',
      good: 'Right everywhere, one string.', cost: 'Less familiar than the word each system uses.',
      mock: () => titled('System notifications', 'Shows a system notification when an agent finishes or needs input while another app is in front.') },
    { key: 'B', name: 'Desktop notifications', from: 'Linux desktops',
      desc: 'The name Linux desktops use (the freedesktop spec), on both systems.',
      good: 'Right on Linux, and plain on macOS.', cost: 'macOS doesn’t call them that.',
      mock: () => titled('Desktop notifications', 'Shows a desktop notification when an agent finishes or needs input while another app is in front.') },
    { key: 'C', name: 'Each system’s own word', from: 'macOS, Linux desktops',
      desc: '“macOS notifications” on a Mac, as now, and “Desktop notifications” on Linux, in the title and the description. The mock shows Linux.',
      good: 'Each user reads the word their system uses.', cost: 'Two strings to keep in step.',
      mock: () => titled('Desktop notifications', 'Shows a desktop notification when an agent finishes or needs input while another app is in front.') },
    { key: 'D', name: 'Just Notifications', from: 'new',
      desc: 'Just “Notifications”, under the page of the same name.',
      good: 'The shortest.', cost: 'The page title twice, and “Notifications” doesn’t say these come from the system.',
      mock: () => titled('Notifications', 'Shows a notification when an agent finishes or needs input while another app is in front.') },
  ],
});
