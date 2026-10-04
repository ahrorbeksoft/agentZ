//! The settings page, laid out like t3code's: a list of sections on the left (General,
//! Appearance, Agents, Machines, then one entry per project) and the chosen section's rows on
//! the right.

use std::collections::{HashMap, HashSet};
use std::ops::Range;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::SystemTime;

use crate::agent_icons::agent_icon;
use crate::machines::{GroupKey, MachineId, Machines, ProjectGroupingMode, ProjectKey, ThreadKey};
use crate::project_store::ProjectStore;
use agentz_protocol::CAPABILITY_IMPORT_SESSIONS;
use agentz_protocol::agents::{AgentId, AgentListing, AgentSession, AgentSessions, InstallState};
use agentz_protocol::workspace::WorkspaceRemoval;
use gpui::{
    AnyElement, App, ClickEvent, Context, Entity, EventEmitter, FocusHandle, Focusable, KeyBinding,
    PathPromptOptions, PromptLevel, ScrollHandle, Subscription, Task, UniformListScrollHandle,
    Window, actions, uniform_list,
};
use projects::{Project, ProjectIcon, ProjectId, ThreadId, ThreadOrder, Workspace};
use text_input::{TextInput, TextInputEvent};
use theme::{Appearance, ThemeRegistry};
use ui::{
    ContextMenu, DropdownMenu, IconButtonShape, IconPosition, PopoverMenu, ScrollableHandle as _,
    Switch, TintColor, ToggleButtonGroup, ToggleButtonGroupSize, ToggleButtonGroupStyle,
    ToggleButtonSimple, Tooltip, WithScrollbar as _, prelude::*,
};
use util::ResultExt as _;

use agent_client_protocol::schema::v1 as acp;
use agentz_protocol::Request;
use agentz_protocol::thread::{AuthStatus, ConnectionStatus};

use std::collections::BTreeMap;

use crate::agent_login::{AgentLogin, LoginLayout};
use crate::agent_view::TOOLBAR_HEIGHT;
use crate::app_settings::{AppSettingsStore, MachineProfile, ThemeMode};
use crate::confirm_dialog::ConfirmRequest;
use crate::controls::{
    ActionButton, ActionStyle, account_badge, avatar, icon_tile, spinner, status_badge, status_dot,
};
use crate::elicitation_card::{ElicitationCard, sync_elicitation_cards};
use crate::machine_icon_picker::MachineIconPicker;
use crate::project_info::{
    MONOGRAM_COLORS, ProjectInfoStore, automatic_monogram, monogram_swatch, render_project_icon,
    workspace_icon,
};
use crate::project_switcher::compact_path;
use crate::registry_store::AgentRegistryStore;
use crate::server_client::{MachineStatus, ServerClient, ServerUpdate};
use crate::sidebar::{SIDEBAR_WIDTH, format_relative_time, render_footer_item};
use crate::thread_entity::AgentThread;

const KEY_CONTEXT: &str = "SettingsPage";
const CONTENT_WIDTH: Pixels = px(720.);
/// An agent can keep hundreds of sessions in a project, so the Threads tab shows them a page at
/// a time, as the sidebar shows archived threads.
const SESSIONS_INITIAL_COUNT: usize = 10;
const SESSIONS_PAGE_COUNT: usize = 25;

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
    /// Open the Add Machine dialog, or Edit… for the machine given.
    EditMachine(Option<MachineProfile>),
    /// Ask before a destructive action, in the shell's modal layer.
    Confirm(ConfirmRequest),
    /// Leave settings for the thread, as an agent's Threads tab opens one.
    OpenThread(ThreadKey),
}

#[derive(Clone, Copy, PartialEq)]
enum Section {
    General,
    Appearance,
    Agents,
    Machines,
    Project(ProjectKey),
}

/// What Settings › Agents shows, as Zed's settings window opens sub-pages.
enum AgentsPage {
    Installed,
    /// Zed's ACP Registry page, to install more agents.
    Registry,
    /// One agent's settings, with the connection made to log in or out.
    Agent(AccountPanel),
}

