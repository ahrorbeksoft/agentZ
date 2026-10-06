// The app icon: five marks on the same Zed-like matte square (img/icon-<key>.svg).

const iconSizes = (key, background) => `<div style="display:flex;align-items:flex-end;gap:22px;padding:16px 20px;border-radius:12px;background:${background}">
    ${[128, 64, 32, 16].map((size) => `<img src="img/icon-${key}.svg" width="${size}" height="${size}" alt="">`).join('')}
  </div>`;

const iconMock = (key) => `<div style="width:640px;display:flex;gap:24px;align-items:center;padding:20px;border-radius:12px;background:#161616">
  <img src="img/icon-${key}.svg" width="272" height="272" alt="">
  <div style="display:flex;flex-direction:column;gap:14px">${iconSizes(key, '#2a2a2a')}${iconSizes(key, '#ececec')}</div>
</div>`;

TOPICS.push({
  id: 'icon', section: 'App icon', title: 'The mark',
  size: 'wide',
  rec: 'E',
  now: 'A cyan-to-violet "Z" with a glow and a green dot, on a blue-grey gradient square.',
  nowImg: 'img/now-icon.png',
  issues: ['The colour gradient and glow look generic next to Zed’s and t3code’s monochrome icons.'],
  options: [
    { key: 'A', name: 'Framed Z', from: 'Zed', desc: 'The Z with its sides drawn in and fading out, so it sits in a square like a window: Zed’s frame of lines, reduced to one.', good: 'Closest to Zed’s line work; reads as a window or pane.', cost: 'The faded sides blur together at 16 px.', mock: () => iconMock('a') },
    { key: 'B', name: 'Z and a cursor', from: 'new', desc: 'The Z beside a terminal’s block cursor in grey, for agents and the terminals next to them.', good: 'Says “terminal” at a glance.', cost: 'Reads as “ZI” in a Finder list.', mock: () => iconMock('b') },
    { key: 'C', name: 'Z at work', from: 'new', desc: 'The bottom bar breaks into three fading steps, like a progress indicator: an agent at work.', good: 'Hints at agents running without a colour.', cost: 'The steps disappear below 32 px, leaving a plain Z.', mock: () => iconMock('c') },
    { key: 'D', name: 'Machine to machine', from: 'new', desc: 'The Z drawn from a ring where it starts to a dot where it ends: one path between machines.', good: 'Distinct silhouette; says “connected”.', cost: 'Looks a little like a circuit or a git graph.', mock: () => iconMock('d') },
    { key: 'E', name: 'Z and its status light', from: 'Zed + today’s dot', desc: 'A plain geometric Z with the green light agentZ shows while an agent works, the one colour in the icon.', good: 'The clearest at every size; keeps today’s green dot, so it still reads as agentZ.', cost: 'The simplest of the five.', mock: () => iconMock('e') },
  ],
});
