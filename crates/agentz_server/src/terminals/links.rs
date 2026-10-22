//! Links on a terminal's screen, found as Zed's terminal finds them
//! (`terminal/src/alacritty/hyperlinks.rs`): hyperlinks the program set (OSC 8), and URLs in
//! the text, with Zed's regex and its trimming of the punctuation that ends a sentence. Zed's
//! file paths aren't looked for, since there's no editor here to open them in.

use std::sync::LazyLock;

use agentz_protocol::terminal::TerminalLink;
use alacritty_terminal::Term;
use alacritty_terminal::event::EventListener;
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::index::{Column, Line};
use alacritty_terminal::term::cell::{Flags, Hyperlink};
use regex::Regex;
use util::ResultExt as _;

/// Zed's `URL_REGEX`.
static URL_REGEX: LazyLock<Option<Regex>> = LazyLock::new(|| {
    Regex::new(
        r#"(ipfs:|ipns:|magnet:|mailto:|gemini://|gopher://|https://|http://|news:|file://|git://|ssh:|ftp://|zed://)[^\u{0000}-\u{001F}\u{007F}-\u{009F}<>"\s{-}\^⟨⟩`']+"#,
    )
    .log_err()
});

/// How far past the screen's edges a wrapped line is followed, for a URL that crosses them.
const MAX_WRAPPED_LINES: i32 = 20;

/// Each screen row's links, from the top of what's shown.
pub(super) fn screen_links<T: EventListener>(term: &Term<T>) -> Vec<Vec<TerminalLink>> {
    let screen_lines = term.screen_lines();
    let columns = term.columns();
    let mut links = vec![Vec::new(); screen_lines];
    if screen_lines == 0 || columns == 0 {
        return links;
    }
    let grid = term.grid();
    let top = Line(-(grid.display_offset() as i32));
    let bottom = Line(top.0 + screen_lines as i32 - 1);
    let wraps = |line: Line| {
        grid[line][Column(columns - 1)]
            .flags
            .contains(Flags::WRAPLINE)
    };
    let mut first = top;
    while first > term.topmost_line() && top.0 - first.0 < MAX_WRAPPED_LINES && wraps(first - 1) {
        first = first - 1;
    }
    while first <= bottom {
        let mut last = first;
        while last < term.bottommost_line() && last.0 - bottom.0 < MAX_WRAPPED_LINES && wraps(last)
        {
            last = last + 1;
        }
        find_links(term, first, last, top, &mut links);
        first = last + 1;
    }
    for row in &mut links {
        row.sort_by_key(|link| link.start);
    }
    links
}

/// A cell of a wrapped line: where its character starts in the line's text, and its place.
struct TextCell {
    offset: usize,
    line: Line,
    column: u16,
    columns: u16,
    hyperlink: Option<Hyperlink>,
}

/// The links in the lines from `first` to `last`, which wrap into one another.
fn find_links<T: EventListener>(
    term: &Term<T>,
    first: Line,
    last: Line,
    top: Line,
    links: &mut [Vec<TerminalLink>],
) {
    let grid = term.grid();
    let mut text = String::new();
    let mut cells = Vec::new();
    for line in first.0..=last.0 {
        let line = Line(line);
        for column in 0..term.columns() {
            let cell = &grid[line][Column(column)];
            if cell
                .flags
                .intersects(Flags::WIDE_CHAR_SPACER | Flags::LEADING_WIDE_CHAR_SPACER)
            {
                continue;
            }
            cells.push(TextCell {
                offset: text.len(),
                line,
                column: column as u16,
                columns: if cell.flags.contains(Flags::WIDE_CHAR) {
                    2
                } else {
                    1
                },
                hyperlink: cell.hyperlink(),
            });
            text.push(cell.c);
            if let Some(zerowidth) = cell.zerowidth() {
                text.extend(zerowidth);
            }
        }
    }

    let mut start = 0;
    while start < cells.len() {
        let Some(hyperlink) = cells[start].hyperlink.clone() else {
            start += 1;
            continue;
        };
        let end = cells[start..]
            .iter()
            .position(|cell| cell.hyperlink.as_ref() != Some(&hyperlink))
            .map_or(cells.len(), |length| start + length);
        add_link(links, top, &cells[start..end], hyperlink.uri(), true);
        start = end;
    }

    let Some(regex) = URL_REGEX.as_ref() else {
        return;
    };
    for found in regex.find_iter(&text) {
        let url = trim_url_punctuation(found.as_str());
        let start = cells.partition_point(|cell| cell.offset < found.start());
        let end = cells.partition_point(|cell| cell.offset < found.start() + url.len());
        let url_cells = &cells[start..end];
        // A hyperlink the program set wins over the text under it, as in Zed.
        if url_cells.iter().any(|cell| cell.hyperlink.is_some()) {
            continue;
        }
        add_link(links, top, url_cells, url, false);
    }
}

