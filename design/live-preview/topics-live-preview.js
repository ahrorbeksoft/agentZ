// Resizing a live page on the design page (design.html): how, where, and what becomes of
// Desktop · Phone. The page and its close-up are drawn as design.html draws them.

const NOW_TEXT_CLOSEUP = 'A live page opens up close (Open page on a card): an interactive frame, 1280 px wide or the window less 128 px, and the window’s height less 64 px; with Phone chosen in the bar, 390 px wide. ← → step through the variants, Esc closes. The frame’s width can’t be changed: to see a phone layout the bar’s Desktop · Phone swaps in the variant’s separate phone page.';

// 1. How a live page is resized -------------------------------------------------------------------
TOPICS.push({
  id: 'how', section: 'Resizing', title: 'How a live page is resized', size: 'wide', rec: 'D',
  now: NOW_TEXT_CLOSEUP,
  nowImg: 'img/now-closeup.png',
  issues: [
    'The close-up has one width, set by the window: nothing lets you see the page at a tablet width, or any width but 1280 and 390.',
    'The phone layout is a second HTML file the agent has to write and keep in step with the first; a responsive page already has one.',
    'Narrowing the browser window to test a layout narrows the whole design page too (the cards’ grid reflows), so it’s no way to resize the page alone.',
  ],
  options: [
    { key: 'A', name: 'A drag handle on the frame’s edge', from: 'Chrome DevTools’ device mode (its right-edge handle)',
      desc: 'In the close-up, a grip on the frame’s right edge: drag it to narrow the page, anywhere from 320 px up to the window’s width, with the width and height beside it (“834 × 640”) while you drag. A dashed outline shows the full width. The frame stays centred; double-clicking the grip goes back to full width.',
      good: 'Nothing added to the screen; dragging is the quickest way to find the width where a layout breaks.', cost: 'No way to type a width or jump to a phone’s: you drag to roughly 390 and guess. The grip is easy to miss.',
      mock: () => lpBoard({ preview: lpDesktopThumb(), over: lpCloseUp({ w: 834, h: 640, handles: `<span class="lp-handle" style="right:-18px"></span><span class="lp-readout" style="position:absolute;right:-102px;top:50%;margin-top:-34px">834 × 640</span><span class="lp-ghost" style="left:-159px;width:1152px"></span>` }) }) },
    { key: 'B', name: 'A preset bar with a width field', from: 'Chrome DevTools’ device toolbar, Firefox Responsive Design Mode',
      desc: 'A bar above the frame in the close-up: Phone 390 · Tablet 768 · Laptop 1280 · Fit, then a width field and a height field you can type in (“834 × 640”). Picking a preset sets the width; Fit follows the window. The frame stays centred. Nothing to drag.',
      good: 'One click to a phone; typing gives an exact width; familiar to anyone who has used a browser’s device mode.', cost: 'A row of controls over every close-up, including when you never resize. Finding the width where a layout breaks means typing numbers.',
      mock: () => lpBoard({ preview: lpDesktopThumb(), over: lpCloseUp({ w: 390, h: 590, above: `<div class="lp-bar">${lpPresets({ on: 390 })}<span class="grow"></span><span class="lp-field ac">390</span><span class="lp-x">×</span><span class="lp-field">590</span></div>` }) }) },
    { key: 'C', name: 'A slider under the page', from: 'new',
      desc: 'A slim bar under the frame in the close-up with “Width” and a slider from 320 to 1280, ticks at 390, 768 and 1280 that the thumb snaps to when close, and the width in pixels (“600 px”). Arrow keys on the slider move it by 10; Shift by 100.',
      good: 'One control that both drags and snaps to a phone and a tablet; takes no room above the page.', cost: 'Reads as a volume control, not a window size; no height, no typed width. Not in any browser’s tools, so not familiar.',
      mock: () => lpBoard({ preview: lpDesktopThumb(), over: lpCloseUp({ w: 600, h: 620, below: `<div class="lp-bar"><b>Width</b>${lpSlider(600, { width: 300 })}<span class="lp-field" style="margin-left:6px">600 px</span><span class="lp-chip sm">Fit</span></div>`, cap: LP_CAP }) }) },
    { key: 'D', name: 'Handles on both edges, and presets', from: 'Chrome DevTools’ device mode (both handles) and its presets',
      desc: 'A grip on each side of the frame, which moves both at once so the page stays centred, with the width beside the grip while dragging; and a small row of presets over the frame (Phone · Tablet · Laptop · Fit) that sets the width in one click. No typed fields. Presets are chips with their width as a faint number; the active one is lit while the width equals it.',
      good: 'Drag to explore, click to land on a known width; both are one gesture. Few controls, none that need explaining.', cost: 'No typed width. Presets take a row above the frame.',
      mock: () => lpBoard({ preview: lpDesktopThumb(), over: lpCloseUp({ w: 700, h: 590, above: `<div class="lp-bar" style="gap:6px">${lpPresets({ size: 'sm', widths: false })}<span class="lp-readout" style="margin-left:6px">700 × 590</span></div>`, handles: `<span class="lp-handle" style="left:-18px"></span><span class="lp-handle" style="right:-18px"></span><span class="lp-ghost" style="left:-226px;width:1152px"></span>` }) }) },
    { key: 'E', name: 'The page follows a pane split', from: 'Zed’s resizable panes',
      desc: 'The close-up becomes two panes: the page, and a side panel (name, description, Pick, + Also, comment) like a card’s. The divider between them drags (and so does the side panel’s width); the page’s width is whatever its pane is, so a wide panel gives a phone-sized page. No width readout or presets.',
      good: 'Picking and comment are beside the page, with no trip back to the grid; the resize is just the divider.', cost: 'The width you get is a by-product of the panel’s size: no exact phone width, and a narrow page means a wide panel for words. A bigger change to the close-up than the others.',
      mock: () => lpBoard({ preview: lpDesktopThumb(), over: lpCloseUp({ w: 760, h: 640, handles: '', side: `<div style="width:12px;display:flex;align-items:center;justify-content:center;flex:none"><span class="lp-handle" style="position:static;margin:0"></span></div><div class="bd-panel" style="border:1px solid var(--b);border-radius:8px;width:280px"><div class="bd-oh"><span class="bd-letter">A</span><span class="bd-oname">Three tiers</span><span class="bd-tag rec">Recommended</span></div><p class="bd-desc">Three plans in cards, Growth marked most popular, each with what it includes.</p><div class="bd-pc"><b>Good</b><span>Familiar; the middle plan stands out.</span><b>Cost</b><span>Comparing one feature means reading three lists.</span></div><div class="grow"></div><div class="bd-foot"><span class="bd-pick">Pick A</span><span class="bd-also">+ Also</span></div><span class="bd-cm" style="flex:none">Comment on A…</span></div>`, cap: 'A. Three tiers · drag the divider to resize · ← → the other variants · Esc closes' }) }) },
  ],
});

