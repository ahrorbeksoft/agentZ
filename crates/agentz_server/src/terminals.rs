//! Terminals the server runs: a PTY with `alacritty_terminal`'s `Term` and event loop, as in
//! Zed's `terminal` crate (`alacritty.rs`) without GPUI, settings or tasks. The terminal keeps
//! running, with its screen and scrollback, while nobody watches; watchers get its screen as
//! [`TerminalFrame`]s.

use std::borrow::Cow;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use agentz_protocol::terminal::{
    TerminalColor, TerminalCursor, TerminalCursorShape, TerminalExit, TerminalFrame, TerminalInput,
    TerminalLine, TerminalModes, TerminalPoint, TerminalRun, TerminalScroll, TerminalSelection,
    TerminalSelectionKind, TerminalStyle,
};
use alacritty_terminal::event::{Event as AlacEvent, EventListener, Notify as _, WindowSize};
use alacritty_terminal::event_loop::{EventLoop, EventLoopSender, Msg, Notifier};
use alacritty_terminal::grid::{Dimensions, Scroll as AlacScroll};
use alacritty_terminal::index::{Column, Line, Point as AlacPoint, Side};
use alacritty_terminal::selection::{Selection, SelectionType};
use alacritty_terminal::sync::FairMutex;
use alacritty_terminal::term::cell::Flags;
use alacritty_terminal::term::{Config, SEMANTIC_ESCAPE_CHARS, Term, TermMode};
use alacritty_terminal::tty;
use alacritty_terminal::vte::ansi::{
    ClearMode, Color as AlacColor, CursorShape as AlacCursorShape, Handler as _, Rgb,
};
use anyhow::{Context as _, Result};
use futures::channel::oneshot;

/// t3code keeps 5,000 lines of scrollback.
const SCROLLBACK_LINES: usize = 5_000;

/// What to start in a terminal.
#[derive(Clone, Debug)]
pub(crate) struct TerminalSpawn {
    /// A program and its arguments; `None` is the user's login shell.
    pub program: Option<(String, Vec<String>)>,
    pub cwd: PathBuf,
    /// Added to the server's own environment.
    pub env: HashMap<String, String>,
}

/// The terminal's size in cells, and the cell size in pixels for programs that ask.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct TerminalSize {
    pub columns: u16,
    pub screen_lines: u16,
    pub cell_width: u16,
    pub cell_height: u16,
}

impl Default for TerminalSize {
    fn default() -> Self {
        Self {
            columns: 80,
            screen_lines: 24,
            cell_width: 8,
            cell_height: 16,
        }
    }
}

impl TerminalSize {
    fn window_size(self) -> WindowSize {
        WindowSize {
            num_lines: self.screen_lines,
            num_cols: self.columns,
            cell_width: self.cell_width,
            cell_height: self.cell_height,
        }
    }
}

impl Dimensions for TerminalSize {
    // Only used to resize, which reads the screen size.
    fn total_lines(&self) -> usize {
        self.screen_lines()
    }

    fn screen_lines(&self) -> usize {
        self.screen_lines.max(1) as usize
    }

    fn columns(&self) -> usize {
        self.columns.max(2) as usize
    }
}

/// Hands the terminal's events to whoever owns it. Wakeups are only passed on once until the
/// owner has looked at the screen, so a stream of output doesn't flood the server.
#[derive(Clone)]
pub(crate) struct Listener {
    on_event: Arc<dyn Fn(AlacEvent) + Send + Sync>,
    wakeup_pending: Arc<AtomicBool>,
}

impl EventListener for Listener {
    fn send_event(&self, event: AlacEvent) {
        if matches!(event, AlacEvent::Wakeup) && self.wakeup_pending.swap(true, Ordering::AcqRel) {
            return;
        }
        (self.on_event)(event);
    }
}

pub(crate) struct Terminal {
    term: Arc<FairMutex<Term<Listener>>>,
    sender: EventLoopSender,
    wakeup_pending: Arc<AtomicBool>,
    spawn: TerminalSpawn,
    size: TerminalSize,
    title: Option<String>,
    exit: Option<TerminalExit>,
    /// The client's theme colors, to answer programs that ask for a color.
    palette: Option<Vec<[u8; 3]>>,
    exit_waiters: Vec<oneshot::Sender<TerminalExit>>,
    /// The process the terminal started, usually a shell.
    child_pid: Option<u32>,
}

