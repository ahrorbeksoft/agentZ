//! The Workspaces view, herdr's: workspaces in a sidebar, each with tabs of split panes holding
//! terminals and threads. The server keeps the workspaces, their tabs and pane trees
//! (`agentz_protocol::spaces`); which tab shows, which pane has focus, and zoom are this
//! window's, as herdr keeps them per client. In code they're spaces, since a thread's
//! workspace is its checkout.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use agentz_protocol::agents::AgentId;
use agentz_protocol::layout::{
    Direction, NavDirection, Node, PaneId, Rect, TileLayout, find_in_direction,
};
use agentz_protocol::spaces::{
    Pane, PaneContent, PaneTerminal, Space, SpaceFolder, SpaceGit, SpaceId, SpaceRequest, Tab,
    TabId,
};
use agentz_protocol::terminal::TerminalKey;
use agentz_protocol::{Request, Response};
use collections::{HashMap, HashSet};
use gpui::{
    Action, AnyElement, App, Bounds, ClickEvent, ClipboardItem, Context, DragMoveEvent, ElementId,
    Entity, EventEmitter, FocusHandle, Focusable, KeyBinding, MouseButton, Point, PromptLevel,
    ScrollHandle, Subscription, Task, Window, actions, relative,
};
use projects::ThreadId;
use text_input::{TextInput, TextInputEvent};
use ui::{
    ContextMenu, ContextMenuEntry, PopoverMenu, PopoverMenuHandle, Tab as TabItem, TabBar,
    TabPosition, Tooltip, WithScrollbar as _, prelude::*, right_click_menu,
};

use crate::OpenSettings;
use crate::agent_icons::agent_icon;
use crate::agent_view::{AgentView, AgentViewEvent, TOOLBAR_HEIGHT};
use crate::confirm_dialog::ConfirmRequest;
use crate::machines::{MachineId, Machines, ProjectKey, ThreadKey, project_at};
use crate::new_space_picker::{NewSpacePicker, SpaceChoice};
use crate::project_info::{ProjectInfoStore, render_project_icon, workspace_icon};
use crate::project_store::ThreadStatus;
use crate::project_switcher::compact_path;
use crate::settings_page::remove_workspace;
use crate::sidebar::{
    DETAILS_DELAY, SIDEBAR_WIDTH, ThreadDetails, details_card, details_row, format_relative_time,
    render_card_popover, render_details_popover, render_folder_icon, render_footer_item,
    render_status_dot, render_status_pill, repository_branch, thread_agent_icon,
};
use crate::terminal_element::TerminalMode;
use crate::terminal_entity::Terminal;
use crate::terminal_view::TerminalView;
use crate::thread_entity::AgentThread;
use crate::worktree_modal::WorktreeModalMode;
use agentz_protocol::workspace::WorkspaceRemoval;

pub(crate) const KEY_CONTEXT: &str = "Workspaces";
const RENAME_KEY_CONTEXT: &str = "WorkspacesRename";
pub(crate) const SEARCH_KEY_CONTEXT: &str = "WorkspacesSearch";
/// The grab area of a split's border. The line drawn in its middle is a pixel wide.
const DIVIDER_SIZE: Pixels = px(5.);
/// How far a worktree's row sits in from its parent's.
const CHILD_ROW_INDENT: Pixels = px(20.);
/// Where the tree line from a parent to its worktrees runs.
const CONNECTOR_LEFT: Pixels = px(14.);
/// An agent row's second line starts past the first's state slot.
const AGENT_ROW_INDENT: Pixels = px(14.);
/// How many agents a workspace row shows by icon before counting the rest.
const MAX_ROW_AGENT_ICONS: usize = 3;
/// The area panes are laid out in to find their neighbors. Only the proportions matter.
const NAVIGATION_AREA: Rect = Rect::new(0, 0, 10_000, 10_000);

actions!(
    workspaces,
    [
        /// Opens the picker for a new workspace.
        NewWorkspace,
        /// Opens a tab with a shell in the workspace.
        NewTab,
        /// Shows the workspace's next tab.
        NextTab,
        /// Shows the workspace's previous tab.
        PreviousTab,
        /// Splits the focused pane, with a shell to its right.
        SplitRight,
        /// Splits the focused pane, with a shell below it.
        SplitDown,
        /// Closes the focused pane.
        ClosePane,
        /// Shows the focused pane alone in its tab, or all of the tab's panes again.
        ToggleZoom,
        /// Focuses the pane to the left.
        ActivatePaneLeft,
        /// Focuses the pane to the right.
        ActivatePaneRight,
        /// Focuses the pane above.
        ActivatePaneUp,
        /// Focuses the pane below.
        ActivatePaneDown,
    ]
);

/// Shows the workspace's tab at this position, counting from 1.
#[derive(Clone, Debug, PartialEq, Action)]
#[action(namespace = workspaces, no_json)]
pub struct ActivateTab(pub usize);

/// Mac-style keys for herdr's actions. A terminal pane needs none of them: they all hold Cmd,
/// which terminals don't receive.
pub fn init(cx: &mut App) {
    let context = Some(KEY_CONTEXT);
    cx.bind_keys([
        KeyBinding::new("cmd-shift-n", NewWorkspace, context),
        KeyBinding::new("cmd-t", NewTab, context),
        KeyBinding::new("cmd-}", NextTab, context),
        KeyBinding::new("cmd-{", PreviousTab, context),
        KeyBinding::new("cmd-d", SplitRight, context),
        KeyBinding::new("cmd-shift-d", SplitDown, context),
        KeyBinding::new("cmd-w", ClosePane, context),
        KeyBinding::new("cmd-shift-enter", ToggleZoom, context),
        KeyBinding::new("cmd-alt-left", ActivatePaneLeft, context),
        KeyBinding::new("cmd-alt-right", ActivatePaneRight, context),
        KeyBinding::new("cmd-alt-up", ActivatePaneUp, context),
        KeyBinding::new("cmd-alt-down", ActivatePaneDown, context),
        KeyBinding::new("cmd-1", ActivateTab(1), context),
        KeyBinding::new("cmd-2", ActivateTab(2), context),
        KeyBinding::new("cmd-3", ActivateTab(3), context),
        KeyBinding::new("cmd-4", ActivateTab(4), context),
        KeyBinding::new("cmd-5", ActivateTab(5), context),
        KeyBinding::new("cmd-6", ActivateTab(6), context),
        KeyBinding::new("cmd-7", ActivateTab(7), context),
        KeyBinding::new("cmd-8", ActivateTab(8), context),
        KeyBinding::new("cmd-9", ActivateTab(9), context),
        KeyBinding::new("enter", menu::Confirm, Some(RENAME_KEY_CONTEXT)),
        KeyBinding::new("escape", menu::Cancel, Some(RENAME_KEY_CONTEXT)),
        KeyBinding::new("escape", menu::Cancel, Some(SEARCH_KEY_CONTEXT)),
    ]);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SpaceKey {
    pub machine: MachineId,
    pub space: SpaceId,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TabKey {
    pub machine: MachineId,
    pub tab: TabId,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PaneKey {
    pub machine: MachineId,
    pub pane: PaneId,
}

impl PaneKey {
    fn element_id(self, prefix: &str) -> ElementId {
        ElementId::Name(format!("{prefix}-{}-{}", self.machine.slug(), self.pane.0).into())
    }
}

pub enum SpacesViewEvent {
    /// Show the thread in Agents.
    OpenThread(ThreadKey),
    /// New Thread Here: a draft in the project, in this checkout, shown in the Agents view.
    NewThread {
        project: ProjectKey,
        folder: PathBuf,
    },
    /// New Thread…: a Workspaces thread working in `folder`, to be shown in the pane once it's
    /// made.
    NewThreadInPane { pane: PaneKey, folder: PathBuf },
    /// New Worktree or Open Worktree… from a workspace in a git repository.
    Worktree {
        machine: MachineId,
        /// The workspace's folder.
        folder: PathBuf,
        /// The repository's name.
        name: SharedString,
        mode: WorktreeModalMode,
    },
    /// Ask before a destructive action, in the shell's modal layer.
    Confirm(ConfirmRequest),
    /// Manage Agents, from a new thread's agent picker.
    OpenAgentSettings,
}

#[derive(Clone)]
enum PaneView {
    Terminal(Entity<TerminalView>),
    Agent(Entity<AgentView>),
}

impl PaneView {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        match self {
            Self::Terminal(view) => view.focus_handle(cx),
            Self::Agent(view) => view.focus_handle(cx),
        }
    }
}

/// The view showing a pane's content, made when the pane is first shown.
struct OpenPane {
    content: PaneContent,
    view: PaneView,
    _subscriptions: Vec<Subscription>,
}

#[derive(Clone, Copy, PartialEq)]
enum RenameTarget {
    Space(SpaceKey),
    Tab(TabKey),
}

/// A split's ratio while its border is dragged, before the server has it.
#[derive(Clone)]
struct SplitOverride {
    tab: TabKey,
    path: Vec<bool>,
    ratio: f32,
}

#[derive(Clone)]
struct DraggedSplit {
    tab: TabKey,
    path: Vec<bool>,
    direction: Direction,
}

impl Render for DraggedSplit {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        gpui::Empty
    }
}

/// A tab, a workspace or a pane being dragged by its label.
#[derive(Clone)]
struct DraggedLabel<T> {
    item: T,
    label: SharedString,
}

impl<T: 'static> Render for DraggedLabel<T> {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .px_2()
            .py_1()
            .rounded_md()
            .elevation_2(cx)
            .child(Label::new(self.label.clone()).size(LabelSize::Small))
    }
}

#[derive(Clone, Copy, PartialEq)]
struct DraggedTab {
    space: SpaceKey,
    tab: TabId,
}

#[derive(Clone, Copy, PartialEq)]
struct DraggedPane {
    tab: TabKey,
    pane: PaneId,
}

/// The pane a dragged pane is over, and the edge it's near: `None` is the middle, which swaps
/// the two.
#[derive(Clone, Copy, PartialEq)]
struct PaneDrop {
    pane: PaneKey,
    edge: Option<NavDirection>,
}

/// How far into a pane its edges reach, as a share of its shorter side (Zed's
/// `drop_target_size`).
const PANE_DROP_EDGE_SIZE: f32 = 0.2;

/// The edge of `bounds` nearest `position` when it's within reach of one, as Zed's
/// `Pane::handle_drag_move` finds it.
fn pane_drop_edge(bounds: Bounds<Pixels>, position: Point<Pixels>) -> Option<NavDirection> {
    let size = bounds.size.width.min(bounds.size.height) * PANE_DROP_EDGE_SIZE;
    let x = position.x - bounds.left();
    let y = position.y - bounds.top();
    let width = bounds.size.width;
    let height = bounds.size.height;
    if x >= size && x <= width - size && y >= size && y <= height - size {
        return None;
    }
    [
        (NavDirection::Up, y),
        (NavDirection::Right, width - x),
        (NavDirection::Down, height - y),
        (NavDirection::Left, x),
    ]
    .into_iter()
    .min_by_key(|(_, distance)| *distance)
    .map(|(edge, _)| edge)
}

/// A row of the sidebar's agents list.
struct AgentEntry {
    pane: PaneKey,
    icon: Icon,
    title: SharedString,
    status: Option<ThreadStatus>,
    space: SharedString,
    tab: SharedString,
}

pub struct SpacesView {
    focus_handle: FocusHandle,
    machines: Entity<Machines>,
    search: Entity<TextInput>,
    sidebar_scroll: ScrollHandle,
    agents_scroll: ScrollHandle,
    new_space_handle: PopoverMenuHandle<NewSpacePicker>,
    active_space: Option<SpaceKey>,
    active_tabs: HashMap<SpaceKey, TabId>,
    /// Each tab's tree, as the server last sent it, with this window's focus in it.
    layouts: HashMap<TabKey, TileLayout>,
    zoomed: HashSet<TabKey>,
    panes: HashMap<PaneKey, OpenPane>,
    /// What was typed in a new thread that another replaced in its pane, for the
    /// replacement's composer once the pane shows it.
    replaced_composer_texts: HashMap<ThreadKey, SharedString>,
    /// A pane the server is making, focused once it arrives.
    pending_focus: Option<PaneKey>,
    split_override: Option<SplitOverride>,
    pane_drop: Option<PaneDrop>,
    renaming: Option<RenameTarget>,
    /// The name the rename started from, so ending it unchanged keeps an automatic name.
    rename_original: SharedString,
    rename_input: Entity<TextInput>,
    _rename_blur: Option<Subscription>,
    /// The workspace whose details popover shows, and the one waiting to show it.
    details_space: Option<SpaceKey>,
    details_delay: Option<(SpaceKey, Task<()>)>,
    hovered_space: Option<SpaceKey>,
    /// Worktree groups folded to their parent (client-only, as in herdr).
    collapsed_groups: HashSet<GroupKey>,
    /// Workspaces by when they were last used here, most recent first, for New Workspace.
    recent_spaces: Vec<SpaceKey>,
    /// The row whose counts are under the mouse, which show a tooltip instead of the details.
    hovered_contents: Option<SpaceKey>,
    is_visible: bool,
    _subscriptions: Vec<Subscription>,
}

impl EventEmitter<SpacesViewEvent> for SpacesView {}

impl Focusable for SpacesView {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl SpacesView {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let machines = Machines::global(cx);
        let search = cx.new(|cx| TextInput::new("Search…", cx));
        let rename_input = cx.new(|cx| TextInput::new("Name", cx));
        let subscriptions = vec![
            cx.observe_in(&machines, window, |this, _, window, cx| {
                this.sync(window, cx)
            }),
            cx.subscribe(&search, |_, _, _: &TextInputEvent, cx| cx.notify()),
            cx.observe(&ProjectInfoStore::global(cx), |_, _, cx| cx.notify()),
            cx.observe_window_activation(window, |this, window, cx| {
                this.mark_visible_seen(window, cx)
            }),
        ];
        let mut this = Self {
            focus_handle: cx.focus_handle(),
            machines,
            search,
            sidebar_scroll: ScrollHandle::new(),
            agents_scroll: ScrollHandle::new(),
            new_space_handle: PopoverMenuHandle::default(),
            active_space: None,
            active_tabs: HashMap::default(),
            layouts: HashMap::default(),
            zoomed: HashSet::default(),
            panes: HashMap::default(),
            replaced_composer_texts: HashMap::default(),
            pending_focus: None,
            split_override: None,
            pane_drop: None,
            renaming: None,
            rename_original: SharedString::default(),
            rename_input,
            _rename_blur: None,
            details_space: None,
            details_delay: None,
            hovered_space: None,
            collapsed_groups: HashSet::default(),
            recent_spaces: Vec::new(),
            hovered_contents: None,
            is_visible: false,
            _subscriptions: subscriptions,
        };
        this.sync(window, cx);
        this
    }

    /// Every machine's spaces, in the sidebar's order.
    fn all_spaces(&self, cx: &App) -> Vec<(MachineId, Space)> {
        let mut spaces = Vec::new();
        for client in self.machines.read(cx).clients() {
            // An older server without spaces sends none.
            let client = client.read(cx);
            for space in &client.spaces().spaces {
                spaces.push((client.machine(), space.clone()));
            }
        }
        spaces
    }

    fn space(&self, key: SpaceKey, cx: &App) -> Option<Space> {
        let client = self.machines.read(cx).client(key.machine, cx)?;
        client.read(cx).spaces().space(key.space).cloned()
    }

    /// The space and tab a pane is in, and the pane.
    fn find_pane(&self, key: PaneKey, cx: &App) -> Option<(Space, TabId, Pane)> {
        let client = self.machines.read(cx).client(key.machine, cx)?;
        let (space, tab, pane) = client.read(cx).spaces().pane(key.pane)?;
        Some((space.clone(), tab.id, pane.clone()))
    }

    fn active_tab_of(&self, key: SpaceKey, space: &Space) -> Option<TabId> {
        self.active_tabs
            .get(&key)
            .copied()
            .filter(|tab| space.tabs.iter().any(|candidate| candidate.id == *tab))
            .or_else(|| space.tabs.first().map(|tab| tab.id))
    }

    /// The tab on screen.
    fn visible_tab(&self, cx: &App) -> Option<(SpaceKey, Space, TabKey)> {
        let key = self.active_space?;
        let space = self.space(key, cx)?;
        let tab = self.active_tab_of(key, &space)?;
        Some((
            key,
            space,
            TabKey {
                machine: key.machine,
                tab,
            },
        ))
    }

    fn focused_pane(&self, cx: &App) -> Option<PaneKey> {
        let (_, _, tab) = self.visible_tab(cx)?;
        Some(PaneKey {
            machine: tab.machine,
            pane: self.layouts.get(&tab)?.focused(),
        })
    }

    /// Follows the servers' spaces: drops what's gone, and notices agents finishing.
    fn sync(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let spaces = self.all_spaces(cx);
        let mut live_spaces = HashSet::default();
        let mut live_tabs = HashSet::default();
        let mut live_panes: HashMap<PaneKey, Pane> = HashMap::default();
        for (machine, space) in &spaces {
            live_spaces.insert(SpaceKey {
                machine: *machine,
                space: space.id,
            });
            for tab in &space.tabs {
                let key = TabKey {
                    machine: *machine,
                    tab: tab.id,
                };
                live_tabs.insert(key);
                match self.layouts.get_mut(&key) {
                    Some(layout) => layout.set_root(tab.root.clone()),
                    None => {
                        let first = tab.root.first_pane();
                        self.layouts
                            .insert(key, TileLayout::from_saved(tab.root.clone(), first));
                    }
                }
                for pane in &tab.panes {
                    let pane_key = PaneKey {
                        machine: *machine,
                        pane: pane.id,
                    };
                    live_panes.insert(pane_key, pane.clone());
                }
            }
        }
        self.layouts.retain(|key, _| live_tabs.contains(key));
        self.zoomed.retain(|key| live_tabs.contains(key));
        self.active_tabs.retain(|key, tab| {
            live_tabs.contains(&TabKey {
                machine: key.machine,
                tab: *tab,
            })
        });
        self.panes.retain(|key, open| {
            live_panes
                .get(key)
                .is_some_and(|pane| pane.content == open.content)
        });

        if !self
            .active_space
            .is_some_and(|key| live_spaces.contains(&key))
        {
            let previous_machine = self.active_space.map(|key| key.machine);
            self.active_space = spaces
                .iter()
                .find(|(machine, _)| Some(*machine) == previous_machine)
                .or_else(|| spaces.first())
                .map(|(machine, space)| SpaceKey {
                    machine: *machine,
                    space: space.id,
                });
        }
        if !cx.has_active_drag() {
            self.split_override = None;
            self.pane_drop = None;
        }

        // Threads shown in panes follow renames and archiving.
        let machines = self.machines.read(cx);
        let mut thread_updates = Vec::new();
        for (key, open) in &self.panes {
            let (PaneContent::Thread(thread_id), PaneView::Agent(view)) =
                (&open.content, &open.view)
            else {
                continue;
            };
            if let Some(thread) = machines
                .projects(key.machine, cx)
                .and_then(|store| store.read(cx).thread(*thread_id).cloned())
            {
                thread_updates.push((view.clone(), thread));
            }
        }
        for (view, thread) in thread_updates {
            view.update(cx, |view, cx| {
                view.set_title(thread.title.clone().into(), cx);
                view.set_archived(thread.archived_at.is_some(), cx);
            });
        }

        self.apply_pending_focus(window, cx);
        self.mark_visible_seen(window, cx);
        cx.notify();
    }

