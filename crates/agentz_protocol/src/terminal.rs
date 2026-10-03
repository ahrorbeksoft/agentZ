//! Terminals the server runs, and their screens as clients draw them. The model follows Zed's
//! `terminal` crate (`Content`, `Modes`, `Cursor`), cut down to what crosses the wire: lines of
//! styled runs rather than cells, and only the lines that changed.

use std::path::PathBuf;

use projects::ThreadId;
use serde::{Deserialize, Serialize};

use crate::layout::PaneId;

pub use projects::TerminalCommand;

/// Which terminal: they're named by what they belong to, so a client can open a thread's
/// terminal without asking for an id first, and the server starts it on demand.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum TerminalKey {
    /// A terminal thread's terminal.
    Thread(ThreadId),
    /// The drawer under an agent thread: its first terminal (t3code's "Terminal 1").
    Drawer(ThreadId),
    /// Another of the drawer's terminals, numbered from 2 (t3code's split and new terminals).
    DrawerTerminal { thread_id: ThreadId, number: u32 },
    /// One a thread's agent started through ACP's `terminal/create`, by the id the agent got.
    Agent {
        thread_id: ThreadId,
        terminal_id: String,
    },
    /// A terminal pane in the Workspaces view.
    Pane(PaneId),
    /// Where an agent connection's terminal login runs ([`crate::Request::TerminalLogin`]).
    Login(crate::ConnectionId),
}

impl TerminalKey {
    /// A thread's drawer terminal by its number, 1 being the drawer's first.
    pub fn drawer(thread_id: ThreadId, number: u32) -> Self {
        if number <= 1 {
            Self::Drawer(thread_id)
        } else {
            Self::DrawerTerminal { thread_id, number }
        }
    }

    /// The thread it belongs to; a pane's belongs to none.
    pub fn thread_id(&self) -> Option<ThreadId> {
        match self {
            Self::Thread(thread_id) | Self::Drawer(thread_id) => Some(*thread_id),
            Self::DrawerTerminal { thread_id, .. } => Some(*thread_id),
            Self::Agent { thread_id, .. } => Some(*thread_id),
            Self::Login(crate::ConnectionId::Thread(thread_id)) => Some(*thread_id),
            Self::Pane(_) | Self::Login(crate::ConnectionId::Account(_)) => None,
        }
    }
}

/// An agent CLI found on the server's `PATH`, for New Thread › Terminal.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TerminalProgram {
    /// The command's name, such as `claude`.
    pub command: String,
    /// What New Thread shows, such as "Claude Code".
    pub label: String,
    pub path: PathBuf,
}

/// The terminal modes a client needs to turn keys, mouse and paste into bytes (Zed's `Modes`,
/// with the same bits).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TerminalModes(pub u32);

impl TerminalModes {
    pub const NONE: Self = Self(0);
    pub const APP_CURSOR: Self = Self(1 << 0);
    pub const APP_KEYPAD: Self = Self(1 << 1);
    pub const SHOW_CURSOR: Self = Self(1 << 2);
    pub const LINE_WRAP: Self = Self(1 << 3);
    pub const ORIGIN: Self = Self(1 << 4);
    pub const INSERT: Self = Self(1 << 5);
    pub const LINE_FEED_NEW_LINE: Self = Self(1 << 6);
    pub const FOCUS_IN_OUT: Self = Self(1 << 7);
    pub const ALTERNATE_SCROLL: Self = Self(1 << 8);
    pub const BRACKETED_PASTE: Self = Self(1 << 9);
    pub const SGR_MOUSE: Self = Self(1 << 10);
    pub const UTF8_MOUSE: Self = Self(1 << 11);
    pub const ALT_SCREEN: Self = Self(1 << 12);
    pub const MOUSE_REPORT_CLICK: Self = Self(1 << 13);
    pub const MOUSE_DRAG: Self = Self(1 << 14);
    pub const MOUSE_MOTION: Self = Self(1 << 15);
    pub const VI: Self = Self(1 << 16);
    pub const MOUSE_MODE: Self =
        Self(Self::MOUSE_REPORT_CLICK.0 | Self::MOUSE_DRAG.0 | Self::MOUSE_MOTION.0);

    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    pub const fn intersects(self, other: Self) -> bool {
        self.0 & other.0 != 0
    }