// 2. Where the preview resizes --------------------------------------------------------------------
const lpWidthBarControl = (width, extra = '') => `<span class="lp-bar" style="padding:2px 8px;gap:6px;border:0;background:none">${ic('monitor', 'xs')}${lpPresets({ on: width, size: 'sm', fit: false, widths: false })}${lpSlider(width, { width: 110, labels: false })}<span class="lp-field" style="height:20px;min-width:52px;font-size:11px">${width} px</span>${extra}</span>`;
const lpCardControl = (width, { own = false } = {}) => (variant) => `<div class="lp-mini">${lpPresets({ on: width, size: 'sm', fit: false, widths: false })}<span class="grow"></span>${own ? '<span class="lp-own">Own width</span>' : `<span class="lp-note">${width} px</span>`}</div>`;
TOPICS.push({
  id: 'where', section: 'Resizing', title: 'Where the preview resizes', size: 'wide', rec: 'C',
  now: 'Nowhere. A card draws the live page at a fixed 1280 × 800 (or 390 × 844 in Phone), scaled down to the card’s width, non-interactive, with Open page over its corner. The close-up has the one width. To compare how three variants look on a phone you switch the whole page to Phone and read narrow cards, five across.',
  nowImg: 'img/now-cards.png',
  issues: [
    'A card’s page is always drawn at desktop width, so what a variant does on a tablet or phone only shows after opening each one.',
    'Comparing the variants at one narrow width means opening the close-up three times and resizing each time, if resizing exists.',
    'Phone mode redraws the whole grid five across, in narrow cards with their descriptions hidden: a different page, not a different width.',
  ],
  options: [
    { key: 'A', name: 'Only in the close-up', from: 'new',
      desc: 'The cards stay as they are (1280 px scaled down). Resizing is in the close-up only, by whichever control topic 1 picks (drawn here with a drag handle). Closing and opening another variant starts at full width again.',
      good: 'Nothing changes on the grid; the least to build.', cost: 'Comparing variants at a narrow width means resizing in each close-up. Nothing on the grid hints that the page is responsive.',
      mock: () => lpBoard({ preview: lpDesktopThumb(), over: lpCloseUp({ w: 834, h: 640, handles: `<span class="lp-handle" style="right:-18px"></span><span class="lp-ghost" style="left:-159px;width:1152px"></span>` }) }) },
    { key: 'B', name: 'A width control under each card’s page', from: 'new',
      desc: 'Under each card’s page, a small row of Phone · Tablet · Laptop chips and the width. Clicking one redraws that card’s page at that width (centred in the card, scaled to the card’s height) while the other cards stay as they are. The close-up opens at the card’s width, and has the control too.',
      good: 'Each variant can be checked right where you read its words, and a card can be left at the width that shows the problem.', cost: 'Another row on every card, which are already full (description, Good and Cost, Pick, + Also, comment). Three cards at three widths is hard to compare.',
      mock: () => lpBoard({ preview: (variant) => lpThumb(variant.kind, { w: { A: 390, B: 768, C: 1280 }[variant.key], boxW: 302, boxH: 189, surround: variant.key !== 'C' }), h: 560, under: (variant) => lpCardControl({ A: 390, B: 768, C: 1280 }[variant.key])(variant) }) },
    { key: 'C', name: 'One width for all the cards, in the bar', from: 'new',
      desc: 'In the bar, before Submit, one control: Phone · Tablet · Laptop chips, a small slider and the width. It redraws every card’s page at that width at once, and the close-up opens at it and follows it (resizing there changes it for every card). Laptop (1280) is where it starts.',
      good: 'Variants are compared at the same width, the point of looking at several; one control however many cards.', cost: 'You can’t leave one card wide while the others are narrow. The bar, already holding progress, Saved and Submit, gets another control.',
      mock: () => lpBoard({ bar: `${lpWidthBarControl(768)}${lpSubmit()}`, preview: lpDesktopThumb(768), h: 560 }) },
    { key: 'D', name: 'The bar’s width, and a card can keep its own', from: 'new',
      desc: 'As C, and a card’s page can also be given its own width (a small width control under its page): that card shows an Own width tag and stops following the bar until its tag is clicked (Follow). Useful for “B is fine on a tablet, but look at its phone layout”.',
      good: 'The shared width for comparing, an exception for the one variant you are chasing.', cost: 'Two places set a width and a card can be the odd one out without it being obvious; more to build and to explain than C.',
      mock: () => lpBoard({ bar: `${lpWidthBarControl(768)}${lpSubmit()}`, preview: (variant) => lpThumb(variant.kind, { w: variant.key === 'B' ? 390 : 768, boxW: 302, boxH: 189, surround: true }), h: 560, under: (variant) => (variant.key === 'B' ? `<div class="lp-mini"><span class="lp-own">Own width</span><span class="lp-chip sm on">${ic('phone', 'xs')}Phone</span><span class="lp-chip sm">Tablet</span><span class="lp-chip sm">Laptop</span><span class="grow"></span><span class="lp-note">390 px · Follow bar</span></div>` : `<div class="lp-mini"><span class="lp-note">Follows the bar · 768 px</span></div>`) }) },
  ],
});

