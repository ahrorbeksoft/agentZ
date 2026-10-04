use std::path::PathBuf;

use crate::machines::{MachineId, Machines, MachinesEvent, ProjectKey, Scope, ThreadKey};
use crate::project_store::ThreadStatus;
use agentz_protocol::agents::{AgentId, InstallState};
use agentz_protocol::workspace::WorkspaceChoice;
use collections::HashMap;
use gpui::{
    AnyView, App, Context, DismissEvent, DragMoveEvent, Entity, FocusHandle, Focusable,
    MouseButton, PathPromptOptions, Subscription, SystemNotification, Task, Window,
    WindowControlArea,
};
use projects::Thread;
use ui::{
    ButtonLike, PopoverMenu, PopoverMenuHandle, ToggleButtonGroup, ToggleButtonGroupSize,
    ToggleButtonSimple, Tooltip, prelude::*,
};
use util::ResultExt as _;

use crate::add_project_modal::{AddProjectModal, AddProjectModalEvent};
use crate::agent_view::{AgentView, AgentViewEvent, RESIZE_EDGE_SIZE};
use crate::app_settings::{AppSettingsStore, MachineProfile, is_sidebar_hidden};
use crate::confirm_dialog::{ConfirmDialog, ConfirmRequest};
use crate::diff_panel::{DIFF_PANEL_WIDTH, DiffPanel, DiffPanelEvent};
use crate::machine_modal::MachineModal;
use crate::new_thread_modal::{NewThreadModal, NewThreadModalEvent};
use crate::project_info::{ProjectInfoStore, render_project_icon};
use crate::project_switcher::ProjectSwitcher;
use crate::server_client::MachineStatus;
use crate::settings_page::{SettingsPage, SettingsPageEvent};
use crate::sidebar::{AWAITING_INPUT_COLOR, SIDEBAR_WIDTH, Sidebar, SidebarEvent};
use crate::spaces_view::{PaneKey, SpacesView, SpacesViewEvent};
use crate::terminal_thread_view::TerminalThreadView;
use crate::thread_entity::AgentThread;
use crate::worktree_modal::{WorktreeModal, WorktreeModalEvent, WorktreeModalMode};
use crate::{
    NewThread, OpenFolder, OpenSettings, ToggleDiff, ToggleProjectSwitcher, ToggleSidebar,
    ToggleTerminalDrawer,
};

const TITLE_BAR_HEIGHT: Pixels = px(40.);
const MIN_DIFF_PANEL_WIDTH: Pixels = px(280.);
/// What a dragged Changes panel leaves of the thread.
const MIN_THREAD_WIDTH: Pixels = px(320.);

/// The Changes panel's left edge, being dragged to resize it.
struct DraggedDiffEdge;
/// Leaves room for the macOS traffic lights.
const TRAFFIC_LIGHTS_WIDTH: Pixels = px(80.);

/// The title bar's tabs: one thread full screen, or herdr's workspaces of panes.
#[derive(Clone, Copy, PartialEq, Eq)]
enum MainView {
    Agents,
    Workspaces,
}

/// An open thread. Kept while the app runs so its agent keeps working in the background.
struct OpenThread {
    view: ThreadView,
    _subscriptions: Vec<Subscription>,
}

#[derive(Clone)]
enum ThreadView {
    Agent(Entity<AgentView>),
    Terminal(Entity<TerminalThreadView>),
}

impl ThreadView {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        match self {
            Self::Agent(view) => view.focus_handle(cx),
            Self::Terminal(view) => view.focus_handle(cx),
        }
    }

    fn set_title(&self, title: SharedString, cx: &mut App) {
        match self {
            Self::Agent(view) => view.update(cx, |view, cx| view.set_title(title, cx)),
            Self::Terminal(view) => view.update(cx, |view, cx| view.set_title(title, cx)),
        }
    }

    fn set_has_agent(&self, has_agent: bool, cx: &mut App) {
        if let Self::Terminal(view) = self {
            view.update(cx, |view, cx| view.set_has_agent(has_agent, cx));
        }
    }

    fn set_archived(&self, is_archived: bool, cx: &mut App) {
        if let Self::Agent(view) = self {
            view.update(cx, |view, cx| view.set_archived(is_archived, cx));
        }
    }

    fn set_diff_open(&self, is_diff_open: bool, cx: &mut App) {
        match self {
            Self::Agent(view) => view.update(cx, |view, cx| view.set_diff_open(is_diff_open, cx)),
            Self::Terminal(view) => {
                view.update(cx, |view, cx| view.set_diff_open(is_diff_open, cx))
            }
        }
    }

    fn into_any_element(self) -> AnyElement {
        match self {
            Self::Agent(view) => view.into_any_element(),
            Self::Terminal(view) => view.into_any_element(),
        }
    }
}

pub struct Shell {
    focus_handle: FocusHandle,
    machines: Entity<Machines>,
    view: MainView,
    sidebar: Entity<Sidebar>,
    spaces_view: Entity<SpacesView>,
    /// The pane New Thread's thread goes in, when it was asked for from one.
    thread_target: Option<PaneKey>,
    /// A draft New Thread is making, so another press doesn't make a second.
    _starting_draft: Option<Task<()>>,
    switcher_handle: PopoverMenuHandle<ProjectSwitcher>,
    new_thread_modal: Option<(Entity<NewThreadModal>, Vec<Subscription>)>,
    add_project_modal: Option<(Entity<AddProjectModal>, Vec<Subscription>)>,
    worktree_modal: Option<(Entity<WorktreeModal>, Vec<Subscription>)>,
    machine_modal: Option<(Entity<MachineModal>, Subscription)>,
    confirm_dialog: Option<(Entity<ConfirmDialog>, Subscription)>,
    /// Shown in the main area in place of the thread while open.
    settings_page: Option<(Entity<SettingsPage>, Subscription)>,
    open_threads: HashMap<ThreadKey, OpenThread>,
    active_thread: Option<ThreadKey>,
    /// Whether the active thread's changes show beside it. Stays on across threads.
    show_diff: bool,
    /// The active thread's changes while shown. Only one, so hidden threads don't reload theirs.
    diff_panel: Option<Entity<DiffPanel>>,
    _diff_panel_events: Option<Subscription>,
    /// The panel's width, as its left edge was last dragged.
    diff_width: Pixels,
    /// The panel fills the thread's area.
    diff_full_screen: bool,
    should_move_window: bool,
    _subscriptions: Vec<Subscription>,
}

