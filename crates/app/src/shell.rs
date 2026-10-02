use agent_client_protocol::schema::v1 as acp;
use agent_thread::{AgentThread, AgentThreadEvent};
use collections::HashMap;
use gpui::{
    App, Context, DismissEvent, Entity, FocusHandle, Focusable, MouseButton, PathPromptOptions,
    Subscription, Window, WindowControlArea,
};
use projects::{ProjectId, ProjectScope, ProjectStore, ThreadId};
use registry::{AgentId, AgentRegistryStore};
use ui::{ButtonLike, PopoverMenu, PopoverMenuHandle, Tooltip, prelude::*};

use crate::agent_view::AgentView;
use crate::new_thread_modal::{NewThreadModal, NewThreadModalEvent};
use crate::project_switcher::ProjectSwitcher;
use crate::sidebar::{SIDEBAR_WIDTH, Sidebar, SidebarEvent};
use crate::{NewThread, OpenFolder, ToggleProjectSwitcher};

const TITLE_BAR_HEIGHT: Pixels = px(40.);
/// Leaves room for the macOS traffic lights.
const TRAFFIC_LIGHTS_WIDTH: Pixels = px(80.);
const MAX_THREAD_TITLE_CHARS: usize = 48;

/// An open thread. Kept while the app runs so its agent keeps working in the background.
struct OpenThread {
    view: Entity<AgentView>,
    _subscription: Subscription,
}

