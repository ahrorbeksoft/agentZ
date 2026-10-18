//! Find in a thread (`design/thread-search`): Zed's thread search, a strip under the header
//! that finds text in the conversation as it shows: the user's and the agent's messages, the
//! plans the agent proposes, the plan section's steps and each tool call's line. Matches are
//! painted in the theme's search colors, as Zed's are.

use std::cell::Cell;
use std::ops::Range;
use std::rc::Rc;
use std::time::Duration;

use collections::HashSet;
use gpui::{Entity, Pixels, SharedString, Subscription, Task, WeakEntity, px};
use markdown::Markdown;
use regex::{Regex, RegexBuilder};
use text_input::TextInput;

/// Zed's wait before searching again as the thread changes; while it streams in, it's searched
/// this often. A changed query searches at once.
pub(crate) const RESCAN_DELAY: Duration = Duration::from_millis(150);
/// t3code's: a match gone to sits this far below the top of the conversation.
pub(crate) const MATCH_TOP_OFFSET: Pixels = px(96.);

/// Zed's search options, all off as the bar opens.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct FindOptions {
    pub case_sensitive: bool,
    pub whole_word: bool,
    pub regex: bool,
}

/// Where a match is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum FindTarget {
    /// A message's or a proposed plan's markdown, by its entry and part.
    Markdown((usize, usize)),
    /// One piece of a tool call's line, by its entry.
    Title { entry: usize, piece: usize },
    /// A step of the plan section, by its place there.
    PlanItem(usize),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct FindMatch {
    pub target: FindTarget,
    pub range: Range<usize>,
}

/// Text to find matches in, with the parts of it that don't show, as a link's address.
pub(crate) struct Searched {
    pub target: FindTarget,
    pub text: SharedString,
    /// Sorted and apart, as `Markdown::non_rendered_source_ranges` gives them.
    pub hidden: Vec<Range<usize>>,
}

/// The open find bar.
pub(crate) struct ThreadFind {
    pub input: Entity<TextInput>,
    pub options: FindOptions,
    /// In the order they show, from the top.
    pub matches: Vec<FindMatch>,
    /// The current match, into `matches`.
    pub active: Option<usize>,
    /// Why the query isn't a regular expression.
    pub error: Option<SharedString>,
    /// The markdowns painted with matches, to clear.
    pub highlighted: Vec<WeakEntity<Markdown>>,
    /// The markdowns, by their entry and part, that weren't parsed yet when searched: what
    /// doesn't show in them is known once they are, and they're searched again then.
    pub unparsed: HashSet<(usize, usize)>,
    /// The search again after the thread changed, while it waits.
    pub rescan: Option<Task<()>>,
    /// The conversation scrolls to the current match's tool call line once the line is laid
    /// out, in a list this tall (it can't be read while the list lays out its rows).
    pub scroll_to_line: Rc<Cell<Option<Pixels>>>,
    /// The plan section's step to scroll to once it's laid out.
    pub scroll_to_plan_item: Cell<Option<usize>>,
    pub _subscription: Subscription,
}

impl ThreadFind {
    pub fn new(input: Entity<TextInput>, subscription: Subscription) -> Self {
        Self {
            input,
            options: FindOptions::default(),
            matches: Vec::new(),
            active: None,
            error: None,
            highlighted: Vec::new(),
            unparsed: HashSet::default(),
            rescan: None,
            scroll_to_line: Rc::default(),
            scroll_to_plan_item: Cell::new(None),
            _subscription: subscription,
        }
    }

    /// Zed's count: the current match and how many there are, or nothing without a query.
    pub fn count(&self, query: &str) -> String {
        if query.is_empty() {
            return String::new();
        }
        match self.active {
            Some(index) => format!("{}/{}", index + 1, self.matches.len()),
            None => format!("0/{}", self.matches.len()),
        }
    }

    /// The match after or before the current one, wrapping around as Zed's do.
    pub fn step(&self, forward: bool) -> Option<usize> {
        let count = self.matches.len();
        if count == 0 {
            return None;
        }
        Some(match self.active {
            Some(index) if forward => (index + 1) % count,
            Some(index) => (index + count - 1) % count,
            None if forward => 0,
            None => count - 1,
        })
    }

    pub fn active_match(&self) -> Option<&FindMatch> {
        self.matches.get(self.active?)
    }

    /// The matches in `target`, each with whether it's the current one.
    pub fn found_in(&self, target: &FindTarget) -> Vec<(Range<usize>, bool)> {
        self.matches
            .iter()
            .enumerate()
            .filter(|(_, found)| &found.target == target)
            .map(|(index, found)| (found.range.clone(), Some(index) == self.active))
            .collect()
    }
}

