//! An account's limits as t3code's `LimitWindows` shows them: a row per window with its name
//! and what's left, a bar of what's left with a hairline where even spending would be, and when
//! it resets. Also the popover of the composer's usage gauge, which shows them.

use std::rc::Rc;
use std::time::{Duration, SystemTime};

use agentz_protocol::accounts::{AccountId, ExtraUsage, LimitResets, LimitWindow};
use agentz_protocol::agents::AgentId;
use gpui::{
    AnyElement, App, ClickEvent, DismissEvent, Entity, EventEmitter, FocusHandle, Focusable,
    FontWeight, Hsla, KeyDownEvent, Subscription, relative,
};
use ui::{Tooltip, prelude::*};

use crate::server_client::ServerClient;
use crate::settings_page::{account_entry, render_entry_avatar};

/// At or under this much left, a bar turns yellow.
pub(crate) const LOW_PERCENT: u8 = 15;
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

/// An account's extra usage balance, under its tab (Droid's Extra Usage).
pub(crate) fn render_balance(key: &str, credits: Option<&str>) -> AnyElement {
    let selector = format!("limit-{key}-balance");
    h_flex()
        .debug_selector(move || selector)
        .gap(px(14.))
        .child(
            div().w(LABEL_WIDTH).flex_none().child(
                Label::new("Balance")
                    .size(LabelSize::Small)
                    .color(Color::Muted),
            ),
        )
        .child(
            // Droid's readers leave out an empty balance, which Droid shows as $0.00.
            Label::new(format!("{} remaining", credits.unwrap_or("$0.00")))
                .size(LabelSize::Small)
                .weight(FontWeight::MEDIUM),
        )
        .into_any_element()
}

