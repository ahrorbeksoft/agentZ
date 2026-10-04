//! A terminal on screen, from Zed's `TerminalView`: keys, the IME, the clipboard and the mouse,
//! turned into input for the server's terminal. The mouse handling is Zed's `Terminal`'s, kept
//! here because the app's copy of a terminal may be shown in more than one place.

use agentz_protocol::terminal::{
    TerminalInput, TerminalMatches, TerminalModes, TerminalRange, TerminalScroll,
    TerminalSelectionKind, TerminalSelectionUpdate,
};
use agentz_protocol::terminal_keys::{self, Keystroke as TerminalKeystroke};
use anyhow::Result;
use gpui::{
    Action, App, Bounds, Context, Entity, FocusHandle, Focusable, KeyBinding, KeyDownEvent,
    MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, Point, ScrollWheelEvent,
    Subscription, Task, TouchPhase, Window, actions, px,
};
use text_input::{TextInput, TextInputEvent};
use ui::{Tooltip, prelude::*};
use util::ResultExt as _;

use crate::app_settings::AppSettingsStore;
use crate::controls::text_field;
use crate::terminal_element::{self, GridLayout, TerminalElement, TerminalMode};
use crate::terminal_entity::Terminal;
use crate::terminal_mouse::{
    SelectionSide, alt_scroll, grid_point, grid_point_and_side, mouse_button_report,
    mouse_moved_report, scroll_report,
};

const KEY_CONTEXT: &str = "Terminal";
/// The find bar's, outside the terminal's so the keys typed there don't reach the shell.
const FIND_KEY_CONTEXT: &str = "TerminalFind";

/// How far the pointer moves before a press becomes a selection, as gpui's `div` drags.
const SELECTION_DRAG_THRESHOLD: f64 = 2.0;

actions!(
    terminal,
    [
        /// Copies the selection.
        Copy,
        /// Pastes the clipboard's text.
        Paste,
        /// Selects everything, history included.
        SelectAll,
        /// Clears the screen and history, keeping the prompt.
        Clear,
        /// Scrolls back one line.
        ScrollLineUp,
        /// Scrolls forward one line.
        ScrollLineDown,
        /// Scrolls back a screen.
        ScrollPageUp,
        /// Scrolls forward a screen.
        ScrollPageDown,
        /// Scrolls to the start of the history.
        ScrollToTop,
        /// Scrolls back to the prompt.
        ScrollToBottom,
        /// Makes every terminal's text bigger.
        IncreaseFontSize,
        /// Makes every terminal's text smaller.
        DecreaseFontSize,
        /// Puts every terminal's text back to its default size.
        ResetFontSize,
        /// Finds text in the terminal and its history.
        Find,
        /// Shows the next match, toward the prompt.
        SelectNextMatch,
        /// Shows the previous match, back in the history.
        SelectPreviousMatch,
        /// Closes the find bar.
        DismissFind,
    ]
);

/// Types text into the terminal.
#[derive(Clone, Debug, PartialEq, Action)]
#[action(namespace = terminal, no_json)]
pub struct SendText(pub String);

/// Presses a key in the terminal, as `ctrl-c` names it.
#[derive(Clone, Debug, PartialEq, Action)]
#[action(namespace = terminal, no_json)]
pub struct SendKeystroke(pub String);

pub fn init(cx: &mut App) {
    let context = Some(KEY_CONTEXT);
    let send_keystroke = |key: &str| SendKeystroke(key.to_string());
    let send_text = |text: &str| SendText(text.to_string());
    cx.bind_keys([
        KeyBinding::new("cmd-c", Copy, context),
        KeyBinding::new("cmd-v", Paste, context),
        KeyBinding::new("cmd-a", SelectAll, context),
        KeyBinding::new("cmd-k", Clear, context),
        // Some nice conveniences
        KeyBinding::new("cmd-backspace", send_keystroke("ctrl-u"), context),
        KeyBinding::new("alt-delete", send_text("\u{1b}d"), context),
        KeyBinding::new("cmd-delete", send_keystroke("ctrl-k"), context),
        KeyBinding::new("cmd-right", send_keystroke("ctrl-e"), context),
        KeyBinding::new("cmd-left", send_keystroke("ctrl-a"), context),
        // Terminal.app compatibility
        KeyBinding::new("alt-left", send_text("\u{1b}b"), context),
        KeyBinding::new("alt-right", send_text("\u{1b}f"), context),
        KeyBinding::new("alt-b", send_text("\u{1b}b"), context),
        KeyBinding::new("alt-f", send_text("\u{1b}f"), context),
        KeyBinding::new("ctrl-delete", send_text("\u{1b}[3;5~"), context),
        // There are conflicting bindings for these keys in parent contexts.
        KeyBinding::new("up", send_keystroke("up"), context),
        KeyBinding::new("pageup", send_keystroke("pageup"), context),
        KeyBinding::new("down", send_keystroke("down"), context),
        KeyBinding::new("pagedown", send_keystroke("pagedown"), context),
        KeyBinding::new("escape", send_keystroke("escape"), context),
        KeyBinding::new("enter", send_keystroke("enter"), context),
        KeyBinding::new("ctrl-c", send_keystroke("ctrl-c"), context),
        KeyBinding::new("ctrl-r", send_keystroke("ctrl-r"), context),
        KeyBinding::new("ctrl-backspace", send_keystroke("ctrl-w"), context),
        KeyBinding::new("shift-pageup", ScrollPageUp, context),
        KeyBinding::new("cmd-up", ScrollPageUp, context),
        KeyBinding::new("shift-pagedown", ScrollPageDown, context),
        KeyBinding::new("cmd-down", ScrollPageDown, context),
        KeyBinding::new("shift-up", ScrollLineUp, context),
        KeyBinding::new("shift-down", ScrollLineDown, context),
        KeyBinding::new("shift-home", ScrollToTop, context),
        KeyBinding::new("cmd-home", ScrollToTop, context),
        KeyBinding::new("shift-end", ScrollToBottom, context),
        KeyBinding::new("cmd-end", ScrollToBottom, context),
        // Zed's keys for the font size.
        KeyBinding::new("cmd-=", IncreaseFontSize, context),
        KeyBinding::new("cmd-+", IncreaseFontSize, context),
        KeyBinding::new("cmd--", DecreaseFontSize, context),
        KeyBinding::new("cmd-0", ResetFontSize, context),
        // Zed's buffer search keys.
        KeyBinding::new("cmd-f", Find, context),
        KeyBinding::new("cmd-f", Find, Some(FIND_KEY_CONTEXT)),
        KeyBinding::new("enter", SelectNextMatch, Some(FIND_KEY_CONTEXT)),
        KeyBinding::new("shift-enter", SelectPreviousMatch, Some(FIND_KEY_CONTEXT)),
        KeyBinding::new("escape", DismissFind, Some(FIND_KEY_CONTEXT)),
    ]);
}

