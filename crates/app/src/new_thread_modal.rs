//! Starting a thread: pick the project (only when all projects are shown), then which of its
//! checkouts when it's combined from several (t3code's environment picker), then one of the
//! installed agents or a terminal (a login shell),
//! then for a git repository where it works (t3code's workspace menu): the
//! checkout, a new pasture or worktree, or one of the project's existing ones. Installing
//! agents lives in Settings › Agents.

use std::path::PathBuf;

use crate::machines::{GroupKey, MachineId, Machines, ProjectKey, ThreadKey};
use crate::project_store::ProjectStore;
use agentz_protocol::agents::{AgentId, InstallState};
use agentz_protocol::terminal::TerminalCommand;
use agentz_protocol::workspace::{PastureSupport, ProjectGit, WorkspaceChoice};
use gpui::{
    AnyElement, App, Context, DismissEvent, Entity, EventEmitter, FocusHandle, Focusable,
    KeyBinding, ScrollHandle, Subscription, Task, Window,
};
use projects::{ProjectId, Workspace, WorkspaceKind};
use text_input::{TextInput, TextInputEvent};
use ui::{
    ButtonLike, CommonAnimationExt as _, ContextMenu, DropdownMenu, IconPosition, ListItem,
    ListItemSpacing, WithScrollbar as _, prelude::*,
};

use crate::project_info::{ProjectInfoStore, render_project_icon, workspace_icon};
use crate::project_switcher::compact_path;
use crate::registry_store::AgentRegistryStore;

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
    /// The checkout of the chosen group, on whichever machine.
    Machine,
    Agent(ProjectId),
    Workspace(ProjectId),
}

/// What the thread runs.
#[derive(Clone, Debug, PartialEq)]
enum Starter {
    Agent(AgentId),
    Terminal(TerminalCommand),
}

#[derive(Clone)]
enum WorkspaceRow {
    Checkout,
    New(WorkspaceKind),
    Existing(Workspace),
}

pub enum NewThreadModalEvent {
    ThreadCreated(ThreadKey),
    OpenAgentSettings,
}

pub struct NewThreadModal {
    machines: Entity<Machines>,
    /// The chosen project's machine, whose agents and terminals the agent step offers.
    machine: MachineId,
    projects: Entity<ProjectStore>,
    registry: Entity<AgentRegistryStore>,
    search: Entity<TextInput>,
    step: Step,
    /// Whether the project was left to pick here, so the agent step can go back to it.
    picks_project: bool,
    /// The combined project whose checkout the machine step picks, kept so the agent step
    /// can go back to it.
    group: Option<GroupKey>,
    project_rows: Vec<GroupKey>,
    machine_rows: Vec<ProjectKey>,
    agent_rows: Vec<Starter>,
    workspace_rows: Vec<WorkspaceRow>,
    selected_index: usize,
    scroll_handle: ScrollHandle,
    /// Set by "New thread in this workspace", which skips the workspace step.
    preset_workspace: Option<PathBuf>,
    agent: Option<Starter>,
    /// The project's repository, loaded when its agent step opens.
    git: Option<(ProjectId, ProjectGit)>,
    /// What a new worktree or pasture starts from, when not the checkout's branch.
    base_branch: Option<String>,
    /// What's being made while the thread's workspace is created.
    creating: Option<SharedString>,
    error: Option<SharedString>,
    _load_git: Task<()>,
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
    /// With no `project_id`, the user picks the project first. With `workspace`, the thread
    /// works in that folder of the project.
    pub fn new(
        project: Option<ProjectKey>,
        workspace: Option<PathBuf>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let machines = Machines::global(cx);
        let machine = project.map_or(MachineId::Local, |project| project.machine);
        let client = machines
            .read(cx)
            .client(machine, cx)
            .unwrap_or_else(|| Machines::local(cx));
        let projects = client.read(cx).projects().clone();
        let registry = client.read(cx).registry().clone();
        let search = cx.new(|cx| TextInput::new("", cx));
        let subscriptions = vec![
            cx.subscribe(&search, |this, _, _: &TextInputEvent, cx| {
                this.selected_index = 0;
                this.update_rows(cx);
            }),
            cx.observe(&machines, |this, _, cx| this.update_rows(cx)),
            cx.observe(&ProjectInfoStore::global(cx), |_, _, cx| cx.notify()),
        ];
        window.focus(&search.focus_handle(cx), cx);

        let mut this = Self {
            machines,
            machine: client.read(cx).machine(),
            projects,
            registry,
            search,
            step: Step::Project,
            picks_project: project.is_none(),
            group: None,
            project_rows: Vec::new(),
            machine_rows: Vec::new(),
            agent_rows: Vec::new(),
            workspace_rows: Vec::new(),
            selected_index: 0,
            scroll_handle: ScrollHandle::new(),
            preset_workspace: workspace,
            agent: None,
            git: None,
            base_branch: None,
            creating: None,
            error: None,
            _load_git: Task::ready(()),
            _subscriptions: subscriptions,
        };
        let only_group = match this.machines.read(cx).visible_groups(cx).as_slice() {
            [group] => Some(group.key.clone()),
            _ => None,
        };
        match (project, only_group) {
            (Some(project), _) => this.choose_project(project, cx),
            // With one project shown, there's only its checkout to choose.
            (None, Some(group)) => {
                this.picks_project = false;
                this.choose_group(group, cx);
            }
            (None, None) => this.go_to(Step::Project, cx),
        }
        this
    }

