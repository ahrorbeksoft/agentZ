//! Snoozing a thread, as t3code does: its presets and the times they show
//! (`thread-settled.ts`), the menu they're in (`SnoozeMenuButton`), and Custom…'s dialog, with
//! only its Duration mode (`CustomSnoozeDialog`).

use std::fmt::Display;
use std::time::{Duration, SystemTime};

use agentz_protocol::accounts::used_up_window;
use agentz_protocol::agents::AgentId;
use anyhow::{Context as _, Result};
use chrono::{DateTime, Datelike as _, Days, Local, NaiveDate, TimeDelta, TimeZone};
use gpui::{
    App, Context, DismissEvent, Entity, EventEmitter, FocusHandle, Focusable, KeyBinding,
    Subscription,
};
use projects::Thread;
use text_input::{TextInput, TextInputEvent};
use ui::{ContextMenu, DropdownMenu, IconPosition, prelude::*};
use util::ResultExt as _;

use crate::machines::{Machines, ThreadKey};
use crate::server_client::ServerClient;

const KEY_CONTEXT: &str = "CustomSnoozeModal";
const MINUTE: Duration = Duration::from_secs(60);
const HOUR: Duration = Duration::from_secs(60 * 60);
const DAY: Duration = Duration::from_secs(24 * 60 * 60);
const EVENING_HOUR: u32 = 18;
const MORNING_HOUR: u32 = 9;
/// agentZ's 24-hour clock, unpadded as the design writes it: "9:00", "18:00".
const TIME_FORMAT: &str = "%-H:%M";
const WEEKDAY_TIME_FORMAT: &str = "%a %-H:%M";

pub fn init(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("enter", menu::Confirm, Some(KEY_CONTEXT)),
        KeyBinding::new("escape", menu::Cancel, Some(KEY_CONTEXT)),
    ]);
}

/// One of the snooze menu's choices.
#[derive(Clone, Debug, PartialEq)]
pub struct SnoozePreset {
    /// Names its row, for tests.
    pub id: &'static str,
    pub label: &'static str,
    /// The time column, which completes the label rather than repeating it: "Tomorrow" with
    /// "9:00".
    pub when: String,
    pub until: SystemTime,
}

/// t3code's presets at `now`: in 1 and 3 hours, this evening while that's over an hour away,
/// tomorrow morning, and next Monday morning unless that's tomorrow.
pub fn snooze_presets(now: SystemTime) -> Vec<SnoozePreset> {
    presets_at(DateTime::<Local>::from(now))
}

fn presets_at<Tz: TimeZone>(now: DateTime<Tz>) -> Vec<SnoozePreset>
where
    Tz::Offset: Display,
{
    let timezone = now.timezone();
    let today = now.date_naive();
    let mut presets = vec![
        preset(
            "hour",
            "In 1 hour",
            now.clone() + TimeDelta::hours(1),
            TIME_FORMAT,
        ),
        preset(
            "three-hours",
            "In 3 hours",
            now.clone() + TimeDelta::hours(3),
            TIME_FORMAT,
        ),
    ];
    if let Some(evening) = at_hour(&timezone, today, EVENING_HOUR)
        && evening.clone() - now.clone() > TimeDelta::hours(1)
    {
        presets.push(preset("evening", "This evening", evening, TIME_FORMAT));
    }
    // Calendar days rather than 24 hours, which land on the wrong day across a clock change.
    let tomorrow = today
        .checked_add_days(Days::new(1))
        .and_then(|day| at_hour(&timezone, day, MORNING_HOUR));
    if let Some(tomorrow) = tomorrow.clone() {
        presets.push(preset("tomorrow", "Tomorrow", tomorrow, TIME_FORMAT));
    }
    let days_until_monday = match (8 - now.weekday().num_days_from_sunday()) % 7 {
        0 => 7,
        days => days,
    };
    if let Some(next_week) = today
        .checked_add_days(Days::new(days_until_monday.into()))
        .and_then(|day| at_hour(&timezone, day, MORNING_HOUR))
        && tomorrow.as_ref() != Some(&next_week)
    {
        presets.push(preset(
            "next-week",
            "Next week",
            next_week,
            WEEKDAY_TIME_FORMAT,
        ));
    }
    presets
}

fn preset<Tz: TimeZone>(
    id: &'static str,
    label: &'static str,
    at: DateTime<Tz>,
    format: &str,
) -> SnoozePreset
where
    Tz::Offset: Display,
{
    SnoozePreset {
        id,
        label,
        when: at.format(format).to_string(),
        until: at.into(),
    }
}