impl Terminal {
    pub(crate) fn start(
        spawn: TerminalSpawn,
        size: TerminalSize,
        palette: Option<Vec<[u8; 3]>>,
        on_event: Arc<dyn Fn(AlacEvent) + Send + Sync>,
    ) -> Result<Self> {
        let mut env = spawn.env.clone();
        env.insert("TERM".into(), "xterm-256color".into());
        env.insert("COLORTERM".into(), "truecolor".into());
        env.insert("TERM_PROGRAM".into(), "agentZ".into());
        // So the shell counts itself as the first level, as in a terminal app (Zed).
        env.insert("SHLVL".into(), "0".into());
        // Started from a `.app`, there may be no locale.
        if std::env::var("LANG").is_err() {
            env.entry("LANG".into())
                .or_insert_with(|| "en_US.UTF-8".into());
        }
        let options = tty::Options {
            shell: spawn
                .program
                .clone()
                .map(|(program, args)| tty::Shell::new(program, args)),
            working_directory: Some(spawn.cwd.clone()),
            drain_on_exit: true,
            env,
            #[cfg(not(windows))]
            child_signal_mask: tty::SignalMask::current().ok(),
            #[cfg(windows)]
            escape_args: true,
        };
        let pty = tty::new(&options, size.window_size(), 0)
            .with_context(|| format!("starting a terminal in {}", spawn.cwd.display()))?;
        #[cfg(unix)]
        let child_pid = Some(pty.child().id());
        #[cfg(not(unix))]
        let child_pid = None;

        let wakeup_pending = Arc::new(AtomicBool::new(false));
        let listener = Listener {
            on_event,
            wakeup_pending: wakeup_pending.clone(),
        };
        let config = Config {
            scrolling_history: SCROLLBACK_LINES,
            semantic_escape_chars: format!("{SEMANTIC_ESCAPE_CHARS}─"),
            ..Config::default()
        };
        let term = Arc::new(FairMutex::new(Term::new(config, &size, listener.clone())));
        let event_loop = EventLoop::new(term.clone(), listener, pty, true, false)
            .context("starting the terminal's event loop")?;
        let sender = event_loop.channel();
        event_loop.spawn();
        Ok(Self {
            term,
            sender,
            wakeup_pending,
            spawn,
            size,
            title: None,
            exit: None,
            palette,
            exit_waiters: Vec::new(),
            child_pid,
        })
    }

    pub(crate) fn spawn(&self) -> &TerminalSpawn {
        &self.spawn
    }

    pub(crate) fn size(&self) -> TerminalSize {
        self.size
    }

    pub(crate) fn exit(&self) -> Option<&TerminalExit> {
        self.exit.as_ref()
    }

    pub(crate) fn modes(&self) -> TerminalModes {
        modes_from_alacritty(*self.term.lock().mode())
    }

    /// The program's own title for itself, if it set one.
    pub(crate) fn title(&self) -> Option<&str> {
        self.title.as_deref()
    }

    /// Lets the next output wake the owner again. Called before reading the screen, so output
    /// that arrives meanwhile isn't missed.
    pub(crate) fn take_wakeup(&self) {
        self.wakeup_pending.store(false, Ordering::Release);
    }

    /// Answers when the process exits, at once if it has.
    pub(crate) fn wait_for_exit(&mut self) -> oneshot::Receiver<TerminalExit> {
        let (sender, receiver) = oneshot::channel();
        match &self.exit {
            Some(exit) => {
                sender.send(exit.clone()).ok();
            }
            None => self.exit_waiters.push(sender),
        }
        receiver
    }

    /// Handles one of the terminal's events. Returns whether the screen may have changed.
    pub(crate) fn handle_event(&mut self, event: AlacEvent) -> bool {
        match event {
            AlacEvent::Wakeup | AlacEvent::MouseCursorDirty | AlacEvent::CursorBlinkingChange => {
                true
            }
            AlacEvent::Title(title) => {
                self.title = Some(title);
                true
            }
            AlacEvent::ResetTitle => {
                self.title = None;
                true
            }
            AlacEvent::PtyWrite(text) => {
                self.write(text.into_bytes());
                false
            }
            AlacEvent::TextAreaSizeRequest(format) => {
                self.write(format(self.size.window_size()).into_bytes());
                false
            }
            // Answered here rather than where it's emitted, to keep its order relative to
            // other replies (Zed).
            AlacEvent::ColorRequest(index, format) => {
                let color = self.term.lock().colors()[index].unwrap_or_else(|| {
                    let [r, g, b] = self
                        .palette
                        .as_ref()
                        .and_then(|palette| palette.get(index).copied())
                        .unwrap_or_else(|| default_color(index));
                    Rgb { r, g, b }
                });
                self.write(format(color).into_bytes());
                false
            }
            // The server has no clipboard; the client's own copy and paste go through
            // selections and `Paste`.
            AlacEvent::ClipboardStore(..) | AlacEvent::Bell => false,
            AlacEvent::ClipboardLoad(_, format) => {
                self.write(format("").into_bytes());
                false
            }
            AlacEvent::ChildExit(status) => {
                self.exited(exit_from_status(status));
                true
            }
            AlacEvent::Exit => {
                if self.exit.is_none() {
                    self.exited(TerminalExit {
                        code: None,
                        signal: None,
                    });
                }
                true
            }
        }
    }

