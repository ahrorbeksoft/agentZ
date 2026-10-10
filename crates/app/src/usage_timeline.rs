//! Settings › Usage's timeline of every account's limit windows (design/subscription-timeline),
//! drawn as CLIProxyAPI's management UI draws its credentials' quota windows
//! (`QuotaTimeline.tsx`): a lane per account with its longest window that fits the view, the
//! current one from when it opened to its reset, filled with what's used, then those to come,
//! dashed, from today on. agentZ knows only a window's length and next reset, so the ones to
//! come are worked out back to back, the earliest they could open.

use std::rc::Rc;
use std::time::{Duration, SystemTime};

use agentz_protocol::accounts::{LimitResets, LimitWindow};
use agentz_protocol::agents::AgentId;
use chrono::{DateTime, Datelike, Days, Local, NaiveDate, Weekday};
use gpui::{AnyElement, App, Hsla, canvas, relative};
use ui::{ToggleButtonGroup, ToggleButtonGroupStyle, ToggleButtonSimple, Tooltip, prelude::*};

use crate::settings_page::{AccountEntry, account_selector, render_entry_avatar};
use crate::usage_limits::{
    Pace, fill_color, format_countdown, format_duration, is_resetting, pace, pace_projection,
};

const HEAD_WIDTH: Pixels = px(168.);
const LANE_HEIGHT: Pixels = px(46.);
/// A lane whose current window's words don't fit in its bar puts them above it.
const TALL_LANE_HEIGHT: Pixels = px(52.);
const AVATAR_SIZE: Pixels = px(16.);
const HOUR: Duration = Duration::from_secs(3600);
/// A bar narrower than this has no words of its own.
const NEXT_LABEL_ROOM: Pixels = px(92.);

/// The two views, as CLIProxyAPI's Weekly and 5-hour switch.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum TimelineZoom {
    /// Two weeks from today, a cell a day, each account's longest window that fits.
    #[default]
    Weekly,
    /// Three days from today, a cell per 6 hours, and only windows five hours long, whatever
    /// their agent calls them.
    FiveHour,
}

impl TimelineZoom {
    fn days(self) -> u64 {
        match self {
            Self::Weekly => 14,
            Self::FiveHour => 3,
        }
    }

    fn cell_hours(self) -> u64 {
        match self {
            Self::Weekly => 24,
            Self::FiveHour => 6,
        }
    }

    fn words(self) -> &'static str {
        match self {
            Self::Weekly => "two weeks",
            Self::FiveHour => "three days",
        }
    }
}

/// An agent's lanes: its accounts as the Usage page's table lists them.
pub(crate) struct TimelineAgent {
    pub(crate) id: AgentId,
    pub(crate) name: SharedString,
    pub(crate) icon: Option<SharedString>,
    pub(crate) accounts: Vec<AccountEntry>,
}

/// What the view shows: from today's midnight, the zoom's days.
#[derive(Clone, Copy, Debug)]
struct Span {
    start: SystemTime,
    end: SystemTime,
    today: NaiveDate,
}

impl Span {
    fn new(zoom: TimelineZoom, now: SystemTime) -> Self {
        let today = DateTime::<Local>::from(now).date_naive();
        // A clock change at midnight has no midnight; the view then starts at the hour.
        let start = today
            .and_hms_opt(0, 0, 0)
            .and_then(|midnight| midnight.and_local_timezone(Local).earliest())
            .map(SystemTime::from)
            .unwrap_or(now);
        Self {
            start,
            end: start + HOUR * (zoom.days() * 24) as u32,
            today,
        }
    }

    /// Where `at` is across the view, 0 to 1.
    fn fraction(&self, at: SystemTime) -> f32 {
        let total = self
            .end
            .duration_since(self.start)
            .unwrap_or_default()
            .as_secs_f64();
        if total <= 0. {
            return 0.;
        }
        let offset = match at.duration_since(self.start) {
            Ok(after) => after.as_secs_f64(),
            Err(before) => -before.duration().as_secs_f64(),
        };
        (offset / total).clamp(0., 1.) as f32
    }

    fn contains(&self, at: SystemTime) -> bool {
        at > self.start && at < self.end
    }

