use std::cell::Cell;
use std::cmp::Ordering;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::{Duration, Instant, SystemTime};

use crate::agent_icons::agent_icon;
use crate::agent_view::TOOLBAR_HEIGHT;
use crate::controls::account_color;
use crate::machines::{
    MachineId, Machines, ProjectKey, Scope, ThreadKey, by_latest_activity, project_at,
};
use crate::project_store::{ProjectStore, ThreadStatus};
use crate::slide_drag::{
    DRAG_SCROLL_STEP, SlideDrag, drag_scroll_direction, render_raised_rows, scroll_while_held,
};
use agentz_protocol::agents::AgentId;
use collections::HashMap;
use gpui::{
    AnyElement, App, ClickEvent, Context, DragMoveEvent, ElementId, Entity, EventEmitter,
    Focusable as _, FontWeight, Hsla, KeyBinding, MouseButton, PromptLevel, ScrollHandle, Stateful,
    Subscription, Task, Window, anchored, canvas, deferred, svg,
};
use projects::{
    GitHead, Project, Thread, ThreadOrder, ThreadSection, Workspace, WorkspaceKind, order_key,
};
use text_input::{TextInput, TextInputEvent};
use ui::{
    CommonAnimationExt as _, ContextMenu, ContextMenuEntry, Tooltip, WithScrollbar as _,
    prelude::*, right_click_menu,
};

use crate::project_info::{ProjectInfo, ProjectInfoStore, render_project_icon, workspace_icon};
use crate::project_switcher::compact_path;
use crate::{NewThread, OpenSettings};

/// How often relative activity times ("5m") are re-rendered.
const ACTIVITY_REFRESH_INTERVAL: Duration = Duration::from_secs(30);
const CARD_HEIGHT: Pixels = px(78.);
pub(crate) const DETAILS_DELAY: Duration = Duration::from_millis(500);
pub const SIDEBAR_WIDTH: Pixels = px(290.);
const RENAME_KEY_CONTEXT: &str = "SidebarRename";
pub(crate) const SEARCH_KEY_CONTEXT: &str = "SidebarSearch";
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

/// How tall the Pinned and Active labels open while a thread is dragged (t3code's
/// `SIDEBAR_DRAG_LABEL_HEIGHT`).
const DRAG_LABEL_HEIGHT: Pixels = px(24.);

/// A thread card or archived row being dragged. The sidebar draws it raised (`ThreadDrag`), so
/// nothing is drawn under the pointer.
#[derive(Clone, Copy)]
struct DraggedThread(ThreadKey);

impl Render for DraggedThread {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        gpui::Empty
    }
}

/// Where a dragged thread is from, or would land.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DragSection {
    Pinned,
    Active,
    Archived,
}

/// What letting go of a dragged thread does, which its badge says (t3code's drop verb).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DropVerb {
    Pin,
    Unpin,
    Archive,
    Unarchive,
}

impl DropVerb {
    /// Nothing when the thread lands where it was.
    fn between(from: DragSection, to: DragSection) -> Option<Self> {
        match (from, to) {
            (DragSection::Pinned, DragSection::Pinned)
            | (DragSection::Active, DragSection::Active)
            | (DragSection::Archived, DragSection::Archived) => None,
            (DragSection::Active | DragSection::Archived, DragSection::Pinned) => Some(Self::Pin),
            (DragSection::Pinned, DragSection::Active) => Some(Self::Unpin),
            (DragSection::Archived, DragSection::Active) => Some(Self::Unarchive),
            (DragSection::Pinned | DragSection::Active, DragSection::Archived) => {
                Some(Self::Archive)
            }
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Pin => "Pin",
            Self::Unpin => "Unpin",
            Self::Archive => "Archive",
            Self::Unarchive => "Unarchive",
        }
    }

    fn icon(self) -> IconName {
        match self {
            Self::Pin => IconName::Pin,
            Self::Unpin => IconName::Unpin,
            Self::Archive => IconName::Archive,
            Self::Unarchive => IconName::Undo,
        }
    }
}

/// What slides along the cards while a thread is dragged: the cards, and the labels that open
/// above the pinned ones and the rest.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum DragItem {
    PinnedLabel,
    ActiveLabel,
    Thread(ThreadKey),
}

/// A thread card, or an archived thread's row, held while dragged up and down the cards or
/// onto Archived, as t3code's sidebar does.
struct ThreadDrag {
    thread: ThreadKey,
    from: DragSection,
    /// An agent CLI's card only moves among the unpinned cards: terminals aren't pinned or
    /// archived.
    is_agent_cli: bool,
    /// When the unpinned cards keep the latest active first, where the thread goes among them,
    /// as they can't be arranged by hand then.
    time_index: Option<usize>,
    /// The pinned cards and the rest in the order shown when the drag started.
    pinned: Vec<ThreadKey>,
    active: Vec<ThreadKey>,
    /// The thread's card or row height, with the space around it.
    height: Pixels,
    /// Whether the pointer is on the Archived header or under it.
    over_archived: bool,
    /// The cards' top in the list scrolled to its top, where the Pinned label opens.
    top: Pixels,
    /// How far the labels opening moved the card's place down. It stays under the pointer, so
    /// its place is held this far below it, as in t3code.
    bias: Pixels,
    left: Pixels,
    width: Pixels,
    slide: SlideDrag<DragItem>,
}

impl ThreadDrag {
    fn item(&self) -> DragItem {
        DragItem::Thread(self.thread)
    }

    fn index(&self, item: DragItem) -> usize {
        self.slide
            .order
            .iter()
            .position(|candidate| *candidate == item)
            .unwrap_or_default()
    }

    /// The Active label's place in the order without the dragged thread.
    fn active_label_index(&self) -> usize {
        let label = self.index(DragItem::ActiveLabel);
        if self.index(self.item()) < label {
            label - 1
        } else {
            label
        }
    }

    fn target(&self) -> DragSection {
        if self.over_archived {
            DragSection::Archived
        } else if self.index(self.item()) < self.index(DragItem::ActiveLabel) {
            DragSection::Pinned
        } else {
            DragSection::Active
        }
    }

    fn verb(&self) -> Option<DropVerb> {
        DropVerb::between(self.from, self.target())
    }

    /// The threads of a section in the order shown.
    fn shown(&self, section: DragSection) -> Vec<ThreadKey> {
        let label = self.index(DragItem::ActiveLabel);
        let items = match section {
            DragSection::Pinned => &self.slide.order[..label],
            DragSection::Active => &self.slide.order[label + 1..],
            DragSection::Archived => &[],
        };
        items
            .iter()
            .filter_map(|item| match item {
                DragItem::Thread(key) => Some(*key),
                _ => None,
            })
            .collect()
    }

    /// Where the dragged thread's start crosses from the pinned cards to the rest: halfway
    /// down the Active label, with the thread just above it.
    fn boundary(&self) -> Pixels {
        let item = self.item();
        let label = self.index(DragItem::ActiveLabel);
        let before = self.slide.order[..label]
            .iter()
            .filter(|candidate| **candidate != item)
            .fold(px(0.), |length, candidate| {
                length + self.slide.length(*candidate)
            });
        before + self.slide.length(DragItem::ActiveLabel) / 2.
    }

    /// Moves the thread where the pointer holds it, `offset` being the list's scroll: among
    /// the pinned cards or the rest, never above the Pinned label, and in the order the rest
    /// keep when they keep one of their own.
    fn arrange(&mut self, offset: Pixels) {
        let item = self.item();
        // An archived row's place opens among the cards once it leaves Archived.
        if self.from == DragSection::Archived {
            let length = if self.over_archived {
                px(0.)
            } else {
                self.height
            };
            self.slide.set_length(item, length);
        }
        let label = self.slide.length(DragItem::PinnedLabel);
        let total = self
            .slide
            .order
            .iter()
            .fold(px(0.), |length, item| length + self.slide.length(*item));
        let last = (total - self.slide.length(item)).max(label);
        let boundary = self.boundary();
        let mut start =
            (self.slide.held_start() - (self.top + offset) + self.bias).clamp(label, last);
        if self.is_agent_cli {
            start = start.max(boundary);
        }
        match self.time_index {
            Some(time_index) if self.is_agent_cli || start > boundary => {
                self.slide
                    .move_to(self.active_label_index() + 1 + time_index);
            }
            Some(_) => {
                // Among the pinned cards, it comes in at their end and moves up from there.
                if self.index(item) > self.index(DragItem::ActiveLabel) {
                    self.slide.move_to(self.active_label_index());
                }
                self.slide.reorder(start.min(boundary));
            }
            None => self.slide.reorder(start),
        }
    }
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
    list_scroll: ScrollHandle,
    thread_drag: Option<ThreadDrag>,
    /// Where the Archived header is in the window, as last drawn, for dragging onto it.
    archived_top: Rc<Cell<Option<Pixels>>>,
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
            list_scroll: ScrollHandle::new(),
            thread_drag: None,
            archived_top: Rc::default(),
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
        let head = store.git_head(&folder).cloned();
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

