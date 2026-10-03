//! The element that paints a terminal, from Zed's `terminal_view::terminal_element`: the grid
//! batched into text runs, backgrounds merged into rectangles, block characters drawn as
//! rectangles so they tile, then the selection and the cursor. The cells come from the frames
//! the server sends rather than from alacritty, and the cursor and selection are plain quads
//! because there's no editor to borrow them from.

use agentz_protocol::terminal::{
    TerminalColor, TerminalCursorShape, TerminalFrame, TerminalModes, TerminalRun, TerminalStyle,
};
use gpui::{
    App, BorderStyle, Bounds, ContentMask, DispatchPhase, Element, ElementId, Entity, FocusHandle,
    Font, FontFeatures, FontStyle, FontWeight, GlobalElementId, Hitbox, Hsla, InputHandler,
    InteractiveElement, Interactivity, IntoElement, LayoutId, MouseButton, MouseMoveEvent, Pixels,
    Point, Rgba, ShapedLine, StrikethroughStyle, TextRun, UTF16Selection, UnderlineStyle, Window,
    fill, outline, point, px, relative, size,
};
use theme::{ActiveTheme as _, Theme};
use ui::utils::ensure_minimum_contrast;
use util::ResultExt as _;

use crate::terminal_entity::{Terminal, TerminalSize};
use crate::terminal_mouse::TerminalBounds;
use crate::terminal_view::TerminalView;

/// Zed's `terminal.line_height` "standard". Its default, "comfortable" (1.618), spaced lines
/// further apart than the user wanted.
const LINE_HEIGHT: f32 = 1.3;
/// t3code's default terminal font size, smaller than the code font's.
const FONT_SIZE: Pixels = px(12.);
/// Zed's `terminal.minimum_contrast` default.
const MINIMUM_CONTRAST: f32 = 45.;

/// Block element glyphs are painted on a subcell grid: each terminal cell is
/// divided into 8 columns (for eighth blocks) and 24 lines (LCM of the 8-way
/// splits of eighth blocks and the 3-way splits of sextants).
const BLOCK_SUBCELL_COLUMNS: i32 = 8;
const BLOCK_SUBCELL_LINES: i32 = 24;

/// How a terminal sits in its parent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TerminalMode {
    /// Fills its parent, which sizes the terminal, and scrolls through its history.
    Scrollable,
    /// As tall as its output, up to `max_lines`, as a tool call shows a command. The terminal
    /// keeps its own height and the parent scrolls.
    Inline { max_lines: usize },
}

/// Where the grid is drawn, kept by the view for mouse events.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GridLayout {
    pub bounds: TerminalBounds,
    /// The screen row drawn first: inline terminals skip the rows above their output.
    pub first_row: u16,
}

impl GridLayout {
    /// The bounds the mouse functions expect, which start at screen row 0.
    pub fn mouse_bounds(&self) -> TerminalBounds {
        let offset = self.bounds.line_height * self.first_row as f32;
        let mut bounds = self.bounds;
        bounds.bounds.origin.y -= offset;
        bounds.bounds.size.height += offset;
        bounds
    }
}

/// The information generated during layout that is necessary for painting.
pub struct LayoutState {
    hitbox: Hitbox,
    batched_text_runs: Vec<BatchedTextRun>,
    block_element_rects: Vec<BlockElementLayoutRect>,
    rects: Vec<LayoutRect>,
    selection_rects: Vec<Bounds<Pixels>>,
    selection_color: Hsla,
    cursor: Option<CursorLayout>,
    ime_cursor_bounds: Option<Bounds<Pixels>>,
    background_color: Hsla,
    dimensions: TerminalBounds,
    font: Font,
    font_size: Pixels,
    foreground: Hsla,
}

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct LayoutPoint {
    line: i32,
    column: i32,
}

impl LayoutPoint {
    fn new(line: i32, column: i32) -> Self {
        Self { line, column }
    }
}

/// A batched text run that combines multiple adjacent cells with the same style
#[derive(Debug)]
pub struct BatchedTextRun {
    pub start_point: LayoutPoint,
    pub text: String,
    pub cell_count: usize,
    pub style: TextRun,
}

impl BatchedTextRun {
    fn new(start_point: LayoutPoint, text: &str, style: TextRun) -> Self {
        let mut batch = BatchedTextRun {
            start_point,
            text: String::with_capacity(100),
            cell_count: 0,
            style: TextRun { len: 0, ..style },
        };
        batch.append(text);
        batch
    }

    fn can_append(&self, other_style: &TextRun) -> bool {
        self.style.font == other_style.font
            && self.style.color == other_style.color
            && self.style.background_color == other_style.background_color
            && self.style.underline == other_style.underline
            && self.style.strikethrough == other_style.strikethrough
    }

    /// Appends one cell: a character, with any zero-width characters that combine with it.
    fn append(&mut self, text: &str) {
        self.text.push_str(text);
        self.cell_count += 1;
        self.style.len += text.len();
    }

    fn paint(
        &self,
        origin: Point<Pixels>,
        dimensions: &TerminalBounds,
        font_size: Pixels,
        window: &mut Window,
        cx: &mut App,
    ) {
        let position = point(
            origin.x + self.start_point.column as f32 * dimensions.cell_width,
            origin.y + self.start_point.line as f32 * dimensions.line_height,
        );
        window
            .text_system()
            .shape_line(
                self.text.clone().into(),
                font_size,
                std::slice::from_ref(&self.style),
                Some(dimensions.cell_width),
            )
            .paint(
                position,
                dimensions.line_height,
                gpui::TextAlign::Left,
                None,
                window,
                cx,
            )
            .log_err();
    }
}

#[derive(Clone, Debug)]
pub struct BlockElementLayoutRect {
    point: LayoutPoint,
    num_of_columns: usize,
    num_of_lines: usize,
    color: Hsla,
}

impl BlockElementLayoutRect {
    fn paint(&self, origin: Point<Pixels>, dimensions: &TerminalBounds, window: &mut Window) {
        let subcell_width = dimensions.cell_width / BLOCK_SUBCELL_COLUMNS as f32;
        let subcell_height = dimensions.line_height / BLOCK_SUBCELL_LINES as f32;
        let position = point(
            origin.x + self.point.column as f32 * subcell_width,
            origin.y + self.point.line as f32 * subcell_height,
        );
        let size = size(
            subcell_width * self.num_of_columns as f32,
            subcell_height * self.num_of_lines as f32,
        );
        window.paint_quad(fill(Bounds::new(position, size), self.color));
    }
}

#[derive(Clone, Debug, Default)]
pub struct LayoutRect {
    point: LayoutPoint,
    num_of_cells: usize,
    color: Hsla,
}

