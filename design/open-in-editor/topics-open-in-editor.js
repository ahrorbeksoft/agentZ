// Open in Editor: a button, as t3code has, that opens a thread's folder in an editor.

const oeWideHeader = (options = {}) => oeHeader({ w: OE_W, ...options });
const oeNarrowHeader = (options = {}) => oeHeader({ w: OE_NARROW, ...options });
// Where the menu hangs under a split button before Changes, Terminal and ⋯.
const OE_MENU_AT = 'right:150px;top:38px';

// What the pictures of Today show: the header, wide and narrow.
const oeToday = () => oePair(
  `<div>${oeScene({ header: oeWideHeader(), h: 170 })}${oeCaption('Wide (700 px): project / title, branch, changes, Terminal, ⋯')}</div>`,
  `<div>${oeScene({ w: OE_NARROW, header: oeNarrowHeader(), h: 170 })}${oeCaption('Narrow (340 px): only ⋯')}</div>`,
);
const oeTodayMenu = () => `<div>${oeScene({ header: oeWideHeader({ more: oeMore(true) }), h: 230, overlays: oeMenu(oeAgentItems(), 'right:8px;top:38px;min-width:200px') })}${oeCaption('The only menu in the header is “⋯”: the agent’s options')}</div>`;
const oeTodayRemote = () => `<div>${oeScene({ header: oeHeader({ project: 'ap', title: 'Fix flaky login test', branch: 'main' }), h: 150, body: oeBody(['Why does the login test fail on CI?', 'It races the rate limiter: I’ll make the test wait for the window.']) })}${oeCaption('A thread of api, whose folder is on Devbox 1: the same header, no way to open the folder')}</div>`;
const oeTodayDiff = () => `<div>${oeDiffPanel({ h: 190, header: oeFileHead() })}${oeCaption('A changed file’s header in the Changes panel: Viewed is its only action')}</div>`;