impl Shell {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let machines = Machines::global(cx);
        let sidebar = cx.new(Sidebar::new);
        let spaces_view = cx.new(|cx| SpacesView::new(window, cx));
        let subscriptions = vec![
            cx.subscribe_in(
                &spaces_view,
                window,
                |this, _, event, window, cx| match event {
                    SpacesViewEvent::OpenThread(thread) => this.open_thread(*thread, window, cx),
                    SpacesViewEvent::NewThreadInPane {
                        pane,
                        project,
                        folder,
                    } => match project {
                        Some(project) => {
                            this.start_draft(*project, folder.clone(), Some(*pane), window, cx)
                        }
                        None => {
                            this.open_new_thread_modal(window, cx);
                            this.thread_target = Some(*pane);
                        }
                    },
                    SpacesViewEvent::NewThread { project, folder } => {
                        this.start_draft(*project, Some(folder.clone()), None, window, cx)
                    }
                    SpacesViewEvent::Worktree {
                        machine,
                        folder,
                        name,
                        mode,
                    } => this.open_worktree_modal(
                        *machine,
                        folder.clone(),
                        name.clone(),
                        *mode,
                        window,
                        cx,
                    ),
                    SpacesViewEvent::Confirm(request) => {
                        this.open_confirm_dialog(request.clone(), window, cx)
                    }
                    SpacesViewEvent::OpenAgentSettings => this.open_agent_settings(window, cx),
                },
            ),
            cx.observe_in(&machines, window, |this, _, window, cx| {
                // Close views (and stop their agents) for threads that were deleted, removed
                // along with their project, or whose machine was removed. Archived threads stay
                // open, read-only.
                let threads: HashMap<ThreadKey, Option<Thread>> = this
                    .open_threads
                    .keys()
                    .map(|key| (*key, this.thread(*key, cx)))
                    .collect();
                let is_live = |key: ThreadKey| threads.get(&key).is_some_and(Option::is_some);
                this.open_threads.retain(|key, _| is_live(*key));
                let machines = this.machines.read(cx);
                let states: Vec<(ThreadView, SharedString, bool, bool)> = this
                    .open_threads
                    .iter()
                    .filter_map(|(key, open_thread)| {
                        let thread = threads.get(key)?.as_ref()?;
                        let has_agent = machines.projects(key.machine, cx).is_some_and(|store| {
                            store.read(cx).terminal_agent(key.thread).is_some()
                        });
                        Some((
                            open_thread.view.clone(),
                            thread.title.clone().into(),
                            thread.archived_at.is_some(),
                            has_agent,
                        ))
                    })
                    .collect();
                let closed_active_thread = this
                    .active_thread
                    .is_some_and(|thread_id| !is_live(thread_id));
                if closed_active_thread {
                    this.active_thread = None;
                    // Deferred: the change may have come from the sidebar itself.
                    let sidebar = this.sidebar.clone();
                    cx.defer(move |cx| {
                        sidebar.update(cx, |sidebar, cx| sidebar.set_active_thread(None, cx))
                    });
                }
                for (view, title, is_archived, has_agent) in states {
                    view.set_title(title, cx);
                    view.set_archived(is_archived, cx);
                    view.set_has_agent(has_agent, cx);
                }
                // Focus was in the closed thread's view. Without moving it here, actions such as
                // New Thread would be dispatched from the window's root, above the shell's
                // handlers, and do nothing.
                if closed_active_thread {
                    window.focus(&this.focus_handle, cx);
                    this.sync_diff_panel(cx);
                }
                this.mark_active_thread_viewed(window, cx);
                cx.notify();
            }),
            cx.subscribe_in(
                &machines,
                window,
                |this, _, event, window, cx| match event {
                    MachinesEvent::NeedsAttention(thread, status) => {
                        this.notify_attention(*thread, *status, window, cx)
                    }
                },
            ),
            cx.observe_window_activation(window, |this, window, cx| {
                this.mark_active_thread_viewed(window, cx)
            }),
            cx.subscribe_in(&sidebar, window, |this, _, event, window, cx| match event {
                SidebarEvent::OpenThread(thread_id) => this.open_thread(*thread_id, window, cx),
                SidebarEvent::OpenProjectSettings(project_id) => {
                    this.open_project_settings(*project_id, window, cx)
                }
            }),
            cx.observe(&ProjectInfoStore::global(cx), |_, _, cx| cx.notify()),
            // With the theme mode set to System, the theme follows macOS's appearance.
            cx.observe_window_appearance(window, |_, _, cx| {
                AppSettingsStore::global(cx).update(cx, |store, cx| store.reapply_theme(cx));
            }),
        ];
        // Clicking a notification is the user asking for that thread, so it may come forward.
        let shell = cx.entity().downgrade();
        let window_handle = window.window_handle();
        cx.on_system_notification_response(move |response, cx| {
            let Some(thread_id) = thread_from_notification_tag(&response.tag) else {
                return;
            };
            let shell = shell.clone();
            window_handle
                .update(cx, |_, window, cx| {
                    window.activate_window();
                    shell
                        .update(cx, |shell, cx| shell.open_thread(thread_id, window, cx))
                        .ok();
                })
                .log_err();
            cx.activate(true);
        });
        Self {
            focus_handle: cx.focus_handle(),
            machines,
            view: MainView::Agents,
            sidebar,
            spaces_view,
            thread_target: None,
            _starting_draft: None,
            switcher_handle: PopoverMenuHandle::default(),
            new_thread_modal: None,
            add_project_modal: None,
            worktree_modal: None,
            machine_modal: None,
            confirm_dialog: None,
            settings_page: None,
            open_threads: HashMap::default(),
            active_thread: None,
            show_diff: false,
            diff_panel: None,
            _diff_panel_events: None,
            diff_width: DIFF_PANEL_WIDTH,
            diff_full_screen: false,
            should_move_window: false,
            _subscriptions: subscriptions,
        }
    }

    /// The Agents view's threads waiting on the user, and the most urgent of their states.
    fn waiting_threads(&self, cx: &App) -> (usize, Option<ThreadStatus>) {
        let machines = self.machines.read(cx);
        let statuses: Vec<ThreadStatus> = machines
            .active_threads(cx)
            .into_iter()
            .filter_map(|(machine, thread)| {
                machines
                    .projects(machine, cx)?
                    .read(cx)
                    .thread_status(thread.id)
            })
            .filter(|status| {
                matches!(
                    status,
                    ThreadStatus::PendingApproval | ThreadStatus::AwaitingInput
                )
            })
            .collect();
        let most_urgent = if statuses.contains(&ThreadStatus::PendingApproval) {
            Some(ThreadStatus::PendingApproval)
        } else {
            statuses.first().copied()
        };
        (statuses.len(), most_urgent)
    }

    /// The app's copy of a thread on any machine.
    fn thread(&self, key: ThreadKey, cx: &App) -> Option<Thread> {
        self.machines
            .read(cx)
            .projects(key.machine, cx)?
            .read(cx)
            .thread(key.thread)
            .cloned()
    }

    /// A draft in the shown project, as t3code's New Thread opens one; with several shown, the
    /// modal asks which first.
    fn new_thread(&mut self, _: &NewThread, window: &mut Window, cx: &mut Context<Self>) {
        let machines = self.machines.read(cx);
        if machines.project_groups(cx).is_empty() {
            window.dispatch_action(Box::new(OpenFolder), cx);
            return;
        }
        let project = match machines.visible_groups(cx).as_slice() {
            [group] => machines.new_thread_member(group, cx),
            _ => None,
        };
        match project {
            Some(project) => self.start_draft(project, None, None, window, cx),
            None => self.open_new_thread_modal(window, cx),
        }
    }

    fn open_new_thread_modal(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let modal = cx.new(|cx| NewThreadModal::new(window, cx));
        let subscriptions = vec![
            cx.subscribe_in(&modal, window, |this, _, _: &DismissEvent, window, cx| {
                this.dismiss_modal(window, cx);
            }),
            cx.subscribe_in(&modal, window, |this, _, event, window, cx| match event {
                NewThreadModalEvent::ProjectChosen(project) => {
                    let pane = this.thread_target.take();
                    this.dismiss_modal(window, cx);
                    this.start_draft(*project, None, pane, window, cx);
                }
            }),
        ];
        self.new_thread_modal = Some((modal, subscriptions));
        cx.notify();
    }

    /// Opens a draft in the project, working in `folder` (one of its workspaces, or its own
    /// folder): the one open already with nothing typed, as t3code reuses an untouched draft,
    /// or else a new one with the agent used last. Without an agent installed, Settings ›
    /// Agents opens instead.
    fn start_draft(
        &mut self,
        project: ProjectKey,
        folder: Option<PathBuf>,
        pane: Option<PaneKey>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(store) = self.machines.read(cx).projects(project.machine, cx) else {
            return;
        };
        let workspace = folder.filter(|folder| {
            store
                .read(cx)
                .project(project.project)
                .is_none_or(|project| project.path != *folder)
        });
        let untouched = self.open_threads.iter().find_map(|(key, open_thread)| {
            let ThreadView::Agent(view) = &open_thread.view else {
                return None;
            };
            let thread = store.read(cx).thread(key.thread)?;
            (key.machine == project.machine
                && thread.project_id == project.project
                && thread.workspace == workspace
                && thread.terminal.is_none()
                && view.read(cx).is_untouched_draft(cx))
            .then_some(*key)
        });
        if let Some(draft) = untouched {
            self.show_new_thread(draft, pane, window, cx);
            return;
        }
        let Some(agent_id) = self.default_agent(project.machine, cx) else {
            self.open_agent_settings(window, cx);
            return;
        };
        if self._starting_draft.is_some() {
            return;
        }
        let choice = workspace.map_or(WorkspaceChoice::Checkout, WorkspaceChoice::Existing);
        let created = store.update(cx, |store, cx| {
            store.create_thread(project.project, agent_id, choice, cx)
        });
        self._starting_draft = Some(cx.spawn_in(window, async move |this, cx| {
            let created = created.await;
            this.update_in(cx, |this, window, cx| {
                this._starting_draft = None;
                match created {
                    Ok(thread) => {
                        let draft = ThreadKey {
                            machine: project.machine,
                            thread,
                        };
                        this.show_new_thread(draft, pane, window, cx);
                    }
                    Err(error) => log::error!("couldn't start a thread: {error:#}"),
                }
            })
            .log_err();
        }));
    }

    /// Shows a thread just started: in the pane it was asked for from, or on its own.
    fn show_new_thread(
        &mut self,
        thread: ThreadKey,
        pane: Option<PaneKey>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match pane {
            Some(pane) if pane.machine == thread.machine => {
                self.spaces_view.update(cx, |view, cx| {
                    view.show_thread_in_pane(pane, thread.thread, cx)
                })
            }
            _ => self.open_thread(thread, window, cx),
        }
    }

    /// The agent a new draft starts with: that of the machine's newest thread, as t3code
    /// carries the user's last choice, or else the first installed.
    fn default_agent(&self, machine: MachineId, cx: &App) -> Option<AgentId> {
        let client = self.machines.read(cx).client(machine, cx)?;
        let client = client.read(cx);
        let registry = client.registry().read(cx);
        let is_installed = |agent_id: &AgentId| {
            matches!(
                registry.install_state(agent_id),
                InstallState::Installed { .. }
            )
        };
        let last_used = client
            .projects()
            .read(cx)
            .threads()
            .iter()
            .filter(|thread| thread.terminal.is_none() && thread.task.is_none())
            .filter_map(|thread| {
                let agent_id = AgentId::new(thread.agent_id.clone()?);
                is_installed(&agent_id).then_some((thread.created_at, thread.id, agent_id))
            })
            .max_by_key(|(created_at, thread_id, _)| (*created_at, *thread_id))
            .map(|(_, _, agent_id)| agent_id);
        last_used.or_else(|| {
            registry
                .agents()
                .iter()
                .find(|agent| agent.supports_current_platform() && is_installed(agent.id()))
                .map(|agent| agent.id().clone())
        })
    }

    fn open_agent_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.open_settings(&OpenSettings, window, cx);
        if let Some((page, _)) = &self.settings_page {
            page.update(cx, |page, cx| page.show_agents(window, cx));
        }
    }

    /// Cmd-J with focus outside the thread view (the Changes panel, the sidebar): the open
    /// thread's terminal. With focus inside, the thread view handles it first.
    fn toggle_terminal_drawer(
        &mut self,
        action: &ToggleTerminalDrawer,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.view != MainView::Agents || self.settings_page.is_some() {
            return;
        }
        let view = self
            .active_thread
            .and_then(|thread_id| self.open_threads.get(&thread_id))
            .map(|open_thread| open_thread.view.clone());
        if let Some(ThreadView::Agent(view)) = view {
            view.update(cx, |view, cx| {
                view.toggle_terminal_drawer(action, window, cx)
            });
        }
    }

    fn toggle_sidebar(&mut self, _: &ToggleSidebar, _: &mut Window, cx: &mut Context<Self>) {
        AppSettingsStore::global(cx).update(cx, |store, cx| {
            store.update(
                |settings| settings.is_sidebar_hidden = !settings.is_sidebar_hidden,
                cx,
            )
        });
        self.spaces_view.update(cx, |_, cx| cx.notify());
        cx.notify();
    }

    fn toggle_diff(&mut self, _: &ToggleDiff, _: &mut Window, cx: &mut Context<Self>) {
        self.show_diff = !self.show_diff;
        self.sync_diff_panel(cx);
    }

    /// Shows the active thread's changes when they're wanted, and tells the views.
    fn sync_diff_panel(&mut self, cx: &mut Context<Self>) {
        let thread_id = self.active_thread.filter(|_| {
            self.show_diff && self.settings_page.is_none() && self.view == MainView::Agents
        });
        match thread_id {
            Some(key) => {
                let is_current = self.diff_panel.as_ref().is_some_and(|panel| {
                    let panel = panel.read(cx);
                    panel.thread_id() == key.thread
                        && panel.client().read(cx).machine() == key.machine
                });
                if !is_current {
                    let panel = self
                        .machines
                        .read(cx)
                        .client(key.machine, cx)
                        .map(|client| cx.new(|cx| DiffPanel::new(client, key.thread, cx)));
                    self._diff_panel_events = panel
                        .as_ref()
                        .map(|panel| cx.subscribe(panel, Self::handle_diff_panel_event));
                    self.diff_panel = panel;
                }
            }
            None => {
                self.diff_panel = None;
                self._diff_panel_events = None;
            }
        }
        let is_full_screen = self.diff_full_screen;
        if let Some(panel) = &self.diff_panel {
            panel.update(cx, |panel, cx| panel.set_full_screen(is_full_screen, cx));
        }
        for (open_thread_id, open_thread) in &self.open_threads {
            let is_diff_open = thread_id == Some(*open_thread_id);
            open_thread.view.set_diff_open(is_diff_open, cx);
        }
        cx.notify();
    }

    fn handle_diff_panel_event(
        &mut self,
        _: Entity<DiffPanel>,
        event: &DiffPanelEvent,
        cx: &mut Context<Self>,
    ) {
        match event {
            DiffPanelEvent::Close => {
                self.show_diff = false;
                self.diff_full_screen = false;
            }
            DiffPanelEvent::ToggleFullScreen => self.diff_full_screen = !self.diff_full_screen,
        }
        self.sync_diff_panel(cx);
    }

    fn open_settings(&mut self, _: &OpenSettings, window: &mut Window, cx: &mut Context<Self>) {
        let page = match &self.settings_page {
            Some((page, _)) => page.clone(),
            None => {
                let page = cx.new(SettingsPage::new);
                let subscription =
                    cx.subscribe_in(&page, window, |this, _, event, window, cx| match event {
                        SettingsPageEvent::Close => this.close_settings(window, cx),
                        SettingsPageEvent::EditMachine(profile) => {
                            this.open_machine_modal(profile.as_ref(), window, cx)
                        }
                        SettingsPageEvent::Confirm(request) => {
                            this.open_confirm_dialog(request.clone(), window, cx)
                        }
                        SettingsPageEvent::OpenThread(thread) => {
                            this.open_thread(*thread, window, cx)
                        }
                    });
                self.settings_page = Some((page.clone(), subscription));
                page
            }
        };
        window.focus(&page.focus_handle(cx), cx);
        self.sync_diff_panel(cx);
    }

    fn open_project_settings(
        &mut self,
        project_id: ProjectKey,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.open_settings(&OpenSettings, window, cx);
        if let Some((page, _)) = &self.settings_page {
            page.update(cx, |page, cx| page.show_project(project_id, window, cx));
        }
    }

    fn close_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.settings_page.take().is_none() {
            return;
        }
        self.focus_main(window, cx);
        self.mark_active_thread_viewed(window, cx);
        self.sync_diff_panel(cx);
    }

    fn open_machine_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.open_settings(&OpenSettings, window, cx);
        if let Some((page, _)) = &self.settings_page {
            page.update(cx, |page, cx| page.show_machines(window, cx));
        }
    }

    fn open_thread(&mut self, thread_id: ThreadKey, window: &mut Window, cx: &mut Context<Self>) {
        self.settings_page = None;
        if self.view != MainView::Agents {
            self.view = MainView::Agents;
            self.spaces_view
                .update(cx, |view, cx| view.set_visible(false, window, cx));
        }
        if !self.open_threads.contains_key(&thread_id) {
            let Some(open_thread) = self.start_thread(thread_id, window, cx) else {
                return;
            };
            self.open_threads.insert(thread_id, open_thread);
        }
        // A new thread is a draft until its first message: leaving it closes it, saving what's
        // typed, and the server removes it once no window has it open, unless something is.
        if let Some(previous) = self.active_thread.filter(|previous| *previous != thread_id)
            && let Some(OpenThread {
                view: ThreadView::Agent(view),
                ..
            }) = self.open_threads.get(&previous)
            && view.read(cx).is_draft(cx)
        {
            self.open_threads.remove(&previous);
        }
        self.active_thread = Some(thread_id);
        // A subthread isn't in the sidebar, so its top-level thread is highlighted.
        let sidebar_thread = ThreadKey {
            machine: thread_id.machine,
            thread: self
                .machines
                .read(cx)
                .projects(thread_id.machine, cx)
                .map_or(thread_id.thread, |store| {
                    store.read(cx).root_thread(thread_id.thread)
                }),
        };
        self.sidebar.update(cx, |sidebar, cx| {
            sidebar.set_active_thread(Some(sidebar_thread), cx)
        });
        if let Some(open_thread) = self.open_threads.get(&thread_id) {
            window.focus(&open_thread.view.focus_handle(cx), cx);
        }
        self.mark_active_thread_viewed(window, cx);
        self.sync_diff_panel(cx);
    }

    /// Whether the user can see the thread right now, as Zed's `agent_status_visible` decides.
    fn is_thread_visible(&self, thread_id: ThreadKey, window: &Window, cx: &App) -> bool {
        if !window.is_window_active() || self.settings_page.is_some() {
            return false;
        }
        match self.view {
            MainView::Agents => self.active_thread == Some(thread_id),
            MainView::Workspaces => self.spaces_view.read(cx).shows_thread(thread_id, cx),
        }
    }

    fn mark_active_thread_viewed(&self, window: &Window, cx: &mut Context<Self>) {
        let Some(thread_id) = self.active_thread else {
            return;
        };
        if self.view == MainView::Agents && self.is_thread_visible(thread_id, window, cx) {
            if let Some(store) = self.machines.read(cx).projects(thread_id.machine, cx) {
                store.update(cx, |store, cx| store.mark_viewed(thread_id.thread, cx));
            }
            cx.dismiss_system_notification(&notification_tag(thread_id));
        }
    }

    /// A macOS notification for a thread that isn't on screen, as Zed notifies.
    fn notify_attention(
        &self,
        thread_id: ThreadKey,
        status: ThreadStatus,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.is_thread_visible(thread_id, window, cx) {
            return;
        }
        let machines = self.machines.read(cx);
        let Some(store) = machines.projects(thread_id.machine, cx) else {
            return;
        };
        let store = store.read(cx);
        let Some(thread) = store.thread(thread_id.thread) else {
            return;
        };
        let caption = match status {
            ThreadStatus::PendingApproval => "Waiting for tool confirmation",
            ThreadStatus::AwaitingInput => "Waiting for your input",
            ThreadStatus::Working | ThreadStatus::Completed => "Finished",
        };
        let mut body = match store.project(thread.project_id) {
            Some(project) => format!("{} · {caption}", project.name()),
            None => caption.to_string(),
        };
        if thread_id.machine != MachineId::Local {
            body = format!("{} · {body}", machines.label(thread_id.machine, cx));
        }
        cx.show_system_notification(SystemNotification {
            tag: notification_tag(thread_id),
            title: thread.title.clone().into(),
            body: body.into(),
            actions: Vec::new(),
        });
        if !window.is_window_active() {
            window.request_attention();
        }
    }

    fn start_thread(
        &mut self,
        key: ThreadKey,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<OpenThread> {
        let client = self.machines.read(cx).client(key.machine, cx)?;
        let thread_id = key.thread;
        let thread = client
            .read(cx)
            .projects()
            .read(cx)
            .thread(thread_id)?
            .clone();
        if let Some(command) = thread.terminal.clone() {
            let title = SharedString::from(thread.title);
            let has_agent = client
                .read(cx)
                .projects()
                .read(cx)
                .terminal_agent(thread_id)
                .is_some();
            let view = cx.new(|cx| {
                let mut view = TerminalThreadView::new(&client, thread_id, title, command, cx);
                view.set_has_agent(has_agent, cx);
                view
            });
            return Some(OpenThread {
                view: ThreadView::Terminal(view),
                _subscriptions: Vec::new(),
            });
        }
        let agent_id = thread.agent_id.clone().map(AgentId::new);
        let agent_thread = AgentThread::shared(&client, thread_id, cx);
        let title = SharedString::from(thread.title);
        let is_archived = thread.archived_at.is_some();
        let store = client.read(cx).projects().clone();
        let view = cx.new(|cx| {
            let mut view = AgentView::new(thread_id, agent_thread, title, agent_id, cx);
            view.set_archived(is_archived, cx);
            view
        });
        let view_subscription = cx.subscribe_in(
            &view,
            window,
            move |this, _, event, window, cx| match event {
                AgentViewEvent::Unarchive => {
                    store.update(cx, |store, cx| store.unarchive_thread(thread_id, cx))
                }
                AgentViewEvent::OpenThread(other) => this.open_thread(
                    ThreadKey {
                        machine: key.machine,
                        thread: *other,
                    },
                    window,
                    cx,
                ),
                AgentViewEvent::Confirm(request) => {
                    this.open_confirm_dialog(request.clone(), window, cx)
                }
                AgentViewEvent::NewThreadInProject(project) => this.start_draft(
                    ProjectKey {
                        machine: key.machine,
                        project: *project,
                    },
                    None,
                    None,
                    window,
                    cx,
                ),
                AgentViewEvent::OpenAgentSettings => this.open_agent_settings(window, cx),
                AgentViewEvent::Replaced { thread, text } => {
                    this.open_thread(*thread, window, cx);
                    if let Some(OpenThread {
                        view: ThreadView::Agent(view),
                        ..
                    }) = this.open_threads.get(thread)
                    {
                        let text = text.clone();
                        view.update(cx, |view, cx| view.set_composer_text(text, cx));
                    }
                }
            },
        );
        Some(OpenThread {
            view: ThreadView::Agent(view),
            _subscriptions: vec![view_subscription],
        })
    }

    fn dismiss_modal(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let had_new_thread_modal = self.new_thread_modal.take().is_some();
        let had_add_project_modal = self.add_project_modal.take().is_some();
        let had_worktree_modal = self.worktree_modal.take().is_some();
        let had_machine_modal = self.machine_modal.take().is_some();
        let had_confirm_dialog = self.confirm_dialog.take().is_some();
        self.thread_target = None;
        if had_new_thread_modal
            || had_add_project_modal
            || had_worktree_modal
            || had_machine_modal
            || had_confirm_dialog
        {
            self.focus_main(window, cx);
            cx.notify();
        }
    }

    /// Focus goes back to what the main area shows.
    fn focus_main(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some((page, _)) = &self.settings_page {
            window.focus(&page.focus_handle(cx), cx);
            return;
        }
        match self.view {
            MainView::Agents => match self
                .active_thread
                .and_then(|thread_id| self.open_threads.get(&thread_id))
            {
                Some(open_thread) => window.focus(&open_thread.view.focus_handle(cx), cx),
                None => window.focus(&self.focus_handle, cx),
            },
            MainView::Workspaces => self
                .spaces_view
                .update(cx, |view, cx| view.focus_active(window, cx)),
        }
    }

    fn set_view(&mut self, view: MainView, window: &mut Window, cx: &mut Context<Self>) {
        self.settings_page = None;
        if self.view != view {
            self.view = view;
            self.spaces_view.update(cx, |spaces_view, cx| {
                spaces_view.set_visible(view == MainView::Workspaces, window, cx)
            });
        }
        self.focus_main(window, cx);
        self.mark_active_thread_viewed(window, cx);
        self.sync_diff_panel(cx);
    }

    /// With other machines, asks which machine the project is on first.
    fn open_folder(&mut self, _: &OpenFolder, window: &mut Window, cx: &mut Context<Self>) {
        if self.machines.read(cx).has_remotes() {
            self.open_add_project_modal(None, window, cx);
        } else {
            self.pick_local_folders(cx);
        }
    }

    fn open_add_project_modal(
        &mut self,
        machine: Option<MachineId>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let modal = cx.new(|cx| AddProjectModal::new(machine, window, cx));
        let subscriptions = vec![
            cx.subscribe_in(&modal, window, |this, _, _: &DismissEvent, window, cx| {
                this.dismiss_modal(window, cx);
            }),
            cx.subscribe_in(&modal, window, |this, _, event, window, cx| match event {
                AddProjectModalEvent::ProjectAdded(project) => {
                    this.dismiss_modal(window, cx);
                    reveal_project(&this.machines, *project, cx);
                }
                AddProjectModalEvent::ChooseLocalFolder => {
                    this.dismiss_modal(window, cx);
                    this.pick_local_folders(cx);
                }
            }),
        ];
        self.add_project_modal = Some((modal, subscriptions));
        cx.notify();
    }

    fn open_machine_modal(
        &mut self,
        profile: Option<&MachineProfile>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let modal = cx.new(|cx| MachineModal::new(profile, window, cx));
        let subscription =
            cx.subscribe_in(&modal, window, |this, _, _: &DismissEvent, window, cx| {
                this.dismiss_modal(window, cx);
            });
        self.machine_modal = Some((modal, subscription));
        cx.notify();
    }

    fn open_confirm_dialog(
        &mut self,
        request: ConfirmRequest,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let dialog = cx.new(|cx| ConfirmDialog::new(request, window, cx));
        let subscription =
            cx.subscribe_in(&dialog, window, |this, _, _: &DismissEvent, window, cx| {
                this.dismiss_modal(window, cx);
            });
        self.confirm_dialog = Some((dialog, subscription));
        cx.notify();
    }

    fn open_worktree_modal(
        &mut self,
        machine: MachineId,
        folder: PathBuf,
        name: SharedString,
        mode: WorktreeModalMode,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let modal = cx.new(|cx| WorktreeModal::new(machine, folder, name, mode, window, cx));
        let subscriptions = vec![
            cx.subscribe_in(&modal, window, |this, _, _: &DismissEvent, window, cx| {
                this.dismiss_modal(window, cx);
            }),
            cx.subscribe_in(&modal, window, |this, _, event, window, cx| match event {
                WorktreeModalEvent::Open { machine, folder } => {
                    this.dismiss_modal(window, cx);
                    this.spaces_view.update(cx, |view, cx| {
                        view.open_space_at(*machine, folder.clone(), window, cx)
                    });
                }
            }),
        ];
        self.worktree_modal = Some((modal, subscriptions));
        cx.notify();
    }

    fn pick_local_folders(&mut self, cx: &mut Context<Self>) {
        let paths = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: true,
            prompt: Some("Open".into()),
        });
        // The folder picker shows this Mac's folders.
        let machines = self.machines.clone();
        let store = Machines::local(cx).read(cx).projects().clone();
        cx.spawn(async move |_, cx| {
            let paths = match paths.await {
                Ok(Ok(Some(paths))) => paths,
                Ok(Ok(None)) | Err(_) => return,
                Ok(Err(error)) => {
                    log::error!("failed to pick folders: {error:#}");
                    return;
                }
            };
            let mut last_added = None;
            for path in paths {
                let added = store.update(cx, |store, cx| store.add_project(path, cx));
                match added.await {
                    Ok(id) => last_added = Some(id),
                    Err(error) => log::error!("failed to add the project: {error:#}"),
                }
            }
            if let Some(project) = last_added {
                cx.update(|cx| {
                    let project = ProjectKey {
                        machine: MachineId::Local,
                        project,
                    };
                    reveal_project(&machines, project, cx)
                });
            }
        })
        .detach();
    }

    fn toggle_project_switcher(
        &mut self,
        _: &ToggleProjectSwitcher,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.switcher_handle.toggle(window, cx);
    }

    fn render_title_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = cx.theme().colors();
        let tabs_border = colors.border;
        let machines = self.machines.read(cx);
        let project_info = ProjectInfoStore::global(cx).read(cx);
        let scope_group = match machines.scope(cx) {
            Scope::Group(key) => machines.group(&key, cx),
            Scope::All => None,
        };
        let scope_machines = scope_group
            .as_ref()
            .filter(|group| group.machines().len() > 1)
            .and_then(|group| machines.group_machines_label(group, cx));
        let (scope_icon, scope_label): (AnyElement, SharedString) = match scope_group
            .as_ref()
            .and_then(|group| Some((group, group.primary()?)))
        {
            Some((group, (machine, project))) => (
                render_project_icon(project, project_info.info(machine, project.id), px(14.), cx),
                group.name(),
            ),
            None => (
                Icon::new(IconName::ListTree)
                    .size(IconSize::Small)
                    .color(Color::Muted)
                    .into_any_element(),
                "All projects".into(),
            ),
        };
        let shell = cx.entity().downgrade();
        let view_tabs = {
            let agents = shell.clone();
            let workspaces = shell.clone();
            ToggleButtonGroup::single_row(
                "main-view",
                [
                    ToggleButtonSimple::new("Agents", move |_, window, cx| {
                        agents
                            .update(cx, |shell, cx| shell.set_view(MainView::Agents, window, cx))
                            .ok();
                    }),
                    ToggleButtonSimple::new("Workspaces", move |_, window, cx| {
                        workspaces
                            .update(cx, |shell, cx| {
                                shell.set_view(MainView::Workspaces, window, cx)
                            })
                            .ok();
                    }),
                ],
            )
            .size(ToggleButtonGroupSize::Default)
            .auto_width()
            .selected_index(match self.view {
                MainView::Agents => 0,
                MainView::Workspaces => 1,
            })
        };
        let shows_switcher = self.view == MainView::Agents;
        // The other view's agents waiting on the user, counted beside its side of the switch.
        let badge = match self.view {
            MainView::Agents => {
                let (count, status) = self.spaces_view.read(cx).waiting(cx);
                status.map(|status| (false, count, status))
            }
            MainView::Workspaces => {
                let (count, status) = self.waiting_threads(cx);
                status.map(|status| (true, count, status))
            }
        };
        let (badge_before, badge_after) = match badge {
            Some((true, count, status)) => (Some(render_waiting_badge(count, status, cx)), None),
            Some((false, count, status)) => (None, Some(render_waiting_badge(count, status, cx))),
            None => (None, None),
        };
        let view_tabs = h_flex()
            .gap_1p5()
            .children(badge_before)
            .child(view_tabs)
            .children(badge_after);

        h_flex()
            .id("title-bar")
            .window_control_area(WindowControlArea::Drag)
            .h(TITLE_BAR_HEIGHT)
            .flex_none()
            .w_full()
            .pl(TRAFFIC_LIGHTS_WIDTH)
            .pr_3()
            .gap_2()
            .border_b_1()
            .border_color(colors.border)
            .bg(colors.title_bar_background)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, _| this.should_move_window = true),
            )
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, _, _| this.should_move_window = false),
            )
            .on_mouse_down_out(cx.listener(|this, _, _, _| this.should_move_window = false))
            .on_mouse_move(cx.listener(|this, _, window, _| {
                if this.should_move_window {
                    this.should_move_window = false;
                    window.start_window_move();
                }
            }))
            .on_click(|event, window, _| {
                if event.click_count() == 2 {
                    window.titlebar_double_click();
                }
            })
            .child(
                // Keeps a press on the button from starting a window drag.
                div()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .child(
                        IconButton::new(
                            "toggle-sidebar",
                            if is_sidebar_hidden(cx) {
                                IconName::ThreadsSidebarLeftClosed
                            } else {
                                IconName::ThreadsSidebarLeftOpen
                            },
                        )
                        .icon_size(IconSize::Small)
                        .tooltip(|_, cx| Tooltip::for_action("Toggle Sidebar", &ToggleSidebar, cx))
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.toggle_sidebar(&ToggleSidebar, window, cx)
                        })),
                    ),
            )
            .when(shows_switcher, |title_bar| {
                title_bar.child(
                    // Keeps a press on the switcher from starting a window drag.
                    div()
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .child(
                            PopoverMenu::new("project-switcher")
                                .with_handle(self.switcher_handle.clone())
                                .menu(move |window, cx| {
                                    let shell = shell.clone();
                                    let open_project_settings =
                                        move |project_id, window: &mut Window, cx: &mut App| {
                                            shell
                                                .update(cx, |shell, cx| {
                                                    shell.open_project_settings(
                                                        project_id, window, cx,
                                                    )
                                                })
                                                .ok();
                                        };
                                    Some(cx.new(|cx| {
                                        ProjectSwitcher::new(open_project_settings, window, cx)
                                    }))
                                })
                                .trigger_with_tooltip(
                                    ButtonLike::new("project-switcher-trigger").child(
                                        h_flex()
                                            .px_1()
                                            .gap_1p5()
                                            .child(scope_icon)
                                            .child(Label::new(scope_label).size(LabelSize::Small))
                                            .children(scope_machines.map(|label| {
                                                Label::new(label)
                                                    .size(LabelSize::Small)
                                                    .color(Color::Muted)
                                            }))
                                            .child(
                                                Icon::new(IconName::ChevronDown)
                                                    .size(IconSize::XSmall)
                                                    .color(Color::Muted),
                                            ),
                                    ),
                                    |_, cx| {
                                        Tooltip::for_action(
                                            "Switch Project",
                                            &ToggleProjectSwitcher,
                                            cx,
                                        )
                                    },
                                )
                                .anchor(gpui::Anchor::TopLeft)
                                .offset(gpui::point(px(0.), px(4.))),
                        ),
                )
            })
            .child(div().flex_1())
            .children(self.render_connection_status(cx))
            .child(
                // Keeps a press on the tabs from starting a window drag. Outlined in the
                // title bar's own border color: Zed's outlined group fades it to near nothing.
                div()
                    .rounded_md()
                    .border_1()
                    .border_color(tabs_border)
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .child(view_tabs),
            )
    }

    /// Each machine that can't be reached or runs an older server, by its own icon with a dot
    /// for what it needs: accent for an update, warning for attention, dim while it
    /// reconnects. A click opens Settings › Machines.
    fn render_connection_status(&self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let colors = cx.theme().colors();
        let title_bar_background = colors.title_bar_background;
        let accent = colors.text_accent;
        let disabled = colors.icon_disabled;
        let warning = cx.theme().status().warning;
        let machines = self.machines.read(cx);
        let mut icons = Vec::new();
        for client in machines.clients() {
            let client = client.read(cx);
            let label = client.label().clone();
            let (summary, detail, dot, icon_color) = match client.status() {
                MachineStatus::Connecting => continue,
                MachineStatus::Online if client.is_outdated() => (
                    "Update available",
                    "Runs an older agentz-server".to_string(),
                    accent,
                    Color::Muted,
                ),
                MachineStatus::Online => continue,
                MachineStatus::Reconnecting(error) => (
                    "Reconnecting…",
                    format!("Disconnected: {error}"),
                    disabled,
                    Color::Disabled,
                ),
                MachineStatus::Attention { error, .. } => (
                    "Needs attention",
                    format!("Disconnected: {error}"),
                    warning,
                    Color::Muted,
                ),
            };
            let title: SharedString = format!("{label} · {summary}").into();
            let detail: SharedString = detail.into();
            icons.push(
                div()
                    .id(SharedString::from(format!(
                        "machine-status-{}",
                        client.machine().slug()
                    )))
                    .relative()
                    .p_0p5()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .cursor_pointer()
                    .child(
                        Icon::new(machines.machine_icon(client.machine(), cx))
                            .size(IconSize::Small)
                            .color(icon_color),
                    )
                    .child(
                        div()
                            .absolute()
                            .bottom_0()
                            .right_0()
                            .size(px(7.))
                            .rounded_full()
                            .border_1()
                            .border_color(title_bar_background)
                            .bg(dot),
                    )
                    .tooltip(move |_, cx| {
                        Tooltip::with_meta(title.clone(), None, detail.clone(), cx)
                    })
                    .on_click(
                        cx.listener(|this, _, window, cx| this.open_machine_settings(window, cx)),
                    )
                    .into_any_element(),
            );
        }
        icons
    }
}