impl LayoutRect {
    fn paint(&self, origin: Point<Pixels>, dimensions: &TerminalBounds, window: &mut Window) {
        let position = point(
            (origin.x + self.point.column as f32 * dimensions.cell_width).floor(),
            origin.y + self.point.line as f32 * dimensions.line_height,
        );
        let size = size(
            (dimensions.cell_width * self.num_of_cells as f32).ceil(),
            dimensions.line_height,
        );
        window.paint_quad(fill(Bounds::new(position, size), self.color));
    }
}

/// Represents a rectangular region with a specific color on a logical grid.
#[derive(Debug, Clone, PartialEq)]
struct BackgroundRegion {
    start_line: i32,
    start_col: i32,
    end_line: i32,
    end_col: i32,
    color: Hsla,
}

impl BackgroundRegion {
    fn new(line: i32, col: i32, color: Hsla) -> Self {
        Self::with_extents(line, col, line, col, color)
    }

    fn with_extents(
        start_line: i32,
        start_col: i32,
        end_line: i32,
        end_col: i32,
        color: Hsla,
    ) -> Self {
        BackgroundRegion {
            start_line,
            start_col,
            end_line,
            end_col,
            color,
        }
    }

    fn can_merge_with(&self, other: &BackgroundRegion) -> bool {
        if self.color != other.color {
            return false;
        }
        if self.start_line == other.start_line && self.end_line == other.end_line {
            return self.end_col + 1 == other.start_col || other.end_col + 1 == self.start_col;
        }
        if self.start_col == other.start_col && self.end_col == other.end_col {
            return self.end_line + 1 == other.start_line || other.end_line + 1 == self.start_line;
        }
        false
    }

    fn merge_with(&mut self, other: &BackgroundRegion) {
        self.start_line = self.start_line.min(other.start_line);
        self.start_col = self.start_col.min(other.start_col);
        self.end_line = self.end_line.max(other.end_line);
        self.end_col = self.end_col.max(other.end_col);
    }
}

/// Merge grid regions to minimize the number of rectangles.
fn merge_background_regions(regions: Vec<BackgroundRegion>) -> Vec<BackgroundRegion> {
    let mut merged = regions;
    let mut changed = true;
    while changed {
        changed = false;
        let mut i = 0;
        while i < merged.len() {
            let mut j = i + 1;
            while j < merged.len() {
                if merged[i].can_merge_with(&merged[j]) {
                    let other = merged.remove(j);
                    merged[i].merge_with(&other);
                    changed = true;
                } else {
                    j += 1;
                }
            }
            i += 1;
        }
    }
    merged
}

/// One grid cell of a run: a character with the zero-width characters that combine with it.
struct LayoutCell<'a> {
    column: u16,
    text: &'a str,
    columns: u16,
}

fn run_cells(run: &TerminalRun) -> Vec<LayoutCell<'_>> {
    if run.style.has(TerminalStyle::WIDE) || run.style.has(TerminalStyle::COMBINING) {
        return vec![LayoutCell {
            column: run.column,
            text: &run.text,
            columns: run.columns() as u16,
        }];
    }
    run.text
        .char_indices()
        .enumerate()
        .map(|(index, (offset, character))| LayoutCell {
            column: run.column.saturating_add(index as u16),
            text: &run.text[offset..offset + character.len_utf8()],
            columns: 1,
        })
        .collect()
}

pub struct GridStyle {
    pub font: Font,
    pub minimum_contrast: f32,
}

/// Lays out screen rows `rows` of the frame, drawn from line 0.
pub fn layout_grid(
    frame: &TerminalFrame,
    rows: std::ops::Range<u16>,
    grid_style: &GridStyle,
    theme: &Theme,
) -> (
    Vec<LayoutRect>,
    Vec<BatchedTextRun>,
    Vec<BlockElementLayoutRect>,
) {
    let mut batched_runs = Vec::new();
    let mut block_element_regions = Vec::new();
    let mut background_regions: Vec<BackgroundRegion> = Vec::new();
    let mut current_batch: Option<BatchedTextRun> = None;

    for (display_line, row) in rows.enumerate() {
        let display_line = display_line as i32;
        if let Some(batch) = current_batch.take() {
            batched_runs.push(batch);
        }
        let Some(line) = frame.line(row) else {
            continue;
        };
        for run in &line.runs {
            let style = run.style;
            let (fg, bg) = if style.has(TerminalStyle::INVERSE) {
                (style.background, style.foreground)
            } else {
                (style.foreground, style.background)
            };
            for cell in run_cells(run) {
                if bg != TerminalColor::BACKGROUND {
                    let color = convert_color(bg, theme);
                    for column in cell.column..cell.column.saturating_add(cell.columns) {
                        let col = column as i32;
                        if let Some(last_region) = background_regions.last_mut()
                            && last_region.color == color
                            && last_region.start_line == display_line
                            && last_region.end_line == display_line
                            && last_region.end_col + 1 == col
                        {
                            last_region.end_col = col;
                        } else {
                            background_regions.push(BackgroundRegion::new(
                                display_line,
                                col,
                                color,
                            ));
                        }
                    }
                }

                if style.has(TerminalStyle::HIDDEN) || is_blank(cell.text, style) {
                    continue;
                }
                let first_char = cell.text.chars().next().unwrap_or(' ');
                let cell_style = cell_style(first_char, style, fg, bg, theme, grid_style);
                let cell_point = LayoutPoint::new(display_line, cell.column as i32);
                if collect_block_element_regions(
                    cell_point,
                    first_char,
                    cell_style.color,
                    &mut block_element_regions,
                ) {
                    if let Some(batch) = current_batch.take() {
                        batched_runs.push(batch);
                    }
                    continue;
                }

                match current_batch.as_mut() {
                    Some(batch)
                        if batch.can_append(&cell_style)
                            && batch.start_point.line == cell_point.line
                            && batch.start_point.column + batch.cell_count as i32
                                == cell_point.column =>
                    {
                        batch.append(cell.text);
                    }
                    _ => {
                        if let Some(batch) = current_batch.take() {
                            batched_runs.push(batch);
                        }
                        current_batch =
                            Some(BatchedTextRun::new(cell_point, cell.text, cell_style));
                    }
                }
            }
        }
    }
    if let Some(batch) = current_batch {
        batched_runs.push(batch);
    }

    let mut rects = Vec::new();
    for region in merge_background_regions(background_regions) {
        for line in region.start_line..=region.end_line {
            rects.push(LayoutRect {
                point: LayoutPoint::new(line, region.start_col),
                num_of_cells: (region.end_col - region.start_col + 1) as usize,
                color: region.color,
            });
        }
    }
    let block_element_rects = merge_background_regions(block_element_regions)
        .into_iter()
        .map(|region| BlockElementLayoutRect {
            point: LayoutPoint::new(region.start_line, region.start_col),
            num_of_columns: (region.end_col - region.start_col + 1) as usize,
            num_of_lines: (region.end_line - region.start_line + 1) as usize,
            color: region.color,
        })
        .collect();
    (rects, batched_runs, block_element_rects)
}

