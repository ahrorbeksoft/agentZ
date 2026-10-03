//! The settings page, laid out like t3code's: a list of sections on the left (General,
//! Appearance, Agents, Machines, then one entry per project) and the chosen section's rows on
//! the right.

use std::path::PathBuf;

use crate::machines::{
    GroupKey, MachineId, Machines, ProjectGroupingMode, ProjectKey, machine_kind_icon,
};
use crate::project_store::ProjectStore;
use agentz_protocol::agents::{AgentId, InstallState};
use agentz_protocol::workspace::WorkspaceRemoval;
use gpui::{
    AnyElement, App, Context, Entity, EventEmitter, FocusHandle, Focusable, KeyBinding,
    PathPromptOptions, PromptLevel, ScrollHandle, Subscription, Window, actions,
};
use projects::{Project, ProjectIcon, ProjectId, ThreadOrder, Workspace};
use text_input::{TextInput, TextInputEvent};
use theme::{Appearance, ThemeRegistry};
use ui::{
    ContextMenu, DropdownMenu, IconPosition, PopoverMenu, Switch, Tooltip, WithScrollbar as _,
    prelude::*,
};
use util::ResultExt as _;

use agent_client_protocol::schema::v1 as acp;
use agentz_protocol::thread::ConnectionStatus;
use agentz_protocol::{CAPABILITY_MACHINE_ICON, MachineKind, Request};

use std::collections::BTreeMap;

use crate::agent_view::{TOOLBAR_HEIGHT, open_in_terminal};
use crate::app_settings::{AppSettingsStore, MachineProfile, ThemeMode};
use crate::project_info::{
    MONOGRAM_COLORS, ProjectInfoStore, automatic_monogram, monogram_swatch, render_project_icon,
    workspace_icon,
};
use crate::project_switcher::compact_path;
use crate::registry_store::AgentRegistryStore;
use crate::server_client::{MachineStatus, ServerClient, ServerUpdate};
use crate::sidebar::{SIDEBAR_WIDTH, render_footer_item};
use crate::thread_entity::AgentThread;

const KEY_CONTEXT: &str = "SettingsPage";
const CONTENT_WIDTH: Pixels = px(720.);

actions!(
    settings,
    [
        /// Closes the settings page.
        CloseSettings,
    ]
);

pub fn init(cx: &mut App) {
    cx.bind_keys([KeyBinding::new("escape", CloseSettings, Some(KEY_CONTEXT))]);
}

pub enum SettingsPageEvent {
    Close,
    AddProject(MachineId),
    /// Open the Add Machine dialog, or Edit… for the machine given.
    EditMachine(Option<MachineProfile>),
}

#[derive(Clone, Copy, PartialEq)]
enum Section {
    General,
    Appearance,
    Agents,
    Machines,
    Project(ProjectKey),
}

pub struct SettingsPage {
    focus_handle: FocusHandle,
    machines: Entity<Machines>,
    app_settings: Entity<AppSettingsStore>,
    section: Section,
    name_input: Entity<TextInput>,
    monogram_input: Entity<TextInput>,
    /// The machine whose agents Settings › Agents shows.
    agents_machine: MachineId,
    agent_search: Entity<TextInput>,
    /// The saved machine the form edits, rather than adding one.
    /// Whether this Mac's server has a launch agent, so it starts at login.
    starts_at_login: bool,
    /// The agent whose account panel is open, with the connection made to log in or out.
    account: Option<AccountPanel>,
    nav_scroll: ScrollHandle,
    content_scroll: ScrollHandle,
    /// Detected favicons, so automatic icons match the sidebar's.
    project_info: Entity<ProjectInfoStore>,
    _subscriptions: Vec<Subscription>,
}

impl EventEmitter<SettingsPageEvent> for SettingsPage {}

impl Focusable for SettingsPage {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl SettingsPage {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let machines = Machines::global(cx);
        let app_settings = AppSettingsStore::global(cx);
        let name_input = cx.new(|cx| TextInput::new("", cx));
        let monogram_input = cx.new(|cx| TextInput::new("", cx));
        let mut subscriptions = vec![
            cx.observe(&machines, |this, _, cx| {
                // A removed project's page has nothing left to show.
                if let Section::Project(key) = this.section
                    && this.project(key, cx).is_none()
                {
                    this.section = Section::General;
                }
                if this
                    .machines
                    .read(cx)
                    .client(this.agents_machine, cx)
                    .is_none()
                {
                    this.agents_machine = MachineId::Local;
                }
                cx.notify();
            }),
            cx.observe(&app_settings, |_, _, cx| cx.notify()),
            cx.subscribe(&name_input, |this, input, _: &TextInputEvent, cx| {
                let Section::Project(key) = this.section else {
                    return;
                };
                let name = input.read(cx).text().to_string();
                for (member, project) in this.group_members(key, cx) {
                    // Opening the page writes the name back unchanged; that's no edit.
                    if project.custom_name.as_deref().unwrap_or_default() == name {
                        continue;
                    }
                    if let Some(store) = this.machines.read(cx).projects(member.machine, cx) {
                        store.update(cx, |store, cx| {
                            store.set_project_name(member.project, &name, cx)
                        });
                    }
                }
            }),
            cx.subscribe(&monogram_input, |this, input, _: &TextInputEvent, cx| {
                let text = input.read(cx).text().trim().to_string();
                if !text.is_empty() {
                    this.set_monogram(Some(text), None, cx);
                }
            }),
        ];
        let agent_search = cx.new(|cx| TextInput::new("Search agents…", cx));
        subscriptions.push(cx.subscribe(&agent_search, |_, _, _: &TextInputEvent, cx| cx.notify()));
        let project_info = ProjectInfoStore::global(cx);
        subscriptions.push(cx.observe(&project_info, |_, _, cx| cx.notify()));
        Self {
            focus_handle: cx.focus_handle(),
            machines,
            app_settings,
            section: Section::General,
            name_input,
            monogram_input,
            agents_machine: MachineId::Local,
            agent_search,
            starts_at_login: crate::login_item::is_enabled(),
            account: None,
            nav_scroll: ScrollHandle::new(),
            content_scroll: ScrollHandle::new(),
            project_info,
            _subscriptions: subscriptions,
        }
    }

    pub fn show_agents(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.select(Section::Agents, window, cx);
    }

    pub fn show_machines(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.select(Section::Machines, window, cx);
    }

    pub fn show_project(&mut self, key: ProjectKey, window: &mut Window, cx: &mut Context<Self>) {
        self.select(Section::Project(key), window, cx);
    }

    fn project(&self, key: ProjectKey, cx: &App) -> Option<Project> {
        self.machines
            .read(cx)
            .projects(key.machine, cx)?
            .read(cx)
            .project(key.project)
            .cloned()
    }

    /// The project and the checkouts combined with it, which share its name and icon.
    fn group_members(&self, key: ProjectKey, cx: &App) -> Vec<(ProjectKey, Project)> {
        match self
            .machines
            .read(cx)
            .group_of(key.machine, key.project, cx)
        {
            Some(group) => group
                .members
                .into_iter()
                .map(|(machine, project)| {
                    (
                        ProjectKey {
                            machine,
                            project: project.id,
                        },
                        project,
                    )
                })
                .collect(),
            None => self
                .project(key, cx)
                .map(|project| (key, project))
                .into_iter()
                .collect(),
        }
    }

    /// Gives the project's group the icon. An image is a file on this Mac, so only this
    /// Mac's checkouts take it.
    fn set_group_icon(&self, key: ProjectKey, icon: Option<ProjectIcon>, cx: &mut App) {
        let is_image = matches!(icon, Some(ProjectIcon::Image { .. }));
        for (member, project) in self.group_members(key, cx) {
            if project.icon == icon || (is_image && member.machine != MachineId::Local) {
                continue;
            }
            if let Some(store) = self.machines.read(cx).projects(member.machine, cx) {
                let icon = icon.clone();
                store.update(cx, |store, cx| {
                    store.set_project_icon(member.project, icon, cx)
                });
            }
        }
    }

    /// The store of the project whose page is open.
    fn project_store(&self, cx: &App) -> Option<Entity<ProjectStore>> {
        let Section::Project(key) = self.section else {
            return None;
        };
        self.machines.read(cx).projects(key.machine, cx)
    }

    /// The machine Settings › Agents shows.
    fn agents_client(&self, cx: &App) -> Entity<ServerClient> {
        self.machines
            .read(cx)
            .client(self.agents_machine, cx)
            .unwrap_or_else(|| Machines::local(cx))
    }

    fn registry(&self, cx: &App) -> Entity<AgentRegistryStore> {
        self.agents_client(cx).read(cx).registry().clone()
    }

    fn select(&mut self, section: Section, window: &mut Window, cx: &mut Context<Self>) {
        if self.section != section {
            self.content_scroll.set_offset(gpui::point(px(0.), px(0.)));
        }
        self.section = section;
        if section == Section::Agents {
            self.registry(cx)
                .update(cx, |registry, cx| registry.refresh_if_stale(cx));
            // Typing on the Agents page searches it.
            window.focus(&self.agent_search.focus_handle(cx), cx);
        }
        if let Section::Project(key) = section
            && let Some(project) = self.project(key, cx)
        {
            // Set before the section's inputs fire their change events, which then write
            // the same values back.
            self.name_input.update(cx, |input, cx| {
                input.set_placeholder(project.folder_name(), cx);
                input.set_text(project.custom_name.clone().unwrap_or_default(), cx);
            });
            let text = match &project.icon {
                Some(ProjectIcon::Monogram { text, .. }) => text.clone(),
                _ => String::new(),
            };
            let (automatic_text, _) = automatic_monogram(&project.name());
            self.monogram_input.update(cx, |input, cx| {
                input.set_placeholder(automatic_text, cx);
                input.set_text(text, cx);
            });
        }
        cx.notify();
    }

    /// Switches the project to a monogram, keeping whichever of its letters and color aren't
    /// being changed.
    fn set_monogram(&mut self, text: Option<String>, color: Option<&str>, cx: &mut Context<Self>) {
        let Section::Project(key) = self.section else {
            return;
        };
        let Some(project) = self.project(key, cx) else {
            return;
        };
        let (automatic_text, automatic_color) = automatic_monogram(&project.name());
        let (current_text, current_color) = match &project.icon {
            Some(ProjectIcon::Monogram { text, color }) => (text.clone(), color.clone()),
            _ => (automatic_text, automatic_color.to_string()),
        };
        let icon = ProjectIcon::Monogram {
            text: text.unwrap_or(current_text),
            color: color.map_or(current_color, str::to_string),
        };
        self.set_group_icon(key, Some(icon), cx);
    }

