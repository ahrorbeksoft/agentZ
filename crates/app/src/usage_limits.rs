//! An account's limits as t3code's `LimitWindows` shows them: a row per window with its name
//! and what's left, a bar of what's left with a hairline where even spending would be, and when
//! it resets.

use std::time::{Duration, SystemTime};

use agentz_protocol::accounts::LimitWindow;
use gpui::{AnyElement, App, FontWeight, relative};
use ui::{Tooltip, prelude::*};

/// At or under this much left, a bar turns yellow.
const LOW_PERCENT: u8 = 15;
const LABEL_WIDTH: Pixels = px(140.);
const RESET_WIDTH: Pixels = px(116.);

/// The windows, one row each. `key` tells the account's rows apart in tests.
pub(crate) fn render_limit_windows(
    key: &str,
    windows: &[LimitWindow],
    now: SystemTime,
    cx: &App,
) -> AnyElement {
    v_flex()
        .gap(px(9.))
        .children(
            windows
                .iter()
                .enumerate()
                .map(|(index, window)| render_window(key, index, window, now, cx)),
        )
        .into_any_element()
}

fn render_window(
    key: &str,
    index: usize,
    window: &LimitWindow,
    now: SystemTime,
    cx: &App,
) -> AnyElement {
    let left = window.left_percent();
    let left_label = if left == 0 {
        Label::new("Used up").color(Color::Error)
    } else {
        Label::new(format!("{left}% left"))
    };
    let selector = format!("limit-{key}-window-{index}");
    h_flex()
        .debug_selector(move || selector)
        .gap(px(14.))
        .child(
            h_flex()
                .w(LABEL_WIDTH)
                .flex_none()
                .gap_2()
                .child(
                    div().flex_1().min_w_0().child(
                        Label::new(window.label.clone())
                            .size(LabelSize::Small)
                            .color(Color::Muted)
                            .truncate(),
                    ),
                )
                .child(left_label.size(LabelSize::Small).weight(FontWeight::MEDIUM)),
        )
        .child(render_bar(key, index, window, now, cx))
        .child(
            h_flex().w(RESET_WIDTH).flex_none().justify_end().child(
                Label::new(format_resets_in(window, now).unwrap_or_default())
                    .size(LabelSize::Small)
                    .color(Color::Muted),
            ),
        )
        .into_any_element()
}

/// The fill is what's left; the hairline is the share of the window still to come, where
/// spending evenly would have left the fill. The tooltip has the exact figures and time.
fn render_bar(
    key: &str,
    index: usize,
    window: &LimitWindow,
    now: SystemTime,
    cx: &App,
) -> AnyElement {
    let colors = cx.theme().colors();
    let status = cx.theme().status();
    let left = window.left_percent();
    let time_left = window.time_left(now);
    let fill = if left <= LOW_PERCENT {
        status.warning
    } else {
        colors.text_accent
    };
    let track = if left == 0 {
        status.error.opacity(0.35)
    } else {
        colors.border
    };
    let summary = match time_left {
        Some(share) => format!(
            "{left}% left · {}% of the window left",
            (share * 100.).round() as u8
        ),
        None => format!("{left}% left"),
    };
    let resets = window.resets_at.map(|resets_at| {
        let mut text = format!("Resets {}", reset_time(resets_at, now));
        if let Some(resets_in) = format_resets_in(window, now) {
            text.push_str(&format!(" · {resets_in}"));
        }
        text
    });
    let has_hairline = time_left.is_some();
    let bar_selector = format!("limit-{key}-bar-{index}");
    let hairline_selector = format!("limit-{key}-hairline-{index}");
    div()
        .id(("limit-bar", index))
        .debug_selector(move || bar_selector)
        .flex_1()
        .min_w_0()
        .h(px(12.))
        .flex()
        .items_center()
        .relative()
        .child(
            div()
                .relative()
                .w_full()
                .h(px(6.))
                .rounded_full()
                .bg(track)
                .when(left > 0, |track| {
                    track.child(
                        div()
                            .absolute()
                            .left_0()
                            .top_0()
                            .bottom_0()
                            .w(relative(f32::from(left) / 100.))
                            .rounded_full()
                            .bg(fill),
                    )
                }),
        )
        .when_some(time_left, |bar, share| {
            bar.child(
                div()
                    .debug_selector(move || hairline_selector)
                    .absolute()
                    .top_0()
                    .bottom_0()
                    .left(relative(share as f32))
                    .w(px(1.))
                    .bg(colors.text.opacity(0.55)),
            )
        })
        .tooltip(Tooltip::element(move |_, _| {
            v_flex()
                .gap_0p5()
                .child(Label::new(summary.clone()).size(LabelSize::Small))
                .when(has_hairline, |tooltip| {
                    tooltip.child(
                        Label::new("The line is where even spending would be.")
                            .size(LabelSize::Small)
                            .color(Color::Muted),
                    )
                })
                .children(resets.clone().map(|resets| {
                    Label::new(resets)
                        .size(LabelSize::Small)
                        .color(Color::Muted)
                }))
                .into_any_element()
        }))
        .into_any_element()
}

/// The window closest to running out, which pickers show.
pub(crate) fn tightest_window(windows: &[LimitWindow]) -> Option<&LimitWindow> {
    windows.iter().min_by_key(|window| window.left_percent())
}

