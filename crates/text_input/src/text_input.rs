//! A text input: one line, or several that wrap and grow up to a limit
//! ([`TextInput::multi_line`]).
//!
//! Derived from gpui's `examples/input.rs` (Apache-2.0), restyled with the active theme and
//! made to emit [`TextInputEvent`]s so owners can react to edits. An input of several lines also
//! holds chips: a piece of its text drawn as an outlined box with an icon, which the cursor steps
//! over and Backspace removes whole (Zed's mention creases).

use std::ops::Range;
use std::sync::Arc;
use std::time::Duration;

use gpui::{
    Anchor, AnyElement, App, Bounds, ClipboardItem, ContentMask, Context, CursorStyle, ElementId,
    ElementInputHandler, Entity, EntityInputHandler, EventEmitter, FocusHandle, Focusable,
    GlobalElementId, Hsla, Image, KeyBinding, LayoutId, MouseButton, MouseDownEvent,
    MouseMoveEvent, MouseUpEvent, PaintQuad, Pixels, Point, ScrollWheelEvent, SharedString, Style,
    Subscription, TextAlign, TextRun, TransformationMatrix, UTF16Selection, UnderlineStyle, Window,
    WrappedLine, actions, anchored, deferred, div, fill, img, point, prelude::*, px, quad,
    relative, size,
};
use theme::ActiveTheme as _;
use unicode_segmentation::UnicodeSegmentation as _;

actions!(
    text_input,
    [
        Backspace,
        Delete,
        Left,
        Right,
        Up,
        Down,
        SelectLeft,
        SelectRight,
        SelectUp,
        SelectDown,
        SelectAll,
        Home,
        End,
        Newline,
        ShowCharacterPalette,
        Paste,
        /// Pastes the clipboard's text, without what an owner makes of the rest of it.
        PasteRaw,
        Cut,
        Copy,
    ]
);

const CURSOR_BLINK_INTERVAL: Duration = Duration::from_millis(500);
const MASK: char = '•';
const KEY_CONTEXT: &str = "TextInput";
/// In the key context of an input of several lines.
pub const MULTI_LINE_CONTEXT: &str = "multiline";
/// In the key context while the input's owner shows a menu for it, such as completions, so the
/// owner's bindings for the menu's keys come before the input's.
pub const MENU_CONTEXT: &str = "menu";
/// A chip's text: room for its icon, then its label, then a little space. Non-breaking, so a
/// chip isn't wrapped across rows.
const CHIP_ICON_ROOM: &str = "\u{a0}\u{a0}\u{a0}";
const CHIP_END: &str = "\u{a0}";
const CHIP_LABEL_MAX_CHARS: usize = 40;

pub fn init(cx: &mut App) {
    let multi_line = Some("TextInput && multiline");
    cx.bind_keys([
        KeyBinding::new("backspace", Backspace, Some(KEY_CONTEXT)),
        KeyBinding::new("delete", Delete, Some(KEY_CONTEXT)),
        KeyBinding::new("left", Left, Some(KEY_CONTEXT)),
        KeyBinding::new("right", Right, Some(KEY_CONTEXT)),
        KeyBinding::new("shift-left", SelectLeft, Some(KEY_CONTEXT)),
        KeyBinding::new("shift-right", SelectRight, Some(KEY_CONTEXT)),
        KeyBinding::new("home", Home, Some(KEY_CONTEXT)),
        KeyBinding::new("end", End, Some(KEY_CONTEXT)),
        KeyBinding::new("secondary-a", SelectAll, Some(KEY_CONTEXT)),
        KeyBinding::new("secondary-v", Paste, Some(KEY_CONTEXT)),
        KeyBinding::new("secondary-c", Copy, Some(KEY_CONTEXT)),
        KeyBinding::new("secondary-x", Cut, Some(KEY_CONTEXT)),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-left", Home, Some(KEY_CONTEXT)),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-right", End, Some(KEY_CONTEXT)),
        #[cfg(target_os = "macos")]
        KeyBinding::new("ctrl-cmd-space", ShowCharacterPalette, Some(KEY_CONTEXT)),
        // Zed's message editor: Shift-Enter for a new line, as Enter sends.
        KeyBinding::new("shift-enter", Newline, multi_line),
        KeyBinding::new("up", Up, multi_line),
        KeyBinding::new("down", Down, multi_line),
        KeyBinding::new("shift-up", SelectUp, multi_line),
        KeyBinding::new("shift-down", SelectDown, multi_line),
    ]);
}

pub enum TextInputEvent {
    Changed,
    /// Paste or Paste as Plain Text (`plain`) in an input whose owner pastes
    /// ([`TextInput::handles_paste`]).
    Paste {
        item: ClipboardItem,
        plain: bool,
    },
    /// A right-click in an input of several lines, where its owner can show a menu.
    ContextMenu(Point<Pixels>),
}

pub type ChipId = u64;

/// What hovering a chip shows.
#[derive(Clone)]
pub enum ChipPreview {
    Text(SharedString),
    Image(Arc<Image>),
}

/// A piece of the text drawn as one outlined box with an icon.
#[derive(Clone)]
pub struct Chip {
    pub id: ChipId,
    /// Its text in the input's.
    pub range: Range<usize>,
    /// The icon's SVG asset path.
    pub icon: SharedString,
    pub preview: ChipPreview,
    /// What copying it gives.
    pub copy_text: SharedString,
}

pub struct TextInput {
    focus_handle: FocusHandle,
    content: SharedString,
    placeholder: SharedString,
    selected_range: Range<usize>,
    selection_reversed: bool,
    marked_range: Option<Range<usize>>,
    layout: Option<TextLayout>,
    is_selecting: bool,
    select_mode: SelectMode,
    /// Whether the blinking cursor is in its visible phase.
    cursor_visible: bool,
    is_blinking: bool,
    /// Invalidates pending blinks when blinking restarts or stops.
    blink_epoch: usize,
    /// Starts blinking when focus arrives from elsewhere (registered on first render, which is
    /// the first time a window is at hand).
    focus_subscription: Option<Subscription>,
    /// Shows a bullet for each character, for secrets, and doesn't copy them.
    masked: bool,
    /// With several lines, how many show before it scrolls.
    max_lines: Option<usize>,
    chips: Vec<Chip>,
    next_chip_id: ChipId,
    scroll_top: Pixels,
    /// Scroll the cursor into view at the next layout.
    autoscroll: bool,
    /// Where Up and Down aim, kept across a run of them as in any editor.
    goal_x: Option<Pixels>,
    menu_open: bool,
    handles_paste: bool,
    hovered_chip: Option<(ChipId, Bounds<Pixels>)>,
}

/// What a drag selects by, set by the click that started it (Zed's editor's `SelectMode`).
enum SelectMode {
    Character,
    /// Whole words, from the double-clicked one.
    Word(Range<usize>),
    /// Whole lines, from the triple-clicked one.
    Line(Range<usize>),
    All,
}

/// Zed's `CharKind`, ordered so a word wins over punctuation, and punctuation over whitespace.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum CharKind {
    Whitespace,
    Punctuation,
    Word,
}

impl CharKind {
    fn of(character: char) -> Self {
        if character.is_alphanumeric() || character == '_' {
            Self::Word
        } else if character.is_whitespace() {
            Self::Whitespace
        } else {
            Self::Punctuation
        }
    }
}

/// The text as last laid out.
struct TextLayout {
    bounds: Bounds<Pixels>,
    line_height: Pixels,
    scroll_top: Pixels,
    lines: Vec<LaidOutLine>,
    rows: Vec<Row>,
    chip_bounds: Vec<(ChipId, Bounds<Pixels>)>,
}

/// One line of the shown text, wrapped into rows.
struct LaidOutLine {
    line: WrappedLine,
    /// Where it starts in the shown text.
    start: usize,
    /// Its top, with the first row's at zero.
    top: Pixels,
}

/// One row on screen.
#[derive(Clone, Copy)]
struct Row {
    line: usize,
    /// Its range in the shown text.
    start: usize,
    end: usize,
    top: Pixels,
}

