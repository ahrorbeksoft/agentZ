// Marking up an image before it's sent: opened from the composer's Image chip in the image viewer
// (attachment_image.rs ImageViewer, agent_view.rs TextInputEvent::ChipClicked).

const c2ToolButton = (icon, { on = false, title = '' } = {}) => `<span class="ibtn" title="${title}" style="${on ? 'background:var(--sel);color:var(--t)' : 'color:#c9ced8'}">${ic(icon, 'sm')}</span>`;
const c2ColorDot = (color, on = false) => `<span style="width:16px;height:16px;border-radius:50%;background:${color};flex:none;${on ? 'box-shadow:0 0 0 2px var(--panel),0 0 0 3.5px #dce0e5' : ''}"></span>`;
const c2ToolSep = () => '<span style="width:1px;height:16px;background:var(--b);margin:0 3px;flex:none"></span>';
const C2_TOOLS = [['pencil', 'Pen'], ['square', 'Rectangle'], ['arrow-up-right', 'Arrow'], ['type', 'Text'], ['highlighter', 'Highlighter'], ['message-square', 'Note']];
// The markup toolbar: the tools, the color, undo, redo and erase, then Cancel and Done.
const c2Toolbar = ({ tools = C2_TOOLS.map(([icon]) => icon), active = 'pencil', colors = true, history = true, actions = true, style = '' } = {}) => `<div class="row" style="gap:2px;padding:4px;border:1px solid var(--b);border-radius:8px;background:var(--panel);box-shadow:0 8px 24px rgba(0,0,0,.45);${style}">
  ${tools.map((icon) => c2ToolButton(icon, { on: icon === active })).join('')}
  ${colors ? `${c2ToolSep()}<span class="row" style="gap:6px;padding:0 4px">${c2ColorDot(C2_RED, true)}${c2ColorDot('#ffd60a')}${c2ColorDot('#0a84ff')}${c2ColorDot('#30d158')}${c2ColorDot('#f5f5f7')}</span>` : ''}
  ${history ? `${c2ToolSep()}${c2ToolButton('undo')}${c2ToolButton('redo')}${c2ToolButton('eraser')}` : ''}
  ${actions ? `${c2ToolSep()}<span class="btn sm ghost" style="color:#c9ced8">Cancel</span><span class="btn sm primary">Done</span>` : ''}</div>`;
const c2MarkupButton = (on = false) => `<span class="btn sm" style="gap:5px;${on ? 'background:var(--sel);border-color:var(--sel);color:var(--t)' : 'background:transparent;border-color:#5a606c;color:#e6e9ee'}">${ic('pencil', 'xs')}Markup</span>`;

const c2ChipMenu = () => `<div class="menu" style="bottom:62px;left:218px;width:190px">${['View', 'Mark Up…'].map((label, i) => `<div class="it ${i === 1 ? 'hl' : ''}">${ic(i ? 'pencil' : 'eye', 'sm')}${label}</div>`).join('')}<div class="hr"></div><div class="it">${ic('trash', 'sm')}Remove</div></div>`;

TOPICS.push({
  id: 'markup-start', section: 'Image markup', title: 'Where markup starts', size: 'wide', rec: 'A',
  now: 'Clicking an Image chip in the composer opens the image viewer, the same one every image in a thread opens: a dark backdrop over the window, the image fitted in the middle, Close above its corner and its name under it (“Image”). A click zooms to 200%, scrolling zooms up to 8×, dragging pans, Escape closes. Nothing can be drawn on it. In every option, markup is only for images in the composer, not yet sent; images in the conversation stay view-only.',
  nowImg: 'img/now-image-viewer.png',
  options: [
    { key: 'A', name: 'A Markup button in the viewer', from: 'macOS Quick Look Markup',
      desc: 'The viewer opened from the composer has a Markup button beside Close. It brings up the toolbar over the image (the tools are the next topic); Done keeps the marks, Cancel drops them. Without it pressed, the viewer zooms and pans as today.',
      good: 'Viewing stays as it is; marking up is one click.', cost: 'One click before you can draw.',
      mock: () => c2Viewer({ image: c2Shot(c2Marks({ text: false, arrow: false })), closeExtra: c2MarkupButton(true), top: c2Toolbar({ style: 'margin-bottom:4px' }) }) },
    { key: 'B', name: 'The tools are there from the start', from: 't3code annotation toolbar',
      desc: 'Opened from the composer, the viewer is the markup view: the toolbar floats at the top center, as t3code’s annotation tools do, with the pen picked, so you can draw at once. Zoom moves to the scroll wheel and pinch (a click draws). Escape or Done closes it.',
      good: 'Drawing on a screenshot is the first thing you can do.', cost: 'Just looking at it means not drawing by accident.',
      mock: () => c2Viewer({ image: c2Shot(c2Marks({ text: false, arrow: false, rect: false })), top: c2Toolbar({ actions: false, style: 'margin-bottom:4px' }), bottom: '<div class="row" style="gap:8px"><span class="btn sm primary">Done</span></div>' }) },
    { key: 'C', name: 'From the chip’s menu', from: 'new (Zed context menus)',
      desc: 'Right-clicking an Image chip opens View, Mark Up… and Remove. Mark Up… opens the viewer with the tools on, as in B; a plain click still just views it, as today.',
      good: 'Nothing changes for viewing; the chip says what you can do.', cost: 'Hard to find: nothing hints that the menu is there.',
      mock: () => c2Thread(c2Card({ body: `Make the pay button full width ${c2ImageChip({ hl: true })}${c2Caret}` }), { overlay: c2ChipMenu(), h: 340 }) },
  ],
});

