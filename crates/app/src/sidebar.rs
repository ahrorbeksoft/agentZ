use std::time::{Duration, SystemTime};

use chrono::{DateTime, Datelike as _, Local, NaiveDate, TimeDelta};
use gpui::{
    AnyElement, App, Context, Entity, EventEmitter, Focusable as _, KeyBinding, MouseButton,
    PromptLevel, Subscription, Task, Window,
};
use projects::{Project, ProjectId, ProjectScope, ProjectStore, Thread, ThreadId, ThreadOrder};
use registry::{AgentId, AgentRegistryStore};
use text_input::{TextInput, TextInputEvent};
use ui::{
    ContextMenu, IconPosition, Indicator, PopoverMenu, Tooltip, prelude::*, right_click_menu,
};

use crate::{NewThread, OpenFolder};

const ROW_HEIGHT: Pixels = px(28.);
/// How often relative activity times ("5m") are re-rendered.
const ACTIVITY_REFRESH_INTERVAL: Duration = Duration::from_secs(30);
pub const SIDEBAR_WIDTH: Pixels = px(290.);
const RENAME_KEY_CONTEXT: &str = "SidebarRename";

pub fn init(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("enter", menu::Confirm, Some(RENAME_KEY_CONTEXT)),
        KeyBinding::new("escape", menu::Cancel, Some(RENAME_KEY_CONTEXT)),
    ]);
}

pub enum SidebarEvent {
    NewThread(ProjectId),
    OpenThread(ThreadId),
}

pub struct Sidebar {
    store: Entity<ProjectStore>,
    registry: Entity<AgentRegistryStore>,
    active_thread: Option<ThreadId>,
    search: Entity<TextInput>,
    /// Zed's "Thread History", showing only archived threads, in place of the list.
    showing_history: bool,
    history_search: Entity<TextInput>,
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
        let history_search = cx.new(|cx| TextInput::new("Search archived threads…", cx));
        let rename_input = cx.new(|cx| TextInput::new("Thread title", cx));
        let subscriptions = vec![
            cx.subscribe(&rename_input, |this, _, _: &TextInputEvent, cx| {
                this.apply_rename(cx)
            }),
            cx.observe(&store, |_, _, cx| cx.notify()),
            cx.observe(&registry, |_, _, cx| cx.notify()),
            cx.subscribe(&search, |_, _, _: &TextInputEvent, cx| cx.notify()),
            cx.subscribe(&history_search, |_, _, _: &TextInputEvent, cx| cx.notify()),
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
            showing_history: false,
            history_search,
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
        // Like Zed, clicking elsewhere ends the rename.
        self._rename_blur = Some(cx.on_blur(&focus_handle, window, |this, _, cx| {
            this.finish_renaming(cx)
        }));
        cx.notify();
    }