    pub fn insert(&mut self, other: Self) {
        self.0 |= other.0;
    }
}

/// A cell color, resolved by the client against its theme. Named colors use alacritty's
/// numbering: 0–15 the ANSI colors, 256 foreground, 257 background, 258 cursor, 259–266 the dim
/// colors, 267 bright foreground, 268 dim foreground.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TerminalColor {
    Named(u16),
    Indexed(u8),
    Rgb(u8, u8, u8),
}

impl TerminalColor {
    pub const FOREGROUND: Self = Self::Named(256);
    pub const BACKGROUND: Self = Self::Named(257);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TerminalStyle {
    pub foreground: TerminalColor,
    pub background: TerminalColor,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub flags: u16,
}

impl TerminalStyle {
    pub const BOLD: u16 = 1 << 0;
    pub const ITALIC: u16 = 1 << 1;
    pub const UNDERLINE: u16 = 1 << 2;
    pub const UNDERCURL: u16 = 1 << 3;
    pub const STRIKEOUT: u16 = 1 << 4;
    pub const INVERSE: u16 = 1 << 5;
    pub const DIM: u16 = 1 << 6;
    pub const HIDDEN: u16 = 1 << 7;
    /// A run of one double-width character, which covers two columns.
    pub const WIDE: u16 = 1 << 8;
    /// A run of one character followed by the zero-width characters that combine with it,
    /// which covers one column, or two with [`Self::WIDE`].
    pub const COMBINING: u16 = 1 << 9;

    pub fn has(&self, flag: u16) -> bool {
        self.flags & flag != 0
    }
}

impl Default for TerminalStyle {
    fn default() -> Self {
        Self {
            foreground: TerminalColor::FOREGROUND,
            background: TerminalColor::BACKGROUND,
            flags: 0,
        }
    }
}

fn is_zero(value: &u16) -> bool {
    *value == 0
}

fn is_default_style(style: &TerminalStyle) -> bool {
    *style == TerminalStyle::default()
}

/// Cells that share a style, from `column`. A wide character is a run of its own, flagged
/// [`TerminalStyle::WIDE`], so a client places each run on the grid rather than trusting the
/// font's advances. Zero-width characters stay with the character they combine with.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TerminalRun {
    pub column: u16,
    pub text: String,
    #[serde(default, skip_serializing_if = "is_default_style")]
    pub style: TerminalStyle,
}

/// One screen line. Blank cells in the default style are left out.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TerminalLine {
    pub runs: Vec<TerminalRun>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum TerminalCursorShape {
    #[default]
    Block,
    Underline,
    Bar,
    HollowBlock,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TerminalCursor {
    /// The screen row, counted from the top of what's shown.
    pub row: u16,
    pub column: u16,
    pub shape: TerminalCursorShape,
}

/// A grid position, as alacritty numbers lines: 0 is the top of the screen when it isn't
/// scrolled back, and history lines are negative.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct TerminalPoint {
    pub line: i32,
    pub column: u16,
}

/// Selected cells, inclusive.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TerminalSelection {
    pub start: TerminalPoint,
    pub end: TerminalPoint,
    pub is_block: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum TerminalSelectionKind {
    #[default]
    Simple,
    /// A word, as a double click selects.
    Semantic,
    /// Whole lines, as a triple click selects.
    Lines,
    Block,
}

/// What a client sees of a terminal. The first frame a subscriber gets is `full`; later ones
/// carry only the lines that changed, but always the rest.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TerminalFrame {
    pub full: bool,
    pub columns: u16,
    pub screen_lines: u16,
    /// Lines by screen row.
    pub lines: Vec<(u16, TerminalLine)>,
    /// `None` when the cursor is hidden or scrolled out of view.
    pub cursor: Option<TerminalCursor>,
    pub modes: TerminalModes,
    /// How far the view is scrolled back into history.
    pub display_offset: u32,
    pub history_lines: u32,
    pub selection: Option<TerminalSelection>,
    pub title: Option<String>,
    /// Set once the process has exited, with its code when it had one.
    pub exited: Option<TerminalExit>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TerminalExit {
    pub code: Option<u32>,
    /// The signal that ended the process, by name.
    #[serde(default)]
    pub signal: Option<String>,
}