// 1. The button and where it goes -----------------------------------------------------------
TOPICS.push({
  id: 'button', section: 'Thread header', title: 'The button and where it goes', size: 'wide', rec: 'A',
  now: 'The open thread’s header (<code>render_toolbar</code> in agent_view.rs) has the project crumb, the title with its menu, a fork’s source pill, the artifacts pill, the branch, the changes button (+added −removed), Terminal and “⋯” (the agent’s options). Under 480 px the branch, artifacts, changes and Terminal move into “⋯”. Nothing opens the thread’s folder (the project’s checkout, a worktree or a pasture) in an editor. The pictures are recreated from the code, not captured from the running app.',
  nowImg: 'img/now-button.png',
  issues: [
    'To edit a file by hand you leave agentZ, find the folder in Finder or a terminal, and open it yourself.',
    'The folder of a worktree or pasture has a generated name (<code>agentz/brave-otter-3fa</code>), so it is hard to find outside the app.',
    'The header is already full at 560 px, so wherever a new control goes decides what it pushes into “⋯”.',
  ],
  options: [
    {
      key: 'A', name: 'Split button before Changes', from: 't3code',
      desc: 't3code’s OpenInPicker: an outlined split button before the changes button. The left part shows the last-used editor’s icon and “Open” and opens the folder there with one click; the chevron lists the installed editors. Narrow, “Open in VS Code ⌘⇧O” and “Open in…” (a submenu of the editors) head the compact “⋯”, as t3code’s compact menu does. The key is ⌘⇧O here: t3code’s ⌘O is Open Folder in agentZ.',
      good: 'One click for the usual case, and the menu for the rest. Nothing else in the header moves.',
      cost: 'About 70 px more in a header the branch already gives way in; “Open” drops its word under 560 px (t3code hides it below its @3xl width) so the button stays an icon and a chevron.',
      mock: () => oePair(
        `<div>${oeScene({
          header: oeWideHeader({ before: oeSplit({ chevron: true }) }),
          overlays: oeMenu([oeEditorItems(OE_DETECTED), oeHr, oeItem({ icon: edIcon('files'), label: 'Finder' })].join(''), OE_MENU_AT),
          h: 270,
        })}${oeCaption('Wide')}</div>`,
        `<div>${oeScene({
          w: OE_NARROW, header: oeNarrowHeader({ more: oeMore(true) }),
          overlays: oeMenu(oeCompactItems([oeHr, oeItem({ icon: edIcon('vscode'), label: 'Open in VS Code', key: OE_KEY }), oeItem({ icon: ic('external', 'sm'), label: 'Open in…', arrow: true })].join('')), 'right:8px;top:38px;min-width:230px'),
          h: 270,
        })}${oeCaption('Narrow: in “⋯”')}</div>`,
      ),
    },
    {
      key: 'B', name: 'One icon that opens the menu', from: 'new',
      desc: 'A single icon button in the header’s own style (no outline, no word) showing the last-used editor’s icon. A click opens the menu of editors, with the last used first and the key beside it. Narrow, the same two entries as A in “⋯”.',
      good: 'About 24 px: the smallest header cost, and it looks like Changes and Terminal.',
      cost: 'Opening is two clicks (or the key), where A’s is one. The icon alone doesn’t say it opens something.',
      mock: () => oePair(
        `<div>${oeScene({
          header: oeWideHeader({ before: oeIconButton('vscode', true) }),
          overlays: oeMenu([oeEditorItems(['vscode', 'cursor', 'zed', 'webstorm']), oeHr, oeItem({ icon: edIcon('files'), label: 'Finder' })].join(''), 'right:122px;top:38px'),
          h: 270,
        })}${oeCaption('Wide')}</div>`,
        `<div>${oeScene({
          w: OE_NARROW, header: oeNarrowHeader({ more: oeMore(true) }),
          overlays: oeMenu(oeCompactItems([oeHr, oeItem({ icon: edIcon('vscode'), label: 'Open in VS Code', key: OE_KEY }), oeItem({ icon: ic('external', 'sm'), label: 'Open in…', arrow: true })].join('')), 'right:8px;top:38px;min-width:230px'),
          h: 270,
        })}${oeCaption('Narrow: in “⋯”')}</div>`,
      ),
    },
    {
      key: 'C', name: 'Only in “⋯”', from: 't3code',
      desc: 'No new header button. “⋯” gets “Open in VS Code ⌘⇧O” and “Open in…” (a submenu of the editors) at its top, above the agent’s options, at every width. t3code does the same in its compact controls menu (OpenInPicker’s “menu” presentation).',
      good: 'The header stays as it is, and narrow and wide are the same. Nothing is pushed out.',
      cost: 'Two clicks and a menu to find it, and “⋯” says “Agent Options” today, so the tooltip would become “More Options” at every width.',
      mock: () => oePair(
        `<div>${oeScene({
          header: oeWideHeader({ more: oeMore(true) }),
          overlays: oeMenu(oeAgentItems([oeItem({ icon: edIcon('vscode'), label: 'Open in VS Code', key: OE_KEY }), oeItem({ icon: ic('external', 'sm'), label: 'Open in…', arrow: true })].join('')), 'right:8px;top:38px;min-width:230px'),
          h: 270,
        })}${oeCaption('Wide')}</div>`,
        `<div>${oeScene({
          w: OE_NARROW, header: oeNarrowHeader({ more: oeMore(true) }),
          overlays: oeMenu(oeCompactItems([oeHr, oeItem({ icon: edIcon('vscode'), label: 'Open in VS Code', key: OE_KEY }), oeItem({ icon: ic('external', 'sm'), label: 'Open in…', arrow: true })].join('')), 'right:8px;top:38px;min-width:230px'),
          h: 270,
        })}${oeCaption('Narrow')}</div>`,
      ),
    },
    {
      key: 'D', name: 'In the thread’s menus', from: 'new',
      desc: 'No header button. The title’s menu (Pin, Rename, Regenerate Title, Archive, Delete…) and the thread card’s right-click menu in the sidebar, which has the same entries, get “Open in VS Code ⌘⇧O” and “Open in…”. Works from the sidebar without opening the thread.',
      good: 'Both places already hold the things you do to a thread, the header stays as it is, and a card can be opened in an editor without selecting it.',
      cost: 'Hidden in a menu you open for something else, and the entry has to be added to two menus (and kept the same).',
      mock: () => oePair(
        `<div>${oeScene({
          header: oeWideHeader({ titleOn: true }),
          overlays: oeMenu([oeItem({ label: 'Pin' }), oeItem({ label: 'Rename' }), oeItem({ label: 'Regenerate Title' }), oeHr, oeItem({ icon: edIcon('vscode'), label: 'Open in VS Code', key: OE_KEY }), oeItem({ icon: ic('external', 'sm'), label: 'Open in…', arrow: true }), oeHr, oeItem({ label: 'Archive' }), oeItem({ label: 'Delete…', danger: true })].join(''), 'left:62px;top:38px;min-width:230px'),
          h: 290,
        })}${oeCaption('The title’s menu')}</div>`,
        `<div>${frame(`<div style="padding:4px 0">${card(THREADS_OE[0], { on: true })}${card(THREADS_OE[1])}${card(THREADS_OE[2])}</div>${oeMenu([oeItem({ label: 'Pin' }), oeItem({ label: 'Rename' }), oeHr, oeItem({ icon: edIcon('vscode'), label: 'Open in VS Code', key: OE_KEY }), oeItem({ icon: ic('external', 'sm'), label: 'Open in…', arrow: true }), oeHr, oeItem({ label: 'Archive' }), oeItem({ label: 'Delete…', danger: true })].join(''), 'left:100px;top:100px;min-width:220px')}`, { w: 320, h: 290, style: 'background:var(--panel);border:1px solid var(--b);border-radius:8px' })}${oeCaption('The card’s right-click menu')}</div>`,
      ),
    },
    {
      key: 'E', name: 'Shortcut and menu bar only', from: 'new',
      desc: '⌘⇧O opens the thread’s folder in the last-used editor (or the first one found), with the same entry, “Open in Editor”, in the File menu and the command palette (⌘⇧P). Nothing in the header or in the thread’s menus, and the editor is chosen in Settings.',
      good: 'No UI in the thread at all, and it works from the keyboard.',
      cost: 'Not discoverable, and a mouse user can’t choose an editor without Settings. It adds a settings page.',
      mock: () => oePair(
        `<div>${oeScene({ header: oeWideHeader(), h: 270, })}${oeCaption('The header does not change')}</div>`,
        `<div>${frame(`<div class="row g3" style="height:26px;padding:0 12px;background:var(--title);border-bottom:1px solid var(--b);font-size:13px"><span class="b6">agentZ</span><span style="background:var(--hov);padding:2px 8px;border-radius:4px">File</span><span class="mu">View</span></div>${oeMenu([oeItem({ label: 'New Thread…', key: '⌘N' }), oeItem({ label: 'Open Folder…', key: '⌘O' }), oeItem({ label: 'Switch Project…', key: '⌘⌥O' }), oeHr, oeItem({ label: 'Open in Editor', key: OE_KEY, hl: true })].join(''), 'left:60px;top:28px;min-width:230px')}`, { w: 320, h: 200, style: 'border:1px solid var(--b);border-radius:8px' })}${oeCaption('File menu, and the palette lists the same')}</div>`,
      ),
    },
  ],
});

