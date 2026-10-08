//! An account's limits as t3code's `LimitWindows` shows them: a row per window with its name
//! and what's left, a bar of what's left with a hairline where even spending would be, and when
//! it resets. The bar is colored by pace, as OpenUsage's meters are: whether the window lasts
//! until its reset at the rate it's been used. Also the cell tables show a window in, and the
//! popover of the composer's usage gauge.

use std::rc::Rc;
use std::time::{Duration, SystemTime};

use agentz_protocol::Request;
use agentz_protocol::accounts::{AccountId, ExtraUsage, LimitResets, LimitWindow};
use agentz_protocol::agents::AgentId;
use gpui::{
    AnyElement, App, ClickEvent, DismissEvent, Entity, EventEmitter, FocusHandle, Focusable,
    FontWeight, Hsla, KeyDownEvent, Subscription, Task, relative,
};
use ui::{Tooltip, prelude::*};

use crate::server_client::ServerClient;
use crate::settings_page::{account_entry, render_entry_avatar};
use crate::sidebar::format_relative_time;

/// At or under this much left, a bar whose pace isn't known turns yellow.
pub(crate) const LOW_PERCENT: u8 = 15;
const LABEL_WIDTH: Pixels = px(140.);
const RESET_WIDTH: Pixels = px(116.);
/// How often the gauge's popover redraws, so its countdowns and "Read 2m ago" move on.
const POPOVER_TICK: Duration = Duration::from_secs(5);

/// Whether a window lasts until its reset at the rate it's been used, as OpenUsage's `Pace`
/// and `meterState` judge it. `projected_used` is the percent it would have used by the reset.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Pace {
    /// On course to reset with at least 10% to spare.
    OnCourse { projected_used: f64 },
    /// On course to reset in its last 10%, with `spare` percent (at least 1) left.
    Close { spare: u8, projected_used: f64 },
    /// On course to run out before the reset: `runs_out_in` from now, or `None` when it lands
    /// at the limit right at the reset.
    RunsOut {
        runs_out_in: Option<Duration>,
        projected_used: f64,
    },
}

/// The window's pace at `now`, or `None` with nothing to go on: no length or reset, past the
/// reset, too early in the window (the first 1%, and at least a minute), nothing used yet, or
/// used up, which is red whatever the pace.
pub(crate) fn pace(window: &LimitWindow, now: SystemTime) -> Option<Pace> {
    if window.left_percent() == 0 {
        return None;
    }
    let length = window
        .length
        .filter(|length| !length.is_zero())?
        .as_secs_f64();
    let remaining = window
        .resets_at?
        .duration_since(now)
        .ok()
        .filter(|remaining| !remaining.is_zero())?
        .as_secs_f64();
    let elapsed = length - remaining;
    let used = window.used_percent.clamp(0., 100.);
    if used <= 0. || elapsed < (length * 0.01).max(60.) {
        return None;
    }
    let projected_used = used / elapsed * length;
    if projected_used <= 90. {
        return Some(Pace::OnCourse { projected_used });
    }
    // A whole-percent read of 1-4% early in a window projects a run-out while nearly all of it
    // is left; OpenUsage trusts no warning below 5% used.
    if used < 5. {
        return None;
    }
    if projected_used <= 100. {
        let spare = (100. - projected_used).round() as u8;
        return Some(if spare >= 1 {
            Pace::Close {
                spare,
                projected_used,
            }
        } else {
            Pace::RunsOut {
                runs_out_in: None,
                projected_used,
            }
        });
    }
    let runs_out_in = (100. - used) / (projected_used / length);
    Some(Pace::RunsOut {
        runs_out_in: (runs_out_in > 0. && runs_out_in < remaining)
            .then(|| Duration::from_secs_f64(runs_out_in)),
        projected_used,
    })
}