/// Zed's terminal search bar: a query, its matches' count and buttons to step through them.
struct FindBar {
    input: Entity<TextInput>,
    /// The matches the server returned, from the top of the history down.
    matches: Vec<TerminalRange>,
    /// How many there are, those the server left out included.
    total: usize,
    /// The match shown, into `matches`.
    active: Option<usize>,
    search: Option<Task<()>>,
    /// The terminal changed while the search ran, so its matches may have moved.
    stale: bool,
    /// The selection on its way to become the query.
    seed: Option<Task<()>>,
    _subscription: Subscription,
}

impl FindBar {
    /// Zed's count: the match shown and how many there are, or 0/0.
    fn count(&self) -> String {
        match self.active {
            Some(index) => format!(
                "{}/{}",
                self.total - self.matches.len() + index + 1,
                self.total
            ),
            None => "0/0".to_string(),
        }
    }
}

pub struct TerminalView {
    terminal: Entity<Terminal>,
    focus_handle: FocusHandle,
    mode: TerminalMode,
    /// The IME's text before it's committed.
    marked_text: Option<String>,
    /// Where the grid was last drawn.
    grid: Option<GridLayout>,
    /// Pixels scrolled that haven't added up to a line yet.
    scroll_px: Pixels,
    selecting: bool,
    mouse_down_position: Option<Point<Pixels>>,
    last_mouse: Option<(agentz_protocol::terminal::TerminalPoint, SelectionSide)>,
    /// Whether the view had focus when it last rendered: focus changes are noticed there, so
    /// views can be made without a window, as a thread's tool calls make them.
    was_focused: bool,
    find_bar: Option<FindBar>,
    _subscriptions: Vec<Subscription>,
}

impl Focusable for TerminalView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl TerminalView {
    pub fn new(terminal: Entity<Terminal>, mode: TerminalMode, cx: &mut Context<Self>) -> Self {
        let focus_handle = cx.focus_handle();
        let view = cx.entity_id();
        let weak_terminal = terminal.downgrade();
        let subscriptions = vec![
            cx.observe(&terminal, |this, _, cx| {
                this.find_again(cx);
                cx.notify();
            }),
            cx.on_release(move |_, cx| {
                weak_terminal
                    .update(cx, |terminal, _| terminal.remove_view(view))
                    .ok();
            }),
        ];
        Self {
            terminal,
            focus_handle,
            mode,
            marked_text: None,
            grid: None,
            scroll_px: px(0.),
            selecting: false,
            mouse_down_position: None,
            last_mouse: None,
            was_focused: false,
            find_bar: None,
            _subscriptions: subscriptions,
        }
    }

    /// The find bar's matches, for the element to highlight.
    pub(crate) fn find_matches(&self) -> &[TerminalRange] {
        self.find_bar
            .as_ref()
            .map_or(&[], |find_bar| find_bar.matches.as_slice())
    }

