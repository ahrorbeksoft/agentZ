// The live preview round's building blocks. The design page, its bar, cards and close-up are the
// artifacts-2 round's (mock-artifacts-2.js, loaded before this); this file adds the sample page
// (a responsive "Plans page" that really reflows at any width), the width controls and the
// close-up around them. Only ICONS may be touched at the top level: decisions.js reads this
// file without the others.

Object.assign(ICONS, {
  tablet: '<rect width="16" height="20" x="4" y="2" rx="2" ry="2"/><line x1="12" x2="12.01" y1="18" y2="18"/>',
});

// A live page as an agent writes it: plain responsive HTML. `kind` is one of the three
// variants; the layout follows the width as media queries would (the mock has no iframe, so the
// breakpoints are worked out here).
const LP_PLANS = [
  ['Starter', '$0', ['Up to 25 products', 'One staff account', 'Email support']],
  ['Growth', '$29', ['Unlimited products', 'Five staff accounts', 'Priority support']],
  ['Scale', '$99', ['Everything in Growth', 'Unlimited staff', 'A named contact']],
];
function lpPage(kind, w, h) {
  const compact = w < 720;
  const pad = compact ? 20 : w < 1000 ? 36 : 56;
  const ink = '#1b1d24';
  const soft = '#656b78';
  const line = '#e4e6eb';
  const accent = '#5b5bd6';
  const check = `<span style="color:${accent};margin-right:8px">✓</span>`;
  const bar = `<i style="display:block;width:20px;height:2px;background:${ink};margin:4px 0"></i>`;
  const link = (text) => `<span style="margin-left:26px">${text}</span>`;
  const header = `<div style="height:56px;display:flex;align-items:center;padding:0 ${pad}px;border-bottom:1px solid ${line};font-size:14px;color:${soft}">
    <b style="color:${ink};font-size:16px;display:flex;align-items:center;gap:8px"><i style="width:18px;height:18px;border-radius:5px;background:${accent};display:inline-block"></i>storefront</b><span style="flex:1"></span>
    ${compact ? `<span>${bar}${bar}${bar}</span>` : `${link('Products')}${link('Plans')}${link('Docs')}<span style="margin-left:26px;padding:7px 14px;border-radius:7px;background:${ink};color:#fff">Sign up</span>`}</div>`;
  const centered = kind === 'one';
  const hero = `<div style="padding:${compact ? 26 : 44}px ${pad}px 8px;text-align:${centered ? 'center' : 'left'}">
    <div style="font-size:${compact ? 27 : w < 1000 ? 34 : 40}px;line-height:1.15;font-weight:600;letter-spacing:-.02em;color:${ink}">Plans that grow with your shop</div>
    <div style="margin-top:10px;font-size:${compact ? 14 : 17}px;color:${soft};line-height:1.45">Start free. Pay more only when you sell more.</div></div>`;
  const button = (text, solid) => `<div style="margin-top:18px;text-align:center;padding:9px 0;border-radius:8px;font-size:14px;font-weight:500;${solid ? `background:${accent};color:#fff` : `border:1px solid ${line};color:${ink}`}">${text}</div>`;
  let body = '';
  if (kind === 'tiers') {
    const columns = w >= 960 ? 3 : w >= 600 ? 2 : 1;
    const cards = LP_PLANS.map(([name, price, items], index) => `<div style="border:${index === 1 ? `2px solid ${accent}` : `1px solid ${line}`};border-radius:14px;padding:22px;position:relative">
      ${index === 1 ? `<span style="position:absolute;top:-11px;left:22px;background:${accent};color:#fff;font-size:11px;padding:2px 9px;border-radius:9px">Most popular</span>` : ''}
      <div style="font-size:15px;font-weight:600">${name}</div>
      <div style="margin:6px 0 14px;font-size:34px;font-weight:600;letter-spacing:-.02em">${price}<span style="font-size:14px;color:${soft};font-weight:400"> /month</span></div>
      ${items.map((item) => `<div style="font-size:14px;color:${soft};padding:4px 0">${check}${item}</div>`).join('')}
      ${button(index === 0 ? 'Start free' : 'Choose ' + name, index === 1)}</div>`).join('');
    body = `<div style="padding:26px ${pad}px;display:grid;gap:18px;grid-template-columns:repeat(${columns},1fr)">${cards}</div>`;
  } else if (kind === 'table') {
    const rows = [['Products', '25', 'Unlimited', 'Unlimited'], ['Staff accounts', '1', '5', 'Unlimited'], ['Checkout themes', '—', '✓', '✓'], ['Support', 'Email', 'Priority', 'A named contact']];
    const wide = w >= 640;
    const columns = wide ? [0, 1, 2, 3] : [0, 2];
    const cell = (value, head, first) => `<td style="padding:${head ? '14px' : '11px'} 12px;border-bottom:1px solid ${line};font-size:${head ? 15 : 14}px;${head ? 'font-weight:600;' : ''}${first ? `color:${soft};` : ''}text-align:left">${value}</td>`;
    const tabs = wide ? '' : `<div style="display:flex;gap:8px;margin-bottom:16px">${['Starter', 'Growth', 'Scale'].map((name, index) => `<span style="padding:6px 14px;border-radius:16px;font-size:13px;${index === 1 ? `background:${accent};color:#fff` : `border:1px solid ${line};color:${soft}`}">${name}</span>`).join('')}</div>`;
    const headRow = columns.map((column) => cell(column === 0 ? '' : `${LP_PLANS[column - 1][0]}<div style="font-weight:400;color:${soft};font-size:13px">${LP_PLANS[column - 1][1]} /month</div>`, true, false)).join('');
    const data = rows.map((row) => `<tr>${(wide ? [0, 1, 2, 3] : [0, 2]).map((column) => cell(row[column], false, column === 0)).join('')}</tr>`).join('');
    body = `<div style="padding:26px ${pad}px">${tabs}<table style="width:100%;border-collapse:collapse"><tr>${headRow}</tr>${data}</table>${button('Choose a plan', true)}</div>`;
  } else {
    const features = ['Unlimited products', 'Unlimited staff accounts', 'Priority support', 'Custom checkout themes', 'Analytics', 'API access'];
    body = `<div style="padding:22px ${pad}px;display:flex;justify-content:center"><div style="width:100%;max-width:560px;border:1px solid ${line};border-radius:16px;padding:${compact ? 22 : 30}px;text-align:center">
      <div style="font-size:${compact ? 48 : 60}px;font-weight:600;letter-spacing:-.03em;color:${accent}">2%</div>
      <div style="font-size:14px;color:${soft};margin-bottom:18px">of each sale. Nothing else.</div>
      <div style="display:grid;grid-template-columns:repeat(${compact ? 1 : 2},1fr);gap:2px 20px;text-align:left">${features.map((feature) => `<div style="font-size:14px;padding:5px 0;color:${soft}">${check}${feature}</div>`).join('')}</div>
      ${button('Start selling', true)}</div></div>`;
  }
  return `<div style="font-family:'IBM Plex Sans',system-ui,sans-serif;position:relative;overflow:hidden;width:${w}px;height:${h}px;background:#fff;color:${ink}">${header}${hero}${body}</div>`;
}