/// The link over `cells`, on each screen row it's on.
fn add_link(
    links: &mut [Vec<TerminalLink>],
    top: Line,
    cells: &[TextCell],
    url: &str,
    explicit: bool,
) {
    for row_cells in cells.chunk_by(|cell, next| cell.line == next.line) {
        let (Some(first), Some(last)) = (row_cells.first(), row_cells.last()) else {
            continue;
        };
        let Some(row_links) = usize::try_from(first.line.0 - top.0)
            .ok()
            .and_then(|row| links.get_mut(row))
        else {
            continue;
        };
        row_links.push(TerminalLink {
            start: first.column,
            end: last.column + last.columns,
            url: url.to_string(),
            explicit,
        });
    }
}

/// Zed's `sanitize_url_punctuation`: drops what usually ends the sentence a URL is in rather
/// than the URL, and closing parentheses the URL didn't open.
fn trim_url_punctuation(url: &str) -> &str {
    let opened = url.matches('(').count();
    let mut closed = url.matches(')').count();
    let mut url = url;
    while let Some(last) = url.chars().last() {
        let trims = match last {
            '.' | ',' | ':' | ';' | '(' => true,
            ')' if closed > opened => {
                closed -= 1;
                true
            }
            _ => false,
        };
        if !trims {
            break;
        }
        url = &url[..url.len() - 1];
    }
    url
}

#[cfg(test)]
mod tests {
    use alacritty_terminal::term::Config;
    use alacritty_terminal::vte::ansi::Processor;

    use super::*;
    use crate::terminals::TerminalSize;

    fn links_on(columns: u16, screen_lines: u16, output: &str) -> Vec<Vec<TerminalLink>> {
        let size = TerminalSize {
            columns,
            screen_lines,
            ..TerminalSize::default()
        };
        let mut term = Term::new(
            Config::default(),
            &size,
            alacritty_terminal::event::VoidListener,
        );
        let mut processor: Processor = Processor::new();
        processor.advance(&mut term, output.as_bytes());
        screen_links(&term)
    }

    fn link(start: u16, end: u16, url: &str) -> TerminalLink {
        TerminalLink {
            start,
            end,
            url: url.to_string(),
            explicit: false,
        }
    }

    #[test]
    fn urls_are_found_without_the_punctuation_after_them() {
        let links = links_on(
            60,
            3,
            "  ➜  Local:   http://localhost:5173/\r\nsee (https://zed.dev/docs).\r\nnone here",
        );
        assert_eq!(links[0], vec![link(14, 36, "http://localhost:5173/")]);
        assert_eq!(links[1], vec![link(5, 25, "https://zed.dev/docs")]);
        assert!(links[2].is_empty());
        assert_eq!(
            trim_url_punctuation("https://en.wikipedia.org/wiki/Rust_(language))."),
            "https://en.wikipedia.org/wiki/Rust_(language)"
        );
    }

    #[test]
    fn a_wrapped_url_is_a_link_on_each_of_its_rows() {
        let links = links_on(10, 3, "go https://example.com/a now");
        assert_eq!(
            links,
            [
                vec![link(3, 10, "https://example.com/a")],
                vec![link(0, 10, "https://example.com/a")],
                vec![link(0, 4, "https://example.com/a")],
            ]
        );
    }

    #[test]
    fn hyperlinks_the_program_set_win_over_the_text() {
        let links = links_on(
            40,
            1,
            "\x1b]8;;https://example.com/real\x1b\\https://shown.dev\x1b]8;;\x1b\\ end",
        );
        assert_eq!(
            links[0],
            vec![TerminalLink {
                explicit: true,
                ..link(0, 17, "https://example.com/real")
            }]
        );
    }
}