/// `hour` o'clock on `day`, or the first time after it, where the clocks skip it.
fn at_hour<Tz: TimeZone>(timezone: &Tz, day: NaiveDate, hour: u32) -> Option<DateTime<Tz>> {
    timezone
        .from_local_datetime(&day.and_hms_opt(hour, 0, 0)?)
        .earliest()
        .or_else(|| {
            timezone
                .from_local_datetime(&day.and_hms_opt(hour + 1, 0, 0)?)
                .earliest()
        })
}

/// When a snooze ends, as agentZ writes times: the clock today ("18:00"), the weekday within
/// a week ("Wed 9:00"), else the date ("Mar 3 9:00").
pub fn wake_time(at: SystemTime, now: SystemTime) -> String {
    wake_time_in(DateTime::<Local>::from(at), DateTime::<Local>::from(now))
}

fn wake_time_in<Tz: TimeZone>(at: DateTime<Tz>, now: DateTime<Tz>) -> String
where
    Tz::Offset: Display,
{
    let days_ahead = (at.date_naive() - now.date_naive()).num_days();
    let format = if days_ahead <= 0 {
        TIME_FORMAT
    } else if days_ahead < 7 {
        WEEKDAY_TIME_FORMAT
    } else {
        "%b %-d %-H:%M"
    };
    at.format(format).to_string()
}

/// t3code's countdown on a snoozed row: "45m", "3h", "2d", rounded up, so it never reads "0m"
/// while the thread is still away.
pub fn time_left(until: SystemTime, now: SystemTime) -> String {
    let remaining = until.duration_since(now).unwrap_or_default();
    if remaining.is_zero() {
        return "now".to_string();
    }
    let rounded_up = |unit: Duration| remaining.as_millis().div_ceil(unit.as_millis());
    if remaining < HOUR {
        format!("{}m", rounded_up(MINUTE).max(1))
    } else if remaining < DAY {
        format!("{}h", rounded_up(HOUR))
    } else {
        format!("{}d", rounded_up(DAY))
    }
}

/// When the thread's account's used-up limit resets, for the menu's "Until the limit resets".
pub fn limit_reset(thread: &Thread, client: &ServerClient, now: SystemTime) -> Option<SystemTime> {
    let accounts = client.accounts(&AgentId::new(thread.agent_id.clone()?));
    let status = accounts.status(thread.account)?;
    used_up_window(&status.status.windows, now)?
        .resets_at
        .filter(|resets_at| *resets_at > now)
}

/// Snoozes the thread from this app, which offers to undo it for a moment.
pub fn snooze_thread(thread: ThreadKey, until: SystemTime, cx: &mut App) -> Result<()> {
    let store = Machines::global(cx)
        .read(cx)
        .projects(thread.machine, cx)
        .context("the thread's machine is gone")?;
    store.update(cx, |store, cx| {
        store.snooze_thread(thread.thread, until, cx)
    })
}

/// t3code's snooze menu: "Until the limit resets" first while the thread's account has a limit
/// used up, then the presets, each with its time on the right, then Custom….
pub fn snooze_menu(
    mut menu: ContextMenu,
    thread: ThreadKey,
    limit_reset: Option<SystemTime>,
    on_custom: impl Fn(&mut Window, &mut App) + 'static,
) -> ContextMenu {
    // At the time the menu opens, so "In 1 hour" is from then.
    let now = SystemTime::now();
    if let Some(resets_at) = limit_reset {
        menu = add_preset(
            menu,
            thread,
            SnoozePreset {
                id: "limit-reset",
                label: "Until the limit resets",
                when: wake_time(resets_at, now),
                until: resets_at,
            },
        )
        .separator();
    }
    for preset in snooze_presets(now) {
        menu = add_preset(menu, thread, preset);
    }
    menu.separator().custom_entry(
        |_, _| {
            div()
                .debug_selector(|| "snooze-custom".into())
                .child(Label::new("Custom…"))
                .into_any_element()
        },
        on_custom,
    )
}