    /// Agents on screen have been seen, as have threads.
    fn mark_visible_seen(&mut self, window: &Window, cx: &mut Context<Self>) {
        if !self.is_visible || !window.is_window_active() {
            return;
        }
        let Some((_, space, tab_key)) = self.visible_tab(cx) else {
            return;
        };
        let Some(tab) = space.tabs.iter().find(|tab| tab.id == tab_key.tab) else {
            return;
        };
        let Some(client) = self.machines.read(cx).client(tab_key.machine, cx) else {
            return;
        };
        let store = client.read(cx).projects().clone();
        for pane in &tab.panes {
            client.update(cx, |client, cx| client.mark_pane_seen(pane.id, cx));
            if let PaneContent::Thread(thread_id) = &pane.content {
                store.update(cx, |store, cx| store.mark_viewed(*thread_id, cx));
            }
        }
    }

    pub fn set_visible(&mut self, is_visible: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.is_visible = is_visible;
        self.mark_visible_seen(window, cx);
        cx.notify();
    }

    /// Whether a pane on screen shows the thread.
    pub fn shows_thread(&self, thread: ThreadKey, cx: &App) -> bool {
        self.is_visible
            && self.visible_tab(cx).is_some_and(|(_, space, tab)| {
                tab.machine == thread.machine
                    && space
                        .tabs
                        .iter()
                        .find(|candidate| candidate.id == tab.tab)
                        .is_some_and(|tab| {
                            tab.panes
                                .iter()
                                .any(|pane| pane.content == PaneContent::Thread(thread.thread))
                        })
            })
    }

    /// Gives focus to the pane on screen.
    pub fn focus_active(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match self.focused_pane(cx) {
            Some(pane) => self.focus_pane(pane, window, cx),
            None => window.focus(&self.focus_handle, cx),
        }
    }

    fn apply_pending_focus(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(pane) = self.pending_focus
            && self.find_pane(pane, cx).is_some()
        {
            self.pending_focus = None;
            self.focus_pane(pane, window, cx);
        }
    }

    /// Shows the pane's workspace and tab, and focuses it.
    pub fn focus_pane(&mut self, key: PaneKey, window: &mut Window, cx: &mut Context<Self>) {
        let Some((space, tab, pane)) = self.find_pane(key, cx) else {
            return;
        };
        let space_key = SpaceKey {
            machine: key.machine,
            space: space.id,
        };
        let tab_key = TabKey {
            machine: key.machine,
            tab,
        };
        self.active_space = Some(space_key);
        self.recent_spaces.retain(|recent| *recent != space_key);
        self.recent_spaces.insert(0, space_key);
        self.active_tabs.insert(space_key, tab);
        if let Some(layout) = self.layouts.get_mut(&tab_key)
            && layout.focused() != key.pane
        {
            layout.focus_pane(key.pane);
            self.zoomed.remove(&tab_key);
        }
        match self.ensure_view(key, &pane.content, cx) {
            Some(view) => {
                let focus_handle = view.focus_handle(cx);
                if !focus_handle.contains_focused(window, cx) {
                    window.focus(&focus_handle, cx);
                }
            }
            None => window.focus(&self.focus_handle, cx),
        }
        self.mark_visible_seen(window, cx);
        cx.notify();
    }

    fn activate_space(&mut self, key: SpaceKey, window: &mut Window, cx: &mut Context<Self>) {
        let Some(space) = self.space(key, cx) else {
            return;
        };
        let Some(tab) = self.active_tab_of(key, &space) else {
            return;
        };
        self.activate_tab(
            TabKey {
                machine: key.machine,
                tab,
            },
            window,
            cx,
        );
    }

    fn activate_tab(&mut self, key: TabKey, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(layout) = self.layouts.get(&key) {
            let pane = PaneKey {
                machine: key.machine,
                pane: layout.focused(),
            };
            self.focus_pane(pane, window, cx);
        }
    }

    /// The view for a pane's content, made the first time it's needed.
    fn ensure_view(
        &mut self,
        key: PaneKey,
        content: &PaneContent,
        cx: &mut Context<Self>,
    ) -> Option<PaneView> {
        if let Some(open) = self.panes.get(&key)
            && open.content == *content
        {
            return Some(open.view.clone());
        }
        let client = self.machines.read(cx).client(key.machine, cx)?;
        let (view, subscriptions) = match content {
            PaneContent::Terminal(_) => {
                let terminal = Terminal::shared(&client, TerminalKey::Pane(key.pane), cx);
                let subscription = cx.observe(&terminal, |_, _, cx| cx.notify());
                let view = cx.new(|cx| TerminalView::new(terminal, TerminalMode::Scrollable, cx));
                (PaneView::Terminal(view), vec![subscription])
            }
            PaneContent::Thread(thread_id) => {
                let thread_id = *thread_id;
                let store = client.read(cx).projects().clone();
                let thread = store.read(cx).thread(thread_id)?.clone();
                if thread.terminal.is_some() {
                    let terminal = Terminal::shared(&client, TerminalKey::Thread(thread_id), cx);
                    let subscription = cx.observe(&terminal, |_, _, cx| cx.notify());
                    let view =
                        cx.new(|cx| TerminalView::new(terminal, TerminalMode::Scrollable, cx));
                    (PaneView::Terminal(view), vec![subscription])
                } else {
                    let agent_id = thread.agent_id.clone().map(AgentId::new);
                    let agent_thread = AgentThread::shared(&client, thread_id, cx);
                    let is_archived = thread.archived_at.is_some();
                    let machine = key.machine;
                    let composer_text = self.replaced_composer_texts.remove(&ThreadKey {
                        machine,
                        thread: thread_id,
                    });
                    let view = cx.new(|cx| {
                        let mut view = AgentView::new(
                            thread_id,
                            agent_thread,
                            thread.title.clone().into(),
                            agent_id,
                            cx,
                        );
                        view.set_archived(is_archived, cx);
                        view.hide_toolbar(cx);
                        if let Some(text) = composer_text {
                            view.set_composer_text(text, cx);
                        }
                        view
                    });
                    let subscription = cx.subscribe(&view, move |this, _, event, cx| match event {
                        AgentViewEvent::Unarchive => {
                            store.update(cx, |store, cx| store.unarchive_thread(thread_id, cx))
                        }
                        AgentViewEvent::OpenThread(other) => {
                            cx.emit(SpacesViewEvent::OpenThread(ThreadKey {
                                machine,
                                thread: *other,
                            }))
                        }
                        AgentViewEvent::Confirm(request) => {
                            cx.emit(SpacesViewEvent::Confirm(request.clone()))
                        }
                        // A pane shows its own header, without the thread's project.
                        AgentViewEvent::NewThreadInProject(_) => {}
                        // A pane holds threads of its own machine only.
                        AgentViewEvent::Replaced { thread, text } if thread.machine == machine => {
                            this.replaced_composer_texts.insert(*thread, text.clone());
                            this.show_thread_in_pane(key, thread.thread, cx);
                        }
                        AgentViewEvent::Replaced { thread, .. } => {
                            cx.emit(SpacesViewEvent::OpenThread(*thread))
                        }
                        AgentViewEvent::OpenAgentSettings => {
                            cx.emit(SpacesViewEvent::OpenAgentSettings)
                        }
                    });
                    (PaneView::Agent(view), vec![subscription])
                }
            }
            PaneContent::Unknown(_) => return None,
        };
        self.panes.insert(
            key,
            OpenPane {
                content: content.clone(),
                view: view.clone(),
                _subscriptions: subscriptions,
            },
        );
        Some(view)
    }

    /// Sends a request, focusing the pane it makes, if any.
    fn request(
        &mut self,
        machine: MachineId,
        request: SpaceRequest,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(client) = self.machines.read(cx).client(machine, cx) else {
            return;
        };
        let response = client.read(cx).request(Request::Spaces(request));
        cx.spawn_in(window, async move |this, cx| match response.await {
            Ok(Response::SpacePane(location)) => {
                this.update_in(cx, |this, window, cx| {
                    this.pending_focus = Some(PaneKey {
                        machine,
                        pane: location.pane,
                    });
                    this.apply_pending_focus(window, cx);
                })
                .ok();
            }
            Ok(_) => {}
            Err(error) => log::error!("the workspace request failed: {error:#}"),
        })
        .detach();
    }

    fn send(&self, machine: MachineId, request: SpaceRequest, cx: &App) {
        if let Some(client) = self.machines.read(cx).client(machine, cx) {
            client.read(cx).send(Request::Spaces(request), cx);
        }
    }

    /// Goes to the folder's open workspace, or opens one there.
    fn create_space(&mut self, choice: SpaceChoice, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(space) = choice.existing {
            let key = SpaceKey {
                machine: choice.machine,
                space,
            };
            if self.space(key, cx).is_some() {
                self.activate_space(key, window, cx);
                return;
            }
        }
        self.request(
            choice.machine,
            SpaceRequest::CreateSpace {
                folder: choice.folder,
                project_id: choice.project_id,
                content: new_shell(),
            },
            window,
            cx,
        );
    }

    /// A new workspace with a shell in the folder.
    pub fn open_space_at(
        &mut self,
        machine: MachineId,
        folder: PathBuf,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let project_id = self
            .machines
            .read(cx)
            .projects(machine, cx)
            .and_then(|store| {
                project_at(store.read(cx).projects(), &folder).map(|project| project.id)
            });
        self.request(
            machine,
            SpaceRequest::CreateSpace {
                folder,
                project_id,
                content: new_shell(),
            },
            window,
            cx,
        );
    }

    fn new_tab_in(&mut self, key: SpaceKey, window: &mut Window, cx: &mut Context<Self>) {
        self.request(
            key.machine,
            SpaceRequest::CreateTab {
                space: key.space,
                content: new_shell(),
            },
            window,
            cx,
        );
    }

    fn split(
        &mut self,
        pane: PaneKey,
        direction: Direction,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.request(
            pane.machine,
            SpaceRequest::SplitPane {
                pane: pane.pane,
                direction,
                content: new_shell(),
            },
            window,
            cx,
        );
    }

    /// Splits with a shell, then starts a thread in the new pane, as New Thread… does, working
    /// where the split pane is.
    fn split_with_thread(
        &mut self,
        pane: PaneKey,
        direction: Direction,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(client) = self.machines.read(cx).client(pane.machine, cx) else {
            return;
        };
        let Some(folder) = self.pane_folder(pane, cx) else {
            return;
        };
        let response = client
            .read(cx)
            .request(Request::Spaces(SpaceRequest::SplitPane {
                pane: pane.pane,
                direction,
                content: new_shell(),
            }));
        cx.spawn_in(window, async move |this, cx| match response.await {
            Ok(Response::SpacePane(location)) => {
                this.update(cx, |_, cx| {
                    cx.emit(SpacesViewEvent::NewThreadInPane {
                        pane: PaneKey {
                            machine: pane.machine,
                            pane: location.pane,
                        },
                        folder,
                    })
                })
                .ok();
            }
            Ok(_) => {}
            Err(error) => log::error!("splitting for a thread failed: {error:#}"),
        })
        .detach();
    }

    fn close_pane(&mut self, key: PaneKey, window: &mut Window, cx: &mut Context<Self>) {
        let running = self
            .find_pane(key, cx)
            .and_then(|(_, _, pane)| running_program(&pane));
        let close = move |this: &mut Self, cx: &mut Context<Self>| {
            this.send(key.machine, SpaceRequest::ClosePane(key.pane), cx)
        };
        match running {
            None => close(self, cx),
            Some(program) => self.confirm_close(
                format!("Close “{program}”?"),
                "It's still running, and closing the pane ends it.".to_string(),
                close,
                window,
                cx,
            ),
        }
    }

    fn close_tab(&mut self, key: TabKey, window: &mut Window, cx: &mut Context<Self>) {
        let running = self
            .machines
            .read(cx)
            .client(key.machine, cx)
            .and_then(|client| {
                let (_, tab) = client.read(cx).spaces().tab(key.tab)?;
                Some(running_programs(&tab.panes))
            })
            .unwrap_or_default();
        let close = move |this: &mut Self, cx: &mut Context<Self>| {
            this.send(key.machine, SpaceRequest::CloseTab(key.tab), cx)
        };
        if running.is_empty() {
            close(self, cx);
            return;
        }
        self.confirm_close(
            "Close this tab?".to_string(),
            still_running(&running, "it"),
            close,
            window,
            cx,
        );
    }