impl Focusable for Shell {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

/// A count in the most urgent waiting state's color, for the view switch.
fn render_waiting_badge(count: usize, status: ThreadStatus, cx: &App) -> Div {
    let color = match status {
        ThreadStatus::PendingApproval => cx.theme().status().warning,
        _ => AWAITING_INPUT_COLOR.color(cx),
    };
    div()
        .min_w(px(16.))
        .h(px(16.))
        .px_1()
        .rounded_full()
        .bg(color)
        .flex()
        .items_center()
        .justify_center()
        .child(
            Label::new(count.to_string())
                .size(LabelSize::XSmall)
                .line_height_style(LineHeightStyle::UiLabel)
                .weight(gpui::FontWeight::BOLD)
                .color(Color::Custom(cx.theme().colors().editor_background)),
        )
}

impl Render for Shell {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let background = cx.theme().colors().background;
        let text_color = cx.theme().colors().text;
        let main_background = cx.theme().colors().editor_background;
        let settings_page = self.settings_page.as_ref().map(|(page, _)| page.clone());
        let is_dialog = self.machine_modal.is_some() || self.confirm_dialog.is_some();
        // Beside the thread, or filling its area when full screen.
        let diff_panel = self.diff_panel.clone();
        let is_diff_full_screen = self.diff_full_screen && diff_panel.is_some();
        let border = cx.theme().colors().border;
        let active_view = self
            .active_thread
            .and_then(|thread_id| self.open_threads.get(&thread_id))
            .map(|open_thread| open_thread.view.clone());
        let shows_workspaces = self.view == MainView::Workspaces && settings_page.is_none();