    fn exited(&mut self, exit: TerminalExit) {
        for waiter in self.exit_waiters.drain(..) {
            waiter.send(exit.clone()).ok();
        }
        self.exit = Some(exit);
    }

    pub(crate) fn write(&self, bytes: impl Into<Cow<'static, [u8]>>) {
        if self.exit.is_none() {
            Notifier(self.sender.clone()).notify(bytes);
        }
    }

    /// Applies a client's input. Returns whether the screen may have changed.
    pub(crate) fn input(&mut self, input: TerminalInput) -> bool {
        match input {
            TerminalInput::Bytes(bytes) => {
                // Typing jumps back to the prompt and drops the selection (Zed's `input`).
                {
                    let mut term = self.term.lock();
                    term.scroll_display(AlacScroll::Bottom);
                    term.selection = None;
                }
                self.write(bytes);
                true
            }
            TerminalInput::Paste(text) => {
                let bracketed = self.term.lock().mode().contains(TermMode::BRACKETED_PASTE);
                let text = if bracketed {
                    format!("\x1b[200~{}\x1b[201~", text.replace('\x1b', ""))
                } else {
                    text.replace("\r\n", "\r").replace('\n', "\r")
                };
                self.input(TerminalInput::Bytes(text.into_bytes()))
            }
            TerminalInput::Resize {
                columns,
                screen_lines,
                cell_width,
                cell_height,
            } => {
                let size = TerminalSize {
                    columns: columns.max(2),
                    screen_lines: screen_lines.max(1),
                    cell_width,
                    cell_height,
                };
                if size == self.size {
                    return false;
                }
                self.size = size;
                if self.exit.is_none() {
                    self.sender.send(Msg::Resize(size.window_size())).ok();
                }
                self.term.lock().resize(size);
                true
            }
            TerminalInput::Scroll(scroll) => {
                self.term.lock().scroll_display(match scroll {
                    TerminalScroll::Lines(lines) => AlacScroll::Delta(lines),
                    TerminalScroll::PageUp => AlacScroll::PageUp,
                    TerminalScroll::PageDown => AlacScroll::PageDown,
                    TerminalScroll::Top => AlacScroll::Top,
                    TerminalScroll::Bottom => AlacScroll::Bottom,
                });
                true
            }
            TerminalInput::Select(None) => {
                self.term.lock().selection = None;
                true
            }
            TerminalInput::Select(Some(update)) => {
                let point = AlacPoint::new(
                    Line(update.point.line),
                    Column(update.point.column as usize),
                );
                let side = if update.right_half {
                    Side::Right
                } else {
                    Side::Left
                };
                let mut term = self.term.lock();
                match update.start {
                    Some(kind) => {
                        let kind = match kind {
                            TerminalSelectionKind::Simple => SelectionType::Simple,
                            TerminalSelectionKind::Semantic => SelectionType::Semantic,
                            TerminalSelectionKind::Lines => SelectionType::Lines,
                            TerminalSelectionKind::Block => SelectionType::Block,
                        };
                        term.selection = Some(Selection::new(kind, point, side));
                    }
                    None => {
                        if let Some(selection) = term.selection.as_mut() {
                            selection.update(point, side);
                        }
                    }
                }
                true
            }
            TerminalInput::SelectAll => {
                let mut term = self.term.lock();
                let start = AlacPoint::new(term.topmost_line(), Column(0));
                let end = AlacPoint::new(term.bottommost_line(), term.last_column());
                let mut selection = Selection::new(SelectionType::Simple, start, Side::Left);
                selection.update(end, Side::Right);
                term.selection = Some(selection);
                true
            }
            TerminalInput::Clear => {
                clear(&mut self.term.lock());
                true
            }
            TerminalInput::Focus(focused) => {
                if self.term.lock().mode().contains(TermMode::FOCUS_IN_OUT) {
                    self.write(if focused {
                        &b"\x1b[I"[..]
                    } else {
                        &b"\x1b[O"[..]
                    });
                }
                false
            }
            TerminalInput::Palette(palette) => {
                self.palette = Some(palette);
                false
            }
            TerminalInput::Unknown(input) => {
                log::warn!("ignoring unsupported terminal input: {input}");
                false
            }
        }
    }

