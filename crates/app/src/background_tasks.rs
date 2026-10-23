//! What an agent left running, its background tasks (JetBrains AIR's, which Claude Agent
//! reports its background commands with): a row each with its time and Stop. They're in the
//! panel above the composer, and while the turn runs, in a popover from the count on the
//! Working row ("Working for 13m 58s · 1 shell", `design/subthreads-3/`, topic 5 C).

use agentz_protocol::thread::BackgroundTask;
use gpui::{
    AnyElement, App, DismissEvent, Entity, EventEmitter, FocusHandle, Focusable, KeyDownEvent,
    Subscription,
};
use ui::{CommonAnimationExt as _, Tooltip, prelude::*};

use crate::agent_view::format_elapsed;
use crate::thread_entity::AgentThread;

/// How many there are, on the Working row: "1 shell", "3 shells", or "2 background tasks"
/// once any isn't a shell.
pub(crate) fn background_tasks_count(tasks: &[BackgroundTask]) -> String {
    let all_shells = tasks.iter().all(|task| task.kind.as_ref() == "shell");
    match (tasks.len(), all_shells) {
        (1, true) => "1 shell".to_string(),
        (count, true) => format!("{count} shells"),
        (1, false) => "1 background task".to_string(),
        (count, false) => format!("{count} background tasks"),
    }
}

/// A row each: a spinner (or pause), the name with its progress or output file in its tooltip,
/// "Shell · 13s", and Stop where the agent can stop it.
pub(crate) fn render_background_task_rows(
    thread: &Entity<AgentThread>,
    tasks: &[BackgroundTask],
    cx: &App,
) -> Vec<AnyElement> {
    let colors = cx.theme().colors();
    let count = tasks.len();
    tasks
        .iter()
        .enumerate()
        .map(|(index, task)| {
            let icon = if task.paused && !task.stopping {
                Icon::new(IconName::DebugPause)
                    .size(IconSize::Small)
                    .color(Color::Muted)
                    .into_any_element()
            } else {
                Icon::new(IconName::LoadCircle)
                    .size(IconSize::Small)
                    .color(if task.stopping {
                        Color::Muted
                    } else {
                        Color::Accent
                    })
                    .with_rotate_animation(2)
                    .into_any_element()
            };
            let state = if task.stopping {
                "Stopping…".to_string()
            } else if task.paused {
                "Paused".to_string()
            } else {
                format_elapsed(task.started_at.elapsed().unwrap_or_default())
            };
            let details = format!("{} · {state}", background_task_kind(&task.kind));
            let name = task.name.clone();
            let about = task
                .progress
                .clone()
                .map(|progress| progress.to_string())
                .or_else(|| {
                    task.output_file
                        .as_ref()
                        .map(|path| format!("Output in {}", path.display()))
                });
            let task_id = task.id.clone();
            let thread = thread.clone();
            h_flex()
                .w_full()
                .p_1p5()
                .gap_1p5()
                .bg(colors.editor_background)
                .when(index + 1 < count, |this| {
                    this.border_b_1().border_color(colors.border_variant)
                })
                .child(icon)
                .child(
                    div()
                        .id(("background-task-name", index))
                        .debug_selector(move || format!("background-task-name-{index}"))
                        .flex_1()
                        .min_w_0()
                        .child(Label::new(name.clone()).size(LabelSize::Small).truncate())
                        .tooltip(move |_, cx| match &about {
                            Some(about) => {
                                Tooltip::with_meta(name.clone(), None, about.clone(), cx)
                            }
                            None => Tooltip::simple(name.clone(), cx),
                        }),
                )
                .child(
                    Label::new(details)
                        .size(LabelSize::Small)
                        .color(Color::Muted),
                )
                .when(task.can_stop, |this| {
                    this.child(
                        div()
                            .debug_selector(move || format!("stop-background-task-{index}"))
                            .child(
                                IconButton::new(("stop-background-task", index), IconName::Stop)
                                    .icon_size(IconSize::Small)
                                    .icon_color(Color::Muted)
                                    .disabled(task.stopping)
                                    .tooltip(Tooltip::text("Stop Task"))
                                    .on_click(move |_, _, cx| {
                                        thread.update(cx, |thread, cx| {
                                            thread.stop_background_task(&task_id, cx)
                                        });
                                    }),
                            ),
                    )
                })
                .into_any_element()
        })
        .collect()
}

/// "Shell", "Monitor", …: the kind with its first letter in capitals.
fn background_task_kind(kind: &str) -> String {
    let mut characters = kind.chars();
    match characters.next() {
        Some(first) => first.to_uppercase().chain(characters).collect(),
        None => "Task".to_string(),
    }
}

/// What the count on the Working row opens: the tasks' rows, each with Stop.
pub(crate) struct BackgroundTasksPopover {
    thread: Entity<AgentThread>,
    focus_handle: FocusHandle,
    _subscription: Subscription,
}

impl EventEmitter<DismissEvent> for BackgroundTasksPopover {}

impl Focusable for BackgroundTasksPopover {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl BackgroundTasksPopover {
    pub(crate) fn new(thread: Entity<AgentThread>, cx: &mut Context<Self>) -> Self {
        // A task that ends leaves it, and the popover closes with the last one.
        let subscription = cx.observe(&thread, |_, thread, cx| {
            if thread.read(cx).state.background_tasks.is_empty() {
                cx.emit(DismissEvent);
            } else {
                cx.notify();
            }
        });
        Self {
            thread,
            focus_handle: cx.focus_handle(),
            _subscription: subscription,
        }
    }
}

impl Render for BackgroundTasksPopover {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let tasks = self.thread.read(cx).state.background_tasks.clone();
        v_flex()
            .debug_selector(|| "background-tasks-popover".into())
            .track_focus(&self.focus_handle)
            .on_mouse_down_out(cx.listener(|_, _, _, cx| cx.emit(DismissEvent)))
            // The composer binds Escape to Cancel, which would stop the turn.
            .on_action(cx.listener(|_, _: &menu::Cancel, _, cx| cx.emit(DismissEvent)))
            .on_key_down(cx.listener(|_, event: &KeyDownEvent, _, cx| {
                if event.keystroke.key == "escape" {
                    cx.emit(DismissEvent)
                }
            }))
            .elevation_2(cx)
            .w(px(440.))
            .overflow_hidden()
            .child(
                v_flex()
                    .id("background-tasks-popover-rows")
                    .max_h(px(320.))
                    .overflow_y_scroll()
                    .children(render_background_task_rows(&self.thread, &tasks, cx)),
            )
    }
}

#[cfg(test)]
mod tests {
    use std::time::SystemTime;

    use super::*;

    fn task(kind: &str) -> BackgroundTask {
        BackgroundTask {
            id: "task".into(),
            name: "Run the tests".into(),
            kind: kind.to_string().into(),
            tool_call_id: None,
            output_file: None,
            progress: None,
            paused: false,
            can_stop: true,
            stopping: false,
            started_at: SystemTime::now(),
        }
    }

    #[test]
    fn counts_shells_by_name() {
        assert_eq!(background_tasks_count(&[task("shell")]), "1 shell");
        assert_eq!(
            background_tasks_count(&[task("shell"), task("shell")]),
            "2 shells"
        );
        assert_eq!(
            background_tasks_count(&[task("monitor")]),
            "1 background task"
        );
        assert_eq!(
            background_tasks_count(&[task("shell"), task("workflow")]),
            "2 background tasks"
        );
    }
}