impl TextLayout {
    fn row_x(&self, row: &Row, index: usize) -> Pixels {
        let line = &self.lines[row.line];
        let layout = &line.line.unwrapped_layout;
        layout.x_for_index(index - line.start) - layout.x_for_index(row.start - line.start)
    }

    /// The row an index of the shown text is on: at a wrap, the row it starts.
    fn row_for_index(&self, index: usize) -> usize {
        self.rows
            .iter()
            .position(|row| index >= row.start && index < row.end)
            .or_else(|| self.rows.iter().rposition(|row| index == row.end))
            .unwrap_or(0)
    }

    /// The point of an index of the shown text, with the first row's top at zero.
    fn position_for_index(&self, index: usize) -> Point<Pixels> {
        let Some(row) = self.rows.get(self.row_for_index(index)) else {
            return Point::default();
        };
        point(self.row_x(row, index), row.top)
    }

    /// The index of the shown text closest to a point, with the first row's top at zero.
    fn index_for_position(&self, position: Point<Pixels>) -> usize {
        let Some(row) = self
            .rows
            .iter()
            .find(|row| position.y < row.top + self.line_height)
            .or(self.rows.last())
        else {
            return 0;
        };
        let line = &self.lines[row.line];
        let local = point(
            position.x.max(px(0.)),
            row.top - line.top + self.line_height / 2.,
        );
        let index = match line
            .line
            .closest_index_for_position(local, self.line_height)
        {
            Ok(index) | Err(index) => index,
        };
        (line.start + index).clamp(row.start, row.end)
    }

    fn content_height(&self) -> Pixels {
        self.rows.len() as f32 * self.line_height
    }
}

impl EventEmitter<TextInputEvent> for TextInput {}

