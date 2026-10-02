//! The settings page, laid out like t3code's: a list of sections on the left (General,
//! Appearance, then one entry per project) and the chosen section's rows on the right.

use collections::HashMap;
use gpui::{
    AnyElement, App, Context, Entity, EventEmitter, FocusHandle, Focusable, KeyBinding,
    PathPromptOptions, PromptLevel, ScrollHandle, Subscription, Window, actions,
};
use projects::{Project, ProjectIcon, ProjectId, ProjectStore, ThreadOrder};
use registry::{AgentId, AgentRegistryStore, InstallState};
use text_input::{TextInput, TextInputEvent};
use theme::{Appearance, ThemeRegistry};
use ui::{ContextMenu, DropdownMenu, IconPosition, Tooltip, WithScrollbar as _, prelude::*};

use crate::app_settings::{AppSettingsStore, ThemeMode};
use crate::project_info::{
    MONOGRAM_COLORS, ProjectInfo, ProjectInfoStore, automatic_monogram, monogram_swatch,
    render_project_icon,
};
use crate::sidebar::{SIDEBAR_WIDTH, render_footer_item};

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
}

#[derive(Clone, Copy, PartialEq)]
enum Section {
    General,
    Appearance,
    Agents,
    Project(ProjectId),
}

pub struct SettingsPage {
    focus_handle: FocusHandle,
    store: Entity<ProjectStore>,
    app_settings: Entity<AppSettingsStore>,
    section: Section,
    name_input: Entity<TextInput>,
    monogram_input: Entity<TextInput>,
    registry: Entity<AgentRegistryStore>,
    agent_search: Entity<TextInput>,
    nav_scroll: ScrollHandle,
    content_scroll: ScrollHandle,
    /// Detected favicons, so automatic icons match the sidebar's.
    project_info: HashMap<ProjectId, ProjectInfo>,
    _subscriptions: Vec<Subscription>,
}

impl EventEmitter<SettingsPageEvent> for SettingsPage {}

impl Focusable for SettingsPage {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl SettingsPage {
    pub fn new(store: Entity<ProjectStore>, cx: &mut Context<Self>) -> Self {
        let app_settings = AppSettingsStore::global(cx);
        let name_input = cx.new(|cx| TextInput::new("", cx));
        let monogram_input = cx.new(|cx| TextInput::new("", cx));
        let mut subscriptions = vec![
            cx.observe(&store, |this, _, cx| {
                // A removed project's page has nothing left to show.
                if let Section::Project(id) = this.section
                    && this.store.read(cx).project(id).is_none()
                {
                    this.section = Section::General;
                }
                cx.notify();
            }),
            cx.observe(&app_settings, |_, _, cx| cx.notify()),
            cx.subscribe(&name_input, |this, input, _: &TextInputEvent, cx| {
                let Section::Project(id) = this.section else {
                    return;
                };
                let name = input.read(cx).text().to_string();
                this.store
                    .update(cx, |store, cx| store.set_project_name(id, &name, cx));
            }),
            cx.subscribe(&monogram_input, |this, input, _: &TextInputEvent, cx| {
                let text = input.read(cx).text().trim().to_string();
                if !text.is_empty() {
                    this.set_monogram(Some(text), None, cx);
                }
            }),
        ];
        let registry = AgentRegistryStore::global(cx);
        let agent_search = cx.new(|cx| TextInput::new("Search agents…", cx));
        subscriptions.push(cx.observe(&registry, |_, _, cx| cx.notify()));
        subscriptions.push(cx.subscribe(&agent_search, |_, _, _: &TextInputEvent, cx| cx.notify()));
        let project_info_store = ProjectInfoStore::global(cx);
        let project_info = project_info_store.read(cx).info().clone();
        subscriptions.push(cx.observe(&project_info_store, |this, store, cx| {
            this.project_info = store.read(cx).info().clone();
            cx.notify();
        }));
        Self {
            focus_handle: cx.focus_handle(),
            store,
            app_settings,
            section: Section::General,
            name_input,
            monogram_input,
            registry,
            agent_search,
            nav_scroll: ScrollHandle::new(),
            content_scroll: ScrollHandle::new(),
            project_info,
            _subscriptions: subscriptions,
        }
    }

    pub fn show_agents(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.select(Section::Agents, window, cx);
    }

    pub fn show_project(&mut self, id: ProjectId, window: &mut Window, cx: &mut Context<Self>) {
        self.select(Section::Project(id), window, cx);
    }