    /// Opens the find bar, or focuses it, with the selection as the query as Zed seeds it.
    fn deploy_find(&mut self, _: &Find, window: &mut Window, cx: &mut Context<Self>) {
        if self.mode != TerminalMode::Scrollable {
            cx.propagate();
            return;
        }
        let selection = self.terminal.read(cx).selection_text(cx);
        let find_bar = self.find_bar.get_or_insert_with(|| {
            let input = cx.new(|cx| TextInput::new("Search…", cx));
            let subscription = cx.subscribe(&input, |this, _, _: &TextInputEvent, cx| {
                this.find(true, cx)
            });
            FindBar {
                input,
                matches: Vec::new(),
                total: 0,
                active: None,
                search: None,
                stale: false,
                seed: None,
                _subscription: subscription,
            }
        });
        find_bar.seed = selection.map(|selection| {
            cx.spawn(async move |this, cx| {
                let Some(text) = selection.await.log_err() else {
                    return;
                };
                if text.is_empty() || text.contains('\n') {
                    return;
                }
                this.update(cx, |this, cx| {
                    if let Some(find_bar) = &this.find_bar {
                        find_bar.input.update(cx, |input, cx| {
                            input.set_text(text, cx);
                            input.select_all_text(cx);
                        });
                    }
                })
                .log_err();
            })
        });
        self.focus_find(&Find, window, cx);
    }

    /// Focuses the query with its text selected, to type over.
    fn focus_find(&mut self, _: &Find, window: &mut Window, cx: &mut Context<Self>) {
        let Some(find_bar) = &self.find_bar else {
            return;
        };
        let input = find_bar.input.clone();
        input.update(cx, |input, cx| input.select_all_text(cx));
        window.focus(&input.focus_handle(cx), cx);
        cx.notify();
    }

    fn dismiss_find(&mut self, _: &DismissFind, window: &mut Window, cx: &mut Context<Self>) {
        self.find_bar = None;
        window.focus(&self.focus_handle, cx);
        cx.notify();
    }

    /// Searches for the query; `show` then shows the match Zed would.
    fn find(&mut self, show: bool, cx: &mut Context<Self>) {
        let Some(find_bar) = &mut self.find_bar else {
            return;
        };
        find_bar.stale = false;
        let query = find_bar.input.read(cx).text().to_string();
        if query.is_empty() {
            find_bar.search = None;
            find_bar.matches.clear();
            find_bar.total = 0;
            find_bar.active = None;
            cx.notify();
            return;
        }
        let search = self.terminal.read(cx).find(query, cx);
        find_bar.search = Some(cx.spawn(async move |this, cx| {
            let matches = search.await;
            this.update(cx, |this, cx| this.found(matches, show, cx))
                .log_err();
        }));
    }

    /// Searches again after the output moved on, as Zed does on each of a terminal's
    /// wakeups, one search at a time.
    fn find_again(&mut self, cx: &mut Context<Self>) {
        let Some(find_bar) = &mut self.find_bar else {
            return;
        };
        if find_bar.input.read(cx).text().is_empty() {
            return;
        }
        if find_bar.search.is_some() {
            find_bar.stale = true;
        } else {
            self.find(false, cx);
        }
    }

    fn found(&mut self, matches: Result<TerminalMatches>, show: bool, cx: &mut Context<Self>) {
        let Some(find_bar) = &mut self.find_bar else {
            return;
        };
        find_bar.search = None;
        let stale = std::mem::take(&mut find_bar.stale);
        if let Some(matches) = matches.log_err() {
            find_bar.matches = matches.matches;
            find_bar.total = matches.total;
            // The match shown is selected, and its selection moves with the output.
            let selection = self
                .terminal
                .read(cx)
                .frame()
                .and_then(|frame| frame.selection);
            find_bar.active = active_match(&find_bar.matches, selection.map(|s| s.end));
            if show && let Some(index) = find_bar.active {
                let range = find_bar.matches[index];
                self.terminal
                    .update(cx, |terminal, cx| terminal.show_match(range, cx));
            }
            cx.notify();
        }
        if stale {
            self.find(false, cx);
        }
    }

    fn select_next_match(&mut self, _: &SelectNextMatch, _: &mut Window, cx: &mut Context<Self>) {
        self.step_match(true, cx);
    }

    fn select_previous_match(
        &mut self,
        _: &SelectPreviousMatch,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.step_match(false, cx);
    }

    /// Shows the next or previous match, wrapping around as Zed's search does.
    fn step_match(&mut self, forward: bool, cx: &mut Context<Self>) {
        let Some(find_bar) = &mut self.find_bar else {
            return;
        };
        let count = find_bar.matches.len();
        if count == 0 {
            return;
        }
        let index = match find_bar.active {
            Some(index) if forward => (index + 1) % count,
            Some(index) => (index + count - 1) % count,
            None => count - 1,
        };
        find_bar.active = Some(index);
        let range = find_bar.matches[index];
        self.terminal
            .update(cx, |terminal, cx| terminal.show_match(range, cx));
        cx.notify();
    }