fn is_blank(text: &str, style: TerminalStyle) -> bool {
    text == " "
        && !style.has(TerminalStyle::UNDERLINE)
        && !style.has(TerminalStyle::UNDERCURL)
        && !style.has(TerminalStyle::STRIKEOUT)
}

/// Checks if a character is a decorative block/box-like character that should
/// preserve its exact colors without contrast adjustment.
///
/// This specifically targets characters used as visual connectors, separators,
/// and borders where color matching with adjacent backgrounds is critical.
/// Regular icons (git, folders, etc.) are excluded as they need to remain readable.
fn is_decorative_character(ch: char) -> bool {
    matches!(
        ch as u32,
        // Unicode Box Drawing and Block Elements
        0x2500..=0x257F // Box Drawing (└ ┐ ─ │ etc.)
        | 0x2580..=0x259F // Block Elements (▀ ▄ █ ░ ▒ ▓ etc.)
        | 0x25A0..=0x25FF // Geometric Shapes (■ ▶ ● etc. - includes triangular/circular separators)
        | 0x1FB00..=0x1FB3B // Symbols for Legacy Computing sextants used by terminal QR renderers

        // Private Use Area - Powerline separator symbols only
        | 0xE0B0..=0xE0B7 // Powerline separators: triangles (E0B0-E0B3) and half circles (E0B4-E0B7)
        | 0xE0B8..=0xE0BF // Powerline separators: corner triangles
        | 0xE0C0..=0xE0CA // Powerline separators: flames (E0C0-E0C3), pixelated (E0C4-E0C7), and ice (E0C8 & E0CA)
        | 0xE0CC..=0xE0D1 // Powerline separators: honeycombs (E0CC-E0CD) and lego (E0CE-E0D1)
        | 0xE0D2..=0xE0D7 // Powerline separators: trapezoid (E0D2 & E0D4) and inverted triangles (E0D6-E0D7)
    )
}

/// Whether the application explicitly picked this foreground color and does not
/// want it adjusted for contrast: 24-bit true color or a 256-color palette entry
/// past the 16 theme-defined ANSI colors.
fn is_app_chosen_exact_color(color: TerminalColor) -> bool {
    matches!(
        color,
        TerminalColor::Rgb(..) | TerminalColor::Indexed(16..=255)
    )
}

/// Returns the filled subcells of a sextant character as a bitmap, where
/// bit `row * 2 + column` is set when that 2x3 subcell is filled.
///
/// U+1FB00..=U+1FB3B enumerate all 2x3 fill combinations except the four
/// that already exist as Block Elements (empty, `▌` = 0b010101,
/// `▐` = 0b101010, and `█` = 0b111111), hence the gap adjustments.
fn sextant_char_to_filled_bits(ch: char) -> Option<u8> {
    let offset = (ch as u32).checked_sub(0x1FB00)?;
    if offset > 0x3B {
        return None;
    }
    Some((offset + 1 + u32::from(offset >= 20) + u32::from(offset >= 40)) as u8)
}

/// Returns the filled quadrants of a quadrant character as a bitmap, where
/// bit `row * 2 + column` is set when that 2x2 subcell is filled.
fn quadrant_char_to_filled_bits(ch: char) -> Option<u8> {
    Some(match ch {
        '▘' => 0b0001,
        '▝' => 0b0010,
        '▖' => 0b0100,
        '▗' => 0b1000,
        '▚' => 0b1001,
        '▞' => 0b0110,
        '▛' => 0b0111,
        '▜' => 0b1011,
        '▙' => 0b1101,
        '▟' => 0b1110,
        _ => return None,
    })
}

/// Returns `(column, line, num_of_columns, num_of_lines)` in subcell units
/// for block element characters that consist of a single rectangle.
fn block_char_to_rect(ch: char) -> Option<(i32, i32, i32, i32)> {
    let codepoint = ch as u32;
    Some(match codepoint {
        // ▀ upper half
        0x2580 => (0, 0, 8, 12),
        // ▁▂▃▄▅▆▇█ lower blocks of 1..=8 eighths
        0x2581..=0x2588 => {
            let eighths = (codepoint - 0x2580) as i32;
            (0, 24 - eighths * 3, 8, eighths * 3)
        }
        // ▉▊▋▌▍▎▏ left blocks of 7..=1 eighths
        0x2589..=0x258F => (0, 0, (0x2590 - codepoint) as i32, 24),
        // ▐ right half
        0x2590 => (4, 0, 4, 24),
        // ▔ upper eighth
        0x2594 => (0, 0, 8, 3),
        // ▕ right eighth
        0x2595 => (7, 0, 1, 24),
        _ => return None,
    })
}

/// Approximates the shade characters `░▒▓` with the foreground color at
/// reduced opacity instead of the stipple patterns fonts use, trading
/// pattern fidelity for seamless cell coverage.
fn shade_char_to_opacity(ch: char) -> Option<f32> {
    match ch {
        '░' => Some(0.25),
        '▒' => Some(0.5),
        '▓' => Some(0.75),
        _ => None,
    }
}

fn collect_block_element_regions(
    point: LayoutPoint,
    ch: char,
    color: Hsla,
    regions: &mut Vec<BackgroundRegion>,
) -> bool {
    if let Some((column, line, num_of_columns, num_of_lines)) = block_char_to_rect(ch) {
        push_block_element_region(
            point,
            column,
            line,
            num_of_columns,
            num_of_lines,
            color,
            regions,
        );
        return true;
    }

    if let Some(filled) = quadrant_char_to_filled_bits(ch) {
        for row in 0..2 {
            for column in 0..2 {
                if filled & (1 << (row * 2 + column)) != 0 {
                    push_block_element_region(point, column * 4, row * 12, 4, 12, color, regions);
                }
            }
        }
        return true;
    }

    if let Some(filled) = sextant_char_to_filled_bits(ch) {
        for row in 0..3 {
            for column in 0..2 {
                if filled & (1 << (row * 2 + column)) != 0 {
                    push_block_element_region(point, column * 4, row * 8, 4, 8, color, regions);
                }
            }
        }
        return true;
    }

    if let Some(opacity) = shade_char_to_opacity(ch) {
        push_block_element_region(point, 0, 0, 8, 24, color.opacity(opacity), regions);
        return true;
    }

    false
}

fn push_block_element_region(
    point: LayoutPoint,
    column: i32,
    line: i32,
    num_of_columns: i32,
    num_of_lines: i32,
    color: Hsla,
    regions: &mut Vec<BackgroundRegion>,
) {
    let start_line = point.line * BLOCK_SUBCELL_LINES + line;
    let start_col = point.column * BLOCK_SUBCELL_COLUMNS + column;
    let end_line = start_line + num_of_lines - 1;
    let end_col = start_col + num_of_columns - 1;

    // Extend the previous region when possible (e.g. runs of `█` in a QR
    // code) to keep the quadratic merge pass over a small input.
    if let Some(last_region) = regions.last_mut()
        && last_region.color == color
        && last_region.start_line == start_line
        && last_region.end_line == end_line
        && last_region.end_col + 1 == start_col
    {
        last_region.end_col = end_col;
        return;
    }

    regions.push(BackgroundRegion::with_extents(
        start_line, start_col, end_line, end_col, color,
    ));
}