    fn choose_icon_file(&mut self, key: ProjectKey, cx: &mut Context<Self>) {
        let paths = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Use as Icon".into()),
        });
        cx.spawn(async move |this, cx| {
            let path = match paths.await {
                Ok(Ok(Some(paths))) => paths.into_iter().next(),
                Ok(Ok(None)) | Err(_) => None,
                Ok(Err(error)) => {
                    log::error!("failed to pick an icon file: {error:#}");
                    None
                }
            };
            if let Some(path) = path {
                this.update(cx, |this, cx| {
                    this.set_group_icon(key, Some(ProjectIcon::Image { path }), cx)
                })
                .ok();
            }
        })
        .detach();
    }

    fn confirm_remove_project(
        &mut self,
        project: &Project,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let answer = window.prompt(
            PromptLevel::Warning,
            &format!("Remove “{}” from agentZ?", project.name()),
            Some("Its threads are removed too. Nothing on disk is touched."),
            &["Remove", "Cancel"],
            cx,
        );
        let Some(store) = self.project_store(cx) else {
            return;
        };
        let id = project.id;
        cx.spawn(async move |_, cx| {
            if answer.await == Ok(0) {
                store.update(cx, |store, cx| store.remove_project(id, cx));
            }
        })
        .detach();
    }

    /// Asks first, then asks again when the server finds work that removing would lose.
    fn confirm_remove_workspace(
        &mut self,
        project_id: ProjectId,
        workspace: &Workspace,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(store) = self.project_store(cx) else {
            return;
        };
        let kind = workspace.kind.label().to_lowercase();
        let thread_count = store.read(cx).threads_in_folder(&workspace.path).len();
        let detail = match thread_count {
            0 => "Its folder is deleted from disk.".to_string(),
            1 => "Its folder is deleted from disk. 1 thread works there and stops.".to_string(),
            count => {
                format!("Its folder is deleted from disk. {count} threads work there and stop.")
            }
        };
        let answer = window.prompt(
            PromptLevel::Warning,
            &format!("Remove the {kind} at {}?", compact_path(&workspace.path)),
            Some(&detail),
            &["Remove", "Cancel"],
            cx,
        );
        let path = workspace.path.clone();
        cx.spawn_in(window, async move |_, cx| {
            if answer.await != Ok(0) {
                return;
            }
            let removal = remove_workspace(&store, project_id, path.clone(), false, cx).await;
            let failure = match removal {
                Ok(WorkspaceRemoval::NeedsConfirmation(reason)) => {
                    let answer = cx.update(|window, cx| {
                        window.prompt(
                            PromptLevel::Warning,
                            &format!("Remove the {kind} anyway?"),
                            Some(&reason),
                            &["Remove Anyway", "Cancel"],
                            cx,
                        )
                    });
                    let Ok(answer) = answer else {
                        return;
                    };
                    if answer.await != Ok(0) {
                        return;
                    }
                    remove_workspace(&store, project_id, path, true, cx)
                        .await
                        .err()
                }
                Ok(_) => None,
                Err(error) => Some(error),
            };
            if let Some(error) = failure {
                let answer = cx.update(|window, cx| {
                    window.prompt(
                        PromptLevel::Critical,
                        &format!("Couldn't remove the {kind}"),
                        Some(&format!("{error:#}")),
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

    /// Project Settings › Checkouts: the project's worktrees and pastures, each with its branch,
    /// folder and threads, and a way to remove it.
    fn render_checkouts(
        &self,
        machine: MachineId,
        project: &Project,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let project_id = project.id;
        let Some(store) = self.machines.read(cx).projects(machine, cx) else {
            return div().into_any_element();
        };
        let mut rows: Vec<AnyElement> = project
            .workspaces
            .iter()
            .enumerate()
            .map(|(index, workspace)| {
                let branch = self
                    .project_info
                    .read(cx)
                    .workspace_head(machine, &workspace.path)
                    .map(|head| head.branch.clone())
                    .or_else(|| workspace.branch.clone())
                    .unwrap_or_else(|| "No branch".to_string());
                let thread_count = store.read(cx).threads_in_folder(&workspace.path).len();
                let mut description = format!(
                    "{} · {}",
                    workspace.kind.label(),
                    compact_path(&workspace.path)
                );
                match thread_count {
                    0 => {}
                    1 => description.push_str(" · 1 thread"),
                    count => description.push_str(&format!(" · {count} threads")),
                }
                let workspace = workspace.clone();
                h_flex()
                    .px_4()
                    .py_3()
                    .gap_3()
                    .child(
                        Icon::new(workspace_icon(workspace.kind))
                            .size(IconSize::Small)
                            .color(Color::Muted),
                    )
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .gap_0p5()
                            .child(Label::new(branch).truncate())
                            .child(
                                Label::new(description)
                                    .size(LabelSize::Small)
                                    .color(Color::Muted)
                                    .truncate_middle(),
                            ),
                    )
                    .child(
                        Button::new(("remove-workspace", index), "Remove…")
                            .style(ButtonStyle::Outlined)
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.confirm_remove_workspace(project_id, &workspace, window, cx)
                            })),
                    )
                    .into_any_element()
            })
            .collect();
        if rows.is_empty() {
            rows.push(
                div()
                    .px_4()
                    .py_3()
                    .child(
                        Label::new(
                            "No worktrees or pastures yet. New Thread offers them for git \
                             repositories, and agents can make them too.",
                        )
                        .size(LabelSize::Small)
                        .color(Color::Muted),
                    )
                    .into_any_element(),
            );
        }
        render_section("Checkouts", rows, cx)
    }

    fn render_nav(&self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = cx.theme().colors().clone();
        let machines = self.machines.read(cx);
        let projects: Vec<(MachineId, Project)> = machines
            .project_groups(cx)
            .into_iter()
            .flat_map(|group| group.members)
            .collect();
        let mut items = vec![
            self.render_nav_item(
                "General",
                Some(IconName::Settings),
                None,
                Section::General,
                cx,
            ),
            self.render_nav_item(
                "Appearance",
                Some(IconName::Eye),
                None,
                Section::Appearance,
                cx,
            ),
            self.render_nav_item("Agents", Some(IconName::Sparkle), None, Section::Agents, cx),
            self.render_nav_item(
                "Machines",
                Some(IconName::Server),
                None,
                Section::Machines,
                cx,
            ),
        ];
        let fixed_count = items.len();
        let mut project_items = Vec::with_capacity(projects.len());
        for (machine, project) in &projects {
            let icon = render_project_icon(
                project,
                self.project_info.read(cx).info(*machine, project.id),
                px(14.),
                cx,
            );
            let label: SharedString = match machine {
                MachineId::Local => project.name(),
                MachineId::Remote(_) => format!(
                    "{} · {}",
                    project.name(),
                    self.machines.read(cx).label(*machine, cx)
                )
                .into(),
            };
            project_items.push(self.render_nav_item(
                label,
                None,
                Some(icon),
                Section::Project(ProjectKey {
                    machine: *machine,
                    project: project.id,
                }),
                cx,
            ));
        }
        items.extend(project_items);
        v_flex()
            .w(SIDEBAR_WIDTH)
            .h_full()
            .flex_none()
            .border_r_1()
            .border_color(colors.border)
            .bg(colors.panel_background)
            .child(
                // Shaped like the search rows atop the other views' sidebars.
                h_flex()
                    .h(TOOLBAR_HEIGHT)
                    .flex_none()
                    .pl_3()
                    .pr_2()
                    .justify_between()
                    .border_b_1()
                    .border_color(colors.border)
                    .child(Label::new("Settings").weight(gpui::FontWeight::MEDIUM))
                    .child(
                        IconButton::new("close-settings", IconName::Close)
                            .icon_size(IconSize::Small)
                            .tooltip(|_, cx| {
                                Tooltip::for_action("Close Settings", &CloseSettings, cx)
                            })
                            .on_click(cx.listener(|_, _, _, cx| cx.emit(SettingsPageEvent::Close))),
                    ),
            )
            .child(
                // The scrollbar belongs to this non-scrolling wrapper, as in Zed, so it stays put
                // while the list moves.
                div()
                    .id("settings-nav-scroll")
                    .flex_1()
                    .min_h_0()
                    .child(
                        v_flex()
                            .id("settings-nav")
                            .size_full()
                            .overflow_y_scroll()
                            .track_scroll(&self.nav_scroll)
                            .p_1()
                            .gap_px()
                            .children(items.drain(..fixed_count))
                            .when(!projects.is_empty(), |nav| {
                                nav.child(
                                    div().px_2().pt_3().pb_1().child(
                                        Label::new("Projects")
                                            .size(LabelSize::Small)
                                            .color(Color::Muted),
                                    ),
                                )
                            })
                            .children(items),
                    )
                    .vertical_scrollbar_for(&self.nav_scroll, window, cx),
            )
            // Where the sidebar's Settings row was, so going back needs no mouse movement.
            .child(render_footer_item(
                "settings-back",
                IconName::ArrowLeft,
                "Back",
                cx.listener(|_, _, _, cx| cx.emit(SettingsPageEvent::Close)),
                cx,
            ))
    }

    fn render_nav_item(
        &self,
        label: impl Into<SharedString>,
        icon: Option<IconName>,
        icon_element: Option<AnyElement>,
        section: Section,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let colors = cx.theme().colors().clone();
        let is_selected = self.section == section;
        let id = match section {
            Section::General => SharedString::from("settings-nav-general"),
            Section::Appearance => "settings-nav-appearance".into(),
            Section::Agents => "settings-nav-agents".into(),
            Section::Machines => "settings-nav-machines".into(),
            Section::Project(key) => format!(
                "settings-nav-project-{}-{}",
                key.machine.slug(),
                key.project.0
            )
            .into(),
        };
        h_flex()
            .id(id)
            .h(px(28.))
            .px_2()
            .gap_2()
            .rounded_md()
            .cursor_pointer()
            .when(is_selected, |item| item.bg(colors.ghost_element_selected))
            .hover(|item| item.bg(colors.ghost_element_hover))
            .children(icon.map(|icon| {
                Icon::new(icon)
                    .size(IconSize::Small)
                    .color(Color::Muted)
                    .into_any_element()
            }))
            .children(icon_element)
            .child(div().flex_1().min_w_0().child(Label::new(label).truncate()))
            .on_click(cx.listener(move |this, _, window, cx| this.select(section, window, cx)))
            .into_any_element()
    }

    fn render_general(&self, window: &mut Window, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let current = self.machines.read(cx).thread_order(cx);
        let store = Machines::local(cx).read(cx).projects().clone();
        let menu = ContextMenu::build(window, cx, move |mut menu, _, _| {
            for (order, label) in [
                (ThreadOrder::LastActivity, "Latest activity"),
                (ThreadOrder::Created, "Newest first"),
            ] {
                let store = store.clone();
                menu = menu.toggleable_entry(
                    label,
                    current == order,
                    IconPosition::End,
                    None,
                    move |_, cx| store.update(cx, |store, cx| store.set_thread_order(order, cx)),
                );
            }
            menu
        });
        let label = match current {
            ThreadOrder::LastActivity => "Latest activity",
            ThreadOrder::Created => "Newest first",
        };
        vec![
            render_section(
                "Threads",
                vec![render_row(
                    "Thread order",
                    "How threads are sorted in the sidebar.",
                    DropdownMenu::new("thread-order", label, menu).into_any_element(),
                    cx,
                )],
                cx,
            ),
            render_section("Projects", self.render_grouping_rows(window, cx), cx),
            render_section(
                "Server",
                vec![render_row(
                    "Start at login",
                    "Starts the server when you log in, before agentZ opens, so scripts \
                         using agentz-server call can reach it.",
                    Switch::new("start-at-login", self.starts_at_login.into())
                        .on_click(cx.listener(|this, state, _, cx| {
                            let enabled = *state == ToggleState::Selected;
                            if crate::login_item::set_enabled(enabled).log_err().is_some() {
                                this.starts_at_login = enabled;
                                cx.notify();
                            }
                        }))
                        .into_any_element(),
                    cx,
                )],
                cx,
            ),
        ]
    }

    /// t3code's "Combine matching repositories" switch, which turns grouping off or back to
    /// the mode last used, and that mode.
    fn render_grouping_rows(&self, window: &mut Window, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let grouping = self.app_settings.read(cx).settings().project_grouping;
        let app_settings = self.app_settings.clone();
        let switch = Switch::new("combine-repositories", grouping.combines().into()).on_click(
            move |state, _, cx| {
                let combine = *state == ToggleState::Selected;
                app_settings.update(cx, |store, cx| {
                    store.update(
                        |settings| {
                            if combine {
                                settings.project_grouping = match settings.last_combined_grouping {
                                    ProjectGroupingMode::Separate => {
                                        ProjectGroupingMode::Repository
                                    }
                                    mode => mode,
                                };
                            } else {
                                if settings.project_grouping.combines() {
                                    settings.last_combined_grouping = settings.project_grouping;
                                }
                                settings.project_grouping = ProjectGroupingMode::Separate;
                            }
                        },
                        cx,
                    )
                });
            },
        );
        let mut rows = vec![render_row(
            "Combine matching repositories across machines",
            "Checkouts of one repository, on this Mac or other machines, share one entry in \
             the projects list.",
            switch.into_any_element(),
            cx,
        )];
        if grouping.combines() {
            let app_settings = self.app_settings.clone();
            let menu = ContextMenu::build(window, cx, move |mut menu, _, _| {
                for mode in [
                    ProjectGroupingMode::Repository,
                    ProjectGroupingMode::RepositoryPath,
                ] {
                    let app_settings = app_settings.clone();
                    menu = menu.toggleable_entry(
                        mode.label(),
                        grouping == mode,
                        IconPosition::End,
                        None,
                        move |_, cx| {
                            app_settings.update(cx, |store, cx| {
                                store.update(
                                    |settings| {
                                        settings.project_grouping = mode;
                                        settings.last_combined_grouping = mode;
                                    },
                                    cx,
                                )
                            })
                        },
                    );
                }
                menu
            });
            rows.push(render_row(
                "Combine by",
                grouping.description(),
                DropdownMenu::new("project-grouping", grouping.label(), menu).into_any_element(),
                cx,
            ));
        }
        rows
    }

    fn confirm_restart_server(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let answer = window.prompt(
            PromptLevel::Warning,
            "Restart the background server?",
            Some("Agents that are working now will stop."),
            &["Restart", "Cancel"],
            cx,
        );
        cx.spawn(async move |_, cx| {
            if answer.await == Ok(0) {
                // The app starts the server again when the connection drops.
                cx.update(|cx| Machines::local(cx).read(cx).send(Request::Shutdown, cx));
            }
        })
        .detach();
    }

    /// Hands the server's terminals to the binary installed now. While turns run, asks first
    /// whether to stop them.
    fn update_server(
        &mut self,
        client: Entity<ServerClient>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let update = client.read(cx).update_server(false, cx);
        cx.spawn_in(window, async move |_, cx| {
            let result = match update.await {
                Ok(ServerUpdate::TurnsRunning(threads)) => {
                    let detail = format!(
                        "{} will stop: {}. Terminals keep running.",
                        if threads.len() == 1 {
                            "This thread's turn".to_string()
                        } else {
                            format!("These {} threads' turns", threads.len())
                        },
                        threads.join(", ")
                    );
                    let Ok(answer) = cx.update(|window, cx| {
                        window.prompt(
                            PromptLevel::Warning,
                            "Stop the running turns and update?",
                            Some(&detail),
                            &["Update", "Cancel"],
                            cx,
                        )
                    }) else {
                        return;
                    };
                    if answer.await != Ok(0) {
                        return;
                    }
                    let update = client.read_with(cx, |client, cx| client.update_server(true, cx));
                    update.await
                }
                result => result,
            };
            if let Err(error) = result {
                let detail = format!("{error:#}");
                let answer = cx.update(|window, cx| {
                    window.prompt(
                        PromptLevel::Critical,
                        "Couldn't update the server",
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

    fn confirm_restart_remote_server(
        &mut self,
        client: Entity<ServerClient>,
        name: SharedString,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let answer = window.prompt(
            PromptLevel::Warning,
            &format!("Restart agentz-server on {name}?"),
            Some("Agents and terminals running there will stop."),
            &["Restart", "Cancel"],
            cx,
        );
        cx.spawn(async move |_, cx| {
            if answer.await == Ok(0) {
                cx.update(|cx| client.read(cx).restart_server(cx));
            }
        })
        .detach();
    }

    fn render_appearance(&self, window: &mut Window, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let settings = self.app_settings.read(cx).settings().clone();
        let app_settings = self.app_settings.clone();
        let mode_menu = ContextMenu::build(window, cx, move |mut menu, _, _| {
            for (mode, label) in [
                (ThemeMode::System, "System"),
                (ThemeMode::Light, "Light"),
                (ThemeMode::Dark, "Dark"),
            ] {
                let app_settings = app_settings.clone();
                menu = menu.toggleable_entry(
                    label,
                    settings.theme_mode == mode,
                    IconPosition::End,
                    None,
                    move |_, cx| {
                        app_settings.update(cx, |store, cx| {
                            store.update(|settings| settings.theme_mode = mode, cx)
                        })
                    },
                );
            }
            menu
        });
        let mode_label = match settings.theme_mode {
            ThemeMode::System => "System",
            ThemeMode::Light => "Light",
            ThemeMode::Dark => "Dark",
        };
        let themes = ThemeRegistry::global(cx).list();
        let theme_menu = |appearance: Appearance, window: &mut Window, cx: &mut App| {
            let current = match appearance {
                Appearance::Light => settings.light_theme.clone(),
                Appearance::Dark => settings.dark_theme.clone(),
            };
            let mut names: Vec<SharedString> = themes
                .iter()
                .filter(|theme| theme.appearance == appearance)
                .map(|theme| theme.name.clone())
                .collect();
            names.sort();
            let app_settings = self.app_settings.clone();
            let menu = ContextMenu::build(window, cx, {
                let current = current.clone();
                move |mut menu, _, _| {
                    for name in names {
                        let app_settings = app_settings.clone();
                        let is_current = *name == *current;
                        menu = menu.toggleable_entry(
                            name.clone(),
                            is_current,
                            IconPosition::End,
                            None,
                            move |_, cx| {
                                let name = name.to_string();
                                app_settings.update(cx, |store, cx| {
                                    store.update(
                                        |settings| match appearance {
                                            Appearance::Light => settings.light_theme = name,
                                            Appearance::Dark => settings.dark_theme = name,
                                        },
                                        cx,
                                    )
                                })
                            },
                        );
                    }
                    menu
                }
            });
            (current, menu)
        };
        let (light_theme, light_menu) = theme_menu(Appearance::Light, window, cx);
        let (dark_theme, dark_menu) = theme_menu(Appearance::Dark, window, cx);
        vec![render_section(
            "Theme",
            vec![
                render_row(
                    "Mode",
                    "Follow macOS, or always use the light or dark theme.",
                    DropdownMenu::new("theme-mode", mode_label, mode_menu).into_any_element(),
                    cx,
                ),
                render_row(
                    "Light theme",
                    "Used when the appearance is light.",
                    DropdownMenu::new("light-theme", light_theme, light_menu).into_any_element(),
                    cx,
                ),
                render_row(
                    "Dark theme",
                    "Used when the appearance is dark.",
                    DropdownMenu::new("dark-theme", dark_theme, dark_menu).into_any_element(),
                    cx,
                ),
            ],
            cx,
        )]
    }

    /// The agents from the ACP Registry: the installed ones to update or uninstall, then the
    /// rest to install.
    fn render_agents(&self, window: &mut Window, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let colors = cx.theme().colors().clone();
        let query = self.agent_search.read(cx).text().trim().to_lowercase();
        let registry = self.registry(cx);
        let registry = registry.read(cx);
        let is_fetching = registry.is_fetching();
        let fetch_error = registry.fetch_error();
        let has_agents = !registry.agents().is_empty();
        let mut installed = Vec::new();
        let mut available = Vec::new();
        for agent in registry.agents() {
            let matches = query.is_empty()
                || agent.name().to_lowercase().contains(&query)
                || agent.id().0.to_lowercase().contains(&query);
            if !agent.supports_current_platform() || !matches {
                continue;
            }
            match registry.install_state(agent.id()) {
                InstallState::Installed { .. } => installed.push(agent.id().clone()),
                _ => available.push(agent.id().clone()),
            }
        }

        let search = h_flex()
            .h(px(32.))
            .px_3()
            .gap_2()
            .rounded_md()
            .border_1()
            .border_color(colors.border)
            .bg(colors.editor_background)
            .child(
                Icon::new(IconName::MagnifyingGlass)
                    .size(IconSize::Small)
                    .color(Color::Muted),
            )
            .child(div().flex_1().min_w_0().child(self.agent_search.clone()))
            .into_any_element();
        let mut sections = vec![search];
        if self.machines.read(cx).has_remotes() {
            sections.insert(0, self.render_agents_machine_picker(window, cx));
        }
        if !installed.is_empty() {
            let rows = installed
                .iter()
                .map(|id| self.render_agent_row(id, window, cx))
                .collect();
            sections.push(render_section("Installed", rows, cx));
        }
        if !available.is_empty() {
            let rows = available
                .iter()
                .map(|id| self.render_agent_row(id, window, cx))
                .collect();
            sections.push(render_section("From the ACP Registry", rows, cx));
        }
        if installed.is_empty() && available.is_empty() {
            let message = if is_fetching && !has_agents {
                "Loading agents from the ACP Registry…".to_string()
            } else if let Some(error) = fetch_error.as_ref().filter(|_| !has_agents) {
                format!("Couldn't load the ACP Registry: {error}")
            } else {
                "No matching agents".to_string()
            };
            sections.push(
                v_flex()
                    .gap_2()
                    .items_start()
                    .child(Label::new(message).color(Color::Muted))
                    .when(fetch_error.is_some() && !has_agents, |column| {
                        column.child(
                            Button::new("retry-registry", "Retry")
                                .style(ButtonStyle::Outlined)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.registry(cx)
                                        .update(cx, |registry, cx| registry.refresh(cx))
                                })),
                        )
                    })
                    .into_any_element(),
            );
        }
        sections
    }

    fn render_agent_row(
        &self,
        id: &AgentId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let registry = self.registry(cx);
        let registry = registry.read(cx);
        let Some(agent) = registry.agent(id) else {
            return div().into_any_element();
        };
        let name = agent.name().clone();
        let icon = match agent.icon_path() {
            Some(path) => Icon::from_external_svg(path.clone()),
            None => Icon::new(IconName::Terminal),
        };
        let element_id = |action: &str| SharedString::from(format!("agent-{action}-{}", id.0));
        let is_account_open = self
            .account
            .as_ref()
            .is_some_and(|account| &account.agent_id == id);
        let install = {
            let id = id.clone();
            cx.listener(move |this, _, _, cx| {
                this.registry(cx)
                    .update(cx, |registry, cx| registry.install(&id, cx))
            })
        };
        let (detail, controls): (SharedString, AnyElement) = match registry.install_state(id) {
            InstallState::Installed {
                version,
                update_available,
            } => {
                let uninstall = {
                    let id = id.clone();
                    let name = name.clone();
                    cx.listener(move |this, _, window, cx| {
                        this.confirm_uninstall(&id, &name, window, cx)
                    })
                };
                (
                    if update_available {
                        format!("v{version} · v{} available", agent.version()).into()
                    } else {
                        format!("v{version}").into()
                    },
                    h_flex()
                        .gap_2()
                        .child(
                            Button::new(element_id("settings"), "Settings")
                                .style(ButtonStyle::Outlined)
                                .toggle_state(is_account_open)
                                .on_click({
                                    let id = id.clone();
                                    let name = name.clone();
                                    cx.listener(move |this, _, _, cx| {
                                        this.toggle_account(&id, &name, cx)
                                    })
                                }),
                        )
                        .when(update_available, |controls| {
                            controls.child(
                                Button::new(element_id("update"), "Update")
                                    .style(ButtonStyle::Outlined)
                                    .on_click(install),
                            )
                        })
                        .child(
                            Button::new(element_id("uninstall"), "Uninstall")
                                .style(ButtonStyle::Subtle)
                                .on_click(uninstall),
                        )
                        .into_any_element(),
                )
            }
            InstallState::Installing => (
                agent.description().clone(),
                Label::new("Installing…")
                    .color(Color::Muted)
                    .into_any_element(),
            ),
            InstallState::NotInstalled => (
                agent.description().clone(),
                Button::new(element_id("install"), "Install")
                    .style(ButtonStyle::Outlined)
                    .on_click(install)
                    .into_any_element(),
            ),
            InstallState::Failed(error) => (
                agent.description().clone(),
                Button::new(element_id("retry"), "Retry")
                    .style(ButtonStyle::Outlined)
                    .color(Color::Error)
                    .tooltip(Tooltip::text(error))
                    .on_click(install)
                    .into_any_element(),
            ),
        };
        let account_panel = is_account_open.then(|| self.render_agent_settings(window, cx));
        v_flex()
            .child(
                h_flex()
                    .px_4()
                    .py_3()
                    .gap_3()
                    .child(icon.size(IconSize::Medium).color(Color::Muted))
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .gap_0p5()
                            .child(Label::new(name))
                            .child(
                                Label::new(detail)
                                    .size(LabelSize::Small)
                                    .color(Color::Muted)
                                    .truncate(),
                            ),
                    )
                    .child(div().flex_none().child(controls)),
            )
            .children(account_panel)
            .into_any_element()
    }

    /// Opens the agent's account panel, starting the agent to talk to it, or closes it (which
    /// stops the agent again).
    fn toggle_account(&mut self, id: &AgentId, name: &SharedString, cx: &mut Context<Self>) {
        if self
            .account
            .as_ref()
            .is_some_and(|account| &account.agent_id == id)
        {
            self.account = None;
            cx.notify();
            return;
        }
        let client = self.agents_client(cx);
        let agent_settings = client.read(cx).agent_settings(&id.0);
        let name = name.clone();
        let account_agent_id = id.clone();
        let connection =
            cx.new(|cx| AgentThread::open_account(client.clone(), account_agent_id, name, cx));
        let agent_id = id.0.to_string();
        // The server remembers the options and modes the agent offers, and logins made in
        // the panel.
        let subscription = cx.observe(&connection, move |this, connection, cx| {
            // A login started in Terminal counts once a check finds the agent logged in.
            let logged_in = connection.read(cx).logged_in() == Some(true);
            let finished_terminal_login = this
                .account
                .as_mut()
                .filter(|_| logged_in)
                .and_then(|panel| panel.pending_terminal_method.take());
            if let Some(method) = finished_terminal_login {
                let method = method.to_string();
                connection
                    .read(cx)
                    .client()
                    .clone()
                    .update(cx, |client, cx| {
                        client.update_agent_settings(
                            &agent_id,
                            |agent| agent.login_method = Some(method),
                            cx,
                        )
                    });
            }
            cx.notify();
        });
        let env_rows = agent_settings
            .env
            .iter()
            .map(|(key, value)| self.new_env_row(key, value, cx))
            .collect();
        self.account = Some(AccountPanel {
            agent_id: id.clone(),
            connection,
            terminal_hint: None,
            pending_terminal_method: None,
            env_rows,
            _subscriptions: [subscription],
        });
        cx.notify();
    }

    fn new_env_row(&self, key: &str, value: &str, cx: &mut Context<Self>) -> EnvRow {
        let key_input = cx.new(|cx| {
            let mut input = TextInput::new("NAME", cx);
            input.set_text(key.to_string(), cx);
            input
        });
        let value_input = cx.new(|cx| {
            let mut input = TextInput::new("value", cx);
            input.set_text(value.to_string(), cx);
            input
        });
        let subscriptions = [
            cx.subscribe(&key_input, |this, _, _: &TextInputEvent, cx| {
                this.save_env(cx)
            }),
            cx.subscribe(&value_input, |this, _, _: &TextInputEvent, cx| {
                this.save_env(cx)
            }),
        ];
        EnvRow {
            key: key_input,
            value: value_input,
            _subscriptions: subscriptions,
        }
    }

    /// Writes the panel's variables to the agent's settings; rows without a name are skipped.
    fn save_env(&mut self, cx: &mut Context<Self>) {
        let Some(panel) = &self.account else {
            return;
        };
        let env: BTreeMap<String, String> = panel
            .env_rows
            .iter()
            .filter_map(|row| {
                let key = row.key.read(cx).text().trim().to_string();
                (!key.is_empty()).then(|| (key, row.value.read(cx).text().to_string()))
            })
            .collect();
        let agent_id = panel.agent_id.0.clone();
        let client = panel.connection.read(cx).client().clone();
        client.update(cx, |client, cx| {
            client.update_agent_settings(&agent_id, |agent| agent.env = env, cx)
        });
    }

    /// The agent's settings: its account, the defaults new threads start with, and its
    /// environment.
    fn render_agent_settings(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let colors = cx.theme().colors().clone();
        let heading = |title: &'static str| {
            Label::new(title)
                .size(LabelSize::Small)
                .weight(gpui::FontWeight::MEDIUM)
        };
        v_flex()
            .mx_4()
            .mb_3()
            .rounded_md()
            .border_1()
            .border_color(colors.border_variant)
            .bg(colors.editor_background)
            .child(
                v_flex()
                    .p_3()
                    .gap_2()
                    .child(heading("Account"))
                    .child(self.render_account_panel(cx)),
            )
            .child(
                v_flex()
                    .p_3()
                    .gap_2()
                    .border_t_1()
                    .border_color(colors.border_variant)
                    .child(heading("Defaults for New Threads"))
                    .child(self.render_agent_defaults(window, cx)),
            )
            .child(
                v_flex()
                    .p_3()
                    .gap_2()
                    .border_t_1()
                    .border_color(colors.border_variant)
                    .child(heading("Environment Variables"))
                    .child(self.render_agent_env(cx)),
            )
            .child(
                h_flex()
                    .p_2()
                    .justify_end()
                    .border_t_1()
                    .border_color(colors.border_variant)
                    .child(
                        Button::new("agent-settings-done", "Done")
                            .style(ButtonStyle::Subtle)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.account = None;
                                cx.notify();
                            })),
                    ),
            )
            .into_any_element()
    }

    /// Zed's per-agent defaults: what a new session starts with. Choosing a setting in a thread
    /// changes these too.
    fn render_agent_defaults(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let Some(panel) = &self.account else {
            return div().into_any_element();
        };
        let agent_id = panel.agent_id.0.to_string();
        let agent_name = panel.connection.read(cx).agent_name().clone();
        let client = panel.connection.read(cx).client().clone();
        let agent = client.read(cx).agent_settings(&agent_id);
        let mut rows: Vec<AnyElement> = Vec::new();
        for option in &agent.known_config_options {
            let config_id = option.id.0.to_string();
            let current = agent.default_config_options.get(&config_id).cloned();
            let choices: Vec<(SharedString, acp::SessionConfigOptionValue)> = match &option.kind {
                acp::SessionConfigKind::Select(select) => select_choices(select)
                    .into_iter()
                    .map(|(name, value)| (name, acp::SessionConfigOptionValue::value_id(value)))
                    .collect(),
                acp::SessionConfigKind::Boolean(_) => vec![
                    ("On".into(), acp::SessionConfigOptionValue::boolean(true)),
                    ("Off".into(), acp::SessionConfigOptionValue::boolean(false)),
                ],
                _ => continue,
            };
            let label = current
                .as_ref()
                .and_then(|current| {
                    choices
                        .iter()
                        .find(|(_, value)| value == current)
                        .map(|(name, _)| name.clone())
                })
                .unwrap_or_else(|| "Agent's choice".into());
            let client = client.clone();
            let menu_agent_id = agent_id.clone();
            let menu = ContextMenu::build(window, cx, move |mut menu, _, _| {
                let entries = std::iter::once((SharedString::from("Agent's choice"), None))
                    .chain(choices.into_iter().map(|(name, value)| (name, Some(value))));
                for (name, value) in entries {
                    let is_current = value == current;
                    let client = client.clone();
                    let agent_id = menu_agent_id.clone();
                    let config_id = config_id.clone();
                    menu = menu.toggleable_entry(
                        name,
                        is_current,
                        IconPosition::End,
                        None,
                        move |_, cx| {
                            let value = value.clone();
                            let config_id = config_id.clone();
                            client.update(cx, |client, cx| {
                                client.update_agent_settings(
                                    &agent_id,
                                    |agent| match value {
                                        Some(value) => {
                                            agent.default_config_options.insert(config_id, value);
                                        }
                                        None => {
                                            agent.default_config_options.remove(&config_id);
                                        }
                                    },
                                    cx,
                                )
                            });
                        },
                    );
                }
                menu
            });
            rows.push(render_default_row(
                option.name.clone().into(),
                DropdownMenu::new(
                    SharedString::from(format!("agent-default-{}", option.id.0)),
                    label,
                    menu,
                )
                .into_any_element(),
            ));
        }
        // Agents that predate config options offer modes instead.
        let has_mode_option = agent
            .known_config_options
            .iter()
            .any(|option| option.category == Some(acp::SessionConfigOptionCategory::Mode));
        if let Some(modes) = agent.known_modes.as_ref().filter(|_| !has_mode_option) {
            let current = agent.default_mode.clone();
            let label = current
                .as_ref()
                .and_then(|current| {
                    modes
                        .available_modes
                        .iter()
                        .find(|mode| mode.id == *current)
                        .map(|mode| SharedString::from(mode.name.clone()))
                })
                .unwrap_or_else(|| "Agent's choice".into());
            let modes: Vec<(SharedString, Option<acp::SessionModeId>)> =
                std::iter::once((SharedString::from("Agent's choice"), None))
                    .chain(
                        modes
                            .available_modes
                            .iter()
                            .map(|mode| (mode.name.clone().into(), Some(mode.id.clone()))),
                    )
                    .collect();
            let menu_agent_id = agent_id;
            let menu = ContextMenu::build(window, cx, move |mut menu, _, _| {
                for (name, mode) in modes {
                    let is_current = mode == current;
                    let client = client.clone();
                    let agent_id = menu_agent_id.clone();
                    menu = menu.toggleable_entry(
                        name,
                        is_current,
                        IconPosition::End,
                        None,
                        move |_, cx| {
                            let mode = mode.clone();
                            client.update(cx, |client, cx| {
                                client.update_agent_settings(
                                    &agent_id,
                                    |agent| agent.default_mode = mode,
                                    cx,
                                )
                            });
                        },
                    );
                }
                menu
            });
            rows.push(render_default_row(
                "Mode".into(),
                DropdownMenu::new("agent-default-mode", label, menu).into_any_element(),
            ));
        }
        if rows.is_empty() {
            let connection = panel.connection.read(cx);
            let message = match (connection.status(), connection.logged_in()) {
                (_, Some(false)) => format!("Log in to {agent_name} to see its settings here."),
                (ConnectionStatus::Connecting, _) => format!("Loading {agent_name}'s settings…"),
                _ => format!("{agent_name} doesn't offer any settings."),
            };
            return Label::new(message)
                .size(LabelSize::Small)
                .color(Color::Muted)
                .into_any_element();
        }
        v_flex()
            .gap_1()
            .child(
                Label::new("Choosing one in a thread also makes it the default, as in Zed.")
                    .size(LabelSize::Small)
                    .color(Color::Muted),
            )
            .children(rows)
            .into_any_element()
    }

    fn render_agent_env(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(panel) = &self.account else {
            return div().into_any_element();
        };
        let colors = cx.theme().colors().clone();
        let agent_name = panel.connection.read(cx).agent_name().clone();
        let input_box = |input: Entity<TextInput>| {
            div()
                .h(px(28.))
                .px_2()
                .flex()
                .items_center()
                .overflow_hidden()
                .rounded_md()
                .border_1()
                .border_color(colors.border)
                .bg(colors.panel_background)
                .child(input)
        };
        let rows: Vec<AnyElement> = panel
            .env_rows
            .iter()
            .enumerate()
            .map(|(index, row)| {
                h_flex()
                    .gap_2()
                    .child(div().w(px(180.)).child(input_box(row.key.clone())))
                    .child(Label::new("=").color(Color::Muted))
                    .child(div().flex_1().min_w_0().child(input_box(row.value.clone())))
                    .child(
                        IconButton::new(("remove-env", index), IconName::Close)
                            .icon_size(IconSize::Small)
                            .tooltip(Tooltip::text("Remove Variable"))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                if let Some(panel) = &mut this.account
                                    && index < panel.env_rows.len()
                                {
                                    panel.env_rows.remove(index);
                                }
                                this.save_env(cx);
                                cx.notify();
                            })),
                    )
                    .into_any_element()
            })
            .collect();
        v_flex()
            .gap_2()
            .child(
                Label::new(format!(
                    "Passed to {agent_name} when it starts. Running threads pick them up after Reload Agent."
                ))
                .size(LabelSize::Small)
                .color(Color::Muted),
            )
            .children(rows)
            .child(
                h_flex().child(
                    Button::new("add-env", "Add Variable")
                        .style(ButtonStyle::Subtle)
                        .start_icon(Icon::new(IconName::Plus).size(IconSize::Small))
                        .on_click(cx.listener(|this, _, _, cx| {
                            let row = this.new_env_row("", "", cx);
                            if let Some(panel) = &mut this.account {
                                panel.env_rows.push(row);
                            }
                            cx.notify();
                        })),
                ),
            )
            .into_any_element()
    }

    /// The agent's own ways to log in (the same ones a thread offers when it needs a login), and
    /// ACP's logout when the agent supports it.
    fn render_account_panel(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(account) = &self.account else {
            return div().into_any_element();
        };
        let connection = account.connection.read(cx);
        let agent_name = connection.agent_name().clone();
        let status = connection.status().clone();
        let mut buttons: Vec<AnyElement> = Vec::new();
        let (message, color): (SharedString, Color) = match &status {
            ConnectionStatus::Connecting => {
                (format!("Talking to {agent_name}…").into(), Color::Muted)
            }
            ConnectionStatus::Failed(error) => (error.clone(), Color::Error),
            ConnectionStatus::Ready | ConnectionStatus::AuthRequired => {
                for method in connection.auth_methods() {
                    let (method_id, method_name, description, is_terminal) = match method {
                        acp::AuthMethod::Agent(method) => (
                            method.id.clone(),
                            method.name.clone(),
                            method.description.clone(),
                            false,
                        ),
                        acp::AuthMethod::Terminal(method) => (
                            method.id.clone(),
                            method.name.clone(),
                            method.description.clone(),
                            true,
                        ),
                        _ => continue,
                    };
                    buttons.push(
                        Button::new(
                            SharedString::from(format!("account-auth-{}", method_id.0)),
                            method_name.clone(),
                        )
                        .style(ButtonStyle::Outlined)
                        .when_some(description, |button, description| {
                            button.tooltip(Tooltip::text(description))
                        })
                        .on_click({
                            let method_name: SharedString = method_name.clone().into();
                            cx.listener(move |this, _, _, cx| {
                                this.log_in(method_id.clone(), method_name.clone(), is_terminal, cx)
                            })
                        })
                        .into_any_element(),
                    );
                }
                if connection.supports_logout() && connection.logged_in() == Some(true) {
                    buttons.push(
                        Button::new("account-logout", "Log Out")
                            .style(ButtonStyle::Outlined)
                            .on_click(cx.listener(|this, _, _, cx| {
                                if let Some(account) = &mut this.account {
                                    account.terminal_hint = None;
                                    account
                                        .connection
                                        .update(cx, |connection, cx| connection.logout(cx));
                                }
                            }))
                            .into_any_element(),
                    );
                }
                if let Some(error) = connection.auth_error() {
                    (error.clone(), Color::Error)
                } else if let Some(hint) = &account.terminal_hint {
                    (hint.clone(), Color::Muted)
                } else if buttons.is_empty() {
                    (
                        format!("{agent_name} doesn't offer logging in or out from agentZ.").into(),
                        Color::Muted,
                    )
                } else {
                    (
                        format!("{agent_name} keeps its login for every thread.").into(),
                        Color::Muted,
                    )
                }
            }
        };
        // ACP can't say which account is logged in, only whether a session opens; the method is
        // the one last used from agentZ.
        let login_method = connection
            .client()
            .read(cx)
            .agent_settings(&account.agent_id.0)
            .login_method;
        let (status_icon, status_text, status_color): (IconName, SharedString, Color) =
            match (&status, connection.logged_in()) {
                (ConnectionStatus::Failed(_), _) => {
                    (IconName::XCircle, "Couldn't start".into(), Color::Error)
                }
                (_, Some(true)) => (
                    IconName::Check,
                    match login_method.as_deref().and_then(login_method_subject) {
                        Some(subject) => format!("Logged in with {subject}").into(),
                        None => "Logged in".into(),
                    },
                    Color::Success,
                ),
                (_, Some(false)) => (IconName::Warning, "Not logged in".into(), Color::Warning),
                _ => (
                    IconName::LoadCircle,
                    "Checking whether it's logged in…".into(),
                    Color::Muted,
                ),
            };
        let can_check = matches!(
            status,
            ConnectionStatus::Ready | ConnectionStatus::AuthRequired
        );
        let agent_info = connection.agent_info().map(|info| {
            let name = info.title.clone().unwrap_or_else(|| info.name.clone());
            format!("{name} {}", info.version)
        });
        v_flex()
            .gap_2()
            .child(
                h_flex()
                    .gap_1p5()
                    .child(
                        Icon::new(status_icon)
                            .size(IconSize::Small)
                            .color(status_color),
                    )
                    .child(Label::new(status_text).color(status_color))
                    .children(agent_info.map(|info| {
                        Label::new(format!("· {info}"))
                            .size(LabelSize::Small)
                            .color(Color::Muted)
                    }))
                    .child(div().flex_1())
                    .when(can_check, |row| {
                        row.child(
                            Button::new("account-check", "Check Again")
                                .style(ButtonStyle::Subtle)
                                .label_size(LabelSize::Small)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    if let Some(account) = &mut this.account {
                                        account.terminal_hint = None;
                                        account.connection.update(cx, |connection, cx| {
                                            connection.check_login(cx)
                                        });
                                    }
                                })),
                        )
                    }),
            )
            .child(Label::new(message).size(LabelSize::Small).color(color))
            .when(!buttons.is_empty(), |panel| {
                panel.child(h_flex().flex_wrap().gap_2().children(buttons))
            })
            .into_any_element()
    }

    /// Agent methods log in through the agent; terminal methods run the agent's login command
    /// in Terminal, as a thread does.
    fn log_in(
        &mut self,
        method_id: acp::AuthMethodId,
        method_name: SharedString,
        is_terminal: bool,
        cx: &mut Context<Self>,
    ) {
        let Some(account) = &mut self.account else {
            return;
        };
        if !is_terminal {
            account.terminal_hint = None;
            account
                .connection
                .update(cx, |connection, cx| connection.authenticate(method_id, cx));
            return;
        }
        let command = account
            .connection
            .read(cx)
            .terminal_auth_command(&method_id);
        let cwd = account.connection.read(cx).cwd().clone();
        if let Some(command) = command {
            account.terminal_hint =
                Some("Finish logging in in Terminal, then choose Check Again.".into());
            account.pending_terminal_method = Some(method_name);
            cx.background_spawn(async move {
                if let Err(error) = open_in_terminal(&command, &cwd).await {
                    log::error!("couldn't open a terminal to log in: {error:#}");
                }
            })
            .detach();
        }
        cx.notify();
    }

    fn confirm_uninstall(
        &mut self,
        id: &AgentId,
        name: &SharedString,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let answer = window.prompt(
            PromptLevel::Warning,
            &format!("Uninstall {name}?"),
            Some("Threads that use it can't continue until it's installed again."),
            &["Uninstall", "Cancel"],
            cx,
        );
        let registry = self.registry(cx);
        let id = id.clone();
        cx.spawn(async move |_, cx| {
            if answer.await == Ok(0) {
                registry.update(cx, |registry, cx| registry.uninstall(&id, cx));
            }
        })
        .detach();
    }

    /// Which machine's agents the Agents page shows, once there's more than this Mac.
    fn render_agents_machine_picker(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let machines: Vec<(MachineId, SharedString)> = self
            .machines
            .read(cx)
            .clients()
            .iter()
            .map(|client| (client.read(cx).machine(), client.read(cx).label().clone()))
            .collect();
        let current = self.agents_machine;
        let label = self.machines.read(cx).label(current, cx);
        let this = cx.entity().downgrade();
        let menu = ContextMenu::build(window, cx, move |mut menu, _, _| {
            for (machine, label) in machines {
                let this = this.clone();
                menu = menu.toggleable_entry(
                    label,
                    machine == current,
                    IconPosition::End,
                    None,
                    move |_, cx| {
                        this.update(cx, |this, cx| this.set_agents_machine(machine, cx))
                            .ok();
                    },
                );
            }
            menu
        });
        render_section(
            "Machine",
            vec![render_row(
                "Agents on",
                "Each machine installs and runs its own agents.",
                DropdownMenu::new("agents-machine", label, menu).into_any_element(),
                cx,
            )],
            cx,
        )
    }

    fn set_agents_machine(&mut self, machine: MachineId, cx: &mut Context<Self>) {
        if self.agents_machine == machine {
            return;
        }
        self.agents_machine = machine;
        self.account = None;
        self.registry(cx)
            .update(cx, |registry, cx| registry.refresh_if_stale(cx));
        cx.notify();
    }

    /// Settings › Machines: this Mac, the saved machines with how their connections are doing,
    /// and the form that adds or edits one (herdr's endpoints).
    fn render_machines(&self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let profiles = self.app_settings.read(cx).settings().machines.clone();
        let mut rows = vec![self.render_machine_row(None, cx)];
        rows.extend(
            profiles
                .iter()
                .map(|profile| self.render_machine_row(Some(profile), cx)),
        );
        // t3code's header: Update All for the servers that can update in place, and Add.
        let updatable: Vec<Entity<ServerClient>> = self
            .machines
            .read(cx)
            .clients()
            .into_iter()
            .filter(|client| {
                let client = client.read(cx);
                client.is_outdated() && client.can_update_server()
            })
            .cloned()
            .collect();
        let actions = h_flex()
            .gap_1()
            .when(!updatable.is_empty(), |actions| {
                actions.child(
                    Button::new("machines-update-all", "Update All")
                        .style(ButtonStyle::Subtle)
                        .label_size(LabelSize::Small)
                        .color(Color::Muted)
                        .on_click(cx.listener(move |this, _, window, cx| {
                            for client in &updatable {
                                this.update_server(client.clone(), window, cx);
                            }
                        })),
                )
            })
            .child(
                Button::new("machines-add", "Add Machine")
                    .style(ButtonStyle::Subtle)
                    .label_size(LabelSize::Small)
                    .color(Color::Muted)
                    .start_icon(
                        Icon::new(IconName::Plus)
                            .size(IconSize::XSmall)
                            .color(Color::Muted),
                    )
                    .on_click(
                        cx.listener(|_, _, _, cx| cx.emit(SettingsPageEvent::EditMachine(None))),
                    ),
            );
        vec![render_section_with_actions(
            "Machines",
            rows,
            actions.into_any_element(),
            cx,
        )]
    }

    /// t3code's `EnvironmentRow`: the machine's icon, its name over one line of how it's
    /// reached, its status and (when there's an update) its server's version, then the
    /// update button, another machine's switch and the row menu. A switched-off row dims.
    fn render_machine_row(
        &self,
        profile: Option<&MachineProfile>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let machine = profile.map_or(MachineId::Local, |profile| MachineId::Remote(profile.id));
        let client = self.machines.read(cx).client(machine, cx);
        let is_enabled = profile.is_none_or(|profile| profile.enabled);
        let label: SharedString = match profile {
            Some(profile) => profile.display_label().into(),
            None => "This Mac".into(),
        };
        let transport = match profile {
            Some(profile) => format!("SSH {}", profile.target),
            None => "Local".to_string(),
        };
        let mut hint = None;
        let mut server_version = None;
        let (status, is_error): (String, bool) = match &client {
            None => ("Off".into(), false),
            Some(client) => {
                let client = client.read(cx);
                match client.status() {
                    MachineStatus::Connecting => ("Connecting".into(), false),
                    MachineStatus::Online => {
                        if client.is_outdated() {
                            server_version = client
                                .connection()
                                .map(|connection| connection.welcome().server_version.clone());
                        }
                        ("Connected".into(), false)
                    }
                    MachineStatus::Reconnecting(error) => (format!("Reconnecting: {error}"), true),
                    MachineStatus::Attention { error, hint: help } => {
                        hint = help.clone();
                        (error.to_string(), true)
                    }
                }
            }
        };
        let subtitle: SharedString = [Some(transport), Some(status.clone()), server_version]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
            .join(" · ")
            .into();
        let is_online = client
            .as_ref()
            .is_some_and(|client| client.read(cx).is_online());
        let (is_outdated, can_update) = client.as_ref().map_or((false, false), |client| {
            let client = client.read(cx);
            (client.is_outdated(), client.can_update_server())
        });
        let status_tooltip: SharedString = if is_outdated {
            format!("{status}\nA newer agentz-server is installed there.").into()
        } else {
            status.into()
        };
        let id_suffix = machine.slug();
        let element_id = |action: &str| SharedString::from(format!("machine-{action}-{id_suffix}"));
        let current_icon = self.machines.read(cx).machine_icon(machine, cx);

        // An older server updates without ending its terminals, or, when it can't, restarts.
        let update_button = client
            .clone()
            .filter(|_| is_online && is_outdated)
            .map(|client| {
                let name = label.clone();
                let tooltip = if can_update {
                    "Update Server"
                } else {
                    "Restart Server to Update…"
                };
                IconButton::new(element_id("update"), IconName::CircleArrowUp)
                    .icon_size(IconSize::Small)
                    .icon_color(Color::Muted)
                    .tooltip(Tooltip::text(tooltip))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        if can_update {
                            this.update_server(client.clone(), window, cx)
                        } else if machine == MachineId::Local {
                            this.confirm_restart_server(window, cx)
                        } else {
                            this.confirm_restart_remote_server(
                                client.clone(),
                                name.clone(),
                                window,
                                cx,
                            )
                        }
                    }))
            });
        // Another machine's switch connects to it or not; Remove… is in the menu.
        let switch = profile.map(|profile| {
            let id = profile.id;
            let tooltip = if profile.enabled {
                "Switch Off"
            } else {
                "Switch On"
            };
            div()
                .id(element_id("switch-tooltip"))
                .tooltip(Tooltip::text(tooltip))
                .child(
                    Switch::new(element_id("switch"), profile.enabled.into()).on_click(
                        cx.listener(move |this, state: &ToggleState, _, cx| {
                            let enabled = *state == ToggleState::Selected;
                            this.update_machine(id, |profile| profile.enabled = enabled, cx)
                        }),
                    ),
                )
        });
        let options_menu = {
            let this = cx.entity().downgrade();
            let profile = profile.cloned();
            let name = label.clone();
            PopoverMenu::new(element_id("options"))
                .menu(move |window, cx| {
                    let this = this.clone();
                    let client = client.clone();
                    let profile = profile.clone();
                    let name = name.clone();
                    Some(ContextMenu::build(window, cx, move |menu, _, cx| {
                        let menu = machine_icon_menu(menu, client.clone(), current_icon, cx);
                        let retry = client
                            .clone()
                            .filter(|client| !client.read(cx).is_online())
                            .map(|client| {
                                move |_: &mut Window, cx: &mut App| {
                                    client.update(cx, |client, _| client.retry())
                                }
                            });
                        let menu = menu
                            .when_some(retry, |menu, retry| menu.entry("Retry Now", None, retry));
                        let Some(profile) = profile.clone() else {
                            let restart = {
                                let this = this.clone();
                                move |window: &mut Window, cx: &mut App| {
                                    this.update(cx, |this, cx| {
                                        this.confirm_restart_server(window, cx)
                                    })
                                    .log_err();
                                }
                            };
                            return menu.when(is_online, |menu| {
                                menu.entry("Restart Server…", None, restart)
                            });
                        };
                        let add_project = {
                            let this = this.clone();
                            move |_: &mut Window, cx: &mut App| {
                                this.update(cx, |_, cx| {
                                    cx.emit(SettingsPageEvent::AddProject(machine))
                                })
                                .log_err();
                            }
                        };
                        let edit = {
                            let this = this.clone();
                            let profile = profile.clone();
                            move |_: &mut Window, cx: &mut App| {
                                this.update(cx, |_, cx| {
                                    cx.emit(SettingsPageEvent::EditMachine(Some(profile.clone())))
                                })
                                .log_err();
                            }
                        };
                        let remove = {
                            let this = this.clone();
                            move |window: &mut Window, cx: &mut App| {
                                this.update(cx, |this, cx| {
                                    this.confirm_remove_machine(
                                        profile.id,
                                        name.clone(),
                                        window,
                                        cx,
                                    )
                                })
                                .log_err();
                            }
                        };
                        menu.when(is_online, |menu| {
                            menu.entry("Add Project…", None, add_project)
                        })
                        .entry("Edit…", None, edit)
                        .separator()
                        .entry("Remove…", None, remove)
                    }))
                })
                .trigger_with_tooltip(
                    IconButton::new(element_id("options-trigger"), IconName::Ellipsis)
                        .icon_size(IconSize::Small)
                        .icon_color(Color::Muted),
                    Tooltip::text("Machine Options"),
                )
                .anchor(gpui::Anchor::TopRight)
        };
        h_flex()
            .px_4()
            .py_2p5()
            .gap_3()
            .when(!is_enabled, |row| row.opacity(0.6))
            .child(
                Icon::new(current_icon)
                    .size(IconSize::Medium)
                    .color(Color::Muted),
            )
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .child(Label::new(label).truncate())
                    .child(
                        div()
                            .id(element_id("status"))
                            .tooltip(Tooltip::text(status_tooltip))
                            .child(
                                Label::new(subtitle)
                                    .size(LabelSize::Small)
                                    .color(if is_error { Color::Error } else { Color::Muted })
                                    .truncate(),
                            ),
                    )
                    .children(
                        hint.map(|hint| {
                            Label::new(hint).size(LabelSize::Small).color(Color::Muted)
                        }),
                    ),
            )
            .child(
                h_flex()
                    .flex_none()
                    .gap_1()
                    .children(update_button)
                    .children(switch)
                    .child(options_menu),
            )
            .into_any_element()
    }

    fn update_machine(
        &mut self,
        id: u64,
        change: impl FnOnce(&mut MachineProfile),
        cx: &mut Context<Self>,
    ) {
        self.app_settings.update(cx, |store, cx| {
            store.update(
                |settings| {
                    if let Some(profile) = settings
                        .machines
                        .iter_mut()
                        .find(|profile| profile.id == id)
                    {
                        change(profile);
                    }
                },
                cx,
            )
        });
    }

    fn confirm_remove_machine(
        &mut self,
        id: u64,
        name: SharedString,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let answer = window.prompt(
            PromptLevel::Warning,
            &format!("Remove {name}?"),
            Some(
                "agentZ stops connecting to it. Its server keeps running there, with its agents and threads; add it again to see them.",
            ),
            &["Remove", "Cancel"],
            cx,
        );
        cx.spawn(async move |this, cx| {
            if answer.await == Ok(0) {
                this.update(cx, |this, cx| {
                    this.app_settings.update(cx, |store, cx| {
                        store.update(
                            |settings| settings.machines.retain(|profile| profile.id != id),
                            cx,
                        )
                    });
                })
                .ok();
            }
        })
        .detach();
    }

    fn render_project(
        &self,
        machine: MachineId,
        project: Project,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let colors = cx.theme().colors().clone();
        let id = project.id;
        let key = ProjectKey {
            machine,
            project: id,
        };
        let is_local = machine == MachineId::Local;
        let icon_description: SharedString = match &project.icon {
            None => "Automatic: the project's favicon, or a monogram.".into(),
            Some(ProjectIcon::Monogram { text, color }) => {
                format!("Monogram · {text} · {color}").into()
            }
            Some(ProjectIcon::Image { path }) => path.display().to_string().into(),
        };
        let current_color = match &project.icon {
            Some(ProjectIcon::Monogram { color, .. }) => Some(color.clone()),
            _ => None,
        };
        let input_box = |input: Entity<TextInput>, width: Pixels| {
            div()
                .w(width)
                .h(px(28.))
                .px_2()
                .flex()
                .items_center()
                .rounded_md()
                .border_1()
                .border_color(colors.border)
                .bg(colors.editor_background)
                .child(input)
        };
        let swatches = h_flex()
            .flex_wrap()
            .gap_1()
            .children(MONOGRAM_COLORS.iter().map(|(name, light, dark)| {
                let is_current = current_color.as_deref() == Some(*name);
                let color = monogram_swatch(*light, *dark, cx);
                div()
                    .id(SharedString::from(format!("monogram-color-{name}")))
                    .size(px(18.))
                    .rounded_full()
                    .cursor_pointer()
                    .bg(color)
                    .border_2()
                    .border_color(if is_current {
                        colors.text
                    } else {
                        gpui::transparent_black()
                    })
                    .tooltip(Tooltip::text(*name))
                    .on_click(
                        cx.listener(move |this, _, _, cx| this.set_monogram(None, Some(name), cx)),
                    )
            }));
        let icon_controls = h_flex()
            .gap_2()
            .child(render_project_icon(
                &project,
                self.project_info.read(cx).info(machine, id),
                px(24.),
                cx,
            ))
            // The picker shows this Mac's files, which another machine can't read.
            .when(is_local, |this| {
                this.child(
                    Button::new("choose-icon-file", "Choose File…")
                        .style(ButtonStyle::Outlined)
                        .on_click(
                            cx.listener(move |this, _, _, cx| this.choose_icon_file(key, cx)),
                        ),
                )
            })
            .when(project.icon.is_some(), |this| {
                this.child(
                    Button::new("reset-icon", "Reset")
                        .style(ButtonStyle::Subtle)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.monogram_input
                                .update(cx, |input, cx| input.set_text("", cx));
                            this.set_group_icon(key, None, cx);
                        })),
                )
            });
        vec![
            render_section(
                "Project",
                vec![
                    render_row(
                        "Name",
                        "Shown in the sidebar and thread lists. Leave it empty for the folder name.",
                        input_box(self.name_input.clone(), px(256.)).into_any_element(),
                        cx,
                    ),
                    render_row(
                        "Icon",
                        icon_description,
                        icon_controls.into_any_element(),
                        cx,
                    ),
                    render_row(
                        "Monogram",
                        "Letters and a color for a custom monogram icon.",
                        v_flex()
                            .items_end()
                            .gap_2()
                            .child(input_box(self.monogram_input.clone(), px(64.)))
                            .child(div().w(px(256.)).child(swatches))
                            .into_any_element(),
                        cx,
                    ),
                    render_row(
                        "Folder",
                        match machine {
                            MachineId::Local => project.path.display().to_string(),
                            MachineId::Remote(_) => format!(
                                "{}: {}",
                                self.machines.read(cx).label(machine, cx),
                                project.path.display()
                            ),
                        },
                        div().into_any_element(),
                        cx,
                    ),
                ],
                cx,
            ),
        ]
        .into_iter()
        .chain(self.render_repository(key, &project, window, cx))
        .chain([
            self.render_checkouts(machine, &project, cx),
            render_section(
                "Danger",
                vec![render_row(
                    "Remove project",
                    "Removes the project and its threads from agentZ. Files on disk are not touched.",
                    Button::new("remove-project", "Remove Project")
                        .style(ButtonStyle::Outlined)
                        .color(Color::Error)
                        .start_icon(
                            Icon::new(IconName::Trash)
                                .size(IconSize::Small)
                                .color(Color::Error),
                        )
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.confirm_remove_project(&project, window, cx)
                        }))
                        .into_any_element(),
                    cx,
                )],
                cx,
            )])
        .collect()
    }

    /// The repository the project's checkout belongs to, what it's combined with, and how.
    fn render_repository(
        &self,
        key: ProjectKey,
        project: &Project,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let repository = project.repository.as_ref()?;
        let settings = self.app_settings.read(cx).settings();
        let default = settings.project_grouping;
        let physical = GroupKey::of_project(key);
        let current = settings.project_grouping_overrides.get(&physical).copied();
        let app_settings = self.app_settings.clone();
        let menu = ContextMenu::build(window, cx, move |mut menu, _, _| {
            let choices = std::iter::once(None).chain(ProjectGroupingMode::ALL.map(Some));
            for choice in choices {
                let label: SharedString = match choice {
                    None => format!("Default ({})", default.label()).into(),
                    Some(mode) => mode.label().into(),
                };
                let app_settings = app_settings.clone();
                let physical = physical.clone();
                menu = menu.toggleable_entry(
                    label,
                    current == choice,
                    IconPosition::End,
                    None,
                    move |_, cx| {
                        app_settings.update(cx, |store, cx| {
                            store.update(
                                |settings| {
                                    let overrides = &mut settings.project_grouping_overrides;
                                    match choice {
                                        Some(mode) => {
                                            overrides.insert(physical.clone(), mode);
                                        }
                                        None => {
                                            overrides.remove(&physical);
                                        }
                                    }
                                },
                                cx,
                            )
                        })
                    },
                );
            }
            menu
        });
        let mode = current.unwrap_or(default);
        let dropdown_label: SharedString = match current {
            Some(mode) => mode.label().into(),
            None => "Default".into(),
        };
        let name = repository
            .display_name
            .clone()
            .unwrap_or_else(|| repository.canonical_key.clone());
        let mut rows = vec![
            render_row(
                "Repository",
                format!(
                    "{name} · {} {}",
                    repository.remote_name, repository.remote_url
                ),
                div().into_any_element(),
                cx,
            ),
            render_row(
                "Grouping",
                mode.description(),
                DropdownMenu::new("project-grouping-override", dropdown_label, menu)
                    .into_any_element(),
                cx,
            ),
        ];
        let machines = self.machines.read(cx);
        let others: Vec<String> = self
            .group_members(key, cx)
            .into_iter()
            .filter(|(member, _)| *member != key)
            .map(|(member, project)| match member.machine {
                MachineId::Local => compact_path(&project.path),
                machine => format!(
                    "{}: {}",
                    machines.label(machine, cx),
                    project.path.display()
                ),
            })
            .collect();
        if !others.is_empty() {
            rows.push(render_row(
                "Combined with",
                format!(
                    "{}. Name and icon changes apply to all of them.",
                    others.join(", ")
                ),
                div().into_any_element(),
                cx,
            ));
        }
        Some(render_section("Repository", rows, cx))
    }
}

