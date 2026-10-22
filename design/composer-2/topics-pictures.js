// Warning agents that give pictures where live pages would do: agentz_design_publish
// (artifact_tools.rs) and the design defaults (settings_page.rs render_design_default_rows).

const C2_ANSWER_WARN = `{"artifactId": "a-41", "version": 1, "url": "http://127.0.0.1:4480/a-41/",
 <span style="color:var(--warn)">"warning": "Variants A, C and D are pictures only. The user would
  rather try live pages: add a page to each (a self-contained .html)
  and publish a new version with this artifactId."</span>}`;
const C2_ANSWER_SENT_BACK = `<span style="color:var(--del)">Not published: variants A, C and D are pictures only, and the user
asks for live pages. Give each a page (a self-contained .html) and
publish again.</span>`;
const c2Published = (answer, { card = c2DesignCard(), failed = false, note = '' } = {}) => c2Thread('', {
  talk: c2Bubble('Mock three checkout layouts I can pick from.') +
    c2ToolRow('palette', `<span class="none">${failed ? 'Failed to publish design' : 'Published design'}</span><span class="trunc" style="color:var(--t)">Checkout page</span>`, failed ? `<span style="color:var(--del);display:inline-flex">${ic('x', 'xs')}</span>` : '') +
    c2Answer(answer) + (failed ? '' : card) + note,
  h: 400, w: 700,
});
const c2PicturesNote = `<div class="row g1 xs" style="color:var(--warn)">${ic('warning', 'xs')}Pictures only: agentZ asked for live pages next time</div>`;
const c2ReplyNudge = () => c2Thread('', {
  talk: c2Bubble('How does the new checkout look?') +
    c2Para('Here it is at desktop size:') +
    `<div style="padding:2px 0 6px"><div style="width:260px;height:160px;overflow:hidden;border-radius:6px;border:1px solid var(--b)"><div style="transform:scale(.5);transform-origin:0 0">${c2Shot()}</div></div></div>` +
    c2Bubble('Make the button bigger') +
    `<div class="xs ph" style="padding:0 0 4px 2px">Sent to the agent with the message:</div>` + c2Answer('<span style="color:var(--warn)">agentZ: your last reply showed a picture of a page (checkout.png).\nThe user would rather try live pages: publish a design with a page.</span>'),
  h: 470, w: 700,
});

TOPICS.push({
  id: 'picture-warning', section: 'Pictures and live pages', title: 'What the warning does', size: 'wide', rec: 'A',
  now: 'agentz_design_publish takes each variant as a picture, a live page, or both. Settings › General › Artifacts has design defaults, which each project can replace with its own (“Use General’s” off): Variants, Include today’s, Sizes and Variants as (Pictures, Live pages or Both; Pictures unless you change it). A design that differs from them (pictures only while Variants as says Live pages, say) is sent back, unless the agent gives a reason (deviationReason), which you then see on the design. Nothing else tells an agent that it gave pictures. In every option the warning is on or off by the setting in the next topic.',
  nowImg: 'img/now-design-defaults.png',
  options: [
    { key: 'A', name: 'A warning in the publish answer', from: 'agentZ design defaults (the publish answer)',
      desc: 'A design whose variants are pictures without a page is published as given, and the answer the agent gets says so: which variants are only pictures, that you’d rather try live pages, and how to add them in a new version. You see nothing extra.',
      good: 'Never blocks the agent, and it learns before the next version.', cost: 'The first version is still pictures.',
      mock: () => c2Published(C2_ANSWER_WARN) },
    { key: 'B', name: 'A warning you see too', from: 'new',
      desc: 'As A, and the design’s card in the thread says “Pictures only: agentZ asked for live pages next time”, so you know why the next version may come with pages.',
      good: 'You know what happened without reading the tool call.', cost: 'One more line on the card.',
      mock: () => c2Published(C2_ANSWER_WARN, { card: c2DesignCard({ note: c2PicturesNote }) }) },
    { key: 'C', name: 'Sent back until it has pages', from: 'agentZ design defaults (sent back)',
      desc: 'A design with pictures only is refused, as a design that differs from the defaults is today, but with no reason to give that lets it through. The agent makes the pages and publishes again.',
      good: 'You always get live pages.', cost: 'Slower: a design that’s fine as pictures (an icon, a logo) still needs pages.',
      mock: () => c2Published(C2_ANSWER_SENT_BACK, { failed: true }) },
    { key: 'D', name: 'Also pictures in replies', from: 'new',
      desc: 'As A for designs, and when an agent’s reply shows a picture it made of a page (a Markdown image of a .png), your next message carries a line to the agent: that you’d rather try a live page, published as a design.',
      good: 'Catches mockups that never went through the design tool.', cost: 'A screenshot that shows what was built gets the line too.',
      mock: () => c2ReplyNudge() },
    { key: 'E', name: 'Live pages by default instead', from: 'agentZ design defaults',
      desc: 'No warning: Variants as starts at Live pages, so today’s check sends a pictures-only design back unless the agent says why.',
      good: 'Nothing new to build or set.', cost: 'Agents can still give a reason and send pictures.',
      mock: () => c2SettingsPage('General', c2Section('Artifacts', c2DesignRows({ variantsAs: 1 })), { h: 330 }) },
  ],
});