/// Zed's filter on the ACP Registry page.
#[derive(Clone, Copy, PartialEq)]
enum RegistryFilter {
    All,
    Installed,
    NotInstalled,
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
    /// Machines whose server is being updated, so a second click doesn't restart it midway.
    updating: HashSet<MachineId>,
    /// Whether this Mac's server has a launch agent, so it starts at login.
    starts_at_login: bool,
    agents_page: AgentsPage,
    registry_filter: RegistryFilter,
    nav_scroll: ScrollHandle,
    content_scroll: ScrollHandle,
    registry_scroll: UniformListScrollHandle,
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
        subscriptions.push(
            cx.subscribe(&agent_search, |this, _, _: &TextInputEvent, cx| {
                this.registry_scroll.set_offset(gpui::point(px(0.), px(0.)));
                cx.notify();
            }),
        );
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
            updating: Default::default(),
            starts_at_login: crate::login_item::is_enabled(),
            agents_page: AgentsPage::Installed,
            registry_filter: RegistryFilter::All,
            nav_scroll: ScrollHandle::new(),
            content_scroll: ScrollHandle::new(),
            registry_scroll: UniformListScrollHandle::new(),
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
        // Agents always opens on the installed agents; leaving an agent's page stops the agent.
        if !matches!(self.agents_page, AgentsPage::Installed) {
            self.show_agents_page(AgentsPage::Installed, window, cx);
        }
        if section == Section::Agents {
            self.registry(cx)
                .update(cx, |registry, cx| registry.refresh_if_stale(cx));
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
        if !client.read(cx).can_update_server() {
            self.restart_to_update(client, window, cx);
            return;
        }
        let machine = client.read(cx).machine();
        if !self.updating.insert(machine) {
            return;
        }
        let update = client.read(cx).update_server(false, cx);
        cx.spawn_in(window, async move |this, cx| {
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
                        this.update(cx, |this, _| this.updating.remove(&machine))
                            .ok();
                        return;
                    }
                    let update = client.read_with(cx, |client, cx| client.update_server(true, cx));
                    update.await
                }
                result => result,
            };
            this.update(cx, |this, _| this.updating.remove(&machine))
                .ok();
            // What runs there can't carry over, but restarting still brings in the new server.
            if let Err(error) = result {
                cx.update(|_, cx| client.read(cx).restart_server(cx)).ok();
                let detail = format!(
                    "Its terminals and agents couldn't be handed over, so they stopped. Agents \
                     load their sessions again when their threads open.\n\n{error:#}"
                );
                let answer = cx.update(|window, cx| {
                    window.prompt(
                        PromptLevel::Warning,
                        "The server restarted to update",
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

    /// A server too old to hand its terminals over updates by restarting, which stops what
    /// runs there; it asks first only when something does.
    fn restart_to_update(
        &mut self,
        client: Entity<ServerClient>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let running = client.read(cx).running(cx);
        if running.is_empty() {
            client.read(cx).restart_server(cx);
            return;
        }
        let detail = format!(
            "Updating restarts agentz-server on {}, which stops {}.",
            client.read(cx).label(),
            running.join(", ")
        );
        let answer = window.prompt(
            PromptLevel::Warning,
            "Stop what's running and update?",
            Some(&detail),
            &["Update", "Cancel"],
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

    /// Settings › Agents: the installed agents, the ACP Registry, or one agent's page.
    fn render_agents(&self, window: &mut Window, cx: &mut Context<Self>) -> Vec<AnyElement> {
        match &self.agents_page {
            AgentsPage::Installed => self.render_installed_agents(cx),
            // Laid out by `render_registry` instead, as its list scrolls on its own.
            AgentsPage::Registry => Vec::new(),
            AgentsPage::Agent(_) => self.render_agent_page(window, cx),
        }
    }

    /// The page's title (Zed's back button and breadcrumb on a sub-page) and the machine whose
    /// agents it shows.
    fn render_agents_header(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let colors = cx.theme().colors().clone();
        let heading = match &self.agents_page {
            AgentsPage::Installed => Headline::new("Agents")
                .size(HeadlineSize::Small)
                .into_any_element(),
            // The agent's own heading follows, so a plain way back is enough.
            AgentsPage::Agent(_) => h_flex()
                .id("agents-back")
                .debug_selector(|| "agents-back".into())
                .ml_neg_1p5()
                .px_1p5()
                .py_0p5()
                .gap_1p5()
                .rounded_md()
                .cursor_pointer()
                .hover(|style| style.bg(colors.ghost_element_hover))
                .on_click(cx.listener(|this, _, window, cx| {
                    this.show_agents_page(AgentsPage::Installed, window, cx)
                }))
                .child(
                    Icon::new(IconName::ArrowLeft)
                        .size(IconSize::XSmall)
                        .color(Color::Muted),
                )
                .child(
                    Label::new("Agents")
                        .size(LabelSize::Small)
                        .color(Color::Muted),
                )
                .into_any_element(),
            AgentsPage::Registry => h_flex()
                .min_w_0()
                .ml_neg_1p5()
                .gap_1()
                .child(
                    div().debug_selector(|| "agents-back".into()).child(
                        IconButton::new("agents-back", IconName::ArrowLeft)
                            .icon_size(IconSize::Small)
                            .shape(IconButtonShape::Square)
                            .tooltip(Tooltip::text("Back to Agents"))
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.show_agents_page(AgentsPage::Installed, window, cx)
                            })),
                    ),
                )
                .child(
                    Headline::new("Agents")
                        .size(HeadlineSize::Small)
                        .color(Color::Muted),
                )
                .child(
                    Headline::new("/")
                        .size(HeadlineSize::Small)
                        .color(Color::Muted),
                )
                .child(Headline::new("ACP Registry").size(HeadlineSize::Small))
                .into_any_element(),
        };
        let machine = if !self.machines.read(cx).has_remotes() {
            None
        } else if let AgentsPage::Agent(_) = self.agents_page {
            // An agent's page belongs to the machine it was opened on.
            let machines = self.machines.read(cx);
            Some(
                h_flex()
                    .gap_1p5()
                    .child(
                        Icon::new(machines.machine_icon(self.agents_machine, cx))
                            .size(IconSize::Small)
                            .color(Color::Muted),
                    )
                    .child(Label::new(machines.label(self.agents_machine, cx)).color(Color::Muted))
                    .into_any_element(),
            )
        } else {
            Some(self.render_agents_machine_picker(window, cx))
        };
        h_flex()
            .h(px(28.))
            .gap_4()
            .justify_between()
            .child(heading)
            .children(machine)
            .into_any_element()
    }

    /// The installed agents, each a link to its page.
    fn render_installed_agents(&self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let registry = self.registry(cx);
        let mut installed: Vec<AgentListing> = registry
            .read(cx)
            .agents()
            .iter()
            .filter(|agent| counts_as_installed(&agent.install_state))
            .cloned()
            .collect();
        installed.sort_by_key(|agent| agent.name().to_lowercase());
        let count = installed.len();
        let mut rows: Vec<AnyElement> = installed
            .iter()
            .enumerate()
            .map(|(index, agent)| {
                self.render_installed_agent(agent, index == 0, index + 1 == count, cx)
            })
            .collect();
        if rows.is_empty() {
            rows.push(self.render_agents_message(
                "No agents installed yet. Add one from the ACP Registry.",
                cx,
            ));
        }
        let add = Button::new("agents-add", "Add Agent")
            .style(ButtonStyle::Subtle)
            .label_size(LabelSize::Small)
            .color(Color::Muted)
            .start_icon(
                Icon::new(IconName::Plus)
                    .size(IconSize::XSmall)
                    .color(Color::Muted),
            )
            .on_click(cx.listener(|this, _, window, cx| {
                this.show_agents_page(AgentsPage::Registry, window, cx)
            }));
        vec![render_section_with_actions(
            "Installed",
            rows,
            add.into_any_element(),
            cx,
        )]
    }

    /// A settings row that opens the agent's page: its icon, name and version, Update when
    /// there's a newer one, and a chevron.
    fn render_installed_agent(
        &self,
        agent: &AgentListing,
        is_first: bool,
        is_last: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let colors = cx.theme().colors().clone();
        let id = agent.id().clone();
        let name = agent.name().clone();
        let (detail, has_update) =
            installed_version(agent).unwrap_or_else(|| ("Installing…".into(), false));
        // Its files are being replaced, so it can't be started to show its page.
        let is_installing = matches!(agent.install_state, InstallState::Installing);
        let row_id = format!("agent-row-{}", id.0);
        h_flex()
            .id(SharedString::from(row_id.clone()))
            .debug_selector(move || row_id)
            .px_4()
            .py_2p5()
            .gap_3()
            // So the hover fills the row up to the group's rounded border.
            .when(is_first, |row| row.rounded_t_lg())
            .when(is_last, |row| row.rounded_b_lg())
            .child(render_agent_tile(agent.id(), px(32.), cx))
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .child(Label::new(name.clone()).truncate())
                    .child(
                        Label::new(detail)
                            .size(LabelSize::Small)
                            .color(Color::Muted)
                            .truncate(),
                    ),
            )
            .when(has_update, |row| {
                row.child(
                    Button::new(
                        SharedString::from(format!("agent-update-{}", id.0)),
                        "Update",
                    )
                    .style(ButtonStyle::Tinted(TintColor::Accent))
                    .label_size(LabelSize::Small)
                    .on_click(self.install_listener(&id, cx)),
                )
            })
            .when(!is_installing, |row| {
                row.cursor_pointer()
                    .hover(|row| row.bg(colors.ghost_element_hover))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.open_agent(&id, &name, window, cx)
                    }))
                    .child(
                        Icon::new(IconName::ChevronRight)
                            .size(IconSize::Small)
                            .color(Color::Muted),
                    )
            })
            .into_any_element()
    }

    /// Zed's ACP Registry page: a search, the All / Installed / Not Installed filter, and a
    /// card for each agent that runs on the machine.
    /// The whole content area: the header and search stay put above Zed's `uniform_list` of
    /// cards. Scrolling re-renders the page every frame, so only the visible cards are built.
    fn render_registry(
        &self,
        header: AnyElement,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let colors = cx.theme().colors().clone();
        let query = self.agent_search.read(cx).text().trim().to_lowercase();
        let filter = self.registry_filter;
        let registry = self.registry(cx);
        let mut matches: Vec<(usize, &AgentListing)> = registry
            .read(cx)
            .agents()
            .iter()
            .enumerate()
            .filter(|(_, agent)| {
                let matches_query = query.is_empty()
                    || agent.name().to_lowercase().contains(&query)
                    || agent.id().0.to_lowercase().contains(&query)
                    || agent.description().to_lowercase().contains(&query);
                let is_installed = counts_as_installed(&agent.install_state);
                let matches_filter = match filter {
                    RegistryFilter::All => true,
                    RegistryFilter::Installed => is_installed,
                    RegistryFilter::NotInstalled => !is_installed,
                };
                agent.supports_current_platform() && matches_query && matches_filter
            })
            .collect();
        matches.sort_by_cached_key(|(_, agent)| agent.name().to_lowercase());
        let matches: Vec<usize> = matches.into_iter().map(|(index, _)| index).collect();

        let search = h_flex()
            .flex_1()
            .min_w_0()
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
            .child(div().flex_1().min_w_0().child(self.agent_search.clone()));
        let filter_buttons = ToggleButtonGroup::single_row(
            "registry-filter",
            [
                ToggleButtonSimple::new(
                    "All",
                    cx.listener(|this, _, _, cx| this.set_registry_filter(RegistryFilter::All, cx)),
                ),
                ToggleButtonSimple::new(
                    "Installed",
                    cx.listener(|this, _, _, cx| {
                        this.set_registry_filter(RegistryFilter::Installed, cx)
                    }),
                ),
                ToggleButtonSimple::new(
                    "Not Installed",
                    cx.listener(|this, _, _, cx| {
                        this.set_registry_filter(RegistryFilter::NotInstalled, cx)
                    }),
                ),
            ],
        )
        .style(ToggleButtonGroupStyle::Outlined)
        .size(ToggleButtonGroupSize::Custom(rems_from_px(32_f32)))
        .label_size(LabelSize::Default)
        .auto_width()
        .selected_index(match filter {
            RegistryFilter::All => 0,
            RegistryFilter::Installed => 1,
            RegistryFilter::NotInstalled => 2,
        });

        let column = || div().w_full().max_w(CONTENT_WIDTH).px_8();
        let list = if matches.is_empty() {
            let has_query = !query.is_empty();
            let empty = match (filter, has_query) {
                (RegistryFilter::All, true) => "No agents match your search.",
                (RegistryFilter::All, false) => "No agents available.",
                (RegistryFilter::Installed, true) => "No installed agents match your search.",
                (RegistryFilter::Installed, false) => "No installed agents.",
                (RegistryFilter::NotInstalled, true) => "No uninstalled agents match your search.",
                (RegistryFilter::NotInstalled, false) => "No uninstalled agents.",
            };
            h_flex()
                .flex_1()
                .min_h_0()
                .items_start()
                .justify_center()
                .child(
                    column().child(
                        div()
                            .rounded_md()
                            .border_1()
                            .border_dashed()
                            .border_color(colors.border)
                            .child(self.render_agents_message(empty, cx)),
                    ),
                )
                .into_any_element()
        } else {
            let count = matches.len();
            let render_cards = move |this: &mut Self,
                                     range: Range<usize>,
                                     _: &mut Window,
                                     cx: &mut Context<Self>| {
                let registry = this.registry(cx);
                let agents: Vec<AgentListing> = matches
                    .get(range)
                    .unwrap_or_default()
                    .iter()
                    .filter_map(|index| registry.read(cx).agents().get(*index).cloned())
                    .collect();
                agents
                    .iter()
                    .map(|agent| {
                        // Each row spans the list, so its scrollbar sits at the window's edge
                        // as on the other pages.
                        h_flex()
                            .w_full()
                            .justify_center()
                            .child(column().pb_2().child(this.render_registry_card(agent, cx)))
                            .into_any_element()
                    })
                    .collect::<Vec<_>>()
            };
            div()
                .id("registry-cards-scroll")
                .flex_1()
                .min_h_0()
                .child(
                    uniform_list("registry-cards", count, cx.processor(render_cards))
                        .size_full()
                        .pb_4()
                        .track_scroll(&self.registry_scroll),
                )
                .vertical_scrollbar_for(&self.registry_scroll, window, cx)
                .into_any_element()
        };
        v_flex()
            .flex_1()
            .min_w_0()
            .h_full()
            .child(
                h_flex().flex_none().w_full().justify_center().child(
                    column()
                        .flex()
                        .flex_col()
                        .pt_6()
                        .pb_3()
                        .gap_6()
                        .child(header)
                        .child(
                            h_flex()
                                .gap_2()
                                .child(search)
                                .child(div().flex_none().child(filter_buttons)),
                        ),
                ),
            )
            .child(list)
            .into_any_element()
    }

    fn set_registry_filter(&mut self, filter: RegistryFilter, cx: &mut Context<Self>) {
        self.registry_filter = filter;
        self.registry_scroll.set_offset(gpui::point(px(0.), px(0.)));
        cx.notify();
    }

    /// Zed's registry card: the agent's icon, name, version and description, links to its
    /// repository, website and license, and what can be done with it.
    fn render_registry_card(&self, agent: &AgentListing, cx: &mut Context<Self>) -> AnyElement {
        let colors = cx.theme().colors().clone();
        let id = agent.id().clone();
        let element_id = |action: &str| SharedString::from(format!("registry-{action}-{}", id.0));
        let mut actions: Vec<AnyElement> = Vec::new();
        match &agent.install_state {
            InstallState::NotInstalled => actions.push(
                Button::new(element_id("install"), "Install")
                    .style(ButtonStyle::Tinted(TintColor::Accent))
                    .start_icon(
                        Icon::new(IconName::Download)
                            .size(IconSize::Small)
                            .color(Color::Muted),
                    )
                    .on_click(self.install_listener(&id, cx))
                    .into_any_element(),
            ),
            InstallState::Installing => actions.push(
                Button::new(element_id("installing"), "Installing…")
                    .style(ButtonStyle::OutlinedGhost)
                    .disabled(true)
                    .into_any_element(),
            ),
            InstallState::Installed {
                update_available, ..
            } => {
                if *update_available {
                    actions.push(
                        Button::new(element_id("update"), "Update")
                            .style(ButtonStyle::Tinted(TintColor::Accent))
                            .on_click(self.install_listener(&id, cx))
                            .into_any_element(),
                    );
                }
                let uninstall = {
                    let id = id.clone();
                    let name = agent.name().clone();
                    cx.listener(move |this, _, window, cx| {
                        this.confirm_uninstall(&id, &name, window, cx)
                    })
                };
                actions.push(
                    Button::new(element_id("uninstall"), "Uninstall")
                        .style(ButtonStyle::OutlinedGhost)
                        .on_click(uninstall)
                        .into_any_element(),
                );
            }
            InstallState::Failed(error) => actions.push(
                Button::new(element_id("retry"), "Retry")
                    .style(ButtonStyle::Outlined)
                    .color(Color::Error)
                    .tooltip(Tooltip::text(error.clone()))
                    .on_click(self.install_listener(&id, cx))
                    .into_any_element(),
            ),
        }
        let card_id = format!("registry-card-{}", id.0);
        h_flex()
            .debug_selector(move || card_id)
            .p_3()
            .gap_3()
            .rounded_md()
            .border_1()
            .border_color(colors.border_variant)
            .bg(colors.elevated_surface_background.opacity(0.5))
            .child(render_agent_tile(agent.id(), px(32.), cx))
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .child(
                        h_flex()
                            .min_w_0()
                            .gap_2()
                            .child(Label::new(agent.name().clone()).truncate())
                            .child(
                                Label::new(version_label(agent.version()))
                                    .size(LabelSize::Small)
                                    .color(Color::Muted),
                            ),
                    )
                    .child(
                        Label::new(agent.description().clone())
                            .size(LabelSize::Small)
                            .color(Color::Muted)
                            .truncate(),
                    ),
            )
            .child(
                h_flex()
                    .flex_none()
                    .gap_1()
                    .children(render_agent_links(agent))
                    .children(actions),
            )
            .into_any_element()
    }

    /// In place of a list of agents: the registry loading, failing to load (with Retry), or
    /// `empty` once it has loaded.
    fn render_agents_message(&self, empty: &str, cx: &mut Context<Self>) -> AnyElement {
        let registry = self.registry(cx);
        let registry = registry.read(cx);
        let has_agents = !registry.agents().is_empty();
        let fetch_error = registry.fetch_error().filter(|_| !has_agents);
        let message: SharedString = if registry.is_fetching() && !has_agents {
            "Loading agents from the ACP Registry…".into()
        } else if let Some(error) = &fetch_error {
            format!("Couldn't load the ACP Registry: {error}").into()
        } else {
            empty.to_string().into()
        };
        h_flex()
            .px_4()
            .py_3()
            .gap_3()
            .justify_between()
            .child(Label::new(message).color(Color::Muted))
            .when(fetch_error.is_some(), |row| {
                row.child(
                    Button::new("retry-registry", "Retry")
                        .style(ButtonStyle::Outlined)
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.registry(cx)
                                .update(cx, |registry, cx| registry.refresh(cx))
                        })),
                )
            })
            .into_any_element()
    }

    fn install_listener(
        &self,
        id: &AgentId,
        cx: &mut Context<Self>,
    ) -> impl Fn(&ClickEvent, &mut Window, &mut App) + 'static {
        let id = id.clone();
        cx.listener(move |this, _, _, cx| {
            this.registry(cx)
                .update(cx, |registry, cx| registry.install(&id, cx))
        })
    }

    /// One agent's page: what it is and whether it's logged in, over tabs for its account, the
    /// defaults new threads start with, and its environment.
    fn render_agent_page(&self, window: &mut Window, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let Some(panel) = self.account() else {
            return Vec::new();
        };
        let listing = self.registry(cx).read(cx).agent(&panel.agent_id).cloned();
        let content = match panel.tab {
            AgentTab::Account => self.render_account_tab(cx),
            AgentTab::Defaults => self.render_agent_defaults(window, cx),
            AgentTab::Environment => self.render_agent_env(cx),
            AgentTab::Threads => self.render_agent_threads(window, cx),
        };
        vec![
            v_flex()
                .gap(px(22.))
                .child(self.render_agent_heading(listing.as_ref(), cx))
                .child(self.render_agent_tabs(panel.tab, cx))
                .into_any_element(),
            content,
        ]
    }

    /// The agent's icon, its name beside whether it's logged in, its version, description and
    /// links, Update when there's a newer version, and a menu with Uninstall.
    fn render_agent_heading(
        &self,
        listing: Option<&AgentListing>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some(panel) = self.account() else {
            return div().into_any_element();
        };
        let colors = cx.theme().colors().clone();
        let status_colors = cx.theme().status().clone();
        let connection = panel.connection.read(cx);
        let id = panel.agent_id.clone();
        let name = listing
            .map(|agent| agent.name().clone())
            .unwrap_or_else(|| connection.agent_name().clone());
        let (badge_label, badge_color) = match AccountState::of(connection) {
            AccountState::Connecting => ("Starting…", colors.text_muted),
            AccountState::Failed => ("Couldn't start", status_colors.error),
            AccountState::LoggingIn => ("Logging in…", colors.text_accent),
            AccountState::LoggedIn => ("Logged in", status_colors.success),
            AccountState::LoggedOut => ("Not logged in", status_colors.warning),
        };
        let (version, update_available) = listing
            .and_then(installed_version)
            .map_or((None, false), |(version, update_available)| {
                (Some(version), update_available)
            });
        let description = listing
            .map(|agent| agent.description().clone())
            .filter(|description| !description.trim().is_empty());
        let links = listing
            .map(|agent| render_agent_text_links(agent, cx))
            .unwrap_or_default();

        let mut details: Vec<AnyElement> = Vec::new();
        details.extend(version.map(|version| {
            div()
                .flex_none()
                .whitespace_nowrap()
                .child(version)
                .into_any_element()
        }));
        details.extend(description.map(|description| {
            div()
                .id("agent-description")
                .min_w_0()
                .overflow_hidden()
                .text_ellipsis()
                .whitespace_nowrap()
                .child(description.clone())
                .tooltip(Tooltip::text(description))
                .into_any_element()
        }));
        details.extend(links);
        let detail_count = details.len();
        let details = details.into_iter().enumerate().flat_map(|(index, detail)| {
            let separator =
                (index + 1 < detail_count).then(|| div().flex_none().child("·").into_any_element());
            std::iter::once(detail).chain(separator)
        });

        let icon = match agent_icon(&id, cx) {
            Some(markup) => Icon::from_svg_markup(markup),
            None => Icon::new(IconName::Sparkle),
        };
        let menu_name = name.clone();
        let menu_id = id.clone();
        let page = cx.weak_entity();
        let menu = PopoverMenu::new("agent-menu")
            .menu(move |window, cx| {
                let page = page.clone();
                let id = menu_id.clone();
                let name = menu_name.clone();
                Some(ContextMenu::build(window, cx, move |menu, _, _| {
                    menu.entry(format!("Uninstall {name}…"), None, move |window, cx| {
                        page.update(cx, |page, cx| {
                            page.confirm_uninstall(&id, &name, window, cx)
                        })
                        .log_err();
                    })
                }))
            })
            .trigger_with_tooltip(
                IconButton::new("agent-menu-trigger", IconName::Ellipsis)
                    .style(ButtonStyle::Outlined)
                    .size(ButtonSize::Medium)
                    .icon_size(IconSize::Small)
                    .icon_color(Color::Muted),
                Tooltip::text("More"),
            )
            .anchor(gpui::Anchor::TopRight)
            .offset(gpui::point(px(0.), px(4.)));

        h_flex()
            .gap_4()
            .child(icon_tile(icon.color(Color::Default), px(52.), cx))
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .gap_1()
                    .child(
                        h_flex()
                            .min_w_0()
                            .gap_2()
                            .child(
                                div()
                                    .min_w_0()
                                    .overflow_hidden()
                                    .text_ellipsis()
                                    .whitespace_nowrap()
                                    .text_size(rems_from_px(18_f32))
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .child(name),
                            )
                            .child(status_badge(badge_label, badge_color)),
                    )
                    .child(
                        h_flex()
                            .min_w_0()
                            .gap_1()
                            .text_size(rems_from_px(12_f32))
                            .text_color(colors.text_muted)
                            .children(details),
                    ),
            )
            .when(update_available, |row| {
                row.child(
                    ActionButton::new("agent-update", "Update")
                        .style(ActionStyle::Primary)
                        .on_click(self.install_listener(&id, cx)),
                )
            })
            .child(menu)
            .into_any_element()
    }

    /// Account, Defaults, Environment and Threads, underlined when chosen.
    fn render_agent_tabs(&self, current: AgentTab, cx: &mut Context<Self>) -> AnyElement {
        let colors = cx.theme().colors().clone();
        h_flex()
            .gap_5()
            .border_b_1()
            .border_color(colors.border_variant)
            .children(
                [
                    (AgentTab::Account, "Account"),
                    (AgentTab::Defaults, "Defaults"),
                    (AgentTab::Environment, "Environment"),
                    (AgentTab::Threads, "Threads"),
                ]
                .into_iter()
                .map(|(tab, label)| {
                    let is_current = tab == current;
                    let selector = format!("agent-tab-{}", label.to_lowercase());
                    div()
                        .id(SharedString::from(selector.clone()))
                        .debug_selector(move || selector)
                        .pb(px(10.))
                        // Over the strip's border, so the underline replaces it.
                        .mb(px(-1.))
                        .border_b_2()
                        .border_color(if is_current {
                            colors.text_accent
                        } else {
                            gpui::transparent_black()
                        })
                        .text_size(rems_from_px(13_f32))
                        .text_color(if is_current {
                            colors.text
                        } else {
                            colors.text_muted
                        })
                        .when(!is_current, |this| {
                            this.cursor_pointer()
                                .hover(|style| style.text_color(colors.text))
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.select_agent_tab(tab, cx);
                                    this.content_scroll.set_offset(gpui::point(px(0.), px(0.)));
                                }))
                        })
                        .child(label)
                }),
            )
            .into_any_element()
    }

    /// The agent whose page is open.
    fn account(&self) -> Option<&AccountPanel> {
        match &self.agents_page {
            AgentsPage::Agent(panel) => Some(panel),
            _ => None,
        }
    }

    fn account_mut(&mut self) -> Option<&mut AccountPanel> {
        match &mut self.agents_page {
            AgentsPage::Agent(panel) => Some(panel),
            _ => None,
        }
    }

    fn show_agents_page(&mut self, page: AgentsPage, window: &mut Window, cx: &mut Context<Self>) {
        self.agents_page = page;
        if let AgentsPage::Registry = self.agents_page {
            self.agent_search
                .update(cx, |search, cx| search.set_text(String::new(), cx));
            // Typing on the registry page searches it.
            window.focus(&self.agent_search.focus_handle(cx), cx);
            self.registry(cx)
                .update(cx, |registry, cx| registry.refresh_if_stale(cx));
        } else {
            // The last page's inputs are gone, and actions need focus under the shell.
            window.focus(&self.focus_handle, cx);
        }
        self.content_scroll.set_offset(gpui::point(px(0.), px(0.)));
        self.registry_scroll.set_offset(gpui::point(px(0.), px(0.)));
        cx.notify();
    }

    /// Opens the agent's page, starting the agent to talk to it. Leaving the page stops it.
    fn open_agent(
        &mut self,
        id: &AgentId,
        name: &SharedString,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let client = self.agents_client(cx);
        let agent_settings = client.read(cx).agent_settings(&id.0);
        let name = name.clone();
        let account_agent_id = id.clone();
        let connection =
            cx.new(|cx| AgentThread::open_account(client.clone(), account_agent_id, name, cx));
        let login = cx
            .new(|cx| AgentLogin::new(connection.clone(), LoginLayout::Rows, Some(id.clone()), cx));
        // The server remembers the options and modes the agent offers, and logins made in
        // the panel.
        let subscription = cx.observe(&connection, |this, connection, cx| {
            if let Some(panel) = this.account_mut() {
                sync_elicitation_cards(&mut panel.elicitation_cards, &connection, cx);
                let thread = connection.read(cx);
                let was_authenticating =
                    std::mem::replace(&mut panel.was_authenticating, thread.is_authenticating());
                // A finished login settles the account change.
                if was_authenticating
                    && !thread.is_authenticating()
                    && thread.auth_error().is_none()
                {
                    panel.changing_account = false;
                }
            }
            cx.notify()
        });
        let env_rows = agent_settings
            .env
            .iter()
            .map(|(key, value)| self.new_env_row(key, value, cx))
            .collect();
        let panel = AccountPanel {
            agent_id: id.clone(),
            tab: AgentTab::Account,
            connection,
            login,
            elicitation_cards: Vec::new(),
            changing_account: false,
            was_authenticating: false,
            env_rows,
            sessions: None,
            sessions_project: None,
            sessions_shown: SESSIONS_INITIAL_COUNT,
            importing: HashSet::new(),
            import_error: None,
            _subscriptions: [subscription],
        };
        self.show_agents_page(AgentsPage::Agent(panel), window, cx);
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
        let Some(panel) = self.account() else {
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

    /// Zed's per-agent defaults: what a new session starts with. Choosing a setting in a thread
    /// changes these too.
    fn render_agent_defaults(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        const TITLE: &str = "Defaults for New Threads";
        let Some(panel) = self.account() else {
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
            rows.push(render_row(
                option.name.clone(),
                option.description.clone().unwrap_or_default(),
                DropdownMenu::new(
                    SharedString::from(format!("agent-default-{}", option.id.0)),
                    label,
                    menu,
                )
                .into_any_element(),
                cx,
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
            rows.push(render_row(
                "Mode",
                "",
                DropdownMenu::new("agent-default-mode", label, menu).into_any_element(),
                cx,
            ));
        }
        if rows.is_empty() {
            let connection = panel.connection.read(cx);
            let message = match (connection.status(), connection.logged_in()) {
                (_, Some(false)) => format!("Log in to {agent_name} to see its settings here."),
                (ConnectionStatus::Connecting, _) => format!("Loading {agent_name}'s settings…"),
                _ => format!("{agent_name} doesn't offer any settings."),
            };
            let message = div()
                .px_4()
                .py_3()
                .child(Label::new(message).color(Color::Muted))
                .into_any_element();
            return render_section(TITLE, vec![message], cx);
        }
        render_section_with_note(
            TITLE,
            rows,
            "Choosing one in a thread also makes it the default.",
            cx,
        )
    }

    /// The variables the agent starts with, one row each.
    fn render_agent_env(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(panel) = self.account() else {
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
                .bg(colors.editor_background)
                .child(input)
        };
        let mut rows: Vec<AnyElement> = panel
            .env_rows
            .iter()
            .enumerate()
            .map(|(index, row)| {
                h_flex()
                    .px_4()
                    .py_2()
                    .gap_2()
                    .child(div().w(px(180.)).child(input_box(row.key.clone())))
                    .child(Label::new("=").color(Color::Muted))
                    .child(div().flex_1().min_w_0().child(input_box(row.value.clone())))
                    .child(
                        IconButton::new(("remove-env", index), IconName::Close)
                            .icon_size(IconSize::Small)
                            .tooltip(Tooltip::text("Remove Variable"))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                if let Some(panel) = this.account_mut()
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
        if rows.is_empty() {
            rows.push(
                div()
                    .px_4()
                    .py_3()
                    .child(Label::new("No variables.").color(Color::Muted))
                    .into_any_element(),
            );
        }
        let add = Button::new("add-env", "Add Variable")
            .style(ButtonStyle::Subtle)
            .label_size(LabelSize::Small)
            .color(Color::Muted)
            .start_icon(
                Icon::new(IconName::Plus)
                    .size(IconSize::XSmall)
                    .color(Color::Muted),
            )
            .on_click(cx.listener(|this, _, _, cx| {
                let row = this.new_env_row("", "", cx);
                if let Some(panel) = this.account_mut() {
                    panel.env_rows.push(row);
                }
                cx.notify();
            }));
        v_flex()
            .gap_2()
            .child(render_section_with_actions(
                "Environment Variables",
                rows,
                add.into_any_element(),
                cx,
            ))
            .child(
                Label::new(format!(
                    "Passed to {agent_name} when it starts. Running threads pick them up after \
                     Reload Agent."
                ))
                .size(LabelSize::Small)
                .color(Color::Muted),
            )
            .into_any_element()
    }

    /// The account card: who the agent is logged in as, with Change Account and Log Out; or
    /// that it isn't, with a row for each way it offers to log in; or the login in progress.
    /// The page a login asks to open shows in the card, other requests for input under it.
    fn render_account_tab(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(account) = self.account() else {
            return div().into_any_element();
        };
        let colors = cx.theme().colors().clone();
        let status_colors = cx.theme().status().clone();
        let connection = account.connection.read(cx);
        let agent_name = connection.agent_name().clone();
        let state = AccountState::of(connection);
        let has_auth_methods = !connection.auth_methods().is_empty();
        let can_log_out = connection.supports_logout();
        let auth_status = connection
            .auth_status()
            .filter(|status| status.is_logged_in())
            .cloned();
        let auth_error = connection.auth_error().cloned();
        let failure = match connection.status() {
            ConnectionStatus::Failed(error) => Some(error.clone()),
            _ => None,
        };
        // Agents that don't report their account leave the method last used from agentZ.
        let login_method = connection
            .client()
            .read(cx)
            .agent_settings(&account.agent_id.0)
            .login_method;
        let machine = self.machines.read(cx).label(self.agents_machine, cx);
        let check_again = IconButton::new("account-check", IconName::RotateCw)
            .icon_size(IconSize::Small)
            .icon_color(Color::Muted)
            .tooltip(Tooltip::text("Check Again"))
            .on_click(cx.listener(|this, _, _, cx| {
                if let Some(account) = this.account_mut() {
                    account
                        .connection
                        .update(cx, |connection, cx| connection.check_login(cx));
                }
            }))
            .into_any_element();

        let mut rows: Vec<AnyElement> = Vec::new();
        let mut shows_login = false;
        match state {
            AccountState::Connecting => rows.push(render_status_row(
                spinner(Color::Muted),
                format!("Checking whether {agent_name} is logged in…").into(),
                None,
                Vec::new(),
            )),
            AccountState::Failed => rows.push(render_status_row(
                status_dot(status_colors.error).into_any_element(),
                format!("Couldn't start {agent_name}").into(),
                failure.map(|error| (error, Color::Error)),
                vec![
                    ActionButton::new("account-retry", "Try Again")
                        .on_click(cx.listener(|this, _, window, cx| this.reopen_agent(window, cx)))
                        .into_any_element(),
                ],
            )),
            AccountState::LoggingIn => shows_login = true,
            AccountState::LoggedOut => {
                let subtitle = match auth_error {
                    Some(error) => (error, Color::Error),
                    None if has_auth_methods => (
                        format!(
                            "Choose how {agent_name} logs in. Every thread with it shares the \
                             login."
                        )
                        .into(),
                        Color::Muted,
                    ),
                    None => (
                        format!(
                            "{agent_name} doesn't offer logging in from agentZ. Log in where it \
                             runs, then check again."
                        )
                        .into(),
                        Color::Muted,
                    ),
                };
                rows.push(render_status_row(
                    status_dot(status_colors.warning).into_any_element(),
                    "Not logged in".into(),
                    Some(subtitle),
                    vec![check_again],
                ));
                shows_login = has_auth_methods;
            }
            AccountState::LoggedIn => {
                let email = auth_status
                    .as_ref()
                    .and_then(|status| status.account.as_ref())
                    .and_then(|account| account.email.clone());
                let title: SharedString = match &email {
                    Some(email) => email.clone().into(),
                    None => logged_in_title(auth_status.as_ref(), login_method.as_deref()),
                };
                let details = auth_status
                    .as_ref()
                    .map(account_details)
                    .filter(|details| !details.is_empty())
                    .map(|details| SharedString::from(details.join(" · ")));
                let mut actions = Vec::new();
                if has_auth_methods {
                    actions.push(if account.changing_account {
                        ActionButton::new("account-change", "Cancel")
                            .style(ActionStyle::Ghost)
                            .on_click(
                                cx.listener(|this, _, _, cx| this.set_changing_account(false, cx)),
                            )
                            .into_any_element()
                    } else {
                        ActionButton::new("account-change", "Change Account")
                            .on_click(
                                cx.listener(|this, _, _, cx| this.set_changing_account(true, cx)),
                            )
                            .into_any_element()
                    });
                }
                if can_log_out && !account.changing_account {
                    actions.push(
                        ActionButton::new("account-logout", "Log Out")
                            .style(ActionStyle::Ghost)
                            .on_click(
                                cx.listener(|this, _, window, cx| this.confirm_logout(window, cx)),
                            )
                            .into_any_element(),
                    );
                }
                rows.push(
                    h_flex()
                        .px_4()
                        .py_3()
                        .gap_3()
                        .child(match &email {
                            Some(email) => avatar(email, cx),
                            None => account_badge(cx),
                        })
                        .child(
                            v_flex()
                                .flex_1()
                                .min_w_0()
                                .gap_0p5()
                                .child(Label::new(title).truncate())
                                .children(details.map(|details| {
                                    Label::new(details)
                                        .size(LabelSize::Small)
                                        .color(Color::Muted)
                                        .truncate()
                                })),
                        )
                        .children(actions)
                        .into_any_element(),
                );
                shows_login = account.changing_account;
            }
        }
        let cards: Vec<AnyElement> = account
            .elicitation_cards
            .iter()
            .map(|card| card.clone().into_any_element())
            .collect();

        v_flex()
            .gap_5()
            .child(
                v_flex()
                    .debug_selector(|| "account-card".into())
                    .rounded_lg()
                    .border_1()
                    .border_color(colors.border)
                    .bg(colors.panel_background)
                    .overflow_hidden()
                    .children(rows)
                    .when(shows_login, |card| card.child(account.login.clone())),
            )
            .when(state == AccountState::LoggedIn, |tab| {
                tab.child(
                    Label::new(format!(
                        "Every thread with {agent_name} on {machine} uses this login."
                    ))
                    .size(LabelSize::Small)
                    .color(Color::Muted),
                )
            })
            .children(cards)
            .into_any_element()
    }

    fn set_changing_account(&mut self, changing_account: bool, cx: &mut Context<Self>) {
        if let Some(account) = self.account_mut() {
            account.changing_account = changing_account;
        }
        cx.notify();
    }

    /// Starts the agent again for its page, on the tab that was open.
    fn reopen_agent(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(account) = self.account() else {
            return;
        };
        let id = account.agent_id.clone();
        let tab = account.tab;
        let name = account.connection.read(cx).agent_name().clone();
        self.open_agent(&id, &name, window, cx);
        self.select_agent_tab(tab, cx);
    }

    /// Shows one of the agent page's tabs. Threads lists the agent's sessions the first time,
    /// and again after the agent couldn't list them.
    fn select_agent_tab(&mut self, tab: AgentTab, cx: &mut Context<Self>) {
        let Some(panel) = self.account_mut() else {
            return;
        };
        panel.tab = tab;
        let needs_listing = match &panel.sessions {
            None | Some(SessionList::Failed(_)) => true,
            Some(SessionList::Listed(sessions)) => !matches!(sessions, AgentSessions::Listed(_)),
            Some(SessionList::Listing { .. }) => false,
        };
        if tab == AgentTab::Threads && needs_listing {
            self.list_agent_sessions(cx);
        }
        cx.notify();
    }

    fn list_agent_sessions(&mut self, cx: &mut Context<Self>) {
        let Some(panel) = self.account() else {
            return;
        };
        let agent_id = panel.agent_id.clone();
        let client = panel.connection.read(cx).client().clone();
        // While disconnected, the request fails and says so.
        let is_outdated = client.read(cx).connection().is_some()
            && !client.read(cx).has_capability(CAPABILITY_IMPORT_SESSIONS);
        if is_outdated {
            if let Some(panel) = self.account_mut() {
                panel.sessions = Some(SessionList::Failed(
                    "This machine's agentz-server can't list threads. Update it to import them."
                        .into(),
                ));
            }
            cx.notify();
            return;
        }
        let listing = client
            .read(cx)
            .projects()
            .read(cx)
            .list_agent_sessions(agent_id.clone(), cx);
        let task = cx.spawn(async move |this, cx| {
            let listing = listing.await;
            this.update(cx, |this, cx| {
                let Some(panel) = this
                    .account_mut()
                    .filter(|panel| panel.agent_id == agent_id)
                else {
                    return;
                };
                panel.sessions = Some(match listing {
                    Ok(sessions) => SessionList::Listed(sessions),
                    Err(error) => SessionList::Failed(format!("{error:#}").into()),
                });
                panel.sessions_shown = SESSIONS_INITIAL_COUNT;
                cx.notify();
            })
            .log_err();
        });
        if let Some(panel) = self.account_mut() {
            panel.sessions = Some(SessionList::Listing { _task: task });
            panel.import_error = None;
        }
        cx.notify();
    }

    /// Adds an archived thread for each of the sessions.
    fn import_agent_sessions(&mut self, sessions: Vec<AgentSession>, cx: &mut Context<Self>) {
        let Some(panel) = self.account_mut() else {
            return;
        };
        let agent_id = panel.agent_id.clone();
        let session_ids: Vec<String> = sessions
            .iter()
            .map(|session| session.session_id.clone())
            .collect();
        panel.importing.extend(session_ids.iter().cloned());
        panel.import_error = None;
        let import = panel
            .connection
            .read(cx)
            .client()
            .read(cx)
            .projects()
            .read(cx)
            .import_agent_sessions(agent_id.clone(), sessions, cx);
        cx.spawn(async move |this, cx| {
            let imported = import.await;
            this.update(cx, |this, cx| {
                let Some(panel) = this
                    .account_mut()
                    .filter(|panel| panel.agent_id == agent_id)
                else {
                    return;
                };
                for session_id in &session_ids {
                    panel.importing.remove(session_id);
                }
                if let Err(error) = imported {
                    panel.import_error = Some(format!("Couldn't import: {error:#}").into());
                }
                cx.notify();
            })
            .log_err();
        })
        .detach();
        cx.notify();
    }

    /// The agent's sessions on the Threads tab: those in the chosen project, newest first,
    /// each with Import, or Open once agentZ has it.
    fn render_agent_threads(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let Some(panel) = self.account() else {
            return div().into_any_element();
        };
        let status_colors = cx.theme().status().clone();
        let connection = panel.connection.read(cx);
        let agent_name = connection.agent_name().clone();
        let client = connection.client().clone();
        let machine = self.machines.read(cx).label(self.agents_machine, cx);
        let message = |text: String| {
            div()
                .px_4()
                .py_3()
                .child(Label::new(text).color(Color::Muted))
                .into_any_element()
        };

        let mut rows: Vec<AnyElement> = Vec::new();
        let mut toolbar = None;
        let mut notes: Vec<(SharedString, Color)> = Vec::new();
        match &panel.sessions {
            None | Some(SessionList::Listing { .. }) => rows.push(render_status_row(
                spinner(Color::Muted),
                format!("Listing {agent_name}'s threads…").into(),
                None,
                Vec::new(),
            )),
            Some(SessionList::Failed(error)) => rows.push(render_status_row(
                status_dot(status_colors.error).into_any_element(),
                format!("Couldn't list {agent_name}'s threads").into(),
                Some((error.clone(), Color::Error)),
                vec![
                    ActionButton::new("sessions-retry", "Try Again")
                        .on_click(cx.listener(|this, _, _, cx| this.list_agent_sessions(cx)))
                        .into_any_element(),
                ],
            )),
            Some(SessionList::Listed(AgentSessions::LoggedOut)) => rows.push(message(format!(
                "Log in to {agent_name} to see its threads here."
            ))),
            Some(SessionList::Listed(AgentSessions::Unsupported | AgentSessions::Unknown(_))) => {
                rows.push(message(format!(
                    "{agent_name} doesn't list its threads, so they can't be imported."
                )))
            }
            Some(SessionList::Listed(AgentSessions::Listed(sessions))) => {
                let store = client.read(cx).projects().read(cx);
                let projects = store.projects().to_vec();
                let threads: HashMap<&str, ThreadId> = sessions
                    .iter()
                    .filter_map(|session| {
                        let thread =
                            store.thread_for_session(&panel.agent_id.0, &session.session_id)?;
                        Some((session.session_id.as_str(), thread))
                    })
                    .collect();
                let thread_of =
                    |session: &AgentSession| threads.get(session.session_id.as_str()).copied();
                let project = panel
                    .sessions_project
                    .and_then(|id| projects.iter().find(|project| project.id == id))
                    .or_else(|| {
                        projects.iter().find(|project| {
                            sessions
                                .iter()
                                .any(|session| session.project_id == Some(project.id))
                        })
                    })
                    .or(projects.first());
                let outside_projects = sessions
                    .iter()
                    .filter(|session| {
                        session
                            .project_id
                            .is_none_or(|id| projects.iter().all(|project| project.id != id))
                    })
                    .count();
                match project {
                    None => rows.push(message(format!(
                        "Add a project on {machine} to import threads into it."
                    ))),
                    Some(project) => {
                        let mut in_project: Vec<&AgentSession> = sessions
                            .iter()
                            .filter(|session| session.project_id == Some(project.id))
                            .collect();
                        in_project.sort_by_key(|session| std::cmp::Reverse(session.updated_at));
                        let to_import: Vec<AgentSession> = in_project
                            .iter()
                            .filter(|session| {
                                thread_of(session).is_none()
                                    && !panel.importing.contains(&session.session_id)
                            })
                            .map(|session| (*session).clone())
                            .collect();
                        let in_agentz = in_project
                            .iter()
                            .filter(|session| thread_of(session).is_some())
                            .count();
                        toolbar = Some(self.render_sessions_toolbar(
                            project,
                            in_project.len(),
                            in_agentz,
                            to_import,
                            window,
                            cx,
                        ));
                        if in_project.is_empty() {
                            rows.push(message(format!(
                                "No threads with {agent_name} in {}.",
                                project.name()
                            )));
                        }
                        let now = SystemTime::now();
                        for session in in_project.iter().take(panel.sessions_shown) {
                            rows.push(self.render_session_row(
                                session,
                                project,
                                thread_of(session),
                                panel.importing.contains(&session.session_id),
                                now,
                                cx,
                            ));
                        }
                        let hidden = in_project.len().saturating_sub(panel.sessions_shown);
                        if hidden > 0 {
                            rows.push(render_show_more_sessions(hidden, cx));
                        }
                    }
                }
                if outside_projects > 0 {
                    notes.push((
                        match outside_projects {
                            1 => {
                                format!("1 more is in a folder that isn't a project on {machine}.")
                            }
                            count => format!(
                                "{count} more are in folders that aren't projects on \
                                     {machine}."
                            ),
                        }
                        .into(),
                        Color::Muted,
                    ));
                }
            }
        }
        if let Some(error) = &panel.import_error {
            notes.push((error.clone(), Color::Error));
        }

        v_flex()
            .gap_3()
            .child(
                v_flex()
                    .gap_0p5()
                    .child(Label::new(format!(
                        "Threads {agent_name} keeps on {machine}"
                    )))
                    .child(
                        Label::new(
                            "Started in agentZ or anywhere else. Imported threads go to \
                             Archived, and open where they left off.",
                        )
                        .size(LabelSize::Small)
                        .color(Color::Muted),
                    ),
            )
            .children(toolbar)
            .child(render_rows(rows, cx))
            .children(
                notes
                    .into_iter()
                    .map(|(note, color)| Label::new(note).size(LabelSize::Small).color(color)),
            )
            .into_any_element()
    }

    /// The project picker, how many of the project's threads agentZ has, and Import All.
    fn render_sessions_toolbar(
        &self,
        project: &Project,
        count: usize,
        in_agentz: usize,
        to_import: Vec<AgentSession>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let projects: Vec<(ProjectId, SharedString)> = self
            .machines
            .read(cx)
            .projects(self.agents_machine, cx)
            .map(|store| {
                store
                    .read(cx)
                    .projects()
                    .iter()
                    .map(|project| (project.id, project.name()))
                    .collect()
            })
            .unwrap_or_default();
        let current = project.id;
        let page = cx.weak_entity();
        let menu = ContextMenu::build(window, cx, move |mut menu, _, _| {
            for (id, name) in projects {
                let page = page.clone();
                menu = menu.toggleable_entry(
                    name,
                    id == current,
                    IconPosition::End,
                    None,
                    move |_, cx| {
                        page.update(cx, |page, cx| {
                            if let Some(panel) = page.account_mut() {
                                panel.sessions_project = Some(id);
                                panel.sessions_shown = SESSIONS_INITIAL_COUNT;
                            }
                            cx.notify();
                        })
                        .log_err();
                    },
                );
            }
            menu
        });
        let summary = match (count, in_agentz) {
            (1, 0) => "1 thread".to_string(),
            (count, 0) => format!("{count} threads"),
            (count, in_agentz) => format!("{count} threads, {in_agentz} in agentZ"),
        };
        let import_count = to_import.len();
        let is_importing = self
            .account()
            .is_some_and(|panel| !panel.importing.is_empty());
        h_flex()
            .gap_3()
            .child(DropdownMenu::new("sessions-project", project.name(), menu))
            .child(
                Label::new(summary)
                    .size(LabelSize::Small)
                    .color(Color::Muted),
            )
            .child(div().flex_1())
            .child(
                IconButton::new("sessions-refresh", IconName::RotateCw)
                    .icon_size(IconSize::Small)
                    .icon_color(Color::Muted)
                    .tooltip(Tooltip::text("Refresh"))
                    .on_click(cx.listener(|this, _, _, cx| this.list_agent_sessions(cx))),
            )
            .when(import_count > 1, |toolbar| {
                toolbar.child(
                    ActionButton::new("sessions-import-all", format!("Import All {import_count}"))
                        .disabled(is_importing)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.import_agent_sessions(to_import.clone(), cx)
                        })),
                )
            })
            .into_any_element()
    }

    /// A session's title, where it ran and when, and Import, or Open once agentZ has it.
    fn render_session_row(
        &self,
        session: &AgentSession,
        project: &Project,
        thread: Option<ThreadId>,
        is_importing: bool,
        now: SystemTime,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let title: SharedString = session
            .title
            .clone()
            .unwrap_or_else(|| session.session_id.clone())
            .into();
        let checkout = session.workspace.as_ref().map(|path| {
            let workspace = project
                .workspaces
                .iter()
                .find(|workspace| workspace.path == *path);
            let name = workspace
                .and_then(|workspace| workspace.branch.clone())
                .or_else(|| {
                    path.file_name()
                        .map(|name| name.to_string_lossy().into_owned())
                })
                .unwrap_or_else(|| path.display().to_string());
            let kind = workspace.map_or("Worktree", |workspace| workspace.kind.label());
            format!("{name} · {kind}")
        });
        let age = session
            .updated_at
            .map(|time| match format_relative_time(time, now).as_str() {
                "now" => "now".to_string(),
                age => format!("{age} ago"),
            });
        let details = checkout
            .into_iter()
            .chain(age)
            .collect::<Vec<_>>()
            .join(" · ");
        let session_id = session.session_id.clone();
        let action_selector = match thread {
            Some(_) => format!("session-in-agentz-{session_id}"),
            None if is_importing => format!("session-importing-{session_id}"),
            None => format!("session-import-{session_id}"),
        };
        let action = match thread {
            Some(thread) => {
                let machine = self.agents_machine;
                let open_selector = format!("session-open-{session_id}");
                h_flex()
                    .gap_2()
                    .child(
                        Icon::new(IconName::Check)
                            .size(IconSize::XSmall)
                            .color(Color::Success),
                    )
                    .child(
                        Label::new("In agentZ")
                            .size(LabelSize::Small)
                            .color(Color::Success),
                    )
                    .child(
                        div().debug_selector(move || open_selector).child(
                            ActionButton::new(
                                SharedString::from(format!("session-open-{session_id}")),
                                "Open",
                            )
                            .style(ActionStyle::Ghost)
                            .on_click(cx.listener(
                                move |_, _, _, cx| {
                                    cx.emit(SettingsPageEvent::OpenThread(ThreadKey {
                                        machine,
                                        thread,
                                    }))
                                },
                            )),
                        ),
                    )
                    .into_any_element()
            }
            None if is_importing => Label::new("Importing…")
                .size(LabelSize::Small)
                .color(Color::Muted)
                .into_any_element(),
            None => {
                let session = session.clone();
                ActionButton::new(
                    SharedString::from(format!("session-import-{session_id}")),
                    "Import",
                )
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.import_agent_sessions(vec![session.clone()], cx)
                }))
                .into_any_element()
            }
        };
        let selector = format!("agent-session-{session_id}");
        h_flex()
            .debug_selector(move || selector)
            .px_4()
            .py_2()
            .gap_3()
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .gap_0p5()
                    .child(Label::new(title).truncate())
                    .when(!details.is_empty(), |column| {
                        column.child(
                            h_flex()
                                .min_w_0()
                                .gap_1()
                                .when(session.workspace.is_some(), |line| {
                                    line.child(
                                        Icon::new(IconName::GitBranch)
                                            .size(IconSize::XSmall)
                                            .color(Color::Muted),
                                    )
                                })
                                .child(
                                    Label::new(details)
                                        .size(LabelSize::Small)
                                        .color(Color::Muted)
                                        .truncate(),
                                ),
                        )
                    }),
            )
            .child(
                div()
                    .flex_none()
                    .debug_selector(move || action_selector)
                    .child(action),
            )
            .into_any_element()
    }

    /// Asks first, as t3code does: logging out affects every thread with the agent.
    fn confirm_logout(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        let Some(account) = self.account() else {
            return;
        };
        let agent_name = account.connection.read(cx).agent_name().clone();
        let page = cx.weak_entity();
        cx.emit(SettingsPageEvent::Confirm(ConfirmRequest::logout(
            &agent_name,
            move |_, cx| {
                page.update(cx, |page, cx| {
                    if let Some(account) = page.account_mut() {
                        account.changing_account = false;
                        account
                            .connection
                            .update(cx, |connection, cx| connection.logout(cx));
                    }
                })
                .log_err();
            },
        )));
    }

    fn confirm_uninstall(
        &mut self,
        id: &AgentId,
        name: &SharedString,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let registry = self.registry(cx);
        let id = id.clone();
        let page = cx.weak_entity();
        cx.emit(SettingsPageEvent::Confirm(ConfirmRequest {
            icon: IconName::Trash,
            title: format!("Uninstall {name}?").into(),
            message: "Threads that use it can't continue until it's installed again.".into(),
            confirm_label: "Uninstall".into(),
            on_confirm: Rc::new(move |window, cx| {
                registry.update(cx, |registry, cx| registry.uninstall(&id, cx));
                page.update(cx, |page, cx| {
                    if page.account().is_some_and(|panel| panel.agent_id == id) {
                        page.show_agents_page(AgentsPage::Installed, window, cx);
                    }
                })
                .log_err();
            }),
        }));
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
        let icon = self.machines.read(cx).machine_icon(current, cx);
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
        let trigger = h_flex()
            .gap_1p5()
            .child(Icon::new(icon).size(IconSize::Small).color(Color::Muted))
            .child(Label::new(label))
            .into_any_element();
        DropdownMenu::new_with_element("agents-machine", trigger, menu)
            .trigger_tooltip(Tooltip::text(
                "Each machine installs and runs its own agents.",
            ))
            .into_any_element()
    }

    fn set_agents_machine(&mut self, machine: MachineId, cx: &mut Context<Self>) {
        if self.agents_machine == machine {
            return;
        }
        self.agents_machine = machine;
        // Only the installed agents and the registry offer the picker.
        if let AgentsPage::Agent(_) = self.agents_page {
            self.agents_page = AgentsPage::Installed;
        }
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
        // t3code's header: Update All for the outdated servers, and Add.
        let updatable: Vec<Entity<ServerClient>> = self
            .machines
            .read(cx)
            .clients()
            .into_iter()
            .filter(|client| client.read(cx).is_outdated())
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

    /// t3code's `EnvironmentRow`: the machine's icon (which opens the icon picker), its name
    /// over one line of how it's reached, its status and (when there's an update) its server's
    /// version, then its actions as buttons and another machine's switch. A switched-off row
    /// dims.
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
                    MachineStatus::Connecting => match client.upload_progress() {
                        Some(progress) => (
                            format!("Uploading agentz-server · {}%", progress.percent()),
                            false,
                        ),
                        None => ("Connecting".into(), false),
                    },
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
        let subtitle: SharedString = [Some(transport), Some(status), server_version]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
            .join(" · ")
            .into();
        let is_online = client
            .as_ref()
            .is_some_and(|client| client.read(cx).is_online());
        let is_outdated = client
            .as_ref()
            .is_some_and(|client| client.read(cx).is_outdated());
        let id_suffix = machine.slug();
        let element_id = |action: &str| SharedString::from(format!("machine-{action}-{id_suffix}"));
        let current_icon = self.machines.read(cx).machine_icon(machine, cx);

        // Updating hands the terminals to the new server, or restarts an older one; either
        // asks first only when something running would stop.
        let update_button = client
            .clone()
            .filter(|_| is_online && is_outdated)
            .map(|client| {
                IconButton::new(element_id("update"), IconName::CircleArrowUp)
                    .icon_size(IconSize::Small)
                    .icon_color(Color::Muted)
                    .tooltip(Tooltip::text("Update Server"))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.update_server(client.clone(), window, cx)
                    }))
            });
        // Another machine's switch connects to it or not; Remove… is its own button.
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
        let retry_button = client
            .clone()
            .filter(|client| !client.read(cx).is_online())
            .map(|client| {
                IconButton::new(element_id("retry"), IconName::RotateCw)
                    .icon_size(IconSize::Small)
                    .icon_color(Color::Muted)
                    .tooltip(Tooltip::text("Retry Now"))
                    .on_click(move |_, _, cx| client.update(cx, |client, _| client.retry()))
            });
        let restart_button = (profile.is_none() && is_online).then(|| {
            IconButton::new(element_id("restart"), IconName::ArrowCircle)
                .icon_size(IconSize::Small)
                .icon_color(Color::Muted)
                .tooltip(Tooltip::text("Restart Server…"))
                .on_click(
                    cx.listener(|this, _, window, cx| this.confirm_restart_server(window, cx)),
                )
        });
        let edit_button = profile.map(|profile| {
            let profile = profile.clone();
            IconButton::new(element_id("edit"), IconName::Pencil)
                .icon_size(IconSize::Small)
                .icon_color(Color::Muted)
                .tooltip(Tooltip::text("Edit…"))
                .on_click(cx.listener(move |_, _, _, cx| {
                    cx.emit(SettingsPageEvent::EditMachine(Some(profile.clone())))
                }))
        });
        let remove_button = profile.map(|profile| {
            let id = profile.id;
            let name = label.clone();
            IconButton::new(element_id("remove"), IconName::Trash)
                .icon_size(IconSize::Small)
                .icon_color(Color::Muted)
                .tooltip(Tooltip::text("Remove…"))
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.confirm_remove_machine(id, name.clone(), window, cx)
                }))
        });
        let icon_picker = PopoverMenu::new(element_id("icon"))
            .menu(move |window, cx| {
                let client = client.clone();
                Some(cx.new(|cx| MachineIconPicker::new(client, window, cx)))
            })
            .trigger_with_tooltip(
                IconButton::new(element_id("icon-trigger"), current_icon)
                    .icon_size(IconSize::Medium)
                    .icon_color(Color::Muted),
                Tooltip::text("Change Icon"),
            )
            .anchor(gpui::Anchor::TopLeft)
            .offset(gpui::point(px(0.), px(4.)));
        h_flex()
            .px_4()
            .py_2p5()
            .gap_3()
            .when(!is_enabled, |row| row.opacity(0.6))
            .child(icon_picker)
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .child(Label::new(label).truncate())
                    .child(
                        Label::new(subtitle)
                            .size(LabelSize::Small)
                            .color(if is_error { Color::Error } else { Color::Muted })
                            .truncate(),
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
                    .children(retry_button)
                    .children(restart_button)
                    .children(edit_button)
                    .children(remove_button)
                    .children(switch),
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

/// The tabs of an agent's page.
#[derive(Clone, Copy, PartialEq, Eq)]
enum AgentTab {
    Account,
    Defaults,
    Environment,
    Threads,
}

/// What the Threads tab knows of the agent's sessions.
enum SessionList {
    Listing { _task: Task<()> },
    Listed(AgentSessions),
    Failed(SharedString),
}

/// Where an agent's account stands, for the badge beside its name and its account card.
#[derive(Clone, Copy, PartialEq, Eq)]
enum AccountState {
    Connecting,
    Failed,
    LoggingIn,
    LoggedIn,
    LoggedOut,
}

impl AccountState {
    fn of(connection: &AgentThread) -> Self {
        if let ConnectionStatus::Failed(_) = connection.status() {
            return Self::Failed;
        }
        if connection.is_authenticating() {
            return Self::LoggingIn;
        }
        match connection.logged_in() {
            Some(true) => Self::LoggedIn,
            Some(false) => Self::LoggedOut,
            None => Self::Connecting,
        }
    }
}

struct AccountPanel {
    agent_id: AgentId,
    tab: AgentTab,
    /// A session-less connection to the agent, alive only while the panel is open.
    connection: Entity<AgentThread>,
    login: Entity<AgentLogin>,
    elicitation_cards: Vec<Entity<ElicitationCard>>,
    /// The user asked to log in to another account while logged in.
    changing_account: bool,
    /// Whether the connection was logging in when last seen, to notice when it's done.
    was_authenticating: bool,
    env_rows: Vec<EnvRow>,
    /// Listed when the Threads tab first opens; `None` before.
    sessions: Option<SessionList>,
    /// The project whose sessions the Threads tab shows, once the user picks one.
    sessions_project: Option<ProjectId>,
    sessions_shown: usize,
    /// Sessions whose import hasn't been answered yet.
    importing: HashSet<String>,
    import_error: Option<SharedString>,
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

/// "Logged in as …" with the account the agent reported (Claude Agent and Codex report one),
/// else the method last used from agentZ.
fn logged_in_title(status: Option<&AuthStatus>, login_method: Option<&str>) -> SharedString {
    if let Some(status) = status {
        if let Some(email) = status
            .account
            .as_ref()
            .and_then(|account| account.email.as_deref())
        {
            return format!("Logged in as {email}").into();
        }
        if let Some(label) = &status.label {
            return format!("Logged in with {label}").into();
        }
    }
    match login_method.and_then(login_method_subject) {
        Some(subject) => format!("Logged in with {subject}").into(),
        None => "Logged in".into(),
    }
}

/// The rest of what the agent said about the account: how it's logged in (when the title
/// shows the email), the plan, the organization, and any detail.
fn account_details(status: &AuthStatus) -> Vec<String> {
    let account = status.account.clone().unwrap_or_default();
    let label = status.label.clone().filter(|_| account.email.is_some());
    [
        label,
        account.plan,
        account.organization,
        status.detail.clone(),
    ]
    .into_iter()
    .flatten()
    .filter(|detail| !detail.trim().is_empty())
    .collect()
}

/// A version as t3code shows it: a bare number gets a "v", anything else (a custom agent's
/// "custom") stays as it is.
fn version_label(version: &str) -> String {
    if version.starts_with(|character: char| character.is_ascii_digit()) {
        format!("v{version}")
    } else {
        version.to_string()
    }
}

/// The installed version, with the registry's newer one when there is one, and whether there
/// is.
fn installed_version(agent: &AgentListing) -> Option<(SharedString, bool)> {
    let InstallState::Installed {
        version,
        update_available,
    } = &agent.install_state
    else {
        return None;
    };
    let label = if *update_available {
        format!(
            "{} · {} available",
            version_label(version),
            version_label(agent.version())
        )
    } else {
        version_label(version)
    };
    Some((label.into(), *update_available))
}

/// Whether an agent belongs with the installed ones. An agent being updated reports
/// Installing, so an agent being installed for the first time joins them a little early.
fn counts_as_installed(state: &InstallState) -> bool {
    matches!(
        state,
        InstallState::Installed { .. } | InstallState::Installing
    )
}

/// An agent's icon at full contrast on a neutral tile. The ACP Registry's icons are drawn in
/// `currentColor`, so they take the text color.
fn render_agent_tile(agent: &AgentId, size: Pixels, cx: &App) -> AnyElement {
    let colors = cx.theme().colors();
    let icon = match agent_icon(agent, cx) {
        Some(markup) => Icon::from_svg_markup(markup),
        None => Icon::new(IconName::Sparkle),
    };
    div()
        .size(size)
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .rounded_md()
        .border_1()
        .border_color(colors.border_variant)
        .bg(colors.element_background)
        .child(
            icon.size(IconSize::Custom(rems_from_px(f32::from(size) / 2.)))
                .color(Color::Default),
        )
        .into_any_element()
}

/// Zed's links from an agent's registry entry: its repository, website and license.
fn render_agent_links(agent: &AgentListing) -> Vec<AnyElement> {
    let metadata = &agent.metadata;
    [
        (
            metadata.repository.clone(),
            IconName::Github,
            "repository",
            "Visit Agent Repository",
        ),
        (
            metadata.website.clone(),
            IconName::Link,
            "website",
            "Visit Agent Website",
        ),
        (
            metadata.license_url.clone(),
            IconName::FileTextOutlined,
            "license",
            "View Agent License or Terms of Service",
        ),
    ]
    .into_iter()
    .filter_map(|(url, icon, kind, title)| {
        let url = url?;
        let tooltip_url = url.clone();
        Some(
            IconButton::new(
                SharedString::from(format!("agent-{kind}-{}", agent.id().0)),
                icon,
            )
            .icon_size(IconSize::Small)
            .icon_color(Color::Muted)
            .tooltip(move |_, cx| Tooltip::with_meta(title, None, tooltip_url.clone(), cx))
            .on_click(move |_, _, cx| cx.open_url(&url))
            .into_any_element(),
        )
    })
    .collect()
}

/// The same links as words, for the line under an agent's name on its page.
fn render_agent_text_links(agent: &AgentListing, cx: &App) -> Vec<AnyElement> {
    let accent = cx.theme().colors().text_accent;
    let metadata = &agent.metadata;
    [
        (metadata.repository.clone(), "Repository"),
        (metadata.website.clone(), "Website"),
        (metadata.license_url.clone(), "License"),
    ]
    .into_iter()
    .filter_map(|(url, label)| {
        let url = url?;
        let tooltip = url.clone();
        Some(
            div()
                .id(SharedString::from(format!(
                    "agent-link-{}",
                    label.to_lowercase()
                )))
                .flex_none()
                .whitespace_nowrap()
                .text_color(accent)
                .cursor_pointer()
                .hover(|style| style.underline())
                .tooltip(Tooltip::text(tooltip))
                .on_click(move |_, _, cx| cx.open_url(&url))
                .child(label)
                .into_any_element(),
        )
    })
    .collect()
}

/// The account card's first row: a status dot or spinner, what it means and why, and what to
/// do about it.
fn render_status_row(
    indicator: AnyElement,
    title: SharedString,
    subtitle: Option<(SharedString, Color)>,
    actions: Vec<AnyElement>,
) -> AnyElement {
    h_flex()
        .px_4()
        .py_3()
        .gap_3()
        .child(div().w(px(7.)).flex().justify_center().child(indicator))
        .child(
            v_flex()
                .flex_1()
                .min_w_0()
                .gap_0p5()
                .child(Label::new(title))
                .children(subtitle.map(|(subtitle, color)| {
                    Label::new(subtitle).size(LabelSize::Small).color(color)
                })),
        )
        .children(actions)
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
    v_flex()
        .gap_2()
        .child(
            h_flex()
                .justify_between()
                .child(Label::new(title).size(LabelSize::Small).color(Color::Muted))
                .child(actions),
        )
        .child(render_rows(rows, cx))
        .into_any_element()
}

/// A section's bordered group of rows, divided by lines.
fn render_rows(rows: Vec<AnyElement>, cx: &App) -> AnyElement {
    let colors = cx.theme().colors().clone();
    let count = rows.len();
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
        }))
        .into_any_element()
}

