use std::time::{Duration, SystemTime};

use gpui::{AnyElement, Context, Entity, EventEmitter, MouseButton, Subscription, Task, Window};
use projects::{Project, ProjectId, ProjectScope, ProjectStore, Thread, ThreadId};
use registry::{AgentId, AgentRegistryStore};
use ui::{ContextMenu, Indicator, Tooltip, prelude::*, right_click_menu};

use crate::{NewThread, OpenFolder};

const ROW_HEIGHT: Pixels = px(28.);
/// How often relative activity times ("5m") are re-rendered.
const ACTIVITY_REFRESH_INTERVAL: Duration = Duration::from_secs(30);
pub const SIDEBAR_WIDTH: Pixels = px(290.);

pub enum SidebarEvent {
    NewThread(ProjectId),
    OpenThread(ThreadId),
}

pub struct Sidebar {
    store: Entity<ProjectStore>,
    registry: Entity<AgentRegistryStore>,
    active_thread: Option<ThreadId>,
    _subscriptions: Vec<Subscription>,
    _activity_refresh: Task<()>,
}

impl EventEmitter<SidebarEvent> for Sidebar {}

impl Sidebar {
    pub fn new(
        store: Entity<ProjectStore>,
        registry: Entity<AgentRegistryStore>,
        cx: &mut Context<Self>,
    ) -> Self {
        let subscriptions = vec![
            cx.observe(&store, |_, _, cx| cx.notify()),
            cx.observe(&registry, |_, _, cx| cx.notify()),
        ];
        let activity_refresh = cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(ACTIVITY_REFRESH_INTERVAL)
                    .await;
                if this.update(cx, |_, cx| cx.notify()).is_err() {
                    break;
                }
            }
        });
        Self {
            store,
            registry,
            active_thread: None,
            _subscriptions: subscriptions,
            _activity_refresh: activity_refresh,
        }
    }

    pub fn set_active_thread(&mut self, thread_id: Option<ThreadId>, cx: &mut Context<Self>) {
        self.active_thread = thread_id;
        cx.notify();
    }

    fn render_header(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let is_all_scope = self.store.read(cx).scope() == ProjectScope::All;
        h_flex()
            .h(px(40.))
            .flex_none()
            .px_3()
            .justify_between()
            .child(
                Label::new(if is_all_scope { "Projects" } else { "Threads" }).color(Color::Muted),
            )
            .map(|header| {
                if is_all_scope {
                    header.child(
                        IconButton::new("sidebar-open-folder", IconName::Plus)
                            .icon_size(IconSize::Small)
                            .tooltip(|_, cx| Tooltip::for_action("Open Folder…", &OpenFolder, cx))
                            .on_click(|_, window, cx| {
                                window.dispatch_action(Box::new(OpenFolder), cx)
                            }),
                    )
                } else {
                    header.child(
                        IconButton::new("sidebar-new-thread", IconName::Plus)
                            .icon_size(IconSize::Small)
                            .tooltip(|_, cx| Tooltip::for_action("New Thread", &NewThread, cx))
                            .on_click(|_, window, cx| {
                                window.dispatch_action(Box::new(NewThread), cx)
                            }),
                    )
                }
            })
    }

    fn render_project(
        &self,
        project: &Project,
        show_header: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let store = self.store.read(cx);
        let threads: Vec<Thread> = store.threads_for(project.id).cloned().collect();
        let is_collapsed = show_header && store.is_collapsed(project.id);

        v_flex()
            .w_full()
            .when(show_header, |group| {
                group.child(self.render_project_header(project, is_collapsed, cx))
            })
            .when(!is_collapsed, |group| {
                let indent = if show_header { px(22.) } else { px(0.) };
                if threads.is_empty() {
                    group.child(
                        h_flex().h(ROW_HEIGHT).pl(indent + px(14.)).child(
                            Label::new("No threads yet")
                                .size(LabelSize::Small)
                                .color(Color::Placeholder),
                        ),
                    )
                } else {
                    let mut rows = Vec::with_capacity(threads.len());
                    for thread in threads {
                        rows.push(self.render_thread(thread, indent, cx));
                    }
                    group.children(rows)
                }
            })
            .into_any_element()
    }

    fn render_project_header(
        &self,
        project: &Project,
        is_collapsed: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let project_id = project.id;
        let name = project.name();
        let store = self.store.clone();
        let sidebar = cx.entity().downgrade();
        let hover_background = cx.theme().colors().ghost_element_hover;
        let menu_open_background = cx.theme().colors().ghost_element_selected;
        let group_name = SharedString::from(format!("project-header-{}", project_id.0));

        right_click_menu(("project-menu", project_id.0))
            .trigger(move |is_menu_open, _, _| {
                h_flex()
                    .id(("project-header", project_id.0))
                    .group(group_name.clone())
                    .h(ROW_HEIGHT)
                    .w_full()
                    .px_2()
                    .gap_1p5()
                    .cursor_pointer()
                    .when(is_menu_open, |row| row.bg(menu_open_background))
                    .hover(|row| row.bg(hover_background))
                    .child(
                        Icon::new(if is_collapsed {
                            IconName::ChevronRight
                        } else {
                            IconName::ChevronDown
                        })
                        .size(IconSize::Small)
                        .color(Color::Muted),
                    )
                    .child(
                        Icon::new(IconName::Folder)
                            .size(IconSize::Small)
                            .color(Color::Muted),
                    )
                    .child(div().flex_1().min_w_0().child(Label::new(name).truncate()))
                    .child(
                        // Stops the press from also toggling the group.
                        div()
                            .visible_on_hover(group_name)
                            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                            .child(
                                IconButton::new(
                                    ("project-new-thread", project_id.0),
                                    IconName::Plus,
                                )
                                .icon_size(IconSize::Small)
                                .tooltip(Tooltip::text("New Thread"))
                                .on_click(move |_, _, cx| {
                                    cx.stop_propagation();
                                    sidebar
                                        .update(cx, |_, cx| {
                                            cx.emit(SidebarEvent::NewThread(project_id))
                                        })
                                        .ok();
                                }),
                            ),
                    )
                    .on_click({
                        let store = store.clone();
                        move |_, _, cx| {
                            store.update(cx, |store, cx| store.toggle_collapsed(project_id, cx))
                        }
                    })
            })
            .menu({
                let store = self.store.clone();
                move |window, cx| {
                    let is_all_scope = store.read(cx).scope() == ProjectScope::All;
                    let store = store.clone();
                    ContextMenu::build(window, cx, move |menu, _, _| {
                        let scope_store = store.clone();
                        let remove_store = store.clone();
                        menu.entry(
                            if is_all_scope {
                                "Show Only This Project"
                            } else {
                                "Show All Projects"
                            },
                            None,
                            move |_, cx| {
                                let scope = if is_all_scope {
                                    ProjectScope::Project(project_id)
                                } else {
                                    ProjectScope::All
                                };
                                scope_store.update(cx, |store, cx| store.set_scope(scope, cx));
                            },
                        )
                        .separator()
                        .entry("Remove From List", None, move |_, cx| {
                            remove_store
                                .update(cx, |store, cx| store.remove_project(project_id, cx));
                        })
                    })
                }
            })
    }

    fn render_thread(&self, thread: Thread, indent: Pixels, cx: &mut Context<Self>) -> AnyElement {
        let hover_background = cx.theme().colors().ghost_element_hover;
        let active_background = cx.theme().colors().ghost_element_selected;
        let is_active = self.active_thread == Some(thread.id);
        let thread_id = thread.id;
        let icon = thread
            .agent_id
            .as_ref()
            .and_then(|agent_id| {
                self.registry
                    .read(cx)
                    .agent(&AgentId::new(agent_id.clone()))?
                    .icon_path()
                    .cloned()
            })
            .map(Icon::from_external_svg)
            .unwrap_or_else(|| Icon::new(IconName::Terminal));
        let is_working = self.store.read(cx).is_thread_working(thread.id);
        let activity = (!is_working)
            .then(|| thread.last_activity_at)
            .flatten()
            .map(|time| format_relative_time(time, SystemTime::now()));
        h_flex()
            .id(("thread", thread.id.0))
            .h(ROW_HEIGHT)
            .w_full()
            .pl(indent + px(8.))
            .pr_2()
            .gap_1p5()
            .cursor_pointer()
            .when(is_active, |row| row.bg(active_background))
            .hover(|row| row.bg(hover_background))
            .on_click(cx.listener(move |_, _, _, cx| cx.emit(SidebarEvent::OpenThread(thread_id))))
            .child(icon.size(IconSize::Small).color(Color::Muted))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .child(Label::new(thread.title).truncate()),
            )
            .when(is_working, |row| {
                row.child(
                    h_flex()
                        .gap_1()
                        .child(Indicator::dot().color(Color::Accent))
                        .child(
                            Label::new("working")
                                .size(LabelSize::Small)
                                .color(Color::Accent),
                        ),
                )
            })
            .when_some(activity, |row, activity| {
                row.child(
                    Label::new(activity)
                        .size(LabelSize::Small)
                        .color(Color::Muted),
                )
            })
            .into_any_element()
    }

    fn render_empty_state(&self) -> impl IntoElement {
        v_flex()
            .flex_1()
            .items_center()
            .justify_center()
            .gap_2()
            .p_4()
            .child(Label::new("No projects yet").color(Color::Muted))
            .child(
                Button::new("empty-open-folder", "Open Folder…")
                    .style(ButtonStyle::Outlined)
                    .on_click(|_, window, cx| window.dispatch_action(Box::new(OpenFolder), cx)),
            )
    }
}