    fn render_find_bar(&self, window: &Window, cx: &mut Context<Self>) -> Option<AnyElement> {
        let find_bar = self.find_bar.as_ref()?;
        let has_match = find_bar.active.is_some();
        let has_query = !find_bar.input.read(cx).text().is_empty();
        let found_nothing = has_query && find_bar.search.is_none() && find_bar.matches.is_empty();
        let input_focus = find_bar.input.focus_handle(cx);
        let step_button =
            |id: &'static str, icon: IconName, title: &'static str, action: Box<dyn Action>| {
                let input_focus = input_focus.clone();
                IconButton::new(id, icon)
                    .icon_size(IconSize::Small)
                    .disabled(!has_match)
                    .tooltip(move |_, cx| {
                        Tooltip::for_action_in(title, action.as_ref(), &input_focus, cx)
                    })
            };
        Some(
            h_flex()
                .debug_selector(|| "terminal-find".into())
                .key_context(FIND_KEY_CONTEXT)
                .on_action(cx.listener(Self::focus_find))
                .on_action(cx.listener(Self::select_next_match))
                .on_action(cx.listener(Self::select_previous_match))
                .on_action(cx.listener(Self::dismiss_find))
                .flex_none()
                .w_full()
                .px_2()
                .py_1()
                .gap_2()
                .border_b_1()
                .border_color(cx.theme().colors().border_variant)
                .child(text_field(&find_bar.input, found_nothing, window, cx).flex_1())
                .child(
                    step_button(
                        "terminal-find-previous",
                        IconName::ChevronLeft,
                        "Select Previous Match",
                        SelectPreviousMatch.boxed_clone(),
                    )
                    .on_click(cx.listener(|this, _, _, cx| this.step_match(false, cx))),
                )
                .child(
                    step_button(
                        "terminal-find-next",
                        IconName::ChevronRight,
                        "Select Next Match",
                        SelectNextMatch.boxed_clone(),
                    )
                    .on_click(cx.listener(|this, _, _, cx| this.step_match(true, cx))),
                )
                .child(
                    div().min_w(rems_from_px(40_f32)).child(
                        Label::new(find_bar.count())
                            .size(LabelSize::Small)
                            .color(if has_match {
                                Color::Default
                            } else {
                                Color::Disabled
                            }),
                    ),
                )
                .child(
                    IconButton::new("terminal-find-close", IconName::Close)
                        .icon_size(IconSize::Small)
                        .tooltip(move |_, cx| {
                            Tooltip::for_action_in(
                                "Close Search Bar",
                                &DismissFind,
                                &input_focus,
                                cx,
                            )
                        })
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.dismiss_find(&DismissFind, window, cx)
                        })),
                )
                .into_any_element(),
        )
    }

    pub fn terminal(&self) -> &Entity<Terminal> {
        &self.terminal
    }

    pub(crate) fn set_grid_layout(&mut self, grid: GridLayout) {
        self.grid = Some(grid);
    }

    pub(crate) fn marked_text(&self) -> Option<&str> {
        self.marked_text.as_deref()
    }

    /// The marked text's range, in UTF-16.
    pub(crate) fn marked_text_range(&self) -> Option<std::ops::Range<usize>> {
        self.marked_text
            .as_ref()
            .map(|text| 0..text.encode_utf16().count())
    }

    pub(crate) fn set_marked_text(&mut self, text: String, cx: &mut Context<Self>) {
        if text.is_empty() {
            return self.clear_marked_text(cx);
        }
        self.marked_text = Some(text);
        cx.notify();
    }

    pub(crate) fn clear_marked_text(&mut self, cx: &mut Context<Self>) {
        if self.marked_text.take().is_some() {
            cx.notify();
        }
    }

    pub(crate) fn commit_text(&mut self, text: &str, cx: &mut Context<Self>) {
        if !text.is_empty() {
            self.write(text.as_bytes().to_vec(), cx);
        }
    }

    fn write(&self, bytes: Vec<u8>, cx: &mut Context<Self>) {
        self.terminal
            .update(cx, |terminal, cx| terminal.write(bytes, cx));
    }

    fn modes(&self, cx: &App) -> TerminalModes {
        self.terminal.read(cx).modes()
    }

    fn display_offset(&self, cx: &App) -> usize {
        self.terminal
            .read(cx)
            .frame()
            .map_or(0, |frame| frame.display_offset as usize)
    }

    /// Whether mouse events go to the program: it asked for them, and shift isn't held to
    /// select instead.
    fn mouse_mode(&self, shift: bool, cx: &App) -> bool {
        self.modes(cx).intersects(TerminalModes::MOUSE_MODE) && !shift
    }

    fn point_and_side(
        &self,
        position: Point<Pixels>,
        cx: &App,
    ) -> Option<(agentz_protocol::terminal::TerminalPoint, SelectionSide)> {
        let bounds = self.grid?.mouse_bounds();
        Some(grid_point_and_side(
            position - bounds.bounds.origin,
            bounds,
            self.display_offset(cx),
        ))
    }

    fn point(
        &self,
        position: Point<Pixels>,
        cx: &App,
    ) -> Option<agentz_protocol::terminal::TerminalPoint> {
        let bounds = self.grid?.mouse_bounds();
        Some(grid_point(
            position - bounds.bounds.origin,
            bounds,
            self.display_offset(cx),
        ))
    }

    fn update_selection(&mut self, position: Point<Pixels>, cx: &mut Context<Self>) {
        if let Some((point, side)) = self.point_and_side(position, cx) {
            self.terminal.update(cx, |terminal, cx| {
                terminal.select(
                    Some(TerminalSelectionUpdate {
                        point,
                        right_half: side == SelectionSide::Right,
                        start: None,
                    }),
                    cx,
                )
            });
        }
    }

    pub(crate) fn selection_started(&self) -> bool {
        self.selecting
    }

    pub(crate) fn mouse_down(&mut self, event: &MouseDownEvent, cx: &mut Context<Self>) {
        if self.mouse_mode(event.modifiers.shift, cx) {
            let Some(point) = self.point(event.position, cx) else {
                return;
            };
            let modes = self.modes(cx);
            if let Some(bytes) =
                mouse_button_report(point, event.button, event.modifiers, true, modes)
            {
                self.write(bytes, cx);
            }
            return;
        }
        if event.button != MouseButton::Left {
            return;
        }
        self.mouse_down_position = Some(event.position);
        let Some((point, side)) = self.point_and_side(event.position, cx) else {
            return;
        };
        let kind = match event.click_count {
            0 => return,
            1 => TerminalSelectionKind::Simple,
            2 => TerminalSelectionKind::Semantic,
            3 => TerminalSelectionKind::Lines,
            _ => return,
        };
        let has_selection = self.terminal.read(cx).has_selection();
        if kind == TerminalSelectionKind::Simple && event.modifiers.shift && has_selection {
            // Shift+click extends the existing selection to this point.
            self.update_selection(event.position, cx);
            return;
        }
        self.terminal.update(cx, |terminal, cx| {
            terminal.select(
                Some(TerminalSelectionUpdate {
                    point,
                    right_half: side == SelectionSide::Right,
                    start: Some(kind),
                }),
                cx,
            )
        });
    }

    pub(crate) fn mouse_drag(
        &mut self,
        event: &MouseMoveEvent,
        region: Bounds<Pixels>,
        cx: &mut Context<Self>,
    ) {
        if self.mouse_mode(event.modifiers.shift, cx) {
            return;
        }
        // Ignore tiny pointer movements so that a click that jitters by a
        // pixel or two (e.g. the window-focusing click) does not begin a
        // selection.
        if !self.selecting
            && let Some(mouse_down_position) = self.mouse_down_position
            && (event.position - mouse_down_position).magnitude() <= SELECTION_DRAG_THRESHOLD
        {
            return;
        }
        self.selecting = true;
        self.update_selection(event.position, cx);

        // Doesn't make sense to scroll the alt screen
        if !self.modes(cx).contains(TerminalModes::ALT_SCREEN)
            && let Some(lines) = self.drag_line_delta(event, region)
        {
            self.terminal.update(cx, |terminal, cx| {
                terminal.scroll(TerminalScroll::Lines(lines), cx)
            });
        }
    }

    fn drag_line_delta(&self, event: &MouseMoveEvent, region: Bounds<Pixels>) -> Option<i32> {
        let line_height = self.grid?.bounds.line_height;
        let top = region.origin.y;
        let bottom = region.bottom_left().y;
        let scroll_lines = if event.position.y < top {
            let scroll_delta = (top - event.position.y).pow(1.1);
            (scroll_delta / line_height).ceil() as i32
        } else if event.position.y > bottom {
            let scroll_delta = -((event.position.y - bottom).pow(1.1));
            (scroll_delta / line_height).floor() as i32
        } else {
            return None;
        };
        Some(scroll_lines.clamp(-3, 3))
    }

    pub(crate) fn mouse_move(&mut self, event: &MouseMoveEvent, cx: &mut Context<Self>) {
        if !self.mouse_mode(event.modifiers.shift, cx) {
            return;
        }
        let Some((point, side)) = self.point_and_side(event.position, cx) else {
            return;
        };
        if self.last_mouse == Some((point, side)) {
            return;
        }
        self.last_mouse = Some((point, side));
        let modes = self.modes(cx);
        if let Some(bytes) = mouse_moved_report(point, event.pressed_button, event.modifiers, modes)
        {
            self.write(bytes, cx);
        }
    }

    pub(crate) fn mouse_up(&mut self, event: &MouseUpEvent, cx: &mut Context<Self>) {
        if self.mouse_mode(event.modifiers.shift, cx)
            && let Some(point) = self.point(event.position, cx)
        {
            let modes = self.modes(cx);
            if let Some(bytes) =
                mouse_button_report(point, event.button, event.modifiers, false, modes)
            {
                self.write(bytes, cx);
            }
        }
        self.selecting = false;
        self.last_mouse = None;
        self.mouse_down_position = None;
    }

    pub(crate) fn scroll_wheel(&mut self, event: &ScrollWheelEvent, cx: &mut Context<Self>) {
        let Some(grid) = self.grid else {
            return;
        };
        let line_height = grid.bounds.line_height;
        let lines = match event.touch_phase {
            TouchPhase::Started => {
                self.scroll_px = px(0.);
                return;
            }
            TouchPhase::Moved => {
                let old_offset = (self.scroll_px / line_height) as i32;
                self.scroll_px += event.delta.pixel_delta(line_height).y;
                let new_offset = (self.scroll_px / line_height) as i32;
                // Whenever we hit the edges, reset our stored scroll to 0
                // so we can respond to changes in direction quickly
                self.scroll_px %= grid.bounds.height();
                new_offset - old_offset
            }
            TouchPhase::Ended | TouchPhase::Cancelled => return,
        };
        if lines == 0 {
            return;
        }
        let modes = self.modes(cx);
        if self.mouse_mode(event.shift, cx) {
            let Some(point) = self.point(event.position, cx) else {
                return;
            };
            if let Some(reports) = scroll_report(point, lines, event, modes) {
                self.write(reports.flatten().collect(), cx);
            }
        } else if modes.contains(TerminalModes(
            TerminalModes::ALT_SCREEN.0 | TerminalModes::ALTERNATE_SCROLL.0,
        )) && !event.shift
        {
            self.write(alt_scroll(lines), cx);
        } else {
            self.terminal.update(cx, |terminal, cx| {
                terminal.scroll(TerminalScroll::Lines(lines), cx)
            });
        }
    }

    fn key_down(&mut self, event: &KeyDownEvent, _window: &mut Window, cx: &mut Context<Self>) {
        if event.prefer_character_input && event.keystroke.key_char.is_some() {
            return;
        }
        if self.process_keystroke(&terminal_keystroke(&event.keystroke), cx) {
            cx.stop_propagation();
        }
    }

    /// Sends a key's escape sequence. Keys that type text are left to the IME, which commits
    /// them through [`Self::commit_text`].
    fn process_keystroke(&mut self, keystroke: &TerminalKeystroke, cx: &mut Context<Self>) -> bool {
        let modes = self.modes(cx);
        match terminal_keys::to_esc_str(keystroke, modes, false) {
            Some(esc) => {
                self.write(esc.into_owned().into_bytes(), cx);
                true
            }
            None => false,
        }
    }

    fn send_text(&mut self, action: &SendText, _: &mut Window, cx: &mut Context<Self>) {
        self.write(action.0.clone().into_bytes(), cx);
    }

    fn send_keystroke(&mut self, action: &SendKeystroke, _: &mut Window, cx: &mut Context<Self>) {
        match TerminalKeystroke::parse(&action.0) {
            Some(keystroke) => {
                self.process_keystroke(&keystroke, cx);
            }
            None => log::error!("invalid terminal keystroke: {}", action.0),
        }
    }

    fn copy(&mut self, _: &Copy, _: &mut Window, cx: &mut Context<Self>) {
        self.terminal.update(cx, |terminal, cx| terminal.copy(cx));
    }

    fn paste(&mut self, _: &Paste, _: &mut Window, cx: &mut Context<Self>) {
        let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) else {
            return;
        };
        self.terminal
            .update(cx, |terminal, cx| terminal.paste(text, cx));
    }

    fn select_all(&mut self, _: &SelectAll, _: &mut Window, cx: &mut Context<Self>) {
        self.terminal
            .update(cx, |terminal, cx| terminal.select_all(cx));
    }

    fn clear(&mut self, _: &Clear, _: &mut Window, cx: &mut Context<Self>) {
        self.terminal
            .update(cx, |terminal, cx| terminal.input(TerminalInput::Clear, cx));
    }

    /// Scrolls the history; full-screen programs have none, so their keys go through.
    fn scroll(&mut self, scroll: TerminalScroll, cx: &mut Context<Self>) {
        if self.modes(cx).contains(TerminalModes::ALT_SCREEN) {
            cx.propagate();
            return;
        }
        self.terminal
            .update(cx, |terminal, cx| terminal.scroll(scroll, cx));
    }

    fn scroll_line_up(&mut self, _: &ScrollLineUp, _: &mut Window, cx: &mut Context<Self>) {
        self.scroll(TerminalScroll::Lines(1), cx);
    }

    fn scroll_line_down(&mut self, _: &ScrollLineDown, _: &mut Window, cx: &mut Context<Self>) {
        self.scroll(TerminalScroll::Lines(-1), cx);
    }

    fn scroll_page_up(&mut self, _: &ScrollPageUp, _: &mut Window, cx: &mut Context<Self>) {
        self.scroll(TerminalScroll::PageUp, cx);
    }

    fn scroll_page_down(&mut self, _: &ScrollPageDown, _: &mut Window, cx: &mut Context<Self>) {
        self.scroll(TerminalScroll::PageDown, cx);
    }

    fn scroll_to_top(&mut self, _: &ScrollToTop, _: &mut Window, cx: &mut Context<Self>) {
        self.scroll(TerminalScroll::Top, cx);
    }

    fn scroll_to_bottom(&mut self, _: &ScrollToBottom, _: &mut Window, cx: &mut Context<Self>) {
        self.scroll(TerminalScroll::Bottom, cx);
    }

    fn focus_changed(&mut self, focused: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.was_focused = focused;
        self.terminal
            .update(cx, |terminal, cx| terminal.focus(focused, cx));
        if focused {
            window.invalidate_character_coordinates();
        }
    }
}