    /// Ends the process, keeping the screen (ACP's `terminal/kill`).
    pub(crate) fn kill(&self) {
        // The event loop drops the PTY, which hangs up on the process.
        self.sender.send(Msg::Shutdown).ok();
    }

    pub(crate) fn selection_text(&self) -> Option<String> {
        self.term.lock().selection_to_string()
    }

    /// All the text, history included, with wrapped lines joined and trailing blank lines
    /// dropped.
    pub(crate) fn text(&self) -> String {
        let term = self.term.lock();
        let start = AlacPoint::new(term.topmost_line(), Column(0));
        let end = AlacPoint::new(term.bottommost_line(), term.last_column());
        let text = term.bounds_to_string(start, end);
        text.lines()
            .map(str::trim_end)
            .collect::<Vec<_>>()
            .join("\n")
            .trim_end()
            .to_string()
    }

    /// The visible screen's text, as herdr's `pane read --source visible` gives it.
    pub(crate) fn screen_text(&self) -> String {
        self.frame().text()
    }

    pub(crate) fn child_pid(&self) -> Option<u32> {
        self.child_pid
    }

    /// The bottom of the screen as agent detection reads it (herdr's `detection_text`): the
    /// screen's height of rows ending at the last non-blank row or the cursor, whichever is
    /// lower, wherever the view is scrolled. Each row is trimmed; blank rows at the end are
    /// dropped.
    pub(crate) fn detection_text(&self) -> String {
        let term = self.term.lock();
        let grid = term.grid();
        let screen_lines = term.screen_lines() as i32;
        let last = screen_lines - 1;
        let row_text = |line: i32| -> String {
            let row = &grid[Line(line)];
            let mut text = String::new();
            for column in 0..term.columns() {
                let cell = &row[Column(column)];
                if cell
                    .flags
                    .intersects(Flags::WIDE_CHAR_SPACER | Flags::LEADING_WIDE_CHAR_SPACER)
                {
                    continue;
                }
                text.push(cell.c);
                if let Some(zerowidth) = cell.zerowidth() {
                    text.extend(zerowidth);
                }
            }
            text.trim_end().to_string()
        };
        let end = if term.mode().contains(TermMode::ALT_SCREEN) {
            last
        } else {
            let last_non_blank = (0..screen_lines)
                .rev()
                .find(|line| !row_text(*line).is_empty())
                .unwrap_or(last);
            last_non_blank.max(grid.cursor.point.line.0)
        };
        let start = (end + 1 - screen_lines).max(-(grid.history_size() as i32));
        let mut rows: Vec<String> = (start..=end).map(row_text).collect();
        while rows.last().is_some_and(String::is_empty) {
            rows.pop();
        }
        if rows.is_empty() {
            return String::new();
        }
        let mut text = rows.join("\n");
        text.push('\n');
        text
    }

    /// The last `lines` lines of [`Self::text`].
    pub(crate) fn recent_text(&self, lines: usize) -> String {
        let text = self.text();
        let all: Vec<&str> = text.lines().collect();
        all[all.len().saturating_sub(lines)..].join("\n")
    }