    /// Asks which checkout when the project is combined from several, defaulting to the one
    /// used last.
    fn choose_group(&mut self, key: GroupKey, cx: &mut Context<Self>) {
        let machines = self.machines.read(cx);
        let Some(group) = machines.group(&key, cx) else {
            return;
        };
        if let [(machine, project)] = group.members.as_slice() {
            let project = ProjectKey {
                machine: *machine,
                project: project.id,
            };
            self.group = None;
            self.choose_project(project, cx);
            return;
        }
        let last_used = machines.last_used_member(&group, cx);
        self.group = Some(key);
        self.go_to(Step::Machine, cx);
        if let Some(index) = last_used.and_then(|last_used| {
            self.machine_rows
                .iter()
                .position(|member| *member == last_used)
        }) {
            self.selected_index = index;
        }
    }

    /// Offers the project's machine's agents and terminals.
    fn choose_project(&mut self, project: ProjectKey, cx: &mut Context<Self>) {
        let client = self.machines.read(cx).client(project.machine, cx);
        if let Some(client) = client
            && client.read(cx).machine() != self.machine
        {
            self.machine = client.read(cx).machine();
            self.projects = client.read(cx).projects().clone();
            self.registry = client.read(cx).registry().clone();
            self.git = None;
        }
        self.registry
            .update(cx, |registry, cx| registry.refresh_if_stale(cx));
        self.go_to(Step::Agent(project.project), cx);
    }