/// The match to show, as Zed's terminal picks it: the first at or after the selection's head,
/// else the last, nearest the prompt.
fn active_match(
    matches: &[TerminalRange],
    selection_head: Option<agentz_protocol::terminal::TerminalPoint>,
) -> Option<usize> {
    let last = matches.len().checked_sub(1)?;
    Some(match selection_head {
        Some(head) => matches
            .iter()
            .position(|range| range.end >= head)
            .unwrap_or(last),
        None => last,
    })
}

/// A GPUI keystroke in the terms of the protocol's key mappings.
fn terminal_keystroke(keystroke: &gpui::Keystroke) -> TerminalKeystroke {
    let modifiers = keystroke.modifiers;
    TerminalKeystroke {
        modifiers: terminal_keys::Modifiers {
            control: modifiers.control,
            alt: modifiers.alt,
            shift: modifiers.shift,
            platform: modifiers.platform,
            function: modifiers.function,
        },
        key: keystroke.key.clone(),
        key_char: keystroke.key_char.clone(),
    }
}

impl Render for TerminalView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let focused = self.focus_handle.is_focused(window);
        if focused != self.was_focused {
            self.focus_changed(focused, window, cx);
        }
        // The focused view is the one the user is using, also when it comes back on screen
        // still focused after another view of the terminal was used.
        let view = cx.entity_id();
        if focused && !self.terminal.read(cx).is_sized_by(view) {
            self.terminal
                .update(cx, |terminal, cx| terminal.set_sizing_view(view, cx));
        }
        let terminal = self.terminal.read(cx);
        let status = match (terminal.frame(), terminal.error()) {
            (_, Some(error)) => Some(error.clone()),
            (None, None) => Some("Starting…".into()),
            (Some(_), None) => None,
        };
        let scrollable = self.mode == TerminalMode::Scrollable;
        let find_bar = self.render_find_bar(window, cx);
        let screen = div()
            .id("terminal-view")
            .w_full()
            .when(scrollable, |this| this.flex_1().min_h_0())
            .relative()
            .track_focus(&self.focus_handle)
            .key_context(KEY_CONTEXT)
            .when(scrollable, |this| {
                this.on_action(cx.listener(Self::deploy_find))
            })
            .on_action(cx.listener(Self::send_text))
            .on_action(cx.listener(Self::send_keystroke))
            .on_action(cx.listener(Self::copy))
            .on_action(cx.listener(Self::paste))
            .on_action(cx.listener(Self::clear))
            .on_action(cx.listener(Self::scroll_line_up))
            .on_action(cx.listener(Self::scroll_line_down))
            .on_action(cx.listener(Self::scroll_page_up))
            .on_action(cx.listener(Self::scroll_page_down))
            .on_action(cx.listener(Self::scroll_to_top))
            .on_action(cx.listener(Self::scroll_to_bottom))
            .on_action(|_: &IncreaseFontSize, _, cx| change_font_size(Some(1.), cx))
            .on_action(|_: &DecreaseFontSize, _, cx| change_font_size(Some(-1.), cx))
            .on_action(|_: &ResetFontSize, _, cx| change_font_size(None, cx))
            .on_action(cx.listener(Self::select_all))
            .on_key_down(cx.listener(Self::key_down))
            .child(TerminalElement::new(
                self.terminal.clone(),
                cx.entity(),
                self.focus_handle.clone(),
                focused,
                self.mode,
            ))
            .when_some(status, |this, status| {
                this.child(
                    div().absolute().top_2().right_3().child(
                        Label::new(status)
                            .size(LabelSize::Small)
                            .color(Color::Muted),
                    ),
                )
            });
        // The find bar sits beside the terminal rather than in it, outside its key context.
        v_flex()
            .w_full()
            .when(scrollable, |this| this.h_full())
            .children(find_bar)
            .child(screen)
    }
}