/// Converts a cell's style to a GPUI text run.
fn cell_style(
    character: char,
    style: TerminalStyle,
    fg: TerminalColor,
    bg: TerminalColor,
    theme: &Theme,
    grid_style: &GridStyle,
) -> TextRun {
    let skip_contrast = is_app_chosen_exact_color(fg);
    let mut fg = convert_color(fg, theme);
    let bg = convert_color(bg, theme);

    if !skip_contrast && !is_decorative_character(character) {
        fg = ensure_minimum_contrast(fg, bg, grid_style.minimum_contrast);
    }

    // Use a dim multiplier that stays close to the existing Alacritty look.
    if style.has(TerminalStyle::DIM) {
        fg.a *= 0.7;
    }

    let underline = (style.has(TerminalStyle::UNDERLINE) || style.has(TerminalStyle::UNDERCURL))
        .then(|| UnderlineStyle {
            color: Some(fg),
            thickness: px(1.0),
            wavy: style.has(TerminalStyle::UNDERCURL),
        });

    let strikethrough = style
        .has(TerminalStyle::STRIKEOUT)
        .then(|| StrikethroughStyle {
            color: Some(fg),
            thickness: px(1.0),
        });

    let weight = if style.has(TerminalStyle::BOLD) {
        FontWeight::BOLD
    } else {
        grid_style.font.weight
    };
    let font_style = if style.has(TerminalStyle::ITALIC) {
        FontStyle::Italic
    } else {
        FontStyle::Normal
    };

    TextRun {
        len: character.len_utf8(),
        color: fg,
        background_color: None,
        font: Font {
            weight,
            style: font_style,
            ..grid_style.font.clone()
        },
        underline,
        strikethrough,
    }
}

/// A cell color in the theme's terms.
pub fn convert_color(color: TerminalColor, theme: &Theme) -> Hsla {
    match color {
        // Named colors differ from the palette indices in two places, as alacritty's do.
        TerminalColor::Named(257) => theme.colors().terminal_ansi_background,
        TerminalColor::Named(268) => theme.colors().terminal_dim_foreground,
        TerminalColor::Named(index) => color_at_index(index as usize, theme),
        TerminalColor::Indexed(index) => color_at_index(index as usize, theme),
        TerminalColor::Rgb(r, g, b) => rgba_color(r, g, b),
    }
}

/// Zed's `terminal::get_color_at_index`.
fn color_at_index(index: usize, theme: &Theme) -> Hsla {
    let colors = theme.colors();
    match index {
        0 => colors.terminal_ansi_black,
        1 => colors.terminal_ansi_red,
        2 => colors.terminal_ansi_green,
        3 => colors.terminal_ansi_yellow,
        4 => colors.terminal_ansi_blue,
        5 => colors.terminal_ansi_magenta,
        6 => colors.terminal_ansi_cyan,
        7 => colors.terminal_ansi_white,
        8 => colors.terminal_ansi_bright_black,
        9 => colors.terminal_ansi_bright_red,
        10 => colors.terminal_ansi_bright_green,
        11 => colors.terminal_ansi_bright_yellow,
        12 => colors.terminal_ansi_bright_blue,
        13 => colors.terminal_ansi_bright_magenta,
        14 => colors.terminal_ansi_bright_cyan,
        15 => colors.terminal_ansi_bright_white,
        // 16-231 are a 6x6x6 RGB color cube, mapped to 0-255 using steps defined by XTerm.
        16..=231 => {
            let (r, g, b) = rgb_for_index(index as u8);
            let step = |value: u8| if value == 0 { 0 } else { value * 40 + 55 };
            rgba_color(step(r), step(g), step(b))
        }
        // 232-255 are a 24-step grayscale ramp from (8, 8, 8) to (238, 238, 238).
        232..=255 => {
            let value = (index as u8 - 232) * 10 + 8;
            rgba_color(value, value, value)
        }
        256 => colors.terminal_foreground,
        257 => colors.terminal_background,
        258 => theme.players().local().cursor,
        259 => colors.terminal_ansi_dim_black,
        260 => colors.terminal_ansi_dim_red,
        261 => colors.terminal_ansi_dim_green,
        262 => colors.terminal_ansi_dim_yellow,
        263 => colors.terminal_ansi_dim_blue,
        264 => colors.terminal_ansi_dim_magenta,
        265 => colors.terminal_ansi_dim_cyan,
        266 => colors.terminal_ansi_dim_white,
        267 => colors.terminal_bright_foreground,
        268 => colors.terminal_ansi_black, // 'Dim Background', non-standard color
        _ => gpui::black(),
    }
}

/// The RGB channels in [0, 5] for an index into the 6x6x6 ANSI color cube.
fn rgb_for_index(i: u8) -> (u8, u8, u8) {
    let i = i.saturating_sub(16);
    let r = (i - (i % 36)) / 36;
    let g = ((i % 36) - (i % 6)) / 6;
    let b = (i % 36) % 6;
    (r, g, b)
}

fn rgba_color(r: u8, g: u8, b: u8) -> Hsla {
    Rgba {
        r: r as f32 / 255.,
        g: g as f32 / 255.,
        b: b as f32 / 255.,
        a: 1.,
    }
    .into()
}

/// The theme's 269 terminal colors, for the server to answer programs that ask for them.
pub fn terminal_palette(cx: &App) -> Vec<[u8; 3]> {
    let theme = cx.theme();
    (0..=268)
        .map(|index| {
            let color = Rgba::from(color_at_index(index, theme));
            let channel = |value: f32| (value.clamp(0., 1.) * 255.).round() as u8;
            [channel(color.r), channel(color.g), channel(color.b)]
        })
        .collect()
}

/// The terminal font and the grid cell it gives.
pub struct TerminalMetrics {
    pub font: Font,
    pub font_size: Pixels,
    pub line_height: Pixels,
    pub cell_width: Pixels,
}

impl TerminalMetrics {
    pub fn new(window: &mut Window, cx: &App) -> Self {
        let settings = theme::theme_settings(cx);
        let buffer_font = settings.buffer_font(cx);
        let font = Font {
            family: buffer_font.family.clone(),
            features: FontFeatures::disable_ligatures(),
            fallbacks: buffer_font.fallbacks.clone(),
            weight: buffer_font.weight,
            style: FontStyle::Normal,
        };
        let font_size = FONT_SIZE;
        let line_height = px((f32::from(font_size) * LINE_HEIGHT).round());
        let text_system = window.text_system();
        let font_id = text_system.resolve_font(&font);
        let cell_width = text_system
            .advance(font_id, font_size, 'm')
            .map(|advance| advance.width)
            .unwrap_or(font_size * 0.6);
        Self {
            font,
            font_size,
            line_height,
            cell_width,
        }
    }
}

