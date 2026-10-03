use crate::project_store::{ProjectStore, ProjectStoreEvent, ThreadStatus};
use agentz_protocol::agents::AgentId;
use collections::HashMap;
use gpui::{
    App, Context, DismissEvent, Entity, FocusHandle, Focusable, MouseButton, PathPromptOptions,
    Subscription, SystemNotification, Window, WindowControlArea,
};
use projects::{ProjectId, ProjectScope, ThreadId};
use ui::{ButtonLike, PopoverMenu, PopoverMenuHandle, Tooltip, prelude::*};
use util::ResultExt as _;

use crate::agent_view::{AgentView, AgentViewEvent};
use crate::app_settings::AppSettingsStore;
use crate::new_thread_modal::{NewThreadModal, NewThreadModalEvent};
use crate::project_info::{ProjectInfoStore, render_project_icon};
use crate::project_switcher::ProjectSwitcher;
use crate::registry_store::AgentRegistryStore;
use crate::server_client::{ServerClient, ServerStatus};
use crate::settings_page::{SettingsPage, SettingsPageEvent};
use crate::sidebar::{SIDEBAR_WIDTH, Sidebar, SidebarEvent};
use crate::thread_entity::AgentThread;
use crate::{NewThread, OpenFolder, OpenSettings, ToggleProjectSwitcher};

const TITLE_BAR_HEIGHT: Pixels = px(40.);
/// Leaves room for the macOS traffic lights.
const TRAFFIC_LIGHTS_WIDTH: Pixels = px(80.);

/// An open thread. Kept while the app runs so its agent keeps working in the background.
struct OpenThread {
    view: Entity<AgentView>,
    _subscriptions: [Subscription; 1],
}

pub struct Shell {
    focus_handle: FocusHandle,
    store: Entity<ProjectStore>,
    registry: Entity<AgentRegistryStore>,
    sidebar: Entity<Sidebar>,
    switcher_handle: PopoverMenuHandle<ProjectSwitcher>,
    new_thread_modal: Option<(Entity<NewThreadModal>, Vec<Subscription>)>,
    /// Shown in the main area in place of the thread while open.
    settings_page: Option<(Entity<SettingsPage>, Subscription)>,
    open_threads: HashMap<ThreadId, OpenThread>,
    active_thread: Option<ThreadId>,
    should_move_window: bool,
    _subscriptions: Vec<Subscription>,
}

