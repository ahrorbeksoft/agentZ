//! Starting a thread: pick the project (only when all projects are shown), then one of the
//! installed agents. Installing agents lives in Settings › Agents.

use gpui::{
    AnyElement, App, Context, DismissEvent, Entity, EventEmitter, FocusHandle, Focusable,
    KeyBinding, ScrollHandle, Subscription, Window,
};
use projects::{ProjectId, ProjectStore, ThreadId};
use registry::{AgentId, AgentRegistryStore, InstallState};
use text_input::{TextInput, TextInputEvent};
use ui::{ButtonLike, ListItem, ListItemSpacing, WithScrollbar as _, prelude::*};

use crate::project_info::{ProjectInfoStore, render_project_icon};
use crate::project_switcher::compact_path;

const KEY_CONTEXT: &str = "NewThreadModal";

pub fn init(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("up", menu::SelectPrevious, Some(KEY_CONTEXT)),
        KeyBinding::new("down", menu::SelectNext, Some(KEY_CONTEXT)),
        KeyBinding::new("enter", menu::Confirm, Some(KEY_CONTEXT)),
        KeyBinding::new("escape", menu::Cancel, Some(KEY_CONTEXT)),
    ]);
}

#[derive(Clone, Copy, PartialEq)]
enum Step {
    Project,
    Agent(ProjectId),
}

pub enum NewThreadModalEvent {
    ThreadCreated(ThreadId),
    OpenAgentSettings,
}

pub struct NewThreadModal {
    projects: Entity<ProjectStore>,
    registry: Entity<AgentRegistryStore>,
    search: Entity<TextInput>,
    step: Step,
    /// Whether the project was left to pick here, so the agent step can go back to it.
    picks_project: bool,
    project_rows: Vec<ProjectId>,
    agent_rows: Vec<AgentId>,
    selected_index: usize,
    scroll_handle: ScrollHandle,
    _subscriptions: Vec<Subscription>,
}

impl EventEmitter<DismissEvent> for NewThreadModal {}
impl EventEmitter<NewThreadModalEvent> for NewThreadModal {}

impl Focusable for NewThreadModal {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.search.focus_handle(cx)
    }
}

impl NewThreadModal {
    /// With no `project_id`, the user picks the project first.
    pub fn new(
        project_id: Option<ProjectId>,
        projects: Entity<ProjectStore>,
        registry: Entity<AgentRegistryStore>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let search = cx.new(|cx| TextInput::new("", cx));
        let subscriptions = vec![
            cx.subscribe(&search, |this, _, _: &TextInputEvent, cx| {
                this.selected_index = 0;
                this.update_rows(cx);
            }),
            cx.observe(&registry, |this, _, cx| this.update_rows(cx)),
            cx.observe(&projects, |this, _, cx| this.update_rows(cx)),
            cx.observe(&ProjectInfoStore::global(cx), |_, _, cx| cx.notify()),
        ];
        window.focus(&search.focus_handle(cx), cx);
        registry.update(cx, |registry, cx| registry.refresh_if_stale(cx));

        let mut this = Self {
            projects,
            registry,
            search,
            step: Step::Project,
            picks_project: project_id.is_none(),
            project_rows: Vec::new(),
            agent_rows: Vec::new(),
            selected_index: 0,
            scroll_handle: ScrollHandle::new(),
            _subscriptions: subscriptions,
        };
        this.go_to(project_id.map_or(Step::Project, Step::Agent), cx);
        this
    }

    fn go_to(&mut self, step: Step, cx: &mut Context<Self>) {
        self.step = step;
        self.selected_index = 0;
        self.scroll_handle.set_offset(gpui::point(px(0.), px(0.)));
        let placeholder = match step {
            Step::Project => "Search projects…",
            Step::Agent(_) => "Search agents…",
        };
        self.search.update(cx, |search, cx| {
            search.set_placeholder(placeholder, cx);
            search.set_text("", cx);
        });
        self.update_rows(cx);
    }

    fn update_rows(&mut self, cx: &mut Context<Self>) {
        let query = self.search.read(cx).text().trim().to_lowercase();
        match self.step {
            Step::Project => {
                self.project_rows = self
                    .projects
                    .read(cx)
                    .visible_projects()
                    .filter(|project| {
                        query.is_empty()
                            || project.name().to_lowercase().contains(&query)
                            || project
                                .path
                                .to_string_lossy()
                                .to_lowercase()
                                .contains(&query)
                    })
                    .map(|project| project.id)
                    .collect();
            }
            Step::Agent(_) => {
                let registry = self.registry.read(cx);
                self.agent_rows = registry
                    .agents()
                    .iter()
                    .filter(|agent| {
                        agent.supports_current_platform()
                            && matches!(
                                registry.install_state(agent.id()),
                                InstallState::Installed { .. }
                            )
                            && (query.is_empty()
                                || agent.name().to_lowercase().contains(&query)
                                || agent.id().0.to_lowercase().contains(&query))
                    })
                    .map(|agent| agent.id().clone())
                    .collect();
            }
        }
        self.selected_index = self.selected_index.min(self.row_count().saturating_sub(1));
        cx.notify();
    }

