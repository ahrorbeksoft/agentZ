//! Scroll latching: a scroll gesture goes to one area that scrolls, from its first scroll to
//! its last, as macOS and browsers latch them. Areas that scroll inside others (a code block, a
//! tool call's output) then never pass a scroll on to the area around them partway through a
//! gesture, and an area that scrolls under the mouse while the one around it moves doesn't
//! catch the scroll.
//!
//! Each area under the mouse offers itself on a gesture's first scroll ([`Window::offer_scroll`],
//! in the capture phase), saying what that scroll would do to it ([`ScrollReach`]). The
//! innermost one that moves along the gesture's axis takes the gesture, so a scroll still goes
//! on to the area around one already at its end when a gesture starts there. In the bubble
//! phase each area asks whether it took the gesture ([`Window::takes_scroll`]) and scrolls only
//! then.

use std::{
    any::Any,
    rc::{Rc, Weak},
    time::Duration,
};

use scheduler::Instant;

use crate::{Axis, IsZero, Pixels, ScrollWheelEvent, TouchPhase, px};

use super::Window;

/// How long a gesture lasts after its last scroll, for scrolls without a phase that starts them
/// (a mouse wheel's, and every scroll on Linux): Chromium's wheel transaction timeout.
const GESTURE_TIMEOUT: Duration = Duration::from_millis(500);

/// How far the mouse may move between two scrolls of one gesture.
const GESTURE_SLOP: Pixels = px(10.);

/// What a gesture's first scroll would do to an area that scrolls, which it offers to take the
/// gesture with ([`Window::offer_scroll`]). The innermost area with the greatest reach takes it,
/// so an area turns a scroll across its axis only where nothing around it scrolls that way.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ScrollReach {
    /// The area scrolls only across the gesture's axis, and would turn the scroll that way.
    Across,
    /// The area scrolls along the gesture's axis, but is already at its end that way.
    AtEnd,
    /// The area moves along the gesture's axis.
    Along,
}

#[derive(Default)]
pub(super) struct ScrollLatch {
    /// When and where the gesture's last scroll was.
    last_scroll: Option<(Instant, crate::Point<Pixels>)>,
    /// The axis of the scroll an area is still to be found for.
    axis: Option<Axis>,
    /// The area that took the gesture.
    area: Option<Weak<dyn Any>>,
    /// The areas offered for the current scroll, outermost first.
    offers: Vec<(ScrollReach, Weak<dyn Any>)>,
}

impl ScrollLatch {
    /// Starts dispatching a scroll: it goes on with the gesture under way, or starts another.
    pub(super) fn begin_scroll(&mut self, event: &ScrollWheelEvent, now: Instant) {
        let goes_on = self.last_scroll.is_some_and(|(at, position)| {
            event.touch_phase != TouchPhase::Started
                && now.saturating_duration_since(at) < GESTURE_TIMEOUT
                && (event.position.x - position.x).abs() <= GESTURE_SLOP
                && (event.position.y - position.y).abs() <= GESTURE_SLOP
        }) && self
            .area
            .as_ref()
            .is_none_or(|area| area.strong_count() > 0);
        if !goes_on {
            *self = Self::default();
        }
        self.last_scroll = Some((now, event.position));
        self.offers.clear();
        // Until an area takes the gesture, each scroll looks for one along its own axis, so a
        // first scroll that moves neither way, or one with nowhere to go, doesn't decide it.
        if self.area.is_none() {
            let delta = event.delta.pixel_delta(px(1.));
            self.axis = if delta.x.is_zero() && delta.y.is_zero() {
                None
            } else if delta.x.abs() <= delta.y.abs() {
                Some(Axis::Vertical)
            } else {
                Some(Axis::Horizontal)
            };
        }
    }
}

impl Window {
    /// The axis of the scroll being dispatched while its gesture has yet to go to an area, for
    /// an area under the mouse to offer itself ([`Window::offer_scroll`]); `None` once an area
    /// has taken the gesture.
    pub fn unlatched_scroll_axis(&self) -> Option<Axis> {
        if self.scroll_latch.area.is_some() {
            None
        } else {
            self.scroll_latch.axis
        }
    }

    /// Offers an area that scrolls, under the mouse, to take the scroll gesture that the
    /// scroll being dispatched starts, with what this scroll would do to it. An area offers
    /// itself in the capture phase, so the areas around it have offered first; an area with
    /// nothing inside it that scrolls may offer itself in the bubble phase instead, before it
    /// asks [`Window::takes_scroll`]. `area` identifies the area across frames, such as the
    /// shared state its scroll offset is kept in.
    pub fn offer_scroll<T: 'static>(&mut self, area: &Rc<T>, reach: ScrollReach) {
        if self.unlatched_scroll_axis().is_some() {
            let area: Rc<dyn Any> = area.clone();
            self.scroll_latch.offers.push((reach, Rc::downgrade(&area)));
        }
    }

    /// Whether the scroll being dispatched goes to this area, in the bubble phase: whether the
    /// area took the gesture, which its first scroll gives to the innermost area offered with
    /// the greatest reach. Areas that scroll by themselves scroll only when this is true.
    pub fn takes_scroll<T: 'static>(&mut self, area: &Rc<T>) -> bool {
        let latch = &mut self.scroll_latch;
        if latch.area.is_none() {
            // `max_by_key` keeps the last of equals, the innermost.
            latch.area = latch
                .offers
                .iter()
                .max_by_key(|(reach, _)| *reach)
                .map(|(_, area)| area.clone());
        }
        // The latch holds a weak reference, so the area's allocation, and with it the address,
        // stays its own while the gesture lasts.
        latch
            .area
            .as_ref()
            .is_some_and(|latched| std::ptr::addr_eq(latched.as_ptr(), Rc::as_ptr(area)))
    }
}