    /// Zed renames as you type; an empty title is ignored.
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
        let is_all_scope = self.store.read(cx).scope() == ProjectScope::All;
        let has_query = !self.search_query(cx).is_empty();
        let add_button = if is_all_scope {
            IconButton::new("sidebar-open-folder", IconName::Plus)
                .icon_size(IconSize::Small)
                .tooltip(|_, cx| Tooltip::for_action("Open Folder…", &OpenFolder, cx))
                .on_click(|_, window, cx| window.dispatch_action(Box::new(OpenFolder), cx))
        } else {
            IconButton::new("sidebar-new-thread", IconName::Plus)
                .icon_size(IconSize::Small)
                .tooltip(|_, cx| Tooltip::for_action("New Thread", &NewThread, cx))
                .on_click(|_, window, cx| window.dispatch_action(Box::new(NewThread), cx))
        };
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
                PopoverMenu::new("thread-display-options")
                    .menu(move |window, cx| {
                        let store = store.clone();
                        let current = store.read(cx).thread_order();
                        let is_grouped = store.read(cx).group_by_project();
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
                            if is_all_scope {
                                let store = store.clone();
                                menu = menu.separator().toggleable_entry(
                                    "Group by Project",
                                    is_grouped,
                                    IconPosition::End,
                                    None,
                                    move |_, cx| {
                                        store.update(cx, |store, cx| {
                                            store.set_group_by_project(!is_grouped, cx)
                                        })
                                    },
                                );
                            }
                            menu
                        }))
                    })
                    .trigger_with_tooltip(
                        IconButton::new("thread-display-options-trigger", IconName::Filter)
                            .icon_size(IconSize::Small),
                        Tooltip::text("Thread Display Options"),
                    )
                    .anchor(gpui::Anchor::TopRight),
            )
            .child(add_button)
    }

    /// `None` when a search is active and none of the project's threads match it.
    fn render_project(
        &self,
        project: &Project,
        show_header: bool,
        query: &str,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let store = self.store.read(cx);
        let threads: Vec<Thread> = store
            .threads_for(project.id)
            .filter(|thread| matches_query(thread, query))
            .cloned()
            .collect();
        if !query.is_empty() && threads.is_empty() {
            return None;
        }
        // Matches are shown even in collapsed projects.
        let is_collapsed = show_header && query.is_empty() && store.is_collapsed(project.id);

        Some(
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
                            rows.push(self.render_thread(thread, indent, None, cx));
                        }
                        group.children(rows)
                    }
                })
                .into_any_element(),
        )
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

    /// `project_name` follows the title when threads aren't grouped by project.
    fn render_thread(
        &self,
        thread: Thread,
        indent: Pixels,
        project_name: Option<SharedString>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let hover_background = cx.theme().colors().ghost_element_hover;
        let selected_background = cx.theme().colors().ghost_element_selected;
        let is_active = self.active_thread == Some(thread.id);
        let is_renaming = self.renaming_thread == Some(thread.id);
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
        let group_name = SharedString::from(format!("thread-row-{}", thread.id.0));
        let title = SharedString::from(thread.title);
        let sidebar = cx.entity().downgrade();

        let text = h_flex()
            .flex_1()
            .min_w_0()
            .gap_1p5()
            .child(if is_renaming {
                self.render_rename_input(cx)
            } else {
                Label::new(title.clone()).truncate().into_any_element()
            })
            .when_some(project_name, |this, project_name| {
                this.child(
                    Label::new(project_name)
                        .size(LabelSize::Small)
                        .color(Color::Muted)
                        .truncate(),
                )
            });

        let store = self.store.clone();
        let hover_actions = h_flex()
            .absolute()
            .right_1()
            .gap_0p5()
            .visible_on_hover(group_name.clone())
            .child(
                IconButton::new(("rename-thread", thread.id.0), IconName::Pencil)
                    .icon_size(IconSize::Small)
                    .tooltip(Tooltip::text("Rename Thread"))
                    .on_click({
                        let title = title.clone();
                        cx.listener(move |this, _, window, cx| {
                            cx.stop_propagation();
                            this.start_renaming(thread_id, title.clone(), window, cx);
                        })
                    }),
            )
            .child(
                IconButton::new(("archive-thread", thread.id.0), IconName::Archive)
                    .icon_size(IconSize::Small)
                    .tooltip(Tooltip::text("Archive Thread"))
                    .on_click(move |_, _, cx| {
                        cx.stop_propagation();
                        store.update(cx, |store, cx| store.archive_thread(thread_id, cx));
                    }),
            );

        let menu_sidebar = sidebar.clone();
        let menu_title = title;
        right_click_menu(("thread-menu", thread.id.0))
            .trigger(move |is_menu_open, _, _| {
                h_flex()
                    .id(("thread", thread_id.0))
                    .group(group_name.clone())
                    .relative()
                    .h(ROW_HEIGHT)
                    .w_full()
                    .pl(indent + px(8.))
                    .pr_2()
                    .gap_1p5()
                    .when(is_active || is_menu_open, |row| row.bg(selected_background))
                    .when(!is_renaming, |row| {
                        row.cursor_pointer()
                            .hover(|row| row.bg(hover_background))
                            .on_click({
                                let sidebar = sidebar.clone();
                                move |_, _, cx| {
                                    sidebar
                                        .update(cx, |_, cx| {
                                            cx.emit(SidebarEvent::OpenThread(thread_id))
                                        })
                                        .ok();
                                }
                            })
                    })
                    .child(icon.size(IconSize::Small).color(Color::Muted))
                    .child(text)
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
                            div()
                                .when(!is_renaming, |this| {
                                    this.group_hover(group_name.clone(), |this| this.invisible())
                                })
                                .child(
                                    Label::new(activity)
                                        .size(LabelSize::Small)
                                        .color(Color::Muted),
                                ),
                        )
                    })
                    .when(!is_renaming, |row| row.child(hover_actions))
            })
            .menu(move |window, cx| {
                let sidebar = menu_sidebar.clone();
                let title = menu_title.clone();
                ContextMenu::build(window, cx, move |menu, _, _| {
                    menu.entry("Rename Title", None, move |window, cx| {
                        sidebar
                            .update(cx, |sidebar, cx| {
                                sidebar.start_renaming(thread_id, title.clone(), window, cx)
                            })
                            .ok();
                    })
                })
            })
            .into_any_element()
    }

    /// "All projects" without grouping, laid out like Zed's Thread History: one list, grouped
    /// by day when ordered by activity, each thread naming its project.
    fn render_flat_threads(&self, query: &str, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let store = self.store.read(cx);
        let by_activity = store.thread_order() == ThreadOrder::LastActivity;
        let threads: Vec<(Thread, Option<SharedString>)> = store
            .visible_threads()
            .into_iter()
            .filter(|thread| matches_query(thread, query))
            .map(|thread| {
                let project_name = store
                    .project(thread.project_id)
                    .map(|project| project.name());
                (thread.clone(), project_name)
            })
            .collect();

        let today = Local::now().date_naive();
        let mut rows = Vec::with_capacity(threads.len());
        let mut current_bucket = None;
        for (thread, project_name) in threads {
            if by_activity {
                // Threads from before activity was recorded sort last, so they belong in "Older".
                let bucket = match thread.last_activity_at {
                    Some(time) => {
                        TimeBucket::from_dates(today, DateTime::<Local>::from(time).date_naive())
                    }
                    None => TimeBucket::Older,
                };
                if current_bucket != Some(bucket) {
                    current_bucket = Some(bucket);
                    rows.push(
                        div()
                            .w_full()
                            .px_2p5()
                            .pt_3()
                            .pb_1()
                            .child(
                                Label::new(bucket.label())
                                    .size(LabelSize::Small)
                                    .color(Color::Muted),
                            )
                            .into_any_element(),
                    );
                }
            }
            rows.push(self.render_thread(thread, px(0.), project_name, cx));
        }
        rows
    }

    fn render_bottom_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let showing_history = self.showing_history;
        h_flex()
            .p_1()
            .gap_1()
            .flex_none()
            .border_t_1()
            .border_color(cx.theme().colors().border)
            .child(
                IconButton::new("history", IconName::Archive)
                    .icon_size(IconSize::Small)
                    .toggle_state(showing_history)
                    .tooltip(Tooltip::text(if showing_history {
                        "Hide Archived Threads"
                    } else {
                        "Show Archived Threads"
                    }))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.showing_history = !this.showing_history;
                        cx.notify();
                    })),
            )
    }

    /// Zed's Thread History for archived threads: newest first, grouped by day, with search.
    fn render_history(&self, cx: &mut Context<Self>) -> AnyElement {
        let colors = cx.theme().colors();
        let query = self.history_search.read(cx).text().trim().to_lowercase();
        let store = self.store.read(cx);
        let show_project_names = store.scope() == ProjectScope::All;
        let threads: Vec<(Thread, Option<SharedString>)> = store
            .archived_threads()
            .into_iter()
            .filter(|thread| query.is_empty() || thread.title.to_lowercase().contains(&query))
            .map(|thread| {
                let project_name = show_project_names
                    .then(|| {
                        store
                            .project(thread.project_id)
                            .map(|project| project.name())
                    })
                    .flatten();
                (thread.clone(), project_name)
            })
            .collect();
        let count = threads.len();

        let search = h_flex()
            .h(px(40.))
            .flex_none()
            .px_3()
            .gap_1()
            .border_b_1()
            .border_color(colors.border)
            .child(
                Icon::new(IconName::MagnifyingGlass)
                    .size(IconSize::Small)
                    .color(Color::Muted),
            )
            .child(div().flex_1().min_w_0().child(self.history_search.clone()))
            .when(!query.is_empty(), |this| {
                this.child(
                    IconButton::new("clear-history-search", IconName::Close)
                        .icon_size(IconSize::Small)
                        .tooltip(Tooltip::text("Clear Search"))
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.history_search
                                .update(cx, |search, cx| search.set_text("", cx));
                        })),
                )
            });

        let toolbar = h_flex()
            .flex_none()
            .pl_2p5()
            .pr_1p5()
            .h(px(32.))
            .justify_between()
            .border_b_1()
            .border_color(colors.border)
            .child(
                Label::new(if count == 1 {
                    "1 archived thread".to_string()
                } else {
                    format!("{count} archived threads")
                })
                .size(LabelSize::Small)
                .color(Color::Muted),
            )
            .child(
                IconButton::new("history-new-thread", IconName::Plus)
                    .icon_size(IconSize::Small)
                    .tooltip(Tooltip::text("Start New Agent Thread"))
                    .on_click(|_, window, cx| window.dispatch_action(Box::new(NewThread), cx)),
            );

        let today = Local::now().date_naive();
        let mut rows = Vec::new();
        let mut current_bucket = None;
        for (thread, project_name) in threads {
            // Threads from before activity was recorded sort last, so they belong in "Older".
            let bucket = match thread.last_activity_at {
                Some(time) => {
                    TimeBucket::from_dates(today, DateTime::<Local>::from(time).date_naive())
                }
                None => TimeBucket::Older,
            };
            if current_bucket != Some(bucket) {
                current_bucket = Some(bucket);
                rows.push(
                    div()
                        .w_full()
                        .px_2p5()
                        .pt_3()
                        .pb_1()
                        .child(
                            Label::new(bucket.label())
                                .size(LabelSize::Small)
                                .color(Color::Muted),
                        )
                        .into_any_element(),
                );
            }
            rows.push(self.render_history_entry(thread, project_name, cx));
        }

        v_flex()
            .flex_1()
            .min_h_0()
            .child(search)
            .child(toolbar)
            .child(
                v_flex()
                    .id("thread-history")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .pb_2()
                    .children(rows),
            )
            .into_any_element()
    }

    fn render_history_entry(
        &self,
        thread: Thread,
        project_name: Option<SharedString>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let hover_background = cx.theme().colors().ghost_element_hover;
        let thread_id = thread.id;
        let group_name = SharedString::from(format!("history-entry-{}", thread.id.0));
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
        let icon_color = Color::Custom(cx.theme().colors().icon_muted.opacity(0.6));
        let timestamp = thread.last_activity_at.map(history_timestamp);
        let title = SharedString::from(thread.title.clone());

        let action = IconButton::new(("delete-thread", thread.id.0), IconName::Trash)
            .icon_size(IconSize::Small)
            .icon_color(Color::Muted)
            .tooltip(Tooltip::text("Delete Thread"))
            .on_click({
                let title = title.clone();
                cx.listener(move |this, _, window, cx| {
                    cx.stop_propagation();
                    this.confirm_delete_thread(thread_id, title.clone(), window, cx);
                })
            });

        h_flex()
            .id(("history-entry", thread.id.0))
            .group(group_name.clone())
            .relative()
            .h(ROW_HEIGHT)
            .w_full()
            .px_2p5()
            .gap_1p5()
            .cursor_pointer()
            .hover(|row| row.bg(hover_background))
            .child(icon.size(IconSize::Small).color(icon_color))
            .child(
                h_flex()
                    .flex_1()
                    .min_w_0()
                    .gap_1p5()
                    .child(Label::new(title).truncate().color(Color::Muted))
                    .when_some(project_name, |this, project_name| {
                        this.child(
                            Label::new(project_name)
                                .size(LabelSize::Small)
                                .color(Color::Muted)
                                .truncate(),
                        )
                    }),
            )
            .when_some(timestamp, |row, timestamp| {
                row.child(
                    div()
                        .group_hover(group_name.clone(), |this| this.invisible())
                        .child(
                            Label::new(timestamp)
                                .size(LabelSize::Small)
                                .color(Color::Muted),
                        ),
                )
            })
            .child(
                div()
                    .absolute()
                    .right_1()
                    .visible_on_hover(group_name)
                    .child(action),
            )
            .on_click(cx.listener(move |this, _, _, cx| {
                // Opening an archived thread restores it, as in Zed.
                this.store
                    .update(cx, |store, cx| store.unarchive_thread(thread_id, cx));
                cx.emit(SidebarEvent::OpenThread(thread_id));
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
}

impl Render for Sidebar {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let border = cx.theme().colors().border;
        let border_variant = cx.theme().colors().border_variant;
        let panel_background = cx.theme().colors().panel_background;
        let query = self.search_query(cx);
        let store = self.store.read(cx);
        let show_headers = store.scope() == ProjectScope::All;
        let is_flat = show_headers && !store.group_by_project();
        let projects: Vec<Project> = store.visible_projects().cloned().collect();

        let content = if self.showing_history {
            self.render_history(cx)
        } else if projects.is_empty() {
            self.render_empty_state().into_any_element()
        } else {
            let rows = if is_flat {
                self.render_flat_threads(&query, cx)
            } else {
                let mut groups = Vec::with_capacity(projects.len());
                for project in &projects {
                    if let Some(group) = self.render_project(project, show_headers, &query, cx) {
                        groups.push(
                            div()
                                .when(show_headers && !groups.is_empty(), |group| {
                                    group.border_t_1().border_color(border_variant)
                                })
                                .child(group)
                                .into_any_element(),
                        );
                    }
                }
                groups
            };
            let empty_message = (rows.is_empty()).then(|| {
                if query.is_empty() {
                    "No threads yet"
                } else {
                    "No matching threads"
                }
            });
            v_flex()
                .id("sidebar-threads")
                .flex_1()
                .min_h_0()
                .overflow_y_scroll()
                .pb_2()
                .when_some(empty_message, |list, message| {
                    list.child(
                        h_flex().h(ROW_HEIGHT).px_3().child(
                            Label::new(message)
                                .size(LabelSize::Small)
                                .color(Color::Placeholder),
                        ),
                    )
                })
                .children(rows)
                .into_any_element()
        };

        v_flex()
            .w(SIDEBAR_WIDTH)
            .h_full()
            .flex_none()
            .border_r_1()
            .border_color(border)
            .bg(panel_background)
            .when(!self.showing_history, |this| {
                this.child(self.render_header(cx))
            })
            .child(content)
            .child(self.render_bottom_bar(cx))
    }
}

fn matches_query(thread: &Thread, query: &str) -> bool {
    query.is_empty() || thread.title.to_lowercase().contains(query)
}

/// Zed's Thread History groups.
#[derive(Clone, Copy, PartialEq, Eq)]
enum TimeBucket {
    Today,
    Yesterday,
    ThisWeek,
    PastWeek,
    Older,
}

impl TimeBucket {
    fn from_dates(reference: NaiveDate, date: NaiveDate) -> Self {
        if date == reference {
            return TimeBucket::Today;
        }
        if date == reference - TimeDelta::days(1) {
            return TimeBucket::Yesterday;
        }
        let week = date.iso_week();
        if reference.iso_week() == week {
            return TimeBucket::ThisWeek;
        }
        if (reference - TimeDelta::days(7)).iso_week() == week {
            return TimeBucket::PastWeek;
        }
        TimeBucket::Older
    }

    fn label(&self) -> &'static str {
        match self {
            TimeBucket::Today => "Today",
            TimeBucket::Yesterday => "Yesterday",
            TimeBucket::ThisWeek => "This Week",
            TimeBucket::PastWeek => "Past Week",
            TimeBucket::Older => "Older",
        }
    }
}

/// Zed's history timestamps: `5m`, `3h`, `2d`, `1w`, `2mo`.
fn history_timestamp(time: SystemTime) -> String {
    let duration = Local::now().signed_duration_since(DateTime::<Local>::from(time));
    let minutes = duration.num_minutes();
    let hours = duration.num_hours();
    let days = duration.num_days();
    let weeks = days / 7;
    let months = days / 30;
    if minutes < 60 {
        format!("{}m", minutes.max(1))
    } else if hours < 24 {
        format!("{}h", hours.max(1))
    } else if days < 7 {
        format!("{}d", days.max(1))
    } else if weeks < 4 {
        format!("{}w", weeks.max(1))
    } else {
        format!("{}mo", months.max(1))
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
