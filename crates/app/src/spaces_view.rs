//! The Workspaces view, herdr's: workspaces in a sidebar, each with tabs of split panes holding
//! terminals and threads. The server keeps the workspaces, their tabs and pane trees
//! (`agentz_protocol::spaces`); which tab shows, which pane has focus, and zoom are this
//! window's, as herdr keeps them per client. In code they're spaces, since a thread's
//! workspace is its checkout.

use std::path::PathBuf;

use agentz_protocol::agents::AgentId;
use agentz_protocol::layout::{
    Direction, NavDirection, Node, PaneId, Rect, TileLayout, find_in_direction,
};
use agentz_protocol::spaces::{
    Pane, PaneContent, PaneTerminal, Space, SpaceFolder, SpaceId, SpaceRequest, Tab, TabId,
};
use agentz_protocol::terminal::TerminalKey;
use agentz_protocol::{Request, Response};
use collections::{HashMap, HashSet};
use gpui::{
    AnyElement, App, ClickEvent, Context, DragMoveEvent, ElementId, Entity, EventEmitter,
    FocusHandle, Focusable, KeyBinding, MouseButton, PromptLevel, ScrollHandle, Subscription, Task,
    Window, actions, relative,
};
use projects::ThreadId;
use text_input::{TextInput, TextInputEvent};
use ui::{
    ContextMenu, ContextMenuEntry, PopoverMenu, PopoverMenuHandle, Tab as TabItem, TabBar,
    TabPosition, Tooltip, WithScrollbar as _, prelude::*, right_click_menu,
};

use crate::OpenSettings;
use crate::agent_view::{AgentView, AgentViewEvent, TOOLBAR_HEIGHT};
use crate::confirm_dialog::ConfirmRequest;
use crate::machines::{MachineId, Machines, ProjectKey, ThreadKey, project_at};
use crate::new_space_picker::{NewSpacePicker, SpaceChoice};
use crate::project_info::{ProjectInfoStore, render_project_icon};
use crate::project_store::ThreadStatus;
use crate::sidebar::{
    ARCHIVED_ROW_HEIGHT, DETAILS_DELAY, SIDEBAR_WIDTH, ThreadDetails, render_details_popover,
    render_folder_icon, render_footer_item, render_status_dot, render_status_pill,
    repository_branch, thread_agent_icon,
};
use crate::terminal_element::TerminalMode;
use crate::terminal_entity::Terminal;
use crate::terminal_view::TerminalView;
use crate::thread_entity::AgentThread;
use crate::worktree_modal::WorktreeModalMode;