/// What the row says beside its reset: `~4% spare` when it will be close, `runs out in 9h 12m`
/// when it won't last. Nothing while it's on course.
pub(crate) fn pace_note(pace: Pace) -> Option<String> {
    match pace {
        Pace::OnCourse { .. } => None,
        Pace::Close { spare, .. } => Some(format!("~{spare}% spare")),
        Pace::RunsOut {
            runs_out_in: Some(runs_out_in),
            ..
        } => Some(format!("runs out in {}", format_countdown(runs_out_in))),
        Pace::RunsOut {
            runs_out_in: None, ..
        } => Some("runs out at the reset".to_string()),
    }
}

/// OpenUsage's tooltip: where the pace lands at the reset.
pub(crate) fn pace_projection(pace: Pace) -> String {
    match pace {
        Pace::OnCourse { projected_used } => {
            format!("~{}% left at reset", (100. - projected_used).round() as u8)
        }
        Pace::Close { projected_used, .. } => {
            format!("~{}% used at reset", projected_used.round() as u8)
        }
        Pace::RunsOut { projected_used, .. } if projected_used > 100. => format!(
            "~{}% over the limit at reset",
            ((projected_used - 100.).round() as u32).max(1)
        ),
        Pace::RunsOut { .. } => "~100% used at reset".to_string(),
    }
}

/// Past its reset, until the next read: what the last read says is out of date.
pub(crate) fn is_resetting(window: &LimitWindow, now: SystemTime) -> bool {
    window.resets_at.is_some_and(|resets_at| resets_at <= now)
}

/// The bar's fill: red when used up or running out first, yellow when it will be close, the
/// accent on course. Without a pace, yellow once it's low.
pub(crate) fn fill_color(window: &LimitWindow, now: SystemTime, cx: &App) -> Hsla {
    let status = cx.theme().status();
    let left = window.left_percent();
    if left == 0 {
        return status.error;
    }
    match pace(window, now) {
        Some(Pace::OnCourse { .. }) => cx.theme().colors().text_accent,
        Some(Pace::Close { .. }) => status.warning,
        Some(Pace::RunsOut { .. }) => status.error,
        None => limit_color(left, cx),
    }
}

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
    // Past the reset, the last read's percentage is out of date until the read at the reset.
    let left_label = if is_resetting(window, now) {
        None
    } else if left == 0 {
        Some(Label::new("Used up").color(Color::Error))
    } else {
        Some(Label::new(format!("{left}% left")))
    };
    let note = pace(window, now).and_then(|pace| render_pace_note(key, index, pace));
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
                .children(
                    left_label.map(|label| label.size(LabelSize::Small).weight(FontWeight::MEDIUM)),
                ),
        )
        .child(render_bar(key, index, window, now, px(6.), cx))
        .child(
            v_flex()
                .w(RESET_WIDTH)
                .flex_none()
                .items_end()
                .child(
                    Label::new(format_resets_in(window, now).unwrap_or_default())
                        .size(LabelSize::Small)
                        .color(Color::Muted),
                )
                .children(note),
        )
        .into_any_element()
}

/// `~4% spare` in yellow, or the flame and `runs out in 9h 12m` in red.
fn render_pace_note(key: &str, index: usize, pace: Pace) -> Option<AnyElement> {
    let note = pace_note(pace)?;
    let color = match pace {
        Pace::RunsOut { .. } => Color::Error,
        _ => Color::Warning,
    };
    let selector = format!("limit-{key}-pace-{index}");
    Some(
        h_flex()
            .debug_selector(move || selector)
            .gap_0p5()
            .when(matches!(pace, Pace::RunsOut { .. }), |note| {
                note.child(
                    Icon::new(IconName::Flame)
                        .size(IconSize::XSmall)
                        .color(color),
                )
            })
            .child(Label::new(note).size(LabelSize::XSmall).color(color))
            .into_any_element(),
    )
}