async fn remove_workspace(
    store: &Entity<ProjectStore>,
    project_id: ProjectId,
    path: PathBuf,
    force: bool,
    cx: &mut gpui::AsyncWindowContext,
) -> anyhow::Result<WorkspaceRemoval> {
    store
        .read_with(cx, |store, cx| {
            store.remove_workspace(project_id, path, force, cx)
        })
        .await
}

struct AccountPanel {
    agent_id: AgentId,
    /// A session-less connection to the agent, alive only while the panel is open.
    connection: Entity<AgentThread>,
    terminal_hint: Option<SharedString>,
    /// A login method started in Terminal, credited once a check finds the agent logged in.
    pending_terminal_method: Option<SharedString>,
    env_rows: Vec<EnvRow>,
    _subscriptions: [Subscription; 1],
}

struct EnvRow {
    key: Entity<TextInput>,
    value: Entity<TextInput>,
    _subscriptions: [Subscription; 2],
}

/// What a login method logs in with, from its name: "Log in with Google" gives "Google", "API
/// Key" stays, and a bare "Log In" gives nothing.
fn login_method_subject(method: &str) -> Option<String> {
    let trimmed = method.trim();
    let lower = trimmed.to_lowercase();
    for prefix in [
        "log in with ",
        "login with ",
        "sign in with ",
        "signin with ",
    ] {
        if lower.starts_with(prefix) {
            return Some(trimmed[prefix.len()..].trim().to_string())
                .filter(|rest| !rest.is_empty());
        }
    }
    let is_bare = ["log in", "login", "sign in", "signin"].contains(&lower.as_str());
    (!is_bare && !trimmed.is_empty()).then(|| trimmed.to_string())
}