        v_flex()
            .key_context("Shell")
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(Self::open_folder))
            .on_action(cx.listener(Self::toggle_project_switcher))
            .on_action(cx.listener(Self::new_thread))
            .on_action(cx.listener(Self::open_settings))
            .on_action(cx.listener(Self::toggle_diff))
            .on_action(cx.listener(Self::toggle_sidebar))
            .on_action(cx.listener(Self::toggle_terminal_drawer))
            .on_drag_move(
                cx.listener(|this, event: &DragMoveEvent<DraggedDiffEdge>, _, cx| {
                    let available = event.bounds.size.width - SIDEBAR_WIDTH - MIN_THREAD_WIDTH;
                    let width = (event.bounds.right() - event.event.position.x)
                        .min(available)
                        .max(MIN_DIFF_PANEL_WIDTH);
                    if this.diff_width != width {
                        this.diff_width = width;
                        cx.notify();
                    }
                }),
            )
            .relative()
            .size_full()
            .bg(background)
            .text_color(text_color)
            .font_ui(cx)
            .text_ui(cx)
            .child(self.render_title_bar(cx))
            .when(shows_workspaces, |shell| {
                shell.child(div().flex_1().min_h_0().child(self.spaces_view.clone()))
            })
            .when(!shows_workspaces, |shell| {
                shell.child(
                    h_flex()
                        .flex_1()
                        .min_h_0()
                        // Settings brings its own navigation in place of the thread list.
                        .when(settings_page.is_none() && !is_sidebar_hidden(cx), |row| {
                            row.child(self.sidebar.clone())
                        })
                        .when(!is_diff_full_screen, |row| {
                            row.child(
                                div()
                                    .flex_1()
                                    .min_w(SIDEBAR_WIDTH)
                                    .h_full()
                                    .bg(main_background)
                                    .map(|main| match (settings_page, active_view) {
                                        (Some(page), _) => main.child(page),
                                        (None, Some(view)) => main.child(view.into_any_element()),
                                        (None, None) => main.child(render_no_thread_selected()),
                                    }),
                            )
                        })
                        .when_some(diff_panel, |row, panel| {
                            row.child(
                                div()
                                    .relative()
                                    .map(|this| {
                                        if is_diff_full_screen {
                                            this.flex_1().min_w_0()
                                        } else {
                                            this.w(self.diff_width).flex_none()
                                        }
                                    })
                                    .h_full()
                                    .border_l_1()
                                    .border_color(border)
                                    .child(panel)
                                    // The left edge drags to resize, as Zed's docks do.
                                    .when(!is_diff_full_screen, |this| {
                                        this.child(
                                            div()
                                                .id("diff-resize-edge")
                                                .absolute()
                                                .left(-RESIZE_EDGE_SIZE / 2.)
                                                .top_0()
                                                .h_full()
                                                .w(RESIZE_EDGE_SIZE)
                                                .cursor_col_resize()
                                                .on_mouse_down(MouseButton::Left, |_, _, cx| {
                                                    cx.stop_propagation()
                                                })
                                                .on_drag(DraggedDiffEdge, |_, _, _, cx| {
                                                    cx.new(|_| gpui::Empty)
                                                }),
                                        )
                                    }),
                            )
                        }),
                )
            })
            .when_some(
                self.new_thread_modal
                    .as_ref()
                    .map(|(modal, _)| AnyView::from(modal.clone()))
                    .or_else(|| {
                        self.add_project_modal
                            .as_ref()
                            .map(|(modal, _)| AnyView::from(modal.clone()))
                    })
                    .or_else(|| {
                        self.worktree_modal
                            .as_ref()
                            .map(|(modal, _)| AnyView::from(modal.clone()))
                    })
                    .or_else(|| {
                        self.machine_modal
                            .as_ref()
                            .map(|(modal, _)| AnyView::from(modal.clone()))
                    })
                    .or_else(|| {
                        self.confirm_dialog
                            .as_ref()
                            .map(|(dialog, _)| AnyView::from(dialog.clone()))
                    }),
                |shell, modal| {
                    shell.child(
                        div()
                            .id("modal-backdrop")
                            .absolute()
                            .inset_0()
                            .flex()
                            .justify_center()
                            // Pickers sit near the top as Zed's; a dialog is centered, as
                            // t3code's.
                            .map(|backdrop| {
                                if is_dialog {
                                    backdrop.items_center()
                                } else {
                                    backdrop.pt(px(96.))
                                }
                            })
                            .bg(gpui::black().opacity(0.25))
                            // As Zed's modal layer: nothing under the backdrop gets the mouse
                            // (the title bar would start a window drag and swallow the click),
                            // and pressing on it dismisses at once.
                            .occlude()
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _, window, cx| this.dismiss_modal(window, cx)),
                            )
                            .child(
                                // Presses inside the modal must not reach the backdrop.
                                div()
                                    .id("modal-container")
                                    .on_mouse_down(MouseButton::Left, |_, _, cx| {
                                        cx.stop_propagation()
                                    })
                                    .child(modal),
                            ),
                    )
                },
            )
    }
}