    fn row_count(&self) -> usize {
        match self.step {
            Step::Project => self.project_rows.len(),
            Step::Agent(_) => self.agent_rows.len(),
        }
    }

    fn select_next(&mut self, _: &menu::SelectNext, _: &mut Window, cx: &mut Context<Self>) {
        let count = self.row_count();
        if count > 0 {
            self.selected_index = (self.selected_index + 1) % count;
            self.scroll_handle.scroll_to_item(self.selected_index);
            cx.notify();
        }
    }

    fn select_previous(
        &mut self,
        _: &menu::SelectPrevious,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let count = self.row_count();
        if count > 0 {
            self.selected_index = self.selected_index.checked_sub(1).unwrap_or(count - 1);
            self.scroll_handle.scroll_to_item(self.selected_index);
            cx.notify();
        }
    }

    fn confirm(&mut self, _: &menu::Confirm, _: &mut Window, cx: &mut Context<Self>) {
        match self.step {
            Step::Project => {
                if let Some(project_id) = self.project_rows.get(self.selected_index).copied() {
                    self.go_to(Step::Agent(project_id), cx);
                }
            }
            Step::Agent(project_id) => {
                if let Some(agent_id) = self.agent_rows.get(self.selected_index).cloned() {
                    self.start_thread(project_id, &agent_id, cx);
                }
            }
        }
    }

    /// Goes back to the project step when there is one, and closes otherwise.
    fn cancel(&mut self, _: &menu::Cancel, _: &mut Window, cx: &mut Context<Self>) {
        if self.picks_project && matches!(self.step, Step::Agent(_)) {
            self.go_to(Step::Project, cx);
        } else {
            cx.emit(DismissEvent);
        }
    }

    fn start_thread(&mut self, project_id: ProjectId, agent_id: &AgentId, cx: &mut Context<Self>) {
        let agent_id = agent_id.0.to_string();
        let thread_id = self.projects.update(cx, |projects, cx| {
            projects.add_thread(project_id, projects::NEW_THREAD_TITLE, Some(agent_id), cx)
        });
        match thread_id {
            Some(thread_id) => cx.emit(NewThreadModalEvent::ThreadCreated(thread_id)),
            None => cx.emit(DismissEvent),
        }
    }

    fn render_project_row(
        &self,
        index: usize,
        project_id: ProjectId,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some(project) = self.projects.read(cx).project(project_id) else {
            return div().into_any_element();
        };
        let info = ProjectInfoStore::global(cx)
            .read(cx)
            .info()
            .get(&project_id);
        ListItem::new(("new-thread-project", index))
            .inset(true)
            .spacing(ListItemSpacing::Sparse)
            .toggle_state(index == self.selected_index)
            .start_slot(render_project_icon(project, info, px(16.), cx))
            .child(
                h_flex()
                    .min_w_0()
                    .gap_2()
                    .child(div().flex_none().child(Label::new(project.name())))
                    .child(
                        div().min_w_0().child(
                            Label::new(compact_path(&project.path))
                                .size(LabelSize::Small)
                                .color(Color::Muted)
                                .truncate(),
                        ),
                    ),
            )
            .on_click(cx.listener(move |this, _, _, cx| this.go_to(Step::Agent(project_id), cx)))
            .into_any_element()
    }

    fn render_agent_row(
        &self,
        index: usize,
        project_id: ProjectId,
        agent_id: AgentId,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let registry = self.registry.read(cx);
        let Some(agent) = registry.agent(&agent_id) else {
            return div().into_any_element();
        };
        let icon = match agent.icon_path() {
            Some(path) => Icon::from_external_svg(path.clone()),
            None => Icon::new(IconName::Terminal),
        };
        let version = match registry.install_state(&agent_id) {
            InstallState::Installed { version, .. } => Some(version),
            _ => None,
        };
        ListItem::new(SharedString::from(format!(
            "new-thread-agent-{}",
            agent_id.0
        )))
        .inset(true)
        .spacing(ListItemSpacing::Sparse)
        .toggle_state(index == self.selected_index)
        .start_slot(icon.color(Color::Muted).size(IconSize::Small))
        .child(Label::new(agent.name().clone()))
        .end_slot::<Label>(version.map(|version| {
            Label::new(format!("v{version}"))
                .size(LabelSize::Small)
                .color(Color::Muted)
        }))
        .on_click(cx.listener(move |this, _, _, cx| this.start_thread(project_id, &agent_id, cx)))
        .into_any_element()
    }