/// Screen rows an inline terminal shows: through the last one with output, or the cursor.
pub fn inline_rows(frame: &TerminalFrame, max_lines: usize) -> std::ops::Range<u16> {
    let last_output = (0..frame.screen_lines)
        .rev()
        .find(|row| frame.line(*row).is_some_and(|line| !line.text().is_empty()));
    let cursor_row = frame
        .cursor
        .filter(|_| frame.exited.is_none())
        .map(|cursor| cursor.row);
    let end = last_output.max(cursor_row).map_or(1, |row| row + 1);
    let lines = (end as usize).min(max_lines.max(1)) as u16;
    end - lines..end
}

struct CursorLayout {
    bounds: Bounds<Pixels>,
    shape: CursorKind,
    color: Hsla,
    text: Option<ShapedLine>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum CursorKind {
    Block,
    Underline,
    Bar,
    Hollow,
}

impl CursorLayout {
    fn paint(&self, origin: Point<Pixels>, window: &mut Window, cx: &mut App) {
        let bounds = self.bounds + origin;
        match self.shape {
            CursorKind::Block => {
                window.paint_quad(fill(bounds, self.color));
                if let Some(text) = &self.text {
                    text.paint(
                        bounds.origin,
                        bounds.size.height,
                        gpui::TextAlign::Left,
                        None,
                        window,
                        cx,
                    )
                    .log_err();
                }
            }
            CursorKind::Bar => {
                window.paint_quad(fill(
                    Bounds::new(bounds.origin, size(px(2.), bounds.size.height)),
                    self.color,
                ));
            }
            CursorKind::Underline => {
                window.paint_quad(fill(
                    Bounds::new(
                        point(bounds.origin.x, bounds.bottom() - px(2.)),
                        size(bounds.size.width, px(2.)),
                    ),
                    self.color,
                ));
            }
            CursorKind::Hollow => {
                window.paint_quad(outline(bounds, self.color, BorderStyle::Solid));
            }
        }
    }
}

/// The character under the cursor, which a block cursor draws over.
fn character_at(frame: &TerminalFrame, row: u16, column: u16) -> Option<(String, usize)> {
    let line = frame.line(row)?;
    line.runs.iter().find_map(|run| {
        let columns = run.columns() as u16;
        if column < run.column || column >= run.column.saturating_add(columns) {
            return None;
        }
        run_cells(run)
            .into_iter()
            .find(|cell| column >= cell.column && column < cell.column + cell.columns)
            .map(|cell| (cell.text.to_string(), cell.columns as usize))
    })
}

/// The selection as one rectangle per line, relative to the grid's origin.
fn selection_rects(
    frame: &TerminalFrame,
    first_row: u16,
    rows: u16,
    dimensions: &TerminalBounds,
) -> Vec<Bounds<Pixels>> {
    let Some(selection) = frame.selection else {
        return Vec::new();
    };
    let offset = frame.display_offset as i32 - first_row as i32;
    let start_line = selection.start.line.saturating_add(offset);
    let end_line = selection.end.line.saturating_add(offset);
    let last_line = rows as i32 - 1;
    if end_line < 0 || start_line > last_line {
        return Vec::new();
    }
    let columns = frame.columns as usize;
    (start_line.max(0)..=end_line.min(last_line))
        .filter_map(|line| {
            let (start, end) = if selection.is_block {
                (
                    selection.start.column as usize,
                    selection.end.column as usize + 1,
                )
            } else {
                let start = if line == start_line {
                    selection.start.column as usize
                } else {
                    0
                };
                let end = if line == end_line {
                    selection.end.column as usize + 1
                } else {
                    columns
                };
                (start, end)
            };
            (end > start).then(|| {
                Bounds::new(
                    point(
                        dimensions.cell_width * start as f32,
                        dimensions.line_height * line as f32,
                    ),
                    size(
                        dimensions.cell_width * (end - start) as f32,
                        dimensions.line_height,
                    ),
                )
            })
        })
        .collect()
}

/// The GPUI element that paints the terminal.
pub struct TerminalElement {
    terminal: Entity<Terminal>,
    view: Entity<TerminalView>,
    focus: FocusHandle,
    focused: bool,
    mode: TerminalMode,
    interactivity: Interactivity,
}

impl InteractiveElement for TerminalElement {
    fn interactivity(&mut self) -> &mut Interactivity {
        &mut self.interactivity
    }
}

impl TerminalElement {
    pub fn new(
        terminal: Entity<Terminal>,
        view: Entity<TerminalView>,
        focus: FocusHandle,
        focused: bool,
        mode: TerminalMode,
    ) -> Self {
        TerminalElement {
            terminal,
            view,
            focus: focus.clone(),
            focused,
            mode,
            interactivity: Interactivity::default(),
        }
        .track_focus(&focus)
    }

    fn register_mouse_listeners(&mut self, hitbox: &Hitbox, window: &mut Window) {
        let focus = self.focus.clone();
        let view = self.view.clone();

        self.interactivity.on_mouse_down(MouseButton::Left, {
            let focus = focus.clone();
            let view = view.clone();
            move |event, window, cx| {
                window.focus(&focus, cx);
                view.update(cx, |view, cx| view.mouse_down(event, cx));
            }
        });

        window.on_mouse_event({
            let view = view.clone();
            let hitbox = hitbox.clone();
            let focus = focus.clone();
            move |event: &MouseMoveEvent, phase, window, cx| {
                if phase != DispatchPhase::Bubble {
                    return;
                }
                let hovered = hitbox.is_hovered(window);
                if event.pressed_button.is_some()
                    && !cx.has_active_drag()
                    && focus.is_focused(window)
                {
                    view.update(cx, |view, cx| {
                        if view.selection_started() || hovered {
                            view.mouse_drag(event, hitbox.bounds, cx);
                        }
                    });
                }
                if hovered {
                    view.update(cx, |view, cx| view.mouse_move(event, cx));
                }
            }
        });

        for button in [MouseButton::Left, MouseButton::Middle, MouseButton::Right] {
            self.interactivity.on_mouse_up(button, {
                let view = view.clone();
                let focus = focus.clone();
                let terminal = self.terminal.clone();
                move |event, window, cx| {
                    if !focus.is_focused(window) {
                        return;
                    }
                    if button != MouseButton::Left
                        && !terminal
                            .read(cx)
                            .modes()
                            .intersects(TerminalModes::MOUSE_MODE)
                    {
                        return;
                    }
                    view.update(cx, |view, cx| view.mouse_up(event, cx));
                }
            });
        }
        for button in [MouseButton::Middle, MouseButton::Right] {
            self.interactivity.on_mouse_down(button, {
                let view = view.clone();
                let focus = focus.clone();
                let terminal = self.terminal.clone();
                move |event, window, cx| {
                    if button == MouseButton::Right
                        && !terminal
                            .read(cx)
                            .modes()
                            .intersects(TerminalModes::MOUSE_MODE)
                    {
                        return;
                    }
                    window.focus(&focus, cx);
                    view.update(cx, |view, cx| view.mouse_down(event, cx));
                }
            });
        }

        if self.mode == TerminalMode::Scrollable {
            self.interactivity.on_scroll_wheel({
                let view = view.downgrade();
                move |event, _, cx| {
                    view.update(cx, |view, cx| view.scroll_wheel(event, cx))
                        .ok();
                }
            });
        }
    }
}

impl Element for TerminalElement {
    type RequestLayoutState = ();
    type PrepaintState = LayoutState;

