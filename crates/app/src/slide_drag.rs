//! Items sliding along a bar or a list while one is dragged, as the Workspaces view's tabs and
//! rows and the Agents sidebar's threads do.

use std::hash::Hash;
use std::time::{Duration, Instant};

use collections::HashMap;
use gpui::{AnyElement, App, BoxShadow, Context, Task};
use ui::prelude::*;

/// An item sliding along a bar or a list while dragged: the order shown meanwhile, with the
/// dragged item where it would land, and the items moving over to make room for it.
pub(crate) struct SlideDrag<T> {
    pub(crate) item: T,
    pub(crate) order: Vec<T>,
    /// Each item's length along the bar or list when the drag started, with any gap after it.
    lengths: HashMap<T, Pixels>,
    /// How far into the item the pointer holds it.
    grab: Pixels,
    pub(crate) pointer: Pixels,
    /// The items sliding over: how far from their new place they started, and when.
    slides: HashMap<T, (Pixels, Instant)>,
    pub(crate) is_scrolling: bool,
    pub(crate) _scroll: Task<()>,
}

impl<T: Copy + Eq + Hash> SlideDrag<T> {
    pub(crate) fn new(
        item: T,
        order: Vec<T>,
        lengths: HashMap<T, Pixels>,
        grab: Pixels,
        pointer: Pixels,
    ) -> Self {
        Self {
            item,
            order,
            lengths,
            grab,
            pointer,
            slides: HashMap::default(),
            is_scrolling: false,
            _scroll: Task::ready(()),
        }
    }

    pub(crate) fn length(&self, item: T) -> Pixels {
        self.lengths.get(&item).copied().unwrap_or_default()
    }

    /// Changes an item's length, sliding the items after it over from where they were.
    pub(crate) fn set_length(&mut self, item: T, length: Pixels) {
        let change = length - self.length(item);
        if change == px(0.) {
            return;
        }
        self.lengths.insert(item, length);
        let Some(index) = self.order.iter().position(|candidate| *candidate == item) else {
            return;
        };
        for follower in index + 1..self.order.len() {
            self.slide(self.order[follower], -change);
        }
    }

    /// Where the pointer holds the dragged item's start, wherever that is.
    pub(crate) fn held_start(&self) -> Pixels {
        self.pointer - self.grab
    }

    /// The dragged item's start: held where it was grabbed, between `start` and `end`.
    pub(crate) fn start(&self, start: Pixels, end: Pixels) -> Pixels {
        let last = (end - self.length(self.item)).max(start);
        self.held_start().clamp(start, last)
    }

    /// Moves the dragged item, its start `start` from the first item's, past each item whose
    /// middle its edge has crossed, and slides that item into the place it left.
    pub(crate) fn reorder(&mut self, start: Pixels) {
        let length = self.length(self.item);
        let Some(mut index) = self.order.iter().position(|item| *item == self.item) else {
            return;
        };
        loop {
            let place = self.order[..index]
                .iter()
                .fold(px(0.), |place, item| place + self.length(*item));
            if let Some(&next) = self.order.get(index + 1)
                && start + length > place + length + self.length(next) / 2.
            {
                self.order.swap(index, index + 1);
                self.slide(next, length);
                index += 1;
                continue;
            }
            if index > 0 {
                let previous = self.order[index - 1];
                if start < place - self.length(previous) / 2. {
                    self.order.swap(index - 1, index);
                    self.slide(previous, -length);
                    index -= 1;
                    continue;
                }
            }
            break;
        }
    }

    /// Moves the dragged item to `target` in the order, sliding the items it passes.
    pub(crate) fn move_to(&mut self, target: usize) {
        let length = self.length(self.item);
        let Some(mut index) = self.order.iter().position(|item| *item == self.item) else {
            return;
        };
        let target = target.min(self.order.len().saturating_sub(1));
        while index < target {
            let next = self.order[index + 1];
            self.order.swap(index, index + 1);
            self.slide(next, length);
            index += 1;
        }
        while index > target {
            let previous = self.order[index - 1];
            self.order.swap(index - 1, index);
            self.slide(previous, -length);
            index -= 1;
        }
    }

    /// Starts `item` sliding to its new place, `by` from it; one already sliding starts from
    /// where it is.
    fn slide(&mut self, item: T, by: Pixels) {
        let now = Instant::now();
        let from = self.offset(item, now);
        self.slides.insert(item, (from + by, now));
    }

    /// How far `item` is from its place at `now`.
    pub(crate) fn offset(&self, item: T, now: Instant) -> Pixels {
        self.slides
            .get(&item)
            .map_or(px(0.), |(from, started)| slide_offset(*from, *started, now))
    }
}

/// How long an item takes to slide over for a dragged one.
pub(crate) const SLIDE_DURATION: Duration = Duration::from_millis(150);
/// How near an end of a bar or list a dragged item scrolls it, and how far each step.
pub(crate) const DRAG_SCROLL_EDGE: Pixels = px(24.);
pub(crate) const DRAG_SCROLL_STEP: Pixels = px(6.);
const DRAG_SCROLL_INTERVAL: Duration = Duration::from_millis(16);

/// How far a sliding item is from its place at `now`, easing out.
pub(crate) fn slide_offset(from: Pixels, started: Instant, now: Instant) -> Pixels {
    let progress =
        now.saturating_duration_since(started).as_secs_f32() / SLIDE_DURATION.as_secs_f32();
    from * (1. - progress.min(1.)).powi(3)
}

/// Which way a bar or list scrolls for an item dragged from `start` to `end` along it: toward
/// the end of `view` it's held near, while there's more to see there (1 toward the start).
pub(crate) fn drag_scroll_direction(
    (start, end): (Pixels, Pixels),
    (view_start, view_end): (Pixels, Pixels),
    offset: Pixels,
    max_offset: Pixels,
) -> Option<f32> {
    if start < view_start + DRAG_SCROLL_EDGE && offset < px(0.) {
        Some(1.)
    } else if end > view_end - DRAG_SCROLL_EDGE && offset > -max_offset {
        Some(-1.)
    } else {
        None
    }
}

/// Takes `step` every little while, until it says it's done: scrolling a bar or list while a
/// dragged item is held near its end.
pub(crate) fn scroll_while_held<V: 'static>(
    step: fn(&mut V, &mut Context<V>) -> bool,
    cx: &mut Context<V>,
) -> Task<()> {
    cx.spawn(async move |this, cx| {
        loop {
            cx.background_executor().timer(DRAG_SCROLL_INTERVAL).await;
            let scrolled = this.update(cx, |this, cx| step(this, cx)).unwrap_or(false);
            if !scrolled {
                break;
            }
        }
    })
}

/// Dragged rows, raised: opaque in the selected row's color, with a shadow.
pub(crate) fn render_raised_rows(
    rows: Vec<AnyElement>,
    width: Pixels,
    cx: &App,
) -> impl IntoElement {
    let colors = cx.theme().colors();
    div()
        .debug_selector(|| "raised-rows".into())
        .relative()
        .w(width)
        .child(
            // As wide as a row, which sits in from the list's sides.
            div()
                .absolute()
                .top_0()
                .bottom_0()
                .left_1()
                .right_1()
                .rounded_md()
                .bg(colors.panel_background)
                .shadow(vec![
                    BoxShadow::new(px(0.), px(10.), gpui::black().opacity(0.55))
                        .blur_radius(px(24.)),
                ])
                .child(
                    div()
                        .size_full()
                        .rounded_md()
                        .bg(colors.ghost_element_selected),
                ),
        )
        .child(v_flex().gap_0p5().children(rows))
}