/// t3code's `ResetCredits` under an account's limits: how many limit resets it has and when
/// the next expires, with Use Reset, which asks first.
pub(crate) fn render_limit_resets(
    key: &str,
    resets: LimitResets,
    using: bool,
    now: SystemTime,
    on_use: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> AnyElement {
    let selector = format!("limit-resets-{key}");
    let button_selector = format!("use-limit-reset-{key}");
    h_flex()
        .debug_selector(move || selector)
        .gap_2()
        .child(
            Icon::new(IconName::RotateCcw)
                .size(IconSize::Small)
                .color(Color::Muted),
        )
        // Wraps rather than pushing the button out of a narrow popover.
        .child(
            div().flex_1().min_w_0().child(
                Label::new(limit_resets_summary(resets, now))
                    .size(LabelSize::Small)
                    .color(Color::Muted),
            ),
        )
        .child(
            div()
                .flex_none()
                .debug_selector(move || button_selector)
                .child(
                    Button::new(
                        SharedString::from(format!("use-limit-reset-{key}")),
                        if using { "Using Reset…" } else { "Use Reset" },
                    )
                    .style(ButtonStyle::Outlined)
                    .label_size(LabelSize::Small)
                    .disabled(using)
                    .on_click(on_use),
                ),
        )
        .into_any_element()
}

/// decisions.md §10's line under an account's limits: what's left of its extra usage, and
/// Manage, which opens the vendor's page for it. agentZ never turns paid usage on itself.
pub(crate) fn render_extra_usage(
    key: &str,
    extra_usage: &ExtraUsage,
    manage_page: Option<String>,
) -> AnyElement {
    let selector = format!("extra-usage-{key}");
    let button_selector = format!("extra-usage-manage-{key}");
    h_flex()
        .debug_selector(move || selector)
        .gap_2()
        .child(
            Icon::new(IconName::Coins)
                .size(IconSize::Small)
                .color(Color::Muted),
        )
        .child(
            Label::new(extra_usage.label.clone())
                .size(LabelSize::Small)
                .color(Color::Muted),
        )
        .child(
            div().flex_1().min_w_0().child(
                Label::new(extra_usage.summary.clone())
                    .size(LabelSize::Small)
                    .truncate(),
            ),
        )
        .children(manage_page.map(|url| {
            div()
                .flex_none()
                .debug_selector(move || button_selector)
                .child(
                    Button::new(
                        SharedString::from(format!("extra-usage-manage-{key}")),
                        "Manage",
                    )
                    .label_size(LabelSize::Small)
                    .color(Color::Accent)
                    .end_icon(
                        Icon::new(IconName::ArrowUpRight)
                            .size(IconSize::XSmall)
                            .color(Color::Accent),
                    )
                    .on_click(move |_, _, cx| cx.open_url(&url)),
                )
        }))
        .into_any_element()
}

/// `1 limit reset available · expires in 27d 23h`.
pub(crate) fn limit_resets_summary(resets: LimitResets, now: SystemTime) -> String {
    let count = if resets.available == 1 {
        "1 limit reset available".to_string()
    } else {
        format!("{} limit resets available", resets.available)
    };
    let expires_in = resets
        .next_expires_at
        .and_then(|expires_at| expires_at.duration_since(now).ok())
        .filter(|remaining| !remaining.is_zero())
        .map(format_duration);
    match expires_in {
        Some(expires_in) if resets.available == 1 => format!("{count} · expires in {expires_in}"),
        Some(expires_in) => format!("{count} · the next expires in {expires_in}"),
        None => count,
    }
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
    let fill = limit_color(left, cx);
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

/// What the composer's usage gauge opens: the thread's account, its plan and its windows, and
/// the agent's usage page, with the account's limit resets.
pub(crate) struct UsagePopover {
    client: Entity<ServerClient>,
    agent_id: AgentId,
    /// `None` being the External one.
    account: Option<AccountId>,
    usage_page: Option<String>,
    limit_reset: LimitResetAction,
    focus_handle: FocusHandle,
    _subscription: Subscription,
}

/// What the popover's Use Reset does: the thread asks first, so it closes the popover for the
/// thread's dialog.
#[derive(Clone)]
pub(crate) struct LimitResetAction {
    pub(crate) using: bool,
    pub(crate) ask: Rc<dyn Fn(&mut Window, &mut App)>,
}

impl EventEmitter<DismissEvent> for UsagePopover {}

impl Focusable for UsagePopover {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl UsagePopover {
    pub(crate) fn new(
        client: Entity<ServerClient>,
        agent_id: AgentId,
        account: Option<AccountId>,
        usage_page: Option<String>,
        limit_reset: LimitResetAction,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let focus_handle = cx.focus_handle();
        window.focus(&focus_handle, cx);
        // A read that lands while it's open shows in it.
        let subscription = cx.observe(&client, |_, _, cx| cx.notify());
        Self {
            client,
            agent_id,
            account,
            usage_page,
            limit_reset,
            focus_handle,
            _subscription: subscription,
        }
    }
}

impl Render for UsagePopover {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let accounts = self.client.read(cx).accounts(&self.agent_id);
        let entry = account_entry(&accounts, self.account);
        let usage_button = self.usage_page.clone().map(|url| {
            Button::new("usage-gauge-page", "Usage")
                .label_size(LabelSize::Small)
                .color(Color::Muted)
                .end_icon(
                    Icon::new(IconName::ArrowUpRight)
                        .size(IconSize::XSmall)
                        .color(Color::Muted),
                )
                .on_click(move |_, _, cx| cx.open_url(&url))
        });
        v_flex()
            .debug_selector(|| "usage-gauge-popover".into())
            .track_focus(&self.focus_handle)
            .on_mouse_down_out(cx.listener(|_, _, _, cx| cx.emit(DismissEvent)))
            // The composer binds Escape to Cancel, which would stop the turn.
            .on_action(cx.listener(|_, _: &menu::Cancel, _, cx| cx.emit(DismissEvent)))
            .on_key_down(cx.listener(|_, event: &KeyDownEvent, _, cx| {
                if event.keystroke.key == "escape" {
                    cx.emit(DismissEvent)
                }
            }))
            .elevation_3(cx)
            .w(px(420.))
            .px_3p5()
            .py_3()
            .gap_2p5()
            .child(
                h_flex()
                    .min_h(px(22.))
                    .gap_2()
                    .child(render_entry_avatar(&entry, px(18.), cx))
                    .child(
                        div()
                            .min_w_0()
                            .child(Label::new(entry.name.clone()).truncate()),
                    )
                    .children(
                        entry.plan.clone().map(|plan| {
                            Label::new(plan).size(LabelSize::Small).color(Color::Muted)
                        }),
                    )
                    .child(div().flex_1())
                    .children(usage_button),
            )
            .child(render_limit_windows(
                "gauge",
                &entry.windows,
                SystemTime::now(),
                cx,
            ))
            .children(entry.limit_resets.map(|resets| {
                let ask = self.limit_reset.ask.clone();
                div().pt_0p5().child(render_limit_resets(
                    "gauge",
                    resets,
                    self.limit_reset.using,
                    SystemTime::now(),
                    cx.listener(move |_, _, window, cx| {
                        cx.emit(DismissEvent);
                        ask(window, cx);
                    }),
                ))
            }))
    }
}

/// What's left's color: the accent, or yellow once it's low.
pub(crate) fn limit_color(left: u8, cx: &App) -> Hsla {
    if left <= LOW_PERCENT {
        cx.theme().status().warning
    } else {
        cx.theme().colors().text_accent
    }
}

/// The window closest to running out, which pickers show.
pub(crate) fn tightest_window(windows: &[LimitWindow]) -> Option<&LimitWindow> {
    windows.iter().min_by_key(|window| window.left_percent())
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

/// t3code's countdown on a segment: `↻ 2h 13m`.
pub(crate) fn format_short_resets_in(window: &LimitWindow, now: SystemTime) -> Option<String> {
    let resets_at = window.resets_at?;
    Some(match resets_at.duration_since(now) {
        Ok(remaining) if !remaining.is_zero() => format!("↻ {}", format_duration(remaining)),
        _ => "↻ now".to_string(),
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
    fn limit_resets_say_how_many_and_when_the_next_expires() {
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(1_000_000);
        let hours = |hours: u64| Duration::from_secs(hours * 3600);
        let one = LimitResets {
            available: 1,
            next_expires_at: Some(now + hours(27 * 24 + 23)),
        };
        assert_eq!(
            limit_resets_summary(one, now),
            "1 limit reset available · expires in 27d 23h"
        );
        let two = LimitResets {
            available: 2,
            next_expires_at: Some(now + hours(5)),
        };
        assert_eq!(
            limit_resets_summary(two, now),
            "2 limit resets available · the next expires in 5h 0m"
        );
        // Past its expiry, or without one, only the count.
        assert_eq!(
            limit_resets_summary(two, now + hours(6)),
            "2 limit resets available"
        );
        let never = LimitResets {
            next_expires_at: None,
            ..one
        };
        assert_eq!(limit_resets_summary(never, now), "1 limit reset available");
    }
}