// A page at its real size, zoomed by `scale`, centred in a box: how a card draws it.
function lpThumb(kind, { w = 1280, h = 800, boxW, boxH, surround = false } = {}) {
  const scale = Math.min(boxW / w, boxH / h);
  const left = (boxW - w * scale) / 2;
  return `<div class="${surround ? 'lp-surround' : ''}" style="position:relative;overflow:hidden;width:${boxW}px;height:${boxH}px;${surround ? '' : 'background:#fff'}"><div style="position:absolute;left:${left}px;top:0;width:${w}px;height:${h}px;transform:scale(${scale});transform-origin:0 0">${lpPage(kind, w, h)}</div></div>`;
}

const LP_VARIANTS = [
  { key: 'A', name: 'Three tiers', kind: 'tiers', desc: 'Three plans in cards, Growth marked most popular, each with what it includes.', good: 'Familiar; the middle plan stands out.', cost: 'Comparing one feature means reading three lists.' },
  { key: 'B', name: 'Compare table', kind: 'table', desc: 'Every feature in one table, a column a plan.', good: 'The easiest to compare.', cost: 'Cramped on a phone.' },
  { key: 'C', name: 'One plan', kind: 'one', desc: 'A single plan, 2% of each sale, with everything in it.', good: 'Nothing to choose.', cost: 'Hides that large shops can ask for a deal.' },
];
const LP_TOPICS = [{ n: 1, title: 'The plans page', label: null }];