    fn id(&self) -> Option<ElementId> {
        self.interactivity.element_id.clone()
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        global_id: Option<&GlobalElementId>,
        inspector_id: Option<&gpui::InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let height: gpui::Length = match self.mode {
            TerminalMode::Inline { max_lines } => {
                let line_height = TerminalMetrics::new(window, cx).line_height;
                let rows = self
                    .terminal
                    .read(cx)
                    .frame()
                    .map_or(1, |frame| inline_rows(frame, max_lines).len());
                // Round up to a whole device pixel so snapping doesn't lose a row.
                let scale_factor = window.scale_factor().max(1.);
                let height = rows as f32 * f32::from(line_height);
                px((height * scale_factor).ceil() / scale_factor).into()
            }
            TerminalMode::Scrollable => relative(1.).into(),
        };
        let layout_id = self.interactivity.request_layout(
            global_id,
            inspector_id,
            window,
            cx,
            |mut style, window, cx| {
                style.size.width = relative(1.).into();
                style.size.height = height;
                window.request_layout(style, None, cx)
            },
        );
        (layout_id, ())
    }

    fn prepaint(
        &mut self,
        global_id: Option<&GlobalElementId>,
        inspector_id: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        self.interactivity.prepaint(
            global_id,
            inspector_id,
            bounds,
            bounds.size,
            window,
            cx,
            |_, _, hitbox, window, cx| {
                let hitbox =
                    hitbox.unwrap_or_else(|| window.insert_hitbox(bounds, Default::default()));
                let metrics = TerminalMetrics::new(window, cx);
                let theme = cx.theme().clone();
                let line_height = metrics.line_height;
                let cell_width = metrics.cell_width;
                let gutter = cell_width;
                let frame = self.terminal.read(cx).frame().cloned();

                let mut grid_size = bounds.size;
                grid_size.width -= gutter;
                // https://github.com/zed-industries/zed/issues/2750
                // if the terminal is one column wide, rendering 🦀
                // causes alacritty to misbehave.
                if grid_size.width < cell_width * 2.0 {
                    grid_size.width = cell_width * 2.0;
                }
                let mut origin = bounds.origin;
                origin.x += gutter;

                let scale_factor = window.scale_factor();
                if self.mode == TerminalMode::Scrollable {
                    let should_anchor_to_bottom = frame.as_ref().is_some_and(|frame| {
                        frame.modes.contains(TerminalModes::ALT_SCREEN)
                            || (frame.display_offset == 0
                                && frame
                                    .line(frame.screen_lines.saturating_sub(1))
                                    .is_some_and(|line| !line.runs.is_empty()))
                    });
                    let line_height_device_px =
                        (f32::from(line_height) * scale_factor).round().max(1.0) as i32;
                    let available_height_device_px = (f32::from(grid_size.height) * scale_factor)
                        .floor()
                        .max(0.0) as i32;
                    let rows = (available_height_device_px / line_height_device_px).max(1);
                    let snapped_height_device_px = rows * line_height_device_px;
                    let padding_device_px =
                        (available_height_device_px - snapped_height_device_px).max(0);
                    grid_size.height = px(snapped_height_device_px as f32 / scale_factor.max(1.0));
                    if should_anchor_to_bottom {
                        origin.y += px(padding_device_px as f32 / scale_factor.max(1.0));
                    }
                }

                // Snap to device pixels to avoid subpixel jitter while resizing.
                let snap_px = |value: Pixels| {
                    Pixels::from((f32::from(value) * scale_factor).floor() / scale_factor)
                };
                origin.x = snap_px(origin.x);
                origin.y = snap_px(origin.y);
                let dimensions = TerminalBounds {
                    cell_width,
                    line_height,
                    bounds: Bounds {
                        origin,
                        size: grid_size,
                    },
                };

                let grid_style = GridStyle {
                    font: metrics.font.clone(),
                    minimum_contrast: MINIMUM_CONTRAST,
                };
                let background_color = theme.colors().terminal_background;
                let player_color = theme.players().local();

                let Some(frame) = frame else {
                    self.view.update(cx, |view, _| {
                        view.set_grid_layout(GridLayout {
                            bounds: dimensions,
                            first_row: 0,
                        })
                    });
                    return LayoutState {
                        hitbox,
                        batched_text_runs: Vec::new(),
                        block_element_rects: Vec::new(),
                        rects: Vec::new(),
                        selection_rects: Vec::new(),
                        selection_color: player_color.selection,
                        cursor: None,
                        ime_cursor_bounds: None,
                        background_color,
                        dimensions,
                        font: metrics.font,
                        font_size: metrics.font_size,
                        foreground: theme.colors().terminal_foreground,
                    };
                };

                let rows = match self.mode {
                    TerminalMode::Scrollable => {
                        0..frame.screen_lines.min(dimensions.num_lines() as u16)
                    }
                    TerminalMode::Inline { max_lines } => inline_rows(&frame, max_lines),
                };
                let first_row = rows.start;
                let row_count = rows.len() as u16;

                let terminal_size = TerminalSize {
                    columns: dimensions.num_columns().max(2) as u16,
                    screen_lines: match self.mode {
                        TerminalMode::Scrollable => dimensions.num_lines().max(1) as u16,
                        // An inline terminal keeps its height; only the width follows the
                        // view.
                        TerminalMode::Inline { .. } => frame.screen_lines,
                    },
                    cell_width: f32::from(cell_width).round() as u16,
                    cell_height: f32::from(line_height).round() as u16,
                };
                let view = self.view.entity_id();
                self.terminal
                    .update(cx, |terminal, cx| terminal.resize(view, terminal_size, cx));
                self.view.update(cx, |view, _| {
                    view.set_grid_layout(GridLayout {
                        bounds: dimensions,
                        first_row,
                    })
                });

                // Only lay out the rows the parent shows, as inside a scrolling thread.
                let visible_bounds = window.content_mask().bounds.intersect(&dimensions.bounds);
                let (rects, batched_text_runs, block_element_rects) = if visible_bounds.size.height
                    <= px(0.)
                    || visible_bounds.size.width <= px(0.)
                {
                    (Vec::new(), Vec::new(), Vec::new())
                } else {
                    layout_grid(&frame, rows, &grid_style, &theme)
                };

                let selection_rects = selection_rects(&frame, first_row, row_count, &dimensions);

                let cursor_cell = frame.cursor.and_then(|cursor| {
                    let line = cursor.row.checked_sub(first_row)?;
                    (line < row_count).then_some((cursor, line))
                });
                let (ime_cursor_bounds, cursor) = match cursor_cell {
                    None => (None, None),
                    Some((cursor, line)) => {
                        let (text, columns) = character_at(&frame, cursor.row, cursor.column)
                            .unwrap_or_else(|| (" ".to_string(), 1));
                        let cursor_bounds = Bounds::new(
                            point(
                                (cell_width * cursor.column as f32).floor(),
                                (line_height * line as f32).floor(),
                            ),
                            size((cell_width * columns as f32).ceil(), line_height),
                        );
                        let shape = match cursor.shape {
                            _ if !self.focused => CursorKind::Hollow,
                            TerminalCursorShape::Block => CursorKind::Block,
                            TerminalCursorShape::Underline => CursorKind::Underline,
                            TerminalCursorShape::Bar => CursorKind::Bar,
                            TerminalCursorShape::HollowBlock => CursorKind::Hollow,
                        };
                        let text =
                            (shape == CursorKind::Block && !text.trim().is_empty()).then(|| {
                                window.text_system().shape_line(
                                    text.clone().into(),
                                    metrics.font_size,
                                    &[TextRun {
                                        len: text.len(),
                                        font: metrics.font.clone(),
                                        color: theme.colors().terminal_ansi_background,
                                        ..Default::default()
                                    }],
                                    None,
                                )
                            });
                        (
                            Some(cursor_bounds),
                            // Like Zed's embedded terminals, an inline one shows its cursor
                            // only while focused.
                            (frame.exited.is_none()
                                && (self.focused || self.mode == TerminalMode::Scrollable))
                                .then_some(CursorLayout {
                                    bounds: cursor_bounds,
                                    shape,
                                    color: player_color.cursor,
                                    text,
                                }),
                        )
                    }
                };

                LayoutState {
                    hitbox,
                    batched_text_runs,
                    block_element_rects,
                    rects,
                    selection_rects,
                    selection_color: player_color.selection,
                    cursor,
                    ime_cursor_bounds,
                    background_color,
                    dimensions,
                    font: metrics.font,
                    font_size: metrics.font_size,
                    foreground: theme.colors().terminal_foreground,
                }
            },
        )
    }