fn render_no_thread_selected() -> impl IntoElement {
    v_flex()
        .size_full()
        .items_center()
        .justify_center()
        .gap_2()
        .child(Label::new("Select a thread, or start a new one").color(Color::Muted))
        .child(
            Button::new("start-thread", "New Thread")
                .style(ButtonStyle::Outlined)
                .on_click(|_, window, cx| window.dispatch_action(Box::new(NewThread), cx)),
        )
}

/// In "All projects" a new project simply appears; otherwise this switches to it so it isn't
/// added out of sight.
fn reveal_project(machines: &Entity<Machines>, project: ProjectKey, cx: &mut App) {
    let machines = machines.read(cx);
    if machines.scope(cx) != Scope::All
        && let Some(group) = machines.group_of(project.machine, project.project, cx)
    {
        Machines::set_scope(Scope::Group(group.key), cx);
    }
}

fn notification_tag(thread: ThreadKey) -> SharedString {
    format!("thread-{}-{}", thread.machine.slug(), thread.thread.0).into()
}

/// The thread a notification is about, from its tag.
fn thread_from_notification_tag(tag: &str) -> Option<ThreadKey> {
    let (machine, thread) = tag.strip_prefix("thread-")?.rsplit_once('-')?;
    Some(ThreadKey {
        machine: MachineId::from_slug(machine)?,
        thread: projects::ThreadId(thread.parse().ok()?),
    })
}

