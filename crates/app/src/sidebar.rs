use std::time::{Duration, SystemTime};

use crate::project_store::{ProjectStore, ThreadStatus};
use agentz_protocol::agents::AgentId;
use collections::HashMap;
use gpui::{
    AnyElement, App, ClickEvent, Context, Entity, EventEmitter, Focusable as _, FontWeight,
    KeyBinding, PromptLevel, ScrollHandle, Subscription, Task, Window, anchored, deferred, svg,
};
use projects::{Project, ProjectId, ProjectScope, Thread, ThreadId};
use text_input::{TextInput, TextInputEvent};
use ui::{
    CommonAnimationExt as _, ContextMenu, ContextMenuEntry, Tooltip, WithScrollbar as _,
    prelude::*, right_click_menu,
};

use crate::project_info::{ProjectInfo, ProjectInfoStore, render_project_icon};
use crate::registry_store::AgentRegistryStore;
use crate::{NewThread, OpenFolder, OpenSettings};

/// How often relative activity times ("5m") are re-rendered.
const ACTIVITY_REFRESH_INTERVAL: Duration = Duration::from_secs(30);
const CARD_HEIGHT: Pixels = px(78.);
const DETAILS_DELAY: Duration = Duration::from_millis(500);
pub const SIDEBAR_WIDTH: Pixels = px(290.);
const RENAME_KEY_CONTEXT: &str = "SidebarRename";
const SEARCH_KEY_CONTEXT: &str = "SidebarSearch";
const ARCHIVED_ROW_HEIGHT: Pixels = px(36.);
/// t3code pages its settled shelf: recent history is the common lookup, the deep tail stays
/// behind "Show more".
const ARCHIVED_INITIAL_COUNT: usize = 10;
const ARCHIVED_PAGE_COUNT: usize = 25;

pub fn init(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("enter", menu::Confirm, Some(RENAME_KEY_CONTEXT)),
        KeyBinding::new("escape", menu::Cancel, Some(RENAME_KEY_CONTEXT)),
        KeyBinding::new("up", menu::SelectPrevious, Some(SEARCH_KEY_CONTEXT)),
        KeyBinding::new("down", menu::SelectNext, Some(SEARCH_KEY_CONTEXT)),
        KeyBinding::new("enter", menu::Confirm, Some(SEARCH_KEY_CONTEXT)),
        KeyBinding::new("escape", menu::Cancel, Some(SEARCH_KEY_CONTEXT)),
    ]);
}

pub enum SidebarEvent {
    OpenThread(ThreadId),
    OpenProjectSettings(ProjectId),
}

/// The thread list, modeled on t3code's sidebar: active threads as cards and archived threads
/// in a collapsible shelf at the bottom (t3code's "Settled" shelf).
pub struct Sidebar {
    store: Entity<ProjectStore>,
    registry: Entity<AgentRegistryStore>,
    active_thread: Option<ThreadId>,
    search: Entity<TextInput>,
    /// The highlighted search result, which Enter opens.
    search_index: usize,
    search_scroll: ScrollHandle,
    archived_shown: usize,
    project_info: HashMap<ProjectId, ProjectInfo>,
    /// The thread whose details popover is showing, after hovering it for a moment.
    details_thread: Option<ThreadId>,
    /// A popover waiting out the hover delay, and the thread it's for.
    details_delay: Option<(ThreadId, Task<()>)>,
    /// The row under the mouse.
    hovered_thread: Option<ThreadId>,
    renaming_thread: Option<ThreadId>,
    rename_input: Entity<TextInput>,
    _rename_blur: Option<Subscription>,
    _subscriptions: Vec<Subscription>,
    _activity_refresh: Task<()>,
    _project_info_subscription: Subscription,
}

impl EventEmitter<SidebarEvent> for Sidebar {}