    /// Asks before ending what still runs, as terminals such as Ghostty do.
    fn confirm_close(
        &mut self,
        title: String,
        detail: String,
        close: impl FnOnce(&mut Self, &mut Context<Self>) + 'static,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let answer = window.prompt(
            PromptLevel::Warning,
            &title,
            Some(&detail),
            &["Close", "Cancel"],
            cx,
        );
        cx.spawn(async move |this, cx| {
            if answer.await == Ok(0) {
                this.update(cx, |this, cx| close(this, cx)).ok();
            }
        })
        .detach();
    }

    fn toggle_zoom_of(&mut self, pane: PaneKey, window: &mut Window, cx: &mut Context<Self>) {
        let Some((_, tab, _)) = self.find_pane(pane, cx) else {
            return;
        };
        let tab = TabKey {
            machine: pane.machine,
            tab,
        };
        let is_zoomed = self.zoomed.contains(&tab)
            && self
                .layouts
                .get(&tab)
                .is_some_and(|layout| layout.focused() == pane.pane);
        self.focus_pane(pane, window, cx);
        if is_zoomed {
            self.zoomed.remove(&tab);
        } else if self
            .layouts
            .get(&tab)
            .is_some_and(|layout| layout.pane_count() > 1)
        {
            self.zoomed.insert(tab);
        }
        cx.notify();
    }

    /// Shows a thread in a pane, in place of what it showed.
    pub fn show_thread_in_pane(&mut self, pane: PaneKey, thread: ThreadId, cx: &mut Context<Self>) {
        self.send(
            pane.machine,
            SpaceRequest::SetPaneContent {
                pane: pane.pane,
                content: PaneContent::Thread(thread),
            },
            cx,
        );
    }

    /// Closes a workspace. A group's only parent takes its worktrees' workspaces with it, as
    /// in herdr; their checkouts and branches stay.
    fn close_space(&mut self, key: SpaceKey, window: &mut Window, cx: &mut Context<Self>) {
        let Some(space) = self.space(key, cx) else {
            return;
        };
        let worktrees = self.group_children_closing_with(key, &space, cx);
        let panes: Vec<Pane> = std::iter::once(&space)
            .chain(&worktrees)
            .flat_map(|space| space.tabs.iter().flat_map(|tab| tab.panes.iter().cloned()))
            .collect();
        let running = running_programs(&panes);
        let mut closing = vec![key.space];
        closing.extend(worktrees.iter().map(|space| space.id));
        let close = move |this: &mut Self, cx: &mut Context<Self>| {
            for space in &closing {
                this.send(key.machine, SpaceRequest::CloseSpace(*space), cx);
            }
        };
        if running.is_empty() {
            close(self, cx);
            return;
        }
        let (title, detail) = match worktrees.len() {
            0 => (
                format!("Close “{}”?", space.label()),
                format!(
                    "{} Its threads stay in Agents.",
                    still_running(&running, "it")
                ),
            ),
            count => (
                format!(
                    "Close “{}” and its {count} {}?",
                    space.label(),
                    if count == 1 { "worktree" } else { "worktrees" }
                ),
                format!(
                    "{} Checkouts, branches and threads stay.",
                    still_running(&running, "them")
                ),
            ),
        };
        self.confirm_close(title, detail, close, window, cx);
    }

    /// The worktrees' workspaces that close with a group's parent: all of them, unless
    /// another workspace on the main checkout stays to hold them.
    fn group_children_closing_with(&self, key: SpaceKey, space: &Space, cx: &App) -> Vec<Space> {
        let Some((main, GroupRole::Parent)) = self.group_membership(key.machine, space, cx) else {
            return Vec::new();
        };
        let members: Vec<(Space, GroupRole)> = self
            .all_spaces(cx)
            .into_iter()
            .filter(|(machine, other)| *machine == key.machine && other.id != space.id)
            .filter_map(|(machine, other)| {
                let (other_main, role) = self.group_membership(machine, &other, cx)?;
                (other_main == main).then_some((other, role))
            })
            .collect();
        if members.iter().any(|(_, role)| *role == GroupRole::Parent) {
            return Vec::new();
        }
        members.into_iter().map(|(space, _)| space).collect()
    }

    fn start_renaming(
        &mut self,
        target: RenameTarget,
        name: SharedString,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.finish_renaming(true, cx);
        self.rename_original = name.clone();
        self.rename_input.update(cx, |input, cx| {
            input.set_text(name, cx);
            input.select_all_text(cx);
        });
        self.renaming = Some(target);
        let focus_handle = self.rename_input.focus_handle(cx);
        window.focus(&focus_handle, cx);
        // Clicking elsewhere keeps the name, as in the Agents sidebar.
        self._rename_blur = Some(cx.on_blur(&focus_handle, window, |this, _, cx| {
            this.finish_renaming(true, cx)
        }));
        cx.notify();
    }

    /// An empty name names the workspace after its folder again, and the tab after what runs
    /// in it.
    fn finish_renaming(&mut self, keep: bool, cx: &mut Context<Self>) {
        let Some(target) = self.renaming.take() else {
            return;
        };
        self._rename_blur = None;
        let text = self.rename_input.read(cx).text().clone();
        if keep && let Some(name) = renamed_to(&self.rename_original, &text) {
            let (machine, request) = match target {
                RenameTarget::Space(key) => (
                    key.machine,
                    SpaceRequest::RenameSpace {
                        space: key.space,
                        name,
                    },
                ),
                RenameTarget::Tab(key) => {
                    (key.machine, SpaceRequest::RenameTab { tab: key.tab, name })
                }
            };
            self.send(machine, request, cx);
        }
        cx.notify();
    }

    fn render_rename_input(&self, cx: &mut Context<Self>) -> AnyElement {
        div()
            .flex_1()
            .min_w_0()
            .key_context(RENAME_KEY_CONTEXT)
            .on_action(cx.listener(|this, _: &menu::Confirm, window, cx| {
                this.finish_renaming(true, cx);
                this.focus_active(window, cx);
            }))
            .on_action(cx.listener(|this, _: &menu::Cancel, window, cx| {
                this.finish_renaming(false, cx);
                this.focus_active(window, cx);
            }))
            .child(self.rename_input.clone())
            .into_any_element()
    }

    // Keyboard actions, on the focused pane.

    fn new_workspace(&mut self, _: &NewWorkspace, window: &mut Window, cx: &mut Context<Self>) {
        self.new_space_handle.toggle(window, cx);
    }

    fn new_tab(&mut self, _: &NewTab, window: &mut Window, cx: &mut Context<Self>) {
        match self.active_space {
            Some(space) => self.new_tab_in(space, window, cx),
            None => self.new_space_handle.show(window, cx),
        }
    }

    fn cycle_tab(&mut self, forward: bool, window: &mut Window, cx: &mut Context<Self>) {
        let Some((space_key, space, tab)) = self.visible_tab(cx) else {
            return;
        };
        let Some(index) = space
            .tabs
            .iter()
            .position(|candidate| candidate.id == tab.tab)
        else {
            return;
        };
        let count = space.tabs.len();
        let next = if forward {
            (index + 1) % count
        } else {
            (index + count - 1) % count
        };
        self.activate_tab(
            TabKey {
                machine: space_key.machine,
                tab: space.tabs[next].id,
            },
            window,
            cx,
        );
    }

    fn activate_tab_at(
        &mut self,
        action: &ActivateTab,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some((space_key, space, _)) = self.visible_tab(cx) else {
            return;
        };
        let Some(tab) = action
            .0
            .checked_sub(1)
            .and_then(|index| space.tabs.get(index))
        else {
            return;
        };
        let tab = TabKey {
            machine: space_key.machine,
            tab: tab.id,
        };
        self.activate_tab(tab, window, cx);
    }

    fn next_tab(&mut self, _: &NextTab, window: &mut Window, cx: &mut Context<Self>) {
        self.cycle_tab(true, window, cx);
    }

    fn previous_tab(&mut self, _: &PreviousTab, window: &mut Window, cx: &mut Context<Self>) {
        self.cycle_tab(false, window, cx);
    }

    fn split_right(&mut self, _: &SplitRight, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(pane) = self.focused_pane(cx) {
            self.split(pane, Direction::Horizontal, window, cx);
        }
    }

    fn split_down(&mut self, _: &SplitDown, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(pane) = self.focused_pane(cx) {
            self.split(pane, Direction::Vertical, window, cx);
        }
    }

    fn close_focused_pane(&mut self, _: &ClosePane, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(pane) = self.focused_pane(cx) {
            self.close_pane(pane, window, cx);
        }
    }

    fn toggle_zoom(&mut self, _: &ToggleZoom, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(pane) = self.focused_pane(cx) {
            self.toggle_zoom_of(pane, window, cx);
        }
    }

    fn activate_pane_in(&mut self, nav: NavDirection, window: &mut Window, cx: &mut Context<Self>) {
        let Some((_, _, tab)) = self.visible_tab(cx) else {
            return;
        };
        if self.zoomed.contains(&tab) {
            return;
        }
        let Some(layout) = self.layouts.get(&tab) else {
            return;
        };
        let panes = layout.panes(NAVIGATION_AREA);
        let target = panes
            .iter()
            .find(|pane| pane.is_focused)
            .and_then(|focused| find_in_direction(focused, nav, &panes));
        if let Some(pane) = target {
            self.focus_pane(
                PaneKey {
                    machine: tab.machine,
                    pane,
                },
                window,
                cx,
            );
        }
    }

    fn render_sidebar(&self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = cx.theme().colors().clone();
        let query = self.search.read(cx).text().trim().to_lowercase();
        let has_remotes = self.machines.read(cx).has_remotes();
        let spaces = self.all_spaces(cx);
        let has_spaces = !spaces.is_empty();
        let entries = if query.is_empty() {
            space_entries(
                spaces,
                |machine, space| self.group_membership(machine, space, cx),
                &self.collapsed_groups,
                self.active_space,
            )
        } else {
            // Search matches rows wherever they are; groups would hide them.
            space_entries(spaces, |_, _| None, &HashSet::default(), None)
                .into_iter()
                .filter(|entry| matches_space(&entry.space, &query))
                .collect()
        };
        let rows: Vec<AnyElement> = entries
            .iter()
            .map(|entry| self.render_space_row(entry, cx))
            .collect();

        v_flex()
            .w(SIDEBAR_WIDTH)
            .h_full()
            .flex_none()
            .border_r_1()
            .border_color(colors.border)
            .bg(colors.panel_background)
            .child(self.render_sidebar_header(cx))
            .children(self.render_needs_you(has_remotes, cx))
            .child(
                div()
                    .id("workspaces-scroll")
                    .flex_1()
                    .min_h_0()
                    .child(
                        v_flex()
                            .id("workspaces-list")
                            .size_full()
                            .overflow_y_scroll()
                            .track_scroll(&self.sidebar_scroll)
                            .py_1()
                            .gap_0p5()
                            .children(rows)
                            .when(!has_spaces, |list| {
                                list.child(
                                    div().px_3().py_2().child(
                                        Label::new("No workspaces yet")
                                            .size(LabelSize::Small)
                                            .color(Color::Muted),
                                    ),
                                )
                            }),
                    )
                    .vertical_scrollbar_for(&self.sidebar_scroll, window, cx),
            )
            .child(self.render_agents(has_remotes, window, cx))
            .child(render_footer_item(
                "workspaces-open-settings",
                IconName::Settings,
                "Settings",
                |_, window, cx| window.dispatch_action(Box::new(OpenSettings), cx),
                cx,
            ))
    }

    fn render_sidebar_header(&self, cx: &mut Context<Self>) -> impl IntoElement {
        // Read now: the picker can open while this view is being updated (its shortcut). The
        // workspace on screen counts as the most recent.
        let mut recent = self.recent_spaces.clone();
        if let Some(active) = self.active_space
            && recent.first() != Some(&active)
        {
            recent.retain(|space| *space != active);
            recent.insert(0, active);
        }
        let has_query = !self.search.read(cx).text().trim().is_empty();
        let this = cx.entity().downgrade();
        h_flex()
            .key_context(SEARCH_KEY_CONTEXT)
            .on_action(cx.listener(|this, _: &menu::Cancel, window, cx| {
                if this.search.read(cx).text().is_empty() {
                    this.focus_active(window, cx);
                } else {
                    this.search.update(cx, |search, cx| search.set_text("", cx));
                }
            }))
            // Level with the tab bar beside it, and with the Agents view's search.
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
            .when(has_query, |header| {
                header.child(
                    IconButton::new("clear-workspace-search", IconName::Close)
                        .icon_size(IconSize::Small)
                        .tooltip(Tooltip::text("Clear Search"))
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.search.update(cx, |search, cx| search.set_text("", cx));
                        })),
                )
            })
            .child(
                PopoverMenu::new("new-workspace")
                    .with_handle(self.new_space_handle.clone())
                    .menu(move |window, cx| {
                        let this = this.clone();
                        let recent = recent.clone();
                        Some(cx.new(|cx| {
                            NewSpacePicker::new(
                                recent,
                                move |choice, window, cx| {
                                    this.update(cx, |this, cx| {
                                        this.create_space(choice, window, cx)
                                    })
                                    .ok();
                                },
                                window,
                                cx,
                            )
                        }))
                    })
                    .trigger_with_tooltip(
                        IconButton::new("new-workspace-button", IconName::Plus)
                            .icon_size(IconSize::Small),
                        |_, cx| Tooltip::for_action("New Workspace", &NewWorkspace, cx),
                    )
                    .anchor(gpui::Anchor::TopLeft)
                    .offset(gpui::point(px(0.), px(4.))),
            )
    }

    /// herdr's space row: the rolled-up state and name, then the branch and ahead/behind.
    fn render_space_row(&self, entry: &SpaceEntry, cx: &mut Context<Self>) -> AnyElement {
        let (machine, index, space) = (entry.machine, entry.index, &entry.space);
        let key = SpaceKey {
            machine,
            space: space.id,
        };
        let colors = cx.theme().colors().clone();
        let is_active = self.active_space == Some(key);
        let label: SharedString = entry
            .child
            .and_then(|_| child_label(space))
            .unwrap_or_else(|| space.label())
            .into();
        // A folded group's parent stands for the whole group (herdr).
        let collapsed_group = entry
            .group
            .clone()
            .filter(|group| self.collapsed_groups.contains(group));
        let status = match &collapsed_group {
            Some(group) => rolled_up(
                self.all_spaces(cx)
                    .into_iter()
                    .filter(|(machine, space)| {
                        self.group_membership(*machine, space, cx)
                            .is_some_and(|(main, _)| (*machine, main) == *group)
                    })
                    .flat_map(|(machine, space)| {
                        space
                            .tabs
                            .iter()
                            .flat_map(|tab| &tab.panes)
                            .filter_map(|pane| self.pane_status(machine, pane, cx))
                            .collect::<Vec<_>>()
                    }),
            ),
            None => rolled_up(
                space
                    .tabs
                    .iter()
                    .flat_map(|tab| &tab.panes)
                    .filter_map(|pane| self.pane_status(machine, pane, cx)),
            ),
        };
        let is_renaming = self.renaming == Some(RenameTarget::Space(key));
        let machine_icon = self.machines.read(cx).machine_icon(machine, cx);
        let machine_label = self.machines.read(cx).label(machine, cx);
        let git = space.git.clone();
        let path: SharedString = space
            .current
            .as_ref()
            .map(|current| current.display_path.clone())
            .unwrap_or_else(|| space.folder.to_string_lossy().into_owned())
            .into();
        let (terminals, agents) = self.space_contents(machine, space, cx);
        let contents = contents_label(terminals, agents);
        let agent_icons = self.space_agent_icons(machine, space, cx);
        let has_remotes = self.machines.read(cx).has_remotes();
        // The project the workspace is in now, if any: its icon stands for the workspace.
        let project = self
            .machines
            .read(cx)
            .projects(machine, cx)
            .and_then(|store| {
                project_at(store.read(cx).projects(), space.current_folder()).cloned()
            });
        let project_info = project.as_ref().and_then(|project| {
            ProjectInfoStore::global(cx)
                .read(cx)
                .info(machine, project.id)
                .cloned()
        });
        let icon = match &project {
            // A worktree or pasture under its parent shows what kind of checkout it is.
            _ if entry.child.is_some() => {
                let is_worktree = git.as_ref().is_some_and(|git| git.is_linked_worktree());
                Icon::new(workspace_icon(if is_worktree {
                    projects::WorkspaceKind::Worktree
                } else {
                    projects::WorkspaceKind::Pasture
                }))
                .size(IconSize::Small)
                .color(Color::Muted)
                .into_any_element()
            }
            Some(project) => render_project_icon(project, project_info.as_ref(), px(16.), cx),
            None => render_folder_icon(),
        };
        let project_key = project.as_ref().map(|project| ProjectKey {
            machine,
            project: project.id,
        });
        let checkout_root = project
            .as_ref()
            .and_then(|project| checkout_root(project, space.current_folder()));
        // herdr offers worktrees in any workspace inside a git repository.
        let worktree_source = git.as_ref().map(|git| {
            let folder = space.current_folder().to_path_buf();
            let name: SharedString = git
                .repository
                .clone()
                .or_else(|| {
                    folder
                        .file_name()
                        .map(|name| name.to_string_lossy().into_owned())
                })
                .unwrap_or_default()
                .into();
            (folder, name)
        });
        let details = ThreadDetails {
            title: label.clone(),
            project: project.map(|project| (project, project_info)),
            machine: (machine_icon, machine_label),
            branch: git.as_ref().map(|git| {
                git.branch
                    .clone()
                    .unwrap_or_else(|| "detached".to_string())
                    .into()
            }),
            path: Some(path.clone()),
            workspace: None,
            agent: None,
            contents: contents.clone().map(Into::into),
        };
        // In git, the checkout at a glance; elsewhere, the general details.
        let details_popover = (self.details_space == Some(key)).then(|| match &git {
            Some(git) => render_card_popover(self.render_git_glance(entry, git, &label, &path, cx)),
            None => render_details_popover(details, cx),
        });
        let id = format!("workspace-{}-{}", machine.slug(), space.id.0);

        let faint_text = colors.text_muted.opacity(0.4);
        let group_name = SharedString::from(format!("{id}-group"));
        // The sidebar's shell row: the icon and name, then the branch with what's inside and
        // the machine below.
        let main_line = h_flex()
            .relative()
            .h_6()
            .gap_2p5()
            .child(div().flex_none().child(icon))
            .child(if is_renaming {
                self.render_rename_input(cx)
            } else {
                div()
                    .flex_1()
                    .min_w_0()
                    .child(Label::new(label.clone()).color(Color::Muted).truncate())
                    .into_any_element()
            })
            .when_some(status.filter(|_| !is_renaming), |line, status| {
                line.child(div().flex_none().child(render_status_pill(status, cx)))
            })
            // A group's parent folds its worktrees away (herdr's ▾/▸).
            .when_some(entry.group.clone(), |line, group| {
                let is_collapsed = self.collapsed_groups.contains(&group);
                line.child(
                    IconButton::new(
                        ElementId::Name(format!("{id}-group-toggle").into()),
                        if is_collapsed {
                            IconName::ChevronRight
                        } else {
                            IconName::ChevronDown
                        },
                    )
                    .icon_size(IconSize::XSmall)
                    .icon_color(Color::Muted)
                    .tooltip(Tooltip::text(if is_collapsed {
                        "Show Worktrees"
                    } else {
                        "Hide Worktrees"
                    }))
                    .on_click(
                        cx.listener(move |this, _, _, cx| this.toggle_group(group.clone(), cx)),
                    ),
                )
            });
        let faint_label = |text: String| {
            Label::new(text)
                .size(LabelSize::Small)
                .color(Color::Custom(faint_text))
        };
        let count_badge = |icon: IconName, count: usize| {
            h_flex()
                .flex_none()
                .gap_0p5()
                .child(
                    Icon::new(icon)
                        .size(IconSize::XSmall)
                        .color(Color::Custom(faint_text)),
                )
                .child(
                    Label::new(count.to_string())
                        .size(LabelSize::XSmall)
                        .color(Color::Custom(faint_text)),
                )
        };
        // The branch inside a repository, and where it is outside one.
        let detail_line = h_flex()
            // Under the title, past the icon and the gap.
            .pl(px(26.))
            .min_w_0()
            .gap_1()
            .child(match &git {
                // A worktree's title is its branch already; say where it is.
                Some(_) if entry.child.is_some() => faint_label(path.to_string()).truncate_middle(),
                Some(git) => faint_label(repository_branch(
                    Some(&label),
                    git.repository.as_deref(),
                    git.branch.as_deref().unwrap_or("detached"),
                ))
                .truncate_middle(),
                None => faint_label(path.to_string()).truncate_middle(),
            })
            // Unpushed commits in the theme's added color, unpulled ones in its deleted color.
            .when_some(git.as_ref().filter(|git| git.ahead > 0), |line, git| {
                line.child(
                    Label::new(format!("↑{}", git.ahead))
                        .size(LabelSize::Small)
                        .color(Color::Created),
                )
            })
            .when_some(git.as_ref().filter(|git| git.behind > 0), |line, git| {
                line.child(
                    Label::new(format!("↓{}", git.behind))
                        .size(LabelSize::Small)
                        .color(Color::Deleted),
                )
            })
            .child(div().flex_1())
            .when_some(contents, |line, contents| {
                line.child(
                    h_flex()
                        .id(ElementId::Name(format!("{id}-contents").into()))
                        .flex_none()
                        .gap_1p5()
                        .tooltip(Tooltip::text(contents))
                        // The tooltip says it already, so the details give way to it.
                        .debug_selector({
                            let id = format!("{id}-contents");
                            move || id
                        })
                        .on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
                            this.hovered_contents = hovered.then_some(key);
                            if *hovered {
                                this.hide_details(cx);
                            } else if this.hovered_space == Some(key) {
                                // Back on the row rather than off it.
                                this.space_hovered(key, true, cx);
                            }
                        }))
                        // The agents by their own icons, a few at most; shells by count.
                        .children(agent_icons.iter().take(MAX_ROW_AGENT_ICONS).cloned().map(
                            |icon| icon.size(IconSize::XSmall).color(Color::Custom(faint_text)),
                        ))
                        .when(agent_icons.len() > MAX_ROW_AGENT_ICONS, |this| {
                            this.child(
                                Label::new(format!("+{}", agent_icons.len() - MAX_ROW_AGENT_ICONS))
                                    .size(LabelSize::XSmall)
                                    .color(Color::Custom(faint_text)),
                            )
                        })
                        .when(terminals > 0, |this| {
                            this.child(count_badge(IconName::Terminal, terminals))
                        }),
                )
            })
            // Which machine, once there's more than one.
            .when(has_remotes, |line| {
                line.child(
                    div().flex_none().opacity(0.6).child(
                        Icon::new(machine_icon)
                            .size(IconSize::Small)
                            .color(Color::Muted),
                    ),
                )
            });

        let row =
            v_flex()
                .id(ElementId::Name(id.clone().into()))
                .debug_selector({
                    let id = id.clone();
                    move || id
                })
                .group(group_name)
                .mx_1()
                // A worktree sits under its parent.
                .when(entry.child.is_some(), |row| row.ml(CHILD_ROW_INDENT))
                .px_2p5()
                .py_1p5()
                .rounded_md()
                .cursor_pointer()
                .when(is_active, |row| row.bg(colors.ghost_element_selected))
                .hover(|row| row.bg(colors.ghost_element_hover))
                .child(main_line)
                .child(detail_line)
                .on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
                    this.space_hovered(key, *hovered, cx)
                }))
                .on_any_mouse_down(cx.listener(|this, _, _, cx| this.hide_details(cx)))
                .on_click(cx.listener(move |this, event: &ClickEvent, window, cx| {
                    if event.click_count() == 2 {
                        let label = this
                            .space(key, cx)
                            .map(|space| space.label())
                            .unwrap_or_default();
                        this.start_renaming(RenameTarget::Space(key), label.into(), window, cx);
                    } else {
                        this.activate_space(key, window, cx);
                    }
                }))
                .on_drag(
                    DraggedLabel {
                        item: key,
                        label: label.clone(),
                    },
                    |dragged, _, _, cx| cx.new(|_| dragged.clone()),
                )
                .drag_over::<DraggedLabel<SpaceKey>>(move |style, dragged, _, cx| {
                    if dragged.item.machine == machine {
                        style.bg(cx.theme().colors().drop_target_background)
                    } else {
                        style
                    }
                })
                .on_drop(
                    cx.listener(move |this, dragged: &DraggedLabel<SpaceKey>, _, cx| {
                        if dragged.item.machine == machine && dragged.item != key {
                            this.send(
                                machine,
                                SpaceRequest::MoveSpace {
                                    space: dragged.item.space,
                                    index,
                                },
                                cx,
                            );
                        }
                    }),
                );

        // herdr's tree lines from the parent to each worktree: ├ for one with more below,
        // └ for the last.
        let connector = entry.child.map(|is_last| {
            let line = colors.border;
            div()
                .absolute()
                .top_0()
                .left(CONNECTOR_LEFT)
                .w(px(8.))
                .map(|this| if is_last { this.h_1_2() } else { this.h_full() })
                .border_l_1()
                .border_color(line)
                .child(
                    div()
                        .absolute()
                        .left_0()
                        .top_1_2()
                        .w_full()
                        .border_t_1()
                        .border_color(line),
                )
                .into_any_element()
        });
        // A worktree or pasture in a group can be deleted from its row (herdr).
        let deletable = entry
            .child
            .and_then(|_| self.checkout_to_delete(machine, space, cx));
        let this = cx.entity().downgrade();
        let folder = space.current_folder().to_path_buf();
        // New Thread Here starts in the checkout the workspace is in.
        let thread_target = project_key.zip(checkout_root);
        right_click_menu(ElementId::Name(format!("{id}-menu").into()))
            .trigger(move |is_menu_open, _, _| {
                div()
                    .relative()
                    .when_some(connector, |this, connector| this.child(connector))
                    .child(row)
                    .when(!is_menu_open, |this| this.children(details_popover))
            })
            .menu(move |window, cx| {
                let this = this.clone();
                let label = label.clone();
                let worktree_source = worktree_source.clone();
                let folder = folder.clone();
                let thread_target = thread_target.clone();
                let deletable = deletable.clone();
                ContextMenu::build(window, cx, move |menu, _, _| {
                    let new_tab = {
                        let this = this.clone();
                        move |window: &mut Window, cx: &mut App| {
                            this.update(cx, |this, cx| this.new_tab_in(key, window, cx))
                                .ok();
                        }
                    };
                    let copy_path = {
                        let folder = folder.clone();
                        move |_: &mut Window, cx: &mut App| {
                            cx.write_to_clipboard(ClipboardItem::new_string(
                                folder.to_string_lossy().into_owned(),
                            ));
                        }
                    };
                    let reveal = {
                        let folder = folder.clone();
                        move |_: &mut Window, cx: &mut App| cx.reveal_path(&folder)
                    };
                    let rename = {
                        let this = this.clone();
                        let label = label.clone();
                        move |window: &mut Window, cx: &mut App| {
                            this.update(cx, |this, cx| {
                                this.start_renaming(
                                    RenameTarget::Space(key),
                                    label.clone(),
                                    window,
                                    cx,
                                )
                            })
                            .ok();
                        }
                    };
                    let close = {
                        let this = this.clone();
                        move |window: &mut Window, cx: &mut App| {
                            this.update(cx, |this, cx| this.close_space(key, window, cx))
                                .ok();
                        }
                    };
                    let worktree =
                        |mode: WorktreeModalMode, (folder, name): (PathBuf, SharedString)| {
                            let this = this.clone();
                            move |_: &mut Window, cx: &mut App| {
                                this.update(cx, |_, cx| {
                                    cx.emit(SpacesViewEvent::Worktree {
                                        machine,
                                        folder: folder.clone(),
                                        name: name.clone(),
                                        mode,
                                    })
                                })
                                .ok();
                            }
                        };
                    menu.item(
                        ContextMenuEntry::new("New Tab")
                            .icon(IconName::Plus)
                            .icon_color(Color::Muted)
                            .action(Box::new(NewTab))
                            .handler(new_tab),
                    )
                    .item(
                        ContextMenuEntry::new("Rename")
                            .icon(IconName::Pencil)
                            .icon_color(Color::Muted)
                            .handler(rename),
                    )
                    .item(
                        ContextMenuEntry::new("Copy Path")
                            .icon(IconName::Copy)
                            .icon_color(Color::Muted)
                            .handler(copy_path),
                    )
                    // Finder can only show this Mac's folders.
                    .when(machine == MachineId::Local, |menu| {
                        menu.item(
                            ContextMenuEntry::new("Reveal in Finder")
                                .icon(IconName::FolderOpen)
                                .icon_color(Color::Muted)
                                .handler(reveal),
                        )
                    })
                    .when_some(thread_target.clone(), |menu, (project, checkout)| {
                        let this = this.clone();
                        menu.item(
                            ContextMenuEntry::new("New Thread Here")
                                .icon(IconName::Chat)
                                .icon_color(Color::Muted)
                                .handler(move |_, cx| {
                                    this.update(cx, |_, cx| {
                                        cx.emit(SpacesViewEvent::NewThread {
                                            project,
                                            folder: checkout.clone(),
                                        })
                                    })
                                    .ok();
                                }),
                        )
                    })
                    .when_some(worktree_source, |menu, source| {
                        menu.separator()
                            .item(
                                ContextMenuEntry::new("New Worktree…")
                                    .icon(IconName::GitWorktree)
                                    .icon_color(Color::Muted)
                                    .handler(worktree(WorktreeModalMode::New, source.clone())),
                            )
                            .item(
                                ContextMenuEntry::new("Open Worktree…")
                                    .icon(IconName::FolderOpen)
                                    .icon_color(Color::Muted)
                                    .handler(worktree(WorktreeModalMode::Open, source)),
                            )
                    })
                    .when_some(deletable.clone(), |menu, checkout| {
                        let this = this.clone();
                        menu.item(
                            ContextMenuEntry::new("Delete Worktree Checkout…")
                                .icon(IconName::Trash)
                                .icon_color(Color::Muted)
                                .handler(move |window, cx| {
                                    this.update(cx, |this, cx| {
                                        this.delete_checkout(key, checkout.clone(), window, cx)
                                    })
                                    .ok();
                                }),
                        )
                    })
                    .separator()
                    .item(
                        ContextMenuEntry::new("Close Workspace")
                            .icon(IconName::Close)
                            .icon_color(Color::Muted)
                            .handler(close),
                    )
                })
            })
            .into_any_element()
    }

    /// Which worktree group a workspace belongs to: a project's pasture under the project, or
    /// any checkout under its repository's main checkout.
    fn group_membership(
        &self,
        machine: MachineId,
        space: &Space,
        cx: &App,
    ) -> Option<(PathBuf, GroupRole)> {
        let folder = space.current_folder();
        let pasture_of = self
            .machines
            .read(cx)
            .projects(machine, cx)
            .and_then(|store| {
                store.read(cx).projects().iter().find_map(|project| {
                    project
                        .workspaces
                        .iter()
                        .any(|workspace| {
                            workspace.kind == projects::WorkspaceKind::Pasture
                                && folder.starts_with(&workspace.path)
                        })
                        .then(|| project.path.clone())
                })
            });
        if let Some(main) = pasture_of {
            return Some((main, GroupRole::Child));
        }
        let git = space.git.as_ref()?;
        let role = if git.is_linked_worktree() {
            GroupRole::Child
        } else {
            GroupRole::Parent
        };
        Some((git.main_checkout.clone()?, role))
    }

    /// The worktree or pasture a workspace is in, which Delete Worktree Checkout removes.
    fn checkout_to_delete(&self, machine: MachineId, space: &Space, cx: &App) -> Option<PathBuf> {
        let folder = space.current_folder();
        let pasture = self
            .machines
            .read(cx)
            .projects(machine, cx)
            .and_then(|store| {
                store.read(cx).projects().iter().find_map(|project| {
                    project
                        .workspaces
                        .iter()
                        .find(|workspace| {
                            workspace.kind == projects::WorkspaceKind::Pasture
                                && folder.starts_with(&workspace.path)
                        })
                        .map(|workspace| workspace.path.clone())
                })
            });
        pasture.or_else(|| {
            let git = space.git.as_ref().filter(|git| git.is_linked_worktree())?;
            git.checkout.clone()
        })
    }

    /// herdr's Delete worktree checkout: asks, removes it safely, asks again before forcing
    /// when it has changes, and closes its workspace. The branch stays.
    fn delete_checkout(
        &mut self,
        key: SpaceKey,
        checkout: PathBuf,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(store) = self.machines.read(cx).projects(key.machine, cx) else {
            return;
        };
        let Some(space) = self.space(key, cx) else {
            return;
        };
        let branch = space.git.as_ref().and_then(|git| git.branch.clone());
        let name = checkout
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        let detail = match &branch {
            Some(branch) => format!(
                "Its folder {} is deleted from disk, and its workspace closes. The branch {branch} stays.",
                compact_path(&checkout)
            ),
            None => format!(
                "Its folder {} is deleted from disk, and its workspace closes.",
                compact_path(&checkout)
            ),
        };
        let answer = window.prompt(
            PromptLevel::Warning,
            &format!("Delete the {name} checkout?"),
            Some(&detail),
            &["Delete", "Cancel"],
            cx,
        );
        cx.spawn_in(window, async move |this, cx| {
            if answer.await != Ok(0) {
                return;
            }
            let removal = remove_workspace(&store, checkout.clone(), false, cx).await;
            let result = match removal {
                Ok(WorkspaceRemoval::NeedsConfirmation(reason)) => {
                    let Ok(answer) = cx.update(|window, cx| {
                        window.prompt(
                            PromptLevel::Warning,
                            &format!("Delete the {name} checkout anyway?"),
                            Some(&reason),
                            &["Delete Anyway", "Cancel"],
                            cx,
                        )
                    }) else {
                        return;
                    };
                    if answer.await != Ok(0) {
                        return;
                    }
                    remove_workspace(&store, checkout, true, cx).await
                }
                removal => removal,
            };
            match result {
                Ok(WorkspaceRemoval::Removed) => {
                    this.update(cx, |this, cx| {
                        this.send(key.machine, SpaceRequest::CloseSpace(key.space), cx)
                    })
                    .ok();
                }
                Ok(_) => {}
                Err(error) => {
                    if let Ok(answer) = cx.update(|window, cx| {
                        window.prompt(
                            PromptLevel::Critical,
                            &format!("Couldn't delete the {name} checkout"),
                            Some(&format!("{error:#}")),
                            &["OK"],
                            cx,
                        )
                    }) {
                        answer.await.ok();
                    }
                }
            }
        })
        .detach();
    }

    fn toggle_group(&mut self, group: GroupKey, cx: &mut Context<Self>) {
        if !self.collapsed_groups.remove(&group) {
            self.collapsed_groups.insert(group);
        }
        cx.notify();
    }

    /// A workspace's checkout at a glance: the branch and its upstream, what isn't committed,
    /// the last commit, where it is, and its worktrees.
    fn render_git_glance(
        &self,
        entry: &SpaceEntry,
        git: &SpaceGit,
        title: &SharedString,
        path: &SharedString,
        cx: &App,
    ) -> AnyElement {
        let small_icon = |name: IconName| {
            Icon::new(name)
                .size(IconSize::XSmall)
                .color(Color::Muted)
                .into_any_element()
        };
        let detail = |text: String| Label::new(text).size(LabelSize::Small);
        let mut rows = Vec::new();
        let branch = git.branch.clone().unwrap_or_else(|| "detached".to_string());
        rows.push(
            h_flex()
                .min_w_0()
                .gap_2()
                .child(small_icon(IconName::GitBranch))
                .child(
                    h_flex()
                        .min_w_0()
                        .gap_1()
                        .child(detail(branch).truncate())
                        .children(git.upstream.clone().map(|upstream| {
                            detail(format!("→ {upstream}"))
                                .color(Color::Muted)
                                .truncate()
                        }))
                        .when(git.ahead > 0, |line| {
                            line.child(detail(format!("↑{}", git.ahead)).color(Color::Created))
                        })
                        .when(git.behind > 0, |line| {
                            line.child(detail(format!("↓{}", git.behind)).color(Color::Deleted))
                        }),
                )
                .into_any_element(),
        );
        let changes = git.changes;
        rows.push(if changes.files == 0 {
            details_row(
                small_icon(IconName::Diff),
                Label::new("Nothing uncommitted"),
                cx,
            )
        } else {
            h_flex()
                .gap_2()
                .child(small_icon(IconName::Diff))
                .child(
                    h_flex()
                        .gap_1()
                        .child(detail(format!(
                            "{} {} changed",
                            changes.files,
                            if changes.files == 1 { "file" } else { "files" }
                        )))
                        .when(changes.added > 0, |line| {
                            line.child(detail(format!("+{}", changes.added)).color(Color::Created))
                        })
                        .when(changes.removed > 0, |line| {
                            line.child(
                                detail(format!("−{}", changes.removed)).color(Color::Deleted),
                            )
                        }),
                )
                .into_any_element()
        });
        if let Some(commit) = &git.last_commit {
            let time = UNIX_EPOCH + Duration::from_secs(commit.time);
            rows.push(details_row(
                small_icon(IconName::GitCommit),
                Label::new(format!(
                    "“{}” · {}",
                    commit.subject,
                    format_relative_time(time, SystemTime::now())
                ))
                .truncate(),
                cx,
            ));
        }
        rows.push(details_row(
            small_icon(IconName::Folder),
            Label::new(path.clone()).truncate_middle(),
            cx,
        ));
        // A parent lists its worktrees; a worktree says whose it is.
        let worktrees = self.group_worktree_names(entry, cx);
        if !worktrees.is_empty() {
            rows.push(details_row(
                small_icon(IconName::GitWorktree),
                Label::new(format!(
                    "{} {}: {}",
                    worktrees.len(),
                    if worktrees.len() == 1 {
                        "worktree"
                    } else {
                        "worktrees"
                    },
                    worktrees.join(", ")
                ))
                .truncate(),
                cx,
            ));
        } else if let Some(repository) = git.repository.as_ref().filter(|_| entry.child.is_some()) {
            rows.push(details_row(
                small_icon(IconName::GitWorktree),
                Label::new(format!("Worktree of {repository}")).truncate(),
                cx,
            ));
        }
        details_card(title.clone(), rows, cx)
    }

    /// The names of a group parent's worktrees, folded or not.
    fn group_worktree_names(&self, entry: &SpaceEntry, cx: &App) -> Vec<String> {
        let Some(group) = &entry.group else {
            return Vec::new();
        };
        self.all_spaces(cx)
            .into_iter()
            .filter(|(machine, space)| {
                self.group_membership(*machine, space, cx)
                    .is_some_and(|(main, role)| {
                        role == GroupRole::Child && (*machine, main) == *group
                    })
            })
            .map(|(_, space)| child_label(&space).unwrap_or_else(|| space.label()))
            .collect()
    }

    /// Shows a workspace's details after a moment, like the thread cards do.
    fn space_hovered(&mut self, key: SpaceKey, hovered: bool, cx: &mut Context<Self>) {
        if !hovered {
            if self.hovered_space == Some(key) {
                self.hovered_space = None;
            }
            let is_pending = self
                .details_delay
                .as_ref()
                .is_some_and(|(pending, _)| *pending == key);
            if is_pending || self.details_space == Some(key) {
                self.hide_details(cx);
            }
            return;
        }
        self.hovered_space = Some(key);
        if self.details_space == Some(key) || self.hovered_contents == Some(key) {
            return;
        }
        let delay = cx.spawn(async move |this, cx| {
            cx.background_executor().timer(DETAILS_DELAY).await;
            this.update(cx, |this, cx| {
                this.details_delay = None;
                this.details_space = Some(key);
                cx.notify();
            })
            .ok();
        });
        self.details_delay = Some((key, delay));
    }

    fn hide_details(&mut self, cx: &mut Context<Self>) {
        self.details_delay = None;
        if self.details_space.take().is_some() {
            cx.notify();
        }
    }

    /// How many terminals and agents a workspace holds across its tabs. An agent CLI in a
    /// terminal is an agent, not a terminal, and a thread in several panes counts once.
    fn space_contents(&self, machine: MachineId, space: &Space, cx: &App) -> (usize, usize) {
        let store = self.machines.read(cx).projects(machine, cx);
        let store = store.as_ref().map(|store| store.read(cx));
        let mut threads = HashSet::default();
        let (mut terminals, mut agents) = (0, 0);
        for pane in space.tabs.iter().flat_map(|tab| &tab.panes) {
            match &pane.content {
                PaneContent::Terminal(_) if pane.agent.is_some() => agents += 1,
                PaneContent::Terminal(_) => terminals += 1,
                PaneContent::Thread(thread_id) => {
                    if !threads.insert(*thread_id) {
                        continue;
                    }
                    let is_terminal = store
                        .and_then(|store| store.thread(*thread_id))
                        .is_some_and(|thread| thread.terminal.is_some());
                    let has_agent =
                        store.is_some_and(|store| store.terminal_agent(*thread_id).is_some());
                    if is_terminal && !has_agent {
                        terminals += 1;
                    } else {
                        agents += 1;
                    }
                }
                PaneContent::Unknown(_) => {}
            }
        }
        (terminals, agents)
    }

    /// The icons of the agents in a workspace's panes, each thread once.
    fn space_agent_icons(&self, machine: MachineId, space: &Space, cx: &App) -> Vec<Icon> {
        let store = self.machines.read(cx).projects(machine, cx);
        let store = store.as_ref().map(|store| store.read(cx));
        let mut threads = HashSet::default();
        let mut icons = Vec::new();
        for pane in space.tabs.iter().flat_map(|tab| &tab.panes) {
            match &pane.content {
                PaneContent::Terminal(_) if pane.agent.is_some() => {
                    icons.push(pane_agent_icon(pane, cx))
                }
                PaneContent::Thread(thread_id) if threads.insert(*thread_id) => {
                    let Some(thread) = store.and_then(|store| store.thread(*thread_id)) else {
                        continue;
                    };
                    let has_agent_cli =
                        store.is_some_and(|store| store.terminal_agent(*thread_id).is_some());
                    if thread.terminal.is_none() {
                        icons.push(thread_agent_icon(thread, cx));
                    } else if has_agent_cli {
                        icons.push(Icon::new(IconName::ZedAgent));
                    }
                }
                PaneContent::Terminal(_) | PaneContent::Thread(_) | PaneContent::Unknown(_) => {}
            }
        }
        icons
    }

    /// The agents in panes on every machine, in workspace and tab order as herdr lists them:
    /// agent CLIs found in terminal panes, and agent threads. A terminal thread is no agent.
    fn agent_entries(&self, cx: &App) -> Vec<AgentEntry> {
        let machines = self.machines.read(cx);
        let mut entries = Vec::new();
        let mut threads_in_panes = HashSet::default();
        for (machine, space) in self.all_spaces(cx) {
            let space_label: SharedString = space.label().into();
            for (index, tab) in space.tabs.iter().enumerate() {
                for pane in &tab.panes {
                    let is_agent = match &pane.content {
                        // A thread in several panes is listed at the first.
                        PaneContent::Thread(thread) => {
                            let is_agent = machines
                                .projects(machine, cx)
                                .and_then(|store| store.read(cx).thread(*thread).cloned())
                                .is_some_and(|thread| thread.terminal.is_none());
                            is_agent
                                && threads_in_panes.insert(ThreadKey {
                                    machine,
                                    thread: *thread,
                                })
                        }
                        PaneContent::Terminal(_) => pane.agent.is_some(),
                        PaneContent::Unknown(_) => false,
                    };
                    if !is_agent {
                        continue;
                    }
                    let key = PaneKey {
                        machine,
                        pane: pane.id,
                    };
                    let (icon, title, _) = self.pane_title(key, pane, cx);
                    entries.push(AgentEntry {
                        pane: key,
                        icon,
                        title,
                        status: self.pane_status(machine, pane, cx),
                        space: space_label.clone(),
                        tab: tab_label(tab, index).into(),
                    });
                }
            }
        }
        entries
    }

    fn render_agents(
        &self,
        has_remotes: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let entries = self.agent_entries(cx);
        let focused_pane = self.is_visible.then(|| self.focused_pane(cx)).flatten();
        let rows = entries
            .into_iter()
            .enumerate()
            .map(|(index, entry)| {
                self.render_agent_row(index, entry, focused_pane, has_remotes, cx)
            })
            .collect::<Vec<_>>();
        let has_rows = !rows.is_empty();
        let rule_color = cx.theme().colors().border_variant;

        v_flex()
            .flex_none()
            .pt_1()
            .child(
                h_flex()
                    .h_8()
                    .mx_1()
                    .px_2()
                    .gap_2()
                    .child(
                        Label::new("Agents")
                            .size(LabelSize::Small)
                            .weight(gpui::FontWeight::MEDIUM)
                            .color(Color::Muted),
                    )
                    .child(div().flex_1().min_w_2().h_px().bg(rule_color)),
            )
            .child(
                div()
                    .relative()
                    .child(
                        v_flex()
                            .id("workspace-agents-list")
                            .max_h(window.viewport_size().height * 0.4)
                            .overflow_y_scroll()
                            .track_scroll(&self.agents_scroll)
                            .px_1()
                            .pb_1()
                            .children(rows)
                            .when(!has_rows, |list| {
                                list.child(
                                    div().px_2().py_1().child(
                                        Label::new("No agents yet")
                                            .size(LabelSize::Small)
                                            .color(Color::Muted),
                                    ),
                                )
                            }),
                    )
                    .vertical_scrollbar_for(&self.agents_scroll, window, cx),
            )
    }

    /// The agents in panes waiting on the user, and the most urgent of their states.
    pub fn waiting(&self, cx: &App) -> (usize, Option<ThreadStatus>) {
        let statuses: Vec<ThreadStatus> = self
            .agent_entries(cx)
            .into_iter()
            .filter_map(|entry| entry.status)
            .filter(|status| {
                matches!(
                    status,
                    ThreadStatus::PendingApproval | ThreadStatus::AwaitingInput
                )
            })
            .collect();
        (statuses.len(), rolled_up(statuses.into_iter()))
    }

    /// The agents waiting on the user (an approval or an answer), in a tinted strip above the
    /// workspaces, each with Go. Nothing shows while none waits.
    fn render_needs_you(&self, has_remotes: bool, cx: &mut Context<Self>) -> Option<AnyElement> {
        let waiting: Vec<AgentEntry> = self
            .agent_entries(cx)
            .into_iter()
            .filter(|entry| {
                matches!(
                    entry.status,
                    Some(ThreadStatus::PendingApproval | ThreadStatus::AwaitingInput)
                )
            })
            .collect();
        if waiting.is_empty() {
            return None;
        }
        let warning = cx.theme().status().warning;
        let rows = waiting.into_iter().enumerate().map(|(index, entry)| {
            let pane = entry.pane;
            let location = match has_remotes.then(|| self.machines.read(cx).label(pane.machine, cx))
            {
                Some(machine) => format!("{machine} · {} › {}", entry.space, entry.tab),
                None => format!("{} › {}", entry.space, entry.tab),
            };
            h_flex()
                .h(px(34.))
                .px_1p5()
                .gap_2()
                .child(render_state_slot(entry.status, cx))
                .child(entry.icon.size(IconSize::Small).color(Color::Muted))
                .child(
                    v_flex()
                        .flex_1()
                        .min_w_0()
                        .child(Label::new(entry.title).size(LabelSize::Small).truncate())
                        .child(
                            Label::new(location)
                                .size(LabelSize::XSmall)
                                .color(Color::Muted)
                                .truncate(),
                        ),
                )
                .child(
                    Button::new(
                        ElementId::Name(format!("needs-you-go-{index}").into()),
                        "Go",
                    )
                    .style(ButtonStyle::Outlined)
                    .label_size(LabelSize::Small)
                    .on_click(
                        cx.listener(move |this, _, window, cx| this.focus_pane(pane, window, cx)),
                    ),
                )
        });
        Some(
            v_flex()
                .id("needs-you")
                .mx_1p5()
                .mt_1p5()
                .p_0p5()
                .rounded_md()
                .border_1()
                .border_color(warning.opacity(0.35))
                .bg(warning.opacity(0.07))
                .children(rows)
                .into_any_element(),
        )
    }

    /// herdr's default agent row: the state and where it is (machine, workspace, tab), then
    /// the agent below.
    fn render_agent_row(
        &self,
        index: usize,
        entry: AgentEntry,
        focused_pane: Option<PaneKey>,
        has_remotes: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let colors = cx.theme().colors();
        let pane = entry.pane;
        let is_active = Some(pane) == focused_pane;
        let is_offline = !self.machines.read(cx).is_online(pane.machine, cx);
        let location = match has_remotes.then(|| self.machines.read(cx).label(pane.machine, cx)) {
            Some(machine) => format!("{machine} · {} › {}", entry.space, entry.tab),
            None => format!("{} › {}", entry.space, entry.tab),
        };

        v_flex()
            .id(ElementId::Name(format!("workspace-agent-{index}").into()))
            .debug_selector(|| format!("agent-row-{index}"))
            .w_full()
            .px_2()
            .py_1()
            .gap_0p5()
            .rounded_md()
            .cursor_pointer()
            .when(is_active, |row| row.bg(colors.ghost_element_selected))
            .when(!is_active, |row| {
                row.hover(|row| row.bg(colors.ghost_element_hover))
            })
            .when(is_offline, |row| row.opacity(0.5))
            .child(
                h_flex()
                    .gap_2()
                    .child(render_state_slot(entry.status, cx))
                    .child(
                        div().flex_1().min_w_0().child(
                            Label::new(location)
                                .size(LabelSize::XSmall)
                                .color(Color::Muted)
                                .truncate(),
                        ),
                    ),
            )
            .child(
                h_flex()
                    // Under the location, past the state's slot.
                    .pl(AGENT_ROW_INDENT)
                    .gap_2()
                    .child(entry.icon.size(IconSize::Small).color(Color::Muted))
                    .child(
                        div().flex_1().min_w_0().child(
                            Label::new(entry.title)
                                .size(LabelSize::Small)
                                .color(if is_active {
                                    Color::Default
                                } else {
                                    Color::Muted
                                })
                                .truncate(),
                        ),
                    ),
            )
            .on_click(cx.listener(move |this, _, window, cx| this.focus_pane(pane, window, cx)))
            .into_any_element()
    }

    /// A pane's state: its thread's, or its terminal agent's.
    fn pane_status(&self, machine: MachineId, pane: &Pane, cx: &App) -> Option<ThreadStatus> {
        match &pane.content {
            PaneContent::Thread(thread_id) => self
                .machines
                .read(cx)
                .projects(machine, cx)?
                .read(cx)
                .thread_status(*thread_id),
            PaneContent::Terminal(_) | PaneContent::Unknown(_) => self
                .machines
                .read(cx)
                .client(machine, cx)?
                .read(cx)
                .pane_agent_status(pane),
        }
    }

    /// A pane's icon, title, and a detail: where a terminal is, or a thread's agent.
    fn pane_title(
        &self,
        key: PaneKey,
        pane: &Pane,
        cx: &App,
    ) -> (Icon, SharedString, Option<SharedString>) {
        match &pane.content {
            PaneContent::Terminal(terminal) => {
                // What runs there: its agent, the command it was opened with, or what's in
                // front of the shell.
                let title = pane
                    .agent
                    .as_ref()
                    .map(|agent| agent.name.clone())
                    .or_else(|| terminal.command.clone())
                    .or_else(|| pane.program.clone())
                    .unwrap_or_else(|| "Shell".to_string());
                let folder = pane.folder.as_ref().and_then(|folder| {
                    let (space, _, _) = self.find_pane(key, cx)?;
                    Some(pane_folder_label(&space, folder).into())
                });
                (pane_agent_icon(pane, cx), title.into(), folder)
            }
            PaneContent::Thread(thread_id) => {
                let machines = self.machines.read(cx);
                let thread = machines
                    .projects(key.machine, cx)
                    .and_then(|store| store.read(cx).thread(*thread_id).cloned());
                match thread {
                    // Like the thread's toolbar in the Agents view: its title, then its agent.
                    Some(thread) => (
                        thread_agent_icon(&thread, cx),
                        thread.title.clone().into(),
                        self.panes.get(&key).and_then(|open| match &open.view {
                            PaneView::Agent(view) => Some(view.read(cx).agent_name(cx)),
                            PaneView::Terminal(_) => None,
                        }),
                    ),
                    None => (Icon::new(IconName::Chat), "Thread".into(), None),
                }
            }
            PaneContent::Unknown(_) => (Icon::new(IconName::Screen), "Unknown".into(), None),
        }
    }

    fn render_main(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let Some((space_key, space, tab_key)) = self.visible_tab(cx) else {
            return self.render_empty_state(cx).into_any_element();
        };
        let Some(tab) = space.tabs.iter().find(|tab| tab.id == tab_key.tab).cloned() else {
            return self.render_empty_state(cx).into_any_element();
        };
        for pane in &tab.panes {
            let key = PaneKey {
                machine: tab_key.machine,
                pane: pane.id,
            };
            self.ensure_view(key, &pane.content, cx);
        }
        let focused = self.layouts.get(&tab_key).map(TileLayout::focused);
        let zoomed_pane = focused
            .filter(|_| self.zoomed.contains(&tab_key) && tab.panes.len() > 1)
            .and_then(|pane| tab.pane(pane));
        let body = match zoomed_pane {
            Some(pane) => self.render_pane(tab_key, pane, true, false, true, cx),
            None => self.render_node(&tab.root, Vec::new(), tab_key, &tab, focused, cx),
        };

        v_flex()
            .flex_1()
            .min_w_0()
            .h_full()
            .child(self.render_tab_bar(space_key, &space, tab_key.tab, cx))
            .child(div().flex_1().min_h_0().child(body))
            .into_any_element()
    }

    /// Zed's empty pane: what's missing, and the shortcut that makes one.
    fn render_empty_state(&self, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .size_full()
            .items_center()
            .justify_center()
            .gap_1()
            .child(Label::new("No workspaces").color(Color::Muted))
            .child(
                Button::new("start-workspace", "New Workspace")
                    .label_size(LabelSize::Small)
                    .color(Color::Muted)
                    .key_binding(
                        ui::KeyBinding::for_action_in(&NewWorkspace, &self.focus_handle, cx)
                            .size(rems_from_px(12_f32)),
                    )
                    .on_click(
                        cx.listener(|this, _, window, cx| this.new_space_handle.show(window, cx)),
                    ),
            )
    }

    fn render_tab_bar(
        &self,
        space_key: SpaceKey,
        space: &Space,
        active_tab: TabId,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let selected = space
            .tabs
            .iter()
            .position(|tab| tab.id == active_tab)
            .unwrap_or_default();
        let count = space.tabs.len();
        let tabs: Vec<AnyElement> = space
            .tabs
            .iter()
            .enumerate()
            .map(|(index, tab)| self.render_tab(space_key, index, tab, selected, count, cx))
            .collect();
        TabBar::new("workspace-tabs").children(tabs).end_child(
            IconButton::new("new-workspace-tab", IconName::Plus)
                .icon_size(IconSize::Small)
                .tooltip(|_, cx| Tooltip::for_action("New Tab", &NewTab, cx))
                .on_click(
                    cx.listener(move |this, _, window, cx| this.new_tab_in(space_key, window, cx)),
                ),
        )
    }

    fn render_tab(
        &self,
        space_key: SpaceKey,
        index: usize,
        tab: &Tab,
        selected: usize,
        count: usize,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let machine = space_key.machine;
        let tab_key = TabKey {
            machine,
            tab: tab.id,
        };
        let label: SharedString = tab_label(tab, index).into();
        let is_automatic = tab.name.is_none();
        let status = rolled_up(
            tab.panes
                .iter()
                .filter_map(|pane| self.pane_status(machine, pane, cx)),
        );
        let position = if index == 0 {
            TabPosition::First
        } else if index + 1 == count {
            TabPosition::Last
        } else {
            TabPosition::Middle(index.cmp(&selected))
        };
        let is_renaming = self.renaming == Some(RenameTarget::Tab(tab_key));
        let id = format!("workspace-tab-{}-{}", machine.slug(), tab.id.0);
        let dragged_tab = DraggedTab {
            space: space_key,
            tab: tab.id,
        };

        let item = TabItem::new(ElementId::Name(id.clone().into()))
            .position(position)
            .toggle_state(index == selected)
            .start_slot::<AnyElement>(
                status.map(|status| render_status_dot(status, cx).into_any_element()),
            )
            .end_slot(
                IconButton::new(
                    ElementId::Name(format!("{id}-close").into()),
                    IconName::Close,
                )
                .icon_size(IconSize::XSmall)
                .visible_on_hover("")
                .tooltip(Tooltip::text("Close Tab"))
                .on_click(
                    cx.listener(move |this, _, window, cx| this.close_tab(tab_key, window, cx)),
                ),
            )
            .child(if is_renaming {
                div()
                    .w(px(120.))
                    .child(self.render_rename_input(cx))
                    .into_any_element()
            } else {
                Label::new(label.clone())
                    .size(LabelSize::Small)
                    .when(is_automatic, |label| label.color(Color::Muted))
                    .into_any_element()
            })
            .when(index < 9, |item| {
                let label = label.clone();
                item.tooltip(move |_, cx| {
                    Tooltip::for_action(label.clone(), &ActivateTab(index + 1), cx)
                })
            })
            .on_click(cx.listener(move |this, event: &ClickEvent, window, cx| {
                if event.click_count() == 2 {
                    let name = this
                        .space(space_key, cx)
                        .and_then(|space| {
                            let index = space.tabs.iter().position(|tab| tab.id == tab_key.tab)?;
                            Some(tab_label(&space.tabs[index], index).into())
                        })
                        .unwrap_or_default();
                    this.start_renaming(RenameTarget::Tab(tab_key), name, window, cx);
                } else {
                    this.activate_tab(tab_key, window, cx);
                }
            }))
            .on_drag(
                DraggedLabel {
                    item: dragged_tab,
                    label: label.clone(),
                },
                |dragged, _, _, cx| cx.new(|_| dragged.clone()),
            )
            .drag_over::<DraggedLabel<DraggedTab>>(move |style, dragged, _, cx| {
                if dragged.item.space == space_key {
                    style.bg(cx.theme().colors().drop_target_background)
                } else {
                    style
                }
            })
            .on_drop(
                cx.listener(move |this, dragged: &DraggedLabel<DraggedTab>, _, cx| {
                    if dragged.item.space == space_key && dragged.item.tab != tab_key.tab {
                        this.send(
                            machine,
                            SpaceRequest::MoveTab {
                                tab: dragged.item.tab,
                                index,
                            },
                            cx,
                        );
                    }
                }),
            );

        let this = cx.entity().downgrade();
        right_click_menu(ElementId::Name(format!("{id}-menu").into()))
            .trigger(move |_, _, _| item)
            .menu(move |window, cx| {
                let this = this.clone();
                let label = label.clone();
                ContextMenu::build(window, cx, move |menu, _, _| {
                    let rename = {
                        let this = this.clone();
                        let label = label.clone();
                        move |window: &mut Window, cx: &mut App| {
                            this.update(cx, |this, cx| {
                                this.start_renaming(
                                    RenameTarget::Tab(tab_key),
                                    label.clone(),
                                    window,
                                    cx,
                                )
                            })
                            .ok();
                        }
                    };
                    let new_tab = {
                        let this = this.clone();
                        move |window: &mut Window, cx: &mut App| {
                            this.update(cx, |this, cx| this.new_tab_in(space_key, window, cx))
                                .ok();
                        }
                    };
                    let close = {
                        let this = this.clone();
                        move |window: &mut Window, cx: &mut App| {
                            this.update(cx, |this, cx| this.close_tab(tab_key, window, cx))
                                .ok();
                        }
                    };
                    menu.entry("Rename", None, rename)
                        .entry("New Tab", Some(Box::new(NewTab)), new_tab)
                        .separator()
                        .entry("Close Tab", None, close)
                })
            })
            .into_any_element()
    }

    /// The tree as nested rows and columns, split by ratio, as herdr lays it out.
    fn render_node(
        &self,
        node: &Node,
        path: Vec<bool>,
        tab_key: TabKey,
        tab: &Tab,
        focused: Option<PaneId>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let (direction, ratio, first, second) = match node {
            Node::Pane(id) => {
                return match tab.pane(*id) {
                    Some(pane) => self.render_pane(
                        tab_key,
                        pane,
                        focused == Some(*id),
                        tab.panes.len() > 1,
                        false,
                        cx,
                    ),
                    None => div().into_any_element(),
                };
            }
            Node::Split {
                direction,
                ratio,
                first,
                second,
            } => (*direction, *ratio, first, second),
        };
        let ratio = self
            .split_override
            .as_ref()
            .filter(|split| split.tab == tab_key && split.path == path)
            .map_or(ratio, |split| split.ratio);
        let is_horizontal = direction == Direction::Horizontal;
        let mut first_path = path.clone();
        first_path.push(false);
        let mut second_path = path.clone();
        second_path.push(true);
        let first = self.render_node(first, first_path, tab_key, tab, focused, cx);
        let second = self.render_node(second, second_path, tab_key, tab, focused, cx);
        let share = |element: AnyElement, share: f32| {
            div()
                .flex_basis(relative(share))
                .flex_shrink(1.)
                .min_w_0()
                .min_h_0()
                .overflow_hidden()
                .map(|cell| {
                    if is_horizontal {
                        cell.h_full()
                    } else {
                        cell.w_full()
                    }
                })
                .child(element)
        };
        let path_label: String = path
            .iter()
            .map(|side| if *side { '1' } else { '0' })
            .collect();
        let drag = DraggedSplit {
            tab: tab_key,
            path: path.clone(),
            direction,
        };

        div()
            .id(ElementId::Name(
                format!(
                    "split-{}-{}-{path_label}",
                    tab_key.machine.slug(),
                    tab_key.tab.0
                )
                .into(),
            ))
            .size_full()
            .flex()
            .map(|split| {
                if is_horizontal {
                    split.flex_row()
                } else {
                    split.flex_col()
                }
            })
            .on_drag_move(
                cx.listener(move |this, event: &DragMoveEvent<DraggedSplit>, _, cx| {
                    let dragged = event.drag(cx);
                    if dragged.tab != tab_key || dragged.path != path {
                        return;
                    }
                    let bounds = event.bounds;
                    let position = event.event.position;
                    let ratio = match dragged.direction {
                        Direction::Horizontal => (position.x - bounds.origin.x) / bounds.size.width,
                        Direction::Vertical => (position.y - bounds.origin.y) / bounds.size.height,
                    };
                    if !ratio.is_finite() {
                        return;
                    }
                    this.split_override = Some(SplitOverride {
                        tab: tab_key,
                        path: path.clone(),
                        ratio: ratio.clamp(0.1, 0.9),
                    });
                    cx.notify();
                }),
            )
            .child(share(first, ratio))
            .child(render_divider(drag, cx))
            .child(share(second, 1. - ratio))
            .into_any_element()
    }

    fn render_pane(
        &self,
        tab_key: TabKey,
        pane: &Pane,
        is_focused: bool,
        shows_focus: bool,
        is_zoomed: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let key = PaneKey {
            machine: tab_key.machine,
            pane: pane.id,
        };
        let colors = cx.theme().colors().clone();
        let (icon, title, detail) = self.pane_title(key, pane, cx);
        let status = self.pane_status(key.machine, pane, cx);
        let view = self
            .panes
            .get(&key)
            .filter(|open| open.content == pane.content)
            .map(|open| open.view.clone());
        // A button for each way to split, each a menu of what the new pane holds: a shell or
        // a thread.
        let split_menu = |direction: Direction, cx: &mut Context<Self>| {
            let this = cx.entity().downgrade();
            let (id, icon, title, action): (_, _, _, Box<dyn Action>) = match direction {
                Direction::Horizontal => (
                    "pane-split-right",
                    IconName::SquareSplitHorizontal,
                    "Split Right",
                    Box::new(SplitRight),
                ),
                Direction::Vertical => (
                    "pane-split-down",
                    IconName::SquareSplitVertical,
                    "Split Down",
                    Box::new(SplitDown),
                ),
            };
            div()
                .debug_selector(|| format!("{id}-{}", key.pane.0))
                .child(
                    PopoverMenu::new(key.element_id(id))
                        .trigger_with_tooltip(
                            IconButton::new(key.element_id(&format!("{id}-button")), icon)
                                .icon_size(IconSize::Small),
                            Tooltip::text(title),
                        )
                        .anchor(gpui::Anchor::TopRight)
                        .menu(move |window, cx| {
                            let shell = this.clone();
                            let thread = this.clone();
                            let action = action.boxed_clone();
                            Some(ContextMenu::build(window, cx, move |menu, _, _| {
                                menu.entry("Shell", Some(action), move |window, cx| {
                                    shell
                                        .update(cx, |this, cx| {
                                            this.split(key, direction, window, cx)
                                        })
                                        .ok();
                                })
                                .entry(
                                    "New Thread…",
                                    None,
                                    move |window, cx| {
                                        thread
                                            .update(cx, |this, cx| {
                                                this.split_with_thread(key, direction, window, cx)
                                            })
                                            .ok();
                                    },
                                )
                            }))
                        }),
                )
        };
        let split_right = split_menu(Direction::Horizontal, cx);
        let split_down = split_menu(Direction::Vertical, cx);
        let thread_buttons = match &view {
            Some(PaneView::Agent(view)) => Some(view.update(cx, |view, cx| {
                view.render_toolbar_buttons(cx).into_any_element()
            })),
            _ => None,
        };
        let header_group =
            SharedString::from(format!("pane-header-{}-{}", key.machine.slug(), key.pane.0));
        let header = h_flex()
            .id(key.element_id("pane-header"))
            .debug_selector(|| format!("pane-header-{}", key.pane.0))
            .group(header_group.clone())
            .h(TOOLBAR_HEIGHT)
            .flex_none()
            .px_2()
            .gap_1p5()
            .border_b_1()
            .border_color(colors.border_variant)
            .bg(if is_focused {
                colors.tab_active_background
            } else {
                colors.tab_inactive_background
            })
            .child(icon.size(IconSize::Small).color(if is_focused {
                Color::Default
            } else {
                Color::Muted
            }))
            // The title fits first; the detail gives way.
            .child(
                div().flex_none().max_w(relative(0.6)).child(
                    Label::new(title.clone())
                        .size(LabelSize::Small)
                        .color(if is_focused {
                            Color::Default
                        } else {
                            Color::Muted
                        })
                        .truncate(),
                ),
            )
            .children(detail.map(|detail| {
                div().min_w_0().child(
                    Label::new(detail)
                        .size(LabelSize::Small)
                        .color(Color::Muted)
                        .truncate(),
                )
            }))
            .children(status.map(|status| render_status_dot(status, cx)))
            .child(div().flex_1())
            .children(thread_buttons)
            .child(
                // An unfocused pane's own buttons wait for the pointer.
                h_flex()
                    .flex_none()
                    .gap_1p5()
                    .when(!is_focused, |buttons| {
                        buttons.visible_on_hover(header_group.clone())
                    })
                    .child(split_right)
                    .child(split_down)
                    .when(shows_focus || is_zoomed, |buttons| {
                        buttons.child(
                            IconButton::new(key.element_id("pane-zoom"), IconName::Maximize)
                                .icon_size(IconSize::Small)
                                .toggle_state(is_zoomed)
                                .selected_icon(IconName::Minimize)
                                .tooltip(move |_, cx| {
                                    Tooltip::for_action(
                                        if is_zoomed { "Zoom Out" } else { "Zoom In" },
                                        &ToggleZoom,
                                        cx,
                                    )
                                })
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    this.toggle_zoom_of(key, window, cx)
                                })),
                        )
                    })
                    .child(
                        IconButton::new(key.element_id("pane-close"), IconName::Close)
                            .icon_size(IconSize::Small)
                            .tooltip(|_, cx| Tooltip::for_action("Close Pane", &ClosePane, cx))
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.close_pane(key, window, cx)
                            })),
                    ),
            )
            .on_drag(
                DraggedLabel {
                    item: DraggedPane {
                        tab: tab_key,
                        pane: pane.id,
                    },
                    label: title,
                },
                |dragged, _, _, cx| cx.new(|_| dragged.clone()),
            );
        let menu = self.pane_menu(key, pane, is_zoomed, cx);
        let header = right_click_menu(key.element_id("pane-menu"))
            .trigger(move |_, _, _| header)
            .menu(menu);

        let content = match view {
            Some(PaneView::Terminal(view)) => div()
                .size_full()
                .pt_1()
                .bg(colors.terminal_background)
                .child(view)
                .into_any_element(),
            Some(PaneView::Agent(view)) => view.into_any_element(),
            None => v_flex()
                .size_full()
                .items_center()
                .justify_center()
                .child(
                    Label::new(match pane.content {
                        PaneContent::Unknown(_) => "This pane needs a newer agentZ",
                        PaneContent::Terminal(_) | PaneContent::Thread(_) => "Loading…",
                    })
                    .color(Color::Muted),
                )
                .into_any_element(),
        };

        v_flex()
            .id(key.element_id("pane"))
            .debug_selector(|| format!("pane-{}", key.pane.0))
            .relative()
            .size_full()
            .when(shows_focus, |pane| {
                pane.border_1().border_color(if is_focused {
                    colors.text_accent
                } else {
                    gpui::transparent_black()
                })
            })
            .bg(colors.editor_background)
            .capture_any_mouse_down(
                cx.listener(move |this, _, window, cx| this.focus_pane(key, window, cx)),
            )
            // Every pane hears every move, so each one only answers while the pointer is
            // over it, and lets go when it leaves.
            .on_drag_move(cx.listener(
                move |this, event: &DragMoveEvent<DraggedLabel<DraggedPane>>, _, cx| {
                    let dragged = event.drag(cx).item;
                    let pane_drop = if event.bounds.contains(&event.event.position) {
                        (dragged.tab == tab_key && dragged.pane != key.pane).then(|| PaneDrop {
                            pane: key,
                            edge: pane_drop_edge(event.bounds, event.event.position),
                        })
                    } else if this
                        .pane_drop
                        .is_some_and(|pane_drop| pane_drop.pane == key)
                    {
                        None
                    } else {
                        return;
                    };
                    if this.pane_drop != pane_drop {
                        this.pane_drop = pane_drop;
                        cx.notify();
                    }
                },
            ))
            .child(header)
            .child(div().flex_1().min_h_0().child(content))
            .children(
                self.pane_drop
                    .filter(|pane_drop| pane_drop.pane == key && cx.has_active_drag())
                    .map(|pane_drop| {
                        // Where the pane would go, as Zed shows it: the half it would split
                        // off, or all of this pane for a swap. It takes the drop, as Zed's
                        // does, so the content under it can't keep it from the pane.
                        let half = relative(0.5);
                        div()
                            .debug_selector(|| format!("pane-drop-{}", key.pane.0))
                            .absolute()
                            .bg(colors.drop_target_background)
                            .on_drop(cx.listener(
                                move |this, dragged: &DraggedLabel<DraggedPane>, _, cx| {
                                    let edge = this
                                        .pane_drop
                                        .take()
                                        .filter(|pane_drop| pane_drop.pane == key)
                                        .and_then(|pane_drop| pane_drop.edge);
                                    cx.notify();
                                    if dragged.item.tab != tab_key || dragged.item.pane == key.pane
                                    {
                                        return;
                                    }
                                    let request = match edge {
                                        Some(edge) => SpaceRequest::MovePane {
                                            pane: dragged.item.pane,
                                            target: key.pane,
                                            edge,
                                        },
                                        None => {
                                            SpaceRequest::SwapPanes(dragged.item.pane, key.pane)
                                        }
                                    };
                                    this.send(key.machine, request, cx);
                                },
                            ))
                            .map(|overlay| match pane_drop.edge {
                                None => overlay.inset_0(),
                                Some(NavDirection::Up) => {
                                    overlay.top_0().left_0().right_0().h(half)
                                }
                                Some(NavDirection::Down) => {
                                    overlay.bottom_0().left_0().right_0().h(half)
                                }
                                Some(NavDirection::Left) => {
                                    overlay.top_0().bottom_0().left_0().w(half)
                                }
                                Some(NavDirection::Right) => {
                                    overlay.top_0().bottom_0().right_0().w(half)
                                }
                            })
                    }),
            )
            .into_any_element()
    }

    /// The pane's right-click menu. Its actions are for this pane, not the focused one.
    fn pane_menu(
        &self,
        key: PaneKey,
        pane: &Pane,
        is_zoomed: bool,
        cx: &mut Context<Self>,
    ) -> impl Fn(&mut Window, &mut App) -> Entity<ContextMenu> + 'static {
        let this = cx.entity().downgrade();
        let is_shell =
            matches!(&pane.content, PaneContent::Terminal(terminal) if terminal.command.is_none());
        let shown_thread = match &pane.content {
            PaneContent::Thread(thread) => Some(*thread),
            PaneContent::Terminal(_) | PaneContent::Unknown(_) => None,
        };
        move |window, cx| {
            let this = this.clone();
            let machines = Machines::global(cx).read(cx);
            // Threads started in panes too, so a closed pane's thread can be shown again.
            let mut threads = machines.active_threads(cx);
            threads.extend(machines.workspaces_threads(cx));
            threads.retain(|(machine, thread)| *machine == key.machine && thread.task.is_none());
            threads.sort_by_key(|(_, thread)| {
                std::cmp::Reverse(thread.last_activity_at.or(thread.created_at))
            });
            let threads: Vec<(ThreadId, SharedString)> = threads
                .into_iter()
                .take(20)
                .map(|(_, thread)| (thread.id, thread.title.into()))
                .collect();
            ContextMenu::build(window, cx, move |menu, _, _| {
                let on = |action: fn(
                    &mut SpacesView,
                    PaneKey,
                    &mut Window,
                    &mut Context<SpacesView>,
                )| {
                    let this = this.clone();
                    move |window: &mut Window, cx: &mut App| {
                        this.update(cx, |this, cx| action(this, key, window, cx))
                            .ok();
                    }
                };
                let threads = threads.clone();
                let thread_this = this.clone();
                menu.entry(
                    "Split Right",
                    Some(Box::new(SplitRight)),
                    on(|this, key, window, cx| this.split(key, Direction::Horizontal, window, cx)),
                )
                .entry(
                    "Split Down",
                    Some(Box::new(SplitDown)),
                    on(|this, key, window, cx| this.split(key, Direction::Vertical, window, cx)),
                )
                .entry(
                    if is_zoomed { "Zoom Out" } else { "Zoom In" },
                    Some(Box::new(ToggleZoom)),
                    on(|this, key, window, cx| this.toggle_zoom_of(key, window, cx)),
                )
                .separator()
                .when(!is_shell, |menu| {
                    menu.entry(
                        "Show a Shell",
                        None,
                        on(|this, key, _, cx| {
                            this.send(
                                key.machine,
                                SpaceRequest::SetPaneContent {
                                    pane: key.pane,
                                    content: new_shell(),
                                },
                                cx,
                            )
                        }),
                    )
                })
                .entry(
                    "New Thread…",
                    None,
                    on(|this, key, _, cx| this.new_thread_in_pane(key, cx)),
                )
                .when(!threads.is_empty(), |menu| {
                    menu.submenu("Show Thread", move |mut menu, _, _| {
                        for (thread_id, title) in &threads {
                            let thread_id = *thread_id;
                            let this = thread_this.clone();
                            menu = menu.toggleable_entry(
                                title.clone(),
                                shown_thread == Some(thread_id),
                                IconPosition::Start,
                                None,
                                move |_, cx| {
                                    this.update(cx, |this, cx| {
                                        this.show_thread_in_pane(key, thread_id, cx)
                                    })
                                    .ok();
                                },
                            );
                        }
                        menu
                    })
                })
                .when_some(shown_thread, |menu, thread| {
                    let this = this.clone();
                    menu.entry("Open in Agents", None, move |_, cx| {
                        this.update(cx, |_, cx| {
                            cx.emit(SpacesViewEvent::OpenThread(ThreadKey {
                                machine: key.machine,
                                thread,
                            }))
                        })
                        .ok();
                    })
                })
                .separator()
                .entry(
                    "Close Pane",
                    Some(Box::new(ClosePane)),
                    on(|this, key, window, cx| this.close_pane(key, window, cx)),
                )
            })
        }
    }

    /// New Thread… in a pane: a Workspaces thread, working where the pane is.
    fn new_thread_in_pane(&mut self, pane: PaneKey, cx: &mut Context<Self>) {
        if let Some(folder) = self.pane_folder(pane, cx) {
            cx.emit(SpacesViewEvent::NewThreadInPane { pane, folder });
        }
    }

    /// Where the pane is: its shell's current folder, or its thread's, or else the workspace's.
    fn pane_folder(&self, pane: PaneKey, cx: &App) -> Option<PathBuf> {
        let (space, _, pane_state) = self.find_pane(pane, cx)?;
        let content_folder = || match &pane_state.content {
            PaneContent::Terminal(terminal) => {
                Some(terminal.folder.clone()).filter(|folder| !folder.as_os_str().is_empty())
            }
            PaneContent::Thread(thread) => self
                .machines
                .read(cx)
                .projects(pane.machine, cx)?
                .read(cx)
                .thread_folder(*thread),
            PaneContent::Unknown(_) => None,
        };
        Some(
            pane_state
                .folder
                .as_ref()
                .map(|folder| folder.path.clone())
                .or_else(content_folder)
                .unwrap_or_else(|| space.current_folder().to_path_buf()),
        )
    }
}