// Threads of the demo world, for the card.
const THREADS_OE = [
  { project: 'st', title: 'Add the checkout page', branch: 'checkout-flow', agent: 'claude', state: 'working', time: 'now' },
  { project: 'st', title: 'Write tests for the rate limiter', branch: 'main', agent: 'codex', state: 'pending', time: '2m' },
  { project: 'ap', title: 'Fix flaky login test', branch: 'main', agent: 'claude', state: 'awaiting', time: '5m' },
];
const card = (thread, { on = false } = {}) => {
  const project = PROJECTS[thread.project];
  return `<div style="margin:0 4px;padding:8px 10px;border-radius:8px;${on ? 'background:var(--sel)' : ''}">
    <div class="row g2" style="height:20px">${mono(project.mono, project.color)}<span class="grow trunc sm mu">${project.name}</span>${pill(thread.state)}</div>
    <div class="trunc" style="margin-top:4px">${thread.title}</div>
    <div class="row g2 sm faint" style="margin-top:4px"><span class="grow">${thread.branch}</span>${ic(project.machine, 'xs')}${glyph(thread.agent, 'sm')}</div></div>`;
};

// 2. What the menu lists --------------------------------------------------------------------
const oeMenuScene = (menu, { w = 330, h = 290, button = oeSplit({ chevron: true }), caption = '' } = {}) =>
  `<div>${frame(`<div class="row" style="height:36px;padding:0 8px;border-bottom:1px solid var(--b);justify-content:flex-end">${button}</div>${menu}`, { w, h, style: 'border:1px solid var(--b);border-radius:8px' })}${caption ? oeCaption(caption) : ''}</div>`;
const OE_MENU_STYLE = 'right:8px;top:38px;min-width:230px';