/// A window in a table (Settings › Usage): what's left and when it resets, over a thin bar.
/// `key` and `index` name its bar in tests, as a row's.
pub(crate) fn render_limit_cell(
    key: &str,
    index: usize,
    window: &LimitWindow,
    now: SystemTime,
    cx: &App,
) -> AnyElement {
    let left = window.left_percent();
    let (text, color) = if is_resetting(window, now) {
        ("resetting".to_string(), Color::Muted)
    } else if left == 0 {
        ("Used up".to_string(), Color::Error)
    } else {
        (format!("{left}%"), Color::Default)
    };
    v_flex()
        .min_w_0()
        .gap_1()
        .child(
            h_flex()
                .gap_1p5()
                .child(
                    Label::new(text)
                        .size(LabelSize::Small)
                        .weight(FontWeight::MEDIUM)
                        .color(color),
                )
                .child(div().flex_1())
                .children(format_short_resets_in(window, now).map(|resets| {
                    div().flex_none().child(
                        Label::new(resets)
                            .size(LabelSize::XSmall)
                            .color(Color::Placeholder),
                    )
                })),
        )
        .child(render_bar(key, index, window, now, px(4.), cx))
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

/// The fill is what's left, colored by pace; the hairline is the share of the window still to
/// come, where spending evenly would have left the fill. The tooltip has the exact figures, the
/// pace and the time. Past the reset it's empty until the next read.
fn render_bar(
    key: &str,
    index: usize,
    window: &LimitWindow,
    now: SystemTime,
    height: Pixels,
    cx: &App,
) -> AnyElement {
    let colors = cx.theme().colors();
    let status = cx.theme().status();
    let resetting = is_resetting(window, now);
    let left = if resetting { 0 } else { window.left_percent() };
    let time_left = window.time_left(now).filter(|_| !resetting);
    let fill = fill_color(window, now, cx);
    let track = if left == 0 && !resetting {
        status.error.opacity(0.35)
    } else {
        colors.border
    };
    let summary = if resetting {
        "Resetting".to_string()
    } else {
        match time_left {
            Some(share) => format!(
                "{left}% left · {}% of the window left",
                (share * 100.).round() as u8
            ),
            None => format!("{left}% left"),
        }
    };
    let projection = pace(window, now).map(|pace| match pace_note(pace) {
        Some(note) => format!("{} · {}", capitalize(&note), pace_projection(pace)),
        None => capitalize(&pace_projection(pace)),
    });
    let resets = window.resets_at.map(|resets_at| {
        let mut text = format!("Resets {}", reset_time(resets_at, now));
        if let Some(resets_in) = format_resets_in(window, now).filter(|_| !resetting) {
            text.push_str(&format!(" · {resets_in}"));
        }
        text
    });
    let has_hairline = time_left.is_some();
    let bar_selector = format!("limit-{key}-bar-{index}");
    let hairline_selector = format!("limit-{key}-hairline-{index}");
    div()
        .id(SharedString::from(bar_selector.clone()))
        .debug_selector(move || bar_selector)
        .flex_1()
        .min_w_0()
        .h(height * 2.)
        .flex()
        .items_center()
        .relative()
        .child(
            div()
                .relative()
                .w_full()
                .h(height)
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
                .children(projection.clone().map(|projection| {
                    Label::new(projection)
                        .size(LabelSize::Small)
                        .color(Color::Muted)
                }))
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

fn capitalize(text: &str) -> String {
    let mut characters = text.chars();
    match characters.next() {
        Some(first) => first.to_uppercase().chain(characters).collect(),
        None => String::new(),
    }
}

/// What the composer's usage gauge opens: the thread's account, its plan and its windows, and
/// the agent's usage page, with the account's limit resets, and when it was read, with
/// Refresh.
pub(crate) struct UsagePopover {
    client: Entity<ServerClient>,
    agent_id: AgentId,
    /// `None` being the External one.
    account: Option<AccountId>,
    usage_page: Option<String>,
    limit_reset: LimitResetAction,
    focus_handle: FocusHandle,
    _subscription: Subscription,
    _tick: Task<()>,
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
        let tick = cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(POPOVER_TICK).await;
                if this.update(cx, |_, cx| cx.notify()).is_err() {
                    break;
                }
            }
        });
        Self {
            client,
            agent_id,
            account,
            usage_page,
            limit_reset,
            focus_handle,
            _subscription: subscription,
            _tick: tick,
        }
    }

    /// The footer: when the account was last read, and Refresh, which reads it now.
    fn render_footer(&self, read_at: Option<SystemTime>, cx: &mut Context<Self>) -> AnyElement {
        let read = read_at.map(|read_at| {
            match format_relative_time(read_at, SystemTime::now()).as_str() {
                "now" => "Read just now".to_string(),
                ago => format!("Read {ago} ago"),
            }
        });
        h_flex()
            .debug_selector(|| "usage-gauge-footer".into())
            .gap_2()
            .children(read.map(|read| {
                Label::new(read)
                    .size(LabelSize::XSmall)
                    .color(Color::Placeholder)
            }))
            .child(div().flex_1())
            .child(
                div().debug_selector(|| "usage-gauge-refresh".into()).child(
                    Button::new("usage-gauge-refresh", "Refresh")
                        .label_size(LabelSize::XSmall)
                        .color(Color::Muted)
                        .start_icon(
                            Icon::new(IconName::RotateCw)
                                .size(IconSize::XSmall)
                                .color(Color::Muted),
                        )
                        .on_click(cx.listener(|this, _, _, cx| {
                            let request = Request::RefreshUsage {
                                agent_id: this.agent_id.clone(),
                                account: this.account,
                            };
                            this.client.read(cx).send(request, cx);
                        })),
                ),
            )
            .into_any_element()
    }
}

