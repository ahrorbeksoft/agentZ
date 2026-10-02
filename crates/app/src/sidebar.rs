use std::time::{Duration, SystemTime};

use gpui::{
    AnyElement, App, ClickEvent, Context, Entity, EventEmitter, Focusable as _, FontWeight,
    KeyBinding, PromptLevel, Subscription, Task, Window,
};
use projects::{ProjectStore, Thread, ThreadId, ThreadOrder};
use registry::{AgentId, AgentRegistryStore};
use text_input::{TextInput, TextInputEvent};
use ui::{
    CommonAnimationExt as _, ContextMenu, IconPosition, PopoverMenu, Tooltip, prelude::*,
    right_click_menu,
};

use crate::{NewThread, OpenFolder};

/// How often relative activity times ("5m") are re-rendered.
const ACTIVITY_REFRESH_INTERVAL: Duration = Duration::from_secs(30);
pub const SIDEBAR_WIDTH: Pixels = px(290.);
const RENAME_KEY_CONTEXT: &str = "SidebarRename";
const ARCHIVED_ROW_HEIGHT: Pixels = px(36.);
/// t3code pages its settled shelf: recent history is the common lookup, the deep tail stays
/// behind "Show more".
const ARCHIVED_INITIAL_COUNT: usize = 10;
const ARCHIVED_PAGE_COUNT: usize = 25;

pub fn init(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("enter", menu::Confirm, Some(RENAME_KEY_CONTEXT)),
        KeyBinding::new("escape", menu::Cancel, Some(RENAME_KEY_CONTEXT)),
    ]);
}

pub enum SidebarEvent {
    OpenThread(ThreadId),
}