// The setting: General's, and each project's over it.
const c2WarnRow = (control, { hl = true } = {}) => c2Row('Warn about pictures', 'Tell agents when they give pictures where a live page would do.', control, { hl });
const c2GeneralArtifacts = (extra = '') => c2Section('Artifacts', c2Row('Open in the browser', 'When a page an agent publishes opens by itself. Otherwise its card’s Open does it.', c2Dropdown('Never')) + extra);
const c2ProjectPage = (rows, { overlay = '', h = 330 } = {}) => c2SettingsPage('agentZ', c2Section('This Mac', c2Row('Folder', '~/projects/agentZ', '') + c2Row('Grouping', 'Combined with the repository’s other checkouts.', c2Dropdown('Default'))) + rows, { h, overlay });
const c2UseGeneral = (summary = '5 variants, today’s included, desktop, pictures.') => c2Row('Use General’s', summary, c2Switch(true));
const c2ProjectMenu = () => `<div class="menu" style="right:27px;top:300px;width:170px">${['Default (On)', 'On', 'Off'].map((label, i) => `<div class="it ${i === 0 ? 'hl' : ''}"><span class="grow">${label}</span>${i === 0 ? ic('check', 'sm') : ''}</div>`).join('')}</div>`;
const c2Pair = (a, b) => `<div style="display:flex;flex-direction:column;gap:10px">${a}${b}</div>`;

TOPICS.push({
  id: 'picture-setting', section: 'Pictures and live pages', title: 'The setting', size: 'wide', rec: 'A',
  now: 'There’s no setting for a warning. The design defaults live in Settings › General › Artifacts; a project’s settings have “Design defaults” with a Use General’s switch, and turning it off gives the project its own copy of every default. Elsewhere a project overrides one General setting with a “Default (…)” menu, as Grouping does.',
  nowImg: 'img/now-design-defaults.png',
  options: [
    { key: 'A', name: 'A switch, with a project menu over it', from: 'agentZ Grouping (“Default (…)”)',
      desc: 'Settings › General › Artifacts gets a switch, “Warn about pictures”, on by default. Each project’s settings get the same row as a menu: Default (On), On or Off, as Grouping overrides General’s choice.',
      good: 'One project can differ without taking over its other design defaults.', cost: 'A second place where design choices live.',
      mock: () => c2Pair(c2SettingsPage('General', c2GeneralArtifacts(c2WarnRow(c2Switch(true))), { h: 240 }), c2ProjectPage(c2Section('Design defaults', c2UseGeneral() + c2WarnRow(c2Dropdown('Default (On)'))), { overlay: c2ProjectMenu(), h: 400 })) },
    { key: 'B', name: 'A row in the design defaults', from: 'agentZ design defaults (“Use General’s”)',
      desc: 'A switch among the design defaults, after Variants as. A project that uses General’s defaults uses General’s switch; one with its own defaults has its own.',
      good: 'All of how agents design is in one place.', cost: 'To change only this for a project, it takes over all its design defaults.',
      mock: () => c2Pair(c2SettingsPage('General', c2Section('Artifacts', c2DesignRows({ extra: c2WarnRow(c2Switch(true)) })), { h: 400 }), c2ProjectPage(c2Section('Design defaults', c2UseGeneral('5 variants, today’s included, desktop, pictures, warned about.')))) },
    { key: 'C', name: 'How firm all the defaults are', from: 'new',
      desc: 'One row after the design defaults, “When a design differs”: Send back (today) or Warn. With Warn, every difference from the defaults (pictures where they ask for pages, a variant count, a missing today’s) is published with a warning, as in A of the topic before. Projects have it among their own defaults, as B.',
      good: 'One idea, warn or refuse, for every default.', cost: 'Changes what today’s check does, not only pictures.',
      mock: () => c2SettingsPage('General', c2Section('Artifacts', c2DesignRows({ variantsAs: 1, extra: c2Row('When a design differs', 'What happens when an agent’s design isn’t as these say.', c2Segments(['Send back', 'Warn'], 1), { hl: true }) })), { h: 380 }) },
  ],
});
