//! A terminal on screen, from Zed's `TerminalView`: keys, the IME, the clipboard and the mouse,
//! turned into input for the server's terminal. The mouse handling is Zed's `Terminal`'s, kept
//! here because the app's copy of a terminal may be shown in more than one place.

use agentz_protocol::terminal::{
    TerminalInput, TerminalModes, TerminalScroll, TerminalSelectionKind, TerminalSelectionUpdate,
};
use agentz_protocol::terminal_keys::{self, Keystroke as TerminalKeystroke};
use gpui::{
    Action, App, Bounds, Context, Entity, FocusHandle, Focusable, KeyBinding, KeyDownEvent,
    MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, Point, ScrollWheelEvent,
    Subscription, TouchPhase, Window, actions, px,
};
use ui::prelude::*;

use crate::terminal_element::{GridLayout, TerminalElement, TerminalMode};
use crate::terminal_entity::Terminal;
use crate::terminal_mouse::{
    SelectionSide, alt_scroll, grid_point, grid_point_and_side, mouse_button_report,
    mouse_moved_report, scroll_report,
};

const KEY_CONTEXT: &str = "Terminal";

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
    ]);
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
            cx.observe(&terminal, |_, _, cx| cx.notify()),
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
            _subscriptions: subscriptions,
        }
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
        div()
            .id("terminal-view")
            .w_full()
            .when(self.mode == TerminalMode::Scrollable, |this| this.h_full())
            .relative()
            .track_focus(&self.focus_handle)
            .key_context(KEY_CONTEXT)
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
            })
    }
}