pub struct Shell {
    focus_handle: FocusHandle,
    store: Entity<ProjectStore>,
    registry: Entity<AgentRegistryStore>,
    sidebar: Entity<Sidebar>,
    switcher_handle: PopoverMenuHandle<ProjectSwitcher>,
    new_thread_modal: Option<(Entity<NewThreadModal>, Vec<Subscription>)>,
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
            cx.observe(&store, |this, store, cx| {
                // Drop views for threads that were removed (e.g. with their project).
                let store = store.read(cx);
                this.open_threads
                    .retain(|thread_id, _| store.thread(*thread_id).is_some());
                if this
                    .active_thread
                    .is_some_and(|thread_id| store.thread(thread_id).is_none())
                {
                    this.active_thread = None;
                }
                cx.notify();
            }),
            cx.subscribe_in(&sidebar, window, |this, _, event, window, cx| match event {
                SidebarEvent::NewThread(project_id) => {
                    this.open_new_thread_modal(*project_id, window, cx)
                }
                SidebarEvent::OpenThread(thread_id) => this.open_thread(*thread_id, window, cx),
            }),
        ];
        Self {
            focus_handle: cx.focus_handle(),
            store,
            registry,
            sidebar,
            switcher_handle: PopoverMenuHandle::default(),
            new_thread_modal: None,
            open_threads: HashMap::default(),
            active_thread: None,
            should_move_window: false,
            _subscriptions: subscriptions,
        }
    }

    fn new_thread(&mut self, _: &NewThread, window: &mut Window, cx: &mut Context<Self>) {
        let store = self.store.read(cx);
        let project_id = match store.scope() {
            ProjectScope::Project(id) => Some(id),
            ProjectScope::All => store.projects().first().map(|project| project.id),
        };
        match project_id {
            Some(project_id) => self.open_new_thread_modal(project_id, window, cx),
            None => window.dispatch_action(Box::new(OpenFolder), cx),
        }
    }

    fn open_new_thread_modal(
        &mut self,
        project_id: ProjectId,
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
            }),
        ];
        self.new_thread_modal = Some((modal, subscriptions));
        cx.notify();
    }

    fn open_thread(&mut self, thread_id: ThreadId, window: &mut Window, cx: &mut Context<Self>) {
        if !self.open_threads.contains_key(&thread_id) {
            let Some(open_thread) = self.start_thread(thread_id, cx) else {
                return;
            };
            self.open_threads.insert(thread_id, open_thread);
        }
        self.active_thread = Some(thread_id);
        self.sidebar.update(cx, |sidebar, cx| {
            sidebar.set_active_thread(Some(thread_id), cx)
        });
        if let Some(open_thread) = self.open_threads.get(&thread_id) {
            window.focus(&open_thread.view.focus_handle(cx), cx);
        }
        cx.notify();
    }

    fn start_thread(&mut self, thread_id: ThreadId, cx: &mut Context<Self>) -> Option<OpenThread> {
        let store = self.store.read(cx);
        let thread = store.thread(thread_id)?.clone();
        let cwd = store.project(thread.project_id)?.path.clone();

        let agent_id = thread.agent_id.clone().map(AgentId::new);
        let agent_name = agent_id
            .as_ref()
            .and_then(|agent_id| self.registry.read(cx).agent(agent_id))
            .map(|agent| agent.name().clone())
            .or_else(|| agent_id.as_ref().map(|agent_id| agent_id.0.clone()))
            .unwrap_or_else(|| "Agent".into());
        let command = agent_id.as_ref().map(|agent_id| {
            self.registry.update(cx, |registry, cx| {
                registry.command_when_loaded(agent_id, cx)
            })
        });

        let agent_thread = cx.new(|cx| match command {
            Some(command) => {
                let previous_session = thread.session_id.clone().map(acp::SessionId::new);
                AgentThread::start(agent_name.clone(), command, cwd, previous_session, cx)
            }
            None => AgentThread::failed(agent_name.clone(), "This thread has no agent."),
        });
        let subscription = cx.subscribe(&agent_thread, move |this, _, event, cx| match event {
            AgentThreadEvent::WorkingChanged(working) => {
                this.store.update(cx, |store, cx| {
                    store.set_thread_working(thread_id, *working, cx)
                });
            }
            AgentThreadEvent::SessionStarted(session_id) => {
                this.store.update(cx, |store, cx| {
                    store.set_thread_session(thread_id, session_id.0.to_string(), cx)
                });
            }
            AgentThreadEvent::TitleChanged(title) => {
                let title = thread_title_from_prompt(title);
                this.store.update(cx, |store, cx| {
                    store.rename_thread(thread_id, title.clone(), cx)
                });
                if let Some(open_thread) = this.open_threads.get(&thread_id) {
                    open_thread
                        .view
                        .update(cx, |view, cx| view.set_title(title.into(), cx));
                }
            }
            AgentThreadEvent::FirstPrompt(prompt) => {
                let title = thread_title_from_prompt(prompt);
                this.store.update(cx, |store, cx| {
                    store.rename_thread(thread_id, title.clone(), cx)
                });
                if let Some(open_thread) = this.open_threads.get(&thread_id) {
                    open_thread
                        .view
                        .update(cx, |view, cx| view.set_title(title.into(), cx));
                }
            }
        });
        let title = SharedString::from(thread.title);
        let registry = self.registry.clone();
        let view = cx.new(|cx| AgentView::new(agent_thread, title, registry, agent_id, cx));
        Some(OpenThread {
            view,
            _subscription: subscription,
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
            store.update(cx, |store, cx| {
                let mut last_added = None;
                for path in paths {
                    last_added = Some(store.add_project(path, cx));
                }
                // In "All projects" the new project simply appears; otherwise switch to it so
                // it doesn't get added out of sight.
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
        let scope_label: SharedString = match store.scope() {
            ProjectScope::All => "All projects".into(),
            ProjectScope::Project(id) => store
                .project(id)
                .map(|project| project.name())
                .unwrap_or_else(|| "All projects".into()),
        };
        let switcher_store = self.store.clone();

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
                                Some(cx.new(|cx| ProjectSwitcher::new(store, window, cx)))
                            })
                            .trigger_with_tooltip(
                                ButtonLike::new("project-switcher-trigger")
                                    .style(ButtonStyle::Outlined)
                                    .child(
                                        h_flex()
                                            .px_1()
                                            .gap_1p5()
                                            .child(
                                                Icon::new(IconName::Folder)
                                                    .size(IconSize::Small)
                                                    .color(Color::Muted),
                                            )
                                            .child(Label::new(scope_label))
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
                    .child(self.sidebar.clone())
                    .child(
                        div()
                            .flex_1()
                            .min_w(SIDEBAR_WIDTH)
                            .h_full()
                            .bg(main_background)
                            .map(|main| match active_view {
                                Some(view) => main.child(view),
                                None => main.child(render_no_thread_selected()),
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

/// The first line of the first prompt, shortened to fit the sidebar.
fn thread_title_from_prompt(prompt: &str) -> String {
    let first_line = prompt.lines().next().unwrap_or_default().trim();
    if first_line.chars().count() <= MAX_THREAD_TITLE_CHARS {
        return first_line.to_string();
    }
    let shortened: String = first_line
        .chars()
        .take(MAX_THREAD_TITLE_CHARS - 1)
        .collect();
    format!("{}…", shortened.trim_end())
}

#[cfg(test)]
mod tests {
    use super::thread_title_from_prompt;

    #[test]
    fn thread_titles() {
        assert_eq!(
            thread_title_from_prompt("Fix the login bug\nmore detail"),
            "Fix the login bug"
        );
        let long = "Build a checkout page with a cart summary, a pay button and order history";
        let title = thread_title_from_prompt(long);
        assert_eq!(title.chars().count(), super::MAX_THREAD_TITLE_CHARS);
        assert!(title.ends_with('…'));
    }
}
