use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use crate::agent_icons::agent_icon;
use crate::agent_view::TOOLBAR_HEIGHT;
use crate::machines::{MachineId, Machines, ProjectKey, Scope, ThreadKey, project_at};
use crate::project_store::{ProjectStore, ThreadStatus};
use agentz_protocol::agents::AgentId;
use gpui::{
    AnyElement, App, ClickEvent, Context, ElementId, Entity, EventEmitter, Focusable as _,
    FontWeight, Hsla, KeyBinding, PromptLevel, ScrollHandle, Stateful, Subscription, Task, Window,
    anchored, deferred, svg,
};
use projects::{Project, Thread, Workspace, WorkspaceKind};
use text_input::{TextInput, TextInputEvent};
use ui::{
    CommonAnimationExt as _, ContextMenu, ContextMenuEntry, Tooltip, WithScrollbar as _,
    prelude::*, right_click_menu,
};

use crate::project_info::{
    GitHead, ProjectInfo, ProjectInfoStore, render_project_icon, workspace_icon,
};
use crate::project_switcher::compact_path;
use crate::{NewThread, OpenFolder, OpenSettings};

/// How often relative activity times ("5m") are re-rendered.
const ACTIVITY_REFRESH_INTERVAL: Duration = Duration::from_secs(30);
const CARD_HEIGHT: Pixels = px(78.);
pub(crate) const DETAILS_DELAY: Duration = Duration::from_millis(500);
pub const SIDEBAR_WIDTH: Pixels = px(290.);
const RENAME_KEY_CONTEXT: &str = "SidebarRename";
const SEARCH_KEY_CONTEXT: &str = "SidebarSearch";
pub(crate) const ARCHIVED_ROW_HEIGHT: Pixels = px(36.);
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
    OpenThread(ThreadKey),
    OpenProjectSettings(ProjectKey),
}

/// An element id for a thread's row, unique across machines.
fn thread_element_id(prefix: &str, key: ThreadKey) -> ElementId {
    ElementId::Name(format!("{prefix}-{}-{}", key.machine.slug(), key.thread.0).into())
}

/// Where a thread works, for its card, details and menu.
struct ThreadCheckout {
    folder: PathBuf,
    workspace: Option<Workspace>,
    head: Option<GitHead>,
}

impl ThreadCheckout {
    fn branch(&self) -> Option<String> {
        self.head
            .as_ref()
            .map(|head| head.branch.clone())
            .or_else(|| self.workspace.as_ref()?.branch.clone())
    }
}

/// The shelves of one-line rows under the thread cards.
#[derive(Clone, Copy, PartialEq)]
enum Shelf {
    Shells,
    Workspaces,
    Archived,
}

#[derive(Clone, Copy)]
enum PastureAction {
    Sync,
    BringBack,
}

/// The thread list, modeled on t3code's sidebar: active threads as cards and archived threads
/// in a collapsible shelf at the bottom (t3code's "Settled" shelf).
pub struct Sidebar {
    machines: Entity<Machines>,
    project_info: Entity<ProjectInfoStore>,
    active_thread: Option<ThreadKey>,
    /// The open draft's row as it was when the draft was opened. Like t3code, the row doesn't
    /// repaint while you type in it, and a draft never left has none.
    frozen_draft: Option<(ThreadKey, Thread)>,
    search: Entity<TextInput>,
    /// The highlighted search result, which Enter opens.
    search_index: usize,
    search_scroll: ScrollHandle,
    archived_shown: usize,
    /// Whether the Shells shelf is open. This window's alone.
    shells_expanded: bool,
    /// The thread whose details popover is showing, after hovering it for a moment.
    details_thread: Option<ThreadKey>,
    /// A popover waiting out the hover delay, and the thread it's for.
    details_delay: Option<(ThreadKey, Task<()>)>,
    /// The row under the mouse.
    hovered_thread: Option<ThreadKey>,
    renaming_thread: Option<ThreadKey>,
    rename_input: Entity<TextInput>,
    _rename_blur: Option<Subscription>,
    _subscriptions: Vec<Subscription>,
    _activity_refresh: Task<()>,
}

impl EventEmitter<SidebarEvent> for Sidebar {}

impl Sidebar {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let machines = Machines::global(cx);
        let project_info = ProjectInfoStore::global(cx);
        let search = cx.new(|cx| TextInput::new("Search…", cx));
        let rename_input = cx.new(|cx| TextInput::new("Thread title", cx));
        let subscriptions = vec![
            cx.subscribe(&rename_input, |this, _, _: &TextInputEvent, cx| {
                this.apply_rename(cx)
            }),
            cx.observe(&machines, |_, _, cx| cx.notify()),
            cx.observe(&project_info, |_, _, cx| cx.notify()),
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
        Self {
            machines,
            project_info,
            active_thread: None,
            frozen_draft: None,
            search,
            search_index: 0,
            search_scroll: ScrollHandle::new(),
            archived_shown: ARCHIVED_INITIAL_COUNT,
            shells_expanded: true,
            details_thread: None,
            details_delay: None,
            hovered_thread: None,
            renaming_thread: None,
            rename_input,
            _rename_blur: None,
            _subscriptions: subscriptions,
            _activity_refresh: activity_refresh,
        }
    }

    pub fn set_active_thread(&mut self, thread: Option<ThreadKey>, cx: &mut Context<Self>) {
        if self.active_thread != thread {
            self.frozen_draft = thread.and_then(|key| {
                let store = self.store(key.machine, cx)?;
                let thread = store.read(cx).thread(key.thread)?;
                (thread.is_draft && thread.unsent_text.is_some()).then(|| (key, thread.clone()))
            });
        }
        self.active_thread = thread;
        cx.notify();
    }

    fn store(&self, machine: MachineId, cx: &App) -> Option<Entity<ProjectStore>> {
        self.machines.read(cx).projects(machine, cx)
    }