// 3. Desktop · Phone and the phone versions -------------------------------------------------------
const lpThreeStack = (boardHtml, settingsHtml, boardLabel, settingsLabel) => stackOf([[boardLabel, boardHtml], [settingsLabel, settingsHtml]], 14);
const lpSeg = (on) => `${seg(['Desktop', 'Phone'], on)}`;
TOPICS.push({
  id: 'modes', section: 'Resizing', title: 'What becomes of Desktop · Phone and the phone versions', size: 'wide', rec: 'A',
  now: 'A variant can have a picture and a live page, and a phone picture and a phone page. The bar has Desktop · Phone when the design has both kinds; Phone redraws every card from its phone picture or phone page (a variant with neither says “No phone version”) in a five-column grid. Settings › General › Artifacts › Sizes (Desktop · Phone · Both) tells agents which to make; with Both and Variants as › Live pages, every variant is two HTML files.',
  nowImg: 'img/now-phone.png',
  issues: [
    'With Sizes › Both, an agent writes and checks two pages per variant for what responsive CSS does in one, and the second can drift from the first.',
    'The toggle changes the page’s whole grid (five narrow columns, descriptions hidden), a bigger change than a viewport width.',
    'Sizes and Variants as interact without saying so: Sizes › Both with Pictures means phone pictures, with Live pages means phone pages, and the setting’s line doesn’t say which.',
  ],
  options: [
    { key: 'A', name: 'Live pages ignore Desktop · Phone; the toggle is for pictures', from: 'new',
      desc: 'A live page is one page: agents publish only <code>page</code>; <code>phone_page</code> goes away. The bar’s toggle shows only when some variant has a phone picture, and then swaps pictures as today; live pages don’t change with it (resize them as topic 1 and 2 say). Settings: Sizes stays and applies to pictures only; with Variants as › Live pages it’s greyed, its line saying “Pictures only: a live page is one responsive page.” With Both, Sizes still sizes the pictures, one page per variant.',
      good: 'The least change: pictures keep both sizes; a live-page design is one file per variant and no toggle.', cost: 'In a design that mixes pictures and pages the toggle moves some cards and not others, which needs saying on the toggle.',
      mock: () => lpThreeStack(lpBoard({ preview: lpDesktopThumb(), h: 330 }), lpSettings({ sizes: 'Desktop', as: 'Live pages', dim: true, sizesDesc: 'Pictures only: a live page is one responsive page.', h: 360 }), 'A design of live pages: the bar has no Desktop · Phone', 'Settings › General › Artifacts: Variants as › Live pages greys Sizes') },
    { key: 'B', name: 'The toggle stays, and sets the preview’s width', from: 'new',
      desc: 'Desktop · Phone stays in the bar for pictures and live pages. Phone redraws the cards in the five-column grid with each live page at 390 px wide (the same page, narrowed), Desktop at 1280, and the close-up opens at 390; a width set as topic 1 or 2 picks is just a width in between. <code>phone_page</code> goes away; <code>phone_picture</code> stays. Settings: Sizes stays and applies to both: for pictures it’s what to draw, for live pages what the toggle offers (a page is always one file).',
      good: 'Phone remains one click and the grid’s layout is as today; no loss for people used to the toggle.', cost: 'Two controls for one thing (the toggle and the width) unless the toggle is made a preset; the five-column grid hides descriptions in Phone.',
      mock: () => lpThreeStack(lpBoard({ bar: `${lpSeg('Phone')}${lpSubmit()}`, phone: true, cols: 5, preview: lpPhoneThumb, h: 520 }), lpSettings({ sizes: 'Both', as: 'Live pages', sizesDesc: 'Pictures: the sizes to draw. Live pages: whether the toggle offers Phone.' }), 'Phone chosen: the same live pages at 390 px', 'Settings: Sizes applies to both') },
    { key: 'C', name: 'Remove Desktop · Phone and Sizes everywhere', from: 'new',
      desc: 'No toggle on the design page, for pictures too; <code>phone_picture</code> and <code>phone_page</code> are dropped. A design is one picture or one live page per variant: an agent that wants to show a phone layout makes it a variant (or a live page you resize). The Sizes row is removed from Settings; Variants as stays. Cards are always drawn in the three-column grid.',
      good: 'The simplest model and the least to maintain: one file per variant, one grid, one fewer setting.', cost: 'Pictures lose their phone versions: a design that is only pictures of a phone layout becomes a variant per size. The most to remove, and it takes away something that works today for pictures.',
      mock: () => lpThreeStack(lpBoard({ preview: lpDesktopThumb(), h: 330 }), lpSettings({ as: 'Live pages', noSizes: true, h: 300 }), 'No toggle, whatever the variants are', 'Settings: no Sizes row') },
    { key: 'D', name: 'Keep phone pages as an optional extra', from: 'new',
      desc: '<code>phone_page</code> stays but is optional: Sizes › Both no longer asks for it with Variants as › Live pages (the Sizes row greys, its line saying “Live pages: one page by default; a phone page only if its layout truly differs”). The toggle shows only if some variant has a phone page or picture; in Phone, a variant without one shows its one page at 390 px, never “No phone version”. Resizing (topics 1 and 2) works on every live page.',
      good: 'Nothing breaks and nothing is lost, and phone-only layouts (a separate mobile site) can still be shown.', cost: 'The two-file model stays in the protocol, and two ways to see a phone (the toggle, resizing) coexist. Agents need telling when to bother.',
      mock: () => lpThreeStack(lpBoard({ bar: `${lpSeg('Phone')}${lpSubmit()}`, phone: true, cols: 5, preview: lpPhoneThumb, h: 520 }), lpSettings({ sizes: 'Both', as: 'Live pages', dim: true, sizesDesc: 'Live pages: one page by default; a phone page only if its layout truly differs.' }), 'Phone chosen: pages without a phone version show their one page at 390 px', 'Settings: Sizes has less to say about live pages') },
  ],
});
