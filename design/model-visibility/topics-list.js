// The Models list in Settings (topic 1’s A): its tools and its order. t3code’s
// ProviderModelsSection.

const mvListPiece = (opts, h = 470) => mvSectionPiece(mvModelsList(opts), { h });
const mvOnly = { filter: false, counts: false, bulk: false, groups: false };
const mvMoveButtons = () => `<span class="row none" style="gap:2px"><span class="ibtn sm hov" style="position:relative">${ic('arrow-up', 'xs')}${mvTip('Move up', 'right:-8px;top:-30px')}</span><span class="ibtn sm">${ic('arrow-down', 'xs')}</span></span>`;
const mvGrip = () => `<span style="display:inline-flex;color:var(--ph);cursor:grab">${ic('grip', 'sm')}</span>`;
const mvIsDeprecated = (name) => name.endsWith('[Deprecated]');

TOPICS.push({
  id: 'tools', section: 'The Models list', title: 'Finding and changing many at once', size: 'wide', type: 'multi', rec: 'ABCD',
  now: 'There’s no list yet. t3code’s has a Filter models field once there are more than eight, a count line (“66 models · 3 favorites · 50 hidden”), one Disable all / Enable all button, and the rows in three groups: Favorites, All, and Hidden from picker. Pick any; each mock shows its tool alone.',
  options: [
    { key: 'A', name: 'Filter models', from: 't3code',
      desc: 'A field over the list narrows it as you type, from nine models up; “No models match.” when nothing does. The rows keep their switches and stars.',
      good: 'Finds one model among 111.', cost: 'None to speak of.',
      mock: () => mvListPiece({ ...mvOnly, filter: true, query: 'gemini' }, 380) },
    { key: 'B', name: 'Counts', from: 't3code',
      desc: 'Beside the heading: “66 models · 3 favorites · 50 hidden”.',
      good: 'Says how much is hidden without scrolling.', cost: 'One more line of small text.',
      mock: () => mvListPiece({ ...mvOnly, counts: true }) },
    { key: 'C', name: 'Hide all, then pick a few', from: 't3code “Disable all”',
      desc: 'Hide all at the heading’s end turns every switch off, then you turn on the ones you use; once all are hidden it says Show all.',
      good: 'Keeping 16 of 66 takes 16 clicks rather than 50.', cost: 'One click away from an empty picker.',
      mock: () => mvListPiece({ ...mvOnly, bulk: true }) },
    { key: 'D', name: 'Groups: Favorites, All, Hidden from picker', from: 't3code',
      desc: 'Favorites first, then the other shown models (All), then Hidden from picker, each in the agent’s order. A row moves to its group when you flip its switch or star.',
      good: 'What’s in the picker reads at a glance.', cost: 'A row jumps away as you change it.',
      mock: () => mvListPiece({ ...mvOnly, groups: true, query: 'opus' }, 420) },
    { key: 'E', name: 'Hide deprecated', from: 'new',
      desc: 'A Hide deprecated button hides every model the agent names “[Deprecated]” (Droid’s three).',
      good: 'One click for the models nobody should pick.', cost: 'Only Droid marks them so; other agents name them their own way.',
      mock: () => mvListPiece({ ...mvOnly, right: '<span class="btn sm ghost">Hide deprecated</span>', names: MV_MODELS.slice(52), each: (name) => (mvIsDeprecated(name) ? { shown: false } : {}) }, 470) },
  ],
});

const mvSorted = () => [...MV_MODELS].sort((a, b) => a.localeCompare(b));

TOPICS.push({
  id: 'order', section: 'The Models list', title: 'The order of the models', size: 'medium', rec: 'A',
  now: 'The picker lists models in the agent’s order: Droid’s goes Auto Model, Fable, Opus, Sonnet, Haiku, GPT, Gemini, then the rest. t3code lets you move a model up or down within its group, in Settings. The mocks use topic 7’s groups and filter to “opus”.',
  options: [
    { key: 'A', name: 'The agent’s order', from: 'today, Zed',
      desc: 'Models keep the order the agent sends; favorites (topic 10) are the only ones that move.',
      good: 'Nothing to keep up; a new model lands where the agent puts it.', cost: 'You can’t put the one you use most first, other than by starring it.',
      mock: () => mvListPiece({ query: 'opus' }, 420) },
    { key: 'B', name: 'Move up and down', from: 't3code',
      desc: 'Hovering a row shows ↑ and ↓ before its switch (“Move up”, “Move down”), within its group; they go while the list is filtered. The picker follows your order.',
      good: 'Exactly the order you want, as t3code does it.', cost: 'Many clicks to move a model far.',
      mock: () => mvListPiece({ query: 'opus', each: (name) => (name === 'Opus 5.5 Fast Mode' ? { hover: true, end: mvMoveButtons() } : {}) }, 420) },
    { key: 'C', name: 'Drag to reorder', from: 'agentZ role chips',
      desc: 'A grip at each row’s start drags it within its group, as the subthread roles’ chips are dragged.',
      good: 'Fast for big moves.', cost: 'Dragging in a long list scrolls as you go.',
      mock: () => mvListPiece({ query: 'opus', each: () => ({ lead: mvGrip() }) }, 420) },
    { key: 'D', name: 'By name', from: 'new',
      desc: 'The list and the picker sort A to Z.', good: 'Easy to find a name.', cost: 'Opus 4.5 lands before Opus 5.5, and families split up (“GLM” before “GPT”).',
      mock: () => mvListPiece({ query: 'opus', names: mvSorted() }, 420) },
  ],
});