impl Render for SpacesView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let main_background = cx.theme().colors().editor_background;
        let main = self.render_main(cx);
        h_flex()
            .key_context(KEY_CONTEXT)
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(Self::new_workspace))
            .on_action(cx.listener(Self::new_tab))
            .on_action(cx.listener(Self::next_tab))
            .on_action(cx.listener(Self::activate_tab_at))
            .on_action(cx.listener(Self::previous_tab))
            .on_action(cx.listener(Self::split_right))
            .on_action(cx.listener(Self::split_down))
            .on_action(cx.listener(Self::close_focused_pane))
            .on_action(cx.listener(Self::toggle_zoom))
            .on_action(cx.listener(|this, _: &ActivatePaneLeft, window, cx| {
                this.activate_pane_in(NavDirection::Left, window, cx)
            }))
            .on_action(cx.listener(|this, _: &ActivatePaneRight, window, cx| {
                this.activate_pane_in(NavDirection::Right, window, cx)
            }))
            .on_action(cx.listener(|this, _: &ActivatePaneUp, window, cx| {
                this.activate_pane_in(NavDirection::Up, window, cx)
            }))
            .on_action(cx.listener(|this, _: &ActivatePaneDown, window, cx| {
                this.activate_pane_in(NavDirection::Down, window, cx)
            }))
            .id("workspaces")
            .size_full()
            .on_drop(cx.listener(|this, dragged: &DraggedSplit, _, cx| {
                if let Some(split) = this
                    .split_override
                    .clone()
                    .filter(|split| split.tab == dragged.tab && split.path == dragged.path)
                {
                    this.send(
                        split.tab.machine,
                        SpaceRequest::SetSplitRatio {
                            tab: split.tab.tab,
                            path: split.path,
                            ratio: split.ratio,
                        },
                        cx,
                    );
                }
            }))
            .when(!crate::app_settings::is_sidebar_hidden(cx), |view| {
                view.child(self.render_sidebar(window, cx))
            })
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .bg(main_background)
                    .child(main),
            )
    }
}