const lpSubmit = () => `<span class="btn">${ic('check', 'xs')}Submit</span>`;
const lpPresets = ({ on = null, size = '', fit = true, widths = true } = {}) => [['phone', 'Phone', 390], ['tablet', 'Tablet', 768], ['monitor', 'Laptop', 1280]]
  .map(([icon, name, width]) => `<span class="lp-chip ${size} ${on === width ? 'on' : ''}">${ic(icon, 'xs')}${name}${widths ? `<i>${width}</i>` : ''}</span>`).join('') + (fit ? `<span class="lp-chip ${size} ${on === 'fit' ? 'on' : ''}">Fit</span>` : '');
function lpSlider(value, { width = 220, min = 320, max = 1280, labels = true } = {}) {
  const at = (n) => ((n - min) / (max - min)) * width;
  return `<span class="lp-slider" style="width:${width}px"><i class="track"></i><i class="fill" style="width:${at(value)}px"></i>${[390, 768, 1280].map((n) => `<i class="tick" style="left:${at(n)}px"></i>${labels ? `<span class="lbl" style="left:${Math.min(at(n), width - 8)}px">${n}</span>` : ''}`).join('')}<i class="thumb" style="left:${at(value)}px"></i></span>`;
}
const lpArrows = () => `<span class="bd-arrow" style="left:10px">${ic('chev-left', 'sm')}</span><span class="bd-arrow" style="right:10px">${ic('chev-right', 'sm')}</span>`;
const LP_CAP = 'A. Three tiers · ← → the other variants · Esc closes';

/**
 * The close-up over the design page, as `renderCloseUp` draws it: the page in a frame, the
 * caption under it, the arrows. `above` and `below` are a round's controls, `handles` sit on the
 * frame, `side` is a panel beside it.
 */
function lpCloseUp({ kind = 'tiers', w = 1152, h = 640, above = '', below = '', handles = '', side = '', cap = LP_CAP, link = '' } = {}) {
  return `<div class="bd-over" style="z-index:15">${above}<div style="display:flex;align-items:stretch"><div style="position:relative;width:${w}px;height:${h}px;flex:none"><div style="border-radius:6px;overflow:hidden">${lpPage(kind, w, h)}</div>${handles}</div>${side}</div>${below}<div class="cap">${cap}${link}</div>${lpArrows()}</div>`;
}

// The design page with its cards, the page's own layout (one topic, so one topic in the nav).
// `preview(variant)` draws each card's page; `bar` is the bar's right end.
function lpBoard({ bar, over = '', preview, cols = 3, phone = false, h = 800, rec = 'A', under } = {}) {
  const cards = LP_VARIANTS.map((variant) => card(variant, {
    rec: variant.key === rec,
    desc: !phone,
    pc: !phone,
    foot: !phone,
    preview: `<div class="bd-fit" style="background:#fff">${preview(variant)}<span class="bd-hint" style="right:8px;bottom:8px">${ic('external', 'xs')}Open page</span></div>${under ? under(variant) : ''}${phone ? `<div class="bd-foot"><span class="bd-pick">Pick ${variant.key}</span><span class="bd-also">+ Also</span></div><span class="bd-cm">Comment on ${variant.key}…</span>` : ''}`,
  })).join('');
  return designPage({ bar: designBar({ buttons: bar ?? lpSubmit() }), nav: boardNav(LP_TOPICS, { on: 1, section: 'Plans page' }), body: topicHead() + grid(cards, cols), over, h });
}
const lpDesktopThumb = (width = 1280) => (variant) => lpThumb(variant.kind, { w: width, boxW: 302, boxH: 189, surround: width < 1280 });
const lpPhoneThumb = (variant) => lpThumb(variant.kind, { w: 390, h: 844, boxW: 165, boxH: 357 });

// Settings › General › Artifacts, the design defaults.
function lpSettings({ sizes = 'Desktop', as = 'Pictures', sizesDesc = 'The sizes each variant is shown at.', dim = false, noSizes = false, h = 330 } = {}) {
  const sizesRow = noSizes ? '' : settingRow('Sizes', sizesDesc, seg(['Desktop', 'Phone', 'Both'], sizes)).replace('class="srowx"', `class="srowx" style="${dim ? 'opacity:.45' : ''}"`);
  const rows = [
    settingRow('Variants', 'How many variants a topic has.', stepper(3)),
    settingRow('Include today’s', 'Today’s version as a variant to keep.', sw(true)),
    sizesRow,
    settingRow('Variants as', 'Pictures, live pages, or both.', seg(['Pictures', 'Live pages', 'Both'], as)),
  ].filter(Boolean);
  return settings2(`<div style="font-size:15px;color:var(--t);margin-bottom:4px">General</div>${settingGroup('Artifacts', rows)}`, { h });
}