/// The thread list, modeled on t3code's sidebar: active threads as cards and archived threads
/// in a collapsible shelf at the bottom (t3code's "Settled" shelf).
pub struct Sidebar {
    store: Entity<ProjectStore>,
    registry: Entity<AgentRegistryStore>,
    active_thread: Option<ThreadId>,
    search: Entity<TextInput>,
    archived_shown: usize,
    renaming_thread: Option<ThreadId>,
    rename_input: Entity<TextInput>,
    _rename_blur: Option<Subscription>,
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
        let search = cx.new(|cx| TextInput::new("Search threads…", cx));
        let rename_input = cx.new(|cx| TextInput::new("Thread title", cx));
        let subscriptions = vec![
            cx.subscribe(&rename_input, |this, _, _: &TextInputEvent, cx| {
                this.apply_rename(cx)
            }),
            cx.observe(&store, |_, _, cx| cx.notify()),
            cx.observe(&registry, |_, _, cx| cx.notify()),
            cx.subscribe(&search, |_, _, _: &TextInputEvent, cx| cx.notify()),
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
            search,
            archived_shown: ARCHIVED_INITIAL_COUNT,
            renaming_thread: None,
            rename_input,
            _rename_blur: None,
            _subscriptions: subscriptions,
            _activity_refresh: activity_refresh,
        }
    }

    pub fn set_active_thread(&mut self, thread_id: Option<ThreadId>, cx: &mut Context<Self>) {
        self.active_thread = thread_id;
        cx.notify();
    }

    fn start_renaming(
        &mut self,
        thread_id: ThreadId,
        title: SharedString,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self
            .renaming_thread
            .is_some_and(|renaming| renaming != thread_id)
        {
            self.finish_renaming(cx);
        }
        // Set before editing the text so the resulting change event doesn't count as a rename.
        self.renaming_thread = None;
        self.rename_input.update(cx, |input, cx| {
            input.set_text(title, cx);
            input.select_all_text(cx);
        });
        self.renaming_thread = Some(thread_id);
        let focus_handle = self.rename_input.focus_handle(cx);
        window.focus(&focus_handle, cx);
        // Clicking elsewhere ends the rename.
        self._rename_blur = Some(cx.on_blur(&focus_handle, window, |this, _, cx| {
            this.finish_renaming(cx)
        }));
        cx.notify();
    }

    /// Renames as you type; an empty title is ignored.
    fn apply_rename(&mut self, cx: &mut Context<Self>) {
        let Some(thread_id) = self.renaming_thread else {
            return;
        };
        let title = self.rename_input.read(cx).text().trim().to_string();
        let is_unchanged = self
            .store
            .read(cx)
            .thread(thread_id)
            .is_none_or(|thread| thread.title == title);
        if title.is_empty() || is_unchanged {
            return;
        }
        self.store
            .update(cx, |store, cx| store.set_custom_title(thread_id, title, cx));
    }

    fn finish_renaming(&mut self, cx: &mut Context<Self>) {
        if self.renaming_thread.take().is_some() {
            self._rename_blur = None;
            cx.notify();
        }
    }

    fn render_rename_input(&self, cx: &mut Context<Self>) -> AnyElement {
        div()
            .flex_1()
            .min_w_0()
            .key_context(RENAME_KEY_CONTEXT)
            .on_action(cx.listener(|this, _: &menu::Confirm, window, cx| {
                this.finish_renaming(cx);
                window.blur(cx);
            }))
            .on_action(cx.listener(|this, _: &menu::Cancel, window, cx| {
                this.finish_renaming(cx);
                window.blur(cx);
            }))
            .child(self.rename_input.clone())
            .into_any_element()
    }

    fn search_query(&self, cx: &App) -> String {
        self.search.read(cx).text().trim().to_lowercase()
    }

    fn render_header(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let has_query = !self.search_query(cx).is_empty();
        let store = self.store.clone();
        h_flex()
            .h(px(40.))
            .flex_none()
            .pl_3()
            .pr_2()
            .gap_1()
            .border_b_1()
            .border_color(cx.theme().colors().border)
            .child(
                Icon::new(IconName::MagnifyingGlass)
                    .size(IconSize::Small)
                    .color(Color::Muted),
            )
            .child(div().flex_1().min_w_0().child(self.search.clone()))
            .when(has_query, |this| {
                this.child(
                    IconButton::new("clear-search", IconName::Close)
                        .icon_size(IconSize::Small)
                        .tooltip(Tooltip::text("Clear Search"))
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.search.update(cx, |search, cx| search.set_text("", cx));
                        })),
                )
            })
            .child(
                PopoverMenu::new("thread-order")
                    .menu(move |window, cx| {
                        let store = store.clone();
                        let current = store.read(cx).thread_order();
                        Some(ContextMenu::build(window, cx, move |menu, _, _| {
                            let mut menu = menu.header("Sort Threads");
                            for (order, label) in [
                                (ThreadOrder::LastActivity, "Latest Activity"),
                                (ThreadOrder::Created, "Newest First"),
                            ] {
                                let store = store.clone();
                                menu = menu.toggleable_entry(
                                    label,
                                    current == order,
                                    IconPosition::End,
                                    None,
                                    move |_, cx| {
                                        store.update(cx, |store, cx| {
                                            store.set_thread_order(order, cx)
                                        })
                                    },
                                );
                            }
                            menu
                        }))
                    })
                    .trigger_with_tooltip(
                        IconButton::new("thread-order-trigger", IconName::Filter)
                            .icon_size(IconSize::Small),
                        Tooltip::text("Sort Threads"),
                    )
                    .anchor(gpui::Anchor::TopRight),
            )
            .child(
                IconButton::new("sidebar-new-thread", IconName::Plus)
                    .icon_size(IconSize::Small)
                    .tooltip(|_, cx| Tooltip::for_action("New Thread", &NewThread, cx))
                    .on_click(|_, window, cx| window.dispatch_action(Box::new(NewThread), cx)),
            )
    }

    fn agent_icon(&self, thread: &Thread, cx: &App) -> Icon {
        thread
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
            .unwrap_or_else(|| Icon::new(IconName::Terminal))
    }

    /// Clicking opens the thread; double-clicking renames it, as in t3code.
    fn thread_click_handler(
        &self,
        thread_id: ThreadId,
        title: SharedString,
        cx: &mut Context<Self>,
    ) -> impl Fn(&ClickEvent, &mut Window, &mut App) + 'static {
        cx.listener(move |this, event: &ClickEvent, window, cx| {
            if event.click_count() >= 2 {
                this.start_renaming(thread_id, title.clone(), window, cx);
            } else {
                cx.emit(SidebarEvent::OpenThread(thread_id));
            }
        })
    }

    /// t3code's thread card: the project and status on top, the title below. The agent's icon
    /// sits at the bottom right.
    fn render_thread_card(
        &self,
        thread: Thread,
        project_name: Option<SharedString>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let colors = cx.theme().colors();
        let hover_background = colors.ghost_element_hover;
        let selected_background = colors.ghost_element_selected;
        let is_active = self.active_thread == Some(thread.id);
        let is_renaming = self.renaming_thread == Some(thread.id);
        let is_working = self.store.read(cx).is_thread_working(thread.id);
        let thread_id = thread.id;
        let icon = self.agent_icon(&thread, cx);
        let time = thread
            .last_activity_at
            .map(|time| format_relative_time(time, SystemTime::now()));
        let group_name = SharedString::from(format!("thread-card-{}", thread.id.0));
        let title = SharedString::from(thread.title);

        let status = if is_working {
            h_flex()
                .gap_1()
                .child(
                    Icon::new(IconName::LoadCircle)
                        .size(IconSize::Small)
                        .color(Color::Accent)
                        .with_rotate_animation(2),
                )
                .child(
                    Label::new("Working")
                        .size(LabelSize::Small)
                        .weight(FontWeight::MEDIUM)
                        .color(Color::Accent),
                )
                .into_any_element()
        } else {
            Label::new(time.unwrap_or_default())
                .size(LabelSize::Small)
                .color(Color::Muted)
                .into_any_element()
        };
        let store = self.store.clone();
        let archive_button = Button::new(("archive-thread", thread.id.0), "Archive")
            .label_size(LabelSize::Small)
            .color(Color::Muted)
            .start_icon(
                Icon::new(IconName::Archive)
                    .size(IconSize::XSmall)
                    .color(Color::Muted),
            )
            .tooltip(Tooltip::text("Archive Thread"))
            .on_click(move |_, _, cx| {
                cx.stop_propagation();
                store.update(cx, |store, cx| store.archive_thread(thread_id, cx));
            });

        let card = v_flex()
            .id(("thread-card", thread.id.0))
            .group(group_name.clone())
            .relative()
            .w_full()
            .px_2p5()
            .py_2()
            .gap_1()
            .rounded_md()
            .when(is_active, |card| card.bg(selected_background))
            .when(!is_renaming, |card| {
                card.cursor_pointer()
                    .hover(|card| card.bg(hover_background))
                    .on_click(self.thread_click_handler(thread_id, title.clone(), cx))
            })
            .child(
                h_flex()
                    .h_5()
                    .min_w_0()
                    .gap_1p5()
                    .child(div().flex_1().min_w_0().children(project_name.map(|name| {
                        Label::new(name)
                            .size(LabelSize::Small)
                            .color(Color::Muted)
                            .truncate()
                    })))
                    .child(
                        // The status yields to the Archive button on hover.
                        div()
                            .flex_none()
                            .when(!is_renaming, |this| {
                                this.group_hover(group_name.clone(), |this| this.invisible())
                            })
                            .child(status),
                    )
                    .when(!is_renaming, |row| {
                        row.child(
                            div()
                                .absolute()
                                .top_1p5()
                                .right_1()
                                .visible_on_hover(group_name.clone())
                                .child(archive_button),
                        )
                    }),
            )
            .child(
                h_flex()
                    .min_w_0()
                    .gap_1p5()
                    .child(if is_renaming {
                        self.render_rename_input(cx)
                    } else {
                        div()
                            .flex_1()
                            .min_w_0()
                            .child(
                                Label::new(title.clone())
                                    .weight(FontWeight::MEDIUM)
                                    .truncate(),
                            )
                            .into_any_element()
                    })
                    .child(
                        div()
                            .flex_none()
                            .opacity(0.6)
                            .child(icon.size(IconSize::Small).color(Color::Muted)),
                    ),
            );

        let sidebar = cx.entity().downgrade();
        right_click_menu(("thread-menu", thread.id.0))
            .trigger(move |_, _, _| div().py_0p5().child(card))
            .menu(move |window, cx| {
                let sidebar = sidebar.clone();
                let title = title.clone();
                ContextMenu::build(window, cx, move |menu, _, _| {
                    let archive_sidebar = sidebar.clone();
                    menu.entry("Rename Title", None, move |window, cx| {
                        sidebar
                            .update(cx, |sidebar, cx| {
                                sidebar.start_renaming(thread_id, title.clone(), window, cx)
                            })
                            .ok();
                    })
                    .entry("Archive Thread", None, move |_, cx| {
                        archive_sidebar
                            .update(cx, |sidebar, cx| {
                                sidebar
                                    .store
                                    .update(cx, |store, cx| store.archive_thread(thread_id, cx))
                            })
                            .ok();
                    })
                })
            })
            .into_any_element()
    }

    /// t3code's shelf header: a label, a rule, and a chevron.
    fn render_archived_header(
        &self,
        count: usize,
        is_expanded: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let rule_color = cx.theme().colors().border_variant;
        h_flex()
            .id("archived-shelf-toggle")
            .h_8()
            .mx_0p5()
            .px_2()
            .gap_2()
            .cursor_pointer()
            .child(
                Label::new(if is_expanded {
                    "Archived".to_string()
                } else {
                    format!("Archived ({count})")
                })
                .size(LabelSize::Small)
                .weight(FontWeight::MEDIUM)
                .color(Color::Muted),
            )
            .child(div().flex_1().min_w_2().h_px().bg(rule_color))
            .child(
                Icon::new(if is_expanded {
                    IconName::ChevronUp
                } else {
                    IconName::ChevronDown
                })
                .size(IconSize::XSmall)
                .color(Color::Muted),
            )
            .on_click(cx.listener(|this, _, _, cx| {
                this.store
                    .update(cx, |store, cx| store.toggle_archived_expanded(cx));
            }))
            .into_any_element()
    }

    /// t3code's slim row for parked threads: dimmed until hovered, with a way back on hover.
    fn render_archived_row(&self, thread: Thread, cx: &mut Context<Self>) -> AnyElement {
        let colors = cx.theme().colors();
        let hover_background = colors.ghost_element_hover;
        let selected_background = colors.ghost_element_selected;
        let is_active = self.active_thread == Some(thread.id);
        let is_renaming = self.renaming_thread == Some(thread.id);
        let thread_id = thread.id;
        let icon = self.agent_icon(&thread, cx);
        let time = thread
            .archived_at
            .map(|time| format_relative_time(time, SystemTime::now()));
        let group_name = SharedString::from(format!("archived-row-{}", thread.id.0));
        let title = SharedString::from(thread.title);
        let store = self.store.clone();

        let row = h_flex()
            .id(("archived-thread", thread.id.0))
            .group(group_name.clone())
            .relative()
            .h(ARCHIVED_ROW_HEIGHT)
            .w_full()
            .px_2p5()
            .gap_2p5()
            .rounded_md()
            .when(is_active, |row| row.bg(selected_background))
            .when(!is_renaming, |row| {
                row.cursor_pointer()
                    .hover(|row| row.bg(hover_background))
                    .on_click(self.thread_click_handler(thread_id, title.clone(), cx))
            })
            .child(
                div()
                    .flex_none()
                    .when(!is_active, |this| {
                        this.opacity(0.4)
                            .group_hover(group_name.clone(), |this| this.opacity(1.))
                    })
                    .child(icon.size(IconSize::Small).color(Color::Muted)),
            )
            .child(if is_renaming {
                self.render_rename_input(cx)
            } else {
                div()
                    .flex_1()
                    .min_w_0()
                    .child(Label::new(title.clone()).color(Color::Muted).truncate())
                    .into_any_element()
            })
            .when_some(time.filter(|_| !is_renaming), |row, time| {
                row.child(
                    div()
                        .flex_none()
                        .group_hover(group_name.clone(), |this| this.invisible())
                        .child(Label::new(time).size(LabelSize::Small).color(Color::Muted)),
                )
            })
            .when(!is_renaming, |row| {
                row.child(
                    div()
                        .absolute()
                        .right_1()
                        .visible_on_hover(group_name.clone())
                        .child(
                            IconButton::new(("unarchive-thread", thread.id.0), IconName::Undo)
                                .icon_size(IconSize::Small)
                                .icon_color(Color::Muted)
                                .tooltip(Tooltip::text("Unarchive Thread"))
                                .on_click(move |_, _, cx| {
                                    cx.stop_propagation();
                                    store.update(cx, |store, cx| {
                                        store.unarchive_thread(thread_id, cx)
                                    });
                                }),
                        ),
                )
            });

        let sidebar = cx.entity().downgrade();
        right_click_menu(("archived-thread-menu", thread.id.0))
            .trigger(move |_, _, _| row)
            .menu(move |window, cx| {
                let sidebar = sidebar.clone();
                let title = title.clone();
                ContextMenu::build(window, cx, move |menu, _, _| {
                    let rename_sidebar = sidebar.clone();
                    let rename_title = title.clone();
                    let unarchive_sidebar = sidebar.clone();
                    let delete_sidebar = sidebar.clone();
                    let delete_title = title.clone();
                    menu.entry("Rename Title", None, move |window, cx| {
                        rename_sidebar
                            .update(cx, |sidebar, cx| {
                                sidebar.start_renaming(thread_id, rename_title.clone(), window, cx)
                            })
                            .ok();
                    })
                    .entry("Unarchive Thread", None, move |_, cx| {
                        unarchive_sidebar
                            .update(cx, |sidebar, cx| {
                                sidebar
                                    .store
                                    .update(cx, |store, cx| store.unarchive_thread(thread_id, cx))
                            })
                            .ok();
                    })
                    .separator()
                    .entry("Delete Thread…", None, move |window, cx| {
                        delete_sidebar
                            .update(cx, |sidebar, cx| {
                                sidebar.confirm_delete_thread(
                                    thread_id,
                                    delete_title.clone(),
                                    window,
                                    cx,
                                )
                            })
                            .ok();
                    })
                })
            })
            .into_any_element()
    }

    fn render_show_more_archived(&self, hidden_count: usize, cx: &mut Context<Self>) -> AnyElement {
        h_flex()
            .id("show-more-archived")
            .h(ARCHIVED_ROW_HEIGHT)
            .px_2p5()
            .gap_2p5()
            .rounded_md()
            .cursor_pointer()
            .hover(|row| row.bg(cx.theme().colors().ghost_element_hover))
            .child(
                Icon::new(IconName::Plus)
                    .size(IconSize::Small)
                    .color(Color::Muted),
            )
            .child(
                Label::new(format!(
                    "Show {} more",
                    hidden_count.min(ARCHIVED_PAGE_COUNT)
                ))
                .color(Color::Muted),
            )
            .on_click(cx.listener(|this, _, _, cx| {
                this.archived_shown += ARCHIVED_PAGE_COUNT;
                cx.notify();
            }))
            .into_any_element()
    }

    fn confirm_delete_thread(
        &mut self,
        thread_id: ThreadId,
        title: SharedString,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let answer = window.prompt(
            PromptLevel::Warning,
            &format!("Delete “{title}”?"),
            Some("The thread and its conversation will be removed. This can't be undone."),
            &["Delete", "Cancel"],
            cx,
        );
        let store = self.store.clone();
        cx.spawn(async move |_, cx| {
            if answer.await == Ok(0) {
                store.update(cx, |store, cx| store.delete_thread(thread_id, cx));
            }
        })
        .detach();
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

    fn render_threads(&self, cx: &mut Context<Self>) -> AnyElement {
        let query = self.search_query(cx);
        let store = self.store.read(cx);
        let active: Vec<(Thread, Option<SharedString>)> = store
            .active_threads()
            .into_iter()
            .filter(|thread| matches_query(thread, &query))
            .map(|thread| {
                let project_name = store
                    .project(thread.project_id)
                    .map(|project| project.name());
                (thread.clone(), project_name)
            })
            .collect();
        let archived: Vec<Thread> = store
            .archived_threads()
            .into_iter()
            .filter(|thread| matches_query(thread, &query))
            .cloned()
            .collect();
        // Matches are shown even while the shelf is collapsed.
        let is_archived_expanded = store.archived_expanded() || !query.is_empty();

        let mut rows = Vec::with_capacity(active.len());
        for (thread, project_name) in active {
            rows.push(self.render_thread_card(thread, project_name, cx));
        }
        if rows.is_empty() {
            rows.push(
                h_flex()
                    .h_8()
                    .px_2p5()
                    .child(
                        Label::new(if query.is_empty() {
                            "No threads yet"
                        } else {
                            "No matching threads"
                        })
                        .size(LabelSize::Small)
                        .color(Color::Placeholder),
                    )
                    .into_any_element(),
            );
        }

        let archived_count = archived.len();
        let mut shelf = Vec::new();
        if archived_count > 0 {
            shelf.push(self.render_archived_header(archived_count, is_archived_expanded, cx));
            if is_archived_expanded {
                let hidden_count = archived_count.saturating_sub(self.archived_shown);
                for thread in archived.into_iter().take(self.archived_shown) {
                    shelf.push(self.render_archived_row(thread, cx));
                }
                if hidden_count > 0 {
                    shelf.push(self.render_show_more_archived(hidden_count, cx));
                }
            }
        }

        v_flex()
            .id("sidebar-threads")
            .flex_1()
            .min_h_0()
            .px_1()
            .pt_1()
            .pb_2()
            .overflow_y_scroll()
            .children(rows)
            .when(!shelf.is_empty(), |list| {
                // Like t3code, the shelf rests at the bottom while the list is short.
                list.child(v_flex().mt_auto().pt_2().children(shelf))
            })
            .into_any_element()
    }
}

impl Render for Sidebar {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = cx.theme().colors();
        let border = colors.border;
        let panel_background = colors.panel_background;
        let has_projects = !self.store.read(cx).projects().is_empty();

        v_flex()
            .w(SIDEBAR_WIDTH)
            .h_full()
            .flex_none()
            .border_r_1()
            .border_color(border)
            .bg(panel_background)
            .child(self.render_header(cx))
            .child(if has_projects {
                self.render_threads(cx)
            } else {
                self.render_empty_state().into_any_element()
            })
    }
}

fn matches_query(thread: &Thread, query: &str) -> bool {
    query.is_empty() || thread.title.to_lowercase().contains(query)
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