    /// The whole screen as it's shown now.
    pub(crate) fn frame(&self) -> TerminalFrame {
        let term = self.term.lock();
        let content = term.renderable_content();
        let display_offset = content.display_offset;
        let columns = term.columns();
        let screen_lines = term.screen_lines();
        let mut rows: Vec<TerminalLine> = vec![TerminalLine::default(); screen_lines];
        let mut current: Option<(usize, TerminalRun)> = None;

        for indexed in content.display_iter {
            let row = (indexed.point.line.0 + display_offset as i32) as usize;
            let column = indexed.point.column.0;
            let cell = indexed.cell;
            if cell
                .flags
                .intersects(Flags::WIDE_CHAR_SPACER | Flags::LEADING_WIDE_CHAR_SPACER)
            {
                continue;
            }
            let style = style_from_cell(cell);
            let is_blank = cell.c == ' '
                && cell.zerowidth().is_none()
                && cell.bg
                    == AlacColor::Named(alacritty_terminal::vte::ansi::NamedColor::Background)
                && !cell
                    .flags
                    .intersects(Flags::INVERSE | Flags::ALL_UNDERLINES | Flags::STRIKEOUT);
            let is_wide = cell.flags.contains(Flags::WIDE_CHAR);

            let continues = match &current {
                Some((current_row, run)) => {
                    *current_row == row
                        && !is_blank
                        && !is_wide
                        && run.style == style
                        && run.column as usize + run.text.chars().count() == column
                        && cell.zerowidth().is_none()
                }
                None => false,
            };
            if continues {
                if let Some((_, run)) = current.as_mut() {
                    run.text.push(cell.c);
                }
                continue;
            }
            if let Some((run_row, run)) = current.take()
                && let Some(line) = rows.get_mut(run_row)
            {
                line.runs.push(run);
            }
            if is_blank {
                continue;
            }
            let mut text = String::new();
            text.push(cell.c);
            let mut style = style;
            if let Some(zerowidth) = cell.zerowidth() {
                text.extend(zerowidth);
                style.flags |= TerminalStyle::COMBINING;
            }
            let run = TerminalRun {
                column: column as u16,
                text,
                style,
            };
            if is_wide || cell.zerowidth().is_some() {
                // A run of its own, so later cells don't need to count its width.
                if let Some(line) = rows.get_mut(row) {
                    line.runs.push(run);
                }
            } else {
                current = Some((row, run));
            }
        }
        if let Some((run_row, run)) = current.take()
            && let Some(line) = rows.get_mut(run_row)
        {
            line.runs.push(run);
        }

        let cursor = (content.cursor.shape != AlacCursorShape::Hidden)
            .then(|| {
                let row = content.cursor.point.line.0 + display_offset as i32;
                (0..screen_lines as i32)
                    .contains(&row)
                    .then(|| TerminalCursor {
                        row: row as u16,
                        column: content.cursor.point.column.0 as u16,
                        shape: match content.cursor.shape {
                            AlacCursorShape::Underline => TerminalCursorShape::Underline,
                            AlacCursorShape::Beam => TerminalCursorShape::Bar,
                            AlacCursorShape::HollowBlock => TerminalCursorShape::HollowBlock,
                            AlacCursorShape::Block | AlacCursorShape::Hidden => {
                                TerminalCursorShape::Block
                            }
                        },
                    })
            })
            .flatten();

        TerminalFrame {
            full: true,
            columns: columns as u16,
            screen_lines: screen_lines as u16,
            lines: rows
                .into_iter()
                .enumerate()
                .map(|(row, line)| (row as u16, line))
                .collect(),
            cursor,
            modes: modes_from_alacritty(content.mode),
            display_offset: display_offset as u32,
            history_lines: term.history_size() as u32,
            selection: content.selection.map(|range| TerminalSelection {
                start: point_from_alacritty(range.start),
                end: point_from_alacritty(range.end),
                is_block: range.is_block,
            }),
            title: self.title.clone(),
            exited: self.exit.clone(),
        }
    }
}

impl Drop for Terminal {
    fn drop(&mut self) {
        self.kill();
    }
}

/// What a subscriber needs to go from the frame it has to the current one: `None` when
/// nothing changed. Both frames are full.
pub(crate) fn frame_changes(
    sent: &TerminalFrame,
    current: &TerminalFrame,
) -> Option<TerminalFrame> {
    if sent.columns != current.columns || sent.screen_lines != current.screen_lines {
        return Some(current.clone());
    }
    let lines: Vec<(u16, TerminalLine)> = current
        .lines
        .iter()
        .filter(|(row, line)| sent.line(*row) != Some(line))
        .cloned()
        .collect();
    let unchanged = lines.is_empty()
        && sent.cursor == current.cursor
        && sent.modes == current.modes
        && sent.display_offset == current.display_offset
        && sent.history_lines == current.history_lines
        && sent.selection == current.selection
        && sent.title == current.title
        && sent.exited == current.exited;
    if unchanged {
        return None;
    }
    Some(TerminalFrame {
        full: false,
        lines,
        ..current.clone()
    })
}