impl Render for UsagePopover {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let accounts = self.client.read(cx).accounts(&self.agent_id);
        let entry = account_entry(&accounts, self.account);
        let read_at = accounts.status(self.account).map(|read| read.read_at);
        let footer = self.render_footer(read_at, cx);
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
            .child(footer)
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

/// The window closest to running out, which pickers and the gauge show. One past its reset is
/// left out, its last read being out of date.
pub(crate) fn tightest_window(windows: &[LimitWindow], now: SystemTime) -> Option<&LimitWindow> {
    windows
        .iter()
        .filter(|window| !is_resetting(window, now))
        .min_by_key(|window| window.left_percent())
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
        Some(remaining) => format!("Used up · {}", format_countdown(remaining)),
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

/// [`format_duration`] for a time still to come, which never says `0m`: `under 1m`.
pub(crate) fn format_countdown(duration: Duration) -> String {
    if duration < Duration::from_secs(60) {
        "under 1m".to_string()
    } else {
        format_duration(duration)
    }
}

/// `resets in 2h 13m`, `resets in under 1m`, then `resetting` until the next read, or `None`
/// when the window has no reset.
pub(crate) fn format_resets_in(window: &LimitWindow, now: SystemTime) -> Option<String> {
    let resets_at = window.resets_at?;
    Some(match resets_at.duration_since(now) {
        Ok(remaining) if !remaining.is_zero() => {
            format!("resets in {}", format_countdown(remaining))
        }
        _ => "resetting".to_string(),
    })
}

/// t3code's countdown in a table's cell: `↻ 2h 13m`, `↻ under 1m`. Past the reset the cell
/// says it's resetting instead.
pub(crate) fn format_short_resets_in(window: &LimitWindow, now: SystemTime) -> Option<String> {
    let remaining = window
        .resets_at?
        .duration_since(now)
        .ok()
        .filter(|remaining| !remaining.is_zero())?;
    Some(format!("↻ {}", format_countdown(remaining)))
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
            format_short_resets_in(&window, now).as_deref(),
            Some("↻ 2h 10m")
        );
        // Under a minute it never says 0m, and past the reset it doesn't count down.
        let almost = now + minutes(130) - Duration::from_secs(30);
        assert_eq!(
            format_resets_in(&window, almost).as_deref(),
            Some("resets in under 1m")
        );
        assert_eq!(
            format_short_resets_in(&window, almost).as_deref(),
            Some("↻ under 1m")
        );
        assert!(!is_resetting(&window, almost));
        let past = now + minutes(200);
        assert_eq!(
            format_resets_in(&window, past).as_deref(),
            Some("resetting")
        );
        assert_eq!(format_short_resets_in(&window, past), None);
        assert!(is_resetting(&window, past));
        let without_reset = LimitWindow {
            resets_at: None,
            ..window
        };
        assert_eq!(format_resets_in(&without_reset, now), None);
    }