    fn go_to(&mut self, step: Step, cx: &mut Context<Self>) {
        self.step = step;
        self.selected_index = 0;
        self.error = None;
        self.scroll_handle.set_offset(gpui::point(px(0.), px(0.)));
        if let Step::Agent(project_id) = step {
            self.load_git(project_id, cx);
        }
        let placeholder = match step {
            Step::Project => "Search projects…",
            Step::Machine => "Search machines…",
            Step::Agent(_) => "Search agents…",
            Step::Workspace(_) => "Search workspaces…",
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
                let machines = self.machines.read(cx);
                let member_matches = |machine: MachineId, project: &projects::Project| {
                    project
                        .path
                        .to_string_lossy()
                        .to_lowercase()
                        .contains(&query)
                        || (machine != MachineId::Local
                            && machines.label(machine, cx).to_lowercase().contains(&query))
                };
                self.project_rows = machines
                    .visible_groups(cx)
                    .into_iter()
                    .filter(|group| {
                        query.is_empty()
                            || group.name().to_lowercase().contains(&query)
                            || group
                                .members
                                .iter()
                                .any(|(machine, project)| member_matches(*machine, project))
                    })
                    .map(|group| group.key)
                    .collect();
            }
            Step::Machine => {
                let machines = self.machines.read(cx);
                let members = self
                    .group
                    .as_ref()
                    .and_then(|key| machines.group(key, cx))
                    .map(|group| group.members)
                    .unwrap_or_default();
                self.machine_rows = members
                    .into_iter()
                    .filter(|(machine, project)| {
                        query.is_empty()
                            || machines.label(*machine, cx).to_lowercase().contains(&query)
                            || project
                                .path
                                .to_string_lossy()
                                .to_lowercase()
                                .contains(&query)
                    })
                    .map(|(machine, project)| ProjectKey {
                        machine,
                        project: project.id,
                    })
                    .collect();
            }
            Step::Agent(_) => {
                let registry = self.registry.read(cx);
                let matches = |text: &str| query.is_empty() || text.to_lowercase().contains(&query);
                let agents = registry
                    .agents()
                    .iter()
                    .filter(|agent| {
                        agent.supports_current_platform()
                            && matches!(
                                registry.install_state(agent.id()),
                                InstallState::Installed { .. }
                            )
                            && (matches(agent.name()) || matches(&agent.id().0))
                    })
                    .map(|agent| Starter::Agent(agent.id().clone()));
                let shell = matches("terminal shell")
                    .then(|| Starter::Terminal(TerminalCommand::default()));
                self.agent_rows = agents.chain(shell).collect();
            }
            Step::Workspace(project_id) => {
                let store = self.projects.read(cx);
                let heads = ProjectInfoStore::global(cx).read(cx);
                let existing = store
                    .project(project_id)
                    .map(|project| project.workspaces.clone())
                    .unwrap_or_default();
                let matches = |text: &str| query.is_empty() || text.to_lowercase().contains(&query);
                self.workspace_rows = [
                    (WorkspaceRow::Checkout, "current checkout".to_string()),
                    (
                        WorkspaceRow::New(WorkspaceKind::Pasture),
                        "new pasture".to_string(),
                    ),
                    (
                        WorkspaceRow::New(WorkspaceKind::Worktree),
                        "new worktree".to_string(),
                    ),
                ]
                .into_iter()
                .chain(existing.into_iter().map(|workspace| {
                    let branch = heads
                        .workspace_head(self.machine, &workspace.path)
                        .map(|head| head.branch.clone())
                        .or_else(|| workspace.branch.clone())
                        .unwrap_or_default();
                    let text = format!(
                        "{} {branch} {}",
                        workspace.kind.label(),
                        workspace.path.display()
                    );
                    (WorkspaceRow::Existing(workspace), text)
                }))
                .filter(|(_, text)| matches(text))
                .map(|(row, _)| row)
                .collect();
            }
        }
        self.selected_index = self.selected_index.min(self.row_count().saturating_sub(1));
        cx.notify();
    }

    fn row_count(&self) -> usize {
        match self.step {
            Step::Project => self.project_rows.len(),
            Step::Machine => self.machine_rows.len(),
            Step::Agent(_) => self.agent_rows.len(),
            Step::Workspace(_) => self.workspace_rows.len(),
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
                if let Some(key) = self.project_rows.get(self.selected_index).cloned() {
                    self.confirm_group(key, cx);
                }
            }
            Step::Machine => {
                if let Some(project) = self.machine_rows.get(self.selected_index).copied()
                    && self.machines.read(cx).is_online(project.machine, cx)
                {
                    self.choose_project(project, cx);
                }
            }
            Step::Agent(project_id) => {
                if let Some(starter) = self.agent_rows.get(self.selected_index).cloned() {
                    self.choose_agent(project_id, starter, cx);
                }
            }
            Step::Workspace(project_id) => {
                if let Some(row) = self.workspace_rows.get(self.selected_index).cloned() {
                    self.choose_workspace(project_id, row, cx);
                }
            }
        }
    }

    /// A project row's choice, unless every machine it's on is offline.
    fn confirm_group(&mut self, key: GroupKey, cx: &mut Context<Self>) {
        let machines = self.machines.read(cx);
        if machines
            .group(&key, cx)
            .is_some_and(|group| !machines.is_group_offline(&group, cx))
        {
            self.choose_group(key, cx);
        }
    }

    /// Goes back a step when there is one, and closes otherwise.
    fn cancel(&mut self, _: &menu::Cancel, _: &mut Window, cx: &mut Context<Self>) {
        if self.creating.is_some() {
            return;
        }
        if !self.go_back(cx) {
            cx.emit(DismissEvent);
        }
    }

    /// Returns whether there was a step to go back to.
    fn go_back(&mut self, cx: &mut Context<Self>) -> bool {
        let previous = match self.step {
            Step::Workspace(project_id) => Step::Agent(project_id),
            Step::Agent(_) if self.group.is_some() => Step::Machine,
            Step::Agent(_) | Step::Machine if self.picks_project => Step::Project,
            _ => return false,
        };
        self.go_to(previous, cx);
        true
    }

    fn load_git(&mut self, project_id: ProjectId, cx: &mut Context<Self>) {
        if self.preset_workspace.is_some()
            || self
                .git
                .as_ref()
                .is_some_and(|(loaded, _)| *loaded == project_id)
        {
            return;
        }
        self.git = None;
        self.base_branch = None;
        let git = self.projects.read(cx).project_git(project_id, cx);
        self._load_git = cx.spawn(async move |this, cx| {
            // An older server, or one that can't read the repository, offers only the checkout.
            let git = git.await.unwrap_or_default();
            this.update(cx, |this, cx| {
                let is_repository = git.is_repository;
                this.git = Some((project_id, git));
                if this.step == Step::Workspace(project_id)
                    && !is_repository
                    && let Some(starter) = this.agent.clone()
                {
                    this.start_thread(project_id, starter, WorkspaceChoice::Checkout, cx);
                }
                cx.notify();
            })
            .ok();
        });
    }

    /// Asks where the thread works, unless that's settled: by "New thread in this workspace",
    /// or because the project isn't a git repository.
    fn choose_agent(&mut self, project_id: ProjectId, starter: Starter, cx: &mut Context<Self>) {
        if let Some(path) = self.preset_workspace.clone() {
            let is_checkout = self
                .projects
                .read(cx)
                .project(project_id)
                .is_some_and(|project| project.path == path);
            let choice = if is_checkout {
                WorkspaceChoice::Checkout
            } else {
                WorkspaceChoice::Existing(path)
            };
            self.start_thread(project_id, starter, choice, cx);
            return;
        }
        let is_repository = self
            .git
            .as_ref()
            .filter(|(loaded, _)| *loaded == project_id)
            .map(|(_, git)| git.is_repository);
        if is_repository == Some(false) {
            self.start_thread(project_id, starter, WorkspaceChoice::Checkout, cx);
            return;
        }
        self.agent = Some(starter);
        self.go_to(Step::Workspace(project_id), cx);
    }

    fn choose_workspace(
        &mut self,
        project_id: ProjectId,
        row: WorkspaceRow,
        cx: &mut Context<Self>,
    ) {
        let Some(starter) = self.agent.clone() else {
            return;
        };
        let choice = match row {
            WorkspaceRow::Checkout => WorkspaceChoice::Checkout,
            WorkspaceRow::New(kind) => {
                if kind == WorkspaceKind::Pasture
                    && let PastureSupport::Unsupported(_) = self.pasture_support()
                {
                    return;
                }
                WorkspaceChoice::New {
                    kind,
                    base: self.base_branch.clone(),
                    branch: None,
                }
            }
            WorkspaceRow::Existing(workspace) => WorkspaceChoice::Existing(workspace.path),
        };
        self.start_thread(project_id, starter, choice, cx);
    }

    fn pasture_support(&self) -> PastureSupport {
        self.git
            .as_ref()
            .map(|(_, git)| git.pastures.clone())
            .unwrap_or(PastureSupport::Unknown(serde_json::Value::Null))
    }

    /// The branch a new worktree or pasture starts from.
    fn base_branch(&self) -> Option<String> {
        self.base_branch
            .clone()
            .or_else(|| self.git.as_ref().and_then(|(_, git)| git.branch.clone()))
    }

    fn start_thread(
        &mut self,
        project_id: ProjectId,
        starter: Starter,
        workspace: WorkspaceChoice,
        cx: &mut Context<Self>,
    ) {
        if self.creating.is_some() {
            return;
        }
        if let WorkspaceChoice::New { kind, .. } = &workspace {
            self.creating = Some(format!("Making a {}…", kind.label().to_lowercase()).into());
            self.error = None;
            cx.notify();
        }
        let created = self.projects.update(cx, |projects, cx| match starter {
            Starter::Agent(agent_id) => projects.create_thread(project_id, agent_id, workspace, cx),
            Starter::Terminal(command) => {
                projects.create_terminal_thread(project_id, command, workspace, cx)
            }
        });
        cx.spawn(async move |this, cx| {
            let created = created.await;
            this.update(cx, |this, cx| match created {
                Ok(thread) => cx.emit(NewThreadModalEvent::ThreadCreated(ThreadKey {
                    machine: this.machine,
                    thread,
                })),
                // Making a workspace can fail for reasons worth reading, like a dirty
                // submodule; the step stays open to try another choice.
                Err(error) if this.creating.is_some() => {
                    this.creating = None;
                    this.error = Some(format!("{error:#}").into());
                    cx.notify();
                }
                Err(error) => {
                    log::error!("failed to create the thread: {error:#}");
                    cx.emit(DismissEvent);
                }
            })
            .ok();
        })
        .detach();
    }

    fn render_project_row(
        &self,
        index: usize,
        key: GroupKey,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let machines = self.machines.read(cx);
        let Some(group) = machines.group(&key, cx) else {
            return div().into_any_element();
        };
        let Some((machine, project)) = group.primary() else {
            return div().into_any_element();
        };
        let info = ProjectInfoStore::global(cx)
            .read(cx)
            .info(machine, project.id);
        let is_offline = machines.is_group_offline(&group, cx);
        let detail = match group.members.as_slice() {
            [(MachineId::Local, project)] => compact_path(&project.path),
            [(machine, project)] => {
                format!(
                    "{}: {}",
                    machines.label(*machine, cx),
                    project.path.display()
                )
            }
            members => match machines.group_machines_label(&group, cx) {
                Some(label) => label.to_string(),
                None => format!("{} checkouts", members.len()),
            },
        };
        let detail = if is_offline {
            format!("{detail} (offline)")
        } else {
            detail
        };
        let name = group.name();
        ListItem::new(("new-thread-project", index))
            .inset(true)
            .spacing(ListItemSpacing::Sparse)
            .toggle_state(index == self.selected_index)
            .disabled(is_offline)
            .start_slot(render_project_icon(project, info, px(16.), cx))
            .child(
                h_flex()
                    .min_w_0()
                    .gap_2()
                    .child(div().flex_none().child(
                        Label::new(name).when(is_offline, |label| label.color(Color::Disabled)),
                    ))
                    .child(
                        div().min_w_0().child(
                            Label::new(detail)
                                .size(LabelSize::Small)
                                .color(Color::Muted)
                                .truncate(),
                        ),
                    ),
            )
            .on_click(cx.listener(move |this, _, _, cx| this.confirm_group(key.clone(), cx)))
            .into_any_element()
    }

    /// A checkout of the combined project: its machine, then its folder.
    fn render_machine_row(
        &self,
        index: usize,
        key: ProjectKey,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let machines = self.machines.read(cx);
        let Some(project) = machines
            .projects(key.machine, cx)
            .and_then(|store| store.read(cx).project(key.project).cloned())
        else {
            return div().into_any_element();
        };
        let is_offline = !machines.is_online(key.machine, cx);
        let label = machines.label(key.machine, cx);
        let path = match key.machine {
            MachineId::Local => compact_path(&project.path),
            MachineId::Remote(_) => project.path.display().to_string(),
        };
        let path = if is_offline {
            format!("{path} (offline)")
        } else {
            path
        };
        let icon = self.machines.read(cx).machine_icon(key.machine, cx);
        ListItem::new(("new-thread-machine", index))
            .inset(true)
            .spacing(ListItemSpacing::Sparse)
            .toggle_state(index == self.selected_index)
            .disabled(is_offline)
            .start_slot(Icon::new(icon).size(IconSize::Small).color(Color::Muted))
            .child(
                h_flex()
                    .min_w_0()
                    .gap_2()
                    .child(div().flex_none().child(
                        Label::new(label).when(is_offline, |label| label.color(Color::Disabled)),
                    ))
                    .child(
                        div().min_w_0().child(
                            Label::new(path)
                                .size(LabelSize::Small)
                                .color(Color::Muted)
                                .truncate(),
                        ),
                    ),
            )
            .on_click(cx.listener(move |this, _, _, cx| {
                if this.machines.read(cx).is_online(key.machine, cx) {
                    this.choose_project(key, cx)
                }
            }))
            .into_any_element()
    }

    fn render_starter_row(
        &self,
        index: usize,
        project_id: ProjectId,
        starter: Starter,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        match starter {
            Starter::Agent(agent_id) => self.render_agent_row(index, project_id, agent_id, cx),
            Starter::Terminal(command) => self.render_terminal_row(index, project_id, command, cx),
        }
    }

    fn render_terminal_row(
        &self,
        index: usize,
        project_id: ProjectId,
        command: TerminalCommand,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let (label, detail): (SharedString, SharedString) = match &command.command {
            None => ("Terminal".into(), "A login shell".into()),
            Some(name) => (name.clone().into(), "In a terminal".into()),
        };
        let id = SharedString::from(format!(
            "new-thread-terminal-{}",
            command.command.as_deref().unwrap_or("shell")
        ));
        ListItem::new(id)
            .inset(true)
            .spacing(ListItemSpacing::Sparse)
            .toggle_state(index == self.selected_index)
            .start_slot(
                Icon::new(IconName::Terminal)
                    .color(Color::Muted)
                    .size(IconSize::Small),
            )
            .child(
                h_flex()
                    .min_w_0()
                    .gap_2()
                    .child(div().flex_none().child(Label::new(label)))
                    .child(
                        div().min_w_0().child(
                            Label::new(detail)
                                .size(LabelSize::Small)
                                .color(Color::Muted)
                                .truncate(),
                        ),
                    ),
            )
            .on_click(cx.listener(move |this, _, _, cx| {
                this.choose_agent(project_id, Starter::Terminal(command.clone()), cx)
            }))
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
        .on_click(cx.listener(move |this, _, _, cx| {
            this.choose_agent(project_id, Starter::Agent(agent_id.clone()), cx)
        }))
        .into_any_element()
    }

    fn render_workspace_row(
        &self,
        index: usize,
        project_id: ProjectId,
        row: WorkspaceRow,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let checkout_branch = self.git.as_ref().and_then(|(_, git)| git.branch.clone());
        let (icon, label, detail, detail_color, end, disabled): (
            IconName,
            SharedString,
            Option<String>,
            Color,
            Option<String>,
            bool,
        ) = match &row {
            WorkspaceRow::Checkout => (
                IconName::Folder,
                "Current checkout".into(),
                checkout_branch,
                Color::Muted,
                None,
                false,
            ),
            WorkspaceRow::New(WorkspaceKind::Pasture) => {
                let (detail, color, disabled) = match self.pasture_support() {
                    PastureSupport::CopyOnWrite => (
                        "A copy-on-write clone of the folder, with dependencies and .env".into(),
                        Color::Muted,
                        false,
                    ),
                    PastureSupport::FullCopy => (
                        "A full copy here: slow, and as big as the project".into(),
                        Color::Warning,
                        false,
                    ),
                    PastureSupport::Unsupported(reason) => (reason, Color::Muted, true),
                    PastureSupport::Unknown(_) => {
                        ("Checking the repository…".into(), Color::Muted, true)
                    }
                };
                (
                    workspace_icon(WorkspaceKind::Pasture),
                    "New pasture".into(),
                    Some(detail),
                    color,
                    None,
                    disabled,
                )
            }
            WorkspaceRow::New(WorkspaceKind::Worktree) => (
                workspace_icon(WorkspaceKind::Worktree),
                "New worktree".into(),
                Some("A git worktree: tracked files only".into()),
                Color::Muted,
                None,
                self.git.is_none(),
            ),
            WorkspaceRow::Existing(workspace) => {
                let branch = ProjectInfoStore::global(cx)
                    .read(cx)
                    .workspace_head(self.machine, &workspace.path)
                    .map(|head| head.branch.clone())
                    .or_else(|| workspace.branch.clone())
                    .unwrap_or_else(|| workspace.kind.label().to_string());
                // Only this Mac's folders can be checked from here.
                let is_missing = self.machine == MachineId::Local && !workspace.path.exists();
                (
                    workspace_icon(workspace.kind),
                    branch.into(),
                    Some(compact_path(&workspace.path)),
                    Color::Muted,
                    Some(workspace.kind.label().to_string()),
                    is_missing,
                )
            }
        };
        ListItem::new(("new-thread-workspace", index))
            .inset(true)
            .spacing(ListItemSpacing::Sparse)
            .toggle_state(index == self.selected_index)
            .disabled(disabled || self.creating.is_some())
            .start_slot(Icon::new(icon).size(IconSize::Small).color(Color::Muted))
            .child(
                h_flex()
                    .min_w_0()
                    .gap_2()
                    .child(div().flex_none().child(
                        Label::new(label).when(disabled, |label| label.color(Color::Disabled)),
                    ))
                    .children(detail.map(|detail| {
                        div().min_w_0().child(
                            Label::new(detail)
                                .size(LabelSize::Small)
                                .color(detail_color)
                                .truncate(),
                        )
                    })),
            )
            .end_slot::<Label>(
                end.map(|end| Label::new(end).size(LabelSize::Small).color(Color::Muted)),
            )
            .on_click(
                cx.listener(move |this, _, _, cx| {
                    this.choose_workspace(project_id, row.clone(), cx)
                }),
            )
            .into_any_element()
    }

    /// The base branch for a new worktree or pasture, or what's being made.
    fn render_workspace_footer(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let border_variant = cx.theme().colors().border_variant;
        let footer = h_flex()
            .px_3()
            .py_1p5()
            .gap_2()
            .border_t_1()
            .border_color(border_variant);
        if let Some(creating) = &self.creating {
            return footer
                .child(
                    Icon::new(IconName::LoadCircle)
                        .size(IconSize::Small)
                        .color(Color::Muted)
                        .with_rotate_animation(2),
                )
                .child(Label::new(creating.clone()).color(Color::Muted))
                .into_any_element();
        }
        let branches = self
            .git
            .as_ref()
            .map(|(_, git)| git.branches.clone())
            .unwrap_or_default();
        let current = self.base_branch();
        let this = cx.entity().downgrade();
        let menu = ContextMenu::build(window, cx, {
            let current = current.clone();
            move |mut menu, _, _| {
                for branch in branches {
                    let this = this.clone();
                    let is_current = current.as_deref() == Some(branch.as_str());
                    menu = menu.toggleable_entry(
                        branch.clone(),
                        is_current,
                        IconPosition::End,
                        None,
                        move |_, cx| {
                            this.update(cx, |this, cx| {
                                this.base_branch = Some(branch.clone());
                                cx.notify();
                            })
                            .ok();
                        },
                    );
                }
                menu
            }
        });
        footer
            .child(
                Label::new("New pastures and worktrees start from")
                    .size(LabelSize::Small)
                    .color(Color::Muted),
            )
            .child(DropdownMenu::new(
                "new-thread-base-branch",
                current.unwrap_or_else(|| "HEAD".to_string()),
                menu,
            ))
            .into_any_element()
    }

    fn render_header(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let border_variant = cx.theme().colors().border_variant;
        let back_button = |cx: &mut Context<Self>| {
            IconButton::new("new-thread-back", IconName::ArrowLeft)
                .icon_size(IconSize::Small)
                .on_click(cx.listener(|this, _, _, cx| {
                    if this.creating.is_none() {
                        this.go_back(cx);
                    }
                }))
        };
        let title: AnyElement = match self.step {
            Step::Project => Label::new("New thread in…")
                .color(Color::Muted)
                .into_any_element(),
            Step::Machine => {
                let machines = self.machines.read(cx);
                let group = self.group.as_ref().and_then(|key| machines.group(key, cx));
                let icon = group.as_ref().and_then(|group| {
                    let (machine, project) = group.primary()?;
                    let info = ProjectInfoStore::global(cx)
                        .read(cx)
                        .info(machine, project.id);
                    Some(render_project_icon(project, info, px(14.), cx))
                });
                h_flex()
                    .gap_1p5()
                    .when(self.picks_project, |row| row.child(back_button(cx)))
                    .children(icon)
                    .child(
                        Label::new(format!(
                            "New thread in {} on…",
                            group.map(|group| group.name()).unwrap_or_default()
                        ))
                        .color(Color::Muted),
                    )
                    .into_any_element()
            }
            Step::Agent(project_id) | Step::Workspace(project_id) => {
                let back = back_button(cx);
                let store = self.projects.read(cx);
                let project = store.project(project_id);
                let info = ProjectInfoStore::global(cx)
                    .read(cx)
                    .info(self.machine, project_id);
                let machine_label = (self.machine != MachineId::Local)
                    .then(|| self.machines.read(cx).label(self.machine, cx));
                h_flex()
                    .gap_1p5()
                    .when(
                        self.picks_project
                            || self.group.is_some()
                            || matches!(self.step, Step::Workspace(_)),
                        |row| row.child(back),
                    )
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
                    .children(machine_label.map(|label| {
                        Label::new(format!("on {label}"))
                            .size(LabelSize::Small)
                            .color(Color::Muted)
                    }))
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
                .map(|(index, project)| self.render_project_row(index, project, cx))
                .collect(),
            Step::Machine => self
                .machine_rows
                .clone()
                .into_iter()
                .enumerate()
                .map(|(index, project)| self.render_machine_row(index, project, cx))
                .collect(),
            Step::Agent(project_id) => self
                .agent_rows
                .clone()
                .into_iter()
                .enumerate()
                .map(|(index, starter)| self.render_starter_row(index, project_id, starter, cx))
                .collect(),
            Step::Workspace(project_id) => self
                .workspace_rows
                .clone()
                .into_iter()
                .enumerate()
                .map(|(index, row)| self.render_workspace_row(index, project_id, row, cx))
                .collect(),
        };
        let empty_state = rows.is_empty().then(|| match self.step {
            Step::Project => div()
                .p_3()
                .child(Label::new("No matching projects").color(Color::Muted))
                .into_any_element(),
            Step::Machine => div()
                .p_3()
                .child(Label::new("No matching machines").color(Color::Muted))
                .into_any_element(),
            Step::Agent(_) => self.render_agent_empty_state(cx),
            Step::Workspace(_) => div()
                .p_3()
                .child(Label::new("No matching workspaces").color(Color::Muted))
                .into_any_element(),
        });
        let error = self.error.clone().map(|error| {
            div()
                .px_3()
                .pb_2()
                .child(Label::new(error).size(LabelSize::Small).color(Color::Error))
        });
        let workspace_footer = matches!(self.step, Step::Workspace(_))
            .then(|| self.render_workspace_footer(window, cx));

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
            .children(error)
            .children(workspace_footer)
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
