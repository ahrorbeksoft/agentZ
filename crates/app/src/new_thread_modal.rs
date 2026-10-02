use gpui::{
    AnyElement, App, Context, DismissEvent, Entity, EventEmitter, FocusHandle, Focusable,
    KeyBinding, Subscription, Window,
};
use projects::{ProjectId, ProjectStore, ThreadId};
use registry::{AgentId, AgentRegistryStore, InstallState, RegistryAgent};
use text_input::{TextInput, TextInputEvent};
use ui::{ListItem, ListItemSpacing, Tooltip, prelude::*};

const KEY_CONTEXT: &str = "NewThreadModal";
/// How many registry agents to list before "Browse all" is clicked.
const COLLAPSED_REGISTRY_COUNT: usize = 6;

pub fn init(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("up", menu::SelectPrevious, Some(KEY_CONTEXT)),
        KeyBinding::new("down", menu::SelectNext, Some(KEY_CONTEXT)),
        KeyBinding::new("enter", menu::Confirm, Some(KEY_CONTEXT)),
        KeyBinding::new("escape", menu::Cancel, Some(KEY_CONTEXT)),
    ]);
}

#[derive(Clone)]
enum Row {
    Installed(AgentId),
    Available(AgentId),
}

impl Row {
    fn agent_id(&self) -> &AgentId {
        match self {
            Row::Installed(id) | Row::Available(id) => id,
        }
    }
}

pub enum NewThreadModalEvent {
    ThreadCreated(ThreadId),
}

pub struct NewThreadModal {
    project_id: ProjectId,
    projects: Entity<ProjectStore>,
    registry: Entity<AgentRegistryStore>,
    search: Entity<TextInput>,
    rows: Vec<Row>,
    hidden_registry_count: usize,
    show_all_registry_agents: bool,
    selected_index: usize,
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
    pub fn new(
        project_id: ProjectId,
        projects: Entity<ProjectStore>,
        registry: Entity<AgentRegistryStore>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let search = cx.new(|cx| TextInput::new("Search agents…", cx));
        let subscriptions = vec![
            cx.subscribe(&search, |this, _, _: &TextInputEvent, cx| {
                this.selected_index = 0;
                this.update_rows(cx);
            }),
            cx.observe(&registry, |this, _, cx| this.update_rows(cx)),
        ];
        window.focus(&search.focus_handle(cx), cx);
        registry.update(cx, |registry, cx| registry.refresh_if_stale(cx));

        let mut this = Self {
            project_id,
            projects,
            registry,
            search,
            rows: Vec::new(),
            hidden_registry_count: 0,
            show_all_registry_agents: false,
            selected_index: 0,
            _subscriptions: subscriptions,
        };
        this.update_rows(cx);
        this
    }

    fn update_rows(&mut self, cx: &mut Context<Self>) {
        let query = self.search.read(cx).text().trim().to_lowercase();
        let registry = self.registry.read(cx);

        let mut installed = Vec::new();
        let mut available = Vec::new();
        for agent in registry.agents() {
            if !agent.supports_current_platform() || !matches_query(&query, agent) {
                continue;
            }
            match registry.install_state(agent.id()) {
                InstallState::Installed { .. } => {
                    installed.push(Row::Installed(agent.id().clone()))
                }
                _ => available.push(Row::Available(agent.id().clone())),
            }
        }

        let limit = if self.show_all_registry_agents || !query.is_empty() {
            available.len()
        } else {
            COLLAPSED_REGISTRY_COUNT
        };
        self.hidden_registry_count = available.len().saturating_sub(limit);
        available.truncate(limit);

        self.rows = installed;
        self.rows.extend(available);
        self.selected_index = self.selected_index.min(self.rows.len().saturating_sub(1));
        cx.notify();
    }

    fn select_next(&mut self, _: &menu::SelectNext, _: &mut Window, cx: &mut Context<Self>) {
        if !self.rows.is_empty() {
            self.selected_index = (self.selected_index + 1) % self.rows.len();
            cx.notify();
        }
    }