/// A split's border: a line in the middle of a strip that drags.
fn render_divider(drag: DraggedSplit, cx: &App) -> AnyElement {
    let is_horizontal = drag.direction == Direction::Horizontal;
    let path_label: String = drag
        .path
        .iter()
        .map(|side| if *side { '1' } else { '0' })
        .collect();
    div()
        .id(ElementId::Name(
            format!(
                "divider-{}-{}-{path_label}",
                drag.tab.machine.slug(),
                drag.tab.tab.0
            )
            .into(),
        ))
        .flex_none()
        .flex()
        .justify_center()
        .bg(cx.theme().colors().editor_background)
        .map(|divider| {
            if is_horizontal {
                divider
                    .flex_row()
                    .w(DIVIDER_SIZE)
                    .h_full()
                    .cursor_col_resize()
                    .child(div().w_px().h_full().bg(cx.theme().colors().border))
            } else {
                divider
                    .flex_col()
                    .h(DIVIDER_SIZE)
                    .w_full()
                    .cursor_row_resize()
                    .child(div().h_px().w_full().bg(cx.theme().colors().border))
            }
        })
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_drag(drag, |drag, _, _, cx| cx.new(|_| drag.clone()))
        .into_any_element()
}

/// The status dot, or a space of its size so names line up.
fn render_state_slot(status: Option<ThreadStatus>, cx: &App) -> AnyElement {
    match status {
        Some(status) => render_status_dot(status, cx).into_any_element(),
        None => div().flex_none().size_1p5().into_any_element(),
    }
}