/// The Threads tab's last row while sessions are hidden, as the sidebar's archived threads
/// show more.
fn render_show_more_sessions(hidden: usize, cx: &mut Context<SettingsPage>) -> AnyElement {
    h_flex()
        .id("sessions-show-more")
        .debug_selector(|| "sessions-show-more".to_string())
        .px_4()
        .py_2()
        .gap_2()
        .cursor_pointer()
        .hover(|row| row.bg(cx.theme().colors().ghost_element_hover))
        .child(
            Icon::new(IconName::Plus)
                .size(IconSize::Small)
                .color(Color::Muted),
        )
        .child(
            Label::new(format!("Show {} more", hidden.min(SESSIONS_PAGE_COUNT)))
                .color(Color::Muted),
        )
        .on_click(cx.listener(|this, _, _, cx| {
            if let Some(panel) = this.account_mut() {
                panel.sessions_shown += SESSIONS_PAGE_COUNT;
            }
            cx.notify();
        }))
        .into_any_element()
}

/// A section with a note under its rows.
fn render_section_with_note(
    title: &'static str,
    rows: Vec<AnyElement>,
    note: impl Into<SharedString>,
    cx: &App,
) -> AnyElement {
    v_flex()
        .gap_2()
        .child(render_section(title, rows, cx))
        .child(
            Label::new(note.into())
                .size(LabelSize::Small)
                .color(Color::Muted),
        )
        .into_any_element()
}