fn render_default_row(name: SharedString, control: AnyElement) -> AnyElement {
    h_flex()
        .py_0p5()
        .gap_3()
        .justify_between()
        .child(Label::new(name))
        .child(control)
        .into_any_element()
}

/// A select option's choices, flattening groups.
fn select_choices(
    select: &acp::SessionConfigSelect,
) -> Vec<(SharedString, acp::SessionConfigValueId)> {
    match &select.options {
        acp::SessionConfigSelectOptions::Ungrouped(options) => options
            .iter()
            .map(|option| (option.name.clone().into(), option.value.clone()))
            .collect(),
        acp::SessionConfigSelectOptions::Grouped(groups) => groups
            .iter()
            .flat_map(|group| &group.options)
            .map(|option| (option.name.clone().into(), option.value.clone()))
            .collect(),
        _ => Vec::new(),
    }
}

/// t3code's settings section: a small heading over a bordered group of rows.
fn render_section(title: &'static str, rows: Vec<AnyElement>, cx: &App) -> AnyElement {
    render_section_with_actions(title, rows, gpui::Empty.into_any_element(), cx)
}

/// A section with buttons beside its title, as t3code's `headerAction`.
fn render_section_with_actions(
    title: &'static str,
    rows: Vec<AnyElement>,
    actions: AnyElement,
    cx: &App,
) -> AnyElement {
    let colors = cx.theme().colors().clone();
    let count = rows.len();
    v_flex()
        .gap_2()
        .child(
            h_flex()
                .justify_between()
                .child(Label::new(title).size(LabelSize::Small).color(Color::Muted))
                .child(actions),
        )
        .child(
            v_flex()
                .rounded_lg()
                .border_1()
                .border_color(colors.border)
                .bg(colors.panel_background)
                .children(rows.into_iter().enumerate().map(|(index, row)| {
                    div()
                        .when(index + 1 < count, |row| {
                            row.border_b_1().border_color(colors.border_variant)
                        })
                        .child(row)
                })),
        )
        .into_any_element()
}