/// herdr's rolled-up state: blocked first (on a permission, then on input), then finished but
/// unseen, then working.
fn rolled_up(statuses: impl Iterator<Item = ThreadStatus>) -> Option<ThreadStatus> {
    statuses.max_by_key(|status| match status {
        ThreadStatus::PendingApproval => 4,
        ThreadStatus::AwaitingInput => 3,
        ThreadStatus::Completed => 2,
        ThreadStatus::Working => 1,
    })
}

/// The name a rename sets, `None` inside for the automatic one again. A name left as it
/// started changes nothing, so an automatic name keeps following its folder.
fn renamed_to(original: &str, text: &str) -> Option<Option<String>> {
    let name = text.trim();
    if name == original.trim() {
        return None;
    }
    Some((!name.is_empty()).then(|| name.to_string()))
}

/// "2 terminals · 1 agent", leaving out what's zero.
fn contents_label(terminals: usize, agents: usize) -> Option<String> {
    let plural =
        |count: usize, word: &str| format!("{count} {word}{}", if count == 1 { "" } else { "s" });
    match (terminals, agents) {
        (0, 0) => None,
        (terminals, 0) => Some(plural(terminals, "terminal")),
        (0, agents) => Some(plural(agents, "agent")),
        (terminals, agents) => Some(format!(
            "{} · {}",
            plural(terminals, "terminal"),
            plural(agents, "agent")
        )),
    }
}