const KEY_CONTEXT: &str = "Workspaces";
const RENAME_KEY_CONTEXT: &str = "WorkspacesRename";
const SEARCH_KEY_CONTEXT: &str = "WorkspacesSearch";
/// The grab area of a split's border. The line drawn in its middle is a pixel wide.
const DIVIDER_SIZE: Pixels = px(5.);
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
    /// New Thread, to be shown in the pane once it's made.
    NewThreadInPane {
        pane: PaneKey,
        project: Option<ProjectKey>,
        folder: Option<PathBuf>,
    },
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
    renaming: Option<RenameTarget>,
    /// The name the rename started from, so ending it unchanged keeps an automatic name.
    rename_original: SharedString,
    rename_input: Entity<TextInput>,
    _rename_blur: Option<Subscription>,
    /// The workspace whose details popover shows, and the one waiting to show it.
    details_space: Option<SpaceKey>,
    details_delay: Option<(SpaceKey, Task<()>)>,
    hovered_space: Option<SpaceKey>,
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
            renaming: None,
            rename_original: SharedString::default(),
            rename_input,
            _rename_blur: None,
            details_space: None,
            details_delay: None,
            hovered_space: None,
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

    fn create_space(&mut self, choice: SpaceChoice, window: &mut Window, cx: &mut Context<Self>) {
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

    fn close_pane(&mut self, pane: PaneKey, cx: &mut Context<Self>) {
        self.send(pane.machine, SpaceRequest::ClosePane(pane.pane), cx);
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

    fn close_space(&mut self, key: SpaceKey, window: &mut Window, cx: &mut Context<Self>) {
        let Some(space) = self.space(key, cx) else {
            return;
        };
        let has_terminals = space
            .tabs
            .iter()
            .flat_map(|tab| &tab.panes)
            .any(|pane| matches!(pane.content, PaneContent::Terminal(_)));
        if !has_terminals {
            self.send(key.machine, SpaceRequest::CloseSpace(key.space), cx);
            return;
        }
        let answer = window.prompt(
            PromptLevel::Warning,
            &format!("Close “{}”?", space.label()),
            Some("Its terminals end. Its threads stay in Agents."),
            &["Close", "Cancel"],
            cx,
        );
        cx.spawn(async move |this, cx| {
            if answer.await == Ok(0) {
                this.update(cx, |this, cx| {
                    this.send(key.machine, SpaceRequest::CloseSpace(key.space), cx)
                })
                .ok();
            }
        })
        .detach();
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

    /// An empty name names the workspace after its folder again, or numbers the tab.
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

    fn close_focused_pane(&mut self, _: &ClosePane, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(pane) = self.focused_pane(cx) {
            self.close_pane(pane, cx);
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
        let mut rows = Vec::new();
        let mut index_in_machine: HashMap<MachineId, usize> = HashMap::default();
        for (machine, space) in spaces {
            let index = index_in_machine.entry(machine).or_default();
            let row_index = *index;
            *index += 1;
            if !matches_space(&space, &query) {
                continue;
            }
            rows.push(self.render_space_row(machine, row_index, &space, cx));
        }

        v_flex()
            .w(SIDEBAR_WIDTH)
            .h_full()
            .flex_none()
            .border_r_1()
            .border_color(colors.border)
            .bg(colors.panel_background)
            .child(self.render_sidebar_header(cx))
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
                        Some(cx.new(|cx| {
                            NewSpacePicker::new(
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
    fn render_space_row(
        &self,
        machine: MachineId,
        index: usize,
        space: &Space,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let key = SpaceKey {
            machine,
            space: space.id,
        };
        let colors = cx.theme().colors().clone();
        let is_active = self.active_space == Some(key);
        let label: SharedString = space.label().into();
        let status = rolled_up(
            space
                .tabs
                .iter()
                .flat_map(|tab| &tab.panes)
                .filter_map(|pane| self.pane_status(machine, pane, cx)),
        );
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
            Some(project) => render_project_icon(project, project_info.as_ref(), px(16.), cx),
            None => render_folder_icon(),
        };
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
        let details_popover =
            (self.details_space == Some(key)).then(|| render_details_popover(details, cx));
        let id = format!("workspace-{}-{}", machine.slug(), space.id.0);

        let faint_text = colors.text_muted.opacity(0.4);
        let group_name = SharedString::from(format!("{id}-group"));
        // The sidebar's shell row: the icon and name, then the branch with what's inside and
        // the machine below.
        let main_line = h_flex()
            .relative()
            .h_6()
            .gap_2p5()
            .child(
                div()
                    .flex_none()
                    .when(!is_active, |this| {
                        this.opacity(0.4)
                            .group_hover(group_name.clone(), |this| this.opacity(1.))
                    })
                    .child(icon),
            )
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
                        .when(terminals > 0, |this| {
                            this.child(count_badge(IconName::Terminal, terminals))
                        })
                        .when(agents > 0, |this| {
                            this.child(count_badge(IconName::UserGroup, agents))
                        }),
                )
            })
            .child(
                div().flex_none().opacity(0.6).child(
                    Icon::new(machine_icon)
                        .size(IconSize::Small)
                        .color(Color::Muted),
                ),
            );

        let row =
            v_flex()
                .id(ElementId::Name(id.clone().into()))
                .debug_selector({
                    let id = id.clone();
                    move || id
                })
                .group(group_name)
                .mx_1()
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

        let this = cx.entity().downgrade();
        right_click_menu(ElementId::Name(format!("{id}-menu").into()))
            .trigger(move |is_menu_open, _, _| {
                div()
                    .relative()
                    .child(row)
                    .when(!is_menu_open, |this| this.children(details_popover))
            })
            .menu(move |window, cx| {
                let this = this.clone();
                let label = label.clone();
                let worktree_source = worktree_source.clone();
                ContextMenu::build(window, cx, move |menu, _, _| {
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
                        ContextMenuEntry::new("Rename")
                            .icon(IconName::Pencil)
                            .icon_color(Color::Muted)
                            .handler(rename),
                    )
                    .item(
                        ContextMenuEntry::new("Close")
                            .icon(IconName::Close)
                            .icon_color(Color::Muted)
                            .handler(close),
                    )
                    .when_some(worktree_source, |menu, source| {
                        menu.separator()
                            .item(
                                ContextMenuEntry::new("New Worktree")
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
                })
            })
            .into_any_element()
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

    /// The Archived shelf's slim row, with herdr's agent tokens: state, agent, then machine,
    /// workspace and tab.
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

        h_flex()
            .id(ElementId::Name(format!("workspace-agent-{index}").into()))
            .debug_selector(|| format!("agent-row-{index}"))
            .h(ARCHIVED_ROW_HEIGHT)
            .w_full()
            .px_2()
            .gap_2()
            .rounded_md()
            .cursor_pointer()
            .when(is_active, |row| row.bg(colors.ghost_element_selected))
            .when(!is_active, |row| {
                row.hover(|row| row.bg(colors.ghost_element_hover))
            })
            .when(is_offline, |row| row.opacity(0.5))
            .child(render_state_slot(entry.status, cx))
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
            )
            .child(
                div().min_w_0().max_w(px(120.)).child(
                    Label::new(location)
                        .size(LabelSize::XSmall)
                        .color(Color::Muted)
                        .truncate(),
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
                (Icon::new(IconName::Terminal), title.into(), folder)
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

    fn render_empty_state(&self, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .size_full()
            .items_center()
            .justify_center()
            .gap_2()
            .child(
                Label::new("Workspaces put terminals and threads side by side").color(Color::Muted),
            )
            .child(
                Button::new("start-workspace", "New Workspace")
                    .style(ButtonStyle::Outlined)
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
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.send(machine, SpaceRequest::CloseTab(tab_key.tab), cx)
                })),
            )
            .child(if is_renaming {
                div()
                    .w(px(120.))
                    .child(self.render_rename_input(cx))
                    .into_any_element()
            } else {
                Label::new(label.clone())
                    .size(LabelSize::Small)
                    .into_any_element()
            })
            .on_click(cx.listener(move |this, event: &ClickEvent, window, cx| {
                if event.click_count() == 2 {
                    let name = this
                        .space(space_key, cx)
                        .and_then(|space| {
                            let index = space.tabs.iter().position(|tab| tab.id == tab_key.tab)?;
                            Some(tab_label(&space.tabs[index], index))
                        })
                        .unwrap_or_default();
                    this.start_renaming(RenameTarget::Tab(tab_key), name.into(), window, cx);
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
                        move |_: &mut Window, cx: &mut App| {
                            this.update(cx, |this, cx| {
                                this.send(machine, SpaceRequest::CloseTab(tab_key.tab), cx)
                            })
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
        let split_menu = {
            let this = cx.entity().downgrade();
            PopoverMenu::new(key.element_id("pane-split"))
                .trigger_with_tooltip(
                    IconButton::new(key.element_id("pane-split-button"), IconName::Split)
                        .icon_size(IconSize::Small),
                    Tooltip::text("Split Pane"),
                )
                .anchor(gpui::Anchor::TopRight)
                .menu(move |window, cx| {
                    let this = this.clone();
                    Some(ContextMenu::build(window, cx, move |menu, _, _| {
                        let split = |direction: Direction| {
                            let this = this.clone();
                            move |window: &mut Window, cx: &mut App| {
                                this.update(cx, |this, cx| this.split(key, direction, window, cx))
                                    .ok();
                            }
                        };
                        menu.entry(
                            "Split Right",
                            Some(Box::new(SplitRight)),
                            split(Direction::Horizontal),
                        )
                        .entry(
                            "Split Down",
                            Some(Box::new(SplitDown)),
                            split(Direction::Vertical),
                        )
                    }))
                })
        };
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
                    .child(split_menu)
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
                            .on_click(cx.listener(move |this, _, _, cx| this.close_pane(key, cx))),
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
            .size_full()
            .when(shows_focus, |pane| {
                pane.border_1().border_color(if is_focused {
                    colors.pane_focused_border
                } else {
                    gpui::transparent_black()
                })
            })
            .bg(colors.editor_background)
            .capture_any_mouse_down(
                cx.listener(move |this, _, window, cx| this.focus_pane(key, window, cx)),
            )
            .drag_over::<DraggedLabel<DraggedPane>>(move |style, dragged, _, cx| {
                if dragged.item.tab == tab_key && dragged.item.pane != key.pane {
                    style.border_color(cx.theme().colors().border_focused)
                } else {
                    style
                }
            })
            .on_drop(
                cx.listener(move |this, dragged: &DraggedLabel<DraggedPane>, _, cx| {
                    if dragged.item.tab == tab_key && dragged.item.pane != key.pane {
                        this.send(
                            key.machine,
                            SpaceRequest::SwapPanes(dragged.item.pane, key.pane),
                            cx,
                        );
                    }
                }),
            )
            .child(header)
            .child(div().flex_1().min_h_0().child(content))
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
            let machines = Machines::global(cx);
            let threads: Vec<(ThreadId, SharedString)> = machines
                .read(cx)
                .active_threads(cx)
                .into_iter()
                .filter(|(machine, thread)| *machine == key.machine && thread.task.is_none())
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
                    on(|this, key, _, cx| this.close_pane(key, cx)),
                )
            })
        }
    }

    /// New Thread, for the pane's workspace's project and folder.
    fn new_thread_in_pane(&mut self, pane: PaneKey, cx: &mut Context<Self>) {
        let Some((space, _, _)) = self.find_pane(pane, cx) else {
            return;
        };
        let project = space.project_id.map(|project| ProjectKey {
            machine: pane.machine,
            project,
        });
        cx.emit(SpacesViewEvent::NewThreadInPane {
            pane,
            project,
            folder: project.map(|_| space.folder.clone()),
        });
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

/// A tab's name, or its number, as herdr numbers unnamed tabs.
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

fn tab_label(tab: &Tab, index: usize) -> String {
    tab.name.clone().unwrap_or_else(|| (index + 1).to_string())
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
        assert_eq!(tab_label(&tab, 1), "2");
        let named = Tab {
            name: Some("server".to_string()),
            ..tab
        };
        assert_eq!(tab_label(&named, 1), "server");
    }
}