#[cfg(test)]
mod tests {
    use super::{notification_tag, thread_from_notification_tag};
    use crate::machines::{MachineId, ThreadKey};
    use projects::ThreadId;

    #[test]
    fn notification_tags_round_trip() {
        for machine in [MachineId::Local, MachineId::Remote(12)] {
            let thread = ThreadKey {
                machine,
                thread: ThreadId(7),
            };
            assert_eq!(
                thread_from_notification_tag(&notification_tag(thread)),
                Some(thread)
            );
        }
        assert_eq!(thread_from_notification_tag("thread-7"), None);
    }
}

#[cfg(test)]
mod modal_tests {
    use agentz_protocol::spaces::SpacesSnapshot;
    use gpui::{Modifiers, TestAppContext, point};
    use projects::{Project, ProjectId, ProjectsSnapshot};

    use super::*;
    use crate::server_client::ServerClient;

    #[gpui::test]
    fn clicking_outside_new_thread_closes_it(cx: &mut TestAppContext) {
        cx.update(|cx| {
            crate::init_for_test(cx);
            let client = ServerClient::new_for_test(
                MachineId::Local,
                "This Mac".into(),
                SpacesSnapshot::default(),
                cx,
            );
            let projects = client.read(cx).projects().clone();
            projects.update(cx, |store, cx| {
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
                        ..Default::default()
                    },
                    cx,
                )
            });
            crate::machines::init_for_test(vec![client], cx);
            crate::project_info::init(cx);
            crate::sidebar::init(cx);
            crate::new_thread_modal::init(cx);
        });
        let (shell, cx) = cx.add_window_view(|window, cx| Shell::new(window, cx));
        shell.update_in(cx, |shell, window, cx| {
            shell.open_new_thread_modal(window, cx)
        });
        cx.run_until_parked();
        assert!(shell.read_with(cx, |shell, _| shell.new_thread_modal.is_some()));

        // Inside the modal, it stays.
        let width = cx.update(|window, _| window.viewport_size().width);
        cx.simulate_click(point(width / 2., px(130.)), Modifiers::none());
        cx.run_until_parked();
        assert!(shell.read_with(cx, |shell, _| shell.new_thread_modal.is_some()));

        // Over the title bar, which drags the window.
        cx.simulate_click(point(px(300.), px(15.)), Modifiers::none());
        cx.run_until_parked();
        assert!(shell.read_with(cx, |shell, _| shell.new_thread_modal.is_none()));
        shell.update_in(cx, |shell, window, cx| {
            shell.open_new_thread_modal(window, cx)
        });
        // Well away from the modal, which sits at the top middle.
        cx.simulate_click(point(px(20.), px(500.)), Modifiers::none());
        cx.run_until_parked();
        assert!(shell.read_with(cx, |shell, _| shell.new_thread_modal.is_none()));
    }

    #[gpui::test]
    fn new_thread_reuses_the_open_draft_until_something_is_typed(cx: &mut TestAppContext) {
        cx.update(|cx| {
            crate::init_for_test(cx);
            let client = ServerClient::new_for_test(
                MachineId::Local,
                "This Mac".into(),
                SpacesSnapshot::default(),
                cx,
            );
            client.update(cx, |client, cx| client.set_online_for_test(cx));
            let projects = client.read(cx).projects().clone();
            let draft: projects::Thread = serde_json::from_value(serde_json::json!({
                "id": 5,
                "project_id": 1,
                "title": "New thread",
                "agent_id": "mock",
                "is_draft": true,
            }))
            .expect("a thread");
            projects.update(cx, |store, cx| {
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
                        threads: vec![draft],
                        ..Default::default()
                    },
                    cx,
                )
            });
            crate::machines::init_for_test(vec![client], cx);
            crate::project_info::init(cx);
            crate::sidebar::init(cx);
        });
        let (shell, cx) = cx.add_window_view(|window, cx| Shell::new(window, cx));
        let draft = ThreadKey {
            machine: MachineId::Local,
            thread: projects::ThreadId(5),
        };
        shell.update_in(cx, |shell, window, cx| shell.open_thread(draft, window, cx));
        cx.run_until_parked();

        // No agent is installed, so a new draft would send the user to Settings › Agents.
        shell.update_in(cx, |shell, window, cx| {
            shell.new_thread(&NewThread, window, cx)
        });
        cx.run_until_parked();
        shell.read_with(cx, |shell, _| {
            assert_eq!(shell.active_thread, Some(draft));
            assert!(shell.settings_page.is_none());
        });

        shell.update_in(cx, |shell, _, cx| {
            let Some(OpenThread {
                view: ThreadView::Agent(view),
                ..
            }) = shell.open_threads.get(&draft)
            else {
                panic!("the draft is open");
            };
            view.update(cx, |view, cx| view.set_composer_text("Fix it".into(), cx));
        });
        shell.update_in(cx, |shell, window, cx| {
            shell.new_thread(&NewThread, window, cx)
        });
        cx.run_until_parked();
        assert!(shell.read_with(cx, |shell, _| shell.settings_page.is_some()));
    }

    #[gpui::test]
    fn cmd_b_hides_and_shows_the_sidebar(cx: &mut TestAppContext) {
        cx.update(|cx| {
            crate::init_for_test(cx);
            let client = ServerClient::new_for_test(
                MachineId::Local,
                "This Mac".into(),
                SpacesSnapshot::default(),
                cx,
            );
            crate::machines::init_for_test(vec![client], cx);
            crate::project_info::init(cx);
            crate::sidebar::init(cx);
            cx.bind_keys([gpui::KeyBinding::new("cmd-b", ToggleSidebar, None)]);
        });
        let (shell, cx) = cx.add_window_view(|window, cx| Shell::new(window, cx));
        shell.update_in(cx, |shell, window, cx| {
            window.focus(&shell.focus_handle, cx)
        });
        let hidden = |cx: &mut gpui::VisualTestContext| cx.update(|_, cx| is_sidebar_hidden(cx));
        assert!(!hidden(cx));
        cx.simulate_keystrokes("cmd-b");
        assert!(hidden(cx));
        cx.simulate_keystrokes("cmd-b");
        assert!(!hidden(cx));
    }
}