/// A setting's title and description on the left, its control on the right.
fn render_row(
    title: &'static str,
    description: impl Into<SharedString>,
    control: AnyElement,
    _cx: &App,
) -> AnyElement {
    h_flex()
        .px_4()
        .py_3()
        .gap_6()
        .justify_between()
        .child(
            v_flex()
                .flex_1()
                .min_w_0()
                .gap_0p5()
                .child(Label::new(title))
                .child(
                    Label::new(description.into())
                        .size(LabelSize::Small)
                        .color(Color::Muted),
                ),
        )
        .child(div().flex_none().child(control))
        .into_any_element()
}

impl Render for SettingsPage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = cx.theme().colors().clone();
        let (title, sections) = match self.section {
            Section::General => ("General".into(), self.render_general(window, cx)),
            Section::Appearance => ("Appearance".into(), self.render_appearance(window, cx)),
            Section::Agents => ("Agents".into(), self.render_agents(window, cx)),
            Section::Machines => ("Machines".into(), self.render_machines(cx)),
            Section::Project(key) => match self.project(key, cx) {
                Some(project) => (
                    project.name(),
                    self.render_project(key.machine, project, window, cx),
                ),
                None => (
                    SharedString::from("General"),
                    self.render_general(window, cx),
                ),
            },
        };
        h_flex()
            .key_context(KEY_CONTEXT)
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(|_, _: &CloseSettings, _, cx| cx.emit(SettingsPageEvent::Close)))
            .size_full()
            .bg(colors.editor_background)
            .child(self.render_nav(window, cx))
            .child(
                div()
                    .id("settings-content-scroll")
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .child(
                        v_flex()
                            .id("settings-content")
                            .size_full()
                            .overflow_y_scroll()
                            .track_scroll(&self.content_scroll)
                            .items_center()
                            .child(
                                v_flex()
                                    .w_full()
                                    .max_w(CONTENT_WIDTH)
                                    .px_8()
                                    .py_6()
                                    .gap_6()
                                    .child(Headline::new(title).size(HeadlineSize::Small))
                                    .children(sections),
                            ),
                    )
                    .vertical_scrollbar_for(&self.content_scroll, window, cx),
            )
    }
}