/// The window that stops the account until it resets: of those used up, the last to reset. One
/// whose reset has passed is no longer spent, though the last read still says so.
pub(crate) fn used_up_window(windows: &[LimitWindow], now: SystemTime) -> Option<&LimitWindow> {
    windows
        .iter()
        .filter(|window| window.left_percent() == 0)
        .filter(|window| window.resets_at.is_none_or(|resets_at| resets_at > now))
        .max_by_key(|window| window.resets_at)
}

/// A picker's line for the window: `62% left`, or `Used up · 2h 10m` in red until it resets.
pub(crate) fn left_label(window: &LimitWindow, now: SystemTime) -> Label {
    let left = window.left_percent();
    if left > 0 {
        return Label::new(format!("{left}% left"));
    }
    let remaining = window
        .resets_at
        .and_then(|resets_at| resets_at.duration_since(now).ok())
        .filter(|remaining| !remaining.is_zero());
    let text = match remaining {
        Some(remaining) => format!("Used up · {}", format_duration(remaining)),
        None => "Used up".to_string(),
    };
    Label::new(text).color(Color::Error)
}

/// t3code's durations: `2d 3h`, `2h 13m`, `12m`.
pub(crate) fn format_duration(duration: Duration) -> String {
    const MINUTE: u64 = 60;
    const HOUR: u64 = 60 * MINUTE;
    const DAY: u64 = 24 * HOUR;
    let seconds = duration.as_secs();
    let (days, hours, minutes) = (
        seconds / DAY,
        (seconds % DAY) / HOUR,
        (seconds % HOUR) / MINUTE,
    );
    if days > 0 {
        format!("{days}d {hours}h")
    } else if hours > 0 {
        format!("{hours}h {minutes}m")
    } else {
        format!("{minutes}m")
    }
}

/// `resets in 2h 13m`, or `None` when the window has no reset.
pub(crate) fn format_resets_in(window: &LimitWindow, now: SystemTime) -> Option<String> {
    let resets_at = window.resets_at?;
    Some(match resets_at.duration_since(now) {
        Ok(remaining) if !remaining.is_zero() => {
            format!("resets in {}", format_duration(remaining))
        }
        _ => "resets now".to_string(),
    })
}

/// The reset's local time: the clock today, the weekday within a week, else the date.
fn reset_time(at: SystemTime, now: SystemTime) -> String {
    let at = chrono::DateTime::<chrono::Local>::from(at);
    let now = chrono::DateTime::<chrono::Local>::from(now);
    let days_ahead = (at.date_naive() - now.date_naive()).num_days();
    if days_ahead <= 0 {
        at.format("%H:%M").to_string()
    } else if days_ahead < 7 {
        at.format("%a %H:%M").to_string()
    } else {
        at.format("%b %-d %H:%M").to_string()
    }
}

/// [`reset_time`] in a sentence: `at 16:10`, `on Mon at 16:10`, `on Mar 3 at 16:10`.
pub(crate) fn reset_phrase(at: SystemTime, now: SystemTime) -> String {
    let time = chrono::DateTime::<chrono::Local>::from(at).format("%H:%M");
    match reset_time(at, now).rsplit_once(' ') {
        Some((day, _)) => format!("on {day} at {time}"),
        None => format!("at {time}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn durations_read_like_t3codes() {
        let minutes = |minutes: u64| Duration::from_secs(minutes * 60);
        assert_eq!(format_duration(minutes(12)), "12m");
        assert_eq!(format_duration(minutes(2 * 60 + 13)), "2h 13m");
        assert_eq!(format_duration(minutes((2 * 24 + 3) * 60 + 59)), "2d 3h");
        assert_eq!(format_duration(Duration::from_secs(30)), "0m");

        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(1_000_000);
        let window = LimitWindow {
            label: "5-hour".into(),
            used_percent: 38.,
            resets_at: Some(now + minutes(130)),
            length: None,
        };
        assert_eq!(
            format_resets_in(&window, now).as_deref(),
            Some("resets in 2h 10m")
        );
        assert_eq!(
            format_resets_in(&window, now + minutes(200)).as_deref(),
            Some("resets now")
        );
        let without_reset = LimitWindow {
            resets_at: None,
            ..window
        };
        assert_eq!(format_resets_in(&without_reset, now), None);
    }

    #[test]
    fn an_account_waits_for_the_last_of_its_used_up_windows() {
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(1_000_000);
        let window = |label: &str, used_percent: f64, resets_in: Option<i64>| LimitWindow {
            label: label.into(),
            used_percent,
            resets_at: resets_in.map(|minutes| {
                if minutes < 0 {
                    now - Duration::from_secs(minutes.unsigned_abs() * 60)
                } else {
                    now + Duration::from_secs(minutes.unsigned_abs() * 60)
                }
            }),
            length: None,
        };
        let label = |windows: &[LimitWindow]| {
            used_up_window(windows, now).map(|window| window.label.clone())
        };
        assert_eq!(label(&[window("5-hour", 62., Some(30))]), None);
        assert_eq!(
            label(&[
                window("5-hour", 100., Some(30)),
                window("Weekly", 99.8, Some(3000)),
                window("Monthly", 40., Some(9000)),
            ]),
            Some("Weekly".into())
        );
        // Past its reset, the last read is out of date.
        assert_eq!(label(&[window("5-hour", 100., Some(-5))]), None);
        assert_eq!(
            label(&[window("5-hour", 100., None)]),
            Some("5-hour".into())
        );
    }
}