    fn last_day(&self, zoom: TimelineZoom) -> NaiveDate {
        self.today
            .checked_add_days(Days::new(zoom.days() - 1))
            .unwrap_or(self.today)
    }
}

/// A column of the axis: a day (its weekday over its date), or a 6-hour part of one.
#[derive(Debug, PartialEq)]
struct AxisCell {
    weekday: Option<String>,
    label: String,
    today: bool,
    weekend: bool,
    /// A part of a day, after its first, whose line is fainter.
    minor: bool,
}

/// The axis's cells. The view's first day and a month's first day name their month; with
/// 6-hour cells each day does, its other parts their hour.
fn axis_cells(zoom: TimelineZoom, today: NaiveDate) -> Vec<AxisCell> {
    let per_day = 24 / zoom.cell_hours();
    (0..zoom.days() * per_day)
        .map(|index| {
            let day = index / per_day;
            let hour = (index % per_day) * zoom.cell_hours();
            let date = today.checked_add_days(Days::new(day)).unwrap_or(today);
            if hour > 0 {
                return AxisCell {
                    weekday: None,
                    label: format!("{hour:02}:00"),
                    today: day == 0,
                    weekend: false,
                    minor: true,
                };
            }
            let names_month = index == 0 || date.day() == 1 || zoom == TimelineZoom::FiveHour;
            AxisCell {
                weekday: Some(date.format("%a").to_string()),
                label: if names_month {
                    date.format("%b %-d").to_string()
                } else {
                    date.day().to_string()
                },
                today: day == 0,
                weekend: zoom == TimelineZoom::Weekly
                    && matches!(date.weekday(), Weekday::Sat | Weekday::Sun),
                minor: false,
            }
        })
        .collect()
}

/// The window an account's lane draws: CLIProxyAPI's `pickLaneWindow`, its longest that fits
/// the two weeks, or the one five hours long. A window without a length can't be placed.
fn lane_window(windows: &[LimitWindow], zoom: TimelineZoom) -> Option<&LimitWindow> {
    let fits = |window: &&LimitWindow| match (zoom, window.length) {
        (_, None) => false,
        (_, Some(length)) if length < HOUR => false,
        (TimelineZoom::Weekly, Some(length)) => length <= HOUR * (14 * 24),
        (TimelineZoom::FiveHour, Some(length)) => length == HOUR * 5,
    };
    windows
        .iter()
        .filter(fits)
        .max_by_key(|window| window.length)
}

#[derive(Debug, PartialEq)]
struct Bar {
    from: SystemTime,
    to: SystemTime,
    current: bool,
}

/// The window's bars in the view: the current one, then those to come, a length apart.
fn bars(window: &LimitWindow, span: &Span) -> Vec<Bar> {
    let (Some(length), Some(resets_at)) = (window.length, window.resets_at) else {
        return Vec::new();
    };
    if length < HOUR {
        return Vec::new();
    }
    let mut bars = vec![Bar {
        from: resets_at.checked_sub(length).unwrap_or(resets_at),
        to: resets_at,
        current: true,
    }];
    let mut from = resets_at;
    while from < span.end {
        bars.push(Bar {
            from,
            to: from + length,
            current: false,
        });
        from += length;
    }
    bars.retain(|bar| bar.to > span.start && bar.from < span.end);
    bars
}

/// `Oct 12, 08:20`.
fn moment(at: SystemTime) -> String {
    DateTime::<Local>::from(at)
        .format("%b %-d, %H:%M")
        .to_string()
}

/// The current window's words, as the tables count down: `13% left · resets in 2d 18h`.
fn current_label(window: &LimitWindow, now: SystemTime) -> String {
    if is_resetting(window, now) {
        return "Resetting".to_string();
    }
    let left = match window.left_percent() {
        0 => "Used up".to_string(),
        left => format!("{left}% left"),
    };
    match window
        .resets_at
        .and_then(|resets_at| resets_at.duration_since(now).ok())
    {
        Some(remaining) => format!("{left} · resets in {}", format_countdown(remaining)),
        None => left,
    }
}