fn style_from_cell(cell: &alacritty_terminal::term::cell::Cell) -> TerminalStyle {
    let mut flags = 0;
    let mut set = |condition: bool, flag: u16| {
        if condition {
            flags |= flag;
        }
    };
    set(cell.flags.intersects(Flags::BOLD), TerminalStyle::BOLD);
    set(cell.flags.intersects(Flags::ITALIC), TerminalStyle::ITALIC);
    set(
        cell.flags.intersects(Flags::ALL_UNDERLINES) && !cell.flags.contains(Flags::UNDERCURL),
        TerminalStyle::UNDERLINE,
    );
    set(
        cell.flags.contains(Flags::UNDERCURL),
        TerminalStyle::UNDERCURL,
    );
    set(
        cell.flags.intersects(Flags::STRIKEOUT),
        TerminalStyle::STRIKEOUT,
    );
    set(cell.flags.contains(Flags::INVERSE), TerminalStyle::INVERSE);
    set(cell.flags.intersects(Flags::DIM), TerminalStyle::DIM);
    set(cell.flags.contains(Flags::HIDDEN), TerminalStyle::HIDDEN);
    set(cell.flags.contains(Flags::WIDE_CHAR), TerminalStyle::WIDE);
    TerminalStyle {
        foreground: color_from_alacritty(cell.fg),
        background: color_from_alacritty(cell.bg),
        flags,
    }
}

/// Zed's `InternalEvent::Clear`: drops the history and the screen, moving the cursor's line to
/// the top.
fn clear<T: EventListener>(term: &mut Term<T>) {
    term.clear_screen(ClearMode::Saved);
    let cursor = term.grid().cursor.point;
    term.grid_mut().reset_region(..cursor.line);
    let columns = term.grid().columns();
    let line = term.grid()[cursor.line][..Column(columns)].to_vec();
    for (column, cell) in line.into_iter().enumerate() {
        term.grid_mut()[Line(0)][Column(column)] = cell;
    }
    term.grid_mut().cursor.point = AlacPoint::new(Line(0), cursor.column);
    let new_cursor = term.grid().cursor.point;
    if (new_cursor.line.0 as usize) < term.screen_lines().saturating_sub(1) {
        term.grid_mut().reset_region((new_cursor.line + 1)..);
    }
}

fn color_from_alacritty(color: AlacColor) -> TerminalColor {
    match color {
        AlacColor::Named(named) => TerminalColor::Named(named as u16),
        AlacColor::Indexed(index) => TerminalColor::Indexed(index),
        AlacColor::Spec(rgb) => TerminalColor::Rgb(rgb.r, rgb.g, rgb.b),
    }
}

fn point_from_alacritty(point: AlacPoint) -> TerminalPoint {
    TerminalPoint {
        line: point.line.0,
        column: point.column.0 as u16,
    }
}

fn modes_from_alacritty(mode: TermMode) -> TerminalModes {
    let pairs = [
        (TermMode::APP_CURSOR, TerminalModes::APP_CURSOR),
        (TermMode::APP_KEYPAD, TerminalModes::APP_KEYPAD),
        (TermMode::SHOW_CURSOR, TerminalModes::SHOW_CURSOR),
        (TermMode::LINE_WRAP, TerminalModes::LINE_WRAP),
        (TermMode::ORIGIN, TerminalModes::ORIGIN),
        (TermMode::INSERT, TerminalModes::INSERT),
        (
            TermMode::LINE_FEED_NEW_LINE,
            TerminalModes::LINE_FEED_NEW_LINE,
        ),
        (TermMode::FOCUS_IN_OUT, TerminalModes::FOCUS_IN_OUT),
        (TermMode::ALTERNATE_SCROLL, TerminalModes::ALTERNATE_SCROLL),
        (TermMode::BRACKETED_PASTE, TerminalModes::BRACKETED_PASTE),
        (TermMode::SGR_MOUSE, TerminalModes::SGR_MOUSE),
        (TermMode::UTF8_MOUSE, TerminalModes::UTF8_MOUSE),
        (TermMode::ALT_SCREEN, TerminalModes::ALT_SCREEN),
        (
            TermMode::MOUSE_REPORT_CLICK,
            TerminalModes::MOUSE_REPORT_CLICK,
        ),
        (TermMode::MOUSE_DRAG, TerminalModes::MOUSE_DRAG),
        (TermMode::MOUSE_MOTION, TerminalModes::MOUSE_MOTION),
        (TermMode::VI, TerminalModes::VI),
    ];
    let mut modes = TerminalModes::NONE;
    for (alacritty, ours) in pairs {
        if mode.contains(alacritty) {
            modes.insert(ours);
        }
    }
    modes
}

