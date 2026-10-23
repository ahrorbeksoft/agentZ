// The picker's shape: the effort in a submenu, and how tall the picker is. model_picker.rs, and
// the agent picker's account submenus (agent_view.rs render_agent_picker) for how a submenu opens.

const mvChevron = () => ic('chev-right', 'xs mu');
// A model's own levels: the effort's choices depend on the model.
const MV_SONNET_EFFORTS = ['Low', 'Medium', 'High'];
const mvLevelsMenu = (levels, current, style) => mvMenu(levels.map((level) => ['', level, { check: level === current, hl: level === current }]), style, 150);
const mvRowsFor = (count) => count * 28 + 8;

TOPICS.push({
  id: 'effort', section: 'The picker', title: 'Reasoning Effort in a submenu', size: 'wide', rec: 'A',
  now: 'The picker lists the effort’s levels under the models, under a “Reasoning Effort” heading (composer-2, topic 2 A), so five more rows follow the models every time it opens. You asked for them in a submenu. agentZ’s agent picker already has submenus: an agent with accounts has a chevron at its row’s end, and its accounts open beside it on hover. Zed has the effort in a chip of its own beside the model’s (“Change Thinking Effort”), t3code in a menu of its own. Each mock opens the submenu to the left, where there’s room beside the composer.',
  nowImg: 'img/now-model-picker-droid.png',
  options: [
    { key: 'A', name: 'One row at the bottom, its levels beside it', from: 'agentZ agent picker’s account submenus',
      desc: 'The levels give way to one row at the picker’s end: “Reasoning Effort”, then the current level and a chevron (“Extra High ›”). Hovering it, or → on it, opens the levels beside the picker, the current one checked; picking one closes both. The model’s other settings, such as Claude Agent’s Fast mode switch, stay rows of their own.',
      good: 'The models get the room; the level stays in sight on its row.', cost: 'Changing the level takes a hover more.',
      mock: () => mvScene(mvPicker({ rows: mvRows(mvShown()), below: mvManage(), effort: { hover: true } }), { extra: mvLevelsMenu(MV_EFFORTS, 'Extra High', 'right:342px;bottom:66px;z-index:21') }) },
    { key: 'B', name: 'Each model’s levels beside it', from: 'new',
      desc: 'Every model’s row ends with a chevron. Hovering one opens that model’s own levels beside it; picking a level picks the model and the level at once, and clicking the row picks the model on the level it had.',
      good: 'Each model shows its own levels, which differ from model to model.', cost: 'A submenu follows the mouse down the list, and the current level shows only on the chip.',
      mock: () => mvScene(mvPicker({ rows: mvRows(mvShown(), { each: (name) => ({ end: mvChevron(), hover: name === 'Sonnet 5.5' }) }), below: mvManage(), effort: false }), { extra: mvLevelsMenu(MV_SONNET_EFFORTS, '', 'right:342px;top:331px;z-index:21') }) },
    { key: 'C', name: 'Both in submenus', from: 'new',
      desc: 'The chip opens a short menu: “Model” with “Opus 5.5 ›”, “Reasoning Effort” with “Extra High ›”, then Manage Models…. The search and the models open beside it, as the levels do.',
      good: 'The smallest menu, and model and level side by side.', cost: 'Picking a model takes one more step, and typing to search needs the submenu open.',
      mock: () => mvScene(mvMenu([['', 'Model', { hl: true, end: `<span class="mu">Opus 5.5</span>${mvChevron()}` }], ['', 'Reasoning Effort', { end: `<span class="mu">Extra High</span>${mvChevron()}` }], 'hr', [ic('settings', 'sm mu'), 'Manage Models…']], 'position:relative', 230), { extra: `<div style="position:absolute;right:284px;bottom:70px;z-index:21">${mvPicker({ rows: mvRows(mvShown()), effort: false })}</div>` }) },
  ],
});

TOPICS.push({
  id: 'height', section: 'The picker', title: 'How tall the picker is', size: 'wide', rec: 'A',
  now: 'The models list is at most 18rem tall, nine rows, and then scrolls. The effort’s levels under it add six rows more. With the effort in a submenu (topic 13’s A) the picker is already about 130px shorter. You asked for it a little smaller. The mocks show the whole picker, with topic 13’s A and Manage Models….',
  nowImg: 'img/now-model-picker-droid.png',
  options: [
    { key: 'A', name: 'Eight rows', from: 'new',
      desc: 'The list is 16rem: eight models, then it scrolls. The picker is about 345px tall.',
      good: 'A little shorter, and still most of a short list at once.', cost: 'One row less to see.',
      mock: () => mvPickerScene(mvPicker({ rows: mvRows(mvShown()), below: mvManage(), maxH: mvRowsFor(8) })) },
    { key: 'B', name: 'Seven rows', from: 'new',
      desc: 'The list is 14rem: seven models. The picker is about 315px tall.',
      good: 'Shorter again, over less of the conversation.', cost: 'A list of 16 scrolls more than half of it.',
      mock: () => mvPickerScene(mvPicker({ rows: mvRows(mvShown()), below: mvManage(), maxH: mvRowsFor(7) })) },
    { key: 'C', name: 'Six rows', from: 'new',
      desc: 'The list is 12rem: six models. The picker is about 290px tall.',
      good: 'The smallest; with favorites (topic 10’s A) they and three more fit.', cost: 'Most models are a scroll or a search away.',
      mock: () => mvPickerScene(mvPicker({ rows: mvRows(mvShown()), below: mvManage(), maxH: mvRowsFor(6) })) },
    { key: 'D', name: 'Nine rows, as today', from: 'today',
      desc: 'The list stays 18rem; only the effort’s levels leave. The picker is about 375px tall.',
      good: 'The most models at once.', cost: 'Only as much shorter as the submenu makes it.',
      mock: () => mvPickerScene(mvPicker({ rows: mvRows(mvShown()), below: mvManage(), maxH: mvRowsFor(9) })) },
  ],
});