impl Shell {
    pub fn new(
        store: Entity<ProjectStore>,
        registry: Entity<AgentRegistryStore>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let sidebar = cx.new(|cx| Sidebar::new(store.clone(), registry.clone(), cx));
        let subscriptions = vec![
            cx.observe_in(&store, window, |this, store, window, cx| {
                // Close views (and stop their agents) for threads that were deleted or removed
                // along with their project. Archived threads stay open, read-only.
                let store = store.read(cx);
                let is_live = |thread_id: ThreadId| store.thread(thread_id).is_some();
                this.open_threads.retain(|thread_id, _| is_live(*thread_id));
                let states: Vec<(Entity<AgentView>, SharedString, bool)> = this
                    .open_threads
                    .iter()
                    .filter_map(|(thread_id, open_thread)| {
                        let thread = store.thread(*thread_id)?;
                        Some((
                            open_thread.view.clone(),
                            thread.title.clone().into(),
                            thread.archived_at.is_some(),
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
                for (view, title, is_archived) in states {
                    view.update(cx, |view, cx| {
                        view.set_title(title, cx);
                        view.set_archived(is_archived, cx);
                    });
                }
                // Focus was in the closed thread's view. Without moving it here, actions such as
                // New Thread would be dispatched from the window's root, above the shell's
                // handlers, and do nothing.
                if closed_active_thread {
                    window.focus(&this.focus_handle, cx);
                }
                this.mark_active_thread_viewed(window, cx);
                cx.notify();
            }),
            cx.subscribe_in(&store, window, |this, _, event, window, cx| match event {
                ProjectStoreEvent::NeedsAttention(thread_id, status) => {
                    this.notify_attention(*thread_id, *status, window, cx)
                }
            }),
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
            cx.observe(&ServerClient::global(cx), |_, _, cx| cx.notify()),
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
            store,
            registry,
            sidebar,
            switcher_handle: PopoverMenuHandle::default(),
            new_thread_modal: None,
            settings_page: None,
            open_threads: HashMap::default(),
            active_thread: None,
            should_move_window: false,
            _subscriptions: subscriptions,
        }
    }

    fn new_thread(&mut self, _: &NewThread, window: &mut Window, cx: &mut Context<Self>) {
        let store = self.store.read(cx);
        if store.projects().is_empty() {
            window.dispatch_action(Box::new(OpenFolder), cx);
            return;
        }
        // With all projects shown, the modal asks for the project first, unless there's only
        // one to choose.
        let project_id = match store.scope() {
            ProjectScope::Project(id) => Some(id),
            ProjectScope::All => match store.projects() {
                [project] => Some(project.id),
                _ => None,
            },
        };
        self.open_new_thread_modal(project_id, window, cx);
    }

    fn open_new_thread_modal(
        &mut self,
        project_id: Option<ProjectId>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let store = self.store.clone();
        let registry = self.registry.clone();
        let modal = cx.new(|cx| NewThreadModal::new(project_id, store, registry, window, cx));
        let subscriptions = vec![
            cx.subscribe_in(&modal, window, |this, _, _: &DismissEvent, window, cx| {
                this.dismiss_new_thread_modal(window, cx);
            }),
            cx.subscribe_in(&modal, window, |this, _, event, window, cx| match event {
                NewThreadModalEvent::ThreadCreated(thread_id) => {
                    this.dismiss_new_thread_modal(window, cx);
                    this.open_thread(*thread_id, window, cx);
                }
                NewThreadModalEvent::OpenAgentSettings => {
                    this.dismiss_new_thread_modal(window, cx);
                    this.open_settings(&OpenSettings, window, cx);
                    if let Some((page, _)) = &this.settings_page {
                        page.update(cx, |page, cx| page.show_agents(window, cx));
                    }
                }
            }),
        ];
        self.new_thread_modal = Some((modal, subscriptions));
        cx.notify();
    }

    fn open_settings(&mut self, _: &OpenSettings, window: &mut Window, cx: &mut Context<Self>) {
        let page = match &self.settings_page {
            Some((page, _)) => page.clone(),
            None => {
                let page = cx.new(|cx| SettingsPage::new(self.store.clone(), cx));
                let subscription =
                    cx.subscribe_in(&page, window, |this, _, event, window, cx| match event {
                        SettingsPageEvent::Close => this.close_settings(window, cx),
                    });
                self.settings_page = Some((page.clone(), subscription));
                page
            }
        };
        window.focus(&page.focus_handle(cx), cx);
        cx.notify();
    }

    fn open_project_settings(
        &mut self,
        project_id: ProjectId,
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
        match self
            .active_thread
            .and_then(|thread_id| self.open_threads.get(&thread_id))
        {
            Some(open_thread) => window.focus(&open_thread.view.focus_handle(cx), cx),
            None => window.focus(&self.focus_handle, cx),
        }
        self.mark_active_thread_viewed(window, cx);
        cx.notify();
    }

    fn open_thread(&mut self, thread_id: ThreadId, window: &mut Window, cx: &mut Context<Self>) {
        self.settings_page = None;
        if !self.open_threads.contains_key(&thread_id) {
            let Some(open_thread) = self.start_thread(thread_id, window, cx) else {
                return;
            };
            self.open_threads.insert(thread_id, open_thread);
        }
        self.active_thread = Some(thread_id);
        // A subthread isn't in the sidebar, so its top-level thread is highlighted.
        let sidebar_thread = self.store.read(cx).root_thread(thread_id);
        self.sidebar.update(cx, |sidebar, cx| {
            sidebar.set_active_thread(Some(sidebar_thread), cx)
        });
        if let Some(open_thread) = self.open_threads.get(&thread_id) {
            window.focus(&open_thread.view.focus_handle(cx), cx);
        }
        self.mark_active_thread_viewed(window, cx);
        cx.notify();
    }

    /// Whether the user can see the thread right now, as Zed's `agent_status_visible` decides.
    fn is_thread_visible(&self, thread_id: ThreadId, window: &Window) -> bool {
        window.is_window_active()
            && self.settings_page.is_none()
            && self.active_thread == Some(thread_id)
    }

    fn mark_active_thread_viewed(&self, window: &Window, cx: &mut Context<Self>) {
        let Some(thread_id) = self.active_thread else {
            return;
        };
        if self.is_thread_visible(thread_id, window) {
            self.store
                .update(cx, |store, cx| store.mark_viewed(thread_id, cx));
            cx.dismiss_system_notification(&notification_tag(thread_id));
        }
    }

    /// A macOS notification for a thread that isn't on screen, as Zed notifies.
    fn notify_attention(
        &self,
        thread_id: ThreadId,
        status: ThreadStatus,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.is_thread_visible(thread_id, window) {
            return;
        }
        let store = self.store.read(cx);
        let Some(thread) = store.thread(thread_id) else {
            return;
        };
        let caption = match status {
            ThreadStatus::PendingApproval => "Waiting for tool confirmation",
            ThreadStatus::Working | ThreadStatus::Completed => "Finished",
        };
        let body = match store.project(thread.project_id) {
            Some(project) => format!("{} · {caption}", project.name()),
            None => caption.to_string(),
        };
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
        thread_id: ThreadId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<OpenThread> {
        let thread = self.store.read(cx).thread(thread_id)?.clone();
        let agent_id = thread.agent_id.clone().map(AgentId::new);
        let agent_thread = AgentThread::shared(thread_id, cx);
        let title = SharedString::from(thread.title);
        let registry = self.registry.clone();
        let is_archived = thread.archived_at.is_some();
        let view = cx.new(|cx| {
            let mut view = AgentView::new(thread_id, agent_thread, title, registry, agent_id, cx);
            view.set_archived(is_archived, cx);
            view
        });
        let view_subscription = cx.subscribe_in(
            &view,
            window,
            move |this, _, event, window, cx| match event {
                AgentViewEvent::Unarchive => this
                    .store
                    .update(cx, |store, cx| store.unarchive_thread(thread_id, cx)),
                AgentViewEvent::OpenThread(other) => this.open_thread(*other, window, cx),
            },
        );
        Some(OpenThread {
            view,
            _subscriptions: [view_subscription],
        })
    }

    fn dismiss_new_thread_modal(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.new_thread_modal.take().is_some() {
            match self
                .active_thread
                .and_then(|thread_id| self.open_threads.get(&thread_id))
            {
                Some(open_thread) => window.focus(&open_thread.view.focus_handle(cx), cx),
                None => window.focus(&self.focus_handle, cx),
            }
            cx.notify();
        }
    }

    fn open_folder(&mut self, _: &OpenFolder, _: &mut Window, cx: &mut Context<Self>) {
        let paths = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: true,
            prompt: Some("Open".into()),
        });
        let store = self.store.clone();
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
            // In "All projects" the new project simply appears; otherwise switch to it so it
            // doesn't get added out of sight.
            store.update(cx, |store, cx| {
                if let Some(id) = last_added
                    && store.scope() != ProjectScope::All
                {
                    store.set_scope(ProjectScope::Project(id), cx);
                }
            });
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
        let store = self.store.read(cx);
        let project_info = ProjectInfoStore::global(cx).read(cx).info();
        let (scope_icon, scope_label): (AnyElement, SharedString) =
            match store.scope().project().and_then(|id| store.project(id)) {
                Some(project) => (
                    render_project_icon(project, project_info.get(&project.id), px(14.), cx),
                    project.name(),
                ),
                None => (
                    Icon::new(IconName::ListTree)
                        .size(IconSize::Small)
                        .color(Color::Muted)
                        .into_any_element(),
                    "All projects".into(),
                ),
            };
        let switcher_store = self.store.clone();
        let shell = cx.entity().downgrade();

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
                // Keeps a press on the switcher from starting a window drag.
                div()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .child(
                        PopoverMenu::new("project-switcher")
                            .with_handle(self.switcher_handle.clone())
                            .menu(move |window, cx| {
                                let store = switcher_store.clone();
                                let shell = shell.clone();
                                let open_project_settings =
                                    move |project_id, window: &mut Window, cx: &mut App| {
                                        shell
                                            .update(cx, |shell, cx| {
                                                shell.open_project_settings(project_id, window, cx)
                                            })
                                            .ok();
                                    };
                                Some(cx.new(|cx| {
                                    ProjectSwitcher::new(store, open_project_settings, window, cx)
                                }))
                            })
                            .trigger_with_tooltip(
                                ButtonLike::new("project-switcher-trigger").child(
                                    h_flex()
                                        .px_1()
                                        .gap_1p5()
                                        .child(scope_icon)
                                        .child(Label::new(scope_label).size(LabelSize::Small))
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
            .child(div().flex_1())
            .children(self.render_connection_status(cx))
    }

    fn render_connection_status(&self, cx: &App) -> Option<AnyElement> {
        match ServerClient::global(cx).read(cx).status() {
            ServerStatus::Connecting | ServerStatus::Connected => None,
            ServerStatus::Disconnected(error) => {
                let tooltip: SharedString =
                    format!("Disconnected from agentz-server: {error}").into();
                Some(
                    div()
                        .id("disconnected")
                        .child(Icon::new(IconName::Disconnected).size(IconSize::Small))
                        .tooltip(Tooltip::text(tooltip))
                        .into_any_element(),
                )
            }
        }
    }
}

impl Focusable for Shell {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for Shell {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let background = cx.theme().colors().background;
        let text_color = cx.theme().colors().text;
        let main_background = cx.theme().colors().editor_background;
        let settings_page = self.settings_page.as_ref().map(|(page, _)| page.clone());
        let active_view = self
            .active_thread
            .and_then(|thread_id| self.open_threads.get(&thread_id))
            .map(|open_thread| open_thread.view.clone());

        v_flex()
            .key_context("Shell")
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(Self::open_folder))
            .on_action(cx.listener(Self::toggle_project_switcher))
            .on_action(cx.listener(Self::new_thread))
            .on_action(cx.listener(Self::open_settings))
            .relative()
            .size_full()
            .bg(background)
            .text_color(text_color)
            .font_ui(cx)
            .text_ui(cx)
            .child(self.render_title_bar(cx))
            .child(
                h_flex()
                    .flex_1()
                    .min_h_0()
                    // Settings brings its own navigation in place of the thread list.
                    .when(settings_page.is_none(), |row| {
                        row.child(self.sidebar.clone())
                    })
                    .child(
                        div()
                            .flex_1()
                            .min_w(SIDEBAR_WIDTH)
                            .h_full()
                            .bg(main_background)
                            .map(|main| match (settings_page, active_view) {
                                (Some(page), _) => main.child(page),
                                (None, Some(view)) => main.child(view),
                                (None, None) => main.child(render_no_thread_selected()),
                            }),
                    ),
            )
            .when_some(
                self.new_thread_modal
                    .as_ref()
                    .map(|(modal, _)| modal.clone()),
                |shell, modal| {
                    shell.child(
                        div()
                            .id("modal-backdrop")
                            .absolute()
                            .inset_0()
                            .flex()
                            .justify_center()
                            .pt(px(96.))
                            .bg(gpui::black().opacity(0.25))
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.dismiss_new_thread_modal(window, cx)
                            }))
                            .child(
                                // Clicks inside the modal must not reach the backdrop.
                                div()
                                    .id("modal-container")
                                    .on_click(|_, _, cx| cx.stop_propagation())
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

fn notification_tag(thread_id: ThreadId) -> SharedString {
    format!("thread-{}", thread_id.0).into()
}

/// The thread a notification is about, from its tag.
fn thread_from_notification_tag(tag: &str) -> Option<ThreadId> {
    tag.strip_prefix("thread-")?.parse().ok().map(ThreadId)
}
