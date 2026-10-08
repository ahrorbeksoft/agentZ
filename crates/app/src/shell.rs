use std::path::PathBuf;

use crate::machines::{MachineId, Machines, MachinesEvent, ProjectKey, Scope, ThreadKey};
use crate::project_store::ThreadStatus;
use agentz_protocol::accounts::AccountChoice;
use agentz_protocol::agents::{AgentId, InstallState};
use agentz_protocol::layout::Node;
use agentz_protocol::workspace::WorkspaceChoice;
use anyhow::Result;
use collections::HashMap;
use gpui::{
    AnyView, App, Context, Decorations, DismissEvent, DragMoveEvent, Entity, EventEmitter,
    FocusHandle, Focusable, Hsla, MouseButton, PathPromptOptions, Subscription, SystemNotification,
    Task, Window, WindowControlArea,
};
use projects::{Thread, ThreadId};
use theme::ThemeColors;
use ui::{ButtonLike, PopoverMenu, PopoverMenuHandle, Tooltip, prelude::*};
use util::ResultExt as _;

use crate::add_project_modal::{AddProjectModal, AddProjectModalEvent};
use crate::agent_view::{AgentView, AgentViewEvent, RESIZE_EDGE_SIZE};
use crate::app_settings::{AppSettingsStore, MachineProfile, is_sidebar_hidden};
use crate::command_palette::CommandPalette;
use crate::confirm_dialog::{ConfirmDialog, ConfirmRequest};
use crate::diff_panel::{DIFF_PANEL_WIDTH, DiffPanel, DiffPanelEvent};
use crate::go_to_picker::{GoToPicker, Place, thread_places};
use crate::machine_modal::MachineModal;
use crate::new_thread_modal::{NewThreadModal, NewThreadModalEvent};
use crate::project_info::render_project_icon;
use crate::project_switcher::ProjectSwitcher;
use crate::save_layout_modal::{LayoutPane, SaveLayoutModal};
use crate::server_client::MachineStatus;
use crate::settings_page::{AccountDialog, SettingsPage, SettingsPageEvent};
use crate::shortcut_sheet::ShortcutSheet;
use crate::sidebar::{AWAITING_INPUT_COLOR, SIDEBAR_WIDTH, Sidebar, SidebarEvent};
use crate::sound;
use crate::spaces_view::{self, PaneKey, SpacesView, SpacesViewEvent};
use crate::terminal_thread_view::TerminalThreadView;
use crate::thread_entity::AgentThread;
use crate::welcome::{Section, SectionButton, render_welcome};
use crate::window_decorations::{self, RoundedTopCorners as _, Side};
use crate::worktree_modal::{WorktreeModal, WorktreeModalEvent, WorktreeModalMode};
use crate::{
    GoTo, NewThread, OpenFolder, OpenSettings, ShowShortcuts, ToggleCommandPalette, ToggleDiff,
    ToggleProjectSwitcher, ToggleSidebar, ToggleTerminalDrawer,
};

pub const KEY_CONTEXT: &str = "Shell";
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

/// The project a draft opens in when the Agents view has no thread to show.
#[derive(Clone, Copy, Debug, PartialEq)]
enum DraftLanding {
    /// That of the sidebar's first thread.
    Latest,
    /// This one while it's there, or else as `Latest`.
    In(ProjectKey),
}

/// The thread shown, or one whose view holds messages queued for its agent.
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

/// The modals opened over whatever has focus, which give it back when they close.
#[derive(Clone, Copy, Debug, PartialEq)]
enum OverlayKind {
    ShortcutSheet,
    CommandPalette,
    GoTo,
    SaveLayout,
}

struct Overlay {
    kind: OverlayKind,
    view: AnyView,
    /// What had focus, which gets it back, as Zed's modal layer gives it back.
    previous_focus: Option<FocusHandle>,
    _subscription: Subscription,
}