/// The query as Zed's search builds it: the text as typed, or a regular expression, either
/// ignoring case unless Match Case Sensitivity is on. Whole words put a boundary where the
/// query starts or ends with a word's character. Nothing to find without a query, and an
/// invalid expression says why.
pub(crate) fn find_query(query: &str, options: FindOptions) -> Result<Option<Regex>, String> {
    if query.is_empty() {
        return Ok(None);
    }
    let mut pattern = if options.regex {
        query.to_string()
    } else {
        regex::escape(query)
    };
    if options.whole_word {
        let is_word = |character: char| character.is_alphanumeric() || character == '_';
        if pattern.chars().next().is_some_and(is_word) {
            pattern.insert_str(0, r"\b");
        }
        if pattern.chars().last().is_some_and(is_word) {
            pattern.push_str(r"\b");
        }
    }
    RegexBuilder::new(&pattern)
        .case_insensitive(!options.case_sensitive)
        .multi_line(true)
        .crlf(true)
        .build()
        .map(Some)
        .map_err(|error| error.to_string())
}

/// The ranges sorted, with those that overlap or touch made one, as [`find_matches`] takes
/// what doesn't show.
pub(crate) fn merge_ranges(mut ranges: Vec<Range<usize>>) -> Vec<Range<usize>> {
    ranges.sort_by_key(|range| range.start);
    let mut merged: Vec<Range<usize>> = Vec::with_capacity(ranges.len());
    for range in ranges {
        match merged.last_mut() {
            Some(last) if range.start <= last.end => last.end = last.end.max(range.end),
            _ => merged.push(range),
        }
    }
    merged
}

/// Every match of `query` in what's searched, in its order, but those in what doesn't show
/// (Zed's markdown preview's `filter_non_rendered_matches`) and empty ones.
pub(crate) fn find_matches(searched: &[Searched], query: &Regex) -> Vec<FindMatch> {
    searched
        .iter()
        .flat_map(|searched| {
            query
                .find_iter(&searched.text)
                .filter(|found| !found.is_empty())
                .map(|found| found.range())
                .filter(|range| {
                    let candidate = searched
                        .hidden
                        .partition_point(|hidden| hidden.end <= range.start);
                    !searched
                        .hidden
                        .get(candidate)
                        .is_some_and(|hidden| hidden.start < range.end)
                })
                .map(|range| FindMatch {
                    target: searched.target.clone(),
                    range,
                })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn searched(text: &str, hidden: Vec<Range<usize>>) -> Vec<Searched> {
        vec![Searched {
            target: FindTarget::PlanItem(0),
            text: text.to_string().into(),
            hidden,
        }]
    }

    fn ranges(text: &str, query: &str, options: FindOptions) -> Vec<Range<usize>> {
        let query = find_query(query, options)
            .expect("a valid query")
            .expect("a query");
        find_matches(&searched(text, Vec::new()), &query)
            .into_iter()
            .map(|found| found.range)
            .collect()
    }

    #[test]
    fn the_options_change_what_matches_as_in_zed() {
        let text = "Checkout: the checkout_total and checkouts";
        let options = FindOptions::default();
        assert_eq!(ranges(text, "checkout", options), [0..8, 14..22, 33..41]);
        let case_sensitive = FindOptions {
            case_sensitive: true,
            ..options
        };
        assert_eq!(ranges(text, "checkout", case_sensitive), [14..22, 33..41]);
        let whole_word = FindOptions {
            whole_word: true,
            ..options
        };
        assert_eq!(ranges(text, "checkout", whole_word), [0..8]);
        let regex = FindOptions {
            regex: true,
            ..options
        };
        assert_eq!(ranges(text, "check\\w+s", regex), [33..42]);
        // As typed, the same text is literal.
        assert!(ranges(text, "check\\w+s", options).is_empty());
        // Empty matches find nothing.
        assert!(ranges(text, "x*", regex).is_empty());
    }

    #[test]
    fn a_query_without_text_finds_nothing_and_a_bad_expression_says_why() {
        assert!(matches!(find_query("", FindOptions::default()), Ok(None)));
        let regex = FindOptions {
            regex: true,
            ..FindOptions::default()
        };
        let error = find_query("(unclosed", regex).expect_err("an invalid expression");
        assert!(error.contains("unclosed group"), "{error}");
        // The same text, as typed, is fine.
        assert!(find_query("(unclosed", FindOptions::default()).is_ok());
    }

    /// A link's address doesn't show, so a match in it isn't one.
    #[test]
    fn what_doesnt_show_has_no_matches() {
        let text = "[the docs](https://docs.example.com) and docs";
        let query = find_query("docs", FindOptions::default())
            .expect("a valid query")
            .expect("a query");
        let found: Vec<Range<usize>> = find_matches(&searched(text, vec![9..36]), &query)
            .into_iter()
            .map(|found| found.range)
            .collect();
        assert_eq!(found, [5..9, 41..45]);
    }

    #[test]
    fn hidden_ranges_are_sorted_and_merged() {
        assert_eq!(
            merge_ranges(vec![10..20, 0..4, 15..30, 4..6, 40..41]),
            [0..6, 10..30, 40..41]
        );
        assert!(merge_ranges(Vec::new()).is_empty());
    }
}