impl TextInput {
    pub fn new(placeholder: impl Into<SharedString>, cx: &mut Context<Self>) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            content: SharedString::default(),
            placeholder: placeholder.into(),
            selected_range: 0..0,
            selection_reversed: false,
            marked_range: None,
            layout: None,
            is_selecting: false,
            select_mode: SelectMode::Character,
            cursor_visible: true,
            is_blinking: false,
            blink_epoch: 0,
            focus_subscription: None,
            masked: false,
            max_lines: None,
            chips: Vec::new(),
            next_chip_id: 0,
            scroll_top: px(0.),
            autoscroll: false,
            goal_x: None,
            menu_open: false,
            handles_paste: false,
            hovered_chip: None,
        }
    }

    /// For secrets such as API keys: shows a bullet for each character and doesn't copy them.
    pub fn masked(mut self) -> Self {
        self.masked = true;
        self
    }

    /// Wraps its text, and grows with it up to `max_lines` before scrolling. Shift-Enter makes a
    /// new line, and pasted text keeps its line breaks.
    pub fn multi_line(mut self, max_lines: usize) -> Self {
        self.max_lines = Some(max_lines.max(1));
        self
    }

    /// Leaves pasting to the owner, as [`TextInputEvent::Paste`].
    pub fn handles_paste(mut self) -> Self {
        self.handles_paste = true;
        self
    }

    pub fn is_masked(&self) -> bool {
        self.masked
    }

    /// Shows or hides a masked input's text, as a password field's eye button does.
    pub fn set_masked(&mut self, masked: bool, cx: &mut Context<Self>) {
        self.masked = masked;
        cx.notify();
    }

    /// Whether the owner shows a menu for it, whose keys then come first ([`MENU_CONTEXT`]).
    pub fn set_menu_open(&mut self, menu_open: bool, cx: &mut Context<Self>) {
        if self.menu_open != menu_open {
            self.menu_open = menu_open;
            cx.notify();
        }
    }

    fn is_multi_line(&self) -> bool {
        self.max_lines.is_some()
    }

    /// Where a byte offset into the content falls in the text shown.
    fn display_offset(&self, offset: usize) -> usize {
        if !self.masked {
            return offset;
        }
        self.content[..offset.min(self.content.len())]
            .chars()
            .count()
            * MASK.len_utf8()
    }

    /// The byte offset into the content for one into the text shown.
    fn content_offset(&self, display_offset: usize) -> usize {
        if !self.masked {
            return display_offset;
        }
        self.content
            .char_indices()
            .nth(display_offset / MASK.len_utf8())
            .map_or(self.content.len(), |(offset, _)| offset)
    }

    pub fn set_placeholder(
        &mut self,
        placeholder: impl Into<SharedString>,
        cx: &mut Context<Self>,
    ) {
        self.placeholder = placeholder.into();
        cx.notify();
    }

    /// The text, chips included as their text.
    pub fn text(&self) -> &SharedString {
        &self.content
    }

    /// The text with each chip as what copying it gives.
    pub fn plain_text(&self) -> String {
        self.text_for_copy(0..self.content.len())
    }

    pub fn chips(&self) -> &[Chip] {
        &self.chips
    }

    /// Where the cursor is, as a byte offset.
    pub fn cursor_offset(&self) -> usize {
        if self.selection_reversed {
            self.selected_range.start
        } else {
            self.selected_range.end
        }
    }

    pub fn set_text(&mut self, text: impl Into<SharedString>, cx: &mut Context<Self>) {
        self.content = text.into();
        self.chips.clear();
        self.selected_range = self.content.len()..self.content.len();
        self.selection_reversed = false;
        self.marked_range = None;
        self.autoscroll = true;
        cx.emit(TextInputEvent::Changed);
        self.pause_blinking(cx);
    }

    pub fn select_all_text(&mut self, cx: &mut Context<Self>) {
        self.move_to(0, cx);
        self.select_to(self.content.len(), cx)
    }

    /// Types `text` in place of the selection.
    pub fn insert(&mut self, text: &str, cx: &mut Context<Self>) {
        let text = if self.is_multi_line() {
            text.replace("\r\n", "\n")
        } else {
            text.replace(['\r', '\n'], " ")
        };
        self.marked_range = None;
        let range = self.replace_range(self.selected_range.clone(), &text);
        let cursor = range.start + text.len();
        self.selected_range = cursor..cursor;
        self.selection_reversed = false;
        cx.emit(TextInputEvent::Changed);
        self.pause_blinking(cx);
    }

    pub fn has_selection(&self) -> bool {
        !self.selected_range.is_empty()
    }

    /// Puts a chip, then a space, in place of `range` (or the selection), and the cursor after
    /// them.
    pub fn insert_chip(
        &mut self,
        range: Option<Range<usize>>,
        label: &str,
        icon: SharedString,
        preview: ChipPreview,
        copy_text: SharedString,
        cx: &mut Context<Self>,
    ) -> ChipId {
        let mut label: String = label
            .chars()
            .map(|character| {
                if character.is_whitespace() {
                    '\u{a0}'
                } else {
                    character
                }
            })
            .collect();
        if label.chars().count() > CHIP_LABEL_MAX_CHARS {
            label = label.chars().take(CHIP_LABEL_MAX_CHARS - 1).collect();
            label.push('…');
        }
        let chip_text = format!("{CHIP_ICON_ROOM}{label}{CHIP_END}");
        let range = range.unwrap_or(self.selected_range.clone());
        self.marked_range = None;
        self.replace_range(range.clone(), &format!("{chip_text} "));
        let id = self.next_chip_id;
        self.next_chip_id += 1;
        let chip_range = range.start..range.start + chip_text.len();
        let index = self
            .chips
            .iter()
            .position(|chip| chip.range.start > chip_range.start)
            .unwrap_or(self.chips.len());
        self.chips.insert(
            index,
            Chip {
                id,
                range: chip_range,
                icon,
                preview,
                copy_text,
            },
        );
        let cursor = range.start + chip_text.len() + 1;
        self.selected_range = cursor..cursor;
        self.selection_reversed = false;
        self.autoscroll = true;
        cx.emit(TextInputEvent::Changed);
        self.pause_blinking(cx);
        id
    }

    /// Replaces `range` with `new_text`, taking whole any chip it touches, and moves the chips
    /// after it. Leaves the selection to the caller.
    fn replace_range(&mut self, range: Range<usize>, new_text: &str) -> Range<usize> {
        let mut range = range;
        let mut removed = Vec::new();
        for chip in &self.chips {
            let overlaps = chip.range.start < range.end && range.start < chip.range.end;
            let inside =
                range.is_empty() && range.start > chip.range.start && range.start < chip.range.end;
            if overlaps || inside {
                range.start = range.start.min(chip.range.start);
                range.end = range.end.max(chip.range.end);
                removed.push(chip.id);
            }
        }
        self.chips.retain(|chip| !removed.contains(&chip.id));
        self.content =
            (self.content[..range.start].to_owned() + new_text + &self.content[range.end..]).into();
        let removed_len = range.end - range.start;
        for chip in &mut self.chips {
            if chip.range.start >= range.end {
                chip.range.start = chip.range.start - removed_len + new_text.len();
                chip.range.end = chip.range.end - removed_len + new_text.len();
            }
        }
        self.goal_x = None;
        self.autoscroll = true;
        range
    }

    /// The text of `range`, with each chip as what copying it gives.
    fn text_for_copy(&self, range: Range<usize>) -> String {
        let mut text = String::new();
        let mut index = range.start;
        for chip in &self.chips {
            if chip.range.end <= range.start || chip.range.start >= range.end {
                continue;
            }
            text.push_str(&self.content[index..chip.range.start.max(index)]);
            text.push_str(&chip.copy_text);
            index = chip.range.end.min(range.end).max(index);
        }
        if index < range.end {
            text.push_str(&self.content[index..range.end]);
        }
        text
    }

    /// The chip an offset is strictly inside of.
    fn chip_around(&self, offset: usize) -> Option<&Chip> {
        self.chips
            .iter()
            .find(|chip| offset > chip.range.start && offset < chip.range.end)
    }

    /// An offset moved out of any chip, to its nearer end.
    fn snap(&self, offset: usize) -> usize {
        match self.chip_around(offset) {
            Some(chip) if offset - chip.range.start < chip.range.end - offset => chip.range.start,
            Some(chip) => chip.range.end,
            None => offset,
        }
    }

    /// The word, run of punctuation or run of spaces around an offset, which a double-click
    /// selects (Zed's `surrounding_word`). A chip is a word of its own.
    fn surrounding_word(&self, offset: usize) -> Range<usize> {
        if let Some(chip) = self.chip_around(offset) {
            return chip.range.clone();
        }
        let chip_starts_at =
            |offset: usize| self.chips.iter().any(|chip| chip.range.start == offset);
        let chip_ends_at = |offset: usize| self.chips.iter().any(|chip| chip.range.end == offset);
        let before = self.content[..offset]
            .chars()
            .next_back()
            .filter(|_| !chip_ends_at(offset))
            .map(CharKind::of);
        let after = self.content[offset..]
            .chars()
            .next()
            .filter(|_| !chip_starts_at(offset))
            .map(CharKind::of);
        let kind = before.max(after);
        let mut start = offset;
        for character in self.content[..offset].chars().rev() {
            if chip_ends_at(start) || character == '\n' || Some(CharKind::of(character)) != kind {
                break;
            }
            start -= character.len_utf8();
        }
        let mut end = offset;
        for character in self.content[offset..].chars() {
            if chip_starts_at(end) || character == '\n' || Some(CharKind::of(character)) != kind {
                break;
            }
            end += character.len_utf8();
        }
        start..end
    }

    fn is_inside_word(&self, offset: usize) -> bool {
        let before = self.content[..offset].chars().next_back();
        let after = self.content[offset..].chars().next();
        self.chip_around(offset).is_some()
            || before
                .zip(after)
                .map(|(before, after)| (CharKind::of(before), CharKind::of(after)))
                == Some((CharKind::Word, CharKind::Word))
    }

    /// The line around an offset with its line break, which a triple-click selects.
    fn line_range(&self, offset: usize) -> Range<usize> {
        let start = self.content[..offset]
            .rfind('\n')
            .map_or(0, |index| index + 1);
        let end = self.content[offset..]
            .find('\n')
            .map_or(self.content.len(), |index| offset + index + 1);
        start..end
    }

    fn left(&mut self, _: &Left, _: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            self.move_to(self.previous_boundary(self.cursor_offset()), cx);
        } else {
            self.move_to(self.selected_range.start, cx)
        }
    }

    fn right(&mut self, _: &Right, _: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            self.move_to(self.next_boundary(self.selected_range.end), cx);
        } else {
            self.move_to(self.selected_range.end, cx)
        }
    }

    fn up(&mut self, _: &Up, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(offset) = self.vertical_target(-1) {
            self.move_vertically(offset, false, cx);
        }
    }

    fn down(&mut self, _: &Down, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(offset) = self.vertical_target(1) {
            self.move_vertically(offset, false, cx);
        }
    }

    fn select_up(&mut self, _: &SelectUp, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(offset) = self.vertical_target(-1) {
            self.move_vertically(offset, true, cx);
        }
    }

    fn select_down(&mut self, _: &SelectDown, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(offset) = self.vertical_target(1) {
            self.move_vertically(offset, true, cx);
        }
    }

    /// Where the cursor goes a row up (`-1`) or down (`1`): the start from the first row, the
    /// end from the last.
    fn vertical_target(&mut self, step: isize) -> Option<usize> {
        let layout = self.layout.as_ref()?;
        let cursor = self.display_offset(self.cursor_offset());
        let row = layout.row_for_index(cursor);
        let goal_x = *self
            .goal_x
            .get_or_insert_with(|| layout.position_for_index(cursor).x);
        let target_row = row as isize + step;
        if target_row < 0 {
            return Some(0);
        }
        let Some(target) = layout.rows.get(target_row as usize) else {
            return Some(self.content.len());
        };
        let index = layout.index_for_position(point(goal_x, target.top));
        Some(self.snap(self.content_offset(index)))
    }

    fn move_vertically(&mut self, offset: usize, select: bool, cx: &mut Context<Self>) {
        let goal_x = self.goal_x;
        if select {
            self.select_to(offset, cx);
        } else {
            self.move_to(offset, cx);
        }
        self.goal_x = goal_x;
    }

    fn select_left(&mut self, _: &SelectLeft, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.previous_boundary(self.cursor_offset()), cx);
    }

    fn select_right(&mut self, _: &SelectRight, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.next_boundary(self.cursor_offset()), cx);
    }

    fn select_all(&mut self, _: &SelectAll, _: &mut Window, cx: &mut Context<Self>) {
        self.select_all_text(cx);
    }

    /// The start of the cursor's row, or of the text in one line.
    fn home(&mut self, _: &Home, _: &mut Window, cx: &mut Context<Self>) {
        let offset = self.row_bounds().map_or(0, |row| row.start);
        self.move_to(offset, cx);
    }

    fn end(&mut self, _: &End, _: &mut Window, cx: &mut Context<Self>) {
        let offset = self.row_bounds().map_or(self.content.len(), |row| row.end);
        self.move_to(offset, cx);
    }

    /// The cursor's row as content offsets, with several lines. A wrapped row ends before the
    /// space it wrapped at.
    fn row_bounds(&self) -> Option<Range<usize>> {
        if !self.is_multi_line() {
            return None;
        }
        let layout = self.layout.as_ref()?;
        let row = layout
            .rows
            .get(layout.row_for_index(self.cursor_offset()))?;
        let text = &self.content[row.start..row.end];
        let end = row.start + text.trim_end_matches(['\n', ' ']).len();
        Some(row.start..end.max(row.start))
    }

    fn newline(&mut self, _: &Newline, window: &mut Window, cx: &mut Context<Self>) {
        if self.is_multi_line() {
            self.replace_text_in_range(None, "\n", window, cx);
        }
    }

    fn backspace(&mut self, _: &Backspace, window: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            let previous = self.previous_boundary(self.cursor_offset());
            if self.cursor_offset() == previous {
                window.play_system_bell();
                return;
            }
            self.select_to(previous, cx)
        }
        self.replace_text_in_range(None, "", window, cx)
    }

    fn delete(&mut self, _: &Delete, window: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            let next = self.next_boundary(self.cursor_offset());
            if self.cursor_offset() == next {
                window.play_system_bell();
                return;
            }
            self.select_to(next, cx)
        }
        self.replace_text_in_range(None, "", window, cx)
    }

    fn on_mouse_down(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let offset = self.index_for_mouse_position(event.position);
        if event.button == MouseButton::Right {
            // Zed's editor: a right-click outside the selection moves the cursor there first.
            if !(self.selected_range.start..=self.selected_range.end).contains(&offset)
                || self.selected_range.is_empty()
            {
                self.move_to(offset, cx);
            }
            window.focus(&self.focus_handle, cx);
            if self.is_multi_line() {
                cx.emit(TextInputEvent::ContextMenu(event.position));
            }
            return;
        }
        self.is_selecting = true;
        match event.click_count {
            0 | 1 => {
                self.select_mode = SelectMode::Character;
                if event.modifiers.shift {
                    self.select_to(offset, cx);
                } else {
                    self.move_to(offset, cx)
                }
            }
            2 => {
                // A chip's edges sit between characters, so a click on its half nearer the
                // text beside it would find that text's word.
                let range = match self.chip_at_position(event.position) {
                    Some((id, _)) => self
                        .chips
                        .iter()
                        .find(|chip| chip.id == id)
                        .map_or_else(|| self.surrounding_word(offset), |chip| chip.range.clone()),
                    None => self.surrounding_word(offset),
                };
                self.select_between(range.start, range.end, cx);
                self.select_mode = SelectMode::Word(range);
            }
            3 => {
                let range = self.line_range(offset);
                self.select_between(range.start, range.end, cx);
                self.select_mode = SelectMode::Line(range);
            }
            _ => {
                self.select_all_text(cx);
                self.select_mode = SelectMode::All;
            }
        }
    }

    fn on_mouse_up(&mut self, _: &MouseUpEvent, _window: &mut Window, _: &mut Context<Self>) {
        self.is_selecting = false;
    }

    fn on_mouse_move(&mut self, event: &MouseMoveEvent, _: &mut Window, cx: &mut Context<Self>) {
        if self.is_selecting {
            self.extend_selection(self.index_for_mouse_position(event.position), cx);
        }
        let hovered = self.chip_at_position(event.position);
        if hovered.map(|(id, _)| id) != self.hovered_chip.map(|(id, _)| id) {
            self.hovered_chip = hovered;
            cx.notify();
        }
    }

    fn on_scroll_wheel(
        &mut self,
        event: &ScrollWheelEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(layout) = &self.layout else {
            return;
        };
        let overflow = layout.content_height() - layout.bounds.size.height;
        if overflow <= px(0.) {
            return;
        }
        let delta = event.delta.pixel_delta(layout.line_height);
        self.scroll_top = (self.scroll_top - delta.y).clamp(px(0.), overflow);
        cx.stop_propagation();
        cx.notify();
    }

    fn show_character_palette(
        &mut self,
        _: &ShowCharacterPalette,
        window: &mut Window,
        _: &mut Context<Self>,
    ) {
        window.show_character_palette();
    }

    fn paste(&mut self, _: &Paste, window: &mut Window, cx: &mut Context<Self>) {
        self.paste_clipboard(false, window, cx);
    }

    fn paste_raw(&mut self, _: &PasteRaw, window: &mut Window, cx: &mut Context<Self>) {
        self.paste_clipboard(true, window, cx);
    }

    fn paste_clipboard(&mut self, plain: bool, _: &mut Window, cx: &mut Context<Self>) {
        let Some(item) = cx.read_from_clipboard() else {
            return;
        };
        if self.handles_paste {
            cx.emit(TextInputEvent::Paste { item, plain });
        } else if let Some(text) = item.text() {
            self.insert(&text, cx);
        }
    }

    fn copy(&mut self, _: &Copy, _: &mut Window, cx: &mut Context<Self>) {
        if !self.selected_range.is_empty() && !self.masked {
            cx.write_to_clipboard(ClipboardItem::new_string(
                self.text_for_copy(self.selected_range.clone()),
            ));
        }
    }

    fn cut(&mut self, _: &Cut, window: &mut Window, cx: &mut Context<Self>) {
        if !self.selected_range.is_empty() && !self.masked {
            cx.write_to_clipboard(ClipboardItem::new_string(
                self.text_for_copy(self.selected_range.clone()),
            ));
            self.replace_text_in_range(None, "", window, cx)
        }
    }

    fn move_to(&mut self, offset: usize, cx: &mut Context<Self>) {
        let offset = self.snap(offset);
        self.selected_range = offset..offset;
        self.selection_reversed = false;
        self.goal_x = None;
        self.autoscroll = true;
        self.pause_blinking(cx);
    }

    /// Like Zed's editor: the cursor stays solid while it's being moved or typed with, and
    /// blinking resumes a blink interval later.
    fn pause_blinking(&mut self, cx: &mut Context<Self>) {
        self.cursor_visible = true;
        self.is_blinking = false;
        self.blink_epoch += 1;
        cx.notify();
    }

    fn schedule_blink(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.is_blinking = true;
        self.blink_epoch += 1;
        let epoch = self.blink_epoch;
        cx.spawn_in(window, async move |this, cx| {
            cx.background_executor().timer(CURSOR_BLINK_INTERVAL).await;
            this.update_in(cx, |this, window, cx| this.blink(epoch, window, cx))
                .ok();
        })
        .detach();
    }

    fn blink(&mut self, epoch: usize, window: &mut Window, cx: &mut Context<Self>) {
        if epoch != self.blink_epoch {
            return;
        }
        if !self.focus_handle.is_focused(window) {
            self.cursor_visible = true;
            self.is_blinking = false;
            return;
        }
        self.cursor_visible = !self.cursor_visible;
        cx.notify();
        self.schedule_blink(window, cx);
    }

    /// The content offset closest to a point, which may be inside a chip.
    fn index_for_mouse_position(&self, position: Point<Pixels>) -> usize {
        if self.content.is_empty() {
            return 0;
        }
        let Some(layout) = self.layout.as_ref() else {
            return 0;
        };
        let bounds = layout.bounds;
        if !self.is_multi_line() {
            if position.y < bounds.top() {
                return 0;
            }
            if position.y > bounds.bottom() {
                return self.content.len();
            }
        }
        let local = point(
            position.x - bounds.left(),
            position.y - bounds.top() + layout.scroll_top,
        );
        self.content_offset(layout.index_for_position(local))
    }

    fn chip_at_position(&self, position: Point<Pixels>) -> Option<(ChipId, Bounds<Pixels>)> {
        self.layout.as_ref().and_then(|layout| {
            layout
                .chip_bounds
                .iter()
                .find(|(_, bounds)| bounds.contains(&position))
                .copied()
        })
    }

    /// Drags the selection's head to `offset`, by what the click that started it selected, as
    /// Zed's editor's `update_selection` does.
    fn extend_selection(&mut self, offset: usize, cx: &mut Context<Self>) {
        let (head, original) = match &self.select_mode {
            SelectMode::Character => {
                self.select_to(offset, cx);
                return;
            }
            SelectMode::Word(original) => {
                let head = if self.is_inside_word(offset) || original.contains(&offset) {
                    let word = self.surrounding_word(offset);
                    if word.start < original.start {
                        word.start
                    } else {
                        word.end
                    }
                } else {
                    self.snap(offset)
                };
                (head, original.clone())
            }
            SelectMode::Line(original) => {
                let line = self.line_range(offset);
                let head = if line.start < original.start {
                    line.start
                } else {
                    line.end
                };
                (head, original.clone())
            }
            SelectMode::All => return,
        };
        let tail = if head <= original.start {
            original.end
        } else {
            original.start
        };
        self.select_between(tail, head, cx);
    }

    /// Selects from `tail` to `head`, where the cursor ends up.
    fn select_between(&mut self, tail: usize, head: usize, cx: &mut Context<Self>) {
        if head < tail {
            self.selected_range = head..tail;
            self.selection_reversed = true;
        } else {
            self.selected_range = tail..head;
            self.selection_reversed = false;
        }
        self.goal_x = None;
        self.autoscroll = true;
        self.pause_blinking(cx);
    }

    fn select_to(&mut self, offset: usize, cx: &mut Context<Self>) {
        let offset = self.snap(offset);
        if self.selection_reversed {
            self.selected_range.start = offset
        } else {
            self.selected_range.end = offset
        };
        if self.selected_range.end < self.selected_range.start {
            self.selection_reversed = !self.selection_reversed;
            self.selected_range = self.selected_range.end..self.selected_range.start;
        }
        self.goal_x = None;
        self.autoscroll = true;
        self.pause_blinking(cx);
    }

    fn offset_from_utf16(&self, offset: usize) -> usize {
        let mut utf8_offset = 0;
        let mut utf16_count = 0;

        for character in self.content.chars() {
            if utf16_count >= offset {
                break;
            }
            utf16_count += character.len_utf16();
            utf8_offset += character.len_utf8();
        }

        utf8_offset
    }

    fn offset_to_utf16(&self, offset: usize) -> usize {
        let mut utf16_offset = 0;
        let mut utf8_count = 0;

        for character in self.content.chars() {
            if utf8_count >= offset {
                break;
            }
            utf8_count += character.len_utf8();
            utf16_offset += character.len_utf16();
        }

        utf16_offset
    }

    fn range_to_utf16(&self, range: &Range<usize>) -> Range<usize> {
        self.offset_to_utf16(range.start)..self.offset_to_utf16(range.end)
    }

    fn range_from_utf16(&self, range_utf16: &Range<usize>) -> Range<usize> {
        self.offset_from_utf16(range_utf16.start)..self.offset_from_utf16(range_utf16.end)
    }

    /// The boundary before an offset: a chip's start from its end.
    fn previous_boundary(&self, offset: usize) -> usize {
        if let Some(chip) = self
            .chips
            .iter()
            .find(|chip| offset > chip.range.start && offset <= chip.range.end)
        {
            return chip.range.start;
        }
        self.content
            .grapheme_indices(true)
            .rev()
            .find_map(|(index, _)| (index < offset).then_some(index))
            .unwrap_or(0)
    }

    /// The boundary after an offset: a chip's end from its start.
    fn next_boundary(&self, offset: usize) -> usize {
        if let Some(chip) = self
            .chips
            .iter()
            .find(|chip| offset >= chip.range.start && offset < chip.range.end)
        {
            return chip.range.end;
        }
        self.content
            .grapheme_indices(true)
            .find_map(|(index, _)| (index > offset).then_some(index))
            .unwrap_or(self.content.len())
    }
}