impl Sidebar {
    pub fn new(
        store: Entity<ProjectStore>,
        registry: Entity<AgentRegistryStore>,
        cx: &mut Context<Self>,
    ) -> Self {
        let search = cx.new(|cx| TextInput::new("Search…", cx));
        let rename_input = cx.new(|cx| TextInput::new("Thread title", cx));
        let subscriptions = vec![
            cx.subscribe(&rename_input, |this, _, _: &TextInputEvent, cx| {
                this.apply_rename(cx)
            }),
            cx.observe(&store, |_, _, cx| cx.notify()),
            cx.observe(&registry, |_, _, cx| cx.notify()),
            cx.subscribe(&search, |this, _, _: &TextInputEvent, cx| {
                this.search_index = 0;
                this.search_scroll.set_offset(gpui::point(px(0.), px(0.)));
                cx.notify();
            }),
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
        let project_info_store = ProjectInfoStore::global(cx);
        let project_info = project_info_store.read(cx).info().clone();
        let project_info_subscription = cx.observe(&project_info_store, |this, store, cx| {
            this.project_info = store.read(cx).info().clone();
            cx.notify();
        });
        Self {
            store,
            registry,
            active_thread: None,
            search,
            search_index: 0,
            search_scroll: ScrollHandle::new(),
            archived_shown: ARCHIVED_INITIAL_COUNT,
            project_info,
            details_thread: None,
            details_delay: None,
            hovered_thread: None,
            renaming_thread: None,
            rename_input,
            _rename_blur: None,
            _subscriptions: subscriptions,
            _activity_refresh: activity_refresh,
            _project_info_subscription: project_info_subscription,
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

    /// t3code's search results: every matching thread, active then archived, in one list.
    fn search_results(&self, cx: &App) -> Vec<Thread> {
        let query = self.search_query(cx);
        let store = self.store.read(cx);
        store
            .active_threads()
            .into_iter()
            .chain(store.archived_threads())
            .filter(|thread| matches_query(thread, &query))
            .cloned()
            .collect()
    }

    fn move_search_highlight(&mut self, forward: bool, cx: &mut Context<Self>) {
        let count = self.search_results(cx).len();
        if count == 0 {
            return;
        }
        self.search_index = if forward {
            (self.search_index + 1) % count
        } else {
            self.search_index.checked_sub(1).unwrap_or(count - 1)
        };
        self.search_scroll.scroll_to_item(self.search_index);
        cx.notify();
    }

    /// Opening a result ends the search, as in t3code.
    fn open_search_result(&mut self, thread_id: ThreadId, cx: &mut Context<Self>) {
        self.search.update(cx, |search, cx| search.set_text("", cx));
        cx.emit(SidebarEvent::OpenThread(thread_id));
    }

    fn render_header(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let has_query = !self.search_query(cx).is_empty();
        h_flex()
            .key_context(SEARCH_KEY_CONTEXT)
            .on_action(
                cx.listener(|this, _: &menu::SelectNext, _, cx| {
                    this.move_search_highlight(true, cx)
                }),
            )
            .on_action(cx.listener(|this, _: &menu::SelectPrevious, _, cx| {
                this.move_search_highlight(false, cx)
            }))
            .on_action(cx.listener(|this, _: &menu::Confirm, _, cx| {
                let result = this.search_results(cx).into_iter().nth(this.search_index);
                if let Some(thread) = result {
                    this.open_search_result(thread.id, cx);
                }
            }))
            .on_action(cx.listener(|this, _: &menu::Cancel, _, cx| {
                if this.search_query(cx).is_empty() {
                    cx.propagate();
                } else {
                    this.search.update(cx, |search, cx| search.set_text("", cx));
                }
            }))
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

    fn render_project_icon(&self, project: Option<&Project>, cx: &App) -> AnyElement {
        match project {
            Some(project) => {
                render_project_icon(project, self.project_info.get(&project.id), px(16.), cx)
            }
            None => div().size_4().flex_none().into_any_element(),
        }
    }

    /// Rename, Archive or Unarchive, Project Settings, and Delete, each with its icon.
    fn thread_menu(
        &self,
        thread_id: ThreadId,
        title: SharedString,
        is_archived: bool,
        cx: &mut Context<Self>,
    ) -> impl Fn(&mut Window, &mut App) -> Entity<ContextMenu> + 'static {
        let sidebar = cx.entity().downgrade();
        move |window, cx| {
            let sidebar = sidebar.clone();
            let title = title.clone();
            ContextMenu::build(window, cx, move |menu, _, _| {
                let rename = {
                    let sidebar = sidebar.clone();
                    let title = title.clone();
                    move |window: &mut Window, cx: &mut App| {
                        sidebar
                            .update(cx, |sidebar, cx| {
                                sidebar.start_renaming(thread_id, title.clone(), window, cx)
                            })
                            .ok();
                    }
                };
                let toggle_archived = {
                    let sidebar = sidebar.clone();
                    move |_: &mut Window, cx: &mut App| {
                        sidebar
                            .update(cx, |sidebar, cx| {
                                sidebar.store.update(cx, |store, cx| {
                                    if is_archived {
                                        store.unarchive_thread(thread_id, cx)
                                    } else {
                                        store.archive_thread(thread_id, cx)
                                    }
                                })
                            })
                            .ok();
                    }
                };
                let open_project_settings = {
                    let sidebar = sidebar.clone();
                    move |_: &mut Window, cx: &mut App| {
                        sidebar
                            .update(cx, |sidebar, cx| {
                                let project_id = sidebar
                                    .store
                                    .read(cx)
                                    .thread(thread_id)
                                    .map(|thread| thread.project_id);
                                if let Some(project_id) = project_id {
                                    cx.emit(SidebarEvent::OpenProjectSettings(project_id));
                                }
                            })
                            .ok();
                    }
                };
                let delete = {
                    let sidebar = sidebar.clone();
                    let title = title.clone();
                    move |window: &mut Window, cx: &mut App| {
                        sidebar
                            .update(cx, |sidebar, cx| {
                                sidebar.confirm_delete_thread(thread_id, title.clone(), window, cx)
                            })
                            .ok();
                    }
                };
                menu.item(
                    ContextMenuEntry::new("Rename")
                        .icon(IconName::Pencil)
                        .icon_color(Color::Muted)
                        .handler(rename),
                )
                .item(
                    ContextMenuEntry::new(if is_archived { "Unarchive" } else { "Archive" })
                        .icon(if is_archived {
                            IconName::Undo
                        } else {
                            IconName::Archive
                        })
                        .icon_color(Color::Muted)
                        .handler(toggle_archived),
                )
                .item(
                    ContextMenuEntry::new("Project Settings")
                        .icon(IconName::Settings)
                        .icon_color(Color::Muted)
                        .handler(open_project_settings),
                )
                .separator()
                .item(
                    ContextMenuEntry::new("Delete…")
                        .icon(IconName::Trash)
                        .icon_color(Color::Muted)
                        .handler(delete),
                )
            })
        }
    }

    /// Shows the hovered thread's details after a moment, like t3code's row tooltip.
    fn thread_hovered(&mut self, thread_id: ThreadId, hovered: bool, cx: &mut Context<Self>) {
        if !hovered {
            if self.hovered_thread == Some(thread_id) {
                self.hovered_thread = None;
            }
            // Leaving one row may be reported after entering the next, so only this row's
            // popover is cancelled.
            let is_pending = self
                .details_delay
                .as_ref()
                .is_some_and(|(pending, _)| *pending == thread_id);
            if is_pending || self.details_thread == Some(thread_id) {
                self.hide_details(cx);
            }
            return;
        }
        self.hovered_thread = Some(thread_id);
        if self.details_thread == Some(thread_id) {
            return;
        }
        let delay = cx.spawn(async move |this, cx| {
            cx.background_executor().timer(DETAILS_DELAY).await;
            this.update(cx, |this, cx| {
                this.details_delay = None;
                this.details_thread = Some(thread_id);
                cx.notify();
            })
            .ok();
        });
        self.details_delay = Some((thread_id, delay));
    }

    fn hide_details(&mut self, cx: &mut Context<Self>) {
        self.details_delay = None;
        if self.details_thread.take().is_some() {
            cx.notify();
        }
    }

    fn thread_details(
        &self,
        thread: &Thread,
        project: Option<&Project>,
        cx: &App,
    ) -> ThreadDetails {
        let agent = thread.agent_id.as_ref().map(|agent_id| {
            let agent_id = AgentId::new(agent_id.clone());
            let registry_agent = self.registry.read(cx).agent(&agent_id);
            let agent_name = registry_agent
                .map(|agent| agent.name().clone())
                .unwrap_or_else(|| agent_id.0.clone());
            let label = match &thread.model {
                Some(model) => format!("{model} · {agent_name}").into(),
                None => agent_name,
            };
            let icon_path = registry_agent.and_then(|agent| agent.icon_path().cloned());
            (icon_path, label)
        });
        ThreadDetails {
            title: thread.title.clone().into(),
            project: project
                .map(|project| (project.clone(), self.project_info.get(&project.id).cloned())),
            branch: project
                .and_then(|project| self.project_info.get(&project.id))
                .and_then(|info| info.git_head.as_ref())
                .map(|git_head| git_head.branch.clone().into()),
            agent,
        }
    }

    /// t3code's thread card: the project and status on top, then the title, then the branch with
    /// the agent's icon at the bottom right.
    fn render_thread_card(
        &self,
        thread: Thread,
        project: Option<Project>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let colors = cx.theme().colors();
        let hover_background = colors.ghost_element_hover;
        let selected_background = colors.ghost_element_selected;
        let is_active = self.active_thread == Some(thread.id);
        let is_renaming = self.renaming_thread == Some(thread.id);
        let thread_status = self.store.read(cx).thread_status(thread.id);
        let thread_id = thread.id;
        let icon = self.agent_icon(&thread, cx);
        let details = self.thread_details(&thread, project.as_ref(), cx);
        // With one project selected, every card would repeat it, so the project line goes and
        // the status moves next to the title.
        let shows_all_projects = self.store.read(cx).scope() == ProjectScope::All;
        let git_head = project
            .as_ref()
            .and_then(|project| self.project_info.get(&project.id))
            .and_then(|info| info.git_head.clone());
        let faint_text = cx.theme().colors().text_muted.opacity(0.4);
        let time = thread
            .last_activity_at
            .map(|time| format_relative_time(time, SystemTime::now()));
        let group_name = SharedString::from(format!("thread-card-{}", thread.id.0));
        let title = SharedString::from(thread.title);

        let status = match thread_status {
            Some(ThreadStatus::Working) => h_flex()
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
                .into_any_element(),
            Some(status) => render_status_pill(status, cx).into_any_element(),
            None => Label::new(time.unwrap_or_default())
                .size(LabelSize::Small)
                .color(Color::Muted)
                .into_any_element(),
        };
        let store = self.store.clone();
        // Like t3code's Settle button: muted text that brightens under the mouse, with no fill
        // of its own over the card's hover background.
        let muted_text = cx.theme().colors().text_muted;
        let bright_text = cx.theme().colors().text;
        let button_group = SharedString::from(format!("archive-button-{}", thread.id.0));
        let archive_button = h_flex()
            .id(("archive-thread", thread.id.0))
            .group(button_group.clone())
            .h_full()
            .px_1p5()
            .gap_1()
            .cursor_pointer()
            .text_ui_sm(cx)
            .text_color(muted_text)
            .hover(|this| this.text_color(bright_text))
            .child(
                svg()
                    .path(IconName::Archive.path())
                    .size(IconSize::XSmall.rems())
                    .flex_none()
                    .text_color(muted_text)
                    .group_hover(button_group, |this| this.text_color(bright_text)),
            )
            .child("Archive")
            // The details would sit beside the button, so they give way to it.
            .on_hover(cx.listener(move |this, hovered, _, cx| {
                if *hovered {
                    this.hide_details(cx);
                } else if this.hovered_thread == Some(thread_id) {
                    // Back on the card rather than off it.
                    this.thread_hovered(thread_id, true, cx);
                }
            }))
            .on_click(move |_, _, cx| {
                cx.stop_propagation();
                store.update(cx, |store, cx| store.archive_thread(thread_id, cx));
            });

        // The status yields to the Archive button on hover.
        let status_slot = div()
            .flex_none()
            .when(!is_renaming, |this| {
                this.group_hover(group_name.clone(), |this| this.invisible())
            })
            .child(status);
        let archive_slot = (!is_renaming).then(|| {
            // Centered on its line, like t3code's Settle button.
            h_flex()
                .absolute()
                .top_0()
                .bottom_0()
                .right_0()
                .visible_on_hover(group_name.clone())
                .child(archive_button)
        });
        let title_element = if is_renaming {
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
        };
        let (project_line, title_line) = if shows_all_projects {
            let project_line = h_flex()
                .relative()
                .h_5()
                .min_w_0()
                .gap_1p5()
                .child(self.render_project_icon(project.as_ref(), cx))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .children(project.as_ref().map(|project| {
                            Label::new(project.name())
                                .size(LabelSize::Small)
                                .color(Color::Muted)
                                .truncate()
                        })),
                )
                .child(status_slot)
                .children(archive_slot);
            let title_line = h_flex().mt_1().min_w_0().child(title_element);
            (Some(project_line), title_line)
        } else {
            let title_line = h_flex()
                .relative()
                .min_w_0()
                .gap_1p5()
                .child(title_element)
                .child(status_slot)
                .children(archive_slot);
            (None, title_line)
        };

        let card =
            v_flex()
                .id(("thread-card", thread.id.0))
                .group(group_name)
                .on_hover(cx.listener(move |this, hovered, _, cx| {
                    this.thread_hovered(thread_id, *hovered, cx)
                }))
                .on_any_mouse_down(cx.listener(|this, _, _, cx| this.hide_details(cx)))
                .relative()
                .w_full()
                .when(shows_all_projects, |card| card.h(CARD_HEIGHT))
                .px_2p5()
                .py_2()
                .rounded_md()
                .when(is_active, |card| card.bg(selected_background))
                .when(!is_renaming, |card| {
                    card.cursor_pointer()
                        .hover(|card| card.bg(hover_background))
                        .on_click(self.thread_click_handler(thread_id, title.clone(), cx))
                })
                .children(project_line)
                .child(title_line)
                .child(
                    h_flex()
                        .mt_0p5()
                        .min_w_0()
                        .gap_1p5()
                        .child(h_flex().flex_1().min_w_0().gap_1().when_some(
                            git_head,
                            |this, git_head| {
                                this.when_some(git_head.worktree.clone(), |this, worktree| {
                                    let tooltip = format!(
                                        "Worktree: {} ({})",
                                        worktree.display(),
                                        git_head.branch
                                    );
                                    this.child(
                                        div()
                                            .id(("thread-worktree", thread_id.0))
                                            .flex_none()
                                            .tooltip(Tooltip::text(tooltip))
                                            .child(
                                                Icon::new(IconName::GitWorktree)
                                                    .size(IconSize::XSmall)
                                                    .color(Color::Custom(faint_text)),
                                            ),
                                    )
                                })
                                .child(
                                    Label::new(git_head.branch)
                                        .size(LabelSize::Small)
                                        .color(Color::Custom(faint_text))
                                        .truncate_middle(),
                                )
                            },
                        ))
                        .child(
                            div()
                                .flex_none()
                                .opacity(0.6)
                                .child(icon.size(IconSize::Small).color(Color::Muted)),
                        ),
                );

        // The details popover stays hidden while the thread's menu is open.
        let details_popover =
            (self.details_thread == Some(thread_id)).then(|| render_details_popover(details, cx));
        let menu = self.thread_menu(thread_id, title, false, cx);
        right_click_menu(("thread-menu", thread.id.0))
            .trigger(move |is_menu_open, _, _| {
                div()
                    .relative()
                    .child(div().py_0p5().child(card))
                    .when(!is_menu_open, |this| this.children(details_popover))
            })
            .menu(menu)
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

    /// t3code's slim row for parked threads: the project's icon, dimmed until hovered, and a way
    /// back on hover.
    fn render_archived_row(&self, thread: Thread, cx: &mut Context<Self>) -> AnyElement {
        let colors = cx.theme().colors();
        let hover_background = colors.ghost_element_hover;
        let selected_background = colors.ghost_element_selected;
        let is_active = self.active_thread == Some(thread.id);
        let is_renaming = self.renaming_thread == Some(thread.id);
        let thread_id = thread.id;
        let project = self.store.read(cx).project(thread.project_id).cloned();
        let project_icon = self.render_project_icon(project.as_ref(), cx);
        let details = self.thread_details(&thread, project.as_ref(), cx);
        let time = thread
            .archived_at
            .map(|time| format_relative_time(time, SystemTime::now()));
        let group_name = SharedString::from(format!("archived-row-{}", thread.id.0));
        let title = SharedString::from(thread.title);
        let store = self.store.clone();

        let row =
            h_flex()
                .id(("archived-thread", thread.id.0))
                .group(group_name.clone())
                .on_hover(cx.listener(move |this, hovered, _, cx| {
                    this.thread_hovered(thread_id, *hovered, cx)
                }))
                .on_any_mouse_down(cx.listener(|this, _, _, cx| this.hide_details(cx)))
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
                        .child(project_icon),
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

        // The details popover stays hidden while the thread's menu is open.
        let details_popover =
            (self.details_thread == Some(thread_id)).then(|| render_details_popover(details, cx));
        let menu = self.thread_menu(thread_id, title, true, cx);
        right_click_menu(("archived-thread-menu", thread.id.0))
            .trigger(move |is_menu_open, _, _| {
                div()
                    .relative()
                    .child(row)
                    .when(!is_menu_open, |this| this.children(details_popover))
            })
            .menu(menu)
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

    /// t3code's search result row: the project's icon, the title, and the time, highlighted
    /// under the keyboard or mouse.
    fn render_search_result(
        &self,
        index: usize,
        thread: Thread,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let colors = cx.theme().colors();
        let selected_background = colors.ghost_element_selected;
        let hover_background = colors.ghost_element_hover;
        let thread_id = thread.id;
        let project = self.store.read(cx).project(thread.project_id).cloned();
        let details = self.thread_details(&thread, project.as_ref(), cx);
        let is_highlighted = index == self.search_index;
        let is_active = self.active_thread == Some(thread_id);
        let time = thread
            .last_activity_at
            .map(|time| format_relative_time(time, SystemTime::now()));
        div()
            .relative()
            .child(
                h_flex()
                    .id(("search-result", thread_id.0))
                    .min_h(px(36.))
                    .px_2p5()
                    .py_1()
                    .gap_2p5()
                    .rounded_md()
                    .cursor_pointer()
                    .when(is_highlighted || is_active, |row| {
                        row.bg(selected_background)
                    })
                    .when(!is_highlighted && !is_active, |row| {
                        row.hover(|row| row.bg(hover_background))
                    })
                    .child(self.render_project_icon(project.as_ref(), cx))
                    .child(
                        div().flex_1().min_w_0().child(
                            Label::new(thread.title)
                                .truncate()
                                .when(!is_highlighted && !is_active, |label| {
                                    label.color(Color::Muted)
                                }),
                        ),
                    )
                    .children(
                        time.map(|time| {
                            Label::new(time).size(LabelSize::Small).color(Color::Muted)
                        }),
                    )
                    .on_mouse_move(cx.listener(move |this, _, _, cx| {
                        if this.search_index != index {
                            this.search_index = index;
                            cx.notify();
                        }
                    }))
                    .on_hover(cx.listener(move |this, hovered, _, cx| {
                        this.thread_hovered(thread_id, *hovered, cx)
                    }))
                    .on_any_mouse_down(cx.listener(|this, _, _, cx| this.hide_details(cx)))
                    .on_click(
                        cx.listener(move |this, _, _, cx| this.open_search_result(thread_id, cx)),
                    ),
            )
            .when(self.details_thread == Some(thread_id), |row| {
                row.child(render_details_popover(details, cx))
            })
            .into_any_element()
    }

    fn render_search_results(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let results = self.search_results(cx);
        let rows: Vec<AnyElement> = results
            .into_iter()
            .enumerate()
            .map(|(index, thread)| self.render_search_result(index, thread, cx))
            .collect();
        div()
            .id("sidebar-search-results-scroll")
            .flex_1()
            .min_h_0()
            .child(
                v_flex()
                    .id("sidebar-search-results")
                    .size_full()
                    .px_1()
                    .pt_1()
                    .pb_2()
                    .gap_px()
                    .overflow_y_scroll()
                    .track_scroll(&self.search_scroll)
                    .when(rows.is_empty(), |list| {
                        list.child(
                            h_flex().justify_center().py_6().child(
                                Label::new("No threads found")
                                    .size(LabelSize::Small)
                                    .color(Color::Muted),
                            ),
                        )
                    })
                    .children(rows),
            )
            .vertical_scrollbar_for(&self.search_scroll, window, cx)
            .into_any_element()
    }

    fn render_threads(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        // While searching, t3code swaps the list for the matching threads.
        if !self.search_query(cx).is_empty() {
            return self.render_search_results(window, cx);
        }
        let store = self.store.read(cx);
        let active: Vec<(Thread, Option<Project>)> = store
            .active_threads()
            .into_iter()
            .map(|thread| (thread.clone(), store.project(thread.project_id).cloned()))
            .collect();
        let archived: Vec<Thread> = store.archived_threads().into_iter().cloned().collect();
        let is_archived_expanded = store.archived_expanded();

        let mut rows = Vec::with_capacity(active.len());
        for (thread, project) in active {
            rows.push(self.render_thread_card(thread, project, cx));
        }
        if rows.is_empty() {
            rows.push(
                h_flex()
                    .h_8()
                    .px_2p5()
                    .child(
                        Label::new("No threads yet")
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
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
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
                self.render_threads(window, cx)
            } else {
                self.render_empty_state().into_any_element()
            })
            .child(render_footer_item(
                "open-settings",
                IconName::Settings,
                "Settings",
                |_, window, cx| window.dispatch_action(Box::new(OpenSettings), cx),
                cx,
            ))
    }
}

/// t3code's thread popover: the title, then the project, branch, and model with agent.
struct ThreadDetails {
    title: SharedString,
    project: Option<(Project, Option<ProjectInfo>)>,
    branch: Option<SharedString>,
    agent: Option<(Option<SharedString>, SharedString)>,
}

impl ThreadDetails {
    fn render(self, cx: &App) -> AnyElement {
        let detail_color = Color::Custom(cx.theme().colors().text.opacity(0.75));
        let detail_row = |icon: AnyElement, label: Label| {
            h_flex()
                .min_w_0()
                .gap_2()
                .child(div().flex_none().child(icon))
                .child(
                    div()
                        .min_w_0()
                        .child(label.size(LabelSize::Small).color(detail_color)),
                )
        };
        let small_icon = |name: IconName| {
            Icon::new(name)
                .size(IconSize::XSmall)
                .color(Color::Muted)
                .into_any_element()
        };
        let mut rows = Vec::new();
        if let Some((project, info)) = &self.project {
            rows.push(detail_row(
                render_project_icon(project, info.as_ref(), px(12.), cx),
                Label::new(project.name()).truncate(),
            ));
        }
        if let Some(branch) = &self.branch {
            rows.push(detail_row(
                small_icon(IconName::GitBranch),
                Label::new(branch.clone()).truncate_middle(),
            ));
        }
        if let Some((icon_path, label)) = &self.agent {
            let icon = icon_path
                .clone()
                .map(Icon::from_external_svg)
                .unwrap_or_else(|| Icon::new(IconName::Terminal));
            rows.push(detail_row(
                div()
                    .opacity(0.6)
                    .child(icon.size(IconSize::XSmall).color(Color::Muted))
                    .into_any_element(),
                Label::new(label.clone()).truncate(),
            ));
        }
        v_flex()
            .elevation_2(cx)
            .font(theme::theme_settings(cx).ui_font(cx).clone())
            .text_ui(cx)
            .py_1()
            .px_2()
            .child(
                v_flex()
                    .min_w(px(220.))
                    .max_w(px(320.))
                    .gap_2()
                    .px_1()
                    .py_2()
                    .child(
                        Label::new(self.title)
                            .size(LabelSize::Small)
                            .weight(FontWeight::MEDIUM)
                            .truncate(),
                    )
                    .child(v_flex().gap_1p5().pl_0p5().children(rows)),
            )
            .into_any_element()
    }
}

/// Placed beside the row's right edge, top-aligned, as t3code places its row tooltip.
fn render_details_popover(details: ThreadDetails, cx: &App) -> AnyElement {
    div()
        .absolute()
        .top_0()
        .left_full()
        .child(
            deferred(
                anchored()
                    .snap_to_window_with_margin(px(8.))
                    .child(div().ml_1().child(details.render(cx))),
            )
            .with_priority(1),
        )
        .into_any_element()
}

/// The row at the bottom of the sidebar. Settings shows its Back row with this same layout, so
/// the two land under the same pointer position.
pub fn render_footer_item(
    id: &'static str,
    icon: IconName,
    label: &'static str,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    cx: &App,
) -> AnyElement {
    let colors = cx.theme().colors();
    div()
        .flex_none()
        .p_1()
        .border_t_1()
        .border_color(colors.border)
        .child(
            h_flex()
                .id(id)
                .h(px(28.))
                .px_2()
                .gap_2()
                .rounded_md()
                .cursor_pointer()
                .hover(|item| item.bg(colors.ghost_element_hover))
                .child(Icon::new(icon).size(IconSize::Small).color(Color::Muted))
                .child(Label::new(label).color(Color::Muted))
                .on_click(on_click),
        )
        .into_any_element()
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

/// t3code's status pill: a dot and a label in the status color.
pub(crate) fn render_status_pill(status: ThreadStatus, cx: &App) -> impl IntoElement {
    let (label, color) = match status {
        ThreadStatus::PendingApproval => ("Pending Approval", Color::Warning),
        ThreadStatus::Working => ("Working", Color::Accent),
        ThreadStatus::Completed => ("Completed", Color::Success),
    };
    h_flex().gap_1().child(render_status_dot(status, cx)).child(
        Label::new(label)
            .size(LabelSize::Small)
            .weight(FontWeight::MEDIUM)
            .color(color),
    )
}

pub(crate) fn render_status_dot(status: ThreadStatus, cx: &App) -> impl IntoElement {
    let color = match status {
        ThreadStatus::PendingApproval => Color::Warning,
        ThreadStatus::Working => Color::Accent,
        ThreadStatus::Completed => Color::Success,
    };
    div()
        .flex_none()
        .size_1p5()
        .rounded_full()
        .bg(color.color(cx))
}