    fn select(&mut self, section: Section, window: &mut Window, cx: &mut Context<Self>) {
        if self.section != section {
            self.content_scroll.set_offset(gpui::point(px(0.), px(0.)));
        }
        self.section = section;
        if section == Section::Agents {
            self.registry
                .update(cx, |registry, cx| registry.refresh_if_stale(cx));
            // Typing on the Agents page searches it.
            window.focus(&self.agent_search.focus_handle(cx), cx);
        }
        if let Section::Project(id) = section
            && let Some(project) = self.store.read(cx).project(id).cloned()
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
        let Section::Project(id) = self.section else {
            return;
        };
        let Some(project) = self.store.read(cx).project(id) else {
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
        self.store
            .update(cx, |store, cx| store.set_project_icon(id, Some(icon), cx));
    }

    fn choose_icon_file(&mut self, id: ProjectId, cx: &mut Context<Self>) {
        let paths = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Use as Icon".into()),
        });
        let store = self.store.clone();
        cx.spawn(async move |_, cx| {
            let path = match paths.await {
                Ok(Ok(Some(paths))) => paths.into_iter().next(),
                Ok(Ok(None)) | Err(_) => None,
                Ok(Err(error)) => {
                    log::error!("failed to pick an icon file: {error:#}");
                    None
                }
            };
            if let Some(path) = path {
                store.update(cx, |store, cx| {
                    store.set_project_icon(id, Some(ProjectIcon::Image { path }), cx)
                });
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
        let store = self.store.clone();
        let id = project.id;
        cx.spawn(async move |_, cx| {
            if answer.await == Ok(0) {
                store.update(cx, |store, cx| store.remove_project(id, cx));
            }
        })
        .detach();
    }

    fn render_nav(&self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = cx.theme().colors().clone();
        let projects: Vec<Project> = self.store.read(cx).projects().to_vec();
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
        ];
        let mut project_items = Vec::with_capacity(projects.len());
        for project in &projects {
            let icon =
                render_project_icon(project, self.project_info.get(&project.id), px(14.), cx);
            project_items.push(self.render_nav_item(
                project.name(),
                None,
                Some(icon),
                Section::Project(project.id),
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
                h_flex()
                    .h(px(40.))
                    .flex_none()
                    .pl_3()
                    .pr_2()
                    .justify_between()
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
                            .children(items.drain(..3))
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
                |_, cx| Tooltip::for_action("Back", &CloseSettings, cx),
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
            Section::Project(id) => format!("settings-nav-project-{}", id.0).into(),
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
        let current = self.store.read(cx).thread_order();
        let store = self.store.clone();
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
        vec![render_section(
            "Threads",
            vec![render_row(
                "Thread order",
                "How threads are sorted in the sidebar.",
                DropdownMenu::new("thread-order", label, menu).into_any_element(),
                cx,
            )],
            cx,
        )]
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
    fn render_agents(&self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let colors = cx.theme().colors().clone();
        let query = self.agent_search.read(cx).text().trim().to_lowercase();
        let registry = self.registry.read(cx);
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
        if !installed.is_empty() {
            let rows = installed
                .iter()
                .map(|id| self.render_agent_row(id, cx))
                .collect();
            sections.push(render_section("Installed", rows, cx));
        }
        if !available.is_empty() {
            let rows = available
                .iter()
                .map(|id| self.render_agent_row(id, cx))
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
                                    this.registry
                                        .update(cx, |registry, cx| registry.refresh(cx))
                                })),
                        )
                    })
                    .into_any_element(),
            );
        }
        sections
    }

    fn render_agent_row(&self, id: &AgentId, cx: &mut Context<Self>) -> AnyElement {
        let registry = self.registry.read(cx);
        let Some(agent) = registry.agent(id) else {
            return div().into_any_element();
        };
        let name = agent.name().clone();
        let icon = match agent.icon_path() {
            Some(path) => Icon::from_external_svg(path.clone()),
            None => Icon::new(IconName::Terminal),
        };
        let element_id = |action: &str| SharedString::from(format!("agent-{action}-{}", id.0));
        let install = {
            let id = id.clone();
            cx.listener(move |this, _, _, cx| {
                this.registry
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
            .child(div().flex_none().child(controls))
            .into_any_element()
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
        let registry = self.registry.clone();
        let id = id.clone();
        cx.spawn(async move |_, cx| {
            if answer.await == Ok(0) {
                registry.update(cx, |registry, cx| registry.uninstall(&id, cx));
            }
        })
        .detach();
    }

    fn render_project(&self, project: Project, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let colors = cx.theme().colors().clone();
        let id = project.id;
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
                self.project_info.get(&id),
                px(24.),
                cx,
            ))
            .child(
                Button::new("choose-icon-file", "Choose File…")
                    .style(ButtonStyle::Outlined)
                    .on_click(cx.listener(move |this, _, _, cx| this.choose_icon_file(id, cx))),
            )
            .when(project.icon.is_some(), |this| {
                this.child(
                    Button::new("reset-icon", "Reset")
                        .style(ButtonStyle::Subtle)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.monogram_input
                                .update(cx, |input, cx| input.set_text("", cx));
                            this.store
                                .update(cx, |store, cx| store.set_project_icon(id, None, cx));
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
                        project.path.display().to_string(),
                        div().into_any_element(),
                        cx,
                    ),
                ],
                cx,
            ),
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
            ),
        ]
    }
}

/// t3code's settings section: a small heading over a bordered group of rows.
fn render_section(title: &'static str, rows: Vec<AnyElement>, cx: &App) -> AnyElement {
    let colors = cx.theme().colors().clone();
    let count = rows.len();
    v_flex()
        .gap_2()
        .child(Label::new(title).size(LabelSize::Small).color(Color::Muted))
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
            Section::Agents => ("Agents".into(), self.render_agents(cx)),
            Section::Project(id) => match self.store.read(cx).project(id).cloned() {
                Some(project) => (project.name(), self.render_project(project, cx)),
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
