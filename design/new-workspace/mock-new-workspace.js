// New worktrees and pastures for a new thread: the new thread screen, its checkout menu, a
// branch list to start from, and the Workspaces view's New Worktree dialog.

const chev = ic('chev-down', 'xs');
const toggle = (on) => `<span style="width:26px;height:15px;border-radius:8px;background:${on ? 'var(--ac)' : 'var(--sel)'};display:inline-flex;align-items:center;padding:2px;justify-content:${on ? 'flex-end' : 'flex-start'}"><i style="width:11px;height:11px;border-radius:50%;background:${on ? '#1b1f26' : 'var(--mu)'};display:block"></i></span>`;

// storefront's branches, as `git for-each-ref` would list them, most recently committed first.
const BRANCHES = [
  { name: 'main', badge: 'default', current: true },
  { name: 'checkout-flow', badge: 'worktree' },
  { name: 'fix-login' },
  { name: 'redesign-cart' },
  { name: 'agentz/3f671a7e', badge: 'worktree' },
];
const REMOTES = [
  { name: 'origin/main', badge: 'remote' },
  { name: 'origin/payments', badge: 'remote' },
  { name: 'origin/fix-login', badge: 'remote' },
];

/** The new thread screen (agent_view.rs render_new_thread): headline, composer and its strip. */
function newThread({ left = chipLocal(), right = branchText('main'), overlay = '', h = 360, above = '' } = {}) {
  return frame(`<div class="col" style="height:100%;align-items:center;padding-top:44px;gap:20px">
    <div style="font-size:22px">What should we work on?</div>
    <div class="col" style="width:600px;gap:8px;position:relative">
      ${above}
      <div class="card" style="padding:10px 12px 8px">
        <div class="ph" style="height:44px">Message the agent…</div>
        <div class="row" style="gap:12px;font-size:13px;color:var(--mu)">${ic('plus', 'sm')}<span class="row g15" style="color:var(--t)">${glyph('claude')}Claude Agent${chev}</span><span class="grow"></span><span class="row g1">Default ${chev}</span><span class="row g1">Sonnet ${chev}</span><span class="row g1">Medium ${chev}</span><span class="mu" style="display:inline-flex">${ic('arrow', 'sm')}</span></div>
      </div>
      <div class="row sm mu" style="justify-content:space-between;padding:0 4px;gap:8px"><span class="row g2">${left}</span><span class="row g1" style="min-width:0">${right}</span></div>
      ${overlay}
    </div></div>`, { w: 720, h });
}
const chip = (icon, label, { on = false, chevron = true, style = '' } = {}) => `<span class="row g1" style="height:22px;padding:0 6px;border-radius:5px;${on ? 'background:var(--sel);color:var(--t);' : ''}${style}">${ic(icon, 'xs')}${label}${chevron ? chev : ''}</span>`;
const chipLocal = (on = false) => chip('folder', 'Local', { on });
const chipWorktree = (on = false) => chip('worktree', 'New worktree', { on });
const chipPasture = (on = false) => chip('pasture', 'New pasture', { on });
const branchText = (name, { muted = true } = {}) => `<span class="row g1 trunc" style="${muted ? '' : 'color:var(--t)'}">${ic('branch', 'xs')}${name}</span>`;
const fromChip = (base, { on = false, origin = false } = {}) => chip('branch', `From ${origin ? 'origin/' : ''}${base}`, { on });

/** Today's checkout menu (render_checkout_picker), with optional rows after New pasture. */
function checkoutMenu({ style = 'top:150px;left:0', worktree = '', existing = true, hl = '' } = {}) {
  return `<div class="menu" style="${style};min-width:230px">
    <div class="lbl">Where It Works</div>
    <div class="it ${hl === 'local' ? 'hl' : ''}">${ic('folder', 'sm')}<span class="grow">Local checkout</span></div>
    <div class="it ${hl === 'worktree' ? 'hl' : ''}">${ic('worktree', 'sm')}<span class="grow">New worktree</span>${worktree ? ic('chev-right', 'xs') : ''}</div>
    <div class="it ${hl === 'pasture' ? 'hl' : ''}">${ic('pasture', 'sm')}<span class="grow">New pasture</span></div>
    ${existing ? `<div class="hr"></div><div class="lbl">Existing</div><div class="it">${ic('worktree', 'sm')}<span class="grow">agentz/3f671a7e</span></div><div class="it">${ic('worktree', 'sm')}<span class="grow">checkout-flow</span></div>` : ''}
  </div>${worktree}`;
}