/// Steps every terminal's font size by a point, or back to the default, and redraws them.
fn change_font_size(step: Option<f32>, cx: &mut App) {
    let size = match step {
        Some(step) => Some((f32::from(terminal_element::font_size(cx)) + step).clamp(
            terminal_element::MIN_FONT_SIZE,
            terminal_element::MAX_FONT_SIZE,
        )),
        None => None,
    };
    AppSettingsStore::global(cx).update(cx, |store, cx| {
        store.update(|settings| settings.terminal_font_size = size, cx)
    });
    cx.refresh_windows();
}

#[cfg(test)]
mod tests {
    use agentz_protocol::spaces::SpacesSnapshot;
    use agentz_protocol::terminal::{TerminalFrame, TerminalKey, TerminalPoint, TerminalSelection};
    use agentz_protocol::{Request, Response};
    use gpui::{TestAppContext, VisualTestContext};
    use projects::ThreadId;

    use super::*;
    use crate::machines::MachineId;
    use crate::server_client::ServerClient;

    #[gpui::test]
    fn cmd_plus_and_minus_size_every_terminal(cx: &mut TestAppContext) {
        let terminal = cx.update(|cx| {
            crate::init_for_test(cx);
            let client = ServerClient::new_for_test(
                MachineId::Local,
                "This Mac".into(),
                SpacesSnapshot::default(),
                cx,
            );
            Terminal::shared(&client, TerminalKey::Drawer(ThreadId(1)), cx)
        });
        let (view, cx) =
            cx.add_window_view(|_, cx| TerminalView::new(terminal, TerminalMode::Scrollable, cx));
        view.update_in(cx, |view, window, cx| {
            window.focus(&view.focus_handle(cx), cx)
        });
        let size = |cx: &mut VisualTestContext| {
            cx.update(|_, cx| f32::from(terminal_element::font_size(cx)))
        };
        assert_eq!(size(cx), terminal_element::DEFAULT_FONT_SIZE);
        cx.simulate_keystrokes("cmd-=");
        assert_eq!(size(cx), terminal_element::DEFAULT_FONT_SIZE + 1.);
        cx.simulate_keystrokes("cmd-- cmd-- cmd--");
        assert_eq!(size(cx), terminal_element::DEFAULT_FONT_SIZE - 2.);
        for _ in 0..20 {
            cx.simulate_keystrokes("cmd--");
        }
        assert_eq!(size(cx), terminal_element::MIN_FONT_SIZE);
        cx.simulate_keystrokes("cmd-0");
        assert_eq!(size(cx), terminal_element::DEFAULT_FONT_SIZE);
    }