    fn select_previous(
        &mut self,
        _: &menu::SelectPrevious,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.rows.is_empty() {
            self.selected_index = self
                .selected_index
                .checked_sub(1)
                .unwrap_or(self.rows.len() - 1);
            cx.notify();
        }
    }

    fn confirm(&mut self, _: &menu::Confirm, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(row) = self.rows.get(self.selected_index).cloned() {
            self.activate(row, cx);
        }
    }

    fn cancel(&mut self, _: &menu::Cancel, _: &mut Window, cx: &mut Context<Self>) {
        cx.emit(DismissEvent);
    }

    fn activate(&mut self, row: Row, cx: &mut Context<Self>) {
        match row {
            Row::Installed(agent_id) => self.start_thread(&agent_id, cx),
            Row::Available(agent_id) => self.install(&agent_id, cx),
        }
    }

    fn install(&mut self, agent_id: &AgentId, cx: &mut Context<Self>) {
        self.registry
            .update(cx, |registry, cx| registry.install(agent_id, cx));
    }

    fn start_thread(&mut self, agent_id: &AgentId, cx: &mut Context<Self>) {
        let project_id = self.project_id;
        let agent_id = agent_id.0.to_string();
        let thread_id = self.projects.update(cx, |projects, cx| {
            projects.add_thread(project_id, "New thread", Some(agent_id), cx)
        });
        match thread_id {
            Some(thread_id) => cx.emit(NewThreadModalEvent::ThreadCreated(thread_id)),
            None => cx.emit(DismissEvent),
        }
    }

    fn render_row(&self, index: usize, row: Row, cx: &mut Context<Self>) -> AnyElement {
        let registry = self.registry.read(cx);
        let Some(agent) = registry.agent(row.agent_id()) else {
            return div().into_any_element();
        };
        let install_state = registry.install_state(row.agent_id());
        let agent_id = row.agent_id().clone();
        let element_id = SharedString::from(format!("agent-{}", agent_id.0));

        let icon = match agent.icon_path() {
            Some(path) => Icon::from_external_svg(path.clone()),
            None => Icon::new(IconName::Terminal),
        };

        let end_slot = match install_state {
            InstallState::Installed {
                version,
                update_available,
            } => h_flex()
                .gap_2()
                .child(
                    Label::new(format!("v{version}"))
                        .size(LabelSize::Small)
                        .color(Color::Muted),
                )
                .when(update_available, |slot| {
                    slot.child(
                        Button::new(element_id.clone(), "Update")
                            .style(ButtonStyle::Outlined)
                            .label_size(LabelSize::Small)
                            .on_click(cx.listener({
                                let agent_id = agent_id.clone();
                                move |this, _, _, cx| this.install(&agent_id, cx)
                            })),
                    )
                })
                .into_any_element(),
            InstallState::Installing => Label::new("Installing…")
                .size(LabelSize::Small)
                .color(Color::Muted)
                .into_any_element(),
            InstallState::NotInstalled => Button::new(element_id.clone(), "Install")
                .style(ButtonStyle::Outlined)
                .label_size(LabelSize::Small)
                .on_click(cx.listener(move |this, _, _, cx| this.install(&agent_id, cx)))
                .into_any_element(),
            InstallState::Failed(error) => Button::new(element_id.clone(), "Retry")
                .style(ButtonStyle::Outlined)
                .label_size(LabelSize::Small)
                .color(Color::Error)
                .tooltip(Tooltip::text(error))
                .on_click(cx.listener(move |this, _, _, cx| this.install(&agent_id, cx)))
                .into_any_element(),
        };

        let is_installed = matches!(row, Row::Installed(_));
        ListItem::new(element_id)
            .inset(true)
            .spacing(ListItemSpacing::Sparse)
            .toggle_state(index == self.selected_index)
            .start_slot(icon.color(Color::Muted).size(IconSize::Small))
            .child(
                v_flex()
                    .min_w_0()
                    .child(
                        Label::new(agent.name().clone())
                            .when(!is_installed, |label| label.color(Color::Muted)),
                    )
                    .when(!is_installed, |column| {
                        column.child(
                            Label::new(agent.description().clone())
                                .size(LabelSize::Small)
                                .color(Color::Placeholder)
                                .truncate(),
                        )
                    }),
            )
            .end_slot(end_slot)
            .on_click(cx.listener(move |this, _, _, cx| this.activate(row.clone(), cx)))
            .into_any_element()
    }
}