/// The exit code and signal, as Zed's ACP terminals report them (from `portable_pty`).
fn exit_from_status(status: std::process::ExitStatus) -> TerminalExit {
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt as _;
        if let Some(signal) = status.signal() {
            let name = nix::sys::signal::Signal::try_from(signal)
                .map(|signal| signal.as_str().to_string())
                .unwrap_or_else(|_| format!("Signal {signal}"));
            return TerminalExit {
                code: Some(status.code().map(|code| code as u32).unwrap_or(1)),
                signal: Some(name),
            };
        }
    }
    TerminalExit {
        code: Some(
            status
                .code()
                .map(|code| code as u32)
                .unwrap_or(if status.success() { 0 } else { 1 }),
        ),
        signal: None,
    }
}

/// xterm's colors, for programs that ask before a client has sent its theme's.
fn default_color(index: usize) -> [u8; 3] {
    const ANSI: [[u8; 3]; 16] = [
        [0, 0, 0],
        [205, 0, 0],
        [0, 205, 0],
        [205, 205, 0],
        [0, 0, 238],
        [205, 0, 205],
        [0, 205, 205],
        [229, 229, 229],
        [127, 127, 127],
        [255, 0, 0],
        [0, 255, 0],
        [255, 255, 0],
        [92, 92, 255],
        [255, 0, 255],
        [0, 255, 255],
        [255, 255, 255],
    ];
    match index {
        0..=15 => ANSI[index],
        16..=231 => {
            let index = index - 16;
            let step = |value: usize| {
                if value == 0 {
                    0
                } else {
                    (value * 40 + 55) as u8
                }
            };
            [step(index / 36), step(index / 6 % 6), step(index % 6)]
        }
        232..=255 => {
            let value = ((index - 232) * 10 + 8) as u8;
            [value, value, value]
        }
        // Background, and the dim background.
        257 | 268 => [0, 0, 0],
        _ => [229, 229, 229],
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    /// A terminal running `sh`, whose events arrive on a channel.
    fn start_sh(
        size: TerminalSize,
    ) -> (
        Terminal,
        futures::channel::mpsc::UnboundedReceiver<AlacEvent>,
    ) {
        let (events, inbox) = futures::channel::mpsc::unbounded();
        let terminal = Terminal::start(
            TerminalSpawn {
                program: Some(("/bin/sh".into(), Vec::new())),
                cwd: std::env::temp_dir(),
                env: HashMap::from_iter([("PS1".to_string(), "$ ".to_string())]),
            },
            size,
            None,
            Arc::new(move |event| {
                events.unbounded_send(event).ok();
            }),
        )
        .expect("starting sh");
        (terminal, inbox)
    }

    /// Handles events until the screen satisfies `done`.
    async fn wait_for(
        terminal: &mut Terminal,
        inbox: &mut futures::channel::mpsc::UnboundedReceiver<AlacEvent>,
        done: impl Fn(&Terminal) -> bool,
    ) {
        use futures::StreamExt as _;
        let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
        loop {
            terminal.take_wakeup();
            if done(terminal) {
                return;
            }
            match tokio::time::timeout_at(deadline, inbox.next()).await {
                Ok(Some(event)) => {
                    terminal.handle_event(event);
                }
                Ok(None) => panic!("the terminal stopped:\n{}", terminal.text()),
                Err(_) => panic!("timed out; the screen is:\n{}", terminal.screen_text()),
            }
        }
    }

    #[tokio::test]
    async fn runs_commands_and_reports_their_output_and_exit() {
        let (mut terminal, mut inbox) = start_sh(TerminalSize::default());
        terminal.input(TerminalInput::Bytes(
            b"printf 'one\\n\\033[1;31mred\\033[0m\\n'\n".to_vec(),
        ));
        wait_for(&mut terminal, &mut inbox, |terminal| {
            terminal.screen_text().contains("red\n$")
        })
        .await;

        let frame = terminal.frame();
        let (row, line) = frame
            .lines
            .iter()
            .find(|(_, line)| line.text() == "red")
            .expect("a line with the red text");
        let run = &line.runs[0];
        assert_eq!(run.style.foreground, TerminalColor::Named(1));
        assert!(run.style.has(TerminalStyle::BOLD));
        assert_eq!(
            frame.line(row - 1).map(TerminalLine::text).as_deref(),
            Some("one")
        );
        assert!(frame.cursor.is_some());
        assert_eq!(terminal.recent_text(2), "red\n$");

        terminal.input(TerminalInput::Bytes(b"exit 3\n".to_vec()));
        let exit = terminal.wait_for_exit();
        wait_for(&mut terminal, &mut inbox, |terminal| {
            terminal.exit().is_some()
        })
        .await;
        assert_eq!(
            exit.await.expect("an exit"),
            TerminalExit {
                code: Some(3),
                signal: None
            }
        );
        assert!(terminal.frame().exited.is_some());
    }

    #[tokio::test]
    async fn frames_carry_only_changed_lines_and_follow_resizes() {
        let (mut terminal, mut inbox) = start_sh(TerminalSize::default());
        wait_for(&mut terminal, &mut inbox, |terminal| {
            terminal.screen_text().starts_with('$')
        })
        .await;
        let first = terminal.frame();
        assert_eq!(frame_changes(&first, &terminal.frame()), None);

        terminal.input(TerminalInput::Bytes(b"echo hello\n".to_vec()));
        wait_for(&mut terminal, &mut inbox, |terminal| {
            terminal.screen_text().contains("hello\n$")
        })
        .await;
        let changes = frame_changes(&first, &terminal.frame()).expect("changes");
        assert!(!changes.full);
        assert!(changes.lines.len() <= 3, "{:?}", changes.lines);
        let mut copy = first.clone();
        copy.apply(changes);
        assert_eq!(copy, terminal.frame());

        terminal.input(TerminalInput::Resize {
            columns: 40,
            screen_lines: 10,
            cell_width: 8,
            cell_height: 16,
        });
        let resized = frame_changes(&copy, &terminal.frame()).expect("a resize");
        assert!(resized.full);
        assert_eq!((resized.columns, resized.screen_lines), (40, 10));
        terminal.input(TerminalInput::Bytes(b"stty size\n".to_vec()));
        wait_for(&mut terminal, &mut inbox, |terminal| {
            terminal.screen_text().contains("10 40")
        })
        .await;
    }

    #[tokio::test]
    async fn selects_scrolls_and_pastes() {
        let (mut terminal, mut inbox) = start_sh(TerminalSize {
            columns: 40,
            screen_lines: 5,
            ..TerminalSize::default()
        });
        terminal.input(TerminalInput::Bytes(
            b"for i in 1 2 3 4 5 6 7 8; do echo line$i; done\n".to_vec(),
        ));
        wait_for(&mut terminal, &mut inbox, |terminal| {
            terminal.screen_text().contains("line8\n$")
        })
        .await;
        assert!(terminal.frame().history_lines > 0);

        terminal.input(TerminalInput::Scroll(TerminalScroll::Top));
        let frame = terminal.frame();
        assert!(frame.display_offset > 0);
        assert!(frame.cursor.is_none() || frame.display_offset < 5);

        let top = -(frame.display_offset as i32);
        terminal.input(TerminalInput::Select(Some(
            agentz_protocol::terminal::TerminalSelectionUpdate {
                point: TerminalPoint {
                    line: top,
                    column: 0,
                },
                right_half: false,
                start: Some(TerminalSelectionKind::Lines),
            },
        )));
        assert!(terminal.frame().selection.is_some());
        let selected = terminal.selection_text().expect("selected text");
        assert!(selected.contains("for i in"), "{selected:?}");

        terminal.input(TerminalInput::Paste("echo pasted\n".into()));
        let frame = terminal.frame();
        assert_eq!(frame.display_offset, 0);
        assert_eq!(frame.selection, None);
        wait_for(&mut terminal, &mut inbox, |terminal| {
            terminal.screen_text().contains("\npasted\n")
        })
        .await;
    }

    #[test]
    fn default_colors_follow_xterm() {
        assert_eq!(default_color(1), [205, 0, 0]);
        assert_eq!(default_color(16), [0, 0, 0]);
        assert_eq!(default_color(231), [255, 255, 255]);
        assert_eq!(default_color(244), [128, 128, 128]);
        assert_eq!(default_color(257), [0, 0, 0]);
    }

    #[test]
    fn exit_statuses() {
        use std::os::unix::process::ExitStatusExt as _;
        assert_eq!(
            exit_from_status(std::process::ExitStatus::from_raw(2 << 8)),
            TerminalExit {
                code: Some(2),
                signal: None
            }
        );
        let killed = exit_from_status(std::process::ExitStatus::from_raw(9));
        assert_eq!(killed.signal.as_deref(), Some("SIGKILL"));
    }
}