    fn render_header(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let border_variant = cx.theme().colors().border_variant;
        let title: AnyElement = match self.step {
            Step::Project => Label::new("New thread in…")
                .color(Color::Muted)
                .into_any_element(),
            Step::Agent(project_id) => {
                let store = self.projects.read(cx);
                let project = store.project(project_id);
                let info = ProjectInfoStore::global(cx)
                    .read(cx)
                    .info()
                    .get(&project_id);
                h_flex()
                    .gap_1p5()
                    .when(self.picks_project, |row| {
                        row.child(
                            IconButton::new("new-thread-back", IconName::ArrowLeft)
                                .icon_size(IconSize::Small)
                                .on_click(
                                    cx.listener(|this, _, _, cx| this.go_to(Step::Project, cx)),
                                ),
                        )
                    })
                    .children(
                        project.map(|project| render_project_icon(project, info, px(14.), cx)),
                    )
                    .child(
                        Label::new(format!(
                            "New thread in {}",
                            project.map(|project| project.name()).unwrap_or_default()
                        ))
                        .color(Color::Muted),
                    )
                    .into_any_element()
            }
        };
        h_flex()
            .px_3()
            .py_2p5()
            .gap_3()
            .border_b_1()
            .border_color(border_variant)
            .child(div().flex_none().child(title))
            .child(div().flex_1().min_w_0().child(self.search.clone()))
    }

    fn render_agent_empty_state(&self, cx: &mut Context<Self>) -> AnyElement {
        let registry = self.registry.read(cx);
        let has_agents = !registry.agents().is_empty();
        let fetch_error = registry.fetch_error();
        let has_installed = registry.agents().iter().any(|agent| {
            matches!(
                registry.install_state(agent.id()),
                InstallState::Installed { .. }
            )
        });
        let message = if !has_agents && registry.is_fetching() {
            "Loading agents…".to_string()
        } else if let Some(error) = fetch_error.filter(|_| !has_agents) {
            format!("Couldn't load the ACP Registry: {error}")
        } else if has_installed {
            "No matching agents".to_string()
        } else {
            "No agents installed yet. Install one in Settings › Agents.".to_string()
        };
        div()
            .p_3()
            .child(Label::new(message).color(Color::Muted))
            .into_any_element()
    }
}

impl Render for NewThreadModal {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let border_variant = cx.theme().colors().border_variant;
        let rows: Vec<AnyElement> = match self.step {
            Step::Project => self
                .project_rows
                .clone()
                .into_iter()
                .enumerate()
                .map(|(index, project_id)| self.render_project_row(index, project_id, cx))
                .collect(),
            Step::Agent(project_id) => self
                .agent_rows
                .clone()
                .into_iter()
                .enumerate()
                .map(|(index, agent_id)| self.render_agent_row(index, project_id, agent_id, cx))
                .collect(),
        };
        let empty_state = rows.is_empty().then(|| match self.step {
            Step::Project => div()
                .p_3()
                .child(Label::new("No matching projects").color(Color::Muted))
                .into_any_element(),
            Step::Agent(_) => self.render_agent_empty_state(cx),
        });

        v_flex()
            .key_context(KEY_CONTEXT)
            .w(rems(36.))
            .max_h(rems(34.))
            .elevation_3(cx)
            .overflow_hidden()
            .on_action(cx.listener(Self::select_next))
            .on_action(cx.listener(Self::select_previous))
            .on_action(cx.listener(Self::confirm))
            .on_action(cx.listener(Self::cancel))
            .child(self.render_header(cx))
            .child(
                // The scrollbar sits on this non-scrolling wrapper so it stays put, as in Zed.
                div()
                    .id("new-thread-rows-scroll")
                    .child(
                        v_flex()
                            .id("new-thread-rows")
                            .max_h(rems(26.))
                            .overflow_y_scroll()
                            .track_scroll(&self.scroll_handle)
                            .p_1()
                            .children(rows)
                            .children(empty_state),
                    )
                    .vertical_scrollbar_for(&self.scroll_handle, window, cx),
            )
            .when(matches!(self.step, Step::Agent(_)), |modal| {
                modal.child(
                    h_flex()
                        .p_1()
                        .border_t_1()
                        .border_color(border_variant)
                        .child(
                            ButtonLike::new("manage-agents")
                                .full_width()
                                .child(
                                    h_flex()
                                        .w_full()
                                        .px_1()
                                        .gap_2()
                                        .child(
                                            Icon::new(IconName::Sparkle)
                                                .size(IconSize::Small)
                                                .color(Color::Muted),
                                        )
                                        .child(Label::new("Manage Agents…")),
                                )
                                .on_click(cx.listener(|_, _, _, cx| {
                                    cx.emit(NewThreadModalEvent::OpenAgentSettings)
                                })),
                        ),
                )
            })
    }
}