// The tools, each with its mark on the screenshot and its button picked in the toolbar.
const c2ToolMock = (active, marks, { colors = false, history = false, overlay = '' } = {}) => frame(`<div style="position:absolute;inset:0;background:#111317;display:flex;flex-direction:column;align-items:center;justify-content:center;gap:10px">${c2Toolbar({ active, colors, history, actions: false })}${c2Shot(marks)}</div>${overlay}`, { w: 560, h: 420 });
const c2NoteBox = `<div class="pop" style="position:absolute;left:170px;top:268px;width:230px;padding:8px;display:flex;flex-direction:column;gap:6px;font-size:12px"><div class="row g1"><span style="width:16px;height:16px;border-radius:50%;background:${C2_RED};color:#fff;font:600 10px sans-serif;display:grid;place-items:center">1</span><span class="mu">Note</span></div><div class="field" style="height:26px;font-size:12px">Make this full width${c2Caret}</div></div>`;

TOPICS.push({
  id: 'markup-tools', section: 'Image markup', title: 'The markup tools', size: 'medium', type: 'multi', rec: 'ABCDH',
  now: 'There are none. Pick any: each is a button in the markup toolbar. A mark can be moved or deleted until Done, in every option.',
  nowImg: 'img/now-image-viewer.png',
  options: [
    { key: 'A', name: 'Pen', from: 't3code Draw, macOS Markup Sketch',
      desc: 'Draws freehand where you drag, in the picked color, 3 px wide.',
      good: 'Circle anything, in one gesture.', cost: 'Freehand lines look rough.',
      mock: () => c2ToolMock('pencil', c2Marks({ rect: false, arrow: false, text: false })) },
    { key: 'B', name: 'Rectangle', from: 't3code Region, macOS Markup Shapes',
      desc: 'Drag to draw a box around a part of the image.',
      good: 'The clearest way to say “this part”.', cost: '—',
      mock: () => c2ToolMock('square', c2Marks({ pen: false, arrow: false, text: false })) },
    { key: 'C', name: 'Arrow', from: 'macOS Markup Shapes',
      desc: 'Drag from the tail to the point to draw an arrow.',
      good: 'Points at something small without covering it.', cost: '—',
      mock: () => c2ToolMock('arrow-up-right', c2Marks({ pen: false, rect: false, text: false })) },
    { key: 'D', name: 'Text', from: 'macOS Markup Text',
      desc: 'Click to type a short label on the image, in the picked color.',
      good: 'Says what to change right where it is.', cost: 'Text on a busy screenshot can be hard to read.',
      mock: () => c2ToolMock('type', c2Marks({ pen: false, rect: false, arrow: false })) },
    { key: 'E', name: 'Highlighter', from: 'macOS Markup Highlight',
      desc: 'A wide, see-through yellow stroke, for text in the screenshot.',
      good: 'Marks a line of text without hiding it.', cost: 'Close to the pen; one more button.',
      mock: () => c2ToolMock('highlighter', c2Marks({ pen: false, rect: false, arrow: false, text: false, highlight: true })) },
    { key: 'F', name: 'Numbered notes', from: 't3code annotation comment',
      desc: 'Click to drop a numbered pin and type a note for it. The pin goes on the picture; the notes go along as text with the message (“1: Make this full width”), as t3code sends an annotation’s comment with its screenshot.',
      good: 'Long remarks without covering the picture; the agent reads them as text.', cost: 'Two places to look: the pins and the notes.',
      mock: () => c2ToolMock('message-square', c2Marks({ pen: false, arrow: false, text: false, numbers: true }), { overlay: c2NoteBox }) },
    { key: 'G', name: 'Colors', from: 'macOS Markup colors',
      desc: 'Five colors to draw in: red (the default), yellow, blue, green and white.',
      good: 'Readable on light and dark screenshots.', cost: 'Without it, every mark is red.',
      mock: () => c2ToolMock('pencil', c2Marks({ rect: false, arrow: false, text: false, color: '#0a84ff' }), { colors: true }) },
    { key: 'H', name: 'Undo, redo and erase', from: 't3code Erase, macOS Markup',
      desc: 'Undo and Redo (Cmd-Z, Shift-Cmd-Z), and an eraser that removes the mark you click.',
      good: 'Mistakes cost nothing.', cost: '—',
      mock: () => c2ToolMock('eraser', c2Marks({ text: false }), { history: true }) },
  ],
});