    /// OpenUsage's pace: the use so far, projected to the reset at the same rate.
    #[test]
    fn pace_projects_the_rate_to_the_reset() {
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(1_000_000);
        let hours = |hours: u64| Duration::from_secs(hours * 3600);
        // A 10-hour window with `elapsed` hours gone.
        let window = |used_percent: f64, elapsed: u64| LimitWindow {
            label: "Session".into(),
            used_percent,
            resets_at: Some(now + hours(10 - elapsed)),
            length: Some(hours(10)),
        };
        let pace_of = |used_percent, elapsed| pace(&window(used_percent, elapsed), now);

        // 40% in 5 hours lands at 80%: on course, with nothing to say.
        let on_course = pace_of(40., 5).expect("a pace");
        assert!(matches!(on_course, Pace::OnCourse { .. }));
        assert_eq!(pace_note(on_course), None);
        assert_eq!(pace_projection(on_course), "~20% left at reset");

        // 48% in 5 hours lands at 96%: close, 4% to spare.
        let close = pace_of(48., 5).expect("a pace");
        assert!(matches!(close, Pace::Close { spare: 4, .. }));
        assert_eq!(pace_note(close).as_deref(), Some("~4% spare"));
        assert_eq!(pace_projection(close), "~96% used at reset");

        // 80% in 4 hours lands at 200%, and the rest goes in an hour.
        let runs_out = pace_of(80., 4).expect("a pace");
        let Pace::RunsOut {
            runs_out_in: Some(runs_out_in),
            projected_used,
        } = runs_out
        else {
            panic!("expected it to run out, not {runs_out:?}");
        };
        assert!(runs_out_in.abs_diff(hours(1)) < Duration::from_secs(1));
        assert!((projected_used - 200.).abs() < 0.001);
        assert_eq!(pace_projection(runs_out), "~100% over the limit at reset");
        let in_nine_hours = Pace::RunsOut {
            runs_out_in: Some(hours(9) + Duration::from_secs(12 * 60)),
            projected_used: 150.,
        };
        assert_eq!(
            pace_note(in_nine_hours).as_deref(),
            Some("runs out in 9h 12m")
        );
        let in_seconds = Pace::RunsOut {
            runs_out_in: Some(Duration::from_secs(20)),
            projected_used: 150.,
        };
        assert_eq!(
            pace_note(in_seconds).as_deref(),
            Some("runs out in under 1m")
        );

        // Landing at the limit with under 1% spare is red, with no time.
        let at_the_limit = pace_of(49.8, 5).expect("a pace");
        assert!(matches!(
            at_the_limit,
            Pace::RunsOut {
                runs_out_in: None,
                ..
            }
        ));
        assert_eq!(
            pace_note(at_the_limit).as_deref(),
            Some("runs out at the reset")
        );
        assert_eq!(pace_projection(at_the_limit), "~100% used at reset");

        // No pace with nothing to go on: used up, nothing used, too early (the first 1% of the
        // window), past the reset, a warning from under 5% used, or no length.
        assert_eq!(pace_of(100., 5), None);
        assert_eq!(pace_of(0., 5), None);
        let early = LimitWindow {
            resets_at: Some(now + hours(10) - Duration::from_secs(300)),
            ..window(30., 0)
        };
        assert_eq!(pace(&early, now), None);
        assert_eq!(pace(&window(40., 5), now + hours(6)), None);
        // 4% in 20 minutes projects a run-out, but under 5% used is too little to warn on.
        let barely_used = LimitWindow {
            resets_at: Some(now + hours(10) - Duration::from_secs(20 * 60)),
            ..window(4., 0)
        };
        assert_eq!(pace(&barely_used, now), None);
        assert!(matches!(pace_of(4., 5), Some(Pace::OnCourse { .. })));
        let without_length = LimitWindow {
            length: None,
            ..window(40., 5)
        };
        assert_eq!(pace(&without_length, now), None);
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