impl EntityInputHandler for TextInput {
    fn text_for_range(
        &mut self,
        range_utf16: Range<usize>,
        actual_range: &mut Option<Range<usize>>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<String> {
        let range = self.range_from_utf16(&range_utf16);
        actual_range.replace(self.range_to_utf16(&range));
        self.content.get(range).map(str::to_string)
    }

    fn selected_text_range(
        &mut self,
        _ignore_disabled_input: bool,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        Some(UTF16Selection {
            range: self.range_to_utf16(&self.selected_range),
            reversed: self.selection_reversed,
        })
    }

    fn marked_text_range(
        &self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Range<usize>> {
        self.marked_range
            .as_ref()
            .map(|range| self.range_to_utf16(range))
    }

    fn unmark_text(&mut self, _window: &mut Window, _cx: &mut Context<Self>) {
        self.marked_range = None;
    }

    fn replace_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let range = range_utf16
            .as_ref()
            .map(|range_utf16| self.range_from_utf16(range_utf16))
            .or(self.marked_range.clone())
            .unwrap_or(self.selected_range.clone());
        let range = self.replace_range(range, new_text);
        self.selected_range = range.start + new_text.len()..range.start + new_text.len();
        self.selection_reversed = false;
        self.marked_range.take();
        cx.emit(TextInputEvent::Changed);
        self.pause_blinking(cx);
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        new_selected_range_utf16: Option<Range<usize>>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let range = range_utf16
            .as_ref()
            .map(|range_utf16| self.range_from_utf16(range_utf16))
            .or(self.marked_range.clone())
            .unwrap_or(self.selected_range.clone());
        let range = self.replace_range(range, new_text);
        if !new_text.is_empty() {
            self.marked_range = Some(range.start..range.start + new_text.len());
        } else {
            self.marked_range = None;
        }
        self.selected_range = new_selected_range_utf16
            .as_ref()
            .map(|range_utf16| self.range_from_utf16(range_utf16))
            .map(|new_range| new_range.start + range.start..new_range.end + range.start)
            .unwrap_or_else(|| range.start + new_text.len()..range.start + new_text.len());

        cx.emit(TextInputEvent::Changed);
        self.pause_blinking(cx);
    }

    fn bounds_for_range(
        &mut self,
        range_utf16: Range<usize>,
        _bounds: Bounds<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let layout = self.layout.as_ref()?;
        let range = self.range_from_utf16(&range_utf16);
        let origin = layout.bounds.origin - point(px(0.), layout.scroll_top);
        let start = layout.position_for_index(self.display_offset(range.start));
        let end = layout.position_for_index(self.display_offset(range.end));
        Some(Bounds::from_corners(
            origin + start,
            origin + point(end.x, end.y + layout.line_height),
        ))
    }

    fn character_index_for_point(
        &mut self,
        point: Point<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<usize> {
        let layout = self.layout.as_ref()?;
        let local = point - layout.bounds.origin + gpui::point(px(0.), layout.scroll_top);
        let index = self.content_offset(layout.index_for_position(local));
        Some(self.offset_to_utf16(index))
    }
}

struct TextElement {
    input: Entity<TextInput>,
}

/// What the shown text is made of for a layout.
struct Shaping {
    text: SharedString,
    runs: Vec<TextRun>,
    font_size: Pixels,
    line_height: Pixels,
    is_placeholder: bool,
}

struct PrepaintState {
    layout: TextLayout,
    cursor: Option<PaintQuad>,
    selection: Vec<PaintQuad>,
    chips: Vec<(PaintQuad, Option<(Bounds<Pixels>, SharedString)>)>,
    scrollbar: Option<PaintQuad>,
    is_placeholder: bool,
}

impl TextElement {
    fn shaping(input: &TextInput, window: &Window, cx: &App) -> Shaping {
        let style = window.text_style();
        let colors = cx.theme().colors();
        let content = if input.masked {
            SharedString::from(MASK.to_string().repeat(input.content.chars().count()))
        } else {
            input.content.clone()
        };
        let is_placeholder = content.is_empty();
        let (text, color) = if is_placeholder {
            (input.placeholder.clone(), colors.text_placeholder)
        } else {
            (content, style.color)
        };
        let base = TextRun {
            len: text.len(),
            font: style.font(),
            color,
            background_color: None,
            underline: None,
            strikethrough: None,
        };
        let runs = if is_placeholder {
            vec![base]
        } else {
            let chip_font = gpui::Font {
                family: theme::theme_settings(cx).buffer_font(cx).family.clone(),
                ..style.font()
            };
            let marked = input
                .marked_range
                .as_ref()
                .map(|range| input.display_offset(range.start)..input.display_offset(range.end));
            runs_for(&text, &base, &chip_font, &input.chips, marked)
        };
        Shaping {
            text,
            runs,
            font_size: style.font_size.to_pixels(window.rem_size()),
            line_height: window.line_height(),
            is_placeholder,
        }
    }
}

/// The runs for the shown text: chips in the code font, and marked text underlined.
fn runs_for(
    text: &str,
    base: &TextRun,
    chip_font: &gpui::Font,
    chips: &[Chip],
    marked: Option<Range<usize>>,
) -> Vec<TextRun> {
    let mut breaks = vec![0, text.len()];
    for chip in chips {
        breaks.extend([chip.range.start, chip.range.end]);
    }
    if let Some(marked) = &marked {
        breaks.extend([marked.start, marked.end]);
    }
    breaks.retain(|offset| *offset <= text.len());
    breaks.sort_unstable();
    breaks.dedup();
    breaks
        .windows(2)
        .map(|window| {
            let range = window[0]..window[1];
            let in_chip = chips
                .iter()
                .any(|chip| chip.range.start <= range.start && range.end <= chip.range.end);
            let is_marked = marked
                .as_ref()
                .is_some_and(|marked| marked.start <= range.start && range.end <= marked.end);
            TextRun {
                len: range.end - range.start,
                font: if in_chip {
                    chip_font.clone()
                } else {
                    base.font.clone()
                },
                underline: is_marked.then(|| UnderlineStyle {
                    color: Some(base.color),
                    thickness: px(1.0),
                    wavy: false,
                }),
                ..base.clone()
            }
        })
        .filter(|run| run.len > 0)
        .collect()
}

/// Lays out the shown text, `wrap_width` wide with several lines.
fn lay_out(
    shaping: &Shaping,
    wrap_width: Option<Pixels>,
    bounds: Bounds<Pixels>,
    scroll_top: Pixels,
    window: &mut Window,
) -> TextLayout {
    let shaped = window
        .text_system()
        .shape_text(
            shaping.text.clone(),
            shaping.font_size,
            &shaping.runs,
            wrap_width,
            None,
        )
        .unwrap_or_default();
    let mut lines = Vec::new();
    let mut rows = Vec::new();
    let mut start = 0;
    let mut top = px(0.);
    for line in shaped {
        let wraps = line.wrap_boundaries().len();
        let mut row_start = start;
        for (index, boundary) in line.wrap_boundaries().iter().enumerate() {
            let glyph = &line.unwrapped_layout.runs[boundary.run_ix].glyphs[boundary.glyph_ix];
            let row_end = start + glyph.index;
            rows.push(Row {
                line: lines.len(),
                start: row_start,
                end: row_end,
                top: top + index as f32 * shaping.line_height,
            });
            row_start = row_end;
        }
        rows.push(Row {
            line: lines.len(),
            start: row_start,
            end: start + line.len(),
            top: top + wraps as f32 * shaping.line_height,
        });
        let len = line.len();
        lines.push(LaidOutLine { line, start, top });
        top += (wraps + 1) as f32 * shaping.line_height;
        // The `\n` after it.
        start += len + 1;
    }
    TextLayout {
        bounds,
        line_height: shaping.line_height,
        scroll_top,
        lines,
        rows,
        chip_bounds: Vec::new(),
    }
}

impl IntoElement for TextElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for TextElement {
    type RequestLayoutState = ();
    type PrepaintState = PrepaintState;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let mut style = Style::default();
        style.size.width = relative(1.).into();
        let input = self.input.read(cx);
        let Some(max_lines) = input.max_lines else {
            style.size.height = window.line_height().into();
            return (window.request_layout(style, [], cx), ());
        };
        let shaping = TextElement::shaping(input, window, cx);
        // As tall as its rows, up to `max_lines`, for the width it gets.
        let layout_id =
            window.request_measured_layout(style, move |known, available, window, _| {
                let width = known.width.or(match available.width {
                    gpui::AvailableSpace::Definite(width) => Some(width),
                    _ => None,
                });
                let rows = width
                    .map(|width| {
                        window
                            .text_system()
                            .shape_text(
                                shaping.text.clone(),
                                shaping.font_size,
                                &shaping.runs,
                                Some(width),
                                None,
                            )
                            .map(|lines| {
                                lines
                                    .iter()
                                    .map(|line| line.wrap_boundaries().len() + 1)
                                    .sum::<usize>()
                            })
                            .unwrap_or(1)
                    })
                    .unwrap_or(1);
                size(
                    width.unwrap_or_default(),
                    rows.clamp(1, max_lines) as f32 * shaping.line_height,
                )
            });
        (layout_id, ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        let input = self.input.read(cx);
        let shaping = TextElement::shaping(input, window, cx);
        let wrap_width = input.max_lines.map(|_| bounds.size.width);
        let mut layout = lay_out(&shaping, wrap_width, bounds, input.scroll_top, window);
        let line_height = layout.line_height;

        // Keep the cursor in view, within what can scroll.
        let overflow = (layout.content_height() - bounds.size.height).max(px(0.));
        let mut scroll_top = input.scroll_top.min(overflow);
        if input.autoscroll && !shaping.is_placeholder {
            let cursor_top = layout
                .position_for_index(input.display_offset(input.cursor_offset()))
                .y;
            if cursor_top < scroll_top {
                scroll_top = cursor_top;
            } else if cursor_top + line_height > scroll_top + bounds.size.height {
                scroll_top = cursor_top + line_height - bounds.size.height;
            }
            scroll_top = scroll_top.clamp(px(0.), overflow);
        }
        layout.scroll_top = scroll_top;
        let origin = bounds.origin - point(px(0.), scroll_top);
        let colors = cx.theme().colors();
        let local_player = cx.theme().players().local();

        let row_rect = |row: &Row, start_x: Pixels, end_x: Pixels| {
            Bounds::from_corners(
                origin + point(start_x, row.top),
                origin + point(end_x, row.top + line_height),
            )
        };
        let mut selection = Vec::new();
        let mut cursor = None;
        if shaping.is_placeholder {
            cursor = Some(fill(
                Bounds::new(bounds.origin, size(px(1.5), line_height)),
                local_player.cursor,
            ));
        } else {
            let start = input.display_offset(input.selected_range.start);
            let end = input.display_offset(input.selected_range.end);
            if start == end {
                let position = layout.position_for_index(start);
                cursor = Some(fill(
                    Bounds::new(origin + position, size(px(1.5), line_height)),
                    local_player.cursor,
                ));
            } else {
                for row in &layout.rows {
                    if row.end < start
                        || row.start > end
                        || (row.end == start && row.end != row.start)
                    {
                        continue;
                    }
                    let start_x = layout.row_x(row, start.max(row.start));
                    let mut end_x = layout.row_x(row, end.min(row.end));
                    // A selected line break shows as a little room at the row's end.
                    if end > row.end {
                        end_x += px(4.);
                    }
                    selection.push(fill(row_rect(row, start_x, end_x), local_player.selection));
                }
            }
        }

        let mut chips = Vec::new();
        if !shaping.is_placeholder {
            for chip in &input.chips {
                let row_index = layout.row_for_index(chip.range.start);
                let Some(row) = layout.rows.get(row_index).copied() else {
                    continue;
                };
                let start_x = layout.row_x(&row, chip.range.start);
                let end_x = if chip.range.end <= row.end {
                    layout.row_x(&row, chip.range.end)
                } else {
                    layout.row_x(&row, row.end)
                };
                let rect = row_rect(&row, start_x, end_x);
                let rect = Bounds::from_corners(
                    rect.origin + point(px(0.), px(1.)),
                    rect.bottom_right() - point(px(2.), px(1.)),
                );
                layout.chip_bounds.push((chip.id, rect));
                let icon_size = px(12.);
                let icon = Bounds::new(
                    point(
                        rect.left() + px(5.),
                        rect.top() + (rect.size.height - icon_size) / 2.,
                    ),
                    size(icon_size, icon_size),
                );
                chips.push((
                    quad(
                        rect,
                        px(4.),
                        gpui::transparent_black(),
                        px(1.),
                        colors.border,
                        Default::default(),
                    ),
                    Some((icon, chip.icon.clone())),
                ));
            }
        }

        let scrollbar = (overflow > px(0.)).then(|| {
            let visible = bounds.size.height / layout.content_height();
            let height = (bounds.size.height * visible).max(px(12.));
            let top = (bounds.size.height - height) * (scroll_top / overflow);
            quad(
                Bounds::new(
                    point(bounds.right() + px(4.), bounds.top() + top),
                    size(px(3.), height),
                ),
                px(1.5),
                colors.scrollbar_thumb_background,
                px(0.),
                gpui::transparent_black(),
                Default::default(),
            )
        });

        PrepaintState {
            layout,
            cursor,
            selection,
            chips,
            scrollbar,
            is_placeholder: shaping.is_placeholder,
        }
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let focus_handle = self.input.read(cx).focus_handle.clone();
        window.handle_input(
            &focus_handle,
            ElementInputHandler::new(bounds, self.input.clone()),
            cx,
        );
        let layout = &prepaint.layout;
        let origin = bounds.origin - point(px(0.), layout.scroll_top);
        let icon_color: Hsla = cx.theme().colors().icon_muted;
        window.with_content_mask(Some(ContentMask { bounds }), |window| {
            for selection in prepaint.selection.drain(..) {
                window.paint_quad(selection);
            }
            for (chip, _) in &prepaint.chips {
                window.paint_quad(chip.clone());
            }
            for line in &layout.lines {
                if let Err(error) = line.line.paint(
                    origin + point(px(0.), line.top),
                    layout.line_height,
                    TextAlign::Left,
                    None,
                    window,
                    cx,
                ) {
                    log::error!("failed to paint text input: {error:#}");
                }
            }
            for (_, icon) in &prepaint.chips {
                if let Some((bounds, path)) = icon
                    && let Err(error) = window.paint_svg(
                        *bounds,
                        path.clone(),
                        None,
                        TransformationMatrix::unit(),
                        icon_color,
                        cx,
                    )
                {
                    log::error!("failed to paint a chip's icon: {error:#}");
                }
            }
            if focus_handle.is_focused(window)
                && self.input.read(cx).cursor_visible
                && let Some(cursor) = prepaint.cursor.take()
            {
                window.paint_quad(cursor);
            }
        });
        if let Some(scrollbar) = prepaint.scrollbar.take() {
            window.paint_quad(scrollbar);
        }

        let is_placeholder = prepaint.is_placeholder;
        let empty = TextLayout {
            bounds,
            line_height: prepaint.layout.line_height,
            scroll_top: prepaint.layout.scroll_top,
            lines: Vec::new(),
            rows: Vec::new(),
            chip_bounds: Vec::new(),
        };
        let layout = std::mem::replace(&mut prepaint.layout, empty);
        self.input.update(cx, |input, _cx| {
            input.scroll_top = layout.scroll_top;
            if !is_placeholder {
                input.autoscroll = false;
            }
            input.layout = Some(layout);
        });
    }
}

impl TextInput {
    /// What hovering a chip shows, above it.
    fn render_chip_preview(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let (id, bounds) = self.hovered_chip?;
        let chip = self.chips.iter().find(|chip| chip.id == id)?;
        let colors = cx.theme().colors();
        let content = match &chip.preview {
            ChipPreview::Text(text) => div()
                .px_2()
                .py_1()
                .text_sm()
                .text_color(colors.text)
                .child(text.clone())
                .into_any_element(),
            ChipPreview::Image(image) => div()
                .p_1()
                .child(img(image.clone()).max_w(px(320.)).max_h(px(240.)))
                .into_any_element(),
        };
        Some(
            deferred(
                anchored()
                    .position(point(bounds.left(), bounds.top() - px(4.)))
                    .anchor(Anchor::BottomLeft)
                    .snap_to_window_with_margin(px(8.))
                    .child(
                        div()
                            .bg(colors.elevated_surface_background)
                            .border_1()
                            .border_color(colors.border)
                            .rounded_md()
                            .shadow_md()
                            .child(content),
                    ),
            )
            .with_priority(1)
            .into_any_element(),
        )
    }
}

impl Render for TextInput {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.focus_subscription.is_none() {
            let focus_handle = self.focus_handle.clone();
            self.focus_subscription =
                Some(cx.on_focus(&focus_handle, window, |this, window, cx| {
                    this.cursor_visible = true;
                    this.schedule_blink(window, cx);
                    cx.notify();
                }));
        }
        if self.focus_handle.is_focused(window) && !self.is_blinking {
            self.schedule_blink(window, cx);
        }
        let preview = self.render_chip_preview(cx);
        div()
            .id("text-input")
            .flex()
            .w_full()
            .key_context({
                let mut context = gpui::KeyContext::new_with_defaults();
                context.add(KEY_CONTEXT);
                if self.is_multi_line() {
                    context.add(MULTI_LINE_CONTEXT);
                }
                if self.menu_open {
                    context.add(MENU_CONTEXT);
                }
                context
            })
            .track_focus(&self.focus_handle(cx))
            .cursor(CursorStyle::IBeam)
            .on_action(cx.listener(Self::backspace))
            .on_action(cx.listener(Self::delete))
            .on_action(cx.listener(Self::left))
            .on_action(cx.listener(Self::right))
            .on_action(cx.listener(Self::up))
            .on_action(cx.listener(Self::down))
            .on_action(cx.listener(Self::select_left))
            .on_action(cx.listener(Self::select_right))
            .on_action(cx.listener(Self::select_up))
            .on_action(cx.listener(Self::select_down))
            .on_action(cx.listener(Self::select_all))
            .on_action(cx.listener(Self::home))
            .on_action(cx.listener(Self::end))
            .on_action(cx.listener(Self::newline))
            .on_action(cx.listener(Self::show_character_palette))
            .on_action(cx.listener(Self::paste))
            .on_action(cx.listener(Self::paste_raw))
            .on_action(cx.listener(Self::cut))
            .on_action(cx.listener(Self::copy))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
            .on_mouse_down(MouseButton::Right, cx.listener(Self::on_mouse_down))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_up_out(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_move(cx.listener(Self::on_mouse_move))
            // Moves are only heard over the input, so leaving it ends a chip's hover.
            .on_hover(cx.listener(|this, hovered: &bool, _, cx| {
                if !hovered && this.hovered_chip.take().is_some() {
                    cx.notify();
                }
            }))
            .when(self.is_multi_line(), |this| {
                this.on_scroll_wheel(cx.listener(Self::on_scroll_wheel))
            })
            .child(TextElement { input: cx.entity() })
            .children(preview)
    }
}

impl Focusable for TextInput {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

#[cfg(test)]
mod tests {
    use gpui::{TestAppContext, VisualTestContext};

    use super::*;

    struct Host(Entity<TextInput>);

    impl Render for Host {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div().child(
                div()
                    .w(px(240.))
                    .debug_selector(|| "input".into())
                    .child(self.0.clone()),
            )
        }
    }

    struct Fonts(gpui::Font);

    impl theme::ThemeSettingsProvider for Fonts {
        fn ui_font<'a>(&'a self, _: &'a App) -> &'a gpui::Font {
            &self.0
        }

        fn buffer_font<'a>(&'a self, _: &'a App) -> &'a gpui::Font {
            &self.0
        }

        fn ui_font_size(&self, _: &App) -> Pixels {
            px(14.)
        }

        fn buffer_font_size(&self, _: &App) -> Pixels {
            px(14.)
        }

        fn ui_density(&self, _: &App) -> theme::UiDensity {
            theme::UiDensity::default()
        }
    }

    fn input(
        multi_line: bool,
        cx: &mut TestAppContext,
    ) -> (Entity<TextInput>, &mut VisualTestContext) {
        cx.update(|cx| {
            theme::init(theme::LoadThemes::JustBase, cx);
            theme::set_theme_settings_provider(Box::new(Fonts(gpui::font("Helvetica"))), cx);
            init(cx);
        });
        let (input, cx) = cx.add_window_view(|_, cx| {
            let input = TextInput::new("", cx);
            if multi_line {
                input.multi_line(3)
            } else {
                input
            }
        });
        let (_, cx) = cx.add_window_view(|_, _| Host(input.clone()));
        let focus = input.read_with(cx, |input, cx| input.focus_handle(cx));
        cx.update(|window, cx| window.focus(&focus, cx));
        cx.run_until_parked();
        (input, cx)
    }

    fn text(input: &Entity<TextInput>, cx: &mut VisualTestContext) -> String {
        input.read_with(cx, |input, _| input.text().to_string())
    }

    #[gpui::test]
    fn shift_enter_makes_lines_and_the_input_grows_to_its_limit(cx: &mut TestAppContext) {
        let (input, cx) = input(true, cx);
        let height = |cx: &mut VisualTestContext| {
            cx.run_until_parked();
            cx.debug_bounds("input").expect("the input").size.height
        };
        cx.simulate_input("one");
        let one_line = height(cx);
        cx.simulate_keystrokes("shift-enter");
        cx.simulate_input("two");
        assert_eq!(text(&input, cx), "one\ntwo");
        assert_eq!(height(cx), one_line * 2.);
        for line in ["three", "four", "five"] {
            cx.simulate_keystrokes("shift-enter");
            cx.simulate_input(line);
        }
        // Three lines at most; the rest scrolls.
        assert_eq!(height(cx), one_line * 3.);
        // Long text wraps rather than running off the edge.
        input.update_in(cx, |input, _, cx| input.set_text("word ".repeat(20), cx));
        assert_eq!(height(cx), one_line * 3.);
    }

    #[gpui::test]
    fn up_and_down_move_between_lines(cx: &mut TestAppContext) {
        let (input, cx) = input(true, cx);
        cx.simulate_input("abc");
        cx.simulate_keystrokes("shift-enter");
        cx.simulate_input("de");
        cx.run_until_parked();
        cx.simulate_keystrokes("up");
        let cursor = input.read_with(cx, |input, _| input.cursor_offset());
        assert_eq!(cursor, 2);
        cx.simulate_keystrokes("down");
        let cursor = input.read_with(cx, |input, _| input.cursor_offset());
        assert_eq!(cursor, 6);
    }

    #[gpui::test]
    fn pasting_keeps_line_breaks_with_several_lines(cx: &mut TestAppContext) {
        let (input, cx) = input(true, cx);
        cx.write_to_clipboard(ClipboardItem::new_string("a\nb".into()));
        cx.simulate_keystrokes("cmd-v");
        assert_eq!(text(&input, cx), "a\nb");
    }

    #[gpui::test]
    fn pasting_into_one_line_joins_lines(cx: &mut TestAppContext) {
        let (input, cx) = input(false, cx);
        cx.write_to_clipboard(ClipboardItem::new_string("a\nb".into()));
        cx.simulate_keystrokes("cmd-v");
        assert_eq!(text(&input, cx), "a b");
    }

    #[gpui::test]
    fn hovering_a_chip_shows_its_preview(cx: &mut TestAppContext) {
        let (input, cx) = input(true, cx);
        input.update(cx, |input, cx| {
            input.insert_chip(
                None,
                "total.ts",
                "icons/file.svg".into(),
                ChipPreview::Text("~/storefront/src/total.ts".into()),
                "@total.ts".into(),
                cx,
            );
        });
        cx.run_until_parked();
        let chip = input.read_with(cx, |input, _| {
            input
                .layout
                .as_ref()
                .and_then(|layout| layout.chip_bounds.first().copied())
        });
        let (_, bounds) = chip.expect("the chip's bounds");
        cx.simulate_mouse_move(bounds.center(), None, gpui::Modifiers::none());
        let hovered = input.read_with(cx, |input, _| input.hovered_chip.map(|(id, _)| id));
        assert_eq!(hovered, Some(0));
        cx.simulate_mouse_move(
            bounds.center() + point(px(200.), px(0.)),
            None,
            gpui::Modifiers::none(),
        );
        let hovered = input.read_with(cx, |input, _| input.hovered_chip);
        assert!(hovered.is_none());
    }

    #[gpui::test]
    fn a_chip_is_one_piece(cx: &mut TestAppContext) {
        let (input, cx) = input(true, cx);
        cx.simulate_input("see ");
        input.update(cx, |input, cx| {
            input.insert_chip(
                None,
                "total.ts",
                "icons/file.svg".into(),
                ChipPreview::Text("src/total.ts".into()),
                "@total.ts".into(),
                cx,
            );
        });
        cx.simulate_input("now");
        let plain = input.read_with(cx, |input, _| input.plain_text());
        assert_eq!(plain, "see @total.ts now");
        // Left steps over the space, then the whole chip.
        cx.simulate_keystrokes("left left left left left");
        let cursor = input.read_with(cx, |input, _| input.cursor_offset());
        assert_eq!(cursor, 4);
        // Backspace after it removes it whole.
        cx.simulate_keystrokes("right backspace");
        let (plain, chips) =
            input.read_with(cx, |input, _| (input.plain_text(), input.chips().len()));
        assert_eq!((plain.as_str(), chips), ("see  now", 0));
    }

    /// Where the text at `offset` is drawn, a little to its right.
    fn position_of(
        input: &Entity<TextInput>,
        offset: usize,
        cx: &mut VisualTestContext,
    ) -> Point<Pixels> {
        cx.run_until_parked();
        input.read_with(cx, |input, _| {
            let layout = input.layout.as_ref().expect("the input's layout");
            layout.bounds.origin
                + layout.position_for_index(offset)
                + point(px(1.), layout.line_height / 2. - layout.scroll_top)
        })
    }

    fn click(position: Point<Pixels>, click_count: usize, cx: &mut VisualTestContext) {
        cx.simulate_event(MouseDownEvent {
            button: MouseButton::Left,
            position,
            modifiers: gpui::Modifiers::none(),
            click_count,
            first_mouse: false,
        });
        cx.simulate_event(MouseUpEvent {
            button: MouseButton::Left,
            position,
            modifiers: gpui::Modifiers::none(),
            click_count,
        });
    }

    fn selection(input: &Entity<TextInput>, cx: &mut VisualTestContext) -> String {
        input.read_with(cx, |input, _| {
            input.content[input.selected_range.clone()].to_string()
        })
    }

    #[gpui::test]
    fn double_click_selects_a_word_and_triple_click_its_line(cx: &mut TestAppContext) {
        let (input, cx) = input(true, cx);
        cx.simulate_input("fix total.ts now");
        cx.simulate_keystrokes("shift-enter");
        cx.simulate_input("then test");
        let total = position_of(&input, 6, cx);
        click(total, 1, cx);
        click(total, 2, cx);
        assert_eq!(selection(&input, cx), "total");
        click(total, 3, cx);
        assert_eq!(selection(&input, cx), "fix total.ts now\n");
        click(total, 4, cx);
        assert_eq!(selection(&input, cx), "fix total.ts now\nthen test");
    }

    #[gpui::test]
    fn dragging_after_a_double_click_selects_whole_words(cx: &mut TestAppContext) {
        let (input, cx) = input(true, cx);
        cx.simulate_input("one two three");
        let two = position_of(&input, 5, cx);
        let three = position_of(&input, 9, cx);
        let one = position_of(&input, 1, cx);
        cx.simulate_event(MouseDownEvent {
            button: MouseButton::Left,
            position: two,
            modifiers: gpui::Modifiers::none(),
            click_count: 2,
            first_mouse: false,
        });
        cx.simulate_mouse_move(three, Some(MouseButton::Left), gpui::Modifiers::none());
        assert_eq!(selection(&input, cx), "two three");
        cx.simulate_mouse_move(one, Some(MouseButton::Left), gpui::Modifiers::none());
        assert_eq!(selection(&input, cx), "one two");
        cx.simulate_mouse_up(one, MouseButton::Left, gpui::Modifiers::none());
    }

    #[gpui::test]
    fn double_clicking_a_chip_selects_it_whole(cx: &mut TestAppContext) {
        let (input, cx) = input(true, cx);
        cx.simulate_input("see ");
        input.update(cx, |input, cx| {
            input.insert_chip(
                None,
                "total.ts",
                "icons/file.svg".into(),
                ChipPreview::Text("src/total.ts".into()),
                "@total.ts".into(),
                cx,
            );
        });
        cx.simulate_input("now");
        cx.run_until_parked();
        let (chip_range, chip_bounds) = input.read_with(cx, |input, _| {
            let layout = input.layout.as_ref().expect("the input's layout");
            (input.chips[0].range.clone(), layout.chip_bounds[0].1)
        });
        for position in [
            chip_bounds.origin + point(px(1.), chip_bounds.size.height / 2.),
            chip_bounds.center(),
        ] {
            click(position, 2, cx);
            let selected = input.read_with(cx, |input, _| input.selected_range.clone());
            assert_eq!(selected, chip_range);
        }
    }

    fn chip(id: ChipId, range: Range<usize>) -> Chip {
        Chip {
            id,
            range,
            icon: "icons/file.svg".into(),
            preview: ChipPreview::Text("".into()),
            copy_text: "@a".into(),
        }
    }

    #[test]
    fn chip_runs_use_the_code_font() {
        let base = TextRun {
            len: 10,
            font: gpui::font("Sans"),
            color: gpui::black(),
            background_color: None,
            underline: None,
            strikethrough: None,
        };
        let code = gpui::font("Mono");
        let runs = runs_for("ab\u{a0}cd efgh", &base, &code, &[chip(0, 2..7)], None);
        let lens: Vec<usize> = runs.iter().map(|run| run.len).collect();
        assert_eq!(lens, vec![2, 5, 4]);
        assert_eq!(runs[1].font.family, code.family);
        assert_eq!(runs[0].font.family, base.font.family);
    }
}