fn add_preset(menu: ContextMenu, thread: ThreadKey, preset: SnoozePreset) -> ContextMenu {
    let SnoozePreset {
        id,
        label,
        when,
        until,
    } = preset;
    menu.custom_entry(
        move |_, _| {
            h_flex()
                .debug_selector(move || format!("snooze-{id}"))
                .w_full()
                .gap_4()
                .justify_between()
                .child(Label::new(label))
                .child(Label::new(when.clone()).color(Color::Muted))
                .into_any_element()
        },
        move |_, cx| {
            snooze_thread(thread, until, cx).log_err();
        },
    )
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum SnoozeUnit {
    Minutes,
    Hours,
    Days,
}

impl SnoozeUnit {
    const ALL: [Self; 3] = [Self::Minutes, Self::Hours, Self::Days];

    fn label(self) -> &'static str {
        match self {
            Self::Minutes => "minutes",
            Self::Hours => "hours",
            Self::Days => "days",
        }
    }

    fn length(self) -> Duration {
        match self {
            Self::Minutes => MINUTE,
            Self::Hours => HOUR,
            Self::Days => DAY,
        }
    }
}

/// How long "Snooze for `amount` `unit`" is, when the amount is a positive number.
fn snooze_duration(amount: &str, unit: SnoozeUnit) -> Option<Duration> {
    let amount: f64 = amount.trim().parse().ok()?;
    if !amount.is_finite() || amount <= 0. {
        return None;
    }
    Duration::try_from_secs_f64(amount * unit.length().as_secs_f64())
        .ok()
        .filter(|duration| !duration.is_zero())
}

/// Custom… from the snooze menu: t3code's dialog in its Duration mode, "Snooze for" an amount
/// of minutes, hours or days.
pub struct CustomSnoozeModal {
    thread: ThreadKey,
    amount_input: Entity<TextInput>,
    unit: SnoozeUnit,
    error: Option<SharedString>,
    _subscription: Subscription,
}

impl EventEmitter<DismissEvent> for CustomSnoozeModal {}

impl Focusable for CustomSnoozeModal {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.amount_input.focus_handle(cx)
    }
}

impl CustomSnoozeModal {
    pub fn new(thread: ThreadKey, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let amount_input = cx.new(|cx| TextInput::new("2", cx));
        // t3code starts at 2 hours, selected so typing replaces it.
        amount_input.update(cx, |input, cx| {
            input.set_text("2", cx);
            input.select_all_text(cx);
        });
        let subscription = cx.subscribe(&amount_input, |this, _, _: &TextInputEvent, cx| {
            this.error = None;
            cx.notify();
        });
        window.focus(&amount_input.focus_handle(cx), cx);
        Self {
            thread,
            amount_input,
            unit: SnoozeUnit::Hours,
            error: None,
            _subscription: subscription,
        }
    }

    fn duration(&self, cx: &App) -> Option<Duration> {
        snooze_duration(self.amount_input.read(cx).text(), self.unit)
    }

    fn set_unit(&mut self, unit: SnoozeUnit, cx: &mut Context<Self>) {
        self.unit = unit;
        self.error = None;
        cx.notify();
    }

    fn confirm(&mut self, _: &menu::Confirm, _: &mut Window, cx: &mut Context<Self>) {
        let Some(duration) = self.duration(cx) else {
            self.error = Some("Enter a positive duration.".into());
            cx.notify();
            return;
        };
        match snooze_thread(self.thread, SystemTime::now() + duration, cx) {
            Ok(()) => cx.emit(DismissEvent),
            Err(error) => {
                self.error = Some(format!("Couldn't snooze it: {error:#}").into());
                cx.notify();
            }
        }
    }

    fn cancel(&mut self, _: &menu::Cancel, _: &mut Window, cx: &mut Context<Self>) {
        cx.emit(DismissEvent);
    }
}