    fn paint(
        &mut self,
        global_id: Option<&GlobalElementId>,
        inspector_id: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut Self::RequestLayoutState,
        layout: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        window.with_content_mask(Some(ContentMask { bounds }), |window| {
            window.paint_quad(fill(bounds, layout.background_color));
            let origin = layout.dimensions.bounds.origin;
            let marked_text = self.view.read(cx).marked_text().map(str::to_string);
            let input_handler = TerminalInputHandler {
                view: self.view.clone(),
                cursor_bounds: layout.ime_cursor_bounds.map(|bounds| bounds + origin),
                cell_width: layout.dimensions.cell_width,
            };

            self.register_mouse_listeners(&layout.hitbox, window);
            window.set_cursor_style(gpui::CursorStyle::IBeam, &layout.hitbox);

            let cursor = layout.cursor.take();
            self.interactivity.paint(
                global_id,
                inspector_id,
                bounds,
                Some(&layout.hitbox),
                window,
                cx,
                |_, window, cx| {
                    window.handle_input(&self.focus, input_handler, cx);

                    for rect in &layout.rects {
                        rect.paint(origin, &layout.dimensions, window);
                    }
                    for selection in &layout.selection_rects {
                        window.paint_quad(fill(*selection + origin, layout.selection_color));
                    }
                    for batch in &layout.batched_text_runs {
                        batch.paint(origin, &layout.dimensions, layout.font_size, window, cx);
                    }
                    for block_element_rect in &layout.block_element_rects {
                        block_element_rect.paint(origin, &layout.dimensions, window);
                    }

                    if let Some(text_to_mark) = marked_text.as_ref().filter(|text| !text.is_empty())
                        && let Some(ime_bounds) = layout.ime_cursor_bounds
                    {
                        let ime_position = (ime_bounds + origin).origin;
                        let underline = UnderlineStyle {
                            color: Some(layout.foreground),
                            thickness: px(1.0),
                            wavy: false,
                        };
                        let shaped_line = window.text_system().shape_line(
                            text_to_mark.clone().into(),
                            layout.font_size,
                            &[TextRun {
                                len: text_to_mark.len(),
                                font: layout.font.clone(),
                                color: layout.foreground,
                                underline: Some(underline),
                                ..Default::default()
                            }],
                            None,
                        );
                        // Paint background to cover terminal text behind marked text
                        window.paint_quad(fill(
                            Bounds::new(
                                ime_position,
                                size(shaped_line.width, layout.dimensions.line_height),
                            ),
                            layout.background_color,
                        ));
                        shaped_line
                            .paint(
                                ime_position,
                                layout.dimensions.line_height,
                                gpui::TextAlign::Left,
                                None,
                                window,
                                cx,
                            )
                            .log_err();
                    }

                    if marked_text.is_none()
                        && let Some(cursor) = cursor
                    {
                        cursor.paint(origin, window, cx);
                    }
                },
            );
        });
    }
}

impl IntoElement for TerminalElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

struct TerminalInputHandler {
    view: Entity<TerminalView>,
    cursor_bounds: Option<Bounds<Pixels>>,
    cell_width: Pixels,
}

impl InputHandler for TerminalInputHandler {
    fn selected_text_range(
        &mut self,
        _ignore_disabled_input: bool,
        _: &mut Window,
        _: &mut App,
    ) -> Option<UTF16Selection> {
        // Always a valid selection, so the IME can place its window even in full-screen
        // programs, which still have a cursor.
        Some(UTF16Selection {
            range: 0..0,
            reversed: false,
        })
    }

    fn marked_text_range(
        &mut self,
        _window: &mut Window,
        cx: &mut App,
    ) -> Option<std::ops::Range<usize>> {
        self.view.read(cx).marked_text_range()
    }

    fn text_for_range(
        &mut self,
        _: std::ops::Range<usize>,
        _: &mut Option<std::ops::Range<usize>>,
        _: &mut Window,
        _: &mut App,
    ) -> Option<String> {
        None
    }

    fn replace_text_in_range(
        &mut self,
        _replacement_range: Option<std::ops::Range<usize>>,
        text: &str,
        window: &mut Window,
        cx: &mut App,
    ) {
        self.view.update(cx, |view, cx| {
            view.clear_marked_text(cx);
            view.commit_text(text, cx);
        });
        window.invalidate_character_coordinates();
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        _range_utf16: Option<std::ops::Range<usize>>,
        new_text: &str,
        _new_marked_range: Option<std::ops::Range<usize>>,
        _window: &mut Window,
        cx: &mut App,
    ) {
        self.view.update(cx, |view, cx| {
            view.set_marked_text(new_text.to_string(), cx)
        });
    }