impl Render for Sidebar {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let border = cx.theme().colors().border;
        let border_variant = cx.theme().colors().border_variant;
        let panel_background = cx.theme().colors().panel_background;
        let store = self.store.read(cx);
        let show_headers = store.scope() == ProjectScope::All;
        let projects: Vec<Project> = store.visible_projects().cloned().collect();

        v_flex()
            .w(SIDEBAR_WIDTH)
            .h_full()
            .flex_none()
            .border_r_1()
            .border_color(border)
            .bg(panel_background)
            .child(self.render_header(cx))
            .child(if projects.is_empty() {
                self.render_empty_state().into_any_element()
            } else {
                let mut groups = Vec::with_capacity(projects.len());
                for (index, project) in projects.iter().enumerate() {
                    groups.push(
                        div()
                            .when(show_headers && index > 0, |group| {
                                group.border_t_1().border_color(border_variant)
                            })
                            .child(self.render_project(project, show_headers, cx)),
                    );
                }
                v_flex()
                    .id("sidebar-projects")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .pb_2()
                    .children(groups)
                    .into_any_element()
            })
    }
}

/// A short "time ago" label: `now`, `5m`, `3h`, `2d`, `1w`.
fn format_relative_time(time: SystemTime, now: SystemTime) -> String {
    let seconds = now.duration_since(time).unwrap_or_default().as_secs();
    const MINUTE: u64 = 60;
    const HOUR: u64 = 60 * MINUTE;
    const DAY: u64 = 24 * HOUR;
    const WEEK: u64 = 7 * DAY;
    match seconds {
        seconds if seconds < MINUTE => "now".to_string(),
        seconds if seconds < HOUR => format!("{}m", seconds / MINUTE),
        seconds if seconds < DAY => format!("{}h", seconds / HOUR),
        seconds if seconds < WEEK => format!("{}d", seconds / DAY),
        seconds => format!("{}w", seconds / WEEK),
    }
}

#[cfg(test)]
mod tests {
    use super::format_relative_time;
    use std::time::{Duration, SystemTime};

    #[test]
    fn relative_times() {
        let now = SystemTime::now();
        let ago = |seconds| now - Duration::from_secs(seconds);
        assert_eq!(format_relative_time(ago(5), now), "now");
        assert_eq!(format_relative_time(ago(5 * 60), now), "5m");
        assert_eq!(format_relative_time(ago(3 * 3600), now), "3h");
        assert_eq!(format_relative_time(ago(2 * 86400), now), "2d");
        assert_eq!(format_relative_time(ago(15 * 86400), now), "2w");
        assert_eq!(
            format_relative_time(now + Duration::from_secs(60), now),
            "now"
        );
    }
}