/// The current bar's tooltip, in the limit rows' words: the window and what's left, when it
/// opened and resets, and where the pace lands.
fn window_tooltip(window: &LimitWindow, now: SystemTime) -> Vec<(String, Color)> {
    let left = match window.left_percent() {
        0 => "Used up".to_string(),
        left => format!("{left}% left"),
    };
    let mut lines = vec![(format!("{} · {left}", window.label), Color::Default)];
    if let (Some(length), Some(resets_at)) = (window.length, window.resets_at) {
        let opened = resets_at.checked_sub(length).unwrap_or(resets_at);
        lines.push((
            format!("Opened {} · resets {}", moment(opened), moment(resets_at)),
            Color::Muted,
        ));
    }
    match pace(window, now) {
        Some(
            pace @ Pace::RunsOut {
                runs_out_in: Some(runs_out_in),
                ..
            },
        ) => {
            lines.push((
                format!("Runs out {}, at this pace", moment(now + runs_out_in)),
                Color::Error,
            ));
            lines.push((pace_projection(pace), Color::Muted));
        }
        Some(pace) => lines.push((pace_projection(pace), Color::Muted)),
        None => {}
    }
    lines
}

/// A limit reset's tick: how many there are and when the first expires, and whether that's
/// before the lane's window resets, when using it would still give something back.
fn resets_tooltip(
    resets: LimitResets,
    expires_at: SystemTime,
    window: &LimitWindow,
    now: SystemTime,
) -> Vec<String> {
    let remaining = expires_at.duration_since(now).unwrap_or_default();
    let when = format!("{}, in {}", moment(expires_at), format_duration(remaining));
    let mut lines = if resets.available == 1 {
        vec!["Limit reset".to_string(), format!("Expires {when}")]
    } else {
        vec![
            format!("{} limit resets", resets.available),
            format!("The first expires {when}"),
        ]
    };
    if window
        .resets_at
        .is_some_and(|resets_at| expires_at < resets_at)
    {
        lines.push("before this window resets".to_string());
    }
    lines
}

/// What a lane draws, worked out before its elements.
struct Lane<'a> {
    agent: &'a AgentId,
    entry: &'a AccountEntry,
    window: &'a LimitWindow,
}

/// What's needed to draw the timeline.
pub(crate) struct UsageTimeline<'a> {
    pub(crate) agents: &'a [TimelineAgent],
    pub(crate) zoom: TimelineZoom,
    /// The tracks' width when they were last drawn, which says whether a bar's words fit in it.
    pub(crate) track_width: Pixels,
    pub(crate) now: SystemTime,
    pub(crate) on_zoom: Rc<dyn Fn(TimelineZoom, &mut Window, &mut App)>,
    pub(crate) on_open: Rc<dyn Fn(&AgentId, &mut Window, &mut App)>,
    pub(crate) on_track_width: Rc<dyn Fn(Pixels, &mut Window, &mut App)>,
}