/// What a terminal pane runs in front of its shell, named as its header names it: a server,
/// say, or an agent CLI, even one waiting at its prompt. An idle shell has nothing to lose,
/// and a thread's pane leaves the thread in Agents.
fn running_program(pane: &Pane) -> Option<String> {
    let PaneContent::Terminal(terminal) = &pane.content else {
        return None;
    };
    let program = pane.program.as_ref()?;
    Some(
        pane.agent
            .as_ref()
            .map(|agent| agent.name.clone())
            .or_else(|| terminal.command.clone())
            .unwrap_or_else(|| program.clone()),
    )
}

/// Each program running in the panes, once.
fn running_programs(panes: &[Pane]) -> Vec<String> {
    let mut programs: Vec<String> = Vec::new();
    for program in panes.iter().filter_map(running_program) {
        if !programs.contains(&program) {
            programs.push(program);
        }
    }
    programs
}

/// "Claude Code and npm are still running in it, and closing it ends them."
fn still_running(programs: &[String], place: &str) -> String {
    let names = match programs {
        [] => String::new(),
        [only] => only.clone(),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
    };
    let (verb, object) = if programs.len() == 1 {
        ("is", "it")
    } else {
        ("are", "them")
    };
    format!("{names} {verb} still running in {place}, and closing {place} ends {object}.")
}

/// A tab's name, or its number, as herdr numbers unnamed tabs.
/// A worktree group: the machine, and its repository's main checkout.
type GroupKey = (MachineId, PathBuf);

/// Where a workspace sits in herdr's worktree groups.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum GroupRole {
    /// On the repository's main checkout.
    Parent,
    /// In a linked worktree, or a project's pasture.
    Child,
}

/// A row of the workspace list.
#[derive(Clone)]
struct SpaceEntry {
    machine: MachineId,
    space: Space,
    /// Its place among its machine's workspaces, for reordering.
    index: usize,
    /// Under its group's parent, and whether it's the group's last child.
    child: Option<bool>,
    /// The group a parent's toggle folds.
    group: Option<GroupKey>,
}

/// herdr's `workspace_entries`: a workspace in a linked worktree of a repository sits under
/// the workspace on that repository's main checkout, however it was opened. A group shows
/// once it has both; otherwise rows keep their order. A folded group shows only its active
/// child.
fn space_entries(
    spaces: Vec<(MachineId, Space)>,
    membership: impl Fn(MachineId, &Space) -> Option<(PathBuf, GroupRole)>,
    collapsed: &HashSet<GroupKey>,
    active: Option<SpaceKey>,
) -> Vec<SpaceEntry> {
    let mut index_in_machine: HashMap<MachineId, usize> = HashMap::default();
    let members: Vec<(SpaceEntry, Option<(GroupKey, GroupRole)>)> = spaces
        .into_iter()
        .map(|(machine, space)| {
            let index = index_in_machine.entry(machine).or_default();
            let entry = SpaceEntry {
                machine,
                index: *index,
                child: None,
                group: None,
                space,
            };
            *index += 1;
            let member =
                membership(machine, &entry.space).map(|(main, role)| ((machine, main), role));
            (entry, member)
        })
        .collect();
    let has = |key: &GroupKey, role: GroupRole| {
        members
            .iter()
            .any(|(_, member)| member.as_ref() == Some(&(key.clone(), role)))
    };
    let mut emitted = HashSet::default();
    let mut entries = Vec::new();
    for (entry, member) in &members {
        let Some((key, _)) = member
            .as_ref()
            .filter(|(key, _)| has(key, GroupRole::Parent) && has(key, GroupRole::Child))
        else {
            entries.push(entry.clone());
            continue;
        };
        if !emitted.insert(key.clone()) {
            continue;
        }
        let in_group = |role: GroupRole| {
            members
                .iter()
                .filter(move |(_, member)| member.as_ref() == Some(&(key.clone(), role)))
                .map(|(entry, _)| entry)
        };
        for parent in in_group(GroupRole::Parent) {
            entries.push(SpaceEntry {
                group: Some(key.clone()),
                ..parent.clone()
            });
        }
        let children: Vec<&SpaceEntry> = in_group(GroupRole::Child)
            .filter(|child| {
                !collapsed.contains(key)
                    || active
                        == Some(SpaceKey {
                            machine: child.machine,
                            space: child.space.id,
                        })
            })
            .collect();
        let count = children.len();
        for (position, child) in children.into_iter().enumerate() {
            entries.push(SpaceEntry {
                child: Some(position + 1 == count),
                ..child.clone()
            });
        }
    }
    entries
}

/// A worktree's row is named after its branch, without agentZ's `agentz/` prefix (herdr
/// drops its `worktree/`), unless the user named it.
fn child_label(space: &Space) -> Option<String> {
    if space.name.is_some() {
        return None;
    }
    let branch = space.git.as_ref()?.branch.as_deref()?;
    Some(branch.strip_prefix("agentz/").unwrap_or(branch).to_string())
}

/// The project's checkout a folder is in: its own folder, or one of its worktrees or pastures.
fn checkout_root(project: &projects::Project, folder: &Path) -> Option<PathBuf> {
    std::iter::once(&project.path)
        .chain(project.workspaces.iter().map(|workspace| &workspace.path))
        .filter(|root| folder.starts_with(root))
        .max_by_key(|root| root.components().count())
        .cloned()
}

/// A terminal pane's icon: its agent CLI's, as the ACP Registry draws that agent, or a terminal.
fn pane_agent_icon(pane: &Pane, cx: &App) -> Icon {
    pane.agent
        .as_ref()
        .and_then(|agent| agent.registry_agent.clone())
        .and_then(|agent| agent_icon(&AgentId::new(agent), cx))
        .map(Icon::from_svg_markup)
        .unwrap_or_else(|| Icon::new(IconName::Terminal))
}

/// Where a pane is: within its workspace's folder as "storefront/src", elsewhere as its
/// path.
fn pane_folder_label(space: &Space, folder: &SpaceFolder) -> String {
    let root = space.current_folder();
    let is_home = space
        .current
        .as_ref()
        .is_some_and(|current| current.display_path == "~");
    if !is_home && let (Ok(rest), Some(name)) = (folder.path.strip_prefix(root), root.file_name()) {
        let name = name.to_string_lossy();
        if rest.as_os_str().is_empty() {
            return name.into_owned();
        }
        return format!("{name}/{}", rest.display());
    }
    folder.display_path.clone()
}

/// A name the user gave, or else the tab's position, so moving it renumbers it.
fn tab_label(tab: &Tab, index: usize) -> String {
    tab.name
        .clone()
        .unwrap_or_else(|| format!("Tab {}", index + 1))
}

fn matches_space(space: &Space, query: &str) -> bool {
    query.is_empty()
        || space.label().to_lowercase().contains(query)
        || space
            .folder
            .to_string_lossy()
            .to_lowercase()
            .contains(query)
        || space
            .git
            .as_ref()
            .and_then(|git| git.branch.as_ref())
            .is_some_and(|branch| branch.to_lowercase().contains(query))
}

/// A shell in the workspace's folder, which the server fills in.
fn new_shell() -> PaneContent {
    PaneContent::Terminal(PaneTerminal {
        folder: PathBuf::new(),
        command: None,
    })
}

#[cfg(test)]
mod tests {
    use agentz_protocol::spaces::{PaneAgent, PaneAgentState, SpacesSnapshot};
    use gpui::TestAppContext;

    use super::*;
    use crate::server_client::ServerClient;

    fn pane(id: u64, agent: Option<PaneAgent>) -> Pane {
        Pane {
            agent,
            ..Pane::new(PaneId(id), PaneContent::Unknown(serde_json::Value::Null))
        }
    }

    /// A tab of three panes: a quarter on the left, and the rest split in half, top and
    /// bottom. The bottom pane runs an agent.
    fn spaces() -> SpacesSnapshot {
        let root = Node::Split {
            direction: Direction::Horizontal,
            ratio: 0.25,
            first: Box::new(Node::Pane(PaneId(3))),
            second: Box::new(Node::Split {
                direction: Direction::Vertical,
                ratio: 0.5,
                first: Box::new(Node::Pane(PaneId(4))),
                second: Box::new(Node::Pane(PaneId(5))),
            }),
        };
        let mut agent_pane = pane(
            5,
            Some(PaneAgent {
                registry_agent: None,
                name: "Claude Code".to_string(),
                state: PaneAgentState::Working,
            }),
        );
        agent_pane.content = PaneContent::Terminal(PaneTerminal {
            folder: PathBuf::from("/tmp/demo"),
            command: Some("claude".to_string()),
        });
        SpacesSnapshot {
            spaces: vec![Space {
                id: SpaceId(1),
                name: None,
                folder: PathBuf::from("/tmp/demo"),
                project_id: None,
                tabs: vec![Tab {
                    id: TabId(2),
                    name: None,
                    root,
                    panes: vec![pane(3, None), pane(4, None), agent_pane],
                }],
                git: None,
                current: None,
            }],
        }
    }

    fn assert_close(actual: f32, expected: f32) {
        assert!(
            (actual - expected).abs() < 0.02,
            "expected about {expected}, got {actual}"
        );
    }

    #[gpui::test]
    fn panes_are_laid_out_as_their_tree_says(cx: &mut TestAppContext) {
        cx.update(|cx| {
            crate::init_for_test(cx);
            let client =
                ServerClient::new_for_test(MachineId::Local, "This Mac".into(), spaces(), cx);
            crate::machines::init_for_test(vec![client], cx);
            crate::project_info::init(cx);
        });
        let (view, cx) = cx.add_window_view(|window, cx| SpacesView::new(window, cx));
        view.update_in(cx, |view, window, cx| view.set_visible(true, window, cx));
        cx.run_until_parked();

        let bounds = |cx: &mut gpui::VisualTestContext, name: &'static str| cx.debug_bounds(name);
        let left = bounds(cx, "pane-3").expect("the left pane is drawn");
        let top = bounds(cx, "pane-4").expect("the top pane is drawn");
        let bottom = bounds(cx, "pane-5").expect("the bottom pane is drawn");
        let width = f32::from(top.right() - left.left());
        assert_close(f32::from(left.size.width) / width, 0.25);
        assert!(top.left() > left.right());
        assert_eq!(top.left(), bottom.left());
        assert_eq!(top.size.width, bottom.size.width);
        assert!(bottom.top() > top.bottom());
        let height = f32::from(bottom.bottom() - top.top());
        assert_close(f32::from(top.size.height) / height, 0.5);
        assert_eq!(left.top(), top.top());
        assert_eq!(left.bottom(), bottom.bottom());

        // The agent in the bottom pane is listed, and clicking it focuses its pane.
        let row = bounds(cx, "agent-row-0").expect("the agent is listed");
        assert!(bounds(cx, "agent-row-1").is_none());
        cx.simulate_click(row.center(), gpui::Modifiers::none());
        let focused = view.read_with(cx, |view, cx| view.focused_pane(cx));
        assert_eq!(
            focused,
            Some(PaneKey {
                machine: MachineId::Local,
                pane: PaneId(5),
            })
        );

        // Zoomed, the focused pane fills the tab alone. The keys reach the view past the
        // focused terminal.
        cx.simulate_keystrokes("cmd-shift-enter");
        cx.run_until_parked();
        assert!(bounds(cx, "pane-3").is_none());
        assert!(bounds(cx, "pane-4").is_none());
        let zoomed = bounds(cx, "pane-5").expect("the zoomed pane is drawn");
        assert_eq!(zoomed.left(), left.left());
        assert_eq!(zoomed.top(), left.top());
        assert_eq!(zoomed.right(), top.right());
        assert_eq!(zoomed.bottom(), bottom.bottom());

        // Back from zoom, the left pane is to the left of the focused one.
        cx.simulate_keystrokes("cmd-shift-enter cmd-alt-left");
        cx.run_until_parked();
        let focused = view.read_with(cx, |view, cx| view.focused_pane(cx));
        assert_eq!(focused.map(|pane| pane.pane), Some(PaneId(3)));
    }

    #[test]
    fn a_pane_drop_is_on_the_nearest_edge_within_reach_or_in_the_middle() {
        let bounds = Bounds::new(gpui::point(px(10.), px(10.)), gpui::size(px(100.), px(50.)));
        let at = |x: f32, y: f32| pane_drop_edge(bounds, gpui::point(px(x), px(y)));
        // The edges reach a fifth of the shorter side, 10px.
        assert_eq!(at(15., 35.), Some(NavDirection::Left));
        assert_eq!(at(105., 30.), Some(NavDirection::Right));
        assert_eq!(at(60., 12.), Some(NavDirection::Up));
        assert_eq!(at(60., 55.), Some(NavDirection::Down));
        assert_eq!(at(21., 35.), None);
        assert_eq!(at(60., 35.), None);
        // In a corner, the nearer edge wins.
        assert_eq!(at(12., 15.), Some(NavDirection::Left));
        assert_eq!(at(15., 12.), Some(NavDirection::Up));
    }