    fn unmark_text(&mut self, _window: &mut Window, cx: &mut App) {
        self.view.update(cx, |view, cx| view.clear_marked_text(cx));
    }

    fn bounds_for_range(
        &mut self,
        range_utf16: std::ops::Range<usize>,
        _window: &mut Window,
        _cx: &mut App,
    ) -> Option<Bounds<Pixels>> {
        let mut bounds = self.cursor_bounds?;
        bounds.origin.x += self.cell_width * range_utf16.start as f32;
        Some(bounds)
    }

    fn apple_press_and_hold_enabled(&mut self) -> bool {
        false
    }

    fn character_index_for_point(
        &mut self,
        _point: Point<Pixels>,
        _window: &mut Window,
        _cx: &mut App,
    ) -> Option<usize> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use agentz_protocol::terminal::{TerminalLine, TerminalPoint, TerminalSelection};

    #[test]
    fn test_sextant_char_to_filled_bits() {
        assert_eq!(sextant_char_to_filled_bits('\u{1FB00}'), Some(0b000001));
        assert_eq!(sextant_char_to_filled_bits('\u{1FB13}'), Some(0b010100));
        // U+1FB14 skips ▌ (0b010101).
        assert_eq!(sextant_char_to_filled_bits('\u{1FB14}'), Some(0b010110));
        // U+1FB28 skips ▐ (0b101010).
        assert_eq!(sextant_char_to_filled_bits('\u{1FB28}'), Some(0b101011));
        assert_eq!(sextant_char_to_filled_bits('\u{1FB3B}'), Some(0b111110));
        assert_eq!(sextant_char_to_filled_bits('\u{1FB3C}'), None);
        assert_eq!(sextant_char_to_filled_bits('█'), None);
    }

    #[test]
    fn test_block_element_rects_merge_across_adjacent_full_blocks() {
        let color = gpui::red();
        let mut regions = Vec::new();
        for column in 0..3 {
            assert!(collect_block_element_regions(
                LayoutPoint::new(0, column),
                '█',
                color,
                &mut regions,
            ));
        }
        let merged = merge_background_regions(regions);
        assert_eq!(
            merged,
            vec![BackgroundRegion::with_extents(0, 0, 23, 23, color)]
        );
    }

    #[test]
    fn test_background_region_merge() {
        let color = gpui::red();
        let regions = vec![
            BackgroundRegion::new(0, 0, color),
            BackgroundRegion::new(0, 1, color),
            BackgroundRegion::new(1, 0, color),
            BackgroundRegion::new(1, 1, color),
        ];
        assert_eq!(
            merge_background_regions(regions),
            vec![BackgroundRegion::with_extents(0, 0, 1, 1, color)]
        );
    }

    #[test]
    fn test_is_app_chosen_exact_color() {
        assert!(is_app_chosen_exact_color(TerminalColor::Rgb(1, 2, 3)));
        assert!(is_app_chosen_exact_color(TerminalColor::Indexed(16)));
        assert!(!is_app_chosen_exact_color(TerminalColor::Indexed(15)));
        assert!(!is_app_chosen_exact_color(TerminalColor::Named(1)));
    }

    #[test]
    fn rgb_cube_matches_xterm() {
        assert_eq!(rgb_for_index(16), (0, 0, 0));
        assert_eq!(rgb_for_index(231), (5, 5, 5));
        assert_eq!(rgb_for_index(16 + 36 + 6 + 1), (1, 1, 1));
    }

    fn run(column: u16, text: &str, flags: u16) -> TerminalRun {
        TerminalRun {
            column,
            text: text.to_string(),
            style: TerminalStyle {
                flags,
                ..TerminalStyle::default()
            },
        }
    }

    fn frame(lines: Vec<TerminalLine>) -> TerminalFrame {
        TerminalFrame {
            full: true,
            columns: 10,
            screen_lines: lines.len() as u16,
            lines: lines
                .into_iter()
                .enumerate()
                .map(|(row, line)| (row as u16, line))
                .collect(),
            ..TerminalFrame::default()
        }
    }

    #[test]
    fn wide_and_combining_runs_are_one_cell() {
        let wide = run(2, "漢", TerminalStyle::WIDE);
        let cells = run_cells(&wide);
        assert_eq!(cells.len(), 1);
        assert_eq!((cells[0].column, cells[0].columns), (2, 2));

        let combining = run(0, "e\u{301}", TerminalStyle::COMBINING);
        let cells = run_cells(&combining);
        assert_eq!(cells.len(), 1);
        assert_eq!(cells[0].text, "e\u{301}");

        let plain = run(3, "ab", 0);
        let cells = run_cells(&plain);
        assert_eq!(
            cells
                .iter()
                .map(|cell| (cell.column, cell.text))
                .collect::<Vec<_>>(),
            vec![(3, "a"), (4, "b")]
        );
    }

    #[test]
    fn character_at_finds_the_cell_under_a_column() {
        let frame = frame(vec![TerminalLine {
            runs: vec![run(0, "ab", 0), run(2, "漢", TerminalStyle::WIDE)],
        }]);
        assert_eq!(character_at(&frame, 0, 1), Some(("b".to_string(), 1)));
        assert_eq!(character_at(&frame, 0, 3), Some(("漢".to_string(), 2)));
        assert_eq!(character_at(&frame, 0, 5), None);
    }

    #[test]
    fn inline_rows_end_at_the_last_output() {
        let mut frame = frame(vec![
            TerminalLine {
                runs: vec![run(0, "one", 0)],
            },
            TerminalLine {
                runs: vec![run(0, "two", 0)],
            },
            TerminalLine::default(),
            TerminalLine::default(),
        ]);
        assert_eq!(inline_rows(&frame, 10), 0..2);
        assert_eq!(inline_rows(&frame, 1), 1..2);
        frame.lines.clear();
        frame.screen_lines = 4;
        assert_eq!(inline_rows(&frame, 10), 0..1);
    }

    #[test]
    fn selection_covers_whole_middle_lines() {
        let mut frame = frame(vec![TerminalLine::default(); 4]);
        frame.selection = Some(TerminalSelection {
            start: TerminalPoint { line: 0, column: 3 },
            end: TerminalPoint { line: 2, column: 1 },
            is_block: false,
        });
        let dimensions = TerminalBounds {
            cell_width: px(10.),
            line_height: px(20.),
            bounds: Bounds::new(point(px(0.), px(0.)), size(px(100.), px(80.))),
        };
        let rects = selection_rects(&frame, 0, 4, &dimensions);
        assert_eq!(rects.len(), 3);
        assert_eq!(rects[0].origin.x, px(30.));
        assert_eq!(rects[1].size.width, px(100.));
        assert_eq!(rects[2].size.width, px(20.));
    }
}