    fn project_info<'a>(
        &self,
        machine: MachineId,
        project: &Project,
        cx: &'a App,
    ) -> Option<&'a ProjectInfo> {
        self.project_info.read(cx).info(machine, project.id)
    }

    fn start_renaming(
        &mut self,
        thread_id: ThreadKey,
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

    /// Renames as you type; an empty title goes back to the automatic one.
    fn apply_rename(&mut self, cx: &mut Context<Self>) {
        let Some(key) = self.renaming_thread else {
            return;
        };
        let Some(store) = self.store(key.machine, cx) else {
            return;
        };
        let title = self.rename_input.read(cx).text().trim().to_string();
        let is_unchanged = store
            .read(cx)
            .thread(key.thread)
            .is_none_or(|thread| thread.title == title);
        if is_unchanged {
            return;
        }
        store.update(cx, |store, cx| {
            store.set_custom_title(key.thread, title, cx)
        });
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

    /// t3code's search results: every matching thread, active, then shells, then Workspaces
    /// threads, then archived, in one list.
    fn search_results(&self, cx: &App) -> Vec<(MachineId, Thread)> {
        let query = self.search_query(cx);
        let machines = self.machines.read(cx);
        machines
            .active_threads(cx)
            .into_iter()
            .chain(machines.shell_threads(cx))
            .chain(machines.workspaces_threads(cx))
            .chain(machines.archived_threads(cx))
            .filter(|(_, thread)| matches_query(thread, &query))
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
    fn open_search_result(&mut self, thread_id: ThreadKey, cx: &mut Context<Self>) {
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
                if let Some((machine, thread)) = result {
                    this.open_search_result(
                        ThreadKey {
                            machine,
                            thread: thread.id,
                        },
                        cx,
                    );
                }
            }))
            .on_action(cx.listener(|this, _: &menu::Cancel, _, cx| {
                if this.search_query(cx).is_empty() {
                    cx.propagate();
                } else {
                    this.search.update(cx, |search, cx| search.set_text("", cx));
                }
            }))
            .h(TOOLBAR_HEIGHT)
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

    /// Clicking opens the thread; double-clicking renames it, as in t3code.
    fn thread_click_handler(
        &self,
        thread_id: ThreadKey,
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

    fn render_project_icon(
        &self,
        machine: MachineId,
        project: Option<&Project>,
        cx: &App,
    ) -> AnyElement {
        match project {
            Some(project) => render_project_icon(
                project,
                self.project_info(machine, project, cx),
                px(16.),
                cx,
            ),
            None => div().size_4().flex_none().into_any_element(),
        }
    }

    /// The project a thread's row shows: its own, or for a Workspaces thread the one its
    /// folder is in, if any.
    fn row_project(&self, machine: MachineId, thread: &Thread, cx: &App) -> Option<Project> {
        let store = self.store(machine, cx)?;
        let store = store.read(cx);
        let project_id = if thread.in_workspaces() {
            store.thread_project(thread.id)?
        } else {
            thread.project_id
        };
        store.project(project_id).cloned()
    }

    /// The row's project icon, or a folder's for a Workspaces thread outside every project.
    fn render_row_icon(
        &self,
        machine: MachineId,
        thread: &Thread,
        project: Option<&Project>,
        cx: &App,
    ) -> AnyElement {
        if project.is_none() && thread.in_workspaces() {
            return render_folder_icon();
        }
        self.render_project_icon(machine, project, cx)
    }

    fn thread_checkout(
        &self,
        machine: MachineId,
        thread: &Thread,
        cx: &App,
    ) -> Option<ThreadCheckout> {
        let store = self.store(machine, cx)?;
        let store = store.read(cx);
        let folder = store.thread_folder(thread.id)?;
        let workspace = store.thread_workspace(thread.id).cloned();
        let project_info = self.project_info.read(cx);
        let head = if thread.workspace.is_some() {
            project_info.workspace_head(machine, &folder).cloned()
        } else {
            project_info
                .info(machine, thread.project_id)
                .and_then(|info| info.git_head.clone())
        };
        Some(ThreadCheckout {
            folder,
            workspace,
            head,
        })
    }

    /// Syncs the thread's pasture, or brings its branch to the project, and says how it went.
    fn run_pasture_action(
        &mut self,
        key: ThreadKey,
        action: PastureAction,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(store) = self.store(key.machine, cx) else {
            return;
        };
        let store = store.read(cx);
        let Some(thread) = store.thread(key.thread) else {
            return;
        };
        let project_id = thread.project_id;
        let Some(folder) = store.thread_folder(key.thread) else {
            return;
        };
        let task = match action {
            PastureAction::Sync => store.sync_workspace(project_id, folder, cx),
            PastureAction::BringBack => store.bring_back_workspace(project_id, folder, cx),
        };
        let title = match action {
            PastureAction::Sync => "Sync from Project",
            PastureAction::BringBack => "Bring Branch to Project",
        };
        cx.spawn_in(window, async move |_, cx| {
            let (level, detail) = match task.await {
                Ok(message) => (PromptLevel::Info, message),
                Err(error) => (PromptLevel::Critical, format!("{error:#}")),
            };
            let answer =
                cx.update(|window, cx| window.prompt(level, title, Some(&detail), &["OK"], cx));
            if let Ok(answer) = answer {
                answer.await.ok();
            }
        })
        .detach();
    }

    /// Rename, Archive or Unarchive, the pasture's actions, Project Settings, and Delete, each
    /// with its icon. A Workspaces thread's has Rename, Move to Agents and Delete.
    fn thread_menu(
        &self,
        machine: MachineId,
        thread: &Thread,
        is_archived: bool,
        // A terminal thread renames and deletes, and can add the folder it's in as a project.
        // It isn't archived, nor tied to the project it started in.
        is_terminal: bool,
        cx: &mut Context<Self>,
    ) -> impl Fn(&mut Window, &mut App) -> Entity<ContextMenu> + 'static {
        let sidebar = cx.entity().downgrade();
        let thread_id = ThreadKey {
            machine,
            thread: thread.id,
        };
        let project_id = ProjectKey {
            machine,
            project: thread.project_id,
        };
        let title = SharedString::from(thread.title.clone());
        let in_workspaces = thread.in_workspaces();
        let checkout = self.thread_checkout(machine, thread, cx);
        let is_pasture = checkout
            .as_ref()
            .and_then(|checkout| checkout.workspace.as_ref())
            .is_some_and(|workspace| workspace.kind == WorkspaceKind::Pasture);
        // Where a terminal is, when that's in no project yet.
        let new_project = is_terminal
            .then(|| {
                let store = self.store(machine, cx)?;
                let store = store.read(cx);
                let folder = store.terminal_folder(thread.id)?;
                project_at(store.projects(), &folder.path)
                    .is_none()
                    .then(|| folder.path.clone())
            })
            .flatten();
        move |window, cx| {
            let sidebar = sidebar.clone();
            let title = title.clone();
            let new_project = new_project.clone();
            ContextMenu::build(window, cx, move |menu, _, _| {
                let pasture_action = |action: PastureAction| {
                    let sidebar = sidebar.clone();
                    move |window: &mut Window, cx: &mut App| {
                        sidebar
                            .update(cx, |sidebar, cx| {
                                sidebar.run_pasture_action(thread_id, action, window, cx)
                            })
                            .ok();
                    }
                };
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
                                let Some(store) = sidebar.store(machine, cx) else {
                                    return;
                                };
                                store.update(cx, |store, cx| {
                                    if is_archived {
                                        store.unarchive_thread(thread_id.thread, cx)
                                    } else {
                                        store.archive_thread(thread_id.thread, cx)
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
                            .update(cx, |_, cx| {
                                cx.emit(SidebarEvent::OpenProjectSettings(project_id));
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
                let menu = menu.item(
                    ContextMenuEntry::new("Rename")
                        .icon(IconName::Pencil)
                        .icon_color(Color::Muted)
                        .handler(rename),
                );
                if in_workspaces {
                    let sidebar = sidebar.clone();
                    return menu
                        .item(
                            ContextMenuEntry::new("Move to Agents")
                                .icon(IconName::ArrowRight)
                                .icon_color(Color::Muted)
                                .handler(move |window, cx| {
                                    sidebar
                                        .update(cx, |sidebar, cx| {
                                            sidebar.move_to_agents(thread_id, window, cx)
                                        })
                                        .ok();
                                }),
                        )
                        .separator()
                        .item(
                            ContextMenuEntry::new("Delete…")
                                .icon(IconName::Trash)
                                .icon_color(Color::Muted)
                                .handler(delete),
                        );
                }
                if is_terminal {
                    let add_project = new_project.clone().map(|path| {
                        let sidebar = sidebar.clone();
                        move |_: &mut Window, cx: &mut App| {
                            sidebar
                                .update(cx, |sidebar, cx| {
                                    sidebar.add_project(machine, path.clone(), cx)
                                })
                                .ok();
                        }
                    });
                    return menu
                        .when_some(add_project, |menu, handler| {
                            menu.item(
                                ContextMenuEntry::new("Add Project")
                                    .icon(IconName::Plus)
                                    .icon_color(Color::Muted)
                                    .handler(handler),
                            )
                        })
                        .separator()
                        .item(
                            ContextMenuEntry::new("Delete…")
                                .icon(IconName::Trash)
                                .icon_color(Color::Muted)
                                .handler(delete),
                        );
                }
                menu.item(
                    ContextMenuEntry::new(if is_archived { "Unarchive" } else { "Archive" })
                        .icon(if is_archived {
                            IconName::Undo
                        } else {
                            IconName::Archive
                        })
                        .icon_color(Color::Muted)
                        .handler(toggle_archived),
                )
                .when(is_pasture, |menu| {
                    menu.separator()
                        .item(
                            ContextMenuEntry::new("Sync from Project")
                                .icon(IconName::ArrowCircle)
                                .icon_color(Color::Muted)
                                .handler(pasture_action(PastureAction::Sync)),
                        )
                        .item(
                            ContextMenuEntry::new("Bring Branch to Project")
                                .icon(IconName::GitBranch)
                                .icon_color(Color::Muted)
                                .handler(pasture_action(PastureAction::BringBack)),
                        )
                        .separator()
                })
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

    /// Makes a Workspaces thread one of the project its folder is in. Outside every project,
    /// it asks to add the folder as one first.
    fn move_to_agents(&mut self, key: ThreadKey, window: &mut Window, cx: &mut Context<Self>) {
        let Some(store) = self.store(key.machine, cx) else {
            return;
        };
        let new_project = {
            let store = store.read(cx);
            store
                .thread_project(key.thread)
                .is_none()
                .then(|| store.thread_folder(key.thread))
                .flatten()
        };
        let answer = new_project.map(|folder| {
            window.prompt(
                PromptLevel::Info,
                &format!("Add “{}” as a project?", compact_path(&folder)),
                Some("Threads in the Agents list belong to a project."),
                &["Add Project", "Cancel"],
                cx,
            )
        });
        cx.spawn_in(window, async move |_, cx| {
            if let Some(answer) = answer
                && answer.await != Ok(0)
            {
                return;
            }
            let moved = store.update(cx, |store, cx| store.move_to_agents(key.thread, cx));
            if let Err(error) = moved.await {
                let detail = format!("{error:#}");
                let answer = cx.update(|window, cx| {
                    window.prompt(
                        PromptLevel::Critical,
                        "Couldn't move the thread",
                        Some(&detail),
                        &["OK"],
                        cx,
                    )
                });
                if let Ok(answer) = answer {
                    answer.await.ok();
                }
            }
        })
        .detach();
    }

    /// Adds a folder on the machine as a project.
    fn add_project(&mut self, machine: MachineId, path: PathBuf, cx: &mut Context<Self>) {
        let Some(store) = self.store(machine, cx) else {
            return;
        };
        let added = store.update(cx, |store, cx| store.add_project(path, cx));
        cx.background_spawn(async move {
            if let Err(error) = added.await {
                log::error!("failed to add the project: {error:#}");
            }
        })
        .detach();
    }

    /// Shows the hovered thread's details after a moment, like t3code's row tooltip.
    fn thread_hovered(&mut self, thread_id: ThreadKey, hovered: bool, cx: &mut Context<Self>) {
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
        machine: MachineId,
        thread: &Thread,
        project: Option<&Project>,
        cx: &App,
    ) -> ThreadDetails {
        let machines = self.machines.read(cx);
        let registry = machines
            .client(machine, cx)
            .map(|client| client.read(cx).registry().read(cx));
        let agent = thread.agent_id.as_ref().map(|agent_id| {
            let agent_id = AgentId::new(agent_id.clone());
            let registry_agent = registry.and_then(|registry| registry.agent(&agent_id));
            let agent_name = registry_agent
                .map(|agent| agent.name().clone())
                .unwrap_or_else(|| agent_id.0.clone());
            let label = match &thread.model {
                Some(model) => format!("{model} · {agent_name}").into(),
                None => agent_name,
            };
            (agent_icon(&agent_id, cx), label)
        });
        let store = machines.projects(machine, cx);
        let store = store.as_ref().map(|store| store.read(cx));
        // A terminal's agent CLI, by name.
        let agent = agent.or_else(|| {
            let name = store?.terminal_agent(thread.id)?;
            Some((None, SharedString::from(name.to_string())))
        });
        let checkout = self.thread_checkout(machine, thread, cx);
        // A terminal is described by where it is now: that folder's project and branch, or its
        // path outside git, and its worktree or pasture only while it's still in there.
        let folder = store.and_then(|store| store.terminal_folder(thread.id));
        let (project, branch, path, checkout) = match (folder, store) {
            (Some(folder), Some(store)) => (
                project_at(store.projects(), &folder.path),
                folder.branch.clone(),
                (!folder.is_repository)
                    .then(|| folder_branch_label(folder, None))
                    .flatten(),
                checkout.filter(|checkout| folder.path.starts_with(&checkout.folder)),
            ),
            _ => (
                project,
                checkout.as_ref().and_then(ThreadCheckout::branch),
                None,
                checkout,
            ),
        };
        ThreadDetails {
            title: thread.title.clone().into(),
            project: project.map(|project| {
                (
                    project.clone(),
                    self.project_info(machine, project, cx).cloned(),
                )
            }),
            machine: (
                machines.machine_icon(machine, cx),
                machines.label(machine, cx),
            ),
            branch: branch.map(SharedString::from),
            path: path.map(SharedString::from),
            workspace: checkout.and_then(|checkout| {
                let workspace = checkout.workspace?;
                Some((
                    workspace.kind,
                    describe_folder(workspace.kind, &checkout.folder),
                ))
            }),
            agent,
            contents: None,
        }
    }

    /// t3code's thread card: the project and status on top, then the title, then the branch with
    /// the agent's icon at the bottom right.
    fn render_thread_card(
        &self,
        store: &Entity<ProjectStore>,
        thread: Thread,
        project: Option<Project>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let colors = cx.theme().colors();
        let hover_background = colors.ghost_element_hover;
        let selected_background = colors.ghost_element_selected;
        let machine = store.read(cx).machine();
        let thread_id = ThreadKey {
            machine,
            thread: thread.id,
        };
        let is_active = self.active_thread == Some(thread_id);
        let is_renaming = self.renaming_thread == Some(thread_id);
        // The open thread's composer shows its text already.
        let has_unsent_text = thread.unsent_text.is_some() && !is_active;
        let thread_status = store.read(cx).thread_status(thread.id);
        let icon = thread_agent_icon(&thread, cx);
        // Which machine it runs on, just before the agent.
        let machine_icon = Icon::new(self.machines.read(cx).machine_icon(machine, cx));
        // A terminal thread is described by where it is now, which may not be where it
        // started: that folder's project, or the folder itself outside every project.
        let folder = store.read(cx).terminal_folder(thread.id).cloned();
        let (project, folder_name) = match &folder {
            Some(folder) => match project_at(store.read(cx).projects(), &folder.path) {
                Some(project) => (Some(project.clone()), None),
                None => (None, Some(folder_name(&folder.path))),
            },
            None => (project, None),
        };
        let details = self.thread_details(machine, &thread, project.as_ref(), cx);
        let checkout = self.thread_checkout(machine, &thread, cx);
        let machines = self.machines.read(cx);
        // With one project selected, every card would repeat it, so the project line goes and
        // the status moves next to the title.
        let shows_all_projects = machines.scope(cx) == Scope::All;
        let is_offline = !machines.is_online(machine, cx);
        let machine_label = (machine != MachineId::Local).then(|| {
            (
                machines.machine_icon(machine, cx),
                machines.label(machine, cx),
            )
        });
        let faint_text = cx.theme().colors().text_muted.opacity(0.4);
        let time = thread
            .last_activity_at
            .map(|time| format_relative_time(time, SystemTime::now()));
        let group_name =
            SharedString::from(format!("thread-card-{}-{}", machine.slug(), thread.id.0));
        let title = SharedString::from(thread.title.clone());
        // A terminal thread is a card while an agent CLI runs in it, named after the agent
        // unless the user renamed it.
        let display_title = store
            .read(cx)
            .terminal_agent(thread.id)
            .filter(|_| !thread.has_custom_title)
            .map(SharedString::from)
            .unwrap_or_else(|| title.clone());
        // The project line names the repository already; without it, a title that isn't the
        // repository's name says which one the branch is in.
        let branch_title = (!shows_all_projects).then_some(display_title.as_ref());
        let (branch, checkout) = match &folder {
            Some(folder) => (
                folder_branch_label(folder, branch_title),
                checkout.filter(|checkout| folder.path.starts_with(&checkout.folder)),
            ),
            None => (checkout.as_ref().and_then(ThreadCheckout::branch), checkout),
        };
        let subthreads = {
            let store = store.read(cx);
            let subthreads = store.subthreads(thread.id);
            let running = subthreads
                .iter()
                .filter(|thread| {
                    thread
                        .task
                        .as_ref()
                        .is_some_and(|task| task.outcome.is_none())
                })
                .count();
            (subthreads.len(), running)
        };
        let started_by = thread
            .created_by
            .map(|creator| format!("Started by {}", store.read(cx).describe_creator(creator)));

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
        let store = store.clone();
        // Like t3code's Settle button: muted text that brightens under the mouse, with no fill
        // of its own over the card's hover background.
        let muted_text = cx.theme().colors().text_muted;
        let bright_text = cx.theme().colors().text;
        let button_group =
            SharedString::from(format!("archive-button-{}-{}", machine.slug(), thread.id.0));
        let archive_button = h_flex()
            .id(thread_element_id("archive-thread", thread_id))
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
            .on_click({
                let store = store.clone();
                move |_, _, cx| {
                    cx.stop_propagation();
                    store.update(cx, |store, cx| store.archive_thread(thread_id.thread, cx));
                }
            });
        let discard_button = has_unsent_text.then(|| {
            render_discard_draft_button(thread_id, store.clone(), cx)
                // The details would sit beside the button, so they give way to it.
                .on_hover(cx.listener(move |this, hovered, _, cx| {
                    if *hovered {
                        this.hide_details(cx);
                    } else if this.hovered_thread == Some(thread_id) {
                        this.thread_hovered(thread_id, true, cx);
                    }
                }))
        });

        // Terminals aren't archived: a shell or an agent CLI is deleted when done with.
        let is_archivable = thread.terminal.is_none();
        let has_hover_buttons = !is_renaming && (is_archivable || has_unsent_text);
        // The status yields to the Archive and Discard buttons on hover.
        let status_slot = div()
            .flex_none()
            .when(has_hover_buttons, |this| {
                this.group_hover(group_name.clone(), |this| this.invisible())
            })
            .child(status);
        let hover_buttons = has_hover_buttons.then(|| {
            // Centered on its line, like t3code's Settle button.
            h_flex()
                .absolute()
                .top_0()
                .bottom_0()
                .right_0()
                .visible_on_hover(group_name.clone())
                .children(discard_button)
                .when(is_archivable, |this| this.child(archive_button))
        });
        // The same pen as the draft rows, so both kinds of unsent work read the same way.
        let unsent_marker = has_unsent_text.then(|| {
            div()
                .id(thread_element_id("unsent-text", thread_id))
                .debug_selector(|| format!("unsent-text-{}", thread_id.thread.0))
                .flex_none()
                .tooltip(Tooltip::text("Unsent draft"))
                .child(render_draft_pen())
        });
        let title_element = if is_renaming {
            self.render_rename_input(cx)
        } else {
            div()
                .flex_1()
                .min_w_0()
                .child(
                    Label::new(display_title)
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
                .children(unsent_marker)
                .child(match &folder_name {
                    Some(_) => render_folder_icon(),
                    None => self.render_project_icon(machine, project.as_ref(), cx),
                })
                .child(
                    h_flex()
                        .flex_1()
                        .min_w_0()
                        .gap_1()
                        .children(
                            project
                                .as_ref()
                                .map(|project| project.name())
                                .or(folder_name.clone())
                                .map(|name| {
                                    Label::new(name)
                                        .size(LabelSize::Small)
                                        .color(Color::Muted)
                                        .truncate()
                                }),
                        )
                        .children(
                            machine_label
                                .map(|(icon, label)| render_machine_tag(icon, label, is_offline)),
                        ),
                )
                .child(status_slot)
                .children(hover_buttons);
            let title_line = h_flex().mt_1().min_w_0().child(title_element);
            (Some(project_line), title_line)
        } else {
            // A project combined across machines keeps telling its threads apart.
            let title_line = h_flex()
                .relative()
                .min_w_0()
                .gap_1p5()
                .children(unsent_marker)
                .child(title_element)
                .children(
                    machine_label.map(|(icon, label)| render_machine_tag(icon, label, is_offline)),
                )
                .child(status_slot)
                .children(hover_buttons);
            (None, title_line)
        };

        let card =
            v_flex()
                .id(thread_element_id("thread-card", thread_id))
                .debug_selector(|| format!("thread-card-{}", thread_id.thread.0))
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
                // Readable while its machine is unreachable, but plainly not live.
                .when(is_offline, |card| card.opacity(0.5))
                .children(project_line)
                .child(title_line)
                .child(
                    h_flex()
                        .mt_0p5()
                        .min_w_0()
                        .gap_1p5()
                        .child(
                            h_flex()
                                .flex_1()
                                .min_w_0()
                                .gap_1()
                                .children(checkout.as_ref().and_then(|checkout| {
                                    render_checkout_marker(thread_id, checkout, faint_text)
                                }))
                                .children(branch.map(|branch| {
                                    Label::new(branch)
                                        .size(LabelSize::Small)
                                        .color(Color::Custom(faint_text))
                                        .truncate_middle()
                                })),
                        )
                        .when(subthreads.0 > 0, |this| {
                            let (count, running) = subthreads;
                            let tooltip = match (count, running) {
                                (1, 0) => "1 agent".to_string(),
                                (count, 0) => format!("{count} agents"),
                                (count, running) => format!("{count} agents, {running} running"),
                            };
                            let color = if running > 0 {
                                Color::Accent
                            } else {
                                Color::Custom(faint_text)
                            };
                            this.child(
                                h_flex()
                                    .id(thread_element_id("thread-agents", thread_id))
                                    .flex_none()
                                    .gap_0p5()
                                    .tooltip(Tooltip::text(tooltip))
                                    .child(
                                        Icon::new(IconName::UserGroup)
                                            .size(IconSize::XSmall)
                                            .color(color),
                                    )
                                    .child(
                                        Label::new(count.to_string())
                                            .size(LabelSize::XSmall)
                                            .color(color),
                                    ),
                            )
                        })
                        .when_some(started_by, |this, started_by| {
                            this.child(
                                div()
                                    .id(thread_element_id("thread-started-by", thread_id))
                                    .flex_none()
                                    .tooltip(Tooltip::text(started_by))
                                    .child(
                                        Icon::new(IconName::Sparkle)
                                            .size(IconSize::XSmall)
                                            .color(Color::Custom(faint_text)),
                                    ),
                            )
                        })
                        .child(
                            div()
                                .flex_none()
                                .opacity(0.6)
                                .child(machine_icon.size(IconSize::Small).color(Color::Muted)),
                        )
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
        let menu = self.thread_menu(machine, &thread, false, thread.terminal.is_some(), cx);
        right_click_menu(thread_element_id("thread-menu", thread_id))
            .trigger(move |is_menu_open, _, _| {
                div()
                    .relative()
                    .child(div().py_0p5().child(card))
                    .when(!is_menu_open, |this| this.children(details_popover))
            })
            .menu(menu)
            .into_any_element()
    }

    /// t3code's draft row: a new thread with something typed and nothing sent, as its project
    /// and the first line of the text, on a warning tint.
    fn render_draft_row(
        &self,
        store: &Entity<ProjectStore>,
        thread: Thread,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let machine = store.read(cx).machine();
        let thread_id = ThreadKey {
            machine,
            thread: thread.id,
        };
        let is_active = self.active_thread == Some(thread_id);
        let project = store.read(cx).project(thread.project_id).cloned();
        let machines = self.machines.read(cx);
        let is_offline = !machines.is_online(machine, cx);
        let machine_label = (machine != MachineId::Local).then(|| {
            (
                machines.machine_icon(machine, cx),
                machines.label(machine, cx),
            )
        });
        let preview = thread
            .unsent_text
            .as_deref()
            .unwrap_or_default()
            .trim()
            .lines()
            .next()
            .unwrap_or_default()
            .to_string();
        let tint = Color::Warning.color(cx);
        let selected_background = cx.theme().colors().ghost_element_selected;
        let group_name =
            SharedString::from(format!("draft-row-{}-{}", machine.slug(), thread.id.0));
        let row = v_flex()
            .id(thread_element_id("draft-row", thread_id))
            .debug_selector(|| format!("draft-row-{}", thread_id.thread.0))
            .group(group_name.clone())
            .w_full()
            .h(CARD_HEIGHT)
            .px_2p5()
            .py_2()
            .rounded_md()
            .cursor_pointer()
            .map(|row| {
                if is_active {
                    row.bg(selected_background)
                } else {
                    row.bg(tint.opacity(0.04))
                        .hover(|row| row.bg(tint.opacity(0.08)))
                }
            })
            .when(is_offline, |row| row.opacity(0.5))
            .on_click(cx.listener(move |_, _, _, cx| cx.emit(SidebarEvent::OpenThread(thread_id))))
            .child(
                h_flex()
                    .h_5()
                    .min_w_0()
                    .gap_1p5()
                    .child(render_draft_pen())
                    .child(self.render_project_icon(machine, project.as_ref(), cx))
                    .child(
                        h_flex()
                            .flex_1()
                            .min_w_0()
                            .gap_1()
                            .children(project.as_ref().map(|project| {
                                Label::new(project.name())
                                    .size(LabelSize::Small)
                                    .color(Color::Muted)
                                    .truncate()
                            }))
                            .children(
                                machine_label.map(|(icon, label)| {
                                    render_machine_tag(icon, label, is_offline)
                                }),
                            ),
                    )
                    .child(
                        h_flex()
                            .h_full()
                            .flex_none()
                            .visible_on_hover(group_name)
                            .child(render_discard_draft_button(thread_id, store.clone(), cx)),
                    ),
            )
            .child(
                div()
                    .mt_0p5()
                    .min_w_0()
                    .child(Label::new(preview).weight(FontWeight::MEDIUM).truncate()),
            );
        div().py_0p5().child(row).into_any_element()
    }

    /// t3code's shelf header: a label, a rule, and a chevron.
    fn render_shelf_header(
        id: &'static str,
        label: &'static str,
        count: usize,
        is_expanded: bool,
        on_toggle: impl Fn(&mut Self, &mut Context<Self>) + 'static,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let rule_color = cx.theme().colors().border_variant;
        h_flex()
            .id(id)
            .debug_selector(|| id.into())
            .h_8()
            .mx_0p5()
            .px_2()
            .gap_2()
            .cursor_pointer()
            .child(
                Label::new(if is_expanded {
                    label.to_string()
                } else {
                    format!("{label} ({count})")
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
            .on_click(cx.listener(move |this, _, _, cx| on_toggle(this, cx)))
            .into_any_element()
    }

    /// t3code's slim row for parked threads: the project's icon, dimmed until hovered, and for
    /// an archived thread a way back on hover. A shell's row adds where it works, and shows
    /// what runs in it in place of its last activity. A Workspaces thread's is one line, like
    /// an archived one's, with the icon of the project its folder is in.
    fn render_slim_row(
        &self,
        store: &Entity<ProjectStore>,
        thread: Thread,
        shelf: Shelf,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let is_archived = shelf == Shelf::Archived;
        let is_shell = shelf == Shelf::Shells;
        let colors = cx.theme().colors();
        let hover_background = colors.ghost_element_hover;
        let selected_background = colors.ghost_element_selected;
        let machine = store.read(cx).machine();
        let thread_id = ThreadKey {
            machine,
            thread: thread.id,
        };
        let is_active = self.active_thread == Some(thread_id);
        let is_renaming = self.renaming_thread == Some(thread_id);
        let is_offline = !self.machines.read(cx).is_online(machine, cx);
        let project = self.row_project(machine, &thread, cx);
        // Where a shell is now, once its server has said.
        let folder = is_shell
            .then(|| store.read(cx).terminal_folder(thread.id).cloned())
            .flatten();
        let project_icon = match &folder {
            // The project the shell is in, or a plain folder outside every project.
            Some(folder) => match project_at(store.read(cx).projects(), &folder.path) {
                Some(project) => self.render_project_icon(machine, Some(project), cx),
                None => render_folder_icon(),
            },
            None => self.render_row_icon(machine, &thread, project.as_ref(), cx),
        };
        let details = self.thread_details(machine, &thread, project.as_ref(), cx);
        let time = if is_archived {
            thread.archived_at
        } else {
            thread.last_activity_at.or(thread.created_at)
        }
        .map(|time| format_relative_time(time, SystemTime::now()));
        let prefix = match shelf {
            Shelf::Shells => "shell",
            Shelf::Workspaces => "workspaces",
            Shelf::Archived => "archived",
        };
        let machine_icon = Icon::new(self.machines.read(cx).machine_icon(machine, cx));
        let running = is_shell
            .then(|| {
                store
                    .read(cx)
                    .terminal_command(thread.id)
                    .map(SharedString::from)
            })
            .flatten();
        let faint_text = colors.text_muted.opacity(0.4);
        let checkout = is_shell
            .then(|| self.thread_checkout(machine, &thread, cx))
            .flatten();
        // The branch where the shell is, and the worktree or pasture marker while it's in its
        // own. Before its server says where it is, the checkout it started in.
        let (branch, checkout) = match &folder {
            Some(folder) => (
                folder_branch_label(folder, Some(&thread.title)),
                checkout.filter(|checkout| folder.path.starts_with(&checkout.folder)),
            ),
            None => (checkout.as_ref().and_then(ThreadCheckout::branch), checkout),
        };
        let detail_line =
            is_shell.then(|| {
                // Under the title, past the icon and the gap.
                h_flex()
                    .pl(px(26.))
                    .min_w_0()
                    .gap_1()
                    .children(checkout.as_ref().and_then(|checkout| {
                        render_checkout_marker(thread_id, checkout, faint_text)
                    }))
                    .children(branch.map(|branch| {
                        Label::new(branch)
                            .size(LabelSize::Small)
                            .color(Color::Custom(faint_text))
                            .truncate_middle()
                    }))
                    .child(div().flex_1())
                    .children(running.clone().map(|command| {
                        div().flex_none().max_w(px(100.)).child(
                            Label::new(command)
                                .size(LabelSize::Small)
                                .color(Color::Custom(faint_text))
                                .truncate(),
                        )
                    }))
                    .child(
                        div()
                            .flex_none()
                            .opacity(0.6)
                            .child(machine_icon.size(IconSize::Small).color(Color::Muted)),
                    )
            });
        let group_name =
            SharedString::from(format!("{prefix}-row-{}-{}", machine.slug(), thread.id.0));
        let title = SharedString::from(thread.title.clone());
        let store = store.clone();

        let main_line = h_flex()
            .relative()
            .when(!is_shell, |line| line.h_full())
            .when(is_shell, |line| line.h_6())
            .gap_2p5()
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
            .when(running.is_some() && !is_renaming, |row| {
                row.child(
                    h_flex()
                        .flex_none()
                        .gap_1()
                        .child(div().size_1p5().rounded_full().bg(Color::Accent.color(cx)))
                        .child(
                            Label::new("Running")
                                .size(LabelSize::Small)
                                .weight(FontWeight::MEDIUM)
                                .color(Color::Accent),
                        ),
                )
            })
            .when_some(
                time.filter(|_| !is_renaming && running.is_none()),
                |row, time| {
                    row.child(
                        div()
                            .flex_none()
                            .when(is_archived, |this| {
                                this.group_hover(group_name.clone(), |this| this.invisible())
                            })
                            .child(Label::new(time).size(LabelSize::Small).color(Color::Muted)),
                    )
                },
            )
            .when(is_archived && !is_renaming, |row| {
                row.child(
                    div()
                        .absolute()
                        .right_1()
                        .visible_on_hover(group_name.clone())
                        .child(
                            IconButton::new(
                                thread_element_id("unarchive-thread", thread_id),
                                IconName::Undo,
                            )
                            .icon_size(IconSize::Small)
                            .icon_color(Color::Muted)
                            .tooltip(Tooltip::text("Unarchive Thread"))
                            .on_click(move |_, _, cx| {
                                cx.stop_propagation();
                                store.update(cx, |store, cx| {
                                    store.unarchive_thread(thread_id.thread, cx)
                                });
                            }),
                        ),
                )
            });
        let row =
            v_flex()
                .id(thread_element_id(&format!("{prefix}-thread"), thread_id))
                .debug_selector(|| format!("{prefix}-row-{}", thread_id.thread.0))
                .group(group_name)
                .on_hover(cx.listener(move |this, hovered, _, cx| {
                    this.thread_hovered(thread_id, *hovered, cx)
                }))
                .on_any_mouse_down(cx.listener(|this, _, _, cx| this.hide_details(cx)))
                .relative()
                .when(!is_shell, |row| row.h(ARCHIVED_ROW_HEIGHT))
                .when(is_shell, |row| row.py_1p5())
                .w_full()
                .px_2p5()
                .rounded_md()
                .when(is_active, |row| row.bg(selected_background))
                .when(!is_renaming, |row| {
                    row.cursor_pointer()
                        .hover(|row| row.bg(hover_background))
                        .on_click(self.thread_click_handler(thread_id, title.clone(), cx))
                })
                .when(is_offline, |row| row.opacity(0.5))
                .child(main_line)
                .children(detail_line);

        // The details popover stays hidden while the thread's menu is open.
        let details_popover =
            (self.details_thread == Some(thread_id)).then(|| render_details_popover(details, cx));
        let menu = self.thread_menu(machine, &thread, is_archived, is_shell, cx);
        right_click_menu(thread_element_id(
            &format!("{prefix}-thread-menu"),
            thread_id,
        ))
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
        thread_id: ThreadKey,
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
        let Some(store) = self.store(thread_id.machine, cx) else {
            return;
        };
        cx.spawn(async move |_, cx| {
            if answer.await == Ok(0) {
                store.update(cx, |store, cx| store.delete_thread(thread_id.thread, cx));
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
        machine: MachineId,
        thread: Thread,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let colors = cx.theme().colors();
        let selected_background = colors.ghost_element_selected;
        let hover_background = colors.ghost_element_hover;
        let thread_id = ThreadKey {
            machine,
            thread: thread.id,
        };
        let project = self.row_project(machine, &thread, cx);
        let icon = self.render_row_icon(machine, &thread, project.as_ref(), cx);
        let details = self.thread_details(machine, &thread, project.as_ref(), cx);
        let is_highlighted = index == self.search_index;
        let is_active = self.active_thread == Some(thread_id);
        let time = thread
            .last_activity_at
            .map(|time| format_relative_time(time, SystemTime::now()));
        let list = if thread.archived_at.is_some() {
            Some("Archived")
        } else if thread.in_workspaces() {
            Some("Workspaces")
        } else {
            None
        };
        let faint_text = colors.text_muted.opacity(0.4);
        div()
            .relative()
            .child(
                h_flex()
                    .id(thread_element_id("search-result", thread_id))
                    .debug_selector(|| format!("search-result-{}", thread_id.thread.0))
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
                    .child(icon)
                    .child(
                        div().flex_1().min_w_0().child(
                            Label::new(thread.title)
                                .truncate()
                                .when(!is_highlighted && !is_active, |label| {
                                    label.color(Color::Muted)
                                }),
                        ),
                    )
                    .children(list.map(|list| {
                        div()
                            .debug_selector(|| format!("search-list-{list}-{}", thread_id.thread.0))
                            .child(
                                Label::new(list)
                                    .size(LabelSize::Small)
                                    .color(Color::Custom(faint_text)),
                            )
                    }))
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
            .map(|(index, (machine, thread))| self.render_search_result(index, machine, thread, cx))
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
        let machines = self.machines.read(cx);
        let drafts = machines.typed_drafts(cx);
        let active = machines.active_threads(cx);
        let archived = machines.archived_threads(cx);
        let is_archived_expanded = machines.archived_expanded(cx);

        // t3code's draft block: interrupted new threads, one click away above the rest.
        let mut draft_rows = Vec::with_capacity(drafts.len());
        for (machine, thread) in drafts {
            let key = ThreadKey {
                machine,
                thread: thread.id,
            };
            let thread = if self.active_thread == Some(key) {
                match &self.frozen_draft {
                    Some((frozen_key, frozen)) if *frozen_key == key => frozen.clone(),
                    _ => continue,
                }
            } else {
                thread
            };
            if let Some(store) = self.store(machine, cx) {
                draft_rows.push(self.render_draft_row(&store, thread, cx));
            }
        }
        if !draft_rows.is_empty() {
            draft_rows.push(
                div()
                    .mx_2p5()
                    .my_1p5()
                    .h_px()
                    .bg(cx.theme().colors().border_variant.opacity(0.6))
                    .into_any_element(),
            );
        }

        let mut rows = Vec::with_capacity(active.len());
        for (machine, thread) in active {
            let Some(store) = self.store(machine, cx) else {
                continue;
            };
            let project = store.read(cx).project(thread.project_id).cloned();
            rows.push(self.render_thread_card(&store, thread, project, cx));
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

        let mut shelf = Vec::new();
        // Terminal threads running no agent CLI. One that starts an agent becomes a thread
        // card, and comes back here when it ends.
        let shells = self.machines.read(cx).shell_threads(cx);
        if !shells.is_empty() {
            shelf.push(Self::render_shelf_header(
                "shells-shelf-toggle",
                "Shells",
                shells.len(),
                self.shells_expanded,
                |this, cx| {
                    this.shells_expanded = !this.shells_expanded;
                    cx.notify();
                },
                cx,
            ));
            if self.shells_expanded {
                for (machine, thread) in shells {
                    if let Some(store) = self.store(machine, cx) {
                        shelf.push(self.render_slim_row(&store, thread, Shelf::Shells, cx));
                    }
                }
            }
        }
        // Threads started in workspace panes, quiet here: their panes show what they're doing.
        let workspaces_threads = self.machines.read(cx).workspaces_threads(cx);
        if !workspaces_threads.is_empty() {
            let is_expanded = self.machines.read(cx).workspaces_expanded(cx);
            shelf.push(Self::render_shelf_header(
                "workspaces-shelf-toggle",
                "Workspaces",
                workspaces_threads.len(),
                is_expanded,
                |this, cx| {
                    // Kept by this Mac's server, as Archived's is.
                    if let Some(store) = this.store(MachineId::Local, cx) {
                        store.update(cx, |store, cx| store.toggle_workspaces_expanded(cx));
                    }
                },
                cx,
            ));
            if is_expanded {
                for (machine, thread) in workspaces_threads {
                    if let Some(store) = self.store(machine, cx) {
                        shelf.push(self.render_slim_row(&store, thread, Shelf::Workspaces, cx));
                    }
                }
            }
        }
        let archived_count = archived.len();
        if archived_count > 0 {
            shelf.push(Self::render_shelf_header(
                "archived-shelf-toggle",
                "Archived",
                archived_count,
                is_archived_expanded,
                |this, cx| {
                    // Kept by this Mac's server, like the thread order.
                    if let Some(store) = this.store(MachineId::Local, cx) {
                        store.update(cx, |store, cx| store.toggle_archived_expanded(cx));
                    }
                },
                cx,
            ));
            if is_archived_expanded {
                let hidden_count = archived_count.saturating_sub(self.archived_shown);
                for (machine, thread) in archived.into_iter().take(self.archived_shown) {
                    if let Some(store) = self.store(machine, cx) {
                        shelf.push(self.render_slim_row(&store, thread, Shelf::Archived, cx));
                    }
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
            .children(draft_rows)
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
        let has_projects = self
            .machines
            .read(cx)
            .clients()
            .iter()
            .any(|client| !client.read(cx).projects().read(cx).projects().is_empty());

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
pub(crate) struct ThreadDetails {
    pub(crate) title: SharedString,
    pub(crate) project: Option<(Project, Option<ProjectInfo>)>,
    /// The machine's icon and name.
    pub(crate) machine: (IconName, SharedString),
    pub(crate) branch: Option<SharedString>,
    /// Where a terminal is, outside git.
    pub(crate) path: Option<SharedString>,
    /// The worktree or pasture it works in, described.
    pub(crate) workspace: Option<(WorkspaceKind, SharedString)>,
    /// The agent's icon (SVG markup) and its label.
    pub(crate) agent: Option<(Option<SharedString>, SharedString)>,
    /// What a Workspaces view workspace holds, such as "2 terminals · 1 agent".
    pub(crate) contents: Option<SharedString>,
}

/// A details card's row: an icon, then a label in the details' color.
pub(crate) fn details_row(icon: AnyElement, label: Label, cx: &App) -> AnyElement {
    let detail_color = Color::Custom(cx.theme().colors().text.opacity(0.75));
    h_flex()
        .min_w_0()
        .gap_2()
        .child(div().flex_none().child(icon))
        .child(
            div()
                .min_w_0()
                .child(label.size(LabelSize::Small).color(detail_color)),
        )
        .into_any_element()
}

/// The card a details popover shows: a title over its rows.
pub(crate) fn details_card(title: SharedString, rows: Vec<AnyElement>, cx: &App) -> AnyElement {
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
                    Label::new(title)
                        .size(LabelSize::Small)
                        .weight(FontWeight::MEDIUM)
                        .truncate(),
                )
                .child(v_flex().gap_1p5().pl_0p5().children(rows)),
        )
        .into_any_element()
}

impl ThreadDetails {
    fn render(self, cx: &App) -> AnyElement {
        let detail_row = |icon: AnyElement, label: Label| details_row(icon, label, cx);
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
        let (machine_icon, machine) = &self.machine;
        rows.push(detail_row(
            small_icon(*machine_icon),
            Label::new(machine.clone()).truncate(),
        ));
        if let Some(branch) = &self.branch {
            rows.push(detail_row(
                small_icon(IconName::GitBranch),
                Label::new(branch.clone()).truncate_middle(),
            ));
        }
        if let Some(path) = &self.path {
            rows.push(detail_row(
                small_icon(IconName::Folder),
                Label::new(path.clone()).truncate_middle(),
            ));
        }
        if let Some((kind, description)) = &self.workspace {
            rows.push(detail_row(
                small_icon(workspace_icon(*kind)),
                Label::new(description.clone()).truncate_middle(),
            ));
        }
        if let Some((icon, label)) = &self.agent {
            let icon = icon
                .clone()
                .map(Icon::from_svg_markup)
                .unwrap_or_else(|| Icon::new(IconName::Terminal));
            rows.push(detail_row(
                div()
                    .opacity(0.6)
                    .child(icon.size(IconSize::XSmall).color(Color::Muted))
                    .into_any_element(),
                Label::new(label.clone()).truncate(),
            ));
        }
        if let Some(contents) = &self.contents {
            rows.push(detail_row(
                small_icon(IconName::Terminal),
                Label::new(contents.clone()).truncate(),
            ));
        }
        details_card(self.title, rows, cx)
    }
}

/// A worktree's or pasture's icon before the card's branch, or the worktree icon when the
/// project's own folder is a linked worktree.
fn render_checkout_marker(
    thread_id: ThreadKey,
    checkout: &ThreadCheckout,
    color: Hsla,
) -> Option<AnyElement> {
    let (icon, tooltip) = match (&checkout.workspace, &checkout.head) {
        (Some(workspace), _) => (
            workspace_icon(workspace.kind),
            describe_folder(workspace.kind, &checkout.folder),
        ),
        (
            None,
            Some(GitHead {
                worktree: Some(worktree),
                ..
            }),
        ) => (
            IconName::GitWorktree,
            describe_folder(WorkspaceKind::Worktree, worktree),
        ),
        _ => return None,
    };
    Some(
        div()
            .id(thread_element_id("thread-checkout", thread_id))
            .flex_none()
            .tooltip(Tooltip::text(tooltip))
            .child(
                Icon::new(icon)
                    .size(IconSize::XSmall)
                    .color(Color::Custom(color)),
            )
            .into_any_element(),
    )
}

/// t3code's pen for unsent work.
fn render_draft_pen() -> Icon {
    Icon::new(IconName::SquarePen)
        .size(IconSize::XSmall)
        .color(Color::Warning)
}

/// t3code's Discard draft button: a muted × that brightens under the mouse. Clearing the
/// text is all it takes; the server removes a draft left with none.
fn render_discard_draft_button(
    thread_id: ThreadKey,
    store: Entity<ProjectStore>,
    cx: &App,
) -> Stateful<Div> {
    let muted_text = cx.theme().colors().text_muted;
    let bright_text = cx.theme().colors().text;
    let group_name = SharedString::from(format!(
        "discard-draft-{}-{}",
        thread_id.machine.slug(),
        thread_id.thread.0
    ));
    h_flex()
        .id(thread_element_id("discard-draft", thread_id))
        .group(group_name.clone())
        .h_full()
        .px_1()
        .cursor_pointer()
        .tooltip(Tooltip::text("Discard draft"))
        .child(
            svg()
                .path(IconName::Close.path())
                .size(IconSize::XSmall.rems())
                .flex_none()
                .text_color(muted_text)
                .group_hover(group_name, |this| this.text_color(bright_text)),
        )
        .on_click(move |_, _, cx| {
            cx.stop_propagation();
            store.read(cx).set_unsent_text(thread_id.thread, None, cx);
        })
}

/// Which machine a thread is on, after its project's name, when it isn't this Mac.
fn render_machine_tag(icon: IconName, label: SharedString, is_offline: bool) -> AnyElement {
    h_flex()
        .flex_none()
        .gap_0p5()
        .child(
            Icon::new(if is_offline {
                IconName::Disconnected
            } else {
                icon
            })
            .size(IconSize::XSmall)
            .color(Color::Muted),
        )
        .child(
            Label::new(label)
                .size(LabelSize::XSmall)
                .color(Color::Muted)
                .truncate(),
        )
        .into_any_element()
}

/// "Pasture · ~/.cow/pastures/app-1a2b".
fn describe_folder(kind: WorkspaceKind, folder: &Path) -> SharedString {
    format!("{} · {}", kind.label(), compact_path(folder)).into()
}

/// Placed beside the row's right edge, top-aligned, as t3code places its row tooltip.
pub(crate) fn render_details_popover(details: ThreadDetails, cx: &App) -> AnyElement {
    render_card_popover(details.render(cx))
}

/// A card beside the row it belongs to, kept inside the window.
pub(crate) fn render_card_popover(card: AnyElement) -> AnyElement {
    div()
        .absolute()
        .top_0()
        .left_full()
        .child(
            deferred(
                anchored()
                    .snap_to_window_with_margin(px(8.))
                    .child(div().ml_1().child(card)),
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
pub(crate) fn format_relative_time(time: SystemTime, now: SystemTime) -> String {
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
    use super::{format_relative_time, repository_branch};

    #[test]
    fn a_branch_names_its_repository_under_another_title() {
        assert_eq!(
            repository_branch(Some("agentZ"), Some("agentZ"), "main"),
            "main"
        );
        assert_eq!(
            repository_branch(Some("Claude Code"), Some("agentZ"), "main"),
            "agentZ/main"
        );
        assert_eq!(repository_branch(None, Some("agentZ"), "main"), "main");
        assert_eq!(repository_branch(Some("app"), None, "main"), "main");
    }
    use crate::machines::project_at;
    use projects::{Project, ProjectId, Workspace, WorkspaceKind};
    use std::path::{Path, PathBuf};
    use std::time::{Duration, SystemTime};

    fn project(id: u64, path: &str, workspaces: &[&str]) -> Project {
        Project {
            id: ProjectId(id),
            path: PathBuf::from(path),
            custom_name: None,
            icon: None,
            workspaces: workspaces
                .iter()
                .map(|path| Workspace {
                    kind: WorkspaceKind::Worktree,
                    path: PathBuf::from(path),
                    branch: None,
                    base: None,
                    created_at: SystemTime::UNIX_EPOCH,
                })
                .collect(),
            repository: None,
        }
    }

    #[test]
    fn a_folder_belongs_to_the_deepest_project_holding_it() {
        let projects = [
            project(1, "/code", &[]),
            project(2, "/code/api", &["/worktrees/api-fix"]),
        ];
        let id = |folder: &str| project_at(&projects, Path::new(folder)).map(|project| project.id);
        assert_eq!(id("/code/api/src"), Some(ProjectId(2)));
        assert_eq!(id("/code/web"), Some(ProjectId(1)));
        assert_eq!(id("/worktrees/api-fix/src"), Some(ProjectId(2)));
        // A sibling whose name only starts the same isn't inside.
        assert_eq!(id("/code/api-old"), Some(ProjectId(1)));
        assert_eq!(id("/tmp"), None);
    }

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

#[cfg(test)]
mod view_tests {
    use agentz_protocol::spaces::SpacesSnapshot;
    use gpui::{Entity, TestAppContext, VisualTestContext};
    use projects::{Project, ProjectId, ProjectsSnapshot, Thread, ThreadId};
    use serde_json::json;

    use super::Sidebar;
    use crate::machines::{MachineId, Machines, Scope, ThreadKey};
    use crate::project_store::ProjectStore;
    use crate::server_client::ServerClient;

    fn thread(id: u64, is_draft: bool, unsent_text: Option<&str>) -> Thread {
        serde_json::from_value(json!({
            "id": id,
            "project_id": 1,
            "title": format!("Thread {id}"),
            "agent_id": "mock",
            "is_draft": is_draft,
            "unsent_text": unsent_text,
        }))
        .expect("a thread")
    }

    fn show(store: &Entity<ProjectStore>, threads: Vec<Thread>, cx: &mut VisualTestContext) {
        show_with(store, threads, false, cx)
    }

    fn show_with(
        store: &Entity<ProjectStore>,
        threads: Vec<Thread>,
        workspaces_expanded: bool,
        cx: &mut VisualTestContext,
    ) {
        cx.update(|_, cx| {
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
                        threads,
                        workspaces_expanded,
                        ..Default::default()
                    },
                    cx,
                )
            })
        });
        cx.run_until_parked();
    }

    /// A thread started in a workspace pane, working in `folder`.
    fn workspaces_thread(id: u64, folder: &str) -> Thread {
        let mut thread = thread(id, false, None);
        thread.project_id = ProjectId::WORKSPACES;
        thread.workspace = Some(folder.into());
        thread
    }

    fn new_sidebar(
        cx: &mut TestAppContext,
    ) -> (
        Entity<Sidebar>,
        Entity<ProjectStore>,
        &mut VisualTestContext,
    ) {
        let store = cx.update(|cx| {
            crate::init_for_test(cx);
            let client = ServerClient::new_for_test(
                MachineId::Local,
                "This Mac".into(),
                SpacesSnapshot::default(),
                cx,
            );
            let store = client.read(cx).projects().clone();
            crate::machines::init_for_test(vec![client], cx);
            crate::project_info::init(cx);
            crate::sidebar::init(cx);
            store
        });
        let (sidebar, cx) = cx.add_window_view(|_, cx| Sidebar::new(cx));
        (sidebar, store, cx)
    }

    fn open(sidebar: &Entity<Sidebar>, thread: u64, cx: &mut VisualTestContext) {
        sidebar.update(cx, |sidebar, cx| {
            sidebar.set_active_thread(
                Some(ThreadKey {
                    machine: MachineId::Local,
                    thread: ThreadId(thread),
                }),
                cx,
            )
        });
        cx.run_until_parked();
    }

    #[gpui::test]
    fn threads_started_in_panes_list_in_the_workspaces_shelf(cx: &mut TestAppContext) {
        let (sidebar, store, cx) = new_sidebar(cx);
        let shown =
            |name: &'static str, cx: &mut VisualTestContext| cx.debug_bounds(name).is_some();
        let threads = || {
            vec![
                thread(1, false, None),
                workspaces_thread(2, "/tmp/demo/src"),
                workspaces_thread(3, "/tmp/docs"),
            ]
        };
        // Not among the cards, and closed at first.
        show(&store, threads(), cx);
        assert!(shown("thread-card-1", cx));
        assert!(!shown("thread-card-2", cx));
        assert!(shown("workspaces-shelf-toggle", cx));
        assert!(!shown("workspaces-row-2", cx));

        show_with(&store, threads(), true, cx);
        assert!(shown("workspaces-row-2", cx));
        assert!(shown("workspaces-row-3", cx));

        // Search finds them, named as Workspaces threads.
        sidebar.update(cx, |sidebar, cx| {
            sidebar
                .search
                .update(cx, |search, cx| search.set_text("Thread", cx))
        });
        cx.run_until_parked();
        assert!(shown("search-result-1", cx));
        assert!(!shown("search-list-Workspaces-1", cx));
        assert!(shown("search-list-Workspaces-2", cx));
        assert!(shown("search-list-Workspaces-3", cx));
        sidebar.update(cx, |sidebar, cx| {
            sidebar
                .search
                .update(cx, |search, cx| search.set_text("", cx))
        });

        // Under a project, only those whose folder is in it.
        cx.update(|_, cx| {
            let key = Machines::global(cx).read(cx).project_groups(cx)[0]
                .key
                .clone();
            Machines::set_scope(Scope::Group(key), cx);
        });
        cx.run_until_parked();
        assert!(shown("workspaces-row-2", cx));
        assert!(!shown("workspaces-row-3", cx));
    }

    #[gpui::test]
    fn drafts_list_above_threads_once_something_is_typed(cx: &mut TestAppContext) {
        let (sidebar, store, cx) = new_sidebar(cx);
        let shown =
            |name: &'static str, cx: &mut VisualTestContext| cx.debug_bounds(name).is_some();
        show(
            &store,
            vec![
                thread(1, true, Some("Fix the login\nand the logout")),
                thread(2, true, None),
                thread(3, false, Some("Half typed")),
                thread(4, false, None),
            ],
            cx,
        );
        // A draft is a row once something is typed, and never a card.
        assert!(shown("draft-row-1", cx));
        assert!(!shown("thread-card-1", cx));
        assert!(!shown("draft-row-2", cx));
        assert!(!shown("thread-card-2", cx));
        // A thread with unsent text is marked with the pen, unless it's open.
        assert!(shown("unsent-text-3", cx));
        assert!(!shown("unsent-text-4", cx));
        open(&sidebar, 3, cx);
        assert!(!shown("unsent-text-3", cx));

        // The open draft keeps its row.
        open(&sidebar, 1, cx);
        assert!(shown("draft-row-1", cx));

        // A draft typed in without leaving it has no row until it's left.
        open(&sidebar, 2, cx);
        show(
            &store,
            vec![
                thread(1, true, Some("Fix the login")),
                thread(2, true, Some("New idea")),
                thread(3, false, Some("Half typed")),
                thread(4, false, None),
            ],
            cx,
        );
        assert!(!shown("draft-row-2", cx));
        open(&sidebar, 4, cx);
        assert!(shown("draft-row-2", cx));

        // Its first message makes it a thread.
        show(
            &store,
            vec![
                thread(1, true, Some("Fix the login")),
                thread(2, false, None),
                thread(3, false, Some("Half typed")),
                thread(4, false, None),
            ],
            cx,
        );
        assert!(!shown("draft-row-2", cx));
        assert!(shown("thread-card-2", cx));
    }
}

/// The icon of the agent a thread runs. A terminal thread's is a terminal.
pub(crate) fn thread_agent_icon(thread: &Thread, cx: &App) -> Icon {
    thread
        .agent_id
        .as_ref()
        .and_then(|agent_id| agent_icon(&AgentId::new(agent_id.clone()), cx))
        .map(Icon::from_svg_markup)
        .unwrap_or_else(|| Icon::new(IconName::Terminal))
}

/// Purple, apart from the other statuses' colors. Every bundled theme's fourth player color is
/// one, and `Color::Player` skips the first (the local user's).
pub(crate) const AWAITING_INPUT_COLOR: Color = Color::Player(2);

/// t3code's status pill: a dot and a label in the status color.
pub(crate) fn render_status_pill(status: ThreadStatus, cx: &App) -> impl IntoElement {
    let (label, color) = match status {
        ThreadStatus::PendingApproval => ("Pending Approval", Color::Warning),
        ThreadStatus::AwaitingInput => ("Awaiting Input", AWAITING_INPUT_COLOR),
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
        ThreadStatus::AwaitingInput => AWAITING_INPUT_COLOR,
        ThreadStatus::Working => Color::Accent,
        ThreadStatus::Completed => Color::Success,
    };
    div()
        .flex_none()
        .size_1p5()
        .rounded_full()
        .bg(color.color(cx))
}

/// The branch a terminal's folder is on, or outside git, where the folder is. Under a title
/// that isn't the repository's name, the branch says which repository it's in.
fn folder_branch_label(folder: &projects::TerminalFolder, title: Option<&str>) -> Option<String> {
    if folder.is_repository {
        let branch = folder.branch.as_deref()?;
        Some(repository_branch(
            title,
            folder.repository.as_deref(),
            branch,
        ))
    } else {
        Some(
            folder
                .display_path
                .clone()
                .unwrap_or_else(|| folder.path.display().to_string()),
        )
    }
}

/// `repository/branch`, unless the title already is the repository's name.
pub(crate) fn repository_branch(
    title: Option<&str>,
    repository: Option<&str>,
    branch: &str,
) -> String {
    match (title, repository) {
        (Some(title), Some(repository)) if title != repository => {
            format!("{repository}/{branch}")
        }
        _ => branch.to_string(),
    }
}

/// A folder outside every project, in a slot as wide as a project's icon so names line up.
pub(crate) fn render_folder_icon() -> AnyElement {
    h_flex()
        .size_4()
        .flex_none()
        .justify_center()
        .child(
            Icon::new(IconName::Folder)
                .size(IconSize::Small)
                .color(Color::Muted),
        )
        .into_any_element()
}

fn folder_name(folder: &Path) -> SharedString {
    folder
        .file_name()
        .map_or_else(
            || folder.display().to_string(),
            |name| name.to_string_lossy().into_owned(),
        )
        .into()
}