impl Render for NewThreadModal {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let border_variant = cx.theme().colors().border_variant;
        let project_name = self
            .projects
            .read(cx)
            .project(self.project_id)
            .map(|project| project.name())
            .unwrap_or_default();
        let registry = self.registry.read(cx);
        let is_fetching = registry.is_fetching();
        let fetch_error = registry.fetch_error();
        let has_agents = !registry.agents().is_empty();

        let installed_count = self
            .rows
            .iter()
            .filter(|row| matches!(row, Row::Installed(_)))
            .count();
        let mut installed_rows = Vec::new();
        let mut available_rows = Vec::new();
        for (index, row) in self.rows.clone().into_iter().enumerate() {
            let element = self.render_row(index, row, cx);
            if index < installed_count {
                installed_rows.push(element);
            } else {
                available_rows.push(element);
            }
        }

        let section_header = |title: &'static str| {
            div()
                .px_2()
                .pt_2()
                .pb_1()
                .child(Label::new(title).size(LabelSize::Small).color(Color::Muted))
        };

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
            .child(
                h_flex()
                    .px_3()
                    .py_2p5()
                    .gap_3()
                    .border_b_1()
                    .border_color(border_variant)
                    .child(div().flex_none().child(
                        Label::new(format!("New thread in {project_name}")).color(Color::Muted),
                    ))
                    .child(self.search.clone()),
            )
            .child(
                v_flex()
                    .id("new-thread-agents")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .p_1()
                    .when(!installed_rows.is_empty(), |list| {
                        list.child(section_header("Installed"))
                            .children(installed_rows)
                    })
                    .when(!available_rows.is_empty(), |list| {
                        list.child(
                            h_flex()
                                .justify_between()
                                .child(section_header("From the ACP Registry"))
                                .when(self.hidden_registry_count > 0, |row| {
                                    row.child(
                                        Button::new("browse-all", "Browse all")
                                            .label_size(LabelSize::Small)
                                            .color(Color::Accent)
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.show_all_registry_agents = true;
                                                this.update_rows(cx);
                                            })),
                                    )
                                }),
                        )
                        .children(available_rows)
                    })
                    .when(self.rows.is_empty(), |list| {
                        let message = if is_fetching && !has_agents {
                            "Loading agents from the ACP Registry…".to_string()
                        } else if let Some(error) = fetch_error.as_ref().filter(|_| !has_agents) {
                            format!("Couldn't load the ACP Registry: {error}")
                        } else {
                            "No matching agents".to_string()
                        };
                        list.child(
                            v_flex()
                                .p_3()
                                .gap_2()
                                .child(Label::new(message).color(Color::Muted))
                                .when(fetch_error.is_some() && !has_agents, |column| {
                                    column.child(
                                        Button::new("retry-registry", "Retry")
                                            .style(ButtonStyle::Outlined)
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.registry.update(cx, |registry, cx| {
                                                    registry.refresh(cx)
                                                });
                                            })),
                                    )
                                }),
                        )
                    }),
            )
    }
}

fn matches_query(query: &str, agent: &RegistryAgent) -> bool {
    query.is_empty()
        || agent.name().to_lowercase().contains(query)
        || agent.id().0.to_lowercase().contains(query)
}