impl Render for CustomSnoozeModal {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = cx.theme().colors();
        let border = colors.border;
        let border_variant = colors.border_variant;
        let editor_background = colors.editor_background;
        let now = SystemTime::now();
        let hint = match (&self.error, self.duration(cx)) {
            (Some(error), _) => Some(
                Label::new(error.clone())
                    .size(LabelSize::Small)
                    .color(Color::Error),
            ),
            (None, Some(duration)) => Some(
                Label::new(format!("Until {}", wake_time(now + duration, now)))
                    .size(LabelSize::Small)
                    .color(Color::Muted),
            ),
            (None, None) => None,
        };
        let modal = cx.weak_entity();
        let current = self.unit;
        let unit_menu = ContextMenu::build(window, cx, move |mut menu, _, _| {
            for unit in SnoozeUnit::ALL {
                let modal = modal.clone();
                menu = menu.toggleable_entry(
                    unit.label(),
                    unit == current,
                    IconPosition::End,
                    None,
                    move |_, cx| {
                        modal
                            .update(cx, |modal, cx| modal.set_unit(unit, cx))
                            .log_err();
                    },
                );
            }
            menu
        });
        v_flex()
            .key_context(KEY_CONTEXT)
            .w(rems(22.))
            .elevation_3(cx)
            .overflow_hidden()
            .on_action(cx.listener(Self::confirm))
            .on_action(cx.listener(Self::cancel))
            .child(
                v_flex()
                    .px_4()
                    .pt_3()
                    .pb_2()
                    .gap_3()
                    .child(
                        v_flex().gap_1().child(Label::new("Custom snooze")).child(
                            Label::new("Choose when the thread comes back to the list.")
                                .size(LabelSize::Small)
                                .color(Color::Muted),
                        ),
                    )
                    .child(
                        v_flex()
                            .gap_1p5()
                            .child(
                                h_flex()
                                    .gap_2()
                                    .child(
                                        Label::new("Snooze for")
                                            .size(LabelSize::Small)
                                            .color(Color::Muted),
                                    )
                                    .child(
                                        div()
                                            .debug_selector(|| "custom-snooze-amount".into())
                                            .w(px(60.))
                                            .h(px(28.))
                                            .px_2()
                                            .flex()
                                            .items_center()
                                            .overflow_hidden()
                                            .rounded_md()
                                            .border_1()
                                            .border_color(border)
                                            .bg(editor_background)
                                            .child(self.amount_input.clone()),
                                    )
                                    .child(
                                        div().debug_selector(|| "custom-snooze-unit".into()).child(
                                            DropdownMenu::new(
                                                "custom-snooze-unit",
                                                self.unit.label(),
                                                unit_menu,
                                            ),
                                        ),
                                    ),
                            )
                            .children(hint),
                    ),
            )
            .child(
                h_flex()
                    .mt_2()
                    .px_4()
                    .py_2()
                    .gap_2()
                    .justify_end()
                    .border_t_1()
                    .border_color(border_variant)
                    .child(
                        Button::new("custom-snooze-cancel", "Cancel")
                            .style(ButtonStyle::Subtle)
                            .on_click(cx.listener(|_, _, _, cx| cx.emit(DismissEvent))),
                    )
                    .child(
                        Button::new("custom-snooze-confirm", "Snooze")
                            .style(ButtonStyle::Filled)
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.confirm(&menu::Confirm, window, cx)
                            })),
                    ),
            )
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::rc::Rc;

    use agentz_protocol::Request;
    use agentz_protocol::spaces::SpacesSnapshot;
    use chrono::Utc;
    use gpui::TestAppContext;
    use projects::{Project, ProjectId, ProjectsSnapshot, ThreadId};
    use serde_json::json;

    use super::*;
    use crate::machines::MachineId;

    fn at(day: u32, hour: u32, minute: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 4, day, hour, minute, 0)
            .single()
            .expect("a time")
    }

    fn ids(presets: &[SnoozePreset]) -> Vec<&'static str> {
        presets.iter().map(|preset| preset.id).collect()
    }

    #[test]
    fn presets_are_t3codes_with_their_times() {
        // Wednesday, April 8th.
        let presets = presets_at(at(8, 10, 0));
        assert_eq!(
            ids(&presets),
            ["hour", "three-hours", "evening", "tomorrow", "next-week"]
        );
        let whens: Vec<&str> = presets.iter().map(|preset| preset.when.as_str()).collect();
        assert_eq!(whens, ["11:00", "13:00", "18:00", "9:00", "Mon 9:00"]);
        assert_eq!(presets[3].until, SystemTime::from(at(9, 9, 0)));
        assert_eq!(presets[4].until, SystemTime::from(at(13, 9, 0)));

        // Evening only while it's over an hour away.
        assert_eq!(
            ids(&presets_at(at(8, 17, 30))),
            ["hour", "three-hours", "tomorrow", "next-week"]
        );
        assert_eq!(
            ids(&presets_at(at(8, 21, 0))),
            ["hour", "three-hours", "tomorrow", "next-week"]
        );
        // On a Sunday, next week is tomorrow.
        assert_eq!(
            ids(&presets_at(at(12, 10, 0))),
            ["hour", "three-hours", "evening", "tomorrow"]
        );
        // On a Monday, it's a week away.
        let monday = presets_at(at(6, 10, 0));
        assert_eq!(monday[4].until, SystemTime::from(at(13, 9, 0)));
    }

    #[test]
    fn wake_times_and_time_left_read_as_the_design_writes_them() {
        let now = at(8, 10, 0);
        assert_eq!(wake_time_in(at(8, 18, 0), now), "18:00");
        assert_eq!(wake_time_in(at(9, 9, 0), now), "Thu 9:00");
        assert_eq!(wake_time_in(at(14, 9, 5), now), "Tue 9:05");
        assert_eq!(wake_time_in(at(15, 9, 0), now), "Apr 15 9:00");

        let now = SystemTime::UNIX_EPOCH + DAY;
        let left = |seconds: u64| time_left(now + Duration::from_secs(seconds), now);
        assert_eq!(left(30), "1m");
        assert_eq!(left(59 * 60), "59m");
        assert_eq!(left(61 * 60), "2h");
        assert_eq!(left(25 * 60 * 60), "2d");
        assert_eq!(time_left(now - HOUR, now), "now");
    }

    #[test]
    fn a_duration_is_a_positive_amount() {
        assert_eq!(snooze_duration("2", SnoozeUnit::Hours), Some(2 * HOUR));
        assert_eq!(
            snooze_duration(" 1.5 ", SnoozeUnit::Hours),
            Some(90 * MINUTE)
        );
        assert_eq!(snooze_duration("3", SnoozeUnit::Days), Some(3 * DAY));
        for amount in ["", "0", "-1", "soon", "inf", "NaN"] {
            assert_eq!(
                snooze_duration(amount, SnoozeUnit::Minutes),
                None,
                "{amount}"
            );
        }
    }

    #[gpui::test]
    fn a_custom_duration_snoozes_the_thread_until_then(cx: &mut TestAppContext) {
        let client = cx.update(|cx| {
            crate::init_for_test(cx);
            init(cx);
            let client = ServerClient::new_for_test(
                MachineId::Local,
                "This Mac".into(),
                SpacesSnapshot::default(),
                cx,
            );
            crate::machines::init_for_test(vec![client.clone()], cx);
            client
        });
        let store = cx.update(|cx| client.read(cx).projects().clone());
        let thread: Thread = serde_json::from_value(json!({
            "id": 1,
            "project_id": 1,
            "title": "Fix the cart",
            "agent_id": "mock",
        }))
        .expect("a thread");
        cx.update(|cx| {
            store.update(cx, |store, cx| {
                store.set_snapshot(
                    ProjectsSnapshot {
                        projects: vec![Project {
                            id: ProjectId(1),
                            path: "/tmp/demo".into(),
                            custom_name: None,
                            icon: None,
                            workspaces: Vec::new(),
                            repository: None,
                        }],
                        threads: vec![thread],
                        ..Default::default()
                    },
                    cx,
                )
            })
        });
        let key = ThreadKey {
            machine: MachineId::Local,
            thread: ThreadId(1),
        };
        let (modal, cx) =
            cx.add_window_view(move |window, cx| CustomSnoozeModal::new(key, window, cx));
        let dismissed = Rc::new(Cell::new(0));
        cx.update(|_, cx| {
            let dismissed = dismissed.clone();
            cx.subscribe(&modal, move |_, _: &DismissEvent, _| {
                dismissed.set(dismissed.get() + 1)
            })
            .detach()
        });

        // Typing replaces the 2 hours it starts with.
        cx.simulate_input("0");
        cx.simulate_keystrokes("enter");
        assert_eq!(dismissed.get(), 0);
        modal.read_with(cx, |modal, _| {
            assert_eq!(
                modal.error.as_ref().map(SharedString::as_ref),
                Some("Enter a positive duration.")
            );
        });

        modal.update(cx, |modal, cx| modal.set_unit(SnoozeUnit::Days, cx));
        cx.simulate_keystrokes("backspace");
        cx.simulate_input("3");
        let before = SystemTime::now();
        cx.simulate_keystrokes("enter");
        assert_eq!(dismissed.get(), 1);
        let until = cx.update(|_, cx| {
            store
                .read(cx)
                .thread(ThreadId(1))
                .and_then(|thread| thread.snoozed_until)
                .expect("snoozed")
        });
        let off = until
            .duration_since(before + 3 * DAY)
            .unwrap_or_else(|error| error.duration());
        assert!(off < MINUTE, "until 3 days later, {off:?} off");
        let sent = cx.update(|_, cx| client.read(cx).sent_for_test());
        assert!(
            sent.iter().any(|request| matches!(
                request,
                Request::SnoozeThread { thread_id: ThreadId(1), until: sent } if *sent == until
            )),
            "{sent:?}"
        );
    }
}