pub struct Shell {
    focus_handle: FocusHandle,
    machines: Entity<Machines>,
    view: MainView,
    sidebar: Entity<Sidebar>,
    spaces_view: Entity<SpacesView>,
    /// A draft New Thread is making, so another press doesn't make a second.
    _starting_draft: Option<Task<()>>,
    switcher_handle: PopoverMenuHandle<ProjectSwitcher>,
    new_thread_modal: Option<(Entity<NewThreadModal>, Vec<Subscription>)>,
    add_project_modal: Option<(Entity<AddProjectModal>, Vec<Subscription>)>,
    worktree_modal: Option<(Entity<WorktreeModal>, Vec<Subscription>)>,
    machine_modal: Option<(Entity<MachineModal>, Subscription)>,
    confirm_dialog: Option<(Entity<ConfirmDialog>, Subscription)>,
    /// An agent's account dialog, over Settings. A confirmation it asks for shows in its place.
    account_dialog: Option<(Entity<AccountDialog>, Subscription)>,
    overlay: Option<Overlay>,
    /// Shown in the main area in place of the thread while open.
    settings_page: Option<(Entity<SettingsPage>, Subscription)>,
    open_threads: HashMap<ThreadKey, OpenThread>,
    active_thread: Option<ThreadKey>,
    /// The active thread's project, kept for after the thread is deleted.
    active_project: Option<ProjectKey>,
    /// A draft to open once the Agents view has no thread to show and one can start, as
    /// t3code's index route drops into one: at launch, and after the open thread is deleted.
    pending_draft: Option<DraftLanding>,
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
                    SpacesViewEvent::NewThreadInPane { pane, folder } => {
                        this.start_pane_draft(*pane, folder.clone(), window, cx)
                    }
                    SpacesViewEvent::NewThread { project, folder } => {
                        this.start_draft(*project, Some(folder.clone()), window, cx)
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
                    SpacesViewEvent::SaveLayout { root, panes, name } => {
                        this.open_save_layout(root.clone(), panes.clone(), name.clone(), window, cx)
                    }
                    SpacesViewEvent::OpenAgentSettings => this.open_agent_settings(window, cx),
                    SpacesViewEvent::OpenAgentAccounts {
                        machine,
                        agent_id,
                        add_account,
                    } => this.open_agent_accounts(*machine, agent_id, *add_account, window, cx),
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
                    // As t3code after deleting the open thread: the project's next thread, or
                    // else a draft there. After the sidebar is told above.
                    let project = this.active_project.take();
                    let next = project.and_then(|project| this.latest_thread_in(project, cx));
                    cx.defer_in(window, move |this, window, cx| {
                        if this.active_thread.is_some() {
                            return;
                        }
                        match next.filter(|_| this.shows_agents()) {
                            Some(next) => this.open_thread(next, window, cx),
                            None => {
                                this.pending_draft =
                                    Some(project.map_or(DraftLanding::Latest, DraftLanding::In));
                                this.open_pending_draft(window, cx);
                            }
                        }
                    });
                } else if let Some(thread) = this.active_thread {
                    // A terminal thread belongs to where its shell is now.
                    this.active_project = this.project_of(thread, cx);
                }
                this.open_pending_draft(window, cx);
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
                    MachinesEvent::PaneNeedsAttention(pane, status) => {
                        this.notify_pane_attention(*pane, *status, window, cx)
                    }
                    MachinesEvent::Archiving(thread) => {
                        this.open_draft_after_archiving(*thread, window, cx)
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
            // With the theme mode set to System, the theme follows macOS's appearance.
            cx.observe_window_appearance(window, |_, _, cx| {
                AppSettingsStore::global(cx).update(cx, |store, cx| store.reapply_theme(cx));
            }),
        ];
        // Clicking a notification is the user asking for that thread, so it may come forward.
        let shell = cx.entity().downgrade();
        let window_handle = window.window_handle();
        cx.on_system_notification_response(move |response, cx| {
            let place = match thread_from_notification_tag(&response.tag) {
                Some(thread_id) => Place::Thread(thread_id),
                None => match PaneKey::from_notification_tag(&response.tag) {
                    Some(pane) => Place::Pane(pane),
                    None => return,
                },
            };
            let shell = shell.clone();
            window_handle
                .update(cx, |_, window, cx| {
                    window.activate_window();
                    shell
                        .update(cx, |shell, cx| shell.go_to(place, window, cx))
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
            _starting_draft: None,
            switcher_handle: PopoverMenuHandle::default(),
            new_thread_modal: None,
            add_project_modal: None,
            worktree_modal: None,
            machine_modal: None,
            confirm_dialog: None,
            account_dialog: None,
            overlay: None,
            settings_page: None,
            open_threads: HashMap::default(),
            active_thread: None,
            active_project: None,
            pending_draft: Some(DraftLanding::Latest),
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

    /// The project the sidebar lists a thread under.
    fn project_of(&self, key: ThreadKey, cx: &App) -> Option<ProjectKey> {
        let thread = self.thread(key, cx)?;
        let project = self
            .machines
            .read(cx)
            .thread_project(key.machine, &thread, cx)?;
        Some(ProjectKey {
            machine: key.machine,
            project,
        })
    }

    /// The project's first thread in the sidebar, which t3code opens after deleting the open
    /// thread.
    fn latest_thread_in(&self, project: ProjectKey, cx: &App) -> Option<ThreadKey> {
        let machines = self.machines.read(cx);
        let group = machines.group_of(project.machine, project.project, cx)?;
        machines
            .active_threads(cx)
            .into_iter()
            .find_map(|(machine, thread)| {
                let project = machines.thread_project(machine, &thread, cx)?;
                group.contains(machine, project).then_some(ThreadKey {
                    machine,
                    thread: thread.id,
                })
            })
    }

    /// The project of the sidebar's first thread, as t3code lands in its most recently active
    /// project, or else the first project shown.
    fn latest_project(&self, cx: &App) -> Option<ProjectKey> {
        let machines = self.machines.read(cx);
        machines
            .active_threads(cx)
            .into_iter()
            .find_map(|(machine, thread)| {
                let project = machines.thread_project(machine, &thread, cx)?;
                machines
                    .is_online(machine, cx)
                    .then_some(ProjectKey { machine, project })
            })
            .or_else(|| {
                machines
                    .visible_groups(cx)
                    .iter()
                    .find_map(|group| machines.new_thread_member(group, cx))
            })
    }

    /// Whether the Agents view's thread area is on screen, rather than Workspaces or Settings.
    fn shows_agents(&self) -> bool {
        self.view == MainView::Agents && self.settings_page.is_none()
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
            Some(project) => self.start_draft(project, None, window, cx),
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
                    this.dismiss_modal(window, cx);
                    this.start_draft(*project, None, window, cx);
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
            self.open_thread(draft, window, cx);
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
            store.create_thread(
                project.project,
                agent_id,
                choice,
                AccountChoice::Default,
                cx,
            )
        });
        self.show_draft_when_made(project.machine, created, None, window, cx);
    }

    /// Opens the pending draft once the Agents view shows no thread and one can start: the
    /// session has arrived, there's a project, and its machine has an agent. Until then it
    /// waits, so the first project, or the first agent installed, opens it.
    fn open_pending_draft(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(landing) = self.pending_draft else {
            return;
        };
        if !self.shows_agents() || self.active_thread.is_some() || self._starting_draft.is_some() {
            return;
        }
        if !Machines::local(cx)
            .read(cx)
            .projects()
            .read(cx)
            .has_snapshot()
        {
            return;
        }
        let machines = self.machines.read(cx);
        let project = match landing {
            DraftLanding::In(project)
                if machines.is_online(project.machine, cx)
                    && machines
                        .projects(project.machine, cx)
                        .is_some_and(|store| store.read(cx).project(project.project).is_some()) =>
            {
                Some(project)
            }
            _ => self.latest_project(cx),
        };
        let Some(project) =
            project.filter(|project| self.default_agent(project.machine, cx).is_some())
        else {
            return;
        };
        self.pending_draft = None;
        self.start_draft(project, None, window, cx);
    }

    /// Archiving the open thread here opens a draft in its project, as t3code's does. The
    /// thread stays on screen, read-only, until the draft is ready, and without an agent.
    fn open_draft_after_archiving(
        &mut self,
        thread: ThreadKey,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.active_thread != Some(thread) || !self.shows_agents() {
            return;
        }
        if let Some(project) = self
            .project_of(thread, cx)
            .filter(|project| self.default_agent(project.machine, cx).is_some())
        {
            self.start_draft(project, None, window, cx);
        }
    }

    /// New Thread… in a pane: a draft of a Workspaces thread working in `folder`, shown in the
    /// pane, with the agent used last.
    fn start_pane_draft(
        &mut self,
        pane: PaneKey,
        folder: PathBuf,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(store) = self.machines.read(cx).projects(pane.machine, cx) else {
            return;
        };
        let Some(agent_id) = self.default_agent(pane.machine, cx) else {
            self.open_agent_settings(window, cx);
            return;
        };
        if self._starting_draft.is_some() {
            return;
        }
        let created = store.update(cx, |store, cx| {
            store.create_workspaces_thread(
                folder,
                agent_id,
                WorkspaceChoice::Checkout,
                AccountChoice::Default,
                cx,
            )
        });
        self.show_draft_when_made(pane.machine, created, Some(pane), window, cx);
    }

    fn show_draft_when_made(
        &mut self,
        machine: MachineId,
        created: Task<Result<ThreadId>>,
        pane: Option<PaneKey>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self._starting_draft = Some(cx.spawn_in(window, async move |this, cx| {
            let created = created.await;
            this.update_in(cx, |this, window, cx| {
                this._starting_draft = None;
                match created {
                    Ok(thread) => {
                        let draft = ThreadKey { machine, thread };
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

    /// Add Account… and Manage Accounts…, from a new thread's account picker.
    fn open_agent_accounts(
        &mut self,
        machine: MachineId,
        agent_id: &AgentId,
        add_account: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.open_settings(&OpenSettings, window, cx);
        if let Some((page, _)) = &self.settings_page {
            page.update(cx, |page, cx| {
                page.show_agent_accounts(machine, agent_id, add_account, window, cx)
            });
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
                        SettingsPageEvent::OpenDialog(dialog) => {
                            this.open_account_dialog(dialog.clone(), window, cx)
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
        self.account_dialog = None;
        self.focus_main(window, cx);
        self.mark_active_thread_viewed(window, cx);
        self.sync_diff_panel(cx);
        self.open_pending_draft(window, cx);
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
        // Leaving a thread closes its view, saving what's typed: the server runs its agent
        // without one, sends its queued messages, and stops it once no window has had the
        // thread open for a while. A draft, a new thread before its first message, is removed
        // then, unless something is typed.
        self.open_threads.retain(|key, open_thread| {
            *key == thread_id || matches!(open_thread.view, ThreadView::Terminal(_))
        });
        self.active_thread = Some(thread_id);
        self.active_project = self.project_of(thread_id, cx);
        self.pending_draft = None;
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

    /// The thread's sound, as the settings say, and a system notification while agentZ isn't
    /// focused, as t3code notifies.
    fn notify_attention(
        &self,
        thread_id: ThreadKey,
        status: ThreadStatus,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        sound::play_for_status(
            status,
            self.is_thread_visible(thread_id, window, cx),
            window.is_window_active(),
            cx,
        );
        if !should_notify(window, cx) {
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
            ThreadStatus::Working | ThreadStatus::Waiting | ThreadStatus::Completed => "Finished",
        };
        let project = store
            .thread_project(thread.id)
            .and_then(|project| store.project(project));
        let mut body = match project {
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
        window.request_attention();
    }

    /// A Workspaces pane's agent CLI finished or got blocked: herdr's sound and toast, as a
    /// thread's.
    fn notify_pane_attention(
        &self,
        key: PaneKey,
        status: ThreadStatus,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let is_visible = window.is_window_active()
            && self.settings_page.is_none()
            && self.view == MainView::Workspaces
            && self.spaces_view.read(cx).shows_pane(key, cx);
        sound::play_for_status(status, is_visible, window.is_window_active(), cx);
        if !should_notify(window, cx) {
            return;
        }
        let machines = self.machines.read(cx);
        let Some(client) = machines.client(key.machine, cx) else {
            return;
        };
        let client = client.read(cx);
        let Some((space, tab, pane)) = client.spaces().pane(key.pane) else {
            return;
        };
        let Some(agent) = &pane.agent else {
            return;
        };
        let tab_index = space
            .tabs
            .iter()
            .position(|candidate| candidate.id == tab.id)
            .unwrap_or_default();
        let caption = match status {
            ThreadStatus::PendingApproval | ThreadStatus::AwaitingInput => "Needs attention",
            ThreadStatus::Working | ThreadStatus::Waiting | ThreadStatus::Completed => "Finished",
        };
        let mut body = format!(
            "{} › {} · {caption}",
            space.label(),
            spaces_view::tab_label(tab, tab_index)
        );
        if key.machine != MachineId::Local {
            body = format!("{} · {body}", machines.label(key.machine, cx));
        }
        cx.show_system_notification(SystemNotification {
            tag: key.notification_tag(),
            title: agent.name.clone().into(),
            body: body.into(),
            actions: Vec::new(),
        });
        window.request_attention();
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
                    window,
                    cx,
                ),
                AgentViewEvent::OpenAgentSettings => this.open_agent_settings(window, cx),
                AgentViewEvent::OpenAgentAccounts {
                    agent_id,
                    add_account,
                } => this.open_agent_accounts(key.machine, agent_id, *add_account, window, cx),
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
        // A confirmation over the account dialog gives it back.
        if let Some((dialog, _)) = &self.account_dialog
            && self.confirm_dialog.take().is_some()
        {
            window.focus(&dialog.focus_handle(cx), cx);
            cx.notify();
            return;
        }
        let had_account_dialog = self.account_dialog.take().is_some();
        let had_new_thread_modal = self.new_thread_modal.take().is_some();
        let had_add_project_modal = self.add_project_modal.take().is_some();
        let had_worktree_modal = self.worktree_modal.take().is_some();
        let had_machine_modal = self.machine_modal.take().is_some();
        let had_confirm_dialog = self.confirm_dialog.take().is_some();
        let overlay = self.overlay.take();
        if let Some(focus) = overlay
            .as_ref()
            .and_then(|overlay| overlay.previous_focus.clone())
        {
            window.focus(&focus, cx);
            cx.notify();
        } else if had_new_thread_modal
            || had_add_project_modal
            || had_worktree_modal
            || had_machine_modal
            || had_confirm_dialog
            || had_account_dialog
            || overlay.is_some()
        {
            self.focus_main(window, cx);
            cx.notify();
        }
    }

    /// Whether an overlay of `kind` may open. Its key closes it when it's open, and it takes
    /// another overlay's place, focus given back first, as Zed's modals replace each other.
    fn make_room_for(
        &mut self,
        kind: OverlayKind,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        // Another modal shows first, so the overlay would take its focus unseen.
        if self.new_thread_modal.is_some()
            || self.add_project_modal.is_some()
            || self.worktree_modal.is_some()
            || self.machine_modal.is_some()
            || self.confirm_dialog.is_some()
            || self.account_dialog.is_some()
        {
            return false;
        }
        match self.overlay_kind() {
            Some(open) => {
                self.dismiss_modal(window, cx);
                open != kind
            }
            None => true,
        }
    }

    fn open_overlay<V: Render + EventEmitter<DismissEvent>>(
        &mut self,
        kind: OverlayKind,
        view: Entity<V>,
        previous_focus: Option<FocusHandle>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let subscription =
            cx.subscribe_in(&view, window, |this, _, _: &DismissEvent, window, cx| {
                this.dismiss_modal(window, cx);
            });
        self.overlay = Some(Overlay {
            kind,
            view: view.into(),
            previous_focus,
            _subscription: subscription,
        });
        cx.notify();
    }

    fn overlay_kind(&self) -> Option<OverlayKind> {
        self.overlay.as_ref().map(|overlay| overlay.kind)
    }

    /// Opens the shortcut sheet for what's focused, or closes it.
    fn toggle_shortcuts(&mut self, _: &ShowShortcuts, window: &mut Window, cx: &mut Context<Self>) {
        if !self.make_room_for(OverlayKind::ShortcutSheet, window, cx) {
            return;
        }
        let context_stack = window.context_stack();
        let previous_focus = window.focused(cx);
        let sheet = cx.new(|cx| ShortcutSheet::new(context_stack, window, cx));
        self.open_overlay(
            OverlayKind::ShortcutSheet,
            sheet,
            previous_focus,
            window,
            cx,
        );
    }

    /// Opens the command palette for what's focused, or closes it.
    fn toggle_command_palette(
        &mut self,
        _: &ToggleCommandPalette,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.make_room_for(OverlayKind::CommandPalette, window, cx) {
            return;
        }
        let Some(previous_focus) = window.focused(cx) else {
            return;
        };
        let palette = {
            let previous_focus = previous_focus.clone();
            cx.new(|cx| CommandPalette::new(previous_focus, window, cx))
        };
        self.open_overlay(
            OverlayKind::CommandPalette,
            palette,
            Some(previous_focus),
            window,
            cx,
        );
    }

    /// Opens Go To, the view on screen's places first, or closes it.
    fn toggle_go_to(&mut self, _: &GoTo, window: &mut Window, cx: &mut Context<Self>) {
        if !self.make_room_for(OverlayKind::GoTo, window, cx) {
            return;
        }
        let workspaces = self.spaces_view.read(cx).places(cx);
        let threads = thread_places(cx);
        let places = match self.view {
            MainView::Workspaces => workspaces.into_iter().chain(threads).collect(),
            MainView::Agents => threads.into_iter().chain(workspaces).collect(),
        };
        let previous_focus = window.focused(cx);
        let shell = cx.entity().downgrade();
        let picker = cx.new(|cx| {
            GoToPicker::new(
                places,
                move |place, window, cx| {
                    shell
                        .update(cx, |shell, cx| shell.go_to(place, window, cx))
                        .ok();
                },
                window,
                cx,
            )
        });
        self.open_overlay(OverlayKind::GoTo, picker, previous_focus, window, cx);
    }

    fn open_save_layout(
        &mut self,
        root: Node,
        panes: Vec<LayoutPane>,
        name: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.make_room_for(OverlayKind::SaveLayout, window, cx) {
            return;
        }
        let modal = cx.new(|cx| SaveLayoutModal::new(root, panes, name, window, cx));
        // From the tab's menu, what has focus is the menu, which is gone once it closes; the
        // tab's pane gets it back instead.
        self.open_overlay(OverlayKind::SaveLayout, modal, None, window, cx);
    }

    /// Shows the place in its view and focuses it.
    fn go_to(&mut self, place: Place, window: &mut Window, cx: &mut Context<Self>) {
        if let Place::Thread(thread) = place {
            self.open_thread(thread, window, cx);
            return;
        }
        self.set_view(MainView::Workspaces, window, cx);
        self.spaces_view.update(cx, |view, cx| match place {
            Place::Space(space) => view.activate_space(space, window, cx),
            Place::Tab(tab) => view.activate_tab(tab, window, cx),
            Place::Pane(pane) => view.focus_pane(pane, window, cx),
            Place::Thread(_) => {}
        });
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
        self.open_pending_draft(window, cx);
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

    /// The settings page's account dialog, which the page focuses as it opens it.
    fn open_account_dialog(
        &mut self,
        dialog: Entity<AccountDialog>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let subscription =
            cx.subscribe_in(&dialog, window, |this, _, _: &DismissEvent, window, cx| {
                this.dismiss_modal(window, cx);
            });
        self.account_dialog = Some((dialog, subscription));
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

    fn render_title_bar(&self, window: &Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = cx.theme().colors();
        let tabs_border = colors.border;
        let machines = self.machines.read(cx);
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
                render_project_icon(machine, project, px(14.), cx),
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
        // Zed's toggle buttons, with the selected side a lighter gray rather than tinted with
        // the accent (the user's pick in `design/jetbrains/`).
        let selected_background = view_switch_selected_background(colors);
        let view_tab = |index: usize, label: &'static str, view: MainView| {
            let shell = shell.clone();
            let is_selected = self.view == view;
            div()
                .when(is_selected, |tab| tab.bg(selected_background))
                .child(
                    ButtonLike::new(("main-view", index))
                        .toggle_state(is_selected)
                        .style(if is_selected {
                            ButtonStyle::Transparent
                        } else {
                            ButtonStyle::Subtle
                        })
                        .child(div().px_2().child(Label::new(label).size(LabelSize::Small)))
                        .on_click(move |_, window, cx| {
                            shell
                                .update(cx, |shell, cx| shell.set_view(view, window, cx))
                                .ok();
                        }),
                )
        };
        let view_tabs = h_flex()
            .rounded_md()
            .overflow_hidden()
            .gap_px()
            .child(view_tab(0, "Agents", MainView::Agents))
            .child(view_tab(1, "Workspaces", MainView::Workspaces));
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

        let decorations = window.window_decorations();
        let supported_controls = window.window_controls();
        let right_controls = (!window.is_fullscreen())
            .then(|| window_decorations::window_controls(Side::Right, window, cx))
            .flatten();
        // Zed's: on Linux, an inactive window's title bar, and one being dragged, dims.
        let title_bar_background = if cfg!(target_os = "macos")
            || (window.is_window_active() && !self.should_move_window)
        {
            colors.title_bar_background
        } else {
            colors.title_bar_inactive_background
        };

        h_flex()
            .id("title-bar")
            .window_control_area(WindowControlArea::Drag)
            .h(TITLE_BAR_HEIGHT)
            .flex_none()
            .w_full()
            // Full screen hides the traffic lights, so nothing needs their room (as in Zed).
            // Linux has window controls where the desktop puts them, when agentZ draws them.
            .map(|title_bar| {
                if window.is_fullscreen() {
                    title_bar.pl_2()
                } else if cfg!(target_os = "macos") {
                    title_bar.pl(TRAFFIC_LIGHTS_WIDTH)
                } else if let Some(controls) =
                    window_decorations::window_controls(Side::Left, window, cx)
                {
                    title_bar.child(controls)
                } else {
                    title_bar.pl_2()
                }
            })
            .when(right_controls.is_none(), |title_bar| title_bar.pr_3())
            .gap_2()
            .border_b_1()
            .border_color(colors.border)
            .bg(title_bar_background)
            .map(|title_bar| match decorations {
                Decorations::Client { tiling } => title_bar.rounded_top_corners(tiling).when(
                    supported_controls.window_menu,
                    |title_bar| {
                        title_bar.on_mouse_down(MouseButton::Right, |event, window, _| {
                            window.show_window_menu(event.position)
                        })
                    },
                ),
                Decorations::Server => title_bar,
            })
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
            .on_click(move |event, window, _| {
                if event.click_count() != 2 {
                    return;
                }
                if cfg!(target_os = "macos") {
                    window.titlebar_double_click();
                } else if supported_controls.maximize && window.is_resizable() {
                    window.zoom_window();
                }
            })
            .child(
                // Keeps a press on the button from starting a window drag.
                div()
                    .debug_selector(|| "toggle-sidebar".into())
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
            .children(right_controls)
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

    /// The Agents view with no thread open. Before the first project, Zed's Welcome page.
    /// Otherwise a draft is opening, as t3code's index route opens one, so nothing shows
    /// meanwhile; with no agent to start it in, what to do.
    fn render_no_thread(&self, cx: &mut Context<Self>) -> AnyElement {
        let has_session = Machines::local(cx)
            .read(cx)
            .projects()
            .read(cx)
            .has_snapshot();
        if !has_session || self._starting_draft.is_some() {
            return div().into_any_element();
        }
        if self.machines.read(cx).project_groups(cx).is_empty() {
            return self.render_welcome(cx).into_any_element();
        }
        render_no_thread_selected().into_any_element()
    }

    fn render_welcome(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let focus = &self.focus_handle;
        let has_agent = self
            .machines
            .read(cx)
            .clients()
            .iter()
            .any(|client| self.default_agent(client.read(cx).machine(), cx).is_some());
        let get_started = Section::new("Get Started")
            .button(SectionButton::for_action(
                "Open Folder…",
                IconName::FolderOpen,
                &OpenFolder,
                focus,
                cx,
            ))
            .when(!has_agent, |section| {
                section.button(SectionButton::new(
                    "Install an Agent…",
                    IconName::Sparkle,
                    cx.listener(|this, _, window, cx| this.open_agent_settings(window, cx)),
                ))
            })
            .button(SectionButton::new(
                "Add Machine…",
                IconName::Server,
                cx.listener(|this, _, window, cx| this.open_machine_modal(None, window, cx)),
            ))
            .button(SectionButton::for_action(
                "Settings",
                IconName::Settings,
                &OpenSettings,
                focus,
                cx,
            ));
        render_welcome("agents-welcome", "Welcome to agentZ", [get_started])
    }
}

impl Focusable for Shell {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

/// The view switch's selected side: the theme's text, faint, over the title bar. Themes may
/// give elements the title bar's own color (Catppuccin does), but their text always stands
/// out from it.
fn view_switch_selected_background(colors: &ThemeColors) -> Hsla {
    colors.text.opacity(0.12)
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
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let background = cx.theme().colors().background;
        let text_color = cx.theme().colors().text;
        let main_background = cx.theme().colors().editor_background;
        let settings_page = self.settings_page.as_ref().map(|(page, _)| page.clone());
        let is_dialog = self.machine_modal.is_some()
            || self.confirm_dialog.is_some()
            || self.account_dialog.is_some()
            || self.overlay_kind() == Some(OverlayKind::SaveLayout);
        // Beside the thread, or filling its area when full screen.
        let diff_panel = self.diff_panel.clone();
        let is_diff_full_screen = self.diff_full_screen && diff_panel.is_some();
        let border = cx.theme().colors().border;
        let active_view = self
            .active_thread
            .and_then(|thread_id| self.open_threads.get(&thread_id))
            .map(|open_thread| open_thread.view.clone());
        let shows_workspaces = self.view == MainView::Workspaces && settings_page.is_none();
        let no_thread = (!shows_workspaces && settings_page.is_none() && active_view.is_none())
            .then(|| self.render_no_thread(cx));
        let decorations = window.window_decorations();

        let shell = v_flex()
            .key_context(KEY_CONTEXT)
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(Self::open_folder))
            .on_action(cx.listener(Self::new_thread))
            .on_action(cx.listener(Self::open_settings))
            .on_action(cx.listener(Self::toggle_sidebar))
            // The Agents view's title bar and thread, which Workspaces doesn't show, so the
            // command palette leaves them out there.
            .when(self.view == MainView::Agents, |shell| {
                shell
                    .on_action(cx.listener(Self::toggle_project_switcher))
                    .on_action(cx.listener(Self::toggle_diff))
                    .on_action(cx.listener(Self::toggle_terminal_drawer))
            })
            .on_action(cx.listener(Self::toggle_shortcuts))
            .on_action(cx.listener(Self::toggle_command_palette))
            .on_action(cx.listener(Self::toggle_go_to))
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
            .map(|shell| match decorations {
                Decorations::Client { tiling } => shell.rounded_top_corners(tiling),
                Decorations::Server => shell,
            })
            .text_color(text_color)
            .font_ui(cx)
            .text_ui(cx)
            .child(self.render_title_bar(window, cx))
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
                                        (None, None) => main.children(no_thread),
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
                                            // Narrower when the window can't fit it beside
                                            // the conversation.
                                            this.w(self.diff_width)
                                                .min_w(MIN_DIFF_PANEL_WIDTH)
                                                .flex_shrink(1.)
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
                    })
                    .or_else(|| {
                        self.account_dialog
                            .as_ref()
                            .map(|(dialog, _)| AnyView::from(dialog.clone()))
                    })
                    .or_else(|| self.overlay.as_ref().map(|overlay| overlay.view.clone())),
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
            );
        window_decorations::client_side_decorations(shell, window, cx)
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

/// Whether an agent's news gets a macOS notification: only while another app is in front, so
/// the sidebar and the sound tell the user otherwise.
fn should_notify(window: &Window, cx: &App) -> bool {
    !window.is_window_active()
        && AppSettingsStore::global(cx)
            .read(cx)
            .settings()
            .notify_when_unfocused
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
    use crate::spaces_view::PaneKey;
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
            let pane = PaneKey {
                machine,
                pane: agentz_protocol::layout::PaneId(7),
            };
            assert_eq!(
                PaneKey::from_notification_tag(&pane.notification_tag()),
                Some(pane)
            );
            assert_eq!(thread_from_notification_tag(&pane.notification_tag()), None);
        }
        assert_eq!(thread_from_notification_tag("thread-7"), None);
    }

    /// The view switch's selected side stands out from the title bar in every bundled theme,
    /// Catppuccin's too, whose element background is its title bar's color.
    #[gpui::test]
    fn the_selected_view_shows_in_every_theme(cx: &mut gpui::TestAppContext) {
        cx.update(|cx| {
            crate::init_for_test(cx);
            let registry = theme::ThemeRegistry::global(cx);
            let names = registry.list_names();
            assert!(names.iter().any(|name| name.starts_with("Catppuccin")));
            for name in names {
                let theme = registry.get(&name).expect("listed theme");
                let colors = theme.colors();
                let title_bar = colors.title_bar_background;
                let selected = title_bar.blend(super::view_switch_selected_background(colors));
                let difference = (selected.l - title_bar.l).abs();
                assert!(
                    difference >= 0.05,
                    "{name}: the selected side is only {difference} lighter or darker"
                );
            }
        });
    }
}

#[cfg(test)]
mod modal_tests {
    use std::cell::RefCell;
    use std::rc::Rc;

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

    // Linux has no traffic lights to leave room for.
    #[cfg(target_os = "macos")]
    #[gpui::test]
    fn full_screen_moves_the_title_bar_left(cx: &mut TestAppContext) {
        cx.update(|cx| {
            crate::init_for_test(cx);
            let client = ServerClient::new_for_test(
                MachineId::Local,
                "This Mac".into(),
                SpacesSnapshot::default(),
                cx,
            );
            crate::machines::init_for_test(vec![client], cx);
            crate::sidebar::init(cx);
        });
        let (_shell, cx) = cx.add_window_view(|window, cx| Shell::new(window, cx));
        cx.run_until_parked();
        let toggle_left = |cx: &mut gpui::VisualTestContext| {
            cx.debug_bounds("toggle-sidebar")
                .expect("the sidebar toggle is shown")
                .left()
        };
        // Past the traffic lights.
        assert_eq!(toggle_left(cx), TRAFFIC_LIGHTS_WIDTH);
        cx.update(|window, _| window.toggle_fullscreen());
        cx.update(|window, _| window.refresh());
        cx.run_until_parked();
        assert!(toggle_left(cx) < px(20.));
    }

    #[gpui::test]
    fn cmd_slash_shows_the_shortcuts_and_gives_focus_back(cx: &mut TestAppContext) {
        cx.update(|cx| {
            crate::init_for_test(cx);
            let client = ServerClient::new_for_test(
                MachineId::Local,
                "This Mac".into(),
                SpacesSnapshot::default(),
                cx,
            );
            crate::machines::init_for_test(vec![client], cx);
            crate::sidebar::init(cx);
            crate::shortcut_sheet::init(cx);
        });
        let (shell, cx) = cx.add_window_view(|window, cx| Shell::new(window, cx));
        shell.update_in(cx, |shell, window, cx| {
            window.focus(&shell.focus_handle, cx)
        });
        cx.run_until_parked();
        let is_open = |cx: &mut gpui::VisualTestContext| {
            shell.read_with(cx, |shell, _| {
                shell.overlay_kind() == Some(OverlayKind::ShortcutSheet)
            })
        };

        cx.simulate_keystrokes("secondary-/");
        assert!(is_open(cx));
        assert!(cx.debug_bounds("shortcut-group-General").is_some());
        // The sheet has the keys while it's open.
        shell.update_in(cx, |shell, window, _| {
            assert!(!shell.focus_handle.is_focused(window));
        });
        cx.simulate_keystrokes("escape");
        assert!(!is_open(cx));
        shell.update_in(cx, |shell, window, _| {
            assert!(shell.focus_handle.is_focused(window));
        });

        // Cmd-/ closes it too.
        cx.simulate_keystrokes("secondary-/");
        assert!(is_open(cx));
        cx.simulate_keystrokes("secondary-/");
        assert!(!is_open(cx));
    }

    /// A project with a thread, and its workspace with two tabs of one pane each.
    fn init_places(cx: &mut TestAppContext) {
        use agentz_protocol::layout::{Node, PaneId};
        use agentz_protocol::spaces::{Pane, PaneContent, Space, SpaceId, Tab, TabId};

        cx.update(|cx| {
            crate::init_for_test(cx);
            let tab = |tab: u64, pane: u64| Tab {
                id: TabId(tab),
                name: None,
                root: Node::Pane(PaneId(pane)),
                panes: vec![Pane::new(
                    PaneId(pane),
                    PaneContent::Unknown(serde_json::Value::Null),
                )],
            };
            let spaces = SpacesSnapshot {
                spaces: vec![Space {
                    id: SpaceId(1),
                    name: None,
                    folder: "/tmp/demo".into(),
                    project_id: None,
                    tabs: vec![tab(2, 3), tab(6, 7)],
                    git: None,
                    current: None,
                }],
            };
            let client =
                ServerClient::new_for_test(MachineId::Local, "This Mac".into(), spaces, cx);
            let projects = client.read(cx).projects().clone();
            let thread: projects::Thread = serde_json::from_value(serde_json::json!({
                "id": 5,
                "project_id": 1,
                "title": "Fix the login",
                "agent_id": "mock",
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
                        threads: vec![thread],
                        ..Default::default()
                    },
                    cx,
                )
            });
            crate::machines::init_for_test(vec![client], cx);
            crate::sidebar::init(cx);
            crate::command_palette::init(cx);
            crate::go_to_picker::init(cx);
        });
    }

    #[gpui::test]
    fn cmd_shift_p_runs_what_applies_where_it_was_opened(cx: &mut TestAppContext) {
        init_places(cx);
        let (shell, cx) = cx.add_window_view(|window, cx| Shell::new(window, cx));
        shell.update_in(cx, |shell, window, cx| {
            window.focus(&shell.focus_handle, cx)
        });
        cx.run_until_parked();
        let is_open = |cx: &mut gpui::VisualTestContext| {
            shell.read_with(cx, |shell, _| {
                shell.overlay_kind() == Some(OverlayKind::CommandPalette)
            })
        };

        // The Agents view's actions, and not the Workspaces view's.
        cx.simulate_keystrokes("secondary-shift-p");
        assert!(is_open(cx));
        assert!(cx.debug_bounds("command-agentz: toggle diff").is_some());
        assert!(cx.debug_bounds("command-workspaces: split right").is_none());
        // A list's own keys aren't commands.
        assert!(cx.debug_bounds("command-menu: confirm").is_none());

        // Filtered as typed, it runs the chosen one where it was opened.
        cx.simulate_input("toggle diff");
        assert!(cx.debug_bounds("command-agentz: toggle sidebar").is_none());
        cx.simulate_keystrokes("enter");
        assert!(!is_open(cx));
        assert!(shell.read_with(cx, |shell, _| shell.show_diff));
        shell.update_in(cx, |shell, window, _| {
            assert!(shell.focus_handle.is_focused(window));
        });

        shell.update_in(cx, |shell, window, cx| {
            shell.set_view(MainView::Workspaces, window, cx)
        });
        cx.run_until_parked();
        cx.simulate_keystrokes("secondary-shift-p");
        assert!(cx.debug_bounds("command-workspaces: split right").is_some());
        assert!(cx.debug_bounds("command-agentz: toggle diff").is_none());
        // Its key closes it, and Go To's opens Go To in its place.
        cx.simulate_keystrokes("secondary-shift-p");
        assert!(!is_open(cx));
        cx.simulate_keystrokes("secondary-shift-p secondary-p");
        assert_eq!(
            shell.read_with(cx, |shell, _| shell.overlay_kind()),
            Some(OverlayKind::GoTo)
        );
    }

    #[gpui::test]
    fn save_layout_names_the_tab_then_gives_focus_back_to_it(cx: &mut TestAppContext) {
        init_places(cx);
        cx.update(crate::save_layout_modal::init);
        let (shell, cx) = cx.add_window_view(|window, cx| Shell::new(window, cx));
        shell.update_in(cx, |shell, window, cx| {
            shell.set_view(MainView::Workspaces, window, cx)
        });
        cx.run_until_parked();

        cx.simulate_keystrokes("secondary-shift-p");
        cx.simulate_input("save layout");
        cx.simulate_keystrokes("enter");
        cx.run_until_parked();
        assert_eq!(
            shell.read_with(cx, |shell, _| shell.overlay_kind()),
            Some(OverlayKind::SaveLayout)
        );
        cx.simulate_input("dev");
        cx.simulate_keystrokes("enter");
        cx.run_until_parked();
        shell.update_in(cx, |shell, window, cx| {
            assert!(shell.overlay.is_none());
            let names: Vec<String> = AppSettingsStore::global(cx)
                .read(cx)
                .settings()
                .saved_layouts
                .iter()
                .map(|layout| layout.name.clone())
                .collect();
            assert_eq!(names, ["dev"]);
            assert!(
                shell
                    .spaces_view
                    .focus_handle(cx)
                    .contains_focused(window, cx)
            );
        });
    }

    #[gpui::test]
    fn cmd_p_goes_to_a_tab_or_a_thread(cx: &mut TestAppContext) {
        init_places(cx);
        let (shell, cx) = cx.add_window_view(|window, cx| Shell::new(window, cx));
        shell.update_in(cx, |shell, window, cx| {
            window.focus(&shell.focus_handle, cx)
        });
        cx.run_until_parked();

        // In Agents, its threads come first.
        cx.simulate_keystrokes("secondary-p");
        let thread = cx
            .debug_bounds("go-to-Fix the login")
            .expect("the thread is listed");
        let workspace = cx
            .debug_bounds("go-to-demo")
            .expect("the workspace is listed");
        assert!(thread.top() < workspace.top());

        // A tab, by its name.
        cx.simulate_input("tab 2");
        cx.simulate_keystrokes("enter");
        cx.run_until_parked();
        shell.read_with(cx, |shell, cx| {
            assert!(shell.overlay.is_none());
            assert!(shell.view == MainView::Workspaces);
            let pane = shell.spaces_view.read(cx).focused_pane(cx);
            assert_eq!(pane.map(|pane| pane.pane.0), Some(7));
        });

        // In Workspaces, the workspaces come first; a thread opens in Agents.
        cx.simulate_keystrokes("secondary-p");
        let thread = cx
            .debug_bounds("go-to-Fix the login")
            .expect("the thread is listed");
        let workspace = cx
            .debug_bounds("go-to-demo")
            .expect("the workspace is listed");
        assert!(workspace.top() < thread.top());
        cx.simulate_input("login");
        cx.simulate_keystrokes("enter");
        cx.run_until_parked();
        shell.read_with(cx, |shell, _| {
            assert!(shell.view == MainView::Agents);
            assert_eq!(shell.active_thread.map(|thread| thread.thread.0), Some(5));
        });
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
            crate::sidebar::init(cx);
        });
        let (shell, cx) = cx.add_window_view(|window, cx| Shell::new(window, cx));
        shell.update_in(cx, |shell, window, cx| {
            window.focus(&shell.focus_handle, cx)
        });
        let hidden = |cx: &mut gpui::VisualTestContext| cx.update(|_, cx| is_sidebar_hidden(cx));
        assert!(!hidden(cx));
        cx.simulate_keystrokes("secondary-b");
        assert!(hidden(cx));
        cx.simulate_keystrokes("secondary-b");
        assert!(!hidden(cx));
    }

    #[gpui::test]
    fn cmd_d_splits_in_workspaces_and_shows_changes_in_agents(cx: &mut TestAppContext) {
        cx.update(|cx| {
            crate::init_for_test(cx);
            let client = ServerClient::new_for_test(
                MachineId::Local,
                "This Mac".into(),
                SpacesSnapshot::default(),
                cx,
            );
            crate::machines::init_for_test(vec![client], cx);
            crate::sidebar::init(cx);
        });
        let (shell, cx) = cx.add_window_view(|window, cx| Shell::new(window, cx));
        let dispatched = Rc::new(RefCell::new(Vec::new()));
        let _subscription = cx.update(|_, cx| {
            let dispatched = dispatched.clone();
            cx.observe_keystrokes(move |event, _, _| {
                if let Some(action) = &event.action {
                    dispatched.borrow_mut().push(action.name());
                }
            })
        });

        shell.update_in(cx, |shell, window, cx| {
            shell.set_view(MainView::Workspaces, window, cx)
        });
        cx.simulate_keystrokes(crate::platform_keys("cmd-d", "ctrl-shift-o"));
        assert_eq!(dispatched.borrow().last(), Some(&"workspaces::SplitRight"));
        assert!(!shell.read_with(cx, |shell, _| shell.show_diff));

        shell.update_in(cx, |shell, window, cx| {
            shell.set_view(MainView::Agents, window, cx)
        });
        cx.simulate_keystrokes("secondary-d");
        assert_eq!(dispatched.borrow().last(), Some(&"agentz::ToggleDiff"));
        assert!(shell.read_with(cx, |shell, _| shell.show_diff));
    }

    fn demo_project(id: u64, path: &str) -> Project {
        Project {
            id: ProjectId(id),
            path: path.into(),
            custom_name: None,
            icon: None,
            workspaces: Vec::new(),
            repository: None,
        }
    }

    fn demo_thread(id: u64, project: u64, active_at: u64, is_draft: bool) -> projects::Thread {
        let time = serde_json::json!({ "secs_since_epoch": active_at, "nanos_since_epoch": 0 });
        serde_json::from_value(serde_json::json!({
            "id": id,
            "project_id": project,
            "title": format!("Thread {id}"),
            "agent_id": "mock",
            "created_at": time,
            "last_activity_at": time,
            "is_draft": is_draft,
        }))
        .expect("a thread")
    }

    /// demo (1) and api (2), with api's thread 6 the latest; demo has 5, then 7. The drafts
    /// a new thread is answered with: 8 in demo, 9 in api.
    fn demo_snapshot(without: &[u64], archived: &[u64]) -> ProjectsSnapshot {
        let threads = [(5, 1, 100), (6, 2, 200), (7, 1, 50)]
            .into_iter()
            .filter(|(id, _, _)| !without.contains(id))
            .map(|(id, project, active_at)| {
                let mut thread = demo_thread(id, project, active_at, false);
                if archived.contains(&id) {
                    thread.archived_at = thread.last_activity_at;
                }
                thread
            })
            .chain([demo_thread(8, 1, 0, true), demo_thread(9, 2, 0, true)])
            .collect();
        ProjectsSnapshot {
            projects: vec![demo_project(1, "/tmp/demo"), demo_project(2, "/tmp/api")],
            threads,
            ..Default::default()
        }
    }

    fn install_mock_agent(cx: &mut App) {
        use agentz_protocol::agents::{AgentListing, RegistryAgentMetadata, RegistrySnapshot};

        let registry = Machines::local(cx).read(cx).registry().clone();
        registry.update(cx, |registry, cx| {
            registry.set_snapshot(
                RegistrySnapshot {
                    agents: vec![AgentListing {
                        metadata: RegistryAgentMetadata {
                            id: AgentId::new("mock".to_string()),
                            name: "Mock".into(),
                            description: "Mock, for tests".into(),
                            version: "1.0.0".into(),
                            repository: None,
                            website: None,
                            license_url: None,
                            icon: None,
                        },
                        supports_current_platform: true,
                        install_state: InstallState::Installed {
                            version: "1.0.0".into(),
                            update_available: false,
                        },
                        custom_command: None,
                        accounts: None,
                    }],
                    is_fetching: false,
                    fetch_error: None,
                },
                cx,
            )
        });
    }

    /// This Mac, online, whose session hasn't arrived yet. A new thread in project `n` is
    /// answered with draft `7 + n`; the projects asked for are returned.
    fn init_for_drafts(
        cx: &mut TestAppContext,
    ) -> (
        Entity<crate::project_store::ProjectStore>,
        Rc<RefCell<Vec<ProjectId>>>,
    ) {
        let created = Rc::new(RefCell::new(Vec::new()));
        let store = cx.update(|cx| {
            crate::init_for_test(cx);
            let client = ServerClient::new_for_test(
                MachineId::Local,
                "This Mac".into(),
                SpacesSnapshot::default(),
                cx,
            );
            let created = created.clone();
            client.update(cx, |client, cx| {
                client.set_online_for_test(cx);
                client.answer_for_test(move |request| match request {
                    agentz_protocol::Request::CreateThread { project_id, .. } => {
                        created.borrow_mut().push(*project_id);
                        Some(agentz_protocol::Response::ThreadCreated(ThreadId(
                            7 + project_id.0,
                        )))
                    }
                    _ => None,
                });
            });
            let store = client.read(cx).projects().clone();
            crate::machines::init_for_test(vec![client], cx);
            crate::sidebar::init(cx);
            store
        });
        (store, created)
    }

    fn active_thread(shell: &Entity<Shell>, cx: &mut gpui::VisualTestContext) -> Option<u64> {
        shell.read_with(cx, |shell, _| {
            shell.active_thread.map(|thread| thread.thread.0)
        })
    }

    fn local_thread(id: u64) -> ThreadKey {
        ThreadKey {
            machine: MachineId::Local,
            thread: ThreadId(id),
        }
    }

    #[gpui::test]
    fn launch_opens_a_draft_in_the_latest_threads_project(cx: &mut TestAppContext) {
        let (store, created) = init_for_drafts(cx);
        cx.update(install_mock_agent);
        let (shell, cx) = cx.add_window_view(|window, cx| Shell::new(window, cx));
        cx.run_until_parked();
        // Nothing until the session says what there is.
        assert!(created.borrow().is_empty());
        assert!(cx.debug_bounds("welcome-Open Folder…").is_none());

        store.update(cx, |store, cx| {
            store.set_snapshot(demo_snapshot(&[], &[]), cx)
        });
        cx.run_until_parked();
        assert_eq!(*created.borrow(), [ProjectId(2)]);
        assert_eq!(active_thread(&shell, cx), Some(9));
    }

    #[gpui::test]
    fn archiving_the_open_thread_here_opens_a_draft_in_its_project(cx: &mut TestAppContext) {
        let (store, created) = init_for_drafts(cx);
        cx.update(install_mock_agent);
        let (shell, cx) = cx.add_window_view(|window, cx| Shell::new(window, cx));
        store.update(cx, |store, cx| {
            store.set_snapshot(demo_snapshot(&[], &[]), cx)
        });
        cx.run_until_parked();
        shell.update_in(cx, |shell, window, cx| {
            shell.open_thread(local_thread(5), window, cx)
        });

        store.update(cx, |store, cx| store.archive_thread(ThreadId(5), cx));
        cx.run_until_parked();
        assert_eq!(*created.borrow(), [ProjectId(2), ProjectId(1)]);
        assert_eq!(active_thread(&shell, cx), Some(8));

        // Archived by an agent or another app, it stays on screen, read-only.
        shell.update_in(cx, |shell, window, cx| {
            shell.open_thread(local_thread(6), window, cx)
        });
        store.update(cx, |store, cx| {
            store.set_snapshot(demo_snapshot(&[], &[5, 6]), cx)
        });
        cx.run_until_parked();
        assert_eq!(active_thread(&shell, cx), Some(6));
        assert_eq!(created.borrow().len(), 2);
    }

    #[gpui::test]
    fn deleting_the_open_thread_opens_the_next_in_its_project(cx: &mut TestAppContext) {
        let (store, created) = init_for_drafts(cx);
        cx.update(install_mock_agent);
        let (shell, cx) = cx.add_window_view(|window, cx| Shell::new(window, cx));
        store.update(cx, |store, cx| {
            store.set_snapshot(demo_snapshot(&[], &[]), cx)
        });
        cx.run_until_parked();
        shell.update_in(cx, |shell, window, cx| {
            shell.open_thread(local_thread(5), window, cx)
        });

        // api's thread is newer, but 7 is demo's.
        store.update(cx, |store, cx| {
            store.set_snapshot(demo_snapshot(&[5], &[]), cx)
        });
        cx.run_until_parked();
        assert_eq!(active_thread(&shell, cx), Some(7));
        assert_eq!(created.borrow().len(), 1);

        // With none left there, a draft in it.
        store.update(cx, |store, cx| {
            store.set_snapshot(demo_snapshot(&[5, 7], &[]), cx)
        });
        cx.run_until_parked();
        assert_eq!(*created.borrow(), [ProjectId(2), ProjectId(1)]);
        assert_eq!(active_thread(&shell, cx), Some(8));
    }

    /// Leaving a thread closes its view, so the server can stop its agent once it's idle. Its
    /// queued messages wait on the server.
    #[gpui::test]
    fn leaving_a_thread_closes_its_view(cx: &mut TestAppContext) {
        let (store, _) = init_for_drafts(cx);
        cx.update(install_mock_agent);
        let (shell, cx) = cx.add_window_view(|window, cx| Shell::new(window, cx));
        store.update(cx, |store, cx| {
            store.set_snapshot(demo_snapshot(&[], &[]), cx)
        });
        cx.run_until_parked();
        let open_threads = |shell: &Entity<Shell>, cx: &mut gpui::VisualTestContext| {
            shell.read_with(cx, |shell, _| {
                let mut threads: Vec<u64> =
                    shell.open_threads.keys().map(|key| key.thread.0).collect();
                threads.sort();
                threads
            })
        };
        shell.update_in(cx, |shell, window, cx| {
            shell.open_thread(local_thread(5), window, cx)
        });
        shell.update_in(cx, |shell, window, cx| {
            shell.open_thread(local_thread(7), window, cx)
        });
        assert_eq!(open_threads(&shell, cx), [7]);
    }

    #[gpui::test]
    fn the_welcome_page_shows_until_the_first_project(cx: &mut TestAppContext) {
        let (store, created) = init_for_drafts(cx);
        let (shell, cx) = cx.add_window_view(|window, cx| Shell::new(window, cx));
        store.update(cx, |store, cx| {
            store.set_snapshot(ProjectsSnapshot::default(), cx)
        });
        cx.run_until_parked();
        let open_folder = cx
            .debug_bounds("welcome-Open Folder…")
            .expect("Open Folder is listed");
        let install = cx
            .debug_bounds("welcome-Install an Agent…")
            .expect("installing an agent is listed");
        let add_machine = cx
            .debug_bounds("welcome-Add Machine…")
            .expect("Add Machine is listed");
        let settings = cx
            .debug_bounds("welcome-Settings")
            .expect("Settings is listed");
        assert!(open_folder.top() < install.top());
        assert!(install.top() < add_machine.top());
        assert!(add_machine.top() < settings.top());

        cx.simulate_click(install.center(), gpui::Modifiers::none());
        assert!(shell.read_with(cx, |shell, _| shell.settings_page.is_some()));
        shell.update_in(cx, |shell, window, cx| shell.close_settings(window, cx));
        cx.update(|_, cx| install_mock_agent(cx));
        cx.run_until_parked();
        assert!(cx.debug_bounds("welcome-Install an Agent…").is_none());
        assert!(cx.debug_bounds("welcome-Open Folder…").is_some());

        // The first project opens a draft in it.
        store.update(cx, |store, cx| {
            store.set_snapshot(
                ProjectsSnapshot {
                    projects: vec![demo_project(1, "/tmp/demo")],
                    threads: vec![demo_thread(8, 1, 0, true)],
                    ..Default::default()
                },
                cx,
            )
        });
        cx.run_until_parked();
        assert!(cx.debug_bounds("welcome-Open Folder…").is_none());
        assert_eq!(*created.borrow(), [ProjectId(1)]);
        assert_eq!(active_thread(&shell, cx), Some(8));
    }

    #[gpui::test]
    fn agents_sound_and_notify_as_the_settings_say(cx: &mut TestAppContext) {
        use agentz_protocol::layout::PaneId;
        use agentz_protocol::spaces::{
            Pane, PaneAgent, PaneAgentState, PaneContent, PaneTerminal, Space, SpaceId, Tab, TabId,
        };
        use gpui::{SystemNotificationResponse, VisualTestContext};
        use sound::Sound;
        use spaces_view::TabKey;

        // Codex alone in the workspace's first tab, Claude in its second.
        let spaces = |codex: PaneAgentState, claude: PaneAgentState| {
            let tab = |tab: u64, pane_id: u64, name: &str, state: PaneAgentState| {
                let mut pane = Pane::new(
                    PaneId(pane_id),
                    PaneContent::Terminal(PaneTerminal {
                        folder: "/tmp/demo".into(),
                        command: None,
                    }),
                );
                pane.agent = Some(PaneAgent {
                    registry_agent: None,
                    name: name.to_string(),
                    state,
                });
                Tab {
                    id: TabId(tab),
                    name: None,
                    root: Node::Pane(PaneId(pane_id)),
                    panes: vec![pane],
                }
            };
            SpacesSnapshot {
                spaces: vec![Space {
                    id: SpaceId(1),
                    name: None,
                    folder: "/tmp/demo".into(),
                    project_id: None,
                    tabs: vec![tab(2, 3, "Codex", codex), tab(6, 7, "Claude", claude)],
                    git: None,
                    current: None,
                }],
            }
        };
        use PaneAgentState::{Blocked, Idle, Working};
        let client = cx.update(|cx| {
            crate::init_for_test(cx);
            cx.set_app_identity("dev.agentz.test", "agentZ");
            let client = ServerClient::new_for_test(
                MachineId::Local,
                "This Mac".into(),
                spaces(Working, Working),
                cx,
            );
            let thread: projects::Thread = serde_json::from_value(serde_json::json!({
                "id": 5,
                "project_id": 1,
                "title": "Fix the login",
                "agent_id": "mock",
            }))
            .expect("a thread");
            client.read(cx).projects().clone().update(cx, |store, cx| {
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
                        threads: vec![thread],
                        ..Default::default()
                    },
                    cx,
                )
            });
            crate::machines::init_for_test(vec![client.clone()], cx);
            crate::sidebar::init(cx);
            client
        });
        let (shell, cx) = cx.add_window_view(|window, cx| Shell::new(window, cx));
        shell.update_in(cx, |shell, window, cx| {
            window.activate_window();
            let tab = TabKey {
                machine: MachineId::Local,
                tab: TabId(2),
            };
            shell.go_to(Place::Tab(tab), window, cx);
        });
        cx.run_until_parked();
        let set = |codex, claude, cx: &mut VisualTestContext| {
            client.update(cx, |client, cx| {
                client.set_spaces_for_test(spaces(codex, claude), cx)
            });
            cx.run_until_parked();
            cx.update(|_, cx| sound::take_played_for_test(cx))
        };
        let shown = |cx: &mut VisualTestContext| {
            cx.shown_system_notifications()
                .into_iter()
                .map(|shown| (shown.tag.to_string(), shown.title.to_string(), shown.body))
                .map(|(tag, title, body)| (tag, title, body.to_string()))
                .collect::<Vec<_>>()
        };

        // With agentZ in front, the finished sound plays only for the agent out of sight, the
        // input sound for both, and nothing is notified.
        assert_eq!(set(Idle, Working, cx), []);
        assert_eq!(set(Idle, Idle, cx), [Sound::Finished]);
        assert_eq!(
            set(Blocked, Blocked, cx),
            [Sound::NeedsInput, Sound::NeedsInput]
        );
        assert!(shown(cx).is_empty());

        // When in another app, the finished sound stays quiet for both while agentZ is in
        // front.
        cx.update(|_, cx| {
            AppSettingsStore::global(cx).update(cx, |store, cx| {
                store.update(
                    |settings| {
                        settings.play_sound_when_finished =
                            crate::app_settings::PlaySound::WhenInAnotherApp
                    },
                    cx,
                )
            })
        });
        assert_eq!(set(Working, Working, cx), []);
        assert_eq!(set(Idle, Idle, cx), []);

        // With another app in front, nothing is in sight, and each gets a notification.
        cx.deactivate_window();
        assert_eq!(set(Working, Working, cx), []);
        assert_eq!(set(Idle, Blocked, cx), [Sound::Finished, Sound::NeedsInput]);
        let thread = ThreadKey {
            machine: MachineId::Local,
            thread: ThreadId(5),
        };
        shell.update_in(cx, |shell, window, cx| {
            shell.notify_attention(thread, ThreadStatus::Completed, window, cx)
        });
        assert_eq!(
            cx.update(|_, cx| sound::take_played_for_test(cx)),
            [Sound::Finished]
        );
        let notification = |tag: &str, title: &str, body: &str| {
            (tag.to_string(), title.to_string(), body.to_string())
        };
        assert_eq!(
            shown(cx),
            [
                notification("pane-local-3", "Codex", "demo › Tab 1 · Finished"),
                notification("pane-local-7", "Claude", "demo › Tab 2 · Needs attention"),
                notification("thread-local-5", "Fix the login", "demo · Finished"),
            ]
        );

        // Turned off, notifications stop, and sounds don't.
        cx.update(|_, cx| {
            AppSettingsStore::global(cx).update(cx, |store, cx| {
                store.update(|settings| settings.notify_when_unfocused = false, cx)
            })
        });
        assert_eq!(set(Working, Working, cx), []);
        assert_eq!(set(Idle, Idle, cx), [Sound::Finished, Sound::Finished]);
        assert_eq!(shown(cx).len(), 3);

        // Clicking a pane's notification shows the pane.
        shell.update_in(cx, |shell, window, cx| {
            shell.set_view(MainView::Agents, window, cx)
        });
        cx.simulate_system_notification_response(SystemNotificationResponse {
            tag: "pane-local-7".into(),
            action_id: None,
        });
        cx.run_until_parked();
        shell.read_with(cx, |shell, cx| {
            assert!(shell.view == MainView::Workspaces);
            assert_eq!(
                shell.spaces_view.read(cx).focused_pane(cx),
                Some(PaneKey {
                    machine: MachineId::Local,
                    pane: PaneId(7),
                })
            );
        });
    }
}