impl UsageTimeline<'_> {
    pub(crate) fn render(self, cx: &App) -> AnyElement {
        let colors = cx.theme().colors().clone();
        let span = Span::new(self.zoom, self.now);
        let cells = axis_cells(self.zoom, span.today);
        let mut rows: Vec<AnyElement> = Vec::new();
        let mut any_lane = false;
        let mut any_tick = false;
        for agent in self.agents {
            let lanes: Vec<Lane> = agent
                .accounts
                .iter()
                .filter_map(|entry| {
                    Some(Lane {
                        agent: &agent.id,
                        entry,
                        window: lane_window(&entry.windows, self.zoom)?,
                    })
                })
                .collect();
            if lanes.is_empty() {
                continue;
            }
            rows.push(render_group(agent, cx));
            for lane in lanes {
                any_lane = true;
                any_tick |= tick_at(lane.entry, &span).is_some();
                rows.push(self.render_lane(&lane, &span, &cells, cx));
            }
        }
        let empty = (!any_lane).then(|| {
            div()
                .px_3p5()
                .py_2p5()
                .border_t_1()
                .border_color(colors.border_variant)
                .child(
                    Label::new(match self.zoom {
                        TimelineZoom::Weekly => "No account reports a window of two weeks or less.",
                        TimelineZoom::FiveHour => "No account reports a 5-hour window.",
                    })
                    .size(LabelSize::Small)
                    .color(Color::Muted),
                )
        });
        v_flex()
            .debug_selector(|| "usage-timeline".into())
            .rounded_lg()
            .border_1()
            .border_color(colors.border)
            .overflow_hidden()
            .child(self.render_head(&span, cx))
            .child(self.render_axis(&cells, cx))
            .children(rows)
            .children(empty)
            .when(any_lane, |timeline| {
                timeline.child(render_legend(self.zoom, any_tick, cx))
            })
            .into_any_element()
    }

    fn render_head(&self, span: &Span, _cx: &App) -> AnyElement {
        let range = format!(
            "{} – {} · from today · {}",
            span.today.format("%b %-d"),
            span.last_day(self.zoom).format("%b %-d"),
            self.zoom.words()
        );
        let on_weekly = self.on_zoom.clone();
        let on_five_hour = self.on_zoom.clone();
        h_flex()
            .px_3p5()
            .pt_3()
            .pb_2p5()
            .gap_3()
            .items_start()
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .gap_0p5()
                    .child(Label::new("Limit windows").weight(gpui::FontWeight::MEDIUM))
                    .child(
                        Label::new(range)
                            .size(LabelSize::XSmall)
                            .color(Color::Placeholder),
                    ),
            )
            .child(
                div().debug_selector(|| "usage-timeline-zoom".into()).child(
                    ToggleButtonGroup::single_row(
                        "usage-timeline-zoom",
                        [
                            ToggleButtonSimple::new("Weekly", move |_, window, cx| {
                                on_weekly(TimelineZoom::Weekly, window, cx)
                            }),
                            ToggleButtonSimple::new("5-hour", move |_, window, cx| {
                                on_five_hour(TimelineZoom::FiveHour, window, cx)
                            }),
                        ],
                    )
                    .style(ToggleButtonGroupStyle::Outlined)
                    .label_size(LabelSize::Small)
                    .auto_width()
                    .selected_index(match self.zoom {
                        TimelineZoom::Weekly => 0,
                        TimelineZoom::FiveHour => 1,
                    }),
                ),
            )
            .into_any_element()
    }

    fn render_axis(&self, cells: &[AxisCell], cx: &App) -> AnyElement {
        let colors = cx.theme().colors().clone();
        let accent = colors.text_accent;
        let measured = self.track_width;
        let on_track_width = self.on_track_width.clone();
        h_flex()
            .border_t_1()
            .border_color(colors.border_variant)
            .child(
                div().w(HEAD_WIDTH).flex_none().px_3().child(
                    Label::new("Account")
                        .size(LabelSize::XSmall)
                        .color(Color::Placeholder),
                ),
            )
            .child(
                h_flex()
                    .relative()
                    .flex_1()
                    .min_w_0()
                    .children(cells.iter().map(|cell| {
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .overflow_hidden()
                            .items_center()
                            .py_1()
                            .border_l_1()
                            .border_color(grid_line(cell, &colors))
                            .when(cell.weekend, |cell| cell.bg(colors.text.opacity(0.018)))
                            .when(cell.today, |cell| cell.bg(accent.opacity(0.1)))
                            .child(
                                div()
                                    .h(px(12.))
                                    .children(cell.weekday.clone().map(|weekday| {
                                        Label::new(weekday).size(LabelSize::XSmall).color(
                                            if cell.today {
                                                Color::Accent
                                            } else {
                                                Color::Muted
                                            },
                                        )
                                    })),
                            )
                            .child(
                                Label::new(cell.label.clone())
                                    .size(LabelSize::XSmall)
                                    .color(if cell.today {
                                        Color::Default
                                    } else {
                                        Color::Placeholder
                                    })
                                    .single_line(),
                            )
                    }))
                    .child(
                        canvas(
                            move |bounds, window, cx| {
                                let width = bounds.size.width;
                                if width != measured {
                                    let on_track_width = on_track_width.clone();
                                    window.defer(cx, move |window, cx| {
                                        on_track_width(width, window, cx)
                                    });
                                }
                            },
                            |_, _, _, _| {},
                        )
                        .absolute()
                        .size_full(),
                    ),
            )
            .into_any_element()
    }

    fn render_lane(&self, lane: &Lane, span: &Span, cells: &[AxisCell], cx: &App) -> AnyElement {
        let colors = cx.theme().colors().clone();
        let key = format!(
            "timeline-{}-{}",
            lane.agent.0,
            account_selector(lane.entry.account)
        );
        let window = lane.window;
        let now = self.now;
        let accent = colors.text_accent;
        let current_color = fill_color(window, now, cx);
        let bars = bars(window, span);
        let track_width = self.track_width;
        let visible_width =
            |bar: &Bar| track_width * (span.fraction(bar.to) - span.fraction(bar.from)).max(0.);
        let label = current_label(window, now);
        // CLIProxyAPI's measure: about 6 px a character, and the bar's padding.
        let room = px(label.chars().count() as f32 * 6. + 18.);
        let label_above = bars
            .iter()
            .find(|bar| bar.current)
            .is_some_and(|bar| visible_width(bar) < room);
        let (bar_top, bar_height) = if label_above {
            (px(24.), px(18.))
        } else {
            (px(12.), px(22.))
        };
        let mut track = div()
            .relative()
            .flex_1()
            .min_w_0()
            .h(if label_above {
                TALL_LANE_HEIGHT
            } else {
                LANE_HEIGHT
            })
            .child(
                h_flex()
                    .absolute()
                    .top_0()
                    .bottom_0()
                    .left_0()
                    .right_0()
                    .children(cells.iter().map(|cell| {
                        div()
                            .flex_1()
                            .h_full()
                            .border_l_1()
                            .border_color(grid_line(cell, &colors))
                            .when(cell.weekend, |cell| cell.bg(colors.text.opacity(0.018)))
                            .when(cell.today, |cell| cell.bg(accent.opacity(0.05)))
                    })),
            );
        if span.contains(now) {
            track = track.child(
                div()
                    .absolute()
                    .top_0()
                    .bottom_0()
                    .left(relative(span.fraction(now)))
                    .w(px(1.))
                    .bg(colors.text_muted.opacity(0.85)),
            );
        }
        if window.resets_at.is_none() {
            track = track.child(
                div()
                    .debug_selector({
                        let key = key.clone();
                        move || format!("{key}-idle")
                    })
                    .absolute()
                    .top_0()
                    .bottom_0()
                    .left(px(10.))
                    .flex()
                    .items_center()
                    .child(
                        Label::new(match window.left_percent() {
                            100 => "All left · no window counting down".to_string(),
                            left => format!("{left}% left · no window counting down"),
                        })
                        .size(LabelSize::XSmall)
                        .color(Color::Placeholder)
                        .italic(),
                    ),
            );
        }
        for (index, bar) in bars.iter().enumerate() {
            let left = span.fraction(bar.from);
            let width = (span.fraction(bar.to) - left).max(0.);
            let element = div()
                .absolute()
                .top(bar_top)
                .h(bar_height)
                .left(relative(left))
                .w(relative(width))
                .rounded_full()
                .overflow_hidden()
                .flex()
                .items_center()
                .px_2()
                .border_1();
            if bar.current {
                let visible_from = bar.from.max(span.start);
                let visible = bar.to.max(visible_from).duration_since(visible_from);
                let length = bar.to.duration_since(bar.from).unwrap_or_default();
                let used = window.used_percent.clamp(0., 100.) / 100.;
                let fill_at = bar.from + length.mul_f64(used);
                let fill = match visible {
                    Ok(visible) if !visible.is_zero() => (fill_at
                        .duration_since(visible_from)
                        .unwrap_or_default()
                        .as_secs_f64()
                        / visible.as_secs_f64())
                    .clamp(0., 1.) as f32,
                    _ => 0.,
                };
                let tooltip = window_tooltip(window, now);
                let selector = format!("{key}-current");
                track = track.child(
                    element
                        .id(SharedString::from(selector.clone()))
                        .debug_selector(move || selector)
                        .bg(current_color.opacity(0.13))
                        .border_color(current_color.opacity(0.6))
                        .when(fill > 0., |bar| {
                            bar.child(
                                div()
                                    .absolute()
                                    .left_0()
                                    .top_0()
                                    .bottom_0()
                                    .w(relative(fill))
                                    .rounded_full()
                                    .bg(current_color.opacity(0.52)),
                            )
                        })
                        .when(!label_above, |bar| {
                            bar.child(
                                div().relative().min_w_0().child(
                                    Label::new(label.clone())
                                        .size(LabelSize::XSmall)
                                        .single_line()
                                        .truncate(),
                                ),
                            )
                        })
                        .tooltip(Tooltip::element(move |_, _| {
                            v_flex()
                                .gap_0p5()
                                .children(tooltip.iter().enumerate().map(
                                    |(index, (line, color))| {
                                        Label::new(line.clone())
                                            .size(LabelSize::Small)
                                            .color(*color)
                                            .when(index == 0, |label| {
                                                label.weight(gpui::FontWeight::MEDIUM)
                                            })
                                    },
                                ))
                                .into_any_element()
                        })),
                );
                if label_above {
                    track = track.child(
                        div().absolute().top(px(5.)).left(relative(left)).child(
                            Label::new(label.clone())
                                .size(LabelSize::XSmall)
                                .single_line(),
                        ),
                    );
                }
            } else {
                let selector = format!("{key}-next-{index}");
                let words = (visible_width(bar) > NEXT_LABEL_ROOM)
                    .then(|| format!("resets {}", moment(bar.to)));
                track = track.child(
                    element
                        .debug_selector(move || selector)
                        .justify_end()
                        .border_dashed()
                        .border_color(accent.opacity(0.38))
                        .children(words.map(|words| {
                            div().min_w_0().child(
                                Label::new(words)
                                    .size(LabelSize::XSmall)
                                    .color(Color::Placeholder)
                                    .single_line()
                                    .truncate(),
                            )
                        })),
                );
            }
        }
        if let Some((resets, expires_at)) = tick_at(lane.entry, span) {
            let lines = resets_tooltip(resets, expires_at, window, now);
            let selector = format!("{key}-limit-reset");
            track = track.child(
                div()
                    .id(SharedString::from(selector.clone()))
                    .debug_selector(move || selector)
                    .absolute()
                    .top(px(7.))
                    .bottom(px(7.))
                    .left(relative(span.fraction(expires_at)))
                    .ml(px(-2.5))
                    .w(px(5.))
                    .rounded_sm()
                    .border_1()
                    .border_color(colors.editor_background)
                    .bg(cx.theme().status().warning)
                    .tooltip(Tooltip::element(move |_, _| {
                        v_flex()
                            .gap_0p5()
                            .children(lines.iter().enumerate().map(|(index, line)| {
                                Label::new(line.clone()).size(LabelSize::Small).color(
                                    if index == 0 {
                                        Color::Default
                                    } else {
                                        Color::Muted
                                    },
                                )
                            }))
                            .into_any_element()
                    })),
            );
        }
        let entry = lane.entry;
        let detail = entry
            .plan
            .iter()
            .cloned()
            .chain(std::iter::once(window.label.clone()))
            .collect::<Vec<_>>()
            .join(" · ");
        let group = SharedString::from(key.clone());
        let agent = lane.agent.clone();
        let on_open = self.on_open.clone();
        let selector = key.clone();
        h_flex()
            .id(SharedString::from(key))
            .debug_selector(move || selector)
            .group(group.clone())
            .border_t_1()
            .border_color(colors.border_variant)
            .cursor_pointer()
            .hover(|lane| lane.bg(colors.text.opacity(0.025)))
            .child(
                v_flex()
                    .w(HEAD_WIDTH)
                    .flex_none()
                    .min_w_0()
                    .px_3()
                    .gap_0p5()
                    .justify_center()
                    .child(
                        h_flex()
                            .min_w_0()
                            .gap_1p5()
                            .child(render_entry_avatar(entry, AVATAR_SIZE, cx))
                            .child(
                                div().flex_1().min_w_0().child(
                                    Label::new(entry.name.clone())
                                        .size(LabelSize::Small)
                                        .truncate(),
                                ),
                            )
                            .child(
                                div().flex_none().visible_on_hover(group).child(
                                    Icon::new(IconName::ChevronRight)
                                        .size(IconSize::XSmall)
                                        .color(Color::Muted),
                                ),
                            ),
                    )
                    .child(
                        div().pl(AVATAR_SIZE + px(6.)).min_w_0().child(
                            Label::new(detail)
                                .size(LabelSize::XSmall)
                                .color(Color::Placeholder)
                                .truncate(),
                        ),
                    ),
            )
            .child(track)
            .on_click(move |_, window, cx| on_open(&agent, window, cx))
            .into_any_element()
    }
}