TOPICS.push({
  id: 'menu', section: 'Thread header', title: 'What the menu lists', size: 'medium', rec: 'B',
  now: 'There is no menu. agentZ lists no editors, and the server on a machine can’t say which are installed.',
  nowImg: 'img/now-menu.png',
  issues: [
    'Nothing finds out which editors a machine has, so there is nothing to list.',
    'A folder can’t be shown in Finder (or Files on Linux) from a thread either.',
    'A path can be copied from a terminal only, so pasting it elsewhere means finding it first.',
  ],
  options: [
    {
      key: 'A', name: 'Installed editors and the file manager', from: 't3code',
      desc: 'Exactly t3code’s list: only editors found on the machine, in t3code’s fixed order (Cursor, Trae, Kiro, VS Code, VS Code Insiders, VSCodium, Zed, Antigravity, the JetBrains IDEs), then the file manager, which is Finder on a Mac and Files on Linux. The key shows beside the last-used one only. With none found: “No installed editors found”, disabled, and the button disabled. The machine’s server finds them by their commands on its PATH (<code>code</code>, <code>cursor</code>, <code>zed</code>, …) and asks again after 5 seconds without an answer.',
      good: 'The same as t3code, nothing to learn, and nothing listed that wouldn’t work.',
      cost: 'The last-used editor isn’t first, so the list reorders nothing and you scan for it. An editor whose command isn’t on PATH (an app dragged into Applications without “Install shell command”) is invisible, with no hint why.',
      mock: () => oePair(
        oeMenuScene(oeMenu([oeEditorItems(OE_DETECTED), oeHr, oeItem({ icon: edIcon('files'), label: 'Finder' })].join(''), OE_MENU_STYLE), { caption: 'VS Code, last used' }),
        oeMenuScene(oeMenu([oeItem({ label: 'No installed editors found', dis: true }), oeHr, oeItem({ icon: edIcon('files'), label: 'Finder' })].join(''), OE_MENU_STYLE), { button: oeSplit({ editor: null, dis: true, chevron: false }), caption: 'None found: the button is disabled' }),
      ),
    },
    {
      key: 'B', name: 'Last used first, Finder and Copy Path', from: 't3code',
      desc: 'A, changed in three ways (t3code does none of them): the last-used editor on top with the key, in its own group; the rest after it; then “Show in Finder” (“Show in Files” on Linux) and Copy Path below a line. With no editor found the menu opens anyway: “No editor found” (disabled) over Show in Finder and Copy Path, and the button shows the plain “Open” with an outward arrow, which opens the menu.',
      good: 'The one you use is always at the top. The menu is never empty, since the folder can still be shown or copied.',
      cost: 'Three groups in a menu of six rows, and the order changes as you use it. Copy Path and Show in Finder are extras t3code’s picker doesn’t have in this menu.',
      mock: () => oePair(
        oeMenuScene(oeMenu([oeItem({ icon: edIcon('vscode'), label: 'VS Code', key: OE_KEY }), oeHr, oeEditorItems(['cursor', 'zed', 'webstorm'], { key: false }), oeHr, oeItem({ icon: ic('folder-closed', 'sm'), label: 'Show in Finder' }), oeItem({ icon: ic('copy', 'sm'), label: 'Copy Path' })].join(''), OE_MENU_STYLE), { h: 300, caption: 'Editors found' }),
        oeMenuScene(oeMenu([oeItem({ label: 'No editor found', dis: true }), oeHr, oeItem({ icon: ic('folder-closed', 'sm'), label: 'Show in Finder' }), oeItem({ icon: ic('copy', 'sm'), label: 'Copy Path' })].join(''), OE_MENU_STYLE), { h: 300, button: oeSplit({ editor: null, chevron: true }), caption: 'None found' }),
      ),
    },
    {
      key: 'C', name: 'A fixed list, absent ones greyed', from: 'new',
      desc: 'Every editor agentZ knows (Cursor, VS Code, VS Code Insiders, VSCodium, Zed, WebStorm, IntelliJ IDEA) in a fixed order, each greyed with “Not installed” when the machine hasn’t it. Then Finder. The last used keeps the key.',
      good: 'Shows what is supported, and a missing editor explains itself instead of vanishing.',
      cost: 'Mostly grey rows, and a long list on a machine with two editors. t3code and Zed’s own pickers hide what isn’t installed.',
      mock: () => oePair(
        oeMenuScene(oeMenu([
          oeItem({ icon: edIcon('cursor'), label: 'Cursor' }),
          oeItem({ icon: edIcon('vscode'), label: 'VS Code', key: OE_KEY }),
          oeItem({ icon: edIcon('vscode', { dim: true }), label: 'VS Code Insiders', dis: true, sub: 'Not installed' }),
          oeItem({ icon: edIcon('vscodium', { dim: true }), label: 'VSCodium', dis: true, sub: 'Not installed' }),
          oeItem({ icon: edIcon('zed'), label: 'Zed' }),
          oeItem({ icon: edIcon('webstorm'), label: 'WebStorm' }),
          oeItem({ icon: edIcon('idea', { dim: true }), label: 'IntelliJ IDEA', dis: true, sub: 'Not installed' }),
          oeHr, oeItem({ icon: edIcon('files'), label: 'Finder' })].join(''), OE_MENU_STYLE), { h: 330, caption: 'Some found' }),
        oeMenuScene(oeMenu([
          oeItem({ icon: edIcon('cursor', { dim: true }), label: 'Cursor', dis: true, sub: 'Not installed' }),
          oeItem({ icon: edIcon('vscode', { dim: true }), label: 'VS Code', dis: true, sub: 'Not installed' }),
          oeItem({ icon: edIcon('zed', { dim: true }), label: 'Zed', dis: true, sub: 'Not installed' }),
          oeItem({ icon: edIcon('webstorm', { dim: true }), label: 'WebStorm', dis: true, sub: 'Not installed' }),
          oeHr, oeItem({ icon: edIcon('files'), label: 'Finder' })].join(''), OE_MENU_STYLE), { h: 330, button: oeSplit({ editor: null, dis: true, chevron: true }), caption: 'None found' }),
      ),
    },
    {
      key: 'D', name: 'Installed editors only', from: 'Zed',
      desc: 'Only the editors found, last used first, with the key, and nothing else: no Finder, no Copy Path. With none found the button isn’t drawn at all (Zed hides actions that can’t run).',
      good: 'The shortest menu, and the least code: no file manager and no path to copy.',
      cost: 'On a machine with no editor the header has no sign the button exists, and a folder can never be shown in Finder from agentZ.',
      mock: () => oePair(
        oeMenuScene(oeMenu(oeEditorItems(['vscode', 'cursor', 'zed', 'webstorm']), OE_MENU_STYLE), { h: 250, caption: 'Editors found' }),
        oeMenuScene('', { h: 250, button: '', caption: 'None found: nothing is drawn' }),
      ),
    },
  ],
});