    /// Pin or Unpin, Rename, Archive or Unarchive, the pasture's actions, Project Settings, and
    /// Delete, each with its icon. A Workspaces thread's has Rename, Move to Threads and Delete.
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
        // `None` for the threads that aren't pinned: shells, agent CLIs and Workspaces threads.
        let is_pinned = thread.can_pin().then(|| thread.is_pinned());
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
                let menu = menu
                    .when_some(is_pinned, |menu, is_pinned| {
                        let sidebar = sidebar.clone();
                        menu.item(
                            ContextMenuEntry::new(if is_pinned { "Unpin" } else { "Pin" })
                                .icon(if is_pinned {
                                    IconName::Unpin
                                } else {
                                    IconName::Pin
                                })
                                .icon_color(Color::Muted)
                                .handler(move |_, cx| {
                                    sidebar
                                        .update(cx, |sidebar, cx| {
                                            sidebar.toggle_pinned(thread_id, is_pinned, cx)
                                        })
                                        .ok();
                                }),
                        )
                    })
                    .item(
                        ContextMenuEntry::new("Rename")
                            .icon(IconName::Pencil)
                            .icon_color(Color::Muted)
                            .handler(rename),
                    );
                if in_workspaces {
                    let sidebar = sidebar.clone();
                    return menu
                        .item(
                            ContextMenuEntry::new("Move to Threads")
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

    /// Pins the thread above the pinned ones, or unpins it.
    fn toggle_pinned(&mut self, key: ThreadKey, is_pinned: bool, cx: &mut Context<Self>) {
        if !is_pinned {
            Machines::pin_thread(&self.machines, key, cx);
        } else if let Some(store) = self.store(key.machine, cx) {
            store.update(cx, |store, cx| store.unpin_thread(key.thread, cx));
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
                Some("Threads in the threads list belong to a project."),
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
        // Cards pass under the pointer while one is dragged.
        if self.details_thread == Some(thread_id) || self.thread_drag.is_some() {
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

    /// The drafts listed above the cards. The open draft's row is the one it had when it was
    /// opened.
    fn shown_drafts(&self, cx: &App) -> Vec<(Entity<ProjectStore>, Thread)> {
        let mut drafts = Vec::new();
        for (machine, thread) in self.machines.read(cx).typed_drafts(cx) {
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
                drafts.push((store, thread));
            }
        }
        drafts
    }

    /// The thread cards, the pinned ones first.
    fn shown_cards(&self, cx: &App) -> Vec<(Entity<ProjectStore>, Thread)> {
        self.machines
            .read(cx)
            .active_threads(cx)
            .into_iter()
            .filter_map(|(machine, thread)| Some((self.store(machine, cx)?, thread)))
            .collect()
    }

    /// Picks up a thread's card, or an archived thread's row held `row_grab` down from its
    /// top, as the list last drew them.
    fn start_thread_drag(
        &mut self,
        key: ThreadKey,
        pointer: Pixels,
        row_grab: Option<Pixels>,
        cx: &mut Context<Self>,
    ) {
        let Some(thread) = self
            .store(key.machine, cx)
            .and_then(|store| store.read(cx).thread(key.thread).cloned())
        else {
            return;
        };
        let from = if thread.archived_at.is_some() {
            DragSection::Archived
        } else if thread.is_pinned() {
            DragSection::Pinned
        } else {
            DragSection::Active
        };
        let cards: Vec<(ThreadKey, Thread)> = self
            .shown_cards(cx)
            .into_iter()
            .map(|(store, thread)| {
                let key = ThreadKey {
                    machine: store.read(cx).machine(),
                    thread: thread.id,
                };
                (key, thread)
            })
            .collect();
        // The cards follow the drafts and the line under them.
        let first_card = match self.shown_drafts(cx).len() {
            0 => 0,
            drafts => drafts + 1,
        };
        let bounds = |index: usize| self.list_scroll.bounds_for_item(index);
        let Some(first) = bounds(first_card) else {
            return;
        };
        let mut lengths = HashMap::default();
        let mut pinned = Vec::new();
        let mut active = Vec::new();
        for (index, (card_key, card)) in cards.iter().enumerate() {
            let Some(card_bounds) = bounds(first_card + index) else {
                return;
            };
            lengths.insert(DragItem::Thread(*card_key), card_bounds.size.height);
            if card.is_pinned() {
                pinned.push(*card_key);
            } else {
                active.push(*card_key);
            }
        }
        let mut order: Vec<DragItem> = std::iter::once(DragItem::PinnedLabel)
            .chain(pinned.iter().copied().map(DragItem::Thread))
            .chain(std::iter::once(DragItem::ActiveLabel))
            .chain(active.iter().copied().map(DragItem::Thread))
            .collect();
        let offset = self.list_scroll.offset().y;
        let (height, grab, bias) = match (from, row_grab) {
            // Its place among the cards opens once it leaves Archived.
            (DragSection::Archived, Some(grab)) => {
                order.push(DragItem::Thread(key));
                (ARCHIVED_ROW_HEIGHT, grab, px(0.))
            }
            (DragSection::Pinned | DragSection::Active, None) => {
                let Some(card_bounds) = cards
                    .iter()
                    .position(|(card_key, _)| *card_key == key)
                    .and_then(|index| bounds(first_card + index))
                else {
                    return;
                };
                let labels_above = if from == DragSection::Pinned { 1. } else { 2. };
                (
                    card_bounds.size.height,
                    pointer - (card_bounds.top() + offset),
                    DRAG_LABEL_HEIGHT * labels_above,
                )
            }
            _ => return,
        };
        let time_index = (self.machines.read(cx).thread_order(cx) == ThreadOrder::LastActivity)
            .then(|| {
                cards
                    .iter()
                    .filter(|(card_key, card)| {
                        *card_key != key
                            && !card.is_pinned()
                            && by_latest_activity((card_key.machine, card), (key.machine, &thread))
                                == Ordering::Less
                    })
                    .count()
            });
        let mut slide = SlideDrag::new(DragItem::Thread(key), order, lengths, grab, pointer);
        // The labels open, and the cards move down to make room for them.
        slide.set_length(DragItem::PinnedLabel, DRAG_LABEL_HEIGHT);
        slide.set_length(DragItem::ActiveLabel, DRAG_LABEL_HEIGHT);
        self.hide_details(cx);
        self.thread_drag = Some(ThreadDrag {
            thread: key,
            from,
            is_agent_cli: thread.terminal.is_some(),
            time_index,
            pinned,
            active,
            height,
            over_archived: false,
            top: first.top(),
            bias,
            left: first.left(),
            width: first.size.width,
            slide,
        });
        self.drag_thread(pointer, cx);
    }

    /// The dragged thread follows the pointer among the cards, which make room for it, or onto
    /// Archived.
    fn drag_thread(&mut self, pointer: Pixels, cx: &mut Context<Self>) {
        let list = self.list_scroll.bounds();
        let offset = self.list_scroll.offset().y;
        let archived_top = self.archived_top.get();
        let Some(drag) = self.thread_drag.as_mut() else {
            return;
        };
        drag.slide.pointer = pointer;
        drag.over_archived = !drag.is_agent_cli
            && archived_top.is_some_and(|top| top < list.bottom() && pointer >= top);
        drag.arrange(offset);
        if !drag.slide.is_scrolling && self.thread_scroll_direction().is_some() {
            let task = scroll_while_held(Self::scroll_threads_step, cx);
            if let Some(drag) = self.thread_drag.as_mut() {
                drag.slide.is_scrolling = true;
                drag.slide._scroll = task;
            }
        }
        cx.notify();
    }

    fn thread_scroll_direction(&self) -> Option<f32> {
        let drag = self.thread_drag.as_ref()?;
        let list = self.list_scroll.bounds();
        let top = drag.slide.held_start();
        drag_scroll_direction(
            (top, top + drag.height),
            (list.top(), list.bottom()),
            self.list_scroll.offset().y,
            self.list_scroll.max_offset().y,
        )
    }

    fn scroll_threads_step(&mut self, cx: &mut Context<Self>) -> bool {
        let direction = self.thread_scroll_direction();
        let Some(drag) = self.thread_drag.as_mut() else {
            return false;
        };
        let Some(direction) = direction else {
            drag.slide.is_scrolling = false;
            return false;
        };
        let pointer = drag.slide.pointer;
        let offset = self.list_scroll.offset();
        let max = self.list_scroll.max_offset().y;
        self.list_scroll.set_offset(gpui::point(
            offset.x,
            (offset.y + DRAG_SCROLL_STEP * direction).clamp(-max, px(0.)),
        ));
        // The cards pass under the held one.
        self.drag_thread(pointer, cx);
        true
    }

    /// Letting go pins, unpins, archives or unarchives the thread, and leaves it where it was
    /// let go: one new order key between its neighbors', as t3code's `planSidebarThreadDrop`
    /// does.
    fn drop_thread(&mut self, cx: &mut Context<Self>) {
        let Some(drag) = self.thread_drag.take() else {
            return;
        };
        cx.notify();
        let key = drag.thread;
        let Some(store) = self.store(key.machine, cx) else {
            return;
        };
        match drag.target() {
            DragSection::Archived => {
                if drag.from != DragSection::Archived {
                    store.update(cx, |store, cx| store.archive_thread(key.thread, cx));
                }
            }
            DragSection::Pinned => {
                let order = drag.shown(DragSection::Pinned);
                if drag.from == DragSection::Pinned && order == drag.pinned {
                    return;
                }
                let keys = self.machines.read(cx).order_keys(ThreadSection::Pinned, cx);
                let mut assignments = order_key::plan_reorder(&order, &keys, key);
                if drag.from != DragSection::Pinned {
                    let own_key = assignments
                        .iter()
                        .position(|(assigned, _)| *assigned == key)
                        .map(|index| assignments.remove(index).1);
                    store.update(cx, |store, cx| store.pin_thread(key.thread, own_key, cx));
                }
                self.write_order_keys(ThreadSection::Pinned, assignments, cx);
            }
            DragSection::Active => {
                match drag.from {
                    DragSection::Pinned => {
                        store.update(cx, |store, cx| store.unpin_thread(key.thread, cx))
                    }
                    DragSection::Archived => {
                        store.update(cx, |store, cx| store.unarchive_thread(key.thread, cx))
                    }
                    DragSection::Active => {}
                }
                // Kept latest active first, the unpinned cards have no order to write.
                let order = drag.shown(DragSection::Active);
                if drag.time_index.is_some()
                    || (drag.from == DragSection::Active && order == drag.active)
                {
                    return;
                }
                let keys = self.machines.read(cx).order_keys(ThreadSection::Active, cx);
                let assignments = order_key::plan_reorder(&order, &keys, key);
                self.write_order_keys(ThreadSection::Active, assignments, cx);
            }
        }
    }

    /// Writes new order keys to each thread's machine.
    fn write_order_keys(
        &self,
        section: ThreadSection,
        assignments: Vec<(ThreadKey, String)>,
        cx: &mut Context<Self>,
    ) {
        let mut by_machine: HashMap<MachineId, Vec<_>> = HashMap::default();
        for (key, order_key) in assignments {
            by_machine
                .entry(key.machine)
                .or_default()
                .push((key.thread, order_key));
        }
        for (machine, keys) in by_machine {
            if let Some(store) = self.store(machine, cx) {
                store.update(cx, |store, cx| store.reorder_threads(section, keys, cx));
            }
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
        let faint_icon = Color::Custom(cx.theme().colors().text_muted.opacity(0.6));
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
            (thread_agent_icon(machine, thread, faint_icon, cx), label)
        });
        let store = machines.projects(machine, cx);
        let store = store.as_ref().map(|store| store.read(cx));
        // A terminal's agent CLI, by name.
        let agent = agent.or_else(|| {
            let name = store?.terminal_agent(thread.id)?;
            Some((
                Icon::new(IconName::Terminal).color(faint_icon),
                SharedString::from(name.to_string()),
            ))
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
            subthreads: store.and_then(|store| {
                let (count, running) = subthread_counts(store, thread.id);
                subthreads_label(count, running).map(SharedString::from)
            }),
            contents: None,
        }
    }

    /// t3code's thread card: the project and status on top, then the title, then the branch with
    /// the agent's icon at the bottom right. `raised` is the copy held above the list while the
    /// card is dragged, which only shows, with what letting go would do.
    fn render_thread_card(
        &self,
        store: &Entity<ProjectStore>,
        thread: Thread,
        project: Option<Project>,
        raised: Option<Option<DropVerb>>,
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
        let is_renaming = raised.is_none() && self.renaming_thread == Some(thread_id);
        // Cards pass under the pointer while one is dragged.
        let is_dragging = self.thread_drag.is_some();
        let drop_verb = raised.flatten();
        // The open thread's composer shows its text already.
        let has_unsent_text = thread.unsent_text.is_some() && !is_active;
        let thread_status = store.read(cx).thread_status(thread.id);
        let faint_icon = Color::Custom(colors.text_muted.opacity(0.6));
        let icon = thread_agent_icon(machine, &thread, faint_icon, cx);
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
        // Which machine it runs on, just before the agent: the only place a card names it.
        let machine_icon = Icon::new(if is_offline {
            IconName::Disconnected
        } else {
            machines.machine_icon(machine, cx)
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
        let subthreads = subthread_counts(store.read(cx), thread.id);
        let started_by = thread
            .created_by
            .map(|creator| format!("Started by {}", store.read(cx).describe_creator(creator)));

        let status = match (drop_verb, thread_status) {
            (Some(verb), _) => render_drop_badge(verb, cx),
            (None, Some(ThreadStatus::Working)) => h_flex()
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
            (None, Some(status)) => render_status_pill(status, cx).into_any_element(),
            (None, None) => Label::new(time.unwrap_or_default())
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
        let has_hover_buttons = !is_renaming && !is_dragging && (is_archivable || has_unsent_text);
        // t3code's pin, before the status: it unpins from the hover buttons, where it moves
        // over with them. A dragged card shows its badge instead, over another section.
        let pin = (thread.is_pinned() && drop_verb.is_none()).then(|| {
            div()
                .flex_none()
                .debug_selector(|| format!("pin-mark-{}", thread_id.thread.0))
                .when(has_hover_buttons, |this| {
                    this.group_hover(group_name.clone(), |this| this.invisible())
                })
                .child(
                    Icon::new(IconName::Pin)
                        .size(IconSize::XSmall)
                        .color(Color::Custom(muted_text.opacity(0.65))),
                )
        });
        let unpin_button = (thread.is_pinned() && has_hover_buttons).then(|| {
            let button_group =
                SharedString::from(format!("unpin-button-{}-{}", machine.slug(), thread.id.0));
            div()
                .id(thread_element_id("unpin-thread", thread_id))
                .debug_selector(|| format!("unpin-thread-{}", thread_id.thread.0))
                .group(button_group.clone())
                .flex()
                .items_center()
                .h_full()
                .px_1()
                .cursor_pointer()
                .tooltip(Tooltip::text("Unpin thread"))
                .child(
                    svg()
                        .path(IconName::Pin.path())
                        .size(IconSize::XSmall.rems())
                        .flex_none()
                        .text_color(muted_text.opacity(0.65))
                        .group_hover(button_group, |this| this.text_color(bright_text)),
                )
                .on_hover(cx.listener(move |this, hovered, _, cx| {
                    if *hovered {
                        this.hide_details(cx);
                    } else if this.hovered_thread == Some(thread_id) {
                        this.thread_hovered(thread_id, true, cx);
                    }
                }))
                .on_click({
                    let store = store.clone();
                    move |_, _, cx| {
                        cx.stop_propagation();
                        store.update(cx, |store, cx| store.unpin_thread(thread_id.thread, cx));
                    }
                })
        });
        // The status yields to the Archive and Discard buttons on hover.
        let status_slot = div()
            .flex_none()
            .when(has_hover_buttons, |this| {
                this.group_hover(group_name.clone(), |this| this.invisible())
            })
            .child(status);
        // The buttons cover the end of the title in the card's own color, fading in from it,
        // so a long title doesn't show through them.
        let cover = colors.panel_background.blend(if is_active {
            selected_background
        } else {
            hover_background
        });
        let hover_buttons = has_hover_buttons.then(|| {
            // Centered on its line, like t3code's Settle button.
            h_flex()
                .absolute()
                .top_0()
                .bottom_0()
                .right_0()
                .visible_on_hover(group_name.clone())
                .child(div().w_6().h_full().bg(gpui::linear_gradient(
                    90.,
                    gpui::linear_color_stop(cover, 1.),
                    gpui::linear_color_stop(cover.opacity(0.), 0.),
                )))
                .child(
                    h_flex()
                        .h_full()
                        .bg(cover)
                        .children(discard_button)
                        .children(unpin_button)
                        .when(is_archivable, |this| this.child(archive_button)),
                )
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
                    h_flex().flex_1().min_w_0().children(
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
                    ),
                )
                .children(pin)
                .child(status_slot)
                .children(hover_buttons);
            let title_line = h_flex().mt_1().min_w_0().child(title_element);
            (Some(project_line), title_line)
        } else {
            let title_line = h_flex()
                .relative()
                .min_w_0()
                .gap_1p5()
                .children(unsent_marker)
                .child(title_element)
                .children(pin)
                .child(status_slot)
                .children(hover_buttons);
            (None, title_line)
        };

        let card = v_flex()
            .id(thread_element_id("thread-card", thread_id))
            .debug_selector(|| format!("thread-card-{}", thread_id.thread.0))
            .group(group_name)
            .when(raised.is_none(), |card| {
                card.on_hover(cx.listener(move |this, hovered, _, cx| {
                    this.thread_hovered(thread_id, *hovered, cx)
                }))
                .on_any_mouse_down(cx.listener(|this, _, _, cx| this.hide_details(cx)))
            })
            .relative()
            .w_full()
            .when(shows_all_projects, |card| card.h(CARD_HEIGHT))
            .px_2p5()
            .py_2()
            .rounded_md()
            .when(is_active, |card| card.bg(selected_background))
            .when(!is_renaming && raised.is_none(), |card| {
                card.cursor_pointer()
                    .when(!is_dragging, |card| {
                        card.hover(|card| card.bg(hover_background))
                    })
                    .on_click(self.thread_click_handler(thread_id, title.clone(), cx))
                    .on_drag(DraggedThread(thread_id), {
                        let this = cx.entity().downgrade();
                        move |dragged, _, window, cx| {
                            let pointer = window.mouse_position().y;
                            this.update(cx, |this, cx| {
                                this.start_thread_drag(dragged.0, pointer, None, cx)
                            })
                            .ok();
                            cx.new(|_| *dragged)
                        }
                    })
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
                        let color = if running > 0 {
                            Color::Accent
                        } else {
                            Color::Custom(faint_text)
                        };
                        // The details popover says what the count is.
                        this.child(
                            h_flex()
                                .flex_none()
                                .gap_0p5()
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
                    .child(div().flex_none().child(icon.size(IconSize::Small))),
            );
        if raised.is_some() {
            return card.into_any_element();
        }

        // The details popover stays hidden while the thread's menu is open.
        let details_popover = (self.details_thread == Some(thread_id) && !is_dragging)
            .then(|| render_details_popover(details, cx));
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
        tone: HeaderTone,
        on_toggle: impl Fn(&mut Self, &mut Context<Self>) + 'static,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let (color, rule_color) = tone.colors(cx);
        h_flex()
            .id(id)
            .debug_selector(|| id.into())
            .h_8()
            .mx_0p5()
            .px_2()
            .gap_2()
            .cursor_pointer()
            .child(
                // Archived shows while a thread is dragged, even with nothing in it.
                Label::new(if is_expanded || count == 0 {
                    label.to_string()
                } else {
                    format!("{label} ({count})")
                })
                .size(LabelSize::Small)
                .weight(FontWeight::MEDIUM)
                .color(color),
            )
            .child(div().flex_1().min_w_2().h_px().bg(rule_color))
            .child(
                Icon::new(if is_expanded {
                    IconName::ChevronUp
                } else {
                    IconName::ChevronDown
                })
                .size(IconSize::XSmall)
                .color(color),
            )
            .on_click(cx.listener(move |this, _, _, cx| on_toggle(this, cx)))
            .into_any_element()
    }

    /// t3code's slim row for parked threads: the project's icon, dimmed until hovered, and for
    /// an archived thread a way back on hover. A shell's row adds where it works, and shows
    /// what runs in it in place of its last activity. A Workspaces thread's is one line, like
    /// an archived one's, with the icon of the project its folder is in. `raised` is an
    /// archived row's copy held above the list while it's dragged, as a card's.
    fn render_slim_row(
        &self,
        store: &Entity<ProjectStore>,
        thread: Thread,
        shelf: Shelf,
        raised: Option<Option<DropVerb>>,
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
        let is_renaming = raised.is_none() && self.renaming_thread == Some(thread_id);
        let is_dragging = self.thread_drag.is_some();
        let drop_verb = raised.flatten();
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
                time.filter(|_| !is_renaming && running.is_none() && drop_verb.is_none()),
                |row, time| {
                    row.child(
                        div()
                            .flex_none()
                            .when(is_archived && !is_dragging, |this| {
                                this.group_hover(group_name.clone(), |this| this.invisible())
                            })
                            .child(Label::new(time).size(LabelSize::Small).color(Color::Muted)),
                    )
                },
            )
            .children(drop_verb.map(|verb| render_drop_badge(verb, cx)))
            .when(is_archived && !is_renaming && !is_dragging, |row| {
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
        let row = v_flex()
            .id(thread_element_id(&format!("{prefix}-thread"), thread_id))
            .debug_selector(|| format!("{prefix}-row-{}", thread_id.thread.0))
            .group(group_name)
            .when(raised.is_none(), |row| {
                row.on_hover(cx.listener(move |this, hovered, _, cx| {
                    this.thread_hovered(thread_id, *hovered, cx)
                }))
                .on_any_mouse_down(cx.listener(|this, _, _, cx| this.hide_details(cx)))
            })
            .relative()
            .when(!is_shell, |row| row.h(ARCHIVED_ROW_HEIGHT))
            .when(is_shell, |row| row.py_1p5())
            .w_full()
            .px_2p5()
            .rounded_md()
            .when(is_active, |row| row.bg(selected_background))
            .when(!is_renaming && raised.is_none(), |row| {
                row.cursor_pointer()
                    .when(!is_dragging, |row| {
                        row.hover(|row| row.bg(hover_background))
                    })
                    .on_click(self.thread_click_handler(thread_id, title.clone(), cx))
            })
            // An archived thread comes back by dragging it up among the cards.
            .when(
                is_archived && !is_renaming && raised.is_none() && thread.can_arrange(),
                |row| {
                    row.on_drag(DraggedThread(thread_id), {
                        let this = cx.entity().downgrade();
                        move |dragged, grab: gpui::Point<Pixels>, window, cx| {
                            let pointer = window.mouse_position().y;
                            this.update(cx, |this, cx| {
                                this.start_thread_drag(dragged.0, pointer, Some(grab.y), cx)
                            })
                            .ok();
                            cx.new(|_| *dragged)
                        }
                    })
                },
            )
            .when(is_offline, |row| row.opacity(0.5))
            .child(main_line)
            .children(detail_line);
        if raised.is_some() {
            return row.into_any_element();
        }

        // The details popover stays hidden while the thread's menu is open.
        let details_popover = (self.details_thread == Some(thread_id) && !is_dragging)
            .then(|| render_details_popover(details, cx));
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

    /// Only says so: the main area's Welcome page has Open Folder….
    fn render_empty_state(&self) -> impl IntoElement {
        v_flex()
            .flex_1()
            .items_center()
            .justify_center()
            .p_4()
            .child(Label::new("No projects yet").color(Color::Muted))
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
        self.archived_top.set(None);
        // While searching, t3code swaps the list for the matching threads.
        if !self.search_query(cx).is_empty() {
            return self.render_search_results(window, cx);
        }
        let drag = self.thread_drag.as_ref();
        let machines = self.machines.read(cx);
        let archived = machines.archived_threads(cx);
        let is_archived_expanded = machines.archived_expanded(cx);

        // t3code's draft block: interrupted new threads, one click away above the rest.
        let mut draft_rows: Vec<AnyElement> = self
            .shown_drafts(cx)
            .into_iter()
            .map(|(store, thread)| self.render_draft_row(&store, thread, cx))
            .collect();
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

        let cards = self.shown_cards(cx);
        let mut rows = Vec::with_capacity(cards.len() + 2);
        match drag {
            None => {
                for (store, thread) in cards {
                    let project = store.read(cx).project(thread.project_id).cloned();
                    rows.push(self.render_thread_card(&store, thread, project, None, cx));
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
            }
            // The cards in the order the drag shows them, sliding over to make room for the
            // dragged one and the labels.
            Some(drag) => {
                let target = drag.target();
                let mut cards: HashMap<ThreadKey, (Entity<ProjectStore>, Thread)> = cards
                    .into_iter()
                    .map(|(store, thread)| {
                        let key = ThreadKey {
                            machine: store.read(cx).machine(),
                            thread: thread.id,
                        };
                        (key, (store, thread))
                    })
                    .collect();
                let now = Instant::now();
                let mut is_sliding = false;
                for item in &drag.slide.order {
                    let row = match item {
                        DragItem::PinnedLabel => {
                            render_drag_label("Pinned", target == DragSection::Pinned, cx)
                        }
                        DragItem::ActiveLabel => {
                            render_drag_label("Active", target == DragSection::Active, cx)
                        }
                        // Its place, kept while it's held above.
                        DragItem::Thread(key) if *key == drag.thread => div()
                            .debug_selector(|| "dragged-thread-place".into())
                            .flex_none()
                            .h(drag.slide.length(*item))
                            .into_any_element(),
                        DragItem::Thread(key) => {
                            let Some((store, thread)) = cards.remove(key) else {
                                continue;
                            };
                            let project = store.read(cx).project(thread.project_id).cloned();
                            self.render_thread_card(&store, thread, project, None, cx)
                        }
                    };
                    let offset = drag.slide.offset(*item, now);
                    is_sliding |= offset != px(0.);
                    rows.push(div().relative().top(offset).child(row).into_any_element());
                }
                if is_sliding {
                    window.request_animation_frame();
                }
            }
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
                HeaderTone::Muted,
                |this, cx| {
                    this.shells_expanded = !this.shells_expanded;
                    cx.notify();
                },
                cx,
            ));
            if self.shells_expanded {
                for (machine, thread) in shells {
                    if let Some(store) = self.store(machine, cx) {
                        shelf.push(self.render_slim_row(&store, thread, Shelf::Shells, None, cx));
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
                HeaderTone::Muted,
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
                        shelf.push(self.render_slim_row(
                            &store,
                            thread,
                            Shelf::Workspaces,
                            None,
                            cx,
                        ));
                    }
                }
            }
        }
        // While a thread that can be archived is held, Archived reads at full strength, even
        // with nothing in it, and takes the accent with the thread over it.
        let archive_tone = match drag.filter(|drag| !drag.is_agent_cli) {
            Some(drag) if drag.target() == DragSection::Archived => Some(HeaderTone::Accent),
            Some(_) => Some(HeaderTone::Emphasized),
            None => None,
        };
        let archived_count = archived.len();
        if archived_count > 0 || archive_tone.is_some() {
            let mut archived_rows = vec![Self::render_shelf_header(
                "archived-shelf-toggle",
                "Archived",
                archived_count,
                is_archived_expanded,
                archive_tone.unwrap_or(HeaderTone::Muted),
                |this, cx| {
                    // Kept by this Mac's server, like the thread order.
                    if let Some(store) = this.store(MachineId::Local, cx) {
                        store.update(cx, |store, cx| store.toggle_archived_expanded(cx));
                    }
                },
                cx,
            )];
            if is_archived_expanded {
                let hidden_count = archived_count.saturating_sub(self.archived_shown);
                for (machine, thread) in archived.into_iter().take(self.archived_shown) {
                    let key = ThreadKey {
                        machine,
                        thread: thread.id,
                    };
                    match drag.filter(|drag| drag.thread == key) {
                        // A dragged row's place stays open while it's over Archived, and moves
                        // among the cards with it.
                        Some(drag) if drag.over_archived => archived_rows
                            .push(div().flex_none().h(ARCHIVED_ROW_HEIGHT).into_any_element()),
                        Some(_) => {}
                        None => {
                            if let Some(store) = self.store(machine, cx) {
                                archived_rows.push(self.render_slim_row(
                                    &store,
                                    thread,
                                    Shelf::Archived,
                                    None,
                                    cx,
                                ));
                            }
                        }
                    }
                }
                if hidden_count > 0 {
                    archived_rows.push(self.render_show_more_archived(hidden_count, cx));
                }
            }
            let archived_top = self.archived_top.clone();
            shelf.push(
                v_flex()
                    .relative()
                    .child(
                        canvas(
                            move |bounds, _, _| archived_top.set(Some(bounds.top())),
                            |_, _, _, _| {},
                        )
                        .absolute()
                        .size_full(),
                    )
                    .children(archived_rows)
                    .into_any_element(),
            );
        }

        v_flex()
            .id("sidebar-threads")
            .flex_1()
            .min_h_0()
            .px_1()
            .pt_1()
            .pb_2()
            .overflow_y_scroll()
            .track_scroll(&self.list_scroll)
            .on_drag_move(
                cx.listener(|this, event: &DragMoveEvent<DraggedThread>, _, cx| {
                    this.drag_thread(event.event.position.y, cx)
                }),
            )
            .children(draft_rows)
            .children(rows)
            .when(!shelf.is_empty(), |list| {
                // Like t3code, the shelf rests at the bottom while the list is short.
                list.child(v_flex().mt_auto().pt_2().children(shelf))
            })
            .into_any_element()
    }

    /// The dragged card or row, raised above the list where the pointer holds it, saying what
    /// letting go does.
    fn render_raised_thread(&self, cx: &mut Context<Self>) -> Option<impl IntoElement + use<>> {
        let drag = self.thread_drag.as_ref()?;
        let store = self.store(drag.thread.machine, cx)?;
        let thread = store.read(cx).thread(drag.thread.thread)?.clone();
        let verb = drag.verb();
        // A card sits in the space around it, and an archived row fills its own.
        let (content, inset) = if drag.from == DragSection::Archived {
            (
                self.render_slim_row(&store, thread, Shelf::Archived, Some(verb), cx),
                px(0.),
            )
        } else {
            let project = store.read(cx).project(thread.project_id).cloned();
            (
                self.render_thread_card(&store, thread, project, Some(verb), cx),
                px(2.),
            )
        };
        let list = self.list_scroll.bounds();
        let top = drag
            .slide
            .held_start()
            .clamp(list.top(), (list.bottom() - drag.height).max(list.top()));
        // `render_raised_rows` draws its background inset from the sides, as workspace rows
        // are, so it spreads past the card's sides by as much, and the card keeps its width.
        Some(
            deferred(
                anchored()
                    .position(gpui::point(drag.left - px(4.), top + inset))
                    .child(render_raised_rows(
                        vec![div().px_1().child(content).into_any_element()],
                        drag.width + px(8.),
                        cx,
                    )),
            )
            .with_priority(1),
        )
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
            .id("sidebar")
            .w(SIDEBAR_WIDTH)
            .h_full()
            .flex_none()
            .border_r_1()
            .border_color(border)
            .bg(panel_background)
            // A dragged thread is dropped wherever the pointer is, even past the sidebar's
            // edge, as a dragged workspace row is.
            .on_drop(cx.listener(|this, _: &DraggedThread, _, cx| this.drop_thread(cx)))
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| this.drop_thread(cx)),
            )
            .child(self.render_header(cx))
            .child(if has_projects {
                self.render_threads(window, cx)
            } else {
                self.render_empty_state().into_any_element()
            })
            .children(self.render_raised_thread(cx))
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
    /// The agent's icon, colored, and its label.
    pub(crate) agent: Option<(Icon, SharedString)>,
    /// How many subthreads the thread has, and how many of them run.
    pub(crate) subthreads: Option<SharedString>,
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
        if let Some((icon, label)) = self.agent {
            rows.push(detail_row(
                icon.size(IconSize::XSmall).into_any_element(),
                Label::new(label).truncate(),
            ));
        }
        if let Some(subthreads) = &self.subthreads {
            rows.push(detail_row(
                small_icon(IconName::UserGroup),
                Label::new(subthreads.clone()).truncate(),
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

/// How a shelf header or a drag label reads: muted, at full strength while a thread is
/// dragged, or in the accent where it would land (t3code's header tones).
#[derive(Clone, Copy, PartialEq)]
enum HeaderTone {
    Muted,
    Emphasized,
    Accent,
}

impl HeaderTone {
    /// The label's color and the rule's.
    fn colors(self, cx: &App) -> (Color, Hsla) {
        let colors = cx.theme().colors();
        match self {
            Self::Muted => (Color::Muted, colors.border_variant),
            Self::Emphasized => (
                Color::Custom(colors.text.opacity(0.8)),
                colors.text.opacity(0.25),
            ),
            Self::Accent => (Color::Accent, Color::Accent.color(cx).opacity(0.5)),
        }
    }
}

/// t3code's drag boundary: "Pinned" over the pinned cards or "Active" over the rest, with a
/// rule, open while a thread is dragged.
fn render_drag_label(label: &'static str, is_target: bool, cx: &App) -> AnyElement {
    let tone = if is_target {
        HeaderTone::Accent
    } else {
        HeaderTone::Emphasized
    };
    let (color, rule_color) = tone.colors(cx);
    h_flex()
        .debug_selector(move || format!("drag-label-{label}"))
        .h(DRAG_LABEL_HEIGHT)
        .mx_0p5()
        .px_2()
        .gap_2()
        .child(
            Label::new(label)
                .size(LabelSize::Small)
                .weight(FontWeight::MEDIUM)
                .color(color),
        )
        .child(div().flex_1().h_px().bg(rule_color))
        .into_any_element()
}

/// What letting go of a dragged thread does, in the accent, in place of its time or state.
fn render_drop_badge(verb: DropVerb, cx: &App) -> AnyElement {
    let accent = Color::Accent.color(cx);
    h_flex()
        .debug_selector(move || format!("drop-badge-{}", verb.label()))
        .flex_none()
        .h_5()
        .px_1p5()
        .gap_1()
        .rounded_sm()
        .border_1()
        .border_color(accent.opacity(0.4))
        .bg(accent.opacity(0.1))
        .child(
            Icon::new(verb.icon())
                .size(IconSize::XSmall)
                .color(Color::Accent),
        )
        .child(
            Label::new(verb.label())
                .size(LabelSize::Small)
                .weight(FontWeight::MEDIUM)
                .color(Color::Accent),
        )
        .into_any_element()
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
            store
                .read(cx)
                .set_unsent_text(thread_id.thread, None, Vec::new(), cx);
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
    use super::{format_relative_time, repository_branch, subthreads_label};

    #[test]
    fn subthreads_are_counted_with_those_running() {
        assert_eq!(subthreads_label(0, 0), None);
        assert_eq!(subthreads_label(1, 0).as_deref(), Some("1 subthread"));
        assert_eq!(subthreads_label(3, 0).as_deref(), Some("3 subthreads"));
        assert_eq!(
            subthreads_label(3, 1).as_deref(),
            Some("3 subthreads, 1 running")
        );
    }

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
    use std::time::{Duration, SystemTime};

    use agentz_protocol::Request;
    use agentz_protocol::spaces::SpacesSnapshot;
    use gpui::{
        Bounds, Entity, Modifiers, MouseButton, Pixels, Point, TestAppContext, VisualTestContext,
        point, px,
    };
    use projects::{
        Project, ProjectId, ProjectsSnapshot, Thread, ThreadId, ThreadOrder, ThreadSection,
    };
    use serde_json::json;

    use super::{DRAG_LABEL_HEIGHT, DragSection, Sidebar, ThreadDrag};
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
        show_snapshot(
            store,
            ProjectsSnapshot {
                threads,
                workspaces_expanded,
                ..Default::default()
            },
            cx,
        )
    }

    /// Shows `snapshot`, with the demo project its threads are in.
    fn show_snapshot(
        store: &Entity<ProjectStore>,
        snapshot: ProjectsSnapshot,
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
                        ..snapshot
                    },
                    cx,
                )
            })
        });
        cx.run_until_parked();
    }

    fn pinned(mut thread: Thread, order_key: &str) -> Thread {
        thread.pinned_at = Some(SystemTime::UNIX_EPOCH);
        thread.pin_order_key = Some(order_key.to_string());
        thread
    }

    fn bounds(cx: &mut VisualTestContext, name: &'static str) -> Bounds<Pixels> {
        cx.debug_bounds(name)
            .unwrap_or_else(|| panic!("{name} is drawn"))
    }

    /// The requests that pin, unpin, arrange, archive or unarchive threads.
    fn organizing_requests(cx: &mut VisualTestContext) -> Vec<Request> {
        cx.update(|_, cx| {
            let Some(client) = Machines::global(cx).read(cx).client(MachineId::Local, cx) else {
                return Vec::new();
            };
            client
                .read(cx)
                .sent_for_test()
                .into_iter()
                .filter(|request| {
                    matches!(
                        request,
                        Request::PinThread { .. }
                            | Request::UnpinThread(_)
                            | Request::ReorderThreads { .. }
                            | Request::ArchiveThread(_)
                            | Request::UnarchiveThread(_)
                    )
                })
                .collect()
        })
    }

    /// Presses on `at` and moves a little, which picks up what's there. Returns where the
    /// pointer is then.
    fn pick_up(at: Point<Pixels>, cx: &mut VisualTestContext) -> Point<Pixels> {
        cx.simulate_mouse_down(at, MouseButton::Left, Modifiers::none());
        let start = at + point(px(0.), px(4.));
        cx.simulate_mouse_move(start, MouseButton::Left, Modifiers::none());
        start
    }

    fn hold(at: Point<Pixels>, cx: &mut VisualTestContext) {
        cx.simulate_mouse_move(at, MouseButton::Left, Modifiers::none());
        cx.simulate_mouse_move(at, MouseButton::Left, Modifiers::none());
    }

    fn let_go(at: Point<Pixels>, cx: &mut VisualTestContext) {
        cx.simulate_mouse_up(at, MouseButton::Left, Modifiers::none());
        cx.run_until_parked();
    }

    fn drag_target(sidebar: &Entity<Sidebar>, cx: &mut VisualTestContext) -> Option<DragSection> {
        sidebar.read_with(cx, |sidebar, _| {
            sidebar.thread_drag.as_ref().map(ThreadDrag::target)
        })
    }

    /// The threads given order keys in `section`, in the order of their keys.
    fn reordered(request: &Request, section: ThreadSection) -> Vec<u64> {
        let Request::ReorderThreads {
            section: written,
            keys,
        } = request
        else {
            panic!("a reorder, not {request:?}");
        };
        assert_eq!(*written, section);
        let mut keys = keys.clone();
        keys.sort_by(|(_, a), (_, b)| a.cmp(b));
        keys.into_iter().map(|(thread, _)| thread.0).collect()
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

    /// A thread's agent icon takes its account's color while the agent has another account,
    /// and only once that account has a color.
    #[gpui::test]
    fn colors_a_threads_agent_icon_by_its_account(cx: &mut TestAppContext) {
        use agentz_protocol::accounts::{AccountChange, AgentAccounts};
        use agentz_protocol::agents::AgentId;

        use super::thread_account_color;
        use crate::controls::account_color;

        let (_, _, cx) = new_sidebar(cx);
        let set_accounts = |accounts: &AgentAccounts, cx: &mut VisualTestContext| {
            cx.update(|_, cx| {
                let client = Machines::global(cx)
                    .read(cx)
                    .client(MachineId::Local, cx)
                    .expect("this Mac's client");
                client.update(cx, |client, cx| {
                    client.set_accounts_for_test(
                        [(AgentId::new("mock"), accounts.clone())].into(),
                        cx,
                    )
                });
            });
        };
        let color_of = |thread: &Thread, cx: &mut VisualTestContext| {
            cx.update(|_, cx| thread_account_color(MachineId::Local, thread, cx))
        };
        let in_theme =
            |hex: &str, cx: &mut VisualTestContext| cx.update(|_, cx| account_color(hex, cx));
        let blue = "#2563eb";
        let green = "#16a34a";

        let mut accounts = AgentAccounts {
            external_logged_in: Some(true),
            ..AgentAccounts::default()
        };
        accounts
            .change(None, AccountChange::SetColor(Some(blue.into())))
            .expect("color");
        let outside = thread(1, false, None);
        set_accounts(&accounts, cx);
        assert_eq!(color_of(&outside, cx), None);

        let work = accounts.add();
        let side = accounts.add();
        accounts
            .change(Some(work), AccountChange::SetColor(Some(green.into())))
            .expect("color");
        set_accounts(&accounts, cx);
        let mut on_work = thread(2, false, None);
        on_work.account = Some(work);
        let mut on_side = thread(3, false, None);
        on_side.account = Some(side);
        assert!(in_theme(blue, cx).is_some());
        assert_eq!(color_of(&outside, cx), in_theme(blue, cx));
        assert_eq!(color_of(&on_work, cx), in_theme(green, cx));
        assert_eq!(color_of(&on_side, cx), None);

        // A terminal thread has no agent, so no account.
        let mut terminal = thread(4, false, None);
        terminal.agent_id = None;
        assert_eq!(color_of(&terminal, cx), None);
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

    #[gpui::test]
    fn a_pinned_card_leads_and_its_pin_unpins_it(cx: &mut TestAppContext) {
        let (_, store, cx) = new_sidebar(cx);
        show(
            &store,
            vec![pinned(thread(1, false, None), "m"), thread(2, false, None)],
            cx,
        );
        let first = bounds(cx, "thread-card-1");
        assert!(first.top() < bounds(cx, "thread-card-2").top());
        assert!(cx.debug_bounds("pin-mark-1").is_some());
        assert!(cx.debug_bounds("pin-mark-2").is_none());
        assert!(cx.debug_bounds("unpin-thread-2").is_none());

        cx.simulate_mouse_move(first.center(), None, Modifiers::none());
        let unpin = bounds(cx, "unpin-thread-1");
        cx.simulate_click(unpin.center(), Modifiers::none());
        cx.run_until_parked();
        assert_eq!(organizing_requests(cx), [Request::UnpinThread(ThreadId(1))]);
        assert!(cx.debug_bounds("pin-mark-1").is_none());
        assert!(bounds(cx, "thread-card-2").top() < bounds(cx, "thread-card-1").top());
    }

    #[gpui::test]
    fn a_card_dragged_up_among_the_pinned_ones_is_pinned_where_it_is_let_go(
        cx: &mut TestAppContext,
    ) {
        let (sidebar, store, cx) = new_sidebar(cx);
        // Shown 1, then the rest newest first: 3, 2.
        show(
            &store,
            vec![
                pinned(thread(1, false, None), "m"),
                thread(2, false, None),
                thread(3, false, None),
            ],
            cx,
        );
        let first = bounds(cx, "thread-card-1");
        let last = bounds(cx, "thread-card-2");
        assert!(cx.debug_bounds("drag-label-Pinned").is_none());

        let start = pick_up(last.center(), cx);
        // The labels open where the cards started, and the card is raised where it was.
        let label = bounds(cx, "drag-label-Pinned");
        assert_eq!(label.top(), first.top() - px(2.));
        assert_eq!(label.size.height, DRAG_LABEL_HEIGHT);
        assert!(cx.debug_bounds("drag-label-Active").is_some());
        let raised = bounds(cx, "thread-card-2");
        assert_eq!(raised.top(), last.top());
        assert_eq!(
            (raised.left(), raised.size.width),
            (last.left(), last.size.width)
        );
        assert_eq!(drag_target(&sidebar, cx), Some(DragSection::Active));
        assert!(cx.debug_bounds("drop-badge-Pin").is_none());

        // Up to the top, past the pinned card.
        let held = point(start.x, start.y - (last.top() - first.top()));
        hold(held, cx);
        assert_eq!(drag_target(&sidebar, cx), Some(DragSection::Pinned));
        assert!(cx.debug_bounds("drop-badge-Pin").is_some());

        let_go(held, cx);
        let requests = organizing_requests(cx);
        let [
            Request::PinThread {
                thread_id,
                order_key: Some(order_key),
            },
        ] = requests.as_slice()
        else {
            panic!("one pin: {requests:?}");
        };
        assert_eq!(*thread_id, ThreadId(2));
        assert!(order_key.as_str() < "m", "{order_key} goes before m");
        // It shows there at once, and the labels close.
        assert!(cx.debug_bounds("drag-label-Pinned").is_none());
        assert_eq!(bounds(cx, "thread-card-2").top(), first.top());
        assert!(cx.debug_bounds("pin-mark-2").is_some());
        assert!(bounds(cx, "thread-card-1").top() < bounds(cx, "thread-card-3").top());
    }

    #[gpui::test]
    fn a_pinned_card_dragged_among_the_rest_is_unpinned_and_stays_where_it_is_let_go(
        cx: &mut TestAppContext,
    ) {
        let (sidebar, store, cx) = new_sidebar(cx);
        show(
            &store,
            vec![
                pinned(thread(1, false, None), "m"),
                thread(2, false, None),
                thread(3, false, None),
            ],
            cx,
        );
        let first = bounds(cx, "thread-card-1");
        let second = bounds(cx, "thread-card-3");
        let step = second.top() - first.top();

        // Down between 3 and 2, past the Active label.
        let start = pick_up(first.center(), cx);
        hold(point(start.x, start.y + step + DRAG_LABEL_HEIGHT), cx);
        assert_eq!(drag_target(&sidebar, cx), Some(DragSection::Active));
        assert!(cx.debug_bounds("drop-badge-Unpin").is_some());

        let_go(point(start.x, start.y + step + DRAG_LABEL_HEIGHT), cx);
        let requests = organizing_requests(cx);
        assert_eq!(requests.len(), 2, "{requests:?}");
        assert_eq!(requests[0], Request::UnpinThread(ThreadId(1)));
        // The rest had no keys of their own, so they all get one.
        assert_eq!(reordered(&requests[1], ThreadSection::Active), [3, 1, 2]);
        assert_eq!(bounds(cx, "thread-card-3").top(), first.top());
        assert_eq!(bounds(cx, "thread-card-1").top(), second.top());
        assert!(cx.debug_bounds("pin-mark-1").is_none());
    }

    #[gpui::test]
    fn a_card_dragged_onto_archived_is_archived(cx: &mut TestAppContext) {
        let (sidebar, store, cx) = new_sidebar(cx);
        show(
            &store,
            vec![thread(1, false, None), thread(2, false, None)],
            cx,
        );
        // With nothing archived, Archived shows only while a card is held.
        assert!(cx.debug_bounds("archived-shelf-toggle").is_none());
        let card = bounds(cx, "thread-card-1");
        let start = pick_up(card.center(), cx);
        let header = bounds(cx, "archived-shelf-toggle");
        assert_eq!(drag_target(&sidebar, cx), Some(DragSection::Active));

        let held = point(start.x, header.center().y);
        hold(held, cx);
        assert_eq!(drag_target(&sidebar, cx), Some(DragSection::Archived));
        assert!(cx.debug_bounds("drop-badge-Archive").is_some());

        let_go(held, cx);
        assert_eq!(
            organizing_requests(cx),
            [Request::ArchiveThread(ThreadId(1))]
        );
        assert!(cx.debug_bounds("thread-card-1").is_none());
        // Folded, it stays folded.
        assert!(cx.debug_bounds("archived-shelf-toggle").is_some());
        assert!(cx.debug_bounds("archived-row-1").is_none());
    }

    #[gpui::test]
    fn an_archived_row_dragged_among_the_cards_is_unarchived_there(cx: &mut TestAppContext) {
        let (sidebar, store, cx) = new_sidebar(cx);
        let mut archived = thread(3, false, None);
        archived.archived_at = Some(SystemTime::UNIX_EPOCH);
        show_snapshot(
            &store,
            ProjectsSnapshot {
                threads: vec![thread(1, false, None), thread(2, false, None), archived],
                archived_expanded: true,
                ..Default::default()
            },
            cx,
        );
        // Shown 2, 1.
        let first = bounds(cx, "thread-card-2");
        let second = bounds(cx, "thread-card-1");
        let row = bounds(cx, "archived-row-3");

        let start = pick_up(row.center(), cx);
        assert_eq!(drag_target(&sidebar, cx), Some(DragSection::Archived));
        assert!(cx.debug_bounds("drop-badge-Unarchive").is_none());
        // Its top where the second card's place is, under the labels.
        let held = point(
            start.x,
            start.y - (row.top() - second.top()) + DRAG_LABEL_HEIGHT * 2.,
        );
        hold(held, cx);
        assert_eq!(drag_target(&sidebar, cx), Some(DragSection::Active));
        assert!(cx.debug_bounds("drop-badge-Unarchive").is_some());

        let_go(held, cx);
        let requests = organizing_requests(cx);
        assert_eq!(requests.len(), 2, "{requests:?}");
        assert_eq!(requests[0], Request::UnarchiveThread(ThreadId(3)));
        assert_eq!(reordered(&requests[1], ThreadSection::Active), [2, 3, 1]);
        assert!(cx.debug_bounds("archived-row-3").is_none());
        assert_eq!(bounds(cx, "thread-card-2").top(), first.top());
        assert_eq!(bounds(cx, "thread-card-3").top(), second.top());
    }

    #[gpui::test]
    fn latest_active_first_the_rest_keep_their_order(cx: &mut TestAppContext) {
        let (sidebar, store, cx) = new_sidebar(cx);
        let active = |id: u64, seconds: u64| {
            let mut thread = thread(id, false, None);
            thread.last_activity_at = Some(SystemTime::UNIX_EPOCH + Duration::from_secs(seconds));
            thread
        };
        show_snapshot(
            &store,
            ProjectsSnapshot {
                threads: vec![active(1, 30), active(2, 20), active(3, 10)],
                thread_order: ThreadOrder::LastActivity,
                ..Default::default()
            },
            cx,
        );
        let first = bounds(cx, "thread-card-1");
        let last = bounds(cx, "thread-card-3");

        // Held above the others it stays last among them, so letting go there does nothing.
        let start = pick_up(last.center(), cx);
        let held = point(start.x, start.y - (last.top() - first.top()) + px(8.));
        hold(held, cx);
        assert_eq!(drag_target(&sidebar, cx), Some(DragSection::Active));
        let shown = sidebar.read_with(cx, |sidebar, _| {
            sidebar
                .thread_drag
                .as_ref()
                .map(|drag| drag.shown(DragSection::Active))
        });
        let key = |thread| ThreadKey {
            machine: MachineId::Local,
            thread: ThreadId(thread),
        };
        assert_eq!(shown, Some(vec![key(1), key(2), key(3)]));
        let_go(held, cx);
        assert_eq!(organizing_requests(cx), []);

        // Above the Active label, it's pinned.
        let start = pick_up(last.center(), cx);
        let held = point(start.x, start.y - (last.top() - first.top()) - px(20.));
        hold(held, cx);
        assert_eq!(drag_target(&sidebar, cx), Some(DragSection::Pinned));
        let_go(held, cx);
        assert!(matches!(
            organizing_requests(cx).as_slice(),
            [Request::PinThread {
                thread_id: ThreadId(3),
                ..
            }]
        ));
    }

    #[gpui::test]
    fn an_agent_cli_card_moves_only_among_the_unpinned_cards(cx: &mut TestAppContext) {
        let (sidebar, store, cx) = new_sidebar(cx);
        let mut terminal = thread(4, false, None);
        terminal.agent_id = None;
        terminal.terminal = Some(projects::TerminalCommand {
            command: Some("claude".to_string()),
        });
        show_snapshot(
            &store,
            ProjectsSnapshot {
                threads: vec![
                    pinned(thread(1, false, None), "m"),
                    thread(2, false, None),
                    terminal,
                ],
                terminal_agents: vec![(ThreadId(4), "Claude Code".to_string())],
                ..Default::default()
            },
            cx,
        );
        let first = bounds(cx, "thread-card-1");
        let card = bounds(cx, "thread-card-4");
        let start = pick_up(card.center(), cx);
        // Terminals aren't archived.
        assert!(cx.debug_bounds("archived-shelf-toggle").is_none());
        let held = point(start.x, start.y - (card.top() - first.top()) - px(40.));
        hold(held, cx);
        assert_eq!(drag_target(&sidebar, cx), Some(DragSection::Active));
        assert!(cx.debug_bounds("drop-badge-Pin").is_none());
        let_go(held, cx);
        assert_eq!(organizing_requests(cx), []);
    }
}

/// The icon of the agent a thread runs, in `color`, or in its account's color
/// ([`thread_account_color`]). A terminal thread's is a terminal.
pub(crate) fn thread_agent_icon(
    machine: MachineId,
    thread: &Thread,
    color: Color,
    cx: &App,
) -> Icon {
    let icon = thread
        .agent_id
        .as_ref()
        .and_then(|agent_id| agent_icon(&AgentId::new(agent_id.clone()), cx))
        .map(Icon::from_svg_markup)
        .unwrap_or_else(|| Icon::new(IconName::Terminal));
    let account_color = thread_account_color(machine, thread, cx);
    icon.color(account_color.map_or(color, Color::Custom))
}

/// The color of the account a thread runs on, while its agent has others
/// ([`AgentAccounts::thread_color`]), in the current theme.
///
/// [`AgentAccounts::thread_color`]: agentz_protocol::accounts::AgentAccounts::thread_color
pub(crate) fn thread_account_color(machine: MachineId, thread: &Thread, cx: &App) -> Option<Hsla> {
    let agent_id = AgentId::new(thread.agent_id.clone()?);
    let client = Machines::global(cx).read(cx).client(machine, cx)?;
    let hex = client.read(cx).thread_color(&agent_id, thread.account)?;
    account_color(hex, cx)
}

/// Purple, apart from the other statuses' colors. Every bundled theme's fourth player color is
/// one, and `Color::Player` skips the first (the local user's).
pub(crate) const AWAITING_INPUT_COLOR: Color = Color::Player(2);

/// The thread's subthreads, and how many of them still run their task.
fn subthread_counts(store: &projects::ProjectStore, thread: projects::ThreadId) -> (usize, usize) {
    let subthreads = store.subthreads(thread);
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
}

/// "3 subthreads, 1 running", or nothing without any.
fn subthreads_label(count: usize, running: usize) -> Option<String> {
    match (count, running) {
        (0, _) => None,
        (1, 0) => Some("1 subthread".to_string()),
        (count, 0) => Some(format!("{count} subthreads")),
        (count, running) => Some(format!("{count} subthreads, {running} running")),
    }
}

/// t3code's status pill: a dot and a label in the status color.
pub(crate) fn render_status_pill(status: ThreadStatus, cx: &App) -> impl IntoElement {
    let (label, color) = match status {
        ThreadStatus::PendingApproval => ("Pending Approval", Color::Warning),
        ThreadStatus::AwaitingInput => ("Awaiting Input", AWAITING_INPUT_COLOR),
        ThreadStatus::Working => ("Working", Color::Accent),
        ThreadStatus::Waiting => ("Waiting", Color::Accent),
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
        ThreadStatus::Working | ThreadStatus::Waiting => Color::Accent,
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