/// The account's limit resets, if the first expires within the view.
fn tick_at(entry: &AccountEntry, span: &Span) -> Option<(LimitResets, SystemTime)> {
    let resets = entry.limit_resets.filter(|resets| resets.available > 0)?;
    let expires_at = resets.next_expires_at.filter(|at| span.contains(*at))?;
    Some((resets, expires_at))
}

fn grid_line(cell: &AxisCell, colors: &theme::ThemeColors) -> Hsla {
    if cell.minor {
        colors.border_variant.opacity(0.45)
    } else {
        colors.border_variant
    }
}

/// The agent's row over its lanes.
fn render_group(agent: &TimelineAgent, cx: &App) -> AnyElement {
    let colors = cx.theme().colors();
    let icon = match agent.icon.clone() {
        Some(markup) => Icon::from_svg_markup(markup),
        None => Icon::new(IconName::Sparkle),
    };
    h_flex()
        .px_3()
        .pt(px(7.))
        .pb(px(5.))
        .gap_1p5()
        .border_t_1()
        .border_color(colors.border_variant)
        .bg(colors.text.opacity(0.02))
        .child(icon.size(IconSize::XSmall).color(Color::Muted))
        .child(
            Label::new(agent.name.clone())
                .size(LabelSize::Small)
                .color(Color::Muted),
        )
        .into_any_element()
}