    #[gpui::test]
    fn dropping_a_pane_on_an_edge_splits_and_in_the_middle_swaps(cx: &mut TestAppContext) {
        let client = cx.update(|cx| {
            crate::init_for_test(cx);
            let client =
                ServerClient::new_for_test(MachineId::Local, "This Mac".into(), spaces(), cx);
            crate::machines::init_for_test(vec![client.clone()], cx);
            crate::project_info::init(cx);
            client
        });
        let (view, cx) = cx.add_window_view(|window, cx| SpacesView::new(window, cx));
        view.update_in(cx, |view, window, cx| view.set_visible(true, window, cx));
        cx.run_until_parked();
        let sent = |cx: &mut gpui::VisualTestContext| -> Vec<Request> {
            client.read_with(cx, |client, _| {
                client
                    .sent_for_test()
                    .into_iter()
                    .filter(|request| matches!(request, Request::Spaces(_)))
                    .collect()
            })
        };
        let none = gpui::Modifiers::none();
        let header = cx
            .debug_bounds("pane-header-4")
            .expect("the top pane has a header");
        let left = cx.debug_bounds("pane-3").expect("the left pane is drawn");
        let bottom = cx.debug_bounds("pane-5").expect("the bottom pane is drawn");
        let start_drag = |cx: &mut gpui::VisualTestContext| {
            cx.simulate_mouse_down(header.center(), MouseButton::Left, none);
            cx.simulate_mouse_move(
                header.center() + gpui::point(px(20.), px(0.)),
                MouseButton::Left,
                none,
            );
        };

        // Near the left pane's left edge, its left half lights up, and dropping puts the
        // top pane there.
        start_drag(cx);
        let edge = gpui::point(left.left() + px(5.), left.center().y);
        cx.simulate_mouse_move(edge, MouseButton::Left, none);
        let overlay = cx
            .debug_bounds("pane-drop-3")
            .expect("the left half lights up");
        assert!(f32::from(overlay.left() - left.left()).abs() <= 1.);
        assert!(f32::from(overlay.top() - left.top()).abs() <= 1.);
        assert_close(overlay.size.width / left.size.width, 0.5);
        assert_close(overlay.size.height / left.size.height, 1.);
        assert!(cx.debug_bounds("pane-drop-4").is_none());
        cx.simulate_mouse_up(edge, MouseButton::Left, none);
        cx.run_until_parked();
        assert!(cx.debug_bounds("pane-drop-3").is_none());
        let move_pane = Request::Spaces(SpaceRequest::MovePane {
            pane: PaneId(4),
            target: PaneId(3),
            edge: NavDirection::Left,
        });
        assert_eq!(sent(cx), std::slice::from_ref(&move_pane));

        // Over the dragged pane itself, nothing lights up.
        start_drag(cx);
        cx.simulate_mouse_move(header.center(), MouseButton::Left, none);
        assert!(cx.debug_bounds("pane-drop-4").is_none());

        // In the middle of the bottom pane, all of it lights up, and dropping swaps.
        cx.simulate_mouse_move(bottom.center(), MouseButton::Left, none);
        let overlay = cx
            .debug_bounds("pane-drop-5")
            .expect("the whole pane lights up");
        assert_close(overlay.size.width / bottom.size.width, 1.);
        assert_close(overlay.size.height / bottom.size.height, 1.);
        cx.simulate_mouse_up(bottom.center(), MouseButton::Left, none);
        cx.run_until_parked();
        assert_eq!(
            sent(cx),
            [
                move_pane,
                Request::Spaces(SpaceRequest::SwapPanes(PaneId(4), PaneId(5)))
            ]
        );
    }

    #[gpui::test]
    fn each_split_button_splits_its_own_way(cx: &mut TestAppContext) {
        let requests = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
        cx.update(|cx| {
            crate::init_for_test(cx);
            let client =
                ServerClient::new_for_test(MachineId::Local, "This Mac".into(), spaces(), cx);
            let requests = requests.clone();
            client.update(cx, |client, _| {
                client.answer_for_test(move |request| {
                    requests.borrow_mut().push(request.clone());
                    None
                })
            });
            crate::machines::init_for_test(vec![client], cx);
            crate::project_info::init(cx);
        });
        let (view, cx) = cx.add_window_view(|window, cx| SpacesView::new(window, cx));
        view.update_in(cx, |view, window, cx| view.set_visible(true, window, cx));
        cx.run_until_parked();
        let focused = view.read_with(cx, |view, cx| view.focused_pane(cx));
        assert_eq!(focused.map(|pane| pane.pane), Some(PaneId(3)));
        // A focused pane's buttons show without the pointer over it.
        view.update_in(cx, |view, window, cx| view.focus_active(window, cx));
        cx.run_until_parked();
        let spaces_requests = || {
            requests
                .borrow_mut()
                .drain(..)
                .filter(|request| matches!(request, Request::Spaces(_)))
                .collect::<Vec<_>>()
        };

        for (button, direction) in [
            ("pane-split-right-3", Direction::Horizontal),
            ("pane-split-down-3", Direction::Vertical),
        ] {
            let bounds = cx
                .debug_bounds(button)
                .expect("the focused pane has the button");
            cx.simulate_click(bounds.center(), gpui::Modifiers::none());
            cx.run_until_parked();
            // A popover menu takes focus two frames after it opens.
            for _ in 0..2 {
                cx.update(|window, cx| window.simulate_next_frame(cx));
                cx.run_until_parked();
            }
            // The menu's first entry is Shell.
            cx.dispatch_action(menu::SelectFirst);
            cx.dispatch_action(menu::Confirm);
            cx.run_until_parked();
            assert_eq!(
                spaces_requests(),
                [Request::Spaces(SpaceRequest::SplitPane {
                    pane: PaneId(3),
                    direction,
                    content: new_shell(),
                })]
            );
        }
    }

    #[gpui::test]
    fn a_new_thread_in_a_pane_works_where_the_pane_is(cx: &mut TestAppContext) {
        // A shell that went to src, the agent in /tmp/demo, and an unknown pane.
        let mut state = spaces();
        state.spaces[0].folder = PathBuf::from("/tmp/workspace");
        let panes = &mut state.spaces[0].tabs[0].panes;
        panes[0].content = new_shell();
        panes[0].folder = Some(SpaceFolder {
            path: PathBuf::from("/tmp/workspace/src"),
            display_path: "/tmp/workspace/src".to_string(),
        });
        cx.update(|cx| {
            crate::init_for_test(cx);
            let client = ServerClient::new_for_test(MachineId::Local, "This Mac".into(), state, cx);
            crate::machines::init_for_test(vec![client], cx);
            crate::project_info::init(cx);
        });
        let (view, cx) = cx.add_window_view(|window, cx| SpacesView::new(window, cx));
        let folder = |pane: u64, cx: &mut gpui::VisualTestContext| {
            view.read_with(cx, |view, cx| {
                view.pane_folder(
                    PaneKey {
                        machine: MachineId::Local,
                        pane: PaneId(pane),
                    },
                    cx,
                )
            })
        };
        assert_eq!(folder(3, cx), Some(PathBuf::from("/tmp/workspace/src")));
        assert_eq!(folder(5, cx), Some(PathBuf::from("/tmp/demo")));
        assert_eq!(folder(4, cx), Some(PathBuf::from("/tmp/workspace")));
    }

    #[gpui::test]
    fn closing_asks_first_only_while_something_runs(cx: &mut TestAppContext) {
        // An idle shell, a server, and Claude Code.
        let mut state = spaces();
        let panes = &mut state.spaces[0].tabs[0].panes;
        panes[0].content = new_shell();
        panes[1].content = new_shell();
        panes[1].program = Some("npm".to_string());
        panes[2].program = Some("claude".to_string());
        let client = cx.update(|cx| {
            crate::init_for_test(cx);
            let client = ServerClient::new_for_test(MachineId::Local, "This Mac".into(), state, cx);
            crate::machines::init_for_test(vec![client.clone()], cx);
            crate::project_info::init(cx);
            client
        });
        let (view, cx) = cx.add_window_view(|window, cx| SpacesView::new(window, cx));
        view.update_in(cx, |view, window, cx| view.set_visible(true, window, cx));
        cx.run_until_parked();
        let sent = |cx: &mut gpui::VisualTestContext| -> Vec<Request> {
            client.read_with(cx, |client, _| {
                client
                    .sent_for_test()
                    .into_iter()
                    .filter(|request| matches!(request, Request::Spaces(_)))
                    .collect()
            })
        };
        let pane = |id: u64| PaneKey {
            machine: MachineId::Local,
            pane: PaneId(id),
        };
        let close_pane = |id: u64| Request::Spaces(SpaceRequest::ClosePane(PaneId(id)));

        view.update_in(cx, |view, window, cx| view.close_pane(pane(3), window, cx));
        cx.run_until_parked();
        assert!(!cx.has_pending_prompt());
        assert_eq!(sent(cx), [close_pane(3)]);

        // Cmd-W on Claude Code asks first, even while it only waits at its prompt.
        view.update_in(cx, |view, window, cx| view.focus_pane(pane(5), window, cx));
        cx.simulate_keystrokes("cmd-w");
        assert_eq!(
            cx.pending_prompt(),
            Some((
                "Close “Claude Code”?".to_string(),
                "It's still running, and closing the pane ends it.".to_string()
            ))
        );
        cx.simulate_prompt_answer("Cancel");
        cx.run_until_parked();
        assert_eq!(sent(cx), [close_pane(3)]);
        cx.simulate_keystrokes("cmd-w");
        cx.simulate_prompt_answer("Close");
        cx.run_until_parked();
        assert_eq!(sent(cx), [close_pane(3), close_pane(5)]);

        let tab = TabKey {
            machine: MachineId::Local,
            tab: TabId(2),
        };
        view.update_in(cx, |view, window, cx| view.close_tab(tab, window, cx));
        assert_eq!(
            cx.pending_prompt().map(|(_, detail)| detail),
            Some(
                "npm and Claude Code are still running in it, and closing it ends them."
                    .to_string()
            )
        );
        cx.simulate_prompt_answer("Close");
        cx.run_until_parked();
        assert_eq!(
            sent(cx).last(),
            Some(&Request::Spaces(SpaceRequest::CloseTab(TabId(2))))
        );

        let space = SpaceKey {
            machine: MachineId::Local,
            space: SpaceId(1),
        };
        view.update_in(cx, |view, window, cx| view.close_space(space, window, cx));
        assert_eq!(
            cx.pending_prompt(),
            Some((
                "Close “demo”?".to_string(),
                "npm and Claude Code are still running in it, and closing it ends them. \
                 Its threads stay in Agents."
                    .to_string()
            ))
        );
        cx.simulate_prompt_answer("Cancel");
        cx.run_until_parked();
        assert_eq!(sent(cx).len(), 3);
    }

    #[test]
    fn still_running_lists_every_program() {
        let programs =
            |names: &[&str]| -> Vec<String> { names.iter().map(|name| name.to_string()).collect() };
        assert_eq!(
            still_running(&programs(&["npm"]), "it"),
            "npm is still running in it, and closing it ends it."
        );
        assert_eq!(
            still_running(&programs(&["npm", "Codex", "Claude Code"]), "them"),
            "npm, Codex and Claude Code are still running in them, and closing them ends them."
        );
    }

    #[test]
    fn rolled_up_state_follows_herdr() {
        use ThreadStatus::*;
        assert_eq!(rolled_up([Working, Completed].into_iter()), Some(Completed));
        assert_eq!(
            rolled_up([Completed, PendingApproval, Working].into_iter()),
            Some(PendingApproval)
        );
        assert_eq!(rolled_up([Working].into_iter()), Some(Working));
        assert_eq!(rolled_up(std::iter::empty()), None);
    }

    #[gpui::test]
    fn a_workspace_shows_its_current_folder_and_details_on_hover(cx: &mut TestAppContext) {
        let mut state = spaces();
        for pane in state.spaces[0].tabs[0].panes.iter_mut().take(2) {
            pane.content = new_shell();
        }
        state.spaces[0].current = Some(SpaceFolder {
            path: PathBuf::from("/tmp/other"),
            display_path: "/tmp/other".to_string(),
        });
        cx.update(|cx| {
            crate::init_for_test(cx);
            let client = ServerClient::new_for_test(MachineId::Local, "This Mac".into(), state, cx);
            crate::machines::init_for_test(vec![client], cx);
            crate::project_info::init(cx);
        });
        let (view, cx) = cx.add_window_view(|window, cx| SpacesView::new(window, cx));
        view.update_in(cx, |view, window, cx| view.set_visible(true, window, cx));
        cx.run_until_parked();

        let key = SpaceKey {
            machine: MachineId::Local,
            space: SpaceId(1),
        };
        // Three panes: two terminals, and one running an agent.
        let (space, contents) = view.read_with(cx, |view, cx| {
            let space = view.space(key, cx).expect("the workspace");
            let contents = view.space_contents(MachineId::Local, &space, cx);
            (space, contents)
        });
        assert_eq!(space.label(), "other");
        assert_eq!(contents, (2, 1));

        let row = cx
            .debug_bounds("workspace-local-1")
            .expect("the workspace row is drawn");
        cx.simulate_mouse_move(row.center(), None, gpui::Modifiers::none());
        assert_eq!(view.read_with(cx, |view, _| view.details_space), None);
        cx.executor().advance_clock(DETAILS_DELAY * 2);
        cx.run_until_parked();
        assert_eq!(view.read_with(cx, |view, _| view.details_space), Some(key));

        // Over the counts, their tooltip shows instead.
        let counts = cx
            .debug_bounds("workspace-local-1-contents")
            .expect("the counts are drawn");
        cx.simulate_mouse_move(counts.center(), None, gpui::Modifiers::none());
        cx.executor().advance_clock(DETAILS_DELAY * 2);
        cx.run_until_parked();
        assert_eq!(view.read_with(cx, |view, _| view.details_space), None);
        cx.simulate_mouse_move(row.center(), None, gpui::Modifiers::none());
        cx.executor().advance_clock(DETAILS_DELAY * 2);
        cx.run_until_parked();
        assert_eq!(view.read_with(cx, |view, _| view.details_space), Some(key));

        cx.simulate_mouse_move(
            gpui::point(px(900.), px(600.)),
            None,
            gpui::Modifiers::none(),
        );
        cx.run_until_parked();
        assert_eq!(view.read_with(cx, |view, _| view.details_space), None);
    }

    #[test]
    fn a_rename_left_as_it_started_changes_nothing() {
        assert_eq!(renamed_to("projects", "projects"), None);
        assert_eq!(renamed_to("projects", " projects "), None);
        assert_eq!(
            renamed_to("projects", "Work"),
            Some(Some("Work".to_string()))
        );
        assert_eq!(renamed_to("Work", ""), Some(None));
    }

    #[test]
    fn contents_leave_out_what_is_zero() {
        assert_eq!(contents_label(0, 0), None);
        assert_eq!(contents_label(1, 0).as_deref(), Some("1 terminal"));
        assert_eq!(contents_label(0, 2).as_deref(), Some("2 agents"));
        assert_eq!(
            contents_label(3, 1).as_deref(),
            Some("3 terminals · 1 agent")
        );
    }

    #[test]
    fn worktrees_sit_under_their_main_checkout_as_herdr_groups_them() {
        let space = |id: u64, name: &str| Space {
            id: SpaceId(id),
            name: Some(name.to_string()),
            ..spaces().spaces.remove(0)
        };
        // The worktree comes before its parent and another repository sits between them.
        let spaces: Vec<(MachineId, Space)> = vec![
            (MachineId::Local, space(1, "feature")),
            (MachineId::Local, space(2, "other")),
            (MachineId::Local, space(3, "main")),
            (MachineId::Local, space(4, "lonely-worktree")),
        ];
        let membership = |_: MachineId, space: &Space| match space.id.0 {
            1 => Some((PathBuf::from("/repo"), GroupRole::Child)),
            3 => Some((PathBuf::from("/repo"), GroupRole::Parent)),
            // A worktree whose main checkout has no workspace stays where it is.
            4 => Some((PathBuf::from("/elsewhere"), GroupRole::Child)),
            _ => None,
        };
        let order = |entries: Vec<SpaceEntry>| {
            entries
                .into_iter()
                .map(|entry| (entry.space.id.0, entry.child, entry.group.is_some()))
                .collect::<Vec<_>>()
        };
        let group = (MachineId::Local, PathBuf::from("/repo"));
        assert_eq!(
            order(space_entries(
                spaces.clone(),
                membership,
                &HashSet::default(),
                None
            )),
            [
                (3, None, true),
                (1, Some(true), false),
                (2, None, false),
                (4, None, false)
            ]
        );
        // Folded, only the active worktree stays under its parent.
        let collapsed = HashSet::from_iter([group]);
        assert_eq!(
            order(space_entries(spaces.clone(), membership, &collapsed, None)),
            [(3, None, true), (2, None, false), (4, None, false)]
        );
        let active = SpaceKey {
            machine: MachineId::Local,
            space: SpaceId(1),
        };
        assert_eq!(
            order(space_entries(spaces, membership, &collapsed, Some(active))),
            [
                (3, None, true),
                (1, Some(true), false),
                (2, None, false),
                (4, None, false)
            ]
        );
    }

    #[test]
    fn panes_say_where_they_are_within_their_workspace() {
        let folder = |path: &str, display_path: &str| SpaceFolder {
            path: PathBuf::from(path),
            display_path: display_path.to_string(),
        };
        let mut space = spaces().spaces.remove(0);
        space.current = Some(folder("/Users/me/w/storefront", "~/w/storefront"));
        let label = |path, display| pane_folder_label(&space, &folder(path, display));
        assert_eq!(
            label("/Users/me/w/storefront", "~/w/storefront"),
            "storefront"
        );
        assert_eq!(
            label("/Users/me/w/storefront/src/app", "~/w/storefront/src/app"),
            "storefront/src/app"
        );
        assert_eq!(label("/Users/me/w/api", "~/w/api"), "~/w/api");
        // Everything is inside home, so a home workspace shows paths.
        space.current = Some(folder("/Users/me", "~"));
        let label = |path, display| pane_folder_label(&space, &folder(path, display));
        assert_eq!(label("/Users/me/docs", "~/docs"), "~/docs");
    }

    #[test]
    fn unnamed_tabs_are_numbered() {
        let tab = Tab {
            id: TabId(4),
            name: None,
            root: Node::Pane(PaneId(1)),
            panes: Vec::new(),
        };
        assert_eq!(tab_label(&tab, 1), "Tab 2");
        let named = Tab {
            name: Some("server".to_string()),
            ..tab
        };
        assert_eq!(tab_label(&named, 1), "server");
    }
}