// What Done does.
const c2ThumbMarked = () => `<div style="width:156px;height:96px;overflow:hidden;border-radius:6px;border:1px solid var(--b);flex:none"><div style="transform:scale(.3);transform-origin:0 0">${c2Shot(c2Marks())}</div></div>`;
const c2ThumbPlain = () => `<div style="width:156px;height:96px;overflow:hidden;border-radius:6px;border:1px solid var(--b);flex:none"><div style="transform:scale(.3);transform-origin:0 0">${c2Shot()}</div></div>`;
const c2HoverPreview = (thumb, style) => `<div class="pop" style="${style};padding:6px;display:flex;flex-direction:column;gap:4px">${thumb}</div>`;

TOPICS.push({
  id: 'markup-result', section: 'Image markup', title: 'What Done does', size: 'wide', rec: 'A',
  now: 'Nothing changes images: what you paste is what the agent gets. Hovering an Image chip shows the picture (up to 320×240); a sent message shows its images as small thumbnails.',
  nowImg: 'img/now-composer-image.png',
  options: [
    { key: 'A', name: 'It replaces the image, and can be undone', from: 'macOS Quick Look Markup (Revert)',
      desc: 'The chip now sends the marked-up picture, and shows a pencil. Opening it again shows the marks still editable, with Revert to Original. The marks are drawn into the picture only when the message is sent.',
      good: 'One image, the one you meant; nothing to tidy up.', cost: 'The unmarked picture isn’t sent alongside.',
      mock: () => c2Thread(c2Card({ body: `Make the pay button full width ${c2ImageChip({ marked: true, hl: true })}${c2Caret}` }), { overlay: c2HoverPreview(c2ThumbMarked(), 'bottom:96px;left:240px'), h: 360 }) },
    { key: 'B', name: 'It adds a marked-up copy', from: 'new',
      desc: 'Done puts a second chip after the first, “Image (marked up)”, and the original stays. Remove either before sending.',
      good: 'The agent sees both: the screen as it is and your marks.', cost: 'Two images in every marked-up message, unless you remove one.',
      mock: () => c2Thread(c2Card({ body: `Make the pay button full width ${c2ImageChip()} ${c2ImageChip({ marked: true, label: 'Image (marked up)' })}${c2Caret}` }), { h: 300 }) },
    { key: 'C', name: 'The notes go along as text', from: 't3code annotation comment',
      desc: 'As A, and with Numbered notes (the topic before), each note is added to the message as text under it (“Image notes: 1. Make this full width”), so the agent reads them even when its model sees pictures poorly.',
      good: 'The agent gets your words as words.', cost: 'Only with Numbered notes.',
      mock: () => c2Thread('', { talk: c2Bubble(`<div class="col" style="gap:8px">Make the pay button full width<div class="row" style="gap:6px">${c2ThumbMarked()}</div><div class="sm" style="color:var(--mu)">Image notes:<br>1. Make this full width<br>2. Remove the shipping line</div></div>`) + `<div class="sm ph" style="padding:2px 0 8px;text-align:right">Sent</div>`, h: 360 }) },
    { key: 'D', name: 'The marks become the picture', from: 'new',
      desc: 'Done draws the marks into the picture for good; the chip sends that picture. To change a mark, paste the image again.',
      good: 'The simplest: no state to keep.', cost: 'No undo after Done.',
      mock: () => c2Thread(c2Card({ body: `Make the pay button full width ${c2ImageChip({ hl: true })}${c2Caret}` }), { overlay: c2HoverPreview(c2ThumbMarked(), 'bottom:96px;left:240px'), h: 360 }) },
  ],
});