const badge = (text) => (text ? `<span class="xs" style="color:var(--faint);padding-left:10px">${text}</span>` : '');
/** One branch in a list: its name, a badge and what clicking it does. */
const refRow = ({ name, badge: tag, current }, { hl = false, check = false, action = '', dim = false } = {}) => `<div class="it ${hl ? 'hl' : ''}" style="${dim ? 'color:var(--ph)' : ''}">${ic('branch', 'xs')}<span class="grow trunc">${name}</span>${action}${badge(current ? 'current' : tag)}${check ? ic('check', 'sm ac') : '<span style="width:14px"></span>'}</div>`;
/** t3code's branch picker: a search field, the refs and an optional foot. */
function refList({ style = 'top:150px;right:0', query = '', rows = BRANCHES.map((ref, index) => refRow(ref, { check: index === 0 })).join(''), foot = '', width = 300, head = '' } = {}) {
  return `<div class="menu" style="${style};width:${width}px;padding:0">
    ${head}
    <div class="row g2" style="height:32px;padding:0 10px;border-bottom:1px solid var(--bv);color:var(--ph)">${ic('search', 'xs')}<span class="grow" style="${query ? 'color:var(--t)' : ''}">${query || 'Search branches…'}</span></div>
    <div style="padding:4px">${rows}</div>
    ${foot}
  </div>`;
}
const originFoot = (on) => `<div class="row g2 sm" style="height:32px;padding:0 10px;border-top:1px solid var(--bv);color:var(--mu)">${ic('restart', 'xs')}<span class="grow">Start from origin</span>${toggle(on)}</div>`;
const hintFoot = (text) => `<div class="sm" style="padding:6px 10px;border-top:1px solid var(--bv);color:var(--ph);white-space:normal">${text}</div>`;

/** The Workspaces view's New Worktree dialog (worktree_modal.rs), as in its screenshot. */
function worktreeDialog({ name = 'agentz/green-valley-86fb', from = 'main', where = 'agentz-green-valley-86fb', rows = '', error = '', note = '', h = 250, overlay = '', title = 'New worktree of storefront' } = {}) {
  return frame(`<div class="pop" style="left:20px;top:16px;width:580px;font-size:14px">
    <div class="row g2" style="height:44px;padding:0 14px;border-bottom:1px solid var(--bv)">${mono('ST')}<span class="mu">${title}</span><span class="field focus" style="height:26px;padding:0 6px">${name}</span></div>
    <div class="row g2 sm" style="height:34px;padding:0 14px;border-bottom:1px solid var(--bv);color:var(--mu)">From <span class="row g1" style="color:var(--ac)">${from}${chev}</span><span class="trunc">in /tmp/az.YoZd/worktrees/storefront/${where}</span></div>
    <div class="col" style="padding:6px;gap:2px">${rows || `<div class="it hl" style="height:30px;border-radius:5px;display:flex;align-items:center;gap:8px;padding:0 8px;background:var(--hov)">${ic('worktree', 'sm')}Worktree <span class="sm ph">A git worktree: tracked files only</span></div><div style="height:30px;display:flex;align-items:center;gap:8px;padding:0 8px">${ic('pasture', 'sm')}Pasture <span class="sm warnc">A full copy here: slow, and as big as the project</span></div>`}</div>
    ${error ? `<div class="sm delc" style="padding:0 14px 8px">${error}</div>` : ''}
    ${note ? `<div class="sm" style="padding:0 14px 8px;color:var(--mu)">${note}</div>` : ''}
    <div class="sm ph" style="padding:8px 14px;border-top:1px solid var(--bv)">Enter makes it and opens a terminal there</div>
    ${overlay}
  </div>`, { w: 620, h, style: 'background:var(--ed)' });
}

/** A conversation's first lines, for what happens once the message is sent. */
const userBubble = (text) => `<div style="align-self:flex-end;max-width:70%;background:var(--panel);border:1px solid var(--bv);border-radius:8px;padding:8px 12px;font-size:13px">${text}</div>`;
const stepRow = (state, text, detail = '') => `<div class="row g2 sm" style="height:22px;color:${state === 'done' ? 'var(--mu)' : state === 'run' ? 'var(--t)' : 'var(--ph)'}">${state === 'done' ? ic('check', 'xs okc') : state === 'run' ? '<span class="spin"></span>' : `<span style="width:12px;height:12px;border-radius:50%;border:1px solid var(--ph);display:inline-block"></span>`}<span>${text}</span><span class="ph">${detail}</span></div>`;
function threadStart(inner, { h = 300 } = {}) {
  return frame(`<div class="col" style="height:100%;padding:18px 40px;gap:12px">${inner}</div>`, { w: 720, h });
}