// 3. A thread on another machine ------------------------------------------------------------
const OE_REMOTE = { project: 'ap', title: 'Fix flaky login test', branch: 'main' };
const oeRemoteBody = oeBody(['Why does the login test fail on CI?', 'It races the rate limiter: I’ll make the test wait for the window.']);
const oeRemote = (options, scene = {}) => `<div>${oeScene({ header: oeWideHeader({ ...OE_REMOTE, ...options }), body: oeRemoteBody, h: 270, ...scene })}${scene.caption ? oeCaption(scene.caption) : ''}</div>`;

TOPICS.push({
  id: 'remote', section: 'Thread header', title: 'A thread on another machine', size: 'medium', rec: 'C',
  now: 'A thread’s folder is on the machine of its project: this Mac, or an SSH machine (Devbox 1) whose server runs the agent. agentZ shows its conversation, changes and terminals everywhere, but nothing opens its folder on the machine that runs it. An editor installed on this Mac can’t read that folder except through its own SSH support: VS Code and Cursor’s Remote - SSH, Zed’s <code>ssh://</code> projects. The app knows the SSH host it dials for a machine (the one it was added with) and the folder’s absolute path.',
  nowImg: 'img/now-remote.png',
  issues: [
    'The header of a Devbox 1 thread is the same as a local one’s, so a new button would appear on both and has to say something different on one.',
    'The editors found on a machine are the ones installed there, which is the wrong machine: the editor that runs is on this Mac.',
    'Opening a path on a machine that isn’t this one needs the host name this Mac knows it by, which is in the machine’s settings, not in the thread.',
  ],
  options: [
    {
      key: 'A', name: 'Hidden', from: 'new',
      desc: 'The button (or menu entry) exists only for threads on this Mac. A thread on Devbox 1 has the header it has today.',
      good: 'Nothing to explain, and nothing that can fail over SSH.',
      cost: 'The button appears and disappears between threads, so the header jumps by about 70 px when you switch, and a Devbox 1 thread has no way out at all.',
      mock: () => oeRemote({}, { caption: 'Devbox 1 thread: no button' }),
    },
    {
      key: 'B', name: 'Shown, disabled, with a tooltip', from: 't3code',
      desc: 't3code disables the picker when it has no route to the machine (“No SSH route to Devbox 1” in its menu). Here the button stays, greyed, and its tooltip says “Its folder is on Devbox 1”. The menu opens but only has that same line.',
      good: 'The header is the same everywhere, and the reason is in the tooltip.',
      cost: 'A button that never works for every remote thread, and the answer to the next question (“how, then?”) isn’t given.',
      mock: () => oeRemote({ before: oeSplit({ dis: true, chevron: true }) }, {
        overlays: oeTip('Its folder is on Devbox 1', 'right:108px;top:64px'), caption: 'Hovering the button',
      }),
    },
    {
      key: 'C', name: 'Open over SSH, in the editors that can', from: 't3code',
      desc: 't3code’s “remote-links”: the menu lists only the editors on this Mac that open remote folders, VS Code, Cursor and VSCodium (<code>vscode://vscode-remote/ssh-remote+devbox1/home/ubuntu/api</code>) and Zed (<code>zed://ssh/devbox1/home/ubuntu/api</code>); the app hands the link to the OS, and the editor does the SSH. A first line says “Opens over SSH. Needs your key on Devbox 1” until one has worked. WebStorm and the file manager aren’t listed. A link the OS can’t open does nothing; the editor’s window reports its own failure.',
      good: 'The same button and menu on every thread, and it really opens the folder. The editor connects by itself.',
      cost: 'Needs the machine’s SSH host name as this Mac’s <code>~/.ssh/config</code> knows it (agentZ would use the host it dials), the editor’s remote extension (VS Code installs it on the first connect; Zed needs nothing), and the key without a prompt. If any is missing the failure is in the editor, not in agentZ.',
      mock: () => oeRemote({ before: oeSplit({ chevron: true }) }, {
        overlays: oeMenu([oeEditorItems(['vscode', 'cursor', 'zed']), oeHr, oeItem({ label: 'Opens over SSH. Needs your key on Devbox 1', dis: true })].join(''), OE_MENU_AT + ';min-width:290px'), caption: 'Devbox 1 thread',
      }),
    },
    {
      key: 'D', name: 'Over SSH, the rest listed disabled', from: 'new',
      desc: 'C, but the editors that can’t open a remote folder from a link (WebStorm, IntelliJ IDEA) and the file manager stay in the list, greyed, each with a short reason (“Not over SSH”, “Folder is on Devbox 1”).',
      good: 'Says why an editor you use isn’t offered, and the list is the same shape for local and remote threads.',
      cost: 'Greyed rows that can’t ever work on a remote thread. JetBrains’ remote needs the Toolbox app and its Gateway, which agentZ can’t check.',
      mock: () => oeRemote({ before: oeSplit({ chevron: true }) }, {
        overlays: oeMenu([oeEditorItems(['vscode', 'cursor', 'zed']), oeHr,
          oeItem({ icon: edIcon('webstorm', { dim: true }), label: 'WebStorm', dis: true, sub: 'Not over SSH' }),
          oeItem({ icon: edIcon('files'), label: 'Finder', dis: true, sub: 'Folder is on Devbox 1' })].join(''), OE_MENU_AT + ';min-width:290px'), caption: 'Devbox 1 thread',
      }),
    },
    {
      key: 'E', name: 'Only copy the path', from: 'new',
      desc: 'No editor is launched. The menu has Copy Path (<code>/home/ubuntu/api</code>) and Copy SSH Path (<code>devbox1:/home/ubuntu/api</code>, the form <code>scp</code> and Zed’s “Open Remote” take) for a remote thread, and the editors, as in B of the last topic, for a local one.',
      good: 'Never fails, and needs no editor, no extension and no host name beyond the one shown.',
      cost: 'You paste it into the editor yourself, which is most of the work this button should save.',
      mock: () => oeRemote({ before: oeSplit({ editor: null, chevron: true }) }, {
        overlays: oeMenu([oeItem({ icon: ic('copy', 'sm'), label: 'Copy Path', sub: '/home/ubuntu/api' }), oeItem({ icon: ic('copy', 'sm'), label: 'Copy SSH Path', sub: 'devbox1:/home/ubuntu/api' })].join(''), OE_MENU_AT + ';min-width:330px'), caption: 'Devbox 1 thread',
      }),
    },
  ],
});