/// CLIProxyAPI's legend: a swatch for each kind of mark, and a sentence on what a bar is.
fn render_legend(zoom: TimelineZoom, has_tick: bool, cx: &App) -> AnyElement {
    let colors = cx.theme().colors();
    let accent = colors.text_accent;
    let swatch = |swatch: Div, text: &'static str| {
        h_flex()
            .gap(px(5.))
            .child(swatch.w(px(16.)).h(px(9.)).rounded_full())
            .child(Label::new(text).size(LabelSize::XSmall).color(Color::Muted))
    };
    let note = match zoom {
        TimelineZoom::Weekly => {
            "Each bar is one whole window, from when it opened to when it resets. Accounts whose \
             bars end together come back together."
        }
        TimelineZoom::FiveHour => {
            "Each bar is one 5-hour window. One opens with the first message after a reset, so \
             those to come are the earliest they could be."
        }
    };
    v_flex()
        .debug_selector(|| "usage-timeline-legend".into())
        .px_3p5()
        .pt(px(9.))
        .pb(px(11.))
        .gap_1p5()
        .border_t_1()
        .border_color(colors.border_variant)
        .child(
            h_flex()
                .flex_wrap()
                .gap_x_3p5()
                .gap_y_1p5()
                .child(swatch(
                    div()
                        .bg(accent.opacity(0.3))
                        .border_1()
                        .border_color(accent.opacity(0.55)),
                    "current window, filled with what’s used",
                ))
                .child(swatch(
                    div()
                        .border_1()
                        .border_dashed()
                        .border_color(accent.opacity(0.45)),
                    "windows to come",
                ))
                .when(has_tick, |legend| {
                    legend.child(
                        h_flex()
                            .gap(px(5.))
                            .child(
                                div()
                                    .w(px(3.))
                                    .h(px(10.))
                                    .rounded_sm()
                                    .bg(cx.theme().status().warning),
                            )
                            .child(
                                Label::new("a limit reset expires")
                                    .size(LabelSize::XSmall)
                                    .color(Color::Muted),
                            ),
                    )
                }),
        )
        .child(
            Label::new(note)
                .size(LabelSize::XSmall)
                .color(Color::Placeholder),
        )
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn window(label: &str, used_percent: f64, resets_in: Option<u32>, length: u32) -> LimitWindow {
        LimitWindow {
            label: label.into(),
            used_percent,
            resets_at: resets_in.map(|hours| SystemTime::now() + HOUR * hours),
            length: Some(HOUR * length),
        }
    }

    /// A lane draws the longest window that fits two weeks, or the one five hours long,
    /// whatever its agent calls it.
    #[test]
    fn lanes_draw_the_longest_window_that_fits() {
        let windows = vec![
            window("5-hour", 3., Some(4), 5),
            window("Weekly", 97., Some(24), 168),
            window("Monthly", 49., Some(567), 720),
        ];
        let label = |zoom| lane_window(&windows, zoom).map(|window| window.label.as_str());
        assert_eq!(label(TimelineZoom::Weekly), Some("Weekly"));
        assert_eq!(label(TimelineZoom::FiveHour), Some("5-hour"));

        let claude = vec![window("Session", 26., Some(2), 5)];
        assert_eq!(
            lane_window(&claude, TimelineZoom::FiveHour).map(|window| window.label.as_str()),
            Some("Session")
        );
        let daily = vec![window("Daily", 0., None, 24)];
        assert!(lane_window(&daily, TimelineZoom::FiveHour).is_none());
        let unplaced = vec![LimitWindow {
            length: None,
            ..window("Weekly", 10., Some(5), 168)
        }];
        assert!(lane_window(&unplaced, TimelineZoom::Weekly).is_none());
    }

    /// The current window runs from when it opened to its reset; those to come follow it, a
    /// length apart, to the end of the view.
    #[test]
    fn the_windows_to_come_follow_the_current_one() {
        let now = SystemTime::now();
        let span = Span::new(TimelineZoom::Weekly, now);
        let weekly = window("Weekly", 87., Some(66), 168);
        let drawn = bars(&weekly, &span);
        let resets_at = weekly.resets_at.expect("a reset");
        assert_eq!(
            drawn.first(),
            Some(&Bar {
                from: resets_at - HOUR * 168,
                to: resets_at,
                current: true
            })
        );
        assert_eq!(drawn.len(), 3);
        assert_eq!(drawn[1].from, resets_at);
        assert_eq!(drawn[2].from, resets_at + HOUR * 168);
        assert!(drawn.iter().skip(1).all(|bar| !bar.current));
        assert!(drawn.last().is_some_and(|bar| bar.from < span.end));

        assert!(bars(&window("Weekly", 0., None, 168), &span).is_empty());
    }

    #[test]
    fn the_axis_names_days_and_hours() {
        let today = NaiveDate::from_ymd_opt(2025, 10, 9).expect("a date");
        let weekly = axis_cells(TimelineZoom::Weekly, today);
        assert_eq!(weekly.len(), 14);
        assert_eq!(weekly[0].weekday.as_deref(), Some("Thu"));
        assert_eq!(weekly[0].label, "Oct 9");
        assert!(weekly[0].today);
        assert_eq!(weekly[1].label, "10");
        assert!(weekly[2].weekend && weekly[3].weekend && !weekly[4].weekend);

        let month_end = NaiveDate::from_ymd_opt(2025, 10, 25).expect("a date");
        let labels: Vec<String> = axis_cells(TimelineZoom::Weekly, month_end)
            .into_iter()
            .map(|cell| cell.label)
            .collect();
        assert_eq!(labels[6..8], ["31".to_string(), "Nov 1".to_string()]);

        let five_hour = axis_cells(TimelineZoom::FiveHour, today);
        assert_eq!(five_hour.len(), 12);
        let labels: Vec<&str> = five_hour[..5]
            .iter()
            .map(|cell| cell.label.as_str())
            .collect();
        assert_eq!(labels, ["Oct 9", "06:00", "12:00", "18:00", "Oct 10"]);
        assert!(five_hour[1].minor && five_hour[1].weekday.is_none());
    }

    #[test]
    fn the_current_window_counts_down() {
        let now = SystemTime::now();
        let window = LimitWindow {
            resets_at: Some(now + HOUR * 66 + Duration::from_secs(30)),
            ..window("Weekly", 87., None, 168)
        };
        assert_eq!(current_label(&window, now), "13% left · resets in 2d 18h");
        let used_up = LimitWindow {
            used_percent: 100.,
            ..window.clone()
        };
        assert_eq!(current_label(&used_up, now), "Used up · resets in 2d 18h");
        let past = LimitWindow {
            resets_at: Some(now - HOUR),
            ..window
        };
        assert_eq!(current_label(&past, now), "Resetting");
    }
}