/// A setting's title and description on the left, its control on the right.
fn render_row(
    title: impl Into<SharedString>,
    description: impl Into<SharedString>,
    control: AnyElement,
    _cx: &App,
) -> AnyElement {
    let description = description.into();
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
                .when(!description.is_empty(), |column| {
                    column.child(
                        Label::new(description)
                            .size(LabelSize::Small)
                            .color(Color::Muted),
                    )
                }),
        )
        .child(div().flex_none().child(control))
        .into_any_element()
}

impl Render for SettingsPage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = cx.theme().colors().clone();
        let headline = |title: SharedString| {
            Headline::new(title)
                .size(HeadlineSize::Small)
                .into_any_element()
        };
        let (header, sections) = match self.section {
            Section::General => (headline("General".into()), self.render_general(window, cx)),
            Section::Appearance => (
                headline("Appearance".into()),
                self.render_appearance(window, cx),
            ),
            Section::Agents => (
                self.render_agents_header(window, cx),
                self.render_agents(window, cx),
            ),
            Section::Machines => (headline("Machines".into()), self.render_machines(cx)),
            Section::Project(key) => match self.project(key, cx) {
                Some(project) => (
                    headline(project.name()),
                    self.render_project(key.machine, project, window, cx),
                ),
                None => (headline("General".into()), self.render_general(window, cx)),
            },
        };
        h_flex()
            .key_context(KEY_CONTEXT)
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(|_, _: &CloseSettings, _, cx| cx.emit(SettingsPageEvent::Close)))
            .size_full()
            .bg(colors.editor_background)
            .child(self.render_nav(window, cx))
            .map(|page| {
                if self.section == Section::Agents
                    && let AgentsPage::Registry = self.agents_page
                {
                    return page.child(self.render_registry(header, window, cx));
                }
                page.child(
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
                                        .child(header)
                                        .children(sections),
                                ),
                        )
                        .vertical_scrollbar_for(&self.content_scroll, window, cx),
                )
            })
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use agentz_protocol::agents::{RegistryAgentMetadata, RegistrySnapshot};
    use agentz_protocol::spaces::SpacesSnapshot;
    use gpui::TestAppContext;
    use projects::ImportedSession;

    use super::*;
    use crate::server_client::ServerClient;

    fn listing(id: &str, name: &str, install_state: InstallState) -> AgentListing {
        AgentListing {
            metadata: RegistryAgentMetadata {
                id: AgentId::new(id.to_string()),
                name: name.to_string().into(),
                description: format!("{name}, for tests").into(),
                version: "2.0.0".into(),
                repository: None,
                website: None,
                license_url: None,
                icon: None,
            },
            supports_current_platform: true,
            install_state,
        }
    }

    fn agent_page_id(
        page: &Entity<SettingsPage>,
        cx: &mut gpui::VisualTestContext,
    ) -> Option<String> {
        page.read_with(cx, |page, _| {
            page.account().map(|panel| panel.agent_id.0.to_string())
        })
    }

    #[gpui::test]
    fn installed_agents_open_their_own_page_and_the_registry_lists_the_rest(
        cx: &mut TestAppContext,
    ) {
        cx.update(|cx| {
            crate::init_for_test(cx);
            let client = ServerClient::new_for_test(
                MachineId::Local,
                "This Mac".into(),
                SpacesSnapshot::default(),
                cx,
            );
            let registry = client.read(cx).registry().clone();
            registry.update(cx, |registry, cx| {
                registry.set_snapshot(
                    RegistrySnapshot {
                        agents: vec![
                            listing(
                                "codex",
                                "Codex",
                                InstallState::Installed {
                                    version: "1.0.0".into(),
                                    update_available: true,
                                },
                            ),
                            listing(
                                "claude",
                                "Claude Agent",
                                InstallState::Installed {
                                    version: "2.0.0".into(),
                                    update_available: false,
                                },
                            ),
                            listing("gemini", "Gemini CLI", InstallState::NotInstalled),
                        ],
                        is_fetching: false,
                        fetch_error: None,
                    },
                    cx,
                )
            });
            crate::machines::init_for_test(vec![client], cx);
            crate::project_info::init(cx);
        });
        let (page, cx) = cx.add_window_view(|_, cx| SettingsPage::new(cx));
        page.update_in(cx, |page, window, cx| page.show_agents(window, cx));
        cx.run_until_parked();

        // Only the installed agents are listed, sorted by name.
        let claude = cx
            .debug_bounds("agent-row-claude")
            .expect("Claude is listed");
        let codex = cx.debug_bounds("agent-row-codex").expect("Codex is listed");
        assert!(claude.top() < codex.top());
        assert!(cx.debug_bounds("agent-row-gemini").is_none());

        // A row opens the agent's page at its account, and the tabs switch what it shows.
        cx.simulate_click(claude.center(), gpui::Modifiers::none());
        assert_eq!(agent_page_id(&page, cx).as_deref(), Some("claude"));
        assert!(cx.debug_bounds("agent-row-codex").is_none());
        assert!(cx.debug_bounds("account-card").is_some());
        let defaults = cx
            .debug_bounds("agent-tab-defaults")
            .expect("the page has tabs");
        cx.simulate_click(defaults.center(), gpui::Modifiers::none());
        assert!(cx.debug_bounds("account-card").is_none());
        let account = cx
            .debug_bounds("agent-tab-account")
            .expect("the page has tabs");
        cx.simulate_click(account.center(), gpui::Modifiers::none());
        assert!(cx.debug_bounds("account-card").is_some());
        let back = cx
            .debug_bounds("agents-back")
            .expect("a sub-page has a back button");
        cx.simulate_click(back.center(), gpui::Modifiers::none());
        assert_eq!(agent_page_id(&page, cx), None);
        assert!(cx.debug_bounds("agent-row-codex").is_some());

        // The registry lists every agent, and typing there searches it.
        page.update_in(cx, |page, window, cx| {
            page.show_agents_page(AgentsPage::Registry, window, cx)
        });
        cx.run_until_parked();
        for card in [
            "registry-card-claude",
            "registry-card-codex",
            "registry-card-gemini",
        ] {
            assert!(cx.debug_bounds(card).is_some(), "{card} is listed");
        }
        cx.simulate_input("gem");
        cx.run_until_parked();
        assert!(cx.debug_bounds("registry-card-gemini").is_some());
        assert!(cx.debug_bounds("registry-card-claude").is_none());

        // Not Installed hides the installed agents even without a search.
        page.update_in(cx, |page, window, cx| {
            page.show_agents_page(AgentsPage::Registry, window, cx);
            page.set_registry_filter(RegistryFilter::NotInstalled, cx);
        });
        cx.run_until_parked();
        assert!(cx.debug_bounds("registry-card-gemini").is_some());
        assert!(cx.debug_bounds("registry-card-codex").is_none());

        // Leaving Agents and coming back starts at the installed agents again.
        page.update_in(cx, |page, window, cx| {
            page.select(Section::General, window, cx);
            page.select(Section::Agents, window, cx);
        });
        cx.run_until_parked();
        assert!(cx.debug_bounds("agent-row-claude").is_some());
    }

    #[gpui::test]
    fn the_registry_builds_only_the_cards_in_view(cx: &mut TestAppContext) {
        cx.update(|cx| {
            crate::init_for_test(cx);
            let client = ServerClient::new_for_test(
                MachineId::Local,
                "This Mac".into(),
                SpacesSnapshot::default(),
                cx,
            );
            let registry = client.read(cx).registry().clone();
            registry.update(cx, |registry, cx| {
                registry.set_snapshot(
                    RegistrySnapshot {
                        agents: (100..300)
                            .map(|number| {
                                listing(
                                    &format!("agent-{number}"),
                                    &format!("Agent {number}"),
                                    InstallState::NotInstalled,
                                )
                            })
                            .collect(),
                        is_fetching: false,
                        fetch_error: None,
                    },
                    cx,
                )
            });
            crate::machines::init_for_test(vec![client], cx);
            crate::project_info::init(cx);
        });
        let (page, cx) = cx.add_window_view(|_, cx| SettingsPage::new(cx));
        page.update_in(cx, |page, window, cx| {
            page.show_agents(window, cx);
            page.show_agents_page(AgentsPage::Registry, window, cx);
        });
        cx.run_until_parked();

        // Scrolling re-renders the page every frame, so building all 200 cards made it lag.
        assert!(cx.debug_bounds("registry-card-agent-100").is_some());
        assert!(cx.debug_bounds("registry-card-agent-299").is_none());
    }

    fn agent_session(id: &str, project_id: Option<ProjectId>, hours_ago: u64) -> AgentSession {
        AgentSession {
            session_id: id.to_string(),
            cwd: "/tmp/somewhere".into(),
            title: Some(format!("Session {id}")),
            updated_at: Some(SystemTime::now() - Duration::from_secs(hours_ago * 3600)),
            project_id,
            workspace: None,
            thread_id: None,
        }
    }

    #[gpui::test]
    fn the_threads_tab_shows_a_projects_sessions_to_import_or_open(cx: &mut TestAppContext) {
        let empty_dir = tempfile::tempdir().expect("temp dir");
        let project_dir = tempfile::tempdir().expect("temp dir");
        let mut store = projects::ProjectStore::load(None);
        let empty = store.add_project(empty_dir.path().to_path_buf());
        let project = store.add_project(project_dir.path().to_path_buf());
        let thread = store
            .add_imported_thread(ImportedSession {
                project_id: project,
                workspace: None,
                agent_id: "mock".into(),
                session_id: "s-03".into(),
                title: "Session s-03".into(),
                updated_at: None,
                archived: true,
            })
            .expect("a thread");
        let snapshot = store.snapshot();
        cx.update(|cx| {
            crate::init_for_test(cx);
            let client = ServerClient::new_for_test(
                MachineId::Local,
                "This Mac".into(),
                SpacesSnapshot::default(),
                cx,
            );
            client
                .read(cx)
                .registry()
                .clone()
                .update(cx, |registry, cx| {
                    registry.set_snapshot(
                        RegistrySnapshot {
                            agents: vec![listing(
                                "mock",
                                "Mock",
                                InstallState::Installed {
                                    version: "2.0.0".into(),
                                    update_available: false,
                                },
                            )],
                            is_fetching: false,
                            fetch_error: None,
                        },
                        cx,
                    )
                });
            client
                .read(cx)
                .projects()
                .clone()
                .update(cx, |store, cx| store.set_snapshot(snapshot, cx));
            crate::machines::init_for_test(vec![client], cx);
            crate::project_info::init(cx);
        });
        let (page, cx) = cx.add_window_view(|_, cx| SettingsPage::new(cx));
        let opened = Rc::new(std::cell::RefCell::new(Vec::new()));
        cx.update(|_, cx| {
            let opened = opened.clone();
            cx.subscribe(&page, move |_, event: &SettingsPageEvent, _| {
                if let SettingsPageEvent::OpenThread(thread) = event {
                    opened.borrow_mut().push(*thread);
                }
            })
            .detach();
        });
        page.update_in(cx, |page, window, cx| page.show_agents(window, cx));
        cx.run_until_parked();
        let row = cx
            .debug_bounds("agent-row-mock")
            .expect("the agent is listed");
        cx.simulate_click(row.center(), gpui::Modifiers::none());
        let tab = cx
            .debug_bounds("agent-tab-threads")
            .expect("the page has a Threads tab");
        cx.simulate_click(tab.center(), gpui::Modifiers::none());
        cx.run_until_parked();
        // Opening the tab lists the sessions, which fails without a server.
        let sessions = |page: &Entity<SettingsPage>, cx: &mut gpui::VisualTestContext| {
            page.read_with(cx, |page, _| match page.account()?.sessions.as_ref()? {
                SessionList::Listing { .. } => Some("listing"),
                SessionList::Listed(_) => Some("listed"),
                SessionList::Failed(_) => Some("failed"),
            })
        };
        assert_eq!(sessions(&page, cx), Some("failed"));

        // Listed oldest first, and one in a folder that isn't a project.
        let mut listed: Vec<AgentSession> = (0..14)
            .rev()
            .map(|number| agent_session(&format!("s-{number:02}"), Some(project), number))
            .collect();
        listed.push(agent_session("elsewhere", None, 0));
        page.update(cx, |page, cx| {
            if let Some(panel) = page.account_mut() {
                panel.sessions = Some(SessionList::Listed(AgentSessions::Listed(listed)));
            }
            cx.notify();
        });
        cx.run_until_parked();

        // The first project with sessions shows, newest first, ten at a time.
        let newest = cx
            .debug_bounds("agent-session-s-00")
            .expect("the newest session shows");
        let next = cx
            .debug_bounds("agent-session-s-01")
            .expect("the next one shows");
        assert!(newest.top() < next.top());
        assert!(cx.debug_bounds("agent-session-s-09").is_some());
        assert!(cx.debug_bounds("agent-session-s-10").is_none());
        assert!(cx.debug_bounds("agent-session-elsewhere").is_none());
        let more = cx
            .debug_bounds("sessions-show-more")
            .expect("the rest are a click away");
        cx.simulate_click(more.center(), gpui::Modifiers::none());
        cx.run_until_parked();
        assert!(cx.debug_bounds("agent-session-s-13").is_some());
        assert!(cx.debug_bounds("sessions-show-more").is_none());

        // A session agentZ has opens its thread; the others import.
        assert!(cx.debug_bounds("session-import-s-00").is_some());
        let open = cx
            .debug_bounds("session-open-s-03")
            .expect("the imported session opens");
        cx.simulate_click(open.center(), gpui::Modifiers::none());
        assert_eq!(
            *opened.borrow(),
            [ThreadKey {
                machine: MachineId::Local,
                thread,
            }]
        );
        let import = cx
            .debug_bounds("session-import-s-00")
            .expect("the session imports");
        cx.simulate_click(import.center(), gpui::Modifiers::none());
        cx.run_until_parked();
        let import_error = page.read_with(cx, |page, _| {
            page.account().and_then(|panel| panel.import_error.clone())
        });
        assert!(
            import_error.is_some_and(|error| error.contains("not connected")),
            "a failed import says why"
        );
        assert!(cx.debug_bounds("session-import-s-00").is_some());

        // Another project shows its own sessions.
        page.update(cx, |page, cx| {
            if let Some(panel) = page.account_mut() {
                panel.sessions_project = Some(empty);
            }
            cx.notify();
        });
        cx.run_until_parked();
        assert!(cx.debug_bounds("agent-session-s-00").is_none());
    }

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