// 4. A changed file ---------------------------------------------------------------------------
TOPICS.push({
  id: 'diff', section: 'Changes panel', title: 'Open a changed file from the Changes panel', size: 'medium', rec: 'B',
  now: 'The Changes panel (Cmd-D) lists the changed files as headers over their hunks, each with a Viewed button. Nothing opens a file in an editor. t3code’s diff panel opens a file in the editor, and its launch commands take a line (<code>code --goto path:line</code>, Zed’s <code>path:line</code>, JetBrains’ <code>--line</code>), which it uses for the file links in chat.',
  nowImg: 'img/now-diff.png',
  issues: [
    'To fix a line in a diff you read, you find the file in an editor and the line by hand.',
    'A file action beside Viewed has to work on a remote thread too (the last topic).',
    'The diff’s line numbers are the file’s, so they could carry the line to an editor.',
  ],
  options: [
    {
      key: 'A', name: 'Nothing', from: 'new',
      desc: 'The Changes panel stays as it is. Files are opened through the header’s button, the folder as a whole.',
      good: 'No new code in the panel, and nothing to disagree with t3code’s own flow.',
      cost: 'From a diff you can’t get to a file without finding it in the editor yourself.',
      mock: () => oeDiffPanel({ h: 190, header: oeFileHead() }),
    },
    {
      key: 'B', name: 'A button on the file header', from: 't3code',
      desc: 'The last-used editor’s icon beside Viewed on each file’s header, with the tooltip “Open in VS Code”, opening the file itself at its first change. It follows the same editor choice as the header’s button, and is hidden when there is none.',
      good: 'Visible, one click, and a file path is exactly what a header names.',
      cost: 'A second icon on every file header, and no line choice.',
      mock: () => oeDiffPanel({ h: 190, header: oeFileHead({ hl: true, end: `<span class="ibtn sm hov">${edIcon('vscode')}</span>` }), overlays: oeTip('Open in VS Code', 'right:72px;top:72px') }),
    },
    {
      key: 'C', name: 'An entry in the file’s right-click menu', from: 'new',
      desc: 'Right-clicking a file’s header opens a menu: “Open in VS Code”, “Open in…”, a line, then “Copy Path”.',
      good: 'No change to the header’s look.',
      cost: 'Hidden, and right-clicking a file header isn’t a thing in the app today, so it’s a new menu.',
      mock: () => oeDiffPanel({ h: 250, header: oeFileHead({ hl: true }), overlays: oeMenu([oeItem({ icon: edIcon('vscode'), label: 'Open in VS Code' }), oeItem({ icon: ic('external', 'sm'), label: 'Open in…', arrow: true }), oeHr, oeItem({ icon: ic('copy', 'sm'), label: 'Copy Path' })].join(''), 'left:80px;top:100px;min-width:200px') }),
    },
    {
      key: 'D', name: 'Clicking a line number', from: 't3code',
      desc: 'A line’s number in the gutter is a link (underlined on hover, tooltip “Open in VS Code at line 14”): it opens the file in the editor at that line, using each editor’s own line syntax. Removed lines’ numbers do nothing.',
      good: 'The only option that lands on the line you were reading.',
      cost: 'A hidden affordance, and the number was a label until now. It needs the launch syntax per editor (<code>--goto</code>, <code>path:line</code>, <code>--line</code>).',
      mock: () => oeDiffPanel({
        h: 210, header: oeFileHead(),
        body: `<div style="padding:4px 0">${oeDiffLine(12, ' ', 'export default function Checkout() {')}${oeDiffLine(13, '-', '  const total = cart.total')}<div class="row mono-font" style="height:19px;font-size:11.5px;background:rgba(161,193,129,.13);white-space:pre"><span style="width:44px;text-align:right;padding-right:10px;flex:none;color:var(--ac);text-decoration:underline">14</span><span style="width:12px;flex:none;color:var(--ok)">+</span><span>  const total = totals(cart)</span></div>${oeDiffLine(15, '+', '  const [paying, setPaying] = useState(false)')}${oeDiffLine(16, ' ', '  return (')}</div>`,
        overlays: oeTip('Open in VS Code at line 14', 'left:56px;top:136px'),
      }),
    },
  ],
});