    #[gpui::test]
    fn cmd_f_finds_in_the_terminal_and_enter_steps_through_the_matches(cx: &mut TestAppContext) {
        let range = |line, column| TerminalRange {
            start: TerminalPoint { line, column },
            end: TerminalPoint {
                line,
                column: column + 2,
            },
        };
        // The server found five, and sent the three nearest the prompt.
        let matches = vec![range(-2, 0), range(0, 4), range(2, 1)];
        let frame = TerminalFrame {
            full: true,
            columns: 20,
            screen_lines: 4,
            ..TerminalFrame::default()
        };
        let (client, terminal) = cx.update(|cx| {
            crate::init_for_test(cx);
            let client = ServerClient::new_for_test(
                MachineId::Local,
                "This Mac".into(),
                SpacesSnapshot::default(),
                cx,
            );
            let answer_matches = matches.clone();
            client.update(cx, |client, _| {
                client.answer_for_test(move |request| match request {
                    Request::SubscribeTerminal(_) => Some(Response::TerminalFrame(frame.clone())),
                    Request::FindInTerminal { query, .. } => {
                        Some(Response::TerminalMatches(match query.as_str() {
                            "x" => TerminalMatches {
                                matches: answer_matches.clone(),
                                total: 5,
                            },
                            _ => TerminalMatches::default(),
                        }))
                    }
                    Request::TerminalSelectionText(_) => Some(Response::Message("x".into())),
                    _ => None,
                })
            });
            let terminal = Terminal::shared(&client, TerminalKey::Drawer(ThreadId(1)), cx);
            (client, terminal)
        });
        let (view, cx) = cx.add_window_view(|_, cx| {
            TerminalView::new(terminal.clone(), TerminalMode::Scrollable, cx)
        });
        view.update_in(cx, |view, window, cx| {
            window.focus(&view.focus_handle(cx), cx)
        });
        cx.run_until_parked();
        let count = |cx: &mut VisualTestContext| {
            view.read_with(cx, |view, _| view.find_bar.as_ref().map(FindBar::count))
        };
        let shown = |cx: &mut VisualTestContext| {
            client.read_with(cx, |client, _| {
                client
                    .sent_for_test()
                    .into_iter()
                    .filter_map(|request| match request {
                        Request::TerminalInput {
                            input: TerminalInput::ShowMatch(range),
                            ..
                        } => Some(range),
                        Request::TerminalInput {
                            input: TerminalInput::Bytes(bytes),
                            ..
                        } => panic!("the shell got {bytes:?}"),
                        _ => None,
                    })
                    .collect::<Vec<_>>()
            })
        };

        cx.simulate_keystrokes("cmd-f");
        cx.run_until_parked();
        assert!(cx.debug_bounds("terminal-find").is_some());
        assert_eq!(count(cx), Some("0/0".into()));

        // Typing searches, and shows the match nearest the prompt, as the fifth of five.
        cx.simulate_input("x");
        cx.run_until_parked();
        assert_eq!(count(cx), Some("5/5".into()));
        assert_eq!(shown(cx), [matches[2]]);
        view.read_with(cx, |view, _| assert_eq!(view.find_matches(), matches));

        // Enter goes on to the next, wrapping to the top; Shift-Enter comes back.
        cx.simulate_keystrokes("enter");
        assert_eq!(count(cx), Some("3/5".into()));
        cx.simulate_keystrokes("shift-enter");
        assert_eq!(count(cx), Some("5/5".into()));
        assert_eq!(shown(cx), [matches[2], matches[0], matches[2]]);

        // Nothing found reads 0/0.
        cx.simulate_input("y");
        cx.run_until_parked();
        assert_eq!(count(cx), Some("0/0".into()));

        // Escape closes the bar and gives the terminal its keys back.
        cx.simulate_keystrokes("escape");
        assert_eq!(count(cx), None);
        assert!(cx.debug_bounds("terminal-find").is_none());
        view.update_in(cx, |view, window, cx| {
            assert!(view.focus_handle(cx).is_focused(window));
            assert!(view.find_matches().is_empty());
        });

        // With a selection, Cmd-F searches for it.
        terminal.update(cx, |terminal, cx| {
            terminal.apply_frame(
                TerminalFrame {
                    full: true,
                    columns: 20,
                    screen_lines: 4,
                    selection: Some(TerminalSelection {
                        start: matches[1].start,
                        end: matches[1].end,
                        is_block: false,
                    }),
                    ..TerminalFrame::default()
                },
                cx,
            )
        });
        cx.simulate_keystrokes("cmd-f");
        cx.run_until_parked();
        view.read_with(cx, |view, cx| {
            let find_bar = view.find_bar.as_ref().expect("the find bar is open");
            assert_eq!(find_bar.input.read(cx).text(), "x");
        });
        // The selected match is the one shown.
        assert_eq!(count(cx), Some("4/5".into()));
    }
}