/// t3code's Icon submenu: the machine kinds, with the one its server detected marked. The
/// server keeps the choice, so it's locked until a server that can is connected.
fn machine_icon_menu(
    menu: ContextMenu,
    client: Option<Entity<ServerClient>>,
    current_icon: IconName,
    cx: &App,
) -> ContextMenu {
    let lock = match client.as_ref().map(|client| client.read(cx)) {
        Some(client) if client.is_online() => {
            if client.has_capability(CAPABILITY_MACHINE_ICON) {
                None
            } else if client.is_outdated() {
                Some("Its server is too old to keep an icon. Update it to choose one.")
            } else {
                Some("Its server is too old to keep an icon.")
            }
        }
        _ => Some("Connect to this machine to change its icon."),
    };
    let icon = client
        .as_ref()
        .map(|client| client.read(cx).machine_icon().clone())
        .unwrap_or_default();
    menu.submenu_with_icon("Icon", current_icon, move |mut menu, _, _| {
        // Wrapped: Zed places a submenu as if it were at most 200px wide, so a wider one
        // would cover its parent menu.
        if let Some(lock) = lock {
            menu = menu
                .custom_entry(
                    move |_, _| {
                        div()
                            .max_w(px(180.))
                            .child(Label::new(lock).size(LabelSize::Small).color(Color::Muted))
                            .into_any_element()
                    },
                    |_, _| {},
                )
                .selectable(false)
                .separator();
        }
        let current = icon.kind();
        let detected = icon.detected.clone().unwrap_or(MachineKind::Server);
        // Zed's entries draw their icon as the check mark, so each kind draws its own row:
        // its icon, its name, "detected" and the check, as t3code's radio items do.
        for kind in MachineKind::ALL {
            let is_current = kind == current;
            let is_detected = kind == detected;
            let is_locked = lock.is_some();
            let row_kind = kind.clone();
            let client = client.clone();
            menu = menu
                .custom_entry(
                    move |_, _| {
                        let color = if is_locked {
                            Color::Disabled
                        } else {
                            Color::Default
                        };
                        h_flex()
                            .w_full()
                            .gap_1p5()
                            .child(
                                Icon::new(machine_kind_icon(&row_kind))
                                    .size(IconSize::Small)
                                    .color(Color::Muted),
                            )
                            .child(Label::new(row_kind.label()).color(color))
                            .when(is_detected, |row| {
                                row.child(
                                    Label::new("detected")
                                        .size(LabelSize::Small)
                                        .color(Color::Muted),
                                )
                            })
                            .child(div().flex_1().min_w_4())
                            .child(
                                div()
                                    .child(
                                        Icon::new(IconName::Check)
                                            .size(IconSize::Small)
                                            .color(Color::Accent),
                                    )
                                    .when(!is_current, |check| check.invisible()),
                            )
                            .into_any_element()
                    },
                    move |_, cx| {
                        if let Some(client) = &client {
                            client.read(cx).choose_machine_icon(kind.clone(), cx)
                        }
                    },
                )
                .selectable(!is_locked);
        }
        menu
    })
}

#[cfg(test)]
mod tests {
    use super::login_method_subject;

    #[test]
    fn login_method_subjects() {
        assert_eq!(
            login_method_subject("Log in with Google").as_deref(),
            Some("Google")
        );
        assert_eq!(
            login_method_subject("Login with opencode").as_deref(),
            Some("opencode")
        );
        assert_eq!(login_method_subject("ChatGPT").as_deref(), Some("ChatGPT"));
        assert_eq!(login_method_subject("API Key").as_deref(), Some("API Key"));
        assert_eq!(login_method_subject("Log In"), None);
        assert_eq!(login_method_subject("Login"), None);
    }
}
