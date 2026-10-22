// A close button for terminals in the Agents view: terminal threads (the Shells shelf), not the
// terminals in a thread's drawer. terminal_thread_view.rs render_toolbar, sidebar.rs
// render_slim_row.

const closeTip = rTip(`Close Terminal <span class="kbd">Ctrl-Shift-W</span>`, 'top:40px;right:8px');
const shellHeader = ({ close = false, hover = false } = {}) => `<div class="row g2" style="height:36px;flex:none;padding:0 8px 0 12px;border-bottom:1px solid var(--b);background:var(--panel);font-size:12px">${ic('terminal', 'sm mu')}<span style="font-size:13px">npm run dev</span><span class="mu trunc">~/projects/agentZ</span><span class="grow"></span><span class="btn ghost sm" style="color:var(--mu)">${ic('rotate-cw', 'xs')}Restart</span><span class="ibtn" style="font-size:15px">±</span>${close ? `<span class="ibtn ${hover ? 'hov' : ''}">${ic('x', 'sm')}</span>` : ''}</div>`;
const shellView = ({ close = false, hover = false, list = rCard({ title: R_PARENT, state: 'working' }) + rOthers(), shells = rShells(), overlay = '' } = {}) => frame(`<div class="body" style="height:100%">${sidebar({ list: `${list}<div class="grow"></div>${shells}` })}<div class="col grow" style="height:100%;position:relative">${shellHeader({ close, hover })}${term(SCREENS.dev.map((line) => line.replace(/storefront/g, 'agentZ').replace('checkout-flow', 'main')))}${overlay}</div></div>`, { w: 980, h: 520 });
const confirm = `<div class="modal-back"></div><div class="pop" style="z-index:21;top:150px;left:50%;transform:translateX(-50%);width:340px;padding:16px 18px;display:flex;flex-direction:column;gap:8px"><span class="row g2 b5">${ic('warning', 'sm warnc')}Close “npm run dev”?</span><span class="sm mu">It’s still running, and closing the terminal ends it.</span><span class="row g2" style="justify-content:flex-end;margin-top:6px"><span class="btn">Cancel</span><span class="btn primary">Close</span></span></div>`;

TOPICS.push({
  id: 'close', section: 'Shells', title: 'A close button for terminals in the Agents view', size: 'wide', rec: 'A',
  now: 'A terminal opened from the Agents view (New Thread › Terminal) is listed under Shells in the sidebar and fills the main area. Its header has the icon, its title, the folder, Restart and the changes button, but nothing to close it. It closes only with Ctrl-Shift-W (Cmd-W on macOS), which asks first while a program runs, or with Delete… in its row’s right-click menu. The terminals in a thread’s drawer already have ×, and stay as they are.',
  nowImg: 'img/now-shell.png',
  options: [
    { key: 'A', name: 'In the terminal’s header', from: 'Zed and Workspaces pane headers',
      desc: 'A × at the right end of the terminal’s header, after Restart and the changes button, with the tooltip “Close Terminal” and its shortcut. It does what Ctrl-Shift-W does: closes at once at a prompt, and asks first while a program runs.',
      good: 'Where every pane’s close button is, in Zed and in Workspaces.', cost: 'You have to open the terminal to close it.',
      mock: () => `<div style="display:flex;flex-direction:column;gap:10px">${shellView({ close: true, hover: true, overlay: closeTip, shells: rShells() })}${shellView({ close: true, overlay: confirm })}</div>` },
    { key: 'B', name: 'On its row in the sidebar', from: 'agentZ draft rows (Discard on hover)',
      desc: 'Hovering a terminal’s row under Shells shows × in place of its time or “Running”, as a draft’s row shows Discard. A click closes it, asking first while a program runs.',
      good: 'Closes any terminal without opening it.', cost: 'Nothing in the terminal itself.',
      mock: () => shellView({ shells: rShells({ hover: 0, close: true }), overlay: '' }) },
    { key: 'C', name: 'Both', from: 'A and B',
      desc: 'The × in the header and on the sidebar row.',
      good: 'Close it from wherever you are.', cost: 'Two buttons for one thing.',
      mock: () => shellView({ close: true, shells: rShells({ hover: 0, close: true }) }) },
  ],
});