/// How a terminal moves through its history.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TerminalScroll {
    /// Positive scrolls back into history.
    Lines(i32),
    PageUp,
    PageDown,
    Top,
    Bottom,
}

/// What a client sends a terminal: [`crate::Request::TerminalInput`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum TerminalInput {
    /// Bytes for the process, from keys or mouse reports the client encoded.
    Bytes(Vec<u8>),
    /// Text to paste, bracketed when the terminal asks for it.
    Paste(String),
    Resize {
        columns: u16,
        screen_lines: u16,
        cell_width: u16,
        cell_height: u16,
    },
    Scroll(TerminalScroll),
    /// Starts or extends a selection; `None` clears it.
    Select(Option<TerminalSelectionUpdate>),
    SelectAll,
    /// Clears the history and the screen but for the cursor's line, which moves to the top.
    Clear,
    /// Whether the terminal has keyboard focus, for programs that asked to know.
    Focus(bool),
    /// The client's theme colors, numbered as [`TerminalColor::Named`] numbers them (0–268),
    /// so the terminal can answer programs that ask for them.
    Palette(Vec<[u8; 3]>),
    /// From a newer client.
    #[serde(untagged)]
    Unknown(serde_json::Value),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TerminalSelectionUpdate {
    pub point: TerminalPoint,
    /// Which half of the cell the pointer is on: selections include a cell once the pointer
    /// passes its middle.
    pub right_half: bool,
    /// Set for the first point; later updates extend the selection.
    pub start: Option<TerminalSelectionKind>,
}

impl TerminalFrame {
    /// Applies a later frame on top of this one, as a client keeps its copy. Afterwards this
    /// frame is full: it has every row, in order.
    pub fn apply(&mut self, frame: TerminalFrame) {
        let mut rows = if frame.full
            || frame.columns != self.columns
            || frame.screen_lines != self.screen_lines
        {
            vec![TerminalLine::default(); frame.screen_lines as usize]
        } else {
            let mut rows = vec![TerminalLine::default(); frame.screen_lines as usize];
            for (row, line) in std::mem::take(&mut self.lines) {
                if let Some(slot) = rows.get_mut(row as usize) {
                    *slot = line;
                }
            }
            rows
        };
        for (row, line) in frame.lines {
            if let Some(slot) = rows.get_mut(row as usize) {
                *slot = line;
            }
        }
        self.full = true;
        self.columns = frame.columns;
        self.screen_lines = frame.screen_lines;
        self.lines = rows
            .into_iter()
            .enumerate()
            .map(|(row, line)| (row as u16, line))
            .collect();
        self.cursor = frame.cursor;
        self.modes = frame.modes;
        self.display_offset = frame.display_offset;
        self.history_lines = frame.history_lines;
        self.selection = frame.selection;
        self.title = frame.title;
        self.exited = frame.exited;
    }

    pub fn line(&self, row: u16) -> Option<&TerminalLine> {
        // A full frame has every row in order, so the row is usually its index.
        match self.lines.get(row as usize) {
            Some((line_row, line)) if *line_row == row => Some(line),
            _ => self
                .lines
                .iter()
                .find(|(line_row, _)| *line_row == row)
                .map(|(_, line)| line),
        }
    }

    /// The screen as plain text, one line per row with trailing blanks trimmed. For tests and
    /// for `agentz_terminal_read`.
    pub fn text(&self) -> String {
        let mut text = String::new();
        for row in 0..self.screen_lines {
            if row > 0 {
                text.push('\n');
            }
            if let Some(line) = self.line(row) {
                text.push_str(&line.text());
            }
        }
        text.trim_end().to_string()
    }
}

impl TerminalLine {
    /// The line's text, with the gaps between runs filled with spaces.
    pub fn text(&self) -> String {
        let mut text = String::new();
        let mut column = 0usize;
        for run in &self.runs {
            let start = run.column as usize;
            if start > column {
                text.extend(std::iter::repeat_n(' ', start - column));
                column = start;
            }
            text.push_str(&run.text);
            column += run.columns();
        }
        text.trim_end().to_string()
    }
}

impl TerminalRun {
    /// Grid columns the run covers.
    pub fn columns(&self) -> usize {
        if self.style.has(TerminalStyle::WIDE) {
            2
        } else if self.style.has(TerminalStyle::COMBINING) {
            1
        } else {
            self.text.chars().count()
        }
    }
}
